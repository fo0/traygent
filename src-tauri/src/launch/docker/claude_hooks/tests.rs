use super::*;
use crate::testing::TempDir;
use serde_json::json;

const STANDARD: &str = r"C:\Users\max\.claude";
const TEAM: &str = r"C:\Users\max\.claude-team";
const WORKSPACE: &str = r"D:\Repos\Shop";
const LINUX: &str = "/workspace/shop-1234abcd";
const PINNED: &str = "C:/Users/max/AppData/Local/nvm/v24.21.0/node.exe";

fn team() -> Roots {
    Roots::new(TEAM, STANDARD, WORKSPACE, LINUX).unwrap()
}

fn standard() -> Roots {
    Roots::new(STANDARD, STANDARD, WORKSPACE, LINUX).unwrap()
}

fn hook(command: &str, args: &[&str]) -> Value {
    json!({ "type": "command", "command": command, "args": args })
}

fn settings(handlers: &[Value]) -> Vec<u8> {
    let value = json!({ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": handlers }] } });
    serde_json::to_vec(&value).unwrap()
}

/// The first group's handlers after projection; `None` when nothing was projected.
fn projected(bytes: &[u8], roots: &Roots, mise: bool) -> Option<Value> {
    let out = project(bytes, roots, mise).unwrap()?;
    let value: Value = serde_json::from_slice(&out.settings).unwrap();
    Some(value["hooks"]["PreToolUse"][0]["hooks"].clone())
}

#[test]
fn a_pinned_nvm_hook_runs_its_exact_version_through_mise_and_keeps_everything_else() {
    let input = r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "mcp__fake__tool",
        "hooks": [
          {
            "type": "command",
            "command": "C:/Users/max/AppData/Local/nvm/v24.21.0/node.exe",
            "args": ["C:/Users/max/.claude/hooks/guard.mjs", "pre", "--mode=strict"],
            "timeout": 10,
            "onFailure": "block"
          },
          { "type": "command", "command": "echo C:\\Users\\max\\.claude\\x && exit 0" }
        ]
      }
    ]
  },
  "permissions": { "allow": ["Bash(git status)"] },
  "cleanupPeriodDays": 30.50
}"#;
    let out = project(input.as_bytes(), &team(), true).unwrap().unwrap();
    assert_eq!(out.pins.into_iter().collect::<Vec<_>>(), ["24.21.0"]);
    let text = String::from_utf8(out.settings).unwrap();
    let mut expected: Value = serde_json::from_str(input).unwrap();
    let handler = &mut expected["hooks"]["PreToolUse"][0]["hooks"][0];
    handler["command"] = json!(NODE_HOOK);
    handler["args"] = json!([
        "24.21.0",
        "/home/node/.claude/hooks/guard.mjs",
        "pre",
        "--mode=strict"
    ]);
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), expected);
    // File order and number tokens survive the rewrite.
    let at = |key: &str| text.find(key).unwrap();
    assert!(at("\"model\"") < at("\"hooks\"") && at("\"hooks\"") < at("\"permissions\""));
    assert!(at("\"command\"") < at("\"args\"") && at("\"args\"") < at("\"onFailure\""));
    assert!(text.contains("30.50") && text.ends_with("}\n"));
}

#[test]
fn unpinned_node_or_mise_off_runs_the_containers_own_node() {
    let script = r"C:\Users\max\.claude-team\hooks\a.mjs";
    for (command, mise) in [
        (r"C:\Users\max\AppData\Local\nvm\v24.21.0\node.exe", false),
        (r"c:\Program Files\nodejs\NODE.EXE", true),
        ("C:/Program Files/nodejs/node", true),
        ("node.exe", true),
        (r"C:\nvm\v24.21\node.exe", true),
        (r"C:\nvm\v024.1.0\node.exe", true),
        (r"C:\nvm\v24.21.0-rc.1\node.exe", true),
        (r"C:\nvm\v24.21.0\bin\node.exe", true),
    ] {
        let handlers = projected(&settings(&[hook(command, &[script])]), &team(), mise);
        let want = hook(NATIVE_NODE, &["/home/node/.claude/hooks/a.mjs"]);
        assert_eq!(handlers.unwrap()[0], want, "{command}");
    }
    let pinned = r"D:\nvm4w\V22.0.0\node.exe";
    let handlers = projected(&settings(&[hook(pinned, &["-v"])]), &team(), true);
    assert_eq!(handlers.unwrap()[0], hook(NODE_HOOK, &["22.0.0", "-v"]));
}

#[test]
fn only_genuine_paths_under_a_mount_map_and_never_into_another_profile() {
    let args = [
        "c:/USERS/max/.CLAUDE-team/Hooks/A.mjs",
        r"C:\Users\max\.claude\hooks\shared.mjs",
        r"D:\Repos\Shop\scripts\check.mjs",
        "d:/repos/shop",
        r"C:\Users\max\.claude-team",
        r"C:\\Users\\max\.claude-team\\x.mjs",
        r"C:\Users\max\.claude-other\hooks\a.mjs",
        r"C:\Users\max\.claude\..\.claude-other\a.mjs",
        r"C:\Users\max\.claude-team\.\a.mjs",
        r"C:\Users\max\.claude-teamx\a.mjs",
        r"D:\Repos\Shopping\a.mjs",
        r"\\server\share\a.mjs",
        r"C:relative\a.mjs",
        r"E:\tools\a.mjs",
        r"--require=C:\Users\max\.claude-team\x.mjs",
        "pre",
    ];
    let handlers = projected(&settings(&[hook("node", &args)]), &team(), true).unwrap();
    let mut want = vec![
        "/home/node/.claude/Hooks/A.mjs",
        "/home/node/.claude/hooks/shared.mjs",
        "/workspace/shop-1234abcd/scripts/check.mjs",
        "/workspace/shop-1234abcd",
        "/home/node/.claude",
        "/home/node/.claude/x.mjs",
    ];
    want.extend(&args[6..]);
    assert_eq!(handlers[0], hook(NATIVE_NODE, &want));
}

#[test]
fn the_standard_profile_maps_its_own_home_only() {
    let args = [
        r"C:\Users\max\.claude\hooks\a.mjs",
        r"C:\Users\max\.claude-team\hooks\a.mjs",
    ];
    let handlers = projected(&settings(&[hook("node", &args)]), &standard(), false).unwrap();
    let want = ["/home/node/.claude/hooks/a.mjs", args[1]];
    assert_eq!(handlers[0], hook(NATIVE_NODE, &want));
    // The same directory spelled differently is still the Standard profile.
    let roots = Roots::new(STANDARD, "c:/users/MAX/.claude/", WORKSPACE, LINUX).unwrap();
    assert!(roots.standard.is_none());
    assert!(team().standard.is_some());
}

#[test]
fn eval_code_stays_untouched_and_script_arguments_still_map() {
    let code = "require('C:/Users/max/.claude-team/x.js')";
    let data = "C:/Users/max/.claude-team/data.json";
    let cases: [(&[&str], &[&str]); 3] = [
        (
            &["-e", code, data],
            &["24.21.0", "-e", code, "/home/node/.claude/data.json"],
        ),
        (
            &["--eval", r"C:\Users\max\.claude-team\x.js"],
            &["24.21.0", "--eval", r"C:\Users\max\.claude-team\x.js"],
        ),
        (
            &["C:/Users/max/.claude-team/s.mjs", "-e", data],
            &[
                "24.21.0",
                "/home/node/.claude/s.mjs",
                "-e",
                "/home/node/.claude/data.json",
            ],
        ),
    ];
    for (args, want) in cases {
        let handlers = projected(&settings(&[hook(PINNED, args)]), &team(), true);
        assert_eq!(handlers.unwrap()[0], hook(NODE_HOOK, want), "{args:?}");
    }
}

#[test]
fn hooks_that_need_no_projection_stay_byte_for_byte_and_alone_give_nothing() {
    let untouched = [
        json!({ "type": "command", "command": PINNED }),
        json!({ "type": "command", "command": "C:/Users/max/.claude/hooks/x.cmd" }),
        hook(
            r"C:\Python\python.exe",
            &[r"C:\Users\max\.claude-team\x.py"],
        ),
        hook("bash", &[r"C:\Users\max\.claude-team\x.sh"]),
        json!({ "type": "prompt", "command": PINNED, "args": [STANDARD] }),
        json!({ "type": "command", "command": PINNED, "args": [TEAM, 1] }),
        json!({ "type": "command", "command": PINNED, "args": TEAM }),
        hook("node", &["-e", "process.exit(0)", "--", "plain"]),
        json!("not a handler"),
    ];
    assert_eq!(
        project(&settings(&untouched), &team(), false).unwrap(),
        None
    );
    let odd_shapes = [
        json!({}),
        json!({ "model": "opus" }),
        json!({ "hooks": [hook(PINNED, &[])] }),
        json!({ "hooks": { "PreToolUse": { "hooks": [hook(PINNED, &[])] } } }),
        json!({ "hooks": { "PreToolUse": [{ "hooks": hook(PINNED, &[]) }] } }),
        json!({ "nested": { "hooks": { "PreToolUse": [{ "hooks": [hook(PINNED, &[])] }] } } }),
    ];
    for shape in odd_shapes {
        let bytes = serde_json::to_vec(&shape).unwrap();
        assert_eq!(project(&bytes, &team(), true).unwrap(), None, "{shape}");
    }
    let mut mixed = untouched.to_vec();
    mixed.push(hook(PINNED, &[]));
    let handlers = projected(&settings(&mixed), &team(), false).unwrap();
    assert_eq!(handlers.as_array().unwrap()[..untouched.len()], untouched);
    assert_eq!(handlers[untouched.len()], hook(NATIVE_NODE, &[]));
}

#[test]
fn a_document_that_is_no_json_object_is_refused_and_a_bom_is_read() {
    for bad in [&b"not json"[..], b"[1]", b"{\"a\":1", b""] {
        assert_eq!(project(bad, &team(), true), Err(NOT_AN_OBJECT.into()));
    }
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend(settings(&[hook(PINNED, &[])]));
    let handlers = projected(&bom, &team(), true).unwrap();
    assert_eq!(handlers[0], hook(NODE_HOOK, &["24.21.0"]));
}

#[test]
fn roots_accept_drive_paths_and_a_clean_linux_workspace_only() {
    let roots = |s: &str, d: &str, w: &str, l: &str| Roots::new(s, d, w, l).map(|_| ());
    assert!(roots(TEAM, STANDARD, r"C:\", "/workspace/x/").is_ok());
    let cases = [
        (
            ("relative", STANDARD, WORKSPACE, LINUX),
            "Ungültiger Claude-Profilordner.",
        ),
        (
            (TEAM, r"\\server\x", WORKSPACE, LINUX),
            "Ungültiger Claude-Standardordner.",
        ),
        (
            (TEAM, STANDARD, "C:", LINUX),
            "Ungültiger Windows-Arbeitsordner.",
        ),
        (
            (TEAM, STANDARD, WORKSPACE, "workspace"),
            "Ungültiger Linux-Arbeitsordner.",
        ),
        (
            (TEAM, STANDARD, WORKSPACE, "/"),
            "Ungültiger Linux-Arbeitsordner.",
        ),
        (
            (TEAM, STANDARD, WORKSPACE, "/a/../b"),
            "Ungültiger Linux-Arbeitsordner.",
        ),
        (
            (TEAM, STANDARD, WORKSPACE, "/a//b"),
            "Ungültiger Linux-Arbeitsordner.",
        ),
        (
            (TEAM, STANDARD, WORKSPACE, r"/a\b"),
            "Ungültiger Linux-Arbeitsordner.",
        ),
    ];
    for ((s, d, w, l), message) in cases {
        assert_eq!(roots(s, d, w, l), Err(message.into()), "{s} {d} {w} {l}");
    }
}

#[test]
fn the_file_step_reads_the_original_and_writes_only_the_output() {
    let temp = TempDir::new("claude-hooks");
    let original = temp.0.join("settings.json");
    let output = temp.0.join("projected.json");
    let roots = team();
    std::fs::write(&output, b"stale").unwrap();
    assert_eq!(
        project_file(&original, &output, &roots, true),
        Ok(Vec::new())
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"");
    assert!(!original.exists());

    let input = settings(&[hook(PINNED, &[r"C:\Users\max\.claude\hooks\a.mjs"])]);
    std::fs::write(&original, &input).unwrap();
    assert_eq!(
        project_file(&original, &output, &roots, true),
        Ok(vec!["24.21.0".into()])
    );
    let want = project(&input, &roots, true).unwrap().unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), want.settings);
    assert_eq!(std::fs::read(&original).unwrap(), input);

    std::fs::write(&original, settings(&[hook("bash", &["x"])])).unwrap();
    assert_eq!(
        project_file(&original, &output, &roots, true),
        Ok(Vec::new())
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"");

    let same = project_file(&original, &original, &roots, true);
    assert_eq!(same, Err(SAME_FILE.into()));
    assert_eq!(
        std::fs::read(&original).unwrap(),
        settings(&[hook("bash", &["x"])])
    );
}

#[test]
fn the_file_step_fails_without_quoting_or_writing_anything() {
    let temp = TempDir::new("claude-hooks-fail");
    let original = temp.0.join("settings.json");
    let output = temp.0.join("projected.json");
    std::fs::write(&output, b"keep").unwrap();
    std::fs::write(&original, b"{\"env\":{\"TOKEN\":\"SECRET-MARKER\"").unwrap();
    let error = project_file(&original, &output, &team(), true).unwrap_err();
    assert_eq!(error, NOT_AN_OBJECT);
    assert!(!error.contains("SECRET-MARKER"));
    std::fs::remove_file(&original).unwrap();
    let file = std::fs::File::create(&original).unwrap();
    file.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(file);
    let error = project_file(&original, &output, &team(), true);
    assert_eq!(error, Err(TOO_LARGE.into()));
    std::fs::remove_file(&original).unwrap();
    std::fs::create_dir(&original).unwrap();
    let error = project_file(&original, &output, &team(), true);
    assert_eq!(error, Err(READ_FAILED.into()));
    assert_eq!(std::fs::read(&output).unwrap(), b"keep");
}

fn cli(args: &[&str]) -> (Option<i32>, String) {
    let mut stderr = Vec::new();
    let code = handle(
        args.iter().map(OsString::from),
        &mut Vec::new(),
        &mut stderr,
    );
    (code, String::from_utf8(stderr).unwrap())
}

#[test]
fn the_cli_answers_only_its_own_flag_and_reports_errors_in_german() {
    assert_eq!(cli(&[]), (None, String::new()));
    assert_eq!(cli(&["--other", "x"]), (None, String::new()));
    let usage = (Some(1), format!("{USAGE}\n"));
    assert_eq!(cli(&[CLI_ARG]), usage);
    assert_eq!(
        cli(&[CLI_ARG, TEAM, STANDARD, WORKSPACE, LINUX, "out"]),
        usage
    );
    let extra = [CLI_ARG, TEAM, STANDARD, WORKSPACE, LINUX, "out", "1", "x"];
    assert_eq!(cli(&extra), usage);
    assert_eq!(
        cli(&[CLI_ARG, TEAM, STANDARD, WORKSPACE, LINUX, "out", "2"]),
        usage
    );
    let bad_root = cli(&[CLI_ARG, "x", STANDARD, WORKSPACE, LINUX, "out", "1"]);
    assert_eq!(
        bad_root,
        (Some(1), "Ungültiger Claude-Profilordner.\n".into())
    );
}

#[test]
fn the_cli_writes_an_empty_output_when_the_profile_has_no_settings() {
    let temp = TempDir::new("claude-hooks-cli");
    let output = temp.0.join("projected.json");
    std::fs::write(&output, b"stale").unwrap();
    let selected = r"C:\traygent-test-missing-profile-6d1f\.claude-team";
    let out = output.to_str().unwrap();
    let result = cli(&[CLI_ARG, selected, STANDARD, WORKSPACE, LINUX, out, "1"]);
    assert_eq!(result, (Some(0), String::new()));
    assert_eq!(std::fs::read(&output).unwrap(), b"");
}

/// The real Windows path: the selected directory is a drive path the CLI reads from.
#[cfg(windows)]
#[test]
fn the_cli_projects_a_real_profile_on_windows() {
    let temp = TempDir::new("claude-hooks-windows");
    let selected = temp.0.join(".claude-team");
    std::fs::create_dir(&selected).unwrap();
    let dir = selected.to_str().unwrap();
    let script = format!(r"{dir}\hooks\a.mjs");
    let input = settings(&[hook(PINNED, &[&script])]);
    std::fs::write(selected.join("settings.json"), &input).unwrap();
    let output = temp.0.join("projected.json");
    let out = output.to_str().unwrap();
    let mut pins = Vec::new();
    let result = handle(
        [CLI_ARG, dir, STANDARD, WORKSPACE, LINUX, out, "1"].map(OsString::from),
        &mut pins,
        &mut Vec::new(),
    );
    assert_eq!(result, Some(0));
    assert_eq!(pins, b"24.21.0\n");
    let value: Value = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    let want = hook(NODE_HOOK, &["24.21.0", "/home/node/.claude/hooks/a.mjs"]);
    assert_eq!(value["hooks"]["PreToolUse"][0]["hooks"][0], want);
    assert_eq!(
        std::fs::read(selected.join("settings.json")).unwrap(),
        input
    );
    std::fs::write(selected.join("settings.json"), b"{\"k\":\"SECRET-MARKER\"").unwrap();
    let (code, stderr) = cli(&[CLI_ARG, dir, STANDARD, WORKSPACE, LINUX, out, "0"]);
    assert_eq!(
        (code, stderr.as_str()),
        (
            Some(2),
            "Die Claude-Einstellungen (settings.json) sind kein gültiges JSON-Objekt.\n"
        )
    );
}
