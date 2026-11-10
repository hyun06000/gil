//! Language selection over the real `gil mcp --serve` process.
//!
//! What this measures: the Core answers in the language it was told — explicitly on `gil_start`,
//! or through the Monitor App's presence report — and remembers it per Project. Nothing here
//! measures a Host screen; a mock Host is the stdio client below.
use std::path::Path;

use gil::Locale;
use gil::locale::text_in;
use serde_json::{Value, json};

mod common;
#[path = "guidance_support/mod.rs"]
mod support;
use support::{Mcp, hints, ok, said, scope_from_start, scratch};

fn poll(mcp: &mut Mcp, scope: &str, sequence: u32, locale: Option<&str>) -> Value {
    let mut args = json!({"scope_id": scope, "instance_id": "app-locale", "sequence": sequence.to_string(), "presentation": "inline"});
    if let Some(tag) = locale { args["locale"] = json!(tag); }
    ok(mcp.call_raw("gil_monitor_poll", args), "gil_monitor_poll")
}

/// The sidecar beside the record. The product record may live outside the working tree on
/// Windows, so the file itself is only inspected where its place is known.
#[cfg(not(windows))]
fn remembered(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join(".gil").join(gil::locale::MEMORY_FILE)).expect("locale sidecar")).unwrap()
}

/// The guidance block that every receipt of that tool carries, in the expected language.
fn speaks(result: &Value, locale: Locale) -> bool {
    let all = format!("{}\n{}", hints(result), said(result));
    all.contains(text_in(locale, "guide.file_boundary")) || all.contains(text_in(locale, "guide.start"))
}

#[test]
fn an_explicit_start_locale_makes_receipts_and_guidance_speak_english_and_is_remembered() {
    let settings = scratch("locale-en-settings");
    let root = scratch("locale-en-project");
    let mut mcp = Mcp::serve(&settings);
    let started = ok(mcp.call("gil_start", &root, json!({"locale": "en-US"})), "gil_start");
    assert!(said(&started).starts_with("Started the project."), "{}", said(&started));
    assert!(hints(&started).contains("GIL start: this folder is now a GIL project."), "{}", hints(&started));
    assert!(hints(&started).contains("GIL start succeeded and the Monitor is prepared."), "{}", hints(&started));
    assert!(!hints(&started).contains("프로젝트를 시작했다"), "no Korean leaks into an English receipt");
    #[cfg(not(windows))]
    assert_eq!(remembered(&root), json!({"schema_version": 1, "start": "en"}));

    // Later calls carry no locale: the Project remembers.
    let context = ok(mcp.call("gil_context", &root, json!({})), "gil_context");
    assert!(hints(&context).contains("GIL resume: when continuing this project"), "{}", hints(&context));
    assert!(said(&context).contains("[Call now]\nNew Interview: open the first question with gil_open(kind: question)"), "{}", said(&context));
    assert!(speaks(&context, Locale::En));
    let status = ok(mcp.call("gil_status", &root, json!({})), "gil_status");
    assert!(said(&status).contains("Position: nothing opened yet") && said(&status).contains("Next: can open — question"), "{}", said(&status));

    // The English receipt is parsed by the English reader: a refused open names the recovery in English.
    let opened = ok(mcp.call("gil_open", &root, json!({"kind": "question", "contract": "objective: ask the goal\nnext_action: ask the user\ndone_when: an answer arrives\n"})), "gil_open");
    assert!(said(&opened).starts_with("Opened: step:C1/S1 · question"), "{}", said(&opened));
    assert!(hints(&opened).contains("GIL progress: a Question is open."), "{}", hints(&opened));
    mcp.done();
}

#[test]
fn without_any_locale_the_core_keeps_its_korean_default() {
    let settings = scratch("locale-default-settings");
    let root = scratch("locale-default-project");
    let mut mcp = Mcp::serve(&settings);
    let started = ok(mcp.call("gil_start", &root, json!({})), "gil_start");
    assert!(said(&started).starts_with("프로젝트를 시작했다."), "{}", said(&started));
    assert!(hints(&started).contains("GIL 시작: 이 폴더는 이제 GIL 프로젝트다."), "{}", hints(&started));
    #[cfg(not(windows))]
    assert!(!root.join(".gil").join(gil::locale::MEMORY_FILE).exists(), "nothing to remember was said");
    let context = ok(mcp.call("gil_context", &root, json!({})), "gil_context");
    assert!(speaks(&context, Locale::Ko));
    assert!(said(&context).contains("[지금 바로 할 호출]"), "{}", said(&context));
    mcp.done();
}

#[test]
fn the_apps_host_locale_is_remembered_but_an_explicit_start_locale_outranks_it() {
    let settings = scratch("locale-app-settings");
    let mut mcp = Mcp::serve(&settings);

    // A Project started without a language follows the Monitor App's Host locale.
    let followed = scratch("locale-app-follow");
    let started = ok(mcp.call("gil_start", &followed, json!({})), "gil_start");
    let scope = scope_from_start(&started);
    poll(&mut mcp, &scope, 1, Some("zh-TW"));
    #[cfg(not(windows))]
    assert_eq!(remembered(&followed), json!({"schema_version": 1, "app": "zh-Hant"}));
    let context = ok(mcp.call("gil_context", &followed, json!({})), "gil_context");
    assert!(speaks(&context, Locale::ZhHant), "{}", hints(&context));
    // An older App that sends no locale changes nothing; a new Host locale replaces the old one.
    poll(&mut mcp, &scope, 2, None);
    assert!(speaks(&ok(mcp.call("gil_context", &followed, json!({})), "gil_context"), Locale::ZhHant));
    poll(&mut mcp, &scope, 3, Some("ja-JP"));
    assert!(speaks(&ok(mcp.call("gil_context", &followed, json!({})), "gil_context"), Locale::Ja));

    // A Project whose start named the language keeps it whatever the App reports.
    let fixed = scratch("locale-app-fixed");
    let started = ok(mcp.call("gil_start", &fixed, json!({"locale": "ja-JP"})), "gil_start");
    let scope = scope_from_start(&started);
    assert!(speaks(&started, Locale::Ja), "{}", hints(&started));
    poll(&mut mcp, &scope, 1, Some("en-US"));
    #[cfg(not(windows))]
    assert_eq!(remembered(&fixed), json!({"schema_version": 1, "start": "ja", "app": "en"}));
    assert!(speaks(&ok(mcp.call("gil_context", &fixed, json!({})), "gil_context"), Locale::Ja));
    mcp.done();
}

#[test]
fn locale_tags_are_validated_by_the_schema_and_unknown_languages_fall_back_to_english() {
    let settings = scratch("locale-schema-settings");
    let root = scratch("locale-schema-project");
    let mut mcp = Mcp::serve(&settings);
    for bad in ["x", "ko KR", "", "1ko", &"a".repeat(36)] {
        let frame = mcp.call_frame("gil_start", &root, json!({"locale": bad}));
        assert!(frame.get("error").is_some(), "{bad:?} was accepted: {frame}");
    }
    #[cfg(not(windows))]
    assert!(!root.join(".gil").exists(), "a refused start creates nothing");
    let started = ok(mcp.call("gil_start", &root, json!({"locale": "fr-FR"})), "gil_start");
    assert!(said(&started).starts_with("Started the project."), "{}", said(&started));
    #[cfg(not(windows))]
    assert_eq!(remembered(&root), json!({"schema_version": 1, "start": "en"}));
    // The poll schema bounds the field the same way; a bad tag never reaches the memory.
    let scope = scope_from_start(&started);
    let frame = mcp.request("tools/call", json!({"name": "gil_monitor_poll", "arguments": {"scope_id": scope, "instance_id": "app-locale", "sequence": "1", "presentation": "inline", "locale": "ko KR"}}));
    assert!(frame.get("error").is_some(), "{frame}");
    mcp.done();
}
