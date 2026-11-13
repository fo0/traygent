//! The profile model and the sync engine, on a temp dir standing in for the user profile.
//! Synthetic data only: every account, path and secret here is made up.
use super::links::Link;
use super::status::{self, Facts};
use super::sync::{read_marker, sync_agent, MARKER};
use super::*;
use crate::models::{AgentProfile, AppSettings};
use crate::testing::TempDir;
use chrono::{DateTime, TimeZone, Utc};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

mod files;
mod links;
mod state;

const CLAUDE: AgentId = AgentId::Claude;

pub(super) const DEFAULT_STATE: &str = r#"{
  "numStartups": 5,
  "oauthAccount": {
    "emailAddress": "max@example.test"
  },
  "mcpServers": {
    "docs": {
      "command": "docs-mcp"
    }
  },
  "projects": {
    "C:/Code/demo": {
      "hasTrustDialogAccepted": true
    }
  }
}
"#;
pub(super) const TEAM_STATE: &str = r#"{
  "installMethod": "native",
  "oauthAccount": {
    "emailAddress": "team@example.test",
    "organizationName": "Team GmbH"
  },
  "mcpServers": {},
  "bigNumber": 12345678901234567890.5000,
  "numStartups": 1
}
"#;

pub(super) fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 13, 9, minute, 0).unwrap()
}

pub(super) fn profile(id: &str) -> AgentProfile {
    AgentProfile {
        id: id.into(),
        label: id.to_uppercase(),
    }
}

/// A user profile in a temp dir, with the default home `.claude` beside the profiles.
pub(super) struct World {
    tmp: TempDir,
    pub homes: Homes,
}

impl World {
    pub fn new(name: &str) -> Self {
        let tmp = TempDir::new(name);
        let homes = Homes::new(tmp.0.to_str().unwrap(), cfg!(windows)).unwrap();
        Self { tmp, homes }
    }

    pub fn home(&self) -> &Path {
        &self.tmp.0
    }

    /// The config directory of profile `id` (`default`: the CLI's own home).
    pub fn dir(&self, id: &str) -> PathBuf {
        PathBuf::from(self.homes.dir(CLAUDE, id).unwrap())
    }

    pub fn state(&self, id: &str) -> PathBuf {
        PathBuf::from(self.homes.state_file(CLAUDE, id).unwrap())
    }

    pub fn sync(&self, ids: &[&str], minute: u32) -> Vec<ProfileReport> {
        let profiles: Vec<_> = ids.iter().map(|id| profile(id)).collect();
        sync_agent(
            &self.homes,
            CLAUDE,
            &profiles,
            linker().as_ref(),
            at(minute),
        )
    }

    /// Syncs and expects no error.
    pub fn synced(&self, ids: &[&str], minute: u32) -> Vec<ProfileReport> {
        let reports = self.sync(ids, minute);
        for report in &reports {
            assert_eq!(report.errors, Vec::<String>::new(), "{}", report.id);
        }
        reports
    }

    /// The one folder a run left under the profile's backup directory.
    pub fn backup(&self, id: &str) -> PathBuf {
        let root = self.dir(id).join(super::files::BACKUP_DIR);
        let mut runs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(runs.len(), 1, "{runs:?}");
        runs.remove(0)
    }
}

pub(super) fn linker() -> Box<dyn Linker> {
    Box::new(crate::testing::TestLinker)
}

/// Makes an existing profile directory the agent's own: without the state file, a sign-in
/// or the marker a non-empty directory is someone else's and is not touched.
pub(super) fn own(world: &World, id: &str) {
    write(&world.dir(id).join(".credentials.json"), "synthetic");
}

pub(super) fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

pub(super) fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// Sets the file time to `seconds` after a fixed day, so "newer" never depends on the clock.
pub(super) fn touch(path: &Path, seconds: u64) {
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds);
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(time).unwrap();
}

/// Every file under `root` (links not followed) with its content and time.
pub(super) fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>, SystemTime)> {
    let mut found = Vec::new();
    let mut open = vec![root.to_path_buf()];
    while let Some(dir) = open.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                open.push(entry.path());
            } else if kind.is_file() {
                let modified = entry.metadata().unwrap().modified().unwrap();
                found.push((entry.path(), std::fs::read(entry.path()).unwrap(), modified));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn only_claude_has_a_recipe_and_the_catalog_says_so() {
    assert!(recipe(AgentId::Claude).is_some());
    assert_eq!(recipe(AgentId::Opencode), None);
    assert_eq!(recipe(AgentId::Codex), None);
    let flags: Vec<_> = crate::catalog::catalog()
        .into_iter()
        .map(|entry| entry.profiles)
        .collect();
    assert_eq!(flags, [true, false, false]);
    let claude = recipe(AgentId::Claude).unwrap();
    assert_eq!(claude.env_var, "CLAUDE_CONFIG_DIR");
    // The sign-in is in no list: no rule can reach it.
    let listed = claude
        .shared_dirs
        .iter()
        .chain(claude.synced_files)
        .chain(claude.history.iter())
        .chain(claude.shared_keys);
    assert!(listed.into_iter().all(|name| *name != claude.credentials));
}

#[test]
fn profile_ids_are_safe_directory_name_parts() {
    for id in ["team", "a", "0", "max-2", "a-", &"a".repeat(24)] {
        assert!(is_valid_id(id), "{id}");
    }
    let too_long = "a".repeat(25);
    for id in [
        "", "default", "Team", "-team", "te am", "a.b", "a/b", "a\\b", "..", "tëam", "a_b",
        &too_long,
    ] {
        assert!(!is_valid_id(id), "{id}");
    }
}

#[test]
fn directories_are_built_from_the_user_profile_and_fixed_names() {
    let homes = Homes::new(r"C:\Users\max\", true).unwrap();
    assert_eq!(
        homes.dir(CLAUDE, "default").unwrap(),
        r"C:\Users\max\.claude"
    );
    assert_eq!(
        homes.state_file(CLAUDE, "default").unwrap(),
        r"C:\Users\max\.claude.json"
    );
    assert_eq!(
        homes.dir(CLAUDE, "team").unwrap(),
        r"C:\Users\max\.claude-team"
    );
    assert_eq!(
        homes.state_file(CLAUDE, "team").unwrap(),
        r"C:\Users\max\.claude-team\.claude.json"
    );
    for id in ["..", "a\\b", "Team", ""] {
        assert_eq!(homes.dir(CLAUDE, id), None, "{id}");
        assert_eq!(homes.state_file(CLAUDE, id), None, "{id}");
    }
    assert_eq!(homes.dir(AgentId::Codex, "team"), None);
    assert_eq!(homes.dir(AgentId::Opencode, "default"), None);
    let unix = Homes::new("/home/max", false).unwrap();
    assert_eq!(unix.dir(CLAUDE, "team").unwrap(), "/home/max/.claude-team");
    // No usable user profile, no directories.
    for home in ["", "max", r"%USERPROFILE%", r"..\max"] {
        assert_eq!(Homes::new(home, true), None, "{home}");
    }
    let mut host = crate::testing::windows_host();
    assert_eq!(
        Homes::of(&host).unwrap().dir(CLAUDE, "team").unwrap(),
        r"C:\Users\max\.claude-team"
    );
    host.user_profile = None;
    assert_eq!(Homes::of(&host), None);
}

#[test]
fn the_junction_list_travels_as_base64_and_the_script_has_no_double_quote() {
    use crate::launch::tests::{decode_base64, decode_command};
    let args = super::links::junction_args();
    assert_eq!(
        args[..3],
        ["-NoProfile", "-NonInteractive", "-EncodedCommand"]
    );
    let script = decode_command(&args[3]);
    assert!(script.contains("New-Item -ItemType Junction"));
    assert!(script.contains("$env:TRAYGENT_LINKS"));
    assert!(!script.contains('"'));
    let pair = |link: &str, target: &str| Link {
        link: link.into(),
        target: target.into(),
    };
    let links = [
        pair(
            r"C:\Users\Max O'Neil\.claude-team\skills",
            r"C:\Users\Max O'Neil\.claude\skills",
        ),
        pair("C:\\bad\nname", r"C:\x"),
        pair(
            r"C:\Users\Max O'Neil\.claude-team\plans",
            r"C:\Users\Max O'Neil\.claude\plans",
        ),
    ];
    let payloads = super::links::junction_payloads(&links);
    assert_eq!(payloads.len(), 1);
    // The paths are nowhere in the command line.
    assert!(!script.contains("Max"));
    assert_eq!(
        decode_base64(&payloads[0]),
        "C:\\Users\\Max O'Neil\\.claude-team\\skills\nC:\\Users\\Max O'Neil\\.claude\\skills\n\
         C:\\Users\\Max O'Neil\\.claude-team\\plans\nC:\\Users\\Max O'Neil\\.claude\\plans\n"
    );
    // A long list is split, so no value outgrows an environment variable.
    let long = "x".repeat(900);
    let many: Vec<_> = (0..40)
        .map(|index| pair(&format!(r"C:\{long}\{index}"), r"C:\target"))
        .collect();
    let payloads = super::links::junction_payloads(&many);
    assert!(payloads.len() > 1);
    assert!(payloads.iter().all(|payload| payload.len() < 32_000));
    let lines: usize = payloads
        .iter()
        .map(|payload| decode_base64(payload).lines().count())
        .sum();
    assert_eq!(lines, 80);
    assert!(super::links::junction_payloads(&[]).is_empty());
}

#[test]
fn an_agent_without_an_extra_profile_is_not_touched() {
    let world = World::new("profiles-none");
    assert!(world.sync(&[], 0).is_empty());
    let plan = SyncPlan::of(Some(&world.homes), &AppSettings::default());
    let outcome = plan.run(linker().as_ref(), at(0));
    assert_eq!(outcome.reports, Some(vec![(CLAUDE, Vec::new())]));
    assert!(std::fs::read_dir(world.home()).unwrap().next().is_none());
    // Without a user profile every profile reports it and nothing is written anywhere.
    let mut settings = AppSettings::default();
    settings.profiles.claude = vec![profile("team")];
    let homeless = SyncPlan::of(None, &settings).run(linker().as_ref(), at(0));
    let reports = &homeless.reports.unwrap()[0].1;
    assert_eq!(reports[0].errors, ["Benutzerordner nicht verfügbar"]);
    assert!(!reports[0].dir_failed);
    assert!(homeless.facts.is_empty());
}

#[test]
fn statuses_show_the_sign_in_without_reading_it() {
    let world = World::new("profiles-status");
    let secret = "sk-synthetic-not-a-real-token";
    write(&world.dir("default").join(".credentials.json"), secret);
    write(
        &world.state("default"),
        r#"{"oauthAccount":{"emailAddress":"max@example.test","organizationName":"Privat"}}"#,
    );
    // An empty credentials file is no sign-in; text from the file is cleaned and capped.
    write(&world.dir("team").join(".credentials.json"), "");
    let long = "o".repeat(300);
    write(
        &world.state("team"),
        &format!(
            r#"{{"oauthAccount":{{"emailAddress":"team\u0007@example.test\n","organizationName":"{long}"}}}}"#
        ),
    );
    write(&world.state("solo"), "{ not json");
    write(
        &world.dir("team").join(MARKER),
        r#"{"version":1,"syncedAt":"2026-11-13T09:00:00Z"}"#,
    );
    write(
        &world.dir("solo").join(MARKER),
        r#"{"version":1,"syncedAt":"gestern"}"#,
    );
    let mut settings = AppSettings::default();
    settings.profiles.claude = vec![profile("team"), profile("solo")];
    let plan = SyncPlan::of(Some(&world.homes), &settings);
    let outcome = plan.read();
    assert_eq!(outcome.reports, None);
    let facts = |id: &str| outcome.facts[&(CLAUDE, id.to_string())].clone();
    assert_eq!(
        facts("default"),
        Facts {
            signed_in: true,
            account: Some("max@example.test".into()),
            organization: Some("Privat".into()),
            synced_at: None,
        }
    );
    let team = facts("team");
    assert!(!team.signed_in);
    assert_eq!(team.account.as_deref(), Some("team@example.test"));
    assert_eq!(team.organization.unwrap().chars().count(), 128);
    assert_eq!(team.synced_at.as_deref(), Some("2026-11-13T09:00:00Z"));
    assert_eq!(facts("solo"), Facts::default());
    let mut errors = std::collections::HashMap::new();
    errors.insert((CLAUDE, "solo".to_string()), "skills: kaputt".to_string());
    let list = status::list(Some(&world.homes), &settings, &outcome.facts, &errors);
    let shown: Vec<_> = list
        .iter()
        .map(|status| (status.id.as_str(), status.label.as_str(), status.signed_in))
        .collect();
    assert_eq!(
        shown,
        [
            ("default", "Standard", true),
            ("team", "TEAM", false),
            ("solo", "SOLO", false)
        ]
    );
    assert_eq!(list[1].dir, world.dir("team").to_str().unwrap());
    assert_eq!(list[2].sync_error.as_deref(), Some("skills: kaputt"));
    let json = serde_json::to_string(&list).unwrap();
    assert!(!json.contains(secret));
    assert!(json.contains(r#""signedIn":true"#) && json.contains(r#""syncedAt":"#));
    // Before anything was read: the profiles of the settings, not signed in, no directory.
    let bare = status::list(None, &settings, &Default::default(), &Default::default());
    assert_eq!(bare.len(), 3);
    assert!(bare
        .iter()
        .all(|status| status.dir.is_empty() && !status.signed_in));
    assert_eq!(read_marker(&world.dir("default")), None);
}
