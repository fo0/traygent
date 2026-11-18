//! Host-backed settings and sign-in for Claude Code and Codex.
use crate::detect::{Fs, Host};
use crate::launch::quote::ps_literal;
use crate::launch::{is_network_path, normalize_folder};
use crate::models::{AgentId, AppSettings, ProfileShare, DEFAULT_PROFILE};
use crate::profiles::share::Sharing;
use crate::profiles::Homes;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentHome {
    pub directory: String,
    /// Default Claude keeps this file beside its config directory. An explicit
    /// CLAUDE_CONFIG_DIR puts it inside the directory instead.
    pub claude_state: Option<String>,
    /// Standard home containing the targets of a named profile's shared junctions.
    pub shared_from: Option<String>,
}

impl AgentHome {
    pub fn resolve(
        agent: AgentId,
        host: &Host,
        settings: &AppSettings,
        profile: Option<&str>,
    ) -> Result<Option<Self>, String> {
        let override_dir = match agent {
            AgentId::Opencode => return Ok(None),
            AgentId::Codex => host.codex_home.as_deref(),
            // Local Standard ignores CLAUDE_CONFIG_DIR when Traygent manages profiles.
            AgentId::Claude if settings.profiles.claude.is_empty() => {
                host.claude_config_dir.as_deref()
            }
            AgentId::Claude => None,
        };
        let mut claude_state = None;
        let mut shared_from = None;
        let path = if let Some(dir) = override_dir {
            dir.to_string()
        } else {
            let user_dir = host
                .user_profile
                .as_deref()
                .ok_or("Benutzerordner für Docker nicht gefunden.")?;
            if agent == AgentId::Claude {
                let homes = Homes::of(host).ok_or("Ungültiger Benutzerordner für Claude Code.")?;
                let id = profile.unwrap_or(DEFAULT_PROFILE);
                if id == DEFAULT_PROFILE {
                    claude_state = homes.state_file(agent, id);
                } else {
                    shared_from = homes.dir(agent, DEFAULT_PROFILE);
                }
                homes.dir(agent, id).ok_or("Ungültiges Claude-Profil.")?
            } else {
                host.join(user_dir, &[".codex"])
            }
        };
        let directory = normalize_folder(&path)
            .map_err(|e| format!("{}-Konfigurationsordner: {e}", agent.label()))?;
        if is_network_path(&directory) {
            return Err(format!(
                "Der {}-Konfigurationsordner muss auf einem lokalen Laufwerk liegen.",
                agent.label()
            ));
        }
        Ok(Some(Self {
            directory,
            claude_state,
            shared_from,
        }))
    }

    /// Docker Desktop exposes NTFS junctions as Linux symlinks pointing outside the bind.
    /// Inspect them in Windows PowerShell after the pre-launch sync, which may have just
    /// created them. Bind only the configured, linked subdirectories, never the Standard home.
    pub(in crate::launch) fn profile_mounts(&self, share: &ProfileShare) -> Option<String> {
        let standard = ps_literal(self.shared_from.as_ref()?);
        let profile = ps_literal(&self.directory);
        let sharing = Sharing::of(crate::profiles::recipe(AgentId::Claude)?, share);
        let names = sharing
            .dirs
            .iter()
            .map(|name| ps_literal(name))
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "$tgProfile=Get-Item -Force -LiteralPath {profile} -ErrorAction Stop; \
             if ($tgProfile.Attributes -band [IO.FileAttributes]::ReparsePoint) {{ throw 'Der Profilordner darf keine Verknüpfung sein.' }}; \
             $tgProfileMounts=@(foreach ($n in @({names})) {{ \
               $src=[IO.Path]::Combine({standard},$n); \
               $link=Get-Item -Force -LiteralPath ([IO.Path]::Combine({profile},$n)) -ErrorAction SilentlyContinue; \
               if ($link.Target -contains $src) {{ \
                 $target=Get-Item -Force -LiteralPath $src -ErrorAction Stop; \
                 if ($target.PSIsContainer -and -not ($target.Attributes -band [IO.FileAttributes]::ReparsePoint)) {{ '-v'; $src+':/home/node/.claude/'+$n+':rw' }} \
               }} \
             }})"
        ))
    }

    pub fn prepare(&self, fs: &dyn Fs, share: &ProfileShare) -> Result<(), String> {
        if let Some(standard) = &self.shared_from {
            if fs.is_link(&self.directory) {
                return Err("Der Profilordner darf keine Verknüpfung sein.".into());
            }
            if fs.refuses_sign_in(AgentId::Claude, &self.directory, standard, share) {
                return Err(format!(
                    "Der {}.",
                    crate::profiles::sync::foreign_dir(AgentId::Claude)
                ));
            }
        }
        fs.create_dir_all(&self.directory).map_err(|_| {
            "Der Konfigurationsordner für Docker konnte nicht geöffnet oder angelegt werden."
        })?;
        if let Some(path) = &self.claude_state {
            fs.ensure_json_file(path).map_err(|_| {
                "Die Datei .claude.json für Docker konnte nicht geöffnet oder angelegt werden."
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::RealFs;
    use crate::models::AgentProfile;
    use crate::testing::{windows_host, TempDir};

    #[test]
    fn docker_uses_each_local_home_and_preserves_claudes_standard_profile_rule() {
        let mut host = windows_host();
        let mut settings = AppSettings::default();
        let resolve = |agent, host: &Host, settings: &AppSettings| {
            AgentHome::resolve(agent, host, settings, None)
                .unwrap()
                .unwrap()
        };
        let standard = resolve(AgentId::Claude, &host, &settings);
        assert_eq!(standard.directory, r"C:\Users\max\.claude");
        assert_eq!(
            standard.claude_state.as_deref(),
            Some(r"C:\Users\max\.claude.json")
        );
        assert_eq!(
            resolve(AgentId::Codex, &host, &settings).directory,
            r"C:\Users\max\.codex"
        );
        host.claude_config_dir = Some(r"D:\Claude's Home".into());
        host.codex_home = Some(r"D:\Codex Home".into());
        assert_eq!(
            resolve(AgentId::Claude, &host, &settings),
            AgentHome {
                directory: r"D:\Claude's Home".into(),
                claude_state: None,
                shared_from: None,
            }
        );
        assert_eq!(
            resolve(AgentId::Codex, &host, &settings).directory,
            r"D:\Codex Home"
        );
        settings.profiles.claude.push(AgentProfile {
            id: "team".into(),
            label: "Team".into(),
        });
        assert_eq!(resolve(AgentId::Claude, &host, &settings), standard);
        for dir in ["relative", r"\\server\share", r"C:\x:ro"] {
            host.codex_home = Some(dir.into());
            assert!(AgentHome::resolve(AgentId::Codex, &host, &settings, None).is_err());
        }
    }

    #[test]
    fn docker_prepares_real_mount_sources_without_overwriting_state() {
        let temp = TempDir::new("docker-home");
        let directory = temp.0.join(".claude");
        let state = temp.0.join(".claude.json");
        let home = AgentHome {
            directory: directory.to_str().unwrap().into(),
            claude_state: Some(state.to_str().unwrap().into()),
            shared_from: None,
        };
        home.prepare(&RealFs, &ProfileShare::default()).unwrap();
        assert!(directory.is_dir());
        assert_eq!(std::fs::read(&state).unwrap(), b"{}");
        std::fs::write(&state, b"existing state").unwrap();
        home.prepare(&RealFs, &ProfileShare::default()).unwrap();
        assert_eq!(std::fs::read(&state).unwrap(), b"existing state");
        std::fs::remove_file(&state).unwrap();
        std::fs::create_dir(&state).unwrap();
        assert!(home.prepare(&RealFs, &ProfileShare::default()).is_err());
    }
}
