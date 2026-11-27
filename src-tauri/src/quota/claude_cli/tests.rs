use super::*;
use crate::models::QuotaMetric;
use chrono::{FixedOffset, NaiveDateTime};
use serde_json::json;
use std::sync::Mutex;

const SUBSCRIPTION: &str = include_str!("../../../../fixtures/claude-usage/subscription.txt");

/// Answers every run with `end` and records the plans it was given.
struct Fake {
    end: CliEnd,
    runs: Mutex<Vec<(HiddenLogin, Duration)>>,
}

impl Fake {
    fn new(end: CliEnd) -> Self {
        Self {
            end,
            runs: Mutex::new(Vec::new()),
        }
    }
}

impl CliRunner for Fake {
    fn run(&self, plan: &HiddenLogin, limit: Duration) -> CliEnd {
        self.runs.lock().unwrap().push((plan.clone(), limit));
        self.end.clone()
    }
}

fn plan() -> HiddenLogin {
    HiddenLogin {
        program: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
        args: vec!["-NoLogo".into()],
        cwd: r"C:\Users\max\AppData\Roaming\io.github.fo0.traygent".into(),
    }
}

fn now() -> DateTime<FixedOffset> {
    let at = NaiveDateTime::parse_from_str("2026-11-27 18:00", "%Y-%m-%d %H:%M").unwrap();
    let zone = FixedOffset::east_opt(3600).unwrap();
    zone.from_local_datetime(&at).unwrap()
}

fn printed(result: &str) -> Vec<u8> {
    json!({
        "type": "result", "is_error": false, "num_turns": 0, "result": result,
        "local_command": "usage"
    })
    .to_string()
    .into_bytes()
}

fn exited(code: i32, stdout: Vec<u8>) -> CliEnd {
    CliEnd::Exited {
        code: Some(code),
        stdout,
    }
}

#[test]
fn a_good_run_gives_the_windows_once_within_the_limit() {
    let fake = Fake::new(exited(0, printed(SUBSCRIPTION)));
    let quota = read(&fake, "team", &CliRead::new(plan()), &now());
    assert_eq!(quota.detail, None);
    assert_eq!(quota.retry_after_secs, None);
    assert_eq!(quota.windows[&QuotaMetric::Week].used, 82.0);
    assert_eq!(quota.windows[&QuotaMetric::Fable].used, 74.0);
    assert_eq!(*fake.runs.lock().unwrap(), [(plan(), READ_LIMIT)]);
}

#[test]
fn every_failed_run_has_its_german_reason_and_no_retry_wait() {
    for (end, reason) in [
        (CliEnd::TimedOut, TIMED_OUT),
        (CliEnd::NotStarted, NOT_STARTED),
        (exited(1, printed(SUBSCRIPTION)), FAILED),
        (
            CliEnd::Exited {
                code: None,
                stdout: Vec::new(),
            },
            FAILED,
        ),
        // Oversized output is no `/usage` answer, even when it would parse.
        (
            exited(0, {
                let mut big = printed(SUBSCRIPTION);
                big.resize(MAX_OUTPUT + 1, b' ');
                big
            }),
            claude_usage::UNREADABLE,
        ),
        (exited(0, b"not json".to_vec()), claude_usage::UNREADABLE),
        (
            exited(0, br#"{"is_error":true,"result":"x"}"#.to_vec()),
            claude_usage::CLI_ERROR,
        ),
        (
            exited(0, printed("Estimated cost: $0.0000")),
            claude_usage::NO_SUBSCRIPTION,
        ),
    ] {
        let quota = read(
            &Fake::new(end.clone()),
            "default",
            &CliRead::new(plan()),
            &now(),
        );
        assert_eq!(quota, AgentQuota::unavailable(reason), "{end:?}");
    }
    // Exactly the cap still counts.
    let mut full = printed(SUBSCRIPTION);
    full.resize(MAX_OUTPUT, b' ');
    let quota = answer(&exited(0, full), &now());
    assert_eq!(quota.windows.len(), 2);
}

#[test]
fn the_log_names_the_exit_the_windows_and_never_the_output() {
    let took = Duration::from_millis(1500);
    let good = exited(0, printed(SUBSCRIPTION));
    let quota = answer(&good, &now());
    let (debug, info) = lines("team", &good, &quota, took);
    assert_eq!(
        debug,
        "quota claude profile=team -> cli exit=0 1500ms windows=2 week=82 fable=74"
    );
    assert_eq!(info, None);
    for (end, exit) in [
        (CliEnd::TimedOut, "timeout"),
        (CliEnd::NotStarted, "not-started"),
        (exited(3, Vec::new()), "3"),
        (
            CliEnd::Exited {
                code: None,
                stdout: Vec::new(),
            },
            "signal",
        ),
    ] {
        let quota = answer(&end, &now());
        let (debug, info) = lines("default", &end, &quota, took);
        assert_eq!(
            debug,
            format!("quota claude profile=default -> cli exit={exit} 1500ms windows=0")
        );
        let reason = quota.detail.unwrap();
        assert_eq!(
            info,
            Some(format!(
                "quota claude profile=default failed: cli exit={exit} reason={reason}"
            ))
        );
    }
    // No subscription is a lasting state: the debug line names it, the info level stays quiet.
    let api_key = exited(0, printed("Estimated cost: $0.0000"));
    let quota = answer(&api_key, &now());
    let (debug, info) = lines("default", &api_key, &quota, took);
    assert_eq!(
        debug,
        format!(
            "quota claude profile=default -> cli exit=0 1500ms windows=0 reason={}",
            claude_usage::NO_SUBSCRIPTION
        )
    );
    assert_eq!(info, None);
    let private = exited(0, printed("Top skills: /private-skill 1%"));
    let quota = answer(&private, &now());
    let (debug, info) = lines("default", &private, &quota, took);
    assert!(!format!("{debug}{info:?}").contains("private"));
}

#[test]
fn a_counted_read_holds_the_update_until_it_is_dropped() {
    // Other tests may hold reads meanwhile; this one only waits for its own.
    let read = CliRead::new(plan());
    assert!(!wait_for_cli_reads(Duration::from_millis(60)));
    let released = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drop(read);
    });
    assert!(wait_for_cli_reads(Duration::from_secs(10)));
    released.join().unwrap();
}

#[cfg(unix)]
#[test]
fn the_process_runner_captures_stdout_caps_it_and_ends_a_late_one() {
    let shell = |script: &str| HiddenLogin {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        cwd: "/".into(),
    };
    let end = ProcessCliRunner.run(
        &shell("printf '{\"result\":\"ok\"}'; echo oops >&2"),
        READ_LIMIT,
    );
    assert_eq!(
        end,
        CliEnd::Exited {
            code: Some(0),
            stdout: br#"{"result":"ok"}"#.to_vec()
        }
    );
    let end = ProcessCliRunner.run(&shell("head -c 70000 /dev/zero; exit 4"), READ_LIMIT);
    let CliEnd::Exited { code, stdout } = end else {
        panic!("{end:?}");
    };
    assert_eq!((code, stdout.len()), (Some(4), MAX_OUTPUT + 1));
    let started = Instant::now();
    let end = ProcessCliRunner.run(&shell("sleep 5"), Duration::from_millis(200));
    assert_eq!(end, CliEnd::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(4));
    // A grandchild that keeps the pipe open after the shell exited: what came counts.
    let started = Instant::now();
    let end = ProcessCliRunner.run(
        &shell("printf '{\"result\":\"ok\"}'; (sleep 3) & exit 0"),
        READ_LIMIT,
    );
    assert_eq!(
        end,
        CliEnd::Exited {
            code: Some(0),
            stdout: br#"{"result":"ok"}"#.to_vec()
        }
    );
    assert!(started.elapsed() < Duration::from_millis(2500));
    let relative = HiddenLogin {
        program: "sh".into(),
        ..shell("true")
    };
    assert_eq!(
        ProcessCliRunner.run(&relative, READ_LIMIT),
        CliEnd::NotStarted
    );
    let missing = HiddenLogin {
        program: "/nonexistent/claude".into(),
        ..shell("true")
    };
    assert_eq!(
        ProcessCliRunner.run(&missing, READ_LIMIT),
        CliEnd::NotStarted
    );
}

#[cfg(unix)]
#[test]
fn the_apps_exit_ends_a_running_read_and_starts_none() {
    let plan = HiddenLogin {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), "sleep 5".into()],
        cwd: "/".into(),
    };
    let exiting = AtomicBool::new(false);
    let started = Instant::now();
    let end = std::thread::scope(|scope| {
        let run = scope.spawn(|| run_process(&plan, &exiting, READ_LIMIT));
        std::thread::sleep(Duration::from_millis(200));
        exiting.store(true, Ordering::SeqCst);
        run.join().unwrap()
    });
    assert_eq!(end, CliEnd::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(4));
    // Once the app exits nothing starts.
    assert_eq!(run_process(&plan, &exiting, READ_LIMIT), CliEnd::NotStarted);
}
