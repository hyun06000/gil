//! Synthetic package acceptance setup only; never upgrades an existing directory.
#[path = "../tests/common/mod.rs"]
mod common;
use gil::{CycleKind, NodeKind};
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("new absolute directory"));
    assert!(root.is_absolute());
    std::fs::create_dir(&root).expect("new directory only");
    common::sow_world(&root);
    let mut p = common::bootstrap();
    for kind in [NodeKind::Define, NodeKind::Hypothesis] {
        common::opened(&mut p, kind);
        let r = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, kind);
        p.close_action_step(r).unwrap();
    }
    common::opened(&mut p, NodeKind::Verify);
    gil::save(
        &p.prepare_visual_upgrade().unwrap(),
        root.join(gil::STATE_PATH),
    )
    .unwrap();
    let mut png = Vec::new();
    png::Encoder::new(&mut png, 2, 2)
        .write_header()
        .unwrap()
        .write_image_data(&[30, 90, 150, 220])
        .unwrap();
    std::fs::write(root.join("합성.png"), png).unwrap();
    println!("Synthetic format-5 open Verify; generated 2x2 grayscale PNG. Not user evidence.");
}
