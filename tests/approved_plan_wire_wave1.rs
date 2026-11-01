//! Wave 1 E — the M6-D approval contract over the real `gil mcp --serve` stdio process.
//!
//! What this checks is wiring, not domain truth (src/approved_plan/tests.rs owns that): the
//! App-only capability lives in `_meta` and never in model content or the record; bare
//! `approved: yes` and tampered/stale close_reports are refused; edits void earlier consent;
//! a restarted process reissues the capability; the Experiment is gated until Outcome and the
//! Interview Cycle close; Verify needs the bound plan document bytes plus fresh evidence;
//! restore/revisit stay blocked; a reinterview request forces an Interview; request_changes
//! closes with approved:no; a Closing Interview needs its own approval and the next Chain
//! cannot reuse it. Only a synthetic temporary project is touched. Every App decision here is a
//! synthetic harness input — it proves the protocol, never a person's consent or a visible screen.
//! Wave 2 D: the record path follows the product layout (Windows private app data below an
//! isolated LOCALAPPDATA), the child environment is isolated, and a missing record is a failure.
#![cfg(feature = "approved-plan")]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::path::Path;
use support::*;

struct Rig { root: std::path::PathBuf, record: std::path::PathBuf, scope: String, c: Client }

impl Rig {
    fn model(&mut self, name: &str, mut args: Value) -> Value {
        if name != "show_gil_monitor" { args["project_root"] = json!(self.root); }
        self.c.call(name, args)
    }
    fn app(&mut self, name: &str, mut args: Value) -> Value { args["scope_id"] = json!(self.scope); self.c.call(name, args) }
    fn read(&mut self) -> Value { structured(&ok(self.model("gil_plan_read", json!({})), "gil_plan_read")).clone() }
    /// Reading twice must not mint approval, advance state, or expose an App capability.
    fn context_readonly(&mut self) -> String {
        let before = record_bytes(&self.record);
        let first = said(&ok(self.model("gil_context", json!({})), "context"));
        let second = said(&ok(self.model("gil_context", json!({})), "context again"));
        assert_eq!(first, second, "context must be a deterministic projection");
        assert_eq!(record_bytes(&self.record), before, "context changed the record");
        assert!(!first.contains("gil/planApprovalToken"));
        first
    }
    /// Question + Interpretation + open Synthesis; returns (basis refs, synthesis ref).
    fn ask_and_interpret(&mut self) -> (Vec<String>, String) {
        let mut basis = Vec::new();
        for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
            let opened = ok(self.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
            basis.push(step_ref(&opened, kind));
            ok(self.model("gil_close", json!({"report":report})), kind);
        }
        let syn = ok(self.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis");
        (basis, step_ref(&syn, "synthesis"))
    }
    fn approve_via_app(&mut self, revision: u64) -> String {
        let review = ok(self.app("gil_plan_review", json!({})), "review");
        let t = token(&review).expect("capability in _meta");
        let p = &structured(&review)["proposal"];
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":"approve"})), "approve");
        let read = self.read();
        assert_eq!(read["proposal"]["receipt"]["decision"], "approve");
        assert_eq!(read["proposal"]["receipt"]["source"], "monitor_button");
        t
    }
    fn close_approved_interview(&mut self, synthesis: &str, action: &str) {
        let read = self.read();
        ok(self.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
        let outcome = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome_ref = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 Monitor 응답으로 승인받았다\nnext_direction.action: {action}\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
        assert!(said(&closed).contains(" · interview · success"), "{closed}");
    }
    fn open_experiment(&mut self, item: &str) -> (u32, String) {
        let opened = ok(self.model("gil_open", json!({"kind":"experiment"})), "open experiment");
        let n = cycle_number(&opened);
        let read = self.read();
        assert_eq!(read["current_item"]["id"], item, "{read}");
        let document = read["document"].as_str().expect("document after binding").to_owned();
        (n, document)
    }
    fn walk_to_verify(&mut self, item: &str) -> String {
        ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        ok(self.model("gil_close", json!({"report":format!("problem: 항목 {item}을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n")})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":format!("hypothesis: 합성 자료만으로 항목 {item}을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n")})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        step_ref(&v, "verify")
    }
    fn finish_experiment(&mut self, n: u32, item: &str, verify: &str, document: &str) {
        std::fs::create_dir_all(self.root.join("plan")).unwrap();
        std::fs::create_dir_all(self.root.join("evidence")).unwrap();
        std::fs::write(self.root.join("plan/approved-plan.md"), document).unwrap();
        let evidence = format!("evidence/C{n}-{item}.md");
        std::fs::write(self.root.join(&evidence), format!("# 합성 관측 C{n}\n\n항목 {item} 결과 (시험 전용)\n")).unwrap();
        ok(self.c.call("gil_close", verify_args(&self.root, verify, &format!("execution: 합성 자료를 확인하고 결과를 저장했다\nresult: 항목 {item} 결과 파일을 남겼다\nplan_document: plan/approved-plan.md\nevidence_files: {evidence}\n"))), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(self.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n"})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "experiment outcome");
        let outcome_ref = step_ref(&o, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 항목 {item}을 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n")})), "close experiment outcome");
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 항목 {item} 완료\nnext_direction.action: open_child\nnext_direction.reason: 남은 항목을 이어간다\n")})), "close experiment cycle");
        assert!(said(&closed).contains(&format!("닫았다: cycle:C{n} · experiment · success")), "{closed}");
    }
}

#[test]
fn approval_contract_holds_over_the_real_stdio_process() {
    let root = scratch("plan-wire-project");
    let settings = scratch("plan-wire-settings");
    let c = Client::serve(&settings);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let state = |_: &Path| record_bytes(&record);
    let mut r = Rig { root: root.clone(), record: record.clone(), scope: String::new(), c };

    // tools/list: App tools are not model tools.
    let listed = r.c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    for (name, app) in [("gil_plan_propose", false), ("gil_plan_read", false), ("gil_plan_request_interview", false), ("gil_plan_review", true), ("gil_plan_decide", true)] {
        let t = tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} missing from tools/list"));
        assert_eq!(t["_meta"]["ui"]["visibility"], json!([if app { "app" } else { "model" }]), "{name}");
    }

    // start once; follow the returned show_gil_monitor; display stays unconfirmed.
    let started = ok(r.model("gil_start", json!({})), "gil_start");
    let text = started["result"]["content"].as_array().unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<String>();
    let at = text.find("show_gil_monitor(").expect("start names show_gil_monitor") + "show_gil_monitor(".len();
    let end = text[at..].find(')').unwrap();
    r.scope = serde_json::from_str::<Value>(&text[at..at + end]).unwrap()["scope_id"].as_str().unwrap().to_owned();
    assert_eq!(started["result"]["structuredContent"]["display_confirmed"], json!(false));
    assert!(r.record.is_file(), "record missing at the product layout path {} (record_storage={})", r.record.display(), started["result"]["structuredContent"]["record_storage"]);
    if cfg!(windows) { assert_eq!(started["result"]["structuredContent"]["record_storage"], json!("private_app_data")); }
    else { assert!(!private_local_app_data(&settings).join("GIL").exists(), "LOCALAPPDATA store must not appear on a non-Windows host"); }
    ok(r.c.call("show_gil_monitor", json!({"scope_id":r.scope})), "show_gil_monitor");

    let (basis, synthesis) = r.ask_and_interpret();
    assert_eq!(synthesis, "step:C1/S3");

    // bare approved:yes is refused and writes nothing; review without a proposal has no capability.
    let before = state(&root);
    refused(r.model("gil_close", json!({"report":format!("statement: 합성 범위\nbasis_refs: {}\napproved: yes\n", basis.join("\n"))})), "bare approved:yes");
    assert_eq!(state(&root), before);
    let empty = ok(r.app("gil_plan_review", json!({})), "review without proposal");
    assert!(structured(&empty)["proposal"].is_null() && token(&empty).is_none(), "{empty}");

    // propose / idempotent / read
    let d = draft(&["A", "B"], &basis, &[]);
    let first = ok(r.model("gil_plan_propose", json!({"draft":d})), "propose");
    assert_eq!(structured(&first)["proposal"]["revision"], 1);
    assert!(structured(&first)["proposal"]["receipt"].is_null());
    assert!(!first.to_string().contains("planApprovalToken"));
    let again = ok(r.model("gil_plan_propose", json!({"draft":d})), "propose again");
    assert_eq!(structured(&again)["proposal"]["digest"], structured(&first)["proposal"]["digest"]);
    let rev1 = r.read()["proposal"].clone();
    assert!(r.context_readonly().contains("Monitor 버튼 응답을 기다린다"));

    // capability only in _meta; state untouched; stable within a process
    let before = state(&root);
    let review = ok(r.app("gil_plan_review", json!({})), "review");
    let t1 = token(&review).expect("token");
    assert_eq!(t1.len(), 64);
    assert!(!structured(&review).to_string().contains(&t1) && !review["result"]["content"].to_string().contains(&t1));
    assert_eq!(state(&root), before);
    assert_eq!(token(&ok(r.app("gil_plan_review", json!({})), "review again")).as_deref(), Some(t1.as_str()));

    // wrong scope / token / digest refused; schema violations rejected; no receipt recorded
    let base = json!({"revision":1,"digest":rev1["digest"],"token":t1,"decision":"approve"});
    let mut other = base.clone(); other["scope_id"] = json!(format!("project:{}", "0".repeat(64)));
    refused(r.c.call("gil_plan_decide", other), "other scope");
    let mut bad = base.clone(); bad["token"] = json!("f".repeat(64));
    refused(r.app("gil_plan_decide", bad), "wrong token");
    let mut bad = base.clone(); bad["digest"] = json!("0".repeat(64));
    refused(r.app("gil_plan_decide", bad), "wrong digest");
    let mut bad = base.clone(); bad["approved"] = json!(true);
    rpc_error(r.app("gil_plan_decide", bad), "extra field");
    rpc_error(r.model("gil_plan_propose", json!({"draft":{"approved":true}})), "malformed draft");
    assert_eq!(state(&root), before);
    assert!(r.read()["proposal"]["receipt"].is_null());

    // edit → revision 2; old decision and old close_report are void
    // An unanswered proposal offers no close_report; a forged revision-1 approval must be refused now and later.
    assert!(r.read()["close_report"].is_null());
    let close_report_rev1 = format!("statement: 합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다\nbasis_refs: |\n  {}\nexploration_checklist: |\n  A\n  B\napproved: yes\nplan_revision: 1\nplan_digest: {}\n", basis.join("\n  "), rev1["digest"].as_str().unwrap());
    let mut edited = d.clone(); edited["scope"]["goal"] = json!("합성 자료 기반 항목별 보고서 (수정)");
    let second = ok(r.model("gil_plan_propose", json!({"draft":edited})), "propose edited");
    assert_eq!(structured(&second)["proposal"]["revision"], 2);
    let rev2 = structured(&second)["proposal"].clone();
    refused(r.app("gil_plan_decide", base.clone()), "decide on superseded revision");
    let before = state(&root);
    refused(r.model("gil_close", json!({"report":close_report_rev1})), "close with revision-1 report");
    assert_eq!(state(&root), before);

    // restart: scope survives, capability is reissued, the old one is refused
    let old = std::mem::replace(&mut r.c, Client::serve(&settings));
    let (status, noise) = old.eof();
    assert!(status.success(), "first process exit: {status:?}");
    assert!(noise.is_empty(), "non-JSON on stdout: {noise:?}");
    let review = ok(r.app("gil_plan_review", json!({})), "review after restart");
    let t2 = token(&review).expect("token after restart");
    assert_ne!(t2, t1);
    assert_eq!(structured(&review)["proposal"]["revision"], 2);
    refused(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t1,"decision":"approve"})), "old capability after restart");
    let accepted = ok(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"approve"})), "approve revision 2");
    assert_eq!(structured(&accepted)["proposal"]["receipt"]["decision"], "approve");

    assert!(r.context_readonly().contains("close_report를 그대로 gil_close"));

    // duplicate approve idempotent; flip refused; no capability after receipt; no token in record
    let saved = state(&root);
    ok(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"approve"})), "duplicate approve");
    assert_eq!(state(&root), saved);
    refused(r.app("gil_plan_decide", json!({"revision":2,"digest":rev2["digest"],"token":t2,"decision":"request_changes"})), "flip");
    assert!(token(&ok(r.app("gil_plan_review", json!({})), "review after receipt")).is_none());
    let record = String::from_utf8(saved).unwrap();
    assert!(!record.contains(&t1) && !record.contains(&t2), "capability written into the record");

    // close only with the exact close_report
    let report = r.read()["close_report"].as_str().unwrap().to_owned();
    assert!(report.contains("plan_revision") && report.contains("plan_digest"), "{report}");
    refused(r.model("gil_close", json!({"report":report.replace("합성 자료로 항목을", "다른 문장으로 항목을")})), "tampered close_report");
    refused(r.model("gil_close", json!({"report":close_report_rev1})), "revision-1 report after revision-2 approval");
    ok(r.model("gil_close", json!({"report":report})), "close synthesis");

    // Experiment gated until Outcome and the Interview Cycle close; then binds A
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment after synthesis");
    let outcome = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
    let outcome_ref = step_ref(&outcome, "outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment before cycle close");
    refused(r.model("gil_cycle", json!({"action":"close"})), "legacy gil_cycle close");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 승인 항목을 순서대로 실행한다\n")})), "close interview cycle");
    let read = r.read();
    assert_eq!(read["active_revision"], 2);
    assert_eq!(read["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), ["A", "B"]);
    let (c2, document) = r.open_experiment("A");
    assert!(document.contains("Revision: 2"));
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: define)") && context.contains("이번 Experiment: A"), "{context}");

    // Verify: document bytes + separate fresh evidence; four refusals first
    let verify = r.walk_to_verify("A");
    let context = r.context_readonly();
    assert!(context.contains("열린 Verify") && context.contains("plan_document") && context.contains("evidence_files"), "{context}");
    std::fs::create_dir_all(root.join("plan")).unwrap();
    std::fs::create_dir_all(root.join("evidence")).unwrap();
    std::fs::write(root.join(format!("evidence/C{c2}-A.md")), "# 합성 관측\n\n항목 A 결과\n").unwrap();
    let before = state(&root);
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nevidence_files: evidence/C{c2}-A.md\n"))), "verify without plan_document");
    std::fs::write(root.join("plan/approved-plan.md"), &document).unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, "execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: plan/approved-plan.md\n")), "plan document as the only evidence");
    std::fs::write(root.join("plan/approved-plan.md"), document.replace("Revision: 2", "Revision: 1")).unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C{c2}-A.md\n"))), "wrong document bytes");
    refused(r.c.call("gil_close", verify_args(&root, &verify, "execution: 확인했다\nresult: 채팅으로 보고했다\n")), "no evidence_files");
    assert_eq!(state(&root), before);
    r.finish_experiment(c2, "A", &verify, &document);
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: experiment)"), "{context}");
    assert!(!context.contains("gil_plan_read.document") && context.contains("닫힌 Experiment의 실행 연결: A"), "closed Cycle still directs writing: {context}");
    // A new server must reconstruct the same projection from the persisted approval/binding.
    let old = std::mem::replace(&mut r.c, Client::serve(&settings));
    let (status, noise) = old.eof();
    assert!(status.success() && noise.is_empty());
    assert_eq!(r.context_readonly(), context);

    let read = r.read();
    assert_eq!(read["executed_cycles"][0]["item_id"], "A");
    assert_eq!(read["executed_cycles"][0]["revision"], 2);

    // restore / revisit stay blocked (product-safety / approved-plan lock) and change nothing
    let before = state(&root);
    refused(r.model("gil_restore", json!({})), "restore blocked");
    refused(r.model("gil_revisit", json!({})), "revisit blocked");
    assert_eq!(state(&root), before);

    // reinterview request preserved through the Experiment; next cycle must be an Interview
    let (c3, _) = r.open_experiment("B");
    ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
    refused(r.model("gil_plan_request_interview", json!({"reason":"   "})), "blank reason");
    ok(r.model("gil_plan_request_interview", json!({"reason":"항목 D가 B 다음에 필요하다"})), "request interview");
    ok(r.model("gil_plan_request_interview", json!({"reason":"항목 D가 B 다음에 필요하다"})), "request again");
    assert_eq!(r.read()["reinterview_requests"].as_array().unwrap().len(), 1);
    refused(r.model("gil_open", json!({"kind":"interview"})), "interview while Define is open");
    ok(r.model("gil_close", json!({"report":"problem: 항목 B을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n"})), "close define");
    ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 항목 B을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let v = ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
    let verify = step_ref(&v, "verify");
    r.finish_experiment(c3, "B", &verify, &document);
    let read = r.read();
    assert_eq!(read["reinterview_requests"].as_array().unwrap().len(), 1, "{read}");
    assert!(read["pending"].as_array().unwrap().is_empty());
    let context = r.context_readonly();
    assert!(context.contains("gil_open(kind: interview)"), "{context}");
    assert!(!context.contains("현재 Cycle의 남은 Step을 실제 순서대로 마치고"), "closed Cycle still directs old steps: {context}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment while reinterview pending");
    let opened = ok(r.model("gil_open", json!({"kind":"interview"})), "open interview");
    let c4 = cycle_number(&opened);

    // request_changes → approved:no → question; re-proposal keeps approved items; re-approval
    let (basis, _) = r.ask_and_interpret();
    let p3 = ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B","D"], &basis, &[])})), "propose rev3");
    assert_eq!(structured(&p3)["proposal"]["revision"], 3);
    assert_eq!(structured(&p3)["proposal"]["base_revision"], 2);
    let review = ok(r.app("gil_plan_review", json!({})), "review rev3");
    let t3 = token(&review).unwrap();
    let d3 = structured(&p3)["proposal"]["digest"].clone();
    ok(r.app("gil_plan_decide", json!({"revision":3,"digest":d3,"token":t3,"decision":"request_changes"})), "request_changes");
    let read = r.read();
    let report = read["close_report"].as_str().unwrap().to_owned();
    // close_report uses the block form for every field.
    assert!(report.contains("approved: |\n  no\n"), "{report}");
    let forged = report.replace("approved: |\n  no\n", "approved: |\n  yes\n");
    assert_ne!(forged, report);
    refused(r.model("gil_close", json!({"report":forged})), "forged approved:yes");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment with Synthesis open");
    let closed = ok(r.model("gil_close", json!({"report":report})), "close with approved:no");
    assert!(said(&closed).contains("question"), "{closed}");
    refused(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome after approved:no");
    let (basis, synthesis4) = r.ask_and_interpret();
    let mut altered = draft(&["A","B","D"], &basis, &[]); altered["items"][1]["done_when"] = json!("바뀐 조건");
    refused(r.model("gil_plan_propose", json!({"draft":altered})), "altered approved item");
    refused(r.model("gil_plan_propose", json!({"draft":draft(&["A","D"], &basis, &[])})), "removed approved item");
    let p4 = ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B","D"], &basis, &[])})), "propose rev4");
    assert_eq!(structured(&p4)["proposal"]["revision"], 4);
    refused(r.app("gil_plan_decide", json!({"revision":3,"digest":d3,"token":t3,"decision":"approve"})), "approve superseded rev3");
    r.approve_via_app(4);
    r.close_approved_interview(&synthesis4, "open_child");
    let read = r.read();
    assert_eq!(read["active_revision"], 4);
    assert_eq!(read["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), ["D"], "{read}");
    assert!(read["reinterview_requests"].as_array().unwrap().is_empty());
    let _ = c4;

    // inserted item D executes under revision 4; the stale revision-2 document is refused
    let (c5, document4) = r.open_experiment("D");
    assert!(document4.contains("Revision: 4"));
    let verify = r.walk_to_verify("D");
    std::fs::write(root.join(format!("evidence/C{c5}-D.md")), "pre").unwrap();
    refused(r.c.call("gil_close", verify_args(&root, &verify, &format!("execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/C{c5}-D.md\n"))), "stale revision-2 document");
    r.finish_experiment(c5, "D", &verify, &document4);
    let read = r.read();
    assert_eq!(read["executed_cycles"].as_array().unwrap().iter().map(|e| format!("{}@{}", e["item_id"].as_str().unwrap(), e["revision"])).collect::<Vec<_>>(), ["A@2", "B@2", "D@4"]);

    // Closing Interview needs its own approval; Chain closes; next Chain cannot reuse it
    ok(r.model("gil_open", json!({"kind":"interview"})), "open closing interview");
    let (basis, synthesis6) = r.ask_and_interpret();
    let closing = draft(&["A","B","D"], &basis, &[("chain_verdict", "success")]);
    let p5 = ok(r.model("gil_plan_propose", json!({"draft":closing})), "propose closing");
    assert_eq!(structured(&p5)["proposal"]["revision"], 5);
    refused(r.model("gil_close", json!({"report":format!("statement: 합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다\nbasis_refs: {}\nexploration_checklist: |\n  A\n  B\n  D\nchain_verdict: success\napproved: yes\n", basis.join("\n"))})), "bare approved:yes on closing synthesis");
    let t5 = r.approve_via_app(5);
    r.close_approved_interview(&synthesis6, "close_chain");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment while Chain Report is due");
    let chain = ok(r.model("gil_close", json!({"report":format!("verdict: success\nclosing_synthesis_ref: {synthesis6}\nhandoff_summary: 합성 항목 A,B,D를 파일 증거와 함께 완료했다\n")})), "close chain");
    assert!(said(&chain).starts_with("닫았다: chain:"), "{chain}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "experiment after Chain closed");
    let next = ok(r.model("gil_open", json!({"kind":"chain"})), "open next chain");
    assert!(said(&next).contains("Opening Interview"), "{next}");
    let read = r.read();
    assert_eq!(read["supported"], true);
    assert!(read["proposal"].is_null() && read["active_revision"].is_null() && read["pending"].as_array().unwrap().is_empty() && read["executed_cycles"].as_array().unwrap().is_empty(), "{read}");
    let (basis, _) = r.ask_and_interpret();
    let px = ok(r.model("gil_plan_propose", json!({"draft":draft(&["X"], &basis, &[])})), "propose in new chain");
    assert_eq!(structured(&px)["proposal"]["revision"], 1);
    assert!(structured(&px)["proposal"]["base_revision"].is_null());
    refused(r.app("gil_plan_decide", json!({"revision":1,"digest":structured(&p5)["proposal"]["digest"],"token":t5,"decision":"approve"})), "old chain digest/capability");
    r.approve_via_app(1);

    // clean shutdown
    let Rig { c, .. } = r;
    let (status, noise) = c.eof();
    assert!(status.success(), "{status:?}");
    assert!(noise.is_empty(), "non-JSON stdout: {noise:?}");
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(settings).ok();
}
