use super::*;
use crate::github::fakes::FakeApi;

fn token() -> SecretToken {
    SecretToken::new("synthetic-token".into())
}

fn repo(owner: &str, name: &str, pushed: Option<&str>) -> String {
    let pushed = pushed.map_or("null".to_string(), |p| format!("\"{p}\""));
    format!(
        r#"{{"id":1,"name":"{name}","full_name":"{owner}/{name}","owner":{{"login":"{owner}","id":2}},"private":true,"pushed_at":{pushed},"description":null,"html_url":"https://github.com/{owner}/{name}"}}"#
    )
}

fn page(repos: &[String]) -> String {
    format!("[{}]", repos.join(","))
}

fn page_url(n: usize) -> String {
    format!("{REPOS_URL}&page={n}")
}

fn next(n: usize) -> String {
    format!(
        "<{}>; rel=\"next\", <{}>; rel=\"last\"",
        page_url(n),
        page_url(99)
    )
}

#[test]
fn user_verifies_the_token_and_names_the_account() {
    let api = FakeApi::default().user(42, "octo-cat");
    let user = fetch_user(&api, &token()).unwrap();
    assert_eq!(
        user,
        User {
            id: 42,
            login: "octo-cat".into()
        }
    );
    assert_eq!(
        *api.requests.lock().unwrap(),
        [(USER_URL.to_string(), "synthetic-token".to_string())]
    );
}

#[test]
fn user_errors_are_german_and_specific() {
    let cases = [
        (401, vec![], "{}", TOKEN_INVALID.to_string()),
        (
            403,
            vec![("x-ratelimit-remaining", "0")],
            "{}",
            RATE_LIMITED.into(),
        ),
        (403, vec![("retry-after", "60")], "{}", RATE_LIMITED.into()),
        (429, vec![], "{}", RATE_LIMITED.into()),
        (403, vec![], "{}", "GitHub-Fehler 403.".into()),
        (500, vec![], "{}", "GitHub-Fehler 500.".into()),
        (200, vec![], "not json", INVALID_RESPONSE.into()),
        (
            200,
            vec![],
            r#"{"id":0,"login":"octocat"}"#,
            INVALID_RESPONSE.into(),
        ),
        (
            200,
            vec![],
            r#"{"id":1,"login":"bad login"}"#,
            INVALID_RESPONSE.into(),
        ),
    ];
    for (status, headers, body, expected) in cases {
        let headers = headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let api = FakeApi::default().respond(USER_URL, status, headers, body);
        assert_eq!(
            fetch_user(&api, &token()).unwrap_err(),
            expected,
            "{status} {body}"
        );
    }
    let offline = FakeApi {
        offline: true,
        ..FakeApi::default()
    };
    assert_eq!(fetch_user(&offline, &token()).unwrap_err(), UNREACHABLE);
    let huge = "x".repeat(MAX_BODY_BYTES + 1);
    let api = FakeApi::default().on(USER_URL, 200, None, &huge);
    assert_eq!(fetch_user(&api, &token()).unwrap_err(), INVALID_RESPONSE);
}

#[test]
fn repos_follow_link_pages_and_sort_newest_push_first() {
    let api = FakeApi::default()
        .on(
            REPOS_URL,
            200,
            Some(&next(2)),
            &page(&[
                repo("zed", "old", Some("2024-01-01T00:00:00Z")),
                repo("acme", "never", None),
            ]),
        )
        .on(
            &page_url(2),
            200,
            None,
            &page(&[
                repo("acme", "new", Some("2026-05-01T10:00:00+02:00")),
                repo("Acme", "Same", Some("2025-01-01T00:00:00Z")),
                repo("acme", "alpha", Some("2025-01-01T00:00:00Z")),
                repo("aaa", "never", None),
                // Duplicates (case-insensitive) and malformed entries are dropped.
                repo("ACME", "NEW", Some("2026-06-01T00:00:00Z")),
                repo("bad owner", "x", None),
                r#"{"name":"no-owner"}"#.to_string(),
            ]),
        );
    let repos = fetch_repos(&api, &token()).unwrap();
    let names: Vec<_> = repos
        .iter()
        .map(|r| format!("{}/{}", r.owner, r.name))
        .collect();
    assert_eq!(
        names,
        [
            "acme/new",
            "acme/alpha",
            "Acme/Same",
            "zed/old",
            "aaa/never",
            "acme/never"
        ]
    );
    assert_eq!(repos[0].pushed_at.as_deref(), Some("2026-05-01T08:00:00Z"));
    assert!(repos[0].private);
    assert_eq!(api.urls(), [REPOS_URL.to_string(), page_url(2)]);
    assert!(REPOS_URL.contains("affiliation=owner,collaborator,organization_member"));
    assert!(REPOS_URL.contains("sort=pushed") && REPOS_URL.contains("per_page=100"));
}

#[test]
fn repos_stop_after_ten_pages() {
    let mut api = FakeApi::default().on(
        REPOS_URL,
        200,
        Some(&next(2)),
        &page(&[repo("o", "r1", None)]),
    );
    for n in 2..=12 {
        let body = page(&[repo("o", &format!("r{n}"), None)]);
        api = api.on(&page_url(n), 200, Some(&next(n + 1)), &body);
    }
    let repos = fetch_repos(&api, &token()).unwrap();
    assert_eq!(repos.len(), MAX_PAGES);
    assert_eq!(api.urls().len(), MAX_PAGES);
}

#[test]
fn an_oversized_page_keeps_the_first_thousand_and_stops() {
    let many: Vec<String> = (0..MAX_REPOS + 5)
        .map(|n| repo("o", &format!("r{n}"), None))
        .collect();
    let api = FakeApi::default().on(REPOS_URL, 200, Some(&next(2)), &page(&many));
    let repos = fetch_repos(&api, &token()).unwrap();
    assert_eq!(repos.len(), MAX_REPOS);
    assert_eq!(api.urls(), [REPOS_URL.to_string()]);
}

#[test]
fn bidi_controls_in_descriptions_become_spaces() {
    let body =
        r#"[{"name":"a","owner":{"login":"o"},"description":"safe\u202etxt.exe\u2066x\u2069"}]"#;
    let api = FakeApi::default().on(REPOS_URL, 200, None, body);
    let repos = fetch_repos(&api, &token()).unwrap();
    assert_eq!(repos[0].description.as_deref(), Some("safe txt.exe x"));
}

#[test]
fn repos_refuse_foreign_or_looping_pages_and_report_401() {
    let foreign = "<https://evil.example/user/repos?page=2>; rel=\"next\"";
    let other_path = "<https://api.github.com/user/orgs?page=2>; rel=\"next\"";
    let looping = format!("<{REPOS_URL}>; rel=\"next\"");
    let double = format!("{}, {}", next(2), next(3));
    for link in [foreign, other_path, &looping, &double] {
        let api = FakeApi::default().on(REPOS_URL, 200, Some(link), "[]");
        assert_eq!(
            fetch_repos(&api, &token()).unwrap_err(),
            INVALID_RESPONSE,
            "{link}"
        );
    }
    let api = FakeApi::default().on(REPOS_URL, 401, None, "{}");
    assert_eq!(fetch_repos(&api, &token()).unwrap_err(), TOKEN_INVALID);
    let api = FakeApi::default().on(REPOS_URL, 200, None, "{}");
    assert_eq!(fetch_repos(&api, &token()).unwrap_err(), INVALID_RESPONSE);
    let api = FakeApi::default().on(REPOS_URL, 200, None, "[]");
    assert!(fetch_repos(&api, &token()).unwrap().is_empty());
}

#[test]
fn descriptions_are_cleaned_and_bounded() {
    let long = "x".repeat(400);
    let body = format!(
        r#"[{{"name":"a","owner":{{"login":"o"}},"description":"line\none  "}},
           {{"name":"b","owner":{{"login":"o"}},"description":"{long}"}},
           {{"name":"c","owner":{{"login":"o"}},"description":"   "}}]"#
    );
    let api = FakeApi::default().on(REPOS_URL, 200, None, &body);
    let repos = fetch_repos(&api, &token()).unwrap();
    assert_eq!(repos[0].description.as_deref(), Some("line one"));
    assert_eq!(repos[1].description.as_ref().unwrap().chars().count(), 301);
    assert_eq!(repos[2].description, None);
    assert!(!repos[0].private);
}

#[test]
fn api_urls_stay_on_api_github_com() {
    for good in [
        USER_URL,
        REPOS_URL,
        "https://api.github.com/user/repos?page=2",
    ] {
        assert!(ApiUrl::parse(good).is_some(), "{good}");
    }
    for bad in [
        "http://api.github.com/user",
        "https://api.github.com.evil.example/user",
        "https://api.github.com@evil.example/user",
        "https://user@api.github.com/user",
        "https://api.github.com:443/user",
        "https://api.github.com//evil.example/user",
        "https://api.github.com/repos/../user",
        "https://api.github.com/user#x",
        "https://api.github.com/user?access_token=ghp_synthetic",
        "https://api.github.com/user?%61=1",
        "https://api.github.com/",
        "https://api.github.com",
    ] {
        assert!(ApiUrl::parse(bad).is_none(), "{bad}");
    }
    assert_eq!(ApiUrl::parse(REPOS_URL).unwrap().path(), "/user/repos");
}

#[test]
fn names_follow_github_rules() {
    for good in ["a", "repo.name", "my_repo-2", &"x".repeat(100)] {
        assert!(is_valid_name(good), "{good}");
    }
    for bad in ["", ".", "..", "a/b", "a b", &"x".repeat(101)] {
        assert!(!is_valid_name(bad), "{bad}");
    }
}

#[test]
fn link_headers_with_commas_inside_targets_parse() {
    let link = format!(
        "<{}>; rel=\"prev\", <{}>; rel=\"next\", <{}>; rel=\"last\"",
        page_url(1),
        page_url(3),
        page_url(9)
    );
    let next = next_page(Some(&link)).unwrap().unwrap();
    assert_eq!(next.as_str(), page_url(3));
    assert_eq!(next_page(None).unwrap(), None);
    let last_only = format!("<{}>; rel=\"last\"", page_url(9));
    assert_eq!(next_page(Some(&last_only)).unwrap(), None);
    assert!(next_page(Some("<https://api.github.com/user/repos; rel=\"next\"")).is_err());
}
