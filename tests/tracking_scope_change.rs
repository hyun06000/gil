//! 구현 순서 5번 — 범위 변경 제안 → Monitor(App-only) 승인 → 승인 저장 → 기준점 보관 실패와 대기 → 명시 재시도 → 다음 Experiment,
//! 정상 종료와 후속 Chain, 즉시 보관 중단·해제를 **실제 `gil` stdio 프로세스**로 확인한다(`tracking-policy` 개발 구성). 합성 임시
//! 프로젝트만 쓴다.
//!
//! # 무엇이 실제 제품 프로세스의 요청이고 무엇이 아닌가
//!
//! - **모든 전이가 `gil mcp --serve` 프로세스의 도구 호출이다**: 시작 준비와 그 승인, Interview 의 단계 열기·닫기·사이클 닫기, 범위
//!   제안·검토·결정·재시도·중단·해제, Experiment·다음 Chain 열기, Chain Report. 시험 프로세스는 `ProjectSession` 을 열지 않고 Project 를
//!   직접 바꾸지 않는다.
//! - Interview 는 **이 빌드의 승인 계약**대로 걷는다. 승인 계획 구성(카탈로그에 `gil_plan_propose` 가 있다)에서는 계획 제안 → App 전용
//!   `gil_plan_review`·`gil_plan_decide` → `gil_plan_read.close_report`, 그 밖의 구성에서는 맨 `approved: yes` Report 다. 구성은 연결된
//!   도구 카탈로그에서 읽는다.
//! - 시험 프로세스가 직접 하는 일은 셋뿐이다: 합성 파일을 만든다, 저장된 파일을 **읽어** 견준다(`state.yaml` 바이트, `.gilignore`), App
//!   전용 capability 를 `_meta` 에서 꺼내 되보낸다. 마지막 것은 시험 하네스의 입력이며 사람의 동의나 보이는 화면의 증거가 아니다.
//! - 프로세스를 여러 번 새로 띄워 **앞 프로세스가 저장한 기록을 새 프로세스가 처음 읽는** 경우를 본다. 기준점 저장의 실패는 debug
//!   빌드의 failpoint(`GIL_SCOPE_FAIL`)로 넣은 것이며, 강제 종료·전원 장애를 일으킨 것이 아니다.
//! - 탐색 계약 구성(`exploration-flow`, 카탈로그에 `gil_report_read` 가 있다)에서도 같은 경로를 걷는다. 달라지는 것은 그 계약이 정한
//!   둘뿐이다: Experiment 는 활성 승인 판의 요구에 **명시 연결**해 열고(`Rig::experiment_args`), 답하지 못한 요구가 남은 Chain 은
//!   사용자가 승인한 `failure` 로만 닫는다(`Rig::closing_verdict`). 범위 변경·보관 대기·재시도·중단의 단정은 그대로다. 이 구성의
//!   단위 시험(`scope_change::tests`)은 여전히 컴파일되지 않으며, 그 0건은 실행 성공이 아니다.
#![cfg(all(feature = "tracking-policy", unix))]
#[path = "support_wave1/mod.rs"]
mod support;
mod common;
use gil::{CycleKind, NodeKind};
use serde_json::{Value, json};
use support::*;

const P1: &str = "scratch/\n";
const P2: &str = "scratch/\ndata/\n";

fn sc(frame: &Value) -> &Value {
    &frame["result"]["structuredContent"]
}

fn is_error(frame: &Value) -> bool {
    frame["result"]["isError"] == json!(true)
}

fn meta<'a>(frame: &'a Value, key: &str) -> Option<&'a str> {
    frame["result"]["_meta"][key].as_str()
}

/// Report 를 **제품의 Report 문법**(`gil::Report::parse` 가 읽는 꼴)으로 적는다. 한 줄 값은 `이름: 값`, 여러 줄 값(예: 한 줄에 하나씩
/// 적는 `basis_refs`)은 `이름: |` 아래 들여쓴 줄들이다. 적은 글을 제품 파서로 다시 읽어 **칸의 이름과 값이 원래 Report 와 같은지**
/// 여기서 확인한다 — 값이 조용히 잘리거나 다른 칸으로 읽힌 글을 보내지 않는다. 문법이나 승인 검사를 느슨하게 하는 것이 아니다.
fn report_text(report: &gil::Report) -> String {
    let mut text = String::new();
    for key in report.field_names() {
        let value = report.get(key).unwrap();
        if value.contains('\n') || value == "|" {
            text.push_str(&format!("{key}: |\n"));
            for line in value.split('\n') {
                text.push_str(&format!("  {line}\n"));
            }
        } else {
            text.push_str(&format!("{key}: {value}\n"));
        }
    }
    let read = gil::Report::parse(&text).unwrap_or_else(|e| panic!("제품 파서가 읽지 못한 Report: {e}\n{text}"));
    assert_eq!(read.field_names().collect::<Vec<_>>(), report.field_names().collect::<Vec<_>>(), "칸의 이름이 그대로다:\n{text}");
    for key in report.field_names() {
        assert_eq!(read.get(key), report.get(key), "칸 {key} 의 값이 그대로다:\n{text}");
    }
    text
}

/// 한 합성 프로젝트와 그것에 연결된 실제 `gil` 프로세스.
struct Rig {
    c: Client,
    settings: std::path::PathBuf,
    root: std::path::PathBuf,
    r: String,
    scope: String,
    /// 이 빌드가 승인 계획 구성인가 — 연결된 도구 카탈로그에서 읽은 값.
    plan: bool,
    /// 이 빌드가 탐색 계약 구성인가 — 같은 카탈로그에서 읽은 값(원본 기록 조회 도구의 존재).
    explore: bool,
}

impl Rig {
    /// 실제 stdio 로 형식 10 을 시작한다(P1). 뿌리 Interview C1 이 열려 있다.
    fn start(name: &str) -> Rig {
        let settings = scratch(&format!("{name}-settings"));
        let root = scratch(name).canonicalize().unwrap();
        std::fs::write(root.join("README.md"), "hello\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.txt"), "alpha\n").unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/raw.csv"), "1,2,3\n").unwrap();
        let r = root.display().to_string();
        let mut c = Client::serve(&settings);
        let listed = c.request("tools/list", json!({}));
        let plan = listed["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "gil_plan_propose");
        let explore = listed["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "gil_report_read");
        let begun = c.call("gil_start", json!({"project_root": r, "start_kind": "tracked_scope"}));
        assert!(!is_error(&begun), "{begun}");
        let scope = sc(&begun)["scope_id"].as_str().unwrap().to_owned();
        let prepared = c.call("gil_start_prepare", json!({"project_root": r, "rules": P1}));
        let (revision, digest) = (sc(&prepared)["proposal"]["revision"].clone(), sc(&prepared)["proposal"]["digest"].clone());
        let review = c.call("gil_start_review", json!({"scope_id": scope}));
        let token = meta(&review, "gil/startApprovalToken").expect("시작 승인 capability").to_owned();
        let decided = c.call("gil_start_decide", json!({"scope_id": scope, "revision": revision, "digest": digest, "token": token, "decision": "approve"}));
        assert_eq!(sc(&decided)["phase"], "started", "{decided}");
        Rig { c, settings, root, r, scope, plan, explore }
    }

    /// `gil_open(kind: experiment)` 의 인자. 탐색 계약 구성에서는 지금 활성인 승인 판의 요구 A 에 연결한다 — 연결 없는 열기는 그
    /// 계약의 거절이라, 그대로 보내면 「보관 대기 때문에 거절된다」를 확인하지 못한다. 조회는 기록을 바꾸지 않는다.
    fn experiment_args(&mut self) -> Value {
        if !self.explore {
            return json!({"kind": "experiment"});
        }
        let read = ok(self.model("gil_plan_read", json!({})), "plan read");
        let active = structured(&read)["active_plan"].clone();
        let digest = active["digest"].as_str().expect("활성 승인 판의 digest").to_owned();
        let link = format!(
            "plan_revision: {}\nplan_digest: {digest}\nrequirement_id: A\nrelevance: 요구 A의 답에 필요한 관측이다\nreadiness: 입력 파일이 프로젝트에 있다\nuncertainty: 아직 판정하지 못한 한 가지를 줄인다\n",
            active["revision"]
        );
        json!({"kind": "experiment", "contract": link})
    }

    /// 이 시험이 Chain 을 닫을 때 쓰는 판정. 탐색 계약에서는 어느 요구도 답하지 않은 Chain 을 성공으로 닫을 수 없다.
    fn closing_verdict(&self) -> &'static str {
        if self.explore { "failure" } else { "success" }
    }

    /// 프로세스를 끝내고 새로 띄운다 — 다음 호출은 저장된 기록을 처음 읽는다.
    fn restart(&mut self) {
        self.restart_with(&[]);
    }

    fn restart_with(&mut self, env: &[(&str, &str)]) {
        let old = std::mem::replace(&mut self.c, Client::serve_with_env(&self.settings, env));
        drop(old);
    }

    fn model(&mut self, name: &str, mut args: Value) -> Value {
        args["project_root"] = json!(self.r);
        self.c.call(name, args)
    }

    fn app(&mut self, name: &str, mut args: Value) -> Value {
        args["scope_id"] = json!(self.scope);
        self.c.call(name, args)
    }

    fn state(&self) -> Vec<u8> {
        std::fs::read(self.root.join(".gil/state.yaml")).unwrap()
    }

    fn status(&mut self) -> Value {
        let got = self.model("gil_scope_status", json!({}));
        assert!(!is_error(&got), "{got}");
        sc(&got).clone()
    }

    /// 지금 열린 Interview 를 Question → Interpretation → Synthesis 까지 걷고 Synthesis 를 **열어 둔다.** 돌려주는 것은 (Synthesis 의
    /// 참조, 근거 참조들).
    fn to_synthesis(&mut self) -> (String, Vec<String>) {
        let mut basis = Vec::new();
        for (kind, report) in [("question", QUESTION), ("interpretation", INTERPRETATION)] {
            let opened = ok(self.model("gil_open", json!({"kind": kind, "contract": CONTRACT})), kind);
            basis.push(step_ref(&opened, kind));
            ok(self.model("gil_close", json!({"report": report})), kind);
        }
        let opened = ok(self.model("gil_open", json!({"kind": "synthesis", "contract": CONTRACT})), "synthesis");
        (step_ref(&opened, "synthesis"), basis)
    }

    /// 열린 Synthesis 를 이 빌드의 계약대로 승인해 닫는다. `closing` 이 있으면 종료 제안(`chain_verdict`)이다.
    fn approve_synthesis(&mut self, basis: &[String], closing: Option<&str>) -> Value {
        if self.plan {
            let extra: Vec<(&str, &str)> = closing.iter().map(|verdict| ("chain_verdict", *verdict)).collect();
            ok(self.model("gil_plan_propose", json!({"draft": draft(&["A", "B", "C"], basis, &extra)})), "plan proposal");
            let review = ok(self.app("gil_plan_review", json!({})), "plan review");
            let capability = token(&review).expect("계획 승인 capability");
            let proposal = structured(&review)["proposal"].clone();
            ok(self.app("gil_plan_decide", json!({"revision": proposal["revision"], "digest": proposal["digest"], "token": capability, "decision": "approve"})), "plan decision");
            let read = ok(self.model("gil_plan_read", json!({})), "plan read");
            let report = structured(&read)["close_report"].clone();
            self.model("gil_close", json!({"report": report}))
        } else {
            let mut report = common::full_report(&common::spec(), CycleKind::Interview, NodeKind::Synthesis)
                .with(common::APPROVED, common::APPROVAL)
                .with(common::BASIS_REFS, basis.join("\n"));
            if let Some(verdict) = closing {
                report.insert("chain_verdict", verdict);
            }
            self.model("gil_close", json!({"report": report_text(&report)}))
        }
    }

    /// 승인된 Synthesis 뒤의 Outcome 과 Interview 닫기. `action` 은 `open_child` 또는 `close_chain`.
    fn finish_interview(&mut self, synthesis: &str, action: &str) {
        let outcome = ok(self.model("gil_open", json!({"kind": "outcome", "contract": CONTRACT})), "open outcome");
        let outcome = step_ref(&outcome, "outcome");
        ok(self.model("gil_close", json!({"report": format!("verdict: success\nlesson: 합의한 범위를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 합의를 얻었다\n")})), "close outcome");
        ok(self.model("gil_close", json!({"report": format!("verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 합성 범위를 합의했다\nnext_direction.action: {action}\nnext_direction.reason: 다음 단계로 간다\n")})), "close interview");
    }

    /// 지금 열린 Interview 를 일반 Interview 로 끝까지 걸어 닫는다.
    fn interview(&mut self) {
        let (synthesis, basis) = self.to_synthesis();
        ok(self.approve_synthesis(&basis, None), "close synthesis");
        self.finish_interview(&synthesis, "open_child");
    }

    /// 닫힌 검토 Interview 의 경계에서 제안하고 App 승인을 보낸다. 돌려주는 것은 결정의 응답.
    fn propose_and_approve(&mut self, rules: &str) -> Value {
        let proposed = self.model("gil_scope_propose", json!({"rules": rules}));
        assert!(!is_error(&proposed), "{proposed}");
        assert_eq!(sc(&proposed)["phase"], "awaiting_approval", "{proposed}");
        let (revision, digest) = (sc(&proposed)["proposal"]["revision"].clone(), sc(&proposed)["proposal"]["digest"].clone());
        let review = self.app("gil_scope_review", json!({}));
        let capability = meta(&review, "gil/scopeApprovalToken").expect("App 전용 승인 capability").to_owned();
        let decided = self.app("gil_scope_decide", json!({"revision": revision, "digest": digest, "token": capability, "decision": "approve"}));
        assert!(!is_error(&decided), "{decided}");
        decided
    }
}

/// 전체 흐름: 검토 Interview → 제안 → App 승인 → 저장 1 → 기준점 저장 실패 → 대기 → (새 프로세스) 조회 → 대기 중 일반 Interview →
/// (새 프로세스) 그 경계에서 명시 재시도 → 최신 확정 세계에서 다음 Experiment → (새 프로세스) reload.
#[test]
fn an_approved_scope_change_waits_after_a_failed_archive_and_a_fresh_process_retries_it_explicitly_before_the_next_experiment() {
    let mut rig = Rig::start("scope-stdio");
    rig.interview();

    // ── 프로세스 2: 기준점 저장이 실패하는 구성.
    rig.restart_with(&[("GIL_SCOPE_FAIL", "baseline")]);
    let listed = rig.c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let visibility = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name}"))["_meta"]["ui"]["visibility"].clone();
    for app in ["gil_scope_review", "gil_scope_decide", "gil_scope_retry", "gil_scope_halt", "gil_scope_release"] {
        assert_eq!(visibility(app), json!(["app"]), "{app}");
    }
    assert_eq!(visibility("gil_scope_propose"), json!(["model"]));
    assert_eq!(visibility("gil_scope_status"), json!(["model"]));

    let before = rig.state();
    let proposed = rig.model("gil_scope_propose", json!({"rules": P2}));
    assert!(!is_error(&proposed), "{proposed}");
    assert_eq!(sc(&proposed)["proposal"]["newly_excluded"], json!(["data/raw.csv"]));
    assert_eq!(sc(&proposed)["scope_id"], json!(rig.scope));
    assert_eq!(rig.state(), before, "제안은 상태를 바꾸지 않는다");
    let (revision, digest) = (sc(&proposed)["proposal"]["revision"].clone(), sc(&proposed)["proposal"]["digest"].as_str().unwrap().to_owned());

    // 모델 입력으로는 승인할 수 없고, 추측한 capability 는 거절된다. 어느 쪽도 기록을 바꾸지 않는다.
    let model = rig.model("gil_scope_decide", json!({"decision": "approve"}));
    assert!(model.get("error").is_some() || is_error(&model), "{model}");
    let forged = rig.app("gil_scope_decide", json!({"revision": revision, "digest": digest, "token": "f".repeat(64), "decision": "approve"}));
    assert!(is_error(&forged), "{forged}");
    assert_eq!(rig.state(), before);

    let review = rig.app("gil_scope_review", json!({}));
    let capability = meta(&review, "gil/scopeApprovalToken").expect("App 전용 승인 capability").to_owned();
    assert!(meta(&review, "gil/scopeRetryToken").is_none(), "대기가 아니면 재시도 capability 가 없다");
    assert!(!review["result"]["content"].to_string().contains(&capability) && !sc(&review).to_string().contains(&capability));
    let decided = rig.app("gil_scope_decide", json!({"revision": revision, "digest": digest, "token": capability, "decision": "approve"}));
    assert!(!is_error(&decided), "{decided}");
    assert_eq!(sc(&decided)["facts"]["approval_response"], "recorded");
    assert_eq!(sc(&decided)["facts"]["policy_applied"], true, "{decided}");
    assert_eq!(sc(&decided)["facts"]["policy_file"], "written");
    assert_eq!(sc(&decided)["facts"]["baseline"]["result"], "failed", "{decided}");
    assert_eq!(std::fs::read_to_string(rig.root.join(".gilignore")).unwrap(), P2);
    assert!(!String::from_utf8_lossy(&rig.state()).contains(&capability), "capability 는 기록에 없다");
    // 같은 capability 의 재전송은 거절되고, 상태는 그대로다.
    let waiting_state = rig.state();
    let resent = rig.app("gil_scope_decide", json!({"revision": revision, "digest": digest, "token": capability, "decision": "approve"}));
    assert!(is_error(&resent));
    assert_eq!(rig.state(), waiting_state);

    // ── 프로세스 3(failpoint 없음): 저장된 대기 기록을 처음 읽는다. 조회·재연결은 시도를 만들지 않는다.
    rig.restart();
    let status = rig.status();
    assert_eq!(status["tracking"]["applied_policy"], "policy:P2", "{status}");
    assert_eq!(status["tracking"]["last_confirmed_world"], "snapshot:A1");
    assert_eq!(status["tracking"]["confirmed_policy"], "policy:P1");
    assert_eq!(status["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(status["tracking"]["waiting"]["failed_attempts"], json!(["store_failure"]));
    let context = rig.model("gil_context", json!({}));
    assert!(!is_error(&context), "{context}");
    assert!(context.to_string().contains("event:M2"), "context 가 보관 대기를 말한다");
    let shown = rig.app("show_gil_monitor", json!({}));
    assert_eq!(sc(&shown)["scope_review_supported"], true, "{shown}");
    assert_eq!(sc(&shown)["tracking"]["now"]["applied_policy"], "policy:P2");
    assert_eq!(sc(&shown)["tracking"]["now"]["confirmed_policy"], "policy:P1");
    assert_eq!(rig.state(), waiting_state, "조회는 아무것도 바꾸지 않는다");

    // 대기 중: Experiment 는 거절되고 Interview 는 열린다.
    let pending_experiment = rig.experiment_args();
    refused(rig.model("gil_open", pending_experiment), "experiment while a baseline is pending");
    assert_eq!(rig.state(), waiting_state, "거절은 아무것도 바꾸지 않는다");
    ok(rig.model("gil_open", json!({"kind": "interview"})), "interview while waiting");
    // 열린 사이클에서는 재시도 capability 가 없다.
    let inside = rig.app("gil_scope_review", json!({}));
    assert!(meta(&inside, "gil/scopeRetryToken").is_none(), "닫힌 경계가 아니다: {inside}");
    let (synthesis, basis) = rig.to_synthesis();
    // 승인 계획이 없는 구성: 종료 Synthesis 를 승인으로 닫는 요청은 거절되고 그 Synthesis 는 열린 그대로다(O-3). 승인 계획 구성의 같은
    // 거절은 계획 승인 상태를 남기므로 여기서 실제 프로세스로 만들지 않는다 — 그 구성의 근거는 `scope_change::tests` 의 단위 시험이다.
    if !rig.plan {
        let open_state = rig.state();
        refused(rig.approve_synthesis(&basis, Some("success")), "closing approval while a baseline is pending");
        assert_eq!(rig.state(), open_state);
    }
    // 같은 Interview 는 일반 Interview 로 완결된다.
    ok(rig.approve_synthesis(&basis, None), "ordinary synthesis while waiting");
    rig.finish_interview(&synthesis, "open_child");

    // ── 프로세스 4: 지금 서 있는 경계 C2 에서 명시 재시도.
    rig.restart();
    let review = rig.app("gil_scope_review", json!({}));
    let retry = meta(&review, "gil/scopeRetryToken").expect("대기 중 닫힌 경계의 재시도 capability").to_owned();
    assert!(meta(&review, "gil/scopeApprovalToken").is_none(), "이미 적용된 제안에는 승인 capability 가 없다");
    let other = rig.app("gil_scope_retry", json!({"event": "event:M9", "token": retry}));
    assert!(is_error(&other), "다른 사건에는 쓰지 못한다: {other}");
    let retried = rig.app("gil_scope_retry", json!({"event": "event:M2", "token": retry}));
    assert!(!is_error(&retried), "{retried}");
    assert_eq!(sc(&retried)["facts"]["baseline"]["result"], "archived", "{retried}");
    assert_eq!(sc(&retried)["facts"]["baseline"]["snapshot"], "snapshot:A2");
    assert_eq!(sc(&retried)["facts"]["baseline"]["archived_at"], "cycle:C2");
    assert_eq!(sc(&retried)["current_state"]["reread"], true);
    let after = rig.app("gil_scope_review", json!({}));
    assert!(meta(&after, "gil/scopeRetryToken").is_none(), "확정된 기준점은 다시 확정하지 않는다");

    // 다음 Experiment — 최신 확정 세계(기준점 A2)에서 열린다.
    let next_experiment = rig.experiment_args();
    ok(rig.model("gil_open", next_experiment), "experiment after the baseline");

    // ── 프로세스 5: 그 기록을 새 프로세스가 읽는다(load 의 검사 전체).
    rig.restart();
    let status = rig.status();
    assert_eq!(status["tracking"]["standing"]["state"], "open_experiment", "{status}");
    assert_eq!(status["tracking"]["standing"]["cycle"], "cycle:C3");
    assert_eq!(status["tracking"]["last_confirmed_world"], "snapshot:A2");
    assert_eq!(status["tracking"]["confirmed_policy"], "policy:P2");
    assert!(status["tracking"]["waiting"].is_null());
    assert!(!is_error(&rig.model("gil_context", json!({}))));
}

/// 기준점이 확정된 뒤의 정상 종료와 후속 Chain: 종료 Interview(Entry = 기준점) → 승인된 종료 Synthesis → `close_chain` → Chain Report →
/// 다음 Chain → (새 프로세스) reload. 종료 경계에서는 새 범위 제안을 받지 않는다.
#[test]
fn after_the_baseline_is_confirmed_the_chain_closes_and_the_next_chain_opens_over_the_real_process() {
    let mut rig = Rig::start("scope-closing");
    rig.interview();
    let decided = rig.propose_and_approve(P2);
    assert_eq!(sc(&decided)["facts"]["baseline"]["result"], "archived", "{decided}");
    assert_eq!(sc(&decided)["facts"]["baseline"]["archived_at"], "cycle:C1");

    ok(rig.model("gil_open", json!({"kind": "interview"})), "closing interview");
    let (synthesis, basis) = rig.to_synthesis();
    let verdict = rig.closing_verdict();
    ok(rig.approve_synthesis(&basis, Some(verdict)), "closing approval after the baseline");
    rig.finish_interview(&synthesis, "close_chain");
    // 종료 경계: 새 승인을 저장하지 않는다(§6.6).
    refused(rig.model("gil_scope_propose", json!({"rules": P1})), "proposal at the closing boundary");
    ok(rig.model("gil_close", json!({"report": format!("verdict: {verdict}\nclosing_synthesis_ref: {synthesis}\nhandoff_summary: 합성 인계 — 시험 자료\n")})), "chain report");
    ok(rig.model("gil_open", json!({"kind": "chain"})), "next chain");

    rig.restart();
    let status = rig.status();
    assert_eq!(status["tracking"]["chain"], "chain:C2", "{status}");
    assert_eq!(status["tracking"]["applied_policy"], "policy:P2");
    // 후속 Chain 안에서는 확정 정책을 유도하지 않는다 — 적용 정책과 같다고 채우지 않는다.
    assert!(status["tracking"]["confirmed_policy"].is_null(), "{status}");
    assert!(!is_error(&rig.model("gil_context", json!({}))));
    refused(rig.model("gil_scope_propose", json!({"rules": P1})), "scope change in a later Chain");
}

/// 즉시 보관 중단: 요청(수신·적용), 진행 거절, 재시작 뒤 유지, 화면이 보여 준 그 중단 건·그 상태에 묶인 해제.
#[test]
fn a_halt_survives_a_restart_refuses_progress_and_is_released_only_with_the_explicit_confirmation() {
    let mut rig = Rig::start("scope-halt");
    rig.interview();
    let state_path = rig.root.join(".gil/state.yaml");

    let review = rig.app("gil_scope_review", json!({}));
    let halt = meta(&review, "gil/scopeHaltToken").expect("중단 capability").to_owned();
    assert!(meta(&review, "gil/scopeReleaseToken").is_none());
    let halted = rig.app("gil_scope_halt", json!({"token": halt}));
    assert!(!is_error(&halted), "{halted}");
    assert_eq!(sc(&halted)["tracking"]["archive_halt"], "cycle:C1");
    // 수신과 적용을 따로 말한다 — 잠금을 바로 얻었으므로 둘 다 참이다.
    assert_eq!(sc(&halted)["halt_request"], json!({"received": true, "applied": true}));
    let frozen = std::fs::read(&state_path).unwrap();
    // 표식이 있는 동안 제안·열기가 거절된다.
    refused(rig.model("gil_scope_propose", json!({"rules": P2})), "proposal while halted");
    refused(rig.model("gil_open", json!({"kind": "interview"})), "interview while halted");
    assert_eq!(std::fs::read(&state_path).unwrap(), frozen);

    // 재시작·재연결로 풀리지 않는다. 중단 중에는 해제 capability 만 있다.
    rig.restart();
    let status = rig.status();
    assert_eq!(status["tracking"]["archive_halt"], "cycle:C1", "{status}");
    let review = rig.app("gil_scope_review", json!({}));
    let release = meta(&review, "gil/scopeReleaseToken").expect("해제 capability").to_owned();
    assert!(meta(&review, "gil/scopeHaltToken").is_none() && meta(&review, "gil/scopeApprovalToken").is_none());
    // 화면이 보여 준 확인 근거: 그 중단 건의 이름과 그때 상태의 지문. 새 프로세스에서도 같은 건이다.
    let (halt_id, state_digest) = (sc(&review)["halt"]["id"].as_str().expect("중단 건의 이름").to_owned(),
        sc(&review)["halt"]["state_digest"].as_str().expect("상태의 지문").to_owned());
    assert_eq!(sc(&review)["halt"]["at"], "cycle:C1");
    let release_with = |halt_id: &str, state_digest: &str, state_confirmed: bool, token: &str| json!({"at": "cycle:C1",
        "halt_id": halt_id, "state_digest": state_digest, "cleanup": "continue", "state_confirmed": state_confirmed, "token": token});
    // 자리만 실은 꼴의 요청은 입력 검사에서 걸린다.
    let place_only = rig.app("gil_scope_release", json!({"at": "cycle:C1", "cleanup": "continue", "state_confirmed": true, "token": release}));
    assert!(place_only.get("error").is_some() || is_error(&place_only), "{place_only}");
    // 상태 확인 없는 해제는 받지 않는다 — capability 도 소모되지 않는다.
    let unconfirmed = rig.app("gil_scope_release", release_with(&halt_id, &state_digest, false, &release));
    assert!(is_error(&unconfirmed), "{unconfirmed}");
    // 다른 중단 건에 대한 확인으로는 그 capability 가 통하지 않는다.
    let other = rig.app("gil_scope_release", release_with(&"0".repeat(32), &state_digest, true, &release));
    assert!(is_error(&other), "{other}");
    assert_eq!(std::fs::read(&state_path).unwrap(), frozen);
    // 화면을 다시 읽으면 같은 건에 대한 capability 가 다시 온다 — 그것으로 해제한다.
    let again = rig.app("gil_scope_review", json!({}));
    let release = meta(&again, "gil/scopeReleaseToken").expect("같은 건의 해제 capability").to_owned();
    assert_eq!(sc(&again)["halt"]["id"].as_str(), Some(halt_id.as_str()), "화면 갱신이 새 중단 건을 만들지 않는다");
    let released = rig.app("gil_scope_release", release_with(&halt_id, &state_digest, true, &release));
    assert!(!is_error(&released), "{released}");
    assert!(sc(&released)["tracking"]["archive_halt"].is_null());
    // 해제는 아무것도 진행시키지 않았다 — 그 뒤의 열기는 별도의 명시 행동이다.
    let status = rig.status();
    assert_eq!(status["tracking"]["standing"]["state"], "closed_boundary", "새 사이클이 열리지 않았다: {status}");
    assert_eq!(status["tracking"]["standing"]["cycle"], "cycle:C1");
    rig.restart();
    ok(rig.model("gil_open", json!({"kind": "interview"})), "interview after the release");
}

/// 해제의 **부분 저장**: 해제 이력(원장)은 저장됐는데 표식을 지우는 상태 저장이 실패한 경우. 표식과 진행 차단이 남고, 새 프로세스의
/// 조회가 그것을 스스로 풀지 않으며, 옛 확인 근거로는 해제할 수 없다. 명시 재등록 → 새 화면의 새 확인 뒤에만 해제되고, 어느 걸음도
/// 재시도·보관을 시작하지 않는다.
///
/// 증거의 종류: 상태 저장의 실패는 debug 빌드의 failpoint(`GIL_SCOPE_FAIL=release`)가 그 저장을 **하지 않고** 오류를 돌려준 것이다.
/// 그 뒤의 읽기는 **새 프로세스**가 저장된 파일을 처음 읽은 것이다. 실제 디스크 장애·강제 종료·전원 장애의 증거는 아니다.
#[test]
fn a_release_whose_history_was_saved_but_whose_marker_was_not_removed_stays_halted_until_a_new_record_and_a_new_confirmation() {
    let mut rig = Rig::start("scope-release-partial");
    rig.interview();
    let review = rig.app("gil_scope_review", json!({}));
    let halt = meta(&review, "gil/scopeHaltToken").expect("중단 capability").to_owned();
    assert!(!is_error(&rig.app("gil_scope_halt", json!({"token": halt}))));
    let frozen = rig.state();

    // ── 표식 제거 저장이 실패하는 프로세스.
    rig.restart_with(&[("GIL_SCOPE_FAIL", "release")]);
    let review = rig.app("gil_scope_review", json!({}));
    let old_token = meta(&review, "gil/scopeReleaseToken").expect("해제 capability").to_owned();
    let (old_id, digest) = (sc(&review)["halt"]["id"].as_str().expect("중단 건의 이름").to_owned(),
        sc(&review)["halt"]["state_digest"].as_str().expect("상태의 지문").to_owned());
    let release = |halt_id: &str, token: &str| json!({"at": "cycle:C1", "halt_id": halt_id, "state_digest": digest, "cleanup": "continue",
        "state_confirmed": true, "token": token});
    let failed = rig.app("gil_scope_release", release(&old_id, &old_token));
    assert!(is_error(&failed), "표식을 지우지 못한 해제는 성공이 아니다: {failed}");
    assert_eq!(rig.state(), frozen, "표식이 그대로다");

    // ── 새 프로세스(failpoint 없음): 조회가 스스로 해제하지 않는다. 표식과 진행 차단이 남아 있다.
    rig.restart();
    let status = rig.status();
    assert_eq!(status["tracking"]["archive_halt"], "cycle:C1", "{status}");
    refused(rig.model("gil_open", json!({"kind": "interview"})), "interview while still halted");
    refused(rig.model("gil_scope_propose", json!({"rules": P2})), "proposal while still halted");
    let review = rig.app("gil_scope_review", json!({}));
    // 그 중단 건은 해제 이력이 적혔으므로 더는 확인 근거가 아니다: 해제 capability 가 없고, 기록을 다시 남기는 길만 있다.
    assert!(meta(&review, "gil/scopeReleaseToken").is_none(), "옛 건에 대한 해제는 제공되지 않는다: {review}");
    assert_eq!(sc(&review)["halt"]["record_missing"], true, "{review}");
    assert!(sc(&review)["halt"]["id"].is_null());
    let register = meta(&review, "gil/scopeHaltToken").expect("기록을 다시 남기는 capability").to_owned();
    assert_eq!(rig.state(), frozen, "조회·화면 읽기는 아무것도 바꾸지 않는다");
    // 옛 확인 근거(옛 이름·옛 capability)로는 해제할 수 없다.
    let stale = rig.app("gil_scope_release", release(&old_id, &old_token));
    assert!(is_error(&stale), "{stale}");
    assert_eq!(rig.state(), frozen);

    // 명시 재등록 → 새 화면이 **새 중단 건**과 그 건의 해제 capability 를 준다.
    let registered = rig.app("gil_scope_halt", json!({"token": register}));
    assert!(!is_error(&registered), "{registered}");
    assert_eq!(rig.state(), frozen, "재등록은 표식을 옮기지 않는다");
    let review = rig.app("gil_scope_review", json!({}));
    let new_token = meta(&review, "gil/scopeReleaseToken").expect("새 건의 해제 capability").to_owned();
    let new_id = sc(&review)["halt"]["id"].as_str().expect("새 중단 건의 이름").to_owned();
    assert_ne!(new_id, old_id, "재등록은 새 확인 대상을 만든다");
    // 옛 이름에 새 capability 를 붙여도 통하지 않는다.
    let mixed = rig.app("gil_scope_release", release(&old_id, &new_token));
    assert!(is_error(&mixed), "{mixed}");
    assert_eq!(rig.state(), frozen);
    let review = rig.app("gil_scope_review", json!({}));
    let new_token = meta(&review, "gil/scopeReleaseToken").expect("같은 새 건의 해제 capability").to_owned();
    assert_eq!(sc(&review)["halt"]["id"].as_str(), Some(new_id.as_str()));
    let released = rig.app("gil_scope_release", release(&new_id, &new_token));
    assert!(!is_error(&released), "{released}");
    assert!(sc(&released)["tracking"]["archive_halt"].is_null());

    // 해제는 아무것도 진행시키지 않았다.
    rig.restart();
    let status = rig.status();
    assert!(status["tracking"]["archive_halt"].is_null(), "{status}");
    assert_eq!(status["tracking"]["standing"], json!({"cycle": "cycle:C1", "state": "closed_boundary"}));
    assert_eq!(status["tracking"]["applied_policy"], "policy:P1");
    assert!(status["tracking"]["waiting"].is_null());
}

/// **같은 서버 연결**이 승인을 처리하는 동안 즉시 보관 중단 요청을 받는다(실제 요청 경로). 승인의 보관은 debug 빌드의 대기 자리
/// (`GIL_SCOPE_HOLD`)로 capture 앞에 세워 둔다 — 그 사이 그 명령이 프로젝트 잠금을 쥐고 있다. 그동안 같은 stdio 로 보낸 중단 요청의
/// 응답이 **승인의 응답보다 먼저** 돌아오고(수신됨·아직 적용 안 됨), 승인의 보관은 다음 걸음을 시작하지 않으며(`halted`, 실패 기록 없음),
/// 그 명령이 멈춘 직후 중단이 적용된다. 이미 끝난 저장(승인 저장·정책 파일)은 그대로다.
///
/// 시험이 정한 것은 「언제 보내는가」뿐이다: 대기 자리에 닿았다는 파일을 본 뒤 중단을 보내고, 중단의 응답을 받은 뒤 대기를 푼다.
#[test]
fn the_same_server_receives_a_halt_while_its_approval_is_in_flight_and_the_archive_starts_no_further_step() {
    let mut rig = Rig::start("scope-inflight");
    rig.interview();
    let gate = scratch("scope-inflight-gate").join("go");
    let reached = gate.with_extension("reached");
    let hold = format!("capture:{}", gate.display());
    rig.restart_with(&[("GIL_SCOPE_HOLD", hold.as_str())]);

    let proposed = rig.model("gil_scope_propose", json!({"rules": P2}));
    assert!(!is_error(&proposed), "{proposed}");
    let (revision, digest) = (sc(&proposed)["proposal"]["revision"].clone(), sc(&proposed)["proposal"]["digest"].clone());
    // 한 번의 화면 읽기가 승인과 중단의 capability 를 함께 준다 — 다른 작업이 시작돼도 중단의 것은 남는다.
    let review = rig.app("gil_scope_review", json!({}));
    let approval = meta(&review, "gil/scopeApprovalToken").expect("승인 capability").to_owned();
    let halt = meta(&review, "gil/scopeHaltToken").expect("중단 capability").to_owned();

    let decide_id = rig.c.post("tools/call", json!({"name": "gil_scope_decide", "arguments":
        {"scope_id": rig.scope, "revision": revision, "digest": digest, "token": approval, "decision": "approve"}}));
    // 승인의 보관이 capture 앞에 닿을 때까지 기다린다(길어야 20초).
    for _ in 0..400 {
        if reached.exists() { break; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(reached.exists(), "승인의 보관이 capture 앞에 닿지 않았다");
    let halt_id = rig.c.post("tools/call", json!({"name": "gil_scope_halt", "arguments": {"scope_id": rig.scope, "token": halt}}));

    // 먼저 오는 응답은 중단 요청의 것이다 — 승인은 아직 세워져 있다.
    let first = rig.c.next_frame();
    assert_eq!(first["id"], halt_id, "진행 중인 승인보다 중단 요청의 응답이 먼저 온다: {first}");
    assert!(!is_error(&first), "{first}");
    assert_eq!(sc(&first)["halt_request"]["received"], true, "{first}");
    assert_eq!(sc(&first)["halt_request"]["applied"], false, "다른 명령이 프로젝트를 쥐고 있어 아직 적용되지 않았다: {first}");

    // 대기를 푼다 → 승인의 응답. 보관은 capture 를 시작하지 않았고, 실패를 기록하지 않았다.
    std::fs::write(&gate, b"").unwrap();
    let second = rig.c.next_frame();
    assert_eq!(second["id"], decide_id, "{second}");
    assert!(!is_error(&second), "{second}");
    assert_eq!(sc(&second)["facts"]["approval_response"], "recorded");
    assert_eq!(sc(&second)["facts"]["policy_applied"], true, "끝난 승인 저장은 그대로다: {second}");
    assert_eq!(sc(&second)["facts"]["policy_file"], "written", "중단 요청 전에 끝난 파일 동기화는 그대로다");
    assert_eq!(sc(&second)["facts"]["policy_file_steps"], json!({"start": "saved", "start_save_returned": "ok", "file": "replaced",
        "base": "saved", "base_save_returned": "ok", "stored_base": "present", "base_left_unconfirmed": false, "halted_before": null}));
    assert_eq!(sc(&second)["tracking"]["archive_halt"], "event:M2", "{second}");
    assert_eq!(sc(&second)["facts"]["baseline"]["result"], "halted", "{second}");
    assert_eq!(sc(&second)["facts"]["baseline"]["before"], "capture");
    assert_eq!(sc(&second)["facts"]["baseline"]["failure_recorded"], false);
    assert_eq!(sc(&second)["facts"]["baseline"]["halt_request"], json!({"received": true, "applied": true}));
    assert_eq!(std::fs::read_to_string(rig.root.join(".gilignore")).unwrap(), P2);

    // 새 프로세스가 읽는 기록: 중단이 적용돼 있고, 사건은 대기이며, 실패 시도는 없다. 확정 세계는 그대로다.
    rig.restart();
    let status = rig.status();
    // 승인 저장 뒤에는 M2 가 보관 대기다 — 표식은 적용되는 때의 자리, 곧 그 사건을 가리킨다(`halt_place`).
    assert_eq!(status["tracking"]["archive_halt"], "event:M2", "{status}");
    assert_eq!(status["tracking"]["standing"]["cycle"], "cycle:C1");
    assert_eq!(status["tracking"]["applied_policy"], "policy:P2");
    assert_eq!(status["tracking"]["last_confirmed_world"], "snapshot:A1");
    assert_eq!(status["tracking"]["confirmed_policy"], "policy:P1");
    assert_eq!(status["tracking"]["waiting"]["event"], "event:M2");
    assert_eq!(status["tracking"]["waiting"]["failed_attempts"], json!([]));
    // 중단 중에는 재시도 capability 가 없고, 해제만 있다 — 자동 재시도도 없다.
    let review = rig.app("gil_scope_review", json!({}));
    assert!(meta(&review, "gil/scopeRetryToken").is_none() && meta(&review, "gil/scopeReleaseToken").is_some(), "{review}");
    assert_eq!(sc(&review)["halt"]["at"], "event:M2", "{review}");
    assert_eq!(rig.status()["tracking"]["waiting"]["failed_attempts"], json!([]));
}

/// **이 실행 파일이 실제로 내는** Monitor 자산의 연결: `tools/list` 의 `show_gil_monitor` 가 가리키는 URI → `resources/read` 가 돌려준
/// MIME·HTML 바이트 → 그 SHA-256 이 URI 와 포장 자산(`monitor.html`·`monitor.json`)과 같다. 그리고 그 HTML 에 범위 변경 패널이 있다.
///
/// 포장 자산을 다시 만들기 전에는 마지막 확인이 **실패한다** — 그것이 해결해야 할 인수 항목이다(이 시험은 그 누락을 통과시키지 않는다).
/// `GIL_SCOPE_MONITOR_HTML_OUT` 이 있으면 돌려받은 HTML 바이트를 그 파일에 적는다: 합성 Host 검사(`mcp-app/scope-browser-check.mjs`)가
/// 포장 파일이나 다른 구성의 실행 파일이 아니라 **이 구성의 실행 파일이 낸 바이트**를 띄우게 하려는 것이다.
#[test]
fn this_executable_serves_the_packaged_monitor_and_the_scope_panel_is_in_it() {
    use sha2::{Digest, Sha256};
    let settings = scratch("scope-resource-settings");
    let mut c = Client::serve(&settings);
    let listed = c.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "gil_scope_review"), "tracking-policy 구성의 실행 파일이다");
    let show = tools.iter().find(|tool| tool["name"] == "show_gil_monitor").expect("show_gil_monitor");
    let uri = show["_meta"]["ui"]["resourceUri"].as_str().expect("resourceUri").to_owned();
    assert_eq!(show["_meta"]["ui/resourceUri"], json!(uri));
    let resource = c.request("resources/read", json!({"uri": uri}));
    assert!(resource.get("error").is_none(), "{resource}");
    let contents = resource["result"]["contents"].as_array().expect("contents 배열");
    assert_eq!(contents.len(), 1, "정확히 하나의 본문이 온다");
    let content = &contents[0];
    assert_eq!(content["uri"], json!(uri), "돌려준 본문의 URI 가 요청한 URI 다");
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    let html = content["text"].as_str().expect("HTML 본문");
    let sha: String = Sha256::digest(html.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
    let packaged: Value = serde_json::from_str(include_str!("../plugins/gil-companion-prototype/assets/monitor.json")).unwrap();
    assert_eq!(packaged["sha256"], json!(sha), "돌려받은 바이트의 SHA-256 이 포장 기록과 같다");
    assert_eq!(packaged["resource_uri"], json!(uri), "URI 가 포장 기록과 같다");
    assert!(uri.contains(&sha[..20]), "URI 가 그 바이트의 지문을 싣는다: {uri}");
    assert_eq!(html, include_str!("../plugins/gil-companion-prototype/assets/monitor.html"), "포장 자산과 바이트가 같다");
    if let Ok(out) = std::env::var("GIL_SCOPE_MONITOR_HTML_OUT") {
        std::fs::write(&out, html.as_bytes()).expect("돌려받은 HTML 을 적는다");
    }
    // 범위 변경 패널이 이 실행 파일이 내는 자산 안에 있어야 한다.
    assert!(html.contains("id=\"scope-review\""), "포장된 Monitor 에 범위 변경 패널의 자리가 없다 — 자산을 다시 만들어야 한다");
    assert!(html.contains("gil_scope_review") && html.contains("gil_scope_halt") && html.contains("gil_scope_release"),
        "포장된 Monitor 에 범위 변경 패널의 코드가 없다 — 자산을 다시 만들어야 한다");
}
