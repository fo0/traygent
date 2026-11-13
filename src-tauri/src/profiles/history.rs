//! The line-based history of a profile pair: both files end as the union of their lines.
use super::files::{load, problem, write_atomic, Change};
use std::collections::HashSet;
use std::path::Path;

/// A history beyond this is left alone.
pub const MAX_HISTORY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(serde::Deserialize)]
struct Stamp {
    timestamp: f64,
}

/// The line's numeric `timestamp`, when it is a JSON object with one.
fn stamp(line: &[u8]) -> Option<f64> {
    serde_json::from_slice::<Stamp>(line)
        .ok()
        .map(|stamp| stamp.timestamp)
}

/// The non-empty lines, each once, in file order, with the time they sort by: their own
/// `timestamp`, or the one of the line before them in the file — a line without one stays
/// behind its predecessor.
fn lines<'a>(bytes: &'a [u8], seen: &mut HashSet<&'a [u8]>) -> Vec<(&'a [u8], f64)> {
    let mut at = f64::NEG_INFINITY;
    let mut kept = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        at = stamp(line).unwrap_or(at);
        if seen.insert(line) {
            kept.push((line, at));
        }
    }
    kept
}

/// Every line of `first`, then the lines only `second` has, exact duplicates dropped. Both
/// lists keep their own order; a line of `second` goes before a line of `first` only when
/// its numeric `timestamp` is smaller, so a file that already is the union keeps its bytes.
pub fn union<'a>(first: &'a [u8], second: &'a [u8]) -> Vec<u8> {
    let mut seen = HashSet::new();
    let ours = lines(first, &mut seen);
    let theirs = lines(second, &mut seen);
    let mut out = Vec::with_capacity(first.len() + second.len());
    let mut push = |line: &[u8]| {
        out.extend_from_slice(line);
        out.push(b'\n');
    };
    let mut theirs = theirs.into_iter().peekable();
    for (line, at) in ours {
        while let Some((other, _)) = theirs.next_if(|(_, other)| *other < at) {
            push(other);
        }
        push(line);
    }
    for (other, _) in theirs {
        push(other);
    }
    out
}

/// Writes the union to each side whose content differs from it.
pub fn sync_history(name: &str, default: &Path, profile: &Path) -> Result<Change, String> {
    let fail = |what: &str| problem(name, what);
    let ours = load(default, MAX_HISTORY_BYTES).map_err(fail)?;
    let theirs = load(profile, MAX_HISTORY_BYTES).map_err(fail)?;
    if ours.is_none() && theirs.is_none() {
        return Ok(Change::default());
    }
    let bytes = |side: &Option<super::files::Loaded>| {
        side.as_ref()
            .map(|loaded| loaded.bytes.clone())
            .unwrap_or_default()
    };
    let (ours_bytes, theirs_bytes) = (bytes(&ours), bytes(&theirs));
    let merged = union(&ours_bytes, &theirs_bytes);
    let mut change = Change::default();
    // A missing side counts as empty, so an empty union creates no file.
    if ours_bytes != merged {
        write_atomic(default, &merged).map_err(fail)?;
        change.default = true;
    }
    if theirs_bytes != merged {
        write_atomic(profile, &merged).map_err(fail)?;
        change.profile = true;
    }
    Ok(change)
}
