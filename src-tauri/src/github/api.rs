//! The two GitHub REST reads: `GET /user` (who owns a token) and `GET /user/repos` (the
//! account's listing). Pure logic over a [`Transport`]; the HTTPS adapter lives in `http`.
use super::secret::{looks_like_credential, SecretToken};
use super::{valid_login, RepoSummary};
use crate::repo::is_valid_name;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

pub const API_ORIGIN: &str = "https://api.github.com";
/// The GitHub REST API version every request asks for.
pub const API_VERSION: &str = "2026-03-10";
/// First page of the listing; `sort=pushed` is newest first, sorted again locally.
pub const REPOS_URL: &str = "https://api.github.com/user/repos?affiliation=owner,collaborator,organization_member&sort=pushed&per_page=100";
pub const USER_URL: &str = "https://api.github.com/user";
/// 10 pages of 100: the listing holds at most 1000 repositories (ADR-0005).
pub const MAX_PAGES: usize = 10;
/// Entries kept even when a page holds more than the 100 asked for.
pub const MAX_REPOS: usize = 1000;
/// One page of 100 full repository objects is well below this.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_DESCRIPTION_CHARS: usize = 300;

pub const TOKEN_INVALID: &str = "GitHub-Token ungültig oder abgelaufen.";
pub const UNREACHABLE: &str = "GitHub nicht erreichbar – Verbindung prüfen.";
pub const INVALID_RESPONSE: &str = "GitHub-Antwort ungültig.";
pub const RATE_LIMITED: &str = "GitHub begrenzt gerade die Anfragen. Später erneut versuchen.";

/// An `https://api.github.com/…` URL: exact origin, no userinfo, port, fragment, dot
/// segments or credential-shaped query values.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiUrl(String);

impl ApiUrl {
    pub fn parse(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(API_ORIGIN)?.strip_prefix('/')?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let bad_byte = |byte: u8| {
            byte.is_ascii_control() || byte.is_ascii_whitespace() || b"\\#<>\"@".contains(&byte)
        };
        if value.len() > 2048
            || !value.is_ascii()
            || value.bytes().any(bad_byte)
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || query.split('&').any(|pair| {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                name.contains('%') || looks_like_credential(value)
            })
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path without origin and query.
    pub fn path(&self) -> &str {
        let rest = &self.0[API_ORIGIN.len()..];
        rest.split('?').next().unwrap_or_default()
    }
}

/// What a transport hands back: the status, the few headers the logic reads, the body.
pub struct Response {
    pub status: u16,
    /// Lower-case names; only `link`, `retry-after`, `x-ratelimit-remaining`.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Headers a transport keeps.
pub const KEPT_HEADERS: [&str; 3] = ["link", "retry-after", "x-ratelimit-remaining"];

/// One authenticated GET. Implementations send `Accept`, `X-GitHub-Api-Version`,
/// `User-Agent` and the bearer token, follow no redirect, time out and cap the body at
/// [`MAX_BODY_BYTES`]. `Err` is a German message without request or token details.
pub trait Transport: Sync {
    fn get(&self, url: &ApiUrl, token: &SecretToken) -> Result<Response, String>;
}

/// The account behind a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub login: String,
}

fn status_error(response: &Response) -> String {
    match response.status {
        401 => TOKEN_INVALID.into(),
        429 => RATE_LIMITED.into(),
        403 if response.header("x-ratelimit-remaining") == Some("0")
            || response.header("retry-after").is_some() =>
        {
            RATE_LIMITED.into()
        }
        status => format!("GitHub-Fehler {status}."),
    }
}

fn checked(response: Response) -> Result<Response, String> {
    if response.status != 200 {
        return Err(status_error(&response));
    }
    if response.body.len() > MAX_BODY_BYTES {
        return Err(INVALID_RESPONSE.into());
    }
    Ok(response)
}

/// `GET /user`: verifies a token and names its account.
pub fn fetch_user(transport: &dyn Transport, token: &SecretToken) -> Result<User, String> {
    let url = ApiUrl::parse(USER_URL).ok_or(INVALID_RESPONSE)?;
    let response = checked(transport.get(&url, token)?)?;
    #[derive(Deserialize)]
    struct Wire {
        id: u64,
        login: String,
    }
    let wire: Wire = serde_json::from_slice(&response.body).map_err(|_| INVALID_RESPONSE)?;
    if wire.id == 0 || !valid_login(&wire.login) {
        return Err(INVALID_RESPONSE.into());
    }
    Ok(User {
        id: wire.id,
        login: wire.login,
    })
}

#[derive(Deserialize)]
struct WireRepo {
    name: String,
    owner: WireOwner,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct WireOwner {
    login: String,
}

fn summary(wire: WireRepo) -> Option<RepoSummary> {
    if !valid_login(&wire.owner.login) || !is_valid_name(&wire.name) {
        return None;
    }
    let pushed_at = wire
        .pushed_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|time| {
            time.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        });
    let description = wire
        .description
        .map(|text| {
            let clean: String = text
                .chars()
                .map(|c| {
                    if c.is_control() || is_bidi_control(c) {
                        ' '
                    } else {
                        c
                    }
                })
                .collect();
            let trimmed = clean.trim();
            match trimmed.char_indices().nth(MAX_DESCRIPTION_CHARS) {
                Some((cut, _)) => format!("{}…", trimmed[..cut].trim_end()),
                None => trimmed.to_string(),
            }
        })
        .filter(|text| !text.is_empty());
    Some(RepoSummary {
        owner: wire.owner.login,
        name: wire.name,
        private: wire.private,
        pushed_at,
        description,
    })
}

/// Unicode bidi embeddings, overrides, isolates and marks: they could reorder the text
/// around a description in the panel.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// The single `rel="next"` target of a `Link` header, if any. Targets are split at their
/// `<…>` brackets, not at commas: GitHub echoes `affiliation=owner,collaborator,…` in them.
pub fn next_page(link: Option<&str>) -> Result<Option<ApiUrl>, String> {
    let mut next = None;
    let mut rest = link.unwrap_or_default();
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let close = after.find('>').ok_or(INVALID_RESPONSE)?;
        let target = &after[..close];
        let tail = &after[close + 1..];
        let params_end = tail.find('<').unwrap_or(tail.len());
        let is_next = tail[..params_end].split(';').any(|parameter| {
            let parameter = parameter.trim().trim_end_matches(',').trim_end();
            parameter.strip_prefix("rel=").is_some_and(|rel| {
                rel.trim_matches('"')
                    .split_ascii_whitespace()
                    .any(|rel| rel == "next")
            })
        });
        rest = &tail[params_end..];
        if is_next {
            match (ApiUrl::parse(target), &next) {
                (Some(url), None) => next = Some(url),
                _ => return Err(INVALID_RESPONSE.into()),
            }
        }
    }
    Ok(next)
}

/// Newest push first, repositories without a push last, ties by owner and name.
pub fn sort_repos(repos: &mut [RepoSummary]) {
    repos.sort_by(|a, b| {
        let key = |repo: &RepoSummary| (repo.owner.to_lowercase(), repo.name.to_lowercase());
        match (&a.pushed_at, &b.pushed_at) {
            (Some(x), Some(y)) => y.cmp(x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| key(a).cmp(&key(b)))
    });
}

/// `GET /user/repos` over at most [`MAX_PAGES`] pages following `Link: rel="next"` on the
/// same path; a longer listing keeps its first 1000 entries. Malformed entries are skipped,
/// duplicates (case-insensitive `owner/name`) dropped.
pub fn fetch_repos(
    transport: &dyn Transport,
    token: &SecretToken,
) -> Result<Vec<RepoSummary>, String> {
    let first = ApiUrl::parse(REPOS_URL).ok_or(INVALID_RESPONSE)?;
    let mut next = Some(first.clone());
    let mut seen_pages = BTreeSet::new();
    let mut seen_repos = BTreeSet::new();
    let mut repos = Vec::new();
    for _ in 0..MAX_PAGES {
        let Some(url) = next.take() else {
            break;
        };
        if url.path() != first.path() || !seen_pages.insert(url.clone()) {
            return Err(INVALID_RESPONSE.into());
        }
        let response = checked(transport.get(&url, token)?)?;
        next = next_page(response.header("link"))?;
        let page: Vec<serde_json::Value> =
            serde_json::from_slice(&response.body).map_err(|_| INVALID_RESPONSE)?;
        for value in page {
            let Some(repo) = serde_json::from_value(value).ok().and_then(summary) else {
                continue;
            };
            let key = format!("{}/{}", repo.owner, repo.name).to_lowercase();
            if seen_repos.insert(key) {
                repos.push(repo);
            }
            if repos.len() == MAX_REPOS {
                break;
            }
        }
        if repos.len() == MAX_REPOS {
            break;
        }
    }
    sort_repos(&mut repos);
    Ok(repos)
}

#[cfg(test)]
mod tests;
