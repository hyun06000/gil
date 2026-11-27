//! Chain recovery preservation boundary (M4-R, `legacy-whole-tree-protected-difference-v1`).
//!
//! The existing `tests/support_wave1/revisit_cases.rs` drives the approved recovery of a failed
//! Cycle whose target is the root Interview Cycle `C1`. This file covers the *Chain in progress*
//! shape that those cases do not: a successful Experiment `C2` has already confirmed a world,
//! the next Experiment `C3` fails on top of it and declares `revisit → cycle:C2`. Against that
//! fixture it fixes, over the real product-profile `gil mcp --serve` process:
//!
//! - partial results written in `C3` over `C2` files are replaced by the `C2` bytes;
//! - files that exist only in the failed world (root and nested) are removed from the working
//!   folder, preserved under `.gil/revisit-preserved/<key>/`, and their now-empty folders go;
//! - a file deleted during `C3` is created again from the `C2` world;
//! - excluded (`.gitignore`) and default-protected paths that did not change are listed in no
//!   change and are not rewritten; a difference in such a path refuses the whole recovery with no
//!   state, file or archive change — whether the exclusion comes from the before or the target
//!   `.gitignore`;
//! - the archive maps every before-world file with its SHA-256 and never contains the App token;
//! - `.gil` records, Snapshot objects, Will/Existence, the approved plan and the failed Cycle's
//!   Report survive; the next `gil_open` branches from `C2` with `revisit_from = C3`;
//! - after completion, a restart plus `gil_revisit` is a no-op that mints no second archive, and
//!   a file written after completion is refused, not swept.
//!
//! Only synthetic temporary folders are touched. App decisions here are harness inputs, not a
//! person's click. Nothing here exercises the unapproved `.gilignore` design (Artifact Model
//! §3.6, PR #128) or lifts the general `restore` / Step-revisit refusal.
#![cfg(all(
    feature = "approved-plan",
    feature = "product-safety",
    feature = "visual-authoring",
    any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)
))]
#[path = "support_wave1/mod.rs"]
mod support;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use support::*;

/// A minimal copy of the Wave 1 rig; the original lives inside another test binary and the shared
/// support module is read-only for this file.
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
        let t = token(&review).expect("capability in _meta");
        let p = &structured(&review)["proposal"];
        assert_eq!(p["revision"], json!(revision), "{review}");
        ok(self.app("gil_plan_decide", json!({"revision":revision,"digest":p["digest"],"token":t,"decision":"approve"})), "approve");
    }
    fn close_approved_interview(&mut self, synthesis: &str) {
        let read = self.read();
        ok(self.model("gil_close", json!({"report":read["close_report"]})), "close synthesis");
        let outcome = ok(self.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "open outcome");
        let outcome_ref = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 범위를 실험으로 확인한다\n")})), "close outcome");
        ok(self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 합성 범위를 Monitor 응답으로 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview cycle");
    }
    /// Opens an Experiment, walks Define/Hypothesis and leaves Verify open; returns (C-number, Verify ref, plan document).
    fn experiment_to_verify(&mut self, item: &str) -> (u32, String, String) {
        let opened = ok(self.model("gil_open", json!({"kind":"experiment"})), "open experiment");
        let n = cycle_number(&opened);
        let read = self.read();
        assert_eq!(read["current_item"]["id"], item, "{read}");
        let document = read["document"].as_str().expect("document after binding").to_owned();
        ok(self.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
        ok(self.model("gil_close", json!({"report":format!("problem: 항목 {item}을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 결과가 저장된다\n")})), "close define");
        ok(self.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
        ok(self.model("gil_close", json!({"report":format!("hypothesis: 합성 자료만으로 항목 {item}을 확인할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n")})), "close hypothesis");
        let v = ok(self.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify");
        (n, step_ref(&v, "verify"), document)
    }
    fn close_verify(&mut self, verify: &str, n: u32, item: &str, result: &str) {
        let evidence = format!("evidence/C{n}-{item}.md");
        ok(self.c.call("gil_close", verify_args(&self.root, verify, &format!("execution: 합성 자료를 확인했다\nresult: {result}\nplan_document: plan/approved-plan.md\nevidence_files: {evidence}\n"))), "close verify");
    }
    fn write(&self, path: &str, bytes: &str) {
        let at = self.root.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
    }
    fn read_file(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
    fn restart(&mut self, settings: &Path) {
        let old = std::mem::replace(&mut self.c, Client::serve(settings));
        let (status, noise) = old.eof();
        assert!(status.success() && noise.is_empty(), "old Core exit {status:?} noise {noise:?}");
    }
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
fn files_under(at: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(base, &p, out);
            } else {
                out.insert(p.strip_prefix(base).unwrap().to_owned(), fs::read(p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    if at.is_dir() {
        visit(at, at, &mut out);
    }
    out
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn yaml(record: &Path) -> Value {
    serde_norway::from_slice(&record_bytes(record)).unwrap()
}
fn cycles_of(record: &Path) -> Value {
    yaml(record)["chains"]["nodes"][0]["cycles"].clone()
}
fn preserved_dir(r: &Rig) -> PathBuf {
    r.record.parent().unwrap().join("revisit-preserved")
}
fn changes(view: &Value) -> BTreeMap<String, String> {
    view["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["path"].as_str().unwrap().to_owned(), c["action"].as_str().unwrap().to_owned()))
        .collect()
}
/// The plan a declared revisit would carry out, as the App reads it. Nothing approves it; the
/// second value is kept only so older call sites read unchanged.
fn review(r: &mut Rig) -> (Value, String) {
    let response = ok(r.app("gil_revisit_review", json!({})), "recovery review");
    assert!(response["result"]["_meta"].is_null(), "no capability exists any more: {response}");
    (structured(&response).clone(), String::new())
}
/// The agent goes back itself with `gil_revisit`.
fn apply(r: &mut Rig, _view: &Value, _unused: &str) -> Value {
    let root = r.root.clone();
    revisit(&mut r.c, &root)
}

/// How the failing Experiment `C3` edits the world before its Verify closes.
#[derive(Clone, Copy)]
enum Failure {
    /// Partial results, current-only files and one deletion; excluded and protected paths untouched.
    Ordinary,
    /// Also creates a file under the `.gitignore`-excluded folder that the baseline already ignores.
    CreatesExcludedFile,
    /// Also widens `.gitignore` itself so the failed world excludes a folder the target world tracks.
    WidensIgnore,
}

struct Chain {
    r: Rig,
    settings: PathBuf,
    /// Working-folder bytes right after `C2` closed — the world recovery must return to.
    world_at_c2: BTreeMap<String, Vec<u8>>,
    /// Working-folder bytes right after `C3` closed as a failure — the world recovery starts from.
    world_at_c3: BTreeMap<String, Vec<u8>>,
}

/// `C1` Interview (plan A/B/C approved) → `C2` Experiment A success → `C3` Experiment B failure
/// declaring `revisit → cycle:C2`. Baseline files include an ignored folder and a `.env`.
fn chain(label: &str, failure: Failure) -> Chain {
    let root = scratch(label);
    let settings = scratch(&format!("{label}-settings"));
    fs::write(root.join("README.md"), "baseline readme\n").unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(root.join("data/source.csv"), "id,value\n1,10\n").unwrap();
    fs::write(root.join(".gitignore"), "scratch/\n").unwrap();
    fs::create_dir_all(root.join("scratch")).unwrap();
    fs::write(root.join("scratch/cache.bin"), "ignored cache bytes").unwrap();
    fs::write(root.join(".env"), "SECRET=baseline").unwrap();
    let c = Client::serve(&settings);
    let record = product_state_path(&root, &private_local_app_data(&settings));
    let mut r = Rig { root, record, scope: String::new(), c };
    ok(r.model("gil_start", direct_start()), "start");
    as_item_queue_record(&r.record);
    let prepared = ok(r.model("gil_monitor_prepare", json!({})), "prepare");
    r.scope = structured(&prepared)["scope_id"].as_str().unwrap().into();
    let (basis, synthesis) = r.ask_and_interpret();
    ok(r.model("gil_plan_propose", json!({"draft":draft(&["A","B","C"], &basis, &[])})), "propose");
    r.approve_via_app(1);
    r.close_approved_interview(&synthesis);

    // C2: Experiment A succeeds and confirms a world with its own results.
    let (c2, verify, document) = r.experiment_to_verify("A");
    assert_eq!(c2, 2);
    r.write("plan/approved-plan.md", &document);
    r.write("evidence/C2-A.md", "# 합성 관측 C2\n\n항목 A 결과 (시험 전용)\n");
    r.write("results/a.txt", "A result confirmed in C2\n");
    r.close_verify(&verify, c2, "A", "항목 A 결과 파일을 남겼다");
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    ok(r.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 합성 결과가 가설과 일치했다\n"})), "close analysis");
    let o = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome_ref = step_ref(&o, "outcome");
    ok(r.model("gil_close", json!({"report":"verdict: success\nlesson: 항목 A를 파일 증거로 확인했다\nnext_direction.action: close_cycle\nnext_direction.reason: 다음 항목으로 간다\n"})), "close outcome");
    ok(r.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome_ref}\nhandoff_summary: 항목 A 완료\nnext_direction.action: open_child\nnext_direction.reason: 남은 항목을 이어간다\n")})), "close C2");
    let world_at_c2 = world(&r.root);

    // C3: Experiment B fails after partial writes on top of the C2 world.
    let (c3, verify, _) = r.experiment_to_verify("B");
    assert_eq!(c3, 3);
    r.write("results/a.txt", "B overwrote A's result halfway\n");
    r.write("README.md", "baseline readme\n\nB: partial notes\n");
    r.write("results/b/partial.csv", "id,value\n1,\n");
    r.write("evidence/C3-B.md", "Attempt C3: source incomplete; cannot finish B.\n");
    fs::remove_file(r.root.join("data/source.csv")).unwrap();
    match failure {
        Failure::Ordinary => {}
        Failure::CreatesExcludedFile => r.write("scratch/b-cache.bin", "cache written by the failed attempt"),
        Failure::WidensIgnore => r.write(".gitignore", "scratch/\nresults/\n"),
    }
    r.close_verify(&verify, c3, "B", "source incomplete");
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    ok(r.model("gil_close", json!({"report":"hypothesis_fit: no\nproblem_solved: no\nsuccess_condition_met: no\nguardrail_triggered: no\ninterpretation: need complete data\n"})), "close analysis");
    let o = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome_ref = step_ref(&o, "outcome");
    ok(r.model("gil_close", json!({"report":"verdict: failure\nlesson: verify source completeness before calculating\nnext_direction.action: close_cycle\nnext_direction.reason: retry with full source\n"})), "failure outcome");
    let closed = ok(r.model("gil_close", json!({"report":format!("verdict: failure\noutcome_ref: {outcome_ref}\nhandoff_summary: incomplete source preserved\nnext_direction.action: revisit\nnext_direction.target_cycle_ref: cycle:C2\nnext_direction.reason: retry on top of A\n")})), "failure C3 → revisit C2");
    assert!(said(&closed).contains("cycle:C3 · experiment · failure"), "{closed}");
    let world_at_c3 = world(&r.root);
    assert_ne!(world_at_c2, world_at_c3);
    Chain { r, settings, world_at_c2, world_at_c3 }
}

#[test]
fn recovery_in_a_chain_returns_to_the_successful_cycle_and_preserves_every_failed_file() {
    let Chain { mut r, settings, world_at_c2, world_at_c3 } = chain("chain-recovery-preserve", Failure::Ordinary);
    let record_before = record_bytes(&r.record);
    let cycles_before = cycles_of(&r.record);
    let objects_before = files_under(&r.record.parent().unwrap().join("artifacts"));
    let untouched = |r: &Rig| -> Vec<std::time::SystemTime> {
        [".env", "scratch/cache.bin", ".gitignore", "evidence/C2-A.md", "plan/approved-plan.md"]
            .iter()
            .map(|p| fs::metadata(r.root.join(p)).unwrap().modified().unwrap())
            .collect()
    };
    let stamps_before = untouched(&r);

    // The model cannot execute or bypass; review is read-only and lists exactly the differences.
    refused(r.model("gil_restore", json!({})), "general restore stays blocked");
    refused(r.model("gil_open", json!({"kind":"experiment"})), "no new branch before recovery");
    let (view, token) = review(&mut r);
    assert_eq!(record_bytes(&r.record), record_before, "review changed the record");
    assert_eq!(world(&r.root), world_at_c3, "review changed working files");
    assert!(!preserved_dir(&r).exists(), "review created an archive");
    assert_eq!(view["state"], "ready");
    assert_eq!(view["from"], "cycle:C3");
    assert_eq!(view["target"], "cycle:C2");
    assert_eq!(view["policy"], "legacy-whole-tree-protected-difference-v1");
    assert_ne!(view["before_world"], view["target_world"]);
    let listed = changes(&view);
    let expected: BTreeMap<String, String> = [
        ("README.md", "replace"),
        ("results/a.txt", "replace"),
        ("results/b/partial.csv", "preserve"),
        ("evidence/C3-B.md", "preserve"),
        ("data/source.csv", "create"),
    ]
    .into_iter()
    .map(|(p, a)| (p.to_owned(), a.to_owned()))
    .collect();
    assert_eq!(listed, expected, "{view}");
    for quiet in [".env", "scratch/cache.bin", ".gitignore", "evidence/C2-A.md", "plan/approved-plan.md"] {
        assert!(!listed.contains_key(quiet), "unchanged path {quiet} must not be listed");
    }
    let _ = &token;

    let done = ok(apply(&mut r, &view, &token), "approved recovery");
    let done = structured(&done).clone();
    assert_eq!(done["state"], "completed");
    assert_eq!(done["digest"], view["digest"]);

    // The working folder is exactly the C2 world: partial results replaced, deletions re-created,
    // current-only files (and their now-empty folder) gone, excluded/protected files as they were.
    let now = world(&r.root);
    assert_eq!(now, world_at_c2);
    assert_eq!(r.read_file("results/a.txt"), "A result confirmed in C2\n");
    assert_eq!(r.read_file("data/source.csv"), "id,value\n1,10\n");
    // No file of the failed attempt may remain below the emptied folder on either OS. The Unix
    // writer also prunes the folder itself (Artifact Model §8.4); the Windows handle-based writer
    // currently leaves it empty — recorded in Chain_Recovery_Preservation_Audit_v0.1.md §5.4 as a
    // separate follow-up, not asserted here either way for Windows.
    assert!(files_under(&r.root.join("results/b")).is_empty(), "files of the failed attempt remain under results/b");
    #[cfg(unix)]
    assert!(!r.root.join("results/b").exists(), "emptied folder must be pruned");
    assert!(!r.root.join("evidence/C3-B.md").exists());
    assert_eq!(r.read_file(".env"), "SECRET=baseline");
    assert_eq!(r.read_file("scratch/cache.bin"), "ignored cache bytes");
    assert_eq!(untouched(&r), stamps_before, "unchanged files were rewritten");
    assert_eq!(r.read_file("evidence/C2-A.md"), "# 합성 관측 C2\n\n항목 A 결과 (시험 전용)\n");

    // Every before-world file is preserved under an opaque ordinal with its path and SHA-256;
    // the archive stores the approved plan but never the App token.
    let archive = PathBuf::from(view["archive"].as_str().unwrap());
    assert!(archive.starts_with(preserved_dir(&r).canonicalize().unwrap()), "{}", archive.display());
    assert_eq!(archive.file_name().unwrap().to_str().unwrap(), view["pending_state_sha256"].as_str().unwrap());
    let mapping: Vec<Value> = serde_json::from_slice(&fs::read(archive.join("files.json")).unwrap()).unwrap();
    let mapped: BTreeMap<String, &Value> = mapping.iter().map(|m| (m["path"].as_str().unwrap().to_owned(), m)).collect();
    assert_eq!(mapped.keys().cloned().collect::<Vec<_>>(), world_at_c3.keys().cloned().collect::<Vec<_>>(), "archive must map the whole before world");
    for (path, bytes) in &world_at_c3 {
        let m = mapped[path];
        let saved = fs::read(archive.join(m["saved_as"].as_str().unwrap())).unwrap();
        assert_eq!(&saved, bytes, "archived bytes differ for {path}");
        assert_eq!(m["sha256"], sha256(bytes), "sha256 differs for {path}");
        assert!(m["saved_as"].as_str().unwrap().starts_with("file-"), "{m}");
    }
    assert_eq!(fs::read(archive.join(mapped["results/b/partial.csv"]["saved_as"].as_str().unwrap())).unwrap(), b"id,value\n1,\n");
    let approval_text = fs::read_to_string(archive.join("approval.json")).unwrap();
    let approval: Value = serde_json::from_str(&approval_text).unwrap();
    assert_eq!(approval["state"], "approved");
    assert_eq!(approval["digest"], view["digest"]);
    // Provenance accompanies the unchanged approval and preserved-file contracts.
    let source_text = fs::read_to_string(archive.join("source.json")).unwrap();
    let source: Value = serde_json::from_str(&source_text).unwrap();
    assert_eq!(source["schema_version"], json!(1));
    // The agent's own gil_revisit started this recovery; no person clicked or approved anything.
    assert_eq!(source["approval_source"], "agent_revisit");
    assert_eq!(source["human_click"], json!(false));
    for field in ["digest", "pending_state_sha256", "from", "target"] {
        assert_eq!(source[field], view[field], "approval source must bind {field}");
    }
    assert!(source.get("setting").is_none(), "manual consent must not claim automatic settings");
    let archive_names: Vec<String> = fs::read_dir(&archive).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(archive_names.iter().all(|n| n == "files.json" || n == "approval.json" || n == "source.json" || n.starts_with("file-")), "{archive_names:?}");
    assert_eq!(archive_names.len(), world_at_c3.len() + 3);

    // Records: objects, Will/Existence, approved plan and every closed Cycle (incl. C3 failure) stay.
    assert_eq!(files_under(&r.record.parent().unwrap().join("artifacts")), objects_before, "snapshot objects changed");
    let before: Value = serde_norway::from_slice(&record_before).unwrap();
    let after = yaml(&r.record);
    for field in ["format", "next_will_id", "current_existence_ref", "existences", "existence_states", "artifacts", "visuals"] {
        assert_eq!(before[field], after[field], "recovery changed preserved {field}");
    }
    assert_eq!(before["chains"]["nodes"][0]["approved_plan"], after["chains"]["nodes"][0]["approved_plan"]);
    let cycles_after = cycles_of(&r.record);
    assert_eq!(cycles_before["nodes"], cycles_after["nodes"], "closed Cycles were rewritten");
    assert_eq!(cycles_after["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(cycles_after["pending_cycle_revisit"], json!(3), "{cycles_after}");
    let c3_node = &cycles_after["nodes"][2];
    assert_eq!(c3_node["parent"], json!(2));
    assert!(c3_node.to_string().contains("failure"), "failed Cycle lost its Report: {c3_node}");

    // The same approval is idempotent, then the next branch starts from C2 with the failure lineage.
    let again = ok(apply(&mut r, &view, &token), "duplicate approval");
    assert_eq!(structured(&again)["state"], "completed");
    assert_eq!(fs::read_to_string(archive.join("source.json")).unwrap(), source_text, "duplicate approval rewrote its source");
    assert_eq!(world(&r.root), world_at_c2);
    assert_eq!(fs::read_dir(preserved_dir(&r)).unwrap().count(), 1, "a second archive was minted");
    let read = r.read();
    assert_eq!(read["executed_cycles"].as_array().unwrap().len(), 2);
    assert_eq!(read["pending"][0]["id"], "B", "failed item must be retryable: {read}");
    let opened = ok(r.model("gil_open", json!({"kind":"experiment"})), "branch after recovery");
    assert_eq!(cycle_number(&opened), 4);
    let cycles = cycles_of(&r.record);
    assert_eq!(cycles["pending_cycle_revisit"], Value::Null);
    let c4 = &cycles["nodes"][3];
    assert_eq!(c4["parent"], json!(2), "{c4}");
    assert_eq!(c4["revisit_from"], json!(3), "{c4}");
    assert_eq!(cycles["nodes"][2], cycles_before["nodes"][2], "C3 changed when C4 opened");
    assert_eq!(r.read()["current_item"]["id"], "B");
    assert_eq!(world(&r.root), world_at_c2, "opening the branch wrote files");
    let (status, noise) = std::mem::replace(&mut r.c, Client::serve(&settings)).eof();
    assert!(status.success() && noise.is_empty());
}

#[test]
fn an_excluded_path_differing_in_either_world_refuses_the_whole_recovery_unchanged() {
    for (i, failure, offending) in [
        (0, Failure::CreatesExcludedFile, "scratch/b-cache.bin"),
        (1, Failure::WidensIgnore, "results/"),
    ] {
        let Chain { mut r, settings: _, world_at_c3, .. } = chain(&format!("chain-recovery-excluded-{i}"), failure);
        let record_before = record_bytes(&r.record);
        let objects_before = files_under(&r.record.parent().unwrap().join("artifacts"));
        let frame = refused(r.app("gil_revisit_review", json!({})), "excluded path differs");
        let why = said(&frame);
        assert!(why.contains(offending), "refusal must name the protected path: {why}");
        assert!(frame["result"]["_meta"]["gil/revisitApprovalToken"].is_null(), "no capability on refusal: {frame}");
        refused(r.model("gil_revisit", json!({})), "model call after refusal");
        refused(r.model("gil_open", json!({"kind":"experiment"})), "no branch without recovery");
        assert_eq!(record_bytes(&r.record), record_before, "refusal changed the record");
        assert_eq!(world(&r.root), world_at_c3, "refusal changed working files");
        assert!(!preserved_dir(&r).exists(), "refusal created an archive");
        assert!(!r.record.parent().unwrap().join("restore").exists(), "refusal opened a restore area");
        assert_eq!(files_under(&r.record.parent().unwrap().join("artifacts")), objects_before);
        assert_eq!(cycles_of(&r.record)["pending_cycle_revisit"], Value::Null, "logical move happened despite refusal");
        // The partial results of the failed attempt remain exactly where the user left them.
        assert_eq!(r.read_file("results/a.txt"), "B overwrote A's result halfway\n");
        assert!(!r.root.join("data/source.csv").exists());
    }
}

#[test]
fn after_completion_a_restart_is_a_no_op_and_a_later_file_is_refused_not_swept() {
    let Chain { mut r, settings, world_at_c2, .. } = chain("chain-recovery-restart", Failure::Ordinary);
    let (view, token) = review(&mut r);
    ok(apply(&mut r, &view, &token), "approved recovery");
    assert_eq!(world(&r.root), world_at_c2);
    let record_after = record_bytes(&r.record);
    let archive = files_under(&preserved_dir(&r));
    assert_eq!(fs::read_dir(preserved_dir(&r)).unwrap().count(), 1);

    // A new process re-reads the durable approval; the completed recovery converges without writes.
    r.restart(&settings);
    let resumed = ok(r.model("gil_revisit", json!({})), "resume after restart");
    let receipt = said(&resumed);
    assert!(receipt.contains("\"state\": \"completed\"") || receipt.contains("\"state\":\"completed\""), "model receipt must report the completed recovery: {receipt}");
    assert_eq!(world(&r.root), world_at_c2);
    assert_eq!(record_bytes(&r.record), record_after);
    assert_eq!(files_under(&preserved_dir(&r)), archive, "resume rewrote or added archive files");
    assert!(!r.record.parent().unwrap().join("restore/active").exists());
    let reviewed = ok(r.app("gil_revisit_review", json!({})), "review of a completed recovery");
    assert_eq!(structured(&reviewed)["state"], "completed");
    assert_eq!(structured(&reviewed)["digest"], view["digest"], "completed view must bind the same plan");

    // Files written after completion belong to the user: no sweep, no archive, explicit refusal.
    r.write("notes/after-recovery.md", "written by hand after the recovery\n");
    let frame = refused(r.model("gil_revisit", json!({})), "unrecorded change after completion");
    assert!(said(&frame).contains("기록되지 않은 파일 변경"), "{frame}");
    assert_eq!(r.read_file("notes/after-recovery.md"), "written by hand after the recovery\n");
    assert_eq!(record_bytes(&r.record), record_after);
    assert_eq!(files_under(&preserved_dir(&r)), archive);
    let mut expected = world_at_c2.clone();
    expected.insert("notes/after-recovery.md".into(), b"written by hand after the recovery\n".to_vec());
    assert_eq!(world(&r.root), expected);
    // The user's file does not trap them between a refused resume and a refused branch: opening
    // the next Cycle is a Graph transition that neither reads nor rewrites the working folder.
    let opened = ok(r.model("gil_open", json!({"kind":"experiment"})), "branch after restart");
    assert_eq!(cycle_number(&opened), 4);
    assert!(said(&opened).contains("cycle:C2") && said(&opened).contains("cycle:C3"), "{opened}");
    assert_eq!(world(&r.root), expected, "opening the branch touched working files");
    assert_eq!(files_under(&preserved_dir(&r)), archive);
}
