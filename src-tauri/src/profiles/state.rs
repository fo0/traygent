//! The CLI's state file (`.claude.json`) of a pair: only a subset is shared — the keys
//! merged entry by entry (`mcpServers`, `projects`) and the keys shared as one value — and
//! inside a project entry the runtime fields stay with their profile. That subset is merged
//! three ways against its state at the last sync ([`super::pair`]), field by field. Every
//! other key keeps its bytes and its place: identity, onboarding state and caches belong
//! to the profile and are never read into the base.
use super::files::{bytes, load, problem, without_bom, Backups};
use super::json::{object, Base, Entries as RawEntries, Layout, Shared};
use super::merge::{loses, merge_pair, reorder};
use super::node::{get, index, Entries, Node};
use super::pair::{commit, read_base, sides_read, Out, Synced};
use serde_json::value::RawValue;
use std::path::Path;

/// A state file beyond this is left alone.
pub const MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;
const UNREADABLE: &str = "ist kein lesbares JSON und bleibt unverändert";

/// One of the two files is no JSON object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotJson;

/// What is known about the pair's last sync.
#[derive(Clone, Copy, Debug)]
pub enum Known<'a> {
    Nothing,
    /// The shared subset as of the last sync.
    Values(&'a [(String, Node)]),
    /// Only the entry names, from a marker written before the value base existed: enough
    /// to tell a removed entry from an added one, once.
    Names(&'a Base),
    /// No base of its own, but another profile's ([`super::merge::Merge::seeded`]).
    Seed(&'a [(String, Node)]),
}

/// One state file: its text, and the shared subset as values.
struct Side<'a> {
    bytes: Option<&'a [u8]>,
    doc: RawEntries,
    /// The subset as the file holds it.
    full: Entries,
    /// The subset without the fields that belong to this side.
    shared: Entries,
    /// Keys merged entry by entry that hold something else here (`"projects": null`); they
    /// are in neither subset.
    odd: Vec<&'a str>,
}

/// `value` without the side-owned fields of its entries.
fn stripped(key: &str, value: &Node, shared: &Shared) -> Node {
    let (Some(own), Node::Object(entries)) = (shared.own(key), value) else {
        return value.clone();
    };
    let strip = |entry: &Node| match entry {
        Node::Object(fields) => {
            let kept = fields.iter().filter(|(name, _)| !own.holds(name));
            Node::Object(kept.cloned().collect())
        }
        other => other.clone(),
    };
    let entries = entries
        .iter()
        .map(|(name, entry)| (name.clone(), strip(entry)));
    Node::Object(entries.collect())
}

impl<'a> Side<'a> {
    fn read(bytes: Option<&'a [u8]>, shared: &Shared<'a>) -> Result<Self, NotJson> {
        let doc = match bytes {
            Some(bytes) => serde_json::from_slice(without_bom(bytes)).map_err(|_| NotJson)?,
            None => RawEntries::default(),
        };
        let mut side = Self {
            bytes,
            doc,
            full: Entries::new(),
            shared: Entries::new(),
            odd: Vec::new(),
        };
        for (key, entries) in keys(shared) {
            let Some(raw) = side.doc.get(key) else {
                continue;
            };
            let value = Node::from_raw(raw).ok_or(NotJson)?;
            if entries && value.as_object().is_none() {
                side.odd.push(key);
                continue;
            }
            side.shared
                .push((key.to_string(), stripped(key, &value, shared)));
            side.full.push((key.to_string(), value));
        }
        Ok(side)
    }

    /// What this side holds of `key` after the merge: the merged value plus the fields of
    /// its entries that belong to this side, in this side's key order.
    fn wanted(&self, key: &str, merged: &Node, shared: &Shared) -> Node {
        let have = get(&self.full, key);
        let mine = have.and_then(Node::as_object).map(|mine| index(mine));
        let value = match (shared.own(key), merged, &mine) {
            (Some(own), Node::Object(entries), Some(mine)) => {
                let with_own = |(name, entry): &(String, Node)| {
                    let kept = match (entry, mine.get(name.as_str())) {
                        (Node::Object(fields), Some(Node::Object(my_fields))) => {
                            let own = my_fields.iter().filter(|(field, _)| own.holds(field));
                            Node::Object(fields.iter().chain(own).cloned().collect())
                        }
                        _ => entry.clone(),
                    };
                    (name.clone(), kept)
                };
                Node::Object(entries.iter().map(with_own).collect())
            }
            _ => merged.clone(),
        };
        reorder(value, have)
    }

    /// The text of `key`'s new value. Only what changed is written anew, in the file's own
    /// layout: an entry that stays as it is keeps its bytes.
    fn raw(&self, key: &str, want: &Node, entries: bool, layout: &Layout) -> Option<Box<RawValue>> {
        let style = layout.style();
        let held = get(&self.full, key).and_then(Node::as_object);
        let text = match (want, held, self.doc.get(key).and_then(object)) {
            (Node::Object(wanted), Some(held), Some(raw)) if entries => {
                let held = index(held);
                let mut rebuilt = RawEntries::default();
                for (name, entry) in wanted {
                    let same = held.get(name.as_str()).is_some_and(|held| *held == entry);
                    let text = match raw.get(name).filter(|_| same) {
                        Some(raw) => raw.to_owned(),
                        None => RawValue::from_string(entry.text(&style, 2)).ok()?,
                    };
                    rebuilt.set(name, text);
                }
                layout.object(&rebuilt, 1)
            }
            _ => want.text(&style, 1),
        };
        RawValue::from_string(text).ok()
    }

    /// The file after the merge; `None` when nothing changes for it. A missing file is
    /// created with the shared subset only.
    fn rebuilt(&self, merged: &[(String, Node)], shared: &Shared) -> Option<Vec<u8>> {
        let layout = Layout::of(self.bytes);
        let mut doc = RawEntries(self.doc.0.clone());
        let mut changed = false;
        for (key, entries) in keys(shared) {
            let have = get(&self.full, key);
            let want = get(merged, key).map(|value| self.wanted(key, value, shared));
            match want {
                None if have.is_none() => {}
                None => {
                    doc.0.retain(|(name, _)| name != key);
                    changed = true;
                }
                Some(want) if have == Some(&want) => {}
                Some(want) => {
                    doc.set(key, self.raw(key, &want, entries, &layout)?);
                    changed = true;
                }
            }
        }
        changed.then(|| layout.file(&doc))
    }
}

/// Every shared key and whether it is merged entry by entry.
fn keys<'a>(shared: &Shared<'a>) -> impl Iterator<Item = (&'a str, bool)> {
    let entries = shared.entries.iter().map(|key| (*key, true));
    entries.chain(shared.values.iter().map(|key| (*key, false)))
}

/// Whether `side` holds none of `names` under `key` (or not the key at all).
fn holds_none<'n>(
    side: &[(String, Node)],
    key: &str,
    mut names: impl Iterator<Item = &'n str>,
) -> bool {
    match get(side, key).and_then(Node::as_object) {
        Some(held) => {
            let held = index(held);
            names.all(|name| !held.contains_key(name))
        }
        None => true,
    }
}

/// The base without the keys one side has reset: when the base of an entry key holds two
/// or more entries and a side holds none of them, that file was recreated (or wiped), it
/// did not remove entry after entry — the key is united in this run, nothing is removed.
/// Removing the only entry, or some of several, stays a removal.
fn without_resets(base: &[(String, Node)], sides: [&Entries; 2], shared: &Shared) -> Entries {
    let reset = |(key, value): &&(String, Node)| {
        let names = value
            .as_object()
            .filter(|_| shared.entries.contains(&key.as_str()));
        names.filter(|names| names.len() >= 2).is_some_and(|names| {
            let none =
                |side: &Entries| holds_none(side, key, names.iter().map(|(name, _)| name.as_str()));
            none(sides[0]) || none(sides[1])
        })
    };
    base.iter().filter(|entry| !reset(entry)).cloned().collect()
}

/// A base made of entry names: every known entry only one side still has, with that side's
/// value — so the merge reads it as removed on the other side, whatever happened to it
/// since. A key a side has reset (as in [`without_resets`]) gets no base.
fn from_names(names: &Base, sides: [&Entries; 2], shared: &Shared) -> Entries {
    let mut base = Entries::new();
    for key in shared.entries {
        let Some(list) = names.get(*key) else {
            continue;
        };
        let known = || list.iter().map(String::as_str);
        if list.len() >= 2 && sides.iter().any(|side| holds_none(side, key, known())) {
            continue;
        }
        let held = sides.map(|side| {
            get(side, key)
                .and_then(Node::as_object)
                .map(|held| index(held))
        });
        let of =
            |side: usize, name: &str| held[side].as_ref().and_then(|held| held.get(name).copied());
        let one_sided = list
            .iter()
            .filter_map(|name| match (of(0, name), of(1, name)) {
                (Some(value), None) | (None, Some(value)) => Some((name.clone(), value.clone())),
                _ => None,
            });
        base.push((key.to_string(), Node::Object(one_sided.collect())));
    }
    base
}

/// The result of [`merge_keys`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merged {
    /// The new content of each side; `None` for a side that stays as it is.
    pub default: Option<Vec<u8>>,
    pub profile: Option<Vec<u8>>,
    /// The merge took something from that side: an entry, a field, or a value it had
    /// changed while the other side won.
    pub lost_default: bool,
    pub lost_profile: bool,
    /// The shared subset both sides now hold: the next sync's base.
    pub shared: Entries,
    /// Keys merged entry by entry that hold no object on a side: left as they are on both
    /// sides and out of the base.
    pub skipped: Vec<String>,
}

/// Both files with the shared subset merged three ways against what is `known` of the last
/// sync: who differs from the base has changed something, down to the fields of an entry;
/// what one side removed is removed on the other; where both changed the same value the
/// default home wins if `default_newer` — and always against another profile's base. A
/// shared key that holds no object on a side is left out ([`Merged::skipped`]).
/// The fields that belong to each side ([`Shared::own_fields`]) neither travel nor count
/// as a difference. While one file is missing nothing is removed.
pub fn merge_keys(
    default: Option<&[u8]>,
    profile: Option<&[u8]>,
    shared: &Shared,
    known: Known,
    default_newer: bool,
) -> Result<Merged, NotJson> {
    let (mut ours, mut theirs) = (Side::read(default, shared)?, Side::read(profile, shared)?);
    // A key that is no object on one side is merged on neither.
    let skipped: Vec<&str> = ours.odd.iter().chain(&theirs.odd).copied().collect();
    let mut kept = shared.entries.to_vec();
    kept.retain(|key| !skipped.contains(key));
    let shared = &Shared {
        entries: &kept,
        ..*shared
    };
    if !skipped.is_empty() {
        (ours, theirs) = (Side::read(default, shared)?, Side::read(profile, shared)?);
    }
    let sides = [&ours.shared, &theirs.shared];
    let (merged, mut lost) = match (default, profile) {
        (Some(_), Some(_)) => {
            let (base, seed) = match known {
                Known::Nothing => (None, None),
                Known::Values(base) => (Some(without_resets(base, sides, shared)), None),
                Known::Names(names) => (Some(from_names(names, sides, shared)), None),
                // A seed the default home holds nothing of tells nothing.
                Known::Seed(seed) => (None, Some(without_resets(seed, [sides[0]; 2], shared))),
            };
            let pair = [ours.shared.as_slice(), theirs.shared.as_slice()];
            // Against a seed an entry is one unit; the keys that hold entries are containers.
            let how: (&[&str], &[&str]) = (&[], shared.entries);
            let merged = merge_pair(how, base.as_deref(), seed.as_deref(), pair, default_newer);
            let lost = [
                merged.conflict && !merged.a_wins,
                merged.conflict && merged.a_wins,
            ];
            (merged.shared, lost)
        }
        // A file that is not there has removed nothing.
        (Some(_), None) => (ours.shared.clone(), [false; 2]),
        (None, _) => (theirs.shared.clone(), [false; 2]),
    };
    let whole = Node::Object(merged.clone());
    for (lost, side) in lost.iter_mut().zip(sides) {
        *lost |= loses(&Node::Object(side.clone()), &whole);
    }
    Ok(Merged {
        default: ours.rebuilt(&merged, shared),
        profile: theirs.rebuilt(&merged, shared),
        lost_default: lost[0],
        lost_profile: lost[1],
        shared: merged,
        skipped: skipped.iter().map(|key| key.to_string()).collect(),
    })
}

/// What a pair without a base of its own can fall back on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fallback<'a> {
    /// The entry names an older marker recorded.
    pub names: Option<&'a Base>,
    /// The base of another profile of the agent, as of the start of the run.
    pub seed: Option<&'a [(String, Node)]>,
}

/// The state files of a pair ([`merge_keys`]). The base is the one in `profile_dir`; a
/// pair without one falls back to the entry names an older marker recorded, then to the
/// seed of another profile ([`Fallback`]). Until the rule's first sync succeeded (`first`) the pair's
/// own base and names are ignored and each file that is rewritten goes to its side's
/// backup first; later a file goes there when the merge takes something from it. An
/// unparsable file on either side stops the rule; it is never overwritten. A shared key
/// that holds no object is skipped and reported, the rest is merged ([`Synced::partial`]).
pub fn sync_state(
    name: &str,
    [default, profile]: [&Path; 2],
    profile_dir: &Path,
    first: bool,
    shared: &Shared,
    fallback: Fallback,
    backups: &mut Backups,
) -> Synced {
    let mut synced = Synced::default();
    let fail = |what: &str| problem(name, what);
    let (ours, theirs) = match (
        load(default, MAX_STATE_BYTES),
        load(profile, MAX_STATE_BYTES),
    ) {
        (Ok(ours), Ok(theirs)) => (ours, theirs),
        (Err(what), _) | (_, Err(what)) => {
            synced.error = Some(fail(what));
            return synced;
        }
    };
    sides_read();
    if ours.is_none() && theirs.is_none() {
        return synced;
    }
    let base = read_base(profile_dir, name).filter(|_| !first);
    let known = match (&base, fallback.names.filter(|_| !first), fallback.seed) {
        (Some(base), _, _) => Known::Values(base),
        (None, Some(names), _) => Known::Names(names),
        (None, None, Some(seed)) => Known::Seed(seed),
        (None, None, None) => Known::Nothing,
    };
    // The default home wins a first sync; later the newer file does.
    let newer = match (&ours, &theirs) {
        (Some(ours), Some(theirs)) => first || ours.modified >= theirs.modified,
        _ => true,
    };
    let merged = match merge_keys(bytes(&ours), bytes(&theirs), shared, known, newer) {
        Ok(merged) => merged,
        Err(NotJson) => {
            synced.error = Some(fail(UNREADABLE));
            return synced;
        }
    };
    let odd = |key: &String| fail(&format!("„{key}“ ist kein Objekt und bleibt unverändert"));
    let odd: Vec<String> = merged.skipped.iter().map(odd).collect();
    let outs = [
        Out {
            path: default,
            seen: &ours,
            new: merged.default,
            backup: first || merged.lost_default,
        },
        Out {
            path: profile,
            seen: &theirs,
            new: merged.profile,
            backup: first || merged.lost_profile,
        },
    ];
    let done = commit(
        name,
        outs,
        profile_dir,
        &merged.shared,
        backups,
        &mut synced.change,
    );
    synced.error = done.err().map(fail);
    if synced.error.is_none() {
        synced.partial = odd;
    }
    synced
}
