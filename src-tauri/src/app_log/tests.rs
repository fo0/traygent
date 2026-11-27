//! The logger against temporary directories; every token below is a synthetic fixture.
use super::*;

/// Token-shaped fixtures, never real credentials.
const FAKE_GITHUB: &str = "ghp_FIXTUREfixtureFIXTUREfixture000000";
const FAKE_ANTHROPIC: &str = "sk-ant-oat01-FIXTUREfixture";
const FAKE_OPENAI: &str = "sk-proj-FIXTUREfixture";

struct TempLog {
    dir: PathBuf,
    logger: Logger,
}
impl TempLog {
    fn new(name: &str, max_bytes: u64) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "traygent-log-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Self {
            logger: Logger::new(dir.join(DIR_NAME), max_bytes),
            dir,
        }
    }
    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.logger.dir().join(name)).unwrap_or_default()
    }
}
impl Drop for TempLog {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn nothing_is_written_before_init() {
    // The process-wide logger is never started in tests: every call is a no-op.
    assert!(dir().is_none());
    info("ignored");
    debug(|| unreachable!("a debug message is not built without a logger"));
}

#[test]
fn info_is_the_default_and_debug_lines_need_the_debug_level() {
    let log = TempLog::new("level", MAX_BYTES);
    assert_eq!(log.logger.level(), Level::Info);
    log.logger.info("first");
    log.logger
        .debug(|| unreachable!("not built at the info level"));
    log.logger.set_level(Level::Debug);
    log.logger.debug(|| "second".into());
    log.logger.set_level(Level::Info);
    log.logger.debug(|| "third".into());
    let text = log.read(FILE_NAME);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].ends_with(" INFO first"), "{text}");
    assert!(lines[1].ends_with(" DEBUG second"), "{text}");
    // RFC 3339 in UTC with milliseconds, then level and message.
    let stamp = lines[0].split(' ').next().unwrap();
    assert!(stamp.ends_with('Z') && chrono::DateTime::parse_from_rfc3339(stamp).is_ok());
}

#[test]
fn the_file_rotates_at_the_cap_and_keeps_one_predecessor() {
    let log = TempLog::new("rotate", 200);
    for index in 0..12 {
        log.logger.info(&format!("line {index} {}", "x".repeat(40)));
    }
    let current = log.read(FILE_NAME);
    let rotated = log.read(ROTATED_NAME);
    assert!(current.len() as u64 <= 200 && rotated.len() as u64 <= 200);
    assert!(current.contains("line 11 "), "{current}");
    assert!(!rotated.is_empty() && !rotated.contains("line 0 "));
    let names: Vec<_> = fs::read_dir(log.logger.dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

#[test]
fn a_credential_looking_word_never_reaches_the_file() {
    let log = TempLog::new("redact", MAX_BYTES);
    log.logger.set_level(Level::Debug);
    // A token in a message, after a scheme word or in a `name=value` form is replaced.
    log.logger.info(&format!("quota failed: {FAKE_GITHUB}"));
    log.logger
        .info(&format!("claude {FAKE_ANTHROPIC} and \"{FAKE_OPENAI}\""));
    log.logger
        .info("header Authorization: Bearer fixture-opaque-value end");
    log.logger.info("Basic fixture-pair end");
    log.logger
        .info("github_pat_FIXTURE token=fixture client_secret=fixture");
    log.logger
        .info("password=fixture api_key=fixture code=fixture refresh_token:fixture");
    log.logger
        .info("-----BEGIN PRIVATE KEY-----\nfixture\n-----END PRIVATE KEY-----");
    log.logger.info("two\nlines\tand\u{7} a bell");
    let text = log.read(FILE_NAME);
    assert!(
        !text.contains("FIXTURE") && !text.contains("fixture"),
        "{text}"
    );
    assert!(text.contains("Bearer [redacted] end\n"), "{text}");
    assert!(text.contains(" INFO Basic [redacted] end\n"), "{text}");
    assert!(text.contains(" INFO [redacted]\n"));
    assert!(text.contains(" INFO two lines and a bell\n"), "{text}");
    assert_eq!(text.lines().count(), 8, "{text}");
}

#[test]
fn ordinary_words_with_key_like_parts_stay_readable() {
    // `sk-` counts only at the start of a word's piece; ids and German text stay.
    let line = "task-1 desk-2 profile=team agent=claude reason=Anmeldung abgebrochen.";
    assert_eq!(sanitise(line), line);
}

#[test]
fn a_long_message_is_cut() {
    let cut = sanitise(&"a".repeat(MAX_MESSAGE_CHARS + 50));
    assert_eq!(cut.chars().count(), MAX_MESSAGE_CHARS + 1);
    assert!(cut.ends_with('…'));
}

#[test]
fn an_unwritable_directory_is_swallowed() {
    let log = TempLog::new("blocked", MAX_BYTES);
    // A file where the directory should be: every write fails, none panics.
    fs::create_dir_all(&log.dir).unwrap();
    fs::write(log.logger.dir(), b"not a directory").unwrap();
    log.logger.info("lost");
    assert_eq!(log.read(FILE_NAME), "");
}

#[test]
fn a_poisoned_lock_still_writes() {
    let log = TempLog::new("poison", MAX_BYTES);
    let _ = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _guard = log.logger.write.lock().unwrap();
                panic!("poison the log lock (expected in this test)");
            })
            .join()
    });
    assert!(log.logger.write.is_poisoned());
    log.logger.info("after");
    assert!(log.read(FILE_NAME).ends_with(" INFO after\n"));
}

#[test]
fn url_path_drops_origin_query_and_fragment() {
    assert_eq!(
        url_path("https://api.anthropic.com/api/oauth/usage?x=1"),
        "/api/oauth/usage"
    );
    assert_eq!(url_path("https://api.github.com"), "/");
    assert_eq!(url_path("/repos/o/r/tags#a?b"), "/repos/o/r/tags");
    assert_eq!(url_path("/plain"), "/plain");
}

#[test]
fn the_level_has_fixed_names() {
    assert_eq!(serde_json::to_string(&Level::Debug).unwrap(), "\"debug\"");
    assert_eq!(
        serde_json::from_str::<Level>("\"info\"").unwrap(),
        Level::Info
    );
    assert!(serde_json::from_str::<Level>("\"trace\"").is_err());
    assert_eq!(
        (Level::Info.label(), Level::Debug.label()),
        ("info", "debug")
    );
}

#[test]
fn a_lasting_failure_is_logged_once_and_again_when_it_changes_or_returns() {
    let failing = |entries: &[(&str, &str)]| -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(scope, reason)| ((*scope).to_owned(), (*reason).to_owned()))
            .collect()
    };
    let mut log = FailureLog::new("profile sync");
    assert!(log.lines(failing(&[])).is_empty(), "healthy from the start");
    assert_eq!(
        log.lines(failing(&[("claude/team", "settings.json: x")])),
        ["profile sync failed: claude/team reason=settings.json: x"]
    );
    assert!(log
        .lines(failing(&[("claude/team", "settings.json: x")]))
        .is_empty());
    // A changed reason and a new scope speak; the unchanged one stays quiet.
    assert_eq!(
        log.lines(failing(&[("claude/side", "x"), ("claude/team", "y"),])),
        [
            "profile sync failed: claude/side reason=x",
            "profile sync failed: claude/team reason=y"
        ]
    );
    assert!(log.lines(failing(&[("claude/team", "y")])).is_empty());
    assert_eq!(log.lines(failing(&[])), ["profile sync recovered"]);
    assert!(log.lines(failing(&[])).is_empty());
    assert_eq!(
        log.lines(failing(&[("claude/team", "y")])).len(),
        1,
        "a failure that returns is logged again"
    );
}
