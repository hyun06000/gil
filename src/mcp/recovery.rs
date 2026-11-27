//! Read-only view of the recovery a closed failed Experiment would take. Nothing here approves or
//! runs it: the agent's own `gil_revisit` carries a declared revisit out (session::revisit_recovery).
use super::Monitor;
use crate::{ProjectSession, RuleSet};
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    scope_id: String,
}
pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 4096) {
        return false;
    }
    name == "gil_revisit_review" && serde_json::from_value::<Review>(input.clone()).is_ok_and(|r| !r.scope_id.is_empty())
}
pub(super) fn call(monitor: &Monitor, name: &str, input: Value) -> CallToolResult {
    let run = || -> Result<CallToolResult, String> {
        if name != "gil_revisit_review" { return Err(crate::locale::text("action.disallowed").into()); }
        let scope = input["scope_id"]
            .as_str()
            .ok_or(crate::locale::text("recovery_tool.scope_required"))?;
        let root = monitor.root(scope).map_err(|e| e.said)?;
        let state = crate::project_state::state_for_root(&root).map_err(|e| e.to_string())?;
        let rules = RuleSet::builtin().map_err(|e| e.to_string())?;
        let s = ProjectSession::open_read_only_at(rules, &root, &state).map_err(|e| e.to_string())?;
        Ok(CallToolResult::structured(s.revisit_review()?))
    };
    run().unwrap_or_else(|why| super::actions::refusal(&why))
}
pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    tools.push(serde_json::from_value(json!({"name":"gil_revisit_review",
        "description":"App-only, read-only: the complete recovery plan a declared revisit would carry out (what returns to the target world and where the replaced files are preserved). Changes no file and no state. It approves nothing and nothing needs approval: the agent's gil_revisit carries the revisit out.",
        "inputSchema":{"type":"object","properties":{"scope_id":{"type":"string"}},"required":["scope_id"],"additionalProperties":false},
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false},"_meta":{"ui":{"visibility":["app"]}}})).unwrap());
    tools
}
