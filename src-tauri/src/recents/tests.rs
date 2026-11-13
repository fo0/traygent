use super::*;
use crate::models::{LaunchOptions, PermissionId, SessionMode};
use chrono::TimeZone;

fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 3, 9, minute, 0).unwrap()
}

fn request(folder: &str, agent: AgentId, env: EnvId) -> LaunchRequest {
    LaunchRequest {
        agent,
        env,
        folder: folder.into(),
        repo: None,
        options: LaunchOptions::default(),
        session: SessionMode::New,
        profile: None,
        ports: Vec::new(),
    }
}

#[test]
fn ids_are_stable_and_ignore_folder_case() {
    let id = recent_id("C:\\Code\\Demo", AgentId::Claude, EnvId::Local);
    assert_eq!(id.len(), 16);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(
        id,
        recent_id("c:\\code\\DEMO", AgentId::Claude, EnvId::Local)
    );
    assert_ne!(
        id,
        recent_id("C:\\Code\\Demo", AgentId::Codex, EnvId::Local)
    );
    assert_ne!(
        id,
        recent_id("C:\\Code\\Demo", AgentId::Claude, EnvId::Docker)
    );
    // Pinned: ids survive releases, so a change here orphans every stored entry.
    assert_eq!(
        id,
        recent_id("C:\\CODE\\DEMO", AgentId::Claude, EnvId::Local)
    );
    assert_eq!(hex_prefix(&[0xab, 0x01], 3), "ab0");
}

#[test]
fn upsert_inserts_first_and_bumps_existing_entries() {
    let mut list = Vec::new();
    let first = upsert(
        &mut list,
        &request("C:\\a", AgentId::Claude, EnvId::Local),
        at(1),
    );
    assert_eq!(first.starts, 1);
    assert_eq!(first.last_started_at, "2026-11-03T09:01:00Z");
    upsert(
        &mut list,
        &request("C:\\b", AgentId::Claude, EnvId::Local),
        at(2),
    );
    let mut again = request("c:\\A", AgentId::Claude, EnvId::Local);
    again.options.permission = PermissionId::Bypass;
    let bumped = upsert(&mut list, &again, at(3));
    assert_eq!(bumped.starts, 2);
    assert_eq!(bumped.id, first.id);
    assert_eq!(bumped.folder, "c:\\A");
    assert_eq!(bumped.options.permission, PermissionId::Bypass);
    let folders: Vec<_> = list.iter().map(|r| r.folder.as_str()).collect();
    assert_eq!(folders, ["c:\\A", "C:\\b"]);
}

#[test]
fn upsert_keeps_at_most_twenty_newest_first() {
    let mut list = Vec::new();
    for minute in 0..25 {
        let folder = format!("C:\\f{minute}");
        upsert(
            &mut list,
            &request(&folder, AgentId::Codex, EnvId::Docker),
            at(minute),
        );
    }
    assert_eq!(list.len(), MAX_RECENTS);
    assert_eq!(list[0].folder, "C:\\f24");
    assert_eq!(list[19].folder, "C:\\f5");
}

#[test]
fn remove_and_restore_round_trip_in_time_order() {
    let mut list = Vec::new();
    for minute in 1..=3 {
        let folder = format!("C:\\f{minute}");
        upsert(
            &mut list,
            &request(&folder, AgentId::Claude, EnvId::Local),
            at(minute),
        );
    }
    let id = list[1].id.clone();
    let removed = remove(&mut list, &id).unwrap();
    assert_eq!(removed.folder, "C:\\f2");
    assert!(remove(&mut list, &id).is_none());
    restore(&mut list, removed.clone()).unwrap();
    let folders: Vec<_> = list.iter().map(|r| r.folder.as_str()).collect();
    assert_eq!(folders, ["C:\\f3", "C:\\f2", "C:\\f1"]);
    // Restoring twice keeps one entry.
    restore(&mut list, removed).unwrap();
    assert_eq!(list.len(), 3);
}

#[test]
fn restore_keeps_a_newer_restart_and_recomputes_the_id() {
    let mut list = Vec::new();
    let old = upsert(
        &mut list,
        &request("C:\\a", AgentId::Claude, EnvId::Local),
        at(1),
    );
    remove(&mut list, &old.id);
    let newer = upsert(
        &mut list,
        &request("C:\\a", AgentId::Claude, EnvId::Local),
        at(5),
    );
    restore(&mut list, old.clone()).unwrap();
    assert_eq!(list, vec![newer]);
    let mut forged = old;
    forged.id = "anything".into();
    let mut empty = Vec::new();
    restore(&mut empty, forged).unwrap();
    assert_eq!(
        empty[0].id,
        recent_id("C:\\a", AgentId::Claude, EnvId::Local)
    );
}

#[test]
fn restore_rejects_unreadable_entries() {
    let mut list = Vec::new();
    let mut recent = upsert(
        &mut list,
        &request("C:\\a", AgentId::Claude, EnvId::Local),
        at(1),
    );
    recent.last_started_at = "yesterday".into();
    assert!(restore(&mut Vec::new(), recent.clone()).is_err());
    recent.last_started_at = timestamp(at(2));
    recent.starts = 0;
    assert!(restore(&mut Vec::new(), recent).is_err());
}

#[test]
fn tidy_sorts_dedups_and_caps_loaded_lists() {
    let mut list = Vec::new();
    for minute in 0..22 {
        let folder = format!("C:\\f{minute}");
        upsert(
            &mut list,
            &request(&folder, AgentId::Claude, EnvId::Local),
            at(minute),
        );
    }
    let mut loaded: Vec<_> = list.iter().rev().cloned().collect();
    loaded.push(list[0].clone());
    let mut broken = list[1].clone();
    broken.last_started_at = "?".into();
    broken.id = "broken".into();
    loaded.push(broken);
    let tidy = tidy(loaded);
    assert_eq!(tidy.len(), 20);
    assert_eq!(tidy[0].folder, "C:\\f21");
    assert!(tidy.iter().all(|r| r.id != "broken"));
}

#[test]
fn upsert_stores_normalized_ports_and_replaces_them_on_the_next_start() {
    let mut list = Vec::new();
    let mut docker = request("C:\\a", AgentId::Claude, EnvId::Docker);
    docker.ports = vec![" 3000".into(), "".into(), "0.0.0.0:8080:80/udp".into()];
    let first = upsert(&mut list, &docker, at(1));
    assert_eq!(first.ports, ["127.0.0.1:3000:3000", "0.0.0.0:8080:80/udp"]);
    docker.ports = vec!["5173".into()];
    let second = upsert(&mut list, &docker, at(2));
    assert_eq!(second.ports, ["127.0.0.1:5173:5173"]);
    docker.ports.clear();
    assert!(upsert(&mut list, &docker, at(3)).ports.is_empty());
    // A local start never keeps ports; an invalid list (refused by the launch) is not stored.
    let mut local = request("C:\\b", AgentId::Claude, EnvId::Local);
    local.ports = vec!["3000".into()];
    assert!(upsert(&mut list, &local, at(4)).ports.is_empty());
    docker.ports = vec!["3000;calc".into()];
    assert!(upsert(&mut list, &docker, at(5)).ports.is_empty());
}

#[test]
fn restore_and_tidy_keep_valid_ports() {
    let mut list = Vec::new();
    let mut docker = request("C:\\a", AgentId::Codex, EnvId::Docker);
    docker.ports = vec!["3000-3001".into()];
    let recent = upsert(&mut list, &docker, at(1));
    remove(&mut list, &recent.id);
    restore(&mut list, recent.clone()).unwrap();
    assert_eq!(list[0].ports, ["127.0.0.1:3000-3001:3000-3001"]);
    let mut tampered = recent;
    tampered.ports = vec!["--privileged".into()];
    assert!(tidy(vec![tampered]).remove(0).ports.is_empty());
}

#[test]
fn a_start_by_path_keeps_the_repository_of_the_same_folder() {
    let repo = RepoRef {
        owner: "fo0".into(),
        name: "traygent".into(),
    };
    let folder = "C:\\Users\\max\\Traygent\\fo0\\traygent";
    let mut list = Vec::new();
    let cloned = LaunchRequest {
        repo: Some(repo.clone()),
        ..request(folder, AgentId::Claude, EnvId::Local)
    };
    upsert(&mut list, &cloned, at(1));
    let by_path = request(&folder.to_lowercase(), AgentId::Claude, EnvId::Local);
    let again = upsert(&mut list, &by_path, at(2));
    assert_eq!((again.starts, again.repo.as_ref()), (2, Some(&repo)));
    let other = upsert(
        &mut list,
        &request("C:\\x", AgentId::Claude, EnvId::Local),
        at(3),
    );
    assert_eq!(other.repo, None);
}

#[test]
fn the_profile_follows_the_latest_start_and_the_id_ignores_it() {
    let mut list = Vec::new();
    let with = |profile: Option<&str>, env: EnvId| LaunchRequest {
        profile: profile.map(Into::into),
        ..request("C:\\a", AgentId::Claude, env)
    };
    let team = upsert(&mut list, &with(Some("team"), EnvId::Local), at(1));
    assert_eq!(team.profile.as_deref(), Some("team"));
    assert_eq!(team.id, recent_id("C:\\a", AgentId::Claude, EnvId::Local));
    let solo = upsert(&mut list, &with(Some("solo"), EnvId::Local), at(2));
    assert_eq!((solo.id.as_str(), solo.starts), (team.id.as_str(), 2));
    assert_eq!(solo.profile.as_deref(), Some("solo"));
    // Unlike the repository, the profile is replaced like the options: none is the default.
    let plain = upsert(&mut list, &with(None, EnvId::Local), at(3));
    assert_eq!(
        (plain.profile.as_deref(), plain.starts, list.len()),
        (None, 3, 1)
    );
    // Only a local session has one; the default and a malformed id are stored as none.
    assert_eq!(
        upsert(&mut list, &with(Some("team"), EnvId::Docker), at(4)).profile,
        None
    );
    for id in ["default", "", "Team", "..\\x", "a b"] {
        assert_eq!(stored_profile(EnvId::Local, Some(id)), None, "{id}");
    }
    assert_eq!(stored_profile(EnvId::Wsl, Some("team")), None);
    let mut tampered = team;
    tampered.profile = Some("..\\..\\evil".into());
    assert_eq!(tidy(vec![tampered]).remove(0).profile, None);
}
