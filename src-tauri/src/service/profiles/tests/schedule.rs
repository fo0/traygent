//! The background sync schedule (#37): when a sync is due. Synthetic data only.
use super::*;
use std::time::{Duration, Instant};

const SECOND: Duration = Duration::from_secs(1);

/// A service with the profiles „Team“ and „Solo“ whose homes live in a temp dir.
fn hosted(state: &TempDir, home: &TempDir) -> Service {
    let mut service = service(state);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows)).unwrap();
    service.set_profile_homes(Some(homes));
    service
}

#[test]
fn the_first_background_sync_is_due_one_interval_after_the_start() {
    let (state, home) = (
        TempDir::new("service-schedule-first"),
        TempDir::new("home-schedule-first"),
    );
    let service = hosted(&state, &home);
    let started = Instant::now();
    assert_eq!(PROFILE_SYNC_INTERVAL, Duration::from_secs(15 * 60));
    assert!(!service.profile_sync_due(started, started));
    assert!(!service.profile_sync_due(started, started + PROFILE_SYNC_INTERVAL - SECOND));
    assert!(service.profile_sync_due(started, started + PROFILE_SYNC_INTERVAL));
    assert!(service.profile_sync_due(started, started + 4 * PROFILE_SYNC_INTERVAL));
}

#[test]
fn every_kind_of_sync_restarts_the_interval_and_a_read_does_not() {
    let (state, home) = (
        TempDir::new("service-schedule-reset"),
        TempDir::new("home-schedule-reset"),
    );
    let mut service = hosted(&state, &home);
    let linker = crate::testing::TestLinker;
    let started = Instant::now();
    let late = started + 2 * PROFILE_SYNC_INTERVAL;
    // Not due again until a full interval after the sync that just finished.
    let restarted = |service: &Service| {
        let now = Instant::now();
        !service.profile_sync_due(started, now + PROFILE_SYNC_INTERVAL - 60 * SECOND)
            && service.profile_sync_due(started, now + PROFILE_SYNC_INTERVAL + SECOND)
    };
    // Reading the statuses (every later detection) is no sync.
    service.refresh_profiles();
    assert!(service.profile_sync_due(started, late));
    // The startup sync, one after a save and the background sync all commit a plan's run.
    let outcome = service.profile_plan().run(&linker, at(1));
    service.apply_profiles(outcome);
    assert!(restarted(&service));
    // `sync_profiles` on demand.
    service.sync_profiles(&linker, at(2));
    assert!(restarted(&service));
    // The sync before a local start (that agent only).
    let local = request(AgentId::Claude, EnvId::Local, None);
    service.sync_for_launch(&local, &linker, at(3)).unwrap();
    assert!(restarted(&service));
    // A background sync that broke down waits the full interval too.
    let now = Instant::now() + 3 * PROFILE_SYNC_INTERVAL;
    service.skip_profile_sync(now);
    assert!(!service.profile_sync_due(started, now + PROFILE_SYNC_INTERVAL - SECOND));
    assert!(service.profile_sync_due(started, now + PROFILE_SYNC_INTERVAL));
}

#[test]
fn without_an_extra_profile_or_known_homes_no_background_sync_is_ever_due() {
    let (state, home) = (
        TempDir::new("service-schedule-never"),
        TempDir::new("home-schedule-never"),
    );
    let started = Instant::now();
    let late = started + 100 * PROFILE_SYNC_INTERVAL;
    // Homes known, but no agent has an extra profile.
    let mut service = Service::open(&state.0);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows)).unwrap();
    service.set_profile_homes(Some(homes));
    assert!(!service.profile_sync_due(started, late));
    // Profiles listed, but the user profile is unknown.
    let unhosted = TempDir::new("service-schedule-unhosted");
    let service = super::service(&unhosted);
    assert!(!service.profile_sync_due(started, late));
    // Removing the last extra profile ends the schedule.
    let removed = TempDir::new("service-schedule-removed");
    let mut service = hosted(&removed, &home);
    assert!(service.profile_sync_due(started, late));
    service.save_settings(AppSettings::default()).unwrap();
    assert!(!service.profile_sync_due(started, late));
}
