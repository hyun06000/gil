//! App capability grants are ephemeral and never part of model-visible content or saved records.
use super::Monitor;
use crate::{
    Project, ProjectSession,
    approved_plan::{Decision, Draft},
};
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::Mutex};

#[derive(Debug, Default)]
pub(super) struct Grants(Mutex<BTreeMap<String, Grant>>);
struct Grant {
    revision: u32,
    digest: String,
    token: String,
}
impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant")
            .field("revision", &self.revision)
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    project_root: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Propose {
    project_root: String,
    draft: Draft,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    scope_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestInterview {
    project_root: String,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decide {
    scope_id: String,
    revision: u32,
    digest: String,
    token: String,
    decision: Decision,
}

pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 72 * 1024) {
        return false;
    }
    match name {
        "gil_plan_propose" => serde_json::from_value::<Propose>(input.clone()).is_ok(),
        "gil_plan_request_interview" => {
            serde_json::from_value::<RequestInterview>(input.clone()).is_ok()
        }
        "gil_plan_read" => serde_json::from_value::<Read>(input.clone()).is_ok(),
        "gil_plan_review" => serde_json::from_value::<Review>(input.clone()).is_ok(),
        "gil_plan_decide" => serde_json::from_value::<Decide>(input.clone()).is_ok(),
        _ => false,
    }
}
fn session(root: &str) -> Result<ProjectSession, String> {
    let root = Path::new(root);
    if !root.is_absolute() {
        return Err("명시한 Project 절대경로가 필요하다".into());
    }
    let state = crate::project_state::state_for_root(root).map_err(|e| e.to_string())?;
    ProjectSession::open_plan_editor(root, &state).map_err(|e| e.to_string())
}
fn view(p: &Project) -> Value {
    let Some(state) = p.approved_plan() else {
        return json!({"supported":false});
    };
    let chain = p.current_chain().unwrap();
    let proposal = state.current(p);
    // Read-only projection of the real current Cycle. `current_item` is a binding that survives
    // the Cycle's close, so only this field says whether that Cycle is still open.
    let cycle = p.current_cycle();
    let current_cycle = json!({"ref":cycle.id().to_ref().to_string(), "state":if cycle.is_closed() {"closed"} else {"open"}});
    json!({"supported":true, "proposal":proposal, "active_revision":state.active(chain).map(|p| p.revision),
        "current_item":p.current_plan_item(), "current_cycle":current_cycle, "pending":state.pending(chain), "reinterview_requests":state.awaiting_interview(chain), "executed_cycles":state.executed_cycles,
        "close_report":proposal.and_then(|p| p.report().ok()).map(|r| crate::approved_plan::report_text(&r)), "document":p.plan_document(),
        "next":"Monitor 응답 후 close_report로 Synthesis를 닫고 Outcome, Interview Cycle까지 닫는다. Experiment Verify는 document와 별도 실행 증거를 파일로 남긴다."})
}
impl Grants {
    fn token(&self, scope: &str, revision: u32, digest: &str) -> Result<String, String> {
        let mut grants = self.0.lock().map_err(|_| "승인 연결을 열지 못했다")?;
        if let Some(g) = grants
            .get(scope)
            .filter(|g| g.revision == revision && g.digest == digest)
        {
            return Ok(g.token.clone());
        }
        if !grants.contains_key(scope) && grants.len() >= 32 {
            return Err("승인 연결 한도".into());
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| "안전한 승인 연결을 만들지 못했다")?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        grants.insert(
            scope.into(),
            Grant {
                revision,
                digest: digest.into(),
                token: token.clone(),
            },
        );
        Ok(token)
    }
    fn check(&self, request: &Decide) -> Result<(), String> {
        let grants = self.0.lock().map_err(|_| "승인 연결을 열지 못했다")?;
        if grants.get(&request.scope_id).is_some_and(|g| {
            g.revision == request.revision && g.digest == request.digest && g.token == request.token
        }) {
            Ok(())
        } else {
            Err("승인 화면 연결이 오래됐거나 일치하지 않는다. Monitor에서 새 제안을 읽는다".into())
        }
    }
}
pub(super) fn call(monitor: &Monitor, grants: &Grants, name: &str, input: Value) -> CallToolResult {
    let run = || -> Result<CallToolResult, String> {
        match name {
            "gil_plan_propose" => {
                let r: Propose = serde_json::from_value(input).map_err(|_| "잘못된 제안 입력")?;
                let mut s = session(&r.project_root)?;
                s.project_mut().propose_plan(r.draft)?;
                s.commit().map_err(|e| e.to_string())?;
                Ok(CallToolResult::structured(view(s.project())))
            }
            "gil_plan_request_interview" => {
                let r: RequestInterview =
                    serde_json::from_value(input).map_err(|_| "잘못된 재인터뷰 요청")?;
                let mut s = session(&r.project_root)?;
                s.project_mut().request_plan_interview(r.reason)?;
                s.commit().map_err(|e| e.to_string())?;
                Ok(CallToolResult::structured(view(s.project())))
            }
            "gil_plan_read" => {
                let r: Read = serde_json::from_value(input).map_err(|_| "잘못된 조회 입력")?;
                let s = session(&r.project_root)?;
                Ok(CallToolResult::structured(view(s.project())))
            }
            "gil_plan_review" => {
                let r: Review = serde_json::from_value(input).map_err(|_| "잘못된 화면 입력")?;
                let root = monitor.root(&r.scope_id).map_err(|e| e.said)?;
                let s = session(root.to_str().ok_or("Project 경로 인코딩")?).map_err(
                    |_| "프로젝트 기록을 읽지 못했다. 다른 작업이 끝난 뒤 다시 확인한다",
                )?;
                monitor.root(&r.scope_id).map_err(|e| e.said)?;
                let value = view(s.project());
                let mut result = serde_json::to_value(CallToolResult::structured(value)).unwrap();
                if let Some(p) = s
                    .project()
                    .approved_plan()
                    .and_then(|state| state.current(s.project()))
                {
                    if p.receipt.is_none() {
                        result["_meta"] = json!({"gil/planApprovalToken": grants.token(&r.scope_id, p.revision, &p.digest)?});
                    }
                }
                serde_json::from_value(result).map_err(|_| "승인 화면 응답을 만들지 못했다".into())
            }
            "gil_plan_decide" => {
                let r: Decide = serde_json::from_value(input).map_err(|_| "잘못된 승인 입력")?;
                grants.check(&r)?;
                let root = monitor.root(&r.scope_id).map_err(|e| e.said)?;
                let mut s = session(root.to_str().ok_or("Project 경로 인코딩")?).map_err(
                    |_| "프로젝트 기록을 읽지 못했다. 다른 작업이 끝난 뒤 다시 확인한다",
                )?;
                monitor.root(&r.scope_id).map_err(|e| e.said)?;
                s.project_mut()
                    .decide_plan(r.revision, &r.digest, r.decision)?;
                s.commit().map_err(
                    |_| "승인 저장 결과를 확인하지 못했다. 다시 조회하고 기록 여부를 확인한다",
                )?;
                Ok(CallToolResult::structured(view(s.project())))
            }
            _ => Err("알 수 없는 승인 도구".into()),
        }
    };
    run().unwrap_or_else(|why| super::actions::refusal(&why))
}

pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    let input = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let string = json!({"type":"string"});
    let draft = json!({"type":"object", "additionalProperties":false, "required":["scope","items","synthesis"], "properties":{
        "scope":{"type":"object","additionalProperties":false,"required":["goal","in_scope","out_of_scope","constraints","success_criteria"],"properties":{
            "goal":string,"in_scope":string,"out_of_scope":string,"constraints":string,"success_criteria":string}},
        "items":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"object","additionalProperties":false,"required":["id","question","done_when"],"properties":{"id":string,"question":string,"done_when":string}}},
        "synthesis":{"type":"object","additionalProperties":{"type":"string"},"description":"Complete Synthesis Report fields, including statement, basis_refs and exploration_checklist (item IDs, one per line in order); omit approved, plan_revision, plan_digest."}}});
    for (name, description, schema, app, read) in [
        (
            "gil_plan_propose",
            "Propose the complete scope and ordered experiment checklist in an OPEN Interview Synthesis. No file creation. Wait for the user's Monitor button; chat approval is insufficient. Then gil_plan_read and close Synthesis, Outcome and Interview before execution.",
            input(
                json!({"project_root":string,"draft":draft}),
                vec!["project_root", "draft"],
            ),
            false,
            false,
        ),
        (
            "gil_plan_request_interview",
            "Preserve a scope-change request during an Experiment. Does not close anything, open another Cycle or approve scope. Finish the current Cycle truthfully, then open a new Interview. Never fabricate evidence to escape an unfinished Step.",
            input(
                json!({"project_root":string,"reason":string}),
                vec!["project_root", "reason"],
            ),
            false,
            false,
        ),
        (
            "gil_plan_read",
            "Read proposal/Monitor decision and exact close_report. Use close_report unchanged with gil_close. In Experiment, save document verbatim to a file during Verify; name plan_document and separate evidence_files. Never invent approval.",
            input(json!({"project_root":string}), vec!["project_root"]),
            false,
            true,
        ),
        (
            "gil_plan_review",
            "App-only: read exact proposal and private ephemeral approval capability. Never implies user approval.",
            input(json!({"scope_id":string}), vec!["scope_id"]),
            true,
            true,
        ),
        (
            "gil_plan_decide",
            "App-only: submit explicit trusted Monitor button input for the displayed scope, revision and digest. Requires the private capability from review.",
            input(
                json!({"scope_id":string,"revision":{"type":"integer","minimum":1},"digest":string,"token":string,"decision":{"type":"string","enum":["approve","request_changes"]}}),
                vec!["scope_id", "revision", "digest", "token", "decision"],
            ),
            true,
            false,
        ),
    ] {
        tools.push(
            serde_json::from_value(
                json!({"name":name,"description":description,"inputSchema":schema,
            "annotations":{"readOnlyHint":read,"destructiveHint":false,"openWorldHint":false},
            "_meta":{"ui":{"visibility":if app {vec!["app"]} else {vec!["model"]}}}}),
            )
            .expect("plan catalog"),
        );
    }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_test_helpers as h;
    #[test]
    fn approved_plan_private_token_stale_revision_restart_and_idempotent_receipt() {
        let root = h::scratch("plan-mcp-capability");
        let mut p = Project::start(h::spec(), h::first_world());
        let draft = crate::approved_plan::tests::synthesis(&mut p, &["A", "B"]);
        crate::save(&p, root.join(crate::STATE_PATH)).unwrap();
        let monitor = Monitor::with_store(super::super::bindings::Store::at(
            root.with_extension("consent"),
        ));
        let scope = monitor.prepare(root.to_str().unwrap()).unwrap()["scope_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let grants = Grants::default();
        let propose = |draft: &Draft| {
            call(
                &monitor,
                &grants,
                "gil_plan_propose",
                json!({"project_root":root,"draft":draft}),
            )
        };
        let result = propose(&draft);
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let before = std::fs::read(root.join(crate::STATE_PATH)).unwrap();
        let read = call(
            &monitor,
            &grants,
            "gil_plan_review",
            json!({"scope_id":scope}),
        );
        let wire = serde_json::to_value(read).unwrap();
        let token = wire["_meta"]["gil/planApprovalToken"].as_str().unwrap();
        assert_eq!(token.len(), 64);
        assert!(!format!("{grants:?}").contains(token));
        assert!(!wire["structuredContent"].to_string().contains(token));
        assert!(!wire["content"].to_string().contains(token));
        assert_eq!(std::fs::read(root.join(crate::STATE_PATH)).unwrap(), before);
        let p = &wire["structuredContent"]["proposal"];
        let mut request = json!({"scope_id":scope,"revision":p["revision"],"digest":p["digest"],"token":token,"decision":"approve"});
        let mut wrong = request.clone();
        wrong["scope_id"] = json!("project:other");
        assert_eq!(
            call(&monitor, &grants, "gil_plan_decide", wrong).is_error,
            Some(true)
        );
        assert_eq!(
            call(
                &monitor,
                &Grants::default(),
                "gil_plan_decide",
                request.clone()
            )
            .is_error,
            Some(true)
        );
        assert_eq!(std::fs::read(root.join(crate::STATE_PATH)).unwrap(), before);
        let mut changed = draft.clone();
        changed.scope.goal = "New scope".into();
        assert_ne!(propose(&changed).is_error, Some(true));
        assert_eq!(
            call(&monitor, &grants, "gil_plan_decide", request.clone()).is_error,
            Some(true)
        );
        let restarted = Grants::default();
        let new_wire = serde_json::to_value(call(
            &monitor,
            &restarted,
            "gil_plan_review",
            json!({"scope_id":scope}),
        ))
        .unwrap();
        request["revision"] = new_wire["structuredContent"]["proposal"]["revision"].clone();
        request["digest"] = new_wire["structuredContent"]["proposal"]["digest"].clone();
        request["token"] = new_wire["_meta"]["gil/planApprovalToken"].clone();
        assert_ne!(request["token"], json!(token));
        let accepted = call(&monitor, &restarted, "gil_plan_decide", request.clone());
        assert_ne!(accepted.is_error, Some(true), "{accepted:?}");
        let saved = std::fs::read(root.join(crate::STATE_PATH)).unwrap();
        assert_ne!(
            call(&monitor, &restarted, "gil_plan_decide", request).is_error,
            Some(true)
        );
        assert_eq!(std::fs::read(root.join(crate::STATE_PATH)).unwrap(), saved);
        assert!(!String::from_utf8(saved).unwrap().contains(token));
        let read = call(
            &monitor,
            &Grants::default(),
            "gil_plan_read",
            json!({"project_root":root}),
        );
        let value = read.structured_content.unwrap();
        assert_eq!(value["proposal"]["receipt"]["decision"], "approve");
        let report = crate::Report::parse(value["close_report"].as_str().unwrap()).unwrap();
        let mut s = session(root.to_str().unwrap()).unwrap();
        s.close_step(report).unwrap();
        s.commit().unwrap();
    }
    /// R3 (Wave 1 review): `current_item` stays bound after its Cycle closes, so the view must
    /// project the real Cycle state separately. Real Core, no mock: the Experiment is walked and
    /// closed through the public Project API.
    #[test]
    fn approved_plan_view_projects_real_current_cycle_state_beside_the_binding() {
        use crate::{CycleKind, NodeKind};
        let mut p = Project::start(h::spec(), h::first_world());
        assert!(view(&p)["current_cycle"].is_object(), "supported projects always carry current_cycle");
        let draft = crate::approved_plan::tests::synthesis(&mut p, &["A", "B"]);
        let proposal = p.propose_plan(draft).unwrap();
        let receipt = p
            .decide_plan(proposal.revision, &proposal.digest, Decision::Approve)
            .unwrap();
        let synthesis = receipt.synthesis_ref.clone();
        p.close_action_step(receipt.report().unwrap()).unwrap();
        let outcome = h::opened(&mut p, NodeKind::Outcome);
        p.close_action_step(
            h::full_report(p.current_cycle().rules(), CycleKind::Interview, NodeKind::Outcome)
                .with("synthesis_ref", synthesis),
        )
        .unwrap();
        p.close_cycle(
            h::cycle_report(p.current_cycle(), "success", h::step_id(&p, outcome))
                .with("next_direction.action", "open_child"),
        )
        .unwrap();
        let interview = view(&p);
        assert_eq!(interview["current_cycle"]["state"], "closed");
        assert_eq!(interview["current_item"], Value::Null, "nothing is bound before the Experiment opens");
        p.open_child_cycle(CycleKind::Experiment).unwrap();
        let open = view(&p);
        let experiment = p.current_cycle().id().to_ref().to_string();
        assert_eq!(open["current_cycle"], json!({"ref":experiment, "state":"open"}));
        assert_eq!(open["current_item"]["id"], "A");
        assert_eq!(open["executed_cycles"][0]["cycle_ref"], json!(experiment));
        for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis, NodeKind::Outcome] {
            h::walked(&mut p, kind);
        }
        let outcome = p.current_cycle().steps().current().unwrap();
        p.close_cycle(
            h::cycle_report(p.current_cycle(), "success", outcome)
                .with("next_direction.action", "open_child"),
        )
        .unwrap();
        assert!(p.current_cycle().is_closed());
        let closed = view(&p);
        assert_eq!(closed["current_cycle"], json!({"ref":experiment, "state":"closed"}));
        assert_eq!(closed["current_item"]["id"], "A", "the binding is kept; it is not an open-state witness");
        assert_eq!(closed["active_revision"], open["active_revision"]);
        assert_eq!(closed["pending"], open["pending"], "the projection changes no approval or queue meaning");
        assert_eq!(closed["executed_cycles"], open["executed_cycles"]);
        assert_eq!(view(&p).as_object().unwrap().keys().filter(|k| *k == "current_cycle").count(), 1);
    }
    #[test]
    fn approved_plan_wire_schema_is_strict_and_app_tools_are_not_model_tools() {
        for tool in catalog(vec![]) {
            let v = serde_json::to_value(&tool).unwrap();
            let app = tool.name == "gil_plan_review" || tool.name == "gil_plan_decide";
            assert_eq!(
                v["_meta"]["ui"]["visibility"],
                if app {
                    json!(["app"])
                } else {
                    json!(["model"])
                }
            );
        }
        assert!(!valid(
            "gil_plan_decide",
            &json!({"scope_id":"s","revision":1,"digest":"d","token":"t","decision":"approve","approved":true})
        ));
        assert!(!valid(
            "gil_plan_propose",
            &json!({"project_root":"/","draft":{"approved":true}})
        ));
        assert!(!valid(
            "gil_plan_decide",
            &json!({"scope_id":"s","revision":-1,"digest":"d","token":"t","decision":"approve"})
        ));
    }
}
