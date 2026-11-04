//! The GitHub part of the IPC data model (docs/adr/0005), mirrored from `src/lib/types.ts`.
use serde::{Deserialize, Serialize};

/// How the connected account's token was obtained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GithubSource {
    #[default]
    Token,
    Gh,
}

/// The one connected GitHub account; its token stays in the OS vault.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubAccount {
    pub login: String,
    /// Stable GitHub user id.
    pub id: u64,
    pub source: GithubSource,
}

/// One repository of the account's `/user/repos` listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub owner: String,
    pub name: String,
    pub private: bool,
    /// RFC 3339 time of the last push, when GitHub reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_at: Option<String>,
    /// Short description, for the tooltip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubState {
    /// Absent while no account is connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<GithubAccount>,
    /// The account's repositories, newest push first, at most 1000; `[]` without one.
    pub repos: Vec<RepoSummary>,
    /// True while the listing loads; a `traygent:snapshot` event follows.
    pub loading: bool,
    /// German error of the last listing, e.g. the token expired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// RFC 3339 time of the last successful listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listed_at: Option<String>,
    /// `%USERPROFILE%\Traygent`, resolved; the clone root while `cloneRoot` is `''`.
    pub default_clone_root: String,
}

/// One sign-in the local GitHub CLI holds for github.com.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAccount {
    pub login: String,
    /// False when gh cannot hand out its token (e.g. an environment token, old gh).
    pub importable: bool,
    /// Already the connected account.
    pub connected: bool,
}

/// Result of `github_local_accounts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAccounts {
    pub accounts: Vec<LocalAccount>,
    /// One German line when gh is missing or holds no sign-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
