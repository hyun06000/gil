//! After a Closing approval: what the real `gil mcp --serve` process tells the Agent to do next.
//!
//! The defect this file fixes in place (2026-11-18): once the user approved a proposal that
//! closes the Chain, the receipts still spoke as if an execution plan had been approved — "the
//! Cycle Report must then be closed before execution work can begin", "open the Experiment after
//! that succeeds". The Core already refused the wrong move; only the guidance pointed at it.
//!
//! What is checked, over stdio and through the same App-only tools the Monitor buttons call:
//!
//! - an approved execution plan keeps its guidance toward the Experiment, word for word;
//! - an approved closing proposal (`chain_verdict: success` and `failure`) is led, step by step,
//!   to the Chain Report whose verdict is the approved one — the Closing Interview's own
//!   `success` is never offered as the Chain verdict, and no Experiment is suggested;
//! - a change request on a closing proposal is not an approval and closes nothing;
//! - the same guidance is read again after a process restart, from the record alone;
//! - a Chain Report with the other verdict is refused and the record is unchanged;
//! - after the Chain Report nothing opens without the explicit next Chain, and the next Chain
//!   starts from the previous handoff while the previous approval cannot close its Synthesis.
//!
//! Already covered elsewhere and not repeated here: the next Chain's own approval and the stale
//! App capability (tests/approved_plan_wire_wave1.rs), the verdict equality at the domain level
//! (src/approved_plan/tests.rs, tests/chain_closing.rs), reading an earlier Chain's originals
//! and refusing its approval as an execution link (tests/exploration_flow.rs,
//! tests/exploration_evidence.rs), Context after each saved decision (tests/resume_guidance.rs).
//!
//! Scope of the evidence: synthetic tool-path checks. The App decisions are harness input, not a
//! person's click on a visible screen, and no model read these sentences. Whether an agent
//! follows them is model and real-use evaluation, which this file does not perform.
#![cfg(feature = "approved-plan")]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::path::PathBuf;
use support::*;

struct Rig { root: PathBuf, settings: PathBuf, record: PathBuf, scope: String, c: Client }

/// Every text block of a tool result in order: the Host guidance first, then the Core receipt.
fn blocks(frame: &Value) -> String {
    frame["result"]["content"].as_array().map(|c| c.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
}
/// The Host guidance: the first text block, which names the one next call.
fn guidance(frame: &Value) -> String { frame["result"]["content"][0]["text"].as_str().unwrap_or_default().to_owned() }
#[track_caller]
fn has(text: &str, needles: &[&str]) { for needle in needles { assert!(text.contains(needle), "missing `{needle}` in: {text}"); } }
#[track_caller]
fn lacks(text: &str, needles: &[&str]) { for needle in needles { assert!(!text.contains(needle), "`{needle}` must not appear in: {text}"); } }

/// Sentences that send the Agent on to execution. Right after an execution-plan approval,
/// wrong after a closing approval.
const TOWARD_EXECUTION: [&str; 4] = ["실행 작업으로 넘어갈 수 있다", "Experiment는 그 성공 뒤에 열고", "새 Experiment는 그 닫기가 성공한 뒤에 시작한다", "gil_open(kind: experiment)"];

impl Rig {
    /// A fresh item-queue Project (the contract every `approved-plan` build writes) with its Monitor scope.
    fn start(label: &str) -> Self {
        let root = scratch(label);
        let settings = scratch(&format!("{label}-settings"));
        let record = product_state_path(&root, &private_local_app_data(&settings));
        let mut r = Rig { root, settings: settings.clone(), record, scope: String::new(), c: Client::serve(&settings) };
        let started = ok(r.model("gil_start", direct_start()), "start");
        as_item_queue_record(&r.record);
        let next = &structured(&started)["next_call"];
        r.scope = next["arguments"]["scope_id"].as_str().unwrap().to_owned();
        ok(r.c.call("show_gil_monitor", next["arguments"].clone()), "show");
        r
    }
    fn model(&mut self, name: &str, mut args: Value) -> Value { args["project_root"] = json!(self.root); self.c.call(name, args) }
    fn app(&mut self, name: &str, mut args: Value) -> Value { args["scope_id"] = json!(self.scope); self.c.call(name, args) }
    fn read(&mut self) -> Value { ok(self.model("gil_plan_read", json!({})), "gil_plan_read") }
    fn context(&mut self) -> String { said(&ok(self.model("gil_context", json!({})), "context")) }
    /// A new Core process over the same record and the same settings: nothing is carried in memory.
    fn restart(&mut self) {
        let old = std::mem::replace(&mut self.c, Client::serve(&self.settings));
        let (status, noise) = old.eof();
        assert!(status.success() && noise.is_empty(), "old Core exit {status:?} noise {noise:?}");
        let prepared = ok(self.model("gil_monitor_prepare", json!({})), "prepare");
        self.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
        // Ask for the Monitor again, as a Host does: otherwise the first text block of the next
        // receipts is the "Monitor is not shown" notice instead of the next call.
        ok(self.c.call("show_gil_monitor", json!({"scope_id":self.scope})), "show after restart");
    }
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
    /// The Monitor button path: review hands out the private capability, decide records the receipt.
    fn decide(&mut self, revision: u64, decision: &str) {
        let review = ok(self.app("gil_plan_review", json!({})), "review");
        let t = token(&review).expect("capability in _meta");
        let p = structured(&review)["proposal"].clone();
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":decision})), "decide");
        let read = self.read();
        assert_eq!(structured(&read)["proposal"]["receipt"], json!({"decision":decision,"source":"monitor_button"}), "{read}");
    }
    fn close_outcome(&mut self, synthesis: &str) -> (Value, String) {
        let opened = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome = step_ref(&opened, "outcome");
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 승인된 제안을 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 제안을 따른다\n")})), "close outcome");
        (closed, outcome)
    }
    fn cycle_report(&mut self, outcome: &str, action: &str) -> Value {
        self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: Monitor 응답으로 승인받았다\nnext_direction.action: {action}\nnext_direction.reason: 승인된 제안을 따른다\n")}))
    }
    /// Opening Interview with an approved plan for `ids`, then one successful Experiment on the first item.
    fn executed(&mut self, ids: &[&str]) {
        let (basis, synthesis) = self.ask_and_interpret();
        ok(self.model("gil_plan_propose", json!({"draft":draft(ids, &basis, &[])})), "propose");
        self.decide(1, "approve");
        let read = self.read();
        ok(self.model("gil_close", json!({"report":structured(&read)["close_report"]})), "close synthesis");
        let (_, outcome) = self.close_outcome(&synthesis);
        ok(self.cycle_report(&outcome, "open_child"), "close opening interview");
        ok(self.model("gil_open", json!({"kind":"experiment"})), "open experiment");
        let read = self.read();
        let document = structured(&read)["document"].as_str().expect("document after binding").to_owned();
        ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        ok(self.model("gil_close", json!({"report":"problem: 합성 자료만으로 첫 항목을 확인할 수 있는가\nsuccess_condition: evidence 파일에 결과가 저장된다\n"})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        std::fs::create_dir_all(self.root.join("evidence")).unwrap();
        std::fs::write(self.root.join("plan.md"), &document).unwrap();
        std::fs::write(self.root.join("evidence/first.md"), "# 합성 관측 (시험 전용)\n").unwrap();
        ok(self.c.call("gil_close", verify_args(&self.root, &step_ref(&v, "verify"), "execution: 합성 자료를 확인했다\nresult: 결과 파일을 남겼다\nplan_document: plan.md\nevidence_files: evidence/first.md\n")), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(self.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n"})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "experiment outcome");
        let outcome = step_ref(&o, "outcome");
        ok(self.model("gil_close", json!({"report":"verdict: success\nlesson: 첫 항목을 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 종료를 검토한다\n"})), "close experiment outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 첫 항목 완료\nnext_direction.action: open_child\nnext_direction.reason: 종료를 검토한다\n")})), "close experiment");
    }
    /// The Closing Interview up to its open Synthesis with the closing proposal recorded (revision 2).
    fn closing_proposed(&mut self, ids: &[&str], verdict: &str) -> String {
        ok(self.model("gil_open", json!({"kind":"interview"})), "closing interview");
        let (basis, synthesis) = self.ask_and_interpret();
        // Before the proposal exists, the open Synthesis says what a closing proposal must carry.
        has(&guidance(&self.read()), &["이 제안이 Chain을 닫는 제안이라면", "원본 Report에서 읽어", "이번 탐색을 지금 마치는 이유", "후속 실험이나 대체 자료 후보", "성공으로 바꾸지 않으며", "종료가 다음 Chain을 열지는 않는다"]);
        ok(self.model("gil_plan_propose", json!({"draft":draft(ids, &basis, &[("chain_verdict", verdict)])})), "closing proposal");
        synthesis
    }
    fn done(self) {
        let Rig { root, settings, c, .. } = self;
        let (status, noise) = c.eof();
        assert!(status.success() && noise.is_empty(), "{status:?} {noise:?}");
        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(settings).ok();
    }
}

#[test]
fn an_execution_plan_approval_keeps_its_guidance_toward_the_experiment() {
    let mut r = Rig::start("handoff-plan");
    let (basis, synthesis) = r.ask_and_interpret();
    // A first Interview has no executed Experiment: nothing about closing is said yet.
    lacks(&guidance(&r.read()), &["Chain을 닫는 제안"]);
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["A"], &basis, &[])})), "propose");
    r.decide(1, "approve");
    let read = r.read();
    has(&guidance(&read), &["제안 판 1이(가) 승인됐다", "close_report를 그대로 gil_close"]);
    lacks(&guidance(&read), &["종료 제안", "close_chain", "Chain Report"]);
    let closed = ok(r.model("gil_close", json!({"report":structured(&read)["close_report"]})), "close synthesis");
    has(&blocks(&closed), &["Interview 마무리", "실행 작업으로 넘어갈 수 있다"]);
    lacks(&blocks(&closed), &["Closing Interview 마무리", "chain_verdict"]);
    has(&guidance(&r.read()), &["승인된 Synthesis가 닫혔다", "gil_open(kind: outcome)"]);
    let (closed, outcome) = r.close_outcome(&synthesis);
    has(&blocks(&closed), &["Experiment는 그 성공 뒤에 열고", "새 Experiment는 그 닫기가 성공한 뒤에 시작한다"]);
    lacks(&blocks(&closed), &["Closing Interview 마무리", "Closing Interview의 Cycle Report", "next_direction.action: close_chain"]);
    // The Core refuses a closing direction here: no approved Synthesis carries a Chain verdict.
    let before = record_bytes(&r.record);
    refused(r.cycle_report(&outcome, "close_chain"), "close_chain without an approved closing proposal");
    assert_eq!(record_bytes(&r.record), before);
    let closed = ok(r.cycle_report(&outcome, "open_child"), "close opening interview");
    has(&guidance(&closed), &["gil_open(kind: experiment)"]);
    ok(r.model("gil_open", json!({"kind":"experiment"})), "the approved plan executes");
    r.done();
}

/// One Chain closed as `verdict` through the real approval path, read again after restarts.
fn closing_approval(verdict: &str) {
    let other = if verdict == "success" { "failure" } else { "success" };
    let mut r = Rig::start(&format!("handoff-closing-{verdict}"));
    r.executed(&["A", "B"]);
    let synthesis = r.closing_proposed(&["A", "B"], verdict);

    // Waiting is not approval: nothing about the Chain can be closed yet.
    let before = record_bytes(&r.record);
    has(&guidance(&r.read()), &["Monitor 버튼 응답을 기다린다"]);
    refused(r.model("gil_close", json!({"report":format!("verdict: {verdict}\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: 승인 전\n")})), "Chain Report before approval");
    assert_eq!(record_bytes(&r.record), before);

    r.decide(2, "approve");
    // A new process reads the saved receipt and the exact proposal: same meaning, no memory.
    r.restart();
    let read = r.read();
    assert_eq!(structured(&read)["proposal"]["draft"]["synthesis"]["chain_verdict"], verdict);
    let approved = guidance(&read);
    has(&approved, &["종료 제안 판 2이(가) 승인됐다", &format!("Chain을 {verdict}로 닫는 데 동의했다"), "close_report를 그대로 gil_close",
        "next_direction.action: close_chain", &format!("verdict가 {verdict}인 Chain Report"), "Experiment를 열지 않고"]);
    lacks(&approved, &TOWARD_EXECUTION);
    lacks(&approved, &[&format!("{other}로 닫는"), &format!("verdict가 {other}인")]);
    assert!(r.context().contains(&approved), "Context names the same next call as gil_plan_read");
    let old_close_report = structured(&read)["close_report"].clone();

    let closed = ok(r.model("gil_close", json!({"report":old_close_report.clone()})), "close closing synthesis");
    let text = blocks(&closed);
    has(&text, &["Closing Interview 마무리", &format!("승인된 종료 Synthesis {synthesis}만 닫혔다 (chain_verdict: {verdict})"), "success이며 Chain 판정이 아니다", "Cycle Report(close_chain)와 Chain Report", "실행 작업은 시작하지 않는다"]);
    lacks(&text, &TOWARD_EXECUTION);
    let next = guidance(&r.read());
    has(&next, &[&format!("승인된 종료 Synthesis가 닫혔다 (chain_verdict: {verdict})"), "gil_open(kind: outcome)", "Chain 판정이 아니다", "Experiment를 열지 않는다"]);

    let opened = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open closing outcome");
    let outcome = step_ref(&opened, "outcome");
    has(&guidance(&r.read()), &["열린 Closing Interview의 Outcome", "verdict: success", &format!("승인된 Chain 판정({verdict})은 여기가 아니라 Chain Report에 적는다"), "next_direction.action: close_chain"]);
    let closed = ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 종료 제안을 승인받았다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 종료를 따른다\n")})), "close closing outcome");
    let text = blocks(&closed);
    has(&guidance(&closed), &["Closing Interview의 Cycle Report", "next_direction.action: close_chain", "Interview 자신의 판정", "승인된 chain_verdict는 뒤따르는 Chain Report에 적는다", "Experiment를 열지 않는다"]);
    has(&text, &["Closing Interview 마무리", &format!("승인된 chain_verdict({verdict})를 verdict로 적은 Chain Report"), "Chain 판정이 아니다", "Experiment나 새 Cycle을 열지 않는다"]);
    lacks(&text, &TOWARD_EXECUTION);
    has(&guidance(&r.read()), &["Closing Interview의 Outcome까지 닫혔고", "next_direction.action: close_chain", &format!("승인된 chain_verdict({verdict})"), "Experiment를 열지 않는다"]);

    // The approved closing cannot be turned into "continue with the next Experiment".
    let before = record_bytes(&r.record);
    refused(r.cycle_report(&outcome, "open_child"), "a Closing Interview closed toward a child Cycle");
    assert_eq!(record_bytes(&r.record), before);
    let closed = ok(r.cycle_report(&outcome, "close_chain"), "close closing interview");
    assert!(said(&closed).contains(" · interview · success"), "the Interview's own verdict is success whatever the Chain verdict: {closed}");
    has(&blocks(&closed), &["Chain Report를 gil_close로 제출한다. 새 Cycle을 열지 않는다.", "handoff_summary는 원본 기록을 근거로 한 번 적는다", "아직 답하지 못한 질문", "이번 탐색을 여기서 마치는 이유",
        "승인된 계획처럼 쓰지 않는다", &format!("verdict: {verdict}\nclosing_synthesis_ref: {synthesis}")]);

    // Chain Report due, read by a new process: the approved verdict, never the Interview's success.
    r.restart();
    let due = guidance(&r.read());
    has(&due, &[&format!("Chain Report를 gil_close로 제출한다: verdict: {verdict}"), &format!("사용자가 {synthesis}에서 승인한 chain_verdict"), "Closing Interview 자신의 success가 아니다",
        &format!("closing_synthesis_ref: {synthesis}"), "새 Cycle이나 Chain을 열지 않는다", "판정·결과물·남은 질문을 알리고 멈춘다", "handoff_summary는 원본 기록을 근거로 한 번 적는다"]);
    lacks(&due, &TOWARD_EXECUTION);
    assert!(r.context().contains(&due));
    let before = record_bytes(&r.record);
    for kind in ["experiment", "interview", "chain"] {
        refused(r.model("gil_open", json!({"kind":kind})), "nothing opens while the Chain Report is due");
    }
    // The Chain Report must carry the verdict the user approved; the Interview's success is not it.
    let wrong = refused(r.model("gil_close", json!({"report":format!("verdict: {other}\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: 승인과 다른 판정\n")})), "Chain Report with the other verdict");
    has(&blocks(&wrong), &[&format!("verdict: {verdict}\nclosing_synthesis_ref: {synthesis}")]);
    assert_eq!(record_bytes(&r.record), before, "refusals changed the record");

    let handoff = "A: evidence/first.md로 확인(step 기록 참조). B: 답하지 못함 — 자료 없음. 마치는 이유: 추가 자료 없이는 진전이 없다. 후속 후보(미승인): B용 자료 확보";
    let chain = ok(r.model("gil_close", json!({"report":format!("verdict: {verdict}\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: {handoff}\n")})), "chain report");
    assert!(said(&chain).starts_with(&format!("닫았다: chain:C1 · {verdict}")), "{chain}");
    has(&guidance(&chain), &[&format!("Chain이 닫혔다 ({verdict})"), "Cycle·Step을 더 열지 않고", "판정·결과물·남은 질문을 알린다", "새 주제가 있을 때만 gil_open(kind: chain)", "재사용하지 않는다"]);
    lacks(&guidance(&chain), &["gil_open(kind: experiment)", "gil_open(kind: interview)"]);

    // Closed: the recorded verdict is the approved one, and nothing continues by itself.
    r.restart();
    let context = r.context();
    has(&context, &[&format!("Chain 판정: {verdict}"), &format!("승인 근거: {synthesis}"), handoff, "새 주제가 있을 때만 gil_open(kind: chain)"]);
    let before = record_bytes(&r.record);
    for kind in ["experiment", "interview", "question"] {
        let refusal = refused(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), "nothing opens in a closed Chain");
        has(&blocks(&refusal), &["Chain이 닫혔다"]);
    }
    let read = r.read();
    has(&guidance(&read), &["Chain이 닫혔다", "새 주제가 있을 때만"]);
    assert_eq!(record_bytes(&r.record), before, "a closed Chain was changed without the explicit next Chain");

    // The explicit next Chain: it starts from the handoff, and the previous approval is not its own.
    let opened = ok(r.model("gil_open", json!({"kind":"chain"})), "next chain");
    has(&guidance(&opened), &["다음 Chain을 열었다", "gil_context", "재사용되지 않는다"]);
    let context = r.context();
    has(&context, &["새 Chain의 Opening Interview", "이전 Chain의 판정과 인계 요약을 읽고", "gil_open(kind: question)", "인계의 후보는 승인이 아니다", "이 Chain은 자신의 승인을 받는다",
        &format!("이전 Chain: chain:C1 · {verdict}"), handoff]);
    let read = r.read();
    let view = structured(&read);
    assert!(view["proposal"].is_null() && view["active_plan"].is_null() && view["active_revision"].is_null(), "{view}");
    assert!(view["pending"].as_array().unwrap().is_empty() && view["executed_cycles"].as_array().unwrap().is_empty(), "{view}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "no execution before the new Chain's own Interview and approval");
    let (basis, _) = r.ask_and_interpret();
    let before = record_bytes(&r.record);
    // The previous Chain's approved close_report (approved: yes with its revision and digest) is not an approval here.
    refused(r.model("gil_close", json!({"report":old_close_report})), "the previous Chain's approval closing the new Synthesis");
    assert_eq!(record_bytes(&r.record), before);
    // Once an opening Question exists the ordinary proposal guidance applies; nothing about closing is suggested.
    lacks(&guidance(&r.read()), &["Chain을 닫는 제안"]);
    let proposed = ok(r.model("gil_plan_propose", json!({"draft":draft(&["B2"], &basis, &[])})), "the new Chain proposes its own plan");
    assert_eq!(structured(&proposed)["proposal"]["revision"], 1);
    assert!(structured(&proposed)["proposal"]["receipt"].is_null(), "a new proposal has no decision: {proposed}");
    has(&guidance(&r.read()), &["Monitor 버튼 응답을 기다린다"]);
    r.done();
}

#[test]
fn an_approved_closing_as_success_is_led_to_the_chain_report_and_stops() { closing_approval("success"); }

#[test]
fn an_approved_closing_as_failure_is_led_to_the_chain_report_and_stops() { closing_approval("failure"); }

#[test]
fn a_change_request_on_a_closing_proposal_is_not_an_approval_and_closes_nothing() {
    let mut r = Rig::start("handoff-closing-changes");
    r.executed(&["A", "B"]);
    let synthesis = r.closing_proposed(&["A", "B"], "success");
    r.decide(2, "request_changes");
    r.restart();
    let read = r.read();
    let changes = guidance(&read);
    has(&changes, &["종료 제안 판 2에 수정 요청이 왔다", "승인이 아니며 Chain은 열린 채로 남는다", "close_report(approved: no)", "다음 Question", "Chain Report를 제출하지 않고", "이전 승인을 재사용하지 않는다"]);
    lacks(&changes, &["승인됐다", "close_chain", "Experiment"]);
    assert!(r.context().contains(&changes));
    // Nothing of the closing procedure is reachable from a change request.
    let before = record_bytes(&r.record);
    refused(r.model("gil_close", json!({"report":format!("verdict: success\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: 수정 요청 뒤\n")})), "Chain Report after a change request");
    refused(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "Outcome while the declined Synthesis is open");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "Experiment while the Interview is open");
    assert_eq!(record_bytes(&r.record), before);

    let report = structured(&read)["close_report"].as_str().unwrap().to_owned();
    assert!(report.contains("approved: |\n  no"), "{report}");
    let closed = ok(r.model("gil_close", json!({"report":report})), "close the declined synthesis");
    has(&guidance(&closed), &["승인되지 않았다", "gil_open(kind: question)"]);
    lacks(&blocks(&closed), &["Closing Interview 마무리", "Chain Report"]);
    refused(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "Outcome after a declined Synthesis");
    // The Chain is still open and a revised proposal is a new revision with no decision.
    let context = r.context();
    has(&context, &["현재 Chain: chain:C1 · open"]);
    let (basis, _) = r.ask_and_interpret();
    let proposed = ok(r.model("gil_plan_propose", json!({"draft":draft(&["A", "B"], &basis, &[("chain_verdict", "failure")])})), "revised closing proposal");
    assert_eq!(structured(&proposed)["proposal"]["revision"], 3);
    assert!(structured(&proposed)["proposal"]["receipt"].is_null(), "the earlier decision is not carried over: {proposed}");
    has(&guidance(&r.read()), &["Monitor 버튼 응답을 기다린다"]);
    r.done();
}

/// The same closing path in a Chain that really runs under the exploration contract: an approved
/// requirement that no Experiment answered is closed as it is, by the user's approval of the
/// exact proposal, and stays stated as unanswered. Reached the way a Host reaches it: `gil_start`
/// in an `exploration-flow` build, a linked Experiment, the App-only review and decision.
#[cfg(all(feature = "exploration-flow", feature = "product-safety", feature = "visual-authoring", any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
#[path = "support_wave1/exploration.rs"]
mod exploration;

#[cfg(all(feature = "exploration-flow", feature = "product-safety", feature = "visual-authoring", any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
#[test]
fn an_exploration_chain_closes_with_an_unanswered_requirement_only_as_the_user_approved_it() {
    use exploration::{Rig as Explore, Run};
    let mut r = Explore::start("handoff-explore", &[("data/dates.csv", "id,date\n1,2026-01-02\n")]);
    assert_eq!(exploration::yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["contract"], "exploration");
    r.interview(&["R1", "R2"], 1, &[], "open_child");
    let c2 = r.open_experiment("R1");
    r.run(c2, &Run { requirement: "R1", problem: "모든 날짜가 ISO 형식인가", files: &[("evidence/dates.md", "한 행 모두 ISO 형식\n")], evidence: "evidence/dates.md", fails_back_to: None });
    // One success completed no requirement, and R2 has no linked Experiment at all.
    let view = r.read();
    assert_eq!(view["requirement_resolution"], "user_approved_closing_interview_only");
    assert_eq!(view["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["R2"]);

    ok(r.model("gil_open", json!({"kind":"interview"})), "closing interview");
    let mut basis = Vec::new();
    for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
        let opened = ok(r.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
        basis.push(step_ref(&opened, kind));
        ok(r.model("gil_close", json!({"report":report})), kind);
    }
    let opened = ok(r.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis");
    let synthesis = step_ref(&opened, "synthesis");
    let propose = r.read();
    has(propose["next"].as_str().unwrap(), &["이 제안이 Chain을 닫는 제안이라면", "원본 Report에서 읽어", "성공으로 바꾸지 않으며"]);
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["R1", "R2"], &basis, &[("chain_verdict", "failure")])})), "closing proposal");
    let review = ok(r.app("gil_plan_review", json!({})), "review");
    let t = token(&review).expect("capability in _meta");
    let digest = structured(&review)["proposal"]["digest"].clone();
    ok(r.app("gil_plan_decide", json!({"revision":2,"digest":digest,"token":t,"decision":"approve"})), "approve");
    r.restart();

    // The approval names the verdict the user approved. It adds no field that says a requirement
    // is answered: R2 is still listed with no Experiment, and the resolution is still the user's.
    let approved = r.read();
    let next = approved["next"].as_str().unwrap().lines().next().unwrap().to_owned();
    has(&next, &["종료 제안 판 2이(가) 승인됐다", "Chain을 failure로 닫는 데 동의했다", "verdict가 failure인 Chain Report", "Experiment를 열지 않고"]);
    assert_eq!(approved["requirement_resolution"], "user_approved_closing_interview_only");
    let r2 = approved["requirements"].as_array().unwrap().iter().find(|q| q["id"] == "R2").unwrap().clone();
    assert!(r2["experiments"].as_array().unwrap().is_empty(), "{r2}");
    for forbidden in ["resolved", "answered", "satisfied", "completion", "progress"] {
        assert!(!approved.as_object().unwrap().keys().any(|k| k.contains(forbidden)) && !r2.as_object().unwrap().keys().any(|k| k.contains(forbidden)), "no completion field `{forbidden}`: {approved}");
    }

    let closed = ok(r.model("gil_close", json!({"report":approved["close_report"]})), "close closing synthesis");
    has(&closed["result"]["content"].to_string(), &["Closing Interview 마무리", "chain_verdict: failure", "Chain 판정이 아니다"]);
    let outcome = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
    let outcome_ref = step_ref(&outcome, "outcome");
    let closed = ok(r.model("gil_close", json!({"report":format!("verdict: success\nlesson: 종료 제안을 승인받았다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 종료를 따른다\n")})), "close outcome");
    let text = closed["result"]["content"].to_string();
    has(&text, &["Closing Interview의 Cycle Report", "승인된 chain_verdict(failure)", "Experiment나 새 Cycle을 열지 않는다"]);
    lacks(&text, &["Experiment는 그 성공 뒤에 열고", "새 Experiment는 그 닫기가 성공한 뒤에 시작한다"]);
    // Another linked Experiment is not a way out of an approved closing.
    let link = r.link("R2");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link.clone()})), "a linked Experiment inside the Closing Interview");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 종료 제안을 승인받았다\nnext_direction.action: close_chain\nnext_direction.reason: 승인된 종료\n")})), "close closing interview");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link})), "a linked Experiment while the Chain Report is due");
    refused(r.model("gil_close", json!({"report":format!("verdict: success\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: 미해결 요구를 성공으로 바꿈\n")})), "an unanswered requirement turned into success");
    let handoff = "R1: ISO 형식 확인(cycle:C2). R2: 답하지 못함. 마치는 이유: 자료 부족. 후속 후보(미승인): 원천 시스템 확인";
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: {handoff}\n")})), "chain report");

    // The next Chain is explicit, reads the earlier record as written, and has approved nothing.
    refused(r.model("gil_open", json!({"kind":"experiment"})), "nothing opens in the closed Chain");
    ok(r.model("gil_open", json!({"kind":"chain"})), "next chain");
    let context = r.context();
    has(&context, &["새 Chain의 Opening Interview", "인계의 후보는 승인이 아니다", "이전 Chain: chain:C1 · failure", handoff]);
    let before = record_bytes(&r.record);
    {
        let earlier = structured(&ok(r.model("gil_report_read", json!({"owner":{"kind":"chain","ref":"chain:C1"}})), "earlier chain")).clone();
        assert_eq!(earlier["chain"]["state"], "closed");
        let fields: Vec<(&str, &str)> = earlier["report"]["fields"].as_array().unwrap().iter().map(|f| (f["name"].as_str().unwrap(), f["value"].as_str().unwrap())).collect();
        assert!(fields.contains(&("verdict", "failure")) && fields.contains(&("handoff_summary", handoff)), "the stored Chain Report is read verbatim: {earlier}");
        let judged = structured(&ok(r.model("gil_report_read", json!({"owner":{"kind":"step","ref":synthesis}})), "approved closing synthesis")).clone();
        let fields: Vec<(&str, &str)> = judged["report"]["fields"].as_array().unwrap().iter().map(|f| (f["name"].as_str().unwrap(), f["value"].as_str().unwrap())).collect();
        assert!(fields.contains(&("approved", "yes")) && fields.contains(&("chain_verdict", "failure")), "{judged}");
    }
    assert_eq!(record_bytes(&r.record), before, "reading the earlier Chain changed the record");
    let now = r.read();
    assert_eq!(now["contract"], "exploration");
    assert!(now["active_plan"].is_null() && now["proposal"].is_null() && now["requirements"].as_array().unwrap().is_empty(), "{now}");
}
