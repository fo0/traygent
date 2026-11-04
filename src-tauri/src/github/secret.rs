//! The GitHub token in memory: never printed, serialized or cloned, overwritten on drop.
use std::fmt;

/// Longest token accepted from the user, gh or the vault. Classic and fine-grained personal
/// access tokens and gh's OAuth tokens are far shorter.
pub const MAX_TOKEN_BYTES: usize = 2048;

/// A deliberately non-serializable, non-cloneable secret. `Debug` never prints its contents;
/// the bytes are overwritten on drop. Callers must avoid intermediate copies they cannot wipe.
pub struct SecretToken(Vec<u8>);

impl SecretToken {
    pub fn new(value: String) -> Self {
        Self(value.into_bytes())
    }

    #[cfg(any(feature = "vault", test))]
    /// From vault bytes; `None` (after wiping them) unless they are a well-formed token.
    pub(crate) fn from_bytes(mut bytes: Vec<u8>) -> Option<Self> {
        if std::str::from_utf8(&bytes).is_ok_and(well_formed) {
            Some(Self(bytes))
        } else {
            wipe(&mut bytes);
            None
        }
    }

    pub fn with_secret<T>(&self, use_secret: impl FnOnce(&str) -> T) -> T {
        // Every constructor starts from a `String` or checked UTF-8.
        use_secret(std::str::from_utf8(&self.0).unwrap_or_default())
    }

    #[cfg(any(feature = "vault", test))]
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Non-empty, at most [`MAX_TOKEN_BYTES`], no whitespace or control characters.
    pub fn is_well_formed(&self) -> bool {
        self.with_secret(well_formed)
    }
}

fn well_formed(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= MAX_TOKEN_BYTES
        && !token
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}

impl Drop for SecretToken {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Overwrites a buffer that held secret bytes; `black_box` keeps the write observable.
pub(crate) fn wipe(bytes: &mut Vec<u8>) {
    bytes.fill(0);
    std::hint::black_box(bytes);
}

/// Wipes a `String` that held a secret (a pasted token, a parsed field).
pub(crate) fn wipe_string(value: &mut String) {
    let mut bytes = std::mem::take(value).into_bytes();
    wipe(&mut bytes);
}

/// Values shaped like a GitHub credential: never accepted as a login, URL value or name.
pub fn looks_like_credential(value: &str) -> bool {
    let value = value.trim();
    ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
        || value.contains("-----BEGIN")
        || value.to_ascii_lowercase().starts_with("bearer ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_secret_and_shape_checks_hold() {
        let token = SecretToken::new("synthetic-secret-value".into());
        assert_eq!(format!("{token:?}"), "SecretToken([REDACTED])");
        assert!(token.is_well_formed());
        token.with_secret(|value| assert_eq!(value, "synthetic-secret-value"));
        for bad in ["", "a b", "a\nb", "a\tb", "a\u{7f}b"] {
            assert!(!SecretToken::new(bad.into()).is_well_formed(), "{bad:?}");
        }
        assert!(!SecretToken::new("x".repeat(MAX_TOKEN_BYTES + 1)).is_well_formed());
        assert!(SecretToken::from_bytes(vec![0xff, 0xfe]).is_none());
        assert!(SecretToken::from_bytes(Vec::new()).is_none());
        assert!(SecretToken::from_bytes(b"synthetic".to_vec()).is_some());
    }

    #[test]
    fn credential_shapes_are_recognised() {
        for value in [
            "ghp_x",
            "github_pat_x",
            " gho_x",
            "Bearer x",
            "-----BEGIN KEY",
        ] {
            assert!(looks_like_credential(value), "{value}");
        }
        assert!(!looks_like_credential("octocat"));
    }
}
