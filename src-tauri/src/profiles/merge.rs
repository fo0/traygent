//! The three-way merge of a settings file (#35): the part that belongs to the sign-in never
//! leaves its profile ([`split`], [`join`]), the rest is merged against the state of the
//! last sync ([`Merge`]) — who differs from that base has changed something.
use super::node::{get, index, set, Entries, Node};
use super::SettingsRule;
use std::collections::HashSet;

impl SettingsRule {
    /// Whether the environment variable `name` belongs to the profile (sign-in, routing).
    /// Variable names are compared without case, as Windows does.
    pub fn own_env(&self, name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        let affixed = |(prefix, suffix): &(&str, &str)| {
            let between = upper.strip_prefix(prefix);
            between.is_some_and(|rest| rest.ends_with(suffix))
        };
        self.own_env_names.contains(&upper.as_str())
            || self
                .own_env_prefixes
                .iter()
                .any(|prefix| upper.starts_with(prefix))
            || self.own_env_affixes.iter().any(affixed)
    }
}

impl SettingsRule {
    /// The objects of the settings whose children are independent of each other: `env` and
    /// the parents of the set lists (`permissions`, `autoMode`).
    pub fn containers(&self) -> Vec<&'static str> {
        let parents = self.set_paths.iter().map(|path| path.trim_end_matches('.'));
        [self.env_key].into_iter().chain(parents).collect()
    }
}

/// Whether a list of strings at `path` (keys joined with `.`) is a set: it lies below one
/// of the `set_paths` prefixes.
fn is_set(set_paths: &[&str], path: &str) -> bool {
    let below = |prefix: &&str| path.len() > prefix.len() && path.starts_with(*prefix);
    set_paths.iter().any(below)
}

/// A settings document in two parts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Split {
    /// What every profile shares. `env` is in it only while it holds a shared variable.
    pub shared: Entries,
    /// The profile's own top-level keys.
    pub own: Entries,
    /// The profile's own variables of `env`.
    pub own_env: Entries,
}

pub fn split(doc: &[(String, Node)], rule: &SettingsRule) -> Split {
    let mut parts = Split::default();
    for (key, value) in doc {
        if rule.own_keys.contains(&key.as_str()) {
            parts.own.push((key.clone(), value.clone()));
            continue;
        }
        let Some(env) = value.as_object().filter(|_| key == rule.env_key) else {
            parts.shared.push((key.clone(), value.clone()));
            continue;
        };
        let (own, shared): (Entries, Entries) = env
            .iter()
            .cloned()
            .partition(|(name, _)| rule.own_env(name));
        parts.own_env = own;
        if !shared.is_empty() {
            parts.shared.push((key.clone(), Node::Object(shared)));
        }
    }
    parts
}

/// One side's document after the merge: the shared part plus that side's own keys and
/// variables, in the key order of its `original` (at every depth), new keys appended. An
/// `env` object the side had stays an object, even an empty one.
pub fn join(
    shared: &[(String, Node)],
    own: Option<&Split>,
    original: Option<&[(String, Node)]>,
    rule: &SettingsRule,
) -> Entries {
    let mut doc = shared.to_vec();
    let own_env = own.map(|own| own.own_env.as_slice()).unwrap_or_default();
    let had_env = original
        .and_then(|original| get(original, rule.env_key))
        .is_some_and(|env| env.as_object().is_some());
    let env = match get(&doc, rule.env_key) {
        Some(Node::Object(entries)) => Some(entries.clone()),
        // A shared `env` that is no object cannot take variables: the side's own ones stay.
        Some(_) if own_env.is_empty() => None,
        None if !had_env && own_env.is_empty() => None,
        _ => Some(Entries::new()),
    };
    if let Some(mut env) = env {
        env.extend(own_env.iter().cloned());
        set(&mut doc, rule.env_key, Node::Object(env));
    }
    doc.extend(own.into_iter().flat_map(|own| own.own.iter().cloned()));
    let original = original.map(|original| Node::Object(original.to_vec()));
    match reorder(Node::Object(doc), original.as_ref()) {
        Node::Object(entries) => entries,
        _ => Entries::new(),
    }
}

/// `value` with the keys of every object in the order `original` has them, others after.
pub(super) fn reorder(value: Node, original: Option<&Node>) -> Node {
    let (Node::Object(entries), Some(order)) = (&value, original.and_then(Node::as_object)) else {
        return value;
    };
    let (held, known) = (index(entries), index(order));
    let first = order
        .iter()
        .filter_map(|(key, _)| held.get_key_value(key.as_str()));
    let rest = entries
        .iter()
        .filter(|(key, _)| !known.contains_key(key.as_str()))
        .map(|(key, value)| (key.as_str(), value));
    let ordered = first
        .map(|(key, value)| (*key, *value))
        .chain(rest)
        .map(|(key, value)| {
            let child = reorder(value.clone(), known.get(key).copied());
            (key.to_string(), child)
        })
        .collect();
    Node::Object(ordered)
}

/// A three-way merge of two sides `a` (the default home) and `b` (the profile).
pub struct Merge<'a> {
    /// Lists of strings below these paths are merged as sets.
    set_paths: &'a [&'a str],
    /// Who wins where both sides changed the same value differently.
    prefer_a: bool,
    /// The paths decided that way.
    pub conflicts: Vec<String>,
    /// The base is another profile's ([`Merge::seeded`]).
    seeded: bool,
    /// Seeded only: the objects (by path, besides the top level) whose children are
    /// independent of each other. Any other object is one unit.
    containers: &'a [&'a str],
}

impl<'a> Merge<'a> {
    pub fn new(set_paths: &'a [&'a str], prefer_a: bool) -> Self {
        Self {
            set_paths,
            prefer_a,
            conflicts: Vec::new(),
            seeded: false,
            containers: &[],
        }
    }

    /// For a pair that has no base of its own: the base is the one of another profile of
    /// the agent and `a`, the default home, wins. The base then only tells what the default
    /// home has removed — of what `b` still has and `a` lacks ([`Merge::unknown`]):
    /// - inside a container (the top level and `containers`: objects whose children are
    ///   independent, like the settings' `env` or the state file's `projects`) each child
    ///   is looked at on its own, a set list item by item;
    /// - anything else — an object that is one unit (`statusLine`, an MCP server, a
    ///   project entry), a plain value — goes when it is exactly what the base knows and
    ///   stays whole when `b` changed it. Nothing is ever reduced to a fragment.
    ///
    /// What the base never saw is `b`'s own and stays. Everything else is united as without
    /// a base, so a value `b` holds from an older state never replaces the default home's.
    pub fn seeded(set_paths: &'a [&'a str], containers: &'a [&'a str]) -> Self {
        Self {
            seeded: true,
            containers,
            ..Self::new(set_paths, true)
        }
    }

    fn is_container(&self, path: &str) -> bool {
        path.is_empty() || self.containers.contains(&path)
    }

    /// The shared parts of both sides merged. `base`: the shared part as of the last sync;
    /// without one nothing is ever removed.
    pub fn objects(
        &mut self,
        base: Option<&[(String, Node)]>,
        a: &[(String, Node)],
        b: &[(String, Node)],
    ) -> Entries {
        let base = base.map(|base| Node::Object(base.to_vec()));
        self.entries(base.as_ref(), a, b, "")
    }

    fn value(&mut self, base: Option<&Node>, a: &Node, b: &Node, path: &str) -> Node {
        // `b` as the base knows it has changed nothing: `a`'s value, also against a seed.
        if a == b || base == Some(b) {
            return a.clone();
        }
        if !self.seeded && base == Some(a) {
            return b.clone();
        }
        if let (Node::Object(a), Node::Object(b)) = (a, b) {
            // A seed says nothing about the inside of an object that is one unit: both
            // sides' fields are united there.
            let base = base.filter(|_| !self.seeded || self.is_container(path));
            return Node::Object(self.entries(base, a, b, path));
        }
        if let (Some(a), Some(b), true) =
            (a.as_strings(), b.as_strings(), is_set(self.set_paths, path))
        {
            return merged_set(base, &a, &b, self.seeded);
        }
        self.conflicts.push(path.to_string());
        if self.prefer_a { a } else { b }.clone()
    }

    /// Key by key: a key both have is merged; a key only one has stays — unless the base
    /// has it with that very value, then the other side removed it.
    fn entries(
        &mut self,
        base: Option<&Node>,
        a: &[(String, Node)],
        b: &[(String, Node)],
        path: &str,
    ) -> Entries {
        let (in_a, in_b) = (index(a), index(b));
        let in_base = base.and_then(Node::as_object).map(|base| index(base));
        let known = |key: &str| in_base.as_ref().and_then(|base| base.get(key).copied());
        let mut merged = Entries::new();
        for (key, value) in a {
            let was = known(key);
            match in_b.get(key.as_str()) {
                Some(other) => {
                    let sub = sub_path(path, key);
                    merged.push((key.clone(), self.value(was, value, other, &sub)));
                }
                None if !self.seeded && was == Some(value) => {}
                None => merged.push((key.clone(), value.clone())),
            }
        }
        for (key, value) in b.iter().filter(|(key, _)| !in_a.contains_key(key.as_str())) {
            let kept = match known(key) {
                Some(seed) if self.seeded => self.unknown(seed, value, &sub_path(path, key)),
                was => (was != Some(value)).then(|| value.clone()),
            };
            merged.extend(kept.map(|kept| (key.clone(), kept)));
        }
        merged
    }

    /// What is left of `value` — something only `b` has — without what the seed knows (the
    /// default home removed that). Inside a container child by child, the container going
    /// only when nothing of it is left; a set list item by item; anything else goes when
    /// it is the seed's very value and stays whole when `b` changed it.
    fn unknown(&self, seed: &Node, value: &Node, path: &str) -> Option<Node> {
        if let (Node::Object(seed), Node::Object(entries), true) =
            (seed, value, self.is_container(path))
        {
            let seed = index(seed);
            let left: Entries = entries
                .iter()
                .filter_map(|(key, child)| {
                    let left = match seed.get(key.as_str()) {
                        Some(known) => self.unknown(known, child, &sub_path(path, key))?,
                        None => child.clone(),
                    };
                    Some((key.clone(), left))
                })
                .collect();
            return (!left.is_empty()).then_some(Node::Object(left));
        }
        if let (Some(seed), Some(items), true) = (
            seed.as_strings(),
            value.as_strings(),
            is_set(self.set_paths, path),
        ) {
            let seed: HashSet<&str> = seed.into_iter().collect();
            let left: Vec<Node> = items
                .into_iter()
                .filter(|item| !seed.contains(item))
                .map(|item| Node::Leaf(item.into()))
                .collect();
            return (!left.is_empty()).then_some(Node::Array(left));
        }
        (seed != value).then(|| value.clone())
    }
}

/// `path` one key deeper.
fn sub_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// The merge of a pair's shared parts.
pub struct Merged {
    pub shared: Entries,
    /// Both sides had changed a value differently; `a_wins` says whose was taken.
    pub conflict: bool,
    pub a_wins: bool,
}

/// The shared parts of the default home (`a`) and a profile (`b`) merged: against the
/// pair's own `base` when it has one — the side with the newer file (`a_newer`) then wins a
/// conflict; else against the `seed` of another profile ([`Merge::seeded`], with its
/// `containers`), the default
/// home winning; else without a base, nothing removed, `a_newer` deciding again. The caller
/// passes `a_newer` as true for a rule's first sync: the default home wins it.
pub fn merge_pair(
    (set_paths, containers): (&[&str], &[&str]),
    base: Option<&[(String, Node)]>,
    seed: Option<&[(String, Node)]>,
    [a, b]: [&[(String, Node)]; 2],
    a_newer: bool,
) -> Merged {
    let (mut merge, base) = match (base, seed) {
        (Some(base), _) => (Merge::new(set_paths, a_newer), Some(base)),
        (None, Some(seed)) => (Merge::seeded(set_paths, containers), Some(seed)),
        (None, None) => (Merge::new(set_paths, a_newer), None),
    };
    let shared = merge.objects(base, a, b);
    Merged {
        shared,
        conflict: !merge.conflicts.is_empty(),
        a_wins: merge.prefer_a,
    }
}

/// Two lists of strings as sets: what one side added stays, what one side removed since the
/// base goes; `a`'s order, then `b`'s new items. `keep_a`: nothing of `a` goes.
fn merged_set(base: Option<&Node>, a: &[&str], b: &[&str], keep_a: bool) -> Node {
    let base = base.and_then(Node::as_strings).unwrap_or_default();
    let in_base: HashSet<&str> = base.into_iter().collect();
    let in_a: HashSet<&str> = a.iter().copied().collect();
    let in_b: HashSet<&str> = b.iter().copied().collect();
    let mut seen = HashSet::new();
    let kept = a
        .iter()
        .filter(|item| keep_a || in_b.contains(*item) || !in_base.contains(*item));
    let added = b
        .iter()
        .filter(|item| !in_a.contains(*item) && !in_base.contains(*item));
    let items = kept
        .chain(added)
        .filter(|item| seen.insert(**item))
        .map(|item| Node::Leaf((*item).into()))
        .collect();
    Node::Array(items)
}

/// Whether `side` holds something `merged` no longer has: a key, a list item, or a whole
/// object or list replaced by something else. A changed plain value is no loss.
pub fn loses(side: &Node, merged: &Node) -> bool {
    match (side, merged) {
        (Node::Object(side), Node::Object(merged)) => {
            let merged = index(merged);
            side.iter().any(|(key, value)| {
                merged
                    .get(key.as_str())
                    .is_none_or(|kept| loses(value, kept))
            })
        }
        (Node::Array(items), Node::Array(kept_items)) => {
            match (side.as_strings(), merged.as_strings()) {
                (Some(items), Some(kept)) => {
                    let kept: HashSet<&str> = kept.into_iter().collect();
                    items.iter().any(|item| !kept.contains(item))
                }
                _ => items != kept_items,
            }
        }
        (Node::Object(side), _) => !side.is_empty(),
        (Node::Array(side), _) => !side.is_empty(),
        (Node::Leaf(_) | Node::Number(_), _) => false,
    }
}
