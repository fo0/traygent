//! Download, verification and staging of the unsigned NSIS installer of a stable release.
//!
//! The bytes are held in memory until their size and SHA-256 match the release asset; only
//! then are they written to disk. No request carries credentials.
use super::validate_download_url;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Upper bound for an installer; the current EXE is a few MiB.
pub const MAX_INSTALLER_BYTES: u64 = 200 * 1024 * 1024;
/// The Tauri updater's NSIS arguments for `installMode: passive`: a passive progress
/// window, `/R` relaunches the app afterwards, `/UPDATE` runs the installer's update path.
pub const NSIS_UPDATE_ARGS: [&str; 3] = ["/P", "/R", "/UPDATE"];
/// Hosts GitHub sends a release download through; anything else stops the download.
const DOWNLOAD_HOSTS: [&str; 4] = [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "github-releases.githubusercontent.com",
];
const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// A stalled connection fails after this long without progress.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
const DOWNLOAD_DEADLINE: Duration = Duration::from_secs(10 * 60);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const DOWNLOAD_FAILED: &str = "Download fehlgeschlagen – nichts installiert.";
const VERIFY_FAILED: &str = "Datei passt nicht zum Release – nichts installiert.";
const STAGE_FAILED: &str = "Installer konnte nicht gespeichert werden.";
/// Staged update folders under the temp directory: `Traygent-update-<version>`.
const STAGE_PREFIX: &str = "Traygent-update-";

/// The exact installer asset of a validated release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installer {
    pub version: String,
    pub url: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

/// `Traygent_<version>_x64-setup.exe`, the asset name the release workflow uploads.
pub fn file_name(version: &str) -> String {
    format!("Traygent_{version}_x64-setup.exe")
}

impl Installer {
    /// Accepts only the canonical download URL, a bounded non-empty size and a
    /// `sha256:<64 hex>` digest; everything else yields `None`.
    pub fn new(version: &str, url: &str, size: u64, digest: &str) -> Option<Self> {
        if !validate_download_url(url, version) || !(1..=MAX_INSTALLER_BYTES).contains(&size) {
            return None;
        }
        Some(Self {
            version: version.into(),
            url: url.into(),
            size,
            sha256: parse_sha256(digest)?,
        })
    }
}

fn parse_sha256(digest: &str) -> Option<[u8; 32]> {
    let hex = digest.strip_prefix("sha256:")?.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let mut hash = [0; 32];
    let (pairs, _) = hex.as_chunks::<2>();
    for (byte, pair) in hash.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        if !pair.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(hash)
}

/// A download may only move on to GitHub's release hosts, over HTTPS, without credentials
/// or a custom port, and for at most `MAX_REDIRECTS` hops.
fn redirect_allowed(hops: usize, url: &reqwest::Url) -> bool {
    hops <= MAX_REDIRECTS
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url
            .host_str()
            .is_some_and(|host| DOWNLOAD_HOSTS.contains(&host))
}

/// Streams `reader` into memory and accepts it only with exactly `size` bytes matching
/// `sha256`. Aborts as soon as the stream exceeds `size`. `progress` receives the byte
/// count at most every 100 ms and once at the end.
pub fn read_verified(
    mut reader: impl Read,
    size: u64,
    sha256: &[u8; 32],
    mut progress: impl FnMut(u64),
) -> Result<Vec<u8>, String> {
    if size > MAX_INSTALLER_BYTES {
        return Err(VERIFY_FAILED.into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut reported = Instant::now();
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(DOWNLOAD_FAILED.into()),
        };
        if bytes.len() as u64 + read as u64 > size {
            return Err(VERIFY_FAILED.into());
        }
        hasher.update(&buffer[..read]);
        bytes.extend_from_slice(&buffer[..read]);
        if reported.elapsed() >= PROGRESS_INTERVAL {
            progress(bytes.len() as u64);
            reported = Instant::now();
        }
    }
    progress(bytes.len() as u64);
    let hash: [u8; 32] = hasher.finalize().into();
    if bytes.len() as u64 != size || &hash != sha256 {
        return Err(VERIFY_FAILED.into());
    }
    Ok(bytes)
}

/// Fails a read once the overall download deadline has passed.
struct Deadline<R>(R, Instant);
impl<R: Read> Read for Deadline<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if Instant::now() >= self.1 {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.0.read(buffer)
    }
}

/// Unauthenticated download of `installer`, verified by `read_verified`.
pub fn download(installer: &Installer, progress: impl FnMut(u64)) -> Result<Vec<u8>, String> {
    if !validate_download_url(&installer.url, &installer.version) {
        return Err(DOWNLOAD_FAILED.into());
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(STALL_TIMEOUT)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            // `previous` starts with the original URL, so its length is this hop's number.
            if redirect_allowed(attempt.previous().len(), attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirect target not allowed")
            }
        }))
        .user_agent(concat!("Traygent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| DOWNLOAD_FAILED)?;
    let deadline = Instant::now() + DOWNLOAD_DEADLINE;
    let response = client
        .get(&installer.url)
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|_| DOWNLOAD_FAILED)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(DOWNLOAD_FAILED.into());
    }
    if response
        .content_length()
        .is_some_and(|length| length != installer.size)
    {
        return Err(VERIFY_FAILED.into());
    }
    read_verified(
        Deadline(response, deadline),
        installer.size,
        &installer.sha256,
        progress,
    )
}

/// Writes verified installer bytes into a fresh `<root>/Traygent-update-<version>/`,
/// replacing a leftover directory of an earlier attempt, and returns the EXE path.
/// Folders of other versions are earlier updates whose installers have finished; they
/// are removed on a best-effort basis, so at most the current one stays behind.
pub fn stage(root: &Path, installer: &Installer, bytes: &[u8]) -> Result<PathBuf, String> {
    let name = format!("{STAGE_PREFIX}{}", installer.version);
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let other = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(STAGE_PREFIX) && n != name);
        // `file_type` does not follow links: only real directories are removed.
        if other && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    let dir = root.join(name);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(STAGE_FAILED.into()),
    }
    std::fs::create_dir(&dir).map_err(|_| STAGE_FAILED)?;
    let path = dir.join(file_name(&installer.version));
    std::fs::write(&path, bytes).map_err(|_| STAGE_FAILED)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "2026.1101.905";
    fn url() -> String {
        url_for(VERSION)
    }
    fn url_for(version: &str) -> String {
        format!("https://github.com/fo0/traygent/releases/download/v{version}/Traygent_{version}_x64-setup.exe")
    }
    fn sha(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
    fn digest(bytes: &[u8]) -> String {
        let hex: String = sha(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("sha256:{hex}")
    }

    #[test]
    fn installer_requires_the_canonical_url_a_bounded_size_and_a_sha256_digest() {
        let good = digest(b"synthetic installer");
        let installer = Installer::new(VERSION, &url(), 19, &good).unwrap();
        assert_eq!(installer.sha256, sha(b"synthetic installer"));
        assert_eq!(installer.size, 19);
        assert!(Installer::new(
            VERSION,
            &url(),
            19,
            &good.to_uppercase().replace("SHA256", "sha256")
        )
        .is_some());
        for size in [0, MAX_INSTALLER_BYTES + 1] {
            assert_eq!(Installer::new(VERSION, &url(), size, &good), None);
        }
        assert!(Installer::new(VERSION, &url(), MAX_INSTALLER_BYTES, &good).is_some());
        for bad in [
            String::new(),
            good.replace("sha256:", ""),
            good.replace("sha256:", "sha512:"),
            good.replace("sha256:", "SHA256:"),
            good[..good.len() - 1].to_string(),
            format!("{good}0"),
            format!("{}g", &good[..good.len() - 1]),
            format!("{}+", &good[..good.len() - 1]),
            format!(" {good}"),
        ] {
            assert_eq!(Installer::new(VERSION, &url(), 19, &bad), None, "{bad:?}");
        }
        for bad_url in [
            url().replace("https:", "http:"),
            url().replace("github.com", "objects.githubusercontent.com"),
            url().replace("fo0/traygent", "other/traygent"),
            format!("{}?x=1", url()),
        ] {
            assert_eq!(Installer::new(VERSION, &bad_url, 19, &good), None);
        }
        assert_eq!(Installer::new("2026.1101.0905", &url(), 19, &good), None);
    }

    #[test]
    fn redirects_stay_on_github_release_hosts_over_https() {
        let parse = |value: &str| reqwest::Url::parse(value).unwrap();
        for host in DOWNLOAD_HOSTS {
            assert!(redirect_allowed(
                1,
                &parse(&format!("https://{host}/a?sig=x"))
            ));
            assert!(redirect_allowed(
                5,
                &parse(&format!("https://{host}:443/a"))
            ));
            assert!(!redirect_allowed(6, &parse(&format!("https://{host}/a"))));
            assert!(!redirect_allowed(1, &parse(&format!("http://{host}/a"))));
            assert!(!redirect_allowed(
                1,
                &parse(&format!("https://{host}:8443/a"))
            ));
            assert!(!redirect_allowed(
                1,
                &parse(&format!("https://user@{host}/a"))
            ));
            assert!(!redirect_allowed(
                1,
                &parse(&format!("https://u:p@{host}/a"))
            ));
            assert!(!redirect_allowed(
                1,
                &parse(&format!("https://{host}.evil.invalid/a"))
            ));
            assert!(!redirect_allowed(
                1,
                &parse(&format!("https://evil.{host}/a"))
            ));
        }
        for url in [
            "https://api.github.com/a",
            "https://githubusercontent.com/a",
            "https://raw.githubusercontent.com/a",
            "https://github.com.evil.invalid/a",
            "https://140.82.121.4/a",
            "file:///C:/a.exe",
        ] {
            assert!(!redirect_allowed(1, &parse(url)), "{url}");
        }
    }

    #[test]
    fn read_verified_accepts_only_the_exact_bytes() {
        let bytes = vec![7u8; 200 * 1024];
        let mut seen = Vec::new();
        let read = read_verified(bytes.as_slice(), bytes.len() as u64, &sha(&bytes), |n| {
            seen.push(n)
        })
        .unwrap();
        assert_eq!(read, bytes);
        assert_eq!(seen.last(), Some(&(bytes.len() as u64)));
        assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]));

        let mut altered = bytes.clone();
        altered[1000] = 8;
        assert_eq!(
            read_verified(altered.as_slice(), bytes.len() as u64, &sha(&bytes), |_| {}),
            Err(VERIFY_FAILED.into())
        );
        // Too short: every byte matches a prefix, the size does not.
        let short = &bytes[..bytes.len() - 1];
        assert_eq!(
            read_verified(short, bytes.len() as u64, &sha(&bytes), |_| {}),
            Err(VERIFY_FAILED.into())
        );
        assert_eq!(
            read_verified(&b""[..], 1, &sha(b""), |_| {}),
            Err(VERIFY_FAILED.into())
        );
    }

    #[test]
    fn read_verified_stops_as_soon_as_the_stream_exceeds_the_size() {
        // An endless stream must not be read beyond the expected size.
        let mut consumed = 0u64;
        let endless = io::repeat(1);
        struct Counting<'a, R>(R, &'a mut u64);
        impl<R: Read> Read for Counting<'_, R> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let read = self.0.read(buffer)?;
                *self.1 += read as u64;
                Ok(read)
            }
        }
        let result = read_verified(
            Counting(endless, &mut consumed),
            1000,
            &sha(&[1; 1000]),
            |_| {},
        );
        assert_eq!(result, Err(VERIFY_FAILED.into()));
        assert!(consumed <= 64 * 1024, "read {consumed} bytes");
        let long = vec![1u8; 1001];
        assert_eq!(
            read_verified(long.as_slice(), 1000, &sha(&long[..1000]), |_| {}),
            Err(VERIFY_FAILED.into())
        );
        assert_eq!(
            read_verified(
                long.as_slice(),
                MAX_INSTALLER_BYTES + 1,
                &sha(&long),
                |_| {}
            ),
            Err(VERIFY_FAILED.into())
        );
    }

    #[test]
    fn read_verified_reports_reader_errors_as_failed_downloads() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::ConnectionReset.into())
            }
        }
        assert_eq!(
            read_verified(Broken, 10, &sha(b""), |_| {}),
            Err(DOWNLOAD_FAILED.into())
        );
        let expired = Deadline(&b"data"[..], Instant::now());
        assert_eq!(
            read_verified(expired, 4, &sha(b"data"), |_| {}),
            Err(DOWNLOAD_FAILED.into())
        );
    }

    #[test]
    fn stage_writes_a_fresh_version_directory() {
        let root = std::env::temp_dir().join(format!("traygent-stage-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let installer = Installer::new(VERSION, &url(), 3, &digest(b"new")).unwrap();
        let leftover = root.join(format!("Traygent-update-{VERSION}"));
        std::fs::create_dir_all(leftover.join("old")).unwrap();
        std::fs::write(leftover.join("stale.exe"), b"stale").unwrap();
        let path = stage(&root, &installer, b"new").unwrap();
        assert_eq!(path, leftover.join(file_name(VERSION)));
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let entries: Vec<_> = std::fs::read_dir(&leftover).unwrap().collect();
        assert_eq!(entries.len(), 1);
        // The next update removes this one; unrelated entries stay.
        let unrelated = root.join("Traygent-other");
        let next_version = "2099.1231.2359";
        std::fs::create_dir_all(&unrelated).unwrap();
        std::fs::write(root.join("Traygent-update-file"), b"file").unwrap();
        let next = Installer::new(next_version, &url_for(next_version), 3, &digest(b"new"));
        stage(&root, &next.unwrap(), b"new").unwrap();
        assert!(!leftover.exists());
        assert!(unrelated.exists());
        assert!(root.join("Traygent-update-file").exists());
        assert!(root
            .join(format!("Traygent-update-{next_version}"))
            .is_dir());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
