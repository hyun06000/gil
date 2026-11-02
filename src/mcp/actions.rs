//! Typed public action boundary. Commands and receipts are shared with the CLI.
//! No subprocess, ambient cwd, report-field replica, or prose interpretation.
use std::path::Path;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

pub const NAMES: &[&str] = &["gil_status", "gil_context", "gil_story", "gil_help", "gil_start",
    "gil_open", "gil_close", "gil_restore", "gil_revisit", "gil_cycle"];

fn result(ok: bool, exit: Option<i32>, said: String, problem: String) -> CallToolResult {
    let mut out = CallToolResult::success(vec![ContentBlock::text(format!("{said}{problem}"))]);
    out.structured_content = Some(json!({"ok": ok, "exit_code": exit, "said": said, "problem": problem}));
    out.is_error = Some(!ok);
    out
}

pub fn refusal(said: impl Into<String>) -> CallToolResult {
    result(false, None, String::new(), said.into())
}

/// A refusal whose text carries a stable bracketed code (for example the Windows reopen refusal
/// `[storage_protection_lost]`) gets that code as a structured field, so the Skill can apply one
/// rule to it instead of reading prose.
#[allow(unused_mut)]
fn annotate_code(mut out: CallToolResult, problem: &str) -> CallToolResult {
    #[cfg(windows)]
    if problem.contains(&format!("[{}]", crate::windows_fs::PROTECTION_LOST_CODE)) {
        if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
            fields.insert("code".into(), json!(crate::windows_fs::PROTECTION_LOST_CODE));
            // Exact meaning: no record was modified, repaired, removed or replaced, and no
            // recovery or reaping ran. The check runs before the lock is taken; if it fails only
            // on the re-check after the lock, `project.lock` may have been opened or updated.
            fields.insert("records_preserved".into(), json!(true));
            fields.insert("preservation_scope".into(), json!("No record was modified, repaired, removed or replaced and no recovery or reaping ran. The check precedes the lock; if it failed on the re-check after the lock, only .gil/project.lock may have been opened or updated."));
            fields.insert("next".into(), json!("Do not ask the user to change ACLs, inheritance, ownership or security settings; GIL never repairs records. Do not conclude that this is a sync folder from a permission error alone, and do not replace this Project with a new one. Report GIL's words; only if the user chooses to continue elsewhere, offer gil_prepare_workspace with their approval."));
        }
    }
    let _ = problem;
    out
}

fn positional(value: Option<&str>, what: &str, required: bool) -> Result<Option<String>, String> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return if required { Err(format!("{what} 가 필요하다")) } else { Ok(None) };
    };
    if value.starts_with('-') { return Err(format!("{what} 는 '-' 로 시작할 수 없다")); }
    if value.contains(['\n', '\r', '\0']) { return Err(format!("{what} 에 줄바꿈이나 NUL 이 있다")); }
    // JS String.length counts UTF-16 code units, not UTF-8 bytes or scalar values.
    if value.encode_utf16().count() > 200 { return Err(format!("{what} 가 너무 길다")); }
    Ok(Some(value.to_owned()))
}

pub fn call(name: &str, input: &Value) -> CallToolResult {
    let command = match name {
        "gil_status" => "status", "gil_context" => "context", "gil_story" => "story",
        "gil_help" => "help", "gil_start" => "start", "gil_open" => "open",
        "gil_close" => "close", "gil_restore" => "restore", "gil_revisit" => "revisit",
        "gil_cycle" => "cycle", _ => return refusal("허용되지 않은 GIL 동작"),
    };
    let given = input.get("project_root").and_then(Value::as_str);
    let mut resolved: Option<String> = None;
    let mut normalized_note: Option<String> = None;
    if name != "gil_help" || given.is_some() {
        let Some(given) = given.filter(|r| !r.is_empty()) else { return refusal("Project 자리가 비었다"); };
        if !Path::new(given).is_absolute() { return refusal(crate::RootError::NotAbsolute.to_string()); }
        match resolve_root(given) {
            Ok((used, note)) => { resolved = Some(used); normalized_note = note; }
            Err(said) => return refusal(said),
        }
    }
    let root = resolved.as_deref();
    let announce = |mut out: CallToolResult| -> CallToolResult {
        if let Some(note) = &normalized_note {
            if out.is_error != Some(true) {
                if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
                    let said = format!("{note}\n{}", fields.get("said").and_then(Value::as_str).unwrap_or(""));
                    fields.insert("said".into(), json!(said));
                    let problem = fields.get("problem").and_then(Value::as_str).unwrap_or("").to_owned();
                    out.content = vec![ContentBlock::text(format!("{said}{problem}"))];
                }
            }
        }
        out
    };
    let mut args = vec![command.to_owned()];
    #[cfg(feature = "visual-authoring")]
    if matches!(name, "gil_status" | "gil_context") {
        return announce(match crate::command::visual_read_action(Path::new(root.expect("validated root")), command) {
            Ok(said) => result(true, Some(0), said, String::new()),
            Err(problem) => annotate_code(result(false, Some(1), String::new(), format!("{problem}\n")), &problem),
        });
    }
    #[cfg(feature = "visual-authoring")]
    if name == "gil_start" {
        return announce(match crate::command::start_visual_action(Path::new(root.expect("validated root"))) {
            Ok(said) => {
                let mut out = result(true, Some(0), said, String::new());
                if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
                    fields.insert("storage_format".into(), json!(if cfg!(feature = "chain-core") { crate::CHAIN_VISUAL_FORMAT } else { crate::VISUAL_FORMAT }));
                    fields.insert("display_confirmed".into(), json!(false));
                    #[cfg(windows)]
                    fields.insert("record_storage".into(), json!("private_app_data"));
                }
                out
            }
            Err(problem) => annotate_code(result(false, Some(1), String::new(), format!("{problem}\n")), &problem),
        });
    }
    let slots: &[(&str, &str, bool)] = match name {
        "gil_help" => &[("topic", "주제", false)],
        "gil_open" => &[("kind", "Cycle 종류", false)],
        "gil_cycle" => &[("action", "cycle 동작", true), ("kind", "Cycle 종류", false)],
        _ => &[],
    };
    for (key, what, required) in slots {
        let value = match positional(input.get(key).and_then(Value::as_str), what, *required) {
            Ok(value) => value, Err(said) => return refusal(said),
        };
        if let Some(value) = value {
            if *key == "action" && !matches!(value.as_str(), "open" | "close") {
                return refusal("cycle 동작 는 open 또는 close 중 하나다");
            }
            args.push(value);
        }
    }
    let body = match name { "gil_open" => input.get("contract"), "gil_close" => input.get("report"), _ => None }
        .and_then(Value::as_str).unwrap_or("");
    announce(match crate::command::action(&args, root.map(Path::new), body) {
        Ok(said) => result(true, Some(0), said, String::new()),
        // The old CLI stderr boundary used eprintln!; preserve its final newline.
        Err(problem) => {
            let problem = if name == "gil_cycle" && input.get("action").and_then(Value::as_str) == Some("close") {
                format!("도구 선택 안내: gil_cycle은 Report를 받는 입력이 없어 닫기에 사용할 수 없다. 같은 gil_cycle 호출을 반복하지 않는다. 아래 Report를 작성해 gil_close(project_root, report)로 제출한다. Cycle도 gil_close로 닫는다. 이 실패로 닫힌 것은 없다.\n\n{problem}")
            } else { problem };
            annotate_code(result(false, Some(1), String::new(), format!("{problem}\n")), &problem)
        },
    })
}

pub const ROOT_UNREADABLE: &str = "Project 자리를 열 수 없다";
const COPY_ROOT_VERBATIM: &str = "gil_context/gil_status/receipt가 돌려준 project_root를 글자 그대로 복사해 다시 보낸다. 다시 타이핑하지 않는다.";

/// The folder the Host selected, found from the string the Agent sent.
///
/// The string is used as given when it names a directory. When it does not, the same path in the
/// other Hangul form (NFC ↔ NFD) is tried, because some file systems retain decomposed names while Agents
/// retype them composed; the form that exists is used and the receipt says so once. Missing paths with compatibility
/// jamo cannot be repaired by normalization; existing literal names remain usable. Nothing is created.
pub(super) fn resolve_root(given: &str) -> Result<(String, Option<String>), String> {
    resolve_root_with(given, |path| std::fs::metadata(path).map(|one| one.is_dir()))
}

fn resolve_root_with(given: &str, mut is_dir: impl FnMut(&str) -> std::io::Result<bool>) -> Result<(String, Option<String>), String> {
    let inaccessible = |error: std::io::Error| format!("{ROOT_UNREADABLE}: 경로 접근을 확인하지 못했다 ({error}). 권한·디스크 오류를 경로 없음으로 판단하거나 다른 폴더로 바꾸지 않는다. {COPY_ROOT_VERBATIM}");
    match is_dir(given) {
        Ok(true) => return Ok((given.to_owned(), None)),
        Ok(false) => return Err("Project 자리가 폴더가 아니다".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(inaccessible(error)),
    }
    if super::hangul::has_compatibility_jamo(given) {
        return Err(format!(
            "{ROOT_UNREADABLE}: 경로에 호환 자모(ㄱ·ㅏ 같은 낱자)가 있어 정규화로 폴더를 추측하지 않는다. {COPY_ROOT_VERBATIM}"
        ));
    }
    let mut found = Vec::new();
    for candidate in [super::hangul::nfc(given), super::hangul::nfd(given)] {
        if candidate == given || found.contains(&candidate) { continue; }
        match is_dir(&candidate) {
            Ok(true) => found.push(candidate),
            Ok(false) => return Err("Project 자리가 폴더가 아니다".to_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(inaccessible(error)),
        }
    }
    if found.len() > 1 {
        return Err(format!("{ROOT_UNREADABLE}: 정규화 후보 폴더가 여러 개라 Project를 선택하지 않았다. {COPY_ROOT_VERBATIM}"));
    }
    if let Some(candidate) = found.pop() {
        let note = format!("project_root 글자를 맞춰 {candidate} 를 사용했다 — 받은 경로의 한글 정규화(NFC/NFD)가 실제 폴더와 달랐다. 다음 호출부터 이 경로를 글자 그대로 복사한다.");
        return Ok((candidate, Some(note)));
    }
    Err(format!("{ROOT_UNREADABLE}: 경로가 존재하지 않는다. 받은 경로의 글자가 사용자가 고른 폴더와 다를 수 있다(한글 정규화). {COPY_ROOT_VERBATIM}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn root_lookup_never_converts_access_errors_or_ambiguity_into_a_different_project() {
        use std::io::{Error, ErrorKind};
        let mut probes = 0;
        let denied = super::resolve_root_with("/tmp/한글", |_| {
            probes += 1; Err(Error::from(ErrorKind::PermissionDenied))
        }).unwrap_err();
        assert_eq!(probes, 1);
        assert!(denied.contains("경로 접근"));
        let mixed = "/tmp/한글";
        let ambiguous = super::resolve_root_with(mixed, |p| {
            if p == mixed { Err(Error::from(ErrorKind::NotFound)) } else { Ok(true) }
        }).unwrap_err();
        assert!(ambiguous.contains("후보 폴더가 여러 개"));
        let unique = super::resolve_root_with(mixed, |p| {
            if p == super::super::hangul::nfc(mixed) { Ok(true) } else { Err(Error::from(ErrorKind::NotFound)) }
        }).unwrap();
        assert_eq!(unique.0, "/tmp/한글");
        assert!(unique.1.is_some());
    }

    use super::*;
    #[test]
    fn positional_rules_match_the_js_boundary_including_utf16_length() {
        for bad in ["-h", "line\nbreak", "\0", "\r"] { assert!(positional(Some(bad), "값", false).is_err()); }
        assert!(positional(Some(&"😀".repeat(100)), "값", false).is_ok());
        assert!(positional(Some(&"😀".repeat(101)), "값", false).is_err());
        assert_eq!(positional(Some(""), "값", true).unwrap_err(), "값 가 필요하다");
    }
    #[test]
    fn no_root_help_does_not_borrow_the_server_working_directory() {
        let got = call("gil_help", &json!({}));
        assert_eq!(got.is_error, Some(false));
        assert_eq!(got.structured_content.unwrap()["said"], crate::help_outside().unwrap());
    }
}
