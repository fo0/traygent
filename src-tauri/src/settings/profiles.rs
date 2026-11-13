//! The profile settings (#33): extra profiles per agent and the one a new session starts
//! with. German errors reach the panel.
use crate::models::{AgentId, AgentProfile, AppSettings, DEFAULT_PROFILE, DEFAULT_PROFILE_LABEL};
use crate::profiles::{self, MAX_PROFILES};

const MAX_LABEL_CHARS: usize = 24;

/// Trims ids and labels; a default profile that is not (or no longer) listed becomes
/// `default`, so removing a profile never blocks the save.
pub fn normalize(settings: &mut AppSettings) {
    for agent in AgentId::ALL {
        for profile in settings.profiles.get_mut(agent).iter_mut() {
            profile.id = profile.id.trim().to_string();
            profile.label = profile.label.trim().to_string();
        }
        let chosen = settings.agent_profile.get(agent).trim().to_string();
        let listed = settings
            .profiles
            .get(agent)
            .iter()
            .any(|profile| profile.id == chosen);
        *settings.agent_profile.get_mut(agent) = if listed {
            chosen
        } else {
            DEFAULT_PROFILE.into()
        };
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    for agent in AgentId::ALL {
        validate_list(agent, settings.profiles.get(agent))?;
    }
    Ok(())
}

fn validate_list(agent: AgentId, list: &[AgentProfile]) -> Result<(), String> {
    let name = agent.label();
    if list.is_empty() {
        return Ok(());
    }
    if profiles::recipe(agent).is_none() {
        return Err(format!("{name} unterstützt keine Profile."));
    }
    if list.len() > MAX_PROFILES {
        return Err(format!(
            "Höchstens {MAX_PROFILES} Profile für {name} möglich."
        ));
    }
    for (index, profile) in list.iter().enumerate() {
        validate_profile(agent, profile)?;
        let earlier = &list[..index];
        if earlier.iter().any(|other| other.id == profile.id) {
            return Err(format!(
                "Profil-ID „{}“ für {name} ist doppelt vergeben.",
                profile.id
            ));
        }
        let label = profile.label.to_lowercase();
        if earlier
            .iter()
            .any(|other| other.label.to_lowercase() == label)
        {
            return Err(format!(
                "Profilname „{}“ für {name} ist doppelt vergeben.",
                profile.label
            ));
        }
    }
    Ok(())
}

fn validate_profile(agent: AgentId, profile: &AgentProfile) -> Result<(), String> {
    let name = agent.label();
    if !profiles::is_valid_id(&profile.id) {
        return Err(format!(
            "Profil-ID für {name} ist ungültig: nur a–z, 0–9 und „-“, höchstens 24 Zeichen, nicht „default“."
        ));
    }
    let label = &profile.label;
    if label.is_empty() {
        return Err(format!("Profilname für {name} fehlt."));
    }
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(format!(
            "Profilname für {name} ist zu lang (höchstens {MAX_LABEL_CHARS} Zeichen)."
        ));
    }
    if label.chars().any(char::is_control) {
        return Err(format!("Profilname für {name} enthält Steuerzeichen."));
    }
    if label.to_lowercase() == DEFAULT_PROFILE_LABEL.to_lowercase() {
        return Err(format!(
            "Profilname „{DEFAULT_PROFILE_LABEL}“ ist für das Standard-Profil reserviert."
        ));
    }
    Ok(())
}

/// Loaded settings (already normalized): every profile that would not validate is dropped,
/// the rest kept in order; the default profile follows.
pub fn sanitize(settings: &mut AppSettings) {
    for agent in AgentId::ALL {
        let mut kept: Vec<AgentProfile> = Vec::new();
        for profile in std::mem::take(settings.profiles.get_mut(agent)) {
            kept.push(profile);
            if validate_list(agent, &kept).is_err() {
                kept.pop();
            }
        }
        *settings.profiles.get_mut(agent) = kept;
    }
    normalize(settings);
}

#[cfg(test)]
mod tests;
