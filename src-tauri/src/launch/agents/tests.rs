//! Per-agent argv: permission, model, effort and session flags in recipe order.
use super::*;

fn with(
    permission: PermissionId,
    model: &str,
    effort: &str,
    session: SessionMode,
) -> (LaunchOptions, SessionMode) {
    let options = LaunchOptions {
        model: model.into(),
        effort: effort.into(),
        permission,
        extra_args: String::new(),
    };
    (options, session)
}

#[test]
fn claude_flags_per_permission_model_effort_session() {
    let extra = || vec!["--verbose".to_string()];
    let (o, s) = with(PermissionId::Ask, "default", "default", SessionMode::New);
    assert_eq!(
        claude(&o, s, vec![]),
        [
            "--permission-mode",
            "default",
            "--allow-dangerously-skip-permissions"
        ]
    );
    let (o, s) = with(PermissionId::Auto, "opus", "high", SessionMode::Continue);
    assert_eq!(
        claude(&o, s, extra()),
        [
            "--permission-mode",
            "auto",
            "--allow-dangerously-skip-permissions",
            "--model",
            "opus",
            "--effort",
            "high",
            "--continue",
            "--verbose"
        ]
    );
    let (o, s) = with(PermissionId::Bypass, "default", "max", SessionMode::Resume);
    assert_eq!(
        claude(&o, s, vec![]),
        [
            "--dangerously-skip-permissions",
            "--effort",
            "max",
            "--resume"
        ]
    );
}

#[test]
fn opencode_flags_and_config() {
    let (o, s) = with(PermissionId::Ask, "default", "high", SessionMode::New);
    assert_eq!(opencode(&o, s, vec![], EnvId::Local), ["--standalone"]);
    assert_eq!(opencode_config(&o), None);
    let (o, s) = with(
        PermissionId::Auto,
        "opencode/big-pickle",
        "high",
        SessionMode::Continue,
    );
    assert_eq!(
        opencode(&o, s, vec!["--x".into()], EnvId::Local),
        ["--standalone", "--auto", "--continue", "--x"]
    );
    assert_eq!(
        opencode(&o, s, vec![], EnvId::Docker),
        ["--auto", "--continue"]
    );
    assert_eq!(
        opencode_config(&o).unwrap(),
        r#"{"model":"opencode/big-pickle#high"}"#
    );
    let (o, _) = with(PermissionId::Bypass, "a/b", "default", SessionMode::New);
    let config: serde_json::Value = serde_json::from_str(&opencode_config(&o).unwrap()).unwrap();
    assert_eq!(config["model"], "a/b");
    assert_eq!(
        config["permissions"],
        serde_json::json!([{ "action": "*", "resource": "*", "effect": "allow" }])
    );
}

#[test]
fn codex_flags_per_environment() {
    let (o, s) = with(PermissionId::Ask, "default", "default", SessionMode::New);
    assert_eq!(
        codex(&o, s, vec![], EnvId::Local),
        ["--sandbox", "read-only", "--ask-for-approval", "on-request"]
    );
    assert_eq!(
        codex(&o, s, vec![], EnvId::Docker),
        [
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "on-request"
        ]
    );
    let (o, s) = with(
        PermissionId::Auto,
        "gpt-6.1-sol",
        "xhigh",
        SessionMode::Continue,
    );
    assert_eq!(
        codex(&o, s, vec!["--x".into()], EnvId::Local),
        [
            "resume",
            "--last",
            "--approve-for-me",
            "-m",
            "gpt-6.1-sol",
            "-c",
            "model_reasoning_effort=xhigh",
            "--x"
        ]
    );
    assert_eq!(
        codex(&o, SessionMode::Resume, vec![], EnvId::Docker),
        [
            "resume",
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "on-request",
            "-c",
            "approvals_reviewer=auto_review",
            "-m",
            "gpt-6.1-sol",
            "-c",
            "model_reasoning_effort=xhigh"
        ]
    );
    let (o, s) = with(PermissionId::Bypass, "default", "default", SessionMode::New);
    for env in [EnvId::Local, EnvId::Docker] {
        assert_eq!(
            codex(&o, s, vec![], env),
            ["--dangerously-bypass-approvals-and-sandbox"]
        );
    }
}
