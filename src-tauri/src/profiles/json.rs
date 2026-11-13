//! The CLI's state file (`.claude.json`): only the shared top-level keys travel between two
//! profiles, entry by entry. Every other key keeps its bytes and its place — identity,
//! onboarding state and caches belong to the profile.
use super::files::{
    bytes, load, problem, unchanged, writable, write_atomic, Backups, Change, CHANGED_MEANWHILE,
};
use super::{OwnFields, Recipe};
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

    fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(name, _)| name.as_str())
    }
}

/// What two state files share.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shared<'a> {
    /// Keys merged entry by entry.
    pub entries: &'a [&'a str],
    /// Keys that travel as one value: the winning file's, never united.
    pub values: &'a [&'a str],
    /// Fields of the entries of one key that stay on their side.
    pub own_fields: Option<&'a OwnFields>,
}

impl<'a> Shared<'a> {
    pub fn of(recipe: &'a Recipe) -> Self {
        Self {
            entries: recipe.shared_keys,
            values: recipe.shared_values,
            own_fields: recipe.own_fields.as_ref(),
        }
    }

    /// The side-owned fields of the entries of `key`, when it has any.
    fn own(&self, key: &str) -> Option<&'a OwnFields> {
        self.own_fields.filter(|own| own.key == key)
    }
}

/// The entry names of each shared key as of the rule's last successful sync — names only,
/// never a value. With it a sync tells an entry one side removed from one the other added.
pub type Base = std::collections::BTreeMap<String, Vec<String>>;

/// A shared key with more entries (or longer names in total) keeps no base and is united
/// without deletions, so the marker stays small.
pub const MAX_BASE_NAMES: usize = 2000;
const MAX_BASE_CHARS: usize = 256 * 1024;

fn object(raw: &RawValue) -> Option<Entries> {
    serde_json::from_str(raw.get()).ok()
}

/// The entry names both sides hold after the merge of one shared key: every name both
/// have, and every name only one has unless the base knows it (the other side removed it).
/// `None` when the values are no objects or outgrow the cap.
fn merged_names(
    ours: Option<&RawValue>,
    theirs: Option<&RawValue>,
    base: Option<&[String]>,
) -> Option<Vec<String>> {
    let (ours, theirs) = match (ours.map(object), theirs.map(object)) {
        (Some(ours), Some(theirs)) => (ours?, theirs?),
        (Some(only), None) | (None, Some(only)) => (only?, Entries::default()),
        (None, None) => return None,
    };
    let removed = |name: &str| base.is_some_and(|base| base.iter().any(|known| known == name));
    let mut names: Vec<String> = ours
        .names()
        .filter(|name| theirs.get(name).is_some() || !removed(name))
        .chain(
            theirs
                .names()
                .filter(|name| ours.get(name).is_none() && !removed(name)),
        )
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    let chars: usize = names.iter().map(String::len).sum();
    (names.len() <= MAX_BASE_NAMES && chars <= MAX_BASE_CHARS).then_some(names)
}

/// Compared as values: formatting alone never causes a write. Neither do the fields of an
/// entry that belong to its side (`own`).
fn same(first: &RawValue, second: &RawValue, own: Option<&OwnFields>) -> bool {
    let parsed = |raw: &RawValue| {
        let mut value = serde_json::from_str::<Value>(raw.get()).ok()?;
        if let (Some(own), Value::Object(fields)) = (own, &mut value) {
            fields.retain(|name, _| !own.holds(name));
        }
        Some(value)
    };
    parsed(first).is_some() && parsed(first) == parsed(second)
}

/// The entry a side holds after taking `other`'s: the other side's fields without its own
/// ones (`own`), this side's own ones kept — in this side's order, new fields appended.
/// Without such fields on either side the entry keeps the other side's text.
fn taken(
    mine: Option<&RawValue>,
    other: &RawValue,
    own: Option<&OwnFields>,
    layout: &Layout,
) -> Box<RawValue> {
    let (Some(own), Some(theirs)) = (own, object(other)) else {
        return other.to_owned();
    };
    let mine = mine.and_then(object).unwrap_or_default();
    if !mine
        .names()
        .chain(theirs.names())
        .any(|name| own.holds(name))
    {
        return other.to_owned();
    }
    let mut entry = Entries::default();
    for (name, value) in &mine.0 {
        if own.holds(name) {
            entry.set(name, value.clone());
        } else if let Some(value) = theirs.get(name) {
            entry.set(name, value.to_owned());
        }
    }
    for (name, value) in &theirs.0 {
        if !own.holds(name) && mine.get(name).is_none() {
            entry.set(name, value.clone());
        }
    }
    RawValue::from_string(layout.object(&entry, 2)).unwrap_or_else(|_| other.to_owned())
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

/// One side's value of a shared key after the merge with the other side's, and whether the
/// merge removed an entry from it; `None` when it stays as it is. Two objects are merged
/// entry by entry: an entry only the other side has is added — unless `base` knows it, then
/// this side removed it; an entry only this side has that `base` knows was removed on the
/// other side and goes; one that differs is taken from the other side when that one wins.
/// Anything else counts as one entry.
fn merged_value(
    side: Option<&RawValue>,
    other: Option<&RawValue>,
    other_wins: bool,
    base: Option<&[String]>,
    own: Option<&OwnFields>,
    layout: &Layout,
) -> Option<(Box<RawValue>, bool)> {
    let other = other?;
    // A key this side lacks is the other side's: nothing was removed here.
    if side.is_none() && own.is_none() {
        return Some((other.to_owned(), false));
    }
    let base = base.filter(|_| side.is_some());
    let mine = match side {
        Some(side) => object(side),
        None => Some(Entries::default()),
    };
    let (Some(mut mine), Some(theirs)) = (mine, object(other)) else {
        return whole_value(side, other, other_wins).map(|value| (value, false));
    };
    let known = |name: &str| base.is_some_and(|base| base.iter().any(|known| known == name));
    let mut changed = false;
    for (key, value) in &theirs.0 {
        let current = mine.get(key);
        let take = match current {
            None => !known(key),
            Some(current) => other_wins && !same(current, value, own),
        };
        if take {
            let entry = taken(current, value, own, layout);
            mine.set(key, entry);
            changed = true;
        }
    }
    let before = mine.0.len();
    mine.0
        .retain(|(name, _)| theirs.get(name).is_some() || !known(name));
    let removed = mine.0.len() != before;
    // A key this side lacks is created even when the other side's object is empty.
    let text = (changed || removed || side.is_none()).then(|| layout.object(&mine, 1))?;
    Some((RawValue::from_string(text).ok()?, removed))
}

/// One side's value of a key that travels as a whole: the other side's when this side has
/// none or the other side wins a difference; `None` when it stays as it is.
fn whole_value(
    side: Option<&RawValue>,
    other: &RawValue,
    other_wins: bool,
) -> Option<Box<RawValue>> {
    let differs = side.is_none_or(|side| other_wins && !same(side, other, None));
    differs.then(|| other.to_owned())
}

/// One side's file after the merge and whether entries were removed from it; `None` when
/// nothing changes for it.
fn merged_side(
    bytes: Option<&[u8]>,
    side: &Entries,
    other: &Entries,
    shared: &Shared,
    other_wins: bool,
    base: Option<&Base>,
) -> Option<(Vec<u8>, bool)> {
    let layout = Layout::of(bytes);
    let entries = shared.entries.iter().filter_map(|key| {
        let known = base.and_then(|base| base.get(*key)).map(Vec::as_slice);
        let (mine, theirs, own) = (side.get(key), other.get(key), shared.own(key));
        let (value, removed) = merged_value(mine, theirs, other_wins, known, own, &layout)?;
        Some((*key, value, removed))
    });
    let values = shared.values.iter().filter_map(|key| {
        let value = whole_value(side.get(key), other.get(key)?, other_wins)?;
        Some((*key, value, false))
    });
    let new: Vec<(&str, Box<RawValue>, bool)> = entries.chain(values).collect();
    if new.is_empty() {
        return None;
    }
    let mut merged = Entries(side.0.clone());
    let mut removed = false;
    for (key, value, gone) in new {
        merged.set(key, value);
        removed |= gone;
    }
    Some((layout.file(&merged), removed))
}

/// The base without the keys one side has reset: when the base of a key holds two or more
/// names and a side's object holds none of them, that file was recreated (or wiped), it did
/// not remove entry after entry — the key is united in this run, nothing is removed.
/// Removing the only entry, or some of several, stays a removal.
fn without_resets(base: &Base, ours: &Entries, theirs: &Entries) -> Base {
    let reset = |side: &Entries, key: &str, names: &[String]| {
        side.get(key)
            .and_then(object)
            .is_some_and(|held| names.iter().all(|name| held.get(name).is_none()))
    };
    base.iter()
        .filter(|(key, names)| {
            names.len() < 2 || !(reset(ours, key, names) || reset(theirs, key, names))
        })
        .map(|(key, names)| (key.clone(), names.clone()))
        .collect()
}

/// The result of [`merge_keys`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Merged {
    /// The new content of each side; `None` for a side that stays as it is.
    pub default: Option<Vec<u8>>,
    pub profile: Option<Vec<u8>>,
    /// The merge removed entries from that side.
    pub removed_default: bool,
    pub removed_profile: bool,
    /// The entry names per shared key both sides now hold: the next sync's base.
    pub base: Base,
}

/// Both files with the shared keys merged: the whole-value keys taken from the winning file
/// (a file without one never removes it), the others entry by entry. An entry only one file has reaches the
/// other; with a `base` (the names as of the last sync) an entry the base knows that one
/// file lacks was removed there and is removed from the other too — whatever the other did
/// to it since, the removal wins; a side that holds none of two or more base names is a
/// reset and removes nothing ([`without_resets`]). One that differs is taken from the default home's file
/// when `default_wins`, else from the profile's — apart from the fields that belong to each
/// side ([`Shared::own_fields`]), which neither travel nor count as a difference. Without a base (first sync, a marker from
/// before it) and while one file is missing nothing is ever removed. A missing file is
/// created with the shared keys only.
pub fn merge_keys(
    default: Option<&[u8]>,
    profile: Option<&[u8]>,
    shared: &Shared,
    default_wins: bool,
    base: Option<&Base>,
) -> Result<Merged, NotJson> {
    let parse = |bytes: Option<&[u8]>| match bytes {
        Some(bytes) => serde_json::from_slice::<Entries>(bytes).map_err(|_| NotJson),
        None => Ok(Entries::default()),
    };
    let (ours, theirs) = (parse(default)?, parse(profile)?);
    // A file that is not there has removed nothing.
    let base = base.filter(|_| default.is_some() && profile.is_some());
    let base = base.map(|base| without_resets(base, &ours, &theirs));
    let base = base.as_ref();
    let new_default = merged_side(default, &ours, &theirs, shared, !default_wins, base);
    let new_profile = merged_side(profile, &theirs, &ours, shared, default_wins, base);
    let mut names = Base::new();
    for key in shared.entries {
        let known = base.and_then(|base| base.get(*key)).map(Vec::as_slice);
        if let Some(list) = merged_names(ours.get(key), theirs.get(key), known) {
            names.insert(key.to_string(), list);
        }
    }
    let split = |side: Option<(Vec<u8>, bool)>| match side {
        Some((bytes, removed)) => (Some(bytes), removed),
        None => (None, false),
    };
    let ((default, removed_default), (profile, removed_profile)) =
        (split(new_default), split(new_profile));
    Ok(Merged {
        default,
        profile,
        removed_default,
        removed_profile,
        base: names,
    })
}

/// The shared keys of a pair's state files, united entry by entry. Until the rule's first
/// sync succeeded (`first`) the default home's entries win and each file that is rewritten
/// goes to its side's backup first; later the newer file's entries win. An unparsable file
/// on either side stops the rule; it is never overwritten. Neither is a file a session
/// rewrote since it was read — the next sync merges into the new content. `base` (ignored
/// on a first sync) lets a removal travel; a file that loses entries goes to its side's
/// backup first as well. Returns the base for the next sync.
pub fn sync_state(
    name: &str,
    default: &Path,
    profile: &Path,
    first: bool,
    shared: &Shared,
    base: Option<&Base>,
    backups: &mut Backups,
) -> Result<(Change, Base), String> {
    let fail = |what: &str| problem(name, what);
    let ours = load(default, MAX_STATE_BYTES).map_err(fail)?;
    let theirs = load(profile, MAX_STATE_BYTES).map_err(fail)?;
    let default_wins = match (&ours, &theirs) {
        (Some(ours), Some(theirs)) => first || ours.modified >= theirs.modified,
        _ => true,
    };
    let base = base.filter(|_| !first);
    let merged = merge_keys(bytes(&ours), bytes(&theirs), shared, default_wins, base)
        .map_err(|NotJson| fail(UNREADABLE))?;
    let (new_default, new_profile) = (merged.default, merged.profile);
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
    if let (Some(_), Some(seen), true) = (&new_default, &ours, first || merged.removed_default) {
        backups.default.keep(name, seen).map_err(fail)?;
    }
    if let (Some(_), Some(seen), true) = (&new_profile, &theirs, first || merged.removed_profile) {
        backups.profile.keep(name, seen).map_err(fail)?;
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
    Ok((change, merged.base))
}
