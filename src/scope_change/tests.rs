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

/// 열린 Synthesis 까지 걸은 Interview — 그 Synthesis 를 **이 프로필의 계약대로** 승인해 닫는 데 필요한 것.
struct AtSynthesis {
    /// 닫힌 Question·Interpretation 의 참조(한 줄에 하나).
    basis: String,
    /// 승인 계획 구성: 이 Interview 가 승인받을 계획 초안.
    #[cfg(feature = "approved-plan")]
    draft: crate::approved_plan::Draft,
}

/// 지금 열린 Interview 를 Question → Interpretation 까지 닫고 Synthesis 를 **열어 둔다.** `note` 는 Interview 마다 다른 한 줄이다.
#[cfg(not(feature = "approved-plan"))]
fn to_synthesis(project: &mut Project, _note: &str) -> AtSynthesis {
    for kind in [NodeKind::Question, NodeKind::Interpretation] {
        common::walked(project, kind);
    }
    common::opened(project, NodeKind::Synthesis);
    AtSynthesis { basis: basis_of(project) }
}

/// 승인 계획 구성(순서 항목 계약): 맨 `approved: yes` 는 Core 가 거절한다. 실제 계약 그대로 계획 초안을 만든다.
#[cfg(feature = "approved-plan")]
fn to_synthesis(project: &mut Project, note: &str) -> AtSynthesis {
    let mut draft = crate::approved_plan::tests::synthesis(project, &["A", "B", "C"]);
    draft.scope.in_scope = format!("Synthetic fixture — {note}");
    AtSynthesis { basis: basis_of(project), draft }
}

fn basis_of(project: &Project) -> String {
    let cycle = project.cycles().current();
    cycle.steps().nodes().iter()
        .filter(|node| matches!(node.kind, NodeKind::Question | NodeKind::Interpretation) && node.is_closed())
        .map(|node| cycle.step_ref(node.id).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 열린 Synthesis 를 승인으로 닫고 Outcome 을 거쳐 Interview 를 `open_child` 방향으로 닫는다 — 일반 Interview 의 완결.
#[cfg(not(feature = "approved-plan"))]
fn finish_interview(project: &mut Project, at: AtSynthesis) {
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL)
        .with(common::BASIS_REFS, at.basis);
    project.close_action_step(report).expect("일반 Synthesis 는 닫힌다");
    let synthesis = project.cycles().current().step_ref(project.cycles().current().steps().current().expect("Synthesis 에 서 있다"));
    let outcome = common::opened(project, NodeKind::Outcome);
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Outcome)
        .with(common::SYNTHESIS_REF, synthesis.to_string())
        .with(common::REASON, "승인된 Synthesis 를 얻었다");
    project.close_action_step(report).expect("Outcome 을 닫는다");
    let mut report = common::cycle_report(project.cycles().current(), "success", common::step_id(project, outcome));
    report.insert(common::ACTION, "open_child");
    project.close_cycle(report).expect("Interview 를 닫는다");
}

/// 승인 계획 구성: 제안 → Monitor 결정(시험이 흉내 낸 값) → 그 `close_report` 로 Synthesis, 이어서 Outcome 과 Interview 닫기.
#[cfg(feature = "approved-plan")]
fn finish_interview(project: &mut Project, at: AtSynthesis) {
    crate::approved_plan::tests::approve_close(project, at.draft);
}

/// 지금 열린 Interview 를 끝까지 걸어 닫고 저장한다.
fn close_interview(root: &Path, note: &str) {
    let mut session = open_session(root);
    let at = to_synthesis(session.project_mut(), note);
    finish_interview(session.project_mut(), at);
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
    close_interview(&root, "review");
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

    // 대기 중에는 종료 Synthesis 를 승인으로 닫지 못한다(O-3). 같은 Interview 는 일반 Interview 로 완결된다.
    {
        let mut session = open_session(&root);
        let project = session.project_mut();
        let at = to_synthesis(project, "while waiting");
        let closing = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Synthesis)
            .with(common::APPROVED, common::APPROVAL)
            .with(common::BASIS_REFS, at.basis.clone())
            .with("chain_verdict", "success");
        let refused = project.close_action_step(closing).expect_err("보관 확정 전에는 종료 승인으로 나아가지 않는다");
        assert!(matches!(refused, crate::ActionError::Tracking(_)), "{refused}");
        assert!(project.cycles().current().step_now_open().is_some(), "거절된 Synthesis 는 열린 그대로다");
        finish_interview(project, at);
        assert!(project.cycles().current().is_closed(), "일반 Interview 는 대기 중에도 닫힌다");
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

/// 화면이 보여 준 해제의 확인 근거 — 검토 읽기가 싣는 그 중단 건의 자리·이름·상태 지문.
fn shown_halt(root: &Path) -> (String, String, String) {
    let (view, _) = review(root).expect("검토 화면 읽기");
    let halt = &view["halt"];
    (
        halt["at"].as_str().expect("표식의 자리").to_owned(),
        halt["id"].as_str().expect("이 중단 건의 기록").to_owned(),
        halt["state_digest"].as_str().expect("상태의 지문").to_owned(),
    )
}

fn release_as(root: &Path, shown: &(String, String, String), cleanup: &str, state_confirmed: bool) -> Result<Value, Refusal> {
    release(root, &Confirmation { at: &shown.0, halt_id: &shown.1, state_digest: &shown.2, cleanup, state_confirmed })
}

/// 보관은 실패해 대기 중인 프로젝트.
fn waiting(label: &str) -> PathBuf {
    let root = started(label);
    let (revision, digest) = proposed(&root, P2);
    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("baseline", false)));
    let decided = decide(&root, revision, &digest, true).expect("승인 — 보관은 실패해 대기");
    assert_eq!(decided["facts"]["baseline"]["result"], "failed", "{decided}");
    root
}

#[test]
fn a_halt_stops_every_transition_keeps_the_state_and_only_an_explicit_release_removes_it_without_retrying() {
    let root = waiting("halt");
    let attempts = failed_attempts(&root);

    let halted = halt(&root).expect("중단");
    assert_eq!(halted["tracking"]["archive_halt"], "event:M2", "보관 대기 중의 표식은 그 사건을 가리킨다: {halted}");
    assert_eq!(halted["halt_request"], serde_json::json!({"received": true, "applied": true}));
    assert_eq!(halted["current_state"]["reread"], true);
    let state = fs::read(root.join(".gil/state.yaml")).unwrap();
    // 재전송은 표식을 옮기지도 무엇을 더하지도 않는다 — 같은 중단 건이다.
    let shown = shown_halt(&root);
    assert_eq!(halt(&root).expect("같은 표식")["tracking"]["archive_halt"], "event:M2");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state);
    assert_eq!(shown_halt(&root), shown, "재전송이 새 중단 건을 만들지 않는다");

    // 표식이 있는 동안: 재시도·승인·제안·사이클 열기(인터뷰 포함)가 모두 거절되고 기록은 그대로다. 조회는 된다.
    assert_eq!(retry(&root, "event:M2").expect_err("중단 중").code, "halted");
    assert_eq!(propose(&s(&root), "scratch/\n".into()).expect_err("중단 중").code, "halted");
    {
        let mut session = open_session(&root);
        assert!(session.open_child_cycle(CycleKind::Interview).is_err(), "인터뷰도 멈춘다");
        assert!(session.open_child_cycle(CycleKind::Experiment).is_err());
        assert!(session.project().tracking_note().contains("event:M2"));
        assert!(session.commit().is_err(), "중단 중에는 일반 저장 문도 닫혀 있다");
    }
    assert_eq!(now(&root)["tracking"]["archive_halt"], "event:M2");
    let (_, grants) = review(&root).expect("조회");
    assert_eq!(grants, [(Purpose::Release, release_target(&shown.0, &shown.1, &shown.2))], "중단 중에는 그 건의 해제만 줄 수 있다");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "다시 열기·조회로 풀리지 않는다");

    // 해제는 사용자의 두 명시 입력을 서버에서도 요구한다: 상태 확인과 정리 방법.
    assert_eq!(release_as(&root, &shown, "continue", false).expect_err("상태 확인 없음").code, "state_not_confirmed");
    assert_eq!(release_as(&root, &shown, "anything", true).expect_err("모르는 정리 방법").code, "cleanup_unknown");
    // 자리만 맞는 요청, 다른 중단 건의 이름, 달라진 상태 지문 — 어느 것도 해제하지 못한다.
    let other_place = ("cycle:C1".to_string(), shown.1.clone(), shown.2.clone());
    assert_eq!(release_as(&root, &other_place, "continue", true).expect_err("다른 자리").code, "halt_place_differs");
    let other_halt = (shown.0.clone(), "0".repeat(32), shown.2.clone());
    assert_eq!(release_as(&root, &other_halt, "continue", true).expect_err("다른 중단 건").code, "halt_confirmation_stale");
    let other_state = (shown.0.clone(), shown.1.clone(), "0".repeat(64));
    assert_eq!(release_as(&root, &other_state, "continue", true).expect_err("다른 상태").code, "halt_confirmation_stale");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "거절된 해제는 아무것도 바꾸지 않는다");

    let released = release_as(&root, &shown, "continue", true).expect("해제");
    assert!(released["tracking"]["archive_halt"].is_null());
    // 해제는 재시도가 아니다: 여전히 대기이고 시도 수가 그대로다.
    assert_eq!(released["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(failed_attempts(&root), attempts);
    assert_eq!(policies(&root), 2);
    // 같은 확인은 다시 쓰이지 않는다.
    assert_eq!(release_as(&root, &shown, "continue", true).expect_err("이미 해제됐다").code, "not_halted");
    // 해제 뒤의 진행은 별도의 명시 행동이다.
    assert_eq!(retry(&root, "event:M2").expect("명시 재시도")["facts"]["baseline"]["result"], "archived");
}

#[test]
fn a_new_halt_at_the_same_place_needs_its_own_confirmation_and_a_missing_record_is_never_a_confirmation() {
    let root = waiting("halt-again");
    halt(&root).expect("첫 중단");
    let first = shown_halt(&root);
    release_as(&root, &first, "continue", true).expect("첫 해제");

    // 같은 자리에서 다시 중단했다 — 자리도 상태 파일의 바이트도 첫 중단 때와 같지만 다른 중단 건이다.
    halt(&root).expect("둘째 중단");
    let second = shown_halt(&root);
    assert_eq!((&first.0, &first.2), (&second.0, &second.2), "자리와 상태 지문은 같다");
    assert_ne!(first.1, second.1, "중단 건의 이름이 다르다");
    assert_eq!(release_as(&root, &first, "continue", true).expect_err("첫 중단의 확인").code, "halt_confirmation_stale");

    // 이 중단 건의 기록이 없는 경우(합성: 원장에서 중단·해제 사건을 지운다) — 해제도 그 capability 도 없다.
    let ledger_path = root.join(".gil/scope/changes.json");
    let kept = fs::read(&ledger_path).unwrap();
    let mut without: Value = serde_json::from_slice(&kept).unwrap();
    without["events"].as_array_mut().unwrap().retain(|event| !matches!(event["kind"].as_str(), Some("halt_requested" | "halt_released")));
    fs::write(&ledger_path, serde_json::to_vec_pretty(&without).unwrap()).unwrap();
    assert_eq!(release_as(&root, &second, "continue", true).expect_err("기록 없음").code, "halt_record_missing");
    let (view, grants) = review(&root).expect("조회");
    assert_eq!(view["halt"]["record_missing"], true, "{view}");
    assert_eq!(grants, [(Purpose::Halt, "halt".to_string())], "기록을 다시 남기는 요청만 줄 수 있다");
    assert!(open_session(&root).project().tracking().unwrap().halt().is_some(), "표식은 그대로다");
    // 기록을 다시 남기면 새 이름의 건이 되고, 그 화면의 확인으로만 해제된다.
    halt(&root).expect("기록 다시 남기기");
    let recorded = shown_halt(&root);
    assert_ne!(recorded.1, second.1);
    assert_eq!(release_as(&root, &second, "continue", true).expect_err("옛 확인").code, "halt_confirmation_stale");
    release_as(&root, &recorded, "propose_scope_change", true).expect("새 확인으로 해제");
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

fn object_files(root: &Path) -> usize {
    fn count(dir: &Path) -> usize {
        fs::read_dir(dir).map_or(0, |entries| {
            entries.flatten().map(|entry| if entry.path().is_dir() { count(&entry.path()) } else { 1 }).sum()
        })
    }
    count(&root.join(".gil/artifacts"))
}

/// 중단 중의 Verify 닫기는 **객체를 쓰기 전에** 거절된다 — 표식이 저장돼 있을 때도, 요청만 받고 아직 적용되지 않았을 때도.
#[test]
fn a_halted_verify_is_refused_before_any_object_is_archived() {
    let root = started("halt-verify");
    {
        let mut session = open_session(&root);
        session.open_child_cycle(CycleKind::Experiment).expect("대기가 없으면 실험이 열린다");
        common::up_to_verify(session.project_mut());
        session.commit().unwrap();
    }
    fs::write(root.join("src/new-evidence.txt"), "evidence\n").unwrap();
    let verify_report = |session: &ProjectSession| {
        common::full_report(session.project().cycles().current().rules(), CycleKind::Experiment, NodeKind::Verify)
            .with("evidence_files", "src/new-evidence.txt")
    };

    // ① 요청은 받았고 아직 적용되지 않았다 — 이미 잠금을 쥐고 진행 중이던 명령이 보관 경계에 닿는 경우.
    let mut session = open_session(&root);
    let before = (object_files(&root), fs::read(root.join(".gil/state.yaml")).unwrap());
    ledger::receive_halt_request(&root.join(".gil"), "synthetic-request").expect("수신 표식");
    let report = verify_report(&session);
    let refused = session.close_step(report).expect_err("받은 중단 요청이 보관을 멈춘다");
    assert!(matches!(refused, crate::SessionError::Action(crate::ActionError::Tracking(_))), "{refused}");
    assert!(session.commit().is_err(), "마지막 저장 문도 닫혀 있다");
    assert_eq!((object_files(&root), fs::read(root.join(".gil/state.yaml")).unwrap()), before, "객체도 상태도 그대로다");
    drop(session);

    // ② 다음 쓰기 열기가 그 요청을 표식으로 적용한다. 표식은 열린 Verify 를 가리키고 그것을 닫지 않는다.
    let mut session = open_session(&root);
    let at = session.project().tracking().unwrap().halt().expect("적용된 표식").to_string();
    assert!(at.starts_with("step:C2/"), "{at}");
    assert!(!halt_requested(&root.join(".gil")), "적용된 요청의 수신 표식은 치워졌다");
    let objects = object_files(&root);
    let report = verify_report(&session);
    let refused = session.close_step(report).expect_err("표식이 있는 동안 Verify 를 닫지 않는다");
    assert!(matches!(refused, crate::SessionError::Action(crate::ActionError::Tracking(_))), "{refused}");
    assert_eq!(object_files(&root), objects, "blob·manifest 를 하나도 쓰지 않았다");
    assert!(session.project().cycles().current().step_now_open().is_some(), "Verify 는 열린 그대로다");
    drop(session);

    // ③ 해제는 그 Verify 를 대신 닫지도, 보관하지도 않는다 — 표식만 지운다. 그 뒤의 닫기는 별도의 명시 행동이다.
    let shown = shown_halt(&root);
    release_as(&root, &shown, "continue", true).expect("해제");
    let session = open_session(&root);
    assert!(session.project().cycles().current().step_now_open().is_some(), "Verify 는 여전히 열려 있다");
    assert_eq!(object_files(&root), objects, "해제가 보관을 시작하지 않았다");
}

/// 다른 명령이 잠금을 쥐고 있는 동안의 중단 요청: 받았다고 답하고(Busy 로 끝내지 않는다), 적용은 잠금을 얻는 쪽이 한다.
#[test]
fn a_halt_requested_while_another_command_holds_the_project_is_received_then_applied_by_the_next_writer() {
    let root = waiting("halt-busy");
    let state = fs::read(root.join(".gil/state.yaml")).unwrap();
    let holder = open_session(&root);
    let asked = halt(&root).expect("요청은 받는다");
    assert_eq!(asked["halt_request"]["received"], true, "{asked}");
    assert_eq!(asked["halt_request"]["applied"], false);
    assert!(asked.get("tracking").is_none(), "적용되지 않은 상태를 현재 상태처럼 싣지 않는다");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "표식은 아직 저장되지 않았다");
    // 잠금을 쥔 그 명령의 저장 문은 이미 닫혔다.
    assert!(holder.commit().is_err());
    drop(holder);
    // 조회는 요청을 적용하지 않고 사실만 싣는다.
    assert_eq!(now(&root)["halt_request_pending"], true);
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state);
    // 다음 쓰기(여기서는 재시도 요청)가 열리며 표식을 적용하고, 그 재시도는 시작되지 않는다.
    let attempts = failed_attempts_raw(&state);
    assert_eq!(retry(&root, "event:M2").expect_err("중단됐다").code, "halted");
    let seen = now(&root);
    assert_eq!(seen["tracking"]["archive_halt"], "event:M2", "{seen}");
    assert!(seen.get("halt_request_pending").is_none());
    assert_eq!(failed_attempts(&root).len(), attempts, "새 시도가 없다");
}

fn failed_attempts_raw(state: &[u8]) -> usize {
    String::from_utf8_lossy(state).matches("result: failed").count()
}

/// 승인을 적용하는 도중에 받은 요청: 저장 1 전이면 정책을 적용하지 않고, 저장 1 뒤면 끝난 승인을 되돌리지 않고 남은 걸음만 멈춘다.
#[test]
fn a_halt_received_during_an_approval_stops_at_the_next_write_boundary_without_undoing_a_finished_save() {
    // 저장 1 뒤에 받은 요청.
    let root = started("halt-mid-approval");
    let (revision, digest) = proposed(&root, P2);
    let gil = root.join(".gil");
    AFTER_APPROVAL.with(|slot| *slot.borrow_mut() = Some(Box::new(move || {
        ledger::receive_halt_request(&gil, "synthetic-request").expect("수신 표식");
    })));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(decided["facts"]["policy_applied"], true, "끝난 승인 저장은 되돌리지 않는다: {decided}");
    assert_eq!(decided["facts"]["policy_file"], "not_written_halt_requested");
    // 보관은 실패한 것이 아니라 시작하지 않은 것이다 — 관측 전에 멈췄고 시도 기록도 없다.
    assert_eq!(decided["facts"]["baseline"]["result"], "halted", "{decided}");
    assert_eq!(decided["facts"]["baseline"]["before"], "observe");
    assert_eq!(decided["facts"]["baseline"]["halt_request"], serde_json::json!({"received": true, "applied": true}));
    assert_eq!(decided["tracking"]["archive_halt"], "event:M2", "표식은 적용되는 때의 자리를 가리킨다");
    assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1, "정책 파일을 쓰지 않았다");
    assert!(failed_attempts(&root).is_empty(), "중단은 시도를 실패로 적지 않는다");
    assert_eq!(policies(&root), 2);

    // 승인 전에 받은 요청: 열기가 표식을 적용하고 응답도 정책도 기록되지 않는다.
    let root = started("halt-before-approval");
    let (revision, digest) = proposed(&root, P2);
    ledger::receive_halt_request(&root.join(".gil"), "synthetic-request").expect("수신 표식");
    assert_eq!(decide(&root, revision, &digest, true).expect_err("중단됐다").code, "halted");
    assert_eq!(policies(&root), 1);
    let seen = now(&root);
    assert_eq!(seen["tracking"]["archive_halt"], "cycle:C1");
    assert_eq!(seen["phase"], "awaiting_approval", "응답이 기록되지 않았다: {seen}");
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

// ── O-3: 쓰기와 load 가 같은 뜻 ────────────────────────────────────────────

/// 보관 대기와 **승인된 종료 Synthesis** 가 함께 있는 열린 Interview 는 쓰기가 거절하므로, 그런 기록은 load 도 받지 않는다.
/// 일반 Interview 의 승인된 Synthesis 는 대기 중에도 그대로 읽힌다.
#[test]
fn load_refuses_an_open_interview_whose_closing_synthesis_was_approved_while_a_baseline_is_pending() {
    let root = waiting("closing-load");
    {
        let mut session = open_session(&root);
        session.open_child_cycle(CycleKind::Interview).unwrap();
        let at = to_synthesis(session.project_mut(), "ordinary while waiting");
        finish_interview(session.project_mut(), at);
        session.commit().unwrap();
        // 한 번 더: 이번에는 Synthesis 를 일반 승인으로 닫은 데까지만 저장한다(Outcome 전의 열린 Interview).
        session.open_child_cycle(CycleKind::Interview).unwrap();
        session.commit().unwrap();
    }
    let state = root.join(".gil/state.yaml");
    let load = || crate::load(crate::RuleSet::builtin().unwrap(), &state);
    assert!(load().is_ok(), "대기 중의 닫힌 일반 Interview 와 새로 연 Interview 는 정상 기록이다");

    // 합성 기록: 닫힌 일반 Interview C2 를 「아직 열린 Interview, 그 승인된 Synthesis 에 chain_verdict」 로 바꿀 수는 없으므로(닫힘·보고서가
    // 얽힌다), C2 의 저장된 Synthesis Report 에 chain_verdict 를 더하고 C2 가 지금 열린 사이클인 꼴은 따로 만든다 — C3 를 지우는 대신
    // C3 에서 같은 일을 한다: C3 를 Synthesis 승인까지 걸어 저장한 뒤 그 Report 에 chain_verdict 를 더한다.
    {
        let mut session = open_session(&root);
        let project = session.project_mut();
        let at = to_synthesis(project, "third");
        approve_synthesis_only(project, at);
        session.commit().unwrap();
    }
    let good = fs::read_to_string(&state).unwrap();
    assert!(load().is_ok(), "일반 승인된 Synthesis 가 있는 열린 Interview 는 대기 중에도 읽힌다");
    fs::write(&state, with_chain_verdict_on_the_last_approved_synthesis(&good)).unwrap();
    let refused = load().expect_err("보관 대기 중 승인된 종료 Synthesis 는 읽히지 않는다");
    #[cfg(not(feature = "approved-plan"))]
    assert!(matches!(refused, crate::StoreError::Tracking(_)), "{refused}");
    let _ = refused;
    fs::write(&state, &good).unwrap();
    assert!(load().is_ok());
}

/// 열린 Synthesis 를 이 프로필의 계약대로 승인해 닫기만 한다(Outcome·Interview 닫기 없음).
#[cfg(not(feature = "approved-plan"))]
fn approve_synthesis_only(project: &mut Project, at: AtSynthesis) {
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL)
        .with(common::BASIS_REFS, at.basis);
    project.close_action_step(report).expect("일반 Synthesis 는 닫힌다");
}

#[cfg(feature = "approved-plan")]
fn approve_synthesis_only(project: &mut Project, at: AtSynthesis) {
    let proposal = project.propose_plan(at.draft).expect("제안");
    let decided = project
        .decide_plan(proposal.revision, &proposal.digest, crate::approved_plan::Decision::Approve)
        .expect("Monitor 결정(시험이 흉내 낸 값)");
    project.close_action_step(decided.report().expect("close_report")).expect("승인된 Synthesis 를 닫는다");
}

/// 저장된 본문에서 **마지막** `approved: yes` 줄(지금 열린 Interview 의 Synthesis Report) 앞에 같은 들여쓰기로 `chain_verdict: success`
/// 를 더한 합성 기록. Report 가 mapping 으로 눕든 본문 블록으로 눕든 같은 줄 꼴이다.
fn with_chain_verdict_on_the_last_approved_synthesis(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().rposition(|line| line.trim_start().starts_with("approved: yes") || line.trim_start().starts_with("approved: 'yes'"))
        .expect("저장된 승인 Synthesis Report 가 있다");
    let indent: String = lines[at].chars().take_while(|c| c.is_whitespace()).collect();
    let mut out: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
    out.insert(at, format!("{indent}chain_verdict: success"));
    out.join("\n") + "\n"
}

// ── 승인된 제안과 저장된 사건의 대응 ───────────────────────────────────────

/// 응답이 있는 승인이라도, 그 제안이 묶은 경계·발급기·비교 근거가 저장된 사건의 승인 칸과 다르면 load 가 거절한다. 재시도로 보관
/// 경계가 승인 경계와 달라진 정상 기록은 그대로 읽힌다(전체 흐름 시험이 그 경우를 load 한다).
#[test]
fn load_refuses_an_approval_whose_bound_boundary_or_basis_differs_from_the_stored_event() {
    let root = started("approval-binding");
    let (revision, digest) = proposed(&root, P2);
    assert_eq!(decide(&root, revision, &digest, true).unwrap()["facts"]["baseline"]["result"], "archived");
    let state = root.join(".gil/state.yaml");
    let ledger_path = root.join(".gil/scope/changes.json");
    let load = || crate::load(crate::RuleSet::builtin().unwrap(), &state);
    assert!(load().is_ok());
    let (good_state, good_ledger) = (fs::read_to_string(&state).unwrap(), fs::read(&ledger_path).unwrap());

    // 합성 기록: 제안의 한 칸을 바꾸고, 그 제안 digest·응답·정책 판의 proposal_digest 를 함께 일관되게 고친다. 원장과 정책 판은 서로
    // 맞지만 저장된 사건의 승인 칸과는 맞지 않는다.
    let forge = |change: &dyn Fn(&mut Proposal)| {
        let mut found = ledger::read(&root.join(".gil")).unwrap().expect("원장");
        let old = found.proposals[0].digest.clone();
        change(&mut found.proposals[0]);
        let new = digest_of(&found.proposals[0]);
        found.proposals[0].digest = new.clone();
        for response in &mut found.responses {
            response.proposal_digest = new.clone();
        }
        for event in &mut found.events {
            if let Event::Applied { proposal_digest, .. } = event {
                *proposal_digest = new.clone();
            }
        }
        fs::write(&ledger_path, serde_json::to_vec_pretty(&found).unwrap()).unwrap();
        fs::write(&state, good_state.replace(&old, &new)).unwrap();
        let refused = load().expect_err("사건과 대응하지 않는 승인");
        assert!(matches!(refused, crate::StoreError::Tracking(_)), "{refused}");
        fs::write(&ledger_path, &good_ledger).unwrap();
        fs::write(&state, &good_state).unwrap();
        refused.to_string()
    };
    assert!(forge(&|proposal| proposal.issuer += 1).contains("next_cycle_id"));
    assert!(forge(&|proposal| proposal.boundary = "cycle:C9".into()).contains("after_cycle_ref"));
    assert!(forge(&|proposal| proposal.world_before = "snapshot:A2".into()).contains("world_before_ref"));
    assert!(forge(&|proposal| proposal.policy_before = "policy:P2".into()).contains("policy_before_ref"));
    assert!(forge(&|proposal| proposal.confirmed_policy = "policy:P2".into()).contains("확정 정책"));
    assert!(forge(&|proposal| proposal.chain = "chain:C2".into()).contains("chain_ref"));
    assert!(load().is_ok(), "되돌린 기록은 다시 읽힌다");
}

// ── 시도의 사실과 현재 상태 ────────────────────────────────────────────────

/// 시도가 기준점 확정을 확인한 뒤 마지막 다시 읽기만 실패한 경우: 시도의 사실은 싣되, 메모리의 옛 값을 현재 상태로 싣지 않는다.
#[test]
fn when_the_final_reread_fails_the_attempts_facts_are_reported_but_no_current_state_is_claimed() {
    let root = started("final-reread");
    let (revision, digest) = proposed(&root, P2);
    FINAL_REREAD_FAULT.with(|slot| *slot.borrow_mut() = Some(()));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(decided["facts"]["policy_applied"], true, "{decided}");
    assert_eq!(decided["facts"]["baseline"]["result"], "archived", "시도가 다시 읽어 확인한 사실이다");
    assert_eq!(decided["facts"]["baseline"]["record_reread"], false);
    assert_eq!(decided["current_state"]["reread"], false);
    assert_eq!(decided["phase"], "unconfirmed");
    assert!(decided["tracking"].is_null(), "확인하지 못한 값을 현재 상태로 싣지 않는다: {decided}");
    assert!(decided["last_known_before_reread_failed"].is_object());
    // 조회가 현재 상태를 말한다.
    let seen = now(&root);
    assert!(seen["tracking"]["waiting"].is_null(), "{seen}");
    assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A2");
}

// ── 회귀: 확정 뒤의 종료 승인·Chain Report·후속 Chain ──────────────────────

/// 기준점이 확정된 뒤에는 기존 종료 흐름이 그대로다: 종료 Synthesis 승인 → Outcome → `close_chain` → Chain Report → 다음 Chain.
/// 다음 Chain 의 Entry 는 닫힌 Chain 의 Exit(= 종료 Interview 의 Entry = 기준점)이고, 그 기록이 load 를 통과한다.
#[cfg(not(feature = "approved-plan"))]
#[test]
fn after_the_baseline_is_confirmed_the_closing_approval_the_chain_report_and_the_next_chain_work_as_before() {
    let root = started("closing-after");
    let (revision, digest) = proposed(&root, P2);
    assert_eq!(decide(&root, revision, &digest, true).unwrap()["facts"]["baseline"]["result"], "archived");
    let mut session = open_session(&root);
    session.open_child_cycle(CycleKind::Interview).expect("종료 Interview");
    assert_eq!(session.project().cycles().current().entry_snapshot().to_string(), "snapshot:A2", "Entry 는 기준점이다");
    let project = session.project_mut();
    let at = to_synthesis(project, "closing");
    let closing = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Synthesis)
        .with(common::APPROVED, common::APPROVAL)
        .with(common::BASIS_REFS, at.basis)
        .with("chain_verdict", "success");
    project.close_action_step(closing).expect("대기가 없으면 종료 Synthesis 승인은 막지 않는다");
    let synthesis = project.cycles().current().step_ref(project.cycles().current().steps().current().unwrap());
    let outcome = common::opened(project, NodeKind::Outcome);
    let report = common::full_report(project.cycles().current().rules(), CycleKind::Interview, NodeKind::Outcome)
        .with(common::SYNTHESIS_REF, synthesis.to_string());
    project.close_action_step(report).expect("Outcome");
    let report = common::cycle_report(project.cycles().current(), "success", common::step_id(project, outcome))
        .with(common::ACTION, "close_chain");
    project.close_cycle(report).expect("종료 Interview 를 닫는다");
    session.commit().unwrap();
    // 종료 경계에서는 새 승인을 저장하지 않는다(§6.6).
    drop(session);
    assert_eq!(propose(&s(&root), P1.into()).expect_err("종료 경계").code, "chain_boundary");
    let mut session = open_session(&root);
    let report = crate::Report::new()
        .with("verdict", "success")
        .with("closing_synthesis_ref", synthesis.to_string())
        .with("handoff_summary", "합성 인계 — 시험 자료");
    session.close_chain(report).expect("Chain Report");
    session.commit().unwrap();
    session.open_child_chain().expect("다음 Chain");
    assert_eq!(session.project().current_chain().unwrap().entry_snapshot().to_string(), "snapshot:A2");
    session.commit().unwrap();
    drop(session);
    // 후속 Chain 안에서는 확정 정책을 유도하지 않는다 — 적용 정책과 같다고 채우지 않는다. 범위 변경도 받지 않는다.
    let seen = now(&root);
    assert_eq!(seen["tracking"]["applied_policy"], "policy:P2", "{seen}");
    assert!(seen["tracking"]["confirmed_policy"].is_null());
    assert!(open_session(&root).project().tracking().unwrap().ledger().policies().len() == 2);
}

// ── 중단 적용은 최신 저장 상태를 보존한다 ──────────────────────────────────

fn halt_arrives_after(step: &'static str, root: &Path) {
    let gil = root.join(".gil");
    AFTER_ADAPTER_SAVE.with(|slot| *slot.borrow_mut() = Some((step, Box::new(move || {
        ledger::receive_halt_request(&gil, "synthetic-request").expect("수신 표식");
    }))));
}

/// 기준점 저장이 **끝난 직후** 받은 중단 요청: 표식은 그 최신 저장 상태 위에 더해진다. 방금 확정한 기준점·등록 세계·시도 이력·적용
/// 정책·sync_base 가 그대로다 — 저장 전의 메모리 값을 다시 저장해 덮지 않는다. 저장이 오류를 돌려줬지만 실제로는 저장된 경우도 같다.
#[test]
fn a_halt_applied_right_after_a_finished_baseline_save_keeps_that_save() {
    for (label, stored_then_error) in [("halt-after-save", false), ("halt-after-save-error", true)] {
        let root = started(label);
        let (revision, digest) = proposed(&root, P2);
        if stored_then_error {
            SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("baseline", true)));
        }
        halt_arrives_after("baseline", &root);
        let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
        assert_eq!(decided["facts"]["baseline"]["result"], "archived", "{label}: 끝난 저장은 확정으로 읽힌다: {decided}");
        assert_eq!(decided["facts"]["baseline"]["halt_request"], serde_json::json!({"received": true, "applied": true}), "{label}");
        // 저장된 기록: 기준점과 표식이 함께 있다.
        let session = open_session(&root);
        let tracking = session.project().tracking().unwrap();
        let (_, event) = tracking.events().last().unwrap();
        let archived = event.archived.expect("확정한 기준점이 남아 있다");
        assert_eq!(archived.snapshot.to_string(), "snapshot:A2", "{label}");
        assert_eq!(archived.after_cycle.unwrap().to_string(), "cycle:C1");
        assert!(session.project().world_manifest(archived.snapshot).is_some(), "{label}: 등록부의 세계가 남아 있다");
        assert_eq!(tracking.ledger().policies().len(), 2, "{label}: 적용 정책 P2");
        assert!(matches!(tracking.sync_base(), sp::SyncBase::Present(_)), "{label}: sync_base 가 남아 있다");
        // 대기가 없으므로 표식은 서 있는 경계를 가리킨다.
        assert_eq!(tracking.halt().expect("적용된 표식").to_string(), "cycle:C1", "{label}");
        assert!(!halt_requested(&root.join(".gil")), "{label}: 적용된 요청의 수신 표식은 치워졌다");
        drop(session);
        let seen = now(&root);
        assert!(seen["tracking"]["waiting"].is_null(), "{label}: {seen}");
        assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A2");
        assert_eq!(seen["policy_file"], "synced");
    }
}

/// 실패 시도 기록이 **끝난 직후** 받은 요청: 그 실패 기록과 보관 대기가 그대로이고 표식만 더해진다. 중단이 새 실패를 만들지 않는다.
#[test]
fn a_halt_applied_right_after_a_recorded_failure_keeps_the_failure_and_adds_none() {
    let root = started("halt-after-failure");
    let (revision, digest) = proposed(&root, P2);
    let changed = root.join("src/a.txt");
    AFTER_APPROVAL.with(|slot| *slot.borrow_mut() = Some(Box::new(move || fs::write(&changed, "changed outside\n").unwrap())));
    halt_arrives_after("failure", &root);
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(decided["facts"]["baseline"]["result"], "failed", "{decided}");
    assert_eq!(decided["facts"]["baseline"]["cause"], "external_change");
    assert_eq!(decided["facts"]["baseline"]["failure_recorded"], true);
    assert_eq!(decided["facts"]["baseline"]["halt_request"]["applied"], true);
    assert_eq!(failed_attempts(&root), ["external_change"], "기록된 실패가 남아 있고 늘지도 않았다");
    let seen = now(&root);
    assert_eq!(seen["tracking"]["archive_halt"], "event:M2", "{seen}");
    assert_eq!(seen["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(seen["tracking"]["applied_policy"], "policy:P2");
}

/// 최신 저장 상태를 읽지 못하면 표식을 적용하지 않는다: 옛 복사본을 저장하지 않고, 수신 요청을 남기고, 적용을 말하지 않는다.
#[test]
fn when_the_latest_state_cannot_be_read_the_halt_is_not_applied_and_the_request_stays_received() {
    let root = started("halt-reread");
    let (revision, digest) = proposed(&root, P2);
    halt_arrives_after("baseline", &root);
    ABSORB_REREAD_FAULT.with(|slot| *slot.borrow_mut() = Some(()));
    let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
    assert_eq!(decided["facts"]["baseline"]["result"], "archived", "끝난 저장은 그대로다: {decided}");
    assert_eq!(decided["facts"]["baseline"]["halt_request"], serde_json::json!({"received": true, "applied": false}));
    assert!(halt_requested(&root.join(".gil")), "요청은 받은 채로 남는다");
    let state = fs::read_to_string(root.join(".gil/state.yaml")).unwrap();
    assert!(state.contains("snapshot:A2") && state.contains("archive_halt: null"), "기준점은 남고 표식은 아직 없다");
    // 받은 요청이 남아 있는 동안 조회는 그 사실을 싣고, 다음 쓰기 열기가 최신 상태 위에 표식을 적용한다.
    assert_eq!(now(&root)["halt_request_pending"], true);
    let session = open_session(&root);
    let tracking = session.project().tracking().unwrap();
    assert_eq!(tracking.halt().expect("이제 적용됐다").to_string(), "cycle:C1");
    assert!(tracking.events().last().unwrap().1.archived.is_some(), "기준점이 보존됐다");
}

/// 관측 뒤·capture 앞, capture 뒤·manifest 저장 앞, manifest 뒤·상태 저장 앞에서 받은 요청: 그 걸음을 시작하지 않는다. 실패가 아니며
/// 시도 기록이 늘지 않고, 어떤 세계도 확정되지 않는다.
#[test]
fn a_halt_received_between_archive_steps_starts_no_further_step_and_records_no_failure() {
    for step in ["capture", "store_manifest", "save"] {
        let root = waiting(&format!("halt-step-{step}"));
        let attempts = failed_attempts(&root);
        let objects = object_files(&root);
        HALT_BEFORE_STEP.with(|slot| *slot.borrow_mut() = Some(step));
        let retried = retry(&root, "event:M2").expect("요청은 받는다");
        let baseline = &retried["facts"]["baseline"];
        assert_eq!(baseline["result"], "halted", "{step}: {retried}");
        assert_eq!(baseline["before"], step);
        assert_eq!(baseline["failure_recorded"], false);
        assert_eq!(baseline["halt_request"], serde_json::json!({"received": true, "applied": true}), "{step}");
        assert_eq!(failed_attempts(&root), attempts, "{step}: 중단을 실패 이력으로 바꾸지 않는다");
        if step == "capture" {
            assert_eq!(object_files(&root), objects, "관측만 했고 객체를 쓰지 않았다");
        }
        let seen = now(&root);
        assert_eq!(seen["tracking"]["waiting"]["event"], "event:M2", "{step}: 여전히 대기다");
        assert_eq!(seen["tracking"]["archive_halt"], "event:M2");
        assert_eq!(seen["tracking"]["last_confirmed_world"], "snapshot:A1", "{step}: 확정된 세계가 없다");
        // 해제 뒤의 명시 재시도는 처음부터 다시 해서 확정한다 — 앞선 시도가 남긴 객체로 성공을 추정하지 않는다.
        let shown = shown_halt(&root);
        release_as(&root, &shown, "continue", true).expect("해제");
        assert_eq!(retry(&root, "event:M2").expect("명시 재시도")["facts"]["baseline"]["result"], "archived", "{step}");
    }
}

/// 정책 파일 동기화의 세 걸음은 한 단위가 아니다: 걸음 사이에 받은 중단 요청은 **다음 쓰기를 시작하지 않게** 할 뿐이다. 끝난 걸음은
/// 그대로이고, 기준이 `unconfirmed` 로 남으면 §5.2 대로 디스크를 다시 읽어 판정한다. 이전 기준으로 되돌리지 않고, 스스로 다시 쓰지
/// 않으며, 적용 정책은 P2 그대로다. (함수 수준 주입 — 걸음 직전에 수신 표식 파일을 놓는다.)
#[test]
fn a_halt_received_between_policy_file_sync_steps_starts_no_further_write_and_restores_nothing() {
    // (시작 기록, 파일, 다시 읽어 확인한 기준, 멈춘 걸음) — 멈춘 뒤의 걸음은 시작하지 않았다.
    let steps = |start: &str, file: &str, stored: &str, before: &str| {
        serde_json::json!({"start": start, "start_save_returned": if start == "saved" { serde_json::json!("ok") } else { Value::Null },
            "file": file, "base": "not_started", "base_save_returned": null, "stored_base": stored,
            "base_left_unconfirmed": stored == "unconfirmed", "halted_before": before})
    };
    for (step, word, on_disk, expected, base, read_as) in [
        // 시작 기록 전: 아무 걸음도 시작하지 않았다. 기준은 확인된 그대로다.
        ("sync_start", "not_written_halt_requested", P1, steps("not_started", "not_started", "present", "start"), "present", "update_needed"),
        // 시작 기록 뒤, 파일 교체 전: 파일을 쓰지 않았고 기준은 unconfirmed 로 남는다.
        ("sync_file_write", "not_written_halted_base_unconfirmed", P1, steps("saved", "not_started", "unconfirmed", "file_write"), "unconfirmed", "update_needed_cause_unconfirmed"),
        // 파일 교체 뒤, 기준 저장 전: 파일은 적용 원문이고 기준은 unconfirmed — 재조회가 동기화됨으로 읽는다.
        ("sync_base_save", "written_halted_base_unconfirmed", P2, steps("saved", "replaced", "unconfirmed", "base_save"), "unconfirmed", "synced"),
    ] {
        let root = started(&format!("halt-{step}"));
        let (revision, digest) = proposed(&root, P2);
        HALT_BEFORE_STEP.with(|slot| *slot.borrow_mut() = Some(step));
        let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
        assert_eq!(decided["facts"]["policy_applied"], true, "{step}: 끝난 승인 저장은 그대로다: {decided}");
        assert_eq!(decided["facts"]["policy_file"], word, "{step}");
        assert_eq!(decided["facts"]["policy_file_steps"], expected, "{step}: 끝난 걸음과 시작하지 않은 걸음");
        // 보관은 시작하지 않았고 실패로 적히지 않았다. 중단은 적용됐다.
        assert_eq!(decided["facts"]["baseline"]["result"], "halted", "{step}: {decided}");
        assert_eq!(decided["facts"]["baseline"]["before"], "observe");
        assert_eq!(decided["facts"]["baseline"]["halt_request"], serde_json::json!({"received": true, "applied": true}));
        assert_eq!(decided["tracking"]["archive_halt"], "event:M2");
        assert!(failed_attempts(&root).is_empty(), "{step}");
        assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), on_disk, "{step}: 디스크의 정책 파일");
        // 저장된 기록: 적용 정책은 P2, 기준과 디스크의 판정은 재조회가 말한다.
        let check = |when: &str| {
            let seen = now(&root);
            assert_eq!(seen["tracking"]["applied_policy"], "policy:P2", "{step} {when}: 앞 판으로 돌아가지 않는다");
            assert_eq!(seen["tracking"]["policy_file_sync_base"], base, "{step} {when}: 기준을 추정으로 되돌리지 않는다");
            assert_eq!(seen["policy_file"], read_as, "{step} {when}");
            assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), on_disk, "{step} {when}: 스스로 다시 쓰지 않는다");
            assert_eq!(policies(&root), 2);
        };
        check("중단 중");
        // 해제도 다시 쓰기·기준 복구·보관을 시작하지 않는다.
        let shown = shown_halt(&root);
        release_as(&root, &shown, "continue", true).expect("해제");
        check("해제 뒤");
        assert!(failed_attempts(&root).is_empty());
        assert_eq!(now(&root)["tracking"]["waiting"]["event"], "event:M2", "{step}: 여전히 보관 대기다");
    }
}

/// 해제의 부분 저장: 해제 이력(원장)은 저장됐고 표식을 지우는 상태 저장은 실패했다. 표식과 차단이 남고, 다시 열어 읽는 것으로는 풀리지
/// 않으며, 옛 확인 근거는 다시 쓰이지 못한다. 명시 재등록이 만든 **새 중단 건**을 새로 확인해야만 해제된다. 재시도는 일어나지 않는다.
/// (함수 수준 주입 — 그 저장을 하지 않고 오류를 돌려준다. 실제 영속 장애의 증거가 아니다.)
#[test]
fn a_release_recorded_in_history_but_not_removed_from_the_state_keeps_the_halt_and_needs_a_new_record_and_confirmation() {
    let root = waiting("release-partial");
    halt(&root).expect("중단");
    let shown = shown_halt(&root);
    let attempts = failed_attempts(&root);
    let state = fs::read(root.join(".gil/state.yaml")).unwrap();

    SAVE_FAULT.with(|slot| *slot.borrow_mut() = Some(("release", false)));
    assert_eq!(release_as(&root, &shown, "continue", true).expect_err("표식을 지우지 못했다").code, "release_not_saved");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "표식이 그대로다");

    // 다시 열어 읽어도(새 인스턴스의 조회) 풀리지 않고, 진행은 여전히 막혀 있다.
    assert_eq!(now(&root)["tracking"]["archive_halt"], "event:M2");
    assert_eq!(retry(&root, "event:M2").expect_err("중단 중").code, "halted");
    assert!(open_session(&root).open_child_cycle(CycleKind::Interview).is_err());
    let (view, grants) = review(&root).expect("조회");
    assert_eq!(view["halt"]["record_missing"], true, "해제 이력이 적힌 건은 더는 확인 근거가 아니다: {view}");
    assert!(view["halt"]["id"].is_null());
    assert_eq!(grants, [(Purpose::Halt, "halt".to_string())], "해제는 제공되지 않고 기록을 다시 남기는 길만 있다");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "조회는 스스로 해제하지 않는다");
    // 옛 확인 근거는 다시 쓰이지 못한다.
    assert_eq!(release_as(&root, &shown, "continue", true).expect_err("옛 확인").code, "halt_record_missing");

    // 명시 재등록 → 새 중단 건. 옛 이름으로는 여전히 안 되고, 새 화면의 확인으로만 해제된다.
    assert_eq!(halt(&root).expect("재등록")["tracking"]["archive_halt"], "event:M2");
    assert_eq!(fs::read(root.join(".gil/state.yaml")).unwrap(), state, "재등록은 표식을 옮기지 않는다");
    let fresh = shown_halt(&root);
    assert_ne!(fresh.1, shown.1, "새 확인 대상이다");
    assert_eq!(release_as(&root, &shown, "continue", true).expect_err("옛 이름").code, "halt_confirmation_stale");
    let released = release_as(&root, &fresh, "continue", true).expect("새 확인으로 해제");
    assert!(released["tracking"]["archive_halt"].is_null());
    // 어느 걸음도 재시도·보관을 시작하지 않았다.
    assert_eq!(released["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(failed_attempts(&root), attempts);
    assert_eq!(policies(&root), 2);
}

/// 정책 파일 동기화의 저장 걸음: **저장 호출의 반환**과 **다시 읽어 확인한 저장 상태**는 다른 사실이다. 저장하지 못한 오류와 저장한 뒤의
/// 오류를 가르고, 다시 읽지 못하면 그 걸음과 기준을 미확정(`unknown`·`null`)으로 남긴다. 결과의 낱말·걸음별 사실, 실제 저장된 기준,
/// 이어지는 현재 상태 표시가 서로 어긋나지 않는다. 어느 경우에도 적용 정책은 P2 이고, 스스로 다시 쓰거나 기준을 되돌리지 않는다.
/// (함수 수준 주입 — 그 저장을 건너뛰거나 저장한 뒤 오류를 돌려주고, 다시 읽기 한 번을 실패로 돌려준다. 실제 디스크 장애·전원 장애의
/// 증거가 아니다.)
#[test]
fn a_policy_file_sync_save_is_reported_from_the_reread_state_not_from_the_save_call() {
    use serde_json::json;
    struct Case {
        label: &'static str,
        /// (자리, 저장한 뒤 오류인가) — 없으면 저장은 정상이다.
        save: Option<(&'static str, bool)>,
        /// 이 저장 뒤의 다시 읽기를 실패로 만든다.
        reread_fails: Option<&'static str>,
        word: &'static str,
        steps: Value,
        on_disk: &'static str,
        /// 실제로 저장된 기준과, 재조회가 읽는 정책 파일 상태.
        stored: &'static str,
        read_as: &'static str,
    }
    let steps = |start: (&str, &str), file: &str, base: (&str, Value), stored: Value| {
        let left = stored.as_str().map(|base| base == "unconfirmed");
        json!({"start": start.0, "start_save_returned": start.1, "file": file, "base": base.0, "base_save_returned": base.1,
            "stored_base": stored, "base_left_unconfirmed": left, "halted_before": null})
    };
    let cases = [
        // ── 시작 기록(sync_start)
        Case { label: "start-not-stored", save: Some(("sync_start", false)), reread_fails: None, word: "not_written_start_not_saved",
            steps: steps(("not_saved", "error"), "not_started", ("not_started", Value::Null), json!("present")),
            on_disk: P1, stored: "present", read_as: "update_needed" },
        // 저장한 뒤의 오류: 시작 기록은 저장돼 있다 — 미저장으로 말하지 않고 이어서 쓴다.
        Case { label: "start-stored-then-error", save: Some(("sync_start", true)), reread_fails: None, word: "written",
            steps: steps(("saved", "error"), "replaced", ("saved", json!("ok")), json!("present")),
            on_disk: P2, stored: "present", read_as: "synced" },
        // 다시 읽지 못했다: 저장은 됐지만 그것을 확인하지 못했으므로 파일을 쓰지 않고, 기준을 말하지 않는다.
        Case { label: "start-reread-fails-stored", save: None, reread_fails: Some("sync_start"), word: "not_written_start_unknown",
            steps: steps(("unknown", "ok"), "not_started", ("not_started", Value::Null), Value::Null),
            on_disk: P1, stored: "unconfirmed", read_as: "update_needed_cause_unconfirmed" },
        Case { label: "start-reread-fails-not-stored", save: Some(("sync_start", false)), reread_fails: Some("sync_start"),
            word: "not_written_start_unknown",
            steps: steps(("unknown", "error"), "not_started", ("not_started", Value::Null), Value::Null),
            on_disk: P1, stored: "present", read_as: "update_needed" },
        // ── 쓰기 뒤 기준 저장(sync_done)
        Case { label: "done-not-stored", save: Some(("sync_done", false)), reread_fails: None, word: "written_base_unconfirmed",
            steps: steps(("saved", "ok"), "replaced", ("not_saved", json!("error")), json!("unconfirmed")),
            on_disk: P2, stored: "unconfirmed", read_as: "synced" },
        // 저장한 뒤의 오류: 기준은 옮겨져 있다 — 「기준 미확인」으로 말하지 않는다.
        Case { label: "done-stored-then-error", save: Some(("sync_done", true)), reread_fails: None, word: "written",
            steps: steps(("saved", "ok"), "replaced", ("saved", json!("error")), json!("present")),
            on_disk: P2, stored: "present", read_as: "synced" },
        Case { label: "done-reread-fails-stored", save: None, reread_fails: Some("sync_done"), word: "written_base_unknown",
            steps: steps(("saved", "ok"), "replaced", ("unknown", json!("ok")), Value::Null),
            on_disk: P2, stored: "present", read_as: "synced" },
        Case { label: "done-reread-fails-not-stored", save: Some(("sync_done", false)), reread_fails: Some("sync_done"),
            word: "written_base_unknown",
            steps: steps(("saved", "ok"), "replaced", ("unknown", json!("error")), Value::Null),
            on_disk: P2, stored: "unconfirmed", read_as: "synced" },
    ];
    for case in cases {
        let label = case.label;
        let root = started(&format!("sync-{label}"));
        let (revision, digest) = proposed(&root, P2);
        SAVE_FAULT.with(|slot| *slot.borrow_mut() = case.save);
        SYNC_REREAD_FAULT.with(|slot| *slot.borrow_mut() = case.reread_fails);
        let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
        assert!(SAVE_FAULT.with(|slot| slot.borrow().is_none()) && SYNC_REREAD_FAULT.with(|slot| slot.borrow().is_none()), "{label}: 주입이 쓰였다");
        assert_eq!(decided["facts"]["policy_applied"], true, "{label}: {decided}");
        assert_eq!(decided["facts"]["policy_file"], case.word, "{label}");
        assert_eq!(decided["facts"]["policy_file_steps"], case.steps, "{label}");
        assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), case.on_disk, "{label}: 디스크의 정책 파일");
        // 결과가 말한 기준은 실제 저장된 기준과 같거나, 확인하지 못했으면 비어 있다 — 다른 값을 확정으로 말하지 않는다.
        let said = &decided["facts"]["policy_file_steps"]["stored_base"];
        assert!(said.is_null() || *said == case.stored, "{label}: 결과의 기준 {said} 과 실제 기준 {}", case.stored);
        // 같은 응답의 현재 상태 표시와, 그 뒤 새로 읽은 상태가 실제 저장 기준을 말한다.
        let seen = now(&root);
        for (when, view) in [("응답", &decided), ("재조회", &seen)] {
            assert_eq!(view["tracking"]["policy_file_sync_base"], case.stored, "{label} {when}: 실제 저장된 기준");
            assert_eq!(view["policy_file"], case.read_as, "{label} {when}: 정책 파일 상태");
            assert_eq!(view["tracking"]["applied_policy"], "policy:P2", "{label} {when}");
        }
        assert_eq!(policies(&root), 2, "{label}");
        // 재조회는 다시 쓰지도 기준을 고치지도 않는다.
        assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), case.on_disk, "{label}");
        assert_eq!(now(&root)["tracking"]["policy_file_sync_base"], case.stored, "{label}");
    }
}

/// 저장 걸음의 판정은 **저장된 상태 전체**로 한다(방어적 일관성 검사). 다시 읽은 상태가 저장 전 상태도, 이 저장의 예상 상태도 아니면 —
/// 기준값이 목표와 같더라도 — 그 걸음은 `unknown` 이다. `unknown` 을 근거로 다음 쓰기를 하지 않고, 기준을 말하지 않으며, 스스로 다시
/// 쓰거나 다시 시도하지 않는다. (정확한 이전 상태 = `not_saved`, 정확한 예상 상태 = `saved` 는 위 시험의 여덟 경우가 본다.)
///
/// 함수 수준 주입이다: 저장과 다시 읽기 사이에 시험이 같은 잠금으로 **다른 유효한 상태**를 저장해 둔다. 실제로 그런 일이 일어나는
/// 경로를 보이려는 것이 아니며, 실제 디스크 장애·전원 장애의 증거도 아니다.
#[test]
fn a_sync_save_whose_reread_state_is_neither_the_previous_nor_the_expected_one_is_unknown_and_drives_no_further_write() {
    use serde_json::json;
    // (자리, 무엇을 저장해 두는가, 그 뒤 실제 저장 기준, 재조회의 정책 파일 상태, 저장된 실패 시도 수)
    for (step, other, stored, read_as, failures) in [
        // 둘과 다른 유효한 상태 — 기준값부터 다르다.
        ("sync_start", "other_base", "absent", "user_changed", 0),
        // 기준값은 목표(unconfirmed)와 같지만 묶여 있던 기록이 다르다.
        ("sync_start", "same_base_other_record", "unconfirmed", "update_needed_cause_unconfirmed", 1),
        ("sync_done", "other_base", "absent", "synced", 0),
        // 기준값은 목표(적용 원문)와 같지만 묶여 있던 기록이 다르다.
        ("sync_done", "same_base_other_record", "present", "synced", 1),
    ] {
        let label = format!("{step}-{other}");
        let root = started(&format!("sync-bind-{label}"));
        let (revision, digest) = proposed(&root, P2);
        SYNC_AFTER_SAVE.with(|slot| *slot.borrow_mut() = Some((step, Box::new(move |session: &ProjectSession, expected: &Project| {
            let state = match other {
                "other_base" => session.project().with_sync_base(sp::SyncBase::Absent).expect("추적 기록이 있다"),
                _ => {
                    let event = expected.waiting_event().expect("M2 가 보관 대기다");
                    expected.with_failed_attempt(event, "external_change").expect("유효한 다른 상태")
                }
            };
            session.save_project(&state).expect("시험이 저장해 두는 다른 유효한 상태");
        }))));
        let decided = decide(&root, revision, &digest, true).expect("응답은 기록된다");
        assert!(SYNC_AFTER_SAVE.with(|slot| slot.borrow().is_none()), "{label}: 주입이 쓰였다");
        assert_eq!(decided["facts"]["policy_applied"], true, "{label}: {decided}");
        let steps = &decided["facts"]["policy_file_steps"];
        match step {
            "sync_start" => {
                assert_eq!(decided["facts"]["policy_file"], "not_written_start_unknown", "{label}");
                assert_eq!(*steps, json!({"start": "unknown", "start_save_returned": "ok", "file": "not_started", "base": "not_started",
                    "base_save_returned": null, "stored_base": null, "base_left_unconfirmed": null, "halted_before": null}), "{label}");
                assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P1, "{label}: unknown 을 근거로 파일을 쓰지 않는다");
            }
            _ => {
                assert_eq!(decided["facts"]["policy_file"], "written_base_unknown", "{label}");
                assert_eq!(*steps, json!({"start": "saved", "start_save_returned": "ok", "file": "replaced", "base": "unknown",
                    "base_save_returned": "ok", "stored_base": null, "base_left_unconfirmed": null, "halted_before": null}), "{label}");
                assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), P2, "{label}: 끝난 교체는 그대로다");
            }
        }
        // 동기화 시점의 결과는 기준을 말하지 않았다. 현재 상태는 따로, 실제 저장된 것을 말한다.
        let seen = now(&root);
        for (when, view) in [("응답", &decided), ("재조회", &seen)] {
            assert_eq!(view["tracking"]["policy_file_sync_base"], stored, "{label} {when}: 실제 저장된 기준");
            assert_eq!(view["policy_file"], read_as, "{label} {when}");
            assert_eq!(view["tracking"]["applied_policy"], "policy:P2", "{label} {when}");
        }
        // 동기화가 실패 이력을 더하지 않았고, 그 뒤 스스로 다시 쓰거나 기준을 고치지 않는다.
        assert_eq!(failed_attempts_raw(&fs::read(root.join(".gil/state.yaml")).unwrap()), failures, "{label}: 시험이 둔 것 말고는 없다");
        let on_disk = fs::read_to_string(root.join(".gilignore")).unwrap();
        assert_eq!(now(&root)["tracking"]["policy_file_sync_base"], stored, "{label}");
        assert_eq!(fs::read_to_string(root.join(".gilignore")).unwrap(), on_disk, "{label}");
        assert_eq!(policies(&root), 2, "{label}");
    }
}
