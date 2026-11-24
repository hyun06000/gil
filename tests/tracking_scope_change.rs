//! 구현 순서 5번 — 범위 변경 제안 → Monitor(App-only) 승인 → 승인 저장 → 기준점 보관 실패와 대기 → 명시 재시도 → 다음 Experiment,
//! 그리고 즉시 보관 중단·해제를 **실제 `gil` stdio 프로세스**로 확인한다(`tracking-policy` 개발 구성). 합성 임시 프로젝트만 쓴다.
//!
//! 여기의 App 호출은 시험 하네스의 입력이며 사람의 동의나 보이는 화면의 증거가 아니다 — 프로토콜과 기록의 연결만 증명한다.
//! 프로세스를 여러 번 새로 띄워 **앞 프로세스가 저장한 기록을 새 프로세스가 처음 읽는** 경우를 본다. 기준점 저장의 실패는 debug 빌드의
//! failpoint(`GIL_SCOPE_FAIL`)로 넣은 것이며, 강제 종료·전원 장애를 일으킨 것이 아니다. Interview 는 시험 프로세스가 공개 API 로 걷는다
//! (합성 Report — 승인 계획 프로필의 Core 는 맨 `approved: yes` 를 거절하므로 그 프로필에서는 이 시험이 컴파일되지 않는다. 그 프로필의
//! 0건은 실행 성공이 아니다).
#![cfg(all(feature = "tracking-policy", unix, not(feature = "approved-plan")))]
#[path = "support_wave1/mod.rs"]
mod support;
mod common;
use gil::{CycleKind, NodeKind};
use serde_json::{Value, json};
use support::*;

const P1: &str = "scratch/\n";
const P2: &str = "scratch/\ndata/\n";

fn sc(frame: &Value) -> &Value {
    &frame["result"]["structuredContent"]
}

fn is_error(frame: &Value) -> bool {
    frame["result"]["isError"] == json!(true)
}

fn meta<'a>(frame: &'a Value, key: &str) -> Option<&'a str> {
    frame["result"]["_meta"][key].as_str()
}

fn open(root: &std::path::Path) -> gil::ProjectSession {
    gil::ProjectSession::open(common::spec(), root.join(".gil/state.yaml")).expect("기록이 load 검사 전체를 통과한다")
}

/// 지금 열린 Interview 를 승인된 Synthesis 까지 걷고 `open_child` 방향으로 닫아 저장한다 — 시험 프로세스의 공개 API.
fn close_interview(root: &std::path::Path) {
    let mut session = open(root);
    let project = session.project_mut();
    for kind in [NodeKind::Question, NodeKind::Interpretation, NodeKind::Synthesis] {
        project.open_action_step(kind, common::plan(kind)).unwrap();
        let cycle = project.cycles().current();
        let mut report = common::full_report(cycle.rules(), CycleKind::Interview, kind);
        if kind == NodeKind::Synthesis {
            report.insert(common::APPROVED, common::APPROVAL);
            let basis: Vec<String> = cycle.steps().nodes().iter()
                .filter(|node| matches!(node.kind, NodeKind::Question | NodeKind::Interpretation) && node.is_closed())
                .map(|node| cycle.step_ref(node.id).to_string())
                .collect();
            report.insert(common::BASIS_REFS, basis.join("\n"));
        }
        project.close_action_step(report).unwrap();
    }
    let synthesis = project.cycles().current().step_ref(project.cycles().current().steps().current().unwrap());
    let outcome = common::opened(project, NodeKind::Outcome);
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Outcome)
        .with(common::SYNTHESIS_REF, synthesis.to_string())
        .with(common::REASON, "승인된 Synthesis 를 얻었다");
    project.close_action_step(report).unwrap();
    let mut report = common::cycle_report(project.cycles().current(), "success", common::step_id(project, outcome));
    report.insert(common::ACTION, "open_child");
    project.close_cycle(report).unwrap();
    session.commit().unwrap();
}

/// 실제 stdio 로 형식 10 을 시작하고(P1), 뿌리 Interview C1 을 닫아 검토 인터뷰의 경계에 선다. 돌려주는 것은 Monitor scope.
fn started(name: &str, settings: &std::path::Path) -> (std::path::PathBuf, String) {
    let root = scratch(name).canonicalize().unwrap();
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.txt"), "alpha\n").unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(root.join("data/raw.csv"), "1,2,3\n").unwrap();
    let r = root.display().to_string();
    let mut c = Client::serve(settings);
    let begun = c.call("gil_start", json!({"project_root": r}));
    assert!(!is_error(&begun), "{begun}");
    let scope = sc(&begun)["scope_id"].as_str().unwrap().to_owned();
    let prepared = c.call("gil_start_prepare", json!({"project_root": r, "rules": P1}));
    let (revision, digest) = (sc(&prepared)["proposal"]["revision"].clone(), sc(&prepared)["proposal"]["digest"].clone());
    let review = c.call("gil_start_review", json!({"scope_id": scope}));
    let token = meta(&review, "gil/startApprovalToken").expect("시작 승인 capability").to_owned();
    let decided = c.call("gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
    assert_eq!(sc(&decided)["phase"], "started", "{decided}");
    drop(c);
    close_interview(&root);
    (root, scope)
}

#[test]
fn an_approved_scope_change_waits_after_a_failed_archive_and_a_fresh_process_retries_it_explicitly_before_the_next_experiment() {
    let settings = scratch("scope-stdio-settings");
    let (root, scope) = started("scope-stdio", &settings);
    let r = root.display().to_string();
    let state = root.join(".gil/state.yaml");

    // ── 프로세스 1: 기준점 저장이 실패하는 구성. 제안 → App 승인 → 저장 1 은 성공, 보관은 실패 → 대기.
    let mut c = Client::serve_with_env(&settings, &[("GIL_SCOPE_FAIL", "baseline")]);
    let listed = c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let visibility = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name}"))["_meta"]["ui"]["visibility"].clone();
    for app in ["gil_scope_review", "gil_scope_decide", "gil_scope_retry", "gil_scope_halt", "gil_scope_release"] {
        assert_eq!(visibility(app), json!(["app"]), "{app}");
    }
    assert_eq!(visibility("gil_scope_propose"), json!(["model"]));
    assert_eq!(visibility("gil_scope_status"), json!(["model"]));

    let before = std::fs::read(&state).unwrap();
    let proposed = c.call("gil_scope_propose", json!({"project_root": r, "rules": P2}));
    assert!(!is_error(&proposed), "{proposed}");
    assert_eq!(sc(&proposed)["phase"], "awaiting_approval");
    assert_eq!(sc(&proposed)["proposal"]["newly_excluded"], json!(["data/raw.csv"]));
    assert_eq!(sc(&proposed)["scope_id"], json!(scope));
    assert_eq!(std::fs::read(&state).unwrap(), before, "제안은 상태를 바꾸지 않는다");
    let (revision, digest) = (sc(&proposed)["proposal"]["revision"].clone(), sc(&proposed)["proposal"]["digest"].as_str().unwrap().to_owned());

    // 모델 입력으로는 승인할 수 없고, 추측한 capability 는 거절된다. 어느 쪽도 기록을 바꾸지 않는다.
    let model = c.call("gil_scope_decide", json!({"project_root": r, "decision": "approve"}));
    assert!(model.get("error").is_some() || is_error(&model), "{model}");
    let forged = c.call("gil_scope_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": "f".repeat(64), "decision": "approve"}));
    assert!(is_error(&forged), "{forged}");
    assert_eq!(std::fs::read(&state).unwrap(), before);

    let review = c.call("gil_scope_review", json!({"scope_id": scope}));
    let token = meta(&review, "gil/scopeApprovalToken").expect("App 전용 승인 capability").to_owned();
    assert!(meta(&review, "gil/scopeRetryToken").is_none(), "대기가 아니면 재시도 capability 가 없다");
    assert!(!review["result"]["content"].to_string().contains(&token) && !sc(&review).to_string().contains(&token));
    let decided = c.call("gil_scope_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
    assert!(!is_error(&decided), "{decided}");
    assert_eq!(sc(&decided)["facts"]["approval_response"], "recorded");
    assert_eq!(sc(&decided)["facts"]["policy_applied"], true, "{decided}");
    assert_eq!(sc(&decided)["facts"]["policy_file"], "written");
    assert_eq!(sc(&decided)["facts"]["baseline"]["result"], "failed", "{decided}");
    assert_eq!(std::fs::read_to_string(root.join(".gilignore")).unwrap(), P2);
    assert!(!std::fs::read_to_string(&state).unwrap().contains(&token), "capability 는 기록에 없다");
    // 같은 capability 의 재전송은 거절되고, 상태는 그대로다.
    let waiting_state = std::fs::read(&state).unwrap();
    let resent = c.call("gil_scope_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
    assert!(is_error(&resent));
    assert_eq!(std::fs::read(&state).unwrap(), waiting_state);
    drop(c);

    // ── 프로세스 2(재시작, failpoint 없음): 저장된 대기 기록을 처음 읽는다. 조회·재연결은 시도를 만들지 않는다.
    let mut c = Client::serve(&settings);
    let status = c.call("gil_scope_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["tracking"]["applied_policy"], "policy:P2", "{status}");
    assert_eq!(sc(&status)["tracking"]["last_confirmed_world"], "snapshot:A1");
    assert_eq!(sc(&status)["tracking"]["confirmed_policy"], "policy:P1");
    assert_eq!(sc(&status)["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(sc(&status)["tracking"]["waiting"]["failed_attempts"], json!(["store_failure"]));
    let context = c.call("gil_context", json!({"project_root": r}));
    assert!(!is_error(&context), "{context}");
    assert!(context.to_string().contains("event:M2"), "context 가 보관 대기를 말한다");
    let shown = c.call("show_gil_monitor", json!({"scope_id": scope}));
    assert_eq!(sc(&shown)["scope_review_supported"], true, "{shown}");
    assert_eq!(sc(&shown)["tracking"]["now"]["applied_policy"], "policy:P2");
    assert_eq!(sc(&shown)["tracking"]["now"]["confirmed_policy"], "policy:P1");
    assert_eq!(std::fs::read(&state).unwrap(), waiting_state, "조회는 아무것도 바꾸지 않는다");
    drop(c);

    // 대기 중 일반 Interview C2 를 열고 닫는다(시험 프로세스) — Experiment 는 거절된다.
    {
        let mut session = open(&root);
        assert!(session.open_child_cycle(CycleKind::Experiment).is_err(), "다음 실험 보류");
        session.open_child_cycle(CycleKind::Interview).expect("인터뷰는 열린다");
        session.commit().unwrap();
    }
    close_interview(&root);

    // ── 프로세스 3: 지금 서 있는 경계 C2 에서 명시 재시도.
    let mut c = Client::serve(&settings);
    let review = c.call("gil_scope_review", json!({"scope_id": scope}));
    let retry = meta(&review, "gil/scopeRetryToken").expect("대기 중 닫힌 경계의 재시도 capability").to_owned();
    assert!(meta(&review, "gil/scopeApprovalToken").is_none(), "이미 적용된 제안에는 승인 capability 가 없다");
    let other = c.call("gil_scope_retry", json!({"scope_id": scope, "event": "event:M9", "token": retry}));
    assert!(is_error(&other), "다른 사건에는 쓰지 못한다: {other}");
    let retried = c.call("gil_scope_retry", json!({"scope_id": scope, "event": "event:M2", "token": retry}));
    assert!(!is_error(&retried), "{retried}");
    assert_eq!(sc(&retried)["facts"]["baseline"]["result"], "archived", "{retried}");
    assert_eq!(sc(&retried)["facts"]["baseline"]["snapshot"], "snapshot:A2");
    assert_eq!(sc(&retried)["facts"]["baseline"]["archived_at"], "cycle:C2");
    let after = c.call("gil_scope_review", json!({"scope_id": scope}));
    assert!(meta(&after, "gil/scopeRetryToken").is_none(), "확정된 기준점은 다시 확정하지 않는다");
    drop(c);

    // 다음 Experiment C3: 부모 C2, Entry 는 기준점 A2. 그 기록을 새 프로세스가 읽는다.
    {
        let mut session = open(&root);
        session.open_child_cycle(CycleKind::Experiment).expect("기준점 확정 뒤의 실험");
        let cycle = session.project().cycles().current();
        assert_eq!(cycle.parent().unwrap().to_ref().to_string(), "cycle:C2");
        assert_eq!(cycle.entry_snapshot().to_string(), "snapshot:A2");
        session.commit().unwrap();
    }
    let mut c = Client::serve(&settings);
    let status = c.call("gil_scope_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["tracking"]["standing"]["state"], "open_experiment", "{status}");
    assert_eq!(sc(&status)["tracking"]["last_confirmed_world"], "snapshot:A2");
    assert_eq!(sc(&status)["tracking"]["confirmed_policy"], "policy:P2");
    assert!(sc(&status)["tracking"]["waiting"].is_null());
    assert!(!is_error(&c.call("gil_context", json!({"project_root": r}))));
}

#[test]
fn a_halt_survives_a_restart_refuses_progress_and_is_released_only_with_the_explicit_confirmation() {
    let settings = scratch("scope-halt-settings");
    let (root, scope) = started("scope-halt", &settings);
    let r = root.display().to_string();
    let state = root.join(".gil/state.yaml");

    let mut c = Client::serve(&settings);
    let review = c.call("gil_scope_review", json!({"scope_id": scope}));
    let halt = meta(&review, "gil/scopeHaltToken").expect("중단 capability").to_owned();
    assert!(meta(&review, "gil/scopeReleaseToken").is_none());
    let halted = c.call("gil_scope_halt", json!({"scope_id": scope, "token": halt}));
    assert!(!is_error(&halted), "{halted}");
    assert_eq!(sc(&halted)["tracking"]["archive_halt"], "cycle:C1");
    let frozen = std::fs::read(&state).unwrap();
    // 표식이 있는 동안 제안·열기가 거절된다.
    let proposed = c.call("gil_scope_propose", json!({"project_root": r, "rules": P2}));
    assert!(is_error(&proposed) || sc(&proposed)["ok"] == false, "{proposed}");
    drop(c);
    assert!(open(&root).open_child_cycle(CycleKind::Interview).is_err(), "인터뷰 열기도 멈춘다");

    // 재시작·재연결로 풀리지 않는다. 중단 중에는 해제 capability 만 있다.
    let mut c = Client::serve(&settings);
    let status = c.call("gil_scope_status", json!({"project_root": r}));
    assert_eq!(sc(&status)["tracking"]["archive_halt"], "cycle:C1", "{status}");
    let review = c.call("gil_scope_review", json!({"scope_id": scope}));
    let release = meta(&review, "gil/scopeReleaseToken").expect("해제 capability").to_owned();
    assert!(meta(&review, "gil/scopeHaltToken").is_none() && meta(&review, "gil/scopeApprovalToken").is_none());
    // 상태 확인 없는 해제는 받지 않는다 — capability 도 소모되지 않는다.
    let unconfirmed = c.call("gil_scope_release", json!({"scope_id": scope, "at": "cycle:C1", "cleanup": "continue", "state_confirmed": false, "token": release}));
    assert!(is_error(&unconfirmed), "{unconfirmed}");
    assert_eq!(std::fs::read(&state).unwrap(), frozen);
    let released = c.call("gil_scope_release", json!({"scope_id": scope, "at": "cycle:C1", "cleanup": "continue", "state_confirmed": true, "token": release}));
    assert!(!is_error(&released), "{released}");
    assert!(sc(&released)["tracking"]["archive_halt"].is_null());
    drop(c);
    // 해제는 아무것도 진행시키지 않았다 — 사이클 수가 그대로이고, 그 뒤의 열기는 별도의 명시 행동이다.
    let mut session = open(&root);
    assert_eq!(session.project().cycles().nodes().len(), 1);
    session.open_child_cycle(CycleKind::Interview).expect("해제 뒤에는 기존 전이가 된다");
}
