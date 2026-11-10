//! App-only approval for an exact, freshly revalidated recovery plan. Tokens never enter model content.
use super::Monitor;
use crate::{ProjectSession, RuleSet};
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Default)]
pub(super) struct Grants(Mutex<BTreeMap<String, (String, String)>>);
impl std::fmt::Debug for Grants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryGrants(<private>)")
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    scope_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    scope_id: String,
    digest: String,
    token: String,
}
pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 4096) {
        return false;
    }
    match name {
        "gil_revisit_review" => {
            serde_json::from_value::<Review>(input.clone()).is_ok_and(|r| !r.scope_id.is_empty())
        }
        "gil_revisit_apply" => serde_json::from_value::<Apply>(input.clone()).is_ok(),
        _ => false,
    }
}
pub(super) fn call(monitor: &Monitor, grants: &Grants, name: &str, input: Value) -> CallToolResult {
    let run = || -> Result<CallToolResult, String> {
        let scope = input["scope_id"]
            .as_str()
            .ok_or(crate::locale::text("recovery_tool.scope_required"))?;
        let root = monitor.root(scope).map_err(|e| e.said)?;
        let state = crate::project_state::state_for_root(&root).map_err(|e| e.to_string())?;
        let rules = RuleSet::builtin().map_err(|e| e.to_string())?;
        if name == "gil_revisit_review" {
            let s = ProjectSession::open_read_only_at(rules, &root, &state)
                .map_err(|e| e.to_string())?;
            let value = s.revisit_review()?;
            monitor.root(scope).map_err(|e| e.said)?;
            let mut out = serde_json::to_value(CallToolResult::structured(value.clone())).unwrap();
            if let Some(digest) = value["digest"].as_str() {
                let mut entries = grants.0.lock().map_err(|_| crate::locale::text("recovery_tool.grants_lock"))?;
                if !entries.contains_key(scope) && entries.len() >= 32 {
                    return Err(crate::locale::text("recovery_tool.grants_limit").into());
                }
                if entries.get(scope).is_none_or(|(d, _)| d != digest) {
                    let mut bytes = [0u8; 32];
                    getrandom::fill(&mut bytes).map_err(|_| crate::locale::text("recovery_tool.grants_random"))?;
                    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                    entries.insert(scope.to_owned(), (digest.to_owned(), token));
                }
                out["_meta"] = json!({"gil/revisitApprovalToken":entries[scope].1});
            }
            return serde_json::from_value(out).map_err(|e| e.to_string());
        }
        let request: Apply = serde_json::from_value(input).map_err(|e| e.to_string())?;
        {
            let entries = grants.0.lock().map_err(|_| crate::locale::text("recovery_tool.grants_lock"))?;
            if !entries
                .get(&request.scope_id)
                .is_some_and(|(d, t)| d == &request.digest && t == &request.token)
            {
                return Err(crate::locale::text("recovery_tool.approve_in_monitor").into());
            }
        }
        let mut s = ProjectSession::open_at(rules, &root, &state).map_err(|e| e.to_string())?;
        monitor.root(&request.scope_id).map_err(|e| e.said)?;
        Ok(CallToolResult::structured(
            s.approve_revisit(&request.digest)?,
        ))
    };
    run().unwrap_or_else(|why| super::actions::refusal(&why))
}
pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    for (name, apply) in [("gil_revisit_review", false), ("gil_revisit_apply", true)] {
        let properties = if apply {
            json!({"scope_id":{"type":"string"},"digest":{"type":"string"},"token":{"type":"string"}})
        } else {
            json!({"scope_id":{"type":"string"}})
        };
        tools.push(serde_json::from_value(json!({"name":name,
            "description":if apply {"App-only: preserve originals and execute the exact recovery plan after the user's Monitor button. Requires private review capability. Never fabricate approval."} else {"App-only: read the complete recovery plan without changing files or state; receive a private approval capability."},
            "inputSchema":{"type":"object","properties":properties,"required":if apply {vec!["scope_id","digest","token"]} else {vec!["scope_id"]},"additionalProperties":false},
            "annotations":{"readOnlyHint":!apply,"destructiveHint":apply,"openWorldHint":false},"_meta":{"ui":{"visibility":["app"]}}})).unwrap());
    }
    tools
}
