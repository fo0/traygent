//! The log lines of one provider request for a quota read (#109; Codex and OpenCode Go, the
//! Claude Code CLI read logs in `claude_cli.rs`): a debug line for every read, an info line
//! for a provider that refused it, limited it or could not be reached. Only the agent, the profile
//! id, the HTTP status, the raw `Retry-After` value, the duration and the panel's German
//! detail are written — never the token, a header, the body or the sign-in file's path.
use super::AgentQuota;
use crate::models::AgentId;
use std::time::Duration;

/// What the provider answered, filled in by the fetch; empty when no request was sent
/// (signed out, unreadable sign-in file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Exchange {
    /// The request was sent (it may still have failed without an answer).
    pub requested: bool,
    pub status: Option<u16>,
    /// The `Retry-After` header as the provider sent it.
    pub retry_after: Option<String>,
}

/// The debug line and, for a refused, limited or failed request, the info line.
pub(super) fn lines(
    agent: AgentId,
    profile: &str,
    exchange: &Exchange,
    result: &Result<AgentQuota, &'static str>,
    took: Duration,
) -> (String, Option<String>) {
    let who = format!("quota {} profile={profile}", agent.id());
    let status = exchange
        .status
        .map_or_else(|| "no-response".to_string(), |status| status.to_string());
    let mut debug = format!("{who} -> {status} {}ms", took.as_millis());
    if let Err(reason) = result {
        debug.push_str(&format!(" reason={reason}"));
    }
    let info = match (exchange.status, result) {
        (Some(429), _) => Some(format!(
            "{who} rate limited: status=429 retry-after={}",
            exchange.retry_after.as_deref().unwrap_or("none")
        )),
        (Some(status), Err(reason)) if !(200..300).contains(&status) => {
            Some(format!("{who} refused: status={status} reason={reason}"))
        }
        (Some(status), Err(reason)) => {
            Some(format!("{who} failed: status={status} reason={reason}"))
        }
        (None, Err(reason)) if exchange.requested => {
            Some(format!("{who} failed: no-response reason={reason}"))
        }
        _ => None,
    };
    (debug, info)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(42);

    fn answered(status: u16, retry_after: Option<&str>) -> Exchange {
        Exchange {
            requested: true,
            status: Some(status),
            retry_after: retry_after.map(Into::into),
        }
    }

    #[test]
    fn a_good_read_writes_the_debug_line_only() {
        let ok = Ok(AgentQuota::off());
        let (debug, info) = lines(AgentId::Codex, "default", &answered(200, None), &ok, MS);
        assert_eq!(debug, "quota codex profile=default -> 200 42ms");
        assert_eq!(info, None);
    }

    #[test]
    fn a_429_is_logged_with_the_raw_retry_after() {
        let limited = Ok(AgentQuota::rate_limited(30));
        let exchange = answered(429, Some("30"));
        let (debug, info) = lines(AgentId::Codex, "team", &exchange, &limited, MS);
        assert_eq!(debug, "quota codex profile=team -> 429 42ms");
        assert_eq!(
            info.as_deref(),
            Some("quota codex profile=team rate limited: status=429 retry-after=30")
        );
        let (_, info) = lines(AgentId::Codex, "team", &answered(429, None), &limited, MS);
        assert!(info.unwrap().ends_with("retry-after=none"));
    }

    #[test]
    fn refusals_and_unreachable_providers_reach_the_info_level() {
        let refused = Err("Anmeldung abgelaufen oder ungültig. Bitte im CLI erneut anmelden.");
        let (_, info) = lines(
            AgentId::Codex,
            "default",
            &answered(401, None),
            &refused,
            MS,
        );
        assert_eq!(
            info.as_deref(),
            Some("quota codex profile=default refused: status=401 reason=Anmeldung abgelaufen oder ungültig. Bitte im CLI erneut anmelden.")
        );
        let invalid = Err("Die Kontingentdaten des Anbieters sind ungültig.");
        let (_, info) = lines(
            AgentId::Opencode,
            "default",
            &answered(200, None),
            &invalid,
            MS,
        );
        assert!(info
            .unwrap()
            .contains("failed: status=200 reason=Die Kontingentdaten"));
        let unreachable = Err("Der Anbieter ist nicht erreichbar.");
        let sent = Exchange {
            requested: true,
            ..Exchange::default()
        };
        let (debug, info) = lines(AgentId::Codex, "default", &sent, &unreachable, MS);
        assert_eq!(
            debug,
            "quota codex profile=default -> no-response 42ms reason=Der Anbieter ist nicht erreichbar."
        );
        assert_eq!(
            info.as_deref(),
            Some("quota codex profile=default failed: no-response reason=Der Anbieter ist nicht erreichbar.")
        );
    }

    #[test]
    fn a_read_without_a_request_stays_at_the_debug_level() {
        let signed_out = Err("Bitte in Codex mit einem ChatGPT-Konto anmelden.");
        let (debug, info) = lines(
            AgentId::Codex,
            "default",
            &Exchange::default(),
            &signed_out,
            MS,
        );
        assert!(debug.ends_with(
            "-> no-response 42ms reason=Bitte in Codex mit einem ChatGPT-Konto anmelden."
        ));
        assert_eq!(info, None);
    }
}
