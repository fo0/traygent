//! The hidden sign-in's decision logic (an injected runner, the wait loop over a fake
//! process) and, on Unix, the real process with `/bin/sh` standing in for PowerShell.
//! Synthetic data only.
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
    fn run(&self, _: &HiddenLogin, cancel: &AtomicBool, limit: Duration) -> LoginEnd {
        self.0.clone().unwrap_or_else(|| {
            assert_eq!(limit, LOGIN_LIMIT);
            while !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            LoginEnd::Cancelled
        })
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
        // Registered before the work starts: a cancel can never miss it.
        assert!(cancel(&team));
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
fn the_apps_exit_ends_every_running_sign_in() {
    let exit = key("app-exit");
    let ticket = begin(exit.clone()).unwrap();
    std::thread::scope(|scope| {
        let waiting = scope.spawn(move || {
            let result = sign_in(&Scripted(None), &plan(), &ticket);
            drop(ticket);
            result
        });
        // Other tests may hold tickets of their own: only this one is checked.
        end_all(Duration::from_millis(500));
        assert_eq!(
            waiting.join().unwrap(),
            Err("Anmeldung abgebrochen.".into())
        );
    });
    assert!(!running(&exit));
}

/// A fake process for the wait loop: its exit code per poll, and whether it was stopped.
struct Fake {
    polls: Vec<Option<Option<i32>>>,
    stopped: bool,
}

fn watched(polls: Vec<Option<Option<i32>>>, cancelled: bool, limit: Duration) -> (LoginEnd, bool) {
    let mut fake = Fake {
        polls,
        stopped: false,
    };
    let end = watch(
        &mut fake,
        |fake| {
            Ok(if fake.polls.is_empty() {
                None
            } else {
                fake.polls.remove(0)
            })
        },
        |fake| fake.stopped = true,
        &AtomicBool::new(cancelled),
        Instant::now() + limit,
    );
    (end, fake.stopped)
}

#[test]
fn an_end_in_the_same_tick_as_a_cancel_or_the_limit_counts_as_ended() {
    // Ended and cancelled at once: the status is read, nothing is stopped.
    assert_eq!(
        watched(vec![Some(Some(0))], true, Duration::ZERO),
        (LoginEnd::Exited(Some(0)), false)
    );
    assert_eq!(
        watched(vec![None], true, LOGIN_LIMIT),
        (LoginEnd::Cancelled, true)
    );
    assert_eq!(
        watched(vec![None], false, Duration::ZERO),
        (LoginEnd::TimedOut, true)
    );
    assert_eq!(
        watched(vec![None, None, Some(Some(2))], false, LOGIN_LIMIT),
        (LoginEnd::Exited(Some(2)), false)
    );
}

#[test]
fn a_relative_program_never_starts() {
    let end = run_hidden(
        Path::new("powershell.exe"),
        &[],
        ".",
        &AtomicBool::new(false),
        LOGIN_LIMIT,
    );
    assert_eq!(end, LoginEnd::NotStarted);
}

#[cfg(unix)]
mod process {
    use super::*;

    fn sh(script: &str, cancel: &AtomicBool, limit: Duration) -> LoginEnd {
        let args = ["-c".to_string(), script.to_string()];
        run_hidden(Path::new("/bin/sh"), &args, "/", cancel, limit)
    }

    #[test]
    fn exit_codes_and_closed_stdin_reach_the_verdict() {
        let calm = AtomicBool::new(false);
        assert_eq!(sh("exit 0", &calm, LOGIN_LIMIT), LoginEnd::Exited(Some(0)));
        assert_eq!(sh("exit 3", &calm, LOGIN_LIMIT), LoginEnd::Exited(Some(3)));
        // A prompt for a pasted code gets end of input at once, never a hang.
        assert_eq!(
            sh("read code || exit 4", &calm, LOGIN_LIMIT),
            LoginEnd::Exited(Some(4))
        );
    }

    #[test]
    fn the_limit_and_a_cancel_end_the_process() {
        let calm = AtomicBool::new(false);
        let started = Instant::now();
        let end = sh("sleep 30", &calm, Duration::from_millis(200));
        assert_eq!(end, LoginEnd::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        std::thread::scope(|scope| {
            let running = scope.spawn(|| sh("sleep 30", &cancel, LOGIN_LIMIT));
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::SeqCst);
            assert_eq!(running.join().unwrap(), LoginEnd::Cancelled);
        });
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn output_goes_nowhere_and_never_blocks_the_cli() {
        let calm = AtomicBool::new(false);
        // 200 KiB on both streams would fill any pipe: the null device takes all of it.
        let end = sh(
            "head -c 102400 /dev/zero; head -c 102400 /dev/zero >&2; exit 0",
            &calm,
            LOGIN_LIMIT,
        );
        assert_eq!(end, LoginEnd::Exited(Some(0)));
    }
}
