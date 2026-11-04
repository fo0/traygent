//! Folder paths a session may start in: absolute Windows paths, normalized, without the
//! device, stream and reserved-name forms Windows resolves to something else.

const MAX_FOLDER_CHARS: usize = 1024;
const NOT_ABSOLUTE: &str = "Bitte einen absoluten Ordnerpfad wählen.";
const DEVICE_PATH: &str = r"Geräte-Pfade wie \\?\ oder \\.\ werden nicht unterstützt.";
const INVALID_CHARS: &str = "Der Ordnerpfad enthält unzulässige Zeichen.";
const RESERVED_NAME: &str =
    "Der Ordnerpfad enthält einen reservierten Gerätenamen (z. B. CON, NUL).";
const DOT_SEGMENT: &str = "Der Ordnerpfad darf keine Abschnitte „.“ oder „..“ enthalten.";
const TRAILING_DOT_OR_SPACE: &str =
    "Ordnernamen im Pfad dürfen nicht auf einen Punkt oder ein Leerzeichen enden.";

/// `C:\…` / `C:/…` or a UNC path; what the CLI, docker.exe and gh.exe must resolve to.
pub(crate) fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    let unc = path.len() > 2 && (path.starts_with(r"\\") || path.starts_with("//"));
    drive || unc
}

/// An absolute Windows folder without control chars, `\` separators, one `\` between
/// segments and no trailing `\` (a drive root keeps its `C:\`). Device (`\\?\`, `\\.\`)
/// and alternate-stream forms, the characters Windows forbids in names, reserved device
/// names, `.`/`..` segments and segments ending in `.` or a space are refused: Windows
/// rewrites the last two silently, so `is_dir`, `Set-Location`, docker `-v` and the recent
/// id would see different strings. A plain UNC path is allowed (Docker refuses it on its
/// own). Existence is checked by the caller.
pub fn normalize_folder(folder: &str) -> Result<String, String> {
    let folder = folder.trim();
    if folder.chars().any(char::is_control) {
        return Err("Der Ordnerpfad enthält Steuerzeichen.".into());
    }
    if folder.chars().count() > MAX_FOLDER_CHARS || !is_windows_absolute(folder) {
        return Err(NOT_ABSOLUTE.into());
    }
    let folder = folder.replace('/', "\\");
    if folder.starts_with(r"\\?\") || folder.starts_with(r"\\.\") {
        return Err(DEVICE_PATH.into());
    }
    // The drive colon is the only `:`; any other one names an alternate data stream.
    let after_drive = if folder.as_bytes()[1] == b':' {
        &folder[2..]
    } else {
        folder.as_str()
    };
    if after_drive.contains(':') || folder.contains(['<', '>', '"', '|', '?', '*']) {
        return Err(INVALID_CHARS.into());
    }
    // Empty segments drop out: repeated `\` collapse, a trailing `\` (which breaks CRT
    // parsing of a quoted native arg, `"C:\a b\"`) goes.
    let (prefix, rest) = folder.split_at(if folder.starts_with(r"\\") { 2 } else { 3 });
    let segments: Vec<&str> = rest.split('\\').filter(|s| !s.is_empty()).collect();
    if prefix == r"\\" && (segments.is_empty() || rest.starts_with('\\')) {
        return Err(NOT_ABSOLUTE.into());
    }
    if segments.iter().any(|segment| is_reserved_name(segment)) {
        return Err(RESERVED_NAME.into());
    }
    if segments
        .iter()
        .any(|segment| matches!(*segment, "." | ".."))
    {
        return Err(DOT_SEGMENT.into());
    }
    if segments.iter().any(|segment| segment.ends_with(['.', ' '])) {
        return Err(TRAILING_DOT_OR_SPACE.into());
    }
    Ok(format!("{prefix}{}", segments.join("\\")))
}

/// `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `COM0`–`COM9`, `LPT0`–`LPT9` (and the
/// superscript `¹²³` forms), case-insensitive, with any extension and trailing spaces.
fn is_reserved_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment);
    let stem = stem.trim_end_matches(' ').to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str()) {
        return true;
    }
    let Some(number) = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
    else {
        return false;
    };
    let mut chars = number.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('0'..='9' | '¹' | '²' | '³'), None)
    )
}

pub fn is_network_path(folder: &str) -> bool {
    folder.starts_with(r"\\")
}

#[cfg(test)]
mod tests;
