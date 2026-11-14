//! The sign-in of a profile (#41): hidden unless „Debug“ is on, and the status watch after a
//! visible one, run against a service in a mutex as the shell does. Synthetic data only.
use super::*;
use crate::launch::tests::decode_command;
use crate::profiles::watch::{self, WatchHost, WATCH_LIMIT, WATCH_TICK};
use crate::profiles::SyncOutcome;
use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[test]
fn a_sign_in_runs_hidden_unless_debug_is_on() {
    let dir = TempDir::new("service-sign-in-mode");
    let mut service = service(&dir);
    let (host, fs) = (windows_host(), fs());
    let SignIn::Hidden(hidden) = service
        .prepare_sign_in(AgentId::Claude, "team", &host, &fs)
        .unwrap()
    else {
        panic!("hidden by default");
    };
    assert_eq!(hidden.cwd, r"C:\Users\max");
    let script = decode_command(hidden.args.last().unwrap());
    assert!(script.contains(&format!("{SET_TEAM}; & '{CLAUDE_EXE}' 'auth' 'login'; ")));
    assert_eq!(fs.created(), [r"C:\Users\max\.claude-team"]);
    // Refused like the terminal, and nothing is created then.
    let refused = |service: &Service, profile: &str| {
        service
            .prepare_sign_in(AgentId::Claude, profile, &host, &FakeFs::default())
            .unwrap_err()
    };
    assert_eq!(refused(&service, "gone"), "Profil nicht gefunden.");
    assert_eq!(
        refused(&service, "team"),
        "Claude Code ist nicht installiert."
    );
    let mut settings = service.settings().clone();
    settings.debug = true;
    service.save_settings(settings).unwrap();
    let SignIn::Visible { plan, fallback } = service
        .prepare_sign_in(AgentId::Claude, "default", &host, &fs)
        .unwrap()
    else {
        panic!("visible with „Debug“");
    };
    assert_eq!(plan.title, "Claude Code · Standard anmelden");
    assert!(plan.args.contains(&"-NoExit".to_string()));
    assert!(fallback.is_none(), "a console needs no fallback");
    assert_eq!(refused(&service, "gone"), "Profil nicht gefunden.");
}

/// The shell's side of a watch: the service behind a mutex, a clock that moves by each sleep
/// and a read that can sign the profile in or panic on a given round.
struct Shell<'a> {
    service: &'a Mutex<Service>,
    clock: Cell<Instant>,
    reads: Cell<u32>,
    on_read: &'a dyn Fn(u32),
    commits: RefCell<Vec<Snapshot>>,
}

impl WatchHost for Shell<'_> {
    fn sleep(&self, duration: Duration) {
        self.clock.set(self.clock.get() + duration);
    }
    fn now(&self) -> Instant {
        self.clock.get()
    }
    fn plan(&self, agent: AgentId) -> crate::profiles::SyncPlan {
        self.service.lock().unwrap().profile_plan().only(agent)
    }
    fn commit(&self, outcome: SyncOutcome) {
        let mut service = self.service.lock().unwrap();
        service.apply_profiles(outcome);
        self.commits.borrow_mut().push(service.snapshot());
    }
    fn read(&self, plan: &crate::profiles::SyncPlan) -> SyncOutcome {
        self.reads.set(self.reads.get() + 1);
        (self.on_read)(self.reads.get());
        plan.read()
    }
}

fn hosted(state: &TempDir, home: &TempDir) -> (Mutex<Service>, std::path::PathBuf) {
    let mut service = service(state);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows)).unwrap();
    let dir = std::path::PathBuf::from(homes.dir(AgentId::Claude, "team").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    service.set_profile_homes(Some(homes));
    service.refresh_profiles();
    (Mutex::new(service), dir)
}

fn shell<'a>(service: &'a Mutex<Service>, on_read: &'a dyn Fn(u32)) -> Shell<'a> {
    Shell {
        service,
        clock: Cell::new(Instant::now()),
        reads: Cell::new(0),
        on_read,
        commits: RefCell::new(Vec::new()),
    }
}

fn team(snapshot: &Snapshot) -> &crate::models::ProfileStatus {
    snapshot.profiles.iter().find(|p| p.id == "team").unwrap()
}

#[test]
fn the_watch_commits_once_when_the_profile_signs_in() {
    let (state, home) = (
        TempDir::new("service-watch-in"),
        TempDir::new("home-watch-in"),
    );
    let (service, dir) = hosted(&state, &home);
    let before = service
        .lock()
        .unwrap()
        .profile_facts(AgentId::Claude, "team");
    assert!(!before.signed_in);
    let credentials = dir.join(".credentials.json");
    let sign_in = |round: u32| {
        if round == 3 {
            std::fs::write(&credentials, "synthetic").unwrap();
        }
    };
    let shell = shell(&service, &sign_in);
    let key = (AgentId::Claude, "team".to_string());
    let revision = service.lock().unwrap().snapshot().revision;
    watch::run(watch::start(key.clone(), before, shell.now()), &shell);
    assert_eq!(
        shell.reads.get(),
        3,
        "every {WATCH_TICK:?} until the change"
    );
    let commits = shell.commits.borrow();
    assert_eq!(commits.len(), 1, "one event, after the change only");
    assert!(team(&commits[0]).signed_in);
    assert!(commits[0].revision > revision);
    assert!(
        service
            .lock()
            .unwrap()
            .profile_facts(AgentId::Claude, "team")
            .signed_in
    );
    assert!(!watch::watching(&key), "the ended watch is gone");
}

#[test]
fn without_a_change_the_watch_ends_at_its_limit_and_commits_nothing() {
    let (state, home) = (
        TempDir::new("service-watch-idle"),
        TempDir::new("home-watch-idle"),
    );
    let (service, _) = hosted(&state, &home);
    let shell = shell(&service, &|_| {});
    let started = shell.now();
    let before = service
        .lock()
        .unwrap()
        .profile_facts(AgentId::Claude, "team");
    let revision = service.lock().unwrap().snapshot().revision;
    watch::run(
        watch::start((AgentId::Claude, "team".into()), before, started),
        &shell,
    );
    let ticks = (WATCH_LIMIT.as_secs() / WATCH_TICK.as_secs()) as u32;
    assert_eq!(shell.reads.get(), ticks);
    assert_eq!(shell.now() - started, WATCH_LIMIT);
    assert!(shell.commits.borrow().is_empty());
    assert_eq!(service.lock().unwrap().snapshot().revision, revision);
}

#[test]
fn a_panicking_read_or_a_newer_watch_ends_the_watch_without_harm() {
    let (state, home) = (
        TempDir::new("service-watch-panic"),
        TempDir::new("home-watch-panic"),
    );
    let (service, _) = hosted(&state, &home);
    let key = (AgentId::Claude, "team".to_string());
    let panics = |round: u32| assert!(round < 2, "synthetic read failure");
    let shell = shell(&service, &panics);
    watch::run(
        watch::start(key.clone(), Facts::default(), shell.now()),
        &shell,
    );
    assert_eq!(shell.reads.get(), 2);
    assert!(shell.commits.borrow().is_empty());
    assert!(!service.is_poisoned());
    assert!(!watch::watching(&key));
    // A second click: the first watch ends at its next tick without reading.
    let quiet = self::shell(&service, &|_| {});
    let first = watch::start(key.clone(), Facts::default(), quiet.now());
    let second = watch::start(key.clone(), Facts::default(), quiet.now());
    watch::run(first, &quiet);
    assert_eq!(quiet.reads.get(), 0);
    assert!(second.current());
}

#[test]
fn exit_code_zero_without_a_sign_in_on_disk_is_a_failed_sign_in() {
    let (state, home) = (
        TempDir::new("service-finish-sign-in"),
        TempDir::new("home-finish-sign-in"),
    );
    let (service, dir) = hosted(&state, &home);
    let mut service = service.into_inner().unwrap();
    let read = |service: &Service| service.profile_plan().only(AgentId::Claude).read();
    // The CLI ended with 0 but wrote nothing: the row must not read „Angemeldet.“
    let outcome = read(&service);
    assert_eq!(
        service
            .finish_sign_in(AgentId::Claude, "team", outcome)
            .unwrap_err(),
        "Anmeldung fehlgeschlagen."
    );
    std::fs::write(dir.join(".credentials.json"), "synthetic").unwrap();
    let outcome = read(&service);
    let snapshot = service
        .finish_sign_in(AgentId::Claude, "team", outcome)
        .unwrap();
    assert!(team(&snapshot).signed_in);
}
