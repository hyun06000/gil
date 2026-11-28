//! Exploration contract — domain and storage boundaries (2026-11-17 agreement).
//!
//! These tests drive the real `Project` API and the real loader. The synthetic decisions are
//! harness inputs, not a person's click, and nothing here judges whether a Define really
//! contributes to its requirement: the Core checks references, not the meaning of prose. The
//! tool path (stdio, files, approved recovery) is covered by `tests/exploration_flow.rs`.
use super::tests::{approve_close, project as item_queue_project, synthesis};
use super::*;
use crate::plan_test_helpers as h;

fn exploration_project() -> Project {
    let project = Project::start(h::spec(), h::first_world());
    assert!(project.exploration_contract(), "an exploration-flow build opens new Chains under the new contract");
    project
}
fn link(p: &Project, requirement: &str) -> ExperimentLink {
    let active = p.approved_plan().unwrap().active(p.current_chain().unwrap()).expect("an approved revision");
    link_to(active.revision, &active.digest, requirement)
}
fn link_to(revision: u32, digest: &str, requirement: &str) -> ExperimentLink {
    ExperimentLink {
        revision,
        digest: digest.into(),
        requirement_id: requirement.into(),
        selection: Selection {
            relevance: format!("{requirement}의 답에 필요한 관측이다"),
            readiness: "필요한 입력이 준비돼 있다".into(),
            uncertainty: "아직 모르는 한 가지를 줄인다".into(),
        },
    }
}
/// Closes the open Experiment with that verdict; a failure declares `revisit` to its parent.
fn close_experiment(p: &mut Project, verdict: &str) -> crate::CycleRef {
    let here = p.current_cycle().id().to_ref();
    let outcome = h::walk_to_the_exit(p, verdict);
    p.close_cycle(h::cycle_report(p.current_cycle(), verdict, outcome)).unwrap();
    here
}
fn roundtrip(p: &Project, label: &str) -> (Project, Vec<u8>) {
    let root = h::scratch(label);
    let path = root.join(crate::STATE_PATH);
    crate::save(p, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let loaded = crate::load(h::spec(), &path).unwrap();
    crate::save(&loaded, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "a reload writes the same bytes");
    std::fs::remove_dir_all(root).ok();
    (loaded, bytes)
}
fn load_edited(p: &Project, label: &str, edit: impl FnOnce(&mut serde_norway::Value)) -> Result<Project, crate::StoreError> {
    let root = h::scratch(label);
    let path = root.join(crate::STATE_PATH);
    crate::save(p, &path).unwrap();
    let mut file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut file);
    std::fs::write(&path, serde_norway::to_string(&file).unwrap()).unwrap();
    let loaded = crate::load(h::spec(), &path);
    std::fs::remove_dir_all(root).ok();
    loaded
}

#[test]
fn a_new_chain_carries_the_marker_and_an_item_queue_chain_is_stored_exactly_as_before() {
    let (loaded, bytes) = roundtrip(&exploration_project(), "explore-marker-new");
    assert!(loaded.exploration_contract());
    assert!(String::from_utf8(bytes).unwrap().contains("contract: exploration"));

    let (loaded, bytes) = roundtrip(&item_queue_project(), "explore-marker-old");
    assert!(!loaded.exploration_contract());
    assert!(!String::from_utf8(bytes).unwrap().contains("contract"), "an item-queue ledger carries no marker at all");
}

#[test]
fn one_requirement_takes_several_small_experiments_and_a_success_consumes_nothing() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1", "R2"]);
    assert!(p.open_child_cycle_linked(CycleKind::Experiment, Some(&link_to(1, "0", "R1"))).is_err(), "no execution before approval");
    approve_close(&mut p, d);
    let revisions = |p: &Project| p.approved_plan().unwrap().proposals.len();

    // Nothing is picked for the Agent: an Experiment without a link is refused, with the link
    // body of this very Project in the refusal.
    let refused = p.open_child_cycle(CycleKind::Experiment).unwrap_err().to_string();
    assert!(is_link_refusal(&refused) && refused.contains("requirement_id: R1 | R2"), "{refused}");
    assert!(p.approved_plan().unwrap().executed_cycles.is_empty());

    let first = p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap().to_ref();
    assert_eq!(p.current_plan_item().unwrap().id, "R1");
    assert_eq!(p.current_plan_selection().unwrap().readiness, "필요한 입력이 준비돼 있다");
    assert!(p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R2"))).is_err(), "one open Cycle at a time");
    close_experiment(&mut p, "success");

    // The success of one small experiment does not complete R1, close the Chain or need a new
    // approval. R1 is simply linked once; R2 has no experiment yet.
    let chain = p.current_chain().unwrap();
    let state = p.approved_plan().unwrap();
    assert!(!chain.is_closed() && !chain.awaiting_report());
    assert_eq!(state.linked("R1").map(|e| e.cycle_ref.as_str()).collect::<Vec<_>>(), vec![first.to_string().as_str()]);
    assert_eq!(state.pending(chain).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["R2"]);
    assert_eq!(state.active(chain).unwrap().draft.items.len(), 2, "both requirements stay in the approved revision");

    // A second, different small experiment on the same requirement — in list order or not.
    let second = p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap().to_ref();
    assert_ne!(first, second);
    close_experiment(&mut p, "success");
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R2"))).unwrap();
    assert_eq!(revisions(&p), 1, "splitting and ordering experiments never re-approves the plan");
    assert_eq!(p.approved_plan().unwrap().linked("R1").count(), 2);

    let (loaded, _) = roundtrip(&p, "explore-two-defines");
    assert_eq!(loaded.approved_plan(), p.approved_plan());
    assert_eq!(crate::context(&loaded), crate::context(&p), "the same position reads the same after a restart");
}

#[test]
fn a_link_to_an_unapproved_requirement_or_a_superseded_approval_is_refused() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1"]);
    approve_close(&mut p, d);
    let first = p.approved_plan().unwrap().active(p.current_chain().unwrap()).unwrap().clone();
    let refused = |p: &mut Project, link: ExperimentLink| {
        let before = p.approved_plan().unwrap().clone();
        let why = p.open_child_cycle_linked(CycleKind::Experiment, Some(&link)).unwrap_err().to_string();
        assert_eq!(p.approved_plan().unwrap(), &before, "a refused link records nothing");
        assert!(is_link_refusal(&why), "{why}");
        why
    };
    let why = refused(&mut p, link_to(first.revision, &first.digest, "R9"));
    assert!(why.contains("R9") && why.contains("R1"), "{why}");
    refused(&mut p, link_to(first.revision, &"0".repeat(64), "R1"));
    let mut blank = link_to(first.revision, &first.digest, "R1");
    blank.selection.uncertainty = "  ".into();
    refused(&mut p, blank);

    // A later Interview changes the requirements themselves: that is a new approval, and the
    // earlier revision no longer opens anything.
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap();
    close_experiment(&mut p, "success");
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let d = synthesis(&mut p, &["R1", "R2"]);
    approve_close(&mut p, d);
    let why = refused(&mut p, link_to(first.revision, &first.digest, "R1"));
    assert!(why.contains('2'), "the refusal names the executable revision: {why}");
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R2"))).unwrap();
    assert_eq!(p.approved_plan().unwrap().executed_cycles.last().unwrap().revision, 2);
    roundtrip(&p, "explore-stale");
}

#[test]
fn a_failure_goes_through_the_recorded_revisit_and_then_a_different_small_experiment() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1"]);
    approve_close(&mut p, d);
    let interview = p.current_cycle().id().to_ref();
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap();
    let failed = close_experiment(&mut p, "failure");
    let world = p.world_snapshot();

    // No child under the failure and no shortcut past the revisit, link or not. This domain
    // fixture changed no file at all: the graph still records the way back.
    assert!(p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).is_err());
    assert!(p.open_branch_cycle(CycleKind::Experiment, Some(&link(&p, "R1"))).is_err(), "nothing was revisited yet");
    assert!(p.cycles().can_revisit());
    let moved = p.revisit_cycle().unwrap();
    assert_eq!((moved.from, moved.target), (failed, interview));
    assert_eq!(moved.target_world, world, "this failure left the world as it was, and the revisit is recorded anyway");

    // The branch needs a link like any Experiment, and may serve the same requirement with a
    // different question. The queue's same-item retry rule does not apply to this contract.
    let why = p.open_branch_cycle(CycleKind::Experiment, None).unwrap_err();
    assert!(matches!(&why, crate::OpenAfterRevisitError::Plan(reason) if is_link_refusal(reason)), "{why}");
    assert!(p.cycles().pending_revisit().is_some(), "a refused branch keeps the pending revisit");
    let next = p.open_branch_cycle(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap();
    let born = p.cycles().node(next).unwrap();
    assert_eq!(born.parent().map(|id| id.to_ref()), Some(interview));
    assert_eq!(born.revisit_from().map(|id| id.to_ref()), Some(failed));
    assert_eq!(born.entry_snapshot(), p.cycles().node(interview.into()).unwrap().exit_snapshot().unwrap());

    // The failure, its Report and its link stay; the requirement is linked twice and still open.
    let state = p.approved_plan().unwrap();
    assert_eq!(state.linked("R1").count(), 2);
    let kept = p.cycles().node(failed.into()).unwrap();
    assert_eq!(kept.report().unwrap().get("verdict"), Some("failure"));
    assert!(state.pending(p.current_chain().unwrap()).is_empty() && !p.current_chain().unwrap().is_closed());
    let (loaded, _) = roundtrip(&p, "explore-failure-branch");
    assert_eq!(loaded.cycles().node(next).unwrap().revisit_from().map(|id| id.to_ref()), Some(failed));
}

#[test]
fn the_item_queue_keeps_its_own_rules_in_this_build_and_takes_no_link() {
    let mut p = item_queue_project();
    let d = synthesis(&mut p, &["A", "B"]);
    approve_close(&mut p, d);
    let state = p.approved_plan().unwrap();
    let active = state.active(p.current_chain().unwrap()).unwrap();
    let stray = link_to(active.revision, &active.digest, "B");
    assert!(p.open_child_cycle_linked(CycleKind::Experiment, Some(&stray)).is_err(), "the queue is not steered by a link");
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    assert_eq!(p.current_plan_item().unwrap().id, "A", "the first pending item is still bound automatically");
    assert_eq!(p.current_plan_selection(), None);
    close_experiment(&mut p, "failure");
    assert_eq!(p.approved_plan().unwrap().pending(p.current_chain().unwrap()).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["A", "B"], "a failed item returns to the queue");
    p.revisit_cycle().unwrap();
    p.open_branch_cycle(CycleKind::Experiment, None).unwrap();
    assert_eq!(p.current_plan_item().unwrap().id, "A", "the retry re-binds the same item from its revisit lineage");
    let (_, bytes) = roundtrip(&p, "explore-queue-unchanged");
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("contract") && !text.contains("selection"), "{text}");
}

#[test]
fn a_closed_item_queue_chain_is_followed_by_an_exploration_chain_without_rewriting_it() {
    let mut p = item_queue_project();
    let d = synthesis(&mut p, &["A"]);
    approve_close(&mut p, d);
    p.open_child_cycle(CycleKind::Experiment).unwrap();
    close_experiment(&mut p, "success");
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let mut closing = synthesis(&mut p, &["A"]);
    closing.synthesis.insert("chain_verdict".into(), "success".into());
    let proposal = p.propose_plan(closing).unwrap();
    let approved = p.decide_plan(proposal.revision, &proposal.digest, Decision::Approve).unwrap();
    p.close_action_step(approved.report().unwrap()).unwrap();
    let outcome = h::opened(&mut p, NodeKind::Outcome);
    p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Interview, NodeKind::Outcome).with("synthesis_ref", &approved.synthesis_ref)).unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", h::step_id(&p, outcome)).with("next_direction.action", "close_chain")).unwrap();
    p.close_chain(Report::new().with("verdict", "success").with("closing_synthesis_ref", &approved.synthesis_ref).with("handoff_summary", "A 완료")).unwrap();
    let old_ledger = p.chains()[0].approved_plan.clone().unwrap();
    let old_digest = old_ledger.proposals[0].digest.clone();

    p.open_child_chain().unwrap();
    assert!(p.exploration_contract(), "the Chain this build opens is an exploration Chain");
    assert_eq!(p.chains()[0].approved_plan.as_ref(), Some(&old_ledger), "the closed Chain keeps its ledger and its meaning");
    assert!(!p.chains()[0].approved_plan.as_ref().unwrap().exploration());

    // The new Chain agrees on its own requirements. The old Chain's approval is evidence to
    // read, not permission to execute here.
    let d = synthesis(&mut p, &["N1"]);
    approve_close(&mut p, d);
    let why = p.open_child_cycle_linked(CycleKind::Experiment, Some(&link_to(1, &old_digest, "A"))).unwrap_err().to_string();
    assert!(is_link_refusal(&why) && why.contains("chain:C1"), "{why}");
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "N1"))).unwrap();
    let (loaded, bytes) = roundtrip(&p, "explore-after-queue");
    assert_eq!(loaded.chains()[0].approved_plan.as_ref(), Some(&old_ledger));
    assert!(loaded.exploration_contract());
    assert_eq!(String::from_utf8(bytes).unwrap().matches("contract: exploration").count(), 1, "only the new Chain is marked");

    // Stored the other way round — an item-queue Chain after an exploration Chain — is refused.
    let reversed = load_edited(&p, "explore-regress", |file| {
        let nodes = file["chains"]["nodes"].as_sequence_mut().unwrap();
        let marker = nodes[1]["approved_plan"].as_mapping_mut().unwrap().remove("contract").unwrap();
        nodes[0]["approved_plan"].as_mapping_mut().unwrap().insert("contract".into(), marker);
    });
    assert!(reversed.is_err(), "a record cannot move a Chain between contracts");
}

#[test]
fn a_record_that_drops_or_forges_part_of_a_link_is_refused_by_the_loader() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1"]);
    approve_close(&mut p, d);
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap();
    for (label, edit) in [
        ("unapproved requirement", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"]["executed_cycles"][0]["item_id"] = "R9".into();
        }) as Box<dyn FnOnce(&mut serde_norway::Value)>),
        ("missing selection", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"]["executed_cycles"][0].as_mapping_mut().unwrap().remove("selection");
        })),
        ("blank selection", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"]["executed_cycles"][0]["selection"]["relevance"] = "".into();
        })),
        ("marker stripped so that queue rules would apply", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"].as_mapping_mut().unwrap().remove("contract");
        })),
        ("unknown contract value", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"]["contract"] = "item_queue".into();
        })),
        ("unknown ledger field", Box::new(|file: &mut serde_norway::Value| {
            file["chains"]["nodes"][0]["approved_plan"].as_mapping_mut().unwrap().insert("future".into(), "x".into());
        })),
    ] {
        assert!(load_edited(&p, "explore-forged", edit).is_err(), "{label} must not load");
    }
}

#[test]
fn a_revisit_target_in_another_chain_is_refused_by_name_and_changes_nothing() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1"]);
    approve_close(&mut p, d);
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap();
    close_experiment(&mut p, "success");
    let old_experiment = p.current_cycle().id().to_ref();
    p.open_child_cycle(CycleKind::Interview).unwrap();
    let mut closing = synthesis(&mut p, &["R1"]);
    closing.synthesis.insert("chain_verdict".into(), "failure".into());
    let proposal = p.propose_plan(closing).unwrap();
    let approved = p.decide_plan(proposal.revision, &proposal.digest, Decision::Approve).unwrap();
    p.close_action_step(approved.report().unwrap()).unwrap();
    let outcome = h::opened(&mut p, NodeKind::Outcome);
    p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Interview, NodeKind::Outcome).with("synthesis_ref", &approved.synthesis_ref)).unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", h::step_id(&p, outcome)).with("next_direction.action", "close_chain")).unwrap();
    p.close_chain(Report::new().with("verdict", "failure").with("closing_synthesis_ref", &approved.synthesis_ref).with("handoff_summary", "R1 미해결")
        .with("analysis", "R1 미해결").with("reflection", "없음").with("next_direction.action", "open_chain").with("next_direction.reason", "새 주제")).unwrap();
    p.open_child_chain().unwrap();
    let d = synthesis(&mut p, &["N1"]);
    approve_close(&mut p, d);
    p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "N1"))).unwrap();
    let outcome = h::walk_to_the_exit(&mut p, "failure");
    let before = serde_json::to_vec(p.approved_plan().unwrap()).unwrap();
    let report = h::cycle_report(p.current_cycle(), "failure", outcome).with("next_direction.target_cycle_ref", old_experiment.to_string());
    let why = p.close_cycle(report).unwrap_err().to_string();
    assert!(why.contains("chain:C1") && why.contains(&old_experiment.to_string()), "{why}");
    assert!(!p.current_cycle().is_closed(), "the refused close left the Cycle open");
    assert_eq!(serde_json::to_vec(p.approved_plan().unwrap()).unwrap(), before);
    // A closed ancestor of the current Chain is still a valid target.
    let report = h::cycle_report(p.current_cycle(), "failure", outcome);
    p.close_cycle(report).unwrap();
}

#[test]
fn the_plan_view_and_context_say_which_requirement_and_never_call_it_done() {
    let mut p = exploration_project();
    let d = synthesis(&mut p, &["R1", "R2"]);
    approve_close(&mut p, d);
    let line = |p: &Project| crate::context::next_call(p);
    assert!(line(&p).contains("requirement_id: R1 | R2") && !line(&p).contains("자동으로 묶인다."), "{}", line(&p));
    let first = p.open_child_cycle_linked(CycleKind::Experiment, Some(&link(&p, "R1"))).unwrap().to_ref();
    let context = crate::context(&p);
    for needle in ["실행 계약: 탐색", "승인 요구 R1에 기여", "관련성:", "준비 상태:", "줄일 불확실성:", "모든 요구는 미해결 상태다", &format!("{first} open")] {
        assert!(context.contains(needle), "{needle}: {context}");
    }
    assert!(!context.contains("미실행 순서"), "the queue's wording stays with the queue: {context}");
    h::opened(&mut p, NodeKind::Define);
    assert!(line(&p).contains("탐색 계약의 Define") && !line(&p).contains("새 설계는 아직 활성화되지 않았다"), "{}", line(&p));
    p.close_action_step(h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Define)).unwrap();
    for kind in [NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] { h::walked(&mut p, kind); }
    let outcome = h::opened(&mut p, NodeKind::Outcome);
    let report = h::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Outcome).with("verdict", "success").with("next_direction.action", "close_cycle");
    p.close_action_step(report).unwrap();
    p.close_cycle(h::cycle_report(p.current_cycle(), "success", h::step_id(&p, outcome))).unwrap();
    let said = line(&p);
    assert!(said.contains("이 성공은 승인 요구의 완료가 아니다") && said.contains("Closing Interview"), "{said}");
    let context = crate::context(&p);
    assert!(context.contains(&format!("{first} success")) && context.contains("연결된 실험: 아직 없음"), "{context}");

    // The item queue keeps every sentence it had.
    let mut q = item_queue_project();
    let d = synthesis(&mut q, &["A"]);
    approve_close(&mut q, d);
    assert!(line(&q).contains("자동으로 묶인다"), "{}", line(&q));
    q.open_child_cycle(CycleKind::Experiment).unwrap();
    h::opened(&mut q, NodeKind::Define);
    assert!(line(&q).contains("새 설계는 아직 활성화되지 않았다") && !line(&q).contains("탐색 계약"), "{}", line(&q));
    assert!(crate::context(&q).contains("미실행 순서") && !crate::context(&q).contains("실행 계약: 탐색"));
}
