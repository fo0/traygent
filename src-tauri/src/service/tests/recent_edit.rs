//! „Bearbeiten …“ of a „Zuletzt“ row through the service.
use super::*;

fn bypass(model: &str) -> LaunchOptions {
    LaunchOptions {
        model: model.into(),
        permission: PermissionId::Bypass,
        ..LaunchOptions::default()
    }
}

#[test]
fn an_edited_recent_keeps_its_place_time_and_count_and_is_saved() {
    let dir = TempDir::new("service-recents-edit");
    let mut service = Service::open(&dir.0);
    let older = service.record_launch(
        &request(AgentId::Claude, EnvId::Docker, r"C:\a"),
        "2026-11-03T08:00:00Z".parse().unwrap(),
    );
    let newer = service.record_launch(
        &request(AgentId::Claude, EnvId::Local, r"C:\b"),
        "2026-11-03T09:00:00Z".parse().unwrap(),
    );
    let before = service.snapshot().revision;
    let snapshot = service
        .update_recent(&older.id, bypass(" opus "), vec![" 3000 ".into()], None)
        .unwrap();
    assert!(snapshot.revision > before);
    let ids: Vec<_> = snapshot.recents.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, [newer.id, older.id.clone()], "the order stays");
    let edited = &snapshot.recents[1];
    assert_eq!(edited.options, bypass("opus"), "stored normalized");
    assert_eq!(edited.ports, ["127.0.0.1:3000:3000"]);
    assert_eq!(edited.last_started_at, older.last_started_at);
    assert_eq!(edited.starts, older.starts);
    assert_eq!(Service::open(&dir.0).snapshot().recents, snapshot.recents);
}

#[test]
fn an_invalid_edit_or_a_gone_entry_changes_nothing() {
    let dir = TempDir::new("service-recents-edit-invalid");
    let mut service = Service::open(&dir.0);
    let local = service.record_launch(&request(AgentId::Claude, EnvId::Local, r"C:\a"), Utc::now());
    let before = service.snapshot().recents;
    assert_eq!(
        service.update_recent(
            &local.id,
            LaunchOptions::default(),
            vec!["3000".into()],
            None
        ),
        Err(crate::ports::DOCKER_ONLY.into())
    );
    assert!(service
        .update_recent(&local.id, bypass("-p"), Vec::new(), None)
        .is_err());
    let quoted = LaunchOptions {
        extra_args: "\"x\"".into(),
        ..LaunchOptions::default()
    };
    assert!(service
        .update_recent(&local.id, quoted, Vec::new(), None)
        .is_err());
    assert_eq!(
        service.update_recent("unknown", LaunchOptions::default(), Vec::new(), None),
        Err(RECENT_GONE.into())
    );
    assert_eq!(service.snapshot().recents, before);
    assert_eq!(Service::open(&dir.0).snapshot().recents, before);
}

/// A service whose Claude Code has the profile „Team“.
fn with_team(dir: &TempDir) -> Service {
    let mut service = Service::open(&dir.0);
    let mut settings = crate::models::AppSettings::default();
    settings.profiles.claude = vec![crate::models::AgentProfile {
        id: "team".into(),
        label: "Team".into(),
    }];
    service.save_settings(settings).unwrap();
    service
}

#[test]
fn an_edit_stores_the_profile_normalized_like_a_start() {
    let dir = TempDir::new("service-recents-edit-profile");
    let mut service = with_team(&dir);
    let local = service.record_launch(&request(AgentId::Claude, EnvId::Local, r"C:\a"), Utc::now());
    let edit = |service: &mut Service, profile: Option<&str>| {
        let snapshot = service
            .update_recent(
                &local.id,
                LaunchOptions::default(),
                Vec::new(),
                profile.map(Into::into),
            )
            .unwrap();
        snapshot.recents[0].profile.clone()
    };
    assert_eq!(edit(&mut service, Some(" team ")).as_deref(), Some("team"));
    assert_eq!(
        Service::open(&dir.0).snapshot().recents[0]
            .profile
            .as_deref(),
        Some("team"),
        "saved"
    );
    for standard in [None, Some(""), Some("  "), Some("default")] {
        edit(&mut service, Some("team"));
        assert_eq!(edit(&mut service, standard), None, "{standard:?}");
    }
    // A container has its own home: a Docker entry never stores a profile.
    let docker = service.record_launch(
        &request(AgentId::Claude, EnvId::Docker, r"C:\b"),
        Utc::now(),
    );
    let snapshot = service
        .update_recent(
            &docker.id,
            LaunchOptions::default(),
            Vec::new(),
            Some("team".into()),
        )
        .unwrap();
    let edited = snapshot.recents.iter().find(|r| r.id == docker.id).unwrap();
    assert_eq!(edited.profile, None);
}

#[test]
fn an_unlisted_profile_is_refused_and_changes_nothing() {
    let dir = TempDir::new("service-recents-edit-profile-unknown");
    let mut service = with_team(&dir);
    let claude =
        service.record_launch(&request(AgentId::Claude, EnvId::Local, r"C:\a"), Utc::now());
    let codex = service.record_launch(&request(AgentId::Codex, EnvId::Local, r"C:\b"), Utc::now());
    let before = service.snapshot().recents;
    for (id, profile) in [
        (&claude.id, "gone"),
        (&claude.id, "Team"),
        (&codex.id, "team"),
    ] {
        assert_eq!(
            service.update_recent(id, bypass("opus"), Vec::new(), Some(profile.into())),
            Err(PROFILE_MISSING.into()),
            "{profile}"
        );
    }
    assert_eq!(service.snapshot().recents, before);
    // An agent without profiles stores none, as before.
    let snapshot = service
        .update_recent(&codex.id, LaunchOptions::default(), Vec::new(), None)
        .unwrap();
    let edited = snapshot.recents.iter().find(|r| r.id == codex.id).unwrap();
    assert_eq!(edited.profile, None);
}
