//! A JSON value that remembers the order of its object keys (`serde_json::Value` sorts them
//! here: the crate is built without `preserve_order`). Compared as a value — key order and
//! formatting never make two documents differ — and printed the way the CLI writes its
//! settings: two spaces, one entry per line.
use super::files::without_bom;
use super::json::Entries as RawEntries;
use serde_json::value::RawValue;
use serde_json::Value;
use std::collections::HashMap;

/// As deep as serde_json itself reads a document.
const MAX_DEPTH: usize = 128;

#[derive(Clone, Debug)]
pub enum Node {
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    /// A number as the file wrote it: never parsed into a float and printed again, so no
    /// digit of a long fraction or a large integer changes.
    Number(String),
    /// `null`, a boolean or a string.
    Leaf(Value),
}

/// The entries of an object in file order.
pub type Entries = Vec<(String, Node)>;

pub fn get<'a>(entries: &'a [(String, Node)], key: &str) -> Option<&'a Node> {
    let found = entries.iter().find(|(name, _)| name == key);
    found.map(|(_, value)| value)
}

/// The entries by key, for an object that is looked into more than once: a document with
/// very many keys must not cost a scan per lookup.
pub fn index(entries: &[(String, Node)]) -> HashMap<&str, &Node> {
    let pairs = entries.iter().map(|(key, value)| (key.as_str(), value));
    pairs.collect()
}

/// Replaces the value of `key` in place, or appends the entry.
pub fn set(entries: &mut Entries, key: &str, value: Node) {
    match entries.iter_mut().find(|(name, _)| name == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key.to_string(), value)),
    }
}

/// Two objects with the same keys and values, in whatever order.
pub fn same_object(first: &[(String, Node)], second: &[(String, Node)]) -> bool {
    let second = index(second);
    first.len() == second.len()
        && first
            .iter()
            .all(|(key, value)| second.get(key.as_str()) == Some(&value))
}

/// The same token, or two spellings of one number (`1.50` and `1.5`, `1e2` and `100.0`).
fn same_number(first: &str, second: &str) -> bool {
    let parsed = |token: &str| serde_json::from_str::<serde_json::Number>(token).ok();
    first == second || (parsed(first).is_some() && parsed(first) == parsed(second))
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Node::Object(first), Node::Object(second)) => same_object(first, second),
            (Node::Array(first), Node::Array(second)) => first == second,
            (Node::Leaf(first), Node::Leaf(second)) => first == second,
            (Node::Number(first), Node::Number(second)) => same_number(first, second),
            _ => false,
        }
    }
}

impl Node {
    pub fn as_object(&self) -> Option<&Entries> {
        match self {
            Node::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::Leaf(Value::String(text)) => Some(text),
            _ => None,
        }
    }

    /// The items of an array of nothing but strings.
    pub fn as_strings(&self) -> Option<Vec<&str>> {
        match self {
            Node::Array(items) => items.iter().map(Node::as_str).collect(),
            _ => None,
        }
    }

    /// The top-level object of a JSON document (a UTF-8 byte order mark before it is
    /// skipped); `None` for anything else.
    pub fn parse_object(bytes: &[u8]) -> Option<Entries> {
        // One pass over the whole text first: it is valid JSON and not nested too deeply.
        let raw: Box<RawValue> = serde_json::from_slice(without_bom(bytes)).ok()?;
        match Node::read(&raw, 0)? {
            Node::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// The value behind its text, level by level, so a number keeps its token. Nesting
    /// beyond [`MAX_DEPTH`] is refused.
    fn read(raw: &RawValue, depth: usize) -> Option<Node> {
        if depth > MAX_DEPTH {
            return None;
        }
        let text = raw.get();
        match text.as_bytes().first()? {
            b'{' => {
                let fields: RawEntries = serde_json::from_str(text).ok()?;
                let mut entries = Entries::new();
                let mut places: HashMap<String, usize> = HashMap::new();
                for (key, value) in fields.0 {
                    let value = Node::read(&value, depth + 1)?;
                    // A duplicate key reads as its last occurrence, as JSON parsers do.
                    match places.get(&key) {
                        Some(place) => entries[*place].1 = value,
                        None => {
                            places.insert(key.clone(), entries.len());
                            entries.push((key, value));
                        }
                    }
                }
                Some(Node::Object(entries))
            }
            b'[' => {
                let items: Vec<Box<RawValue>> = serde_json::from_str(text).ok()?;
                let items = items.iter().map(|item| Node::read(item, depth + 1));
                Some(Node::Array(items.collect::<Option<_>>()?))
            }
            b'"' | b't' | b'f' | b'n' => Some(Node::Leaf(serde_json::from_str(text).ok()?)),
            _ => Some(Node::Number(text.to_string())),
        }
    }

    fn print(&self, depth: usize, out: &mut String) {
        let pad = |out: &mut String, depth: usize| {
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
        };
        match self {
            Node::Leaf(value) => out.push_str(&value.to_string()),
            Node::Number(token) => out.push_str(token),
            Node::Array(items) if items.is_empty() => out.push_str("[]"),
            Node::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Node::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    pad(out, depth + 1);
                    item.print(depth + 1, out);
                }
                pad(out, depth);
                out.push(']');
            }
            Node::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    pad(out, depth + 1);
                    out.push_str(&Value::from(key.as_str()).to_string());
                    out.push_str(": ");
                    value.print(depth + 1, out);
                }
                pad(out, depth);
                out.push('}');
            }
        }
    }
}

/// The object as a file: two-space indent and a final line break.
pub fn to_file(entries: &[(String, Node)]) -> Vec<u8> {
    let mut text = String::new();
    Node::Object(entries.to_vec()).print(0, &mut text);
    text.push('\n');
    text.into_bytes()
}
