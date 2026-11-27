use super::*;
use crate::quota::{MAX_RETRY_AFTER_SECS, RATE_LIMITED};
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
        assert!(!req.headers().contains_key("anthropic-beta"));
    }
    // Claude Code's quota comes from its CLI (ADR-0015): no login, no request.
    assert!(parse_cli_login(AgentId::Claude, auth.as_bytes()).is_err());
    let login = Login {
        token: SecretToken::new("demo-claude".into()),
        account_id: None,
    };
    assert!(usage_request(&client, AgentId::Claude, login).is_err());
}

#[test]
fn a_claude_read_opens_no_sign_in_file_and_sends_nothing() {
    let temp = TempDir::new("quota-claude-http");
    let file = temp.0.join(".credentials.json");
    fs::write(&file, br#"{"claudeAiOauth":{"accessToken":"demo"}}"#).unwrap();
    let mut exchange = Exchange::default();
    assert_eq!(
        super::fetch(AgentId::Claude, &file, &mut exchange),
        Err(UNREADABLE)
    );
    assert_eq!(exchange, Exchange::default());
    assert_eq!(login(AgentId::Claude, &file).err(), Some(UNREADABLE));
}

#[test]
fn unrelated_logins_and_oversized_or_invalid_files_do_not_produce_usage() {
    let temp = TempDir::new("quota-auth");
    let file = temp.0.join("auth.json");
    fs::write(&file, br#"{"OPENAI_API_KEY":"unused"}"#).unwrap();
    let mut exchange = Exchange::default();
    let result = super::fetch(AgentId::Codex, &file, &mut exchange);
    assert_eq!(result, Err(login_required(AgentId::Codex)));
    // No request was sent: the log keeps the read at the debug level.
    assert_eq!(exchange, Exchange::default());
    fs::write(&file, b"private material").unwrap();
    let fetched = super::fetch(AgentId::Codex, &file, &mut Exchange::default());
    assert_eq!(fetched, Err(UNREADABLE));
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
    let codex = |token: &str| json!({"tokens": {"access_token": token}}).to_string();
    let jwt = |len: usize| "x".repeat(len);
    for auth in [
        codex("a b"),
        codex("a\nb"),
        codex("a\u{7f}b"),
        codex("synthetic-\u{e9}"),
        codex("a\tb"),
        codex(&jwt(MAX_TOKEN_BYTES + 1)),
    ] {
        assert_eq!(
            parse_cli_login(AgentId::Codex, auth.as_bytes()).err(),
            Some(login_required(AgentId::Codex))
        );
    }
    for auth in [
        codex(" demo-padded\n"),
        codex(&jwt(4096)),
        codex(&jwt(MAX_TOKEN_BYTES)),
    ] {
        assert!(parse_cli_login(AgentId::Codex, auth.as_bytes()).is_ok());
    }
}

#[test]
fn a_429_is_a_limited_read_with_the_providers_wait() {
    let now = DateTime::parse_from_rfc3339("2026-02-01T12:00:00Z")
        .unwrap()
        .to_utc();
    let limited = |retry_after: Option<&str>| {
        refusal(AgentId::Codex, 429, retry_after, now)
            .unwrap()
            .unwrap()
    };
    for (header, secs) in [
        (None, 0),
        (Some("0"), 0),
        (Some(" 120 "), 120),
        (Some("-5"), 0),
        (Some("soon"), 0),
        (Some("Sun, 01 Feb 2026 12:05:00 GMT"), 300),
        (Some("Sun, 01 Feb 2026 11:00:00 GMT"), 0),
        (Some("999999999"), MAX_RETRY_AFTER_SECS),
        (Some("99999999999999999999999"), MAX_RETRY_AFTER_SECS),
    ] {
        let quota = limited(header);
        assert_eq!(quota.retry_after_secs, Some(secs), "{header:?}");
        assert!(quota.windows.is_empty());
        assert_eq!(quota.detail.as_deref(), Some(RATE_LIMITED));
    }
    assert_eq!(
        serde_json::to_value(limited(Some("30"))).unwrap(),
        json!({"windows": {}, "detail": RATE_LIMITED, "retryAfterSecs": 30})
    );
    // Every other answer keeps its shape: no `retryAfterSecs` on the wire.
    assert!(refusal(AgentId::Codex, 200, Some("30"), now).is_none());
    for status in [401, 403, 500, 503] {
        assert!(refusal(AgentId::Codex, status, Some("30"), now)
            .unwrap()
            .is_err());
    }
    assert!(!serde_json::to_value(AgentQuota::unavailable("x"))
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("retryAfterSecs"));
}

#[test]
fn a_sized_buffer_does_not_grow_while_reading() {
    let data = [b'x'; 4096];
    let bytes = read_bounded(&data[..], MAX_AUTH_BYTES, data.len() + 1).unwrap();
    assert_eq!(
        (bytes.len(), bytes.capacity()),
        (data.len(), data.len() + 1)
    );
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
