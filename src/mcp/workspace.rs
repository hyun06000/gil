//! Optional Windows workspace-folder preparation. GIL no longer requires this for OneDrive:
//! private records live in local app data while the user's selected working folder stays put.
use crate::windows_fs as win;
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::ffi::OsStr;
#[cfg(test)]
use std::path::Path;

pub const NAME: &str = "gil_prepare_workspace";

pub fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    tools.push(serde_json::from_value(json!({"name": NAME,
        "description": "Optional Windows convenience, only when the user explicitly asks for a separate empty local workspace folder. It is not a repair step and must not be offered merely because the selected folder is in OneDrive: GIL keeps private records in local app data and leaves the working folder in place. Plan (confirm:false) or create (confirm:true) <parent>\\<name>, parent defaulting to %USERPROFILE%\\gil-projects. confirm:true requires the exact approved_path returned by the plan after human approval. Existing entries are never reused or replaced.",
        "inputSchema": {"type":"object","additionalProperties":false,"required":["name","confirm"],
            "properties":{"name":{"type":"string","minLength":1,"maxLength":120},
                          "parent":{"type":"string","minLength":3,"maxLength":1024},
                          "confirm":{"type":"boolean"},
                          "approved_path":{"type":"string","minLength":3,"maxLength":1200,"description":"Required with confirm:true: the exact `path` string the plan returned and the user approved."}}},
        "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false},
        "_meta": {"ui": {"visibility": ["model"]}}})).unwrap());
    tools
}

/// Wire shape (-32602 on mismatch): `name` string and `confirm` boolean are required; `parent`
/// and `approved_path` are optional strings; nothing else is accepted.
pub fn valid(input: &Value) -> bool {
    let Some(object) = input.as_object() else { return false; };
    if !object.get("name").is_some_and(Value::is_string) || !object.get("confirm").is_some_and(Value::is_boolean) {
        return false;
    }
    object.iter().all(|(key, value)| match key.as_str() {
        "name" | "confirm" => true,
        "parent" | "approved_path" => value.is_string(),
        _ => false,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    name: String,
    parent: Option<String>,
    confirm: bool,
    approved_path: Option<String>,
}

fn fail(code: &str, said: impl Into<String>) -> CallToolResult {
    let mut out = CallToolResult::structured(json!({"ok": false, "code": code, "said": said.into()}));
    out.is_error = Some(true);
    out
}

pub fn call(input: Value) -> CallToolResult {
    let Ok(request) = serde_json::from_value::<Request>(input) else {
        return fail("invalid_request", "name(폴더 이름)·confirm(승인 여부)·parent·approved_path 만 받는다.");
    };
    if request.name.len() > 120 || request.name.contains(['\n', '\r', '\0']) {
        return fail("invalid_name", "폴더 이름이 너무 길거나 줄바꿈을 포함한다.");
    }
    let parent = match &request.parent {
        Some(p) => std::path::PathBuf::from(p),
        None => match win::default_workspace_parent() {
            Ok(p) => p,
            Err(_) => return fail("location_unverifiable", "사용자 프로필 위치를 확인하지 못해 기본 작업 폴더를 정할 수 없다."),
        },
    };
    let plan = match win::plan_workspace(&parent, OsStr::new(&request.name)) {
        Ok(plan) => plan,
        // With confirm:true every refusal states what this call created: nothing, here.
        Err(refusal) => return if request.confirm { with_created(refuse(refusal), &[]) } else { refuse(refusal) },
    };
    // Checked, not guaranteed: outside the roots OneDrive declares and without cloud-files
    // attributes. Other synchronization products are not detected (contract §8 1b).
    let note = "사용자가 별도 로컬 작업 폴더를 명시적으로 요청한 경우에만 쓰는 선택 기능이다. OneDrive 안의 기존 작업 폴더를 옮길 필요는 없다. 폴더를 만드는 것과 Codex의 작업 폴더로 선택하는 것은 별개다.";
    let shown = plan.path.display().to_string();
    if !request.confirm {
        return CallToolResult::structured(json!({"ok": true, "stage": "planned", "path": shown,
            "parent": plan.parent.display().to_string(), "creates_parent": plan.creates_parent,
            "said": format!("승인하면 다음 폴더를 새로 만든다: {}{}. {}", plan.path.display(),
                if plan.creates_parent { format!(" (상위 폴더 {}도 새로 만든다)", plan.parent.display()) } else { String::new() }, note)}));
    }
    // confirm:true binds the request to the path the user saw; it is the Agent's assertion of
    // approval, which this tool cannot verify.
    match request.approved_path.as_deref() {
        None => return with_created(fail("invalid_request", "confirm:true 에는 계획이 보여 준 정확한 경로를 approved_path 로 함께 보내야 한다."), &[]),
        Some(approved) if approved != shown => return with_created(fail("path_mismatch", format!("approved_path 가 계획한 경로와 다르다: 승인 {approved} / 계획 {shown}.")), &[]),
        Some(_) => {}
    }
    match win::create_workspace(&plan) {
        Ok(created) => CallToolResult::structured(json!({"ok": true, "stage": "prepared", "path": shown,
            "parent": plan.parent.display().to_string(), "created_parent": plan.creates_parent,
            "created_by_this_call": created.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "said": format!("새 작업 폴더를 만들었다: {}. 이제 Codex 앱에서 이 폴더를 작업 폴더로 선택해 달라고 안내하고 기다린다. {}", plan.path.display(), note)})),
        Err(failure) => with_created(refuse(failure.refusal), &failure.created),
    }
}

/// A confirm:true refusal always reports exactly what this call created and whether each entry
/// still exists; existing entries are never touched and nothing created is removed.
fn with_created(mut out: CallToolResult, created: &[(std::path::PathBuf, bool)]) -> CallToolResult {
    if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
        let list: Vec<Value> = created.iter().map(|(p, remains)| json!({"path": p.display().to_string(), "remains": remains})).collect();
        let remaining = created.iter().filter(|(_, r)| *r).count();
        if let Some(said) = fields.get("said").and_then(Value::as_str) {
            let said = if list.is_empty() { format!("{said} 이번 호출은 아무것도 만들지 않았다.") }
                else { format!("{said} 이번 호출이 만든 항목 {}개 중 {}개가 남아 있다(기존 항목은 건드리지 않았다).", list.len(), remaining) };
            fields.insert("said".into(), json!(said));
        }
        fields.insert("created_by_this_call".into(), Value::Array(list));
    }
    out
}

fn refuse(refusal: win::WorkspaceRefusal) -> CallToolResult {
    use win::WorkspaceRefusal::*;
    match refusal {
        InvalidName => fail("invalid_name", "폴더 이름은 경로 구분자·콜론·예약 장치 이름 없이 한 단계여야 한다."),
        ParentNotAbsolute => fail("invalid_request", "parent 는 드라이브 문자로 시작하는 절대 경로여야 한다."),
        OutsideAllowed(p) => fail("outside_allowed_range", format!("이 도구는 사용자 프로필(%USERPROFILE%) 아래에만 폴더를 만든다: {}", p.display())),
        PlanChanged => fail("plan_changed", "생성 직전 재검사 결과가 계획과 달라 만들지 않았다. 계획부터 다시 보여 주고 승인을 받아야 한다."),
        ParentMissing(p) => fail("parent_missing", format!("상위 폴더가 없어 만들 수 없다: {}", p.display())),
        Unsupported(location) => {
            let mut out = fail("unsupported_location", format!("{} (아무것도 만들지 않았다.)", location.kind.notice()));
            if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
                fields.insert("location_kind".into(), json!(location.kind.code()));
                fields.insert("evidence".into(), json!(location.evidence().code()));
                fields.insert("detail".into(), json!(location.detail));
            }
            out
        }
        Exists(p) => fail("exists", format!("이미 같은 이름의 항목이 있어 재사용하거나 덮어쓰지 않는다: {}", p.display())),
        Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => fail("permission_denied", format!("이 위치에 폴더를 만들 권한이 없다(관리자 실행으로 확대하지 않는다): {e}")),
        Io(e) => fail("io_error", format!("폴더를 만들지 못했다: {e}")),
    }
}

/// Contract §6 W12-7: late failures report exactly what this call created, matching the file
/// system; an unverifiable location refuses plan and creation without touching anything.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_fs::workspace_hooks as hooks;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("gil-workspace-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        hooks::ALLOWED_ROOT.with(|c| *c.borrow_mut() = Some(root.clone()));
        root
    }
    fn content(out: &CallToolResult) -> Value {
        out.structured_content.clone().unwrap()
    }
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        let mut out = BTreeMap::new();
        fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() { out.insert(path.clone(), None); walk(&path, out); }
                else { out.insert(path.clone(), Some(fs::read(&path).unwrap())); }
            }
        }
        walk(root, &mut out);
        out
    }
    fn plan(parent: &Path, name: &str) -> String {
        let got = content(&call(json!({"name": name, "parent": parent, "confirm": false})));
        assert_eq!(got["ok"], true, "{got}");
        got["path"].as_str().unwrap().to_owned()
    }
    fn confirm(parent: &Path, name: &str, approved: &str) -> Value {
        content(&call(json!({"name": name, "parent": parent, "confirm": true, "approved_path": approved})))
    }

    #[test]
    fn child_failure_after_parent_creation_reports_the_parent_as_remaining() {
        let base = scratch("after-parent");
        let parent = base.join("상위 새 폴더");
        let shown = plan(&parent, "작업");
        assert!(!parent.exists());
        hooks::FAIL_AFTER_PARENT.with(|c| c.set(true));
        let got = confirm(&parent, "작업", &shown);
        assert_eq!(got["ok"], false, "{got}");
        assert_eq!(got["code"], "permission_denied");
        assert_eq!(got["created_by_this_call"], json!([{"path": parent.display().to_string(), "remains": true}]), "{got}");
        assert!(got["said"].as_str().unwrap().contains("1개 중 1개가 남아 있다"), "{got}");
        // The report matches the file system: the parent exists and is empty, the folder does not.
        assert!(parent.is_dir());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        assert!(!PathBuf::from(&shown).exists());
        assert!(!hooks::FAIL_AFTER_PARENT.with(std::cell::Cell::get), "the injected failure fired exactly once");
        // A second approved attempt on the now-existing parent creates only the folder.
        let again = confirm(&parent, "작업", &shown);
        assert_eq!(again["ok"], true, "{again}");
        assert_eq!(again["created_by_this_call"], json!([shown]));
    }

    #[test]
    fn unverifiable_location_refuses_plan_and_creation_without_touching_anything() {
        let base = scratch("unverifiable");
        let parent = base.join("기존 상위");
        fs::create_dir(&parent).unwrap();
        fs::write(parent.join("keep.txt"), b"user file").unwrap();
        fs::create_dir(parent.join("other")).unwrap();
        let before = snapshot(&base);
        hooks::UNVERIFIABLE.with(|c| c.set(true));
        let planned = content(&call(json!({"name": "새 폴더", "parent": parent, "confirm": false})));
        assert_eq!(planned["ok"], false, "{planned}");
        assert_eq!(planned["code"], "unsupported_location");
        assert_eq!(planned["location_kind"], "unverifiable");
        assert_eq!(planned["evidence"], "check_failed");
        assert!(planned["said"].as_str().unwrap().contains("확인할 수 없었습니다") && planned["said"].as_str().unwrap().contains("확인을 대신하지 않습니다"), "{planned}");
        let forced = confirm(&parent, "새 폴더", &parent.join("새 폴더").display().to_string());
        assert_eq!(forced["code"], "unsupported_location", "{forced}");
        assert_eq!(forced["created_by_this_call"], json!([]));
        // Also with a missing parent: nothing is created either.
        let missing = base.join("없는 상위");
        let deeper = confirm(&missing, "x", &missing.join("x").display().to_string());
        assert_eq!(deeper["code"], "unsupported_location", "{deeper}");
        assert_eq!(deeper["created_by_this_call"], json!([]));
        hooks::UNVERIFIABLE.with(|c| c.set(false));
        assert_eq!(snapshot(&base), before, "an unverifiable location changes nothing");
        // Once verifiable again the same plan proceeds.
        let shown = plan(&parent, "새 폴더");
        assert_eq!(confirm(&parent, "새 폴더", &shown)["ok"], true);
    }

    #[test]
    fn unverifiable_result_after_creation_reports_both_entries_as_remaining() {
        let base = scratch("after-create");
        let parent = base.join("gil-projects");
        let shown = plan(&parent, "작업");
        hooks::UNVERIFIABLE_AFTER_CREATE.with(|c| c.set(true));
        let got = confirm(&parent, "작업", &shown);
        assert_eq!(got["ok"], false, "{got}");
        assert_eq!(got["code"], "unsupported_location");
        assert_eq!(got["location_kind"], "unverifiable");
        assert_eq!(got["created_by_this_call"], json!([
            {"path": parent.display().to_string(), "remains": true},
            {"path": shown, "remains": true}]), "{got}");
        assert!(got["said"].as_str().unwrap().contains("2개 중 2개가 남아 있다"), "{got}");
        assert!(parent.is_dir() && PathBuf::from(&shown).is_dir());
        assert_eq!(fs::read_dir(PathBuf::from(&shown)).unwrap().count(), 0);
    }
}
