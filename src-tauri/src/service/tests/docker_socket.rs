//! The host Docker socket (#46) through the service: prepared launches, recorded and edited
//! recents.
use super::*;

const MOUNT: &str = "'-v' '/var/run/docker.sock:/var/run/docker.sock'";

#[test]
fn a_docker_launch_mounts_the_socket_and_records_it() {
    let dir = TempDir::new("service-socket-launch");
    let mut service = Service::open(&dir.0);
    let host = windows_host();
    let fs = FakeFs::with(&[DOCKER, CLAUDE_EXE]).dir(r"C:\x");
    let mut req = request(AgentId::Claude, EnvId::Docker, r"C:\x");
    req.docker_socket = true;
    let prepared = service.prepare_launch(&req, &host, &fs, "n").unwrap();
    assert!(prepared.request.docker_socket);
    assert!(script(&prepared.plan).contains(MOUNT));
    let recent = service.record_launch(&prepared.request, Utc::now());
    assert!(recent.docker_socket);
    assert!(Service::open(&dir.0).snapshot().recents[0].docker_socket);
    // Off: no mount, and the next start replaces the stored value.
    req.docker_socket = false;
    let prepared = service.prepare_launch(&req, &host, &fs, "n").unwrap();
    assert!(!script(&prepared.plan).contains("docker.sock"));
    assert!(
        !service
            .record_launch(&prepared.request, Utc::now())
            .docker_socket
    );
}

#[test]
fn a_local_launch_ignores_the_socket_without_an_error() {
    let dir = TempDir::new("service-socket-local");
    let service = Service::open(&dir.0);
    let fs = FakeFs::with(&[CLAUDE_EXE]).dir(r"C:\x");
    let mut req = request(AgentId::Claude, EnvId::Local, r"C:\x");
    req.docker_socket = true;
    let prepared = service
        .prepare_launch(&req, &windows_host(), &fs, "n")
        .unwrap();
    assert!(!prepared.request.docker_socket, "normalized");
    assert!(!script(&prepared.plan).contains("docker.sock"));
}

#[test]
fn an_edit_stores_the_socket_for_docker_rows_only() {
    let dir = TempDir::new("service-socket-edit");
    let mut service = Service::open(&dir.0);
    let docker =
        service.record_launch(&request(AgentId::Codex, EnvId::Docker, r"C:\a"), Utc::now());
    let local = service.record_launch(&request(AgentId::Codex, EnvId::Local, r"C:\b"), Utc::now());
    let edit = |service: &mut Service, id: &str, socket: bool| {
        let snapshot = service
            .update_recent(
                id,
                LaunchOptions::default(),
                Vec::new(),
                None,
                false,
                socket,
            )
            .unwrap();
        let recent = snapshot.recents.iter().find(|r| r.id == id).unwrap();
        recent.docker_socket
    };
    assert!(edit(&mut service, &docker.id, true));
    let saved = Service::open(&dir.0).snapshot().recents;
    assert!(
        saved
            .iter()
            .find(|r| r.id == docker.id)
            .unwrap()
            .docker_socket
    );
    assert!(!edit(&mut service, &docker.id, false));
    assert!(
        !edit(&mut service, &local.id, true),
        "ignored outside Docker"
    );
}
