//! A JSON value that remembers the order of its object keys (`serde_json::Value` sorts them
//! here: the crate is built without `preserve_order`). Compared as a value — key order and
//! formatting never make two documents differ — and printed the way the CLI writes its
//! settings: two spaces, one entry per line.
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Node {
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    /// `null`, a boolean, a number or a string.
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

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Node::Object(first), Node::Object(second)) => same_object(first, second),
            (Node::Array(first), Node::Array(second)) => first == second,
            (Node::Leaf(first), Node::Leaf(second)) => first == second,
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

    /// The top-level object of a JSON document; `None` for anything else.
    pub fn parse_object(bytes: &[u8]) -> Option<Entries> {
        match serde_json::from_slice::<Node>(bytes).ok()? {
            Node::Object(entries) => Some(entries),
            _ => None,
        }
    }

    fn print(&self, depth: usize, out: &mut String) {
        let pad = |out: &mut String, depth: usize| {
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
        };
        match self {
            Node::Leaf(value) => out.push_str(&value.to_string()),
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

impl<'de> serde::Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Any;
        impl<'de> Visitor<'de> for Any {
            type Value = Node;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON value")
            }
            fn visit_bool<E>(self, value: bool) -> Result<Node, E> {
                Ok(Node::Leaf(value.into()))
            }
            fn visit_i64<E>(self, value: i64) -> Result<Node, E> {
                Ok(Node::Leaf(value.into()))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Node, E> {
                Ok(Node::Leaf(value.into()))
            }
            fn visit_f64<E>(self, value: f64) -> Result<Node, E> {
                Ok(Node::Leaf(value.into()))
            }
            fn visit_str<E>(self, value: &str) -> Result<Node, E> {
                Ok(Node::Leaf(value.into()))
            }
            fn visit_unit<E>(self) -> Result<Node, E> {
                Ok(Node::Leaf(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Node>()? {
                    items.push(item);
                }
                Ok(Node::Array(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
                let mut entries = Entries::new();
                let mut places: HashMap<String, usize> = HashMap::new();
                while let Some((key, value)) = map.next_entry::<String, Node>()? {
                    // A duplicate key reads as its last occurrence, as JSON parsers do.
                    match places.get(&key) {
                        Some(place) => entries[*place].1 = value,
                        None => {
                            places.insert(key.clone(), entries.len());
                            entries.push((key, value));
                        }
                    }
                }
                Ok(Node::Object(entries))
            }
        }
        deserializer.deserialize_any(Any)
    }
}
