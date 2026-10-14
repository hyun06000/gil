//! Independent Agent acceptance setup. Synthetic history, never a user migration.
#[path = "../tests/common/mod.rs"]
mod common;
use gil::{CycleKind, NodeKind};
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("new absolute directory"));
    assert!(root.is_absolute());
    std::fs::create_dir(&root).expect("new directory only; never overwrite");
    let world = common::sow_world(&root);
    let mut p = common::bootstrap_from(gil::Project::start(common::spec(), world));
    for kind in [NodeKind::Define, NodeKind::Hypothesis] {
        common::opened(&mut p, kind);
        let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, kind);
        let report = if kind == NodeKind::Define {
            report.with("problem", "합성 PNG 증거를 Verify 보고서에 저장하고 최종 Cycle 보고서에서 재사용할 수 있는가")
                .with("success_condition", "Agent가 실제 PNG를 만들고 확인한 뒤 저장하고, 같은 asset과 digest를 최종 Cycle에서 원출처 및 별도 설명으로 재사용한다")
        } else {
            report.with("hypothesis", "연결된 작성 도구로 PNG를 확정하면 기존 증거를 복제하지 않고 최종 보고서에서 인용할 수 있다")
                .with("rationale", "합성 실험으로 작성과 재사용 경계를 검수한다; 실제 사용자 화면의 관측이 아니다")
                .with("guardrail", "이 새 합성 폴더만 수정하고 기존 설치본·사용자 기록·인증정보는 건드리지 않는다")
        };
        p.close_action_step(report).unwrap();
    }
    common::opened(&mut p, NodeKind::Verify);
    gil::save(&p.prepare_visual_upgrade().unwrap(), root.join(gil::STATE_PATH)).unwrap();
    println!("Synthetic format-5 open Verify step:C2/S3; no image or visual report created.");
}
