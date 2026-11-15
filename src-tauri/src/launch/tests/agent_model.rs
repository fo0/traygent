//! A workspace start set to „Modell & Effort“ „Agent“ (#43): no model or effort flag for any
//! agent; permission, session and extra arguments unchanged.
use super::*;

const CODEX: &str = r"C:\Users\max\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe";
const OPENCODE: &str =
    r"C:\Users\max\AppData\Roaming\npm\node_modules\@opencode\cli\bin\opencode.exe";

fn kept(agent: AgentId, model: &str, keep: bool) -> LaunchRequest {
    let mut req = request(agent, EnvId::Local, r"C:\x");
    req.options.model = model.into();
    req.options.effort = "high".into();
    req.options.extra_args = "--verbose".into();
    req.session = SessionMode::Continue;
    req.keep_agent_model = keep;
    req
}

fn plan_script(req: &LaunchRequest, cli: &str) -> String {
    script(&plan(req, &console(), &resolved(Some((cli, false)))).unwrap())
}

#[test]
fn claude_gets_no_model_or_effort_but_everything_else() {
    let set = plan_script(&kept(AgentId::Claude, "opus", false), CLAUDE);
    assert!(set.contains("'--model' 'opus' '--effort' 'high'"), "{set}");
    let script = plan_script(&kept(AgentId::Claude, "opus", true), CLAUDE);
    assert!(
        !script.contains("--model") && !script.contains("--effort"),
        "{script}"
    );
    assert!(script.contains(&format!(
        "& '{CLAUDE}' '--permission-mode' 'auto' '--allow-dangerously-skip-permissions' \
         '--continue' '--verbose'"
    )));
}

#[test]
fn codex_gets_no_model_or_reasoning_effort() {
    let model = "gpt-6-sol";
    let set = plan_script(&kept(AgentId::Codex, model, false), CODEX);
    assert!(set.contains(&format!("'-m' '{model}'")), "{set}");
    assert!(set.contains("model_reasoning_effort=high"), "{set}");
    let script = plan_script(&kept(AgentId::Codex, model, true), CODEX);
    assert!(!script.contains("'-m'"), "{script}");
    assert!(!script.contains("model_reasoning_effort"), "{script}");
    assert!(script.contains("'resume' '--last' '--approve-for-me' '--verbose'"));
}

#[test]
fn opencode_gets_no_model_key_but_keeps_the_bypass_rule() {
    let mut set = kept(AgentId::Opencode, "opencode/big-pickle", false);
    set.options.permission = PermissionId::Bypass;
    let config = env_value(&plan_script(&set, OPENCODE), "OPENCODE_CONFIG_CONTENT").unwrap();
    assert!(
        config.contains(r#""model":"opencode/big-pickle#high""#),
        "{config}"
    );
    let keep = LaunchRequest {
        keep_agent_model: true,
        ..set
    };
    let script = plan_script(&keep, OPENCODE);
    let config = env_value(&script, "OPENCODE_CONFIG_CONTENT").unwrap();
    assert!(!config.contains("model"), "{config}");
    assert!(config.contains(r#""effect":"allow""#), "{config}");
    // Without bypass nothing is left to configure: no variable at all.
    let plain = LaunchRequest {
        options: LaunchOptions {
            permission: PermissionId::Auto,
            ..keep.options.clone()
        },
        ..keep
    };
    let script = plan_script(&plain, OPENCODE);
    assert!(
        env_value(&script, "OPENCODE_CONFIG_CONTENT").is_none(),
        "{script}"
    );
    assert!(script.contains("'--standalone' '--auto' '--continue' '--verbose'"));
}
