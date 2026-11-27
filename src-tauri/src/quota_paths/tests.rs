use super::*;
use crate::models::AgentProfile;
use crate::testing::{windows_host, TempDir};

// These fixtures do not inherit a developer's own CLI credentials or provider flags.
fn quota_source(
    agent: AgentId,
    profile: Option<&str>,
    env: EnvId,
    settings: &AppSettings,
    host: &Host,
) -> Result<QuotaSource, &'static str> {
    choose_source(agent, profile, env, settings, host, |_| false)
}

/// The file a read uses; panics for a CLI read.
fn credentials_path(
    agent: AgentId,
    profile: Option<&str>,
    env: EnvId,
    settings: &AppSettings,
    host: &Host,
) -> Result<PathBuf, &'static str> {
    quota_source(agent, profile, env, settings, host).map(|source| match source {
        QuotaSource::File(path) => path,
        QuotaSource::ClaudeCli { .. } => panic!("{agent:?} reads through its CLI"),
    })
}

/// The profile directory a Claude Code CLI read sets; panics for a file read.
fn claude_dir(
    profile: Option<&str>,
    env: EnvId,
    settings: &AppSettings,
    host: &Host,
) -> Result<Option<String>, &'static str> {
    quota_source(AgentId::Claude, profile, env, settings, host).map(|source| match source {
        QuotaSource::ClaudeCli { profile_dir } => profile_dir,
        QuotaSource::File(path) => panic!("a file read of {path:?}"),
    })
}

fn profile(id: &str) -> AgentProfile {
    AgentProfile {
        id: id.into(),
        label: id.into(),
    }
}

#[test]
fn standard_windows_accounts_use_the_same_sources_as_launches() {
    let mut host = windows_host();
    let mut settings = AppSettings::default();
    for env in [EnvId::Local, EnvId::Docker] {
        let path = credentials_path(AgentId::Codex, None, env, &settings, &host).unwrap();
        assert_eq!(path, PathBuf::from(r"C:\Users\max\.codex\auth.json"));
        // Claude Code's CLI finds its own default home: no directory is set.
        assert_eq!(claude_dir(None, env, &settings, &host), Ok(None));
    }
    host.codex_home = Some(r"D:\My Codex".into());
    host.claude_config_dir = Some(r"D:\My Claude".into());
    for env in [EnvId::Local, EnvId::Docker] {
        let path = credentials_path(AgentId::Codex, None, env, &settings, &host).unwrap();
        assert_eq!(path, PathBuf::from(r"D:\My Codex\auth.json"));
        assert_eq!(claude_dir(None, env, &settings, &host), Ok(None));
    }
    settings.profiles.claude.push(profile("team"));
    for env in [EnvId::Local, EnvId::Docker] {
        for (id, expected) in [
            ("default", None),
            ("team", Some(r"C:\Users\max\.claude-team")),
        ] {
            let dir = claude_dir(Some(id), env, &settings, &host).unwrap();
            assert_eq!(dir.as_deref(), expected);
        }
    }
}

#[test]
fn opencode_uses_its_local_data_directory_and_has_its_own_docker_volume() {
    let mut host = windows_host();
    let settings = AppSettings::default();
    let path = |host: &Host| {
        credentials_path(AgentId::Opencode, None, EnvId::Local, &settings, host).unwrap()
    };
    assert_eq!(
        path(&host),
        PathBuf::from(r"C:\Users\max\.local\share\opencode\opencode.db")
    );
    host.xdg_data_home = Some(r"D:\XDG".into());
    assert_eq!(path(&host), PathBuf::from(r"D:\XDG\opencode\opencode.db"));
    host.opencode_db = Some("work/db.sqlite".into());
    assert_eq!(
        path(&host),
        PathBuf::from(r"D:\XDG\opencode\work\db.sqlite")
    );
    host.opencode_db = Some(r"E:\Open Code\alternate.sqlite".into());
    assert_eq!(path(&host), PathBuf::from(r"E:\Open Code\alternate.sqlite"));
    assert_eq!(
        credentials_path(AgentId::Opencode, None, EnvId::Docker, &settings, &host),
        Err(DOCKER_OPENCODE)
    );
    for db in [
        ":memory:",
        "",
        r"\unknown-root.sqlite",
        r"\\server\share\remote.sqlite",
        "E:drive-relative",
    ] {
        host.opencode_db = Some(db.into());
        assert_eq!(
            credentials_path(AgentId::Opencode, None, EnvId::Local, &settings, &host),
            Err(NO_HOME)
        );
    }
}

#[test]
fn unknown_profiles_paths_and_unsupported_environments_never_choose_a_host_account() {
    let mut host = windows_host();
    let mut settings = AppSettings::default();
    settings.profiles.claude.push(profile("team"));
    for env in [EnvId::Local, EnvId::Docker] {
        for id in ["missing", "../other", r"C:\secrets", ""] {
            assert_eq!(
                quota_source(AgentId::Claude, Some(id), env, &settings, &host),
                Err(PROFILE_MISSING)
            );
        }
        assert_eq!(
            quota_source(AgentId::Codex, Some("team"), env, &settings, &host),
            Err(PROFILE_MISSING)
        );
    }
    for agent in AgentId::ALL {
        assert_eq!(
            quota_source(agent, None, EnvId::Wsl, &settings, &host),
            Err(WSL)
        );
    }
    for invalid in [
        "relative",
        r"\\server\share",
        r"\\?\C:\temp",
        r"C:\temp:stream",
    ] {
        host.codex_home = Some(invalid.into());
        assert_eq!(
            credentials_path(AgentId::Codex, None, EnvId::Local, &settings, &host),
            Err(NO_HOME)
        );
    }
}

#[test]
fn file_quota_follows_the_credentials_inherited_by_each_launch() {
    let host = windows_host();
    let unmanaged = AppSettings::default();
    let mut managed = unmanaged.clone();
    managed.profiles.claude.push(profile("team"));
    for (settings, id) in [
        (&unmanaged, "default"),
        (&managed, "default"),
        (&managed, "team"),
    ] {
        for var in [
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_AUTH_TOKEN",
            "CLAUDE_CODE_CUSTOM_OAUTH_URL",
            "CLAUDE_CODE_USE_VERTEX",
        ] {
            let read = |env| {
                choose_source(AgentId::Claude, Some(id), env, settings, &host, |name| {
                    name == var
                })
            };
            assert_eq!(read(EnvId::Local), Err(ENV_LOGIN), "{id} {var}");
            let forwarded = matches!(var, "ANTHROPIC_API_KEY" | "CLAUDE_CODE_OAUTH_TOKEN");
            assert_eq!(
                read(EnvId::Docker).is_err(),
                settings.profiles.claude.is_empty() && forwarded,
                "{id} {var}"
            );
        }
    }
    let codex = |env, var| {
        choose_source(AgentId::Codex, None, env, &unmanaged, &host, |name| {
            name == var
        })
    };
    assert_eq!(codex(EnvId::Local, "CODEX_ACCESS_TOKEN"), Err(ENV_LOGIN));
    assert!(codex(EnvId::Docker, "CODEX_ACCESS_TOKEN").is_ok());
    for var in ["OPENAI_API_KEY", "CODEX_API_KEY"] {
        assert!(codex(EnvId::Local, var).is_ok());
        assert!(codex(EnvId::Docker, var).is_ok());
    }
    // OpenCode v2's active stored credential wins over an available environment key.
    let go = choose_source(
        AgentId::Opencode,
        None,
        EnvId::Local,
        &unmanaged,
        &host,
        |name| name == "OPENCODE_API_KEY",
    )
    .unwrap();
    assert_eq!(
        go,
        QuotaSource::File(PathBuf::from(
            r"C:\Users\max\.local\share\opencode\opencode.db"
        ))
    );
}

#[test]
fn switching_profile_uses_only_the_selected_accounts_directory() {
    let temp = TempDir::new("quota-profiles");
    let host = Host {
        user_profile: Some(temp.0.to_str().unwrap().into()),
        windows: cfg!(windows),
        ..Host::default()
    };
    let mut settings = AppSettings::default();
    for id in ["daily", "team"] {
        settings.profiles.claude.push(profile(id));
        std::fs::create_dir(temp.0.join(format!(".claude-{id}"))).unwrap();
    }
    for (id, env) in [
        ("team", EnvId::Local),
        ("daily", EnvId::Docker),
        ("team", EnvId::Docker),
    ] {
        let dir = claude_dir(Some(id), env, &settings, &host)
            .unwrap()
            .unwrap();
        assert_eq!(PathBuf::from(dir), temp.0.join(format!(".claude-{id}")));
    }
    #[cfg(unix)]
    {
        settings.profiles.claude.push(profile("linked"));
        std::os::unix::fs::symlink(temp.0.join(".claude-team"), temp.0.join(".claude-linked"))
            .unwrap();
        assert!(quota_source(
            AgentId::Claude,
            Some("linked"),
            EnvId::Local,
            &settings,
            &host
        )
        .is_err());
    }
}
