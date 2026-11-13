//! The CLI's state file (`.claude.json`): only the shared top-level keys travel between two
//! profiles. Every other key keeps its bytes and its place — identity, onboarding state and
//! caches belong to the profile.
use super::files::{load, problem, write_atomic, Change};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::Value;
use std::path::Path;

/// A state file beyond this is left alone.
pub const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;
const UNREADABLE: &str = "ist kein lesbares JSON und bleibt unverändert";
const CHANGED_MEANWHILE: &str = "wurde gerade geändert, nächster Versuch beim nächsten Abgleich";

/// One of the two files is no JSON object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotJson;

/// The top-level object in file order, every value as the text it was written in.
struct Entries(Vec<(String, Box<RawValue>)>);

impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Top;
        impl<'de> Visitor<'de> for Top {
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
        deserializer.deserialize_map(Top)
    }
}

impl Entries {
    /// A duplicate key reads as its last occurrence, as JSON parsers do.
    fn get(&self, key: &str) -> Option<&RawValue> {
        let found = self.0.iter().rev().find(|(name, _)| name == key);
        found.map(|(_, value)| value.as_ref())
    }

    fn set(&mut self, key: &str, value: &RawValue) {
        match self.0.iter_mut().rev().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = value.to_owned(),
            None => self.0.push((key.to_string(), value.to_owned())),
        }
    }
}

fn parse(bytes: &[u8]) -> Option<Entries> {
    serde_json::from_slice(bytes).ok()
}

/// How the file indents its top-level keys (`None`: on one line) and whether it ends with a
/// line break; a new file is written the way the CLI writes its own.
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

    fn render(&self, entries: &Entries) -> Vec<u8> {
        let (open, between, close) = match &self.indent {
            _ if entries.0.is_empty() => (String::new(), String::new(), String::new()),
            Some(indent) => (
                format!("{}{indent}", self.newline),
                format!(",{}{indent}", self.newline),
                self.newline.to_string(),
            ),
            None => (String::new(), ",".into(), String::new()),
        };
        let colon = if self.indent.is_some() { ": " } else { ":" };
        let mut out = String::from("{");
        for (index, (key, value)) in entries.0.iter().enumerate() {
            out.push_str(if index == 0 { &open } else { &between });
            out.push_str(&Value::from(key.as_str()).to_string());
            out.push_str(colon);
            out.push_str(value.get());
        }
        out.push_str(&close);
        out.push('}');
        if self.trailing {
            out.push_str(self.newline);
        }
        out.into_bytes()
    }
}

/// `target` with the `keys` of `source`; `None` when it already has them. A key the source
/// lacks stays as the target has it.
pub fn merge_keys(
    source: &[u8],
    target: Option<&[u8]>,
    keys: &[&str],
) -> Result<Option<Vec<u8>>, NotJson> {
    let from = parse(source).ok_or(NotJson)?;
    let mut into = match target {
        Some(bytes) => parse(bytes).ok_or(NotJson)?,
        None => Entries(Vec::new()),
    };
    let mut changed = false;
    for key in keys {
        let Some(value) = from.get(key) else {
            continue;
        };
        let same = into.get(key).is_some_and(|current| {
            // Compared as values: formatting alone never causes a write.
            let parsed = |raw: &RawValue| serde_json::from_str::<Value>(raw.get()).ok();
            parsed(current).is_some() && parsed(current) == parsed(value)
        });
        if !same {
            into.set(key, value);
            changed = true;
        }
    }
    Ok(changed.then(|| Layout::of(target).render(&into)))
}

/// The shared keys of a pair's state files, taken from the newer file (the default home's on
/// the first sync, and on a tie) and written into the other. An unparsable file on either
/// side stops the rule; it is never overwritten.
pub fn sync_state(
    name: &str,
    default: &Path,
    profile: &Path,
    first: bool,
    keys: &[&str],
) -> Result<Change, String> {
    let fail = |what: &str| problem(name, what);
    let ours = load(default, MAX_STATE_BYTES).map_err(fail)?;
    let theirs = load(profile, MAX_STATE_BYTES).map_err(fail)?;
    let from_default = match (&ours, &theirs) {
        (None, None) => return Ok(Change::default()),
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (Some(ours), Some(theirs)) => first || ours.modified >= theirs.modified,
    };
    let (source, target, target_path) = if from_default {
        (ours, theirs, profile)
    } else {
        (theirs, ours, default)
    };
    let Some(source) = source else {
        return Ok(Change::default());
    };
    let current = target.as_ref().map(|loaded| loaded.bytes.as_slice());
    let Some(merged) =
        merge_keys(&source.bytes, current, keys).map_err(|NotJson| fail(UNREADABLE))?
    else {
        return Ok(Change::default());
    };
    // The CLI rewrites this file while it runs: a change since the read wins, the next sync
    // merges into the new content.
    let now = load(target_path, MAX_STATE_BYTES).map_err(fail)?;
    let unchanged = match (&target, &now) {
        (None, None) => true,
        (Some(before), Some(now)) => before.modified == now.modified && before.bytes == now.bytes,
        _ => false,
    };
    if !unchanged {
        return Err(fail(CHANGED_MEANWHILE));
    }
    write_atomic(target_path, &merged).map_err(fail)?;
    Ok(Change {
        default: !from_default,
        profile: from_default,
    })
}
