//! Release policy for in-app updates.
//!
//! Builds with a compiled-in updater public key install Tauri-signed packages; the standard
//! unsigned builds install the release's NSIS EXE after checking its exact name, URL, size
//! and the SHA-256 digest GitHub publishes for the asset (`installer`).
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

pub mod installer;
pub use installer::Installer;

/// Signed updater feed. Standard unsigned releases publish no `latest.json`, so this resolves
/// to 404 until signing is introduced. A signed build then needs a dedicated feed URL that
/// only ever serves signed manifests instead of this "latest release asset" shortcut.
pub const ENDPOINT: &str = "https://github.com/fo0/traygent/releases/latest/download/latest.json";
/// Shown when the signed feed above yields no manifest. The updater plugin reports every
/// non-success status (404, 403, 429, 5xx) as `ReleaseNotFound`, so the text stays neutral.
pub const NO_SIGNED_FEED: &str =
    "Kein Update-Feed für signierte Updates erreichbar oder veröffentlicht.";
/// Public release metadata, read without a token.
pub const LATEST_RELEASE_API: &str = "https://api.github.com/repos/fo0/traygent/releases/latest";
/// The GitHub REST API version the release read asks for.
pub const API_VERSION: &str = "2026-03-10";
const MAX_RELEASE_BYTES: usize = 1024 * 1024;
const MAX_NOTES_CHARS: usize = 4000;
const MAX_VERSION_CHARS: usize = 48;
/// Shown for a failed check, here and by the desktop shell.
pub const CHECK_FAILED: &str = "Update-Prüfung fehlgeschlagen – Verbindung prüfen.";
const RATE_LIMITED: &str = "GitHub begrenzt gerade die Update-Prüfung. Versuche es später erneut.";
const INVALID_RELEASE: &str = "Die Angaben zur neuesten Version konnten nicht gelesen werden.";
const NO_INSTALLER: &str = "Für diese Version fehlt ein prüfbarer Windows-Installer.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    /// RFC 3339 UTC publication time, when known.
    pub date: Option<String>,
}

/// A newer stable release together with the only installer `install_update` may run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub info: UpdateInfo,
    pub installer: Installer,
}

/// A canonical stable `MAJOR.MINOR.PATCH` (no prefix, leading zeros, pre-release or build).
fn stable_version(value: &str) -> Option<semver::Version> {
    if value.len() > MAX_VERSION_CHARS {
        return None;
    }
    semver::Version::parse(value).ok().filter(|parsed| {
        parsed.pre.is_empty() && parsed.build.is_empty() && parsed.to_string() == value
    })
}

fn newer_than(version: &semver::Version, current: &str) -> Result<bool, String> {
    let current = semver::Version::parse(current).map_err(|_| "App-Version ungültig.")?;
    Ok(*version > current)
}

pub fn validate_download_url(url: &str, version: &str) -> bool {
    stable_version(version).is_some()
        && url == format!("https://github.com/fo0/traygent/releases/download/v{version}/Traygent_{version}_x64-setup.exe")
}

pub fn validate_update(url: &str, version: &str, current: &str) -> Result<(), String> {
    if !validate_download_url(url, version) {
        return Err("Das Update gehört nicht zu einem stabilen Traygent-Release.".into());
    }
    let version = semver::Version::parse(version).map_err(|_| "Update-Version ungültig.")?;
    if !newer_than(&version, current)? {
        return Err("Das Update ist nicht neuer als die installierte Version.".into());
    }
    Ok(())
}

/// Formats a publication time as RFC 3339 UTC with second precision.
pub fn format_date(unix_seconds: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp(unix_seconds, 0)
        .map(|date| date.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn normalize_date(value: Option<&str>) -> Option<String> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .and_then(|date| format_date(date.timestamp()))
}

fn bounded_notes(notes: Option<String>) -> String {
    let notes = notes.unwrap_or_default();
    match notes.char_indices().nth(MAX_NOTES_CHARS) {
        Some((cut, _)) => format!("{}…", notes[..cut].trim_end()),
        None => notes,
    }
}

/// Interprets `GET /repos/fo0/traygent/releases/latest`. `Ok(None)` means no newer stable
/// release exists; a newer release that cannot be trusted as a stable `v<version>` tag with
/// its exact installer asset is an error, so nothing is ever installed from it.
pub fn parse_latest_release(
    status: u16,
    body: &[u8],
    current: &str,
) -> Result<Option<Release>, String> {
    match status {
        200 => {}
        // GitHub answers 404 while no published, non-prerelease release exists.
        404 => return Ok(None),
        403 | 429 => return Err(RATE_LIMITED.into()),
        _ => return Err(CHECK_FAILED.into()),
    }
    if body.len() > MAX_RELEASE_BYTES {
        return Err(INVALID_RELEASE.into());
    }
    #[derive(Deserialize)]
    struct Latest {
        tag_name: String,
        draft: bool,
        prerelease: bool,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        published_at: Option<String>,
        #[serde(default)]
        assets: Vec<Asset>,
    }
    #[derive(Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
        size: u64,
        #[serde(default)]
        digest: Option<String>,
    }
    let release: Latest = serde_json::from_slice(body).map_err(|_| INVALID_RELEASE)?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = release
        .tag_name
        .strip_prefix('v')
        .and_then(stable_version)
        .ok_or(INVALID_RELEASE)?;
    if !newer_than(&version, current)? {
        return Ok(None);
    }
    let version = version.to_string();
    let name = installer::file_name(&version);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or(NO_INSTALLER)?;
    let installer = Installer::new(
        &version,
        &asset.browser_download_url,
        asset.size,
        asset.digest.as_deref().unwrap_or_default(),
    )
    .ok_or(NO_INSTALLER)?;
    Ok(Some(Release {
        info: UpdateInfo {
            version,
            notes: bounded_notes(release.body),
            date: normalize_date(release.published_at.as_deref()),
        },
        installer,
    }))
}

/// Unauthenticated, blocking read of the latest public release: fixed `api.github.com` URL,
/// no redirects, bounded body, timeouts.
pub fn fetch_latest_release(current: &str) -> Result<Option<Release>, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Traygent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| CHECK_FAILED)?;
    let response = client
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .map_err(|_| CHECK_FAILED)?;
    let status = response.status().as_u16();
    let mut body = Vec::new();
    response
        .take(MAX_RELEASE_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|_| CHECK_FAILED)?;
    parse_latest_release(status, &body, current)
}

#[cfg(test)]
mod tests;
