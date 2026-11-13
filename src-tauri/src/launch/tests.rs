use super::quote::base64;
use super::*;
use crate::models::{LaunchOptions, PermissionId, SessionMode};

pub(crate) const PS: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
pub(crate) const WT: &str = r"C:\Users\max\AppData\Local\Microsoft\WindowsApps\wt.exe";
const CLAUDE: &str = r"C:\Users\max\.local\bin\claude.exe";

fn decode_bytes(text: &str) -> Vec<u8> {
    let value = |c: u8| ALPHABET_TEST.iter().position(|a| *a == c).unwrap() as u32;
    let mut bytes = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let pad = chunk.iter().filter(|c| **c == b'=').count();
        let n = chunk
            .iter()
            .fold(0, |n, c| (n << 6) | if *c == b'=' { 0 } else { value(*c) });
        bytes.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    bytes
}

pub(crate) fn decode_base64(text: &str) -> String {
    String::from_utf8(decode_bytes(text)).unwrap()
}

/// What PowerShell runs for `-EncodedCommand <text>`: Base64, then UTF-16LE.
pub(crate) fn decode_command(text: &str) -> String {
    let bytes = decode_bytes(text);
    assert_eq!(bytes.len() % 2, 0);
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).unwrap()
}

/// The script of a plan: the value after `-EncodedCommand`, its last argument.
pub(crate) fn script(plan: &LaunchPlan) -> String {
    let flag = plan.args.len() - 2;
    assert_eq!(plan.args[flag], "-EncodedCommand", "{:?}", plan.args);
    decode_command(&plan.args[flag + 1])
}
const ALPHABET_TEST: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Every Base64 value the script decodes into `$env:<name>`.
pub(crate) fn env_value(script: &str, name: &str) -> Option<String> {
    let marker =
        format!("$env:{name}=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('");
    let start = script.find(&marker)? + marker.len();
    let end = script[start..].find('\'')? + start;
    Some(decode_base64(&script[start..end]))
}

pub(crate) fn resolved(cli: Option<(&str, bool)>) -> Resolved {
    Resolved {
        cli: cli.map(|(path, shim)| Candidate {
            path: path.into(),
            shim,
        }),
        docker: Some("C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe".into()),
        gh: None,
        gitconfig: None,
        git: None,
        powershell: PS.into(),
        wt: Some(WT.into()),
        nonce: "a1b2c3".into(),
        profile_dir: None,
    }
}

pub(crate) fn request(agent: AgentId, env: EnvId, folder: &str) -> LaunchRequest {
    LaunchRequest {
        agent,
        env,
        folder: folder.into(),
        repo: None,
        options: LaunchOptions::default(),
        session: SessionMode::New,
        profile: None,
        ports: Vec::new(),
    }
}

fn tab(wt: &str) -> Tab<'_> {
    Tab { wt, shared: false }
}

fn console() -> AppSettings {
    AppSettings {
        terminal: TerminalId::Console,
        ..AppSettings::default()
    }
}

#[test]
fn ps_literals_double_every_single_quote_powershell_knows() {
    assert_eq!(quote::ps_literal("plain"), "'plain'");
    assert_eq!(quote::ps_literal(""), "''");
    assert_eq!(quote::ps_literal("Max's"), "'Max''s'");
    assert_eq!(
        quote::ps_literal("Max\u{2019}s \u{2018}x\u{201A}\u{201B}"),
        "'Max\u{2019}\u{2019}s \u{2018}\u{2018}x\u{201A}\u{201A}\u{201B}\u{201B}'"
    );
    assert_eq!(
        quote::ps_literal("$env:X `n; & | \"q\""),
        "'$env:X `n; & | \"q\"'"
    );
    let line = quote::invoke("C:\\a b\\x.exe", &["--m".into(), "it's".into()]);
    assert_eq!(line, "& 'C:\\a b\\x.exe' '--m' 'it''s'");
}

#[test]
fn base64_matches_rfc_4648_vectors() {
    let vectors = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
        ("ü", "w7w="),
    ];
    for (plain, encoded) in vectors {
        assert_eq!(base64(plain.as_bytes()), encoded);
        assert_eq!(decode_base64(encoded), plain);
    }
    let script = quote::set_env_base64("X", "{\"a\":\"b;c'\"}");
    assert!(!script[6..].contains(['"', ';']));
    assert_eq!(env_value(&script, "X").unwrap(), "{\"a\":\"b;c'\"}");
}

#[test]
fn encoded_commands_are_utf16le_base64_without_separators() {
    let script = "Set-Location -LiteralPath 'C:\\a b; 100%'; & 'x' \"q\" \u{2019}ü\u{1F600}";
    let encoded = quote::encode_command(script);
    assert_eq!(decode_command(&encoded), script);
    assert!(encoded
        .bytes()
        .all(|c| ALPHABET_TEST.contains(&c) || c == b'='));
    // PowerShell's own example: `dir` → ZABpAHIA.
    assert_eq!(quote::encode_command("dir"), "ZABpAHIA");
}

#[test]
fn an_encoded_command_over_the_limit_fails_in_german() {
    let ok = wrap("t", r"C:\x", &"a".repeat(11_000), PS, Some(tab(WT)), false).unwrap();
    assert_eq!(script(&ok).len(), 11_000);
    let long = wrap("t", r"C:\x", &"a".repeat(12_000), PS, None, false);
    assert_eq!(long, Err(COMMAND_TOO_LONG.into()));
    assert!(COMMAND_TOO_LONG.starts_with("Der Startbefehl ist zu lang."));
}

#[test]
fn a_console_launch_runs_powershell_in_the_folder() {
    let plan = plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\Code\demo\"),
        &console(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    assert_eq!(plan.program, PS);
    assert_eq!(plan.terminal, TerminalId::Console);
    assert_eq!(plan.cwd, r"C:\Code\demo");
    assert_eq!(plan.title, "\u{2733} demo");
    assert_eq!(plan.args[..3], ["-NoLogo", "-NoExit", "-EncodedCommand"]);
    assert_eq!(plan.args.len(), 4);
    assert_eq!(
        script(&plan),
        "$Host.UI.RawUI.WindowTitle='\u{2733} demo'; \
         Set-Location -LiteralPath 'C:\\Code\\demo' -ErrorAction Stop; \
         $env:CLAUDE_CODE_DISABLE_TERMINAL_TITLE='1'; \
         $tgCode=$null; $tgFailed=$false; \
         try { & 'C:\\Users\\max\\.local\\bin\\claude.exe' '--permission-mode' 'auto' '--allow-dangerously-skip-permissions'; \
         $tgCode=$LASTEXITCODE } catch { $tgFailed=$true; throw } \
         finally { if (-not $tgFailed -and ($null -eq $tgCode -or $tgCode -in 0,130,-1073741510)) { [Environment]::Exit(0) } }; \
         Write-Host ''; Write-Host ('[traygent] Claude Code wurde mit Code ' + $tgCode + ' beendet.')"
    );
}

/// A failed Set-Location must end the script before the agent starts (S5).
#[test]
fn the_folder_is_entered_with_stop_before_anything_runs() {
    for env in [EnvId::Local, EnvId::Docker] {
        let plan = plan(
            &request(AgentId::Codex, env, r"C:\Code\demo"),
            &console(),
            &resolved(Some((r"C:\x\codex.exe", false))),
        )
        .unwrap();
        let script = script(&plan);
        let statements: Vec<_> = script.split("; ").collect();
        assert!(statements[0].starts_with("$Host.UI.RawUI.WindowTitle="));
        assert_eq!(
            statements[1],
            "Set-Location -LiteralPath 'C:\\Code\\demo' -ErrorAction Stop"
        );
        assert!(statements[2..].iter().all(|s| !s.contains("Set-Location")));
        assert!(script.matches("& '").count() == 1, "{script}");
    }
}

#[test]
fn special_folder_names_stay_literal() {
    let folder = r"C:\Max's Projekt; & Co (1) 100%";
    let plan = plan(
        &request(AgentId::Claude, EnvId::Local, folder),
        &AppSettings::default(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    // wt splits at `;` and expands `%VAR%` in its options: no `-d`, Set-Location alone sets
    // the folder, the tab title gets `,` and a fullwidth percent sign.
    assert_eq!(
        plan.args[..5],
        [
            "new-tab",
            "--title",
            "\u{2733} Max's Projekt, & Co (1) 100\u{FF05}",
            "--suppressApplicationTitle",
            PS
        ]
    );
    let text = script(&plan);
    assert!(text.contains(r"Set-Location -LiteralPath 'C:\Max''s Projekt; & Co (1) 100%'"));
    assert!(text.contains("$Host.UI.RawUI.WindowTitle='\u{2733} Max''s Projekt; & Co (1) 100%'"));
    let percent = super::plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\%USERPROFILE%"),
        &AppSettings::default(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    assert_eq!(percent.args[..2], ["new-tab", "--title"]);
    assert_eq!(percent.args[2], "\u{2733} \u{FF05}USERPROFILE\u{FF05}");
    let console = super::plan(
        &request(AgentId::Claude, EnvId::Local, folder),
        &console(),
        &resolved(Some((CLAUDE, false))),
    )
    .unwrap();
    assert_eq!(console.cwd, folder);
    assert!(script(&console).contains(r"'C:\Max''s Projekt; & Co (1) 100%'"));
}

#[test]
fn launch_requests_are_validated() {
    let settings = AppSettings::default();
    let ok = resolved(Some((CLAUDE, false)));
    let missing = plan(
        &request(AgentId::Claude, EnvId::Local, r"C:\x"),
        &settings,
        &resolved(None),
    );
    assert_eq!(missing, Err("Claude Code ist nicht installiert.".into()));
    let wsl = plan(
        &request(AgentId::Claude, EnvId::Wsl, r"C:\x"),
        &settings,
        &ok,
    );
    assert_eq!(wsl, Err("WSL ist noch nicht verfügbar.".into()));
    let mut resume = request(AgentId::Opencode, EnvId::Local, r"C:\x");
    resume.session = SessionMode::Resume;
    assert_eq!(
        plan(&resume, &settings, &ok),
        Err("OpenCode unterstützt diesen Session-Modus nicht.".into())
    );
    let mut quoted = request(AgentId::Claude, EnvId::Local, r"C:\x");
    quoted.options.extra_args = "--x \"y\"".into();
    assert!(plan(&quoted, &settings, &ok).is_err());
    let relative = plan(&request(AgentId::Claude, EnvId::Local, "x"), &settings, &ok);
    assert_eq!(
        relative,
        Err("Bitte einen absoluten Ordnerpfad wählen.".into())
    );
}

/// A CLI resolved from a relative PATH entry would run from Traygent's cwd (S4).
#[test]
fn only_absolute_cli_and_docker_paths_launch() {
    let settings = console();
    let req = request(AgentId::Claude, EnvId::Local, r"C:\x");
    for cli in [
        "claude.exe",
        r".\claude.exe",
        r"bin\claude.exe",
        "/usr/bin/claude",
    ] {
        assert_eq!(
            plan(&req, &settings, &resolved(Some((cli, false)))),
            Err("Der Pfad zu Claude Code ist kein absoluter Windows-Pfad.".into()),
            "{cli}"
        );
    }
    let mut relative_docker = resolved(None);
    relative_docker.docker = Some("docker.exe".into());
    assert_eq!(
        plan(
            &request(AgentId::Codex, EnvId::Docker, r"C:\x"),
            &settings,
            &relative_docker
        ),
        Err("Der Pfad zu Docker ist kein absoluter Windows-Pfad.".into())
    );
}

#[test]
fn npm_shims_refuse_cmd_metacharacters() {
    let shim = r"C:\Users\max\AppData\Roaming\npm\codex.cmd";
    let mut req = request(AgentId::Codex, EnvId::Local, r"C:\x");
    for extra in ["--note a&b", "--note (x)", "--note x)"] {
        req.options.extra_args = extra.into();
        let error = plan(&req, &console(), &resolved(Some((shim, true)))).unwrap_err();
        assert!(error.contains("& | < > ^ % ! ( )"), "{error}");
    }
    let exe = r"C:\Users\max\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe";
    assert!(plan(&req, &console(), &resolved(Some((exe, false)))).is_ok());
}

#[test]
fn opencode_local_passes_its_config_through_base64() {
    let mut req = request(AgentId::Opencode, EnvId::Local, r"C:\x");
    req.options.model = "opencode/big-pickle".into();
    req.options.effort = "max".into();
    req.options.permission = PermissionId::Bypass;
    let exe = r"C:\Users\max\AppData\Roaming\npm\node_modules\@opencode\cli\bin\opencode.exe";
    let plan = plan(&req, &console(), &resolved(Some((exe, false)))).unwrap();
    let script = script(&plan);
    let config = env_value(&script, "OPENCODE_CONFIG_CONTENT").unwrap();
    assert!(config.contains(r#""model":"opencode/big-pickle#max""#));
    assert!(config.contains(r#""effect":"allow""#));
    assert!(script.contains(&format!(
        "try {{ & '{exe}' '--standalone' '--auto'; $tgCode="
    )));
}

#[test]
fn install_opens_powershell_with_the_catalog_command() {
    let plan = install_plan(
        AgentId::Opencode,
        &console(),
        &resolved(None),
        r"C:\Users\max",
    )
    .unwrap();
    assert_eq!(plan.cwd, r"C:\Users\max");
    assert_eq!(plan.title, "OpenCode installieren");
    assert_eq!(
        plan.args[..5],
        [
            "-NoLogo",
            "-NoExit",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand"
        ]
    );
    assert!(script(&plan).contains("; $LASTEXITCODE=0; npm install -g @opencode/cli@latest; if "));
    let tab = install_plan(
        AgentId::Claude,
        &AppSettings::default(),
        &resolved(None),
        r"C:\Users\max",
    )
    .unwrap();
    assert_eq!(tab.program, WT);
    assert!(script(&tab).contains("; irm https://claude.ai/install.ps1 | iex; "));
}

/// „Fertig“ only after a successful installer, else a German failure line (F9).
#[test]
fn the_install_result_line_depends_on_the_installer() {
    let plan = install_plan(AgentId::Codex, &console(), &resolved(None), r"C:\Users\max").unwrap();
    let script = script(&plan);
    let (_, check) = script
        .split_once("irm https://chatgpt.com/codex/install.ps1 | iex; ")
        .unwrap();
    assert_eq!(
        check,
        "if ($? -and $LASTEXITCODE -eq 0) { Write-Host ''; \
         Write-Host '[traygent] Fertig. Danach in Traygent die Erkennung aktualisieren.' } \
         else { Write-Host ''; \
         Write-Host '[traygent] Installation fehlgeschlagen. Bitte die Meldungen oben prüfen.' }"
    );
}

/// PowerShell 5.1 strips embedded `"`, drops empty args and mis-parses a spaced arg ending
/// in `\` — no element of any plan may hit one of those.
#[test]
fn no_plan_ever_contains_powershell_51_pitfalls() {
    let folders = [
        r"C:\",
        r"C:\Code\demo",
        r"C:\Max's Projekt\& Co; x",
        r"D:\Ünïcode Ordner\",
        r"C:\100% (sicher)",
    ];
    let mut resolved = resolved(Some((CLAUDE, false)));
    resolved.gh = Some(r"C:\Program Files\GitHub CLI\gh.exe".into());
    resolved.gitconfig = Some(r"C:\Users\max\.gitconfig".into());
    let mut count = 0;
    for terminal in [TerminalId::WindowsTerminal, TerminalId::Console] {
        let mut settings = AppSettings {
            terminal,
            ..AppSettings::default()
        };
        settings.docker.gh_token = true;
        for agent in AgentId::ALL {
            for env in [EnvId::Local, EnvId::Docker] {
                for permission in [PermissionId::Ask, PermissionId::Auto, PermissionId::Bypass] {
                    for session in [SessionMode::New, SessionMode::Continue, SessionMode::Resume] {
                        for folder in folders {
                            let mut req = request(agent, env, folder);
                            req.options.permission = permission;
                            req.options.effort = "high".into();
                            req.options.extra_args = "--a b;c".into();
                            req.session = session;
                            if env == EnvId::Docker {
                                req.ports = vec!["3000".into(), "0.0.0.0:53:53/udp".into()];
                            }
                            let Ok(plan) = plan(&req, &settings, &resolved) else {
                                assert_eq!(
                                    (agent, session),
                                    (AgentId::Opencode, SessionMode::Resume)
                                );
                                continue;
                            };
                            count += 1;
                            for arg in plan.args.iter().chain([&plan.program]) {
                                assert!(!arg.is_empty() && !arg.contains('"'), "{arg}");
                                assert!(!(arg.contains(' ') && arg.ends_with('\\')), "{arg}");
                            }
                            if plan.terminal == TerminalId::WindowsTerminal {
                                for arg in &plan.args {
                                    assert!(!arg.contains([';', '%']), "{arg}");
                                }
                            }
                            assert!(script(&plan).contains("-ErrorAction Stop; "));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 2 * 2 * 3 * 5 * (3 + 2 + 3));
}

mod clone;
mod profiles;
mod terminal;
