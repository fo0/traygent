//! The one connected GitHub account (ADR-0005): a personal access token or an imported gh
//! sign-in, verified against `/user`, kept in the OS vault, and the account's repository
//! listing. Ported from RepoWatch (`local_auth`, `auth::native`, `github`).
//!
//! Every function blocks (process, vault, network): the shell runs them off the UI thread.
//! `Err` strings are German and carry no token, process output or platform error.
//! The account and listing types are the IPC contract's, defined once in [`crate::models`].

pub mod api;
pub mod discover;
#[cfg(test)]
pub(crate) mod fakes;
pub mod gh;
#[cfg(feature = "updates")]
pub mod http;
pub mod process;
pub mod secret;
pub mod vault;

pub use crate::models::{GithubAccount, GithubSource, LocalAccount, LocalAccounts, RepoSummary};
pub use api::{Transport, User, TOKEN_INVALID};
pub use gh::GhCli;
#[cfg(feature = "updates")]
pub use http::HttpTransport;
pub use secret::SecretToken;
#[cfg(feature = "vault")]
pub use vault::KeyringVault;
pub use vault::{NoVault, Vault};

pub const TOKEN_EMPTY: &str = "Bitte ein GitHub-Token eingeben.";
pub const TOKEN_MALFORMED: &str = "Das GitHub-Token enthält ungültige Zeichen.";
pub const TOKEN_MISSING: &str = "GitHub-Token fehlt im Tresor – Konto neu verbinden.";

/// A GitHub login: 1–39 of `A-Za-z0-9-`, no leading or trailing `-`, not credential-shaped.
pub fn valid_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !secret::looks_like_credential(value)
}

fn store(
    user: User,
    token: &SecretToken,
    source: GithubSource,
    vault: &dyn Vault,
) -> Result<GithubAccount, String> {
    vault.write(user.id, token)?;
    Ok(GithubAccount {
        login: user.login,
        id: user.id,
        source,
    })
}

/// Connects with a pasted personal access token: trims it, verifies it against `/user`
/// and stores it in the vault. The pasted string is wiped.
pub fn connect_token(
    mut token: String,
    vault: &dyn Vault,
    api: &dyn Transport,
) -> Result<GithubAccount, String> {
    let secret = SecretToken::new(token.trim().to_owned());
    secret::wipe_string(&mut token);
    if secret.with_secret(str::is_empty) {
        return Err(TOKEN_EMPTY.into());
    }
    if !secret.is_well_formed() {
        return Err(TOKEN_MALFORMED.into());
    }
    let user = api::fetch_user(api, &secret)?;
    store(user, &secret, GithubSource::Token, vault)
}

/// The local gh sign-ins; `connected` is the connected account's login. Reads no token.
pub fn local_accounts(gh: &GhCli, connected: Option<&str>) -> LocalAccounts {
    gh::local_accounts(gh, connected)
}

/// Imports the token of the gh sign-in `login` the user chose and confirmed. `/user` must
/// name that same login before the token is stored.
pub fn import_local(
    login: &str,
    gh: &GhCli,
    vault: &dyn Vault,
    api: &dyn Transport,
) -> Result<GithubAccount, String> {
    let token = gh::read_token(gh, login)?;
    let user = api::fetch_user(api, &token)?;
    if !user.login.eq_ignore_ascii_case(login) {
        return Err(format!(
            "Das gh-Token gehört nicht zu {login}. Lokale Anmeldung erneut prüfen."
        ));
    }
    store(user, &token, GithubSource::Gh, vault)
}

/// Removes Traygent's copy of the token; gh stays signed in.
pub fn disconnect(account: &GithubAccount, vault: &dyn Vault) -> Result<(), String> {
    vault.delete(account.id)
}

/// The account's repositories, newest push first, at most 1000.
pub fn list_repos(
    account: &GithubAccount,
    vault: &dyn Vault,
    api: &dyn Transport,
) -> Result<Vec<RepoSummary>, String> {
    let token = vault.read(account.id)?.ok_or(TOKEN_MISSING)?;
    api::fetch_repos(api, &token)
}

#[cfg(test)]
mod tests;
