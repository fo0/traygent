use super::api::{REPOS_URL, USER_URL};
use super::fakes::{FakeApi, FakeGh};
use super::vault::{MemoryVault, VAULT_UNAVAILABLE};
use super::*;

fn stored(vault: &MemoryVault, id: u64) -> Option<String> {
    vault.values.lock().unwrap().get(&id).cloned()
}

#[test]
fn types_serialize_like_the_ts_contract() {
    let account = GithubAccount {
        login: "octocat".into(),
        id: 7,
        source: GithubSource::Gh,
    };
    assert_eq!(
        serde_json::to_value(&account).unwrap(),
        serde_json::json!({ "login": "octocat", "id": 7, "source": "gh" })
    );
    let repo = RepoSummary {
        owner: "o".into(),
        name: "n".into(),
        private: false,
        pushed_at: Some("2026-01-01T00:00:00Z".into()),
        description: None,
    };
    assert_eq!(
        serde_json::to_value(&repo).unwrap(),
        serde_json::json!({ "owner": "o", "name": "n", "private": false, "pushedAt": "2026-01-01T00:00:00Z" })
    );
    let local = LocalAccounts {
        accounts: vec![LocalAccount {
            login: "octocat".into(),
            importable: true,
            connected: false,
        }],
        message: None,
    };
    assert_eq!(
        serde_json::to_value(&local).unwrap(),
        serde_json::json!({ "accounts": [{ "login": "octocat", "importable": true, "connected": false }] })
    );
    let token: GithubSource = serde_json::from_str("\"token\"").unwrap();
    assert_eq!(token, GithubSource::Token);
}

#[test]
fn connect_token_verifies_trims_and_vaults_the_token() {
    let api = FakeApi::default().user(42, "octo-cat");
    let vault = MemoryVault::default();
    let account = connect_token("  synthetic-pat\r\n".into(), &vault, &api).unwrap();
    assert_eq!(
        account,
        GithubAccount {
            login: "octo-cat".into(),
            id: 42,
            source: GithubSource::Token,
        }
    );
    assert_eq!(stored(&vault, 42).as_deref(), Some("synthetic-pat"));
    assert_eq!(
        *api.requests.lock().unwrap(),
        [(USER_URL.to_string(), "synthetic-pat".to_string())]
    );
}

#[test]
fn connect_token_refuses_empty_malformed_invalid_tokens_and_a_closed_vault() {
    let api = FakeApi::default().user(42, "octo-cat");
    let vault = MemoryVault::default();
    assert_eq!(
        connect_token("  ".into(), &vault, &api).unwrap_err(),
        TOKEN_EMPTY
    );
    assert_eq!(
        connect_token("two words".into(), &vault, &api).unwrap_err(),
        TOKEN_MALFORMED
    );
    assert!(api.requests.lock().unwrap().is_empty());
    let rejected = FakeApi::default().on(USER_URL, 401, None, "{}");
    let error = connect_token("synthetic-pat".into(), &vault, &rejected).unwrap_err();
    assert_eq!(error, TOKEN_INVALID);
    assert!(vault.values.lock().unwrap().is_empty());
    let closed = MemoryVault {
        fail: true,
        ..MemoryVault::default()
    };
    let error = connect_token("synthetic-pat".into(), &closed, &api).unwrap_err();
    assert_eq!(error, VAULT_UNAVAILABLE);
    assert!(!error.contains("synthetic"));
}

#[test]
fn import_local_reads_the_chosen_sign_in_and_checks_its_login() {
    let (cli, calls) = FakeGh::default().cli();
    let api = FakeApi::default().user(5, "Fixture-Alex");
    let vault = MemoryVault::default();
    let account = import_local("fixture-alex", &cli, &vault, &api).unwrap();
    assert_eq!(account.source, GithubSource::Gh);
    assert_eq!(account.login, "Fixture-Alex");
    assert_eq!(stored(&vault, 5).as_deref(), Some("synthetic-gh-token"));
    let calls = calls.lock().unwrap();
    assert!(calls.last().unwrap().contains(&"fixture-alex".to_string()));
}

#[test]
fn import_local_refuses_a_token_of_another_account_and_stores_nothing() {
    let (cli, _) = FakeGh::default().cli();
    let api = FakeApi::default().user(6, "someone-else");
    let vault = MemoryVault::default();
    let error = import_local("fixture-alex", &cli, &vault, &api).unwrap_err();
    assert_eq!(
        error,
        "Das gh-Token gehört nicht zu fixture-alex. Lokale Anmeldung erneut prüfen."
    );
    assert!(vault.values.lock().unwrap().is_empty());
    // An environment token is listed but never imported; no /user call happens.
    let api = FakeApi::default().user(6, "fixture-ci");
    assert_eq!(
        import_local("fixture-ci", &cli, &vault, &api).unwrap_err(),
        gh::GONE
    );
    assert!(api.requests.lock().unwrap().is_empty());
}

#[test]
fn list_repos_reads_the_vault_token_and_disconnect_removes_it() {
    let account = GithubAccount {
        login: "octocat".into(),
        id: 9,
        source: GithubSource::Token,
    };
    let vault = MemoryVault::default();
    let api = FakeApi::default().on(
        REPOS_URL,
        200,
        None,
        r#"[{"name":"hello","owner":{"login":"octocat"},"private":false,"pushed_at":"2026-01-01T00:00:00Z"}]"#,
    );
    assert_eq!(
        list_repos(&account, &vault, &api).unwrap_err(),
        TOKEN_MISSING
    );
    vault
        .write(9, &SecretToken::new("synthetic-pat".into()))
        .unwrap();
    let repos = list_repos(&account, &vault, &api).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(api.requests.lock().unwrap()[0].1, "synthetic-pat");
    disconnect(&account, &vault).unwrap();
    assert_eq!(stored(&vault, 9), None);
    let expired = FakeApi::default().on(REPOS_URL, 401, None, "{}");
    vault
        .write(9, &SecretToken::new("synthetic-pat".into()))
        .unwrap();
    assert_eq!(
        list_repos(&account, &vault, &expired).unwrap_err(),
        TOKEN_INVALID
    );
    assert_eq!(
        list_repos(&account, &NoVault, &api).unwrap_err(),
        VAULT_UNAVAILABLE
    );
}

#[test]
fn local_accounts_marks_the_connected_login() {
    let (cli, _) = FakeGh::default().cli();
    let listed = local_accounts(&cli, Some("fixture-sam"));
    let connected: Vec<_> = listed
        .accounts
        .iter()
        .filter(|a| a.connected)
        .map(|a| a.login.as_str())
        .collect();
    assert_eq!(connected, ["fixture-sam"]);
}

#[test]
fn logins_follow_github_rules() {
    for good in ["a", "octo-cat", "A1", &"x".repeat(39)] {
        assert!(valid_login(good), "{good}");
    }
    for bad in ["", "-a", "a-", "a_b", "a b", "ghp_x", &"x".repeat(40)] {
        assert!(!valid_login(bad), "{bad}");
    }
}
