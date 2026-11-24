//! 범위 변경 사건의 제품 연결 시험 — **합성 임시 폴더**와 그 안의 실제 `.gil/state.yaml`·객체 창고만 쓴다(실제 사용자 프로젝트·소스
//! 저장소의 `gil start` 없음). Monitor 버튼은 capability 검사를 마친 뒤 부르는 [`decide`]·[`retry`]·[`halt`]·[`release`] 로 흉내 낸다 —
//! 시험의 입력이며 사람의 승인 증거가 아니다. capability 의 경계(app-only·메모리·대상 결속)는 `src/mcp/scope_tests.rs` 가 본다.
//!
//! 증거의 종류를 섞지 않는다: 여기의 실패는 **함수 수준의 실패 주입**(`SAVE_FAULT`·`WRITE_FAULT`·`REREAD_FAULT`·`AFTER_APPROVAL`)이고,
//! 저장은 **실제 파일 저장과 같은 프로세스 안의 다시 읽기**(매번 `ProjectSession::open` 이 load 의 검사 전체를 지난다)다. 새 프로세스의
//! 재열기는 `tests/tracking_scope_change.rs` 가, 강제 종료·전원 장애는 어느 시험도 하지 않는다.
//!
//! 사이클 번호는 계약 §6.5 의 예(C4·C5·C6)와 다르다 — 최초 Interview 가 C1 이므로 검토 인터뷰 C1, 대기 중 인터뷰 C2, 다음 실험 C3 이다.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::*;
use crate::{CycleKind, NodeKind};

#[path = "../../tests/common/mod.rs"]
mod common;

const P1: &str = "scratch/\n";
/// `data/` 를 더 제외한다.
const P2: &str = "scratch/\ndata/\n";

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-scope-change-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험 폴더");
    dir.canonicalize().expect("실제 경로")
}

fn s(root: &Path) -> String {
    root.display().to_string()
}

fn open_session(root: &Path) -> ProjectSession {
    ProjectSession::open(crate::RuleSet::builtin().unwrap(), root.join(".gil/state.yaml")).expect("기록이 load 검사 전체를 통과한다")
}

/// 지금 열린 Interview 를 Question → Interpretation → 승인된 Synthesis → Outcome 으로 걷고 `open_child` 방향으로 닫아 저장한다.
fn close_interview(root: &Path) {
    let mut session = open_session(root);
    let project = session.project_mut();
    for kind in [NodeKind::Question, NodeKind::Interpretation, NodeKind::Synthesis] {
        project.open_action_step(kind, common::plan(kind)).unwrap_or_else(|err| panic!("{kind} 를 열지 못했다: {err}"));
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
        project.close_action_step(report).unwrap_or_else(|err| panic!("{kind} 를 닫지 못했다: {err}"));
    }
    let synthesis = project.cycles().current().step_ref(project.cycles().current().steps().current().expect("Synthesis 에 서 있다"));
    let outcome = common::opened(project, NodeKind::Outcome);
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Outcome)
        .with(common::SYNTHESIS_REF, synthesis.to_string())
        .with(common::REASON, "승인된 Synthesis 를 얻었다");
    project.close_action_step(report).expect("Outcome 을 닫는다");
    let mut report = common::cycle_report(project.cycles().current(), "success", common::step_id(project, outcome));
    report.insert(common::ACTION, "open_child");
    project.close_cycle(report).expect("Interview 를 닫는다");
    session.commit().expect("저장한다");
}

/// 형식 10 으로 정식 시작한 합성 프로젝트 — P1 승인, A1, 열린 뿌리 Interview C1. 그 뒤 C1 을 닫아 검토 인터뷰의 경계에 선다.
fn started(label: &str) -> PathBuf {
    let root = scratch(label);
    fs::write(root.join("README.md"), "hello\n").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.txt"), "alpha\n").unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(root.join("data/raw.csv"), "1,2,3\n").unwrap();
    fs::create_dir_all(root.join("scratch")).unwrap();
    fs::write(root.join("scratch/tmp.txt"), "tmp\n").unwrap();
    use crate::start_prep as prep;
    prep::begin(&s(&root)).expect("시작 준비");
    let prepared = prep::prepare(&s(&root), prep::PrepareInput {
        rules: P1.into(), project_recommendations: vec![], combine_with_revision: None, restart_canceled: false,
    }).expect("제안");
    let (revision, digest) = (prepared["proposal"]["revision"].as_u64().unwrap() as u32, prepared["proposal"]["digest"].as_str().unwrap().to_owned());
    let begun = prep::decide(&root, revision, &digest, true).expect("Monitor 승인 흉내");
    assert_eq!(begun["phase"], "started", "{begun}");
    close_interview(&root);
    root
}

fn proposed(root: &Path, rules: &str) -> (u32, String) {
    let view = propose(&s(root), rules.into()).expect("제안");
    assert_eq!(view["phase"], "awaiting_approval", "{view}");
    (view["proposal"]["revision"].as_u64().unwrap() as u32, view["proposal"]["digest"].as_str().unwrap().to_owned())
}

fn now(root: &Path) -> Value {
    status(&s(root)).expect("조회")
}

fn policies(root: &Path) -> usize {
    open_session(root).project().tracking().unwrap().ledger().policies().len()
}

fn failed_attempts(root: &Path) -> Vec<String> {
    let session = open_session(root);
    session.project().tracking().unwrap().waiting().map(|(_, event)| event.failed.clone()).unwrap_or_default()
}

fn manifest_paths(root: &Path, world: &str) -> Vec<String> {
    let session = open_session(root);
    let project = session.project();
    let address = project.world_manifest(world.parse().unwrap()).expect("등록된 세계").clone();
    crate::artifact::ObjectStore::at(&root.join(".gil")).read_manifest(&address).unwrap().paths().map(|p| p.as_str().to_owned()).collect()
}

// ── 전체 흐름 ──────────────────────────────────────────────────────────────

/// C1(검토 인터뷰) 닫힘 → 정확한 제안 → Monitor 승인 → 저장 1(P2 적용, world_before A1, 기준점 없음) → 정책 파일 동기화 →
/// 보관 실패(외부 변경) → 대기 중 일반 Interview C2 → 그 닫힌 경계에서 명시 재시도 → 기준점 A2(P2, 보관 경계 C2) → Experiment C3(Entry A2).
#[test]
fn approval_applies_the_policy_then_a_failed_archive_waits_through_an_interview_until_the_explicit_retry_opens_the_next_experiment() {
    let root = started("flow");
    let (revision, digest) = proposed(&root, P2);
    // 제안은 아무것도 바꾸지 않는다.
    assert_eq!(policies(&root), 1);
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1);

    // 승인 저장 뒤·보관 전에 계속 추적한 경로 하나가 밖에서 바뀐다.
    let changed = root.join("src/a.txt");
    AFTER_APPROVAL.with(|slot| *slot.borrow_mut() = Some(Box::new(move || fs::write(&changed, "changed outside\n").unwrap())));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");

    // 세 결과가 따로 선다: 승인 = 적용, 파일 동기화, 기준점 보관.
    assert_eq!(decided["facts"]["approval_response"], "recorded", "{decided}");
    assert_eq!(decided["facts"]["policy_applied"], true, "{decided}");
    assert_eq!(decided["facts"]["policy"], "policy:P2");
    assert_eq!(decided["facts"]["event"], "event:M2");
    assert_eq!(decided["facts"]["policy_file"], "written");
    assert_eq!(decided["facts"]["baseline"]["result"], "failed", "{decided}");
    assert_eq!(decided["facts"]["baseline"]["cause"], "external_change");
    assert_eq!(decided["facts"]["baseline"]["still_waiting"], true);
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P2);

    // 저장된 기록을 다시 읽는다: P2 는 적용됐고, 마지막 확정 세계는 P1 으로 확정한 A1 이고, 기준점은 없다 — 유효한 대기다.
    let seen = now(&root);
    assert_eq!(seen["phase"], "applied");
    assert_eq!(seen["tracking"]["applied_policy"], "policy:P2");
    assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A1");
    assert_eq!(seen["tracking"]["confirmed_policy"], "policy:P1");
    assert_eq!(seen["tracking"]["basis_agrees"], false);
    assert_eq!(seen["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(seen["tracking"]["waiting"]["world_before"], "snapshot:A1");
    assert_eq!(seen["policy_file"], "synced");
    assert_eq!(failed_attempts(&root), ["external_change"], "실패 시도가 기록에 남는다");
    {
        let session = open_session(&root);
        let (name, event) = session.project().tracking().unwrap().waiting().expect("보관 대기");
        assert_eq!(name.to_string(), "event:M2");
        assert_eq!(event.after_cycle.unwrap().to_string(), "cycle:C1");
        assert_eq!(event.review_cycle.unwrap().to_string(), "cycle:C1");
        assert_eq!(event.next_cycle_id, 2);
        assert_eq!(session.project().tracking_note().contains("event:M2"), true, "status·context 가 대기를 말한다");
    }

    // 대기 중: Experiment 는 거절되고 아무것도 바뀌지 않는다. Interview 는 열린다(Entry = 마지막 확정 세계 A1).
    {
        let mut session = open_session(&root);
        let refused = session.open_child_cycle(CycleKind::Experiment).expect_err("② 위반 — 다음 실험 보류");
        assert!(matches!(refused, crate::SessionError::Cycles(crate::cycles::CyclesError::Tracking(_))), "{refused}");
        assert_eq!(session.project().cycles().nodes().len(), 1, "거절은 이름 발급기도 움직이지 않는다");
        let opened = session.open_child_cycle(CycleKind::Interview).expect("인터뷰는 막지 않는다");
        assert_eq!(opened.to_ref().to_string(), "cycle:C2");
        assert_eq!(session.project().cycles().current().entry_snapshot().to_string(), "snapshot:A1");
        session.commit().unwrap();
    }
    // 열린 사이클 안에서는 재시도를 시작하지 않는다 — 시도 수가 그대로다.
    assert_eq!(retry(&root, "event:M2").expect_err("닫힌 경계가 아니다").code, "not_at_boundary");
    assert_eq!(failed_attempts(&root).len(), 1);
    // 열린 Interview 안에서도 세 값은 같다: 적용 P2, 마지막 확정 세계 A1(P1 을 물려받는다).
    let inside = now(&root);
    assert_eq!(inside["tracking"]["standing"]["state"], "open_interview");
    assert_eq!(inside["tracking"]["last_confirmed_world"], "snapshot:A1");
    assert_eq!(inside["tracking"]["confirmed_policy"], "policy:P1");

    // 대기 중에는 종료 Synthesis 를 승인으로 닫지 못한다(O-3). 일반 Interview 로는 완결된다.
    {
        let mut session = open_session(&root);
        let project = session.project_mut();
        for kind in [NodeKind::Question, NodeKind::Interpretation] {
            common::walked(project, kind);
        }
        project.open_action_step(NodeKind::Synthesis, common::plan(NodeKind::Synthesis)).unwrap();
        let cycle = project.cycles().current();
        let basis: Vec<String> = cycle.steps().nodes().iter()
            .filter(|node| matches!(node.kind, NodeKind::Question | NodeKind::Interpretation))
            .map(|node| cycle.step_ref(node.id).to_string()).collect();
        let closing = common::full_report(cycle.rules(), CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL)
            .with(common::BASIS_REFS, basis.join("\n"))
            .with("chain_verdict", "success");
        let refused = project.close_action_step(closing).expect_err("보관 확정 전에는 종료 승인으로 나아가지 않는다");
        assert!(matches!(refused, crate::ActionError::Tracking(_)), "{refused}");
        assert!(project.cycles().current().step_now_open().is_some(), "거절된 Synthesis 는 열린 그대로다");
        // 같은 Synthesis 를 일반 승인으로 닫는 것은 막지 않는다 — 시험이 직접 닫아 그 뒤를 걷는다.
        let ordinary = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL)
            .with(common::BASIS_REFS, basis.join("\n"));
        project.close_action_step(ordinary).expect("일반 Synthesis 는 닫힌다");
        let synthesis = project.cycles().current().step_ref(project.cycles().current().steps().current().unwrap());
        let outcome = common::opened(project, NodeKind::Outcome);
        let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Outcome)
            .with(common::SYNTHESIS_REF, synthesis.to_string())
            .with(common::REASON, "승인된 Synthesis 를 얻었다");
        project.close_action_step(report).unwrap();
        let mut report = common::cycle_report(project.cycles().current(), "success", common::step_id(project, outcome));
        report.insert(common::ACTION, "open_child");
        project.close_cycle(report).expect("일반 Interview 는 대기 중에도 닫힌다");
        assert_eq!(project.cycles().current().exit_snapshot().unwrap().to_string(), "snapshot:A1");
        session.commit().unwrap();
    }

    // 미확정 변경이 남아 있으면 같은 정책의 재시도도 실패하고 대기가 유지된다.
    let again = retry(&root, "event:M2").expect("요청은 받는다");
    assert_eq!(again["facts"]["baseline"]["result"], "failed", "{again}");
    assert_eq!(failed_attempts(&root), ["external_change", "external_change"]);
    assert_eq!(policies(&root), 2, "실패가 정책을 P1 으로 되돌리지 않는다");

    // 사용자가 기존 절차로 정리한 뒤, 지금 서 있는 경계 C2 에서 명시 재시도.
    fs::write(root.join("src/a.txt"), "alpha\n").unwrap();
    let retried = retry(&root, "event:M2").expect("명시 재시도");
    assert_eq!(retried["facts"]["baseline"]["result"], "archived", "{retried}");
    assert_eq!(retried["facts"]["baseline"]["snapshot"], "snapshot:A2");
    assert_eq!(retried["facts"]["baseline"]["archived_at"], "cycle:C2");
    assert!(retried["tracking"]["waiting"].is_null(), "{retried}");
    assert_eq!(retried["tracking"]["last_confirmed_world"], "snapshot:A2");
    assert_eq!(retried["tracking"]["confirmed_policy"], "policy:P2");
    assert_eq!(retried["tracking"]["basis_agrees"], true);
    {
        let session = open_session(&root);
        let tracking = session.project().tracking().unwrap();
        let (_, event) = tracking.events().last().unwrap();
        let archived = event.archived.expect("기준점이 확정됐다");
        assert_eq!(archived.after_cycle.unwrap().to_string(), "cycle:C2", "보관한 경계는 지금 서 있는 경계다");
        assert_eq!(archived.next_cycle_id, 3, "그때의 Cycle 발급기");
        assert_eq!(event.after_cycle.unwrap().to_string(), "cycle:C1", "승인 경계는 바뀌지 않는다");
        assert_eq!(event.failed, ["external_change", "external_change"], "앞선 실패 시도가 보존된다");
        // 과거의 Exit 은 그대로이고 사건이 그 옆에 선다. 경계의 세계는 B(T) = 기준점이다.
        assert_eq!(session.project().cycles().current().exit_snapshot().unwrap().to_string(), "snapshot:A1");
        assert_eq!(session.project().world_snapshot().to_string(), "snapshot:A2");
    }
    // 당시 정책으로 본다: A1 은 P1 의 세계(data/ 포함), A2 는 P2 의 세계(data/ 없음).
    assert!(manifest_paths(&root, "snapshot:A1").iter().any(|path| path.starts_with("data/")));
    assert!(!manifest_paths(&root, "snapshot:A2").iter().any(|path| path.starts_with("data/")));

    // 이미 확정된 기준점은 다시 확정하지 않는다.
    assert_eq!(retry(&root, "event:M2").expect_err("대기가 아니다").code, "not_waiting");

    // 다음 Experiment: 부모는 C2, Entry 는 사건의 기준점 A2.
    {
        let mut session = open_session(&root);
        let opened = session.open_child_cycle(CycleKind::Experiment).expect("기준점 확정 뒤에는 실험이 열린다");
        let cycle = session.project().cycles().current();
        assert_eq!(opened.to_ref().to_string(), "cycle:C3");
        assert_eq!(cycle.parent().unwrap().to_ref().to_string(), "cycle:C2");
        assert_eq!(cycle.entry_snapshot().to_string(), "snapshot:A2");
        session.commit().unwrap();
    }
    // 저장된 기록이 load 의 검사 전체(①·②·T8·객체의 당시 정책 범위·Monitor 승인 응답)를 다시 통과한다.
    let reopened = open_session(&root);
    assert_eq!(reopened.project().cycles().current().entry_snapshot().to_string(), "snapshot:A2");
    assert_eq!(reopened.project().tracking().unwrap().ledger().policies().len(), 2);
}

// ── 저장 1 이 일어나지 않는 경우 ───────────────────────────────────────────

#[test]
fn without_the_exact_approval_or_with_changed_conditions_no_policy_is_saved() {
    let root = started("no-save");
    let before = fs::read(root.join(".gil/state.yaml")).unwrap();
    let (revision, digest) = proposed(&root, P2);
    // 다른 제안·다른 판.
    assert_eq!(decide(&root, revision, &"0".repeat(64), true).expect_err("모르는 제안").code, "stale_proposal");
    assert_eq!(decide(&root, revision + 1, &digest, true).expect_err("다른 판").code, "stale_proposal");
    // 변경 요청은 정책을 만들지 않는다.
    let asked = decide(&root, revision, &digest, false).expect("응답 기록");
    assert_eq!(asked["phase"], "changes_requested");
    assert_eq!(decide(&root, revision, &digest, true).expect_err("응답은 한 번이다").code, "response_final");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), before, "상태 파일이 바이트까지 그대로다");

    // 승인 뒤 실제 추적 목록이 제안과 달라졌으면 판을 만들지 않는다 — 새 제안이 필요하다.
    let (revision, digest) = proposed(&root, P2);
    fs::write(root.join("src/new.txt"), "new\n").unwrap();
    let held = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(held["facts"]["approval_response"], "recorded");
    assert_eq!(held["facts"]["policy_applied"], false, "{held}");
    // 새 파일은 계속 추적한 경로의 미확정 변경이기도 하다 — 어느 쪽이든 저장 1 은 없다.
    assert!(["unconfirmed_change", "scope_changed"].contains(&held["application_held"]["code"].as_str().unwrap()), "{held}");
    assert_eq!(held["phase"], "approved_not_applied");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), before);
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1, "정책 파일도 그대로다");
}

#[test]
fn a_proposed_exclusion_cannot_hide_an_unconfirmed_change_on_a_continuously_tracked_path() {
    let root = started("hide");
    let before = fs::read(root.join(".gil/state.yaml")).unwrap();
    fs::write(root.join("src/a.txt"), "changed before approval\n").unwrap();
    // `src/` 를 제외하는 제안 — 제안이 보는 추적 목록에는 그 변경이 없다.
    let (revision, digest) = proposed(&root, "scratch/\nsrc/\n");
    let held = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(held["application_held"]["code"], "unconfirmed_change", "{held}");
    assert_eq!(held["facts"]["policy_applied"], false);
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), before, "전후가 같다");
    assert_eq!(policies(&root), 1);
}

// ── 저장 1 의 실패·저장 뒤 오류 ────────────────────────────────────────────

#[test]
fn a_failed_approval_save_applies_nothing_and_a_save_that_errors_after_storing_is_settled_by_rereading() {
    let root = started("save1");
    let before = fs::read(root.join(".gil/state.yaml")).unwrap();
    let (revision, digest) = proposed(&root, P2);
    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("approval", false)));
    let failed = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(failed["application_held"]["code"], "approval_not_saved", "{failed}");
    assert_eq!(failed["facts"]["policy_applied"], false);
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), before, "앞 상태를 유지한다");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1, "적용되지 않은 정책의 파일을 쓰지 않는다");
    assert_eq!(now(&root)["tracking"]["applied_policy"], "policy:P1");
    // 같은 승인의 재전송은 조회일 뿐이다 — 판·사건·시도를 만들지 않는다.
    let resent = decide(&root, revision, &digest, true).expect("조회");
    assert_eq!(resent["resent"], true);
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), before);

    // 저장은 됐는데 오류가 돌아온 경우: 다시 읽은 기록이 적용을 말한다.
    let (revision, digest) = proposed(&root, P2);
    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("approval", true)));
    let stored = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(stored["facts"]["policy_applied"], true, "{stored}");
    assert_eq!(stored["facts"]["baseline"]["result"], "archived", "{stored}");
    assert_eq!(policies(&root), 2);
}

// ── 세 결과의 독립 ─────────────────────────────────────────────────────────

#[test]
fn a_policy_file_write_failure_and_an_archive_save_failure_leave_the_applied_policy_in_force() {
    let root = started("independent");
    let (revision, digest) = proposed(&root, P2);
    WRITE_FAULT.with(|slot| *slot.borrow_mut() = Some("write"));
    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("baseline", false)));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    WRITE_FAULT.with(|slot| *slot.borrow_mut() = None);
    assert_eq!(decided["facts"]["policy_applied"], true, "{decided}");
    // 파일은 구판 그대로이고, 교체 전의 확인된 장애라 기준은 이전 값으로 되돌아갔다 — 「파일 갱신 필요」가 따로 선다.
    assert_eq!(decided["facts"]["policy_file"], "update_failed");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1);
    assert_eq!(now(&root)["policy_file"], "update_needed");
    // 기준점 저장이 실패했다 — 정책은 P2 그대로, 보관 대기.
    assert_eq!(decided["facts"]["baseline"]["result"], "failed", "{decided}");
    assert_eq!(decided["facts"]["baseline"]["cause"], "store_failure");
    assert_eq!(now(&root)["tracking"]["applied_policy"], "policy:P2");
    assert_eq!(failed_attempts(&root), ["store_failure"]);
    // 조회는 시도를 만들지 않는다.
    let _ = now(&root);
    let _ = review(&root).expect("검토 화면 읽기");
    assert_eq!(failed_attempts(&root).len(), 1);
    // 같은 경계에서 명시 재시도 — 기준점의 경계·발급기는 사건의 값과 같다(인터뷰가 지나지 않았다).
    let retried = retry(&root, "event:M2").expect("재시도");
    assert_eq!(retried["facts"]["baseline"]["result"], "archived", "{retried}");
    assert_eq!(retried["facts"]["baseline"]["archived_at"], "cycle:C1");
}

#[test]
fn a_reread_failure_at_the_time_of_writing_stores_no_baseline_and_never_reports_an_archive() {
    let root = started("unknown");
    let (revision, digest) = proposed(&root, P2);
    // 쓰는 시점의 다시 읽기(결속 확인)를 실패시킨다 — 저장 경계는 확인하지 못한 기록 위에 기준점을 적지 않는다.
    REREAD_FAULT.with(|slot| *slot.borrow_mut() = Some(()));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(decided["facts"]["policy_applied"], true, "{decided}");
    assert_ne!(decided["facts"]["baseline"]["result"], "archived", "읽지 못한 기록 위에서 확정을 말하지 않는다: {decided}");
    assert!(now(&root)["tracking"]["waiting"].is_object(), "기록은 여전히 대기다");
}

// ── 즉시 보관 중단(O-2) ────────────────────────────────────────────────────

#[test]
fn a_halt_stops_every_transition_keeps_the_state_and_only_an_explicit_release_removes_it_without_retrying() {
    let root = started("halt");
    let (revision, digest) = proposed(&root, P2);
    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("baseline", false)));
    decide(&root, revision, &digest, true).expect("승인 — 보관은 실패해 대기");
    let attempts = failed_attempts(&root);

    let halted = halt(&root).expect("중단");
    assert_eq!(halted["tracking"]["archive_halt"], "event:M2", "보관 대기 중의 표식은 그 사건을 가리킨다: {halted}");
    let state = fs::read(root.join(".gil/state.yaml")).unwrap();
    // 재전송은 표식을 옮기지도 무엇을 더하지도 않는다.
    assert_eq!(halt(&root).expect("같은 표식")["tracking"]["archive_halt"], "event:M2");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state);

    // 표식이 있는 동안: 재시도·승인·제안·사이클 열기(인터뷰 포함)가 모두 거절되고 기록은 그대로다. 조회는 된다.
    assert_eq!(retry(&root, "event:M2").expect_err("중단 중").code, "halted");
    assert_eq!(propose(&s(&root), "scratch/\n".into()).expect_err("중단 중").code, "halted");
    {
        let mut session = open_session(&root);
        assert!(session.open_child_cycle(CycleKind::Interview).is_err(), "인터뷰도 멈춘다");
        assert!(session.open_child_cycle(CycleKind::Experiment).is_err());
        assert!(session.project().tracking_note().contains("event:M2"));
    }
    assert_eq!(now(&root)["tracking"]["archive_halt"], "event:M2");
    let (_, grants) = review(&root).expect("조회");
    assert_eq!(grants, [(Purpose::Release, "event:M2".to_string())], "중단 중에는 해제만 줄 수 있다");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "다시 열기·조회로 풀리지 않는다");

    // 해제는 화면에 보인 자리와 정리 방법을 요구한다.
    assert_eq!(release(&root, "cycle:C1", "continue").expect_err("다른 자리").code, "halt_place_differs");
    assert_eq!(release(&root, "event:M2", "anything").expect_err("모르는 정리 방법").code, "cleanup_unknown");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state);
    let released = release(&root, "event:M2", "continue").expect("해제");
    assert!(released["tracking"]["archive_halt"].is_null());
    // 해제는 재시도가 아니다: 여전히 대기이고 시도 수가 그대로다.
    assert_eq!(released["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(failed_attempts(&root), attempts);
    assert_eq!(policies(&root), 2);
    // 해제 뒤의 진행은 별도의 명시 행동이다.
    assert_eq!(retry(&root, "event:M2").expect("명시 재시도")["facts"]["baseline"]["result"], "archived");
}

#[test]
fn a_halt_inside_an_open_step_points_at_that_step_and_closes_nothing() {
    let root = started("halt-step");
    {
        let mut session = open_session(&root);
        session.open_child_cycle(CycleKind::Interview).unwrap();
        session.project_mut().open_action_step(NodeKind::Question, common::plan(NodeKind::Question)).unwrap();
        session.commit().unwrap();
    }
    let halted = halt(&root).expect("중단");
    assert_eq!(halted["tracking"]["archive_halt"], "step:C2/S1", "{halted}");
    let mut session = open_session(&root);
    let project = session.project_mut();
    assert!(project.cycles().current().step_now_open().is_some(), "열린 단계를 닫지 않는다");
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Question);
    assert!(matches!(project.close_action_step(report), Err(crate::ActionError::Tracking(_))), "닫기도 멈춘다");
}

// ── load 가 거절하는 기록 ──────────────────────────────────────────────────

#[test]
fn load_refuses_a_forged_entry_a_missing_approval_and_an_experiment_opened_while_waiting() {
    let root = started("forged");
    let (revision, digest) = proposed(&root, P2);
    assert_eq!(decide(&root, revision, &digest, true).unwrap()["facts"]["baseline"]["result"], "archived");
    {
        let mut session = open_session(&root);
        session.open_child_cycle(CycleKind::Experiment).unwrap();
        session.commit().unwrap();
    }
    let state = root.join(".gil/state.yaml");
    let good = fs::read_to_string(&state).unwrap();
    let load = || crate::load(crate::RuleSet::builtin().unwrap(), &state);
    assert!(load().is_ok());
    // ① 기준점이 있는데 다음 사이클의 Entry 가 부모의 Exit(A1)이다.
    assert_eq!(good.matches("entry_snapshot_ref: snapshot:A2").count(), 1, "C2 의 Entry 가 기준점이다");
    fs::write(&state, good.replace("entry_snapshot_ref: snapshot:A2", "entry_snapshot_ref: snapshot:A1")).unwrap();
    assert!(matches!(load(), Err(crate::StoreError::Tracking(_))), "위조된 Entry");
    // ② 기준점이 없다고 고친 기록 — 대기 사건 뒤에 연 Experiment(T9). 꼴은 유효한 대기이므로 연결 검사가 거절해야 한다.
    let mut file: serde_norway::Value = serde_norway::from_str(&good).unwrap();
    {
        let baseline = file["tracking"]["events"][1]["baseline"].as_mapping_mut().expect("M2 의 baseline");
        baseline.remove("after_cycle_ref");
        baseline.remove("next_cycle_id");
        baseline.insert("snapshot_ref".into(), serde_norway::Value::Null);
        baseline.insert("attempts".into(), serde_norway::Value::Sequence(Vec::new()));
    }
    fs::write(&state, serde_norway::to_string(&file).unwrap()).unwrap();
    assert!(matches!(load(), Err(crate::StoreError::Tracking(_))), "대기 사건 뒤의 실험은 읽히지 않는다");
    fs::write(&state, &good).unwrap();
    // P2 의 Monitor 승인 응답이 없는 기록.
    let ledger = root.join(".gil/scope/changes.json");
    let kept = fs::read(&ledger).unwrap();
    let mut forged: Value = serde_json::from_slice(&kept).unwrap();
    forged["responses"] = serde_json::json!([]);
    fs::write(&ledger, serde_json::to_vec_pretty(&forged).unwrap()).unwrap();
    assert!(matches!(load(), Err(crate::StoreError::Tracking(_))), "승인 응답 없는 정책 판");
    fs::write(&ledger, kept).unwrap();
    assert!(load().is_ok(), "되돌린 기록은 다시 읽힌다");
}
