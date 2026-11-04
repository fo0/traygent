//! A repository start whose folder is missing: the terminal opens in the clone root and the
//! script clones before it enters the folder (docs/SPEC.md → Repositories).
use super::*;
use crate::models::RepoRef;

const GIT: &str = r"C:\Program Files\Git\cmd\git.exe";
const ROOT: &str = r"C:\Users\max\Traygent";
const FOLDER: &str = r"C:\Users\max\Traygent\fo0\traygent";
const DOCKER: &str = "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe";

fn repo_request(agent: AgentId, env: EnvId, folder: &str) -> LaunchRequest {
    LaunchRequest {
        repo: Some(RepoRef {
            owner: "fo0".into(),
            name: "traygent".into(),
        }),
        ..request(agent, env, folder)
    }
}

fn with_git() -> Resolved {
    Resolved {
        git: Some(GIT.into()),
        ..resolved(Some((CLAUDE, false)))
    }
}

fn statements(plan: &LaunchPlan) -> Vec<String> {
    script(plan).split("; ").map(str::to_string).collect()
}

#[test]
fn a_local_clone_runs_git_before_set_location_in_the_clone_root() {
    let req = repo_request(AgentId::Claude, EnvId::Local, FOLDER);
    let plan = clone_plan(&req, &AppSettings::default(), &with_git(), ROOT).unwrap();
    assert_eq!(plan.program, WT);
    assert_eq!(plan.cwd, ROOT);
    assert_eq!(plan.args[..3], ["new-tab", "-d", ROOT]);
    assert_eq!(plan.title, "\u{2733} traygent");
    let statements = statements(&plan);
    assert_eq!(
        statements[..4],
        [
            "$Host.UI.RawUI.WindowTitle='\u{2733} traygent'".to_string(),
            format!("& '{GIT}' 'clone' '--' 'https://github.com/fo0/traygent.git' '{FOLDER}'"),
            format!("if ($LASTEXITCODE -ne 0) {{ throw '{CLONE_FAILED}' }}"),
            format!("Set-Location -LiteralPath '{FOLDER}' -ErrorAction Stop"),
        ]
    );
    assert_eq!(statements[4], "$env:CLAUDE_CODE_DISABLE_TERMINAL_TITLE='1'");
    assert!(statements[7].starts_with(&format!("try {{ & '{CLAUDE}' ")));
    for arg in plan.args.iter().chain([&plan.program, &plan.cwd]) {
        assert!(!arg.is_empty() && !arg.contains('"'), "{arg}");
    }
}

#[test]
fn a_plain_plan_for_the_same_request_reuses_the_folder() {
    let req = repo_request(AgentId::Claude, EnvId::Local, FOLDER);
    let plan = plan(&req, &console(), &with_git()).unwrap();
    assert_eq!(plan.cwd, FOLDER);
    assert!(!script(&plan).contains("'clone'"));
    assert!(script(&plan).contains(&format!("Set-Location -LiteralPath '{FOLDER}'")));
}

#[test]
fn a_docker_clone_mounts_the_folder_after_cloning_on_the_host() {
    let req = repo_request(AgentId::Codex, EnvId::Docker, FOLDER);
    let plan = clone_plan(&req, &console(), &with_git(), ROOT).unwrap();
    assert_eq!((plan.program.as_str(), plan.cwd.as_str()), (PS, ROOT));
    let script = script(&plan);
    let clone = script.find("'clone'").unwrap();
    let enter = script.find("Set-Location").unwrap();
    let run = script.find(&format!("& '{DOCKER}' 'run'")).unwrap();
    assert!(clone < enter && enter < run, "{script}");
    assert!(script.contains(&format!("'-v' '{FOLDER}:/workspace/traygent-")));
}

#[test]
fn a_clone_needs_an_absolute_git_and_a_repository() {
    let req = repo_request(AgentId::Claude, EnvId::Local, FOLDER);
    let settings = AppSettings::default();
    let no_git = resolved(Some((CLAUDE, false)));
    assert_eq!(
        clone_plan(&req, &settings, &no_git, ROOT),
        Err("Git ist nicht installiert.".into())
    );
    let relative = Resolved {
        git: Some("git.exe".into()),
        ..no_git
    };
    assert_eq!(
        clone_plan(&req, &settings, &relative, ROOT),
        Err("Der Pfad zu Git ist kein absoluter Windows-Pfad.".into())
    );
    let plain = request(AgentId::Claude, EnvId::Local, FOLDER);
    assert_eq!(
        clone_plan(&plain, &settings, &with_git(), ROOT),
        Err("Kein Repository angegeben.".into())
    );
    let mut invalid = req.clone();
    invalid.repo.as_mut().unwrap().name = "a'b".into();
    assert_eq!(
        clone_plan(&invalid, &settings, &with_git(), ROOT),
        Err(crate::repo::INVALID_NAME.into())
    );
    assert_eq!(
        clone_plan(&req, &settings, &with_git(), "relative"),
        Err("Bitte einen absoluten Ordnerpfad wählen.".into())
    );
}

#[test]
fn quotes_and_separators_in_the_clone_root_stay_literal() {
    let root = r"C:\Max's Code; 100%";
    let folder = format!(r"{root}\fo0\traygent");
    let req = repo_request(AgentId::Claude, EnvId::Local, &folder);
    let plan = clone_plan(&req, &AppSettings::default(), &with_git(), root).unwrap();
    assert_eq!(plan.cwd, root);
    // wt splits at `;` and expands `%`: no `-d`, the process cwd applies.
    assert!(!plan.args.iter().any(|arg| arg == "-d"));
    let quoted = r"'C:\Max''s Code; 100%\fo0\traygent'";
    let script = script(&plan);
    assert!(script.contains(&format!(
        "'https://github.com/fo0/traygent.git' {quoted}; if ("
    )));
    assert!(script.contains(&format!(
        "Set-Location -LiteralPath {quoted} -ErrorAction Stop"
    )));
}
