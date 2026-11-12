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
#[serde(deny_unknown_fields)]
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

// Read only single regular control-file entries through the existing pinned-directory helpers.
fn control_file(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    { crate::internal_fs::file(path, false).map(|(_, file)| file) }
    #[cfg(windows)]
    { crate::windows_fs::control_file(path, false).map(|(_, file)| file) }
    #[cfg(not(any(unix, windows)))]
    { let _ = path; Err(std::io::Error::other("locale persistence is unsupported here")) }
}

fn memory_path(root: &Path) -> std::io::Result<std::path::PathBuf> {
    let state = crate::project_state::state_for_root(root).map_err(std::io::Error::other)?;
    // An orphan directory is not an existing Project. Never create a record or follow a link.
    control_file(&state)?;
    Ok(state.parent().ok_or_else(|| std::io::Error::other("no project record here"))?.join(MEMORY_FILE))
}

fn read_memory(path: &Path) -> std::io::Result<Option<Memory>> {
    use std::io::Read;
    let file = match control_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 { return Err(std::io::Error::other("locale sidecar is too large")); }
    let memory: Memory = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    if memory.schema_version != MEMORY_SCHEMA
        || [&memory.start, &memory.app].into_iter().any(|tag| tag.as_deref().is_some_and(|s| Locale::from_tag(s).is_none())) {
        return Err(std::io::Error::other("unsupported locale sidecar; preserved"));
    }
    Ok(Some(memory))
}

/// The language remembered for the Project at `root`, if any was ever reported.
/// Unreadable preferences fall back without repairing or rewriting them.
pub fn remembered(root: &Path) -> Option<Locale> {
    read_memory(&memory_path(root).ok()?).ok().flatten().and_then(|m| m.resolve())
}

/// The language to speak for the Project at `root`: remembered, else [`DEFAULT_LOCALE`].
pub fn for_project(root: &Path) -> Locale { remembered(root).unwrap_or(DEFAULT_LOCALE) }

fn remember(root: &Path, locale: Locale, explicit: bool) -> std::io::Result<()> {
    let path = memory_path(root)?;
    let parent = path.parent().expect("sidecar has a parent");
    // Serialize read/modify/publish with other GIL writers. Busy is best-effort failure for
    // display preferences, never permission to touch the Project's state or Snapshot objects.
    let lock = crate::lock::ProjectLock::acquire(parent).map_err(std::io::Error::other)?;
    memory_path(root)?;
    let mut memory = read_memory(&path)?.unwrap_or_default();
    memory.schema_version = MEMORY_SCHEMA;
    let field = if explicit { &mut memory.start } else { &mut memory.app };
    if field.as_deref() == Some(locale.tag()) { return Ok(()); }
    *field = Some(locale.tag().to_owned());
    let encoded = serde_json::to_vec_pretty(&memory).map_err(std::io::Error::other)?;
    lock.validate(&parent.join(crate::lock::LOCK_FILE))?;
    // This publisher works for a named .gil control file: exclusive no-follow temporary,
    // pinned parent, verified bytes, fsync, and atomic replacement on Unix and Windows.
    // A stale temporary is preserved and refused, never truncated or automatically deleted.
    #[cfg(any(unix, windows))]
    { crate::visual_report::state_file::write(&path, &encoded) }
    #[cfg(not(any(unix, windows)))]
    { let _ = encoded; Err(std::io::Error::other("locale persistence is unsupported here")) }
}

/// Remember the language named by `gil_start`. It outranks later App reports.
pub fn remember_start(root: &Path, locale: Locale) -> std::io::Result<()> { remember(root, locale, true) }

/// Remember the most recent App locale. An unchanged value leaves the file untouched.
pub fn remember_app(root: &Path, locale: Locale) -> std::io::Result<()> { remember(root, locale, false) }

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
        assert!(remember_app(&root, Locale::Ja).is_err(), "an orphan .gil is not a Project");
        std::fs::write(root.join(crate::STATE_PATH), b"existing record fixture").unwrap();
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

    fn memory_fixture(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("gil-locale-{label}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".gil")).unwrap();
        std::fs::write(root.join(crate::STATE_PATH), b"preserved record fixture").unwrap();
        root
    }

    #[test]
    fn locale_updates_preserve_records_and_refuse_busy_or_damaged_preferences() {
        let root = memory_fixture("preserve");
        let state = std::fs::read(root.join(crate::STATE_PATH)).unwrap();
        let path = root.join(".gil").join(MEMORY_FILE);
        let lock = crate::lock::ProjectLock::acquire(&root.join(".gil")).unwrap();
        assert!(remember_start(&root, Locale::Ja).is_err());
        assert!(!path.exists());
        drop(lock);
        remember_start(&root, Locale::Ja).unwrap();
        let before = std::fs::read(&path).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        remember_start(&root, Locale::Ja).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        for damaged in [b"invalid JSON".to_vec(), br#"{"schema_version":2,"start":"en"}"#.to_vec(), vec![b' '; 4097]] {
            std::fs::write(&path, &damaged).unwrap();
            assert_eq!(remembered(&root), None);
            assert!(remember_app(&root, Locale::En).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), damaged);
        }
        assert_eq!(std::fs::read(root.join(crate::STATE_PATH)).unwrap(), state);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn occupied_temporary_and_hardlinked_sidecar_never_change_existing_bytes() {
        let root = memory_fixture("links");
        let state = root.join(crate::STATE_PATH);
        let bytes = std::fs::read(&state).unwrap();
        let tmp = root.join(".gil/locale.json.writing");
        std::fs::write(&tmp, b"preserved interrupted write").unwrap();
        assert!(remember_app(&root, Locale::En).is_err());
        assert_eq!(std::fs::read(&tmp).unwrap(), b"preserved interrupted write");
        assert_eq!(std::fs::read(&state).unwrap(), bytes);
        std::fs::remove_file(&tmp).unwrap();
        // Simulate a leftover entry aliased to a record: it must never be truncated.
        std::fs::hard_link(&state, &tmp).unwrap();
        assert!(remember_app(&root, Locale::En).is_err());
        assert_eq!(std::fs::read(&state).unwrap(), bytes);
        assert_eq!(std::fs::read(&tmp).unwrap(), bytes);
        std::fs::remove_file(&tmp).unwrap();
        let sidecar = root.join(".gil").join(MEMORY_FILE);
        std::fs::hard_link(&state, &sidecar).unwrap();
        assert_eq!(remembered(&root), None);
        assert!(remember_start(&root, Locale::Ja).is_err());
        assert_eq!(std::fs::read(&state).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn linked_preference_and_parent_are_never_followed() {
        use std::os::unix::fs::symlink;
        let root = memory_fixture("symlink");
        let target = root.join("preserved.json");
        std::fs::write(&target, br#"{"schema_version":1,"start":"ja"}"#).unwrap();
        let bytes = std::fs::read(&target).unwrap();
        let sidecar = root.join(".gil").join(MEMORY_FILE);
        symlink(&target, &sidecar).unwrap();
        assert_eq!(remembered(&root), None);
        assert!(remember_start(&root, Locale::En).is_err());
        assert!(std::fs::symlink_metadata(&sidecar).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        std::fs::remove_file(sidecar).unwrap();
        std::fs::rename(root.join(".gil"), root.join("saved-records")).unwrap();
        symlink(root.join("saved-records"), root.join(".gil")).unwrap();
        assert!(remember_app(&root, Locale::En).is_err());
        assert!(!root.join("saved-records").join(MEMORY_FILE).exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_start_and_app_updates_keep_both_fields() {
        let root = memory_fixture("concurrent");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|threads| {
            for explicit in [true, false] {
                let root = &root;
                let barrier = barrier.clone();
                threads.spawn(move || {
                    barrier.wait();
                    let mut saved = false;
                    for _ in 0..1000 {
                        if remember(root, if explicit { Locale::Ja } else { Locale::En }, explicit).is_ok() { saved = true; break; }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    assert!(saved, "bounded test retry after a busy lock");
                });
            }
        });
        assert_eq!(remembered(&root), Some(Locale::Ja));
        let memory = read_memory(&memory_path(&root).unwrap()).unwrap().unwrap();
        assert_eq!(memory.start.as_deref(), Some("ja"));
        assert_eq!(memory.app.as_deref(), Some("en"));
        std::fs::remove_dir_all(root).unwrap();
    }

}
