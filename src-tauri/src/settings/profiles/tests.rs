use super::*;
use crate::settings;

fn profile(id: &str, label: &str) -> AgentProfile {
    AgentProfile {
        id: id.into(),
        label: label.into(),
    }
}

fn with(list: &[(&str, &str)]) -> AppSettings {
    let mut settings = AppSettings::default();
    settings.profiles.claude = list.iter().map(|(id, label)| profile(id, label)).collect();
    settings
}

fn error(list: &[(&str, &str)]) -> String {
    settings::validate(&settings::normalize(with(list))).unwrap_err()
}

#[test]
fn defaults_have_no_extra_profile_and_start_with_the_default_one() {
    let settings = AppSettings::default();
    for agent in AgentId::ALL {
        assert!(settings.profiles.get(agent).is_empty());
        assert_eq!(settings.agent_profile.get(agent), "default");
    }
    assert_eq!(settings::validate(&settings), Ok(()));
}

#[test]
fn valid_profiles_are_trimmed_and_kept_in_order() {
    let mut raw = with(&[(" team ", "  Team  "), ("max-2", "Max 2"), ("0", "Ö")]);
    raw.agent_profile.claude = " max-2 ".into();
    let settings = settings::normalize(raw);
    assert_eq!(settings::validate(&settings), Ok(()));
    assert_eq!(
        settings.profiles.claude,
        [
            profile("team", "Team"),
            profile("max-2", "Max 2"),
            profile("0", "Ö")
        ]
    );
    assert_eq!(settings.agent_profile.claude, "max-2");
    let full: Vec<_> = (0..8).map(|n| (format!("p{n}"), format!("P{n}"))).collect();
    let full: Vec<_> = full
        .iter()
        .map(|(id, label)| (id.as_str(), label.as_str()))
        .collect();
    assert_eq!(settings::validate(&with(&full)), Ok(()));
}

#[test]
fn invalid_profiles_are_refused_in_german() {
    for id in [
        "",
        "default",
        "Team",
        "-a",
        "a b",
        "a.b",
        "..",
        "a/b",
        &"a".repeat(25),
    ] {
        let text = error(&[(id, "Team")]);
        assert!(
            text.starts_with("Profil-ID für Claude Code ist ungültig"),
            "{id}: {text}"
        );
    }
    assert_eq!(
        error(&[("team", "   ")]),
        "Profilname für Claude Code fehlt."
    );
    assert_eq!(
        error(&[("team", &"x".repeat(25))]),
        "Profilname für Claude Code ist zu lang (höchstens 24 Zeichen)."
    );
    assert_eq!(
        settings::validate(&with(&[("team", &"ä".repeat(24))])),
        Ok(())
    );
    assert_eq!(
        error(&[("team", "Te\tam")]),
        "Profilname für Claude Code enthält Steuerzeichen."
    );
    for reserved in ["Standard", "standard", " STANDARD "] {
        assert_eq!(
            error(&[("team", reserved)]),
            "Profilname „Standard“ ist für das Standard-Profil reserviert."
        );
    }
    assert_eq!(
        error(&[("team", "Team"), ("team", "Zwei")]),
        "Profil-ID „team“ für Claude Code ist doppelt vergeben."
    );
    assert_eq!(
        error(&[("team", "Team"), ("zwei", "TEAM")]),
        "Profilname „TEAM“ für Claude Code ist doppelt vergeben."
    );
    let nine: Vec<_> = (0..9).map(|n| (format!("p{n}"), format!("P{n}"))).collect();
    let nine: Vec<_> = nine
        .iter()
        .map(|(id, label)| (id.as_str(), label.as_str()))
        .collect();
    assert_eq!(error(&nine), "Höchstens 8 Profile für Claude Code möglich.");
    // An agent whose catalog entry has no profiles keeps an empty list.
    for (agent, name) in [(AgentId::Opencode, "OpenCode"), (AgentId::Codex, "Codex")] {
        let mut settings = AppSettings::default();
        *settings.profiles.get_mut(agent) = vec![profile("team", "Team")];
        assert_eq!(
            settings::validate(&settings),
            Err(format!("{name} unterstützt keine Profile."))
        );
    }
}

#[test]
fn a_default_profile_that_is_not_listed_becomes_the_default_one() {
    for chosen in ["gone", "", "Team", "default"] {
        let mut raw = with(&[("team", "Team")]);
        raw.agent_profile.claude = chosen.into();
        raw.agent_profile.codex = "team".into();
        let settings = settings::normalize(raw);
        assert_eq!(settings.agent_profile.claude, "default", "{chosen}");
        assert_eq!(settings.agent_profile.codex, "default");
        assert_eq!(settings::validate(&settings), Ok(()));
    }
}

#[test]
fn sanitize_drops_only_the_profiles_that_would_not_validate() {
    let mut raw = with(&[
        ("team", "Team"),
        ("../x", "Böse"),
        ("team", "Doppelt"),
        ("solo", "team"),
        ("ok", "Standard"),
        ("max", "Max"),
    ]);
    raw.agent_profile.claude = "max".into();
    raw.profiles.codex = vec![profile("team", "Team")];
    raw.agent_profile.opencode = "team".into();
    let clean = settings::sanitize(raw);
    assert_eq!(
        clean.profiles.claude,
        [profile("team", "Team"), profile("max", "Max")]
    );
    assert_eq!(clean.agent_profile.claude, "max");
    assert!(clean.profiles.codex.is_empty());
    assert_eq!(clean.agent_profile.opencode, "default");
    assert_eq!(settings::validate(&clean), Ok(()));
    // More than the cap: the first eight stay.
    let many: Vec<_> = (0..12)
        .map(|n| (format!("p{n}"), format!("P{n}")))
        .collect();
    let many: Vec<_> = many
        .iter()
        .map(|(id, label)| (id.as_str(), label.as_str()))
        .collect();
    assert_eq!(settings::sanitize(with(&many)).profiles.claude.len(), 8);
}
