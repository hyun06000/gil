//! Opt-in synthetic Host acceptance fixture. Refuses an existing destination.
//! Never run against a user Project; not a migration or public authoring command.
#[path = "../tests/common/mod.rs"]
mod common;
use gil::{CycleKind, NodeKind, ProjectSession};
use gil::visual_report::{VisualDeclaration, VisualUse, image::VisualAttachment,
    registry::{ReportOwner, ReportVisuals, SourceKind}};
use std::{fs, path::PathBuf};

fn chart(second: bool) -> Vec<u8> {
    let (w, h) = (640usize, 300usize);
    let mut pixels = vec![248u8; w * h * 3];
    for y in 0..h { for x in 0..w {
        let bar = (0..4).any(|i| x >= 70 + i*135 && x < 140+i*135 &&
            y >= 260 - (if second { [180,120,210,160][i] } else { [80,140,190,220][i] }) && y < 260);
        let rgb = if bar { if second { [25,155,115] } else { [65,100,235] } }
            else if y == 260 && x > 40 && x < 610 { [90,100,110] } else { [248,248,248] };
        pixels[(y*w+x)*3..(y*w+x)*3+3].copy_from_slice(&rgb);
    }}
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
    bytes
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("new absolute fixture directory required"));
    assert!(root.is_absolute(), "absolute path required");
    fs::create_dir(&root).expect("destination must not already exist; nothing overwritten");
    common::sow_world(&root);
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    gil::save(&p.prepare_visual_upgrade().unwrap(), root.join(gil::STATE_PATH)).unwrap();
    let files = ["blue-chart.png", "green-chart.png"];
    for (i, file) in files.iter().enumerate() { fs::write(root.join(file), chart(i == 1)).unwrap(); }
    let mut session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let report = common::full_report(session.project().current_cycle().rules(), CycleKind::Experiment, NodeKind::Verify)
        .with("execution", "G4 합성 차트 두 장을 생성하고 불변 시각 자료로 저장했다. 실제 성능 측정 자료가 아니다.");
    session.close_verify_with_images_and_commit(report, files.iter().map(|path| VisualAttachment {
        path: (*path).into(), source_kind: SourceKind::Generated,
        source_description: "G4 검수용 생성 차트 — 외부 자료·개인 자료 없음".into(),
    }).collect(), |ids| VisualDeclaration::Attached { uses: ids.iter().enumerate().map(|(i,id)| VisualUse {
        asset: *id, caption: format!("G4 합성 차트 {} — 이미지와 채팅 동시 표시 검수", i+1),
        alt: if i == 0 { "높이가 증가하는 파란 막대 네 개" } else { "높이가 서로 다른 초록 막대 네 개" }.into(),
        purpose: "이미지 전환·설명·원래 Verify 출처를 확인한다".into(),
    }).collect() }).unwrap();
    let owner = ReportOwner::Step(gil::StepRef::new(gil::CycleRef::new(2).unwrap(), 3).unwrap());
    let ReportVisuals::Verify(VisualDeclaration::Attached { uses }) = session.project().visual_state().unwrap().registry().report(owner).unwrap() else { panic!() };
    let mut reused = uses[0].clone();
    reused.caption = "Analysis에서 같은 파란 차트 인용 — 원래 출처는 Verify C2/S3".into();
    let p = session.project_mut();
    common::opened(p, NodeKind::Analysis);
    let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, NodeKind::Analysis);
    p.close_step_with_visuals(report, None, ReportVisuals::References(vec![reused])).unwrap();
    session.commit().unwrap();
    drop(session);
    let read = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    println!("{}", serde_json::to_string_pretty(&read.project().report_visuals_v1(owner).unwrap()).unwrap());
}
