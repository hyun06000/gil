//! M4-R approval modes of closed-failure recovery over the real `gil mcp --serve` stdio process.
//!
//! Opt-in build only (`recovery-auto-approval`): the Core reads a per-Project setting beside the
//! record and may stand in for the Monitor click. Checked here: manual stays the default and the
//! explicit manual setting keeps the click requirement; damaged, forged or linked settings are
//! manual and never repaired; a valid automatic setting completes the recovery without any App
//! token, records the setting edition as the approval source, passes the same refusals as the
//! manual path (protected paths, unrecorded edits, corrupt evidence), does not start on reads or
//! on `gil_open`, is idempotent on duplicate requests, and a released setting blocks new automatic
//! approvals while a started recovery resumes from its stored record. Every file here is a
//! synthetic temporary fixture; the setting is written directly by the test because the product
//! has no write path yet — that path is an open decision, not something this file assumes.
#![cfg(all(
    feature = "recovery-auto-approval",
    feature = "approved-plan",
    feature = "visual-authoring",
    any(target_os = "macos", windows)
))]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use support::*;

const SETTING_FILE: &str = "recovery-approval.json";

struct Rig {
    root: PathBuf,
    record: PathBuf,
    scope: String,
    c: Client,
}

impl Rig {
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
    /// The Monitor's canonical view; `recovery_approval` is the mode shown to the user.
    fn monitor_mode(&mut self) -> Value {
        structured(&ok(self.app("gil_monitor_read", json!({})), "monitor read"))["recovery_approval"].clone()
    }
    fn setting_path(&self) -> PathBuf {
        self.record.parent().unwrap().join(SETTING_FILE)
    }
    fn archive_dir(&self) -> PathBuf {
        self.record.parent().unwrap().join("revisit-preserved")
    }
    fn set_setting(&self, value: Value) -> Vec<u8> {
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(self.setting_path(), &bytes).unwrap();
        bytes
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
    fn approve_via_app(&mut self, revision: u64) {
        let review = ok(self.app("gil_plan_review", json!({})), "plan review");
        let t = token(&review).expect("plan capability in _meta");
        let p = &structured(&review)["proposal"];
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":"approve"})), "approve plan");
    }
    fn close_approved_interview(&mut self, synthesis: &str) {
        let read = self.read();
        ok(self.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
        let outcome = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome_ref = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 Monitor 응답으로 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
    }
    fn open_experiment(&mut self, item: &str) -> (u32, String) {
        let opened = ok(self.model("gil_open", json!({"kind":"experiment"})), "open experiment");
        let n = cycle_number(&opened);
        let read = self.read();
        assert_eq!(read["current_item"]["id"], item, "{read}");
        (n, read["document"].as_str().expect("document after binding").to_owned())
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
        fs::create_dir_all(self.root.join("plan")).unwrap();
        fs::create_dir_all(self.root.join("evidence")).unwrap();
        fs::write(self.root.join("plan/approved-plan.md"), document).unwrap();
        let evidence = format!("evidence/C{n}-{item}.md");
        fs::write(self.root.join(&evidence), format!("# 합성 관측 C{n}\n\n항목 {item} 결과 (시험 전용)\n")).unwrap();
        ok(self.c.call("gil_close", verify_args(&self.root, verify, &format!("execution: 합성 자료를 확인하고 결과를 저장했다\nresult: 항목 {item} 결과 파일을 남겼다\nplan_document: plan/approved-plan.md\nevidence_files: {evidence}\n"))), "close verify");
        ok(self.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
        ok(self.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n"})), "close analysis");
        let o = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "experiment outcome");
        let outcome_ref = step_ref(&o, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 항목 {item}을 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n")})), "close experiment outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: synthetic result\nnext_direction.action: open_child\nnext_direction.reason: 다음 항목으로 간다\n")})), "close experiment cycle");
    }
}

/// A Project whose first Experiment closed as failure → revisit C1. Mirrors the manual-path fixture.
fn failed(label: &str, protect: Option<&str>) -> (Rig, PathBuf) {
    let root = scratch(label);
    let settings = scratch(&format!("{label}-settings"));
    fs::write(root.join("report.txt"), "baseline report").unwrap();
    if let Some(path) = protect {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), "original protected content").unwrap();
    }
    let c = Client::serve(&settings);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig { root, record, scope: String::new(), c };
    ok(r.model("gil_start", json!({})), "start");
    let prepared = ok(r.model("gil_monitor_prepare", json!({})), "prepare");
    r.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
    let (basis, synthesis) = r.ask_and_interpret();
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B"], &basis, &[])})), "propose");
    r.approve_via_app(1);
    r.close_approved_interview(&synthesis);
    fail_attempt(&mut r, protect);
    (r, settings)
}

fn fail_attempt(r: &mut Rig, protect: Option<&str>) {
    let (n, document) = r.open_experiment("A");
    let verify = r.walk_to_verify("A");
    fs::write(r.root.join("plan.md"), document).unwrap();
    fs::write(r.root.join("evidence.md"), format!("Attempt {n}: incomplete daily source data; cannot calculate volatility.")).unwrap();
    fs::write(r.root.join("report.txt"), format!("partial attempt {n}")).unwrap();
    if let Some(path) = protect {
        fs::write(r.root.join(path), "changed protected content").unwrap();
    }
    ok(r.c.call("gil_close", verify_args(&r.root, &verify, "execution: 合成資料を調査\nresult: source incomplete\nevidence_files: evidence.md\nplan_document: plan.md\n")), "verify");
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    ok(r.model("gil_close", json!({"report":"hypothesis_fit: no\nproblem_solved: no\nsuccess_condition_met: no\nguardrail_triggered: no\ninterpretation: need complete data\n"})), "close analysis");
    let o = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome = step_ref(&o, "outcome");
    ok(r.model("gil_close", json!({"report":"verdict: failure\nlesson: verify source completeness before calculating\nnext_direction.action: close_cycle\nnext_direction.reason: retry with full source\n"})), "failure outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\noutcome_ref: {outcome}\nhandoff_summary: incomplete source preserved\nnext_direction.action: revisit\nnext_direction.target_cycle_ref: cycle:C1\nnext_direction.reason: retry with complete data\n")})), "failure cycle");
}

/// Every regular file below `at`, except the GIL record folder (`.gil`) when it lives inline.
fn tree(at: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if dir == base && p.file_name().is_some_and(|n| n == ".gil") { continue; }
            if p.is_dir() { visit(base, &p, out); } else { out.insert(p.strip_prefix(base).unwrap().to_owned(), fs::read(&p).unwrap()); }
        }
    }
    let mut out = Default::default();
    visit(at, at, &mut out);
    out
}

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn automatic(revision: u32) -> Value { json!({"schema_version":1,"mode":"automatic","revision":revision}) }
fn manual(revision: u32) -> Value { json!({"schema_version":1,"mode":"manual","revision":revision}) }

/// Nothing a user would notice changed: files in the working tree, the record, no preservation area.
#[track_caller]
fn untouched(r: &Rig, record: &[u8], files: &std::collections::BTreeMap<PathBuf, Vec<u8>>) {
    assert_eq!(record, record_bytes(&r.record), "record changed");
    assert_eq!(*files, tree(&r.root), "working files changed");
    assert!(!r.archive_dir().exists(), "a preservation area was created");
}

/// `gil_revisit` answers the model in text; the completed plan is embedded as compact JSON.
#[track_caller]
fn completed_with(frame: &Value, source: &str) {
    let text = said(frame);
    assert!(text.contains("\"state\":\"completed\""), "{text}");
    assert!(text.contains(&format!("\"approval_source\":\"{source}\"")), "{text}");
    assert!(!frame.to_string().contains("revisitApprovalToken"), "a token leaked into model content: {frame}");
}

fn source_record(r: &Rig, view: &Value) -> Value {
    let archive = r.root.join(view["archive"].as_str().unwrap());
    serde_json::from_slice(&fs::read(archive.join("source.json")).unwrap()).unwrap()
}

#[test]
fn manual_is_the_default_and_an_explicit_manual_setting_keeps_the_click_requirement() {
    let (mut r, _settings) = failed("mode-manual", None);
    let record = record_bytes(&r.record);
    let files = tree(&r.root);
    assert_eq!(r.monitor_mode(), json!({"mode":"manual","basis":"default"}));
    refused(r.model("gil_revisit", json!({})), "no setting, no click");
    untouched(&r, &record, &files);

    let bytes = r.set_setting(manual(1));
    assert_eq!(r.monitor_mode(), json!({"mode":"manual","basis":"setting","revision":1}));
    refused(r.model("gil_revisit", json!({})), "explicit manual, no click");
    untouched(&r, &record, &files);
    assert_eq!(fs::read(r.setting_path()).unwrap(), bytes, "reading rewrote the setting");

    // The Monitor path is unchanged and now records its source.
    let review = ok(r.app("gil_revisit_review", json!({})), "review");
    let t = review["result"]["_meta"]["gil/revisitApprovalToken"].as_str().unwrap().to_owned();
    let v = structured(&review).clone();
    assert_eq!(v["state"], "awaiting_approval");
    assert_eq!(v["approval_mode"], json!({"mode":"manual","basis":"setting","revision":1}));
    untouched(&r, &record, &files);
    let done = ok(r.app("gil_revisit_apply", json!({"digest":v["digest"],"token":t})), "button");
    assert_eq!(structured(&done)["state"], "completed");
    assert_eq!(structured(&done)["approval_source"], "monitor_button");
    assert_eq!(fs::read_to_string(r.root.join("report.txt")).unwrap(), "baseline report");
    let source = source_record(&r, &v);
    assert_eq!(source["approval_source"], "monitor_button");
    assert_eq!(source["human_click"], true);
    assert_eq!(source["pending_state_sha256"], v["pending_state_sha256"]);
    assert_eq!(source["digest"], v["digest"]);
    assert!(source.get("setting").is_none(), "a manual click must not name a setting: {source}");
    // approval.json keeps the exact pre-existing shape: the mode is not inside the stored plan.
    let approval: Value = serde_json::from_slice(&fs::read(r.root.join(v["archive"].as_str().unwrap()).join("approval.json")).unwrap()).unwrap();
    assert!(approval.get("approval_mode").is_none(), "{approval}");
    assert_eq!(approval["state"], "approved");
}

#[test]
fn damaged_forged_or_linked_settings_are_manual_and_never_repaired() {
    let (mut r, _settings) = failed("mode-damaged", None);
    let record = record_bytes(&r.record);
    let files = tree(&r.root);
    let cases: Vec<Vec<u8>> = vec![
        serde_json::to_vec(&json!({"schema_version":2,"mode":"automatic","revision":1})).unwrap(),
        serde_json::to_vec(&json!({"schema_version":1,"mode":"auto","revision":1})).unwrap(),
        serde_json::to_vec(&json!({"schema_version":1,"mode":"automatic","revision":0})).unwrap(),
        serde_json::to_vec(&json!({"schema_version":1,"mode":"automatic","revision":1,"approved":"yes"})).unwrap(),
        serde_json::to_vec(&json!({"mode":"automatic"})).unwrap(),
        b"{".to_vec(),
        vec![b' '; 4097],
    ];
    for bad in cases {
        fs::write(r.setting_path(), &bad).unwrap();
        let mode = r.monitor_mode();
        assert_eq!(mode["mode"], "manual", "{}", String::from_utf8_lossy(&bad[..bad.len().min(80)]));
        assert_eq!(mode["basis"], "unreadable");
        refused(r.model("gil_revisit", json!({})), "damaged setting is not automatic");
        untouched(&r, &record, &files);
        assert_eq!(fs::read(r.setting_path()).unwrap(), bad, "the Core repaired or rewrote a damaged setting");
    }
    // A valid automatic setting reached only through a link is not this Project's setting.
    fs::remove_file(r.setting_path()).unwrap();
    let elsewhere = r.root.join("elsewhere-recovery-approval.json");
    fs::write(&elsewhere, serde_json::to_vec(&automatic(1)).unwrap()).unwrap();
    let files = tree(&r.root);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, r.setting_path()).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&elsewhere, r.setting_path()).unwrap();
    let mode = r.monitor_mode();
    assert_eq!(mode["mode"], "manual");
    assert_eq!(mode["basis"], "unreadable");
    refused(r.model("gil_revisit", json!({})), "linked setting");
    untouched(&r, &record, &files);
    assert!(fs::symlink_metadata(r.setting_path()).unwrap().file_type().is_symlink());
}

#[test]
fn a_valid_automatic_setting_recovers_without_a_click_and_binds_the_setting_edition() {
    let (mut r, settings) = failed("mode-automatic", None);
    let record = record_bytes(&r.record);
    let files = tree(&r.root);
    let objects = tree(&r.record.parent().unwrap().join("artifacts"));
    let bytes = r.set_setting(automatic(2));
    assert_eq!(r.monitor_mode(), json!({"mode":"automatic","basis":"setting","revision":2}));

    // Reads and the Monitor's own review show the mode but start nothing.
    let context = r.context();
    assert!(context.contains("gil_revisit") || context.contains("Monitor"), "{context}");
    let review = ok(r.app("gil_revisit_review", json!({})), "review in automatic mode");
    let v = structured(&review).clone();
    assert_eq!(v["state"], "awaiting_approval");
    assert_eq!(v["approval_mode"]["mode"], "automatic");
    untouched(&r, &record, &files);
    // The setting is not a licence to skip the recovery: the next Experiment stays gated.
    refused(r.model("gil_open", json!({"kind":"experiment"})), "automatic mode does not bypass recovery");
    untouched(&r, &record, &files);
    assert_eq!(fs::read(r.setting_path()).unwrap(), bytes);

    let done = ok(r.model("gil_revisit", json!({})), "automatic approval");
    completed_with(&done, "automatic_setting");
    assert_eq!(fs::read_to_string(r.root.join("report.txt")).unwrap(), "baseline report");
    assert!(!r.root.join("evidence.md").exists());
    let archive = r.root.join(v["archive"].as_str().unwrap());
    for name in ["approval.json", "files.json", "source.json"] {
        assert!(archive.join(name).is_file(), "{name} missing");
    }
    let source = source_record(&r, &v);
    assert_eq!(source["approval_source"], "automatic_setting");
    assert_eq!(source["human_click"], false);
    assert_eq!(source["setting"]["file"], SETTING_FILE);
    assert_eq!(source["setting"]["revision"], 2);
    assert_eq!(source["setting"]["sha256"], sha256(&bytes), "the approval must bind the exact setting bytes");
    assert_eq!(source["pending_state_sha256"], v["pending_state_sha256"]);
    assert_eq!(source["digest"], v["digest"]);
    assert_eq!(source["from"], v["from"]);
    assert_eq!(source["target"], v["target"]);
    let mappings: Value = serde_json::from_slice(&fs::read(archive.join("files.json")).unwrap()).unwrap();
    let evidence = mappings.as_array().unwrap().iter().find(|x| x["path"] == "evidence.md").unwrap();
    assert!(fs::read_to_string(archive.join(evidence["saved_as"].as_str().unwrap())).unwrap().contains("incomplete daily"));

    // Preservation identical to the manual path: failure graph, snapshots, plan and self survive.
    assert_eq!(objects, tree(&r.record.parent().unwrap().join("artifacts")));
    let old: Value = serde_norway::from_slice(&record).unwrap();
    let new: Value = serde_norway::from_slice(&record_bytes(&r.record)).unwrap();
    for field in ["format", "next_will_id", "current_existence_ref", "existences", "existence_states", "artifacts", "visuals"] {
        assert_eq!(old[field], new[field], "automatic recovery changed preserved {field}");
    }
    assert_eq!(old["chains"]["nodes"][0]["approved_plan"], new["chains"]["nodes"][0]["approved_plan"]);
    assert_eq!(old["chains"]["nodes"][0]["cycles"]["nodes"], new["chains"]["nodes"][0]["cycles"]["nodes"]);

    // A duplicate request converges without a second approval or a second preservation area.
    let again = ok(r.model("gil_revisit", json!({})), "duplicate request");
    completed_with(&again, "automatic_setting");
    assert_eq!(fs::read_dir(r.archive_dir()).unwrap().count(), 1);
    assert_eq!(source_record(&r, &v), source);
    let context = r.context();
    let next = context.split("[지금 바로 할 호출]\n").nth(1).unwrap().lines().next().unwrap();
    assert!(next.contains("되돌아왔다") && next.contains("gil_open"), "{next}");

    // The next attempt fails again; the same setting approves the second recovery on its own.
    r.c = Client::serve(&settings);
    assert_eq!(r.monitor_mode()["mode"], "automatic", "a valid setting survives a new session");
    fail_attempt(&mut r, None);
    let second = ok(r.model("gil_revisit", json!({})), "second automatic approval");
    completed_with(&second, "automatic_setting");
    assert_eq!(fs::read_dir(r.archive_dir()).unwrap().count(), 2);
    let newer = fs::read_dir(r.archive_dir()).unwrap().flatten().map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_str().unwrap() != v["pending_state_sha256"].as_str().unwrap()).unwrap();
    let second_source: Value = serde_json::from_slice(&fs::read(newer.join("source.json")).unwrap()).unwrap();
    assert_eq!(second_source["approval_source"], "automatic_setting");
    assert_eq!(second_source["setting"]["sha256"], sha256(&bytes));
    assert_ne!(second_source["pending_state_sha256"], v["pending_state_sha256"]);
    let (n, doc) = r.open_experiment("A");
    let verify = r.walk_to_verify("A");
    r.finish_experiment(n, "A", &verify, &doc);
    let read = r.read();
    assert_eq!(read["executed_cycles"].as_array().unwrap().len(), 3);
    assert_eq!(read["pending"][0]["id"], "B");
}

#[test]
fn automatic_mode_keeps_every_refusal_of_the_manual_path() {
    for (i, path) in [".env", ".git/HEAD", "node_modules/cache.js"].iter().enumerate() {
        let (mut r, _settings) = failed(&format!("mode-protected-{i}"), Some(path));
        r.set_setting(automatic(1));
        let record = record_bytes(&r.record);
        let files = tree(&r.root);
        let why = refused(r.model("gil_revisit", json!({})), "protected difference");
        assert!(said(&why).contains("보호 대상") || said(&why).contains("복원"), "{why}");
        untouched(&r, &record, &files);
        assert_eq!(fs::read_to_string(r.root.join(path)).unwrap(), "changed protected content");
    }

    let (mut r, _settings) = failed("mode-dirty-and-corrupt", None);
    r.set_setting(automatic(1));
    let record = record_bytes(&r.record);
    fs::write(r.root.join("unrecorded.txt"), "user edit").unwrap();
    let files = tree(&r.root);
    refused(r.model("gil_revisit", json!({})), "unrecorded change");
    untouched(&r, &record, &files);
    fs::remove_file(r.root.join("unrecorded.txt")).unwrap();

    // Immutable failed evidence must be readable before anything is approved automatically.
    let blob = fs::read_dir(r.record.parent().unwrap().join("artifacts/blobs/sha256")).unwrap().next().unwrap().unwrap().path();
    let file = fs::read_dir(blob).unwrap().next().unwrap().unwrap().path();
    let bytes = fs::read(&file).unwrap();
    fs::write(&file, b"corrupt").unwrap();
    let files = tree(&r.root);
    refused(r.model("gil_revisit", json!({})), "corrupt evidence");
    untouched(&r, &record, &files);
    fs::write(&file, bytes).unwrap();

    // Once every check passes, the same request completes.
    let done = ok(r.model("gil_revisit", json!({})), "clean automatic approval");
    completed_with(&done, "automatic_setting");
    assert_eq!(fs::read_to_string(r.root.join("report.txt")).unwrap(), "baseline report");
}

#[test]
fn releasing_the_setting_blocks_new_approvals_but_a_started_recovery_resumes_from_its_record() {
    let (mut r, settings) = failed("mode-release", None);
    let first_setting = r.set_setting(automatic(1));
    r.c = Client::serve_with_env(&settings, &[("GIL_REVISIT_FAIL", "recovery-after-state-save")]);
    refused(r.model("gil_revisit", json!({})), "injected failure after the durable approval");
    assert_eq!(fs::read_to_string(r.root.join("report.txt")).unwrap(), "partial attempt 2");
    let archive = fs::read_dir(r.archive_dir()).unwrap().next().unwrap().unwrap().path();
    let stored: Value = serde_json::from_slice(&fs::read(archive.join("source.json")).unwrap()).unwrap();
    assert_eq!(stored["approval_source"], "automatic_setting");
    assert_eq!(stored["setting"]["sha256"], sha256(&first_setting));
    let source_bytes = fs::read(archive.join("source.json")).unwrap();
    let approval_bytes = fs::read(archive.join("approval.json")).unwrap();

    // The user releases automatic mode. The started recovery still finishes from its own record.
    r.set_setting(manual(2));
    r.c = Client::serve(&settings);
    assert_eq!(r.monitor_mode(), json!({"mode":"manual","basis":"setting","revision":2}));
    let resumed = ok(r.model("gil_revisit", json!({})), "resume the stored approval");
    completed_with(&resumed, "automatic_setting");
    assert_eq!(fs::read_to_string(r.root.join("report.txt")).unwrap(), "baseline report");
    assert_eq!(fs::read(archive.join("source.json")).unwrap(), source_bytes, "resumption rewrote the approval source");
    assert_eq!(fs::read(archive.join("approval.json")).unwrap(), approval_bytes, "resumption rewrote the approval");
    assert_eq!(fs::read_dir(r.archive_dir()).unwrap().count(), 1);

    // A new failure under the released setting needs the Monitor again.
    fail_attempt(&mut r, None);
    let record = record_bytes(&r.record);
    let files = tree(&r.root);
    refused(r.model("gil_revisit", json!({})), "released setting approves nothing new");
    assert_eq!(record, record_bytes(&r.record));
    assert_eq!(files, tree(&r.root));
    assert_eq!(fs::read_dir(r.archive_dir()).unwrap().count(), 1);
    // Removing the file entirely is the same release.
    fs::remove_file(r.setting_path()).unwrap();
    assert_eq!(r.monitor_mode(), json!({"mode":"manual","basis":"default"}));
    refused(r.model("gil_revisit", json!({})), "no setting approves nothing new");
    assert_eq!(fs::read_dir(r.archive_dir()).unwrap().count(), 1);
}
