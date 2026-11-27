//! A Project whose first Chain really is an exploration Chain, driven over the stdio process.
//!
//! Shared by tests/exploration_flow.rs and tests/exploration_tools.rs. The Chain is reached the
//! way a Host reaches it and in no other way: `gil_start` in an `exploration-flow` build, the
//! opening Interview's proposal, the App-only review and decision, the approved Synthesis and an
//! Experiment opened with an explicit requirement link. Nothing here strips the contract marker,
//! writes the approval ledger, or injects a bare `approved: yes`.
//!
//! The App calls (`gil_plan_review`/`gil_plan_decide`, `gil_revisit_review`/`gil_revisit_apply`)
//! are **synthetic harness input**. They exercise the same tools the Monitor buttons call, but no
//! person looked at a screen or clicked anything.
#![allow(dead_code)]
use super::support::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Rig {
    pub root: PathBuf,
    pub record: PathBuf,
    pub settings: PathBuf,
    pub scope: String,
    pub c: Client,
    /// Everything the last Interview Cycle close said, guidance included.
    pub last_close: String,
}

/// How one small Experiment ends, with the files its Verify leaves behind.
pub struct Run<'a> {
    pub requirement: &'a str,
    /// The yes/no proposition this Define fixes.
    pub problem: &'a str,
    pub files: &'a [(&'a str, &'a str)],
    pub evidence: &'a str,
    /// `None`: success. `Some(target)`: failure declaring `revisit` to that Cycle.
    pub fails_back_to: Option<&'a str>,
}

impl Rig {
    pub fn start(label: &str, seed: &[(&str, &str)]) -> Self {
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
        ok(r.model("gil_start", direct_start()), "start");
        r.prepare();
        r
    }
    pub fn prepare(&mut self) {
        let prepared = ok(self.model("gil_monitor_prepare", json!({})), "prepare");
        self.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
    }
    pub fn model(&mut self, name: &str, mut args: Value) -> Value {
        args["project_root"] = json!(self.root);
        self.c.call(name, args)
    }
    pub fn app(&mut self, name: &str, mut args: Value) -> Value {
        args["scope_id"] = json!(self.scope);
        self.c.call(name, args)
    }
    pub fn read(&mut self) -> Value {
        structured(&ok(self.model("gil_plan_read", json!({})), "gil_plan_read")).clone()
    }
    pub fn context(&mut self) -> String {
        said(&ok(self.model("gil_context", json!({})), "context"))
    }
    pub fn restart(&mut self) {
        let old = std::mem::replace(&mut self.c, Client::serve(&self.settings));
        let (status, noise) = old.eof();
        assert!(status.success() && noise.is_empty(), "old Core exit {status:?} noise {noise:?}");
        self.prepare();
    }
    /// Interview up to an approved, closed plan. `action` is `open_child` or `close_chain`.
    pub fn interview(&mut self, ids: &[&str], revision: u64, extra: &[(&str, &str)], action: &str) -> String {
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
    pub fn link(&mut self, requirement: &str) -> String {
        let read = self.read();
        link_body(&read["active_plan"]["revision"], read["active_plan"]["digest"].as_str().expect("active plan digest"), requirement)
    }
    pub fn open_experiment(&mut self, requirement: &str) -> u32 {
        let body = self.link(requirement);
        let opened = ok(self.model("gil_open", json!({"kind":"experiment","contract":body})), "open linked experiment");
        assert!(said(&opened).contains(&format!("승인 요구 {requirement}에 기여")), "{opened}");
        cycle_number(&opened)
    }
    /// Define and Hypothesis of the already-open linked Experiment, then its Verify, opened with
    /// the approved document saved where the Report will name it. Returns the Verify's StepRef.
    pub fn open_verify(&mut self, requirement: &str, problem: &str) -> String {
        let read = self.read();
        assert_eq!(read["current_experiment"]["requirement_id"], requirement, "{read}");
        let document = read["document"].as_str().expect("document after linking").to_owned();
        let define = ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        assert!(define["result"]["content"].to_string().contains("탐색 계약의 Define"), "exploration Define guidance: {define}");
        ok(self.model("gil_close", json!({"report":format!("problem: {problem}\nsuccess_condition: 지정한 파일의 관측 증거가 위 명제의 긍정 답을 뒷받침한다. 근거 있는 부정 답이나 판정 미도달은 성공이 아니다\n")})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        // Opening the Verify saved the approved document itself, byte for byte, and said where.
        assert_eq!(fs::read_to_string(self.root.join(PLAN_DOCUMENT)).expect("GIL saved the plan document"), document);
        assert!(v["result"]["content"].to_string().contains(PLAN_DOCUMENT), "the receipt names the saved path: {v}");
        step_ref(&v, "verify")
    }
    /// Walks one already-open Experiment to its Cycle Report.
    pub fn run(&mut self, n: u32, run: &Run<'_>) {
        let verify = self.open_verify(run.requirement, run.problem);
        for (path, bytes) in run.files {
            self.write(path, bytes);
        }
        let failed = run.fails_back_to.is_some();
        ok(self.c.call("gil_close", verify_args(&self.root, &verify, &format!("execution: 합성 자료를 관측했다\nresult: {}\nplan_document: {PLAN_DOCUMENT}\nevidence_files: {}\n", if failed { "판정 기준에 미달했다" } else { "판정 기준을 충족했다" }, run.evidence))), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(self.model("gil_close", json!({"report":analysis_report(failed)})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
        let outcome_ref = step_ref(&o, "outcome");
        ok(self.model("gil_close", json!({"report":outcome_report(failed)})), "close outcome");
        let closed = ok(self.model("gil_close", json!({"report":cycle_report(n, &outcome_ref, run.fails_back_to)})), "close experiment");
        let verdict = if failed { "failure" } else { "success" };
        assert!(said(&closed).contains(&format!("cycle:C{n} · experiment · {verdict}")), "{closed}");
        assert!(said(&closed).contains("실행 계약: 탐색"), "an exploration Chain's close receipt names its contract: {closed}");
    }
    pub fn write(&self, path: &str, bytes: &str) {
        self.write_bytes(path, bytes.as_bytes());
    }
    pub fn write_bytes(&self, path: &str, bytes: &[u8]) {
        let at = self.root.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
    }
    /// The recovery the agent carries out with gil_revisit; returns the plan as the App reads it.
    pub fn recover(&mut self) -> Value {
        let response = ok(self.app("gil_revisit_review", json!({})), "recovery review");
        let view = structured(&response).clone();
        assert_eq!(view["state"], "ready", "{view}");
        // Nobody approves a recovery: the agent's own gil_revisit carries the declared revisit out.
        let root = self.root.clone();
        let done = ok(revisit(&mut self.c, &root), "the agent's revisit");
        assert_eq!(structured(&done)["state"], "completed", "{done}");
        view
    }
    pub fn experiments(&mut self, requirement: &str) -> Vec<(String, String)> {
        let read = self.read();
        let found = read["requirements"].as_array().unwrap().iter().find(|r| r["id"] == requirement).unwrap_or_else(|| panic!("requirement {requirement}: {read}")).clone();
        found["experiments"].as_array().unwrap().iter()
            .map(|e| (e["cycle_ref"].as_str().unwrap().to_owned(), e["verdict"].as_str().unwrap_or(e["state"].as_str().unwrap()).to_owned()))
            .collect()
    }
}

/// Where a Verify saves the approved document; its Report names this path as `plan_document`.
pub const PLAN_DOCUMENT: &str = "plan/approved-plan.md";

pub fn link_body(revision: &Value, digest: &str, requirement: &str) -> String {
    format!("plan_revision: {revision}\nplan_digest: {digest}\nrequirement_id: {requirement}\nrelevance: 요구 {requirement}의 답에 필요한 관측이다\nreadiness: 입력 파일이 프로젝트에 있다\nuncertainty: 아직 판정하지 못한 한 가지를 줄인다\n")
}
pub fn analysis_report(failed: bool) -> String {
    let yes = if failed { "no" } else { "yes" };
    format!("hypothesis_fit: {yes}\nproblem_solved: {yes}\nsuccess_condition_met: {yes}\nguardrail_triggered: no\ninterpretation: 관측한 값과 판정 기준을 대조했다\n")
}
pub fn outcome_report(failed: bool) -> String {
    let verdict = if failed { "failure" } else { "success" };
    format!("verdict: {verdict}\nlesson: 원인 위치와 복원 기준점을 구분해 다음 작은 질문의 근거로 남긴다\nnext_direction.action: close_cycle\nnext_direction.reason: 이 질문의 판정을 마쳤다\n")
}
/// The Experiment Cycle Report of Cycle `n`: success hands on, failure declares its revisit.
pub fn cycle_report(n: u32, outcome_ref: &str, fails_back_to: Option<&str>) -> String {
    let (verdict, direction) = match fails_back_to {
        Some(target) => ("failure", format!("next_direction.action: revisit\nnext_direction.target_cycle_ref: {target}\n")),
        None => ("success", "next_direction.action: open_child\n".to_string()),
    };
    format!("verdict: {verdict}\noutcome_ref: {outcome_ref}\nhandoff_summary: C{n}의 관측과 한계를 남겼다\n{direction}next_direction.reason: 관측에 따라 다음을 정한다\n")
}

/// Working-folder files by `/`-separated relative path, excluding the root `.gil` record folder.
pub fn world(root: &Path) -> BTreeMap<String, Vec<u8>> {
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
pub fn yaml(record: &Path) -> Value {
    serde_norway::from_slice(&record_bytes(record)).unwrap()
}
pub fn object_count(record: &Path) -> usize {
    fn visit(at: &Path) -> usize {
        fs::read_dir(at).map(|entries| entries.map(|e| e.unwrap().path()).map(|p| if p.is_dir() { visit(&p) } else { 1 }).sum()).unwrap_or(0)
    }
    visit(&record.parent().unwrap().join("artifacts"))
}
/// Reading tools project the record; they never write it.
pub fn reads_change_nothing(r: &mut Rig) {
    let before = record_bytes(&r.record);
    let (plan, context) = (r.read(), r.context());
    assert_eq!((r.read(), r.context()), (plan, context), "the projection is deterministic");
    ok(r.model("gil_status", json!({})), "status");
    assert_eq!(record_bytes(&r.record), before, "a read changed the record");
}
