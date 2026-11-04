use super::*;
use crate::models::{PermissionId, TerminalId};

fn options(model: &str, effort: &str, extra: &str) -> LaunchOptions {
    LaunchOptions {
        model: model.into(),
        effort: effort.into(),
        permission: PermissionId::Auto,
        extra_args: extra.into(),
    }
}

#[test]
fn defaults_match_the_spec() {
    let settings = AppSettings::default();
    assert_eq!(settings.default_agent, AgentId::Claude);
    assert_eq!(settings.default_env, EnvId::Local);
    assert_eq!(settings.terminal, TerminalId::WindowsTerminal);
    for agent in AgentId::ALL {
        assert_eq!(settings.agents.get(agent), &LaunchOptions::default());
        assert_eq!(settings.docker.images.get(agent), "node:22-trixie");
    }
    assert!(!settings.docker.gh_token);
    assert_eq!(validate(&settings), Ok(()));
}

#[test]
fn wsl_cannot_be_the_default_environment() {
    let settings = AppSettings {
        default_env: EnvId::Wsl,
        ..AppSettings::default()
    };
    assert_eq!(
        validate(&settings),
        Err("WSL ist noch nicht verfügbar.".into())
    );
}

#[test]
fn every_environment_is_offered_by_default() {
    assert_eq!(AppSettings::default().environments, EnvId::ALL.to_vec());
}

#[test]
fn environments_normalize_to_each_once_in_canonical_order() {
    let settings = AppSettings {
        environments: vec![EnvId::Wsl, EnvId::Docker, EnvId::Wsl, EnvId::Local],
        ..AppSettings::default()
    };
    assert_eq!(normalize(settings).environments, EnvId::ALL.to_vec());
    assert_eq!(
        normalize_environments(&[EnvId::Docker, EnvId::Docker]),
        [EnvId::Docker]
    );
}

#[test]
fn environments_keep_one_that_starts_and_the_default() {
    let with = |environments: Vec<EnvId>, default_env| AppSettings {
        environments,
        default_env,
        ..AppSettings::default()
    };
    assert_eq!(
        validate(&with(Vec::new(), EnvId::Local)),
        Err("Mindestens eine Umgebung muss aktiv sein.".into())
    );
    assert_eq!(
        validate(&with(vec![EnvId::Wsl], EnvId::Local)),
        Err("WSL ist noch nicht verfügbar.".into())
    );
    assert_eq!(
        validate(&with(vec![EnvId::Local, EnvId::Wsl], EnvId::Docker)),
        Err("Die Standard-Umgebung ist deaktiviert.".into())
    );
    assert_eq!(validate(&with(vec![EnvId::Docker], EnvId::Docker)), Ok(()));
}

/// A hand-edited list: no usable environment → all of them; a hidden default → the first shown.
#[test]
fn sanitize_repairs_the_environments_and_their_default() {
    for broken in [Vec::new(), vec![EnvId::Wsl]] {
        let clean = sanitize(AppSettings {
            environments: broken,
            ..AppSettings::default()
        });
        assert_eq!(clean.environments, EnvId::ALL.to_vec());
        assert_eq!(clean.default_env, EnvId::Local);
    }
    let clean = sanitize(AppSettings {
        environments: vec![EnvId::Wsl, EnvId::Docker],
        default_env: EnvId::Local,
        ..AppSettings::default()
    });
    assert_eq!(clean.environments, [EnvId::Docker, EnvId::Wsl]);
    assert_eq!(clean.default_env, EnvId::Docker);
    assert_eq!(validate(&clean), Ok(()));
}

#[test]
fn models_accept_aliases_ids_and_provider_paths() {
    for model in [
        "default",
        "opus",
        "claude-opus-5-5",
        "opus[1m]",
        "us.anthropic.x:0",
    ] {
        assert_eq!(
            validate_options(AgentId::Claude, &options(model, "default", "")),
            Ok(())
        );
    }
    for model in ["gpt-6.1-sol", "my-custom@2"] {
        assert_eq!(
            validate_options(AgentId::Codex, &options(model, "high", "")),
            Ok(())
        );
    }
    for model in [
        "opencode/big-pickle",
        "github-copilot/claude-sonnet-5.5",
        "a.b/c:d@e/f",
    ] {
        assert_eq!(
            validate_options(AgentId::Opencode, &options(model, "high", "")),
            Ok(())
        );
    }
}

#[test]
fn models_reject_unsafe_text() {
    let unsafe_models = [
        "opus sonnet",
        "--dangerously-skip-permissions",
        "a;b",
        "a'b",
        "a\"b",
        "a&b",
        "a#b",
        "ü",
    ];
    for model in unsafe_models {
        let error = validate_options(AgentId::Claude, &options(model, "default", "")).unwrap_err();
        assert_eq!(
            error, "Modell für Claude Code enthält unzulässige Zeichen.",
            "{model}"
        );
    }
    let long = "a".repeat(129);
    assert_eq!(
        validate_options(AgentId::Codex, &options(&long, "default", "")),
        Err("Modell für Codex ist zu lang.".into())
    );
    for model in ["big-pickle", "/model", "provider/", "pro vider/x"] {
        assert!(
            validate_options(AgentId::Opencode, &options(model, "default", "")).is_err(),
            "{model}"
        );
    }
    assert_eq!(
        validate_options(AgentId::Opencode, &options("big-pickle", "default", "")),
        Err("Modell für OpenCode als provider/model angeben.".into())
    );
}

#[test]
fn efforts_must_come_from_the_catalog() {
    assert_eq!(
        validate_options(AgentId::Codex, &options("default", "xhigh", "")),
        Ok(())
    );
    assert_eq!(
        validate_options(AgentId::Codex, &options("default", "ultra", "")),
        Err("Unbekannte Denkstufe für Codex.".into())
    );
}

#[test]
fn extra_args_split_on_whitespace() {
    assert_eq!(extra_args(AgentId::Claude, ""), Ok(vec![]));
    assert_eq!(
        extra_args(AgentId::Claude, "  --verbose   --add-dir  C:\\x  "),
        Ok(vec!["--verbose".into(), "--add-dir".into(), "C:\\x".into()])
    );
    assert_eq!(
        extra_args(AgentId::Codex, "a;b &c"),
        Ok(vec!["a;b".into(), "&c".into()])
    );
}

#[test]
fn extra_args_reject_quotes_control_chars_and_length() {
    for raw in ["--x \"y\"", "it's", "`x", "\u{2019}x", "\u{201E}x"] {
        assert_eq!(
            extra_args(AgentId::Codex, raw),
            Err("Zusatzargumente für Codex dürfen keine Anführungszeichen enthalten.".into()),
            "{raw}"
        );
    }
    for raw in ["a\tb", "a\nb", "a\u{7}"] {
        assert_eq!(
            extra_args(AgentId::Claude, raw),
            Err("Zusatzargumente für Claude Code enthalten Steuerzeichen.".into())
        );
    }
    assert_eq!(
        extra_args(AgentId::Opencode, &"a".repeat(1001)),
        Err("Zusatzargumente für OpenCode sind zu lang.".into())
    );
    let settings = AppSettings {
        agents: PerAgent::from_fn(|_| options("default", "default", "x\"")),
        ..AppSettings::default()
    };
    assert!(validate(&settings).is_err());
}

#[test]
fn shim_args_reject_cmd_metacharacters() {
    assert_eq!(
        check_shim_args(AgentId::Codex, &["--ok".into(), "a=b".into()]),
        Ok(())
    );
    for bad in ["a&b", "a|b", "<", ">", "^", "%PATH%", "!x!", "(x", "x)"] {
        assert!(
            check_shim_args(AgentId::Codex, &[bad.into()]).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn images_must_be_docker_references() {
    let mut settings = AppSettings::default();
    for image in [
        "node:22-trixie",
        "ghcr.io/org/image:1.2",
        "registry:5000/a/b@sha256:abc",
    ] {
        *settings.docker.images.get_mut(AgentId::Codex) = image.into();
        assert_eq!(validate(&settings), Ok(()), "{image}");
    }
    for image in ["", "-x", "node 22", "node;rm", "a\"b"] {
        *settings.docker.images.get_mut(AgentId::Codex) = image.into();
        assert_eq!(
            validate(&settings),
            Err("Docker-Image für Codex ist ungültig.".into()),
            "{image}"
        );
    }
}

#[test]
fn normalize_trims_and_fills_blanks() {
    let mut settings = AppSettings::default();
    *settings.agents.get_mut(AgentId::Claude) = options("  opus ", "", " --verbose ");
    *settings.docker.images.get_mut(AgentId::Opencode) = "  ".into();
    let normalized = normalize(settings);
    let claude = normalized.agents.get(AgentId::Claude);
    assert_eq!(
        (claude.model.as_str(), claude.effort.as_str()),
        ("opus", "default")
    );
    assert_eq!(claude.extra_args, "--verbose");
    assert_eq!(
        normalized.docker.images.get(AgentId::Opencode),
        DEFAULT_IMAGE
    );
    assert_eq!(normalize_options(options("", " ", "")).model, "default");
}

/// Load-time repair: each invalid field takes its default, valid neighbours stay (S1/F8).
#[test]
fn sanitize_replaces_only_the_invalid_fields() {
    let mut settings = AppSettings {
        default_env: EnvId::Wsl,
        terminal: TerminalId::Console,
        ..AppSettings::default()
    };
    *settings.agents.get_mut(AgentId::Claude) = options("--evil", "high", "--verbose");
    *settings.agents.get_mut(AgentId::Codex) = options("gpt-5.5", "ultra", "it's");
    *settings.agents.get_mut(AgentId::Opencode) = options("big-pickle", "max", "");
    *settings.docker.images.get_mut(AgentId::Claude) = "--privileged".into();
    *settings.docker.images.get_mut(AgentId::Codex) = "-eX=\u{A0}\" \"--privileged".into();
    *settings.docker.images.get_mut(AgentId::Opencode) = " ghcr.io/me/oc:1 ".into();
    let clean = sanitize(settings);
    assert_eq!(clean.default_env, EnvId::Local);
    assert_eq!(clean.terminal, TerminalId::Console);
    assert_eq!(
        clean.agents.get(AgentId::Claude),
        &options("default", "high", "--verbose")
    );
    assert_eq!(
        clean.agents.get(AgentId::Codex),
        &options("gpt-5.5", "default", "")
    );
    assert_eq!(
        clean.agents.get(AgentId::Opencode),
        &options("default", "max", "")
    );
    assert_eq!(clean.docker.images.get(AgentId::Claude), DEFAULT_IMAGE);
    assert_eq!(clean.docker.images.get(AgentId::Codex), DEFAULT_IMAGE);
    assert_eq!(
        clean.docker.images.get(AgentId::Opencode),
        "ghcr.io/me/oc:1"
    );
    assert_eq!(validate(&clean), Ok(()));
    let valid = AppSettings::default();
    assert_eq!(sanitize(valid.clone()), valid);
}

#[test]
fn image_checks_refuse_flags_and_non_ascii() {
    for image in [
        "--privileged",
        "-eX=\u{A0}\" \"--privileged",
        "node\u{A0}22",
        "ä",
    ] {
        assert!(!is_valid_image(image), "{image}");
    }
    assert!(is_valid_image(DEFAULT_IMAGE));
    assert!(!is_valid_image(&"a".repeat(256)));
}

#[test]
fn docker_ports_default_empty_are_normalized_and_validated() {
    assert!(AppSettings::default().docker.ports.is_empty());
    let mut settings = AppSettings::default();
    settings.docker.ports = vec![" 3000 ".into(), "".into(), "0.0.0.0:8080:80/udp".into()];
    let normalized = normalize(settings);
    assert_eq!(
        normalized.docker.ports,
        ["127.0.0.1:3000:3000", "0.0.0.0:8080:80/udp"]
    );
    assert_eq!(validate(&normalized), Ok(()));
    let mut invalid = AppSettings::default();
    invalid.docker.ports = vec!["3000".into(), "3000 --privileged".into()];
    let kept = normalize(invalid);
    assert_eq!(kept.docker.ports, ["3000", "3000 --privileged"]);
    assert_eq!(
        validate(&kept),
        Err("Port „3000 --privileged“ ist ungültig.".into())
    );
    let mut twice = AppSettings::default();
    twice.docker.ports = vec!["3000".into(), "3000:80".into()];
    assert_eq!(
        validate(&normalize(twice)),
        Err("Host-Port 3000 ist doppelt belegt.".into())
    );
}

#[test]
fn sanitize_drops_an_invalid_port_list_and_keeps_the_rest() {
    let mut settings = AppSettings::default();
    settings.docker.gh_token = true;
    settings.docker.ports = vec!["5173".into(), "-p".into()];
    let clean = sanitize(settings);
    assert!(clean.docker.ports.is_empty());
    assert!(clean.docker.gh_token);
    let mut valid = AppSettings::default();
    valid.docker.ports = vec!["5173".into()];
    assert_eq!(sanitize(valid).docker.ports, ["127.0.0.1:5173:5173"]);
}

#[test]
fn the_clone_root_is_empty_or_a_normalized_folder() {
    let mut settings = AppSettings::default();
    assert_eq!(settings.github.clone_root, "");
    assert_eq!(validate(&settings), Ok(()));
    settings.github.clone_root = r" D:/src\repos\ ".into();
    let normalized = normalize(settings.clone());
    assert_eq!(normalized.github.clone_root, r"D:\src\repos");
    assert_eq!(validate(&normalized), Ok(()));
    settings.github.clone_root = r"src\repos".into();
    assert_eq!(
        validate(&normalize(settings.clone())),
        Err("Klon-Ordner: Bitte einen absoluten Ordnerpfad wählen.".into())
    );
    assert_eq!(sanitize(settings).github.clone_root, "");
}
