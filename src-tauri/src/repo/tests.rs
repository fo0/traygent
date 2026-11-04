use super::*;
use crate::testing::windows_host;

const TARGETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/targets.json");

fn repo_ref(owner: &str, name: &str) -> RepoRef {
    RepoRef {
        owner: owner.into(),
        name: name.into(),
    }
}

/// The shared vectors the panel's classifier checks too.
#[test]
fn classify_matches_every_shared_test_vector() {
    let text = std::fs::read_to_string(TARGETS).expect("fixtures/targets.json");
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 30);
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let field = |key: &str| case[key].as_str().unwrap().to_string();
        let expected = match case["kind"].as_str().unwrap() {
            "empty" => Target::Empty,
            "folder" => Target::Folder(input.trim().into()),
            "repo" => Target::Repo(repo_ref(&field("owner"), &field("name"))),
            "search" => Target::Search(field("query")),
            kind => panic!("unknown kind {kind}"),
        };
        assert_eq!(classify(input), expected, "input {input:?}");
    }
}

#[test]
fn owners_follow_the_github_rules() {
    assert!(is_valid_owner("a"));
    assert!(is_valid_owner("Some-Org-2"));
    assert!(is_valid_owner(&"a".repeat(39)));
    for bad in ["", "-a", "a-", "a_b", "a.b", "ä", "a b", &"a".repeat(40)] {
        assert!(!is_valid_owner(bad), "{bad:?}");
    }
}

#[test]
fn names_follow_the_github_rules() {
    for good in ["a", "my.repo_name-2", ".github", "..a", &"a".repeat(100)] {
        assert!(is_valid_name(good), "{good:?}");
    }
    for bad in ["", ".", "..", "a/b", "a b", "a'b", "ü", &"a".repeat(101)] {
        assert!(!is_valid_name(bad), "{bad:?}");
    }
}

#[test]
fn validate_drops_one_git_suffix_and_names_the_bad_part() {
    assert_eq!(
        validate(&repo_ref("fo0", "traygent.git")),
        Ok(repo_ref("fo0", "traygent"))
    );
    assert_eq!(
        validate(&repo_ref("fo0", "x.git.git")),
        Ok(repo_ref("fo0", "x.git"))
    );
    assert_eq!(
        validate(&repo_ref("-fo0", "x")),
        Err(INVALID_OWNER.to_string())
    );
    for name in [".git", "", "..", "a;b", "a'b"] {
        assert_eq!(
            validate(&repo_ref("fo0", name)),
            Err(INVALID_NAME.to_string()),
            "{name:?}"
        );
    }
}

#[test]
fn classify_is_case_insensitive_for_scheme_and_host_only() {
    assert_eq!(
        classify("HTTPS://GitHub.com/Fo0/TrayGent"),
        Target::Repo(repo_ref("Fo0", "TrayGent"))
    );
    assert_eq!(
        classify("git@GITHUB.COM:fo0/x"),
        Target::Repo(repo_ref("fo0", "x"))
    );
    assert_eq!(
        classify("fo0/traygent/"),
        Target::Search("fo0/traygent/".into())
    );
    assert_eq!(
        classify("https://github.com//fo0/traygent"),
        Target::Search("https://github.com//fo0/traygent".into())
    );
    assert_eq!(classify("ü/x"), Target::Search("ü/x".into()));
    let spaced = "https://github.com/fo0/traygent/tree/a b";
    assert_eq!(classify(spaced), Target::Search(spaced.into()));
}

#[test]
fn clone_urls_are_https_with_a_git_suffix() {
    assert_eq!(
        clone_url(&repo_ref("fo0", "traygent")),
        "https://github.com/fo0/traygent.git"
    );
}

#[test]
fn the_default_clone_root_lives_in_the_user_profile() {
    let host = windows_host();
    assert_eq!(default_clone_root(&host), r"C:\Users\max\Traygent");
    let trailing = Host {
        user_profile: Some(r"C:\Users\max\".into()),
        ..host.clone()
    };
    assert_eq!(default_clone_root(&trailing), r"C:\Users\max\Traygent");
    for profile in [None, Some("/home/max"), Some("max"), Some(r"\\?\C:\x")] {
        let host = Host {
            user_profile: profile.map(Into::into),
            ..host.clone()
        };
        assert_eq!(default_clone_root(&host), "", "{profile:?}");
    }
}

#[test]
fn clone_folders_are_validated_like_any_folder() {
    let repo = repo_ref("fo0", "traygent");
    assert_eq!(
        clone_folder(r"D:\src\", &repo),
        Ok(r"D:\src\fo0\traygent".to_string())
    );
    assert!(clone_folder("", &repo).is_err());
    assert!(clone_folder(r"C:\src", &repo_ref("fo0", "CON")).is_err());
    assert!(clone_folder(r"C:\src", &repo_ref("fo0", "x.")).is_err());
}
