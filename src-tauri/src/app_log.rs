//! The local diagnostic log (#109, ported from RepoWatch): `logs/traygent.log` under the app
//! data directory, one rotated predecessor, level `info` unless the user picks `debug`.
//!
//! Logging never breaks or blocks the app: every failure is swallowed, and before [`init`]
//! (tests, a run without an app data directory) every call is a no-op. Credentials never
//! reach the file: every line passes [`sanitise`], and URLs are reduced to their path by
//! [`url_path`]. Callers log only agent and profile ids, environments, statuses, durations,
//! versions and the German text the panel already shows — never tokens, headers, bodies,
//! file contents, sign-in output or account e-mails. Lines are English; an embedded
//! `reason=` carries the panel's German error text.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex, OnceLock, PoisonError,
    },
};

/// The directory below the app data directory.
pub const DIR_NAME: &str = "logs";
pub const FILE_NAME: &str = "traygent.log";
/// The one predecessor kept when the file reaches its cap.
pub const ROTATED_NAME: &str = "traygent.log.1";
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_MESSAGE_CHARS: usize = 2000;
const REDACTED: &str = "[redacted]";
/// Lower-case marks anywhere in a word that may carry a credential: Anthropic keys, GitHub
/// tokens and the usual `name=value` forms. OpenAI's shorter `sk-` counts only where a
/// piece of the word starts with it ([`key_piece`]), so `task-1` stays readable.
const SECRET_MARKS: [&str; 14] = [
    "sk-ant-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "token=",
    "secret=",
    "password=",
    "key=",
    "code=",
    "access_token",
    "refresh_token",
];

/// What is written: `Info` is the default, `Debug` adds every quota read. With `Debug` a
/// profile sign-in also opens the visible terminal (#41).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    #[default]
    Info,
    Debug,
}

impl Level {
    /// The fixed name in log lines, the settings file and IPC.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
    fn from_u8(value: u8) -> Self {
        if value == 1 {
            Self::Debug
        } else {
            Self::Info
        }
    }
}

/// The path of a URL or request target: no scheme, host, userinfo, query or fragment.
pub fn url_path(url: &str) -> &str {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("/", |start| &rest[start..]),
        None => url,
    };
    &rest[..rest.find(['?', '#']).unwrap_or(rest.len())]
}

/// Names whose value follows a `:` or `=` (`"access_token": "…"`, `x-api-key: …`,
/// `Authorization:Bearer …`), matched at the end of the name where a `_`, `-`, `.` or a
/// capital letter starts them (`apiKey`, not `monkey`).
const SECRET_NAMES: [&str; 10] = [
    "token",
    "secret",
    "password",
    "passwd",
    "key",
    "apikey",
    "cookie",
    "authorization",
    "credential",
    "credentials",
];

/// The one point every line passes before it is written: control and invisible format
/// characters go first (so no zero-width or bidi mark hides a credential), then a word that
/// looks like a credential — a mark, an API key, a JWT, the value of a secret name, the word
/// after `bearer` / `basic` — is replaced, line breaks go, and the length is bounded.
pub fn sanitise(message: &str) -> String {
    if message.contains("-----BEGIN") {
        return REDACTED.into();
    }
    let visible: String = message.chars().filter(|c| !hidden(*c)).collect();
    let mut out = String::with_capacity(visible.len().min(MAX_MESSAGE_CHARS));
    // The word after `bearer` / `basic`, and the value after a secret name's `:` or `=`;
    // a scheme word between a name and its value stays readable.
    let (mut after_scheme, mut after_name) = (false, false);
    for word in visible.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let lower = word.to_ascii_lowercase();
        let scheme = is_scheme(&lower);
        let named = named_value(word);
        let secret = after_scheme
            || (after_name && !scheme)
            || named.is_some_and(|value| !value.is_empty())
            || SECRET_MARKS.iter().any(|mark| lower.contains(mark))
            || key_piece(&lower)
            || jwt(word);
        after_name = match named {
            Some(value) => value.is_empty(),
            None => after_name && scheme,
        };
        after_scheme = scheme || named.is_some_and(is_scheme);
        out.push_str(if secret { REDACTED } else { word });
    }
    if out.chars().count() > MAX_MESSAGE_CHARS {
        out = out.chars().take(MAX_MESSAGE_CHARS).collect();
        out.push('…');
    }
    out
}

/// Control characters other than whitespace, and the zero-width and bidirectional format
/// characters that would make a line read differently from what it holds.
fn hidden(c: char) -> bool {
    (c.is_control() && !c.is_whitespace())
        || matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

fn is_scheme(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    word.eq_ignore_ascii_case("bearer") || word.eq_ignore_ascii_case("basic")
}

/// For a word `<secret name>:<value>` or `<secret name>=<value>`: the value, trimmed of
/// quotes and punctuation (empty when it is the next word).
fn named_value(word: &str) -> Option<&str> {
    let at = word.find([':', '='])?;
    let name = word[..at].trim_matches(|c: char| !c.is_ascii_alphanumeric());
    secret_name(name).then(|| word[at + 1..].trim_matches(|c: char| !c.is_ascii_alphanumeric()))
}

fn secret_name(name: &str) -> bool {
    SECRET_NAMES.iter().any(|secret| {
        let Some(at) = name.len().checked_sub(secret.len()) else {
            return false;
        };
        if !name.is_char_boundary(at) {
            return false;
        }
        let (head, tail) = name.split_at(at);
        tail.eq_ignore_ascii_case(secret)
            && (head.is_empty()
                || head.ends_with(['_', '-', '.'])
                || tail.starts_with(|c: char| c.is_ascii_uppercase()))
    })
}

/// Whether a piece of `word` (split at anything but letters, digits, `-` and `_`) starts
/// like an API key (`sk-…`).
fn key_piece(word: &str) -> bool {
    word.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .any(|piece| piece.starts_with("sk-"))
}

/// Whether a piece of `word` is shaped like a JSON Web Token: `eyJ…` with three non-empty
/// dot-separated parts.
fn jwt(word: &str) -> bool {
    word.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .any(|piece| {
            piece.starts_with("eyJ")
                && piece.split('.').filter(|part| !part.is_empty()).count() >= 3
        })
}

/// One log directory. The process uses a single instance behind [`init`]; tests build their
/// own with a small cap.
pub struct Logger {
    dir: PathBuf,
    max_bytes: u64,
    level: AtomicU8,
    /// Serializes rotation and append; a poisoned lock is still used.
    write: Mutex<()>,
}

impl Logger {
    pub fn new(dir: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            dir: dir.into(),
            max_bytes,
            level: AtomicU8::new(Level::Info as u8),
            write: Mutex::new(()),
        }
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn level(&self) -> Level {
        Level::from_u8(self.level.load(Ordering::Relaxed))
    }
    pub fn set_level(&self, level: Level) {
        self.level.store(level as u8, Ordering::Relaxed);
    }
    pub fn info(&self, message: &str) {
        self.line(Level::Info, message);
    }
    /// The message is only built when the level is `Debug`.
    pub fn debug(&self, message: impl FnOnce() -> String) {
        if self.level() == Level::Debug {
            self.line(Level::Debug, &message());
        }
    }
    fn line(&self, level: Level, message: &str) {
        let line = format!(
            "{} {} {}\n",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level.label().to_ascii_uppercase(),
            sanitise(message)
        );
        let _guard = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        // Best effort: a full disk, a locked file or a missing directory loses the line only.
        let _ = self.append(line.as_bytes());
    }
    fn append(&self, line: &[u8]) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(FILE_NAME);
        let size = fs::metadata(&path).map_or(0, |file| file.len());
        if size > 0 && size.saturating_add(line.len() as u64) > self.max_bytes {
            // A predecessor that cannot be replaced (open elsewhere) must not let the file grow.
            if fs::rename(&path, self.dir.join(ROTATED_NAME)).is_err() {
                fs::File::create(&path)?;
            }
        }
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(line)
    }
}

/// The failing scopes of a repeating job (the background profile sync). A scope that keeps
/// failing would otherwise write its line on every run and rotate the useful history away,
/// so a failure is only written when it is new or its reason changed.
#[derive(Debug)]
pub struct FailureLog {
    /// The job's name at the start of each line, e.g. `profile sync`.
    job: &'static str,
    last: BTreeMap<String, String>,
}

impl FailureLog {
    pub const fn new(job: &'static str) -> Self {
        Self {
            job,
            last: BTreeMap::new(),
        }
    }
    /// The lines for this run's failures (scope → reason), and one recovery line once no
    /// scope fails any more.
    pub fn lines(&mut self, current: BTreeMap<String, String>) -> Vec<String> {
        let job = self.job;
        let mut lines: Vec<String> = current
            .iter()
            .filter(|(scope, reason)| self.last.get(*scope) != Some(*reason))
            .map(|(scope, reason)| format!("{job} failed: {scope} reason={reason}"))
            .collect();
        if current.is_empty() && !self.last.is_empty() {
            lines.push(format!("{job} recovered"));
        }
        self.last = current;
        lines
    }
}

static GLOBAL: OnceLock<Logger> = OnceLock::new();
/// The level in effect, also when chosen before [`init`].
static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// Starts the process-wide log in `dir`; a second call changes nothing.
pub fn init(dir: &Path) {
    let logger = Logger::new(dir, MAX_BYTES);
    logger.set_level(level());
    let _ = GLOBAL.set(logger);
}
/// The log directory once [`init`] ran.
pub fn dir() -> Option<&'static Path> {
    GLOBAL.get().map(Logger::dir)
}
pub fn level() -> Level {
    Level::from_u8(LEVEL.load(Ordering::Relaxed))
}
pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
    if let Some(logger) = GLOBAL.get() {
        logger.set_level(level);
    }
}
pub fn info(message: &str) {
    if let Some(logger) = GLOBAL.get() {
        logger.info(message);
    }
}
pub fn debug(message: impl FnOnce() -> String) {
    if let Some(logger) = GLOBAL.get() {
        logger.debug(message);
    }
}

#[cfg(test)]
mod tests;
