//! The text side of the CLI's state file (`.claude.json`): its top-level object with every
//! value as the bytes it was written in, and the layout a rewrite keeps. What travels
//! between two profiles is decided in [`super::state`].
use super::node::Style;
use super::{OwnFields, Recipe};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::Value;

/// A JSON object in file order, every value as the text it was written in.
#[derive(Default)]
pub(super) struct Entries(pub(super) Vec<(String, Box<RawValue>)>);

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
    pub(super) fn get(&self, key: &str) -> Option<&RawValue> {
        let found = self.0.iter().rev().find(|(name, _)| name == key);
        found.map(|(_, value)| value.as_ref())
    }

    pub(super) fn set(&mut self, key: &str, value: Box<RawValue>) {
        match self.0.iter_mut().rev().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key.to_string(), value)),
        }
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
    pub(super) fn own(&self, key: &str) -> Option<&'a OwnFields> {
        self.own_fields.filter(|own| own.key == key)
    }
}

/// The entry names of each shared key as an older version recorded them in the marker —
/// names only, never a value. Read once, for the first sync that has no value base yet.
pub type Base = std::collections::BTreeMap<String, Vec<String>>;

pub(super) fn object(raw: &RawValue) -> Option<Entries> {
    serde_json::from_str(raw.get()).ok()
}

/// How the file indents (`None`: everything on one line) and whether it ends with a line
/// break; a new file is written the way the CLI writes its own.
pub(super) struct Layout {
    indent: Option<String>,
    newline: &'static str,
    trailing: bool,
    /// The file starts with a byte order mark; a rewrite keeps it.
    bom: bool,
}

impl Layout {
    pub(super) fn of(text: Option<&[u8]>) -> Self {
        let Some(text) = text.map(String::from_utf8_lossy) else {
            return Self {
                indent: Some("  ".into()),
                newline: "\n",
                trailing: false,
                bom: false,
            };
        };
        let bom = text.starts_with('\u{feff}');
        let text = text.trim_start_matches('\u{feff}');
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
            bom,
        }
    }

    pub(super) fn style(&self) -> Style<'_> {
        Style {
            indent: self.indent.as_deref(),
            newline: self.newline,
        }
    }

    /// The object `entries` as text, `depth` levels inside the file.
    pub(super) fn object(&self, entries: &Entries, depth: usize) -> String {
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

    pub(super) fn file(&self, entries: &Entries) -> Vec<u8> {
        let mut text = String::from(if self.bom { "\u{feff}" } else { "" });
        text.push_str(&self.object(entries, 0));
        if self.trailing {
            text.push_str(self.newline);
        }
        text.into_bytes()
    }
}
