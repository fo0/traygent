use super::{opencode, windows, AgentQuota};
use crate::{
    github::secret::{wipe, SecretToken},
    models::AgentId,
    profiles::files::without_bom,
};
use chrono::Utc;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::HeaderValue;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::time::Duration;

const MAX_AUTH_BYTES: u64 = 1024 * 1024;
const MAX_USAGE_BYTES: u64 = 256 * 1024;
const UNREADABLE: &str = "Die Anmeldedatei konnte nicht gelesen werden.";
const UNREACHABLE: &str = "Der Anbieter ist nicht erreichbar.";
const INVALID_RESPONSE: &str = "Die Kontingentdaten des Anbieters sind ungültig.";
const CODEX_CONFIG_UNREADABLE: &str =
    "Der Anmeldespeicher in der Codex-Konfiguration konnte nicht ermittelt werden.";
const CODEX_OTHER_STORE: &str =
    "Der konfigurierte Codex-Anmeldespeicher unterstützt keine Kontingentabfrage über auth.json.";

struct Login {
    token: SecretToken,
    account_id: Option<String>,
}

pub(super) fn fetch(agent: AgentId, path: &Path) -> Result<AgentQuota, &'static str> {
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("Traygent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| UNREACHABLE)?;
    let request = usage_request(&client, agent, login(agent, path)?)?;
    let response = request.send().map_err(|_| UNREACHABLE)?;
    match response.status().as_u16() {
        200..=299 => {}
        401 => return Err("Anmeldung abgelaufen oder ungültig. Bitte im CLI erneut anmelden."),
        403 if agent == AgentId::Opencode => {
            return Err("Zugriff verweigert. Bitte OpenCode-Go-Abo und Anmeldung prüfen.");
        }
        403 => return Err("Der Anbieter verweigert den Zugriff auf die Kontingente."),
        429 => return Err("Zu viele Abfragen. Bitte später erneut versuchen."),
        _ => return Err("Der Anbieter konnte die Kontingente nicht liefern."),
    }
    let headers = response.headers();
    let header_percent = |name: &str| headers.get(name)?.to_str().ok()?.parse::<f64>().ok();
    let codex_used = [
        header_percent("x-codex-primary-used-percent"),
        header_percent("x-codex-secondary-used-percent"),
    ];
    let bytes = read_bounded(response, MAX_USAGE_BYTES, 0).ok_or(INVALID_RESPONSE)?;
    let body = serde_json::from_slice(without_bom(&bytes)).map_err(|_| INVALID_RESPONSE)?;
    Ok(windows::parse(agent, &body, codex_used, Utc::now()))
}

fn login_required(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Claude => "Bitte in Claude Code anmelden.",
        AgentId::Codex => "Bitte in Codex mit einem ChatGPT-Konto anmelden.",
        AgentId::Opencode => "Bitte in OpenCode mit OpenCode Go anmelden.",
    }
}

fn login(agent: AgentId, path: &Path) -> Result<Login, &'static str> {
    if agent == AgentId::Opencode {
        return Ok(Login {
            token: opencode::go_key(path)?,
            account_id: None,
        });
    }
    if agent == AgentId::Codex {
        check_codex_store(path)?;
    }
    let mut bytes = read_local_file(path)
        .map_err(|_| UNREADABLE)?
        .ok_or_else(|| login_required(agent))?;
    let login = parse_cli_login(agent, &bytes);
    wipe(&mut bytes);
    login
}

fn check_codex_store(auth_path: &Path) -> Result<(), &'static str> {
    #[derive(Deserialize)]
    struct Config {
        cli_auth_credentials_store: Option<String>,
    }
    let Some(mut bytes) = read_local_file(&auth_path.with_file_name("config.toml"))
        .map_err(|_| CODEX_CONFIG_UNREADABLE)?
    else {
        return Ok(());
    };
    let config =
        toml::de::from_slice::<Config>(without_bom(&bytes)).map_err(|_| CODEX_CONFIG_UNREADABLE);
    wipe(&mut bytes);
    match config?.cli_auth_credentials_store.as_deref() {
        None | Some("file") => Ok(()),
        _ => Err(CODEX_OTHER_STORE),
    }
}

fn read_local_file(path: &Path) -> Result<Option<Vec<u8>>, ()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    if !meta.is_file() || meta.len() > MAX_AUTH_BYTES {
        return Err(());
    }
    let mut open = fs::OpenOptions::new();
    open.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        open.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = open.open(path).map_err(|_| ())?;
    let opened = file.metadata().map_err(|_| ())?;
    if !opened.is_file() || opened.len() > MAX_AUTH_BYTES {
        return Err(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (opened.dev(), opened.ino()) != (meta.dev(), meta.ino()) {
            return Err(());
        }
    }
    read_bounded(file, MAX_AUTH_BYTES, opened.len() as usize + 1)
        .map(Some)
        .ok_or(())
}

/// `expected` sizes the buffer up front: a buffer that grows leaves unwiped copies behind.
fn read_bounded(source: impl Read, limit: u64, expected: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(expected);
    if source.take(limit + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > limit {
        wipe(&mut bytes);
        return None;
    }
    Some(bytes)
}

fn parse_cli_login(agent: AgentId, auth: &[u8]) -> Result<Login, &'static str> {
    let (section, token_field) = match agent {
        AgentId::Claude => ("claudeAiOauth", "accessToken"),
        AgentId::Codex => ("tokens", "access_token"),
        AgentId::Opencode => return Err(UNREADABLE),
    };
    // Borrow raw sections from the file so unrelated accounts and refresh tokens are
    // never materialized. The caller clears the file buffer after extracting the token.
    let sections: BTreeMap<String, &RawValue> =
        serde_json::from_slice(without_bom(auth)).map_err(|_| UNREADABLE)?;
    if agent == AgentId::Codex && !codex_file_login(&sections) {
        return Err(login_required(agent));
    }
    let section = sections.get(section).ok_or_else(|| login_required(agent))?;
    let fields: BTreeMap<String, &RawValue> =
        serde_json::from_str(section.get()).map_err(|_| UNREADABLE)?;
    let field_text = |name: &str| {
        serde_json::from_str::<String>(fields.get(name)?.get())
            .ok()
            .filter(|s| !s.trim().is_empty())
    };
    let token = SecretToken::new(field_text(token_field).ok_or_else(|| login_required(agent))?);
    // The same shape rule as the OpenCode key: anything else could not become a header.
    if !token.with_secret(|s| SecretToken::new(s.trim().to_owned()).is_well_formed()) {
        return Err(login_required(agent));
    }
    let account_id = if agent == AgentId::Codex {
        field_text("account_id")
    } else {
        None
    };
    Ok(Login { token, account_id })
}

fn usage_request(
    client: &Client,
    agent: AgentId,
    login: Login,
) -> Result<RequestBuilder, &'static str> {
    let url = match agent {
        AgentId::Claude => "https://api.anthropic.com/api/oauth/usage",
        AgentId::Codex => "https://chatgpt.com/backend-api/wham/usage",
        AgentId::Opencode => "https://opencode.ai/zen/go/v1/usage",
    };
    let mut request = login.token.with_secret(|token| {
        client
            .get(url)
            .header("Accept", "application/json")
            .bearer_auth(token.trim())
    });
    match agent {
        AgentId::Claude => {
            // No User-Agent override: Traygent never presents itself as the provider's CLI.
            request = request.header("anthropic-beta", "oauth-2025-04-20");
        }
        AgentId::Codex => {
            if let Some(id) = login.account_id {
                let mut header = HeaderValue::from_str(id.trim()).map_err(|_| UNREADABLE)?;
                header.set_sensitive(true);
                request = request.header("ChatGPT-Account-Id", header);
            }
        }
        AgentId::Opencode => {}
    }
    Ok(request)
}

fn codex_file_login(sections: &BTreeMap<String, &RawValue>) -> bool {
    // Current Codex records the active mode. Older files select API/other credentials
    // first when present, even if an old tokens section survived.
    if let Some(mode) = sections
        .get("auth_mode")
        .filter(|value| value.get() != "null")
    {
        return serde_json::from_str::<String>(mode.get()).is_ok_and(|s| s == "chatgpt");
    }
    [
        "OPENAI_API_KEY",
        "personal_access_token",
        "bedrock_api_key",
        "bedrock_access_keys",
    ]
    .iter()
    .all(|key| sections.get(*key).is_none_or(|value| value.get() == "null"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;
    use serde_json::json;

    #[test]
    fn only_the_matching_provider_login_goes_to_its_usage_endpoint() {
        let auth = json!({
            "claudeAiOauth": {"accessToken": "demo-claude", "refreshToken": "unused"},
            "tokens": {"access_token": "demo-codex", "account_id": "demo-account"},
            "another-provider": {"key": "unused"}
        })
        .to_string();
        let client = Client::new();
        for (agent, url, bearer) in [
            (
                AgentId::Claude,
                "https://api.anthropic.com/api/oauth/usage",
                "Bearer demo-claude",
            ),
            (
                AgentId::Codex,
                "https://chatgpt.com/backend-api/wham/usage",
                "Bearer demo-codex",
            ),
            (
                AgentId::Opencode,
                "https://opencode.ai/zen/go/v1/usage",
                "Bearer demo-go",
            ),
        ] {
            let login = match agent {
                AgentId::Opencode => Login {
                    token: SecretToken::new("demo-go".into()),
                    account_id: None,
                },
                _ => parse_cli_login(agent, auth.as_bytes()).unwrap(),
            };
            let req = usage_request(&client, agent, login)
                .unwrap()
                .build()
                .unwrap();
            assert_eq!(req.method(), reqwest::Method::GET);
            assert_eq!(req.url().as_str(), url);
            assert_eq!(req.headers()["accept"], "application/json");
            assert_eq!(req.headers()["authorization"], bearer);
            assert!(req.headers()["authorization"].is_sensitive());
            // The client's own `Traygent/<version>` stays the only User-Agent.
            assert!(!req.headers().contains_key("user-agent"));
            if agent == AgentId::Codex {
                assert_eq!(req.headers()["chatgpt-account-id"], "demo-account");
                assert!(req.headers()["chatgpt-account-id"].is_sensitive());
            } else {
                assert!(!req.headers().contains_key("chatgpt-account-id"));
            }
            if agent == AgentId::Claude {
                assert_eq!(req.headers()["anthropic-beta"], "oauth-2025-04-20");
            } else {
                assert!(!req.headers().contains_key("anthropic-beta"));
            }
        }
    }

    #[test]
    fn unrelated_logins_and_oversized_or_invalid_files_do_not_produce_usage() {
        let temp = TempDir::new("quota-auth");
        let file = temp.0.join("auth.json");
        fs::write(&file, br#"{"OPENAI_API_KEY":"unused"}"#).unwrap();
        for agent in [AgentId::Claude, AgentId::Codex] {
            assert_eq!(super::fetch(agent, &file), Err(login_required(agent)));
        }
        fs::write(&file, b"private material").unwrap();
        assert_eq!(super::fetch(AgentId::Codex, &file), Err(UNREADABLE));
        fs::File::create(&file)
            .unwrap()
            .set_len(MAX_AUTH_BYTES + 1)
            .unwrap();
        assert_eq!(login(AgentId::Codex, &file).err(), Some(UNREADABLE));
        assert_eq!(read_bounded(&b"{}   "[..], 4, 0), None);
    }

    #[test]
    fn codex_only_reads_a_file_login_when_its_config_selects_file_storage() {
        let temp = TempDir::new("codex-quota-store");
        let auth = temp.0.join("auth.json");
        let config = temp.0.join("config.toml");
        fs::write(&auth, br#"{"tokens":{"access_token":"demo-account"}}"#).unwrap();
        assert!(login(AgentId::Codex, &auth).is_ok());
        for text in [
            "model = 'demo'",
            "cli_auth_credentials_store = 'file'",
            "[mcp_servers.demo]\ncli_auth_credentials_store = 'keyring'",
            "notes = '''\ncli_auth_credentials_store = 'auto'\n'''",
        ] {
            fs::write(&config, text).unwrap();
            assert!(login(AgentId::Codex, &auth).is_ok());
        }
        for store in ["keyring", "auto", "ephemeral", "unrecognized"] {
            fs::write(&config, format!("'cli_auth_credentials_store' = '{store}'")).unwrap();
            assert_eq!(login(AgentId::Codex, &auth).err(), Some(CODEX_OTHER_STORE));
        }
        fs::write(config, b"broken toml").unwrap();
        assert_eq!(
            login(AgentId::Codex, &auth).err(),
            Some(CODEX_CONFIG_UNREADABLE)
        );
    }

    #[test]
    fn codex_respects_the_active_auth_mode_with_stale_tokens_present() {
        for entry in [
            json!({"auth_mode":"apikey","tokens":{"access_token":"unused"}}),
            json!({"OPENAI_API_KEY":"demo","tokens":{"access_token":"unused"}}),
            json!({"auth_mode":"chatgptAuthTokens","tokens":{"access_token":"unused"}}),
        ] {
            assert!(parse_cli_login(AgentId::Codex, entry.to_string().as_bytes()).is_err());
        }
        let entry =
            json!({"auth_mode":"chatgpt", "OPENAI_API_KEY":null, "tokens":{"access_token":"demo"}});
        assert!(parse_cli_login(AgentId::Codex, entry.to_string().as_bytes()).is_ok());
    }

    #[test]
    fn a_token_that_cannot_be_a_header_reads_as_signed_out() {
        for bad in ["a b", "a\nb", "a\u{7f}b"] {
            let auth = json!({"claudeAiOauth": {"accessToken": bad}}).to_string();
            assert_eq!(
                parse_cli_login(AgentId::Claude, auth.as_bytes()).err(),
                Some(login_required(AgentId::Claude))
            );
        }
        let auth = json!({"claudeAiOauth": {"accessToken": " demo-padded\n"}}).to_string();
        assert!(parse_cli_login(AgentId::Claude, auth.as_bytes()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn linked_credential_file_is_rejected() {
        let temp = TempDir::new("quota-link");
        let file = temp.0.join("auth.json");
        fs::write(&file, b"{}").unwrap();
        let link = temp.0.join("link");
        std::os::unix::fs::symlink(file, &link).unwrap();
        assert_eq!(login(AgentId::Codex, &link).err(), Some(UNREADABLE));
    }
}
