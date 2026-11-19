//! 단일 종료 이력과 미달성 종료 — **형식 8 의 도메인·저장 계약** (Storage Transition Contract §4).
//!
//! 여기서 재는 것:
//!
//! 1. 새 프로젝트는 형식 8 이고 Journey 는 `ended_wills` 하나만 지닌다.
//! 2. 미달성 닫기 한 번으로 Step·Report·자리 비움·이력 추가·Journey 판이 함께 바뀌고,
//!    저장했다 읽어도 같은 기록이다. 원래 약속은 한 글자도 바뀌지 않는다.
//! 3. 지목이 틀리면·이미 끝났으면·다른 약속이 걸려 있으면 아무것도 바뀌지 않는다.
//! 4. 미달성 종료는 Experiment 에만 있고(E7), 그 실험은 실패로만 닫힌다(E8).
//! 5. 미달성 종료가 없는 실험의 반복 문법은 그대로다.
//! 6. 손으로 고친 파일 — 꼴의 혼용·누락, 모르는 결과값, 어긋난 참조 — 은 읽히지 않는다.
//! 7. 형식 4·6 프로젝트는 같은 빌드에서도 `done_wills` 그대로 읽히고 쓰이며, 미달성 요청은
//!    전환 안내와 함께 거절된다.
//!
//! 맨 `approved: yes` 로 Interview 를 닫는 fixture 를 쓰므로 승인 계획 프로필에서는 짓지 않는다.
//! 그 프로필과의 조합은 기존 suite 를 형식 8 로 돌려 본다(CI 의 기능 조합 단계).
#![cfg(all(feature = "will-end-history", not(feature = "approved-plan")))]

use gil::{
    ActionError, Closed, CycleKind, EndResult, NodeKind, Project, ProjectError, Report, StoreError,
    WillRef, load, save,
};
use serde_norway::Value;

mod common;
use common::legacy::{self, Layout};
use common::{
    ACTION, TARGET, bootstrap, cycle_report, first_world, full_report, opened, scratch, spec,
    step_id, up_to_verify, walked,
};

fn journey(project: &Project) -> &gil::Journey {
    project.current_existence().journey()
}

fn active(project: &Project) -> WillRef {
    project.active_will().expect("걸린 행동이 있다").id()
}

/// 종료 이력을 견줄 수 있는 꼴로 — 이름·대상·결과.
fn history(project: &Project) -> Vec<(String, String, EndResult)> {
    journey(project)
        .ended_wills()
        .iter()
        .map(|ended| (ended.will().id().to_string(), ended.will().target().to_string(), ended.result()))
        .collect()
}

/// 거절된 요청 뒤에 **한 글자도 달라지지 않았는지** 재는 눈.
fn frozen(project: &Project) -> String {
    format!("{project:?}")
}

fn report(project: &Project, kind: NodeKind) -> Report {
    full_report(project.cycles().rules(), project.current_cycle().kind(), kind)
}

/// 열려 있는 자리를 지금 걸린 약속을 지목해 미달성으로 닫는다.
fn close_unachieved(project: &mut Project, kind: NodeKind) -> Result<Closed, ActionError> {
    let will = active(project);
    let report = report(project, kind);
    let world = (kind == NodeKind::Verify).then(first_world);
    project.close_step_ending(report, world, Some(will))
}

fn failing_outcome(project: &Project) -> Report {
    report(project, NodeKind::Outcome).with("verdict", "failure").with(ACTION, "close_cycle")
}

/// Define·Hypothesis 를 달성으로 닫고 Verify 를 열어 둔 Experiment.
fn at_open_verify() -> Project {
    let mut project = bootstrap();
    up_to_verify(&mut project);
    project
}

/// Verify 를 미달성으로 닫고 Analysis 까지 달성으로 닫은 Experiment.
fn after_unachieved_verify() -> Project {
    let mut project = at_open_verify();
    close_unachieved(&mut project, NodeKind::Verify).expect("미달성으로 닫는다");
    walked(&mut project, NodeKind::Analysis);
    project
}

/// 미달성 Verify 뒤 실패로 끝까지 닫은 Experiment — Outcome 과 Cycle Report 까지.
fn unachieved_experiment_closed() -> Project {
    let mut project = after_unachieved_verify();
    let outcome = opened(&mut project, NodeKind::Outcome);
    let report = failing_outcome(&project);
    project.close_action_step(report).expect("실패 판정으로 닫는다");
    let report = cycle_report(project.current_cycle(), "failure", step_id(&project, outcome));
    project.close_cycle(report).expect("실패 Cycle 을 닫는다");
    project
}

fn stored(label: &str, project: &Project) -> (std::path::PathBuf, Value) {
    let path = scratch(label).join(gil::STATE_PATH);
    save(project, &path).expect("저장한다");
    let file = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    (path, file)
}

fn x1(file: &mut Value) -> &mut Value {
    &mut file["existences"]["X1"]["journey"]
}

/// 저장한 파일을 손으로 고친 뒤 읽는다 — 읽히면 그 자체가 실패다.
fn forged(project: &Project, label: &str, edit: impl FnOnce(&mut Value)) -> StoreError {
    let (path, mut file) = stored(label, project);
    edit(&mut file);
    std::fs::write(&path, serde_norway::to_string(&file).unwrap()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let err = match load(spec(), &path) {
        Err(err) => err,
        Ok(_) => panic!("걸어서 만들 수 없는 종료 이력이 읽혔다"),
    };
    assert_eq!(std::fs::read(&path).unwrap(), before, "거절하면서 파일을 바꿨다");
    err
}

fn not_whole(err: StoreError) -> ProjectError {
    match err {
        StoreError::NotWhole(err) => err,
        other => panic!("도메인 불변식 거절이 아니다: {other}"),
    }
}

// ── ① 형식과 꼴 ────────────────────────────────────────────────────────────

#[test]
fn a_new_project_is_format_8_and_its_journey_has_only_the_end_history() {
    let project = Project::start(spec(), first_world());
    assert!(project.records_end_results());
    let (_, mut file) = stored("end-new-format", &project);

    assert_eq!(file["format"], Value::from(gil::END_HISTORY_FORMAT));
    assert_eq!(x1(&mut file)["ended_wills"], Value::Sequence(Vec::new()));
    assert!(x1(&mut file).get("done_wills").is_none(), "두 목록이 함께 있는 꼴은 없다");
    assert!(file.get("chains").is_some() && file.get("cycles").is_none(), "종료 이력은 Chain 형식에만 있다");
}

// ── ② 한 번의 확정과 다시 읽기 ─────────────────────────────────────────────

#[test]
fn one_unachieved_close_moves_step_report_register_history_and_journey_together() {
    let mut project = at_open_verify();
    let promised = project.active_will().unwrap().clone();
    let journey_before = project.current_existence().current_journey();
    let done_before = history(&project).len();

    let closed = close_unachieved(&mut project, NodeKind::Verify).expect("미달성으로 닫는다");
    assert_eq!(closed.result, EndResult::Unachieved);
    assert_eq!(closed.will, promised.id());
    assert_eq!(closed.step, promised.target());

    // 단계·보고서·세계·판.
    let cycle = project.current_cycle();
    let node = cycle.steps().node(step_id(&project, closed.step)).unwrap();
    assert!(node.is_closed() && node.report.is_some() && node.snapshot.is_some());
    assert_eq!(node.journey, Some(closed.journey));
    assert_ne!(closed.journey, journey_before, "새 판이 나야 한다");
    assert_eq!(project.current_existence().current_journey(), closed.journey);
    assert_eq!(project.current_existence().current_revision().unwrap().will_head(), Some(promised.id()));

    // 자리는 비고 이력은 하나 는다 — **원래 약속 그대로.**
    assert!(project.active_will().is_none());
    assert_eq!(history(&project).len(), done_before + 1);
    let last = journey(&project).last_ended_will().unwrap();
    assert_eq!(last.will(), &promised, "종료하면서 약속의 내용이나 대상을 바꿨다");
    assert_eq!(last.result(), EndResult::Unachieved);
    assert_eq!(project.recorded_end_result(closed.step), Some(EndResult::Unachieved));

    // 달성만 필요한 조회는 이력에서 고른다.
    assert!(journey(&project).done_wills().iter().all(|will| will.id() != promised.id()));
}

#[test]
fn achieved_and_unachieved_ends_read_back_the_same_and_resave_to_the_same_bytes() {
    let project = unachieved_experiment_closed();
    let (path, mut file) = stored("end-roundtrip", &project);
    let bytes = std::fs::read(&path).unwrap();

    let again = load(spec(), &path).expect("걸어서 만든 종료 이력은 되살아난다");
    assert!(again.records_end_results());
    assert_eq!(history(&again), history(&project));
    assert_eq!(
        history(&again).iter().filter(|(_, _, result)| *result == EndResult::Unachieved).count(),
        1,
        "미달성은 Verify 하나다"
    );
    for (before, after) in journey(&project).ended_wills().iter().zip(journey(&again).ended_wills()) {
        assert_eq!(before, after, "약속의 원문이 프로세스를 넘으며 달라졌다");
    }

    // 조회와 재저장은 기록을 바꾸지 않는다.
    save(&again, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // 기록의 칸은 약속의 여섯과 결과 하나뿐이다 — 사유·부분 결과·잠정 답의 자리가 없다.
    let record = x1(&mut file)["ended_wills"].as_sequence().unwrap().last().unwrap().clone();
    let mut keys: Vec<&str> = record.as_mapping().unwrap().keys().map(|key| key.as_str().unwrap()).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["done_when", "end_result", "existence_ref", "id", "next_action", "objective", "target_node_ref"]);
    let results: Vec<&str> = x1(&mut file)["ended_wills"].as_sequence().unwrap().iter()
        .map(|record| record["end_result"].as_str().unwrap()).collect();
    assert!(results.iter().all(|result| matches!(*result, "achieved" | "unachieved")));
    assert_eq!(results.iter().filter(|result| **result == "unachieved").count(), 1);
}

// ── ③ 지목·재전송·오래된 요청 ──────────────────────────────────────────────

#[test]
fn a_marker_that_does_not_name_the_active_will_changes_nothing() {
    let mut project = at_open_verify();
    let before = frozen(&project);
    let here = project.active_will().unwrap().target();

    // 발급된 적 없는 이름.
    let ghost = WillRef::new(99).unwrap();
    let err = project.close_step_ending(report(&project, NodeKind::Verify), Some(first_world()), Some(ghost)).unwrap_err();
    assert_eq!(err, ActionError::UnachievedWillMismatch { asked: ghost, active: Some((active(&project), here)) });
    assert_eq!(frozen(&project), before);

    // 이미 달성으로 끝난 약속 — 결과는 바뀌지 않는다.
    let done = journey(&project).ended_wills().last().unwrap().clone();
    let err = project.close_step_ending(report(&project, NodeKind::Verify), Some(first_world()), Some(done.will().id())).unwrap_err();
    assert_eq!(err, ActionError::WillAlreadyEnded {
        will: done.will().id(), target: done.will().target(), result: EndResult::Achieved,
    });
    assert_eq!(frozen(&project), before);
    assert_eq!(project.recorded_end_result(done.will().target()), Some(EndResult::Achieved));
}

#[test]
fn resending_the_same_request_never_writes_a_second_record_or_closes_a_newer_step() {
    let mut project = at_open_verify();
    let closed = close_unachieved(&mut project, NodeKind::Verify).unwrap();
    let ended = WillRef::new(closed.will.number()).unwrap();
    let already = ActionError::WillAlreadyEnded { will: ended, target: closed.step, result: EndResult::Unachieved };

    // 응답을 잃고 같은 요청을 다시 보낸다 — 닫을 자리가 없어도 「이미 끝났다」로 답한다.
    let before = frozen(&project);
    let err = project.close_step_ending(report(&project, NodeKind::Verify), Some(first_world()), Some(ended)).unwrap_err();
    assert_eq!(err, already);
    assert_eq!(project.check_unachieved_request(ended).unwrap_err(), already);
    assert_eq!(frozen(&project), before);

    // 그사이 다음 자리가 열렸다. 오래된 요청은 그 자리를 닫지 못한다.
    let analysis = opened(&mut project, NodeKind::Analysis);
    let newer = active(&project);
    let before = frozen(&project);
    let err = project.close_step_ending(report(&project, NodeKind::Analysis), None, Some(ended)).unwrap_err();
    assert_eq!(err, already);
    assert_eq!(frozen(&project), before);
    assert_eq!(active(&project), newer);
    assert!(!project.current_cycle().steps().node(step_id(&project, analysis)).unwrap().is_closed());
    assert_eq!(history(&project).iter().filter(|(id, _, _)| *id == ended.to_string()).count(), 1);
}

// ── ④ E7 · E8 ──────────────────────────────────────────────────────────────

#[test]
fn an_interview_step_is_never_ended_as_unachieved_and_keeps_waiting() {
    let mut project = Project::start(spec(), first_world());
    let question = opened(&mut project, NodeKind::Question);
    let will = active(&project);
    let before = frozen(&project);

    let err = project.close_action_step_unachieved(report(&project, NodeKind::Question), will).unwrap_err();
    assert_eq!(err, ActionError::UnachievedOutsideExperiment { will, target: question, kind: CycleKind::Interview });
    assert_eq!(frozen(&project), before, "응답 대기 상태가 바뀌었다");
    assert_eq!(active(&project), will);
    assert!(!project.current_cycle().steps().node(step_id(&project, question)).unwrap().is_closed());
}

#[test]
fn an_experiment_with_an_unachieved_end_refuses_a_new_hypothesis_success_and_inner_revisit() {
    let mut project = after_unachieved_verify();
    let cycle = project.current_cycle().id().to_ref();
    let unachieved = project.unachieved_in(cycle).unwrap().will().clone();

    // 문법만 보면 hypothesis 도 열린다. 종료 이력까지 본 안내는 outcome 하나다.
    assert!(project.current_cycle().openable_here().contains(&NodeKind::Hypothesis));
    assert_eq!(project.openable_here(), vec![NodeKind::Outcome]);
    let before = frozen(&project);
    let err = project.open_action_step(NodeKind::Hypothesis, common::plan(NodeKind::Hypothesis)).unwrap_err();
    assert_eq!(err, ActionError::UnachievedExperimentNoRetry { cycle, will: unachieved.id(), step: unachieved.target() });
    assert_eq!(frozen(&project), before);

    // Outcome: success 도, 실험 안 되돌아가기도 받지 않는다.
    let outcome = opened(&mut project, NodeKind::Outcome);
    let before = frozen(&project);
    let must_fail = ActionError::UnachievedExperimentMustFail { cycle, will: unachieved.id(), step: outcome };
    let success = report(&project, NodeKind::Outcome).with("verdict", "success").with(ACTION, "close_cycle");
    assert_eq!(project.close_action_step(success).unwrap_err(), must_fail);
    let analysis = project.current_cycle().steps().nodes().iter().find(|node| node.kind == NodeKind::Analysis).unwrap().id;
    let revisit = report(&project, NodeKind::Outcome).with("verdict", "failure").with(ACTION, "revisit")
        .with(TARGET, project.current_cycle().step_ref(analysis).to_string());
    assert_eq!(project.close_action_step(revisit).unwrap_err(), must_fail);
    assert_eq!(frozen(&project), before);

    // failure · close_cycle 만 닫힌다. Cycle 도 실패로만 닫히고 방향은 기존 규칙대로 revisit 이다.
    project.close_action_step(failing_outcome(&project)).expect("실패 판정은 닫힌다");
    let before = frozen(&project);
    let success = cycle_report(project.current_cycle(), "success", step_id(&project, outcome));
    assert!(project.close_cycle(success).is_err(), "미달성 실험이 성공으로 닫혔다");
    assert_eq!(frozen(&project), before);
    let failure = cycle_report(project.current_cycle(), "failure", step_id(&project, outcome));
    assert_eq!(failure.get(ACTION), Some("revisit"));
    project.close_cycle(failure).expect("실패 Cycle 은 닫힌다");

    // 실패 판정은 되돌아갈 준비일 뿐이다 — Chain 을 닫지 않는다.
    assert!(project.cycles().can_revisit());
    assert!(!project.current_chain().unwrap().is_closed());
}

#[test]
fn no_step_is_skipped_after_an_unachieved_end() {
    // Hypothesis 가 미달성이어도 Verify 를 열고 닫아야 Analysis 로 간다.
    let mut project = bootstrap();
    walked(&mut project, NodeKind::Define);
    opened(&mut project, NodeKind::Hypothesis);
    close_unachieved(&mut project, NodeKind::Hypothesis).expect("미달성 Hypothesis");
    assert_eq!(project.openable_here(), vec![NodeKind::Verify]);
    assert!(project.open_action_step(NodeKind::Analysis, common::plan(NodeKind::Analysis)).is_err());
    walked(&mut project, NodeKind::Verify);
    walked(&mut project, NodeKind::Analysis);
    assert_eq!(project.openable_here(), vec![NodeKind::Outcome]);

    // Define 이 미달성이면 첫 Hypothesis 는 열린다 — 다시 시도가 아니라 문법의 다음 자리다.
    let mut project = bootstrap();
    opened(&mut project, NodeKind::Define);
    close_unachieved(&mut project, NodeKind::Define).expect("미달성 Define");
    assert_eq!(project.openable_here(), vec![NodeKind::Hypothesis]);
    walked(&mut project, NodeKind::Hypothesis);
}

#[test]
fn an_unachieved_outcome_itself_closes_only_as_failure() {
    let mut project = bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut project, kind);
    }
    let outcome = opened(&mut project, NodeKind::Outcome);
    let will = active(&project);
    let cycle = project.current_cycle().id().to_ref();
    let before = frozen(&project);
    let success = report(&project, NodeKind::Outcome).with("verdict", "success").with(ACTION, "close_cycle");
    assert_eq!(
        project.close_action_step_unachieved(success, will).unwrap_err(),
        ActionError::UnachievedExperimentMustFail { cycle, will, step: outcome }
    );
    assert_eq!(frozen(&project), before);
    let closed = project.close_action_step_unachieved(failing_outcome(&project), will).unwrap();
    assert_eq!(closed.result, EndResult::Unachieved);
}

// ── ⑤ 일반 실험의 문법은 그대로다 ──────────────────────────────────────────

#[test]
fn an_experiment_without_an_unachieved_end_keeps_its_repetition_grammar() {
    let mut project = bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut project, kind);
    }
    // 같은 실험 안의 다음 가설.
    assert_eq!(project.openable_here(), project.current_cycle().openable_here());
    assert!(project.openable_here().contains(&NodeKind::Hypothesis));
    for kind in [NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut project, kind);
    }
    // 형제 가설로 되돌아가기.
    opened(&mut project, NodeKind::Outcome);
    let analysis = project.current_cycle().steps().nodes().iter().find(|node| node.kind == NodeKind::Analysis).unwrap().id;
    let revisit = report(&project, NodeKind::Outcome).with("verdict", "failure").with(ACTION, "revisit")
        .with(TARGET, project.current_cycle().step_ref(analysis).to_string());
    project.close_action_step(revisit).expect("미달성이 없으면 실험 안 되돌아가기가 닫힌다");
    project.cycles_mut().current_mut().revisit_step().expect("되돌아간다");
    walked(&mut project, NodeKind::Hypothesis);

    assert!(history(&project).iter().all(|(_, _, result)| *result == EndResult::Achieved));
    assert!(project.unachieved_in(project.current_cycle().id().to_ref()).is_none());

    // 그 기록은 저장을 넘어서도 읽힌다 — E8 은 미달성이 있는 실험에만 걸린다.
    let (path, _) = stored("end-plain-grammar", &project);
    load(spec(), &path).expect("일반 반복이 있는 형식 8 기록");
}

#[test]
fn an_achieved_will_can_still_end_in_a_falsified_question() {
    // 약속을 달성해도 관측이 질문을 반증하면 실험은 실패한다 — 그것은 미달성이 아니다.
    let mut project = bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut project, kind);
    }
    let outcome = opened(&mut project, NodeKind::Outcome);
    project.close_action_step(failing_outcome(&project)).unwrap();
    let failure = cycle_report(project.current_cycle(), "failure", step_id(&project, outcome));
    project.close_cycle(failure).unwrap();

    assert!(history(&project).iter().all(|(_, _, result)| *result == EndResult::Achieved));
    assert!(project.all_cycles().all(|cycle| project.unachieved_in(cycle.id().to_ref()).is_none()));
}

// ── ⑥ 손으로 고친 파일 ─────────────────────────────────────────────────────

#[test]
fn mixed_missing_or_unknown_end_fields_are_refused() {
    let project = unachieved_experiment_closed();
    let last = |file: &mut Value| x1(file)["ended_wills"].as_sequence_mut().unwrap().last_mut().unwrap().as_mapping_mut().unwrap().clone();

    // 결과가 빠졌다 · 모르는 값이다 · 사유 칸이 끼어들었다.
    let err = forged(&project, "end-forged-missing", |file| {
        let mut record = last(file);
        record.remove(Value::from("end_result"));
        *x1(file)["ended_wills"].as_sequence_mut().unwrap().last_mut().unwrap() = Value::Mapping(record);
    });
    assert!(matches!(err, StoreError::Decode(_)), "{err}");
    let err = forged(&project, "end-forged-value", |file| {
        x1(file)["ended_wills"].as_sequence_mut().unwrap().last_mut().unwrap()["end_result"] = Value::from("abandoned");
    });
    assert!(matches!(err, StoreError::Decode(_)), "{err}");
    let err = forged(&project, "end-forged-reason", |file| {
        x1(file)["ended_wills"].as_sequence_mut().unwrap().last_mut().unwrap()["reason"] = Value::from("시간이 없었다");
    });
    assert!(matches!(err, StoreError::Decode(_)), "{err}");

    // 두 목록이 함께 있다 · 형식 8 에 done_wills 만 있다 · 둘 다 없다.
    let err = forged(&project, "end-forged-both", |file| {
        x1(file)["done_wills"] = Value::Sequence(Vec::new());
    });
    assert!(matches!(err, StoreError::EndHistory(_)), "{err}");
    let err = forged(&project, "end-forged-done-only", |file| {
        let journey = x1(file).as_mapping_mut().unwrap();
        journey.remove(Value::from("ended_wills"));
        journey.insert(Value::from("done_wills"), Value::Sequence(Vec::new()));
    });
    assert!(matches!(err, StoreError::EndHistory(_)), "{err}");
    let err = forged(&project, "end-forged-neither", |file| {
        x1(file).as_mapping_mut().unwrap().remove(Value::from("ended_wills"));
    });
    assert!(matches!(err, StoreError::EndHistory(_)), "{err}");

    // 번호만 낮춘 파일 — 형식 6 에는 ended_wills 가 없다.
    let err = forged(&project, "end-forged-lowered", |file| {
        file["format"] = Value::from(gil::CHAIN_FORMAT);
    });
    assert!(matches!(err, StoreError::EndHistory(_)), "{err}");
    // 시각 등록부 없는 9 — 형식과 구조가 어긋난다.
    let err = forged(&project, "end-forged-odd", |file| {
        file["format"] = Value::from(gil::END_HISTORY_VISUAL_FORMAT);
    });
    assert!(matches!(err, StoreError::VisualState(_)), "{err}");
}

/// 목록 키는 **값이 아니라 존재로** 판정한다 — `null` 도, 빈 목록도, 값 있는 목록도 같은 키다.
///
/// 형식 8 에는 `done_wills` 가 없어야 하고, 4–7 에는 `ended_wills` 가 없어야 한다. 그 형식이
/// 요구하는 목록은 빠져도 `null` 이어도 거절한다. 어느 거절도 파일을 바꾸지 않는다(`forged`).
#[test]
fn a_list_key_is_judged_by_its_presence_null_and_empty_lists_included() {
    // 형식 8: 금지 키 `done_wills` 의 세 값, 필수 키 `ended_wills` 의 누락과 null.
    let project = unachieved_experiment_closed();
    let (_, mut file) = stored("end-presence-source", &project);
    let mut plain = x1(&mut file)["ended_wills"][0].clone();
    plain.as_mapping_mut().unwrap().remove(Value::from("end_result"));
    for (label, value) in [("null", Value::Null), ("empty", Value::Sequence(Vec::new())), ("value", Value::Sequence(vec![plain.clone()]))] {
        let err = forged(&project, &format!("end-presence-8-done-{label}"), |file| x1(file)["done_wills"] = value.clone());
        assert!(matches!(err, StoreError::EndHistory(_)), "format 8 with done_wills {label}: {err}");
    }
    let err = forged(&project, "end-presence-8-ended-null", |file| x1(file)["ended_wills"] = Value::Null);
    assert!(matches!(err, StoreError::EndHistory(_)), "format 8 with ended_wills null: {err}");
    let err = forged(&project, "end-presence-8-ended-missing", |file| { x1(file).as_mapping_mut().unwrap().remove(Value::from("ended_wills")); });
    assert!(matches!(err, StoreError::EndHistory(_)), "format 8 without ended_wills: {err}");

    // 형식 4·6: 금지 키 `ended_wills` 의 세 값, 필수 키 `done_wills` 의 누락과 null.
    for (layout, format) in [(Layout::Cycles, gil::FORMAT), (Layout::Chain, gil::CHAIN_FORMAT)] {
        let dir = scratch(&format!("end-presence-legacy-{format}"));
        let mut legacy = load(spec(), legacy::record(&dir, layout)).unwrap();
        walked(&mut legacy, NodeKind::Question);
        let (_, mut file) = stored(&format!("end-presence-legacy-{format}-source"), &legacy);
        assert_eq!(file["format"], Value::from(format));
        let mut ended = x1(&mut file)["done_wills"][0].clone();
        ended["end_result"] = Value::from("achieved");
        for (label, value) in [("null", Value::Null), ("empty", Value::Sequence(Vec::new())), ("value", Value::Sequence(vec![ended.clone()]))] {
            let err = forged(&legacy, &format!("end-presence-{format}-ended-{label}"), |file| x1(file)["ended_wills"] = value.clone());
            assert!(matches!(err, StoreError::EndHistory(_)), "format {format} with ended_wills {label}: {err}");
        }
        let err = forged(&legacy, &format!("end-presence-{format}-done-null"), |file| x1(file)["done_wills"] = Value::Null);
        assert!(matches!(err, StoreError::EndHistory(_)), "format {format} with done_wills null: {err}");
        let err = forged(&legacy, &format!("end-presence-{format}-done-missing"), |file| { x1(file).as_mapping_mut().unwrap().remove(Value::from("done_wills")); });
        assert!(matches!(err, StoreError::EndHistory(_)), "format {format} without done_wills: {err}");

        // 정상 구형 기록은 그대로 읽히고 같은 바이트로 다시 쓰인다.
        let (path, _) = stored(&format!("end-presence-legacy-{format}-kept"), &legacy);
        let bytes = std::fs::read(&path).unwrap();
        save(&load(spec(), &path).expect("정상 구형 기록"), &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

/// 이력 끝에 「있었던 적 없는 종료 기록」을 덧붙인다 — 판의 머리까지 맞춰서, E4 만 걸리게.
fn append_ghost_record(file: &mut Value, target: &str) {
    let next = file["next_will_id"].as_u64().unwrap();
    let ghost: Value = serde_norway::from_str(&format!(
        "id: W{next}\nexistence_ref: existence:X1\ntarget_node_ref: {target}\n\
         objective: 무엇을\nnext_action: 무엇을\ndone_when: 무엇을\nend_result: achieved\n"
    )).unwrap();
    x1(file)["ended_wills"].as_sequence_mut().unwrap().push(ghost);
    file["next_will_id"] = Value::from(next + 1);
    let revisions = x1(file)["revisions"].as_mapping().unwrap().len();
    x1(file)["revisions"][format!("J{revisions}").as_str()] = serde_norway::from_str(&format!(
        "existence_state_ref: state:ES0\nknowledge_head_ref: null\nmemory_head_ref: null\n\
         relations_head_ref: null\nwill_head_ref: will:W{next}\n"
    )).unwrap();
    file["existences"]["X1"]["current_journey_ref"] = Value::from(format!("journey:X1@J{revisions}"));
}

#[test]
fn an_end_record_must_point_at_exactly_one_closed_step() {
    // 열린 Chain 의 닫힌 Experiment — Current 판을 옮겨도 Chain 검사에 걸리지 않는다.
    let closed = unachieved_experiment_closed();
    let err = not_whole(forged(&closed, "end-forged-no-step", |file| append_ghost_record(file, "step:C2/S99")));
    assert!(matches!(err, ProjectError::EndedWillWithoutStep { .. }), "{err}");
    let err = not_whole(forged(&closed, "end-forged-twice", |file| append_ghost_record(file, "step:C2/S1")));
    assert!(matches!(err, ProjectError::TwoEndedWillsForOneStep { .. }), "{err}");

    // 대상 자리가 아직 열려 있다.
    let open = at_open_verify();
    let target = open.active_will().unwrap().target().to_string();
    let err = not_whole(forged(&open, "end-forged-open-step", |file| append_ghost_record(file, &target)));
    assert!(matches!(err, ProjectError::EndedWillOnOpenStep { .. }), "{err}");
}

/// 그 자리를 대상으로 한 종료 기록의 결과를 뒤집는다.
fn flip_to_unachieved(file: &mut Value, target: &str) {
    let record = x1(file)["ended_wills"].as_sequence_mut().unwrap().iter_mut()
        .find(|record| record["target_node_ref"].as_str() == Some(target))
        .unwrap_or_else(|| panic!("{target} 의 종료 기록이 있어야 한다"));
    record["end_result"] = Value::from("unachieved");
}

fn step_of(project: &Project, kind: NodeKind) -> String {
    let cycle = project.current_cycle();
    let node = cycle.steps().nodes().iter().find(|node| node.kind == kind).expect("그 종류의 자리가 있다");
    cycle.step_ref(node.id).to_string()
}

#[test]
fn a_forged_unachieved_result_is_caught_by_the_experiment_it_would_bind() {
    // E7: Interview 의 약속을 미달성으로 뒤집는다.
    let project = bootstrap();
    let err = not_whole(forged(&project, "end-forged-interview", |file| flip_to_unachieved(file, "step:C1/S1")));
    assert!(matches!(err, ProjectError::UnachievedOutsideExperiment { kind: CycleKind::Interview, .. }), "{err}");

    // E8: 성공으로 닫힌 실험의 Verify 를 미달성으로 뒤집는다.
    let success = common::graph_with_one_closed_cycle("success");
    let verify = step_of(&success, NodeKind::Verify);
    let err = not_whole(forged(&success, "end-forged-success", |file| flip_to_unachieved(file, &verify)));
    assert!(matches!(err, ProjectError::UnachievedExperimentNotFailed { .. }), "{err}");

    // E8: 그 뒤에 같은 실험 안에서 새 가설을 연 기록.
    let mut retried = bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut retried, kind);
    }
    let first_verify = step_of(&retried, NodeKind::Verify);
    let err = not_whole(forged(&retried, "end-forged-retried", |file| flip_to_unachieved(file, &first_verify)));
    assert!(matches!(err, ProjectError::UnachievedExperimentRetried { .. }), "{err}");

    // E8: 그 뒤에 실험 안 되돌아가기를 적은 Outcome.
    let mut revisited = bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
        walked(&mut revisited, kind);
    }
    opened(&mut revisited, NodeKind::Outcome);
    let analysis = step_of(&revisited, NodeKind::Analysis);
    let inner = report(&revisited, NodeKind::Outcome).with("verdict", "failure").with(ACTION, "revisit").with(TARGET, analysis);
    revisited.close_action_step(inner).expect("미달성이 없으면 실험 안 되돌아가기가 닫힌다");
    let verify = step_of(&revisited, NodeKind::Verify);
    let err = not_whole(forged(&revisited, "end-forged-revisit", |file| flip_to_unachieved(file, &verify)));
    assert!(matches!(err, ProjectError::UnachievedExperimentNotFailed { .. } | ProjectError::UnachievedExperimentRetried { .. }), "{err}");
}

// ── ⑦ 형식 4·6 은 그대로다 ─────────────────────────────────────────────────

#[test]
fn format_4_and_6_keep_done_wills_resave_to_the_same_bytes_and_refuse_the_marker() {
    for (layout, format) in [(Layout::Chain, gil::CHAIN_FORMAT), (Layout::Cycles, gil::FORMAT)] {
        let dir = scratch(&format!("end-legacy-{format}"));
        let path = legacy::record(&dir, layout);
        let fresh = std::fs::read(&path).unwrap();

        // 조회와 재저장으로 형식이 오르지 않는다 — 같은 바이트다.
        let mut project = load(spec(), &path).expect("기존 형식은 읽힌다");
        assert!(!project.records_end_results());
        save(&project, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), fresh, "형식 {format} 의 재저장이 바이트를 바꿨다");

        // 달성 닫기는 이전과 같다.
        walked(&mut project, NodeKind::Question);
        opened(&mut project, NodeKind::Interpretation);
        save(&project, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let file: Value = serde_norway::from_str(&text).unwrap();
        assert_eq!(file["format"], Value::from(format));
        assert_eq!(file["existences"]["X1"]["journey"]["done_wills"].as_sequence().map(Vec::len), Some(1));
        assert!(!text.contains("ended_wills") && !text.contains("end_result"), "형식 {format} 에 새 꼴이 섞였다");
        assert_eq!(project.recorded_end_result("step:C1/S1".parse().unwrap()), None, "옛 기록에 결과를 소급해 붙이지 않는다");

        // 미달성 요청은 전환 안내와 함께 거절된다 — 조용히 달성으로 닫지 않는다.
        let before = frozen(&project);
        let will = active(&project);
        let err = project.close_action_step_unachieved(report(&project, NodeKind::Interpretation), will).unwrap_err();
        assert_eq!(err, ActionError::EndHistoryUnsupported { format });
        let said = err.to_string();
        assert!(said.contains("8/9") && said.contains(&format.to_string()), "{said}");
        assert_eq!(frozen(&project), before);
        save(&project, &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn a_format_6_experiment_keeps_its_grammar_and_cannot_record_an_unachieved_end() {
    let dir = scratch("end-legacy-experiment");
    let path = legacy::record(&dir, Layout::Chain);
    let mut project = common::bootstrap_from(load(spec(), &path).unwrap());
    up_to_verify(&mut project);

    let before = frozen(&project);
    let err = close_unachieved(&mut project, NodeKind::Verify).unwrap_err();
    assert_eq!(err, ActionError::EndHistoryUnsupported { format: gil::CHAIN_FORMAT });
    assert_eq!(frozen(&project), before);

    walked_verify_then_analysis(&mut project);
    assert!(project.openable_here().contains(&NodeKind::Hypothesis));
    save(&project, &path).unwrap();
    let again = load(spec(), &path).unwrap();
    assert!(!again.records_end_results());
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("format: 6\n"));
}

fn walked_verify_then_analysis(project: &mut Project) {
    let report = report(project, NodeKind::Verify);
    project.close_verify_step(report, first_world()).expect("달성 Verify");
    walked(project, NodeKind::Analysis);
}
