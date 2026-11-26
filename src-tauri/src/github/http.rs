//! The HTTPS [`Transport`]: blocking reqwest with rustls, fixed GitHub headers, no redirects,
//! timeouts and a capped body.
use super::api::{
    ApiUrl, Response, Transport, API_VERSION, INVALID_RESPONSE, KEPT_HEADERS, MAX_BODY_BYTES,
    UNREACHABLE,
};
use super::secret::SecretToken;
use std::io::{self, Read};
use std::time::{Duration, Instant};

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
        let body = read_within(response, MAX_BODY_BYTES + 1, Duration::from_secs(30))
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

/// Reads at most `max` bytes of `body` and stops once `budget` of wall-clock time has passed:
/// reqwest's blocking timeout bounds each read only, so a body that drips would hold the
/// caller indefinitely. The last read may still take up to that per-read timeout.
pub(crate) fn read_within(body: impl Read, max: usize, budget: Duration) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + budget;
    let (mut body, mut bytes, mut chunk) = (body.take(max as u64), Vec::new(), [0; 16 * 1024]);
    loop {
        match body.read(&mut chunk) {
            Ok(0) => return Ok(bytes),
            Ok(read) => bytes.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_within_caps_the_body_and_stops_at_the_deadline() {
        let long = Duration::from_secs(60);
        assert_eq!(read_within(&b"abcdef"[..], 4, long).unwrap(), b"abcd");
        assert_eq!(read_within(&b""[..], 4, Duration::ZERO).unwrap(), b"");
        let late = read_within(&b"abc"[..], 4, Duration::ZERO).unwrap_err();
        assert_eq!(late.kind(), io::ErrorKind::TimedOut);
    }
}
