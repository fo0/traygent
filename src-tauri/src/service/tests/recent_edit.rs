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
        .update_recent(&older.id, bypass(" opus "), vec![" 3000 ".into()])
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
        service.update_recent(&local.id, LaunchOptions::default(), vec!["3000".into()]),
        Err(crate::ports::DOCKER_ONLY.into())
    );
    assert!(service
        .update_recent(&local.id, bypass("-p"), Vec::new())
        .is_err());
    let quoted = LaunchOptions {
        extra_args: "\"x\"".into(),
        ..LaunchOptions::default()
    };
    assert!(service
        .update_recent(&local.id, quoted, Vec::new())
        .is_err());
    assert_eq!(
        service.update_recent("unknown", LaunchOptions::default(), Vec::new()),
        Err(RECENT_GONE.into())
    );
    assert_eq!(service.snapshot().recents, before);
    assert_eq!(Service::open(&dir.0).snapshot().recents, before);
}
