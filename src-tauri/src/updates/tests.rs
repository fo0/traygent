use super::*;

const DIGEST: &str = "sha256:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn url(version: &str) -> String {
    format!("https://github.com/fo0/traygent/releases/download/v{version}/Traygent_{version}_x64-setup.exe")
}
fn asset(version: &str) -> serde_json::Value {
    serde_json::json!({
        "name": installer::file_name(version),
        "browser_download_url": url(version),
        "size": 4_194_304,
        "digest": DIGEST,
        "content_type": "application/x-msdownload",
    })
}
fn release_with(tag: &str, draft: bool, prerelease: bool, assets: serde_json::Value) -> Vec<u8> {
    serde_json::json!({
        "tag_name": tag,
        "draft": draft,
        "prerelease": prerelease,
        "body": "Synthetic changelog",
        "published_at": "2026-11-01T09:05:42+01:00",
        "html_url": "https://github.com/fo0/traygent/releases/tag/ignored",
        "assets": assets,
    })
    .to_string()
    .into_bytes()
}
fn release(tag: &str, draft: bool, prerelease: bool) -> Vec<u8> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    release_with(tag, draft, prerelease, serde_json::json!([asset(version)]))
}

#[test]
fn stable_calendar_updates_are_newer_across_year_and_clock_changes() {
    for (version, current) in [
        ("2026.101.0", "0.1.0"),
        ("2026.1025.100", "2026.1025.59"),
        ("2027.101.0", "2026.1231.2359"),
    ] {
        assert!(validate_update(&url(version), version, current).is_ok());
        assert!(validate_update(&url(current), current, version).is_err());
        assert!(validate_update(&url(version), version, version).is_err());
    }
}
#[test]
fn rejects_foreign_urls_credentials_queries_and_version_mismatches() {
    let expected = url("0.2.0");
    for candidate in [
        expected.replace("https:", "http:"),
        expected.replace("github.com", "github.com.attacker.invalid"),
        expected.replace("github.com", "token@github.com"),
        expected.replace("fo0/traygent", "other/repo"),
        expected.replace("v0.2.0/", "v0.3.0/"),
        format!("{expected}?token=x"),
        format!("{expected}#fragment"),
    ] {
        assert!(validate_update(&candidate, "0.2.0", "0.1.0").is_err());
    }
    for version in ["0.2.0-beta", "0.2.0+build", "../0.2.0", "garbage"] {
        assert!(validate_update(&url(version), version, "0.1.0").is_err());
    }
}
#[test]
fn reports_a_newer_stable_release_with_its_verified_installer() {
    // release.mjs: 2026-11-01T09:05Z → 2026.1101.905, midnight → minute component 0.
    let found = parse_latest_release(200, &release("v2026.1101.905", false, false), "0.1.0")
        .unwrap()
        .unwrap();
    assert_eq!(
        found.info,
        UpdateInfo {
            version: "2026.1101.905".into(),
            notes: "Synthetic changelog".into(),
            date: Some("2026-11-01T08:05:42Z".into()),
        }
    );
    assert_eq!(found.installer.version, "2026.1101.905");
    assert_eq!(found.installer.url, url("2026.1101.905"));
    assert_eq!(found.installer.size, 4_194_304);
    assert_eq!(found.installer.sha256[..4], [0x00, 0x11, 0x22, 0x33]);
    assert_eq!(found.installer.sha256[31], 0xff);
    assert_eq!(
        serde_json::to_value(&found.info).unwrap(),
        serde_json::json!({
            "version": "2026.1101.905",
            "notes": "Synthetic changelog",
            "date": "2026-11-01T08:05:42Z",
        })
    );
    for (tag, current) in [
        ("v2026.1101.0", "2026.1031.2359"),
        ("v2027.101.0", "2026.1231.2359"),
        ("v2026.1025.100", "2026.1025.59"),
    ] {
        let found = parse_latest_release(200, &release(tag, false, false), current).unwrap();
        assert_eq!(found.unwrap().info.version, tag[1..]);
    }
    for current in ["2026.1101.905", "2026.1101.906", "2027.101.0"] {
        assert_eq!(
            parse_latest_release(200, &release("v2026.1101.905", false, false), current),
            Ok(None)
        );
    }
}
#[test]
fn picks_the_exact_installer_among_other_assets() {
    let version = "2026.1101.905";
    let mut other = asset(version);
    other["name"] = "Traygent_2026.1101.905_x64-setup.exe.sig".into();
    other["browser_download_url"] = format!("{}.sig", url(version)).into();
    let body = release_with(
        "v2026.1101.905",
        false,
        false,
        serde_json::json!([other, asset(version)]),
    );
    let found = parse_latest_release(200, &body, "0.1.0").unwrap().unwrap();
    assert_eq!(found.installer.url, url(version));
}
#[test]
fn a_newer_release_without_a_trustworthy_installer_fails_closed() {
    let version = "2026.1101.905";
    let changed = |key: &str, value: serde_json::Value| {
        let mut asset = asset(version);
        asset[key] = value;
        serde_json::json!([asset])
    };
    let mut without_digest = asset(version);
    without_digest.as_object_mut().unwrap().remove("digest");
    let foreign_host = url(version).replace(
        "https://github.com",
        "https://objects.githubusercontent.com",
    );
    for assets in [
        serde_json::json!([]),
        serde_json::Value::Null,
        serde_json::json!([without_digest]),
        changed("name", "Traygent_2026.1101.905_x64-setup.msi".into()),
        changed("name", "traygent_2026.1101.905_x64-setup.exe".into()),
        changed("name", "Traygent_2026.1101.904_x64-setup.exe".into()),
        changed("browser_download_url", url("2026.1101.904").into()),
        changed("browser_download_url", foreign_host.into()),
        changed(
            "browser_download_url",
            format!("{}?x=1", url(version)).into(),
        ),
        changed("size", 0.into()),
        changed("size", (installer::MAX_INSTALLER_BYTES + 1).into()),
        changed("size", (-1).into()),
        changed("digest", serde_json::Value::Null),
        changed("digest", "".into()),
        changed("digest", DIGEST.replace("sha256:", "md5:").into()),
        changed("digest", DIGEST[..DIGEST.len() - 2].into()),
        changed(
            "digest",
            format!("{}zz", &DIGEST[..DIGEST.len() - 2]).into(),
        ),
    ] {
        let body = release_with("v2026.1101.905", false, false, assets.clone());
        assert!(
            parse_latest_release(200, &body, "0.1.0").is_err(),
            "{assets} must be rejected"
        );
    }
    // An equal or older release needs no installer at all.
    let body = release_with("v2026.1101.905", false, false, serde_json::json!([]));
    assert_eq!(parse_latest_release(200, &body, "2026.1101.905"), Ok(None));
}
#[test]
fn ignores_drafts_prereleases_and_missing_releases() {
    for (draft, prerelease) in [(true, false), (false, true), (true, true)] {
        let body = release("v2026.1101.905", draft, prerelease);
        assert_eq!(parse_latest_release(200, &body, "0.1.0"), Ok(None));
    }
    assert_eq!(parse_latest_release(404, b"{}", "0.1.0"), Ok(None));
}
#[test]
fn rejects_untrusted_tags_statuses_and_bodies() {
    for tag in [
        "2026.1101.905",
        "V2026.1101.905",
        "v2026.1101.0905",
        "v02026.1101.905",
        "v2026.1101",
        "v2026.1101.905.1",
        "v2026.1101.905-rc.1",
        "v2026.1101.905+build",
        " v2026.1101.905",
        "v2026.1101.905 ",
        "v../2026.1101.905",
        "vv2026.1101.905",
        "",
    ] {
        assert!(
            parse_latest_release(200, &release(tag, false, false), "0.1.0").is_err(),
            "{tag:?} must be rejected"
        );
    }
    for status in [301, 302, 304, 401, 500, 503] {
        assert_eq!(
            parse_latest_release(status, &release("v9.0.0", false, false), "0.1.0"),
            Err(CHECK_FAILED.into())
        );
    }
    for status in [403, 429] {
        assert_eq!(
            parse_latest_release(status, b"", "0.1.0"),
            Err(RATE_LIMITED.into())
        );
    }
    let mut oversized = release("v9.0.0", false, false);
    oversized.resize(MAX_RELEASE_BYTES + 1, b' ');
    for body in [
        oversized,
        b"not json".to_vec(),
        br#"{"tag_name":"v9.0.0"}"#.to_vec(),
    ] {
        assert!(parse_latest_release(200, &body, "0.1.0").is_err());
    }
    assert!(parse_latest_release(200, &release("v9.0.0", false, false), "dev").is_err());
}
#[test]
fn bounds_notes_and_tolerates_missing_metadata() {
    let body = serde_json::json!({
        "tag_name": "v1.0.0",
        "draft": false,
        "prerelease": false,
        "body": "ä".repeat(MAX_NOTES_CHARS + 10),
        "published_at": "not a date",
        "assets": [asset("1.0.0")],
    });
    let info = parse_latest_release(200, body.to_string().as_bytes(), "0.1.0")
        .unwrap()
        .unwrap()
        .info;
    assert_eq!(info.notes.chars().count(), MAX_NOTES_CHARS + 1);
    assert!(info.notes.ends_with('…'));
    assert_eq!(info.date, None);
    let body = serde_json::json!({
        "tag_name": "v1.0.0",
        "draft": false,
        "prerelease": false,
        "body": null,
        "assets": [asset("1.0.0")],
    });
    let info = parse_latest_release(200, body.to_string().as_bytes(), "0.1.0")
        .unwrap()
        .unwrap()
        .info;
    assert_eq!((info.notes.as_str(), info.date), ("", None));
}
#[test]
fn signed_update_dates_are_rfc3339_utc() {
    assert_eq!(format_date(0).as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(
        format_date(1_793_523_942).as_deref(),
        Some("2026-11-01T09:05:42Z")
    );
}
