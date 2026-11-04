//! Test fakes: a scripted GitHub API and a gh CLI modelled per version.
use super::api::{ApiUrl, Response, Transport, UNREACHABLE};
use super::gh::GhCli;
use super::process::{Output, Runner, GH_FAILED};
use super::secret::SecretToken;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// gh's JSON status with every case: usable, unverified scopes, failed, environment token,
/// an invalid login and another host.
pub(crate) const STATUS: &str = r#"{"hosts":{
  "github.com":[
    {"host":"github.com","login":"fixture-alex","state":"success","active":true,"tokenSource":"keyring","scopes":"repo, read:org"},
    {"host":"github.com","login":"fixture-sam","state":"success","active":false,"tokenSource":"C:/Users/token-user/AppData/Roaming/GitHub CLI/hosts.yml"},
    {"host":"github.com","login":"fixture-expired","state":"error","tokenSource":"keyring","error":"untrusted diagnostic must never be displayed"},
    {"host":"github.com","login":"fixture-ci","state":"success","tokenSource":"GH_TOKEN"},
    {"host":"github.com","login":"--malicious","state":"success","tokenSource":"keyring"},
    {"host":"github.com","login":"fixture-alex","state":"success","tokenSource":"keyring"}
  ],
  "enterprise.example":[{"host":"enterprise.example","login":"fixture-enterprise","state":"success"}]
}}"#;

/// One gh version: which flags its help lists. An unlisted flag fails like real gh does.
pub(crate) struct FakeGh {
    pub json: bool,
    pub no_refresh: bool,
    pub user_flag: bool,
    pub status: String,
    pub status_success: bool,
    pub stderr: &'static str,
    /// The legacy `gh api user` answer.
    pub legacy_login: &'static str,
    pub token: &'static str,
    pub calls: Calls,
}

pub(crate) type Calls = Arc<Mutex<Vec<Vec<String>>>>;

impl Default for FakeGh {
    fn default() -> Self {
        Self {
            json: true,
            no_refresh: true,
            user_flag: true,
            status: STATUS.into(),
            status_success: true,
            stderr: "",
            legacy_login: "fixture-sam\n",
            token: "synthetic-gh-token\r\n",
            calls: Calls::default(),
        }
    }
}

fn output(success: bool, stdout: &[u8], stderr: &[u8]) -> Result<Output, String> {
    Ok(Output {
        success,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    })
}

/// An absolute path on the host the tests run on (`Path::is_absolute` differs per OS).
const FAKE_GH: &str = if cfg!(windows) {
    r"C:\fixtures\gh.exe"
} else {
    "/fixtures/gh.exe"
};

impl Runner for FakeGh {
    fn run(&self, program: &Path, args: &[&str]) -> Result<Output, String> {
        assert!(program.is_absolute());
        let call: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        self.calls.lock().unwrap().push(call);
        let known = |flag: &str| match flag {
            "--json" => self.json,
            "--no-refresh" => self.no_refresh,
            "--user" => self.user_flag,
            _ => true,
        };
        if let Some(flag) = args.iter().find(|arg| arg.starts_with("--") && !known(arg)) {
            return output(false, b"", format!("unknown flag: {flag}").as_bytes());
        }
        let mut help = String::from("--hostname --show-token");
        for flag in ["--json", "--no-refresh", "--user"] {
            if known(flag) {
                help.push(' ');
                help.push_str(flag);
            }
        }
        match args {
            ["auth", _, "--help"] => output(true, help.as_bytes(), b""),
            ["auth", "status", ..] => output(
                self.status_success,
                self.status.as_bytes(),
                self.stderr.as_bytes(),
            ),
            ["auth", "token", ..] => output(true, self.token.as_bytes(), b""),
            ["api", "--hostname", "github.com", "user", "--jq", ".login"] => {
                let success = !self.legacy_login.is_empty();
                output(
                    success,
                    self.legacy_login.as_bytes(),
                    self.stderr.as_bytes(),
                )
            }
            _ => Err(GH_FAILED.into()),
        }
    }
}

impl FakeGh {
    /// The CLI around this fake plus a handle on the calls it records.
    pub(crate) fn cli(self) -> (GhCli, Calls) {
        let calls = self.calls.clone();
        let cli = GhCli {
            program: Some(FAKE_GH.into()),
            runner: Box::new(self),
        };
        (cli, calls)
    }
}

/// Status, headers and body of one scripted answer.
type Scripted = (u16, Vec<(String, String)>, Vec<u8>);

/// A scripted API: one response per URL, every request recorded with its token.
#[derive(Default)]
pub(crate) struct FakeApi {
    pub responses: Mutex<BTreeMap<String, Scripted>>,
    pub requests: Mutex<Vec<(String, String)>>,
    pub offline: bool,
}

impl FakeApi {
    pub(crate) fn on(self, url: &str, status: u16, link: Option<&str>, body: &str) -> Self {
        let headers = link
            .map(|link| vec![("link".to_string(), link.to_string())])
            .unwrap_or_default();
        self.respond(url, status, headers, body)
    }

    pub(crate) fn respond(
        self,
        url: &str,
        status: u16,
        headers: Vec<(String, String)>,
        body: &str,
    ) -> Self {
        let entry = (status, headers, body.as_bytes().to_vec());
        self.responses.lock().unwrap().insert(url.into(), entry);
        self
    }

    pub(crate) fn user(self, id: u64, login: &str) -> Self {
        let body = format!(r#"{{"id":{id},"login":"{login}","name":"Synthetic"}}"#);
        self.on(super::api::USER_URL, 200, None, &body)
    }

    pub(crate) fn urls(&self) -> Vec<String> {
        let requests = self.requests.lock().unwrap();
        requests.iter().map(|(url, _)| url.clone()).collect()
    }
}

impl Transport for FakeApi {
    fn get(&self, url: &ApiUrl, token: &SecretToken) -> Result<Response, String> {
        let secret = token.with_secret(str::to_owned);
        let request = (url.as_str().to_string(), secret);
        self.requests.lock().unwrap().push(request);
        if self.offline {
            return Err(UNREACHABLE.into());
        }
        let responses = self.responses.lock().unwrap();
        let (status, headers, body) =
            responses
                .get(url.as_str())
                .cloned()
                .unwrap_or((404, Vec::new(), b"{}".to_vec()));
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}
