//! The settings file of a pair (#35): a three-way merge of the shared part against its
//! state at the last sync ([`super::pair`]); the keys of the sign-in stay in their profile.
use super::files::{load, problem, without_bom, Backups, Loaded, MAX_FILE_BYTES};
use super::merge::{join, loses, merge_pair, split, Split};
use super::node::{get, index, same_object, to_file, Entries, Node};
use super::pair::{commit, modified, sides_read, Out};
pub use super::pair::{read_base, Synced, BASE_DIR};
use super::SettingsRule;
use std::path::Path;

const UNREADABLE: &str = "ist kein lesbares JSON-Objekt und bleibt unverändert";
const ENV_NO_OBJECT: &str = "„env“ ist kein Objekt und bleibt unverändert";

/// One side as read.
struct Side<'a> {
    path: &'a Path,
    seen: Option<Loaded>,
    doc: Option<Entries>,
    parts: Option<Split>,
}

impl<'a> Side<'a> {
    fn read(path: &'a Path, rule: &SettingsRule) -> Result<Self, &'static str> {
        let seen = load(path, MAX_FILE_BYTES)?;
        let doc = match &seen {
            Some(loaded) if !blank(without_bom(&loaded.bytes)) => {
                Some(Node::parse_object(&loaded.bytes).ok_or(UNREADABLE)?)
            }
            _ => None,
        };
        let parts = doc.as_ref().map(|doc| split(doc, rule));
        Ok(Self {
            path,
            seen,
            doc,
            parts,
        })
    }

    /// A file without content: the CLI is writing it.
    fn in_flight(&self) -> bool {
        let seen = self.seen.as_ref();
        seen.is_some_and(|seen| blank(without_bom(&seen.bytes)))
    }

    /// The side's new content, when the merge changes it; a missing file is created from
    /// the shared part.
    fn merged(&self, shared: &[(String, Node)], rule: &SettingsRule) -> Option<Vec<u8>> {
        let joined = join(shared, self.parts.as_ref(), self.doc.as_deref(), rule);
        let same = self
            .doc
            .as_ref()
            .is_some_and(|doc| same_object(doc, &joined));
        (!same).then(|| to_file(&joined))
    }

    /// The side after the merge; `backup`: its file is kept whatever the merge does to it.
    fn out(&self, shared: &[(String, Node)], rule: &SettingsRule, backup: bool) -> Out<'_> {
        Out {
            path: self.path,
            seen: &self.seen,
            new: self.merged(shared, rule),
            backup: backup || self.loses(shared),
        }
    }

    /// Whether the merge takes something from this side that it had.
    fn loses(&self, shared: &[(String, Node)]) -> bool {
        self.parts.as_ref().is_some_and(|parts| {
            loses(
                &Node::Object(parts.shared.clone()),
                &Node::Object(shared.to_vec()),
            )
        })
    }
}

fn blank(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

/// A side that holds none of the base's keys, the base having two or more: that file was
/// recreated or emptied, it did not remove key after key. Merging against the base would
/// take every shared setting from the other side — and, through the default home, from
/// every profile.
fn reset(base: &[(String, Node)], side: &[(String, Node)]) -> bool {
    let held = index(side);
    base.len() >= 2 && base.iter().all(|(key, _)| !held.contains_key(key.as_str()))
}

/// The settings file of a pair. The shared part of both sides is merged against the base
/// of `profile_dir`; each side then is that merge plus its own keys. The newer file wins a
/// conflict — the default home until the rule's first sync succeeded (`first`). Until then,
/// whenever there is no base and when a side was reset ([`reset`]), nothing is removed —
/// except, for a pair without a base, what `seed` (the base of another profile of the
/// agent) says the default home has removed already; the default home wins such a merge. A side is written only when its
/// content (not its formatting) changes, and goes to its backup first on the first sync,
/// when it loses a key or a list item, and when a conflict went against it. The base is
/// written last, after both sides.
pub fn sync_settings(
    rule: &SettingsRule,
    [default, profile]: [&Path; 2],
    profile_dir: &Path,
    first: bool,
    seed: Option<&[(String, Node)]>,
    backups: &mut Backups,
) -> Synced {
    let mut synced = Synced::default();
    let paths = [default, profile];
    if let Err(what) = run(rule, paths, profile_dir, first, seed, backups, &mut synced) {
        synced.error = Some(problem(rule.file, what));
    }
    synced
}

fn run(
    rule: &SettingsRule,
    paths: [&Path; 2],
    profile_dir: &Path,
    first: bool,
    seed: Option<&[(String, Node)]>,
    backups: &mut Backups,
    synced: &mut Synced,
) -> Result<(), &'static str> {
    let name = rule.file;
    let ours = Side::read(paths[0], rule)?;
    let theirs = Side::read(paths[1], rule)?;
    if ours.in_flight() || theirs.in_flight() {
        synced.skipped = true;
        return Ok(());
    }
    sides_read();
    let base = if first {
        None
    } else {
        read_base(profile_dir, name)
    };
    let (mut default_wins, mut conflict) = (true, false);
    let shared = match (&ours.parts, &theirs.parts) {
        (None, None) => return Ok(()),
        // A file that is not there has removed nothing.
        (Some(only), None) | (None, Some(only)) => only.shared.clone(),
        (Some(a), Some(b)) => {
            let was_reset = base
                .as_ref()
                .is_some_and(|base| reset(base, &a.shared) || reset(base, &b.shared));
            // A seed whose keys the default home holds none of tells nothing either.
            let seed = seed.filter(|seed| !reset(seed, &a.shared));
            let (base, seed) = match was_reset {
                true => (None, None),
                false => (base.as_deref(), seed),
            };
            // The default home wins a first sync; later the newer file does, with or
            // without a base (a profile synced by a version from before the base).
            let newer = first || modified(&ours.seen) >= modified(&theirs.seen);
            let how = (rule.set_paths, rule.containers());
            let how = (how.0, how.1.as_slice());
            let merged = merge_pair(how, base, seed, [&a.shared, &b.shared], newer);
            (default_wins, conflict) = (merged.a_wins, merged.conflict);
            merged.shared
        }
    };
    // A shared `env` that is no object cannot hold a side's own variables: writing would
    // either drop them or record a base that side never has.
    let own_env = |side: &Side| {
        side.parts
            .as_ref()
            .is_some_and(|parts| !parts.own_env.is_empty())
    };
    let env = get(&shared, rule.env_key);
    if env.is_some_and(|env| env.as_object().is_none()) && (own_env(&ours) || own_env(&theirs)) {
        return Err(ENV_NO_OBJECT);
    }
    let lost = [conflict && !default_wins, conflict && default_wins];
    let outs = [
        ours.out(&shared, rule, first || lost[0]),
        theirs.out(&shared, rule, first || lost[1]),
    ];
    commit(
        name,
        outs,
        profile_dir,
        &shared,
        backups,
        &mut synced.change,
    )
}
