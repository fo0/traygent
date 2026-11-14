//! The hidden sign-in's decision logic (an injected runner) and, on Unix, the real process
//! loop with `/bin/sh` standing in for PowerShell. Synthetic data only.
use super::*;
use crate::models::AgentId;

fn plan() -> HiddenLogin {
    HiddenLogin {
        program: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
        args: vec!["-EncodedCommand".into(), "AA==".into()],
        cwd: r"C:\Users\max".into(),
    }
}

fn key(id: &str) -> Key {
    (AgentId::Claude, id.into())
}

/// Ends as told; with `None` it waits for the cancel flag (or reports the limit it got).
struct Scripted(Option<LoginEnd>);

impl LoginRunner for Scripted {
    fn run(&self, _: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginRun {
        let end = self.0.clone().unwrap_or_else(|| {
            assert_eq!(limit, LOGIN_LIMIT);
            while !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            LoginEnd::Cancelled
        });
        LoginRun {
            end,
            output: Output::default(),
        }
    }
}

#[test]
fn only_exit_code_zero_signs_in() {
    assert_eq!(LOGIN_LIMIT, Duration::from_secs(300));
    assert_eq!(verdict(&LoginEnd::Exited(Some(0))), Ok(()));
    for end in [
        LoginEnd::Exited(Some(1)),
        LoginEnd::Exited(Some(-1073741510)),
        LoginEnd::Exited(None),
        LoginEnd::NotStarted,
    ] {
        assert_eq!(verdict(&end), Err("Anmeldung fehlgeschlagen."), "{end:?}");
    }
    assert_eq!(verdict(&LoginEnd::TimedOut), Err("Anmeldung abgelaufen."));
    assert_eq!(verdict(&LoginEnd::Cancelled), Err("Anmeldung abgebrochen."));
    let ticket = begin(key("verdict")).unwrap();
    let scripted = |end| sign_in(&Scripted(Some(end)), &plan(), &ticket);
    assert_eq!(scripted(LoginEnd::Exited(Some(0))), Ok(()));
    assert_eq!(
        scripted(LoginEnd::TimedOut),
        Err("Anmeldung abgelaufen.".to_string())
    );
}

#[test]
fn one_sign_in_per_profile_and_a_cancel_ends_it() {
    let team = key("one-at-a-time");
    let ticket = begin(team.clone()).unwrap();
    assert!(running(&team));
    assert_eq!(
        begin(team.clone()).err(),
        Some("Die Anmeldung läuft bereits.")
    );
    // Another profile signs in beside it.
    let other = begin(key("one-at-a-time-2")).unwrap();
    drop(other);
    assert!(!running(&key("one-at-a-time-2")));
    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| sign_in(&Scripted(None), &plan(), &ticket));
        while !cancel(&team) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            waiting.join().unwrap(),
            Err("Anmeldung abgebrochen.".into())
        );
    });
    drop(ticket);
    assert!(!running(&team));
    assert!(!cancel(&team), "nothing left to cancel");
    // The next click starts afresh.
    assert!(begin(team).is_ok());
}

#[test]
fn a_panicking_sign_in_frees_its_profile() {
    let team = key("panic");
    let result = std::panic::catch_unwind(|| {
        let _ticket = begin(team.clone()).unwrap();
        panic!("synthetic");
    });
    assert!(result.is_err());
    assert!(!running(&team));
    assert!(begin(team).is_ok());
}

#[test]
fn escape_sequences_are_stripped_from_the_output() {
    let text = "\u{1b}[1;32mOK\u{1b}[0m \u{1b}]8;;https://example.com\u{7}Link\u{1b}]8;;\u{1b}\\ end\u{1b}c";
    assert_eq!(strip_ansi(text), "OK Link end");
    assert_eq!(strip_ansi("plain\nline"), "plain\nline");
}

#[test]
fn a_relative_program_never_starts() {
    let run = run_hidden(
        Path::new("powershell.exe"),
        &[],
        ".",
        &AtomicBool::new(false),
        LOGIN_LIMIT,
    );
    assert_eq!(run.end, LoginEnd::NotStarted);
}

#[cfg(unix)]
mod process {
    use super::*;

    fn sh(script: &str, cancel: &AtomicBool, limit: Duration) -> LoginRun {
        let args = ["-c".to_string(), script.to_string()];
        run_hidden(Path::new("/bin/sh"), &args, "/", cancel, limit)
    }

    #[test]
    fn exit_codes_and_closed_stdin_reach_the_verdict() {
        let calm = AtomicBool::new(false);
        let run = sh("printf '\\033[1mdone\\033[0m'; exit 0", &calm, LOGIN_LIMIT);
        assert_eq!(run.end, LoginEnd::Exited(Some(0)));
        assert_eq!(run.output.text(), "done");
        assert_eq!(
            sh("exit 3", &calm, LOGIN_LIMIT).end,
            LoginEnd::Exited(Some(3))
        );
        // A prompt for a pasted code gets end of input at once, never a hang.
        let run = sh("read code || exit 4", &calm, LOGIN_LIMIT);
        assert_eq!(run.end, LoginEnd::Exited(Some(4)));
    }

    #[test]
    fn the_limit_and_a_cancel_end_the_process() {
        let calm = AtomicBool::new(false);
        let started = Instant::now();
        let run = sh("sleep 30", &calm, Duration::from_millis(200));
        assert_eq!(run.end, LoginEnd::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        std::thread::scope(|scope| {
            let running = scope.spawn(|| sh("sleep 30", &cancel, LOGIN_LIMIT));
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::SeqCst);
            assert_eq!(running.join().unwrap().end, LoginEnd::Cancelled);
        });
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_output_is_capped_and_the_rest_is_drained() {
        let calm = AtomicBool::new(false);
        // 200 KiB on both streams: the CLI never blocks on a full pipe, 64 KiB are kept.
        let run = sh(
            "head -c 102400 /dev/zero; head -c 102400 /dev/zero >&2; exit 0",
            &calm,
            LOGIN_LIMIT,
        );
        assert_eq!(run.end, LoginEnd::Exited(Some(0)));
        assert_eq!(run.output.len(), OUTPUT_LIMIT);
    }
}
