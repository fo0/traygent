use super::*;
use crate::github::fakes::{FakeGh, STATUS};

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

fn reads_a_token(calls: &[Vec<String>]) -> bool {
    calls.iter().any(|call| {
        call.get(1).is_some_and(|arg| arg == "token")
            && call.last().is_some_and(|arg| arg != "--help")
    })
}

#[test]
fn json_status_lists_github_com_sign_ins_without_reading_a_token() {
    let (cli, calls) = FakeGh::default().cli();
    let listed = local_accounts(&cli, Some("FIXTURE-ALEX"));
    assert_eq!(listed.message, None);
    let view: Vec<_> = listed
        .accounts
        .iter()
        .map(|a| (a.login.as_str(), a.importable, a.connected))
        .collect();
    assert_eq!(
        view,
        [
            ("fixture-alex", true, true),
            ("fixture-ci", false, false),
            ("fixture-expired", false, false),
            // A config path containing "token" is still gh's own stored credential.
            ("fixture-sam", true, false),
        ]
    );
    let json = serde_json::to_string(&listed).unwrap();
    for hidden in [
        "untrusted diagnostic",
        "GH_TOKEN",
        "--malicious",
        "enterprise",
    ] {
        assert!(!json.contains(hidden), "{hidden}");
    }
    let calls = calls.lock().unwrap();
    assert_eq!(calls[0], strings(&["auth", "status", "--help"]));
    assert_eq!(
        calls[1],
        strings(&[
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
            "--no-refresh"
        ])
    );
    assert_eq!(calls[2], strings(&["auth", "token", "--help"]));
    assert_eq!(calls.len(), 3);
    assert!(!reads_a_token(&calls));
    assert!(!calls
        .iter()
        .flatten()
        .any(|arg| ["login", "switch", "refresh", "logout"].contains(&arg.as_str())));
}

#[test]
fn older_json_gh_never_gets_the_unsupported_no_refresh_flag() {
    let gh = FakeGh {
        no_refresh: false,
        ..FakeGh::default()
    };
    let (cli, calls) = gh.cli();
    let listed = local_accounts(&cli, None);
    assert_eq!(listed.accounts.iter().filter(|a| a.importable).count(), 2);
    let token = read_token(&cli, "fixture-sam").unwrap();
    token.with_secret(|value| assert_eq!(value, "synthetic-gh-token"));
    let calls = calls.lock().unwrap();
    assert!(!calls.iter().flatten().any(|arg| arg == "--no-refresh"));
    assert_eq!(
        calls.last().unwrap(),
        &strings(&[
            "auth",
            "token",
            "--hostname",
            "github.com",
            "--user",
            "fixture-sam"
        ])
    );
}

#[test]
fn legacy_gh_without_json_offers_only_its_verified_active_account() {
    let gh = FakeGh {
        json: false,
        no_refresh: false,
        ..FakeGh::default()
    };
    let (cli, calls) = gh.cli();
    let listed = local_accounts(&cli, None);
    assert_eq!(
        listed.accounts,
        [LocalAccount {
            login: "fixture-sam".into(),
            importable: true,
            connected: false,
        }]
    );
    assert!(!reads_a_token(&calls.lock().unwrap()));
    assert!(read_token(&cli, "fixture-sam").is_ok());
    assert_eq!(read_token(&cli, "fixture-alex").unwrap_err(), GONE);
    let calls = calls.lock().unwrap();
    assert!(calls.contains(&strings(&[
        "api",
        "--hostname",
        "github.com",
        "user",
        "--jq",
        ".login"
    ])));
    assert!(!calls
        .iter()
        .flatten()
        .any(|arg| arg == "--json" || arg == "--no-refresh"));
}

#[test]
fn gh_without_json_but_with_no_refresh_is_incompatible() {
    let gh = FakeGh {
        json: false,
        ..FakeGh::default()
    };
    let (cli, _) = gh.cli();
    assert_eq!(
        local_accounts(&cli, None).message.as_deref(),
        Some(INCOMPATIBLE)
    );
}

#[test]
fn gh_without_user_flag_lists_but_never_imports() {
    let gh = FakeGh {
        user_flag: false,
        ..FakeGh::default()
    };
    let (cli, calls) = gh.cli();
    let listed = local_accounts(&cli, None);
    assert!(!listed.accounts.is_empty());
    assert!(listed.accounts.iter().all(|a| !a.importable));
    assert_eq!(read_token(&cli, "fixture-alex").unwrap_err(), NO_USER_FLAG);
    assert!(!reads_a_token(&calls.lock().unwrap()));
}

#[test]
fn missing_gh_and_missing_sign_ins_give_one_german_line() {
    let cli = GhCli {
        program: None,
        runner: Box::new(FakeGh::default()),
    };
    assert_eq!(
        local_accounts(&cli, None).message.as_deref(),
        Some(GH_MISSING)
    );
    assert_eq!(read_token(&cli, "fixture-alex").unwrap_err(), GH_MISSING);
    for (status, success, stderr) in [
        (r#"{"hosts":{}}"#, true, ""),
        ("{}", true, ""),
        (
            "",
            false,
            "You are not logged into any GitHub hosts. To log in, run: gh auth login",
        ),
    ] {
        let gh = FakeGh {
            status: status.into(),
            status_success: success,
            stderr,
            ..FakeGh::default()
        };
        let listed = local_accounts(&gh.cli().0, None);
        assert!(listed.accounts.is_empty());
        assert_eq!(listed.message.as_deref(), Some(NO_SIGN_IN), "{status}");
    }
    let legacy = FakeGh {
        json: false,
        no_refresh: false,
        legacy_login: "",
        stderr: "To get started with GitHub CLI, please run:  gh auth login",
        ..FakeGh::default()
    };
    assert_eq!(
        local_accounts(&legacy.cli().0, None).message.as_deref(),
        Some(NO_SIGN_IN)
    );
}

#[test]
fn failing_gh_reports_a_fixed_line_without_its_output() {
    let gh = FakeGh {
        status: "not json private-diagnostic".into(),
        status_success: false,
        stderr: "private-diagnostic",
        ..FakeGh::default()
    };
    let listed = local_accounts(&gh.cli().0, None);
    assert_eq!(listed.message.as_deref(), Some(GH_FAILED));
    // A non-zero exit with valid JSON (one sign-in failed) still lists every sign-in.
    let partial = FakeGh {
        status_success: false,
        ..FakeGh::default()
    };
    assert_eq!(local_accounts(&partial.cli().0, None).accounts.len(), 4);
    let legacy = FakeGh {
        json: false,
        no_refresh: false,
        legacy_login: "not a login!\n",
        ..FakeGh::default()
    };
    assert_eq!(
        local_accounts(&legacy.cli().0, None).message.as_deref(),
        Some(GH_FAILED)
    );
}

#[test]
fn only_the_chosen_usable_account_gets_an_exact_token_request() {
    let (cli, calls) = FakeGh::default().cli();
    let token = read_token(&cli, "fixture-alex").unwrap();
    token.with_secret(|value| assert_eq!(value, "synthetic-gh-token"));
    assert!(!format!("{token:?}").contains("synthetic-gh-token"));
    assert_eq!(
        calls.lock().unwrap().last().unwrap(),
        &strings(&[
            "auth",
            "token",
            "--hostname",
            "github.com",
            "--user",
            "fixture-alex",
            "--no-refresh"
        ])
    );
}

#[test]
fn tampered_or_unusable_selections_never_read_a_token() {
    let (cli, calls) = FakeGh::default().cli();
    for login in ["--help", "a;whoami", "a\nb", "", "ghp_synthetic", "-x"] {
        assert_eq!(
            read_token(&cli, login).unwrap_err(),
            INVALID_LOGIN,
            "{login:?}"
        );
    }
    for login in ["fixture-expired", "fixture-ci", "unknown", "FIXTURE-ALEX"] {
        assert_eq!(read_token(&cli, login).unwrap_err(), GONE, "{login}");
    }
    assert!(!reads_a_token(&calls.lock().unwrap()));
}

#[test]
fn malformed_token_output_is_refused() {
    for token in ["", "\r\n", "two words\n", "synthetic\ttoken"] {
        let gh = FakeGh {
            token,
            ..FakeGh::default()
        };
        assert_eq!(
            read_token(&gh.cli().0, "fixture-alex").unwrap_err(),
            GH_FAILED
        );
    }
}

#[test]
fn status_parser_keeps_github_com_valid_unique_logins() {
    let accounts = parse_status(STATUS.as_bytes()).unwrap();
    let logins: Vec<_> = accounts.iter().map(|a| a.login.as_str()).collect();
    assert_eq!(
        logins,
        [
            "fixture-alex",
            "fixture-ci",
            "fixture-expired",
            "fixture-sam"
        ]
    );
    assert!(parse_status(b"not json").is_err());
    assert!(parse_status(b"\"text\"").is_err());
}
