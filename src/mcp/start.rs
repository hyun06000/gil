//! 시작 준비·Monitor 승인 도구(`tracking-policy` 개발 구성). App capability 는 이 프로세스 메모리에만 있고,
//! 모델에 보이는 내용·`structuredContent`·준비 기록·정식 기록 어디에도 들어가지 않는다.
//!
//! ```text
//! model  gil_start(project_root)            준비 기록 만들기/이어 가기 → Monitor scope → show_gil_monitor
//!        gil_start_prepare(project_root,…)  정확한 원문으로 제안 판 n
//!        gil_start_status(project_root)     저장 상태 조회(쓰지 않는다)
//!        gil_start_cancel(project_root)     사용자의 명시 취소. 기록 보존
//! app    gil_start_review(scope_id)         최신 제안 + `_meta` 의 일회성 capability — 승인 대기면 결정용, 이미 승인됐지만
//!                                           시작이 확정되지 않았으면 적용 재시도용(쓰임이 다른 별개의 값)
//!        gil_start_decide(scope_id, revision, digest, token, decision)
//!        gil_start_apply(scope_id, revision, digest, token)   기록된 Monitor 승인의 적용 재시도. 승인을 만들지 않는다
//! ```
//!
//! 기록(준비 기록)을 정식 시작으로 바꾸는 문은 App 전용 두 도구뿐이다 — 모델은 준비 기록을 손으로 고쳐도 그것을 적용할
//! 도구가 없다. 준비 기록 자체는 서명된 기록이 아니며 `.gil` 쓰기 권한을 가진 쪽의 위조를 암호학적으로 막지 않는다.
use super::Monitor;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Mutex};

pub(super) const NAMES: &[&str] = &[
    "gil_start", "gil_start_prepare", "gil_start_status", "gil_start_cancel",
    "gil_start_review", "gil_start_decide", "gil_start_apply", "gil_start_list",
];
/// App 전용 두 도구 — 카탈로그 가시성 시험과 Plugin Skill 시험(`skill.test.mjs`)이 이 목록을 읽는다.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const APP_NAMES: &[&str] = &["gil_start_review", "gil_start_decide", "gil_start_apply", "gil_start_list"];

#[derive(Debug, Default)]
pub(super) struct Grants(Mutex<BTreeMap<String, Grant>>);
struct Grant { revision: u32, digest: String, purpose: crate::start_prep::Purpose, token: String }
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant").field("revision", &self.revision).field("digest", &self.digest)
            .field("purpose", &self.purpose).finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Root { project_root: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Begin {
    project_root: String,
    #[allow(dead_code)] locale: Option<String>,
    /// The explicit new-start selection. Its value is judged in `start_kind`, before this tool runs.
    #[allow(dead_code)] start_kind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRec { key: String, rules: Vec<String>, why: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    project_root: String,
    rules: String,
    #[serde(default)]
    project_recommendations: Vec<AgentRec>,
    combine_with_revision: Option<u32>,
    #[serde(default)]
    restart_canceled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply { scope_id: String, revision: u32, digest: String, token: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review { scope_id: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List { scope_id: String, revision: u32, digest: String, list: String, offset: u32 }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decide { scope_id: String, revision: u32, digest: String, token: String, decision: Decision }
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Decision { Approve, RequestChanges }

pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 512 * 1024) {
        return false;
    }
    let ok = |r: Result<(), serde_json::Error>| r.is_ok();
    match name {
        "gil_start" => ok(serde_json::from_value::<Begin>(input.clone()).map(drop)),
        "gil_start_prepare" => ok(serde_json::from_value::<Prepare>(input.clone()).map(drop)),
        "gil_start_status" | "gil_start_cancel" => ok(serde_json::from_value::<Root>(input.clone()).map(drop)),
        "gil_start_apply" => ok(serde_json::from_value::<Apply>(input.clone()).map(drop)),
        "gil_start_review" => ok(serde_json::from_value::<Review>(input.clone()).map(drop)),
        "gil_start_decide" => ok(serde_json::from_value::<Decide>(input.clone()).map(drop)),
        "gil_start_list" => ok(serde_json::from_value::<List>(input.clone()).map(drop)),
        _ => false,
    }
}

impl Grants {
    fn token(&self, scope: &str, revision: u32, digest: &str, purpose: crate::start_prep::Purpose) -> Result<String, String> {
        let mut grants = self.0.lock().map_err(|_| crate::locale::text("plan_tool.grants_lock"))?;
        if let Some(g) = grants.get(scope).filter(|g| g.revision == revision && g.digest == digest && g.purpose == purpose) {
            return Ok(g.token.clone());
        }
        if !grants.contains_key(scope) && grants.len() >= 32 {
            return Err(crate::locale::text("plan_tool.grants_limit").into());
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| crate::locale::text("plan_tool.grants_random"))?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        grants.insert(scope.into(), Grant { revision, digest: digest.into(), purpose, token: token.clone() });
        Ok(token)
    }
    /// 이 scope 의 같은 판·digest 에 이 프로세스가 준 capability 인가. 맞으면 **쓰고 버린다**(재전송은 기록 조회가 답한다).
    fn take(&self, scope: &str, revision: u32, digest: &str, token: &str, purpose: crate::start_prep::Purpose) -> Result<(), String> {
        let mut grants = self.0.lock().map_err(|_| crate::locale::text("plan_tool.grants_lock"))?;
        let matches = grants.get(scope).is_some_and(|g| {
            g.revision == revision && g.digest == digest && g.token == token && g.purpose == purpose
        });
        if !matches {
            return Err(crate::locale::text("start_tool.stale_grant").into());
        }
        grants.remove(scope);
        Ok(())
    }
}

fn structured(value: Value) -> CallToolResult {
    let said = value["said"].as_str().unwrap_or_default().to_owned();
    let next = value["next"].as_str().unwrap_or_default().to_owned();
    let mut out = CallToolResult::structured(value);
    let mut text = said;
    if !next.is_empty() { text = format!("{text}\n{next}"); }
    out.content.insert(0, ContentBlock::text(text));
    out
}

fn refused(r: crate::start_prep::Refusal) -> CallToolResult {
    let mut out = super::actions::refusal(&r.said);
    if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
        fields.insert("code".into(), json!(r.code));
        if !r.detail.is_null() { fields.insert("detail".into(), r.detail.clone()); }
    }
    if !r.detail.is_null() {
        out.content.push(ContentBlock::text(serde_json::to_string_pretty(&r.detail).unwrap_or_default()));
    }
    out
}

pub(super) fn call(monitor: &Monitor, grants: &Grants, name: &str, input: Value) -> CallToolResult {
    use crate::start_prep as sp;
    let bad = || super::actions::refusal(crate::locale::text("start_tool.bad_input"));
    let headless_next = |mut value: Value| {
        if monitor.headless {
            match value["phase"].as_str() {
                Some("preparing") => value["next"] = json!(crate::locale::text("start_tool.headless_prepare")),
                Some("started") => value["next"] = json!(crate::locale::text("ctx.next.new_interview")),
                _ => {}
            }
        }
        value
    };
    let with_scope = |mut value: Value, root: &str| -> Value {
        // 정식 시작 전에도 같은 Monitor 경로로 준비 화면을 띄운다. scope 는 비밀이 아니다(capability 가 아니다).
        if !monitor.headless && let Ok(prepared) = monitor.prepare_start(root)
            && let Some(scope) = prepared["scope_id"].as_str() {
            value["scope_id"] = json!(scope);
            value["next_call"] = json!({"name": "show_gil_monitor", "arguments": {"scope_id": scope}});
        }
        headless_next(value)
    };
    match name {
        "gil_start" => {
            let Ok(r) = serde_json::from_value::<Begin>(input) else { return bad() };
            match sp::begin(&r.project_root) {
                Ok(value) => structured(with_scope(value, &r.project_root)),
                Err(e) => refused(e),
            }
        }
        "gil_start_prepare" => {
            let Ok(r) = serde_json::from_value::<Prepare>(input) else { return bad() };
            let input = sp::PrepareInput {
                rules: r.rules,
                project_recommendations: r.project_recommendations.into_iter()
                    .map(|a| sp::AgentRec { key: a.key, rules: a.rules, why: a.why }).collect(),
                combine_with_revision: r.combine_with_revision,
                restart_canceled: r.restart_canceled,
            };
            match sp::prepare(&r.project_root, input) {
                Ok(value) => structured(with_scope(value, &r.project_root)),
                Err(e) => refused(e),
            }
        }
        "gil_start_status" => {
            let Ok(r) = serde_json::from_value::<Root>(input) else { return bad() };
            sp::status(&r.project_root).map(headless_next).map(structured).unwrap_or_else(refused)
        }
        "gil_start_apply" => {
            let Ok(r) = serde_json::from_value::<Apply>(input) else { return bad() };
            if let Err(why) = grants.take(&r.scope_id, r.revision, &r.digest, &r.token, sp::Purpose::Apply) {
                return super::actions::refusal(&why);
            }
            let root = match monitor.root(&r.scope_id) { Ok(root) => root, Err(e) => return super::actions::refusal(e.said) };
            match sp::apply(&root, r.revision, &r.digest) {
                Ok(mut value) => {
                    value["scope_id"] = json!(r.scope_id);
                    structured(value)
                }
                Err(e) => refused(e),
            }
        }
        "gil_start_cancel" => {
            let Ok(r) = serde_json::from_value::<Root>(input) else { return bad() };
            sp::cancel(&r.project_root).map(structured).unwrap_or_else(refused)
        }
        "gil_start_review" => {
            let Ok(r) = serde_json::from_value::<Review>(input) else { return bad() };
            let root = match monitor.root(&r.scope_id) { Ok(root) => root, Err(e) => return super::actions::refusal(e.said) };
            match sp::review(&root) {
                Ok((value, grant)) => {
                    let mut result = serde_json::to_value(structured(value)).expect("result");
                    if let Some((revision, digest, purpose)) = grant {
                        let key = match purpose { sp::Purpose::Decide => "gil/startApprovalToken", sp::Purpose::Apply => "gil/startApplyToken" };
                        match grants.token(&r.scope_id, revision, &digest, purpose) {
                            Ok(token) => result["_meta"] = json!({ key: token }),
                            Err(why) => return super::actions::refusal(&why),
                        }
                    }
                    serde_json::from_value(result).unwrap_or_else(|_| bad())
                }
                Err(e) => refused(e),
            }
        }
        "gil_start_list" => {
            let Ok(r) = serde_json::from_value::<List>(input) else { return bad() };
            let root = match monitor.root(&r.scope_id) { Ok(root) => root, Err(e) => return super::actions::refusal(e.said) };
            sp::list(&root, r.revision, &r.digest, &r.list, r.offset as usize).map(CallToolResult::structured).unwrap_or_else(refused)
        }
        "gil_start_decide" => {
            let Ok(r) = serde_json::from_value::<Decide>(input) else { return bad() };
            if let Err(why) = grants.take(&r.scope_id, r.revision, &r.digest, &r.token, sp::Purpose::Decide) {
                return super::actions::refusal(&why);
            }
            let root = match monitor.root(&r.scope_id) { Ok(root) => root, Err(e) => return super::actions::refusal(e.said) };
            match sp::decide(&root, r.revision, &r.digest, r.decision == Decision::Approve) {
                Ok(mut value) => {
                    value["scope_id"] = json!(r.scope_id);
                    structured(value)
                }
                Err(e) => refused(e),
            }
        }
        _ => super::actions::refusal(crate::locale::text("start_tool.unknown")),
    }
}

pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    let input = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let string = json!({"type":"string"});
    let root = json!({"type":"string","description":"The user's explicitly selected absolute project folder."});
    if let Some(start) = tools.iter_mut().find(|t| t.name == "gil_start") {
        start.description = Some("Development tracking-policy start (format 10). Begins or resumes a START PREPARATION for the user's explicitly selected absolute project_root: no Chain, Cycle, Step or Snapshot is created and no work-file content is archived. Returns recommendations (general/secret candidates preselected, project-specific ones unselected; .gitignore is reference only), a default .gilignore text and the Monitor scope. Next: discuss scope with the user, then gil_start_prepare with the exact rules text, then show_gil_monitor(scope_id) and wait for the user's Monitor approval. Chat approval, editing .gilignore or any agent input is never approval, and no model tool turns the preparation record into a start. If a project already exists, read gil_context instead. Never repeat start to open a screen.".into());
    }
    for (name, description, schema, app, read) in [
        ("gil_start_prepare",
         "Record one exact .gilignore proposal (revision n) for the open start preparation: the exact UTF-8 rules bytes (never normalized), the engine gil-gilignore/1, the tracked/excluded effect (fully excluded folders are not explored), recommendation sources and whether each is in the rules, blockers (a nested .gilignore holds the start) and the policy-file effect. Writes nothing in the project. A new proposal supersedes older ones; their approvals are never reused. combine_with_revision: only after the Monitor application answered combine_required (read it with gil_start_status) (first check found no .gilignore, the user's approved proposal B, and a different .gilignore C now on disk): tell the user ONCE that you will combine B and C (do not ask whether you may), write the combined text D yourself keeping the engine's last-matching-rule order, and pass B's revision here; the Monitor then shows D with its differences from B and from C. restart_canceled: only when the user explicitly asks to restart a canceled preparation.",
         input(json!({"project_root":root,"rules":{"type":"string","description":"Exact root .gilignore text to approve."},
             "project_recommendations":{"type":"array","maxItems":64,"items":{"type":"object","additionalProperties":false,"required":["key","rules","why"],"properties":{"key":string,"rules":{"type":"array","items":string,"maxItems":64},"why":{"type":"string","description":"Purpose and archive/restore impact; never a secret value."}}},"description":"Project-specific exclusion suggestions. Always shown UNSELECTED; include their lines in rules only when the user chose them."},
             "combine_with_revision":{"type":"integer","minimum":1},
             "restart_canceled":{"type":"boolean"}}), vec!["project_root","rules"]),
         false, false),
        ("gil_start_status",
         "Read the stored start preparation: phase (preparing, awaiting_approval, changes_requested, approved, blocked, canceled, started), proposal revision/digest, history and, once started, policy:P1, event:M1, the initial Snapshot, chain:C1 and the policy-file sync base. Writes nothing. Use after a lost response, reconnect or restart instead of guessing or repeating an approval. If phase is approved but not started, ask the user to press the retry button in the Monitor; there is no model tool that applies it.",
         input(json!({"project_root":root}), vec!["project_root"]), false, true),
        ("gil_start_cancel",
         "Cancel the start preparation ONLY on the user's explicit request. Keeps every proposal and approval record and marks it canceled; closing the app or a lost connection is not cancel. A canceled preparation never starts by itself; restart needs the user's explicit wish (gil_start_prepare restart_canceled). After a committed start it changes nothing and reports the existing project.",
         input(json!({"project_root":root}), vec!["project_root"]), false, false),
        ("gil_start_review",
         "App-only: read the exact latest start proposal and a private ephemeral approval capability. Never implies user approval.",
         input(json!({"scope_id":string}), vec!["scope_id"]), true, true),
        ("gil_start_decide",
         "App-only: submit explicit trusted Monitor button input for the displayed scope, revision and digest. Requires the private capability from review.",
         input(json!({"scope_id":string,"revision":{"type":"integer","minimum":1},"digest":string,"token":string,"decision":{"type":"string","enum":["approve","request_changes"]}}),
             vec!["scope_id","revision","digest","token","decision"]), true, false),
        ("gil_start_list",
         "App-only, read-only: one page (500 entries) of the tracked or excluded list of the exact stored proposal revision/digest the Monitor shows, so a shortened list can be read in full before approval. Creates no approval, application or capability.",
         input(json!({"scope_id":string,"revision":{"type":"integer","minimum":1},"digest":string,"list":{"type":"string","enum":["tracked","excluded"]},"offset":{"type":"integer","minimum":0}}),
             vec!["scope_id","revision","digest","list","offset"]), true, true),
        ("gil_start_apply",
         "App-only: on a trusted Monitor click, retry applying the proposal ALREADY approved in the Monitor (after a held application or a lost reply). Requires the private apply capability from review; never creates approval. Under the project lock it rechecks rules, the actual tracked list, path safety and .gilignore, captures the initial Snapshot with the same policy and commits P1, M1, the first Chain and Interview in one save; a repeat after success returns the same start.",
         input(json!({"scope_id":string,"revision":{"type":"integer","minimum":1},"digest":string,"token":string}),
             vec!["scope_id","revision","digest","token"]), true, false),
    ] {
        tools.push(serde_json::from_value(json!({"name":name,"description":description,"inputSchema":schema,
            "annotations":{"readOnlyHint":read,"destructiveHint":false,"openWorldHint":false},
            "_meta":{"ui":{"visibility":if app {vec!["app"]} else {vec!["model"]}}}})).expect("start catalog"));
    }
    tools
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
