//! 범위 변경·기준점 보관·즉시 보관 중단의 도구(`tracking-policy` 개발 구성). App capability 는 이 프로세스 메모리에만 있고,
//! 모델에 보이는 내용·`structuredContent`·원장·정식 기록 어디에도 들어가지 않는다.
//!
//! ```text
//! model  gil_scope_propose(project_root, rules)   닫힌 검토 Interview 의 경계에서 정확한 제안 판 n. 프로젝트를 바꾸지 않는다
//!        gil_scope_status(project_root)           저장 상태 조회(쓰지 않는다. 판·사건·시도를 만들지 않는다)
//! app    gil_scope_review(scope_id)               최신 제안·지금 자리의 세 값·대기·중단 + `_meta` 의 일회성 capability 들
//!        gil_scope_decide(scope_id, revision, digest, token, decision)      Monitor 버튼의 응답. 승인이면 적용까지
//!        gil_scope_retry(scope_id, event, token)                            기준점 보관의 명시 재시도
//!        gil_scope_halt(scope_id, token)                                    즉시 보관 중단
//!        gil_scope_release(scope_id, at, cleanup, state_confirmed, token)   현재 상태와 정리 방법을 확인한 해제
//! ```
//!
//! 승인·재시도·중단 요청·해제는 **App 전용**이다(공유 계약 §5.1 「승인 채널은 모니터 하나」, §12 O-2 의 2026-11-24 결정: 사람과 소통하는
//! 기본 모드에서 중단 요청과 해제 모두 Monitor 가 기본 경로). 모델 도구에는 그 값의 자리가 없다. capability 는 쓰임마다 다른 값이고
//! 정확한 대상(제안 판·digest, 사건, 표식의 자리)에 묶이며, 쓰고 버린다. 그것은 화면을 거쳤다는 표식이지 개인 신원의 인증이 아니다.
use super::Monitor;
use crate::scope_change::{self as sc, Purpose};
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Mutex};

pub(super) const NAMES: &[&str] = &[
    "gil_scope_propose", "gil_scope_status",
    "gil_scope_review", "gil_scope_decide", "gil_scope_retry", "gil_scope_halt", "gil_scope_release",
];
/// App 전용 도구 — 카탈로그 가시성 시험이 이 목록을 읽는다.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const APP_NAMES: &[&str] =
    &["gil_scope_review", "gil_scope_decide", "gil_scope_retry", "gil_scope_halt", "gil_scope_release"];

/// (scope, 쓰임) → (묶인 대상, capability).
#[derive(Default)]
pub(super) struct Grants(Mutex<BTreeMap<(String, Purpose), (String, String)>>);
impl std::fmt::Debug for Grants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grants").finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Propose { project_root: String, rules: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Root { project_root: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review { scope_id: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decide { scope_id: String, revision: u32, digest: String, token: String, decision: Decision }
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Decision { Approve, RequestChanges }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retry { scope_id: String, event: String, token: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Halt { scope_id: String, token: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release { scope_id: String, at: String, cleanup: String, state_confirmed: bool, token: String }

pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |bytes| bytes.len() > 512 * 1024) {
        return false;
    }
    let ok = |read: Result<(), serde_json::Error>| read.is_ok();
    match name {
        "gil_scope_propose" => ok(serde_json::from_value::<Propose>(input.clone()).map(drop)),
        "gil_scope_status" => ok(serde_json::from_value::<Root>(input.clone()).map(drop)),
        "gil_scope_review" => ok(serde_json::from_value::<Review>(input.clone()).map(drop)),
        "gil_scope_decide" => ok(serde_json::from_value::<Decide>(input.clone()).map(drop)),
        "gil_scope_retry" => ok(serde_json::from_value::<Retry>(input.clone()).map(drop)),
        "gil_scope_halt" => ok(serde_json::from_value::<Halt>(input.clone()).map(drop)),
        "gil_scope_release" => ok(serde_json::from_value::<Release>(input.clone()).map(drop)),
        _ => false,
    }
}

impl Grants {
    /// 이 scope·쓰임·대상의 capability. 같은 대상이면 같은 값을 다시 준다(화면의 다시 그리기가 버튼을 무효로 만들지 않는다).
    fn token(&self, scope: &str, purpose: Purpose, target: &str) -> Result<String, String> {
        let mut grants = self.0.lock().map_err(|_| crate::locale::text("plan_tool.grants_lock"))?;
        let key = (scope.to_owned(), purpose);
        if let Some((bound, token)) = grants.get(&key)
            && bound == target
        {
            return Ok(token.clone());
        }
        if !grants.contains_key(&key) && grants.len() >= 128 {
            return Err(crate::locale::text("plan_tool.grants_limit").into());
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| crate::locale::text("plan_tool.grants_random"))?;
        let token = bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        grants.insert(key, (target.to_owned(), token.clone()));
        Ok(token)
    }

    /// 이 프로세스가 그 scope·쓰임·**그 대상**에 준 capability 인가. 맞으면 쓰고 버린다 — 재전송은 기록 조회가 답한다.
    fn take(&self, scope: &str, purpose: Purpose, target: &str, token: &str) -> Result<(), String> {
        let mut grants = self.0.lock().map_err(|_| crate::locale::text("plan_tool.grants_lock"))?;
        let key = (scope.to_owned(), purpose);
        let matches = grants.get(&key).is_some_and(|(bound, given)| bound == target && given == token);
        if !matches {
            return Err(crate::locale::text("scope_tool.stale_grant").into());
        }
        grants.remove(&key);
        Ok(())
    }
}

fn structured(value: Value) -> CallToolResult {
    let said = value["said"].as_str().unwrap_or_default().to_owned();
    let next = value["next"].as_str().unwrap_or_default().to_owned();
    let mut out = CallToolResult::structured(value);
    let mut text = said;
    if !next.is_empty() {
        text = format!("{text}\n{next}");
    }
    out.content.insert(0, ContentBlock::text(text));
    out
}

fn refused(refusal: sc::Refusal) -> CallToolResult {
    let mut out = super::actions::refusal(&refusal.said);
    if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
        fields.insert("code".into(), json!(refusal.code));
        if !refusal.detail.is_null() {
            fields.insert("detail".into(), refusal.detail.clone());
        }
    }
    if !refusal.detail.is_null() {
        out.content.push(ContentBlock::text(serde_json::to_string_pretty(&refusal.detail).unwrap_or_default()));
    }
    out
}

fn meta_key(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::Decide => "gil/scopeApprovalToken",
        Purpose::Retry => "gil/scopeRetryToken",
        Purpose::Halt => "gil/scopeHaltToken",
        Purpose::Release => "gil/scopeReleaseToken",
    }
}

pub(super) fn call(monitor: &Monitor, grants: &Grants, name: &str, input: Value) -> CallToolResult {
    let bad = || super::actions::refusal(crate::locale::text("scope_tool.bad_input"));
    let with_scope = |mut value: Value, scope: &str| -> CallToolResult {
        value["scope_id"] = json!(scope);
        structured(value)
    };
    // 사용자가 결정할 화면은 같은 Monitor 다. scope 는 비밀이 아니다(capability 가 아니다).
    let with_monitor = |mut value: Value, root: &str| -> Value {
        if let Ok(prepared) = monitor.prepare_start(root)
            && let Some(scope) = prepared["scope_id"].as_str()
        {
            value["scope_id"] = json!(scope);
            value["next_call"] = json!({"name": "show_gil_monitor", "arguments": {"scope_id": scope}});
        }
        value
    };
    macro_rules! root_of {
        ($scope:expr) => {
            match monitor.root($scope) {
                Ok(root) => root,
                Err(e) => return super::actions::refusal(e.said),
            }
        };
    }
    match name {
        "gil_scope_propose" => {
            let Ok(r) = serde_json::from_value::<Propose>(input) else { return bad() };
            sc::propose(&r.project_root, r.rules).map(|value| structured(with_monitor(value, &r.project_root))).unwrap_or_else(refused)
        }
        "gil_scope_status" => {
            let Ok(r) = serde_json::from_value::<Root>(input) else { return bad() };
            sc::status(&r.project_root).map(|value| structured(with_monitor(value, &r.project_root))).unwrap_or_else(refused)
        }
        "gil_scope_review" => {
            let Ok(r) = serde_json::from_value::<Review>(input) else { return bad() };
            let root = root_of!(&r.scope_id);
            match sc::review(&root) {
                Ok((mut value, wanted)) => {
                    value["scope_id"] = json!(r.scope_id);
                    let mut result = serde_json::to_value(structured(value)).expect("result");
                    let mut meta = serde_json::Map::new();
                    for (purpose, target) in wanted {
                        match grants.token(&r.scope_id, purpose, &target) {
                            Ok(token) => {
                                meta.insert(meta_key(purpose).into(), json!(token));
                            }
                            Err(why) => return super::actions::refusal(&why),
                        }
                    }
                    if !meta.is_empty() {
                        result["_meta"] = Value::Object(meta);
                    }
                    serde_json::from_value(result).unwrap_or_else(|_| bad())
                }
                Err(e) => refused(e),
            }
        }
        "gil_scope_decide" => {
            let Ok(r) = serde_json::from_value::<Decide>(input) else { return bad() };
            let target = format!("{}:{}", r.revision, r.digest);
            if let Err(why) = grants.take(&r.scope_id, Purpose::Decide, &target, &r.token) {
                return super::actions::refusal(&why);
            }
            let root = root_of!(&r.scope_id);
            match sc::decide(&root, r.revision, &r.digest, r.decision == Decision::Approve) {
                Ok(value) => with_scope(value, &r.scope_id),
                Err(e) => refused(e),
            }
        }
        "gil_scope_retry" => {
            let Ok(r) = serde_json::from_value::<Retry>(input) else { return bad() };
            if let Err(why) = grants.take(&r.scope_id, Purpose::Retry, &r.event, &r.token) {
                return super::actions::refusal(&why);
            }
            let root = root_of!(&r.scope_id);
            match sc::retry(&root, &r.event) {
                Ok(value) => with_scope(value, &r.scope_id),
                Err(e) => refused(e),
            }
        }
        "gil_scope_halt" => {
            let Ok(r) = serde_json::from_value::<Halt>(input) else { return bad() };
            if let Err(why) = grants.take(&r.scope_id, Purpose::Halt, "halt", &r.token) {
                return super::actions::refusal(&why);
            }
            let root = root_of!(&r.scope_id);
            match sc::halt(&root) {
                Ok(value) => with_scope(value, &r.scope_id),
                Err(e) => refused(e),
            }
        }
        "gil_scope_release" => {
            let Ok(r) = serde_json::from_value::<Release>(input) else { return bad() };
            // 해제는 사용자가 화면에서 현재 상태를 확인했다는 명시 입력을 요구한다 — 버튼 한 번의 암묵 확인으로 읽지 않는다.
            if !r.state_confirmed {
                return super::actions::refusal(crate::locale::text("scope_tool.release_needs_confirmation"));
            }
            if let Err(why) = grants.take(&r.scope_id, Purpose::Release, &r.at, &r.token) {
                return super::actions::refusal(&why);
            }
            let root = root_of!(&r.scope_id);
            match sc::release(&root, &r.at, &r.cleanup) {
                Ok(value) => with_scope(value, &r.scope_id),
                Err(e) => refused(e),
            }
        }
        _ => super::actions::refusal(crate::locale::text("scope_tool.unknown")),
    }
}

pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    let input = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let string = json!({"type":"string"});
    let root = json!({"type":"string","description":"The user's explicitly selected absolute project folder."});
    for (name, description, schema, app, read) in [
        ("gil_scope_propose",
         "Development tracking-policy scope change (format 10). After a review Interview is CLOSED and you stand on its boundary, record one exact proposal (revision n) of the NEW root .gilignore text: the exact UTF-8 bytes (never normalized), which paths become newly tracked or newly excluded, the review Interview, the approval boundary, the applied policy and the last confirmed world it compares against. Writes nothing in the project and applies nothing: a proposal is never used for any check before the user approves it in the Monitor. Chat approval, editing .gilignore, or any agent input is never approval, and no model tool approves, retries an archive, halts or releases. A new proposal supersedes older ones. Refused inside an open Cycle, at a Chain closing boundary, in a later Chain, or while an archive halt marker is present. Next: show_gil_monitor(scope_id) and wait for the user's decision; read gil_scope_status afterwards.",
         input(json!({"project_root":root,"rules":{"type":"string","description":"Exact new root .gilignore text to approve."}}), vec!["project_root","rules"]),
         false, false),
        ("gil_scope_status",
         "Read the stored scope-change state: the APPLIED policy (in force from the moment its approval was saved), the LAST CONFIRMED world and the policy that confirmed it (they differ while a baseline archive is pending), whether an event waits for its baseline archive (this holds only the next Experiment; Interviews stay available), failed archive attempts, the archive halt marker, the policy-file sync state and the latest proposal phase (no_proposal, awaiting_approval, changes_requested, approved_not_applied, applied, superseded_by_progress). Writes nothing and never starts an archive attempt. Use it after a lost response, reconnect or restart instead of guessing or repeating an approval. Retrying the archive, halting and releasing are the user's Monitor buttons; there is no model tool for them.",
         input(json!({"project_root":root}), vec!["project_root"]), false, true),
        ("gil_scope_review",
         "App-only: read the exact latest scope-change proposal, the applied policy / last confirmed world / waiting state, the archive halt marker and private ephemeral capabilities for the buttons that are valid now. Never implies user approval.",
         input(json!({"scope_id":string}), vec!["scope_id"]), true, true),
        ("gil_scope_decide",
         "App-only: submit explicit trusted Monitor button input for the displayed scope, revision and digest. Requires the private capability from review. On approve, under the project lock it rechecks the boundary, unconfirmed changes on continuously tracked paths (with the APPLIED policy, never the proposal), the rules, the actual tracked list and .gilignore, then saves the new policy and its scope_change event in one save, syncs the policy file and makes ONE baseline archive attempt. A repeat of the same response returns the stored state and creates nothing.",
         input(json!({"scope_id":string,"revision":{"type":"integer","minimum":1},"digest":string,"token":string,"decision":{"type":"string","enum":["approve","request_changes"]}}),
             vec!["scope_id","revision","digest","token","decision"]), true, false),
        ("gil_scope_retry",
         "App-only: on a trusted Monitor click, explicitly retry the baseline archive of the waiting event at the closed boundary the project stands on now, starting from a fresh check and a fresh stable observation. Never automatic; refused inside an open Cycle, at a Chain closing boundary or while halted. An already confirmed baseline is never confirmed twice.",
         input(json!({"scope_id":string,"event":string,"token":string}), vec!["scope_id","event","token"]), true, false),
        ("gil_scope_halt",
         "App-only: on a trusted Monitor click, record the archive halt marker immediately. Leaves every record and file as it is: nothing is closed, failed, restored or deleted. While it is present every open, close and world confirmation is refused; reads stay available.",
         input(json!({"scope_id":string,"token":string}), vec!["scope_id","token"]), true, false),
        ("gil_scope_release",
         "App-only: release the archive halt marker after the user explicitly confirmed the displayed state and chose how to clean up. Changes nothing else and starts nothing: it is not a retry, archive, restore or closing approval. Reconnect, restart or a repeated request never releases.",
         input(json!({"scope_id":string,"at":string,"cleanup":{"type":"string","enum":sc::CLEANUPS},"state_confirmed":{"type":"boolean"},"token":string}),
             vec!["scope_id","at","cleanup","state_confirmed","token"]), true, false),
    ] {
        tools.push(serde_json::from_value(json!({"name":name,"description":description,"inputSchema":schema,
            "annotations":{"readOnlyHint":read,"destructiveHint":false,"openWorldHint":false},
            "_meta":{"ui":{"visibility":if app {vec!["app"]} else {vec!["model"]}}}})).expect("scope catalog"));
    }
    tools
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
