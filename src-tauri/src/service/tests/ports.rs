//! Ports through the service: saved settings, prepared launches, recorded and restored recents.
use super::*;

#[test]
fn the_snapshot_offers_the_eight_presets() {
    let presets = Snapshot::native().port_presets;
    assert_eq!(presets, crate::ports::presets());
    assert_eq!(presets.len(), 8);
}

#[test]
fn saved_default_ports_are_normalized_and_invalid_ones_refused() {
    let dir = TempDir::new("service-ports-settings");
    let mut service = Service::open(&dir.0);
    let mut settings = AppSettings::default();
    settings.docker.ports = vec!["3000-3000".into(), "-p".into()];
    assert_eq!(
        service.save_settings(settings.clone()).map(|_| ()),
        Err("Port „-p“ ist ungültig.".into())
    );
    settings.docker.ports = vec![" 3000 ".into(), "8080:80".into()];
    let saved = service.save_settings(settings).unwrap();
    assert_eq!(
        saved.snapshot.settings.docker.ports,
        ["127.0.0.1:3000:3000", "127.0.0.1:8080:80"]
    );
    assert!(!saved.start_detection, "ports never restart a detection");
    assert_eq!(
        Service::open(&dir.0).settings().docker.ports,
        ["127.0.0.1:3000:3000", "127.0.0.1:8080:80"]
    );
}

#[test]
fn a_docker_launch_records_its_normalized_ports() {
    let dir = TempDir::new("service-ports-launch");
    let mut service = Service::open(&dir.0);
    let host = windows_host();
    let fs = FakeFs::with(&[DOCKER, CLAUDE_EXE]).dir(r"C:\x");
    let mut req = request(AgentId::Claude, EnvId::Docker, r"C:\x");
    req.ports = vec!["5173".into(), " ".into()];
    let prepared = service.prepare_launch(&req, &host, &fs, "n").unwrap();
    assert_eq!(prepared.request.ports, ["127.0.0.1:5173:5173"]);
    assert!(script(&prepared.plan).contains("'-p' '127.0.0.1:5173:5173'"));
    let recent = service.record_launch(&prepared.request, Utc::now());
    assert_eq!(recent.ports, ["127.0.0.1:5173:5173"]);
    assert_eq!(
        Service::open(&dir.0).snapshot().recents[0].ports,
        ["127.0.0.1:5173:5173"]
    );
    req.ports = vec!["5173".into(), "5173/tcp".into()];
    assert_eq!(
        service.prepare_launch(&req, &host, &fs, "n"),
        Err("Host-Port 5173 ist doppelt belegt.".into())
    );
    let mut local = request(AgentId::Claude, EnvId::Local, r"C:\x");
    local.ports = vec!["3000".into()];
    assert_eq!(
        service.prepare_launch(&local, &host, &fs, "n"),
        Err(crate::ports::DOCKER_ONLY.into())
    );
}

#[test]
fn restored_recents_keep_valid_docker_ports_only() {
    let dir = TempDir::new("service-ports-restore");
    let mut service = Service::open(&dir.0);
    let mut req = request(AgentId::Codex, EnvId::Docker, r"C:\a");
    req.ports = vec!["4200".into()];
    let recent = service.record_launch(&req, Utc::now());
    service.remove_recent(&recent.id).unwrap();
    let mut typed = recent.clone();
    typed.ports = vec![" 4200/tcp".into()];
    let restored = service.restore_recent(typed).unwrap();
    assert_eq!(restored.recents[0].ports, ["127.0.0.1:4200:4200"]);
    service.remove_recent(&recent.id).unwrap();
    let mut forged = recent.clone();
    forged.ports = vec!["4200 --privileged".into()];
    assert_eq!(
        service.restore_recent(forged),
        Err("Port „4200 --privileged“ ist ungültig.".into())
    );
    let mut local = recent;
    local.env = EnvId::Local;
    assert_eq!(
        service.restore_recent(local),
        Err(crate::ports::DOCKER_ONLY.into())
    );
}
