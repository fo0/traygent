//! The sync runs of the service, on a temp dir standing in for the user profile.
use super::*;

/// A service whose profiles live in a temp dir standing in for the user profile.
fn hosted(state: &TempDir, home: &TempDir) -> (Service, crate::profiles::Homes) {
    let mut service = service(state);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows)).unwrap();
    service.set_profile_homes(Some(homes.clone()));
    (service, homes)
}

#[test]
fn the_sync_before_a_start_warns_and_only_a_blocked_profile_directory_stops_it() {
    let (state, home) = (
        TempDir::new("service-profiles-sync"),
        TempDir::new("home-sync"),
    );
    let (mut service, homes) = hosted(&state, &home);
    let linker: Box<dyn crate::profiles::Linker> = Box::new(crate::testing::TestLinker);
    let local = |profile: Option<&str>| request(AgentId::Claude, EnvId::Local, profile);
    let dir = |id: &str| std::path::PathBuf::from(homes.dir(AgentId::Claude, id).unwrap());
    std::fs::create_dir_all(dir("default")).unwrap();
    std::fs::write(dir("default").join("settings.json"), "{}").unwrap();
    std::fs::write(dir("default").join(".credentials.json"), "synthetic").unwrap();
    // A clean sync: no warning, every profile in line, the statuses read anew.
    let revision = service.snapshot().revision;
    assert_eq!(
        service.sync_for_launch(&local(Some("team")), linker.as_ref(), at(1)),
        Ok(None)
    );
    assert_eq!(
        std::fs::read_to_string(dir("solo").join("settings.json")).unwrap(),
        "{}"
    );
    let snapshot = service.snapshot();
    assert!(snapshot.revision > revision);
    let status = |snapshot: &Snapshot, id: &str| {
        snapshot
            .profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .clone()
    };
    assert!(status(&snapshot, "default").signed_in);
    assert!(!status(&snapshot, "team").signed_in);
    assert_eq!(
        status(&snapshot, "team").synced_at.as_deref(),
        Some("2026-11-13T09:01:00Z")
    );
    assert_eq!(status(&snapshot, "team").sync_error, None);
    // Something the sync cannot do is a warning; the start goes on.
    std::fs::remove_dir(dir("default").join("plans")).unwrap();
    std::fs::write(dir("default").join("plans"), "a file").unwrap();
    let warning = service
        .sync_for_launch(&local(None), linker.as_ref(), at(2))
        .unwrap()
        .unwrap();
    // One sentence; the detail is each profile's `syncError`.
    assert_eq!(
        warning,
        "Profile konnten nicht vollständig synchronisiert werden."
    );
    assert_eq!(
        status(&service.snapshot(), "team").sync_error.as_deref(),
        Some("plans: ist im Standard-Profil kein Ordner")
    );
    let snapshot = service.snapshot();
    assert_eq!(
        status(&snapshot, "solo").sync_error.as_deref(),
        Some("plans: ist im Standard-Profil kein Ordner")
    );
    // The time of the last sync without an error stays.
    assert_eq!(
        status(&snapshot, "solo").synced_at.as_deref(),
        Some("2026-11-13T09:01:00Z")
    );
    // `sync_profiles` is the same sync on demand; fixed, the error is gone.
    std::fs::remove_file(dir("default").join("plans")).unwrap();
    assert_eq!(service.sync_profiles(linker.as_ref(), at(3)), None);
    assert_eq!(status(&service.snapshot(), "solo").sync_error, None);
    // A profile whose directory cannot be created: its own start fails, any other warns.
    let mut settings = service.settings().clone();
    settings
        .profiles
        .claude
        .push(profile("blocked", "Blockiert"));
    assert!(service.save_settings(settings).unwrap().profiles_changed);
    std::fs::write(dir("blocked"), "a file in the way").unwrap();
    assert_eq!(
        service.sync_for_launch(&local(Some("blocked")), linker.as_ref(), at(4)),
        Err("Der Profilordner konnte nicht angelegt werden.".into())
    );
    let warning = service.sync_for_launch(&local(Some("team")), linker.as_ref(), at(5));
    assert_eq!(
        warning.unwrap().unwrap(),
        "Profile konnten nicht vollständig synchronisiert werden."
    );
    assert_eq!(
        status(&service.snapshot(), "blocked").sync_error.as_deref(),
        Some("Profilordner konnte nicht angelegt werden")
    );
    // A removed profile leaves its directory on disk and no status behind.
    let mut settings = service.settings().clone();
    settings
        .profiles
        .claude
        .retain(|profile| profile.id == "team");
    service.save_settings(settings).unwrap();
    service.refresh_profiles();
    let ids: Vec<_> = service
        .snapshot()
        .profiles
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, ["default", "team"]);
    assert!(dir("solo").join("settings.json").exists());
    assert!(dir("blocked").is_file());
}

#[test]
fn nothing_is_synced_without_an_extra_profile_or_for_another_agent() {
    let (state, home) = (
        TempDir::new("service-profiles-idle"),
        TempDir::new("home-idle"),
    );
    let mut service = Service::open(&state.0);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows));
    service.set_profile_homes(homes);
    let linker: Box<dyn crate::profiles::Linker> = Box::new(crate::testing::TestLinker);
    for agent in AgentId::ALL {
        let synced =
            service.sync_for_launch(&request(agent, EnvId::Local, None), linker.as_ref(), at(1));
        assert_eq!(synced, Ok(None));
    }
    assert_eq!(service.sync_profiles(linker.as_ref(), at(1)), None);
    assert!(home.entries().is_empty(), "nothing is created");
    assert!(service.take_startup_profile_sync());
    assert!(!service.take_startup_profile_sync());
}

/// `~/.claude-mem` and the like: an existing directory that is not Claude Code's is never
/// adopted, and no session starts in it.
#[test]
fn a_foreign_directory_is_not_adopted_and_its_profile_does_not_start() {
    let (state, home) = (
        TempDir::new("service-profiles-foreign"),
        TempDir::new("home-foreign"),
    );
    let mut service = Service::open(&state.0);
    let homes = crate::profiles::Homes::new(home.0.to_str().unwrap(), cfg!(windows)).unwrap();
    service.set_profile_homes(Some(homes.clone()));
    let mut settings = with_profiles(&[("mem", "Mem"), ("team", "Team")]);
    settings.agent_profile.claude = "team".into();
    service.save_settings(settings).unwrap();
    let foreign = std::path::PathBuf::from(homes.dir(AgentId::Claude, "mem").unwrap());
    std::fs::create_dir_all(foreign.join("skills")).unwrap();
    std::fs::write(foreign.join("skills/notes.md"), "someone else's").unwrap();
    std::fs::write(foreign.join("settings.json"), "theirs").unwrap();
    let linker = crate::testing::TestLinker;
    let local = |profile: &str| request(AgentId::Claude, EnvId::Local, Some(profile));
    assert_eq!(
        service.sync_for_launch(&local("mem"), &linker, at(1)),
        Err("Der Profilordner gehört nicht zu Claude Code.".into())
    );
    // Another profile starts, with the warning; the foreign directory is as it was.
    assert_eq!(
        service.sync_for_launch(&local("team"), &linker, at(2)),
        Ok(Some(
            "Profile konnten nicht vollständig synchronisiert werden.".into()
        ))
    );
    let snapshot = service.snapshot();
    let mem = snapshot.profiles.iter().find(|p| p.id == "mem").unwrap();
    assert_eq!(
        mem.sync_error.as_deref(),
        Some("Profilordner gehört nicht zu Claude Code")
    );
    let mut names: Vec<_> = std::fs::read_dir(&foreign)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["settings.json", "skills"]);
    assert!(foreign.join("skills").symlink_metadata().unwrap().is_dir());
    assert_eq!(
        std::fs::read_to_string(foreign.join("settings.json")).unwrap(),
        "theirs"
    );
    let default = std::path::PathBuf::from(homes.dir(AgentId::Claude, "default").unwrap());
    assert!(!default.join("skills/notes.md").exists());
}
