use super::*;
use crate::github::api::{REPOS_URL, USER_URL};
use crate::github::fakes::{FakeApi, FakeGh};
use crate::github::vault::{MemoryVault, VAULT_UNAVAILABLE};
use crate::github::{GithubSource, SecretToken, TOKEN_INVALID, TOKEN_MISSING};
use crate::store;
use crate::testing::TempDir;
use chrono::TimeZone;

const REPOS: &str = r#"[{"name":"hello","owner":{"login":"octo-cat"},"private":true,"pushed_at":"2026-01-01T00:00:00Z"}]"#;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 4, 9, 30, 0).unwrap()
}

fn account(id: u64, login: &str) -> GithubAccount {
    GithubAccount {
        login: login.into(),
        id,
        source: GithubSource::Token,
    }
}

fn repo(name: &str) -> RepoSummary {
    RepoSummary {
        owner: "octo-cat".into(),
        name: name.into(),
        private: false,
        pushed_at: None,
        description: None,
    }
}

fn stored(vault: &MemoryVault, id: u64) -> Option<String> {
    vault.values.lock().unwrap().get(&id).cloned()
}

fn vaulted(vault: &MemoryVault, id: u64) {
    vault
        .write(id, &SecretToken::new(format!("synthetic-pat-{id}")))
        .unwrap();
}

/// A service with `account` connected and its first listing finished with `repos`.
fn listed(dir: &TempDir, vault: &MemoryVault, repos: Vec<RepoSummary>) -> Service {
    let mut service = Service::open(&dir.0);
    vaulted(vault, 42);
    let listing = service.connect_github(account(42, "octo-cat"), vault);
    service.finish_listing(listing, Ok(repos), now()).unwrap();
    service
}

#[test]
fn connecting_with_a_token_persists_the_account_and_lists_its_repositories() {
    let dir = TempDir::new("service-github-token");
    let mut service = Service::open(&dir.0);
    let api = FakeApi::default()
        .user(42, "octo-cat")
        .on(REPOS_URL, 200, None, REPOS);
    let vault = MemoryVault::default();
    let verified = github::connect_token(" synthetic-pat ".into(), &vault, &api).unwrap();
    let before = service.snapshot().revision;
    let listing = service.connect_github(verified, &vault);
    let snapshot = service.snapshot();
    assert!(snapshot.revision > before);
    assert_eq!(snapshot.github.account, Some(account(42, "octo-cat")));
    assert!(snapshot.github.loading);
    assert_eq!(listing.account, account(42, "octo-cat"));
    // state.json holds login, id and source; the token only the vault.
    let text = std::fs::read_to_string(store::path(&dir.0)).unwrap();
    assert!(text.contains("\"login\": \"octo-cat\"") && !text.contains("synthetic-pat"));
    assert_eq!(stored(&vault, 42).as_deref(), Some("synthetic-pat"));
    assert_eq!(
        Service::open(&dir.0).github_account(),
        Some(&account(42, "octo-cat"))
    );

    let repos = github::list_repos(&listing.account, &vault, &api);
    let finished = service.finish_listing(listing, repos, now()).unwrap();
    assert!(finished.revision > snapshot.revision);
    let github = finished.github;
    assert!(!github.loading && github.error.is_none());
    assert_eq!(github.repos.len(), 1);
    assert_eq!(github.repos[0].name, "hello");
    assert_eq!(github.listed_at.as_deref(), Some("2026-11-04T09:30:00Z"));
    assert_eq!(api.urls(), [USER_URL, REPOS_URL]);
}

#[test]
fn importing_a_local_sign_in_connects_it_as_a_gh_account() {
    let dir = TempDir::new("service-github-gh");
    let mut service = Service::open(&dir.0);
    let (cli, _) = FakeGh::default().cli();
    let api = FakeApi::default().user(5, "fixture-alex");
    let vault = MemoryVault::default();
    let verified = github::import_local("fixture-alex", &cli, &vault, &api).unwrap();
    let listing = service.connect_github(verified, &vault);
    assert_eq!(listing.account.source, GithubSource::Gh);
    let loaded = Service::open(&dir.0);
    assert_eq!(
        loaded.github_account().map(|a| a.source),
        Some(GithubSource::Gh)
    );
    let local = github::local_accounts(&cli, loaded.github_account().map(|a| a.login.as_str()));
    assert!(local
        .accounts
        .iter()
        .any(|entry| entry.login == "fixture-alex" && entry.connected));
}

#[test]
fn a_loaded_account_starts_idle_and_one_listing_runs_at_a_time() {
    let dir = TempDir::new("service-github-load");
    let mut service = Service::open(&dir.0);
    assert_eq!(service.begin_listing(), None, "no account, no listing");
    let vault = MemoryVault::default();
    service.connect_github(account(42, "octo-cat"), &vault);

    let mut service = Service::open(&dir.0);
    let github = service.snapshot().github;
    assert_eq!(github.account, Some(account(42, "octo-cat")));
    assert!(!github.loading && github.repos.is_empty());
    let before = service.snapshot().revision;
    let listing = service.begin_listing().unwrap();
    assert!(service.snapshot().github.loading);
    assert!(service.snapshot().revision > before);
    let running = service.snapshot().revision;
    assert_eq!(
        service.begin_listing(),
        None,
        "a running listing is deduped"
    );
    assert_eq!(service.snapshot().revision, running);
    service.abort_listing(&listing);
    assert!(!service.snapshot().github.loading);
    assert!(service.begin_listing().is_some());
}

#[test]
fn a_failed_listing_keeps_the_account_and_the_last_repositories() {
    let dir = TempDir::new("service-github-errors");
    let vault = MemoryVault::default();
    let mut service = listed(&dir, &vault, vec![repo("kept")]);
    let listed_at = service.github().listed_at.clone();
    assert!(listed_at.is_some());

    let listing = service.begin_listing().unwrap();
    let expired = FakeApi::default().on(REPOS_URL, 401, None, "{}");
    let result = github::list_repos(&listing.account, &vault, &expired);
    let github = service
        .finish_listing(listing, result, now())
        .unwrap()
        .github;
    assert_eq!(github.error.as_deref(), Some(TOKEN_INVALID));
    assert_eq!(github.account, Some(account(42, "octo-cat")));
    assert_eq!(github.repos, [repo("kept")]);
    assert_eq!(github.listed_at, listed_at);
    assert!(!github.loading);

    let listing = service.begin_listing().unwrap();
    let result = github::list_repos(&listing.account, &MemoryVault::default(), &expired);
    let github = service
        .finish_listing(listing, result, now())
        .unwrap()
        .github;
    assert_eq!(github.error.as_deref(), Some(TOKEN_MISSING));
    assert_eq!(github.repos, [repo("kept")]);

    let listing = service.begin_listing().unwrap();
    let github = service
        .finish_listing(listing, Ok(vec![repo("fresh")]), now())
        .unwrap()
        .github;
    assert_eq!((github.error, github.repos), (None, vec![repo("fresh")]));
}

#[test]
fn another_account_replaces_the_old_one_and_drops_its_listing() {
    let dir = TempDir::new("service-github-replace");
    let vault = MemoryVault::default();
    let mut service = listed(&dir, &vault, vec![repo("old")]);
    let stale = service.begin_listing().unwrap();

    vaulted(&vault, 7);
    let listing = service.connect_github(account(7, "other"), &vault);
    assert_eq!(stored(&vault, 42), None, "the old token leaves the vault");
    assert!(stored(&vault, 7).is_some());
    let github = service.github().clone();
    assert_eq!(github.account, Some(account(7, "other")));
    assert!(github.repos.is_empty() && github.listed_at.is_none() && github.loading);

    let revision = service.snapshot().revision;
    assert_eq!(
        service.finish_listing(stale, Ok(vec![repo("old")]), now()),
        None
    );
    assert_eq!(service.snapshot().revision, revision);
    assert!(
        service.github().loading,
        "the new account's listing still runs"
    );
    let github = service
        .finish_listing(listing, Ok(vec![repo("new")]), now())
        .unwrap()
        .github;
    assert_eq!(github.repos, [repo("new")]);
    assert_eq!(
        Service::open(&dir.0).github_account(),
        Some(&account(7, "other"))
    );
}

#[test]
fn reconnecting_the_same_account_keeps_its_token_entry_and_listing() {
    let dir = TempDir::new("service-github-same");
    let vault = MemoryVault::default();
    let mut service = listed(&dir, &vault, vec![repo("kept")]);
    let stale = service.begin_listing().unwrap();
    let renamed = GithubAccount {
        login: "octo-renamed".into(),
        source: GithubSource::Gh,
        ..account(42, "")
    };
    let listing = service.connect_github(renamed.clone(), &vault);
    assert!(stored(&vault, 42).is_some());
    let github = service.github().clone();
    assert_eq!(github.account, Some(renamed));
    assert_eq!(github.repos, [repo("kept")]);
    assert!(github.listed_at.is_some() && github.loading);
    assert_eq!(service.finish_listing(stale, Ok(Vec::new()), now()), None);
    assert!(service
        .finish_listing(listing, Ok(Vec::new()), now())
        .is_some());
}

#[test]
fn disconnecting_deletes_the_token_and_forgets_the_account() {
    let dir = TempDir::new("service-github-disconnect");
    let vault = MemoryVault::default();
    let mut service = listed(&dir, &vault, vec![repo("gone")]);
    let running = service.begin_listing().unwrap();

    let refused = MemoryVault {
        fail: true,
        ..MemoryVault::default()
    };
    assert_eq!(
        service.disconnect_github(&refused).unwrap_err(),
        VAULT_UNAVAILABLE
    );
    assert_eq!(service.github_account(), Some(&account(42, "octo-cat")));
    assert!(service.github().loading);

    let before = service.snapshot().revision;
    let github = service.disconnect_github(&vault).unwrap().github;
    assert!(service.snapshot().revision > before);
    assert_eq!(stored(&vault, 42), None);
    assert_eq!(github.account, None);
    assert!(github.repos.is_empty() && !github.loading);
    assert!(github.error.is_none() && github.listed_at.is_none());
    assert_eq!(
        service.finish_listing(running, Ok(vec![repo("late")]), now()),
        None
    );
    assert!(service.github().repos.is_empty());
    let text = std::fs::read_to_string(store::path(&dir.0)).unwrap();
    assert!(!text.contains("octo-cat"));
    assert_eq!(Service::open(&dir.0).github_account(), None);
    // Nothing connected: nothing to delete, nothing changes.
    assert_eq!(service.disconnect_github(&refused).unwrap().github, github);
}

#[test]
fn an_account_connected_without_a_writable_state_lives_for_the_session() {
    let mut service = Service::read_only(store::NO_APP_DATA);
    let vault = MemoryVault::default();
    service.connect_github(account(42, "octo-cat"), &vault);
    assert_eq!(service.github_account(), Some(&account(42, "octo-cat")));
    assert_eq!(service.notice(), Some(store::NO_APP_DATA));

    let dir = TempDir::new("service-github-unwritable");
    let mut service = Service::open(&dir.0);
    // The temp file `save` writes first cannot be created.
    let temp = format!("{}.tmp-{}", store::FILE, std::process::id());
    std::fs::create_dir(dir.0.join(temp)).unwrap();
    service.connect_github(account(42, "octo-cat"), &vault);
    assert_eq!(service.github_account(), Some(&account(42, "octo-cat")));
    assert_eq!(service.notice(), Some(crate::service::SAVE_FAILED));
}
