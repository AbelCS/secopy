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

    /// The message in English, from the catalog the UI reads: for tests, which pin
    /// today's words. Numbers and sizes read as the UI shows them in English; a key the
    /// catalog lacks shows as itself.
    #[cfg(test)]
    pub fn english(&self) -> String {
        self.in_language("en")
    }

    /// The message in the app's language (the Mac's, when Secopy has its words): for what
    /// Rust shows itself, the menus and the menu bar icon. What that catalog lacks is said
    /// in English.
    pub fn text(&self) -> String {
        self.in_language(app_language())
    }

    /// The message in `tag`'s words and number format; what its catalog lacks in English.
    pub fn in_language(&self, tag: &str) -> String {
        let tag = language_for(&[tag.to_string()]);
        self.render(&[catalog(tag), catalog("en")], Numbers::of(tag))
    }

    fn render(&self, catalogs: &[&serde_json::Value], numbers: Numbers) -> String {
        let entry = catalogs.iter().find_map(|c| {
            self.key
                .split('.')
                .try_fold(*c, |node, part| node.get(part))
        });
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
                    out.push_str(&self.args[name].render(catalogs, numbers));
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

/// The catalogs Secopy has words in, as the UI's `ui/src/locales/<tag>.json` (a test keeps
/// the two lists the same). Adding a language adds its line here.
pub const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../../../ui/src/locales/en.json")),
    ("es", include_str!("../../../ui/src/locales/es.json")),
];

/// The catalog for `tag` (one of [`CATALOGS`]).
fn catalog(tag: &str) -> &'static serde_json::Value {
    static PARSED: std::sync::LazyLock<Vec<(&str, serde_json::Value)>> =
        std::sync::LazyLock::new(|| {
            CATALOGS
                .iter()
                .map(|(tag, json)| (*tag, serde_json::from_str(json).expect("a catalog is JSON")))
                .collect()
        });
    PARSED
        .iter()
        .find(|(t, _)| *t == tag)
        .or_else(|| PARSED.first())
        .map(|(_, c)| c)
        .expect("English is built in")
}

/// The first of `preferred` (BCP 47 tags, the Mac's order) Secopy has words in, by the whole
/// tag or its language ("de-AT" → "de"); English otherwise.
pub fn language_for(preferred: &[String]) -> &'static str {
    for tag in preferred {
        let base = tag.split('-').next().unwrap_or(tag);
        if let Some((found, _)) = CATALOGS.iter().find(|(t, _)| *t == tag || *t == base) {
            return found;
        }
    }
    "en"
}

/// The language Secopy uses (#181): the one Settings chose, or the Mac's. `None` until
/// Settings are read at launch: the Mac's until then.
static IN_USE: std::sync::RwLock<Option<&'static str>> = std::sync::RwLock::new(None);

/// A chosen language Secopy has words for, or else the Mac's first one it has (English
/// otherwise). `mac` is the Mac's list only, never Secopy's own choice.
pub fn resolve(choice: Option<&str>, mac: &[String]) -> &'static str {
    choice
        .and_then(|tag| CATALOGS.iter().find(|(t, _)| *t == tag).map(|(t, _)| *t))
        .unwrap_or_else(|| language_for(mac))
}

/// Sets the language Secopy uses from Settings' choice (`None` = Automatic).
pub fn set_language(choice: Option<&str>) {
    let tag = resolve(choice, &mac_languages());
    *IN_USE.write().unwrap_or_else(|e| e.into_inner()) = Some(tag);
}

/// The language Secopy's own words are in: the menus, the menu bar icon, the UI's start.
pub fn language_in_use() -> &'static str {
    let in_use = *IN_USE.read().unwrap_or_else(|e| e.into_inner());
    in_use.unwrap_or_else(|| language_for(&mac_languages()))
}

fn app_language() -> &'static str {
    language_in_use()
}

/// The Mac's languages, in order, from the system's own setting: not the app's preferred
/// languages, which carry Secopy's choice once it has told macOS (`tell_macos`).
pub fn mac_languages() -> Vec<String> {
    use objc2_foundation::{NSArray, NSString, NSUserDefaults, ns_string};
    let defaults = NSUserDefaults::standardUserDefaults();
    let Some(global) = defaults.persistentDomainForName(ns_string!("NSGlobalDomain")) else {
        return Vec::new();
    };
    let Some(value) = global.objectForKey(ns_string!("AppleLanguages")) else {
        return Vec::new();
    };
    let Ok(list) = value.downcast::<NSArray>() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| item.downcast::<NSString>().ok().map(|s| s.to_string()))
        .collect()
}

/// Tells macOS Secopy's language, as System Settings' per-app language does, so its own
/// windows (Open, Save, About) follow from the next launch; Automatic removes it.
pub fn tell_macos(choice: Option<&str>) {
    use objc2_foundation::{NSArray, NSString, NSUserDefaults, ns_string};
    let defaults = NSUserDefaults::standardUserDefaults();
    let key = ns_string!("AppleLanguages");
    match choice {
        Some(tag) => {
            let list = NSArray::from_retained_slice(&[NSString::from_str(tag)]);
            unsafe { defaults.setObject_forKey(Some(&list), key) };
        }
        None => defaults.removeObjectForKey(key),
    }
}

/// Tests that set the language, or read Rust's words in it, take turns.
#[cfg(test)]
pub(crate) fn language_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

impl Arg {
    fn render(&self, catalogs: &[&serde_json::Value], numbers: Numbers) -> String {
        match self {
            Arg::Number(n) => numbers.format(*n),
            Arg::Text(t) => t.clone(),
            Arg::List(items) => items
                .iter()
                .map(|m| m.render(catalogs, numbers))
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
                    format!("{} B", numbers.format(*bytes))
                } else {
                    format!("{} {}", numbers.one_decimal(value), UNITS[unit])
                }
            }
            Arg::Message(m) => m.render(catalogs, numbers),
        }
    }
}

/// A language's number format, as the UI's `Intl.NumberFormat`: English groups thousands with
/// a comma from four digits on ("1,284"); Spanish with a point from five ("1284", "12.845"),
/// and writes a decimal comma.
#[derive(Clone, Copy)]
struct Numbers {
    group: char,
    decimal: char,
    /// The fewest digits that are grouped.
    min_digits: usize,
}

impl Numbers {
    fn of(tag: &str) -> Self {
        match tag.split('-').next().unwrap_or(tag) {
            "es" => Numbers {
                group: '.',
                decimal: ',',
                min_digits: 5,
            },
            _ => Numbers {
                group: ',',
                decimal: '.',
                min_digits: 4,
            },
        }
    }

    fn format(self, n: f64) -> String {
        if n.fract() != 0.0 {
            return n.to_string().replace('.', &self.decimal.to_string());
        }
        let digits = (n.abs() as u64).to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && digits.len() >= self.min_digits && (digits.len() - i).is_multiple_of(3) {
                out.push(self.group);
            }
            out.push(c);
        }
        if n < 0.0 { format!("-{out}") } else { out }
    }

    /// One decimal, rounded half up as the UI's `toFixed(1)` and grouped: 212.4 → "212.4" or
    /// "212,4"; 999.95 → "1,000.0".
    fn one_decimal(self, value: f64) -> String {
        let tenths = (value * 10.0).round() as u64;
        format!(
            "{}{}{}",
            self.format((tenths / 10) as f64),
            self.decimal,
            tenths % 10
        )
    }
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
    /// D4: the Mac's first language with a catalog; English otherwise (as the UI picks).
    #[test]
    fn the_language_is_the_macs_first_with_a_catalog() {
        let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(language_for(&tags(&["de-DE", "en-US"])), "en");
        assert_eq!(language_for(&tags(&["en-GB"])), "en");
        assert_eq!(language_for(&tags(&["xx"])), "en");
        assert_eq!(language_for(&tags(&["es-ES", "en-US"])), "es");
        assert_eq!(language_for(&tags(&["de-DE", "es-419"])), "es");
        assert_eq!(language_for(&[]), "en");
    }

    /// Rust shows the words of the same catalogs the UI has: every one is built in.
    #[test]
    fn every_catalog_is_built_in() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/locales");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json") && n != "rust-keys.json")
            .map(|n| n.trim_end_matches(".json").to_string())
            .collect();
        files.sort();
        let mut built_in: Vec<String> = CATALOGS.iter().map(|(tag, _)| tag.to_string()).collect();
        built_in.sort();
        assert_eq!(built_in, files);
    }

    /// Spanish: its words, plurals and number format (as the UI's `Intl.NumberFormat("es")`:
    /// a point groups thousands from five digits on, a comma before decimals).
    #[test]
    fn in_spanish_text_is_spanish() {
        let files = |done: u64, count: u64| {
            crate::msg!("menubar.files", done = done, count = count).in_language("es")
        };
        assert_eq!(files(1, 1), "1 de 1 archivo");
        assert_eq!(files(1284, 12845), "1284 de 12.845 archivos");
        let space = crate::msg!(
            "errors.blocker.notEnoughSpace",
            needed = Size(212_400_000_000),
            available = Size(999)
        );
        assert_eq!(
            space.in_language("es"),
            "No hay espacio suficiente. Espacio necesario: 212,4 GB; disponible: 999 B"
        );
        assert_eq!(
            space.in_language("en"),
            "Not enough space: 212.4 GB needed, 999 B available"
        );
    }

    /// macOS shows its own panels (Open, Save, About) and the .secopy file type in the
    /// app's languages only when the bundle says it has them: each catalog has an `.lproj`.
    #[test]
    fn the_bundle_has_every_language() {
        let app = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plist = std::fs::read_to_string(app.join("Info.plist")).unwrap();
        let conf = std::fs::read_to_string(app.join("tauri.conf.json")).unwrap();
        for (tag, _) in CATALOGS {
            let file = format!(
                r#""Resources/{tag}.lproj/InfoPlist.strings": "./infoplist/{tag}.lproj/InfoPlist.strings""#
            );
            assert!(conf.contains(&file), "tauri.conf.json bundles {tag}.lproj");
            assert!(
                plist.contains(&format!("<string>{tag}</string>")),
                "{tag} in Info.plist"
            );
            let strings = app.join(format!("infoplist/{tag}.lproj/InfoPlist.strings"));
            let text = std::fs::read_to_string(&strings).unwrap_or_default();
            assert!(
                text.contains("\"Secopy settings\" ="),
                "{tag}: the file type's name"
            );
        }
    }

    /// Review of #175: sizes round and group as the UI's `formatBytes` (half up, as
    /// `toFixed`; "1,000.0 KB"), and a regional tag reads its language's words.
    #[test]
    fn sizes_round_and_group_as_the_ui() {
        let size = |bytes: u64, tag: &str| {
            crate::msg!("format.speed", size = Size(bytes)).in_language(tag)
        };
        assert_eq!(size(1250, "es"), "1,3 KB/s");
        assert_eq!(size(1250, "en"), "1.3 KB/s");
        assert_eq!(size(999_950, "en"), "1,000.0 KB/s");
        assert_eq!(size(999_950, "es"), "1000,0 KB/s");
        assert_eq!(
            crate::msg!("menu.file.start").in_language("es-ES"),
            "Empezar"
        );
    }

    /// By hand: the Mac's languages as Secopy reads them (`defaults read -g AppleLanguages`):
    /// `cargo test -p secopy-app --lib mac_languages_by_hand -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn mac_languages_by_hand() {
        let mac = mac_languages();
        eprintln!(
            "the Mac's languages: {mac:?}; Automatic: {}",
            language_for(&mac)
        );
        assert!(!mac.is_empty());
    }

    /// #181: a chosen language wins; Automatic is the Mac's first language with a catalog,
    /// English otherwise; a language without a catalog is Automatic.
    #[test]
    fn resolve_picks_the_choice_then_the_mac() {
        let mac = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(resolve(Some("es"), &mac(&["en-US"])), "es");
        assert_eq!(resolve(None, &mac(&["es-ES"])), "es");
        assert_eq!(resolve(None, &mac(&["el-GR"])), "en");
        assert_eq!(resolve(Some("fr"), &mac(&["es-ES"])), "es");
    }

    /// #181 review focus 2: Automatic reads only the Mac's list, never what Secopy chose.
    #[test]
    fn automatic_looks_past_secopys_own_choice() {
        let _guard = language_guard();
        set_language(Some("en"));
        let mac = vec!["de-DE".to_string(), "es-ES".to_string()];
        assert_eq!(resolve(None, &mac), "es");
        set_language(None);
    }

    #[test]
    fn text_follows_the_language_in_use() {
        let _guard = language_guard();
        set_language(Some("es"));
        assert_eq!(language_in_use(), "es");
        assert_eq!(crate::msg!("menu.file.start").text(), "Empezar");
        set_language(Some("en"));
        assert_eq!(crate::msg!("menu.file.start").text(), "Start");
        set_language(None);
    }

    #[test]
    fn in_english_text_is_english() {
        let _guard = language_guard();
        set_language(Some("en"));
        assert_eq!(crate::msg!("menu.file.start").text(), "Start");
        set_language(None);
    }

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
            available = Size(999)
        );
        assert_eq!(
            space.english(),
            "Not enough space: 212.4 GB needed, 999 B available"
        );
        let nested = crate::msg!(
            "errors.file.readSource",
            why = crate::msg!("errors.os.permissionDenied")
        );
        assert_eq!(nested.english(), "Can’t read the source: Permission denied");
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
