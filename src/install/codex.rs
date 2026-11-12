//! Codex — `config.toml`, `[mcp_servers.gil]`.
//!
//! Codex CLI, the IDE extension and the ChatGPT desktop app read the same file, so one entry
//! serves all three, and the file is edited directly (a user of the desktop app alone may have
//! no `codex` CLI). `toml_edit` keeps every other table, key and comment as written.

use super::{Registration, SERVER_NAME, SERVE_ARGS, file, is_gil_executable, same_path};
use std::path::Path;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const WHAT: &str = "Codex config.toml";
const SECTION: &str = "mcp_servers";

pub(crate) struct Loaded {
    pub doc: DocumentMut,
    pub file: file::Loaded,
}

pub(crate) fn load(path: &Path) -> Result<Loaded, String> {
    let loaded = file::load(path, MAX_CONFIG_BYTES, WHAT)?;
    let doc = parse(&loaded.bytes)?;
    Ok(Loaded { doc, file: loaded })
}

pub(crate) fn parse(bytes: &[u8]) -> Result<DocumentMut, String> {
    let text = file::text_of(bytes, WHAT)?;
    let doc: DocumentMut = text.parse().map_err(|err| format!("{WHAT} could not be parsed: {err}"))?;
    if doc.get(SECTION).is_some_and(|item| !item.is_table_like()) {
        return Err(format!("{WHAT} `{SECTION}` is not a table; left untouched"));
    }
    Ok(doc)
}

fn entry(doc: &DocumentMut) -> Option<&dyn toml_edit::TableLike> {
    doc.get(SECTION)?.as_table_like()?.get(SERVER_NAME)?.as_table_like()
}

pub(crate) fn command_of(doc: &DocumentMut) -> Option<&str> {
    entry(doc)?.get("command")?.as_str()
}

pub(crate) fn classify(doc: &DocumentMut, exe: &Path) -> Registration {
    let Some(table) = entry(doc) else {
        return if doc.get(SECTION).and_then(|s| s.as_table_like()).and_then(|s| s.get(SERVER_NAME)).is_some() {
            Registration::Foreign
        } else {
            Registration::Missing
        };
    };
    let Some(command) = table.get("command").and_then(Item::as_str) else { return Registration::Foreign };
    let args_ok = table.get("args").and_then(Item::as_array)
        .is_some_and(|args| args.iter().map(Value::as_str).eq(SERVE_ARGS.map(Some)));
    let only_known_keys = table.iter().all(|(key, _)| key == "command" || key == "args");
    if same_path(command, &exe.to_string_lossy()) && args_ok && only_known_keys {
        Registration::Ours
    } else if is_gil_executable(command) {
        Registration::Stale
    } else {
        Registration::Foreign
    }
}

/// A TOML string that shows a Windows path as written. Literal (single-quoted) strings carry
/// no escapes, so backslashes survive a later hand edit; a value that cannot be literal falls
/// back to a correctly escaped basic string.
fn path_value(text: &str) -> Value {
    if !text.contains('\'') && !text.contains(['\n', '\r']) {
        if let Ok(doc) = format!("v = '{text}'\n").parse::<DocumentMut>()
            && let Some(value) = doc.get("v").and_then(Item::as_value)
        {
            return value.clone();
        }
    }
    Value::from(text)
}

fn with_registration(doc: &mut DocumentMut, exe: &Path) {
    let section = doc.entry(SECTION).or_insert_with(|| {
        let mut table = Table::new();
        table.set_implicit(true);
        Item::Table(table)
    });
    let Some(section) = section.as_table_like_mut() else { return };
    let mut table = Table::new();
    table.insert("command", Item::Value(path_value(&exe.to_string_lossy())));
    let mut args = Array::new();
    for arg in SERVE_ARGS { args.push(arg); }
    table.insert("args", Item::Value(Value::Array(args)));
    section.insert(SERVER_NAME, Item::Table(table));
}

fn without_registration(doc: &mut DocumentMut) {
    if let Some(section) = doc.get_mut(SECTION).and_then(Item::as_table_like_mut) {
        section.remove(SERVER_NAME);
    }
}

/// Register at `path`. Returns `OK` (already ours), `ADDED`, `UPDATED` or an error.
pub(crate) fn register(path: &Path, exe: &Path, replace: bool) -> Result<&'static str, String> {
    let mut loaded = load(path)?;
    let outcome = match classify(&loaded.doc, exe) {
        Registration::Ours => return Ok("OK"),
        Registration::Missing => "ADDED",
        Registration::Stale => "UPDATED",
        Registration::Foreign if replace => "UPDATED",
        Registration::Foreign => {
            return Err(format!(
                "the existing `gil` entry runs {:?}, which is not GIL; left untouched (use --replace to overwrite)",
                command_of(&loaded.doc).unwrap_or("<no command>")
            ));
        }
    };
    with_registration(&mut loaded.doc, exe);
    file::store(path, &loaded.file, &loaded.doc.to_string(), WHAT)?;
    if classify(&load(path)?.doc, exe) != Registration::Ours {
        return Err("the Codex entry did not read back as expected".into());
    }
    Ok(outcome)
}

/// Remove our entry. Returns `REMOVED`, `MISSING`, or `KEPT` for a foreign entry.
pub(crate) fn unregister(path: &Path) -> Result<String, String> {
    let mut loaded = load(path)?;
    match classify(&loaded.doc, Path::new("")) {
        Registration::Missing => Ok("MISSING".into()),
        Registration::Foreign => Ok("KEPT (not a GIL entry)".into()),
        _ => {
            without_registration(&mut loaded.doc);
            file::store(path, &loaded.file, &loaded.doc.to_string(), WHAT)?;
            Ok(format!("REMOVED ({})", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn exe() -> PathBuf {
        if cfg!(windows) { PathBuf::from(r"C:\Users\Me\AppData\Local\Programs\gil\gil.exe") } else { PathBuf::from("/Users/me/gil/bin/gil") }
    }

    fn config(tag: &str, text: &str) -> PathBuf {
        let dir = super::super::tests::scratch(&format!("codex-{tag}"));
        let path = dir.join("config.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn register_adds_a_table_and_keeps_comments_and_other_servers() {
        let original = "# my codex settings\nmodel = \"gpt-5\"   # keep\n\n[mcp_servers.other]\ncommand = \"node\"\nargs = [\"x.js\"]\n";
        let path = config("add", original);
        assert_eq!(register(&path, &exe(), false).unwrap(), "ADDED");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(original), "formatting preserved:\n{text}");
        assert!(text.contains("[mcp_servers.gil]"), "{text}");
        assert!(text.contains("args = [\"mcp\", \"--serve\"]"), "{text}");
        let doc = parse(text.as_bytes()).unwrap();
        assert_eq!(classify(&doc, &exe()), Registration::Ours);
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("node"));
        let before = std::fs::read(&path).unwrap();
        assert_eq!(register(&path, &exe(), false).unwrap(), "OK");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(unregister(&path).unwrap().starts_with("REMOVED"));
        let doc = parse(std::fs::read(&path).unwrap().as_slice()).unwrap();
        assert_eq!(classify(&doc, &exe()), Registration::Missing);
        assert!(std::fs::read_to_string(&path).unwrap().contains("[mcp_servers.other]"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn windows_paths_are_written_as_literal_strings() {
        let path = config("literal", "");
        let win = PathBuf::from(r"C:\Users\홍 길동\AppData\Local\Programs\gil\gil.exe");
        assert_eq!(register(&path, &win, false).unwrap(), "ADDED");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(r"command = 'C:\Users\홍 길동\AppData\Local\Programs\gil\gil.exe'"), "{text}");
        assert_eq!(classify(&parse(text.as_bytes()).unwrap(), &win), Registration::Ours);
        // An apostrophe forces the escaped basic form, which still round-trips.
        let odd = PathBuf::from(r"C:\Users\O'Neil\gil.exe");
        assert_eq!(register(&path, &odd, false).unwrap(), "UPDATED");
        assert_eq!(classify(&parse(std::fs::read(&path).unwrap().as_slice()).unwrap(), &odd), Registration::Ours);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn stale_gil_entries_are_updated_but_foreign_ones_are_kept() {
        let path = config("stale", "[mcp_servers.gil]\ncommand = 'C:\\GIL\\runtime\\gil-launcher.exe'\nargs = [\"mcp\", \"--serve\"]\n");
        assert_eq!(classify(&load(&path).unwrap().doc, &exe()), Registration::Stale);
        assert_eq!(register(&path, &exe(), false).unwrap(), "UPDATED");
        assert_eq!(classify(&load(&path).unwrap().doc, &exe()), Registration::Ours);

        let foreign = config("foreign", "[mcp_servers.gil]\ncommand = \"node\"\n");
        assert_eq!(classify(&load(&foreign).unwrap().doc, &exe()), Registration::Foreign);
        assert!(register(&foreign, &exe(), false).unwrap_err().contains("not GIL"));
        assert_eq!(unregister(&foreign).unwrap(), "KEPT (not a GIL entry)");
        assert_eq!(register(&foreign, &exe(), true).unwrap(), "UPDATED");
        let inline = config("inline", "mcp_servers = { gil = { command = \"node\" } }\n");
        assert_eq!(classify(&load(&inline).unwrap().doc, &exe()), Registration::Foreign);
        for p in [path, foreign, inline] { let _ = std::fs::remove_dir_all(p.parent().unwrap()); }
    }

    #[test]
    fn broken_files_are_reported_not_overwritten() {
        let path = config("broken", "[mcp_servers\nx = ");
        let err = register(&path, &exe(), false).unwrap_err();
        assert!(err.contains("could not be parsed"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[mcp_servers\nx = ");
        let scalar = config("scalar", "mcp_servers = 1\n");
        assert!(register(&scalar, &exe(), false).unwrap_err().contains("not a table"));
        for p in [path, scalar] { let _ = std::fs::remove_dir_all(p.parent().unwrap()); }
    }
}
