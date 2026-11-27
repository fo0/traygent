use super::*;
use crate::models::AgentProfile;
use crate::settings;
use crate::store;
use crate::testing::TempDir;
use serde_json::json;

fn with_profiles(ids: &[&str]) -> AppSettings {
    let mut settings = AppSettings::default();
    settings.profiles.claude = ids
        .iter()
        .map(|id| AgentProfile {
            id: (*id).into(),
            label: id.to_uppercase(),
        })
        .collect();
    settings
}

fn shown(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_string()).collect()
}

#[test]
fn defaults_show_no_extra_profile() {
    let settings = AppSettings::default();
    for agent in AgentId::ALL {
        assert!(settings.quota_profiles.get(agent).is_empty());
    }
    let value = serde_json::to_value(&settings).unwrap();
    assert_eq!(
        value["quotaProfiles"],
        json!({"claude": [], "opencode": [], "codex": []})
    );
}

#[test]
fn normalize_keeps_listed_ids_once_and_drops_the_rest() {
    let mut settings = with_profiles(&["team", "work"]);
    settings.quota_profiles.claude = shown(&[" team ", "gone", "team", "default", "Bad Id"]);
    settings.quota_profiles.codex = shown(&["default", "team"]);
    let settings = settings::normalize(settings);
    assert_eq!(settings.quota_profiles.claude, shown(&["team", "default"]));
    // Codex has no profiles: only its own home can be listed.
    assert_eq!(settings.quota_profiles.codex, shown(&["default"]));
    assert_eq!(settings::validate(&settings), Ok(()));
}

#[test]
fn removing_a_profile_drops_it_from_the_tile() {
    let mut settings = with_profiles(&["team", "work"]);
    settings.quota_profiles.claude = shown(&["default", "team", "work"]);
    settings
        .profiles
        .claude
        .retain(|profile| profile.id != "team");
    let settings = settings::normalize(settings);
    assert_eq!(settings.quota_profiles.claude, shown(&["default", "work"]));
}

#[test]
fn a_save_past_the_cap_is_refused() {
    let mut settings = with_profiles(&["team", "work", "side"]);
    settings.quota_profiles.claude = shown(&["default", "team", "work", "side"]);
    let error = settings::validate(&settings::normalize(settings)).unwrap_err();
    assert_eq!(error, "Höchstens 3 Profile für Claude Code im Kontingent.");
}

#[test]
fn a_loaded_list_is_sanitized_and_cut_to_the_cap() {
    let dir = TempDir::new("quota-profiles");
    let profiles = json!([
        {"id": "team", "label": "Team"},
        {"id": "work", "label": "Work"},
        {"id": "side", "label": "Side"},
        {"id": "../x", "label": "Bad"}
    ]);
    std::fs::write(
        store::path(&dir.0),
        json!({"version": 1, "settings": {
            "logLevel": "debug",
            "profiles": {"claude": profiles},
            "quotaProfiles": {
                "claude": ["side", 7, "../x", "side", "team", "default", "work"],
                "codex": "default",
                "nobody": ["default"]
            }
        }})
        .to_string(),
    )
    .unwrap();
    let loaded = store::load(&dir.0).state.settings;
    assert_eq!(loaded.log_level, crate::app_log::Level::Debug);
    assert_eq!(
        loaded.quota_profiles.claude,
        shown(&["side", "team", "default"])
    );
    assert!(loaded.quota_profiles.codex.is_empty());
    assert!(loaded.quota_profiles.opencode.is_empty());
}

#[test]
fn a_file_from_before_the_setting_shows_the_start_profile_only() {
    let dir = TempDir::new("quota-profiles-absent");
    std::fs::write(
        store::path(&dir.0),
        json!({"version": 1, "settings": {"debug": true}}).to_string(),
    )
    .unwrap();
    let loaded = store::load(&dir.0).state.settings;
    for agent in AgentId::ALL {
        assert!(loaded.quota_profiles.get(agent).is_empty());
    }
}
