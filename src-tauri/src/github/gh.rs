//! The local GitHub CLI, ported from RepoWatch `local_auth`: list its github.com sign-ins
//! without reading a token, and read exactly one token after explicit consent.
//!
//! Flags follow the installed version: `--json` and `--no-refresh` only when its help lists
//! them (older gh fails on unknown flags). Old gh without JSON status confirms only its
//! active account through `gh api user`. Human-readable status, credential files and raw
//! process output are never parsed into an error.
use super::discover::installed_gh;
use super::process::{Output, ProcessRunner, Runner, GH_FAILED};
use super::secret::SecretToken;
use super::{valid_login, LocalAccount, LocalAccounts};
use crate::detect::host::Host;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const GH_MISSING: &str =
    "GitHub CLI (gh) nicht gefunden. Nach der Installation Traygent neu starten.";
pub const NO_SIGN_IN: &str =
    "Keine gh-Anmeldung gefunden. Im Terminal mit „gh auth login“ anmelden.";
pub const INCOMPATIBLE: &str =
    "Diese gh-Version stellt keinen kompatiblen Kontostatus bereit. GitHub CLI aktualisieren.";
pub const NO_USER_FLAG: &str =
    "Diese gh-Version unterstützt keine Kontoauswahl. GitHub CLI aktualisieren.";
pub const INVALID_LOGIN: &str = "Lokales Konto ungültig. Lokale Anmeldung erneut prüfen.";
pub const GONE: &str =
    "Die gewählte gh-Anmeldung ist nicht mehr verfügbar. Lokale Anmeldung erneut prüfen.";
/// At most this many sign-ins are taken from gh's status.
const MAX_ACCOUNTS: usize = 50;

/// gh.exe (when found) and the runner that starts it.
pub struct GhCli {
    pub program: Option<PathBuf>,
    pub runner: Box<dyn Runner>,
}

impl GhCli {
    /// gh.exe on this PC with the bounded process runner; no process starts here.
    pub fn installed(host: &Host) -> Self {
        Self {
            program: installed_gh(host),
            runner: Box::new(ProcessRunner),
        }
    }
}

/// One gh sign-in for github.com.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhAccount {
    pub login: String,
    /// The state is `success` and the token is gh's own, not an environment variable.
    pub usable: bool,
}

fn run(program: &Path, runner: &dyn Runner, args: &[&str]) -> Result<Output, String> {
    runner.run(program, args)
}

/// `gh auth <command> --help`: the flags this gh version knows.
pub fn help(program: &Path, runner: &dyn Runner, command: &str) -> Result<String, String> {
    let output = run(program, runner, &["auth", command, "--help"])?;
    if !output.success {
        return Err(GH_FAILED.into());
    }
    std::str::from_utf8(&output.stdout)
        .map(str::to_owned)
        .map_err(|_| GH_FAILED.into())
}

/// gh's stderr when nothing is signed in; matched, never shown.
fn not_signed_in(output: &Output) -> bool {
    let text = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    text.contains("not logged in") || text.contains("gh auth login")
}

/// The github.com sign-ins and whether gh is a legacy version without JSON status.
pub fn status(program: &Path, runner: &dyn Runner) -> Result<(Vec<GhAccount>, bool), String> {
    let help = help(program, runner, "status")?;
    if help.contains("--json") {
        let mut args = vec![
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
        ];
        if help.contains("--no-refresh") {
            args.push("--no-refresh");
        }
        let output = run(program, runner, &args)?;
        // gh exits non-zero when one sign-in fails; its JSON still lists them all.
        return match parse_status(&output.stdout) {
            Ok(accounts) => Ok((accounts, false)),
            Err(_) if !output.success && not_signed_in(&output) => Ok((Vec::new(), false)),
            Err(error) => Err(error),
        };
    }
    if help.contains("--no-refresh") {
        return Err(INCOMPATIBLE.into());
    }
    // Legacy gh: ask its read-only /user call for the active identity instead of parsing
    // human-readable status or reading credential files.
    let args = ["api", "--hostname", "github.com", "user", "--jq", ".login"];
    let output = run(program, runner, &args)?;
    if !output.success {
        return if not_signed_in(&output) {
            Ok((Vec::new(), true))
        } else {
            Err(GH_FAILED.into())
        };
    }
    let login = std::str::from_utf8(&output.stdout)
        .map_err(|_| GH_FAILED)?
        .trim();
    if !valid_login(login) {
        return Err(GH_FAILED.into());
    }
    let account = GhAccount {
        login: login.into(),
        usable: true,
    };
    Ok((vec![account], true))
}

#[derive(Deserialize)]
struct WireStatus {
    #[serde(default)]
    hosts: std::collections::BTreeMap<String, Vec<WireAccount>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAccount {
    host: String,
    login: String,
    state: String,
    #[serde(default)]
    token_source: String,
}

/// `auth status --json hosts`: github.com entries only, valid logins, sorted, unique.
pub fn parse_status(bytes: &[u8]) -> Result<Vec<GhAccount>, String> {
    let status: WireStatus = serde_json::from_slice(bytes).map_err(|_| GH_FAILED)?;
    let mut accounts: Vec<GhAccount> = status
        .hosts
        .get("github.com")
        .into_iter()
        .flatten()
        .take(MAX_ACCOUNTS)
        .filter(|account| account.host == "github.com" && valid_login(&account.login))
        .map(|account| GhAccount {
            login: account.login.clone(),
            // GH_TOKEN / GITHUB_TOKEN sources are not gh's to hand out.
            usable: account.state == "success"
                && !account
                    .token_source
                    .to_ascii_uppercase()
                    .ends_with("_TOKEN"),
        })
        .collect();
    accounts.sort_by_key(|account| account.login.to_lowercase());
    accounts.dedup_by(|a, b| a.login.eq_ignore_ascii_case(&b.login));
    Ok(accounts)
}

/// The sign-ins as the panel lists them, or one German line when there are none.
pub fn local_accounts(gh: &GhCli, connected: Option<&str>) -> LocalAccounts {
    let only = |message: &str| LocalAccounts {
        accounts: Vec::new(),
        message: Some(message.into()),
    };
    let Some(program) = gh.program.as_deref() else {
        return only(GH_MISSING);
    };
    let runner = gh.runner.as_ref();
    let accounts = match status(program, runner) {
        Ok((accounts, _)) if accounts.is_empty() => return only(NO_SIGN_IN),
        Ok((accounts, _)) => accounts,
        Err(error) => return only(&error),
    };
    // Without `--user` gh cannot hand out a chosen account's token.
    let selectable = help(program, runner, "token").is_ok_and(|help| help.contains("--user"));
    let accounts = accounts
        .into_iter()
        .map(|account| LocalAccount {
            connected: connected.is_some_and(|login| login.eq_ignore_ascii_case(&account.login)),
            importable: account.usable && selectable,
            login: account.login,
        })
        .collect();
    LocalAccounts {
        accounts,
        message: None,
    }
}

/// Reads the token of `login` — only after the user chose it and confirmed the import.
/// The sign-in must still be listed and usable; gh must support `--user`.
pub fn read_token(gh: &GhCli, login: &str) -> Result<SecretToken, String> {
    if !valid_login(login) {
        return Err(INVALID_LOGIN.into());
    }
    let program = gh.program.as_deref().ok_or(GH_MISSING)?;
    let runner = gh.runner.as_ref();
    let (accounts, _) = status(program, runner)?;
    if !accounts
        .iter()
        .any(|account| account.login == login && account.usable)
    {
        return Err(GONE.into());
    }
    let help = help(program, runner, "token")?;
    if !help.contains("--user") {
        return Err(NO_USER_FLAG.into());
    }
    let mut args = vec!["auth", "token", "--hostname", "github.com", "--user", login];
    if help.contains("--no-refresh") {
        args.push("--no-refresh");
    }
    let output = run(program, runner, &args)?;
    if !output.success {
        return Err(GH_FAILED.into());
    }
    let token = std::str::from_utf8(&output.stdout)
        .map_err(|_| GH_FAILED)?
        .trim_end_matches(['\r', '\n']);
    let token = SecretToken::new(token.to_owned());
    if !token.is_well_formed() {
        return Err(GH_FAILED.into());
    }
    Ok(token)
}

#[cfg(test)]
mod tests;
