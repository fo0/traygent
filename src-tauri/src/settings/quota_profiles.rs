//! The profiles the dashboard's quota tile shows per agent (#94). Each one is one more quota
//! read, so the list is short; the start profile is always shown (`quotaTypes.ts →
//! shownQuotaProfiles`). German errors reach the panel.
use crate::models::{AgentId, AppSettings, DEFAULT_PROFILE};
use crate::profiles;

/// At most this many profiles per agent; mirrors `MAX_QUOTA_PROFILES` in the panel.
pub const MAX: usize = 3;

/// Trims the ids and keeps each valid one once that is `default` or a listed profile, so
/// removing a profile drops it here too and never blocks the save.
pub fn normalize(settings: &mut AppSettings) {
    for agent in AgentId::ALL {
        let listed = settings.profiles.get(agent);
        let mut kept: Vec<String> = Vec::new();
        for id in settings.quota_profiles.get(agent) {
            let id = id.trim();
            let known = id == DEFAULT_PROFILE
                || (profiles::is_valid_id(id) && listed.iter().any(|item| item.id == id));
            if known && !kept.iter().any(|other| other == id) {
                kept.push(id.to_string());
            }
        }
        *settings.quota_profiles.get_mut(agent) = kept;
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    for agent in AgentId::ALL {
        if settings.quota_profiles.get(agent).len() > MAX {
            return Err(format!(
                "Höchstens {MAX} Profile für {} im Kontingent.",
                agent.label()
            ));
        }
    }
    Ok(())
}

/// Loaded settings (profiles already sanitized): normalized again against the kept
/// profiles, then cut to the first [`MAX`].
pub fn sanitize(settings: &mut AppSettings) {
    normalize(settings);
    for agent in AgentId::ALL {
        settings.quota_profiles.get_mut(agent).truncate(MAX);
    }
}

#[cfg(test)]
mod tests;
