//! Isolated acceptance setup, not a product migration command.
//! The pre-Verify history is synthetic; actual browser work starts after this exits.
#[path = "../tests/common/mod.rs"]
mod common;
use gil::{CycleKind, NodeKind};
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("new absolute directory"));
    assert!(root.is_absolute());
    std::fs::create_dir(&root).expect("must be a new directory; never overwrite");
    common::sow_world(&root);
    let mut p = common::bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis] {
        common::opened(&mut p, kind);
        let r = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, kind);
        let r = if kind == NodeKind::Define {
            r.with("problem", "격리된 곱셈 계산기의 실제 화면을 보고서에 남긴다")
             .with("success_condition", "12 × 7 = 84를 브라우저에서 관측하고 캡처·설명·출처를 노드에 표시한다")
        } else {
            r.with("hypothesis", "실제 캡처를 Verify close에 연결하면 노드에서 같은 증거를 볼 수 있다")
             .with("rationale", "합성 초기 이력 이후 실제 브라우저 작업과 MCP 작성 경로를 잇는다")
             .with("guardrail", "개인 자료·기존 프로젝트·설치본·보안 설정을 변경하지 않는다")
        };
        p.close_action_step(r).unwrap();
    }
    common::opened(&mut p, NodeKind::Verify);
    gil::save(&p.prepare_visual_upgrade().unwrap(), root.join(gil::STATE_PATH)).unwrap();
    println!("Prepared isolated open Verify step:C2/S3; earlier Interview is synthetic, not a user interview.");
}
