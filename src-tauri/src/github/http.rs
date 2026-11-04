//! The HTTPS [`Transport`]: blocking reqwest with rustls, fixed GitHub headers, no redirects,
//! timeouts and a capped body.
use super::api::{
    ApiUrl, Response, Transport, API_VERSION, INVALID_RESPONSE, KEPT_HEADERS, MAX_BODY_BYTES,
    UNREACHABLE,
};
use super::secret::SecretToken;
use std::io::Read;
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 4096;

pub struct HttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Traygent/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .build()
            .map_err(|_| UNREACHABLE)?;
        Ok(Self { client })
    }
}

impl Transport for HttpTransport {
    fn get(&self, url: &ApiUrl, token: &SecretToken) -> Result<Response, String> {
        // Re-checked here so no caller can point the token at another host.
        let url = ApiUrl::parse(url.as_str()).ok_or(INVALID_RESPONSE)?;
        let request = self
            .client
            .get(url.as_str())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION);
        // `bearer_auth` marks the header sensitive.
        let response = token
            .with_secret(|secret| request.bearer_auth(secret))
            .send()
            .map_err(|_| UNREACHABLE)?;
        let status = response.status().as_u16();
        let headers = KEPT_HEADERS
            .iter()
            .filter_map(|name| {
                let value = response.headers().get(*name)?.to_str().ok()?;
                (value.len() <= MAX_HEADER_BYTES).then(|| (name.to_string(), value.to_string()))
            })
            .collect();
        let mut body = Vec::new();
        response
            .take(MAX_BODY_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| UNREACHABLE)?;
        if body.len() > MAX_BODY_BYTES {
            return Err(INVALID_RESPONSE.into());
        }
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}
