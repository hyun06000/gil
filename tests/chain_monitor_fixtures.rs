//! Real Core -> Monitor wire -> shared UI fixtures. Synthetic inputs, never human consent.
//! Generate/check via tools/chain-monitor-fixtures/run.mjs under the shared Cargo queue.
#[path = "support_wave1/mod.rs"]
mod wire;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use wire::*;

fn rules() -> gil::RuleSet {
    gil::RuleSet::from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/gil-spec.yaml"))
        .unwrap()
}
fn fixture(name: &str, value: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ui/fixtures/chain-core")
        .join(format!("{name}.json"));
    let bytes = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var("GIL_CHAIN_MONITOR_WRITE").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
    } else {
        assert_eq!(
            fs::read_to_string(&path).expect("generate dedicated fixtures first"),
            bytes,
            "Core fixture drift: {name}"
        );
    }
}
// Compare file bytes, not mtimes. No state edits. Includes objects and source artifacts.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(base, &entry.path(), out);
            } else if entry.file_type().unwrap().is_file() {
                out.insert(
                    entry.path().strip_prefix(base).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
struct Rig {
    base: PathBuf,
    root: PathBuf,
    settings: PathBuf,
    record: PathBuf,
    scope: String,
    c: Option<Client>,
}
impl Drop for Rig {
    fn drop(&mut self) {
        drop(self.c.take());
        let _ = fs::remove_dir_all(&self.base);
    }
}
impl Rig {
    fn new(name: &str) -> Self {
        let base = scratch(&format!("chain-monitor-{name}"));
        let root = base.join("synthetic-project");
        fs::create_dir(&root).unwrap();
        let settings = base.join("settings");
        let record = product_state_path(&root, &private_local_app_data(&settings));
        let c = Some(Client::serve(&settings));
        let mut r = Self {
            base,
            root,
            settings,
            record,
            scope: String::new(),
            c,
        };
        let start = r.model("gil_start", json!({"locale":"ko"}));
        r.scope = start["next_call"]["arguments"]["scope_id"]
            .as_str()
            .unwrap()
            .into();
        r
    }
    fn model(&mut self, tool: &str, mut args: Value) -> Value {
        args["project_root"] = json!(self.root);
        structured(&ok(self.c.as_mut().unwrap().call(tool, args), tool)).clone()
    }
    fn app(&mut self, tool: &str, mut args: Value) -> Value {
        args["scope_id"] = json!(self.scope);
        structured(&ok(self.c.as_mut().unwrap().call(tool, args), tool)).clone()
    }
    fn open(&mut self, kind: &str) -> String {
        let args = json!({"project_root":self.root,"kind":kind,"contract":CONTRACT});
        let response = ok(self.c.as_mut().unwrap().call("gil_open", args), kind);
        step_ref(&response, kind)
    }
    fn close(&mut self, report: &str) {
        self.model("gil_close", json!({"report":report}));
    }
    fn restart(&mut self) {
        let (exit, noise) = self.c.take().unwrap().eof();
        assert!(exit.success());
        assert!(noise.is_empty());
        self.c = Some(Client::serve(&self.settings));
        let prepared = self.model("gil_monitor_prepare", json!({}));
        assert_eq!(prepared["scope_id"], self.scope);
    }
    fn capture(&mut self) -> Value {
        let before = record_bytes(&self.record);
        let artifacts = files(&self.root);
        let envelope = self.app("gil_monitor_read", json!({}));
        let mut view = envelope["view"].clone();
        assert!(view["captured_at_unix_ms"].is_number());
        view["captured_at_unix_ms"] = json!(0); // only normalization inside MonitorViewV1
        gil::decode_view_v1(&view.to_string()).unwrap();
        let mut details = BTreeMap::new();
        for cycle in view["timeline"].as_array().unwrap() {
            for step in cycle["steps"].as_array().unwrap() {
                let reference = step["step_ref"].as_str().unwrap();
                let reply = self.app("gil_monitor_detail", json!({"step_ref":reference}));
                gil::decode_detail_v1(&reply["detail"].to_string()).unwrap();
                details.insert(reference.to_owned(), reply["detail"].clone());
            }
        }
        #[allow(unused_mut)] // only the approved-plan profile adds the plan response
        let mut out = json!({"view":view,"details":details});
        #[cfg(feature = "approved-plan")]
        {
            out["plan"] = self.model("gil_plan_read", json!({}));
        }
        // Independent loaded domain oracle: exact ownership, state and raw report fields.
        let p = gil::load(rules(), &self.record).unwrap();
        if let Some(current) = p.current_chain() {
            assert_eq!(
                out["view"]["current"]["chain_ref"],
                current.id().to_string()
            );
            assert_eq!(
                out["view"]["chains"].as_array().unwrap().len(),
                p.chains().len()
            );
            for (chain, actual) in p
                .chains()
                .iter()
                .zip(out["view"]["chains"].as_array().unwrap())
            {
                assert_eq!(actual["chain_ref"], chain.id().to_string());
                assert_eq!(
                    actual["state"],
                    if chain.is_closed() { "closed" } else { "open" }
                );
                assert_eq!(
                    actual["cycle_refs"],
                    json!(
                        chain
                            .cycles()
                            .nodes()
                            .iter()
                            .map(|c| c.id().to_ref().to_string())
                            .collect::<Vec<_>>()
                    )
                );
                if let Some(report) = chain.report() {
                    let raw: BTreeMap<_, _> = report
                        .field_names()
                        .map(|k| (k, report.get(k).unwrap()))
                        .collect();
                    assert_eq!(actual["report"], json!(raw));
                } else {
                    assert!(actual.get("report").is_none() || actual["report"].is_null());
                }
            }
        } else {
            assert!(out["view"].get("chain").is_none());
        }
        assert_eq!(
            record_bytes(&self.record),
            before,
            "reads must preserve state bytes"
        );
        assert_eq!(
            files(&self.root),
            artifacts,
            "reads must preserve every project file/object"
        );
        let serialized = out.to_string();
        for forbidden in [
            self.base.to_string_lossy().as_ref(),
            "gil/planApprovalToken",
            "approval_token",
        ] {
            assert!(!serialized.contains(forbidden), "private data in fixture");
        }
        out
    }
    fn checkpoint(&mut self, name: &str) -> Value {
        let first = self.capture();
        self.restart();
        assert_eq!(
            self.capture(),
            first,
            "process restart changed canonical responses"
        );
        fixture(name, &first);
        first
    }
}

#[cfg(not(feature = "chain-core"))]
#[test]
fn legacy_fixture_from_default_core() {
    let mut r = Rig::new("legacy");
    r.open("question");
    let packet = r.checkpoint("legacy");
    assert!(packet["view"].get("chain").is_none());
    assert!(packet["view"].get("chains").is_none());
    assert!(packet["view"]["current"].get("chain_ref").is_none());
    assert_eq!(packet["view"]["timeline"][0]["kind"], "interview");
}

#[cfg(all(
    feature = "approved-plan",
    feature = "visual-authoring",
    feature = "product-safety"
))]
mod product {
    use super::*;
    use base64::Engine;
    impl Rig {
        fn interview(&mut self, ids: &[&str], closing: Option<&str>) -> String {
            let mut basis = vec![];
            for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
                basis.push(self.open(kind));
                self.close(report);
            }
            let synthesis = self.open("synthesis");
            let extra: Vec<_> = closing
                .map(|v| vec![("chain_verdict", v)])
                .unwrap_or_default();
            self.model(
                "gil_plan_propose",
                json!({"draft":draft(ids, &basis, &extra)}),
            );
            let review = ok(
                self.c
                    .as_mut()
                    .unwrap()
                    .call("gil_plan_review", json!({"scope_id":self.scope})),
                "review",
            );
            let proposal = &structured(&review)["proposal"];
            self.app("gil_plan_decide", json!({"revision":proposal["revision"],"digest":proposal["digest"],"token":token(&review).unwrap(),"decision":"approve"}));
            // This is synthetic App input. Keep the Core receipt as returned, never claim a real click.
            let read = self.model("gil_plan_read", json!({}));
            self.close(read["close_report"].as_str().unwrap());
            let outcome = self.open("outcome");
            self.close(&format!("verdict: success\nlesson: Synthetic scope agreed\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: Continue synthetic scenario"));
            let action = if closing.is_some() {
                "close_chain"
            } else {
                "open_child"
            };
            self.close(&format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: Synthetic App approval, not human consent\nnext_direction.action: {action}\nnext_direction.reason: Continue synthetic scenario"));
            synthesis
        }
        fn experiment(&mut self) -> (String, Value, Vec<u8>) {
            self.model("gil_open", json!({"kind":"experiment"}));
            let read = self.model("gil_plan_read", json!({}));
            self.open("define");
            self.close("problem: 항목 A을 합성 자료로 확인한다\nsuccess_condition: evidence 파일에 항목 A 결과가 저장된다");
            self.open("hypothesis");
            self.close("hypothesis: A generated gray pixel can be preserved\nrationale: Deterministic synthetic input\nguardrail: No user files");
            let verify = self.open("verify");
            fs::write(
                self.root.join("plan.md"),
                read["document"].as_str().unwrap(),
            )
            .unwrap();
            let mut png = Vec::new();
            png::Encoder::new(&mut png, 1, 1)
                .write_header()
                .unwrap()
                .write_image_data(&[127])
                .unwrap();
            fs::write(self.root.join("evidence.png"), &png).unwrap();
            let closed = self.model("gil_close", json!({
                "report":"execution: Generated one synthetic gray pixel\nresult: PNG bytes saved\nplan_document: plan.md\nevidence_files: evidence.png",
                "expected_owner":{"kind":"step","ref":verify},
                "visuals":{"attachments":[{"key":"pixel","path":"evidence.png","source_kind":"generated","source_description":"Synthetic pixel, not a screenshot"}],
                "uses":[{"source":{"kind":"new","key":"pixel"},"caption":"Synthetic evidence","alt":"One gray pixel","purpose":"Check stored evidence retrieval"}],"declaration":{"state":"attached"}}
            }));
            let asset = closed["visuals"]["uses"][0].clone();
            assert!(asset["asset_id"].is_string(), "{closed}");
            let cite = json!({"attachments":[],"uses":[{"source":{"kind":"stored","owner":{"kind":"step","ref":verify},"asset_id":asset["asset_id"],"sha256":asset["sha256"]},"caption":"Synthetic evidence","alt":"One gray pixel","purpose":"Check stored evidence retrieval"}]});
            let analysis = self.open("analysis");
            self.model("gil_close", json!({"report":"hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: Synthetic pixel was preserved", "expected_owner":{"kind":"step","ref":analysis},"visuals":cite}));
            let outcome = self.open("outcome");
            self.close("verdict: success\nlesson: Synthetic image saved\nnext_direction.action: close_cycle\nnext_direction.reason: Experiment complete");
            self.model("gil_close", json!({"report":format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: Synthetic pixel verified\nnext_direction.action: open_child\nnext_direction.reason: Ask about Chain closure"),"expected_owner":{"kind":"cycle","ref":"cycle:C2"},"visuals":cite}));
            (verify, asset, png)
        }
        fn evidence(&mut self, verify: &str, asset: &Value, expected: &[u8]) -> Value {
            let before = record_bytes(&self.record);
            let artifacts = files(&self.root);
            let owner = json!({"kind":"step","ref":verify});
            let mut visuals = self.app("gil_monitor_report_visuals", json!({"owner":owner}));
            let mut image = self.app(
                "gil_monitor_report_image",
                json!({"owner":owner,"asset_id":asset["asset_id"],"offset":0}),
            );
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(image["data_base64"].as_str().unwrap())
                    .unwrap(),
                expected
            );
            for response in [&mut visuals, &mut image] {
                assert_eq!(response["scope_id"], self.scope);
                response["scope_id"] = json!(format!("project:{}", "0".repeat(64))); // transport binding only
            }
            assert_eq!(record_bytes(&self.record), before);
            assert_eq!(
                files(&self.root),
                artifacts,
                "evidence reads preserve files/objects"
            );
            json!({"visuals":visuals,"image":image})
        }
    }
    #[test]
    fn approved_chains_match_real_monitor_responses() {
        for verdict in ["success", "failure"] {
            let mut r = Rig::new(verdict);
            r.open("question");
            r.checkpoint(&format!("{verdict}-first-interview"));
            // Continue the already-open Question through the public close API.
            r.close(QUESTION);
            let interpretation = r.open("interpretation");
            r.close(INTERPRETATION);
            let synthesis = r.open("synthesis");
            let basis = vec!["step:C1/S1".to_owned(), interpretation];
            r.model(
                "gil_plan_propose",
                json!({"draft":draft(&["A"], &basis, &[])}),
            );
            let review = ok(
                r.c.as_mut()
                    .unwrap()
                    .call("gil_plan_review", json!({"scope_id":r.scope})),
                "review",
            );
            let p = &structured(&review)["proposal"];
            r.app("gil_plan_decide", json!({"revision":p["revision"],"digest":p["digest"],"token":token(&review).unwrap(),"decision":"approve"}));
            let read = r.model("gil_plan_read", json!({}));
            r.close(read["close_report"].as_str().unwrap());
            let outcome = r.open("outcome");
            r.close(&format!("verdict: success\nlesson: Synthetic scope agreed\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: Begin experiment"));
            r.close(&format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: Synthetic App approval, not human consent\nnext_direction.action: open_child\nnext_direction.reason: Begin experiment"));
            let (verify, asset, png) = r.experiment();
            let done = r.checkpoint(&format!("{verdict}-experiment-closed"));
            assert_eq!(done["view"]["chain"]["state"], "open");
            assert_eq!(done["view"]["timeline"].as_array().unwrap().len(), 2);
            let original_evidence = r.evidence(&verify, &asset, &png);
            r.model("gil_open", json!({"kind":"interview"}));
            let closing = r.interview(&["A"], Some(verdict));
            r.checkpoint(&format!("{verdict}-closing-ready"));
            let handoff = format!(
                "Synthetic {verdict}: pixel experiment succeeded; Chain verdict is a separate approved conclusion.\nRemaining question: broader input coverage.\nLiteral text: <script>window.chainFixtureInjected=1</script>"
            );
            r.close(&format!(
                "verdict: {verdict}\nclosing_synthesis_ref: {closing}\nhandoff_summary: |\n{}",
                handoff
                    .lines()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
            let closed = r.checkpoint(&format!("{verdict}-chain-closed"));
            assert_eq!(
                closed["view"]["chain"]["report"]["handoff_summary"],
                handoff
            );
            r.model("gil_open", json!({"kind":"chain"}));
            let next = r.checkpoint(&format!("{verdict}-next-chain"));
            assert_eq!(next["plan"]["active_plan"], Value::Null);
            assert_eq!(next["plan"]["executed_cycles"], json!([]));
            assert_eq!(next["view"]["current"]["chain_ref"], "chain:C2");
            assert_eq!(next["view"]["current"]["cycle_ref"], "cycle:C4");
            assert_eq!(next["view"]["chains"][0], closed["view"]["chains"][0]);
            let before = record_bytes(&r.record);
            refused(
                r.c.as_mut().unwrap().call(
                    "gil_open",
                    json!({"project_root":r.root,"kind":"experiment"}),
                ),
                "old Chain approval cannot open new Experiment",
            );
            assert_eq!(record_bytes(&r.record), before);
            // Source deletion is a test artifact action, not a state edit. Reads must still use stored bytes.
            fs::remove_file(r.root.join("evidence.png")).unwrap();
            r.restart();
            assert_eq!(r.evidence(&verify, &asset, &png), original_evidence);
            fixture(
                &format!("{verdict}-historical-evidence"),
                &original_evidence,
            );
        }
    }
}

#[test]
fn all_committed_views_and_details_use_the_real_decoders() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/fixtures/chain-core");
    if !dir.exists() {
        assert_eq!(
            std::env::var("GIL_CHAIN_MONITOR_WRITE").as_deref(),
            Ok("1"),
            "missing committed fixtures"
        );
        return; // a first --write may run this before the producing test
    }
    let mut views = 0;
    for file in fs::read_dir(dir).unwrap() {
        let file = file.unwrap().path();
        if file.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let packet: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        if let Some(view) = packet.get("view") {
            gil::decode_view_v1(&view.to_string()).unwrap();
            views += 1;
            for detail in packet["details"].as_object().unwrap().values() {
                gil::decode_detail_v1(&detail.to_string()).unwrap();
            }
        }
    }
    if std::env::var("GIL_CHAIN_MONITOR_WRITE").as_deref() != Ok("1") {
        assert_eq!(views, 11);
    }
}
