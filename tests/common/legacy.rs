//! Synthetic historical records for read-model tests. Never a product migration.
//! Author only a fresh, empty fixture, then load it through the real format gate.
use std::path::{Path, PathBuf};
use gil::Project;

#[derive(Clone, Copy, Debug)]
pub enum Layout { Cycles, Chain }

pub fn assert_unplanned(project: &Project, layout: Layout) {
    assert!(project.approved_plan().is_none(), "historical fixture has no approval ledger");
    assert_eq!(project.current_chain().is_some(), matches!(layout, Layout::Chain));
}

pub fn record(dir: &Path, layout: Layout) -> PathBuf {
    let path = dir.join(gil::STATE_PATH);
    assert!(!path.exists(), "only author a new synthetic record, never rewrite an existing one");
    let fresh = Project::start(super::spec(), super::first_world());
    assert!(fresh.current_cycle().steps().nodes().is_empty());
    gil::save(&fresh, &path).expect("save empty synthetic project");
    let mut file: serde_norway::Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let format = match layout {
        Layout::Cycles => {
            if file.get("chains").is_some() {
                file["cycles"] = file["chains"]["nodes"][0]["cycles"].clone();
                file.as_mapping_mut().unwrap().remove(serde_norway::Value::from("chains"));
            }
            gil::FORMAT
        }
        Layout::Chain => {
            let nodes = file["chains"]["nodes"].as_sequence_mut().expect("Chain fixture requires chain-core");
            assert_eq!(nodes.len(), 1);
            nodes[0].as_mapping_mut().unwrap().remove(serde_norway::Value::from("approved_plan"));
            gil::CHAIN_FORMAT
        }
    };
    file["format"] = format.into();
    std::fs::write(&path, serde_norway::to_string(&file).unwrap()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let loaded = gil::load(super::spec(), &path).expect("historical fixture must load through real storage");
    assert_unplanned(&loaded, layout);
    assert_eq!(std::fs::read(&path).unwrap(), before, "load must not migrate the historical record");
    path
}

pub fn project(label: &str, layout: Layout) -> Project {
    let dir = super::scratch(&format!("historical-{label}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let path = record(&dir, layout);
    let project = gil::load(super::spec(), &path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    project
}
