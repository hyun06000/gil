//! Tool and visual-report paths on a Chain that really carries the exploration contract.
//!
//! Before this file those paths were fixed only elsewhere: `tests/mcp.rs` walks them in the
//! `chain-core` profile from a bare `approved: yes` fixture, and `tests/chain_monitor_fixtures.rs`
//! walks them on an item-queue Chain of the `approved-plan` build. Neither runs in an
//! `exploration-flow` build, and the item-queue suites that do run there start from a record
//! with the marker removed. Here every Project is started by this build, so its first Chain is
//! marked `contract: exploration`, and it is reached only through the opening Interview's
//! proposal, the App review and decision, and an Experiment opened with an explicit requirement
//! link (tests/support_wave1/exploration.rs). No ledger is edited and no marker is stripped.
//!
//! What this file fixes on such a Chain, over the real `gil mcp --serve` process:
//!
//! - a Verify keeps the visual declaration rules and the evidence rules: the plain text close,
//!   a one-word reason, an image listed as evidence but declared unnecessary, a missing or
//!   unchanged evidence file, an absent or altered approved document, a wrong owner and an
//!   unusable attachment are each refused, and each refusal leaves the record and the open
//!   Verify as they were;
//! - what a Verify attached must be cited by the Analysis and the Experiment Cycle Report of the
//!   same Cycle, may be cited by a later Define under the same requirement, and reads back the
//!   same — metadata and bytes — after the tool process restarts;
//! - a failed Experiment's visual evidence stays readable and citable after the approved revisit
//!   returned the working files, the attached image file included, to the target world;
//! - invalid input changes nothing and names a recovery this build can actually take: a manual
//!   topic that opens in this very process, and never the direct restore this build refuses.
//!
//! Not repeated here, because tests/exploration_flow.rs already fixes it: which links are
//! refused, the meaning of one success for a requirement, the restored world and preserved
//! bytes of a failure with and without file differences, and item-queue Chains in this build.
//! The schema-level shape errors of a typed close and the chunked transfer of a large image are
//! Chain-independent and stay with tests/mcp.rs.
//!
//! Scope of the evidence: synthetic tool-path checks. The App decisions are harness inputs, not
//! a person's click. A stored image and a successful read are not a Host screen showing it.
//! Whether a Define really is a yes/no proposition, or an image really shows the observed
//! result, is meaning the Core does not verify and this file does not claim.
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
use std::fs;
use std::path::PathBuf;
use support::*;

const MORE: &str = "더 알아보기";
const PLAIN_REASON: &str = "문서 증거만 필요한 합성 시험";

/// A one-pixel grayscale PNG; different shades are different assets.
fn png(shade: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    png::Encoder::new(&mut bytes, 1, 1).write_header().unwrap().write_image_data(&[shade]).unwrap();
    bytes
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn step(reference: &str) -> Value {
    json!({"kind":"step","ref":reference})
}
fn cycle(n: u32) -> Value {
    json!({"kind":"cycle","ref":format!("cycle:C{n}")})
}
/// A Verify Report naming the approved document and its evidence files, one per line.
fn verify_report(evidence: &[&str]) -> String {
    let files: String = evidence.iter().map(|path| format!("  {path}\n")).collect();
    format!("execution: 합성 자료를 관측했다\nresult: 관측한 값을 파일에 남겼다\nplan_document: {PLAN_DOCUMENT}\nevidence_files: |\n{files}")
}
/// Typed close arguments: a Report, the owner it is meant for, and its visuals.
fn typed(r: &Rig, report: &str, owner: &Value, visuals: Value) -> Value {
    json!({"project_root":r.root,"report":report,"expected_owner":owner,"visuals":visuals})
}
fn not_needed(reason: &str) -> Value {
    json!({"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":reason}})
}
/// One new PNG at `path`, attached and used.
fn attached(path: &str) -> Value {
    json!({"attachments":[{"key":"chart","path":path,"source_kind":"generated","source_description":"합성 1픽셀 PNG, 실제 화면 캡처가 아니다"}],
        "uses":[{"source":{"kind":"new","key":"chart"},"caption":"관측한 값","alt":"회색 픽셀 하나","purpose":"관측 결과 제시"}],
        "declaration":{"state":"attached"}})
}
/// A citation of an already confirmed asset, with this Report's own caption.
fn citing(owner: &Value, asset: &Value, caption: &str) -> Value {
    json!({"attachments":[],"uses":[{"source":{"kind":"stored","owner":owner,"asset_id":asset["asset_id"],"sha256":asset["sha256"]},
        "caption":caption,"alt":"회색 픽셀 하나","purpose":"근거 인용"}]})
}
fn code(frame: &Value) -> &str {
    structured(frame)["code"].as_str().unwrap_or("")
}
/// The whole text of a reply, guidance blocks included.
fn everything(frame: &Value) -> String {
    let mut text: Vec<String> = frame["result"]["content"].as_array().map(|c| c.iter().filter_map(|b| b["text"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
    for key in ["said", "problem"] {
        if let Some(more) = structured(frame)[key].as_str() { text.push(more.to_owned()); }
    }
    text.join("\n")
}
/// The one manual topic a refusal points at, if it points at any.
fn pointed_at(frame: &Value) -> Option<String> {
    let text = everything(frame);
    let block = &text[text.find(MORE)?..];
    let line = block.lines().nth(1).unwrap_or_default().trim().strip_prefix("gil help ").unwrap_or_else(|| panic!("no lookup after `{MORE}`: {text}"));
    assert!(!line.contains(' '), "one address: {line:?}");
    Some(line.to_owned())
}
/// The topic exists in this build and opens through this process; reading it writes nothing.
fn topic_opens(r: &mut Rig, topic: &str) -> String {
    let before = record_bytes(&r.record);
    let body = everything(&ok(r.model("gil_help", json!({"topic":topic})), topic));
    assert!(body.trim().lines().count() > 3, "{topic} has a body: {body}");
    assert_eq!(record_bytes(&r.record), before, "reading {topic} changed the record");
    body
}
/// The confirmed (or still unconfirmed) visuals of one Report, as the model-side read gives them.
fn visuals_of(r: &mut Rig, owner: &Value) -> Value {
    structured(&ok(r.model("gil_report_visuals", json!({"owner":owner})), "gil_report_visuals")).clone()
}
/// The stored bytes of one asset, read through the Monitor scope the way the App reads them.
fn image(r: &mut Rig, owner: &Value, asset: &Value) -> Vec<u8> {
    let got = ok(r.app("gil_monitor_report_image", json!({"owner":owner,"asset_id":asset["asset_id"],"offset":0})), "report image");
    let data = structured(&got);
    assert_eq!(data["sha256"], asset["sha256"], "{got}");
    assert!(data["next_offset"].is_null(), "a one-pixel image is one chunk: {got}");
    base64::engine::general_purpose::STANDARD.decode(data["data_base64"].as_str().unwrap()).unwrap()
}
/// The open place, as the status read says it.
fn status(r: &mut Rig) -> String {
    said(&ok(r.model("gil_status", json!({})), "status"))
}
/// One linked Experiment whose Verify attaches `chart`, and whose Analysis and Cycle Report cite
/// it. Returns the Cycle number, the Verify owner and the stored asset.
fn visual_experiment(r: &mut Rig, requirement: &str, problem: &str, files: &[(&str, &[u8])], chart: &str, fails_back_to: Option<&str>) -> (u32, Value, Value) {
    let n = r.open_experiment(requirement);
    let verify = step(&r.open_verify(requirement, problem));
    for (path, bytes) in files {
        r.write_bytes(path, bytes);
    }
    let evidence: Vec<&str> = files.iter().map(|(path, _)| *path).collect();
    let closed = ok(r.c.call("gil_close", typed(r, &verify_report(&evidence), &verify, attached(chart))), "close verify with an image");
    let asset = structured(&closed)["visuals"]["uses"][0].clone();
    let failed = fails_back_to.is_some();
    let analysis = step(&step_ref(&ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis"), "analysis"));
    ok(r.c.call("gil_close", typed(r, &analysis_report(failed), &analysis, citing(&verify, &asset, "해석의 근거"))), "close analysis citing the image");
    let outcome = step_ref(&ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome"), "outcome");
    ok(r.model("gil_close", json!({"report":outcome_report(failed)})), "close outcome");
    ok(r.c.call("gil_close", typed(r, &cycle_report(n, &outcome, fails_back_to), &cycle(n), citing(&verify, &asset, "인계의 근거"))), "close the Cycle citing the image");
    (n, verify, asset)
}

#[test]
fn a_verify_of_an_exploration_chain_keeps_the_visual_declaration_and_evidence_rules() {
    let mut r = Rig::start("explore-verify-rules", &[("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")]);
    assert_eq!(yaml(&r.record)["chains"]["nodes"][0]["approved_plan"]["contract"], "exploration", "this build's own Chain, not a rewritten record");
    r.interview(&["R1"], 1, &[], "open_child");
    assert_eq!(r.open_experiment("R1"), 2);
    let verify = r.open_verify("R1", "지정한 자료에 허용 날짜 형식(YYYY-MM-DD)을 벗어나는 값이 존재하는가?");
    let owner = step(&verify);
    r.write("evidence/C2.md", "2행이 DD/MM/YYYY 형식이다: 존재한다\n");
    r.write_bytes("report/irregular.png", &png(91));
    r.write("report/not-an-image.png", "PNG가 아닌 글자\n");
    r.write("plan/summary.md", "승인 계획의 요약본\n");
    let before = record_bytes(&r.record);
    let documents = verify_report(&["evidence/C2.md"]);

    // The declaration is required: the plain text close is the wrong shape for this Verify.
    let plain = refused(r.model("gil_close", json!({"report":documents})), "plain text close of a visual Verify");
    let told = everything(&plain);
    for needle in ["시각 자료 선언이 필요하다", "expected_owner", verify.as_str(), "not_needed"] {
        assert!(told.contains(needle), "{needle}: {told}");
    }
    assert_eq!(pointed_at(&plain).as_deref(), Some("report/visuals"), "{told}");
    // One half of the typed shape is a schema error that names what is missing and closes nothing.
    let half = rpc_error(r.model("gil_close", json!({"report":documents,"expected_owner":owner})), "expected_owner without visuals");
    assert_eq!(half["error"]["code"], -32602);
    let message = half["error"]["message"].as_str().unwrap();
    assert!(message.contains("without 'visuals'") && message.contains("Nothing was closed") && !message.contains(r.root.to_str().unwrap()), "{message}");
    let unfinished = rpc_error(r.c.call("gil_close", typed(&r, &documents, &owner, json!({"attachments":[],"uses":[],"declaration":{"state":"missing","reason":"렌더러가 준비되지 않았다"}}))), "missing without a follow-up");
    assert!(unfinished["error"]["message"].as_str().unwrap().contains("follow_up"), "{unfinished}");

    // The declaration has substance and does not contradict the evidence.
    let cases: Vec<(&str, Value, &str, &str)> = vec![
        ("a one-word reason", typed(&r, &documents, &owner, not_needed("없음")), "visual_reason_weak", "두 단어 이상"),
        ("an image listed as evidence but declared unnecessary", typed(&r, &verify_report(&["evidence/C2.md", "report/irregular.png"]), &owner, not_needed(PLAIN_REASON)), "visual_declaration_conflict", "report/irregular.png"),
        ("an attachment outside the project", typed(&r, &documents, &owner, attached("../outside.png")), "visual_rejected", "attachments[0].path"),
        ("an attachment that is not a PNG", typed(&r, &documents, &owner, attached("report/not-an-image.png")), "visual_rejected", "정적 PNG"),
        // The evidence rules of the approved plan hold next to a valid declaration.
        ("an evidence file that does not exist", typed(&r, &verify_report(&["evidence/none.md"]), &owner, not_needed(PLAIN_REASON)), "verify_evidence_required", "evidence/none.md"),
        ("only a file unchanged since the previous world", typed(&r, &verify_report(&["data/dates.csv"]), &owner, not_needed(PLAIN_REASON)), "verify_evidence_required", ""),
        ("no approved document", typed(&r, "execution: 관측했다\nresult: 남겼다\nevidence_files: evidence/C2.md\n", &owner, not_needed(PLAIN_REASON)), "verify_evidence_required", "승인 계획"),
        ("a summary instead of the approved document", typed(&r, &documents.replace(PLAN_DOCUMENT, "plan/summary.md"), &owner, not_needed(PLAIN_REASON)), "verify_evidence_required", "gil_plan_read"),
        // The Report grammar and the owner are checked on the typed path too.
        ("a Report without its execution", typed(&r, &documents.replace("execution: 합성 자료를 관측했다\n", ""), &owner, not_needed(PLAIN_REASON)), "close_not_confirmed", "execution"),
        ("another Step as the owner", typed(&r, &documents, &step("step:C2/S2"), not_needed(PLAIN_REASON)), "owner_mismatch", verify.as_str()),
    ];
    for (what, args, expected, names) in cases {
        let got = refused(r.c.call("gil_close", args), what);
        assert_eq!(code(&got), expected, "{what}: {got}");
        assert!(everything(&got).contains(names), "{what} names {names:?}: {got}");
        if expected.starts_with("visual_") {
            assert!(everything(&got).contains("report/visuals"), "{what} names the visual topic: {got}");
        }
        assert_eq!(record_bytes(&r.record), before, "{what} changed the record");
    }
    // Every refusal left the same Verify open and unconfirmed; the topic they name opens here.
    assert_eq!(visuals_of(&mut r, &owner)["confirmed"], false);
    assert!(status(&mut r).contains("verify (open)"));
    assert!(topic_opens(&mut r, "report/visuals").contains("visual_citation_required"));
    assert_eq!(record_bytes(&r.record), before);

    // The corrected close: the image is attached, the documents are evidence.
    fs::remove_file(r.root.join("report/not-an-image.png")).unwrap();
    fs::remove_file(r.root.join("plan/summary.md")).unwrap();
    let args = typed(&r, &verify_report(&["evidence/C2.md", "report/irregular.png"]), &owner, attached("report/irregular.png"));
    let receipt = ok(r.c.call("gil_close", args.clone()), "attached close");
    let confirmed = structured(&receipt).clone();
    assert_eq!((&confirmed["confirmed"], &confirmed["visuals"]["state"]), (&json!(true), &json!("attached")), "{receipt}");
    assert_eq!(confirmed["display_confirmed"], false, "a stored image is not a displayed image");
    let asset = confirmed["visuals"]["uses"][0].clone();
    assert_eq!(asset["origin_step_ref"], json!(verify));
    assert_eq!(asset["sha256"], json!(sha256(&png(91))));
    // A lost receipt is recovered by reading, never by closing twice.
    let after = record_bytes(&r.record);
    assert_eq!(code(&refused(r.c.call("gil_close", args), "the same close again")), "owner_mismatch");
    assert_eq!(record_bytes(&r.record), after);
    assert_eq!(visuals_of(&mut r, &owner), confirmed);
    assert_eq!(image(&mut r, &owner, &asset), png(91));
    // The Verify closed inside the linked Experiment; the requirement is still only linked.
    assert_eq!(r.read()["current_experiment"]["requirement_id"], "R1");
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "open".to_owned())]);
}

#[test]
fn attached_visuals_are_cited_by_later_reports_and_read_the_same_after_a_restart() {
    let mut r = Rig::start("explore-visual-citation", &[("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")]);
    r.interview(&["R1", "R2"], 1, &[], "open_child");
    assert_eq!(r.open_experiment("R1"), 2);
    let verify = step(&r.open_verify("R1", "지정한 자료에 허용 날짜 형식을 벗어나는 값이 존재하는가?"));
    r.write("evidence/C2.md", "2행이 DD/MM/YYYY 형식이다: 존재한다\n");
    r.write_bytes("report/irregular.png", &png(91));
    let closed = ok(r.c.call("gil_close", typed(&r, &verify_report(&["evidence/C2.md", "report/irregular.png"]), &verify, attached("report/irregular.png"))), "verify");
    let asset = structured(&closed)["visuals"]["uses"][0].clone();

    // The Analysis must cite what its Verify attached. The refusal hands over the citable values.
    let analysis = step(&step_ref(&ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis"), "analysis"));
    let before = record_bytes(&r.record);
    let uncited = refused(r.model("gil_close", json!({"report":analysis_report(false)})), "an Analysis that cites nothing");
    let told = everything(&uncited);
    for needle in ["visual_citation_required", asset["asset_id"].as_str().unwrap(), asset["sha256"].as_str().unwrap(), verify["ref"].as_str().unwrap()] {
        assert!(told.contains(needle), "{needle}: {told}");
    }
    assert_eq!(pointed_at(&uncited).as_deref(), Some("report/visuals"), "{told}");
    let mut forged = citing(&verify, &asset, "해석의 근거");
    forged["uses"][0]["source"]["sha256"] = json!("0".repeat(64));
    assert_eq!(code(&refused(r.c.call("gil_close", typed(&r, &analysis_report(false), &analysis, forged)), "a citation of bytes that were never stored")), "visual_unavailable");
    assert_eq!(record_bytes(&r.record), before, "a refused Analysis changed the record");
    let cited = ok(r.c.call("gil_close", typed(&r, &analysis_report(false), &analysis, citing(&verify, &asset, "해석의 근거"))), "the Analysis citing the image");
    assert_eq!(structured(&cited)["visuals"]["uses"][0]["caption"], "해석의 근거");
    assert_eq!(structured(&cited)["visuals"]["uses"][0]["origin_step_ref"], verify["ref"]);

    // The Outcome may stay plain; the Experiment Cycle Report must cite.
    let opened = ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome");
    let outcome = step_ref(&opened, "outcome");
    ok(r.model("gil_close", json!({"report":outcome_report(false)})), "a plain Outcome");
    let before = record_bytes(&r.record);
    let report = cycle_report(2, &outcome, None);
    let uncited = refused(r.model("gil_close", json!({"report":report})), "a Cycle Report that cites nothing");
    assert!(everything(&uncited).contains("visual_citation_required") && everything(&uncited).contains(asset["sha256"].as_str().unwrap()), "{uncited}");
    assert_eq!(pointed_at(&uncited).as_deref(), Some("report/visuals"));
    assert_eq!(record_bytes(&r.record), before, "a refused Cycle Report changed the record");
    ok(r.c.call("gil_close", typed(&r, &report, &cycle(2), citing(&verify, &asset, "인계의 근거"))), "the Cycle Report citing the image");

    // The cited success is one linked Experiment, not the requirement's completion or a new approval.
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "success".to_owned())]);
    assert_eq!(r.experiments("R2"), vec![]);
    let record = yaml(&r.record);
    assert_eq!(record["chains"]["nodes"][0]["state"], "open");
    assert_eq!(record["chains"]["nodes"][0]["approved_plan"]["proposals"].as_array().unwrap().len(), 1);

    // A different small Define under the same requirement cites the earlier Cycle's image.
    assert_eq!(r.open_experiment("R1"), 3);
    let define = step(&step_ref(&ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define"), "define"));
    let define_report = "problem: 허용 형식을 벗어난 모든 값을 정보 손실 없이 YYYY-MM-DD로 변환할 수 있는가?\nsuccess_condition: 변환 결과의 관측 증거가 위 명제의 긍정 답을 뒷받침한다\n";
    let reused = ok(r.c.call("gil_close", typed(&r, define_report, &define, citing(&verify, &asset, "앞선 관측에서 출발한다"))), "a Define citing the earlier image");
    assert_eq!(structured(&reused)["visuals"]["uses"][0]["origin_step_ref"], verify["ref"]);
    assert_eq!(r.read()["current_experiment"]["cycle_ref"], "cycle:C3");

    // Everything reads the same from a new process: Reports, their citations, the stored bytes.
    let owners = [verify.clone(), analysis.clone(), step(&outcome), cycle(2), define.clone()];
    let reads = |r: &mut Rig| -> Vec<Value> {
        let mut seen: Vec<Value> = owners.iter().map(|owner| visuals_of(r, owner)).collect();
        for tool in ["gil_status", "gil_story", "gil_context", "gil_plan_read"] {
            seen.push(ok(r.model(tool, json!({})), tool)["result"].clone());
        }
        // The Monitor view of the same record, without the moment it was captured.
        let mut view = structured(&ok(r.app("gil_monitor_read", json!({})), "monitor read"))["view"].clone();
        assert!(view.as_object_mut().expect("Monitor view").remove("captured_at_unix_ms").is_some(), "{view}");
        seen.push(view);
        seen.push(structured(&ok(r.app("gil_monitor_report_visuals", json!({"owner":cycle(2)})), "monitor visuals")).clone());
        seen
    };
    let (seen, bytes) = (reads(&mut r), record_bytes(&r.record));
    assert_eq!(seen[0]["visuals"]["state"], "attached");
    for (at, caption) in [(1, "해석의 근거"), (3, "인계의 근거"), (4, "앞선 관측에서 출발한다")] {
        assert_eq!((&seen[at]["confirmed"], &seen[at]["visuals"]["state"], &seen[at]["visuals"]["uses"][0]["caption"]), (&json!(true), &json!("references"), &json!(caption)), "{}", seen[at]);
        assert_eq!(seen[at]["visuals"]["uses"][0]["sha256"], asset["sha256"]);
    }
    assert_eq!(seen[2]["visuals"]["uses"], json!([]), "the plain Outcome cites nothing: {}", seen[2]);
    r.restart();
    for (at, (again, first)) in reads(&mut r).iter().zip(&seen).enumerate() {
        assert_eq!(again, first, "read {at} means something else after a restart");
    }
    assert_eq!(image(&mut r, &verify, &asset), png(91));
    assert_eq!(image(&mut r, &cycle(2), &asset), png(91), "a citing Report serves the same stored bytes");
    assert_eq!(record_bytes(&r.record), bytes, "restarting or reading changed the record");
}

#[test]
fn a_failed_experiments_visual_evidence_survives_the_approved_revisit_and_stays_citable() {
    let mut r = Rig::start("explore-visual-failure", &[("data/dates.csv", "id,date\n1,2026-01-02\n2,02/01/2026\n")]);
    r.interview(&["R1"], 1, &[], "open_child");
    let kept = png(40);
    let (c2, verify2, asset2) = visual_experiment(&mut r, "R1", "허용 날짜 형식을 벗어나는 값이 존재하는가?",
        &[("evidence/C2.md", "벗어나는 값 1건\n".as_bytes()), ("report/chart.png", &kept)], "report/chart.png", None);
    let world_at_c2 = world(&r.root);

    // C3 replaces the same image file with its own result, attaches it, and fails.
    let lost = png(200);
    let (c3, verify3, asset3) = visual_experiment(&mut r, "R1", "대상 파일의 모든 날짜를 합의한 규칙으로 공통 형식에 변환할 수 있는가?",
        &[("evidence/C3.md", "2행을 변환하지 못했다: 결측 규칙이 없다\n".as_bytes()), ("report/chart.png", &lost)], "report/chart.png", Some("cycle:C2"));
    assert_eq!((c2, c3), (2, 3));
    assert_ne!(asset2["sha256"], asset3["sha256"]);
    assert_eq!(world(&r.root)["report/chart.png"], lost);
    let failed_reads: Vec<Value> = [verify3.clone(), cycle(3)].iter().map(|owner| visuals_of(&mut r, owner)).collect();
    assert_eq!(failed_reads[0]["visuals"]["state"], "attached");

    // Until the approved revisit nothing opens. Each refusal names the next step, and the step
    // it finally names is the Monitor approval — the only recovery this build performs.
    let (record_before, world_before) = (record_bytes(&r.record), world(&r.root));
    let link = r.link("R1");
    let blocked = refused(r.model("gil_open", json!({"kind":"experiment","contract":link})), "the next Experiment before the revisit");
    assert!(everything(&blocked).contains("revisit"), "{blocked}");
    let waiting = refused(r.model("gil_revisit", json!({})), "the model-side revisit without an approval");
    assert!(everything(&waiting).contains("보존 후 되돌아가기") && everything(&waiting).contains("바꾸지 않았다"), "{waiting}");
    assert!(r.context().contains("보존 후 되돌아가기"));
    assert_eq!((record_bytes(&r.record), world(&r.root)), (record_before, world_before));

    // The revisit returns the whole tracked world: the image file is C2's again.
    let view = r.recover();
    assert_eq!((view["from"].as_str(), view["target"].as_str()), (Some("cycle:C3"), Some("cycle:C2")));
    let listed: Vec<(&str, &str)> = view["changes"].as_array().unwrap().iter().map(|c| (c["path"].as_str().unwrap(), c["action"].as_str().unwrap())).collect();
    assert!(listed.contains(&("report/chart.png", "replace")) && listed.contains(&("evidence/C3.md", "preserve")), "{view}");
    assert_eq!(world(&r.root), world_at_c2);
    assert_eq!(world(&r.root)["report/chart.png"], kept);

    // The failure's visual evidence stays: its Reports read as before, its bytes are served,
    // and the replaced working file is in the preserved copy as well.
    let after: Vec<Value> = [verify3.clone(), cycle(3)].iter().map(|owner| visuals_of(&mut r, owner)).collect();
    assert_eq!(after, failed_reads, "the revisit rewrote a failed Report's visuals");
    assert_eq!(image(&mut r, &verify3, &asset3), lost);
    assert_eq!(image(&mut r, &verify2, &asset2), kept);
    let preserved = PathBuf::from(view["archive"].as_str().unwrap());
    let mapping: Value = serde_json::from_slice(&fs::read(preserved.join("files.json")).unwrap()).unwrap();
    let saved = mapping.as_array().unwrap().iter().find(|m| m["path"] == "report/chart.png").expect("the replaced image is preserved");
    assert_eq!(fs::read(preserved.join(saved["saved_as"].as_str().unwrap())).unwrap(), lost);
    assert_eq!(r.experiments("R1"), vec![("cycle:C2".to_owned(), "success".to_owned()), ("cycle:C3".to_owned(), "failure".to_owned())]);

    // After a restart the next small Define under the same requirement cites the failed
    // observation as its ground. The learning is usable; the failed files are not in the world.
    r.restart();
    assert_eq!(image(&mut r, &verify3, &asset3), lost);
    assert_eq!(r.open_experiment("R1"), 4);
    assert_eq!(r.read()["current_experiment"]["revisit_from"], "cycle:C3");
    let define = step(&step_ref(&ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define"), "define"));
    let reused = ok(r.c.call("gil_close", typed(&r, "problem: 대상 자료의 날짜 열에 결측값이 존재하는가?\nsuccess_condition: 결측값의 관측 증거가 위 명제의 긍정 답을 뒷받침한다\n", &define, citing(&verify3, &asset3, "실패한 변환에서 본 것"))), "a Define citing the failed Cycle's image");
    assert_eq!(structured(&reused)["visuals"]["uses"][0]["origin_step_ref"], verify3["ref"]);
    assert_eq!(structured(&reused)["visuals"]["uses"][0]["sha256"], json!(sha256(&lost)));
    assert_eq!(world(&r.root), world_at_c2, "citing stored evidence brings no file back");
}

#[test]
fn invalid_input_changes_nothing_and_names_a_recovery_this_build_can_take() {
    let mut r = Rig::start("explore-invalid-input", &[("data/in.txt", "input\n")]);
    let mut topics: Vec<String> = Vec::new();
    let mut note = |frame: &Value, expected: Option<&str>, what: &str| {
        let topic = pointed_at(frame);
        assert_eq!(topic.as_deref(), expected, "{what}: {frame}");
        topics.extend(topic);
    };

    // A started Project is not started again; the refusal sends the caller to the record.
    let before = record_bytes(&r.record);
    let again = refused(r.model("gil_start", json!({})), "a second start");
    assert!(everything(&again).contains("gil_context"), "{again}");
    note(&refused(r.model("gil_open", json!({"kind":"question"})), "a Step without its action contract"), Some("action/open-contract"), "question without a contract");
    assert_eq!(record_bytes(&r.record), before);

    r.interview(&["R1"], 1, &[], "open_child");
    assert_eq!(r.open_experiment("R1"), 2);
    let before = record_bytes(&r.record);
    note(&refused(r.model("gil_open", json!({"kind":"define"})), "a Define without its action contract"), Some("action/open-contract"), "define without a contract");
    let unknown = refused(r.model("gil_open", json!({"kind":"reflection","contract":CONTRACT})), "a Step kind the grammar does not have");
    assert!(everything(&unknown).contains("define"), "the refusal names what can open here: {unknown}");
    note(&unknown, None, "an unknown kind");
    let early = refused(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "a Verify before Define and Hypothesis");
    assert!(everything(&early).contains("define"), "{early}");
    note(&early, None, "a Step out of order");
    assert_eq!(record_bytes(&r.record), before);

    // A file written outside a Verify cannot be confirmed by a Define. This build refuses the
    // direct restore, so the refusal must not offer it: it tells the person to clear the change.
    ok(r.model("gil_open", json!({"kind":"define","contract":CONTRACT})), "define");
    r.write("stray.txt", "Verify 밖에서 쓴 파일\n");
    let (before, files) = (record_bytes(&r.record), world(&r.root));
    let define_report = "problem: 입력 파일이 비어 있는가?\nsuccess_condition: 입력 파일의 관측 증거가 위 명제의 긍정 답을 뒷받침한다\n";
    let dirty = refused(r.model("gil_close", json!({"report":define_report})), "a Define close over a changed world");
    let told = everything(&dirty);
    for needle in ["Artifact 변경을 확정할 수 없다", "stray.txt", "이 빌드는 Artifact 되돌리기(restore)를 거부한다", "프로젝트 밖의 새 보존 위치로 옮긴다", "그대로 열려 있다"] {
        assert!(told.contains(needle), "{needle}: {told}");
    }
    assert!(!told.contains("`gil restore` 로"), "a refused command is not the recovery: {told}");
    note(&dirty, Some("artifact/dirty/non-verify"), "a dirty non-Verify close");
    let restore = refused(r.model("gil_restore", json!({})), "the direct restore");
    assert!(everything(&restore).contains("이 제품판에서는") && everything(&restore).contains("한 글자도 바꾸지 않았다"), "{restore}");
    let suggested = everything(&ok(r.model("gil_help", json!({})), "state-aware help"));
    assert!(suggested.contains("artifact/dirty/non-verify") && suggested.contains("artifact/restore"), "{suggested}");
    assert!(status(&mut r).contains("dirty"));
    assert_eq!((record_bytes(&r.record), world(&r.root)), (before, files), "a refusal or a read changed the record or the files");
    // The named recovery works: the new file leaves the project, then the same close is accepted.
    fs::rename(r.root.join("stray.txt"), r.settings.join("stray-kept.txt")).unwrap();
    ok(r.model("gil_close", json!({"report":define_report})), "the same Define close after clearing the change");

    // An Experiment Cycle Report: a contract error and a missing revisit target are two topics.
    ok(r.model("gil_open", json!({"kind":"hypothesis","contract":CONTRACT})), "hypothesis");
    ok(r.model("gil_close", json!({"report":"hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n"})), "close hypothesis");
    let verify = step_ref(&ok(r.model("gil_open", json!({"kind":"verify","contract":CONTRACT})), "verify"), "verify");
    let document = r.read()["document"].as_str().unwrap().to_owned();
    r.write(PLAN_DOCUMENT, &document);
    r.write("evidence/C2.md", "입력 파일은 한 줄이다\n");
    ok(r.c.call("gil_close", typed(&r, &verify_report(&["evidence/C2.md"]), &step(&verify), not_needed(PLAIN_REASON))), "close verify");
    ok(r.model("gil_open", json!({"kind":"analysis","contract":CONTRACT})), "analysis");
    ok(r.model("gil_close", json!({"report":analysis_report(true)})), "close analysis");
    let outcome = step_ref(&ok(r.model("gil_open", json!({"kind":"outcome","contract":CONTRACT})), "outcome"), "outcome");
    ok(r.model("gil_close", json!({"report":outcome_report(true)})), "close outcome");
    let before = record_bytes(&r.record);
    let good = cycle_report(2, &outcome, Some("cycle:C1"));
    note(&refused(r.model("gil_close", json!({"report":good.replace("verdict: failure\n", "")})), "a Cycle Report without a verdict"), Some("cycle/experiment/close"), "no verdict");
    note(&refused(r.model("gil_close", json!({"report":good.replace("failure", "success")})), "a Cycle verdict that contradicts its Outcome"), Some("cycle/experiment/close"), "contradicting verdict");
    note(&refused(r.model("gil_close", json!({"report":good.replace("next_direction.target_cycle_ref: cycle:C1\n", "")})), "a revisit without a target"), Some("cycle/revisit/target"), "no target");
    note(&refused(r.model("gil_close", json!({"report":good.replace("cycle:C1", "cycle:C9")})), "a revisit to a Cycle that does not exist"), Some("cycle/revisit/target"), "unknown target");
    assert_eq!(record_bytes(&r.record), before, "a refused Cycle Report changed the record");
    let closed = ok(r.model("gil_close", json!({"report":good})), "the corrected Cycle Report");
    assert!(said(&closed).contains("cycle:C2 · experiment · failure") && said(&closed).contains("실행 계약: 탐색"), "{closed}");

    // Every topic a refusal named exists in this build, and the two recovery topics describe
    // this build: the person clears a dirty world, and the direct restore is refused.
    topics.sort();
    topics.dedup();
    assert_eq!(topics, ["action/open-contract", "artifact/dirty/non-verify", "cycle/experiment/close", "cycle/revisit/target"]);
    for topic in &topics {
        topic_opens(&mut r, topic);
    }
    assert!(topic_opens(&mut r, "artifact/dirty/non-verify").contains("이 빌드는 Artifact 되돌리기(restore)를 거부한다"));
    assert!(topic_opens(&mut r, "artifact/restore").contains("이 제품판은 gil restore 를 거절"));
    let before = record_bytes(&r.record);
    refused(r.model("gil_help", json!({"topic":"no/such/topic"})), "a topic this build does not carry");
    refused(r.model("gil_help", json!({"topic":"not a topic"})), "something that is not a topic address");
    assert_eq!(record_bytes(&r.record), before);
}
