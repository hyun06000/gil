//! Interruption points of the transition that only the crate can reach (contract §7.3 table).
//! Synthetic records in per-process temporary folders; never a real user project.
use super::*;
use crate::store::CHAIN_FORMAT;
use crate::{ProjectSession, STATE_PATH};
use std::collections::BTreeMap;

fn scratch(label: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("gil-unit-transition-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).unwrap();
    at
}

fn rules() -> RuleSet {
    RuleSet::builtin().unwrap()
}

/// A started project rewritten into the historical format 6 shape. A fresh start has no ended
/// Will, which is enough for the file-handling points measured here.
fn format_6(label: &str) -> (PathBuf, PathBuf) {
    let root = scratch(label);
    std::fs::write(root.join("work.txt"), "user file").unwrap();
    let state = root.join(STATE_PATH);
    let session = ProjectSession::start(rules(), &state).unwrap();
    session.commit().unwrap();
    drop(session);
    let mut file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    assert_eq!(file["format"], serde_norway::Value::from(END_HISTORY_FORMAT));
    file["format"] = serde_norway::Value::from(CHAIN_FORMAT);
    for (_, existence) in file["existences"].as_mapping_mut().unwrap() {
        let journey = existence["journey"].as_mapping_mut().unwrap();
        let ended = journey.remove("ended_wills").unwrap();
        assert!(ended.as_sequence().unwrap().is_empty());
        journey.insert("done_wills".into(), ended);
    }
    std::fs::write(&state, serde_norway::to_string(&file).unwrap()).unwrap();
    (root, state)
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                out.insert(name, None);
                walk(root, &path, out);
            } else {
                out.insert(name, Some(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

struct Hook;
impl Hook {
    fn set(act: fn(&str) -> io::Result<()>) -> Self {
        hooks::AT.with(|c| c.set(Some(act)));
        Hook
    }
}
impl Drop for Hook {
    fn drop(&mut self) {
        hooks::AT.with(|c| c.set(None));
    }
}

fn approved(checked: &Value) -> (u32, u32, String, String) {
    let with = &checked["apply_with"];
    (with["current_format"].as_u64().unwrap() as u32, with["target_format"].as_u64().unwrap() as u32,
     with["state_sha256"].as_str().unwrap().to_owned(), with["proposal_sha256"].as_str().unwrap().to_owned())
}

fn run(root: &Path, state: &Path, checked: &Value) -> Result<Value, Refusal> {
    let (current_format, target_format, digest, proposal) = approved(checked);
    apply(rules(), root, state, &Approval {
        current_format, target_format, state_sha256: &digest, proposal_sha256: &proposal, user_approved: true,
    })
}

#[test]
fn an_interruption_before_the_replacement_keeps_format_6_and_the_copy_is_reused() {
    let (root, state) = format_6("before-publish");
    let original = std::fs::read(&state).unwrap();
    let checked = check(rules(), &root, &state).unwrap();
    assert_eq!(checked["status"], "ready", "{checked}");
    let copy = root.join(".gil").join(checked["proposal"]["copy_file"].as_str().unwrap().trim_start_matches(".gil/"));

    let stopped = {
        let _hook = Hook::set(|stage| if stage == "before_publish" { Err(io::Error::other("simulated stop")) } else { Ok(()) });
        run(&root, &state, &checked).unwrap_err()
    };
    assert_eq!((stopped.code, stopped.class), ("apply_interrupted", Class::NotApplied), "{stopped:?}");
    assert_eq!(std::fs::read(&state).unwrap(), original, "state.yaml changed before the commit point");
    assert_eq!(std::fs::read(&copy).unwrap(), original, "the copy must hold the original bytes");
    let before_retry = tree(&root);

    // Checking again sees the same proposal and an equal copy; nothing is written by the check.
    let again = check(rules(), &root, &state).unwrap();
    assert_eq!(again["proposal_sha256"], checked["proposal_sha256"]);
    assert_eq!(again["proposal"]["copy_present"], "present_matching");
    assert_eq!(tree(&root), before_retry);

    let applied = run(&root, &state, &again).unwrap();
    assert_eq!(applied["copy"], "reused", "{applied}");
    assert_eq!(applied["storage_format"], END_HISTORY_FORMAT);
    assert_eq!(std::fs::read(&copy).unwrap(), original, "a reused copy is never rewritten");
    let reread = store::load(rules(), &state).unwrap();
    assert!(reread.records_end_results());
}

#[test]
fn a_stop_after_the_copy_is_synced_leaves_format_6_and_the_complete_copy() {
    let (root, state) = format_6("copy-synced");
    let original = std::fs::read(&state).unwrap();
    let checked = check(rules(), &root, &state).unwrap();
    let failed = {
        let _hook = Hook::set(|stage| if stage == "copy_synced" { Err(io::Error::other("simulated crash")) } else { Ok(()) });
        run(&root, &state, &checked).unwrap_err()
    };
    assert_eq!((failed.code, failed.class), ("copy_failed", Class::NotApplied), "{failed:?}");
    assert_eq!(std::fs::read(&state).unwrap(), original);
    let applied = run(&root, &state, &check(rules(), &root, &state).unwrap()).unwrap();
    assert_eq!(applied["copy"], "reused");
}

#[test]
fn a_failure_after_the_replacement_is_reported_unconfirmed_and_a_resend_writes_nothing() {
    let (root, state) = format_6("after-publish");
    let original = std::fs::read(&state).unwrap();
    let checked = check(rules(), &root, &state).unwrap();
    let uncertain = {
        let _hook = Hook::set(|stage| if stage == "after_publish" { Err(io::Error::other("simulated lost confirmation")) } else { Ok(()) });
        run(&root, &state, &checked).unwrap_err()
    };
    assert_eq!((uncertain.code, uncertain.class), ("apply_unconfirmed", Class::Unconfirmed), "{uncertain:?}");
    // The replacement did happen: the result is read from the record, never guessed.
    let converted = std::fs::read(&state).unwrap();
    assert_ne!(converted, original);
    assert!(String::from_utf8_lossy(&converted).starts_with("format: 8\n"));
    let after = tree(&root);

    let read = check(rules(), &root, &state).unwrap();
    assert_eq!(read["status"], "already_transitioned", "{read}");
    let resent = run(&root, &state, &checked).unwrap_err();
    assert_eq!(resent.code, "already_transitioned");
    assert_eq!(resent.facts["copy_for_request"], "present_matching", "{:?}", resent.facts);
    assert_eq!(resent.facts["state_is_that_transition"], true, "{:?}", resent.facts);
    assert_eq!(tree(&root), after, "a resend wrote or copied again");
}

#[test]
fn approval_and_transition_pairs_are_refused_before_anything_is_read() {
    let (root, state) = format_6("pairs");
    let before = tree(&root);
    let checked = check(rules(), &root, &state).unwrap();
    let (_, _, digest, proposal) = approved(&checked);
    let ask = |current_format, target_format, user_approved| apply(rules(), &root, &state, &Approval {
        current_format, target_format, state_sha256: &digest, proposal_sha256: &proposal, user_approved,
    }).unwrap_err().code;
    assert_eq!(ask(6, 8, false), "approval_required");
    for (from, to) in [(4, 6), (5, 7), (6, 9), (7, 8), (8, 6), (9, 7), (6, 10), (7, 11), (8, 10), (4, 8)] {
        assert_eq!(ask(from, to, true), "unsupported_transition", "{from} → {to}");
    }
    assert_eq!(tree(&root), before);
}
