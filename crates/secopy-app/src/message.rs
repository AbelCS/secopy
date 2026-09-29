//! Texts for the UI as codes (#84): a catalog key and the values of its placeholders. The UI
//! turns them into words from `ui/src/locales/<lang>.json`; Rust knows no language.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

/// A text for the UI: a catalog key and the values of its placeholders. Build it with
/// [`msg!`](crate::msg).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Message {
    pub key: String,
    pub args: BTreeMap<String, Arg>,
}

/// A placeholder's value. The UI formats numbers for its language, a `Size` as bytes
/// ("212.4 GB"), translates a nested `Message` first and joins a `List` ("a, b").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(untagged)]
pub enum Arg {
    /// Counts and bytes are exact as `f64` up to 2^53.
    Number(#[specta(type = specta_typescript::Number)] f64),
    Text(String),
    List(Vec<Message>),
    Size {
        #[specta(type = specta_typescript::Number)]
        bytes: f64,
    },
    Message(Box<Message>),
}

/// A byte count shown as a size.
pub struct Size(pub u64);

impl Message {
    /// Text shown as it is: a path, a name, an OS error's words.
    pub fn raw(text: impl Into<String>) -> Message {
        crate::msg!("format.raw", text = text.into())
    }

    /// The message in English, from the same catalog the UI reads: for what Rust shows
    /// itself (menus, the menu bar icon) and for tests. Numbers and sizes read as the UI
    /// shows them in English; a key the catalog lacks shows as itself.
    pub fn english(&self) -> String {
        static CATALOG: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("../../../ui/src/locales/en.json"))
                .expect("en.json is JSON")
        });
        let entry = self
            .key
            .split('.')
            .try_fold(&*CATALOG, |node, part| node.get(part));
        let text = match entry {
            Some(serde_json::Value::String(text)) => text.as_str(),
            Some(serde_json::Value::Object(forms)) => {
                let one = matches!(self.args.get("count"), Some(Arg::Number(n)) if *n == 1.0);
                let form = if one { "one" } else { "other" };
                match forms.get(form).or_else(|| forms.get("other")) {
                    Some(serde_json::Value::String(text)) => text.as_str(),
                    _ => return self.key.clone(),
                }
            }
            _ => return self.key.clone(),
        };
        let mut out = String::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            match after.find('}').map(|close| (&after[..close], close)) {
                Some((name, close)) if self.args.contains_key(name) => {
                    out.push_str(&self.args[name].english());
                    rest = &after[close + 1..];
                }
                _ => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }
}

impl Arg {
    fn english(&self) -> String {
        match self {
            Arg::Number(n) => grouped(*n),
            Arg::Text(t) => t.clone(),
            Arg::List(items) => items
                .iter()
                .map(Message::english)
                .collect::<Vec<_>>()
                .join(", "),
            Arg::Size { bytes } => {
                const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
                let (mut value, mut unit) = (*bytes, 0);
                while value >= 1000.0 && unit < UNITS.len() - 1 {
                    value /= 1000.0;
                    unit += 1;
                }
                if unit == 0 {
                    format!("{} B", grouped(*bytes))
                } else {
                    format!("{value:.1} {}", UNITS[unit])
                }
            }
            Arg::Message(m) => m.english(),
        }
    }
}

/// 1284 → "1,284", as the UI's English number format.
fn grouped(n: f64) -> String {
    if n.fract() != 0.0 {
        return n.to_string();
    }
    let digits = (n.abs() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0.0 { format!("-{out}") } else { out }
}

/// `msg!("errors.file.inTheWay", path = &p)`: a [`Message`] with a literal key, so the
/// registry test can check every key against the catalog.
#[macro_export]
macro_rules! msg {
    ($key:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::message::Message {
            key: $key.to_string(),
            args: [$((stringify!($name).to_string(), $crate::message::Arg::from($value))),*]
                .into_iter()
                .collect(),
        }
    };
}

impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Text(v.to_string())
    }
}
impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Text(v)
    }
}
impl From<&String> for Arg {
    fn from(v: &String) -> Self {
        Arg::Text(v.clone())
    }
}
impl From<&Path> for Arg {
    fn from(v: &Path) -> Self {
        Arg::Text(crate::dto::show(v))
    }
}
impl From<&PathBuf> for Arg {
    fn from(v: &PathBuf) -> Self {
        Arg::Text(crate::dto::show(v))
    }
}
impl From<PathBuf> for Arg {
    fn from(v: PathBuf) -> Self {
        Arg::Text(crate::dto::show(&v))
    }
}
macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg {
            fn from(v: $t) -> Self {
                Arg::Number(v as f64)
            }
        }
    )*};
}
numbers!(u8, u16, u32, u64, usize, i32, i64, f64);
impl From<Size> for Arg {
    fn from(v: Size) -> Self {
        Arg::Size { bytes: v.0 as f64 }
    }
}
impl From<Message> for Arg {
    fn from(v: Message) -> Self {
        Arg::Message(Box::new(v))
    }
}
impl From<&Message> for Arg {
    fn from(v: &Message) -> Self {
        Arg::Message(Box::new(v.clone()))
    }
}
impl From<Vec<Message>> for Arg {
    fn from(v: Vec<Message>) -> Self {
        Arg::List(v)
    }
}

/// The `msg!` calls in Rust source: each key and its argument names (sorted).
#[cfg(test)]
fn calls(source: &str) -> Vec<(String, Vec<String>)> {
    // Comments (doc examples) aren't calls.
    let source: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = source.as_str();
    let mut found = Vec::new();
    for (start, _) in source.match_indices("msg!(") {
        // `"msg!("` in a string is no call.
        if source[..start].ends_with('"') {
            continue;
        }
        let rest = &source[start + "msg!(".len()..];
        let mut items = vec![String::new()];
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for ch in rest.chars() {
            if in_string {
                items.last_mut().unwrap().push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' if depth == 0 => break,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    items.push(String::new());
                    continue;
                }
                _ => {}
            }
            items.last_mut().unwrap().push(ch);
        }
        let key = items[0].trim().trim_matches('"').to_string();
        let mut names: Vec<String> = items[1..]
            .iter()
            .filter(|i| !i.trim().is_empty())
            .map(|i| i.split('=').next().unwrap().trim().to_string())
            .collect();
        names.sort();
        found.push((key, names));
    }
    found
}

/// What doesn't match the catalog: a key it lacks, or other placeholders than the text's.
#[cfg(test)]
fn mismatches(calls: &[(String, Vec<String>)], catalog: &serde_json::Value) -> Vec<String> {
    fn placeholders(text: &str, into: &mut std::collections::BTreeSet<String>) {
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            rest = &rest[open + 1..];
            if let Some(close) = rest.find('}') {
                let name = &rest[..close];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    into.insert(name.to_string());
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (key, args) in calls {
        if !seen.insert((key.clone(), args.clone())) {
            continue;
        }
        let entry = key
            .split('.')
            .try_fold(catalog, |node, part| node.get(part));
        let mut wanted = std::collections::BTreeSet::new();
        match entry {
            Some(serde_json::Value::String(text)) => placeholders(text, &mut wanted),
            Some(serde_json::Value::Object(forms)) if forms.contains_key("other") => {
                for form in forms.values() {
                    placeholders(form.as_str().unwrap_or(""), &mut wanted);
                }
                wanted.insert("count".to_string());
            }
            _ => {
                out.push(format!("{key}: not in en.json"));
                continue;
            }
        }
        let sent: std::collections::BTreeSet<String> = args.iter().cloned().collect();
        if sent != wanted {
            out.push(format!(
                "{key}: sends {args:?} but the text has {:?}",
                wanted.into_iter().collect::<Vec<_>>()
            ));
        }
    }
    out
}

/// Tests compare messages with the English the UI shows.
#[cfg(test)]
mod english_in_tests {
    use super::Message;

    impl PartialEq<&str> for Message {
        fn eq(&self, other: &&str) -> bool {
            self.english() == *other
        }
    }

    impl PartialEq<str> for Message {
        fn eq(&self, other: &str) -> bool {
            self.english() == other
        }
    }

    impl PartialEq<String> for Message {
        fn eq(&self, other: &String) -> bool {
            &self.english() == other
        }
    }

    impl Message {
        pub fn contains(&self, part: &str) -> bool {
            self.english().contains(part)
        }
        pub fn starts_with(&self, part: &str) -> bool {
            self.english().starts_with(part)
        }
        pub fn ends_with(&self, part: &str) -> bool {
            self.english().ends_with(part)
        }
        pub fn is_empty(&self) -> bool {
            self.english().is_empty()
        }
    }

    impl std::fmt::Display for Message {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.english())
        }
    }

    /// `Some(message)` → `Some(its English)`.
    pub trait En {
        fn en(&self) -> Option<String>;
    }

    impl En for Option<Message> {
        fn en(&self) -> Option<String> {
            self.as_ref().map(Message::english)
        }
    }
}

#[cfg(test)]
pub use english_in_tests::En;

#[cfg(test)]
mod tests {
    /// #84: what the app does never depends on its English words.
    #[test]
    fn no_decision_is_made_on_english_text() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let code = text.split("\n#[cfg(test)]\nmod tests").next().unwrap();
            for (i, line) in code.lines().enumerate() {
                let english = [
                    "strip_prefix(\"",
                    "ends_with(\"",
                    "full_stop(",
                    "fn sentence(",
                ]
                .iter()
                .any(|p| line.contains(p))
                    || line.match_indices("starts_with(\"").any(|(at, p)| {
                        line[at + p.len()..].starts_with(|c: char| c.is_ascii_uppercase())
                    });
                if english {
                    found.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }
        assert_eq!(found, Vec::<String>::new());
    }

    #[test]
    fn a_message_in_english_reads_as_the_ui_says_it() {
        assert_eq!(
            crate::msg!("errors.queue.running").english(),
            "The queue is running."
        );
        assert_eq!(
            crate::msg!("queue.reason.filesFailed", count = 1u32).english(),
            "1 file failed."
        );
        assert_eq!(
            crate::msg!("queue.reason.filesFailed", count = 1284u32).english(),
            "1,284 files failed."
        );
        let space = crate::msg!(
            "errors.blocker.notEnoughSpace",
            needed = Size(212_400_000_000),
            free = Size(999)
        );
        assert_eq!(
            space.english(),
            "Not enough free space: 212.4 GB needed, 999 B free"
        );
        let nested = crate::msg!(
            "errors.file.readSource",
            why = crate::msg!("errors.os.permissionDenied")
        );
        assert_eq!(nested.english(), "Cannot read source: Permission denied");
        let parts = vec![
            crate::msg!("queue.reason.part.changed", count = 2u32),
            crate::msg!("queue.reason.part.missing", count = 1u32),
        ];
        assert_eq!(
            crate::msg!("queue.reason.check", parts = parts).english(),
            "2 changed, 1 missing."
        );
        assert_eq!(crate::msg!("no.such.key").english(), "no.such.key");
    }

    use super::*;
    use std::path::Path;

    #[test]
    fn a_message_is_a_key_and_its_values() {
        let m = crate::msg!("errors.test", name = "x", count = 3u32);
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"key":"errors.test","args":{"count":3.0,"name":"x"}}"#
        );
        assert_eq!(crate::msg!("errors.none").args.len(), 0);
    }

    #[test]
    fn sizes_paths_nested_messages_and_lists() {
        let inner = crate::msg!("errors.os.full");
        let m = crate::msg!(
            "errors.test",
            size = Size(1_000),
            path = Path::new("/Volumes/A"),
            why = inner.clone(),
            parts = vec![inner.clone(), Message::raw("x")],
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["args"]["size"], serde_json::json!({ "bytes": 1000.0 }));
        assert_eq!(json["args"]["path"], "/Volumes/A");
        assert_eq!(json["args"]["why"]["key"], "errors.os.full");
        assert_eq!(json["args"]["parts"][1]["args"]["text"], "x");
        let back: Message = serde_json::from_value(json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn the_registry_reads_msg_calls() {
        let source = r#"
            let a = msg!("errors.one");
            let b = msg!("errors.two", name = f(x, y), count = n,);
            let c = msg!(
                "errors.three",
                why = msg!("errors.one"),
            );
        "#;
        assert_eq!(
            calls(source),
            vec![
                ("errors.one".to_string(), vec![]),
                (
                    "errors.two".to_string(),
                    vec!["count".to_string(), "name".to_string()]
                ),
                ("errors.three".to_string(), vec!["why".to_string()]),
                ("errors.one".to_string(), vec![]),
            ]
        );
    }

    #[test]
    fn the_registry_reports_missing_keys_and_wrong_placeholders() {
        let catalog = serde_json::json!({
            "errors": { "one": "One", "two": "Hi {name}", "n": { "one": "{count} file", "other": "{count} files" } }
        });
        let found = mismatches(
            &[
                ("errors.one".into(), vec![]),
                ("errors.nope".into(), vec![]),
                ("errors.two".into(), vec![]),
                ("errors.n".into(), vec!["count".into()]),
            ],
            &catalog,
        );
        assert_eq!(
            found,
            vec![
                "errors.nope: not in en.json",
                "errors.two: sends [] but the text has [\"name\"]"
            ]
        );
    }

    /// Every key the app can send is in the catalog, with the same placeholders.
    #[test]
    fn every_message_the_app_sends_is_in_the_catalog() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let catalog: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("../../ui/src/locales/en.json")).unwrap(),
        )
        .unwrap();
        let mut all = Vec::new();
        for entry in std::fs::read_dir(root.join("src")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let code = text.split("\n#[cfg(test)]\nmod tests").next().unwrap();
                all.extend(calls(code));
                if path.file_name().unwrap() != "message.rs" {
                    // A struct literal, not a function returning one.
                    let literal = code
                        .match_indices("Message {")
                        .any(|(i, _)| !code[..i].ends_with("-> ") && !code[..i].ends_with("::"));
                    assert!(!literal, "{}: build messages with msg!", path.display());
                }
            }
        }
        assert!(!all.is_empty());
        assert_eq!(mismatches(&all, &catalog), Vec::<String>::new());

        // The UI's unused-key test counts these as used.
        let keys: std::collections::BTreeSet<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
        let fresh = serde_json::to_string_pretty(&keys).unwrap() + "\n";
        let committed = root.join("../../ui/src/locales/rust-keys.json");
        if std::env::var_os("SECOPY_UPDATE_BINDINGS").is_some() {
            std::fs::write(&committed, &fresh).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&committed).unwrap_or_default(),
            fresh,
            "ui/src/locales/rust-keys.json is out of date: run SECOPY_UPDATE_BINDINGS=1 cargo test -p secopy-app"
        );
    }
}
