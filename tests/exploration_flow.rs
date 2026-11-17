//! First exploration flow over the real product-profile `gil mcp --serve` process.
//!
//! What this file fixes (Approved Plan contract, "후속 구현 이행"; Cycle Model §7·§9):
//!
//! - one approved requirement takes two different small yes/no Defines in sequence, with no new
//!   proposal or approval, and the first success does not complete the requirement;
//! - an Experiment opens only with an explicit link to an approved requirement of the active
//!   revision: no link, an unknown requirement, a superseded revision, another Chain's approval,
//!   an open Cycle and a not-yet-approved plan are each refused without changing the record;
//! - a failure goes through the explicit, approved revisit — with file changes and with no file
//!   difference at all — before a different small Define may follow under the same requirement;
//!   the working files and the produced outputs both return to the target world, while the
//!   failed Cycle's Report, its Snapshot objects and the preserved bytes stay;
//! - a Chain recorded without the marker keeps the item-queue meaning in this build (automatic
//!   first item, consumption, no link), and after it closes the next Chain opens under the
//!   exploration contract without rewriting the closed one;
//! - every position reads the same after a process restart, and reading changes nothing.
//!
//! Scope of the evidence: these are synthetic tool-path checks. The App decisions are harness
//! inputs, not a person's click. Whether a Define's wording really is a yes/no proposition, or
//! really contributes to its requirement, is natural-language meaning the Core does not verify;
//! that belongs to model and real-use evaluation, which this file does not perform.
#![cfg(all(
    feature = "exploration-flow",
    feature = "product-safety",
    feature = "visual-authoring",
    any(target_os = "macos", windows)
))]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use support::*;

struct Rig {
    root: PathBuf,
    record: PathBuf,
    settings: PathBuf,
    scope: String,
    c: Client,
    /// Everything the last Interview Cycle close said, guidance included.
    last_close: String,
}

/// How one small Experiment ends, with the files its Verify leaves behind.
struct Run<'a> {
    requirement: &'a str,
    /// The yes/no proposition this Define fixes.
    problem: &'a str,
    files: &'a [(&'a str, &'a str)],
    evidence: &'a str,
    /// `None`: success. `Some(target)`: failure declaring `revisit` to that Cycle.
    fails_back_to: Option<&'a str>,
}

impl Rig {
    fn start(label: &str, seed: &[(&str, &str)]) -> Self {
        let root = scratch(label);
        let settings = scratch(&format!("{label}-settings"));
        for (path, bytes) in seed {
            let at = root.join(path);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::write(at, bytes).unwrap();
        }
        let c = Client::serve(&settings);
        let record = product_state_path(&root, &private_local_app_data(&settings));
        let mut r = Rig { root, record, settings, scope: String::new(), c, last_close: String::new() };
        ok(r.model("gil_start", json!({})), "start");
        r.prepare();
        r
    }
    fn prepare(&mut self) {
        let prepared = ok(self.model("gil_monitor_prepare", json!({})), "prepare");
        self.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
    }
    fn model(&mut self, name: &str, mut args: Value) -> Value {
        args["project_root"] = json!(self.root);
        self.c.call(name, args)
    }
    fn app(&mut self, name: &str, mut args: Value) -> Value {
        args["scope_id"] = json!(self.scope);
        self.c.call(name, args)
    }
    fn read(&mut self) -> Value {
        structured(&ok(self.model("gil_plan_read", json!({})), "gil_plan_read")).clone()
    }
    fn context(&mut self) -> String {
        said(&ok(self.model("gil_context", json!({})), "context"))
    }
    fn restart(&mut self) {
        let old = std::mem::replace(&mut self.c, Client::serve(&self.settings));
        let (status, noise) = old.eof();
        assert!(status.success() && noise.is_empty(), "old Core exit {status:?} noise {noise:?}");
        self.prepare();
    }
    /// Interview up to an approved, closed plan. `action` is `open_child` or `close_chain`.
    fn interview(&mut self, ids: &[&str], revision: u64, extra: &[(&str, &str)], action: &str) -> String {
        let mut basis = Vec::new();
        for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
            let opened = ok(self.model("gil_open", json!({"kind":kind,"contract":CONTRACT})), kind);
            basis.push(step_ref(&opened, kind));
            ok(self.model("gil_close", json!({"report":report})), kind);
        }
        let opened = ok(self.model("gil_open", json!({"kind":"synthesis","contract":CONTRACT})), "synthesis");
        let synthesis = step_ref(&opened, "synthesis");
        ok(self.model("gil_plan_propose", json!({"draft":draft(ids, &basis, extra)})), "propose");
        let review = ok(self.app("gil_plan_review", json!({})), "plan review");
        let t = token(&review).expect("capability in _meta");
        let p = structured(&review)["proposal"].clone();
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":"approve"})), "approve");
        let read = self.read();
        ok(self.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
        let outcome = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome_ref = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 요구를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 요구를 탐색한다\n")})), "close outcome");
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 요구를 Monitor 응답으로 승인받았다\nnext_direction.action: {action}\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
        self.last_close = closed["result"]["content"].to_string();
        synthesis
    }
    /// The link body for `gil_open(kind: experiment)`, copied from the active approved revision.
    fn link(&mut self, requirement: &str) -> String {
        let read = self.read();
        link_body(&read["active_plan"]["revision"], read["active_plan"]["digest"].as_str().expect("active plan digest"), requirement)
    }
    fn open_experiment(&mut self, requirement: &str) -> u32 {
        let body = self.link(requirement);
        let opened = ok(self.model("gil_open", json!({"kind":"experiment","contract":body})), "open linked experiment");
        assert!(said(&opened).contains(&format!("승인 요구 {requirement}에 기여")), "{opened}");
        cycle_number(&opened)
    }
    /// Walks one already-open Experiment to its Cycle Report and returns the Cycle number.
    fn run(&mut self, n: u32, run: &Run<'_>) {
        let read = self.read();
        assert_eq!(read["current_experiment"]["requirement_id"], run.requirement, "{read}");
        let document = read["document"].as_str().expect("document after linking").to_owned();
        let define = ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        assert!(define["result"]["content"].to_string().contains("탐색 계약의 Define"), "exploration Define guidance: {define}");
        ok(self.model("gil_close", json!({"report":format!("problem: {}\nsuccess_condition: 지정한 파일에서 관측한 값으로 예/아니오를 판정한다\n", run.problem)})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        let verify = step_ref(&v, "verify");
        self.write("plan/approved-plan.md", &document);
        for (path, bytes) in run.files {
            self.write(path, bytes);
        }
        let failed = run.fails_back_to.is_some();
        ok(self.c.call("gil_close", verify_args(&self.root, &verify, &format!("execution: 합성 자료를 관측했다\nresult: {}\nplan_document: plan/approved-plan.md\nevidence_files: {}\n", if failed { "판정 기준에 미달했다" } else { "판정 기준을 충족했다" }, run.evidence))), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        let yes = if failed { "no" } else { "yes" };
        ok(self.model("gil_close", json!({"report":format!("hypothesis_fit: {yes}\nproblem_solved: {yes}\nsuccess_condition_met: {yes}\nguardrail_triggered: no\ninterpretation: 관측한 값과 판정 기준을 대조했다\n")})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
        let outcome_ref = step_ref(&o, "outcome");
        let verdict = if failed { "failure" } else { "success" };
        ok(self.model("gil_close", json!({"report":format!("verdict: {verdict}\nlesson: 원인 위치와 복원 기준점을 구분해 다음 작은 질문의 근거로 남긴다\nnext_direction.action: close_cycle\nnext_direction.reason: 이 질문의 판정을 마쳤다\n")})), "close outcome");
        let direction = match run.fails_back_to {
            Some(target) => format!("next_direction.action: revisit\nnext_direction.target_cycle_ref: {target}\n"),
            None => "next_direction.action: open_child\n".to_string(),
        };
        let closed = ok(self.model("gil_close", json!({"report":format!("verdict: {verdict}\noutcome_ref: {outcome_ref}\nhandoff_summary: C{n}의 관측과 한계를 남겼다\n{direction}next_direction.reason: 관측에 따라 다음을 정한다\n")})), "close experiment");
        assert!(said(&closed).contains(&format!("cycle:C{n} · experiment · {verdict}")), "{closed}");
        assert!(said(&closed).contains("실행 계약: 탐색"), "an exploration Chain's close receipt names its contract: {closed}");
    }
    fn write(&self, path: &str, bytes: &str) {
        let at = self.root.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
    }
    /// Approved recovery through the App-only pair; returns the reviewed plan.
    fn recover(&mut self) -> Value {
        let response = ok(self.app("gil_revisit_review", json!({})), "recovery review");
        let token = response["result"]["_meta"]["gil/revisitApprovalToken"].as_str().expect("recovery capability").to_owned();
        let view = structured(&response).clone();
        assert_eq!(view["state"], "awaiting_approval", "{view}");
        let done = ok(self.app("gil_revisit_apply", json!({"digest":view["digest"],"token":token})), "approved recovery");
        assert_eq!(structured(&done)["state"], "completed", "{done}");
        view
    }
    fn experiments(&mut self, requirement: &str) -> Vec<(String, String)> {
        let read = self.read();
        let found = read["requirements"].as_array().unwrap().iter().find(|r| r["id"] == requirement).unwrap_or_else(|| panic!("requirement {requirement}: {read}")).clone();
        found["experiments"].as_array().unwrap().iter()
            .map(|e| (e["cycle_ref"].as_str().unwrap().to_owned(), e["verdict"].as_str().unwrap_or(e["state"].as_str().unwrap()).to_owned()))
            .collect()
    }
}

fn link_body(revision: &Value, digest: &str, requirement: &str) -> String {
    format!("plan_revision: {revision}\nplan_digest: {digest}\nrequirement_id: {requirement}\nrelevance: 요구 {requirement}의 답에 필요한 관측이다\nreadiness: 입력 파일이 프로젝트에 있다\nuncertainty: 아직 판정하지 못한 한 가지를 줄인다\n")
}

/// Working-folder files by `/`-separated relative path, excluding the root `.gil` record folder.
fn world(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                if at == base && p.file_name().unwrap() == ".gil" {
                    continue;
                }
                visit(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, fs::read(p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn yaml(record: &Path) -> Value {
    serde_norway::from_slice(&record_bytes(record)).unwrap()
}
fn object_count(record: &Path) -> usize {
    fn visit(at: &Path) -> usize {
        fs::read_dir(at).map(|entries| entries.map(|e| e.unwrap().path()).map(|p| if p.is_dir() { visit(&p) } else { 1 }).sum()).unwrap_or(0)
    }
    visit(&record.parent().unwrap().join("artifacts"))
}
/// Reading tools project the record; they never write it.
fn reads_change_nothing(r: &mut Rig) {
    let before = record_bytes(&r.record);
    let (plan, context) = (r.read(), r.context());
    assert_eq!((r.read(), r.context()), (plan, context), "the projection is deterministic");
    ok(r.model("gil_status", json!({})), "status");
    assert_eq!(record_bytes(&r.record), before, "a read changed the record");
}

#[test]
fn one_requirement_takes_two_small_defines_without_reapproval_and_reads_the_same_after_restart() {
    let mut r = Rig::start("explore-two-defines", &[("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")]);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["contract"], "exploration");

    // Before approval nothing executes, with or without a link.
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &"0".repeat(64), "R1")})), "experiment before any approval");
    r.interview(&["R1", "R2"], 1, &[], "open_child");
    assert!(r.last_close.contains("탐색 계약이라 항목이 자동으로 묶이지 않는다") && !r.last_close.contains("첫 미실행 항목이 자동으로 묶인다"), "{}", r.last_close);
    let approved = r.read();
    assert_eq!(approved["contract"], "exploration");
    assert_eq!(approved["requirement_resolution"], "user_approved_closing_interview_only");
    assert!(approved["next"].as_str().unwrap().contains("requirement_id: R1 | R2"), "{approved}");
    assert!(approved["experiment_link"].as_str().unwrap().contains(approved["active_plan"]["digest"].as_str().unwrap()));

    // The link is required and checked; every refusal leaves the record as it was.
    let before = record_bytes(&r.record);
    let digest = approved["active_plan"]["digest"].as_str().unwrap().to_owned();
    let no_link = refused(r.model("gil_open", json!({"kind":"experiment"})), "no first pending item is picked");
    assert!(no_link.to_string().contains("gil_plan_read") && no_link.to_string().contains("requirement_id"), "{no_link}");
    assert!(no_link.to_string().contains("승인 요구 연결의 문제") && !no_link.to_string().contains("행동 계약(objective"), "a refused link is told apart from a contract error and from an approval state: {no_link}");
    let unknown = refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &digest, "R9")})), "unapproved requirement");
    assert!(unknown.to_string().contains("R9") && unknown.to_string().contains("요구 연결"), "{unknown}");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &"f".repeat(64), "R1")})), "digest of no approval");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":format!("plan_revision: 1\nplan_digest: {digest}\nrequirement_id: R1\n")})), "missing selection statements");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":format!("{}extra: x\n", link_body(&json!(1), &digest, "R1"))})), "unknown link field");
    assert_eq!(record_bytes(&r.record), before, "a refused link changed the record");

    // First small Define under R1 succeeds.
    let c2 = r.open_experiment("R1");
    assert_eq!(c2, 2);
    let open = r.read();
    assert_eq!(open["current_experiment"]["cycle_ref"], "cycle:C2");
    assert_eq!(open["current_experiment"]["selection"]["readiness"], "입력 파일이 프로젝트에 있다");
    assert_eq!(open["current_item"]["id"], "R1");
    let link = r.link("R2");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link})), "a second Cycle while one is open");
    let context = r.context();
    for needle in ["실행 계약: 탐색", "승인 요구 R1에 기여", "모든 요구는 미해결 상태다", "cycle:C2 open"] {
        assert!(context.contains(needle), "{needle}: {context}");
    }
    r.run(c2, &Run { requirement: "R1", problem: "지정한 자료에 허용 날짜 형식(YYYY-MM-DD)을 벗어나는 값이 존재하는가?",
        files: &[("evidence/C2.md", "2행이 DD/MM/YYYY 형식이다: 존재한다\n")], evidence: "evidence/C2.md", fails_back_to: None });

    // One success is not the requirement: R1 stays, the Chain stays open, nothing was re-approved.
    let after = r.read();
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "success".to_owned())]);
    assert_eq!(r.experiments("R2"), vec![]);
    assert_eq!(after["active_revision"], 1);
    assert_eq!(after["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["R2"], "pending lists requirements with no experiment yet");
    assert_eq!(after["retry_pending"], json!([]));
    assert!(after["next"].as_str().unwrap().contains("이 성공은 승인 요구의 완료가 아니다"), "{after}");
    let record = yaml(&r.record);
    assert_eq!(record["chains"]["nodes"][0]["state"], "open");
    assert_eq!(record["chains"]["nodes"][0]["approved_plan"]["proposals"].as_array().unwrap().len(), 1);

    // A second, different small Define under the same requirement — no proposal, no approval.
    let c3 = r.open_experiment("R1");
    assert_eq!(c3, 3);
    r.run(c3, &Run { requirement: "R1", problem: "허용 형식을 벗어난 모든 값을 정보 손실 없이 YYYY-MM-DD로 변환할 수 있는가?",
        files: &[("evidence/C3.md", "02/01/2026은 일·월 순서가 정해지면 변환된다: 가능하다\n"), ("out/dates.csv", "id,date\n1,2026-01-02\n2,2026-01-02\n")],
        evidence: "evidence/C3.md", fails_back_to: None });
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "success".to_owned()), ("cycle:C3".to_owned(), "success".to_owned())]);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["proposals"].as_array().unwrap().len(), 1, "no re-approval for a second Define");
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["state"], "open", "two successes do not close the Chain");

    // Save, restart, read: the same meaning, and reading writes nothing.
    let (plan, context, bytes) = (r.read(), r.context(), record_bytes(&r.record));
    r.restart();
    assert_eq!(r.read(), plan);
    assert_eq!(r.context(), context);
    assert_eq!(record_bytes(&r.record), bytes);
    reads_change_nothing(&mut r);

    // A requirement itself must change: that is not a Define. The request is preserved, the
    // Experiment finishes truthfully, and until the new approval nothing executes — said as an
    // approval state, not as a missing link.
    let c4 = r.open_experiment("R2");
    ok(r.model("gil_plan_request_interview", json!({"reason":"R3(금액 열)도 답해야 한다"})), "preserve the scope change");
    r.run(c4, &Run { requirement: "R2", problem: "금액 열에 숫자가 아닌 값이 존재하는가?", files: &[("evidence/C4.md", "숫자가 아닌 값 없음\n")], evidence: "evidence/C4.md", fails_back_to: None });
    let before = record_bytes(&r.record);
    for contract in [None, Some(link_body(&json!(1), &digest, "R1"))] {
        let args = match contract { Some(body) => json!({"kind":"experiment","contract":body}), None => json!({"kind":"experiment"}) };
        let waiting = refused(r.model("gil_open", args), "execution while a re-interview is pending");
        assert!(waiting.to_string().contains("재인터뷰 요청") && !waiting.to_string().contains("요구 연결"), "{waiting}");
    }
    assert_eq!(record_bytes(&r.record), before);

    // The Interview that changes the requirements is a new approval; the old revision is stale.
    ok(r.model("gil_open", json!({"kind":"interview"})), "re-interview");
    r.interview(&["R1", "R2", "R3"], 2, &[], "open_child");
    let stale = refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &digest, "R1")})), "superseded approval");
    assert!(stale.to_string().contains("지난 승인"), "{stale}");
    assert_eq!(r.open_experiment("R3"), 6);
    assert_eq!(r.experiments("R1").len(), 2, "earlier links stay with their own revision");
}

#[test]
fn a_failure_restores_work_and_outputs_through_the_approved_revisit_before_a_new_small_define() {
    let mut r = Rig::start("explore-failure-restore", &[("src/convert.py", "def convert(v): return v\n"), ("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")]);
    r.interview(&["R1"], 1, &[], "open_child");
    let c2 = r.open_experiment("R1");
    r.run(c2, &Run { requirement: "R1", problem: "허용 날짜 형식을 벗어나는 값이 존재하는가?",
        files: &[("evidence/C2.md", "벗어나는 값 1건\n"), ("out/report.txt", "C2 output: 1 irregular date\n")], evidence: "evidence/C2.md", fails_back_to: None });
    let world_at_c2 = world(&r.root);

    // C3 changes the work (code), overwrites an output, adds a new output and fails.
    let c3 = r.open_experiment("R1");
    r.run(c3, &Run { requirement: "R1", problem: "대상 파일의 모든 날짜를 합의한 규칙으로 공통 형식에 변환할 수 있는가?",
        files: &[("src/convert.py", "def convert(v): return v[::-1]  # wrong\n"), ("out/report.txt", "C3 output: half converted\n"),
            ("out/dates.csv", "id,date\n1,2026-01-02\n2,\n"), ("evidence/C3.md", "2행을 변환하지 못했다: 결측 규칙이 없다\n")],
        evidence: "evidence/C3.md", fails_back_to: Some("cycle:C2") });
    let world_at_c3 = world(&r.root);
    assert_ne!(world_at_c2, world_at_c3);
    let objects = object_count(&r.record);
    let failed_report = yaml(&r.record)["chains"]["nodes"][0]["cycles"]["nodes"].as_array().unwrap().iter().find(|c| c["id"] == 3).unwrap().clone();
    assert_eq!(failed_report["report"]["verdict"], "failure");

    // No next experiment and no model-side restore before the approved revisit.
    let record_before = record_bytes(&r.record);
    let link = r.link("R1");
    let bypass = refused(r.model("gil_open", json!({"kind":"experiment","contract":link.clone()})), "next experiment before the revisit");
    assert!(!bypass.to_string().contains("열었다:"), "{bypass}");
    refused(r.model("gil_revisit", json!({})), "model call without a durable approval");
    refused(r.model("gil_restore", json!({})), "general restore stays blocked");
    assert_eq!(record_bytes(&r.record), record_before);
    assert_eq!(world(&r.root), world_at_c3);
    let next = r.context();
    assert!(next.contains("파일 변경이 없어도 되돌아가기를 생략하지 않는다"), "{next}");

    let view = r.recover();
    assert_eq!((view["from"].as_str(), view["target"].as_str()), (Some("cycle:C3"), Some("cycle:C2")));
    let listed: BTreeMap<String, String> = view["changes"].as_array().unwrap().iter()
        .map(|c| (c["path"].as_str().unwrap().to_owned(), c["action"].as_str().unwrap().to_owned())).collect();
    assert_eq!(listed.get("src/convert.py").map(String::as_str), Some("replace"), "the work is restored: {view}");
    assert_eq!(listed.get("out/report.txt").map(String::as_str), Some("replace"), "the produced output is restored too: {view}");
    assert_eq!(listed.get("out/dates.csv").map(String::as_str), Some("preserve"), "a failed-only output is not kept in the world: {view}");

    // The whole tracked world is the C2 world: work and outputs alike, nothing carried over.
    assert_eq!(world(&r.root), world_at_c2);
    // The failure stays as evidence and learning: Report, Snapshot objects, preserved bytes.
    let cycles = yaml(&r.record)["chains"]["nodes"][0]["cycles"].clone();
    assert_eq!(cycles["nodes"].as_array().unwrap().iter().find(|c| c["id"] == 3).unwrap(), &failed_report, "the failed Cycle's record is untouched");
    assert!(object_count(&r.record) >= objects, "no Snapshot object was removed");
    let preserved = PathBuf::from(view["archive"].as_str().unwrap());
    let mapping: Value = serde_json::from_slice(&fs::read(preserved.join("files.json")).unwrap()).unwrap();
    let saved = mapping.as_array().unwrap().iter().find(|m| m["path"] == "out/dates.csv").expect("failed output preserved");
    assert_eq!(fs::read(preserved.join(saved["saved_as"].as_str().unwrap())).unwrap(), world_at_c3["out/dates.csv"]);
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "success".to_owned()), ("cycle:C3".to_owned(), "failure".to_owned())]);

    // After the restore a *different* small Define follows under the same requirement, and it
    // records where it came from. No re-approval, and the queue's same-item retry is not used.
    let said_next = r.context();
    assert!(said_next.contains("실패한 Define을 되풀이할 필요는 없다"), "{said_next}");
    let c4 = r.open_experiment("R1");
    assert_eq!(c4, 4);
    let read = r.read();
    assert_eq!(read["current_experiment"]["revisit_from"], "cycle:C3");
    assert_eq!(read["retry_pending"], json!([]));
    let born = yaml(&r.record)["chains"]["nodes"][0]["cycles"]["nodes"].as_array().unwrap().iter().find(|c| c["id"] == 4).unwrap().clone();
    assert_eq!(born["parent"], 2, "{born}");
    r.run(c4, &Run { requirement: "R1", problem: "대상 자료의 날짜 열에 결측값이 존재하는가?",
        files: &[("evidence/C4.md", "결측값 없음: 존재하지 않는다\n")], evidence: "evidence/C4.md", fails_back_to: Some("cycle:C2") });
    assert_eq!(r.experiments("R1").len(), 3);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["proposals"].as_array().unwrap().len(), 1);
    r.restart();
    assert_eq!(r.experiments("R1").iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(), vec!["success", "failure", "failure"]);
    reads_change_nothing(&mut r);
}

#[test]
fn a_failure_that_leaves_no_file_difference_still_takes_the_recorded_revisit() {
    // C2 writes log=A, C3 writes log=B, the failing C4 writes log=A again: its world is the C2
    // world byte for byte, so the recovery has nothing to change. The revisit is recorded anyway.
    let mut r = Rig::start("explore-failure-noop", &[("data/in.txt", "input\n")]);
    r.interview(&["R1"], 1, &[], "open_child");
    let c2 = r.open_experiment("R1");
    r.run(c2, &Run { requirement: "R1", problem: "입력 파일이 비어 있지 않은가?", files: &[("evidence/log.md", "A\n")], evidence: "evidence/log.md", fails_back_to: None });
    let world_at_c2 = world(&r.root);
    let c3 = r.open_experiment("R1");
    r.run(c3, &Run { requirement: "R1", problem: "입력 파일이 한 줄인가?", files: &[("evidence/log.md", "B\n")], evidence: "evidence/log.md", fails_back_to: None });
    let c4 = r.open_experiment("R1");
    r.run(c4, &Run { requirement: "R1", problem: "입력 파일이 규칙에 맞는가?", files: &[("evidence/log.md", "A\n")], evidence: "evidence/log.md", fails_back_to: Some("cycle:C2") });
    assert_eq!(world(&r.root), world_at_c2, "the failed world equals the target world");

    let link = r.link("R1");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link})), "no shortcut when nothing would change");
    refused(r.model("gil_revisit", json!({})), "the approval is still required");
    let view = r.recover();
    assert_eq!(view["changes"], json!([]), "{view}");
    assert_eq!((view["from"].as_str(), view["target"].as_str()), (Some("cycle:C4"), Some("cycle:C2")));
    assert_eq!(world(&r.root), world_at_c2);
    assert!(PathBuf::from(view["archive"].as_str().unwrap()).join("approval.json").is_file(), "the restore path is recorded even without changes");

    let c5 = r.open_experiment("R1");
    let read = r.read();
    assert_eq!(read["current_experiment"]["revisit_from"], "cycle:C4");
    let born = yaml(&r.record)["chains"]["nodes"][0]["cycles"]["nodes"].as_array().unwrap().iter().find(|c| c["id"] == c5).unwrap().clone();
    assert_eq!(born["parent"], 2, "{born}");
    assert_eq!(r.experiments("R1").iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(), vec!["success", "success", "failure", "open"]);
}

#[test]
fn an_item_queue_chain_keeps_its_meaning_and_the_next_chain_opens_under_the_new_contract() {
    let mut r = Rig::start("explore-after-queue", &[("data/in.txt", "input\n")]);
    // A Project started by a build without the exploration contract: the same record, no marker.
    let text = String::from_utf8(record_bytes(&r.record)).unwrap();
    let old: String = text.lines().filter(|line| line.trim() != "contract: exploration").map(|line| format!("{line}\n")).collect();
    assert_eq!(text.lines().count(), old.lines().count() + 1, "exactly the marker line is absent in an older record");
    fs::write(&r.record, old).unwrap();
    r.restart();

    r.interview(&["A", "B"], 1, &[], "open_child");
    assert!(r.last_close.contains("첫 미실행 항목이 자동으로 묶인다") && !r.last_close.contains("탐색 계약"), "the queue's close guidance is unchanged: {}", r.last_close);
    let queued = r.read();
    assert!(queued.get("contract").is_none() && queued.get("requirements").is_none(), "an item-queue view is unchanged: {queued}");
    assert!(queued["next"].as_str().unwrap().contains("자동으로 묶인다"), "{queued}");

    // Automatic first item; a link body is not read and steers nothing.
    let opened = ok(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), queued["active_plan"]["digest"].as_str().unwrap(), "B")})), "queue experiment");
    assert!(!said(&opened).contains("실행 계약: 탐색"), "{opened}");
    let bound = r.read();
    assert_eq!(bound["current_item"]["id"], "A");
    assert!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["executed_cycles"][0].get("selection").is_none());
    let document = bound["document"].as_str().unwrap().to_owned();
    let define = ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
    let guidance = define["result"]["content"].to_string();
    assert!(guidance.contains("새 설계는 아직 활성화되지 않았다") && !guidance.contains("탐색 계약"), "the queue keeps its own guidance: {guidance}");
    ok(r.model("gil_close", json!({"report":"problem: 항목 A을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n"})), "close define");
    ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let v = ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
    r.write("plan/approved-plan.md", &document);
    r.write("evidence/A.md", "항목 A 결과\n");
    ok(r.c.call("gil_close", verify_args(&r.root, &step_ref(&v, "verify"), "execution: 확인했다\nresult: 저장했다\nplan_document: plan/approved-plan.md\nevidence_files: evidence/A.md\n")), "close verify");
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    ok(r.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 일치했다\n"})), "close analysis");
    let o = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome_ref = step_ref(&o, "outcome");
    ok(r.model("gil_close", json!({"report":"verdict: success\nlesson: 항목 A를 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n"})), "close outcome");
    let closed = ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 항목 A 완료\nnext_direction.action: open_child\nnext_direction.reason: 남은 항목\n")})), "close C2");
    assert!(!said(&closed).contains("실행 계약: 탐색"), "{closed}");
    // The success consumed A: the queue's meaning, unchanged.
    let after = r.read();
    assert_eq!(after["pending"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["B"]);
    assert!(!String::from_utf8(record_bytes(&r.record)).unwrap().contains("contract"), "the item-queue record gained no marker");

    // Close the Chain through its user-approved Closing Interview, then open the next Chain.
    ok(r.model("gil_open", json!({"kind":"interview"})), "closing interview");
    let closing = r.interview(&["A", "B"], 2, &[("chain_verdict", "failure")], "close_chain");
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\nclosing_synthesis_ref: {closing}\nhandoff_summary: A는 확인했고 B는 남았다\n")})), "chain report");
    let first_chain = yaml(&r.record)["chains"]["nodes"][0].clone();
    let old_digest = first_chain["approved_plan"]["proposals"][0]["digest"].as_str().unwrap().to_owned();
    ok(r.model("gil_open", json!({"kind":"chain"})), "next chain");
    let record = yaml(&r.record);
    assert_eq!(record["chains"]["nodes"][0], first_chain, "the closed item-queue Chain is not rewritten");
    assert!(record["chains"]["nodes"][0]["approved_plan"].get("contract").is_none());
    assert_eq!(record["chains"]["nodes"][1]["approved_plan"]["contract"], "exploration");

    // The new Chain agrees on its own requirements in its own Interview; the previous approval
    // is evidence to read, never an execution approval here.
    let handoff = r.context();
    assert!(handoff.contains("A는 확인했고 B는 남았다"), "the previous answer and limits reach the new Interview: {handoff}");
    refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &old_digest, "B")})), "no execution before the new Chain's approval");
    r.interview(&["N1"], 1, &[], "open_child");
    let other = refused(r.model("gil_open", json!({"kind":"experiment","contract":link_body(&json!(1), &old_digest, "B")})), "another Chain's approval");
    assert!(other.to_string().contains("chain:C1"), "{other}");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "the new Chain binds nothing automatically");
    r.open_experiment("N1");
    let (plan, bytes) = (r.read(), record_bytes(&r.record));
    r.restart();
    assert_eq!(r.read(), plan);
    assert_eq!(record_bytes(&r.record), bytes);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0], first_chain);
}
