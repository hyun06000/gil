//! **MCP stdio 표면의 시험** — 진짜 프로세스를 세워 진짜 frame 을 주고받는다.
//!
//! 여기서 재는 것은 배선이지 domain 이 아니다. GIL 이 무엇을 옳다고 판정하는가는 이미 다른
//! 시험들이 잡았다. 이 파일이 지키는 것은 넷이다.
//!
//! 1. **같은 글자** — 같은 Project 에서 CLI 와 MCP 의 `said` 가 byte 로 같다
//! 2. **정확 경로** — 자식 자리를 받고 부모 Project 로 물러서지 않는다
//! 3. **깨끗한 stdout** — JSON-RPC frame 말고는 한 글자도 없다
//! 4. **명시적 tool 표** — 기존 12개 + 읽기 전용 MCP App 7개, UI 쓰기 문은 없다

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
mod common;

/// State path used by the product-facing MCP start. On Windows the working tree and the private
/// GIL record deliberately live in different roots.
fn product_state_path(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use sha2::{Digest, Sha256};
        use std::os::windows::ffi::OsStrExt;
        let canonical = root.canonicalize().unwrap();
        let mut identity = Vec::new();
        for unit in canonical.as_os_str().encode_wide() {
            identity.extend_from_slice(&unit.to_le_bytes());
        }
        let key = format!("{:x}", Sha256::digest(&identity));
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("GIL/project-state-v1").join(key).join(gil::STATE_PATH)
    }
    #[cfg(not(windows))]
    { root.join(gil::STATE_PATH) }
}

fn load_product(root: &Path) -> gil::Project {
    gil::load(common::spec(), product_state_path(root)).unwrap()
}

fn save_product(root: &Path, project: &gil::Project) {
    gil::save(project, product_state_path(root)).unwrap();
}

fn open_product(root: &Path) -> gil::ProjectSession {
    gil::open_project_root(common::spec(), root).unwrap()
}

fn open_product_read_only(root: &Path) -> gil::ReadOnlySession {
    gil::open_project_root_read_only(common::spec(), root).unwrap()
}

fn product_temp_path(root: &Path) -> PathBuf {
    product_state_path(root).with_file_name("state.yaml.writing")
}

#[cfg(all(target_os="macos",target_arch="aarch64",feature="visual-authoring"))]
#[test]
fn migration_stdio_two_approvals_backup_apply_restart_and_preservation() {
    use serde_json::{Value,json};
    use std::{collections::BTreeMap,fs,os::unix::fs::{MetadataExt,PermissionsExt}};
    fn tree(root:&Path)->BTreeMap<PathBuf,(u32,Vec<u8>)> {
        fn walk(root:&Path,dir:&Path,out:&mut BTreeMap<PathBuf,(u32,Vec<u8>)>) {
            for e in fs::read_dir(dir).unwrap() {
                let p=e.unwrap().path();let m=fs::symlink_metadata(&p).unwrap();
                assert!(!m.file_type().is_symlink());
                out.insert(p.strip_prefix(root).unwrap().into(),(m.mode()&0o7777,if m.is_file(){fs::read(&p).unwrap()}else{Vec::new()}));
                if m.is_dir(){walk(root,&p,out)}
            }
        }
        let mut out=BTreeMap::new();walk(root,root,&mut out);out
    }
    fn logical(bytes:&[u8])->serde_norway::Value {
        let mut v:serde_norway::Value=serde_norway::from_slice(bytes).unwrap();
        for k in ["format","visuals"] {v.as_mapping_mut().unwrap().remove(serde_norway::Value::from(k));}v
    }
    for closed in [false,true] {
        let base=scratch(if closed{"migration-closed"}else{"migration-open"}).canonicalize().unwrap();
        let root=base.join("프로젝트 🧭");fs::create_dir(&root).unwrap();
        common::sow_world(&root);
        let mut s=gil::ProjectSession::start(common::spec(),root.join(gil::STATE_PATH)).unwrap();
        let p=if closed {common::graph_with_one_closed_cycle("success")}else{
            let mut p=common::bootstrap_from(s.project().clone());common::up_to_verify(&mut p);p
        };
        *s.project_mut()=p;s.commit().unwrap();drop(s);
        fs::write(root.join("미확정 변경.txt"),"공백과 한글\n다음 줄\n").unwrap();
        fs::write(root.join(".gil/unreferenced"),b"retain even unreferenced").unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/local-note"),b"git is not skipped").unwrap();
        fs::write(root.join("run.sh"),b"synthetic executable").unwrap();
        fs::set_permissions(root.join("run.sh"),fs::Permissions::from_mode(0o755)).unwrap();
        let before=tree(&root);let backup=base.join("private-backup");
        let mut mcp=Client::serve_with(&base.join("settings"));mcp.initialize();
        let check=monitor_call(&mut mcp,"gil_migration_check",json!({"project_root":root}));
        assert_eq!(check["structuredContent"]["storage_format"],4,"{check}");
        assert_eq!(check["structuredContent"]["cycle_open"],!closed);
        assert_eq!(tree(&root),before);
        let denied=json!({"project_root":root,"backup_root":backup,"backup_approved":false});
        assert_eq!(monitor_call(&mut mcp,"gil_migration_prepare",denied)["structuredContent"]["code"],"approval_required");
        assert!(!backup.exists());assert_eq!(tree(&root),before);
        let missing=mcp.request("tools/call",json!({"name":"gil_migration_prepare","arguments":{"project_root":root,"backup_root":backup}}));
        assert!(missing["error"].is_object());assert!(!backup.exists());
        let prepared=monitor_call(&mut mcp,"gil_migration_prepare",json!({"project_root":root,"backup_root":backup,"backup_approved":true}));
        assert_ne!(prepared["isError"],true,"{prepared}");
        let plan=prepared["structuredContent"]["plan_sha256"].clone();assert!(plan.is_string());
        assert_eq!(tree(&root),before);assert_eq!(tree(&backup.join("restored")),before);
        let original=tree(&backup.join("original"));
        for (rel,(_,bytes)) in &before {assert_eq!(&original[rel].1,bytes);}
        let mut apply=json!({"project_root":root,"backup_root":backup,"plan_sha256":plan,"apply_approved":false});
        assert_eq!(monitor_call(&mut mcp,"gil_migration_apply",apply.clone())["structuredContent"]["code"],"approval_required");
        assert_eq!(tree(&root),before);
        apply["apply_approved"]=json!(true);
        let mut bad=apply.clone();bad["plan_sha256"]=json!("0".repeat(64));
        assert_eq!(monitor_call(&mut mcp,"gil_migration_apply",bad)["isError"],true);
        // Another valid project cannot borrow this approval/plan.
        let other=base.join("other");fs::create_dir(&other).unwrap();started(&other);
        let other_before=tree(&other);let mut wrong=apply.clone();wrong["project_root"]=json!(other);
        assert_eq!(monitor_call(&mut mcp,"gil_migration_apply",wrong)["isError"],true);assert_eq!(tree(&other),other_before);
        // Drift invalidates the old preparation, not a reason to erase new files.
        fs::write(root.join("new-evidence"),b"new work").unwrap();
        assert_eq!(monitor_call(&mut mcp,"gil_migration_apply",apply.clone())["isError"],true);
        assert_eq!(fs::read(root.join("new-evidence")).unwrap(),b"new work");
        let backup2=base.join("fresh-private-backup");
        let prepared=monitor_call(&mut mcp,"gil_migration_prepare",json!({"project_root":root,"backup_root":backup2,"backup_approved":true}));
        assert_ne!(prepared["isError"],true,"{prepared}");
        // Simulated fresh human consent after inspecting the new result.
        apply["backup_root"]=json!(backup2);apply["plan_sha256"]=prepared["structuredContent"]["plan_sha256"].clone();
        let current=tree(&root);
        let got=monitor_call(&mut mcp,"gil_migration_apply",apply.clone());
        assert_eq!(got["structuredContent"]["preserved"],true,"{got}");
        let after=tree(&root);
        assert_eq!(logical(&current[Path::new(gil::STATE_PATH)].1),logical(&after[Path::new(gil::STATE_PATH)].1));
        for (rel,value) in &current {if rel!=Path::new(gil::STATE_PATH){assert_eq!(&after[rel],value);}}
        assert_eq!(tree(&backup.join("original")),original);
        assert_eq!(tree(&backup2.join("restored")),current);
        // Simulated lost apply receipt: fresh server only reads the durable result.
        mcp.eof();let mut mcp=Client::serve_with(&base.join("restart-settings"));mcp.initialize();
        let mut status=apply.clone();status.as_object_mut().unwrap().remove("apply_approved");
        let got=monitor_call(&mut mcp,"gil_migration_status",status);
        assert_eq!(got["structuredContent"]["state"],"applied_state_present");
        assert_eq!(got["structuredContent"]["preserved"],true);
        assert_eq!(tree(&root),after);
        assert_eq!(monitor_call(&mut mcp,"gil_migration_apply",apply)["isError"],true);
        assert_eq!(tree(&root),after);mcp.eof();
        let _:Value=check;
        // Retain synthetic originals and backups as evidence; no destructive cleanup.
    }
}

/// A format-5 Verify without images must close with the typed shape carrying a `not_needed` (or
/// `missing`) declaration; the ordinary text close of that Verify is refused and converts nothing.
#[cfg(all(any(unix, windows), feature = "visual-authoring"))]
#[test]
fn format5_verify_without_images_uses_typed_close_with_a_declaration() {
    use serde_json::json;
    use gil::{CycleKind, NodeKind};
    let root=scratch("verify-no-images");
    let mut mcp=Client::serve_with(&scratch("verify-no-images-settings")); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp,"gil_start",json!({"project_root":root}))["structuredContent"]["storage_format"],5);
    let mut p=common::bootstrap_from(load_product(&root));
    common::up_to_verify(&mut p);
    save_product(&root, &p);
    let r=common::full_report(&common::spec(),CycleKind::Experiment,NodeKind::Verify);
    let report: String=r.field_names().map(|k| format!("{k}: {}\n",r.get(k).unwrap())).collect();
    let before=std::fs::read(product_state_path(&root)).unwrap();
    // The ordinary text close is the wrong shape for a format-5 Verify: refused, nothing written.
    let text=monitor_call(&mut mcp,"gil_close",json!({"project_root":root,"report":report}));
    assert_eq!(text["isError"],true,"{text}");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
    // Half-typed shape is a schema rejection that names the missing field.
    let half=mcp.request("tools/call",json!({"name":"gil_close","arguments":{"project_root":root,"report":report,
        "expected_owner":{"kind":"step","ref":"step:C2/S3"}}}));
    assert_eq!(half["error"]["code"],-32602);
    assert!(half["error"]["message"].as_str().unwrap().contains("not_needed/missing"),"{half}");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
    // A `missing` declaration without follow_up is refused by the schema too.
    let bad=mcp.request("tools/call",json!({"name":"gil_close","arguments":{"project_root":root,"report":report,
        "expected_owner":{"kind":"step","ref":"step:C2/S3"},
        "visuals":{"attachments":[],"uses":[],"declaration":{"state":"missing","reason":"no renderer"}}}}));
    assert_eq!(bad["error"]["code"],-32602);
    // The correct shape for an image-less Verify: typed close, empty arrays, not_needed with a reason.
    let closed=monitor_call(&mut mcp,"gil_close",json!({"project_root":root,"report":report,
        "expected_owner":{"kind":"step","ref":"step:C2/S3"},
        "visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"numeric assertions only"}}}));
    assert_eq!(closed["structuredContent"]["ok"],true,"{closed}");
    assert_eq!(closed["structuredContent"]["confirmed"],true);
    assert_eq!(closed["structuredContent"]["visuals"]["state"],"not_needed");
    assert_eq!(closed["structuredContent"]["visuals"]["uses"].as_array().unwrap().len(),0);
    let read=monitor_call(&mut mcp,"gil_report_visuals",json!({"project_root":root,"owner":{"kind":"step","ref":"step:C2/S3"}}));
    assert_eq!(read["structuredContent"]["confirmed"],true);
    assert_eq!(read["structuredContent"]["visuals"]["state"],"not_needed");
    mcp.eof();
}

#[cfg(all(any(unix, windows), feature = "visual-authoring"))]
#[test]
fn authoring_stdio_close_reuse_confirmation_and_rejections() {
    use serde_json::{Value, json};
    use gil::{CycleKind, NodeKind};
    fn report(kind: NodeKind) -> String {
        let r=common::full_report(&common::spec(),CycleKind::Experiment,kind);
        r.field_names().map(|k| format!("{k}: {}\n",r.get(k).unwrap())).collect()
    }
    let root=scratch("authoring-new-wire");
    let mut mcp=Client::serve_with(&scratch("authoring-settings")); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp,"gil_start",json!({"project_root":root}))["structuredContent"]["storage_format"],5);
    let mut p=common::bootstrap_from(load_product(&root));
    common::up_to_verify(&mut p);
    save_product(&root, &p);
    let mut png=Vec::new();
    png::Encoder::new(&mut png,1,1).write_header().unwrap().write_image_data(&[91]).unwrap();
    std::fs::write(root.join("합성.png"),&png).unwrap();
    let owner=json!({"kind":"step","ref":"step:C2/S3"});
    let read_args=json!({"project_root":root,"owner":owner});
    let initial=monitor_call(&mut mcp,"gil_report_visuals",read_args.clone());
    assert_eq!(initial["structuredContent"]["confirmed"],false);
    let args=json!({"project_root":root,"report":report(NodeKind::Verify),"expected_owner":owner,
        "visuals":{"attachments":[{"key":"screen","path":"합성.png","source_kind":"generated","source_description":"synthetic only"}],
        "uses":[{"source":{"kind":"new","key":"screen"},"caption":"<script>not executable</script>","alt":"gray pixel","purpose":"wire check"}],"declaration":{"state":"attached"}}});
    let before=std::fs::read(product_state_path(&root)).unwrap();
    for (path,value) in [
        ("/expected_owner/ref",json!("step:C2/S2")),
        ("/visuals/attachments/0/path",json!("../outside.png")),
        ("/report",json!("bad: report")),
    ] {
        let mut bad=args.clone(); *bad.pointer_mut(path).unwrap()=value;
        let got=monitor_call(&mut mcp,"gil_close",bad);
        assert_eq!(got["isError"],true,"{got}");
        assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
    }
    let mut malformed=args.clone(); malformed["visuals"]["surprise"]=json!(true);
    assert!(mcp.request("tools/call",json!({"name":"gil_close","arguments":malformed}))["error"].is_object());
    let mut orphan=args.clone(); orphan["visuals"]["uses"]=json!([]);
    assert!(mcp.request("tools/call",json!({"name":"gil_close","arguments":orphan}))["error"].is_object());
    // The VM failure: expected_owner without visuals (and the mirror case) must be a -32602 that names
    // the missing field, echoes no input, and closes nothing.
    for drop in ["visuals","expected_owner"] {
        let mut half=args.clone(); half.as_object_mut().unwrap().remove(drop);
        let err=mcp.request("tools/call",json!({"name":"gil_close","arguments":half}));
        assert_eq!(err["error"]["code"],-32602,"{err}");
        let message=err["error"]["message"].as_str().unwrap();
        assert!(message.contains(&format!("without '{drop}'")),"{message}");
        assert!(message.contains("Nothing was closed") && !message.contains(root.to_str().unwrap()),"{message}");
        assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
    }
    std::fs::write(root.join("broken.png"),b"not png").unwrap();
    let mut broken=args.clone(); broken["visuals"]["attachments"][0]["path"]=json!("broken.png");
    assert_eq!(monitor_call(&mut mcp,"gil_close",broken)["isError"],true);
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
    std::fs::remove_file(root.join("broken.png")).unwrap();
    let receipt=monitor_call(&mut mcp,"gil_close",args.clone());
    assert_ne!(receipt["isError"],true,"{receipt}");
    let committed=receipt["structuredContent"].clone();
    assert_eq!(committed["confirmed"],true);
    assert_eq!(committed["display_confirmed"],false);
    assert_eq!(committed["visuals"]["state"],"attached");
    let after=std::fs::read(product_state_path(&root)).unwrap();
    assert_eq!(monitor_call(&mut mcp,"gil_close",args)["structuredContent"]["code"],"owner_mismatch");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),after);
    // Treat the receipt as lost: new process reads the durable result, never repeats close.
    mcp.eof(); let mut mcp=Client::serve_with(&scratch("authoring-restart")); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp,"gil_report_visuals",read_args.clone())["structuredContent"],committed);
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),after);
    let stored=&committed["visuals"]["uses"][0];
    let mut use_args=json!({"project_root":root,"report":report(NodeKind::Analysis),"expected_owner":{"kind":"step","ref":"step:C2/S4"},
        "visuals":{"attachments":[],"uses":[{"source":{"kind":"stored","owner":owner,"asset_id":stored["asset_id"],"sha256":stored["sha256"]},"caption":"Analysis-specific caption","alt":"same pixel","purpose":"reuse"}]}});
    {
        let mut s=open_product(&root);
        s.open_action_step(NodeKind::Analysis,common::plan(NodeKind::Analysis)).unwrap(); s.commit().unwrap();
    }
    let open=std::fs::read(product_state_path(&root)).unwrap();
    let mut wrong=use_args.clone(); wrong["visuals"]["uses"][0]["source"]["sha256"]=json!("0".repeat(64));
    assert_eq!(monitor_call(&mut mcp,"gil_close",wrong)["structuredContent"]["code"],"visual_unavailable");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),open);
    // Non-Verify still obeys the world dirty gate.
    std::fs::write(root.join("dirty.txt"),b"dirty").unwrap();
    assert_eq!(monitor_call(&mut mcp,"gil_close",use_args.clone())["isError"],true);
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),open);
    std::fs::remove_file(root.join("dirty.txt")).unwrap();
    std::fs::create_dir(product_temp_path(&root)).unwrap();
    assert_eq!(monitor_call(&mut mcp,"gil_close",use_args.clone())["isError"],true);
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),open);
    let confirmation=monitor_call(&mut mcp,"gil_report_visuals",json!({"project_root":root,"owner":use_args["expected_owner"]}));
    assert_eq!(confirmation["structuredContent"]["confirmed"],false);
    std::fs::remove_dir(product_temp_path(&root)).unwrap();
    let reused=monitor_call(&mut mcp,"gil_close",use_args.clone());
    assert_ne!(reused["isError"],true,"{reused}");
    assert_eq!(reused["structuredContent"]["visuals"]["uses"][0]["caption"],"Analysis-specific caption");
    assert_eq!(reused["structuredContent"]["visuals"]["uses"][0]["origin_step_ref"],"step:C2/S3");
    // Reuse through Outcome, Cycle close, then the next Cycle's Define via the same wire.
    let outcome_ref;
    {
        let mut s=open_product(&root);
        outcome_ref=s.open_action_step(NodeKind::Outcome,common::plan(NodeKind::Outcome)).unwrap().step; s.commit().unwrap();
    }
    let outcome=common::full_report(&common::spec(),CycleKind::Experiment,NodeKind::Outcome).with("verdict","success");
    use_args["report"]=json!(outcome.field_names().map(|k|format!("{k}: {}\n",outcome.get(k).unwrap())).collect::<String>());
    use_args["expected_owner"]=json!({"kind":"step","ref":outcome_ref.to_string()});
    assert_ne!(monitor_call(&mut mcp,"gil_close",use_args.clone())["isError"],true);
    let cycle_body={
        let s=open_product_read_only(&root);
        let r=common::cycle_report(s.project().current_cycle(),"success",common::step_id(s.project(),outcome_ref));
        r.field_names().map(|k|format!("{k}: {}\n",r.get(k).unwrap())).collect::<String>()
    };
    use_args["report"]=json!(cycle_body);
    use_args["expected_owner"]=json!({"kind":"cycle","ref":"cycle:C2"});
    let cycle=monitor_call(&mut mcp,"gil_close",use_args.clone());
    assert_ne!(cycle["isError"],true,"{cycle}");
    assert_eq!(cycle["structuredContent"]["visuals"]["uses"][0]["sha256"],stored["sha256"]);
    {
        let mut s=open_product(&root);
        s.open_child_cycle(CycleKind::Experiment).unwrap();
        s.open_action_step(NodeKind::Define,common::plan(NodeKind::Define)).unwrap(); s.commit().unwrap();
    }
    use_args["expected_owner"]=json!({"kind":"step","ref":"step:C3/S1"});
    use_args["report"]=json!(report(NodeKind::Define));
    let next=monitor_call(&mut mcp,"gil_close",use_args.clone());
    assert_ne!(next["isError"],true,"{next}");
    assert_eq!(next["structuredContent"]["visuals"]["uses"][0]["origin_step_ref"],"step:C2/S3");
    // Source deletion cannot change the persisted report image (read-only operation).
    std::fs::remove_file(root.join("합성.png")).unwrap();
    let prepared=monitor_call(&mut mcp,"gil_monitor_prepare",json!({"project_root":root}));
    let bytes=monitor_call(&mut mcp,"gil_monitor_report_image",json!({"scope_id":prepared["structuredContent"]["scope_id"],"owner":owner,"asset_id":stored["asset_id"],"offset":0}));
    assert_eq!(bytes["structuredContent"]["sha256"],stored["sha256"]);
    let legacy=common::scratch("authoring-legacy"); let mut p=common::bootstrap(); common::up_to_verify(&mut p);
    gil::save(&p,legacy.join(gil::STATE_PATH)).unwrap();
    let legacy_before=std::fs::read(legacy.join(gil::STATE_PATH)).unwrap();
    use_args["project_root"]=json!(legacy);
    assert_eq!(monitor_call(&mut mcp,"gil_close",use_args)["structuredContent"]["code"],"visual_upgrade_required");
    assert_eq!(std::fs::read(legacy.join(gil::STATE_PATH)).unwrap(),legacy_before);
    let _: Value=monitor_call(&mut mcp,"gil_report_visuals",read_args);
    mcp.eof();
}

#[cfg(all(any(unix, windows), feature = "visual-authoring"))]
#[test]
fn authoring_declarations_and_failed_commit_keep_reports_open() {
    use serde_json::json;
    let mut mcp=Client::serve_with(&scratch("authoring-declarations")); mcp.initialize();
    for state in ["not_needed","missing","attached"] {
        let root=scratch(&format!("authoring-new-{state}"));
        let started=monitor_call(&mut mcp,"gil_start",json!({"project_root":root}));
        assert_eq!(started["structuredContent"]["storage_format"],5,"{started}");
        let p=load_product(&root);
        let mut p=common::bootstrap_from(p); common::up_to_verify(&mut p);
        save_product(&root, &p);
        let mut png=Vec::new(); png::Encoder::new(&mut png,1,1).write_header().unwrap().write_image_data(&[2]).unwrap();
        std::fs::write(root.join("a.png"),png).unwrap();
        let report=common::full_report(&common::spec(),gil::CycleKind::Experiment,gil::NodeKind::Verify);
        let body=report.field_names().map(|k|format!("{k}: {}\n",report.get(k).unwrap())).collect::<String>();
        let owner=json!({"kind":"step","ref":"step:C2/S3"});
        let mut args=json!({"project_root":root,"report":body,"expected_owner":owner,
            "visuals":{"attachments":[],"uses":[],"declaration":{"state":state}}});
        if state=="not_needed" { args["visuals"]["declaration"]["reason"]=json!("numeric assertion only"); }
        else {
            args["visuals"]["attachments"]=json!([{"key":"a","path":"a.png","source_kind":"generated","source_description":"synthetic"}]);
            args["visuals"]["uses"]=json!([{"source":{"kind":"new","key":"a"},"caption":"one","alt":"pixel","purpose":"test"}]);
            if state=="missing" {
                args["visuals"]["declaration"]["reason"]=json!("second capture unavailable");
                args["visuals"]["declaration"]["follow_up"]=json!("next Cycle after normal close");
            }
        }
        let before=std::fs::read(product_state_path(&root)).unwrap();
        // Default text path also cannot bypass the mandatory format-5 declaration.
        assert_eq!(monitor_call(&mut mcp,"gil_close",json!({"project_root":root,"report":body}))["isError"],true);
        assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
        std::fs::create_dir(product_temp_path(&root)).unwrap();
        assert_eq!(monitor_call(&mut mcp,"gil_close",args.clone())["isError"],true);
        assert_eq!(std::fs::read(product_state_path(&root)).unwrap(),before);
        let read=monitor_call(&mut mcp,"gil_report_visuals",json!({"project_root":root,"owner":owner}));
        assert_eq!(read["structuredContent"]["confirmed"],false,"{read}");
        std::fs::remove_dir(product_temp_path(&root)).unwrap();
        let result=monitor_call(&mut mcp,"gil_close",args);
        assert_eq!(result["structuredContent"]["confirmed"],true,"{result}");
        assert_eq!(result["structuredContent"]["visuals"]["state"],state);
        assert_eq!(result["structuredContent"]["visuals"]["uses"].as_array().unwrap().len(),if state=="not_needed" {0} else {1});
    }
    mcp.eof();
}

#[cfg(all(any(unix, windows), feature = "visual-authoring"))]
#[test]
fn authoring_competing_closes_publish_only_one_report() {
    use serde_json::json;
    let root=common::scratch("authoring-race");
    let mut p=common::bootstrap(); common::up_to_verify(&mut p);
    gil::save(&p.prepare_visual_upgrade().unwrap(),root.join(gil::STATE_PATH)).unwrap();
    common::private_storage(&root);
    let report=common::full_report(&common::spec(),gil::CycleKind::Experiment,gil::NodeKind::Verify);
    let body=report.field_names().map(|k|format!("{k}: {}\n",report.get(k).unwrap())).collect::<String>();
    let args=json!({"project_root":root,"report":body,"expected_owner":{"kind":"step","ref":"step:C2/S3"},
        "visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"synthetic race"}}});
    let barrier=std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles:Vec<_>=(0..2).map(|i| {
        let args=args.clone(); let barrier=barrier.clone();
        std::thread::spawn(move || {
            let mut mcp=Client::serve_with(&scratch(&format!("authoring-race-{i}"))); mcp.initialize();
            barrier.wait();
            let got=monitor_call(&mut mcp,"gil_close",args); mcp.eof(); got
        })
    }).collect();
    let results:Vec<_>=handles.into_iter().map(|h|h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r|r["structuredContent"]["confirmed"]==true).count(),1,"{results:?}");
    assert_eq!(results.iter().filter(|r|r["isError"]==true).count(),1,"{results:?}");
    let session=gil::ProjectSession::open_read_only(common::spec(),root.join(gil::STATE_PATH)).unwrap();
    assert!(session.project().current_cycle().step_now_open().is_none());
}

#[test]
#[cfg(any(unix, windows))]
fn visual_report_tools_read_exact_bytes_and_refuse_unlinked_assets_over_stdio() {
    use gil::visual_report::{VisualDeclaration, VisualUse, image::VisualAttachment, registry::SourceKind};
    use base64::Engine;
    let root = common::scratch("visual-wire");
    let mut p = common::bootstrap();
    common::up_to_verify(&mut p);
    gil::save(&p.prepare_visual_upgrade().unwrap(), root.join(gil::STATE_PATH)).unwrap();
    common::private_storage(&root);
    let mut bytes = Vec::new();
    // Incompressible deterministic pixels force several wire chunks.
    let mut seed = 17u32;
    let pixels: Vec<u8> = (0..1024*1024).map(|_| { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed as u8 }).collect();
    png::Encoder::new(&mut bytes, 1024, 1024).write_header().unwrap().write_image_data(&pixels).unwrap();
    assert!(bytes.len() > 262144);
    std::fs::write(root.join("image.png"), &bytes).unwrap();
    let mut session = gil::ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let report = common::full_report(session.project().current_cycle().rules(), gil::CycleKind::Experiment, gil::NodeKind::Verify);
    session.close_verify_with_images_and_commit(report, vec![VisualAttachment {path:"image.png".into(),source_kind:SourceKind::Generated,source_description:"synthetic".into()}], |ids| VisualDeclaration::Attached { uses:vec![VisualUse {asset:ids[0],caption:"wire test".into(),alt:"noise".into(),purpose:"transport".into()}] }).unwrap();
    drop(session);
    std::fs::remove_file(root.join("image.png")).unwrap();
    let before = std::fs::read(root.join(gil::STATE_PATH)).unwrap();
    let settings = scratch("visual-wire-settings");
    let mut mcp = Client::serve_with(&settings); mcp.initialize();
    let prepared = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({"project_root":root}));
    let scope = prepared["structuredContent"]["scope_id"].clone();
    let owner = serde_json::json!({"kind":"step","ref":"step:C2/S3"});
    let list = monitor_call(&mut mcp,"gil_monitor_report_visuals",serde_json::json!({"scope_id":scope,"owner":owner}));
    assert_eq!(list["structuredContent"]["state"], "attached");
    let asset = list["structuredContent"]["uses"][0]["asset_id"].clone();
    let digest = list["structuredContent"]["uses"][0]["sha256"].clone();
    let args = serde_json::json!({"scope_id":scope,"owner":owner,"asset_id":asset,"offset":0});
    let mut offset = 0;
    let mut received = Vec::new();
    loop {
        let mut args = args.clone(); args["offset"] = serde_json::json!(offset);
        let got = monitor_call(&mut mcp,"gil_monitor_report_image",args);
        assert_ne!(got["isError"], true, "{got}");
        let data = &got["structuredContent"];
        assert_eq!(data["sha256"],digest);
        assert_eq!(data["offset"],offset);
        let chunk = base64::engine::general_purpose::STANDARD.decode(data["data_base64"].as_str().unwrap()).unwrap();
        assert!(chunk.len() <= 262144);
        received.extend(chunk);
        if data["next_offset"].is_null() { break; }
        offset = data["next_offset"].as_u64().unwrap();
    }
    assert_eq!(received, bytes);
    for (key,value) in [("offset",serde_json::json!(1)),("offset",serde_json::json!(-1)),("asset_id",serde_json::json!("01")),("owner",serde_json::json!({"kind":"step","ref":"step:C2/S3","path":"secret"})),("path",serde_json::json!("secret"))] {
        let mut bad=args.clone(); bad[key]=value;
        let reply=mcp.request("tools/call",serde_json::json!({"name":"gil_monitor_report_image","arguments":bad}));
        assert!(reply["error"].is_object(),"{reply}");
    }
    let mut wrong=args.clone(); wrong["owner"]["ref"]=serde_json::json!("step:C2/S1");
    assert_eq!(monitor_call(&mut mcp,"gil_monitor_report_image",wrong)["structuredContent"]["code"],"visual_unavailable");
    let mut unknown=args.clone(); unknown["scope_id"]=serde_json::json!(format!("project:{}","f".repeat(64)));
    assert_eq!(monitor_call(&mut mcp,"gil_monitor_report_image",unknown)["structuredContent"]["code"],"reconnect_required");
    mcp.eof();
    let mut fresh=Client::serve_with(&settings); fresh.initialize();
    let got=monitor_call(&mut fresh,"gil_monitor_report_image",args);
    assert_eq!(got["structuredContent"]["sha256"],digest);
    let hex=digest.as_str().unwrap();
    let blob=root.join(".gil/artifacts/blobs/sha256").join(&hex[..2]).join(&hex[2..]);
    std::fs::write(blob,b"corrupt").unwrap();
    let bad=monitor_call(&mut fresh,"gil_monitor_report_image",serde_json::json!({"scope_id":scope,"owner":owner,"asset_id":asset,"offset":0}));
    assert_eq!(bad["structuredContent"]["code"],"visual_unavailable");
    fresh.eof();
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(),before);
}

fn gil() -> PathBuf {
    // 시험이 부르는 것은 **지금 지은 그 binary** 다.
    let mut at = std::env::current_exe().expect("시험 실행 파일");
    at.pop();
    if at.ends_with("deps") {
        at.pop();
    }
    at.join(format!("gil{}", std::env::consts::EXE_SUFFIX))
}

fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!(
        "gil-mcp-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("자리");
    at
}

/// A half-typed close (expected_owner without visuals) is the exact call that failed in the field.
/// It must be refused with a -32602 that names the missing field, must close nothing, and the agent
/// must be able to recover with read-only status plus one corrected ordinary close.
#[test]
fn half_typed_close_is_rejected_with_the_missing_field_and_recovery_is_an_ordinary_close() {
    use serde_json::json;
    use gil::{CycleKind, NodeKind};
    let at = scratch("half-typed-close");
    started(&at);
    let mut mcp = Client::serve_with(&scratch("half-typed-close-settings"));
    mcp.initialize();
    // The field situation: the first Interview step C1/S1 is open when the half-typed close arrives.
    let opened = monitor_call(&mut mcp, "gil_open", json!({"project_root": at, "kind": "question",
        "contract": "objective: 사용자의 목표를 확인한다\nnext_action: 무엇을 만들고 싶은지 묻는다\ndone_when: 사용자의 원문 응답을 얻는다\n"}));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    let before = std::fs::read(at.join(gil::STATE_PATH)).unwrap();
    let wrong = json!({"project_root": at, "report": "question: 무엇을 만들까\n",
        "expected_owner": {"kind": "step", "ref": "step:C1/S1"}});
    let err = mcp.request("tools/call", json!({"name": "gil_close", "arguments": wrong}));
    assert_eq!(err["error"]["code"], -32602, "{err}");
    let message = err["error"]["message"].as_str().unwrap();
    assert!(message.contains("visuals") && message.contains("expected_owner"), "{message}");
    assert!(message.contains("Nothing was closed"), "{message}");
    assert!(!message.contains(at.to_str().unwrap()), "input values must not be echoed");
    assert_eq!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), before, "a rejected close must not touch the project");
    // Recovery path: read-only status/context show C1/S1 still open, then one corrected ordinary close.
    let status = monitor_call(&mut mcp, "gil_status", json!({"project_root": at}));
    assert_eq!(status["structuredContent"]["ok"], true, "{status}");
    assert!(status["structuredContent"]["said"].as_str().unwrap().contains("C1/S1"), "{status}");
    let r = common::full_report(&common::spec(), CycleKind::Interview, NodeKind::Question);
    let report: String = r.field_names().map(|k| format!("{k}: {}\n", r.get(k).unwrap())).collect();
    let closed = monitor_call(&mut mcp, "gil_close", json!({"project_root": at, "report": report}));
    assert_eq!(closed["structuredContent"]["ok"], true, "{closed}");
    assert_ne!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), before);
    mcp.eof();
    std::fs::remove_dir_all(at).unwrap();
}

/// 진짜 Project 하나를 만든다 — CLI 로. 시험이 저장 형식을 손으로 짓지 않는다.
fn started(at: &Path) {
    let done = Command::new(gil())
        .arg("start")
        .current_dir(at)
        .output()
        .expect("gil start");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

fn cli_status(at: &Path) -> String {
    let done = Command::new(gil())
        .arg("status")
        .current_dir(at)
        .output()
        .expect("gil status");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8(done.stdout).expect("utf-8")
}

// ── MCP 손님 ───────────────────────────────────────────────────────────────

struct Client {
    kid: Child,
    out: BufReader<std::process::ChildStdout>,
    next: i64,
    settings: Option<PathBuf>,
}

impl Client {
    /// **Node 없이** 선다 — Rust binary 하나가 전부다.
    fn serve() -> Self {
        let settings = scratch("client-settings");
        let mut client = Self::serve_with(&settings);
        client.settings = Some(settings);
        client
    }

    fn serve_with(settings: &Path) -> Self {
        Self::serve_with_env(settings, &[])
    }

    /// Extra environment for the server process (e.g. a synthetic `OneDrive` root).
    fn serve_with_env(settings: &Path, env: &[(&str, &Path)]) -> Self {
        let mut command = Command::new(gil());
        for (key, value) in env { command.env(key, value); }
        let mut kid = command
            .args(["mcp", "--serve"])
            // No Node, cargo, repository executable, or global gil on PATH.
            .env("PATH", "/usr/bin:/bin")
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            // cwd 를 **다른 자리**에 둔다. 요청 결과가 여기에 흔들리면 그것이 결함이다.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().expect("stdout"));
        Self { kid, out, next: 1, settings: None }
    }

    fn send(&mut self, frame: &serde_json::Value) {
        let line = serde_json::to_string(frame).expect("frame");
        let stdin = self.kid.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{line}").expect("보낸다");
        stdin.flush().expect("흘린다");
    }

    fn read(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.out.read_line(&mut line).expect("받는다");
        assert!(!line.trim().is_empty(), "빈 줄을 받았다");
        serde_json::from_str(&line)
            .unwrap_or_else(|err| panic!("JSON-RPC 가 아닌 것이 stdout 에 있다: {line:?} ({err})"))
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next;
        self.next += 1;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        let got = self.read();
        assert_eq!(got["id"], serde_json::json!(id), "id 가 어긋났다");
        got
    }

    fn initialize(&mut self) -> serde_json::Value {
        let got = self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "gil-test", "version": "0" }
            }),
        );
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        }));
        got
    }

    fn status(&mut self, root: &str) -> serde_json::Value {
        self.request(
            "tools/call",
            serde_json::json!({
                "name": "gil_status",
                "arguments": { "project_root": root }
            }),
        )
    }

    /// stdin 을 닫고 **깨끗이** 끝나는지 본다 — 남은 stdout 도 함께 돌려준다.
    fn eof(mut self) -> (std::process::ExitStatus, String) {
        drop(self.kid.stdin.take());
        let mut rest = String::new();
        use std::io::Read as _;
        self.out.read_to_string(&mut rest).ok();
        let done = self.kid.wait().expect("끝난다");
        if let Some(settings) = self.settings.take() { std::fs::remove_dir_all(settings).unwrap(); }
        (done, rest)
    }
}

fn text_of(reply: &serde_json::Value) -> String {
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("text 가 없다: {reply}"))
        .to_string()
}

// ── 시험 ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
#[test]
fn companion_session_tracks_actual_project_use_and_stdio_eof() {
    use std::os::unix::fs::DirBuilderExt;
    use std::time::{Duration, Instant};
    use serde_json::json;
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let base = PathBuf::from("/private/tmp").join(format!("gil-mcp-session-{}-{unique}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
    let socket = base.join("session.sock");
    let server = gil::companion_ipc::Server::bind(&socket).unwrap();
    let root = base.join("project"); std::fs::create_dir(&root).unwrap();
    let mut a = Client::serve_with_env(&base.join("a"), &[(gil::companion_ipc::ENV_ENDPOINT, &socket)]);
    a.initialize();
    a.request("tools/list", json!({}));
    assert_eq!(server.snapshot().unwrap().active_connections, 0);
    assert_eq!(a.status(root.to_str().unwrap())["result"]["isError"], true);
    assert_eq!(server.snapshot().unwrap().active_connections, 0);
    let help = monitor_call(&mut a, "gil_help", json!({"topic":"overview", "project_root":root}));
    let _ = help; // Whether the topic exists is unrelated; help cannot register a session.
    assert_eq!(server.snapshot().unwrap().active_connections, 0);
    let started = monitor_call(&mut a, "gil_start", json!({"project_root":root}));
    assert_eq!(started["isError"], false, "{started}");
    assert_eq!(server.snapshot().unwrap().active_connections, 1);
    let before = std::fs::read(root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(a.status(root.to_str().unwrap())["result"]["isError"], false);
    assert_eq!(server.snapshot().unwrap().active_connections, 1);
    let mut b = Client::serve_with_env(&base.join("b"), &[(gil::companion_ipc::ENV_ENDPOINT, &socket)]);
    b.initialize(); b.status(root.to_str().unwrap());
    assert_eq!(server.snapshot().unwrap().active_connections, 2);
    assert!(a.eof().0.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.snapshot().unwrap().active_connections != 1 {
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!server.finished());
    assert!(b.eof().0.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !server.finished() {
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    // Unavailable endpoint cannot undo/replay successful Project operations.
    let mut c = Client::serve_with_env(&base.join("c"), &[(gil::companion_ipc::ENV_ENDPOINT, &socket)]);
    c.initialize();
    assert_eq!(c.status(root.to_str().unwrap())["result"]["isError"], false);
    assert!(c.eof().0.success());
    drop(server);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn initialize_succeeds_and_says_who_is_answering() {
    let mut mcp = Client::serve();
    let got = mcp.initialize();
    assert_eq!(got["jsonrpc"], "2.0");
    assert!(got["result"]["serverInfo"]["name"].is_string(), "{got}");
    assert!(got["result"]["capabilities"]["tools"].is_object(), "{got}");
    let (done, _) = mcp.eof();
    assert!(done.success());
}

#[test]
fn tools_list_preserves_nineteen_tools_and_the_read_only_app_boundary() {
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.request("tools/list", serde_json::json!({}));
    let tools = got["result"]["tools"].as_array().expect("tools");
    let mut names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    let mut expected=vec!["gil_close", "gil_companion_status", "gil_context", "gil_cycle", "gil_help", "gil_monitor_detail", "gil_monitor_poll", "gil_monitor_prepare", "gil_monitor_read", "gil_monitor_report_image", "gil_monitor_report_visuals", "gil_open", "gil_restore", "gil_revisit", "gil_start", "gil_status", "gil_story", "show_gil_companion", "show_gil_monitor"];
    if cfg!(feature="visual-authoring") {
        expected.extend(["gil_report_visuals", "gil_migration_check", "gil_migration_prepare", "gil_migration_apply", "gil_migration_status"]);
        if cfg!(windows) { expected.push("gil_prepare_workspace"); }
        expected.sort();
    }
    assert_eq!(names, expected);
    for tool in tools.iter().filter(|t| t["name"]=="gil_report_visuals") {
        assert_eq!(tool["annotations"]["readOnlyHint"],true);
        assert_eq!(tool["_meta"]["ui"]["visibility"],serde_json::json!(["model"]));
    }
    for tool in tools.iter().filter(|t| t["name"].as_str().unwrap().starts_with("gil_monitor_") || t["name"] == "show_gil_monitor") {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    for tool in tools.iter().filter(|t| t["name"].as_str().unwrap().starts_with("gil_monitor_report_")) {
        assert_eq!(tool["_meta"]["ui"]["visibility"],serde_json::json!(["app"]));
    }
    mcp.eof();
}

#[test]
fn the_app_resource_is_embedded_and_binds_both_metadata_keys() {
    let mut mcp = Client::serve();
    let init = mcp.initialize();
    assert!(init["result"]["capabilities"]["resources"].is_object());
    let got = mcp.request("tools/list", serde_json::json!({}));
    let show = got["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "show_gil_monitor").unwrap();
    let uri = show["_meta"]["ui"]["resourceUri"].clone();
    assert_eq!(uri, show["_meta"]["ui/resourceUri"]);
    let listed = mcp.request("resources/list", serde_json::json!({}));
    assert_eq!(listed["result"]["resources"][0]["uri"], uri);
    let resource = mcp.request("resources/read", serde_json::json!({"uri": uri}));
    let content = &resource["result"]["contents"][0];
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    assert_eq!(content["text"], include_str!("../plugins/gil-companion-prototype/assets/monitor.html"));
    assert_eq!(content["_meta"]["ui"]["csp"]["connectDomains"], serde_json::json!([]));
    let rejected = mcp.request("resources/read", serde_json::json!({"uri": "file:///etc/passwd"}));
    assert!(rejected["error"].is_object());
    mcp.eof();
}

#[test]
fn native_actions_keep_the_wrapper_and_reject_extra_monitor_inputs() {
    let at = scratch("native-actions");
    let mut mcp = Client::serve(); mcp.initialize();
    let start = monitor_call(&mut mcp, "gil_start", serde_json::json!({"project_root": at}));
    assert_eq!(start["structuredContent"]["ok"], true);
    assert_eq!(start["structuredContent"]["exit_code"], 0);
    let refused = monitor_call(&mut mcp, "gil_open", serde_json::json!({"project_root": at, "kind": "--help"}));
    assert_eq!(refused["isError"], true);
    assert_eq!(refused["structuredContent"]["exit_code"], serde_json::Value::Null);
    assert_eq!(refused["structuredContent"]["problem"], "Cycle 종류 는 '-' 로 시작할 수 없다");
    let root = serde_json::json!({"project_root": at});
    let prepare = monitor_call(&mut mcp, "gil_monitor_prepare", root);
    let scope = prepare["structuredContent"]["scope_id"].clone();
    let invalid = mcp.request("tools/call", serde_json::json!({"name": "gil_monitor_read", "arguments": {"scope_id": scope, "project_root": at}}));
    assert!(invalid["error"].is_object());
    let view = monitor_call(&mut mcp, "show_gil_monitor", serde_json::json!({"scope_id": scope}));
    assert_ne!(view["isError"], true);
    assert!(view["content"].as_array().is_some_and(|content| !content.is_empty()), "{view}");
    assert_eq!(view["structuredContent"]["scope_id"], scope);
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

fn monitor_call(client: &mut Client, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    client.request("tools/call", serde_json::json!({ "name": name, "arguments": arguments }))["result"].clone()
}

fn record_question(at: &Path, answer: &str) {
    for (args, input) in [
        (vec!["open", "question"], "objective: 재시작 검수\nnext_action: 질문한다\ndone_when: 답을 기록했다\n".to_owned()),
        (vec!["close"], format!("question: 어느 Project 인가\nchoices: 하나 / 둘\nresponse: {answer}\n")),
    ] {
        let mut process = Command::new(gil()).args(args).current_dir(at).stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        process.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let done = process.wait_with_output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    }
}

#[test]
fn a_new_process_restores_each_old_scope_without_prepare_or_a_project_path() {
    let at = scratch("automatic-resume");
    let a = at.join("a/same-name"); let b = at.join("b/same-name");
    for root in [&a, &b] { std::fs::create_dir_all(root).unwrap(); started(root); }
    record_question(&a, "첫 번째 기록"); record_question(&b, "두 번째 기록");
    let original_a = std::fs::read(a.join(gil::STATE_PATH)).unwrap();
    let original_b = std::fs::read(b.join(gil::STATE_PATH)).unwrap();
    let mut old = Client::serve_with(&at.join("settings")); old.initialize();
    let mut captured = Vec::new();
    for root in [&a, &b] {
        let prepared = monitor_call(&mut old, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
        assert_ne!(prepared["isError"], true, "{prepared}");
        let scope = prepared["structuredContent"]["scope_id"].clone();
        let detail = monitor_call(&mut old, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
        let view = monitor_call(&mut old, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
        assert_ne!(view["isError"], true, "{view}");
        captured.push((scope, detail["structuredContent"].clone(), view["structuredContent"].clone()));
    }
    assert_ne!(captured[0].0, captured[1].0); assert_ne!(captured[0].1, captured[1].1);
    assert!(old.eof().0.success());

    let mut fresh = Client::serve_with(&at.join("settings")); fresh.initialize();
    // No root, prepare, current directory, Companion selection, or Agent intervention.
    for (scope, expected_detail, expected_view) in &captured {
        let detail = monitor_call(&mut fresh, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
        assert_eq!(&detail["structuredContent"], expected_detail);
        let view = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
        assert_eq!(view["structuredContent"]["view"]["timeline"], expected_view["view"]["timeline"]);
        assert_ne!(view["structuredContent"]["revision"], expected_view["revision"]);
        assert!(!view.to_string().contains(at.to_str().unwrap()));
    }
    assert_eq!(std::fs::read(a.join(gil::STATE_PATH)).unwrap(), original_a);
    assert_eq!(std::fs::read(b.join(gil::STATE_PATH)).unwrap(), original_b);
    assert!(fresh.eof().0.success()); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn restart_does_not_open_a_replacement_even_after_it_is_explicitly_prepared() {
    let at = scratch("replacement-safety"); let root = at.join("project");
    std::fs::create_dir(&root).unwrap(); started(&root);
    let mut old = Client::serve_with(&at.join("settings")); old.initialize();
    let prepared = monitor_call(&mut old, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    let scope = prepared["structuredContent"]["scope_id"].clone();
    old.eof();
    std::fs::rename(&root, at.join("original-project")).unwrap();
    let mut fresh = Client::serve_with(&at.join("settings")); fresh.initialize();
    let missing = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(missing["structuredContent"]["code"], "project_missing");
    std::fs::create_dir(&root).unwrap(); started(&root);
    let replaced = monitor_call(&mut fresh, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_ne!(replaced["isError"], true, "{replaced}");
    assert_ne!(replaced["structuredContent"]["scope_id"], scope);
    let refused = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(refused["structuredContent"]["code"], "project_moved");
    assert!(!refused.to_string().contains(at.to_str().unwrap()));
    fresh.eof(); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn monitor_uses_exact_scope_and_canonical_view_without_touching_project_bytes() {
    let at = scratch("monitor-read-only");
    started(&at);
    std::fs::write(at.join("work.txt"), "원문 세계").unwrap();
    let state = std::fs::read(at.join(gil::STATE_PATH)).unwrap();
    let mut mcp = Client::serve(); mcp.initialize();
    let ready = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": at }));
    assert_ne!(ready["isError"], true, "{ready}");
    assert!(!ready.to_string().contains(at.to_str().unwrap()));
    let scope = ready["structuredContent"]["scope_id"].clone();
    let seen = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_ne!(seen["isError"], true, "{seen}");
    let mut actual = seen["structuredContent"]["view"].clone();
    let session = gil::ProjectSession::open_read_only(gil::RuleSet::builtin().unwrap(), at.join(gil::STATE_PATH)).unwrap();
    let view = gil::monitor_view_v1(&session.monitor().unwrap()).unwrap();
    let mut expected = serde_json::to_value(&view).unwrap();
    actual["captured_at_unix_ms"] = serde_json::json!(0);
    expected["captured_at_unix_ms"] = serde_json::json!(0);
    assert_eq!(actual, expected);
    drop(session);
    let hint = monitor_call(&mut mcp, "gil_monitor_poll", serde_json::json!({ "scope_id": scope }));
    assert_eq!(hint["structuredContent"].as_object().unwrap().len(), 3);
    assert!(hint["structuredContent"]["revision"].is_string());
    assert!(hint["structuredContent"].get("view").is_none());
    assert_eq!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), state);
    assert_eq!(std::fs::read_to_string(at.join("work.txt")).unwrap(), "원문 세계");
    // Unknown identity is well-formed. A malformed scope is a schema rejection,
    // matching the public JS adapter, and must not enter the binding registry.
    let malformed = mcp.request("tools/call", serde_json::json!({
        "name": "gil_monitor_read", "arguments": {"scope_id": "project:unknown"}
    }));
    assert_eq!(malformed["error"]["code"], -32602);
    let unknown = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": format!("project:{}", "0".repeat(64)) }));
    assert_eq!(unknown["structuredContent"]["code"], "reconnect_required");
    let child = at.join("child"); std::fs::create_dir(&child).unwrap();
    let refused = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": child }));
    assert_eq!(refused["structuredContent"]["code"], "not_a_project");
    assert!(!refused.to_string().contains(at.to_str().unwrap()));
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn monitor_report_is_complete_and_a_busy_project_is_not_recovered_or_rewritten() {
    let at = scratch("monitor-detail"); started(&at);
    let mut opening = Command::new(gil()).args(["open", "question"]).current_dir(&at)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    opening.stdin.take().unwrap().write_all("objective: Monitor 검수\nnext_action: 질문한다\ndone_when: 답을 기록했다\n".as_bytes()).unwrap();
    let opened = opening.wait_with_output().unwrap();
    assert!(opened.status.success(), "{}", String::from_utf8_lossy(&opened.stderr));
    let mut close = Command::new(gil()).args(["close"]).current_dir(&at)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    close.stdin.take().unwrap().write_all("question: 무엇을 만들까\nchoices: 검수 / 다음\nresponse: <script>원문</script>\n".as_bytes()).unwrap();
    let done = close.wait_with_output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let before = std::fs::read(at.join(gil::STATE_PATH)).unwrap();
    let mut mcp = Client::serve(); mcp.initialize();
    let ready = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": at }));
    let scope = ready["structuredContent"]["scope_id"].clone();
    let detail = monitor_call(&mut mcp, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
    assert_ne!(detail["isError"], true, "{detail}");
    let fields = detail["structuredContent"]["detail"]["report"]["fields"].as_array().unwrap();
    assert_eq!(fields.iter().map(|f| f["name"].as_str().unwrap()).collect::<Vec<_>>(), ["choices", "question", "response"]);
    assert_eq!(fields[2]["value"], "<script>원문</script>");
    let missing = monitor_call(&mut mcp, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C2/S1" }));
    assert_eq!(missing["structuredContent"]["code"], "not_found");
    let locked = gil::ProjectSession::open(gil::RuleSet::builtin().unwrap(), at.join(gil::STATE_PATH)).unwrap();
    let busy = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(busy["structuredContent"]["code"], "busy");
    drop(locked);
    assert_eq!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), before);
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

/// **이 조각의 목표 문장.** 같은 Project 에서 두 진입점이 같은 글자를 낸다.
#[test]
fn the_cli_and_the_mcp_say_the_very_same_bytes() {
    let at = scratch("same");
    started(&at);
    let by_cli = cli_status(&at);

    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(at.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(false), "{got}");
    let by_mcp = text_of(&got);
    mcp.eof();

    assert_eq!(
        by_mcp.as_bytes(),
        by_cli.as_bytes(),
        "같은 자리에서 다른 말을 했다\nCLI:\n{by_cli}\nMCP:\n{by_mcp}"
    );
    std::fs::remove_dir_all(&at).ok();
}

/// 부모가 Project 여도 **자식은 자식이다.** 말없이 위로 물러서지 않는다.
#[test]
fn a_child_directory_never_opens_the_parent_project() {
    let parent = scratch("parent");
    started(&parent);
    let child = parent.join("tmp");
    std::fs::create_dir_all(&child).expect("자리");

    let before = std::fs::read(parent.join(".gil/state.yaml")).expect("기록");
    let by_cli_at_parent = cli_status(&parent);

    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(child.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    let said = text_of(&got);
    mcp.eof();

    // 거절이지, 부모의 상태가 아니다.
    assert_ne!(said, by_cli_at_parent, "자식 요청이 부모 Project 를 열었다");
    assert!(said.contains("위로 거슬러 오르지 않았다"), "{said}");
    // 거절문에 절대경로를 싣지 않는다.
    assert!(!said.contains('/'), "거절문에 경로가 새어 나왔다: {said}");

    // 부모의 bytes 와 Journey 는 그대로다.
    let after = std::fs::read(parent.join(".gil/state.yaml")).expect("기록");
    assert_eq!(before, after, "부모의 기록이 바뀌었다");
    assert_eq!(cli_status(&parent), by_cli_at_parent, "부모의 자리가 움직였다");

    std::fs::remove_dir_all(&parent).ok();
}

#[test]
fn a_relative_project_root_is_refused() {
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status("some/where");
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    let said = text_of(&got);
    assert!(said.contains("온전한 경로"), "{said}");
    mcp.eof();
}

#[test]
fn starting_receipt_uses_the_requested_root_not_the_servers_directory() {
    let base = scratch("start-receipt");
    let cli_root = base.join("cli-project");
    let mcp_root = base.join("mcp-project");
    std::fs::create_dir(&cli_root).unwrap();
    std::fs::create_dir(&mcp_root).unwrap();
    let cli = Command::new(gil()).arg("start").current_dir(&cli_root).output().unwrap();
    assert!(cli.status.success());
    let mut mcp = Client::serve();
    mcp.initialize();
    let reply = mcp.request("tools/call", serde_json::json!({
        "name": "gil_start", "arguments": {"project_root": mcp_root}
    }));
    #[cfg(not(windows))]
    {
        assert_eq!(reply["result"]["structuredContent"]["said"], String::from_utf8(cli.stdout).unwrap());
        let relative = Path::new(".gil").join("state.yaml");
        assert!(text_of(&reply).ends_with(&format!("기록: {}\n", relative.display())));
    }
    #[cfg(windows)]
    {
        let said = text_of(&reply);
        assert!(said.ends_with("기록: Windows 개인 앱 데이터의 보호된 기록\n"), "{said}");
        assert!(!said.contains("project-state-v1") && !said.contains("LOCALAPPDATA"), "{said}");
    }
    mcp.eof();
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn a_place_without_a_project_is_refused() {
    let empty = scratch("empty");
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(empty.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    assert!(text_of(&got).contains("걷기가 없다"), "{got}");
    mcp.eof();
    std::fs::remove_dir_all(&empty).ok();
}

/// server 의 cwd 가 **다른 GIL Project** 여도 답은 준 자리의 것이다.
#[test]
fn the_servers_own_working_directory_never_leaks_into_the_answer() {
    let asked = scratch("asked");
    let elsewhere = scratch("elsewhere");
    started(&asked);
    started(&elsewhere);

    let mut kid = Command::new(gil())
        .args(["mcp", "--serve"])
        .current_dir(&elsewhere) // ← 다른 Project 안에 서 있다
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve");
    let out = BufReader::new(kid.stdout.take().expect("stdout"));
    let mut mcp = Client { kid, out, next: 1, settings: None };
    mcp.initialize();
    let got = mcp.status(asked.to_str().expect("utf-8"));
    let said = text_of(&got);
    mcp.eof();

    assert_eq!(said.as_bytes(), cli_status(&asked).as_bytes(), "{said}");
    std::fs::remove_dir_all(&asked).ok();
    std::fs::remove_dir_all(&elsewhere).ok();
}

/// stdin 이 닫히면 **깨끗이** 끝난다 — 매달리지 않고, 뒤에 쓰레기를 남기지 않는다.
#[test]
fn closing_stdin_ends_the_server_cleanly_and_leaves_nothing_behind() {
    let at = scratch("eof");
    started(&at);
    let mut mcp = Client::serve();
    mcp.initialize();
    mcp.status(at.to_str().expect("utf-8"));
    let (done, rest) = mcp.eof();
    assert!(done.success(), "깨끗이 끝나지 않았다: {done:?}");
    assert!(rest.trim().is_empty(), "마지막 frame 뒤에 글자가 남았다: {rest:?}");
    std::fs::remove_dir_all(&at).ok();
}

/// **stdout 에는 JSON-RPC 말고 아무것도 없다.** 진단 한 줄이 새면 여기서 걸린다.
#[test]
fn stdout_carries_json_rpc_frames_and_nothing_else() {
    let at = scratch("clean");
    started(&at);

    let mut kid = Command::new(gil())
        .args(["mcp", "--serve"])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve");
    {
        let stdin = kid.stdin.as_mut().expect("stdin");
        for line in [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#.to_string(),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#.to_string(),
            serde_json::json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"gil_status","arguments":{"project_root": at.to_str().expect("utf-8")}}
            }).to_string(),
            // 일부러 거절당할 것 하나 — 거절의 말이 stdout 으로 새는지 본다.
            serde_json::json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"gil_status","arguments":{"project_root":"relative/path"}}
            }).to_string(),
        ] {
            writeln!(stdin, "{line}").expect("보낸다");
        }
    }
    drop(kid.stdin.take());
    let done = kid.wait_with_output().expect("끝난다");
    assert!(done.status.success());

    let said = String::from_utf8(done.stdout).expect("utf-8");
    let mut frames = 0;
    for line in said.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let one: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("stdout 에 JSON-RPC 가 아닌 줄이 있다: {line:?} ({err})"));
        assert_eq!(one["jsonrpc"], "2.0", "frame 이 아니다: {line}");
        frames += 1;
    }
    assert_eq!(frames, 4, "답이 넷이어야 한다:\n{said}");

    std::fs::remove_dir_all(&at).ok();
}

// ── T1.1 시작과 관측을 한 흐름으로 ──────────────────────────────────────────
//
// Skill 이 「GIL 프로젝트를 시작하자」 한 요청에 잇는 도구 차례를 **같은 stdio 연결**에서
// 그대로 밟는다. 여기서 재는 것은 배선의 성질 넷이다.
//
//   1. start → prepare → show 가 한 연결에서 이어지고, 첫 View 는 열린 Interview 하나다
//   2. 이미 시작된 자리의 두 번째 start 는 거절되고 기록은 byte 로 같다 — 재초기화 없음
//   3. 화면 재시도(prepare/show 반복)는 같은 scope 를 돌려주고 binding 도 하나뿐이다
//   4. 시작 실패와 App 조회의 typed 거절은 서로 다른 답이며, 어느 쪽도 Project 를 만들거나 바꾸지 않는다
//
// 이것은 **stdio 도구 연쇄**의 시험이다. 실제 Host 의 카드 렌더링·fullscreen 전환·화면 표시는
// 여기서 재지 않는다 — 그것은 App 진입점 시험(`mcp-app/display.test.mjs`)과 사용자 화면
// 확인이 따로 맡는다. 아래 5의 알 수 없는 scope 거절도 Host 렌더링 실패의 시험이 아니다.

fn agent_call(client: &mut Client, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    client.request("tools/call", serde_json::json!({ "name": name, "arguments": arguments }))["result"].clone()
}

fn binding_files(settings: &Path) -> usize {
    std::fs::read_dir(settings.join("bindings")).map(|dir| dir.filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json")).count()).unwrap_or(0)
}

#[cfg(all(feature="visual-authoring", any(all(target_os="macos", target_arch="aarch64"), windows)))]
#[test]
fn visual_start_preserves_existing_and_partial_records_and_survives_response_loss() {
    use serde_json::json;
    fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root:&Path, path:&Path, out:&mut std::collections::BTreeMap<PathBuf,Vec<u8>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path=entry.unwrap().path();
                if path.is_dir() { walk(root,&path,out); }
                else { out.insert(path.strip_prefix(root).unwrap().to_owned(),std::fs::read(path).unwrap()); }
            }
        }
        let mut out=std::collections::BTreeMap::new(); walk(root,root,&mut out); out
    }
    let base=scratch("n3-preserve");
    let settings=base.join("settings");
    let mut mcp=Client::serve_with(&settings); mcp.initialize();
    for kind in ["legacy", "visual", "partial", "unknown", "no-lock"] {
        let root=base.join(kind); std::fs::create_dir(&root).unwrap();
        if kind=="visual" {
            assert_eq!(monitor_call(&mut mcp,"gil_start",json!({"project_root":root}))["structuredContent"]["ok"],true);
        } else if kind=="legacy" {
            let s=gil::ProjectSession::start(common::spec(),root.join(gil::STATE_PATH)).unwrap(); s.commit().unwrap();
        } else {
            std::fs::create_dir(root.join(".gil")).unwrap();
            std::fs::write(root.join(".gil/state.yaml.writing"),b"partial evidence").unwrap();
            if kind=="unknown" {
                std::fs::write(root.join(gil::STATE_PATH),b"format: 999\n").unwrap();
                std::fs::write(root.join(".gil/project.lock"),b"").unwrap();
            }
            if kind=="no-lock" { std::fs::write(root.join(gil::STATE_PATH),b"unreadable old record").unwrap(); }
        }
        // Even a read must not reap these unfinished objects.
        std::fs::create_dir_all(root.join(".gil/artifacts/tmp")).unwrap();
        std::fs::write(root.join(".gil/artifacts/tmp/unfinished"),b"keep evidence").unwrap();
        let before=files(&root);
        let refusal=monitor_call(&mut mcp,"gil_start",json!({"project_root":root}));
        assert_eq!(refusal["structuredContent"]["ok"],false,"{refusal}");
        for name in ["gil_status","gil_context"] {
            let got=monitor_call(&mut mcp,name,json!({"project_root":root}));
            assert_eq!(got["structuredContent"]["ok"],matches!(kind,"legacy"|"visual"),"{kind}: {got}");
        }
        assert_eq!(files(&root),before,"{kind}");
    }
    let root=base.join("response-lost"); std::fs::create_dir(&root).unwrap();
    assert_eq!(monitor_call(&mut mcp,"gil_start",json!({"project_root":root}))["structuredContent"]["ok"],true);
    let before=files(&root);
    mcp.eof(); // Caller loses the receipt; new server must inspect, not restart.
    let mut mcp=Client::serve_with(&settings); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp,"gil_status",json!({"project_root":root}))["structuredContent"]["ok"],true);
    let prepared=monitor_call(&mut mcp,"gil_monitor_prepare",json!({"project_root":root}));
    let shown=monitor_call(&mut mcp,"show_gil_monitor",json!({"scope_id":prepared["structuredContent"]["scope_id"]}));
    assert_eq!(shown["structuredContent"]["view"]["timeline"].as_array().unwrap().len(),1);
    assert!(shown["structuredContent"]["view"]["current"]["step_ref"].is_null());
    assert_eq!(files(&root),before);
    mcp.eof();
}

#[test]
fn starting_a_project_leads_to_one_monitor_scope_and_the_first_interview_updates_it() {
    let at = scratch("start-and-watch");
    let root = at.join("project");
    std::fs::create_dir(&root).unwrap();
    let settings = at.join("settings");
    let mut mcp = Client::serve_with(&settings); mcp.initialize();

    // 1. 시작 — Skill 의 첫 걸음. 요청한 root 의 receipt 를 그대로 받는다.
    let started = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(started["isError"], false, "{started}");
    assert_eq!(started["structuredContent"]["ok"], true);
    let receipt = started["structuredContent"]["said"].as_str().unwrap();
    assert!(receipt.starts_with("프로젝트를 시작했다.\n현재: cycle:C1 · interview\n"), "{receipt}");
    assert!(receipt.contains("실행\n  gil open\n"), "{receipt}");
    let state = std::fs::read(product_state_path(&root)).unwrap();
    assert!(state.starts_with(if cfg!(feature="visual-authoring") { b"format: 5\n" } else { b"format: 4\n" }));
    if cfg!(feature="visual-authoring") {
        assert_eq!(started["structuredContent"]["storage_format"],5);
        assert_eq!(started["structuredContent"]["display_confirmed"],false);
    }
    assert_eq!(binding_files(&settings), 0, "시작만으로는 Monitor binding 을 만들지 않는다");

    // 2. 준비 → 표시 — 같은 연결, 별도 요청 없음. 첫 View 는 Step 이 없는 열린 Interview 다.
    let prepared = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_ne!(prepared["isError"], true, "{prepared}");
    let scope = prepared["structuredContent"]["scope_id"].clone();
    assert_eq!(prepared["structuredContent"]["label"], "project");
    let shown = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": scope }));
    assert_ne!(shown["isError"], true, "{shown}");
    assert!(shown["content"][0]["text"].as_str().unwrap().contains("fullscreen을 한 번 자동 요청"), "{shown}");
    let view = &shown["structuredContent"]["view"];
    assert_eq!(view["schema_version"], 1);
    assert_eq!(view["current"]["cycle_ref"], "cycle:C1");
    assert!(view["current"]["step_ref"].is_null());
    assert_eq!(view["timeline"].as_array().unwrap().len(), 1);
    assert_eq!(view["timeline"][0]["kind"], "interview");
    assert_eq!(view["timeline"][0]["state"], "open");
    assert_eq!(view["timeline"][0]["steps"], serde_json::json!([]));
    assert_eq!(shown["structuredContent"]["scope_id"], scope);
    assert!(!shown.to_string().contains(at.to_str().unwrap()), "App 응답에 경로가 새어 나왔다");
    let first_hint = shown["structuredContent"]["revision"].as_str().unwrap().to_owned();
    let watching = shown["structuredContent"]["watching"] == true;
    assert_eq!(binding_files(&settings), 1);

    // 3. 두 번째 start — 이미 걷고 있다. 거절이며 기록은 byte 로 같다.
    let again = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(again["isError"], true, "{again}");
    assert_eq!(again["structuredContent"]["ok"], false);
    let problem = again["structuredContent"]["problem"].as_str().unwrap();
    assert!(problem.contains("여기서 새로 시작할 수 없다.") && problem.contains("이미 걷고 있다"), "{problem}");
    assert!(problem.contains("gil status"), "복구 안내가 없다: {problem}");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(), state, "거절된 start 가 기록을 바꿨다");

    // 4. 화면 재시도 — prepare/show 를 다시 불러도 같은 scope, binding 하나, 기록 불변.
    let retried = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_eq!(retried["structuredContent"]["scope_id"], scope, "재시도가 새 scope 를 만들었다");
    let reshown = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": scope }));
    assert_ne!(reshown["isError"], true, "{reshown}");
    assert_eq!(reshown["structuredContent"]["view"]["timeline"], view["timeline"]);
    assert_eq!(binding_files(&settings), 1, "재시도가 binding 을 늘렸다");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(), state);

    // 5. App 조회의 typed 거절(알 수 없는 scope)은 Project 를 만지지 않는다 — 그 뒤 start 도 여전히
    //    거절이다. Host 가 카드를 그리지 못한 경우를 흉내 내는 것이 아니다.
    let lost = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": format!("project:{}", "0".repeat(64)) }));
    assert_eq!(lost["isError"], true);
    assert_eq!(lost["structuredContent"]["code"], "reconnect_required");
    assert!(lost["structuredContent"].get("ok").is_none(), "Monitor 거절이 Agent 동작의 모양을 흉내 냈다");
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(), state);
    let still = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(still["structuredContent"]["ok"], false);
    assert_eq!(std::fs::read(product_state_path(&root)).unwrap(), state);

    // 6. 시작 실패는 화면 이전에 끝난다 — 없는 자리는 start 도 prepare 도 거절이고 binding 은 그대로다.
    let nowhere = at.join("missing");
    let failed = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": nowhere }));
    assert_eq!(failed["structuredContent"]["ok"], false);
    assert_eq!(failed["structuredContent"]["problem"], "Project 자리를 열 수 없다");
    assert!(!nowhere.exists(), "실패한 start 가 폴더를 만들었다");
    let unprepared = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": nowhere }));
    assert_eq!(unprepared["isError"], true);
    assert_eq!(unprepared["structuredContent"]["code"], "unreadable");
    assert_eq!(binding_files(&settings), 1);

    // 7. 첫 Interview Step — Skill 의 셋째 걸음. 렌더 도구를 다시 부르지 않아도 View 가 바뀐다.
    let opened = agent_call(&mut mcp, "gil_open", serde_json::json!({ "project_root": root, "kind": "question",
        "contract": "objective: 사용자의 목표를 확인한다\nnext_action: 무엇을 만들고 싶은지 묻는다\ndone_when: 사용자의 원문 응답을 얻는다\n" }));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    let updated = agent_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_ne!(updated["isError"], true, "{updated}");
    let now = &updated["structuredContent"]["view"];
    assert_eq!(now["current"]["step_ref"], "step:C1/S1");
    assert_eq!(now["timeline"][0]["steps"][0]["step_ref"], "step:C1/S1");
    assert_eq!(now["timeline"][0]["steps"][0]["state"], "open");
    assert_eq!(now["timeline"][0]["steps"][0]["kind"], "question");
    assert_eq!(now["current_will"]["objective"], "사용자의 목표를 확인한다");

    // 8. 변화 hint — watcher 가 서 있으면 첫 Step 의 기록이 유한한 시간 안에 새 revision 이 된다.
    //    OS 사건이 막힌 환경이면 hint 없이도 위 완전 재조회가 이미 새 사실을 읽었다(§8.3).
    //    어느 분기를 밟았는지는 `--nocapture` 로 보이게 적는다.
    if watching {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut hint = first_hint.clone();
        while hint == first_hint && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let polled = agent_call(&mut mcp, "gil_monitor_poll", serde_json::json!({ "scope_id": scope }));
            hint = polled["structuredContent"]["revision"].as_str().unwrap().to_owned();
        }
        assert_ne!(hint, first_hint, "첫 Step 을 열었는데 30초 안에 변화 hint 가 오지 않았다");
        eprintln!("watcher branch: live — 실제 OS 감시가 첫 Step 의 hint 를 냈다");
    } else {
        eprintln!("watcher branch: unavailable — hint 없이 완전 재조회로만 갱신을 확인했다");
    }

    assert!(mcp.eof().0.success());
    std::fs::remove_dir_all(at).unwrap();
}

/// A OneDrive working folder stays selected while private records go to local app data. The
/// separate-workspace helper remains an explicitly approved convenience, not a repair path.
#[cfg(all(windows, feature = "visual-authoring"))]
#[test]
fn onedrive_working_folder_stays_put_and_workspace_prep_still_needs_approval() {
    use serde_json::json;
    let base = scratch("ws-location");
    let settings = base.join("settings");
    // A synthetic OneDrive root: the environment names it, nothing is synchronized. The user
    // profile is the test base, so the allowed range for new workspaces is the base.
    let onedrive = base.join("OneDrive");
    std::fs::create_dir_all(onedrive.join("문서 test")).unwrap();
    std::fs::write(onedrive.join("문서 test").join("existing.txt"), b"keep").unwrap();
    let mut mcp = Client::serve_with_env(&settings, &[("OneDrive", &onedrive), ("USERPROFILE", &base)]); mcp.initialize();
    let selected = onedrive.join("문서 test");
    let before: Vec<_> = std::fs::read_dir(&selected).unwrap().map(|e| e.unwrap().file_name()).collect();
    let started_here = monitor_call(&mut mcp, "gil_start", json!({"project_root": selected}));
    assert_eq!(started_here["structuredContent"]["ok"], true, "{started_here}");
    assert_eq!(started_here["structuredContent"]["record_storage"], "private_app_data");
    assert!(!selected.join(".gil").exists(), "private records must not be placed in OneDrive");
    let after: Vec<_> = std::fs::read_dir(&selected).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(after, before);
    // A supported folder outside that root starts normally (same server, same environment).
    let fine = base.join("정상 폴더");
    std::fs::create_dir(&fine).unwrap();
    assert_eq!(monitor_call(&mut mcp, "gil_start", json!({"project_root": fine}))["structuredContent"]["ok"], true);

    // Workspace preparation: plan first (creates nothing), then confirm (creates exactly that).
    let parent = base.join("gil-projects");
    let plan = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "새 작업 폴더", "parent": parent, "confirm": false}));
    assert_eq!(plan["structuredContent"]["ok"], true, "{plan}");
    assert_eq!(plan["structuredContent"]["stage"], "planned");
    assert_eq!(plan["structuredContent"]["creates_parent"], true);
    let shown = plan["structuredContent"]["path"].as_str().unwrap().to_owned();
    let planned = std::path::PathBuf::from(&shown);
    assert_eq!(planned, parent.join("새 작업 폴더"));
    assert!(!parent.exists(), "a plan creates nothing");
    // The default parent (no `parent` given) is %USERPROFILE%\gil-projects.
    let default = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "기본", "confirm": false}));
    assert_eq!(default["structuredContent"]["path"], parent.join("기본").display().to_string(), "{default}");
    assert!(!parent.exists());
    // confirm:true without the approved path, or with a different one, creates nothing.
    let unbound = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "새 작업 폴더", "parent": parent, "confirm": true}));
    assert_eq!(unbound["structuredContent"]["code"], "invalid_request", "{unbound}");
    let other = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "새 작업 폴더", "parent": parent, "confirm": true, "approved_path": parent.join("다른 폴더").display().to_string()}));
    assert_eq!(other["structuredContent"]["code"], "path_mismatch", "{other}");
    assert!(!parent.exists(), "nothing is created without the exact approved path");
    let made = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "새 작업 폴더", "parent": parent, "confirm": true, "approved_path": shown}));
    assert_eq!(made["structuredContent"]["ok"], true, "{made}");
    assert_eq!(made["structuredContent"]["stage"], "prepared");
    assert_eq!(made["structuredContent"]["created_by_this_call"], json!([parent.display().to_string(), shown]));
    assert!(planned.is_dir());
    assert_eq!(std::fs::read_dir(&planned).unwrap().count(), 0, "the workspace is empty: no .gil until gil_start");
    assert!(made["structuredContent"]["said"].as_str().unwrap().contains("OneDrive 안의 기존 작업 폴더를 옮길 필요는 없다"), "{made}");
    // Same name again: refused, nothing changed, nothing substituted.
    std::fs::write(planned.join("note.txt"), b"user file").unwrap();
    let dup = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "새 작업 폴더", "parent": parent, "confirm": true, "approved_path": shown}));
    assert_eq!(dup["structuredContent"]["code"], "exists", "{dup}");
    assert_eq!(dup["structuredContent"]["created_by_this_call"], json!([]));
    assert_eq!(std::fs::read(planned.join("note.txt")).unwrap(), b"user file");
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 1, "no alternative name was created");
    // Under the synthetic OneDrive root, outside the user profile, or with an unsafe name:
    // refused without creating.
    let inside = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": onedrive, "confirm": true, "approved_path": onedrive.join("x").display().to_string()}));
    assert_eq!(inside["structuredContent"]["code"], "unsupported_location", "{inside}");
    assert_eq!(inside["structuredContent"]["evidence"], "declared_by_environment");
    assert!(!onedrive.join("x").exists());
    let outside = std::env::temp_dir().join("gil-ws-outside-profile");
    let out = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": outside, "confirm": false}));
    assert_eq!(out["structuredContent"]["code"], "outside_allowed_range", "{out}");
    assert!(!outside.exists());
    for bad in ["a\\b", "..", "CON", "name.", "a:b"] {
        let got = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": bad, "parent": parent, "confirm": true, "approved_path": parent.join(bad).display().to_string()}));
        assert_eq!(got["structuredContent"]["ok"], false, "{bad}: {got}");
    }
    let dotted = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": base.join("gil-projects").join("..").join("gil-projects"), "confirm": false}));
    assert_eq!(dotted["structuredContent"]["ok"], false, "{dotted}");
    // Korean and spaces in both parent and name.
    let korean = base.join("내 프로젝트 폴더");
    std::fs::create_dir(&korean).unwrap();
    let kplan = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "작업 1 (검수)", "parent": korean, "confirm": false}));
    assert_eq!(kplan["structuredContent"]["creates_parent"], false, "{kplan}");
    let kpath = kplan["structuredContent"]["path"].as_str().unwrap().to_owned();
    let kmade = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "작업 1 (검수)", "parent": korean, "confirm": true, "approved_path": kpath}));
    assert_eq!(kmade["structuredContent"]["ok"], true, "{kmade}");
    assert!(korean.join("작업 1 (검수)").is_dir());
    assert_eq!(kmade["structuredContent"]["created_by_this_call"], json!([korean.join("작업 1 (검수)").display().to_string()]));
    // A missing grandparent is a check failure, not a reason to create more.
    let deep = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": base.join("missing").join("deeper"), "confirm": false}));
    assert_eq!(deep["structuredContent"]["code"], "parent_missing", "{deep}");
    assert!(!base.join("missing").exists());
    // The parent is replaced by a junction between plan and confirm: the re-check right before
    // creation refuses, and the junction target receives nothing.
    let swapped = base.join("swapped");
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir(&swapped).unwrap();
    let splan = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": swapped, "confirm": false}));
    assert_eq!(splan["structuredContent"]["ok"], true, "{splan}");
    std::fs::remove_dir(&swapped).unwrap();
    junction(&swapped, &elsewhere);
    let sconf = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": swapped, "confirm": true, "approved_path": splan["structuredContent"]["path"]}));
    assert_eq!(sconf["structuredContent"]["code"], "unsupported_location", "{sconf}");
    assert_eq!(sconf["structuredContent"]["location_kind"], "reparse_point");
    assert_eq!(sconf["structuredContent"]["evidence"], "observed");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0, "the junction target received nothing");
    assert!(std::fs::symlink_metadata(&swapped).unwrap().file_type().is_symlink(), "the junction itself is untouched");
    // Directory creation denied in the parent: refused as permission_denied, nothing created,
    // no elevation.
    let locked = base.join("locked");
    std::fs::create_dir(&locked).unwrap();
    {
        let _deny = DenyCreate::new(&locked);
        let lplan = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": locked, "confirm": false}));
        assert_eq!(lplan["structuredContent"]["ok"], true, "a plan does not need write access: {lplan}");
        let lconf = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": locked, "confirm": true, "approved_path": lplan["structuredContent"]["path"]}));
        assert_eq!(lconf["structuredContent"]["code"], "permission_denied", "{lconf}");
        assert_eq!(lconf["structuredContent"]["created_by_this_call"], json!([]));
    }
    assert_eq!(std::fs::read_dir(&locked).unwrap().count(), 0);
    // Starting in the prepared folder works once it is the explicitly given root.
    let started = monitor_call(&mut mcp, "gil_start", json!({"project_root": planned}));
    assert_eq!(started["structuredContent"]["ok"], true, "{started}");
    assert_eq!(started["structuredContent"]["storage_format"], 5);
    mcp.eof();
}

/// Like `monitor_call`, but a JSON-RPC error (for example a -32602 schema rejection) fails with
/// the whole frame instead of yielding `null`.
#[cfg(all(windows, feature = "visual-authoring"))]
fn tool_result(client: &mut Client, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    let got = client.request("tools/call", serde_json::json!({ "name": name, "arguments": arguments }));
    assert!(got.get("error").is_none(), "{name} answered a JSON-RPC error: {got}");
    got["result"].clone()
}

/// A registry-declared sync root is accepted as a working folder because records are external.
/// The optional local-workspace helper still excludes it. The registration is made
/// with the official `reg.exe` under a GilTest provider of the current user's hive and removed
/// afterwards; it runs only where `GIL_TEST_REAL_REGISTRY=1` says the hive may be used (CI).
#[cfg(all(windows, feature = "visual-authoring"))]
#[test]
fn registry_declared_sync_root_stays_selected_without_inline_records() {
    use serde_json::json;
    if std::env::var_os("GIL_TEST_REAL_REGISTRY").as_deref() != Some(std::ffi::OsStr::new("1")) {
        eprintln!("skipped: GIL_TEST_REAL_REGISTRY != 1"); return;
    }
    let base = scratch("ws-registry");
    let root = base.join("동기화 루트 (등록)");
    std::fs::create_dir_all(root.join("바탕 화면").join("test3")).unwrap();
    std::fs::write(root.join("바탕 화면").join("test3").join("keep.txt"), b"user file").unwrap();
    // One provider key per run, so cleanup removes exactly what this run created.
    let provider = format!("HKCU\\Software\\SyncEngines\\Providers\\GilTest-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let key = format!("{provider}\\{{registration}}");
    let reg = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/reg.exe");
    struct Cleanup(PathBuf, String);
    impl Drop for Cleanup { fn drop(&mut self) { let _ = Command::new(&self.0).args(["delete", &self.1, "/f"]).output(); } }
    let _cleanup = Cleanup(reg.clone(), provider.clone());
    let added = Command::new(&reg).args(["add", &key, "/v", "MountPoint", "/t", "REG_SZ", "/d", &root.display().to_string(), "/f"]).output().unwrap();
    assert!(added.status.success(), "reg add failed: {}", String::from_utf8_lossy(&added.stderr));
    // The server gets no OneDrive* variable (the test harness never sets one).
    let mut mcp = Client::serve_with_env(&base.join("settings"), &[("USERPROFILE", &base)]); mcp.initialize();
    let selected = root.join("바탕 화면").join("test3");
    let before: Vec<_> = std::fs::read_dir(&selected).unwrap().map(|e| e.unwrap().file_name()).collect();
    let started_here = monitor_call(&mut mcp, "gil_start", json!({"project_root": selected}));
    assert_eq!(started_here["structuredContent"]["ok"], true, "{started_here}");
    assert_eq!(started_here["structuredContent"]["record_storage"], "private_app_data");
    assert!(!selected.join(".gil").exists(), "no private record belongs in the sync root");
    let after: Vec<_> = std::fs::read_dir(&selected).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(after, before);
    // Preparing a workspace under the registered root is refused the same way; outside it works.
    let inside = tool_result(&mut mcp, "gil_prepare_workspace", json!({"name": "x", "parent": root, "confirm": false}));
    assert_eq!(inside["structuredContent"]["code"], "unsupported_location", "{inside}");
    let outside = base.join("로컬 폴더");
    std::fs::create_dir(&outside).unwrap();
    let started = monitor_call(&mut mcp, "gil_start", json!({"project_root": outside}));
    assert_eq!(started["structuredContent"]["ok"], true, "{started}");
    assert_eq!(started["structuredContent"]["record_storage"], "private_app_data");
    mcp.eof();
}

/// A successful Windows start declares that records are in private app data.
#[cfg(all(windows, feature = "visual-authoring"))]
#[test]
fn successful_start_reports_private_app_data_storage() {
    use serde_json::json;
    let base = scratch("ws-check");
    let folder = base.join("정상");
    std::fs::create_dir(&folder).unwrap();
    let mut mcp = Client::serve_with_env(&base.join("settings"), &[("USERPROFILE", &base)]); mcp.initialize();
    let started = monitor_call(&mut mcp, "gil_start", json!({"project_root": folder}));
    assert_eq!(started["structuredContent"]["ok"], true, "{started}");
    assert_eq!(started["structuredContent"]["record_storage"], "private_app_data");
    assert!(started["structuredContent"].get("location_check").is_none());
    assert!(!folder.join(".gil").exists());
    mcp.eof();
}

/// Windows contract §6 W12-9: after a successful start, a reopen that no longer finds `.gil`
/// protected refuses with a stable code, preserves everything and tells the Agent not to ask for
/// permission changes. The loss is simulated on the test's own folder by re-enabling inheritance.
#[cfg(all(windows, feature = "visual-authoring"))]
#[test]
fn reopen_after_protection_loss_refuses_with_code_and_changes_nothing() {
    use serde_json::json;
    let base = scratch("ws-protection");
    let folder = base.join("프로젝트");
    let local = base.join("local-app-data");
    std::fs::create_dir(&folder).unwrap();
    let mut mcp = Client::serve_with_env(&base.join("settings"), &[("USERPROFILE", &base), ("LOCALAPPDATA", &local)]); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp, "gil_start", json!({"project_root": folder}))["structuredContent"]["ok"], true);
    assert_eq!(monitor_call(&mut mcp, "gil_status", json!({"project_root": folder}))["structuredContent"]["ok"], true, "reopen works while protected");
    let layout = local.join("GIL/project-state-v1");
    let record_root = std::fs::read_dir(&layout).unwrap().next().unwrap().unwrap().path();
    let gil = record_root.join(".gil");
    let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
    let snapshot = |label: &str| -> (Vec<(PathBuf, u64)>, String) {
        let mut files: Vec<(PathBuf, u64)> = walkdir(&gil);
        files.sort();
        let acl = String::from_utf8_lossy(&Command::new(&icacls).arg(&gil).output().unwrap().stdout).to_string();
        eprintln!("{label}: {} entries", files.len());
        (files, acl)
    };
    fn walkdir(dir: &Path) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            if meta.is_dir() { out.extend(walkdir(&entry.path())); } else { out.push((entry.path(), meta.len())); }
        }
        out
    }
    // Simulate the field condition on this test's own .gil: inheritance enabled again.
    let enabled = Command::new(&icacls).arg(&gil).args(["/inheritance:e"]).output().unwrap();
    assert!(enabled.status.success(), "fixture: {}", String::from_utf8_lossy(&enabled.stderr));
    let (files_before, acl_before) = snapshot("before");
    for tool in ["gil_status", "gil_context", "gil_story"] {
        let refused = monitor_call(&mut mcp, tool, json!({"project_root": folder}));
        assert_eq!(refused["structuredContent"]["ok"], false, "{tool}: {refused}");
        assert_eq!(refused["structuredContent"]["code"], "storage_protection_lost", "{tool}: {refused}");
        assert_eq!(refused["structuredContent"]["records_preserved"], true);
        assert!(refused["structuredContent"]["preservation_scope"].as_str().unwrap().contains("project.lock"), "{refused}");
        let problem = refused["structuredContent"]["problem"].as_str().unwrap();
        assert!(problem.contains("권한·상속·소유권·보안 설정 변경을 요청하지 않는다"), "{tool}: {problem}");
        assert!(problem.contains("동기화 폴더라고 단정하지 않고"), "{tool}: {problem}");
        assert!(refused["structuredContent"]["next"].as_str().unwrap().contains("Do not ask the user to change ACLs"), "{refused}");
    }
    // Nothing was repaired, removed or replaced. The lock file is compared by presence only:
    // the stated scope allows it to be opened or updated.
    let (files_after, acl_after) = snapshot("after");
    let without_lock = |files: &[(PathBuf, u64)]| files.iter().filter(|(p, _)| p.file_name() != Some(std::ffi::OsStr::new("project.lock"))).cloned().collect::<Vec<_>>();
    assert_eq!(without_lock(&files_after), without_lock(&files_before), "records must stay exactly as they were");
    assert!(files_after.iter().any(|(p, _)| p.file_name() == Some(std::ffi::OsStr::new("project.lock"))));
    assert_eq!(acl_after, acl_before, "GIL must not touch the ACL");
    // Starting again in the same place is refused too (existing records are never replaced).
    let again = monitor_call(&mut mcp, "gil_start", json!({"project_root": folder}));
    assert_eq!(again["structuredContent"]["ok"], false, "{again}");
    assert_eq!(walkdir(&gil).len(), files_before.len());
    mcp.eof();
}

/// Junction fixture (no privilege required); paths travel as environment values.
#[cfg(all(windows, feature = "visual-authoring"))]
fn junction(link: &Path, target: &Path) {
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let made = Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
        .env("GIL_TEST_LINK", link).env("GIL_TEST_TARGET", target).output().unwrap();
    assert!(made.status.success(), "junction fixture creation failed: {}", String::from_utf8_lossy(&made.stderr));
}

/// Deny directory creation for everyone (including administrators) while held.
#[cfg(all(windows, feature = "visual-authoring"))]
struct DenyCreate(PathBuf);
#[cfg(all(windows, feature = "visual-authoring"))]
impl DenyCreate {
    fn new(dir: &Path) -> Self {
        let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
        let out = Command::new(icacls).arg(dir).args(["/deny", "*S-1-1-0:(AD,WD)"]).output().unwrap();
        assert!(out.status.success(), "icacls deny failed");
        Self(dir.to_owned())
    }
}
#[cfg(all(windows, feature = "visual-authoring"))]
impl Drop for DenyCreate {
    fn drop(&mut self) {
        let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
        let _ = Command::new(icacls).arg(&self.0).args(["/remove:d", "*S-1-1-0"]).output();
    }
}

/// OneDrive environment declarations no longer decide whether a working folder can be used.
#[cfg(all(windows, feature = "visual-authoring"))]
#[test]
fn onedrive_environment_values_do_not_move_the_working_folder() {
    use serde_json::json;
    let base = scratch("ws-detection");
    let folder = base.join("synced-but-unnamed");
    std::fs::create_dir(&folder).unwrap();
    // A stale declaration and a matching declaration both leave the working folder in place.
    let mut mcp = Client::serve_with_env(&base.join("s1"), &[("OneDrive", Path::new("C:\\nonexistent-gil-onedrive-root"))]); mcp.initialize();
    assert_eq!(monitor_call(&mut mcp, "gil_start", json!({"project_root": folder}))["structuredContent"]["ok"], true);
    mcp.eof();
    let stale = base.join("stale-root");
    std::fs::create_dir_all(stale.join("p")).unwrap();
    let mut mcp = Client::serve_with_env(&base.join("s2"), &[("OneDriveConsumer", &stale)]); mcp.initialize();
    let got = monitor_call(&mut mcp, "gil_start", json!({"project_root": stale.join("p")}));
    assert_eq!(got["structuredContent"]["ok"], true, "{got}");
    assert_eq!(got["structuredContent"]["record_storage"], "private_app_data");
    assert!(!stale.join("p").join(".gil").exists());
    mcp.eof();
}
