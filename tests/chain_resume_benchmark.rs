//! Synthetic inputs through public MCP tools; saved records are never edited.
#[path = "support_wave1/mod.rs"]
mod wire;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use wire::*;
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(at).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(base, &e.path(), out);
            } else {
                out.insert(
                    e.path().strip_prefix(base).unwrap().into(),
                    fs::read(e.path()).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
struct Rig {
    base: PathBuf,
    root: PathBuf,
    settings: PathBuf,
    c: Option<Client>,
    scope: String,
    events: Vec<Value>,
    checks: Vec<Value>,
    calls: BTreeMap<String, usize>,
}
impl Drop for Rig {
    fn drop(&mut self) {
        drop(self.c.take());
        let _ = fs::remove_dir_all(&self.base);
    }
}
impl Rig {
    fn new() -> Self {
        let base = scratch("resume-benchmark");
        let root = base.join("project");
        fs::create_dir(&root).unwrap();
        let settings = base.join("settings");
        let mut r = Self {
            c: Some(Client::serve(&settings)),
            base,
            root,
            settings,
            scope: String::new(),
            events: vec![],
            checks: vec![],
            calls: BTreeMap::new(),
        };
        r.model("gil_start", json!({"locale":"ko"}));
        r.scope = structured(&r.model("gil_monitor_prepare", json!({})))["scope_id"]
            .as_str()
            .unwrap()
            .into();
        r
    }
    fn call(&mut self, name: &str, args: Value) -> Value {
        *self.calls.entry(name.into()).or_default() += 1;
        ok(self.c.as_mut().unwrap().call(name, args), name)
    }
    fn model(&mut self, name: &str, mut args: Value) -> Value {
        args["project_root"] = json!(self.root);
        self.call(name, args)
    }
    #[cfg(any(feature = "approved-plan", feature = "product-safety"))]
    fn app(&mut self, name: &str, mut args: Value) -> Value {
        args["scope_id"] = json!(self.scope);
        self.call(name, args)
    }
    fn open(&mut self, kind: &str) -> String {
        step_ref(
            &self.model("gil_open", json!({"kind":kind,"contract":CONTRACT})),
            kind,
        )
    }
    fn close(&mut self, report: &str) {
        self.model("gil_close", json!({"report":report}));
    }
    fn restart(&mut self) {
        let (s, n) = self.c.take().unwrap().eof();
        assert!(s.success() && n.is_empty());
        self.c = Some(Client::serve(&self.settings));
    }
    fn checkpoint(&mut self, label: &str, required: &[&str]) {
        let record = product_state_path(&self.root, &private_local_app_data(&self.settings));
        let domain = gil::load(gil::RuleSet::builtin().unwrap(), &record).unwrap();
        let saved = record_bytes(&record);
        let project = files(&self.root);
        let storage = files(record.parent().unwrap());
        let mut previous = None;
        for pass in 0..2 {
            self.restart();
            for name in ["gil_context", "gil_help"] {
                let result = self.model(name, json!({}))["result"].clone();
                let text = result["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|x| x["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if name == "gil_context" {
                    for key in required {
                        assert!(text.contains(key), "{label} missing {key}: {text}");
                    }
                    assert!(text.contains(&domain.current_cycle().id().to_ref().to_string()));
                    if let Some(id) = domain.current_cycle().steps().current() {
                        assert!(text.contains(&domain.current_cycle().step_ref(id).to_string()));
                    }
                    if let Some(plan) = domain.approved_plan() {
                        if let Some(p) = plan.current(&domain) {
                            assert!(text.contains(&p.digest));
                            assert!(text.contains(&format!("판 {}", p.revision)));
                            let state = match p.receipt.as_ref().map(|r| &r.decision) {
                                None => "Monitor 결정을 기다림",
                                Some(gil::approved_plan::Decision::Approve) => "승인 저장됨",
                                Some(gil::approved_plan::Decision::RequestChanges) => {
                                    "수정 요청 저장됨"
                                }
                            };
                            assert!(text.contains(state));
                        }
                        if let Some(b) = plan.executed_cycles.iter().find(|b| {
                            b.cycle_ref == domain.current_cycle().id().to_ref().to_string()
                        }) {
                            assert!(text.contains(&format!(
                                "실행 바인딩: 항목 {} · 판 {}",
                                b.item_id, b.revision
                            )));
                        }
                    }
                    for key in [
                        "[저장 기록으로 재개]",
                        "[지금 바로 할 호출]",
                        "gil_status",
                        "gil_story",
                    ] {
                        assert!(text.contains(key), "missing {key}");
                    }
                    if let Some(old) = &previous {
                        assert_eq!(&text, old);
                    }
                    previous = Some(text);
                }
                self.events.push(json!({"seq":self.events.len()+1,"case":label,"pass":pass,"synthetic":true,"request":{"name":name,"arguments":{"project_root":self.root}},"result":result,"error":null}));
            }
            #[cfg(feature = "approved-plan")]
            {
                let read = self.model("gil_plan_read", json!({}));
                let p = &structured(&read)["proposal"];
                if let Some(digest) = p["digest"].as_str() {
                    assert!(previous.as_ref().unwrap().contains(digest));
                }
            }
            assert_eq!(record_bytes(&record), saved);
            assert_eq!(files(&self.root), project);
            assert_eq!(files(record.parent().unwrap()), storage);
        }
        self.checks.push(json!({"case":label,"fresh_processes":2,"required_count":required.len(),"restart_equal":true,"record_and_files_unchanged":true}));
    }
    #[cfg(feature = "approved-plan")]
    fn ask(&mut self) -> (Vec<String>, String) {
        let q = self.open("question");
        self.close(QUESTION);
        let i = self.open("interpretation");
        self.close(INTERPRETATION);
        (vec![q, i], self.open("synthesis"))
    }
    #[cfg(feature = "approved-plan")]
    fn decision(&mut self, decision: &str) {
        let review = self.app("gil_plan_review", json!({}));
        let p = &structured(&review)["proposal"];
        self.app("gil_plan_decide",json!({"revision":p["revision"],"digest":p["digest"],"token":token(&review).unwrap(),"decision":decision}));
    }
    fn finish_interview(&mut self, synthesis: &str, closing: bool) {
        #[cfg(feature = "approved-plan")]
        {
            let r = self.model("gil_plan_read", json!({}));
            self.close(structured(&r)["close_report"].as_str().unwrap());
        }
        #[cfg(not(feature = "approved-plan"))]
        self.close(
            "statement: Synthetic scope\nbasis_refs: |\n  step:C1/S1\n  step:C1/S2\napproved: yes",
        );
        let o = self.open("outcome");
        self.close(&format!("verdict: success\nlesson: scope agreed\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: continue"));
        let action = if closing { "close_chain" } else { "open_child" };
        self.close(&format!("verdict: success\noutcome_ref: {o}\nhandoff_summary: synthetic agreement\nnext_direction.action: {action}\nnext_direction.reason: continue"));
    }
    fn experiment(&mut self, attempt: usize, failure: bool, long: bool) {
        self.model("gil_open", json!({"kind":"experiment"}));
        self.open("define");
        self.close("problem: 항목 A을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 항목 A 결과가 저장된다");
        self.open("hypothesis");
        let payload = if long {
            "Synthetic observation detail. ".repeat(256)
        } else {
            "Synthetic observation detail.".into()
        };
        self.close(&format!(
            "hypothesis: {payload}\nrationale: synthetic input\nguardrail: no user data"
        ));
        let v = self.open("verify");
        let mut required = vec!["열린 Verify", "step:", payload.trim()];
        if attempt > 1 {
            required.push("attempt-1-result");
        }
        if attempt > 2 {
            required.push("attempt-2-result");
        }
        self.checkpoint(&format!("attempt-{attempt}-verify"), &required);
        #[cfg(feature = "approved-plan")]
        {
            let r = self.model("gil_plan_read", json!({}));
            fs::write(
                self.root.join("plan.md"),
                structured(&r)["document"].as_str().unwrap(),
            )
            .unwrap();
        }
        fs::write(
            self.root.join("evidence.txt"),
            format!("synthetic attempt {attempt}"),
        )
        .unwrap();
        self.call("gil_close",verify_args(&self.root,&v,"execution: synthetic measurement\nresult: measured fixture\nplan_document: plan.md\nevidence_files: evidence.txt"));
        self.open("analysis");
        let yes = if failure { "no" } else { "yes" };
        self.close(&format!("hypothesis_fit: {yes}\nproblem_solved: {yes}\nsuccess_condition_met: {yes}\nguardrail_triggered: no\ninterpretation: synthetic result"));
        let o = self.open("outcome");
        let verdict = if failure { "failure" } else { "success" };
        self.close(&format!("verdict: {verdict}\nlesson: attempt-{attempt}-lesson\nnext_direction.action: close_cycle\nnext_direction.reason: continue"));
        let direction = if failure {
            "revisit\nnext_direction.target_cycle_ref: cycle:C1"
        } else {
            "open_child"
        };
        self.close(&format!("verdict: {verdict}\noutcome_ref: {o}\nhandoff_summary: attempt-{attempt}-result\nnext_direction.action: {direction}\nnext_direction.reason: continue"));
        self.checkpoint(
            &format!("attempt-{attempt}-closed"),
            &[&format!("attempt-{attempt}-result"), verdict],
        );
        if failure {
            #[cfg(feature = "product-safety")]
            {
                let r = self.app("gil_revisit_review", json!({}));
                self.app("gil_revisit_apply",json!({"digest":structured(&r)["digest"],"token":r["result"]["_meta"]["gil/revisitApprovalToken"]}));
            }
            #[cfg(not(feature = "product-safety"))]
            self.model("gil_revisit", json!({}));
        }
    }
}
#[test]
fn actual_core_resume_baseline() {
    let mut r = Rig::new();
    r.open("question");
    r.checkpoint(
        "unanswered",
        &["응답 대상 step:C1/S1", "답변은 아직 기록되지 않았다"],
    );
    r.close(QUESTION);
    let i = r.open("interpretation");
    r.close(INTERPRETATION);
    #[allow(unused_mut)]
    let mut s = r.open("synthesis");
    assert!(s.starts_with("step:"));
    #[cfg(feature = "approved-plan")]
    {
        r.model(
            "gil_plan_propose",
            json!({"draft":draft(&["A"], &["step:C1/S1".into(),i.clone()], &[])}),
        );
        r.checkpoint("waiting", &["제안 판 1", "Monitor 결정을 기다림"]);
        r.decision("request_changes");
        r.checkpoint(
            "changes",
            &["제안 판 1", "수정 요청 저장됨", "approved: no"],
        );
        let read = r.model("gil_plan_read", json!({}));
        r.close(structured(&read)["close_report"].as_str().unwrap());
        let (basis, syn) = r.ask();
        s = syn;
        r.model(
            "gil_plan_propose",
            json!({"draft":draft(&["A"],&basis,&[])}),
        );
        r.decision("approve");
        r.checkpoint(
            "approved-reconnect",
            &["제안 판 2", "승인 저장됨", "close_report"],
        );
    }
    let _ = &i;
    r.finish_interview(&s, false);
    r.experiment(1, true, false);
    r.experiment(2, true, false);
    r.experiment(3, false, true);
    r.checkpoint(
        "accumulated-failures",
        &["attempt-1-result", "attempt-2-result", "attempt-3-result"],
    );
    #[cfg(feature = "approved-plan")]
    {
        r.model("gil_open", json!({"kind":"interview"}));
        let (basis, s) = r.ask();
        r.model(
            "gil_plan_propose",
            json!({"draft":draft(&["A"],&basis,&[("chain_verdict","success")])}),
        );
        r.decision("approve");
        r.finish_interview(&s, true);
        r.checkpoint("chain-report", &["Chain Report", "closing_synthesis_ref"]);
        r.close(&format!(
            "verdict: success\nclosing_synthesis_ref: {s}\nhandoff_summary: synthetic-chain-handoff"
        ));
        r.checkpoint(
            "chain-closed",
            &[
                "마지막 닫힌 결과: chain:C1",
                "synthetic-chain-handoff",
                "gil_open(kind: chain)",
            ],
        );
        r.model("gil_open", json!({"kind":"chain"}));
        r.checkpoint("next-chain", &["chain:C2", "synthetic-chain-handoff"]);
    }
    if let Ok(dir) = std::env::var("GIL_RESUME_BENCH_OUTPUT") {
        let dir = Path::new(&dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("events.jsonl"),
            r.events
                .iter()
                .map(|v| format!("{v}\n"))
                .collect::<String>(),
        )
        .unwrap();
        fs::write(dir.join("capture.json"),serde_json::to_vec_pretty(&json!({"case_version":1,"locale":"ko","checks":r.checks,"tool_calls":r.calls,"binary_sha256":format!("{:x}",Sha256::digest(fs::read(gil()).unwrap())),"root":r.root,"scope":r.scope})).unwrap()).unwrap();
    }
}
