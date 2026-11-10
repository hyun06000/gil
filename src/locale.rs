//! User-facing language selection.
//!
//! GIL speaks to people in their own language. The record itself never changes with the
//! language: Reports, references and the stored state are what the user and the Agent wrote.
//! Only the words that GIL adds — receipts, guidance, refusals, the Monitor's labels — come
//! from the language tables in `src/i18n/`.
//!
//! # One normalization rule
//!
//! A Host hands over a BCP 47 tag (`ko-KR`, `zh-Hant-TW`, `en_US`). It is reduced to one of
//! five supported languages by [`Locale::normalize`]. The same table drives the Monitor App
//! (`ui/locale.js`); both sides are checked against `src/i18n/locale-cases.json`.
//!
//! # Where the language comes from
//!
//! The MCP initialize handshake carries no locale, so the Core learns it from two optional
//! fields and remembers the answer per Project in a small sidecar file next to the record
//! (`.gil/locale.json`). The record format number does not change.
//!
//! ```text
//! gil_start(locale)        explicit, from the Agent who knows the user's language   (highest)
//! gil_monitor_poll(locale) the App's own Host locale, reported with its presence
//! DEFAULT_LOCALE           when nothing was ever reported
//! ```
//!
//! # Lookup
//!
//! [`text`] reads one key in the current thread's locale, falling back to English and finally
//! to the key itself; a missing translation can never panic a tool call. The current locale is
//! a thread-local set by [`with`] at the MCP dispatch boundary, so deep code (`command`,
//! `context`, `monitor`) does not need a parameter it would otherwise thread through every
//! function.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The supported user-facing languages. The wire form is the BCP 47 tag from [`Locale::tag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale { En, Ko, ZhHans, ZhHant, Ja }

/// The Core's language when no Host, App or Agent has said anything. Change it here only.
pub const DEFAULT_LOCALE: Locale = Locale::Ko;

impl Locale {
    pub const ALL: [Locale; 5] = [Locale::En, Locale::Ko, Locale::ZhHans, Locale::ZhHant, Locale::Ja];

    /// The canonical tag: script-level BCP 47.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::En => "en", Locale::Ko => "ko", Locale::ZhHans => "zh-Hans",
            Locale::ZhHant => "zh-Hant", Locale::Ja => "ja",
        }
    }

    /// Reduce any BCP 47 tag (also `_`-separated POSIX forms) to a supported language.
    ///
    /// `zh` chooses its script from an explicit script subtag first, else from the region
    /// (TW/HK/MO → Traditional), else Simplified. Unknown languages and unparsable input are
    /// English, the reference language of every table.
    pub fn normalize(tag: &str) -> Locale {
        let mut parts = tag.trim().split(['-', '_']).filter(|p| !p.is_empty());
        let Some(language) = parts.next() else { return Locale::En; };
        let language = language.to_ascii_lowercase();
        match language.as_str() {
            "en" => Locale::En,
            "ko" => Locale::Ko,
            "ja" => Locale::Ja,
            "zh" => {
                let rest: Vec<String> = parts.map(str::to_ascii_lowercase).collect();
                if let Some(script) = rest.iter().find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_alphabetic())) {
                    return match script.as_str() {
                        "hant" => Locale::ZhHant,
                        _ => Locale::ZhHans,
                    };
                }
                match rest.iter().find(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_alphabetic())).map(String::as_str) {
                    Some("tw" | "hk" | "mo") => Locale::ZhHant,
                    _ => Locale::ZhHans,
                }
            }
            _ => Locale::En,
        }
    }

    /// Only the five canonical tags, as written by [`Locale::tag`] (case-insensitive).
    pub fn from_tag(tag: &str) -> Option<Locale> {
        Locale::ALL.into_iter().find(|l| l.tag().eq_ignore_ascii_case(tag.trim()))
    }

    fn index(self) -> usize {
        match self { Locale::En => 0, Locale::Ko => 1, Locale::ZhHans => 2, Locale::ZhHant => 3, Locale::Ja => 4 }
    }
}

impl Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.tag()) }
}

// ── current locale ──────────────────────────────────────────────────────────

thread_local! {
    static CURRENT: Cell<Locale> = const { Cell::new(DEFAULT_LOCALE) };
}

/// The locale of the current thread. [`DEFAULT_LOCALE`] unless [`with`] is active.
pub fn current() -> Locale { CURRENT.with(Cell::get) }

/// Run `work` with `locale` as the current locale, then restore the previous one.
pub fn with<R>(locale: Locale, work: impl FnOnce() -> R) -> R {
    struct Restore(Locale);
    impl Drop for Restore { fn drop(&mut self) { CURRENT.with(|c| c.set(self.0)); } }
    let _restore = Restore(CURRENT.with(|c| c.replace(locale)));
    work()
}

// ── tables ──────────────────────────────────────────────────────────────────

const SOURCES: [&str; 5] = [
    include_str!("i18n/en.json"),
    include_str!("i18n/ko.json"),
    include_str!("i18n/zh-Hans.json"),
    include_str!("i18n/zh-Hant.json"),
    include_str!("i18n/ja.json"),
];

static TABLES: [OnceLock<HashMap<String, String>>; 5] = [const { OnceLock::new() }; 5];

/// Every key of one language. Parsing happens once; a malformed bundled table is a build
/// defect, so it panics loudly instead of silently speaking the key names.
pub fn table(locale: Locale) -> &'static HashMap<String, String> {
    TABLES[locale.index()].get_or_init(|| {
        serde_json::from_str(SOURCES[locale.index()])
            .unwrap_or_else(|err| panic!("bundled language table {} is malformed: {err}", locale.tag()))
    })
}

/// One sentence in `locale`, with English as the fallback and the key as the last resort.
pub fn text_in(locale: Locale, key: &str) -> &'static str {
    if let Some(found) = table(locale).get(key) { return found; }
    if locale != Locale::En {
        if let Some(found) = table(Locale::En).get(key) { return found; }
    }
    // A missing key must not fail a tool call. Tests check the tables for completeness.
    Box::leak(key.to_owned().into_boxed_str())
}

/// One sentence in the current locale.
pub fn text(key: &str) -> &'static str { text_in(current(), key) }

/// Replace `{name}` placeholders. Unknown placeholders and other braces stay as written.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in args {
        let needle = format!("{{{name}}}");
        if out.contains(&needle) { out = out.replace(&needle, &value.to_string()); }
    }
    out
}

/// `tr!("key")` — the sentence as an owned `String`; `tr!("key", a = x, b = y)` fills `{a}`/`{b}`.
#[macro_export]
macro_rules! tr {
    ($key:literal) => { $crate::locale::text($key).to_string() };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::locale::fill($crate::locale::text($key), &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

// ── per-Project memory ──────────────────────────────────────────────────────

/// File name of the sidecar beside the record (`.gil/locale.json`). Not part of the record
/// format: a Project without it is simply one that was never told a language.
pub const MEMORY_FILE: &str = "locale.json";
const MEMORY_SCHEMA: u32 = 1;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Memory {
    schema_version: u32,
    /// The language the Agent named in `gil_start(locale)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    /// The most recent Host locale the Monitor App reported with its presence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app: Option<String>,
}

impl Memory {
    fn resolve(&self) -> Option<Locale> {
        self.start.as_deref().and_then(Locale::from_tag)
            .or_else(|| self.app.as_deref().and_then(Locale::from_tag))
    }
}

fn memory_path(root: &Path) -> Option<std::path::PathBuf> {
    let state = crate::project_state::state_for_root(root).ok()?;
    // Only a Project that exists has a language to remember; never create `.gil/` here.
    state.parent().filter(|dir| dir.is_dir()).map(|dir| dir.join(MEMORY_FILE))
}

fn read_memory(root: &Path) -> Option<Memory> {
    let path = memory_path(root)?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 4096 { return None; }
    serde_json::from_slice::<Memory>(&bytes).ok().filter(|m| m.schema_version == MEMORY_SCHEMA)
}

fn write_memory(root: &Path, memory: &Memory) -> std::io::Result<()> {
    let Some(path) = memory_path(root) else { return Err(std::io::Error::other("no project record here")); };
    let encoded = serde_json::to_vec_pretty(memory).map_err(std::io::Error::other)?;
    let tmp = path.with_file_name(format!("{MEMORY_FILE}.writing"));
    std::fs::write(&tmp, encoded)?;
    std::fs::rename(&tmp, &path)
}

/// The language remembered for the Project at `root`, if any was ever reported.
pub fn remembered(root: &Path) -> Option<Locale> { read_memory(root).and_then(|m| m.resolve()) }

/// The language to speak for the Project at `root`: remembered, else [`DEFAULT_LOCALE`].
pub fn for_project(root: &Path) -> Locale { remembered(root).unwrap_or(DEFAULT_LOCALE) }

/// Remember the language named by `gil_start`. It outranks later App reports.
pub fn remember_start(root: &Path, locale: Locale) -> std::io::Result<()> {
    let mut memory = read_memory(root).unwrap_or_default();
    memory.schema_version = MEMORY_SCHEMA;
    if memory.start.as_deref() == Some(locale.tag()) { return Ok(()); }
    memory.start = Some(locale.tag().to_owned());
    write_memory(root, &memory)
}

/// Remember the most recent locale the Monitor App reported. A no-op when unchanged, so a
/// quiet App polling every two seconds never rewrites the file.
pub fn remember_app(root: &Path, locale: Locale) -> std::io::Result<()> {
    let mut memory = read_memory(root).unwrap_or_default();
    memory.schema_version = MEMORY_SCHEMA;
    if memory.app.as_deref() == Some(locale.tag()) { return Ok(()); }
    memory.app = Some(locale.tag().to_owned());
    write_memory(root, &memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Languages whose tables must carry every English key. Extended as translations land.
    const COMPLETE: [Locale; 5] = Locale::ALL;

    #[test]
    fn the_shared_case_table_normalizes_on_the_rust_side() {
        let cases: serde_json::Value = serde_json::from_str(include_str!("i18n/locale-cases.json")).unwrap();
        let cases = cases["cases"].as_array().expect("cases");
        assert!(cases.len() >= 40, "the case table shrank");
        for case in cases {
            let input = case[0].as_str();
            let expected = case[1].as_str().unwrap();
            let got = input.map(Locale::normalize).unwrap_or(Locale::En);
            assert_eq!(got.tag(), expected, "{input:?}");
        }
    }

    #[test]
    fn canonical_tags_round_trip_and_nothing_else_parses_exactly() {
        for locale in Locale::ALL {
            assert_eq!(Locale::from_tag(locale.tag()), Some(locale));
            assert_eq!(Locale::normalize(locale.tag()), locale);
            assert_eq!(Locale::from_tag(&locale.tag().to_uppercase()), Some(locale));
        }
        for tag in ["ko-KR", "zh-TW", "zh", "", "xx"] { assert_eq!(Locale::from_tag(tag), None, "{tag}"); }
    }

    #[test]
    fn every_language_table_has_exactly_the_english_keys() {
        let english = table(Locale::En);
        assert!(!english.is_empty());
        for locale in COMPLETE {
            if locale == Locale::En { continue; }
            let other = table(locale);
            let mut missing: Vec<&String> = english.keys().filter(|k| !other.contains_key(*k)).collect();
            let mut extra: Vec<&String> = other.keys().filter(|k| !english.contains_key(*k)).collect();
            missing.sort(); extra.sort();
            assert!(missing.is_empty(), "{} lacks {} key(s): {missing:?}", locale.tag(), missing.len());
            assert!(extra.is_empty(), "{} has {} key(s) unknown to en: {extra:?}", locale.tag(), extra.len());
            for (key, value) in other {
                assert!(!value.trim().is_empty(), "{}:{key} is empty", locale.tag());
            }
        }
    }

    #[test]
    fn placeholders_match_between_english_and_every_translation() {
        fn names(s: &str) -> std::collections::BTreeSet<String> {
            let re = regex::Regex::new(r"\{([a-z_][a-z0-9_]*)\}").unwrap();
            re.captures_iter(s).map(|c| c[1].to_owned()).collect()
        }
        let english = table(Locale::En);
        for locale in COMPLETE {
            for (key, value) in table(locale) {
                if let Some(reference) = english.get(key) {
                    assert_eq!(names(value), names(reference), "{}:{key} placeholders differ", locale.tag());
                }
            }
        }
    }

    #[test]
    fn every_key_used_in_rust_source_exists_in_the_english_table() {
        let re = regex::Regex::new(r#"(?:\btr!\(|locale::text\(|locale::text_in\([^,]+,\s*)\s*"([A-Za-z0-9_.-]+)""#).unwrap();
        let english = table(Locale::En);
        let mut missing = Vec::new();
        for entry in walk(Path::new(env!("CARGO_MANIFEST_DIR")).join("src")) {
            if entry.ends_with("locale.rs") { continue; }
            let source = std::fs::read_to_string(&entry).unwrap();
            for found in re.captures_iter(&source) {
                let key = &found[1];
                if !english.contains_key(key) && !key.ends_with('.') {
                    missing.push(format!("{}: {key}", entry.display()));
                }
            }
        }
        assert!(missing.is_empty(), "keys used but not in src/i18n/en.json:\n{}", missing.join("\n"));
    }

    fn walk(dir: std::path::PathBuf) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() { out.extend(walk(path)); }
            else if path.extension().is_some_and(|e| e == "rs") { out.push(path); }
        }
        out
    }

    #[test]
    fn lookup_falls_back_to_english_then_to_the_key_and_fill_replaces_named_slots() {
        assert_eq!(text_in(Locale::Ko, "no.such.key"), "no.such.key");
        assert_eq!(text_in(Locale::En, "no.such.key"), "no.such.key");
        assert_eq!(fill("{a} and {b} and {a} {c}", &[("a", &1), ("b", &"two")]), "1 and two and 1 {c}");
        let said = with(Locale::En, || tr!("locale.self_test", n = 3));
        assert_eq!(said, "self test 3");
        assert_eq!(with(Locale::Ko, || tr!("locale.self_test", n = 3)), "자체 시험 3");
    }

    #[test]
    fn the_current_locale_is_scoped_and_restored() {
        assert_eq!(current(), DEFAULT_LOCALE);
        with(Locale::Ja, || {
            assert_eq!(current(), Locale::Ja);
            with(Locale::ZhHant, || assert_eq!(current(), Locale::ZhHant));
            assert_eq!(current(), Locale::Ja);
        });
        assert_eq!(current(), DEFAULT_LOCALE);
    }

    #[test]
    fn project_memory_prefers_the_start_locale_over_the_app_and_never_creates_a_record() {
        let root = std::env::temp_dir().join(format!("gil-locale-memory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(remembered(&root), None);
        assert!(remember_app(&root, Locale::Ja).is_err(), "no .gil → nothing to remember");
        assert!(!root.join(".gil").exists());
        std::fs::create_dir_all(root.join(".gil")).unwrap();
        remember_app(&root, Locale::ZhHant).unwrap();
        assert_eq!(remembered(&root), Some(Locale::ZhHant));
        assert_eq!(for_project(&root), Locale::ZhHant);
        remember_start(&root, Locale::Ja).unwrap();
        assert_eq!(remembered(&root), Some(Locale::Ja), "the explicit start locale outranks the App");
        remember_app(&root, Locale::En).unwrap();
        assert_eq!(remembered(&root), Some(Locale::Ja));
        let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(".gil").join(MEMORY_FILE)).unwrap()).unwrap();
        assert_eq!(stored, serde_json::json!({"schema_version": 1, "start": "ja", "app": "en"}));
        assert!(!root.join(".gil").join(format!("{MEMORY_FILE}.writing")).exists());
        std::fs::write(root.join(".gil").join(MEMORY_FILE), b"not json").unwrap();
        assert_eq!(remembered(&root), None, "a damaged sidecar is ignored, not fatal");
        assert_eq!(for_project(&root), DEFAULT_LOCALE);
        std::fs::remove_dir_all(&root).ok();
    }
}
