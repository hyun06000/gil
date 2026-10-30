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
    let root = input.get("project_root").and_then(Value::as_str);
    if name != "gil_help" || root.is_some() {
        let Some(root) = root.filter(|r| !r.is_empty()) else { return refusal("Project 자리가 비었다"); };
        if !Path::new(root).is_absolute() { return refusal(crate::RootError::NotAbsolute.to_string()); }
        match std::fs::metadata(root) {
            Ok(one) if one.is_dir() => {},
            Ok(_) => return refusal("Project 자리가 폴더가 아니다"),
            Err(_) => return refusal("Project 자리를 열 수 없다"),
        }
    }
    let mut args = vec![command.to_owned()];
    #[cfg(feature = "visual-authoring")]
    if matches!(name, "gil_status" | "gil_context") {
        return match crate::command::visual_read_action(Path::new(root.expect("validated root")), command) {
            Ok(said) => result(true, Some(0), said, String::new()),
            Err(problem) => annotate_code(result(false, Some(1), String::new(), format!("{problem}\n")), &problem),
        };
    }
    #[cfg(feature = "visual-authoring")]
    if name == "gil_start" {
        return match crate::command::start_visual_action(Path::new(root.expect("validated root"))) {
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
        };
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
    match crate::command::action(&args, root.map(Path::new), body) {
        Ok(said) => result(true, Some(0), said, String::new()),
        // The old CLI stderr boundary used eprintln!; preserve its final newline.
        Err(problem) => {
            let problem = if name == "gil_cycle" && input.get("action").and_then(Value::as_str) == Some("close") {
                format!("도구 선택 안내: gil_cycle은 Report를 받는 입력이 없어 닫기에 사용할 수 없다. 같은 gil_cycle 호출을 반복하지 않는다. 아래 Report를 작성해 gil_close(project_root, report)로 제출한다. Cycle도 gil_close로 닫는다. 이 실패로 닫힌 것은 없다.\n\n{problem}")
            } else { problem };
            annotate_code(result(false, Some(1), String::new(), format!("{problem}\n")), &problem)
        },
    }
}

#[cfg(test)]
mod tests {
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
