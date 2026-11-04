//! Quoting for the PowerShell 5.1 script a terminal runs (docs/AGENT_RECIPES.md §0.3).
//!
//! Every value in the script is a single-quoted literal; JSON and shell scripts travel as
//! Base64 (alphabet without `' " ;`) and are decoded into env vars inside the script. The
//! whole script reaches PowerShell as `-EncodedCommand` ([`encode_command`]).

/// The characters PowerShell treats as a single quote; inside a literal each is doubled.
const SINGLE_QUOTES: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'];

/// A PowerShell single-quoted literal: no expansion, quotes doubled.
pub fn ps_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        if SINGLE_QUOTES.contains(&c) {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// `& '<program>' '<arg>' …` — the call operator with every part a literal.
pub fn invoke(program: &str, args: &[String]) -> String {
    let mut line = format!("& {}", ps_literal(program));
    for arg in args {
        line.push(' ');
        line.push_str(&ps_literal(arg));
    }
    line
}

/// Sets `$env:<name>` from Base64 inside the script, so the value never needs quoting.
/// `name` is always a constant identifier.
pub fn set_env_base64(name: &str, value: &str) -> String {
    format!(
        "$env:{name}=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}'))",
        base64(value.as_bytes())
    )
}

/// `-EncodedCommand`'s value: the script as UTF-16LE, then Base64. Its alphabet has no
/// `;`, `%`, quote or space, so neither wt.exe nor PowerShell's argument parser touches it.
pub fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard Base64 with padding (RFC 4648 §4), what `[Convert]::FromBase64String` reads.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}
