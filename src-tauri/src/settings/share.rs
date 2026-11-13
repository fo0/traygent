//! The extra shared directories and files per agent (#35). German errors reach the panel.
use crate::models::{AgentId, AppSettings, ProfileShare};
use crate::profiles::share::{check_name, NameError, MAX_SHARED};
use crate::profiles::{self, Recipe};

/// Trims every name, drops empty ones and exact repeats (within a list).
pub fn normalize(settings: &mut AppSettings) {
    for agent in AgentId::ALL {
        let share = settings.profile_share.get_mut(agent);
        for list in [&mut share.dirs, &mut share.files] {
            let mut kept: Vec<String> = Vec::new();
            for name in list.iter().map(|name| name.trim()) {
                if !name.is_empty() && !kept.iter().any(|known| known == name) {
                    kept.push(name.to_string());
                }
            }
            *list = kept;
        }
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    for agent in AgentId::ALL {
        validate_share(agent, settings.profile_share.get(agent))?;
    }
    Ok(())
}

fn validate_share(agent: AgentId, share: &ProfileShare) -> Result<(), String> {
    let label = agent.label();
    if share.dirs.is_empty() && share.files.is_empty() {
        return Ok(());
    }
    let Some(recipe) = profiles::recipe(agent) else {
        return Err(format!("{label} unterstützt keine Profile."));
    };
    if share.dirs.len() > MAX_SHARED {
        return Err(format!(
            "Höchstens {MAX_SHARED} weitere Ordner für {label} möglich."
        ));
    }
    if share.files.len() > MAX_SHARED {
        return Err(format!(
            "Höchstens {MAX_SHARED} weitere Dateien für {label} möglich."
        ));
    }
    let mut seen: Vec<String> = Vec::new();
    for name in share.dirs.iter().chain(&share.files) {
        validate_name(agent, recipe, name)?;
        // One name, one thing: a folder and a file cannot share it, whatever the case.
        let lower = name.to_ascii_lowercase();
        if seen.contains(&lower) {
            return Err(format!("Name „{name}“ für {label} ist doppelt vergeben."));
        }
        seen.push(lower);
    }
    Ok(())
}

fn validate_name(agent: AgentId, recipe: &Recipe, name: &str) -> Result<(), String> {
    let label = agent.label();
    match check_name(recipe, name) {
        Ok(()) => Ok(()),
        Err(NameError::Invalid) => Err(format!("Name „{name}“ für {label} ist ungültig.")),
        Err(NameError::Handled) => Err(format!(
            "Name „{name}“ für {label} wird bereits abgeglichen."
        )),
    }
}

/// Loaded settings: every name that would not validate is dropped, the rest kept in order,
/// so nothing unchecked from a hand-edited `state.json` ever reaches a path.
pub fn sanitize(settings: &mut AppSettings) {
    normalize(settings);
    for agent in AgentId::ALL {
        let loaded = std::mem::take(settings.profile_share.get_mut(agent));
        let mut kept = ProfileShare::default();
        for (name, is_dir) in dirs_then_files(&loaded) {
            let list = if is_dir {
                &mut kept.dirs
            } else {
                &mut kept.files
            };
            list.push(name.clone());
            if validate_share(agent, &kept).is_err() {
                let list = if is_dir {
                    &mut kept.dirs
                } else {
                    &mut kept.files
                };
                list.pop();
            }
        }
        *settings.profile_share.get_mut(agent) = kept;
    }
}

fn dirs_then_files(share: &ProfileShare) -> impl Iterator<Item = (&String, bool)> {
    let dirs = share.dirs.iter().map(|name| (name, true));
    dirs.chain(share.files.iter().map(|name| (name, false)))
}

#[cfg(test)]
mod tests;
