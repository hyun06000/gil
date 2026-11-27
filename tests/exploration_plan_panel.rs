//! Real Core -> `gil_plan_review` -> the existing approval panel, for an exploration Chain.
//!
//! The approval panel (mcp-app/plans.mjs) reads what `gil_plan_review` returns. This file walks
//! one exploration Chain over the real `gil mcp --serve` process and keeps the App-path response
//! at each position, unmodified, in `mcp-app/fixtures/exploration-plan-review.json`. The panel
//! test (mcp-app/plans.test.mjs) renders those responses, so the link between the server and the
//! screen is not a hand-written wire shape.
//!
//! Positions kept, in order: an approved plan with nothing linked; an open Experiment on R1; the
//! same Experiment closed; a second Experiment on R1 closed as a failure; after the approved
//! revisit, an open Experiment on **R2** (the current Experiment moved to another requirement);
//! that Experiment closed; a later Interview with no bound requirement; a new proposal waiting
//! for a decision; the same proposal with its recorded approval.
//!
//! Scope of the evidence: synthetic tool-path input. The App calls are harness input, not a
//! person's click, and nothing here shows a rendered browser screen or what a reader understands.
//!
//! Generate: `GIL_EXPLORATION_PLAN_WRITE=1 cargo test --locked -p gil --features
//! exploration-flow,visual-authoring,product-safety --test exploration_plan_panel`. Without the
//! variable the test compares a fresh run with the committed file and writes nothing.
#![cfg(all(
    feature = "exploration-flow",
    feature = "product-safety",
    feature = "visual-authoring",
    any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)
))]
#[path = "support_wave1/mod.rs"]
mod support;
#[path = "support_wave1/exploration.rs"]
mod exploration;
use exploration::*;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::*;

/// The App-path response as the panel receives it. The one-time approval capability travels in
/// `_meta` and is never part of what is kept.
fn review(r: &mut Rig, name: &str, kept: &mut Vec<Value>) -> Value {
    let frame = ok(r.app("gil_plan_review", json!({})), name);
    let view = structured(&frame).clone();
    if let Some(secret) = token(&frame) {
        assert!(!view.to_string().contains(&secret), "{name}: the capability leaked into the view");
    }
    // The same projection, except the two bodies the model sends on: it reads those as the objects
    // gil_close and gil_open take, while the App keeps the stored text.
    let without_bodies = |mut value: Value| {
        for field in ["close_report", "experiment_link"] { value.as_object_mut().unwrap().remove(field); }
        // The model reads an item's observation under that name; the App keeps the stored names.
        body_shim::as_stored(value)
    };
    assert_eq!(without_bodies(view.clone()), without_bodies(r.read()), "{name}: the App and the model read the same projection");
    kept.push(json!({"name":name, "review":view}));
    view
}

fn requirement<'a>(view: &'a Value, id: &str) -> &'a Value {
    view["requirements"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap_or_else(|| panic!("requirement {id}: {view}"))
}
fn links(view: &Value) -> Vec<(String, String)> {
    view["executed_cycles"].as_array().unwrap().iter()
        .map(|e| (e["cycle_ref"].as_str().unwrap().to_owned(), e["item_id"].as_str().unwrap().to_owned())).collect()
}
fn text(id: &str) -> Value {
    // The App reads the stored item: its deciding observation under `done_when`, and the source the wire recorded.
    json!({"id":id, "question":format!("항목 {id}을 합성 자료로 확인한다"), "done_when":format!("evidence 파일에 항목 {id} 결과가 저장된다"),
        "source":"the synthetic request of this test", "command":"cat evidence/result.md", "part_of":"M1"})
}

#[test]
fn the_review_response_keeps_every_approved_requirement_text_while_the_current_experiment_moves() {
    let mut kept = Vec::new();
    let mut r = Rig::start("explore-plan-panel", &[("data/in.txt", "input\n")]);
    r.interview(&["R1", "R2"], 1, &[], "open_child");
    let approved = review(&mut r, "approved-nothing-linked", &mut kept);
    assert_eq!(approved["contract"], "exploration");
    assert_eq!((&approved["proposal"], &approved["current_item"]), (&Value::Null, &Value::Null));
    assert_eq!(approved["pending"], json!([text("R1"), text("R2")]));

    let c2 = r.open_experiment("R1");
    let open = review(&mut r, "r1-open", &mut kept);
    assert_eq!(open["current_item"], text("R1"));
    assert_eq!(open["current_cycle"], json!({"ref":"cycle:C2", "state":"open"}));
    r.run(c2, &Run { requirement: "R1", problem: "입력 파일이 비어 있지 않은가?", files: &[("evidence/C2.md", "한 줄이 있다\n")], evidence: "evidence/C2.md", fails_back_to: None });
    let closed = review(&mut r, "r1-closed", &mut kept);
    assert_eq!(closed["current_cycle"], json!({"ref":"cycle:C2", "state":"closed"}));
    assert_eq!(closed["current_item"], text("R1"), "the binding survives the close");

    // A second Experiment on the same requirement fails; the requirement is still one entry.
    let c3 = r.open_experiment("R1");
    r.run(c3, &Run { requirement: "R1", problem: "입력 파일이 두 줄 이상인가?", files: &[("evidence/C3.md", "한 줄뿐이다\n")], evidence: "evidence/C3.md", fails_back_to: Some("cycle:C2") });
    let failed = review(&mut r, "r1-failed", &mut kept);
    assert_eq!(links(&failed), vec![("cycle:C2".into(), "R1".into()), ("cycle:C3".into(), "R1".into())]);
    assert_eq!(failed["requirements"].as_array().unwrap().len(), 2);
    assert_eq!(failed["retry_pending"], json!([]));
    r.recover();

    // The current Experiment moves to R2. This is the position the panel dropped R1's text at.
    let c4 = r.open_experiment("R2");
    let moved = review(&mut r, "moved-to-r2-open", &mut kept);
    assert_eq!(moved["contract"], "exploration");
    assert_eq!(moved["proposal"], Value::Null);
    assert_eq!(moved["active_revision"], 1);
    assert_eq!(moved["current_item"], text("R2"), "current_item carries the current requirement only");
    assert_eq!(moved["current_cycle"], json!({"ref":"cycle:C4", "state":"open"}));
    assert_eq!(links(&moved), vec![("cycle:C2".into(), "R1".into()), ("cycle:C3".into(), "R1".into()), ("cycle:C4".into(), "R2".into())]);
    assert_eq!((&moved["pending"], &moved["retry_pending"]), (&json!([]), &json!([])));
    for id in ["R1", "R2"] {
        let one = requirement(&moved, id);
        assert_eq!(json!({"id":one["id"], "question":one["question"], "done_when":one["done_when"], "source":one["source"], "command":one["command"], "part_of":one["part_of"]}), text(id), "the server still carries the approved text of {id}");
    }
    let r1: Vec<_> = requirement(&moved, "R1")["experiments"].as_array().unwrap().iter().map(|e| (e["cycle_ref"].as_str().unwrap(), e["state"].as_str().unwrap(), e["verdict"].as_str())).collect();
    assert_eq!(r1, vec![("cycle:C2", "closed", Some("success")), ("cycle:C3", "closed", Some("failure"))]);
    assert_eq!(moved["active_plan"]["items"], json!([text("R1"), text("R2")]));

    r.run(c4, &Run { requirement: "R2", problem: "입력 파일의 첫 줄이 input인가?", files: &[("evidence/C4.md", "첫 줄은 input이다\n")], evidence: "evidence/C4.md", fails_back_to: None });
    review(&mut r, "r2-closed", &mut kept);

    // An Interview is bound to no requirement: the texts are only in `requirements`.
    ok(r.model("gil_open", json!({"kind":"interview"})), "later interview");
    let interview = review(&mut r, "interview-open", &mut kept);
    assert_eq!((&interview["proposal"], &interview["current_item"]), (&Value::Null, &Value::Null));
    assert_eq!(interview["current_cycle"], json!({"ref":"cycle:C5", "state":"open"}));
    assert_eq!(interview["pending"], json!([]));
    assert_eq!(interview["requirements"].as_array().unwrap().iter().map(|one| json!({"id":one["id"], "question":one["question"], "done_when":one["done_when"], "source":one["source"], "command":one["command"], "part_of":one["part_of"]})).collect::<Vec<_>>(), vec![text("R1"), text("R2")]);

    // A new proposal adds R3. It is not approved text: `requirements` stays the approved revision.
    let mut basis = Vec::new();
    for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
        let opened = ok(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
        basis.push(step_ref(&opened, kind));
        ok(r.model("gil_close", json!({"report":report})), kind);
    }
    let synthesis = step_ref(&ok(r.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis"), "synthesis");
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["R1", "R2", "R3"], &basis, &[])})), "propose");
    let waiting = review(&mut r, "proposal-waiting", &mut kept);
    assert_eq!(waiting["proposal"]["revision"], 2);
    assert_eq!(waiting["proposal"]["receipt"], Value::Null);
    assert_eq!(waiting["proposal"]["draft"]["items"], json!([text("R1"), text("R2"), text("R3")]));
    assert_eq!(waiting["active_revision"], 1);
    assert_eq!(waiting["requirements"].as_array().unwrap().len(), 2, "a proposal is not an approved requirement");

    let frame = ok(r.app("gil_plan_review", json!({})), "capability");
    let (p, t) = (structured(&frame)["proposal"].clone(), token(&frame).expect("capability in _meta"));
    ok(r.app("gil_plan_decide", json!({"revision":p["revision"],"digest":p["digest"],"token":t,"decision":"approve"})), "approve");
    let decided = review(&mut r, "proposal-approved", &mut kept);
    assert_eq!(decided["proposal"]["receipt"]["decision"], "approve");

    // The new revision becomes the approved one and an Experiment opens on R3. R1 and R2 were
    // linked under revision 1; their text now comes from the approved revision 2, where Core
    // keeps an existing requirement's definition unchanged.
    ok(r.model("gil_close", json!({"report":decided["close_report"]})), "close synthesis");
    let outcome = step_ref(&ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome"), "outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 요구 R3을 더해 승인받았다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 요구를 탐색한다\n")})), "close outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 요구 R3을 Monitor 응답으로 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
    assert_eq!(r.open_experiment("R3"), 6);
    let third = review(&mut r, "reapproved-r3-open", &mut kept);
    assert_eq!((&third["proposal"], &third["active_revision"]), (&Value::Null, &json!(2)));
    assert_eq!(third["current_item"], text("R3"));
    assert_eq!(third["executed_cycles"].as_array().unwrap().iter().map(|e| e["revision"].as_u64().unwrap()).collect::<Vec<_>>(), vec![1, 1, 1, 2]);
    assert_eq!(third["requirements"].as_array().unwrap().iter().map(|one| json!({"id":one["id"], "question":one["question"], "done_when":one["done_when"], "source":one["source"], "command":one["command"], "part_of":one["part_of"]})).collect::<Vec<_>>(), vec![text("R1"), text("R2"), text("R3")]);

    let packet = json!({
        "source": "tests/exploration_plan_panel.rs: gil_plan_review structuredContent of the real exploration-flow Core, unmodified. Synthetic harness input; never a person's approval.",
        "checkpoints": kept,
    });
    let bytes = format!("{}\n", serde_json::to_string_pretty(&packet).unwrap());
    for forbidden in ["gil/planApprovalToken", "/Users/", "/private/", ":\\\\Users\\\\"] {
        assert!(!bytes.contains(forbidden), "the kept responses carry {forbidden}");
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("mcp-app/fixtures/exploration-plan-review.json");
    if std::env::var("GIL_EXPLORATION_PLAN_WRITE").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
    } else {
        // Compared as JSON values, so a checkout that rewrites line endings still compares equal.
        let committed: Value = serde_json::from_str(&fs::read_to_string(&path).expect("generate the fixture first (GIL_EXPLORATION_PLAN_WRITE=1)")).expect("fixture JSON");
        assert_eq!(committed, packet, "the committed panel fixture is not what this Core returns");
    }
}
