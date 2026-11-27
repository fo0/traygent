use super::{opencode, read_log::Exchange, windows, AgentQuota};
use crate::github::http::read_within;
use crate::{
    github::secret::{wipe, SecretToken},
    models::AgentId,
    profiles::files::without_bom,
};
use chrono::{DateTime, Utc};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderValue, RETRY_AFTER};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::time::Duration;

const MAX_AUTH_BYTES: u64 = 1024 * 1024;
const MAX_USAGE_BYTES: usize = 256 * 1024;
const MAX_TOKEN_BYTES: usize = 8 * 1024;
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

/// `exchange` records what the provider answered, for the log only. Codex and OpenCode Go
/// only: Claude Code's quota comes from its CLI (ADR-0015), so no request and no sign-in
/// file of it is ever built or opened here.
pub(super) fn fetch(
    agent: AgentId,
    path: &Path,
    exchange: &mut Exchange,
) -> Result<AgentQuota, &'static str> {
    if agent == AgentId::Claude {
        return Err(UNREADABLE);
    }
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("Traygent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| UNREACHABLE)?;
    let request = usage_request(&client, agent, login(agent, path)?)?;
    exchange.requested = true;
    let response = request.send().map_err(|_| UNREACHABLE)?;
    let headers = response.headers();
    let retry_after = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok());
    exchange.status = Some(response.status().as_u16());
    exchange.retry_after = retry_after.map(str::to_string);
    if let Some(refusal) = refusal(agent, response.status().as_u16(), retry_after, Utc::now()) {
        return refusal;
    }
    let header_percent = |name: &str| headers.get(name)?.to_str().ok()?.parse::<f64>().ok();
    let codex_used = [
        header_percent("x-codex-primary-used-percent"),
        header_percent("x-codex-secondary-used-percent"),
    ];
    let bytes = read_within(response, MAX_USAGE_BYTES + 1, Duration::from_secs(10)).ok();
    let bytes = bytes
        .filter(|bytes| bytes.len() <= MAX_USAGE_BYTES)
        .ok_or(INVALID_RESPONSE)?;
    let body = serde_json::from_slice(without_bom(&bytes)).map_err(|_| INVALID_RESPONSE)?;
    Ok(windows::parse(agent, &body, codex_used, Utc::now()))
}

/// The answer to a status that carries no usage; `None` for a success. A 429 is not an
/// error but a limited read (#100): it carries the wait the provider asks for.
fn refusal(
    agent: AgentId,
    status: u16,
    retry_after: Option<&str>,
    now: DateTime<Utc>,
) -> Option<Result<AgentQuota, &'static str>> {
    Some(match status {
        200..=299 => return None,
        401 => Err("Anmeldung abgelaufen oder ungültig. Bitte im CLI erneut anmelden."),
        403 if agent == AgentId::Opencode => {
            Err("Zugriff verweigert. Bitte OpenCode-Go-Abo und Anmeldung prüfen.")
        }
        403 => Err("Der Anbieter verweigert den Zugriff auf die Kontingente."),
        429 => Ok(AgentQuota::rate_limited(retry_after_secs(retry_after, now))),
        _ => Err("Der Anbieter konnte die Kontingente nicht liefern."),
    })
}

/// `Retry-After` as delay seconds or an HTTP date; `0` when missing, invalid or past.
/// `AgentQuota::rate_limited` caps it.
fn retry_after_secs(value: Option<&str>, now: DateTime<Utc>) -> u64 {
    let Some(value) = value.map(str::trim) else {
        return 0;
    };
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        // Past `u64` still asks for a long wait; `rate_limited` caps it.
        return value.parse().unwrap_or(u64::MAX);
    }
    DateTime::parse_from_rfc2822(value)
        .ok()
        .and_then(|at| u64::try_from((at.to_utc() - now).num_seconds()).ok())
        .unwrap_or(0)
}

fn login_required(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Opencode => "Bitte in OpenCode mit OpenCode Go anmelden.",
        _ => "Bitte in Codex mit einem ChatGPT-Konto anmelden.",
    }
}

fn login(agent: AgentId, path: &Path) -> Result<Login, &'static str> {
    match agent {
        AgentId::Opencode => {
            return Ok(Login {
                token: opencode::go_key(path)?,
                account_id: None,
            })
        }
        AgentId::Codex => check_codex_store(path)?,
        AgentId::Claude => return Err(UNREADABLE),
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

/// Codex's `auth.json`.
fn parse_cli_login(agent: AgentId, auth: &[u8]) -> Result<Login, &'static str> {
    let (section, token_field) = match agent {
        AgentId::Codex => ("tokens", "access_token"),
        AgentId::Claude | AgentId::Opencode => return Err(UNREADABLE),
    };
    // Borrow raw sections from the file so unrelated accounts and refresh tokens are
    // never materialized. The caller clears the file buffer after extracting the token.
    let sections: BTreeMap<String, &RawValue> =
        serde_json::from_slice(without_bom(auth)).map_err(|_| UNREADABLE)?;
    if !codex_file_login(&sections) {
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
    if !token.with_secret(header_safe) {
        return Err(login_required(agent));
    }
    let account_id = field_text("account_id");
    Ok(Login { token, account_id })
}

/// A provider token as it is sent, trimmed. Codex access tokens are JWTs of a few KiB, so
/// the limit is the usual header size and not the one for GitHub tokens.
fn header_safe(token: &str) -> bool {
    let token = token.trim();
    (1..=MAX_TOKEN_BYTES).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic())
}

fn usage_request(
    client: &Client,
    agent: AgentId,
    login: Login,
) -> Result<RequestBuilder, &'static str> {
    let url = match agent {
        AgentId::Codex => "https://chatgpt.com/backend-api/wham/usage",
        AgentId::Opencode => "https://opencode.ai/zen/go/v1/usage",
        AgentId::Claude => return Err(UNREADABLE),
    };
    let mut request = login.token.with_secret(|token| {
        client
            .get(url)
            .header("Accept", "application/json")
            .bearer_auth(token.trim())
    });
    // No User-Agent override: Traygent never presents itself as the provider's CLI.
    if let Some(id) = login.account_id {
        let mut header = HeaderValue::from_str(id.trim()).map_err(|_| UNREADABLE)?;
        header.set_sensitive(true);
        request = request.header("ChatGPT-Account-Id", header);
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
mod tests;
