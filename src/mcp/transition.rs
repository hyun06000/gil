//! Model-visible development tools for the explicit storage-format transition 6 → 8 / 7 → 9
//! (Storage Transition Contract §7, implementation order 1b). Registered only in a
//! `will-end-history` build; no product profile enables it.
//!
//! `user_approved` is the Agent's relay of the user's explicit consent in the conversation
//! (contract §12 O-1) — not authentication, never stored. No UI resource, no Monitor button, no
//! implicit transition, recovery, cleanup or automatic retry. Kept apart from the format 4 → 5
//! visual migration tools (`gil_migration_*`), whose meaning and behavior this does not change.
use crate::session::format_transition::{self as core, Approval, Class, Refusal};
use crate::RuleSet;
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;

pub(super) const CHECK: &str = "gil_format_transition_check";
pub(super) const APPLY: &str = "gil_format_transition_apply";
pub(super) const NAMES: [&str; 2] = [CHECK, APPLY];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    project_root: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    project_root: String,
    current_format: u32,
    target_format: u32,
    state_sha256: String,
    proposal_sha256: String,
    user_approved: bool,
}

pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    tools.extend(serde_json::from_str::<Vec<Tool>>(include_str!("transition-tools.json")).expect("transition catalog"));
    tools
}

fn root_ok(root: &str) -> bool {
    Path::new(root).is_absolute() && !root.contains('\0')
}
fn digest_ok(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Exact shape only: every key present and non-null, nothing extra, absolute root, canonical
/// digests. A wire refusal reads and changes nothing.
pub(super) fn valid(name: &str, input: &Value) -> bool {
    if input.as_object().is_some_and(|o| o.values().any(Value::is_null)) {
        return false;
    }
    match name {
        CHECK => serde_json::from_value::<Check>(input.clone()).is_ok_and(|r| root_ok(&r.project_root)),
        APPLY => serde_json::from_value::<Apply>(input.clone())
            .is_ok_and(|r| root_ok(&r.project_root) && digest_ok(&r.state_sha256) && digest_ok(&r.proposal_sha256)),
        _ => false,
    }
}

pub(super) fn rejection(name: &str) -> Option<&'static str> {
    match name {
        CHECK => Some("gil_format_transition_check: expected exactly {project_root (absolute string)}. Nothing was read or changed."),
        APPLY => Some("gil_format_transition_apply: expected exactly {project_root (absolute string), current_format (integer), target_format (integer), state_sha256 and proposal_sha256 (64 lowercase hex digits copied from the check's apply_with), user_approved (boolean)}; no null, no extra field. A missing approval is not an approval. Nothing was read or changed."),
        _ => None,
    }
}

fn refusal(refusal: Refusal) -> CallToolResult {
    let class = match refusal.class {
        Class::Refused => "refused",
        Class::NotApplied => "not_applied",
        Class::Unconfirmed => "unconfirmed",
    };
    let mut body = json!({"ok": false, "code": refusal.code, "class": class, "said": refusal.said,
        "state_changed": match refusal.class { Class::Unconfirmed => Value::from("unknown"), _ => Value::Bool(false) },
        "automatic_retry": false});
    if let (Some(fields), Some(facts)) = (body.as_object_mut(), refusal.facts.as_object()) {
        for (key, value) in facts {
            fields.insert(key.clone(), value.clone());
        }
    }
    let mut out = CallToolResult::structured(body);
    out.is_error = Some(true);
    out
}

fn located(root: &str) -> Result<(RuleSet, std::path::PathBuf), CallToolResult> {
    let fail = |code: &'static str, said: String| refusal(Refusal { code, class: Class::Refused, said, facts: json!({}) });
    if !Path::new(root).is_dir() {
        return Err(fail("not_a_project", "project_root is not an existing directory. Nothing was read or changed.".into()));
    }
    let rules = RuleSet::builtin().map_err(|e| fail("record_unreadable", e.to_string()))?;
    let state = crate::project_state::state_for_root(Path::new(root)).map_err(|e| fail("not_a_project", e))?;
    Ok((rules, state))
}

pub(super) fn call(name: &str, input: Value) -> CallToolResult {
    match name {
        CHECK => {
            let Ok(request) = serde_json::from_value::<Check>(input) else { return refusal(core_invalid()); };
            let (rules, state) = match located(&request.project_root) { Ok(v) => v, Err(out) => return out };
            match core::check(rules, Path::new(&request.project_root), &state) {
                Ok(body) => CallToolResult::structured(body),
                Err(error) => refusal(error),
            }
        }
        APPLY => {
            let Ok(request) = serde_json::from_value::<Apply>(input) else { return refusal(core_invalid()); };
            let approval = Approval {
                current_format: request.current_format,
                target_format: request.target_format,
                state_sha256: &request.state_sha256,
                proposal_sha256: &request.proposal_sha256,
                user_approved: request.user_approved,
            };
            // A declined or missing approval stops before the project is even located.
            if !approval.user_approved {
                return refusal(Refusal { code: "approval_required", class: Class::Refused, said: "user_approved is false: the user has not approved this proposal (or declined it). Nothing was read or changed. Do not apply without the user's explicit agreement in the conversation.".into(), facts: json!({}) });
            }
            let (rules, state) = match located(&request.project_root) { Ok(v) => v, Err(out) => return out };
            match core::apply(rules, Path::new(&request.project_root), &state, &approval) {
                Ok(body) => CallToolResult::structured(body),
                Err(error) => refusal(error),
            }
        }
        _ => refusal(core_invalid()),
    }
}

fn core_invalid() -> Refusal {
    Refusal { code: "invalid_request", class: Class::Refused, said: "Use the exact connected schema. Nothing was read or changed.".into(), facts: json!({}) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_are_exact_and_every_value_is_required() {
        let root = if cfg!(windows) { "C:/synthetic" } else { "/synthetic" };
        let digest = "a".repeat(64);
        let full = json!({"project_root": root, "current_format": 6, "target_format": 8,
            "state_sha256": digest, "proposal_sha256": digest, "user_approved": true});
        assert!(valid(APPLY, &full));
        assert!(valid(CHECK, &json!({"project_root": root})));
        for key in full.as_object().unwrap().keys() {
            let mut missing = full.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(!valid(APPLY, &missing), "missing {key}");
            let mut null = full.clone();
            null[key] = Value::Null;
            assert!(!valid(APPLY, &null), "null {key}");
        }
        for (key, wrong) in [("user_approved", json!("yes")), ("current_format", json!("6")), ("state_sha256", json!("A".repeat(64))),
            ("proposal_sha256", json!("a".repeat(63))), ("project_root", json!("relative/path"))] {
            let mut bad = full.clone();
            bad[key] = wrong;
            assert!(!valid(APPLY, &bad), "{key}");
        }
        let mut extra = full.clone();
        extra["approve_everything"] = json!(true);
        assert!(!valid(APPLY, &extra));
        assert!(!valid(CHECK, &json!({"project_root": root, "backup_root": root})));
        // Kept apart from the visual migration tools: no name is shared or overloaded.
        for tool in catalog(Vec::new()) {
            assert!(!tool.name.starts_with("gil_migration_"));
            let wire = serde_json::to_value(&tool).unwrap();
            assert_eq!(wire["_meta"]["ui"]["visibility"], json!(["model"]));
            assert!(wire["_meta"]["ui"].get("resourceUri").is_none(), "no Monitor surface for a transition");
        }
    }

    #[test]
    fn a_declined_approval_reads_nothing() {
        let out = call(APPLY, json!({"project_root": "/nonexistent-gil-transition-root", "current_format": 6, "target_format": 8,
            "state_sha256": "a".repeat(64), "proposal_sha256": "b".repeat(64), "user_approved": false}));
        let body = out.structured_content.unwrap();
        assert_eq!(body["code"], "approval_required");
        assert_eq!(body["state_changed"], false);
    }
}
