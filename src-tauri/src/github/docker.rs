//! Account checks for the Docker session's short-lived vault helper.
use super::{SecretToken, Vault, TOKEN_MISSING};
use crate::store;
use std::path::Path;

pub const HELPER_ARG: &str = "--docker-github-token";
pub const NO_ACCOUNT: &str = "Für die Token-Weitergabe bitte unter GitHub ein Konto verbinden oder „GitHub-Token weitergeben“ ausschalten.";
pub const UNAVAILABLE: &str = "GitHub-Token für das gewählte Konto nicht verfügbar. Bitte das Konto und die Token-Weitergabe in Traygent prüfen.";

/// The terminal may open after a long clone. Recheck the selection and switch on disk
/// immediately before reading the vault. Never fall back to ambient tokens or host gh.
pub fn session_token(
    dir: &Path,
    expected_id: u64,
    vault: &dyn Vault,
) -> Result<SecretToken, String> {
    if expected_id == 0 || store::docker_github_account_id(dir) != Some(expected_id) {
        return Err(UNAVAILABLE.into());
    }
    vault.read(expected_id)?.ok_or_else(|| TOKEN_MISSING.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::vault::MemoryVault;
    use crate::models::{GithubAccount, GithubSource};
    use crate::store::Persisted;
    use crate::testing::TempDir;

    #[test]
    fn only_the_selected_vault_entry_can_reach_an_opted_in_session() {
        let dir = TempDir::new("github-docker");
        let vault = MemoryVault::default();
        let mut state = Persisted::default();
        state.settings.docker.gh_token = true;
        vault
            .write(11, &SecretToken::new("other-synthetic".into()))
            .unwrap();
        for source in [GithubSource::Gh, GithubSource::Token] {
            state.github = Some(GithubAccount {
                login: "fixture".into(),
                id: 22,
                source,
            });
            store::save(&dir.0, &state).unwrap();
            assert_eq!(session_token(&dir.0, 11, &vault).unwrap_err(), UNAVAILABLE);
            assert_eq!(
                session_token(&dir.0, 22, &vault).unwrap_err(),
                TOKEN_MISSING
            );
            vault
                .write(22, &SecretToken::new("selected-synthetic".into()))
                .unwrap();
            session_token(&dir.0, 22, &vault)
                .unwrap()
                .with_secret(|s| assert_eq!(s, "selected-synthetic"));
            vault.delete(22).unwrap();
        }
        state.settings.docker.gh_token = false;
        store::save(&dir.0, &state).unwrap();
        assert_eq!(session_token(&dir.0, 22, &vault).unwrap_err(), UNAVAILABLE);
    }
}
