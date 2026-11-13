//! The CLI's state file (`.claude.json`): only the shared top-level keys travel between two
//! profiles, entry by entry. Every other key keeps its bytes and its place — identity,
//! onboarding state and caches belong to the profile.
use super::files::{
    bytes, load, problem, unchanged, writable, write_atomic, Backups, Change, CHANGED_MEANWHILE,
};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::Value;
use std::path::Path;

/// A state file beyond this is left alone.
pub const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;
const UNREADABLE: &str = "ist kein lesbares JSON und bleibt unverändert";

/// One of the two files is no JSON object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotJson;

/// A JSON object in file order, every value as the text it was written in.
#[derive(Default)]
struct Entries(Vec<(String, Box<RawValue>)>);

impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = Entries;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Entries, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, Box<RawValue>>()? {
                    entries.push(entry);
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Object)
    }
}

impl Entries {
    /// A duplicate key reads as its last occurrence, as JSON parsers do.
    fn get(&self, key: &str) -> Option<&RawValue> {
        let found = self.0.iter().rev().find(|(name, _)| name == key);
        found.map(|(_, value)| value.as_ref())
    }

    fn set(&mut self, key: &str, value: Box<RawValue>) {
        match self.0.iter_mut().rev().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key.to_string(), value)),
        }
    }
}

/// Compared as values: formatting alone never causes a write.
fn same(first: &RawValue, second: &RawValue) -> bool {
    let parsed = |raw: &RawValue| serde_json::from_str::<Value>(raw.get()).ok();
    parsed(first).is_some() && parsed(first) == parsed(second)
}

/// How the file indents (`None`: everything on one line) and whether it ends with a line
/// break; a new file is written the way the CLI writes its own.
struct Layout {
    indent: Option<String>,
    newline: &'static str,
    trailing: bool,
}

impl Layout {
    fn of(text: Option<&[u8]>) -> Self {
        let Some(text) = text.map(String::from_utf8_lossy) else {
            return Self {
                indent: Some("  ".into()),
                newline: "\n",
                trailing: false,
            };
        };
        let body = text.trim_start().strip_prefix('{').unwrap_or("");
        let newline = if body.starts_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let indent = body.strip_prefix(newline).map(|rest| {
            let blank = |c: &char| *c == ' ' || *c == '\t';
            rest.chars().take_while(blank).collect()
        });
        Self {
            indent,
            newline,
            trailing: text.ends_with('\n'),
        }
    }

    /// The object `entries` as text, `depth` levels inside the file.
    fn object(&self, entries: &Entries, depth: usize) -> String {
        let Some(indent) = self.indent.as_ref().filter(|_| !entries.0.is_empty()) else {
            let list: Vec<String> = entries
                .0
                .iter()
                .map(|(key, value)| format!("{}:{}", Value::from(key.as_str()), value.get()))
                .collect();
            return format!("{{{}}}", list.join(","));
        };
        let (inner, outer) = (indent.repeat(depth + 1), indent.repeat(depth));
        let list: Vec<String> = entries
            .0
            .iter()
            .map(|(key, value)| format!("{inner}{}: {}", Value::from(key.as_str()), value.get()))
            .collect();
        let between = format!(",{}", self.newline);
        format!(
            "{{{nl}{}{nl}{outer}}}",
            list.join(&between),
            nl = self.newline
        )
    }

    fn file(&self, entries: &Entries) -> Vec<u8> {
        let mut text = self.object(entries, 0);
        if self.trailing {
            text.push_str(self.newline);
        }
        text.into_bytes()
    }
}

/// One side's value of a shared key after the merge with the other side's; `None` when it
/// stays as it is. Two objects are united entry by entry: an entry only the other side has
/// is added, one that differs is taken from the other side when that one wins. Anything
/// else counts as one entry.
fn merged_value(
    side: Option<&RawValue>,
    other: Option<&RawValue>,
    other_wins: bool,
    layout: &Layout,
) -> Option<Box<RawValue>> {
    let other = other?;
    let Some(side) = side else {
        return Some(other.to_owned());
    };
    let object = |raw: &RawValue| serde_json::from_str::<Entries>(raw.get()).ok();
    let (Some(mut mine), Some(theirs)) = (object(side), object(other)) else {
        return (other_wins && !same(side, other)).then(|| other.to_owned());
    };
    let mut changed = false;
    for (key, value) in &theirs.0 {
        let take = match mine.get(key) {
            None => true,
            Some(current) => other_wins && !same(current, value),
        };
        if take {
            mine.set(key, value.clone());
            changed = true;
        }
    }
    let text = changed.then(|| layout.object(&mine, 1))?;
    RawValue::from_string(text).ok()
}

/// One side's file after the merge; `None` when nothing changes for it.
fn merged_side(
    bytes: Option<&[u8]>,
    side: &Entries,
    other: &Entries,
    keys: &[&str],
    other_wins: bool,
) -> Option<Vec<u8>> {
    let layout = Layout::of(bytes);
    let new: Vec<(&str, Box<RawValue>)> = keys
        .iter()
        .filter_map(|key| {
            let value = merged_value(side.get(key), other.get(key), other_wins, &layout)?;
            Some((*key, value))
        })
        .collect();
    if new.is_empty() {
        return None;
    }
    let mut merged = Entries(side.0.clone());
    for (key, value) in new {
        merged.set(key, value);
    }
    Some(layout.file(&merged))
}

/// The new (default, profile) content; `None` for a side that stays as it is.
pub type Merged = (Option<Vec<u8>>, Option<Vec<u8>>);

/// Both files with the `keys` united entry by entry: an entry only one file has reaches the
/// other, one that differs is taken from the default home's file when `default_wins`, else
/// from the profile's. Returns the new (default, profile) content, `None` for a side that
/// stays as it is. A missing file is created with the shared keys only.
pub fn merge_keys(
    default: Option<&[u8]>,
    profile: Option<&[u8]>,
    keys: &[&str],
    default_wins: bool,
) -> Result<Merged, NotJson> {
    let parse = |bytes: Option<&[u8]>| match bytes {
        Some(bytes) => serde_json::from_slice::<Entries>(bytes).map_err(|_| NotJson),
        None => Ok(Entries::default()),
    };
    let (ours, theirs) = (parse(default)?, parse(profile)?);
    Ok((
        merged_side(default, &ours, &theirs, keys, !default_wins),
        merged_side(profile, &theirs, &ours, keys, default_wins),
    ))
}

/// The shared keys of a pair's state files, united entry by entry. Until the rule's first
/// sync succeeded (`first`) the default home's entries win and each file that is rewritten
/// goes to its side's backup first; later the newer file's entries win. An unparsable file
/// on either side stops the rule; it is never overwritten. Neither is a file a session
/// rewrote since it was read — the next sync merges into the new content.
pub fn sync_state(
    name: &str,
    default: &Path,
    profile: &Path,
    first: bool,
    keys: &[&str],
    backups: &mut Backups,
) -> Result<Change, String> {
    let fail = |what: &str| problem(name, what);
    let ours = load(default, MAX_STATE_BYTES).map_err(fail)?;
    let theirs = load(profile, MAX_STATE_BYTES).map_err(fail)?;
    let default_wins = match (&ours, &theirs) {
        (Some(ours), Some(theirs)) => first || ours.modified >= theirs.modified,
        _ => true,
    };
    let (new_default, new_profile) = merge_keys(bytes(&ours), bytes(&theirs), keys, default_wins)
        .map_err(|NotJson| fail(UNREADABLE))?;
    // Everything is checked before the first write, so a refusal leaves both files alone.
    let sides = [
        (default, &ours, &new_default),
        (profile, &theirs, &new_profile),
    ];
    for (path, seen, new) in sides {
        if new.is_some() {
            writable(path).map_err(fail)?;
            if !unchanged(path, seen.as_ref()) {
                return Err(fail(CHANGED_MEANWHILE));
            }
        }
    }
    if first {
        if let (Some(_), Some(seen)) = (&new_default, &ours) {
            backups.default.keep(name, seen).map_err(fail)?;
        }
        if let (Some(_), Some(seen)) = (&new_profile, &theirs) {
            backups.profile.keep(name, seen).map_err(fail)?;
        }
    }
    let mut change = Change::default();
    if let Some(bytes) = &new_default {
        write_atomic(default, bytes).map_err(fail)?;
        change.default = true;
    }
    if let Some(bytes) = &new_profile {
        write_atomic(profile, bytes).map_err(fail)?;
        change.profile = true;
    }
    Ok(change)
}
