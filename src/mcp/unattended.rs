//! Unattended sessions: no Monitor and no person. Whoever started the server answers for the user.
//!
//! ```text
//! runner  GIL_MCP_DISPLAY=headless GIL_MCP_APPROVAL=unattended gil mcp --serve
//! model   the ordinary tools, minus every display and App tool
//! runner  gil_unattended_review(project_root)                         what waits for the user, exactly as stored
//!         gil_unattended_decide(project_root, subject, digest, decision)  the answer, for that exact digest
//! ```
//!
//! The two runner tools carry `visibility: ["runner"]`: the runner keeps them out of the tool list
//! it hands the model, as a Host keeps App tools out. This process cannot tell callers apart, so
//! the separation is the runner's routing and is not enforced here. Every approval written on this
//! path is marked `unattended_runner` (`crate::approval_source`); it is never recorded as a Monitor
//! button. The mode is chosen only by the environment of the process, never by a tool input, a
//! project file or the absence of a display, and it does not carry over to another session.
//!
//! Covered: the tracked-scope start proposal and the plan proposal of an open Synthesis — the two
//! things a user decides. Going back after a failure and restoring a changed world need no answer:
//! the agent's own gil_revisit and gil_restore carry them out. Scope changes, baseline retries and
//! archive halts have no answer here and keep waiting.
// A build without one of the three approvals keeps this module whole; the absent subject answers "nothing waits".
#![cfg_attr(not(all(feature = "approved-plan", feature = "tracking-policy", unix)), allow(dead_code, unused_variables, unreachable_code))]
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) const ENV: &str = "GIL_MCP_APPROVAL";
pub(super) const MODE: &str = "unattended";
pub(super) const REVIEW: &str = "gil_unattended_review";
pub(super) const DECIDE: &str = "gil_unattended_decide";
pub(super) const NAMES: &[&str] = &[REVIEW, DECIDE];

/// What the model reads first in this mode. It overrides the display prose of the shared workflow.
pub(super) const PREFACE: &str = "This session is unattended: there is no Monitor, no screen and no display tool. Wherever the workflow below or a receipt names the Monitor, a Monitor button, show_gil_monitor, gil_monitor_* or fullscreen, read it as: the user's decision is given outside this conversation and arrives recorded. Never try to open, prepare or check a display. When a receipt lists awaiting_user, say what you proposed, end your turn and call nothing else; when you are continued, read gil_context (or gil_start_status before the project has started, gil_plan_read for a plan) and follow the recorded state. Chat consent and your own input are still never approval, and you have no tool that approves anything.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review { project_root: String }
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Subject { Start, Plan }
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Decision { Approve, RequestChanges }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decide { project_root: String, subject: Subject, digest: String, decision: Decision }

pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 4096) { return false; }
    match name {
        REVIEW => serde_json::from_value::<Review>(input.clone()).is_ok(),
        DECIDE => serde_json::from_value::<Decide>(input.clone()).is_ok(),
        _ => false,
    }
}

fn root_of(given: &str) -> Result<PathBuf, String> {
    let path = Path::new(given);
    if !path.is_absolute() { return Err(crate::RootError::NotAbsolute.to_string()); }
    path.canonicalize().map_err(|_| super::actions::root_unreadable().to_owned())
}

fn started(root: &Path) -> bool {
    crate::project_state::state_for_root(root).map(|state| state.is_file()).unwrap_or(false)
}

/// The start proposal that waits for a decision, or for the retry of an approved application.
#[cfg(all(feature = "tracking-policy", unix))]
fn start(root: &Path) -> Option<(Value, u32, String, crate::start_prep::Purpose)> {
    let (value, grant) = crate::start_prep::review(root).ok()?;
    let (revision, digest, purpose) = grant?;
    Some((value, revision, digest, purpose))
}

#[cfg(feature = "approved-plan")]
fn plan_session(root: &Path) -> Result<crate::ProjectSession, String> {
    let state = crate::project_state::state_for_root(root).map_err(|e| e.to_string())?;
    crate::ProjectSession::open_plan_editor(root, &state).map_err(|e| e.to_string())
}

/// Everything that waits for the user in this project, read from the stored records. Writes nothing.
pub(super) fn pending(root: &Path) -> Vec<Value> {
    #[allow(unused_mut)]
    let mut waits: Vec<Value> = Vec::new();
    #[cfg(all(feature = "tracking-policy", unix))]
    if let Some((value, revision, digest, purpose)) = start(root) {
        let retry = purpose == crate::start_prep::Purpose::Apply;
        waits.push(json!({"subject":"start", "revision":revision, "digest":digest,
            "kind":if retry {"apply_approved"} else {"decide"}, "proposal":value["proposal"], "current":value["current"]}));
    }
    if !started(root) { return waits; }
    #[cfg(feature = "approved-plan")]
    if let Ok(session) = plan_session(root) {
        let project = session.project();
        if let Some(proposal) = project.approved_plan().and_then(|state| state.current(project)).filter(|p| p.receipt.is_none()) {
            waits.push(json!({"subject":"plan", "revision":proposal.revision, "digest":proposal.digest,
                "synthesis_ref":proposal.synthesis_ref, "draft":proposal.draft}));
        }
    }
    waits
}

/// The line a model-facing receipt gets while something waits. Names subjects only.
pub(super) fn note(waits: &[Value]) -> Option<(String, Value)> {
    if waits.is_empty() { return None; }
    let subjects: Vec<&str> = waits.iter().filter_map(|wait| wait["subject"].as_str()).collect();
    Some((format!("Awaiting the user's decision ({}). This session has no Monitor: the decision is given outside this conversation. Tell the user what you proposed, end your turn and call nothing else until you are continued.", subjects.join(", ")),
        json!(subjects)))
}

pub(super) fn call(name: &str, input: Value) -> CallToolResult {
    let run = || -> Result<Value, String> {
        if name == REVIEW {
            let request: Review = serde_json::from_value(input).map_err(|e| e.to_string())?;
            let root = root_of(&request.project_root)?;
            return Ok(json!({"ok":true, "approval_source":crate::approval_source::UNATTENDED, "pending":pending(&root)}));
        }
        let request: Decide = serde_json::from_value(input).map_err(|e| e.to_string())?;
        let root = root_of(&request.project_root)?;
        let approve = request.decision == Decision::Approve;
        let stale = || "Nothing with this digest waits for the user. Nothing was recorded; read gil_unattended_review again.".to_owned();
        let result: Value = match request.subject {
            #[cfg(all(feature = "tracking-policy", unix))]
            Subject::Start => {
                use crate::start_prep as sp;
                let (_, revision, digest, purpose) = start(&root).ok_or_else(stale)?;
                if digest != request.digest { return Err(stale()); }
                match purpose {
                    sp::Purpose::Decide => sp::decide_from(&root, revision, &digest, approve, crate::approval_source::UNATTENDED),
                    sp::Purpose::Apply if approve => sp::apply(&root, revision, &digest),
                    sp::Purpose::Apply => return Err("This proposal is already approved; only its application can be retried. Nothing was recorded.".into()),
                }.map_err(|refusal| refusal.said)?
            }
            #[cfg(not(all(feature = "tracking-policy", unix)))]
            Subject::Start => return Err(stale()),
            #[cfg(feature = "approved-plan")]
            Subject::Plan => {
                let mut session = plan_session(&root)?;
                let project = session.project();
                let revision = project.approved_plan().and_then(|state| state.current(project))
                    .filter(|p| p.receipt.is_none() && p.digest == request.digest).map(|p| p.revision).ok_or_else(stale)?;
                let decision = if approve { crate::approved_plan::Decision::Approve } else { crate::approved_plan::Decision::RequestChanges };
                let decided = session.project_mut().decide_plan_from(revision, &request.digest, decision, crate::approval_source::UNATTENDED)?;
                session.commit().map_err(|e| e.to_string())?;
                json!({"revision":decided.revision, "digest":decided.digest, "receipt":decided.receipt})
            }
            #[cfg(not(feature = "approved-plan"))]
            Subject::Plan => return Err(stale()),
        };
        Ok(json!({"ok":true, "approval_source":crate::approval_source::UNATTENDED, "result":result, "pending":pending(&root)}))
    };
    match run() {
        Ok(value) => CallToolResult::structured(value),
        Err(why) => super::actions::refusal(why),
    }
}

/// The tool list of an unattended session: nothing that shows, prepares or reads a display,
/// nothing a Host would reserve for the App, and the two runner tools.
pub(super) fn catalog(tools: &[Tool]) -> Vec<Tool> {
    let app_only = |tool: &Tool| serde_json::to_value(tool).ok()
        .is_some_and(|wire| wire["_meta"]["ui"]["visibility"] == json!(["app"]));
    let display = |name: &str| name.starts_with("gil_monitor_") || matches!(name, "show_gil_monitor" | "gil_companion_status" | "show_gil_companion");
    let mut kept: Vec<Tool> = tools.iter().filter(|tool| !display(tool.name.as_ref()) && !app_only(tool)).cloned().collect();
    let root = json!({"type":"string","description":"The absolute project folder of this run."});
    for (name, description, properties, required, read) in [
        (REVIEW, "Runner-only, never given to the model: read what waits for the user in this unattended session, exactly as stored (start proposal, plan proposal). Writes nothing and approves nothing.",
            json!({"project_root":root}), vec!["project_root"], true),
        (DECIDE, "Runner-only, never given to the model: record the answer of whoever stands in for the user, for the exact digest gil_unattended_review returned. The record marks the approval unattended_runner, never a Monitor button.",
            json!({"project_root":root,"subject":{"type":"string","enum":["start","plan"]},"digest":{"type":"string"},
                "decision":{"type":"string","enum":["approve","request_changes"]}}),
            vec!["project_root","subject","digest","decision"], false),
    ] {
        kept.push(serde_json::from_value(json!({"name":name,"description":description,
            "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
            "annotations":{"readOnlyHint":read,"destructiveHint":!read,"openWorldHint":false},
            "_meta":{"ui":{"visibility":["runner"]}}})).expect("unattended catalog"));
    }
    kept
}
