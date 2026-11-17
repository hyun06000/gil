//! From an approved requirement to the original records of its Experiments, over the real
//! `gil mcp --serve` process.
//!
//! `gil_plan_read` lists the Experiments linked to a requirement. This file fixes the rest of
//! the path, which `gil_report_read` and `gil_evidence_read` open in an `exploration-flow` build:
//!
//! ```text
//! requirement → linked Experiment → its Define → its Verify and declared evidence
//!             → Analysis · Outcome · Cycle Report as stored → the stored evidence bytes
//! ```
//!
//! What is checked here and nowhere else:
//!
//! - several Experiments under one requirement are each found and read verbatim, the failed
//!   one exactly like the successful one, and no answer names a requirement as answered;
//! - after the approved revisit returned the working folder to an earlier world, the failed
//!   Experiment's Reports and stored evidence are still read, and a file whose path now holds
//!   other content is not mistaken for that Experiment's evidence;
//! - a restarted process gives the same originals with the same provenance;
//! - an unknown reference, a reference of another scope, an open Step, an undeclared path and
//!   an unreadable store are told apart, and none of them is answered with a nearby record;
//! - long evidence is cut openly, with the way to read the rest, and reassembles byte for byte;
//! - reading changes neither the record nor the working folder;
//! - a Chain without the contract marker keeps its own responses and is still readable, also
//!   from the Chain that follows it.
//!
//! Not repeated here: which links are refused and how a failure is restored
//! (tests/exploration_flow.rs), visual declarations, citations and stored images
//! (tests/exploration_tools.rs).
//!
//! Scope of the evidence: synthetic tool-path checks. The App decisions are harness inputs, not
//! a person's click. That a model reads the right original, reasons soundly from it or stops
//! treating a guess as a premise is behavior this file does not and cannot show.
#![cfg(all(
    feature = "exploration-flow",
    feature = "product-safety",
    feature = "visual-authoring",
    any(target_os = "macos", windows)
))]
#[path = "support_wave1/mod.rs"]
mod support;
#[path = "support_wave1/exploration.rs"]
mod exploration;
use base64::Engine;
use exploration::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use support::*;

const SUCCESS_CONDITION: &str = "지정한 파일의 관측 증거가 위 명제의 긍정 답을 뒷받침한다. 근거 있는 부정 답이나 판정 미도달은 성공이 아니다";
const CHUNK: u64 = 16 * 1024;

/// One small Experiment with its own words in every Report, so each original is recognizable.
struct Spec<'a> {
    requirement: &'a str,
    problem: &'a str,
    files: &'a [(&'a str, &'a [u8])],
    evidence: &'a [&'a str],
    result: &'a str,
    interpretation: &'a str,
    lesson: &'a str,
    fails_back_to: Option<&'a str>,
}
/// The canonical references of one walked Experiment.
struct Walked {
    cycle: String,
    define: String,
    hypothesis: String,
    verify: String,
    analysis: String,
    outcome: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn verify_report(spec: &Spec<'_>) -> String {
    let files: String = spec.evidence.iter().map(|path| format!("  {path}\n")).collect();
    format!("execution: 합성 자료를 관측했다\nresult: {}\nplan_document: {PLAN_DOCUMENT}\nevidence_files: |\n{files}", spec.result)
}
fn yes_no(failed: bool) -> &'static str {
    if failed { "no" } else { "yes" }
}

/// Define → Hypothesis → Verify → Analysis → Outcome → Cycle Report of the open Experiment `n`.
fn walk(r: &mut Rig, n: u32, spec: &Spec<'_>, document: &str) -> Walked {
    let failed = spec.fails_back_to.is_some();
    let define = step_ref(&ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define"), "define");
    ok(r.model("gil_close", json!({"report":format!("problem: {}\nsuccess_condition: {SUCCESS_CONDITION}\n", spec.problem)})), "close define");
    let hypothesis = step_ref(&ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis"), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let verify = step_ref(&ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify"), "verify");
    r.write(PLAN_DOCUMENT, document);
    for (path, bytes) in spec.files {
        r.write_bytes(path, bytes);
    }
    ok(r.c.call("gil_close", verify_args(&r.root, &verify, &verify_report(spec))), "close verify");
    let analysis = step_ref(&ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis"), "analysis");
    let fit = yes_no(failed);
    ok(r.model("gil_close", json!({"report":format!("hypothesis_fit: {fit}\nproblem_solved: {fit}\nsuccess_condition_met: {fit}\nguardrail_triggered: no\ninterpretation: {}\n", spec.interpretation)})), "close analysis");
    let outcome = step_ref(&ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome"), "outcome");
    let verdict = if failed { "failure" } else { "success" };
    ok(r.model("gil_close", json!({"report":format!("verdict: {verdict}\nlesson: {}\nnext_direction.action: close_cycle\nnext_direction.reason: 이 질문의 판정을 마쳤다\n", spec.lesson)})), "close outcome");
    ok(r.model("gil_close", json!({"report":cycle_report(n, &outcome, spec.fails_back_to)})), "close experiment");
    Walked { cycle: format!("cycle:C{n}"), define, hypothesis, verify, analysis, outcome }
}
/// Opens an Experiment linked to the requirement and walks it.
fn experiment(r: &mut Rig, spec: &Spec<'_>) -> Walked {
    let n = r.open_experiment(spec.requirement);
    let document = r.read()["document"].as_str().expect("document after linking").to_owned();
    walk(r, n, spec, &document)
}

fn owner(reference: &str) -> Value {
    let kind = reference.split(':').next().unwrap();
    json!({"kind":kind,"ref":reference})
}
fn report(r: &mut Rig, reference: &str) -> Value {
    structured(&ok(r.model("gil_report_read", json!({"owner":owner(reference)})), reference)).clone()
}
fn evidence_at(r: &mut Rig, verify: &str, path: &str, offset: Option<u64>) -> Value {
    let mut args = json!({"owner":owner(verify),"path":path});
    if let Some(offset) = offset { args["offset"] = json!(offset); }
    r.model("gil_evidence_read", args)
}
fn evidence(r: &mut Rig, verify: &str, path: &str) -> Value {
    structured(&ok(evidence_at(r, verify, path, None), path)).clone()
}
/// A Core refusal with a stable code; never a JSON-RPC error and never a success.
#[track_caller]
fn code_of(frame: Value, what: &str) -> (String, Value) {
    let frame = refused(frame, what);
    let body = structured(&frame).clone();
    (body["code"].as_str().unwrap_or_else(|| panic!("{what}: no code in {frame}")).to_owned(), body)
}
fn keys(value: &Value) -> BTreeSet<String> {
    value.as_object().unwrap_or_else(|| panic!("not an object: {value}")).keys().cloned().collect()
}
fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| name.to_string()).collect()
}
fn fields(read: &Value) -> BTreeMap<String, String> {
    assert_eq!(read["report"]["state"], "written", "{read}");
    read["report"]["fields"].as_array().unwrap().iter()
        .map(|field| (field["name"].as_str().unwrap().to_owned(), field["value"].as_str().unwrap().to_owned())).collect()
}
/// Every object key anywhere in an answer.
fn all_keys(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => for (key, inner) in map { into.insert(key.clone()); all_keys(inner, into); },
        Value::Array(list) => for inner in list { all_keys(inner, into); },
        _ => {}
    }
}
/// No answer carries a synthesized answer, a validity or confidence judgment or a completion rate.
#[track_caller]
fn carries_no_judgment(answer: &Value) {
    let mut found = BTreeSet::new();
    all_keys(answer, &mut found);
    for forbidden in ["answer", "answered", "resolved", "resolution", "requirement_resolution", "complete", "completed", "completion",
        "completion_rate", "progress", "confidence", "valid", "validity", "trusted", "summary", "synthesis", "conclusion",
        "current_answer", "score", "satisfied", "met", "adopted", "preferred"] {
        assert!(!found.contains(forbidden), "an original-record read carries `{forbidden}`: {answer}");
    }
}
/// Every file below the record folder. A read must leave all of them as they were; the lock
/// file is the one thing a read is allowed to open.
fn record_tree(record: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() { visit(base, &path, out); continue; }
            let name = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            if name == "project.lock" { continue; }
            out.insert(name, fs::read(&path).unwrap());
        }
    }
    let base = record.parent().unwrap();
    let mut out = BTreeMap::new();
    visit(base, base, &mut out);
    assert!(out.contains_key("state.yaml") && out.len() > 1, "the record and its objects are in view: {:?}", out.keys().collect::<Vec<_>>());
    out
}

const COLUMNS: &str = "evidence/columns.md";
const ROWS: &str = "evidence/rows.md";
const COLUMNS_FIRST: &str = "열 수 관측: 2 (첫 실험)\n";
const COLUMNS_REWRITTEN: &str = "열 수 재관측: 판독 실패 (둘째 실험이 같은 경로를 덮어썼다)\n";
const ROWS_SEEN: &str = "행 수 관측: 1\n";

/// One requirement, a successful Experiment, then a failed one that rewrote the first one's
/// evidence path and added a file, then the approved revisit and a third, still empty Experiment.
fn two_experiments_and_a_revisit(label: &str) -> (Rig, Walked, Walked, u32) {
    let mut r = Rig::start(label, &[("data/input.csv", "a,b\n1,2\n")]);
    r.interview(&["R1", "R2"], 1, &[], "open_child");
    let first = experiment(&mut r, &Spec {
        requirement: "R1", problem: "입력 파일의 열은 두 개인가?",
        files: &[(COLUMNS, COLUMNS_FIRST.as_bytes())], evidence: &[COLUMNS],
        result: "열 두 개를 관측했다", interpretation: "관측한 열 수가 명제와 일치한다",
        lesson: "열 구조는 확인됐다고 기록한다", fails_back_to: None,
    });
    let second = experiment(&mut r, &Spec {
        requirement: "R1", problem: "입력 파일의 행은 열 개 이상인가?",
        files: &[(COLUMNS, COLUMNS_REWRITTEN.as_bytes()), (ROWS, ROWS_SEEN.as_bytes())], evidence: &[COLUMNS, ROWS],
        result: "행 한 개를 관측했고 열 수는 다시 읽지 못했다", interpretation: "관측한 행 수가 기준에 못 미친다",
        lesson: "행이 충분하다는 가정이 관측과 어긋났다", fails_back_to: Some(&first.cycle),
    });
    r.recover();
    let third = r.open_experiment("R1");
    (r, first, second, third)
}

#[test]
fn a_requirement_leads_to_each_linked_experiments_original_reports_and_stored_evidence() {
    let (mut r, first, second, third) = two_experiments_and_a_revisit("evidence-path");
    let third_cycle = format!("cycle:C{third}");

    // The tools exist in this build as model-visible reads, and the link list points at them.
    let listed = r.c.request("tools/list", json!({}));
    for name in ["gil_report_read", "gil_evidence_read"] {
        let tool = listed["result"]["tools"].as_array().unwrap().iter().find(|tool| tool["name"] == name).unwrap_or_else(|| panic!("{name} is not listed"));
        assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["model"]), "{tool}");
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
    }
    let plan = r.read();
    assert_eq!(r.experiments("R1"), vec![(first.cycle.clone(), "success".to_string()), (second.cycle.clone(), "failure".to_string()), (third_cycle.clone(), "open".to_string())]);
    assert!(r.experiments("R2").is_empty());
    let next = plan["next"].as_str().unwrap();
    assert!(next.contains("gil_report_read") && next.contains("gil_evidence_read") && next.contains("판정하지 않는다"), "{next}");
    let digest = plan["active_plan"]["digest"].as_str().unwrap().to_owned();
    let document = plan["document"].as_str().unwrap().to_owned();
    let context = r.context();
    assert!(context.contains("원본 근거") && context.contains("gil_report_read") && context.contains("잠정 정리를 따로 쌓아"), "{context}");

    // The working folder is the restored world: the first Experiment's file, no trace of the second's.
    let folder = world(&r.root);
    assert_eq!(folder[COLUMNS], COLUMNS_FIRST.as_bytes());
    assert!(!folder.contains_key(ROWS), "{:?}", folder.keys().collect::<Vec<_>>());
    let (record_before, world_before) = (record_tree(&r.record), world(&r.root));

    // ── requirement → the failed Experiment, found by the reference the link list gave ──
    let cycle = report(&mut r, &second.cycle);
    assert_eq!(keys(&cycle), names(&["ok", "schema_version", "source", "owner", "chain", "cycle", "plan_link", "steps", "report", "note"]));
    assert_eq!(cycle["source"], "stored_record");
    assert_eq!(cycle["owner"], owner(&second.cycle));
    assert_eq!(cycle["chain"], json!({"ref":"chain:C1","state":"open"}));
    assert_eq!(cycle["plan_link"], json!({"contract":"exploration","plan_revision":1,"plan_digest":digest,"requirement_id":"R1"}));
    let position = &cycle["cycle"];
    assert_eq!((position["ref"].as_str(), position["kind"].as_str(), position["state"].as_str(), position["verdict"].as_str()),
        (Some(second.cycle.as_str()), Some("experiment"), Some("closed"), Some("failure")), "{cycle}");
    assert_eq!((position["parent"].as_str(), &position["revisit_from"]), (Some(first.cycle.as_str()), &Value::Null), "{cycle}");
    let stored_world = position["exit_snapshot"].as_str().unwrap().to_owned();
    assert!(stored_world.starts_with("snapshot:A") && position["entry_snapshot"].as_str().unwrap() != stored_world, "{cycle}");
    // The Steps are addresses only: no Report body travels with the index.
    let steps = cycle["steps"].as_array().unwrap();
    assert_eq!(steps.iter().map(|step| (step["ref"].as_str().unwrap(), step["kind"].as_str().unwrap())).collect::<Vec<_>>(),
        vec![(second.define.as_str(), "define"), (second.hypothesis.as_str(), "hypothesis"), (second.verify.as_str(), "verify"), (second.analysis.as_str(), "analysis"), (second.outcome.as_str(), "outcome")]);
    for step in steps {
        assert_eq!((step["state"].as_str(), step["report"].as_str()), (Some("closed"), Some("written")), "{step}");
        assert!(step.get("fields").is_none() && step.get("value").is_none(), "{step}");
    }
    assert_eq!(steps[2]["snapshot"], json!(stored_world), "the Verify confirmed the world the Cycle exited with");
    // The Cycle Report as it was written, the failure's revisit declaration included.
    let wrote = fields(&cycle);
    assert_eq!(wrote, BTreeMap::from([
        ("verdict".to_string(), "failure".to_string()), ("outcome_ref".to_string(), second.outcome.clone()),
        ("handoff_summary".to_string(), format!("C{}의 관측과 한계를 남겼다", &second.cycle["cycle:C".len()..])),
        ("next_direction.action".to_string(), "revisit".to_string()), ("next_direction.target_cycle_ref".to_string(), first.cycle.clone()),
        ("next_direction.reason".to_string(), "관측에 따라 다음을 정한다".to_string())]));

    // ── its Define: the question and the judgment condition, as fixed before the observation ──
    let define = report(&mut r, &second.define);
    assert_eq!(keys(&define), names(&["ok", "schema_version", "source", "owner", "chain", "cycle", "plan_link", "step", "report", "note"]));
    assert_eq!(define["step"], json!({"ref":second.define,"kind":"define","state":"closed"}));
    assert_eq!(fields(&define), BTreeMap::from([("problem".to_string(), "입력 파일의 행은 열 개 이상인가?".to_string()), ("success_condition".to_string(), SUCCESS_CONDITION.to_string())]));
    assert_eq!(define["plan_link"], cycle["plan_link"]);
    assert_eq!(define["cycle"], cycle["cycle"], "a Step answer names the same Cycle position");

    // ── its Verify: the observation and what it declared as evidence ──
    let verify = report(&mut r, &second.verify);
    assert_eq!(keys(&verify), names(&["ok", "schema_version", "source", "owner", "chain", "cycle", "plan_link", "step", "report", "evidence", "note"]));
    assert_eq!(verify["step"], json!({"ref":second.verify,"kind":"verify","state":"closed","snapshot":stored_world}));
    let observed = fields(&verify);
    assert_eq!(observed["result"], "행 한 개를 관측했고 열 수는 다시 읽지 못했다");
    assert_eq!(observed["execution"], "합성 자료를 관측했다");
    assert_eq!(observed["plan_document"], PLAN_DOCUMENT);
    assert_eq!(observed["evidence_files"].lines().map(str::trim).collect::<Vec<_>>(), vec![COLUMNS, ROWS]);
    let current_world = verify["evidence"]["current_world"].as_str().unwrap().to_owned();
    assert_ne!(current_world, stored_world, "the folder was restored to another world");
    assert_eq!(verify["evidence"], json!({"state":"indexed","source":"stored_snapshot","snapshot":stored_world,"current_world":current_world,"read_with":"gil_evidence_read","files":[
        {"path":COLUMNS,"declared_in":"evidence_files","stored":"present","sha256":sha256(COLUMNS_REWRITTEN.as_bytes()),"in_current_world":"different"},
        {"path":ROWS,"declared_in":"evidence_files","stored":"present","sha256":sha256(ROWS_SEEN.as_bytes()),"in_current_world":"absent"},
        {"path":PLAN_DOCUMENT,"declared_in":"plan_document","stored":"present","sha256":sha256(document.as_bytes()),"in_current_world":"same"}]}));

    // ── Analysis and Outcome: the interpretation and the lesson, with the links the Core recorded ──
    let analysis = fields(&report(&mut r, &second.analysis));
    assert_eq!(analysis["interpretation"], "관측한 행 수가 기준에 못 미친다");
    assert_eq!((analysis["success_condition_met"].as_str(), analysis["evidence.verify_ref"].as_str()), ("no", second.verify.as_str()));
    let outcome = fields(&report(&mut r, &second.outcome));
    assert_eq!(outcome["lesson"], "행이 충분하다는 가정이 관측과 어긋났다");
    assert_eq!((outcome["verdict"].as_str(), outcome["learning.analysis_ref"].as_str()), ("failure", second.analysis.as_str()));

    // ── the stored evidence: the failed Experiment's bytes, not the folder's file of that name ──
    let rewritten = evidence(&mut r, &second.verify, COLUMNS);
    assert_eq!(keys(&rewritten), names(&["ok", "schema_version", "source", "owner", "chain", "cycle", "plan_link", "snapshot", "path", "sha256",
        "total_bytes", "offset", "returned_bytes", "truncated", "next_offset", "remaining_bytes", "encoding", "text", "current_world", "in_current_world", "note"]));
    assert_eq!((rewritten["source"].as_str(), rewritten["snapshot"].as_str(), rewritten["path"].as_str()), (Some("stored_snapshot"), Some(stored_world.as_str()), Some(COLUMNS)));
    assert_eq!((rewritten["encoding"].as_str(), rewritten["text"].as_str()), (Some("utf8"), Some(COLUMNS_REWRITTEN)));
    assert_eq!(rewritten["sha256"], json!(sha256(COLUMNS_REWRITTEN.as_bytes())));
    assert_eq!((rewritten["total_bytes"].as_u64(), rewritten["returned_bytes"].as_u64(), rewritten["offset"].as_u64()), (Some(COLUMNS_REWRITTEN.len() as u64), Some(COLUMNS_REWRITTEN.len() as u64), Some(0)));
    assert_eq!((&rewritten["truncated"], &rewritten["next_offset"], rewritten["remaining_bytes"].as_u64()), (&json!(false), &Value::Null, Some(0)));
    assert_eq!((rewritten["in_current_world"].as_str(), rewritten["current_world"].as_str()), (Some("different"), Some(current_world.as_str())));
    assert_eq!((&rewritten["chain"], &rewritten["cycle"], &rewritten["plan_link"]), (&cycle["chain"], &cycle["cycle"], &cycle["plan_link"]), "the bytes carry the same provenance");
    assert!(rewritten["note"].as_str().unwrap().contains("작업 폴더를 읽지 않았고"), "{rewritten}");
    assert_eq!(world(&r.root)[COLUMNS], COLUMNS_FIRST.as_bytes(), "the folder still holds the other content at the same path");
    let rows = evidence(&mut r, &second.verify, ROWS);
    assert_eq!((rows["text"].as_str(), rows["in_current_world"].as_str()), (Some(ROWS_SEEN), Some("absent")));
    assert!(!world(&r.root).contains_key(ROWS), "reading stored evidence did not bring the file back");
    // The approved revision the Experiment ran under is stored evidence too.
    assert_eq!(evidence(&mut r, &second.verify, PLAN_DOCUMENT)["text"], json!(document));

    // ── the successful Experiment under the same requirement reads the same way ──
    let earlier = report(&mut r, &first.cycle);
    assert_eq!((earlier["cycle"]["verdict"].as_str(), &earlier["plan_link"]), (Some("success"), &cycle["plan_link"]));
    assert_eq!(fields(&report(&mut r, &first.define))["problem"], "입력 파일의 열은 두 개인가?");
    assert_eq!(fields(&report(&mut r, &first.verify))["result"], "열 두 개를 관측했다");
    let original = evidence(&mut r, &first.verify, COLUMNS);
    assert_eq!((original["text"].as_str(), original["in_current_world"].as_str()), (Some(COLUMNS_FIRST), Some("same")));
    assert_eq!(original["snapshot"], earlier["cycle"]["exit_snapshot"]);
    // Two records disagree about the same path; each is returned as written and neither is chosen.
    assert_ne!(original["text"], rewritten["text"]);
    assert_ne!(original["snapshot"], rewritten["snapshot"]);

    // ── the Chain as addresses: the failed Experiment is listed beside the others ──
    let chain = report(&mut r, "chain:C1");
    assert_eq!(keys(&chain), names(&["ok", "schema_version", "source", "owner", "chain", "cycles", "report", "note"]));
    assert_eq!((chain["chain"]["ref"].as_str(), chain["chain"]["state"].as_str(), &chain["chain"]["parent"], &chain["chain"]["exit_snapshot"]), (Some("chain:C1"), Some("open"), &Value::Null, &Value::Null));
    assert_eq!(chain["report"], json!({"state":"not_written"}), "an open Chain has no Chain Report");
    let listed: Vec<(String, String, Value)> = chain["cycles"].as_array().unwrap().iter()
        .map(|c| (c["ref"].as_str().unwrap().to_owned(), c["verdict"].as_str().unwrap_or(c["state"].as_str().unwrap()).to_owned(), c["plan_link"]["requirement_id"].clone())).collect();
    assert_eq!(listed, vec![("cycle:C1".to_string(), "success".to_string(), Value::Null), (first.cycle.clone(), "success".to_string(), json!("R1")),
        (second.cycle.clone(), "failure".to_string(), json!("R1")), (third_cycle.clone(), "open".to_string(), json!("R1"))]);
    let open_cycle = report(&mut r, &third_cycle);
    assert_eq!((open_cycle["cycle"]["state"].as_str(), &open_cycle["cycle"]["verdict"], open_cycle["cycle"]["revisit_from"].as_str()), (Some("open"), &Value::Null, Some(second.cycle.as_str())));
    assert_eq!((&open_cycle["steps"], &open_cycle["report"]), (&json!([]), &json!({"state":"not_written"})));

    // ── no answer judges: nothing says a requirement is answered, valid, trusted or n% done ──
    let reads: Vec<(&str, Value)> = vec![
        ("gil_report_read", json!({"owner":owner("chain:C1")})),
        ("gil_report_read", json!({"owner":owner(&first.cycle)})), ("gil_report_read", json!({"owner":owner(&second.cycle)})), ("gil_report_read", json!({"owner":owner(&third_cycle)})),
        ("gil_report_read", json!({"owner":owner(&first.verify)})), ("gil_report_read", json!({"owner":owner(&second.define)})),
        ("gil_report_read", json!({"owner":owner(&second.verify)})), ("gil_report_read", json!({"owner":owner(&second.analysis)})), ("gil_report_read", json!({"owner":owner(&second.outcome)})),
        ("gil_evidence_read", json!({"owner":owner(&first.verify),"path":COLUMNS})), ("gil_evidence_read", json!({"owner":owner(&second.verify),"path":COLUMNS})),
        ("gil_evidence_read", json!({"owner":owner(&second.verify),"path":ROWS})), ("gil_evidence_read", json!({"owner":owner(&second.verify),"path":PLAN_DOCUMENT})),
    ];
    let answers = |r: &mut Rig| -> Vec<Value> { reads.iter().map(|(name, args)| structured(&ok(r.model(name, args.clone()), name)).clone()).collect() };
    let before_restart = answers(&mut r);
    for answer in &before_restart {
        carries_no_judgment(answer);
        assert!(answer["note"].as_str().unwrap().contains("참이거나 지금도 유효하다는 판정이 아니"), "{answer}");
    }
    // The digest is the one the visual read reports for the same Report.
    for reference in [&second.verify, &second.cycle] {
        let visuals = ok(r.model("gil_report_visuals", json!({"owner":owner(reference)})), "visuals");
        assert_eq!(structured(&visuals)["report_sha256"], report(&mut r, reference)["report"]["sha256"], "{reference}");
    }

    // ── reading changed nothing, and a new process finds the same originals and provenance ──
    assert_eq!(answers(&mut r), before_restart, "a repeated read is the same answer");
    assert_eq!(record_tree(&r.record), record_before, "a read changed the record or its objects");
    assert_eq!(world(&r.root), world_before, "a read changed the working folder");
    assert_eq!(r.read(), plan, "the link list is unchanged; nothing was consumed or completed by reading");
    r.restart();
    assert_eq!(answers(&mut r), before_restart, "after a restart the same originals and provenance are found");
    assert_eq!(r.read(), plan);
    assert_eq!(record_tree(&r.record), record_before);
    assert_eq!(world(&r.root), world_before);
}

#[test]
fn absent_other_scope_open_and_unreadable_are_each_told_apart() {
    let (mut r, first, second, third) = two_experiments_and_a_revisit("evidence-absent");
    let document = r.read()["document"].as_str().unwrap().to_owned();
    // The third Experiment: an open Define, to stand on an open Step.
    let define = step_ref(&ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define"), "define");
    let (record_before, world_before) = (record_tree(&r.record), world(&r.root));

    // A reference this Project does not hold is refused under the asked address; nothing near
    // it — the neighbouring number, the same Step number of another Cycle — is answered instead.
    for reference in ["cycle:C99", "step:C99/S1", &format!("{}/S99", first.cycle.replace("cycle:", "step:")), "chain:C9"] {
        let (code, body) = code_of(r.model("gil_report_read", json!({"owner":owner(reference)})), reference);
        assert_eq!((code.as_str(), &body["owner"]), ("not_found", &owner(reference)), "{body}");
        assert!(body["said"].as_str().unwrap().contains(reference) && body.get("report").is_none(), "{body}");
    }
    let (code, _) = code_of(evidence_at(&mut r, "step:C99/S3", COLUMNS, None), "evidence of an unknown Step");
    assert_eq!(code, "not_found");

    // An open Step and an open Cycle are records without a Report yet — an answer, not a refusal,
    // and not an empty Report.
    let open = report(&mut r, &define);
    assert_eq!((&open["step"], &open["report"]), (&json!({"ref":define,"kind":"define","state":"open"}), &json!({"state":"not_written"})));
    assert!(open.get("evidence").is_none(), "{open}");
    let open_cycle = report(&mut r, &format!("cycle:C{third}"));
    assert_eq!(open_cycle["steps"], json!([{"ref":define,"kind":"define","state":"open","report":"not_written"}]));
    assert_eq!(open_cycle["report"], json!({"state":"not_written"}));

    // Evidence is asked of a Verify that has confirmed a world; every other case says which it is.
    let (code, body) = code_of(evidence_at(&mut r, &first.analysis, COLUMNS, None), "evidence of an Analysis");
    assert_eq!((code.as_str(), body["kind"].as_str()), ("not_a_verify", Some("analysis")), "{body}");
    // A path another Verify declared, and a path that is in the Snapshot but was never declared.
    for path in [ROWS, "data/input.csv"] {
        let (code, body) = code_of(evidence_at(&mut r, &first.verify, path, None), path);
        assert_eq!((code.as_str(), &body["declared"]), ("not_declared", &json!([COLUMNS, PLAN_DOCUMENT])), "{body}");
        assert!(body.get("text").is_none() && body.get("data_base64").is_none(), "{body}");
    }
    for path in ["../outside.md", ".gil/state.yaml", "/etc/hosts", "evidence/../data/input.csv"] {
        let (code, _) = code_of(evidence_at(&mut r, &first.verify, path, None), path);
        assert_eq!(code, "invalid_path", "{path}");
    }
    let total = COLUMNS_FIRST.len() as u64;
    let (code, body) = code_of(evidence_at(&mut r, &first.verify, COLUMNS, Some(total)), "offset at the end");
    assert_eq!((code.as_str(), body["total_bytes"].as_u64()), ("invalid_offset", Some(total)), "{body}");
    let tail = structured(&ok(evidence_at(&mut r, &first.verify, COLUMNS, Some(total - 1)), "last byte")).clone();
    assert_eq!((tail["text"].as_str(), tail["returned_bytes"].as_u64(), &tail["truncated"]), (Some("\n"), Some(1), &json!(false)));

    // Shapes the schema refuses never reach the record: a non-canonical reference, a Cycle
    // where a Verify is required, an extra field. The rejection names the shape, not the input.
    for (name, args) in [
        ("gil_report_read", json!({"owner":{"kind":"cycle","ref":"cycle:C02"}})),
        ("gil_report_read", json!({"owner":{"kind":"cycle","ref":first.verify}})),
        ("gil_report_read", json!({"owner":owner(&first.cycle),"path":COLUMNS})),
        ("gil_evidence_read", json!({"owner":owner(&first.cycle),"path":COLUMNS})),
        ("gil_evidence_read", json!({"owner":owner(&first.verify)})),
        ("gil_evidence_read", json!({"owner":owner(&first.verify),"path":COLUMNS,"offset":-1})),
        ("gil_evidence_read", json!({"owner":owner(&first.verify),"path":COLUMNS,"sha256":"0"})),
    ] {
        let error = rpc_error(r.model(name, args.clone()), name);
        let message = error["error"]["message"].as_str().unwrap();
        assert!(message.starts_with(name) && message.contains("Nothing was read or changed"), "{message}");
        assert!(!message.contains(r.root.to_str().unwrap()), "{message}");
    }
    // A folder with no Project: the record is unavailable, which is not "no such Report".
    let empty = scratch("evidence-absent-empty");
    let (code, _) = code_of(r.c.call("gil_report_read", json!({"project_root":empty,"owner":owner("cycle:C1")})), "no project");
    assert_eq!(code, "record_unavailable");

    assert_eq!(record_tree(&r.record), record_before, "a refused read changed the record");
    assert_eq!(world(&r.root), world_before, "a refused read changed the working folder");

    // An open Verify has no Report and no Snapshot: no stored evidence yet, not "no evidence".
    ok(r.model("gil_close", json!({"report":format!("problem: 입력 파일에 머리글 줄이 있는가?\nsuccess_condition: {SUCCESS_CONDITION}\n")})), "close define");
    ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let verify = step_ref(&ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify"), "verify");
    r.write(PLAN_DOCUMENT, &document);
    r.write("evidence/header.md", "머리글 관측: a,b\n");
    let (code, _) = code_of(evidence_at(&mut r, &verify, "evidence/header.md", None), "evidence of an open Verify");
    assert_eq!(code, "report_not_written", "a file already in the folder is not that Verify's stored evidence");
    let open = report(&mut r, &verify);
    assert_eq!((&open["step"], &open["report"]), (&json!({"ref":verify,"kind":"verify","state":"open","snapshot":null}), &json!({"state":"not_written"})));
    assert!(open.get("evidence").is_none(), "{open}");

    // A stored object that cannot be read is "unavailable", distinct from "not in the Snapshot"
    // and from "not declared". The harness hides one blob and puts it back; the tool only reads.
    let objects = r.record.parent().unwrap().join("artifacts");
    let hidden = find_object(&objects, ROWS_SEEN.as_bytes());
    let aside = hidden.with_extension("hidden-by-test");
    fs::rename(&hidden, &aside).unwrap();
    let (code, body) = code_of(evidence_at(&mut r, &second.verify, ROWS, None), "a hidden blob");
    assert_eq!(code, "evidence_unavailable", "{body}");
    assert!(body["said"].as_str().unwrap().contains("증거가 없었다거나 틀렸다는 뜻이 아니다") && body.get("text").is_none(), "{body}");
    assert!(!r.root.join(ROWS).exists(), "an unreadable store is not replaced by a folder file");
    // The other evidence of the same Verify is still read.
    assert_eq!(evidence(&mut r, &second.verify, COLUMNS)["text"], json!(COLUMNS_REWRITTEN));
    fs::rename(&aside, &hidden).unwrap();
    assert_eq!(evidence(&mut r, &second.verify, ROWS)["text"], json!(ROWS_SEEN), "the same read succeeds once the store is whole");
}

/// The object-store file holding exactly these bytes.
fn find_object(dir: &Path, bytes: &[u8]) -> PathBuf {
    fn visit(at: &Path, bytes: &[u8], found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() { visit(&path, bytes, found); } else if fs::read(&path).is_ok_and(|held| held == bytes) { found.push(path); }
        }
    }
    let mut found = Vec::new();
    visit(dir, bytes, &mut found);
    assert_eq!(found.len(), 1, "exactly one stored object holds the evidence bytes: {found:?}");
    found.remove(0)
}

/// Reads one stored file to its end the way the answers direct, checking every cut.
fn read_to_end(r: &mut Rig, verify: &str, path: &str, whole: &[u8]) -> (Vec<u8>, Vec<String>) {
    let (mut bytes, mut encodings, mut offset) = (Vec::new(), Vec::new(), 0u64);
    loop {
        let part = structured(&ok(evidence_at(r, verify, path, if offset == 0 { None } else { Some(offset) }), path)).clone();
        let carried = part["returned_bytes"].as_u64().unwrap();
        assert!(carried > 0 && carried <= CHUNK, "{carried}");
        assert_eq!((part["offset"].as_u64(), part["total_bytes"].as_u64(), part["sha256"].as_str()), (Some(offset), Some(whole.len() as u64), Some(sha256(whole).as_str())), "every part describes the whole file");
        let encoding = part["encoding"].as_str().unwrap().to_owned();
        let piece = match encoding.as_str() {
            "utf8" => { assert!(part.get("data_base64").is_none()); part["text"].as_str().unwrap().as_bytes().to_vec() }
            "base64" => { assert!(part.get("text").is_none()); base64::engine::general_purpose::STANDARD.decode(part["data_base64"].as_str().unwrap()).unwrap() }
            other => panic!("unknown encoding {other}"),
        };
        assert_eq!(piece.len() as u64, carried, "returned_bytes counts stored bytes");
        bytes.extend_from_slice(&piece);
        encodings.push(encoding);
        let end = offset + carried;
        assert_eq!(part["remaining_bytes"].as_u64(), Some(whole.len() as u64 - end));
        if part["truncated"] == json!(true) {
            // The cut is stated, with the exact call that reads the rest.
            assert_eq!(part["next_offset"].as_u64(), Some(end), "{part}");
            assert!(part["note"].as_str().unwrap().contains(&format!("offset: {end}")), "{part}");
            offset = end;
        } else {
            assert_eq!((&part["next_offset"], end), (&Value::Null, whole.len() as u64), "{part}");
            return (bytes, encodings);
        }
    }
}

#[test]
fn long_evidence_is_cut_openly_and_reads_back_whole_while_a_report_is_never_cut() {
    let mut r = Rig::start("evidence-long", &[("data/input.csv", "a,b\n1,2\n")]);
    r.interview(&["R1"], 1, &[], "open_child");
    // Multi-byte text longer than two parts, binary bytes that are not text, and a long Report field.
    let long: String = (0..3000).map(|line| format!("관측 줄 {line:04}: 값을 그대로 적는다\n")).collect();
    let binary: Vec<u8> = (0..20_000u32).map(|i| (i % 256) as u8).collect();
    let long_result = "관측 ".repeat(8000);
    assert!(long.len() as u64 > 2 * CHUNK && std::str::from_utf8(&binary).is_err() && long_result.len() as u64 > CHUNK);
    let run = experiment(&mut r, &Spec {
        requirement: "R1", problem: "긴 관측 기록을 끝까지 읽을 수 있는가?",
        files: &[("evidence/long.md", long.as_bytes()), ("evidence/raw.bin", &binary)], evidence: &["evidence/long.md", "evidence/raw.bin"],
        result: long_result.trim_end(), interpretation: "기록의 길이와 내용이 보존됐다", lesson: "긴 기록도 원문으로 남는다", fails_back_to: None,
    });
    let (record_before, world_before) = (record_tree(&r.record), world(&r.root));

    let (text, encodings) = read_to_end(&mut r, &run.verify, "evidence/long.md", long.as_bytes());
    assert_eq!(text, long.as_bytes(), "the parts reassemble to the stored file byte for byte");
    assert!(encodings.len() >= 3 && encodings.iter().all(|encoding| encoding == "utf8"), "a cut never splits a character into non-text: {encodings:?}");
    let (raw, encodings) = read_to_end(&mut r, &run.verify, "evidence/raw.bin", &binary);
    assert_eq!(raw, binary);
    assert!(encodings.len() >= 2 && encodings.iter().all(|encoding| encoding == "base64"), "{encodings:?}");

    // A Report is one original: its long field arrives whole, with no elision marker.
    let verify = report(&mut r, &run.verify);
    assert_eq!(fields(&verify)["result"], long_result.trim_end());
    assert!(verify.get("truncated").is_none(), "{:?}", keys(&verify));
    // The Cycle index stayed short however long the Reports are: addresses, not bodies.
    let index = report(&mut r, &run.cycle);
    assert!(serde_json::to_vec(&index).unwrap().len() < 4096, "the Cycle index carries {} bytes", serde_json::to_vec(&index).unwrap().len());

    assert_eq!(record_tree(&r.record), record_before);
    assert_eq!(world(&r.root), world_before);
}

#[test]
fn a_chain_without_the_marker_keeps_its_responses_and_is_read_from_the_chain_after_it() {
    let mut r = Rig::start("evidence-queue", &[("data/in.txt", "input\n")]);
    // A record as a build without the exploration contract writes it: no marker line.
    as_item_queue_record(&r.record);
    r.restart();
    r.interview(&["A", "B"], 1, &[], "open_child");
    let opened = ok(r.model("gil_open", json!({"kind":"experiment"})), "queue experiment");
    let n = cycle_number(&opened);
    let bound = r.read();
    let document = bound["document"].as_str().unwrap().to_owned();
    let digest = bound["active_plan"]["digest"].as_str().unwrap().to_owned();
    let run = walk(&mut r, n, &Spec {
        requirement: "A", problem: "항목 A을 합성 자료로 확인한다",
        files: &[("evidence/A.md", "항목 A 결과\n".as_bytes())], evidence: &["evidence/A.md"],
        result: "항목 A를 저장했다", interpretation: "저장한 결과가 항목과 일치했다", lesson: "항목 A를 확인했다", fails_back_to: None,
    }, &document);

    // The item-queue responses are the ones that Chain always had: no contract, no requirement
    // list, and no pointer to tools its guidance never named.
    let queued = r.read();
    assert!(queued.get("contract").is_none() && queued.get("requirements").is_none(), "{queued}");
    let next = queued["next"].as_str().unwrap();
    assert!(next.contains("자동으로 묶인다") && !next.contains("gil_report_read") && !next.contains("gil_evidence_read"), "{next}");
    let context = r.context();
    assert!(!context.contains("원본 근거") && !context.contains("gil_report_read"), "{context}");
    assert!(!String::from_utf8(record_bytes(&r.record)).unwrap().contains("contract"), "the record gained no marker");

    // Its records are read all the same; the link is the queue binding, without a contract name.
    let cycle = report(&mut r, &run.cycle);
    assert_eq!(cycle["plan_link"], json!({"plan_revision":1,"plan_digest":digest,"item_id":"A"}));
    assert_eq!(cycle["chain"], json!({"ref":"chain:C1","state":"open"}));
    assert_eq!(fields(&report(&mut r, &run.verify))["result"], "항목 A를 저장했다");
    assert_eq!(evidence(&mut r, &run.verify, "evidence/A.md")["text"], "항목 A 결과\n");
    let before_close: Vec<Value> = [run.cycle.as_str(), run.define.as_str(), run.verify.as_str(), run.analysis.as_str(), run.outcome.as_str()]
        .iter().map(|reference| report(&mut r, reference)).collect();

    // Close that Chain through its user-approved Closing Interview and open the next one.
    ok(r.model("gil_open", json!({"kind":"interview"})), "closing interview");
    let closing = r.interview(&["A", "B"], 2, &[("chain_verdict", "failure")], "close_chain");
    ok(r.model("gil_close", json!({"report":format!("verdict: failure\nclosing_synthesis_ref: {closing}\nhandoff_summary: A는 확인했고 B는 남았다\n")})), "chain report");
    ok(r.model("gil_open", json!({"kind":"chain"})), "next chain");
    assert_eq!(yaml(&r.record)["chains"]["nodes"][1]["approved_plan"]["contract"], "exploration");
    let record_before = record_tree(&r.record);

    // From the new Chain: the current link list holds nothing of the earlier Chain …
    let now = r.read();
    assert_eq!(now["contract"], "exploration");
    assert!(now["requirements"].as_array().unwrap().is_empty() && now["executed_cycles"].as_array().unwrap().is_empty(), "{now}");
    // … the Chain Report read by its own address names where it was judged and what it handed on …
    let earlier = report(&mut r, "chain:C1");
    assert_eq!((earlier["chain"]["ref"].as_str(), earlier["chain"]["state"].as_str()), (Some("chain:C1"), Some("closed")));
    assert_eq!(fields(&earlier), BTreeMap::from([("verdict".to_string(), "failure".to_string()), ("closing_synthesis_ref".to_string(), closing.clone()), ("handoff_summary".to_string(), "A는 확인했고 B는 남았다".to_string())]));
    let found: Vec<(&str, &str, &Value)> = earlier["cycles"].as_array().unwrap().iter()
        .map(|c| (c["ref"].as_str().unwrap(), c["kind"].as_str().unwrap(), &c["plan_link"])).collect();
    assert_eq!(found[1], (run.cycle.as_str(), "experiment", &json!({"plan_revision":1,"plan_digest":digest,"item_id":"A"})), "{found:?}");
    assert_eq!(found.len(), 3, "the opening Interview, the Experiment and the Closing Interview: {found:?}");
    // … the approved closing Synthesis it points at is opened by that reference …
    let judged = fields(&report(&mut r, &closing));
    assert_eq!((judged["approved"].as_str(), judged["chain_verdict"].as_str()), ("yes", "failure"));
    // … and the earlier Experiment's originals are the same answers, now under a closed Chain.
    for (reference, before) in [run.cycle.as_str(), run.define.as_str(), run.verify.as_str(), run.analysis.as_str(), run.outcome.as_str()].iter().zip(&before_close) {
        let mut after = report(&mut r, reference);
        assert_eq!(after["chain"], json!({"ref":"chain:C1","state":"closed"}), "{reference}");
        after["chain"] = before["chain"].clone();
        // The closing Verify-less Interview confirmed no new world, so the comparison with the
        // current world is unchanged as well.
        assert_eq!(&after, before, "{reference}");
    }
    assert_eq!(evidence(&mut r, &run.verify, "evidence/A.md")["text"], "항목 A 결과\n");
    // A reference of the new Chain is its own record; the earlier Chain's is not offered for it.
    let here = report(&mut r, "chain:C2");
    assert_eq!((here["chain"]["parent"].as_str(), here["chain"]["state"].as_str(), &here["report"]), (Some("chain:C1"), Some("open"), &json!({"state":"not_written"})));
    assert_eq!(here["cycles"].as_array().unwrap().len(), 1, "{here}");
    assert_eq!(record_tree(&r.record), record_before, "reading across Chains changed the record");
}
