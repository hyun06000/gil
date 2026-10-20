//! Windows development surface: location support check before `gil_start` creates anything,
//! and approval-based preparation of a new local workspace folder (Windows contract §4 W1 0).
//! Creating a folder is not connecting it: the Host's working-folder selection stays the user's.
use crate::windows_fs as win;
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::path::Path;

pub const NAME: &str = "gil_prepare_workspace";

pub fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    tools.push(serde_json::from_value(json!({"name": NAME,
        "description": "Windows development surface. Plan (confirm:false) or create (confirm:true) one new empty local workspace folder for a GIL Project: <parent>\\<name>, parent defaulting to %USERPROFILE%\\gil-projects. Allowed range: parent must lie under %USERPROFILE%; at most the missing parent and the folder itself are created; nothing else is ever touched. confirm:true must carry approved_path equal to the exact path the plan returned and may only be sent after the user approved that path in chat (the tool cannot verify human approval; it only binds the request to the shown path). Location (local fixed NTFS, no OneDrive/cloud-sync root, no reparse point), ancestors and conflicts are re-checked right before creation; an existing entry is never reused or replaced and no other name is chosen; on any refusal nothing is created, and if a refusal happens after creation the response lists exactly what this call created and whether it remains. Creating the folder does not connect it: the user must select it as the working folder in the app before gil_start.",
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

/// Structured refusal for `gil_start` on an unsupported location: nothing was created.
pub fn start_refusal(selected: &Path, location: &win::UnsupportedLocation) -> CallToolResult {
    let suggested = win::default_workspace_parent().map(|p| p.display().to_string()).unwrap_or_default();
    let said = format!("{} (아직 아무것도 만들지 않았습니다.)", location.kind.notice());
    let mut out = CallToolResult::structured(json!({"ok": false, "code": "unsupported_location",
        "location_kind": location.kind.code(), "evidence": location.evidence().code(), "detail": location.detail,
        "selected": selected.display().to_string(), "suggested_parent": suggested,
        "next": "Explain the support-scope notice in `said` and, only with the user's approval, call gil_prepare_workspace(confirm:false) to show the exact path, then confirm:true. Then ask the user to select the new folder in the app and wait; never call gil_start on it until it is the selected working folder.",
        "said": said}));
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
        Err(refusal) => return refuse(refusal),
    };
    // Checked, not guaranteed: outside the roots OneDrive declares and without cloud-files
    // attributes. Other synchronization products are not detected (contract §8 1b).
    let note = "확인한 범위에서 이 폴더는 OneDrive가 선언한 백업 위치 밖이고 클라우드 동기화 속성이 없다(다른 동기화 제품의 유무는 확인하지 않는다). 폴더를 만드는 것과 Codex의 작업 폴더로 선택하는 것은 별개다: 사용자가 앱에서 이 폴더를 선택한 뒤에만 gil_start를 부른다.";
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
        None => return fail("invalid_request", "confirm:true 에는 계획이 보여 준 정확한 경로를 approved_path 로 함께 보내야 한다(아무것도 만들지 않았다)."),
        Some(approved) if approved != shown => return fail("path_mismatch", format!("approved_path 가 계획한 경로와 다르다: 승인 {approved} / 계획 {shown} (아무것도 만들지 않았다).")),
        Some(_) => {}
    }
    match win::create_workspace(&plan) {
        Ok(created) => CallToolResult::structured(json!({"ok": true, "stage": "prepared", "path": shown,
            "parent": plan.parent.display().to_string(), "created_parent": plan.creates_parent,
            "created_by_this_call": created.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "said": format!("새 작업 폴더를 만들었다: {}. 이제 Codex 앱에서 이 폴더를 작업 폴더로 선택해 달라고 안내하고 기다린다. {}", plan.path.display(), note)})),
        Err(failure) => {
            let mut out = refuse(failure.refusal);
            if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
                let created: Vec<Value> = failure.created.iter().map(|(p, remains)| json!({"path": p.display().to_string(), "remains": remains})).collect();
                let remaining = failure.created.iter().filter(|(_, r)| *r).count();
                if let Some(said) = fields.get("said").and_then(Value::as_str) {
                    let said = if created.is_empty() { format!("{said} 이번 호출은 아무것도 만들지 않았다.") }
                        else { format!("{said} 이번 호출이 만든 항목 {}개 중 {}개가 남아 있다(기존 항목은 건드리지 않았다).", created.len(), remaining) };
                    fields.insert("said".into(), json!(said));
                }
                fields.insert("created_by_this_call".into(), Value::Array(created));
            }
            out
        }
    }
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
