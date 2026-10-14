//! Synthetic end-to-end acceptance only; refuses existing destinations.
#[cfg(unix)]
#[path = "../tests/common/mod.rs"]
mod common;
#[cfg(unix)]
fn main() {
    use gil::visual_report::{
        VisualDeclaration, VisualUse, image::VisualAttachment, registry::SourceKind,
    };
    use gil::{CycleKind, NodeKind, ProjectSession};
    use std::{fs, path::PathBuf};
    let root = PathBuf::from(std::env::args().nth(1).expect("new synthetic root"));
    let backup = PathBuf::from(std::env::args().nth(2).expect("new separate backup"));
    assert!(root.is_absolute() && backup.is_absolute());
    fs::create_dir(&root).expect("never overwrite a Project");
    fs::write(
        root.join("notes.txt"),
        "Synthetic migration acceptance only.\n",
    )
    .unwrap();
    let mut session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let mut p = common::bootstrap_from(session.project().clone());
    for kind in [NodeKind::Define, NodeKind::Hypothesis] {
        common::opened(&mut p, kind);
        let report = common::full_report(p.current_cycle().rules(), CycleKind::Experiment, kind);
        let report = if kind == NodeKind::Define {
            report
                .with(
                    "problem",
                    "이관 전 보고서: 기존 기록을 바꾸지 않고 새 시각 자료를 추가한다.",
                )
                .with(
                    "success_condition",
                    "기존 문장과 새 이미지가 재시작 후에도 그대로 조회된다.",
                )
        } else {
            report
                .with(
                    "hypothesis",
                    "이관 전 가설: 저장된 보고서와 이미지 참조는 서버 수명과 독립적이다.",
                )
                .with("rationale", "새 서버에서 bytes와 출처를 다시 대조한다.")
                .with(
                    "guardrail",
                    "실제 사용자 프로젝트와 설치된 플러그인은 변경하지 않는다.",
                )
        };
        p.close_action_step(report).unwrap();
    }
    common::opened(&mut p, NodeKind::Verify);
    *session.project_mut() = p;
    session.commit().unwrap();
    drop(session);
    let original = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let plan = gil::visual_migration::prepare(&root, &backup).unwrap();
    gil::visual_migration::apply(&root, &backup, &plan).unwrap();
    assert_eq!(
        fs::read(backup.join("original").join(gil::STATE_PATH)).unwrap(),
        original
    );
    let (w, h) = (640usize, 300usize);
    let mut pixels = vec![248; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let bar = (0..4).any(|i| {
                x >= 70 + i * 135
                    && x < 140 + i * 135
                    && y >= 260 - [80, 140, 190, 220][i]
                    && y < 260
            });
            let rgb = if bar { [40, 110, 230] } else { [248, 248, 248] };
            pixels[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&rgb);
        }
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    fs::write(root.join("migration-bars.png"), png).unwrap();
    let mut session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let report = common::full_report(
        session.project().current_cycle().rules(),
        CycleKind::Experiment,
        NodeKind::Verify,
    )
    .with(
        "execution",
        "이관 후 보고서: 파란 막대 네 개를 합성 PNG로 생성하고 첨부했다. 실제 성능 측정이 아니다.",
    )
    .with(
        "result",
        "서버 재시작 전후 이미지 bytes·설명·출처와 기존 보고서를 비교한다. 화면 인수는 별도다.",
    );
    session
        .close_verify_with_images_and_commit(
            report,
            vec![VisualAttachment {
                path: "migration-bars.png".into(),
                source_kind: SourceKind::Generated,
                source_description:
                    "이관 검수용 코드로 생성한 합성 막대그래프 — 실제 측정 자료 아님".into(),
            }],
            |ids| VisualDeclaration::Attached {
                uses: vec![VisualUse {
                    asset: ids[0],
                    caption: "이관 후 첨부: 높이가 증가하는 파란 막대 네 개".into(),
                    alt: "밝은 배경의 파란 막대 네 개".into(),
                    purpose: "재시작 후에도 같은 이미지와 출처가 보존되는지 확인".into(),
                }],
            },
        )
        .unwrap();
    drop(session);
    let read = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    read.verify_snapshot_objects().unwrap();
    assert_eq!(
        fs::read(backup.join("original").join(gil::STATE_PATH)).unwrap(),
        original
    );
    println!(
        "{}",
        serde_json::json!({"plan":plan,"old_report":"step:C2/S1","new_image_report":"step:C2/S3","synthetic":true})
    );
}
#[cfg(not(unix))]
fn main() {
    panic!("Unix migration acceptance only");
}
