//! Short host guidance shared by Plugin and direct-MCP installations.
//! These are instructions, not new Project facts or structured receipt fields.
//!
//! Each hint states **one next call or one wait first**, read from the receipt that Core already
//! produced (`structuredContent.said` for text actions, typed fields for plan tools). Nothing
//! here recomputes Project state or promises that a screen is visible. When a receipt shape is
//! not recognised, the generic sentence for that tool is used instead of guessing.
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::Value;

// The Plugin skill embeds this exact text; CI rejects a stale generated copy.
pub const INSTRUCTIONS: &str = include_str!("workflow.md");

pub fn prepend(result: &mut CallToolResult, text: impl Into<String>) {
    result.content.insert(0, ContentBlock::text(text.into()));
}

pub fn after(name: &str, result: &mut CallToolResult) {
    if result.is_error == Some(true) {
        if let Some(hint) = refused(name) {
            let problem = problem(result);
            // A dirty refusal of a close has one more real recovery than "fix the input".
            let hint = if name == "gil_close" && (problem.contains("기준 세계") || problem.contains("dirty")) {
                format!("{hint} {DIRTY_CLOSE_RECOVERY}")
            } else { hint.to_string() };
            prepend(result, hint);
        }
        return;
    }
    let hint = match name {
        "gil_open" => opened(result),
        "gil_close" => closed(result),
        "gil_context" => "GIL 재개: 아래 context의 [지금 바로 할 호출] 한 줄을 먼저 따른다. 완료 조건을 충족했다면 추가 작업보다 gil_close가 먼저다. 이미 닫힌 단계를 반복하지 않고, 기존 프로젝트에 gil_start를 다시 부르지 않는다. 조사·채팅 보고서는 열린 Interview에서 수행하지 않는다.".to_string(),
        "gil_status" => "GIL 위치: 아래 '다음:' 줄 하나가 지금 밟을 수 있는 수다. 같은 세션에서 status를 반복 호출하지 않고 그 수를 실행한다.".to_string(),
        "gil_revisit" => "GIL 되돌아감 완료: 작업 폴더가 대상 Cycle의 세계로 복원됐다. 다음 호출은 gil_open(kind: interview|experiment)으로 새 Cycle을 여는 것이다. gil_revisit을 반복하지 않는다.".to_string(),
        "show_gil_monitor" => "GIL 진행: Monitor 표시를 요청했다. 신규 시작 흐름이면 같은 project_root로 첫 Interview의 gil_open을 호출하고 질문한다. 기존 프로젝트라면 현재 열린 단계를 이어간다. 실제 화면 표시 여부는 아직 확인되지 않았다.".to_string(),
        "gil_plan_propose" | "gil_plan_read" | "gil_plan_request_interview" => match plan(name, result) {
            Some(hint) => hint,
            None => return,
        },
        _ => return,
    };
    prepend(result, hint);
}

/// A refused action moved nothing. Say so once and point at the one recovery call.
fn refused(name: &str) -> Option<&'static str> {
    Some(match name {
        "gil_close" => "GIL 거절: 닫힌 것은 없다. 아래 이유의 입력 오류를 고쳐 같은 gil_close를 한 번 다시 보낸다. 위치가 불확실하면 먼저 gil_status를 읽는다. 다른 단계를 건너뛰거나 기록을 몰아 채우지 않는다.",
        "gil_open" => "GIL 거절: 열린 것은 없다. 아래 이유를 읽고 현재 위치에서 허용되는 종류와 행동 계약(objective · next_action · done_when)으로 한 번 다시 보낸다. 위치가 불확실하면 gil_status를 읽는다.",
        "gil_revisit" => "GIL 거절: 되돌아가지 않았다. 이유가 dirty 세계·승인 계획 바인딩·적어 둔 방향 없음이면 그 조건을 사용자에게 알린다. gil_revisit을 반복하거나 restore를 자동 실행하지 않는다.",
        "gil_start" => "GIL 거절: 새 프로젝트를 만들지 않았다. 이미 프로젝트가 있다면 gil_context를 한 번 읽고 기록된 위치에서 이어간다. gil_start를 반복하지 않는다.",
        "gil_plan_propose" => "GIL 거절: 제안이 기록되지 않았다. 열린 Interview Synthesis인지, scope 5칸·items(id · question · done_when)·synthesis 칸이 모두 있는지, exploration_checklist가 items의 id를 같은 순서로 한 줄씩 적었는지, approved · plan_revision · plan_digest를 넣지 않았는지 확인해 한 번 다시 보낸다. 이 프로젝트가 승인 계획 프로필이 아니라면 일반 Synthesis로 사용자의 답을 기다린다.",
        "gil_plan_read" => "GIL 거절: 승인 상태를 읽지 못했다. 응답이 불확실하면 gil_plan_read를 한 번 다시 읽는다. 승인을 지어내거나 Synthesis를 닫지 않는다.",
        "gil_plan_request_interview" => "GIL 거절: 재인터뷰 요청이 기록되지 않았다. 열린 Experiment에서 1~4096 bytes 이유로 한 번 다시 보낸다. 현재 Step의 다음 호출은 바뀌지 않는다 — 마지막 receipt의 다음 행동을 그대로 이어간다.",
        _ => return None,
    })
}

/// One line for a close refused by the dirty gate — only what this build can actually do.
const DIRTY_CLOSE_RECOVERY: &str = if cfg!(feature = "product-safety") {
    "dirty 거절이면: 이 빌드는 Artifact 되돌리기(restore)를 거부한다. 거절문의 변경 목록을 보고 새 파일은 프로젝트 밖의 새 보존 위치로 옮기고(내용을 남기려면 지금 자리를 닫은 뒤 다음 Experiment의 Verify에서 다시 만들어 evidence_files에 적는다), 바뀐 기존 파일은 현재 내용을 별도로 보존한 뒤 기준 내용으로 되돌린 뒤 같은 gil_close를 보낸다."
} else {
    "dirty 거절이면: 거절문의 변경 목록을 사용자에게 알리고, 사용자가 되돌리기를 택하면 gil_restore 뒤 같은 gil_close를 보낸다. Verify 밖에서 만든 전달물은 다음 Experiment의 Verify에서 다시 만든다."
};

fn problem(result: &CallToolResult) -> &str {
    result.structured_content.as_ref()
        .and_then(|v| v.get("problem")).and_then(Value::as_str).unwrap_or("")
}

fn said(result: &CallToolResult) -> &str {
    result.structured_content.as_ref()
        .and_then(|v| v.get("said")).and_then(Value::as_str).unwrap_or("")
}

/// `열었다: step:C1/S2 · hypothesis` → `hypothesis`; `열었다: cycle:C2 · experiment` → `experiment`.
fn first_line_kind<'a>(said: &'a str, lead: &str) -> Option<&'a str> {
    let line = said.lines().next()?.strip_prefix(lead)?;
    line.split(" · ").nth(1).map(str::trim)
}

fn opened(result: &CallToolResult) -> String {
    let generic = "GIL 진행: 아래 '지금 할 일'만 수행한다. 완료 조건을 충족하면 관측한 결과로 gil_close한 뒤 다음 행동으로 간다. 다른 목적의 일을 이 노드에 계속 붙이지 않는다. Interview는 목표 합의까지만 한다. 조사·계산·보고서 작성(채팅 전달도 포함)·구현·테스트는 Experiment의 Verify에서 계약 범위만큼 수행한다.";
    let said = said(result);
    let Some(kind) = first_line_kind(said, "열었다: ") else { return generic.into(); };
    let specific = match kind {
        "question" => "GIL 진행: Question을 열었다. 사용자에게 질문하고 턴을 끝내 답을 기다린다. 답을 받으면 그 원문으로 gil_close한다. 기다리는 동안 조사·보고서 작성을 하지 않는다.",
        "interpretation" => "GIL 진행: Interpretation을 열었다. 받은 답에서 확정된 것과 모르는 것을 갈라 gil_close한다. 외부 조사는 하지 않는다.",
        "synthesis" => "GIL 진행: Synthesis를 열었다. 제안 statement를 사용자에게 보여 주고 턴을 끝내 답을 기다린다(승인 계획 프로필이면 먼저 gil_plan_propose로 제안하고 Monitor 버튼을 기다린다). 명시적 동의가 있을 때만 gil_close한다. 침묵·이전 요청·자체 판단은 승인이 아니다. 기다리는 동안 조사·보고서 작성을 하지 않는다.",
        "outcome" if said.contains("Interview") => "GIL 진행: Interview Outcome을 열었다. 승인된 범위를 판정해 gil_close한다. 프로젝트가 끝날 때까지 열어 두지 않는다. 닫은 뒤 Interview Cycle Report를 별도로 gil_close한다.",
        "outcome" => "GIL 진행: Experiment Outcome을 열었다. 실제 Analysis에 근거한 verdict·lesson·next_direction으로 gil_close한다. 닫은 뒤 Cycle Report를 별도로 gil_close한다.",
        "define" => "GIL 진행: Define을 열었다. 확인할 질문 하나와 판정 기준만 적어 gil_close한다. 자료 조회·계산은 Verify에서 한다.",
        "hypothesis" => "GIL 진행: Hypothesis를 열었다. 예상·근거·guardrail을 적어 gil_close한다. 닫힌 뒤 다음 호출은 Verify의 gil_open이며, 그 성공 전에는 조사·계산·파일 작성을 하지 않는다.",
        "verify" => "GIL 진행: Verify를 열었다. 이제 계약의 next_action을 실제로 수행한다. 관측 결과를 프로젝트 파일로 저장하고 evidence_files를 포함해 gil_close한다. 채팅만으로는 닫히지 않는다. 시각 자료를 함께 남긴다: 데이터는 코드로 그린 플롯, 구조는 다이어그램, UI·웹은 실제 브라우저(헤드리스 가능) 스크린샷, 파일 변화는 비교표를 PNG로 저장해 visuals.attachments/uses와 declaration attached로 첨부한다(AI 생성 이미지는 시각 자료가 아니다). 정말 그림이 필요 없으면 not_needed에 무엇이 이미지를 대신하는지 구체적으로 적는다 — 한 단어 이유는 거절된다. 승인 계획 프로필이면 plan_document 파일과 별도 실행 증거를 모두 남긴다.",
        "analysis" => "GIL 진행: Analysis를 열었다. 저장된 Verify 증거만 해석해 nonblank interpretation으로 gil_close한다. 새 조사는 하지 않는다. Verify가 시각 자료를 첨부했다면 이 close는 그 자료를 인용해야 한다: expected_owner와 visuals를 함께 보내고 uses에 Verify receipt의 owner·asset_id·sha256을 stored source로 적는다(attachments는 빈 배열, declaration 생략).",
        "interview" => "GIL 진행: Interview Cycle을 열었다. 다음 호출은 gil_open(kind: question)에 행동 계약을 적는 것이다. 조사·보고서 작성은 Interview에서 하지 않는다.",
        "experiment" => "GIL 진행: Experiment Cycle을 열었다. 다음 호출은 gil_open(kind: define)에 행동 계약을 적는 것이다. 질문 하나만 다루며 Define·Hypothesis를 닫기 전에는 조사하지 않는다.",
        _ => generic,
    };
    specific.into()
}

fn closed(result: &CallToolResult) -> String {
    let generic = "GIL 진행: 이 닫기가 성공했다. 다음 작업 전에 아래 영수증의 다음 행동을 따른다. Hypothesis를 닫았다면 다음 행동은 Verify의 gil_open이다. 열기 성공을 확인하기 전에는 자료 읽기·검색·계산·파일 작성을 하지 않는다. 다음 안내가 없는 typed close라면 gil_status를 한 번 읽는다. 기록되지 않은 다음 단계를 먼저 수행하지 않는다.";
    let said = said(result);
    let Some(kind) = first_line_kind(said, "닫았다: ") else { return generic.into(); };
    let next = next_from_receipt(said);
    let specific = match (kind, next.as_ref()) {
        ("hypothesis", _) => "다음 행동: gil_open(kind: verify)에 행동 계약을 적어 Verify를 연다. 열기 성공 전에는 자료 읽기·검색·계산·파일 작성을 하지 않는다.".to_string(),
        ("synthesis", Some(Next::Open(k))) if k == "outcome" => "다음 행동: gil_open(kind: outcome)으로 Interview Outcome을 연다. 승인을 다시 묻거나 Synthesis를 다시 열지 않는다. Outcome을 닫은 뒤 Interview Cycle Report도 별도로 닫는다.".to_string(),
        ("synthesis", Some(Next::Open(k))) if k == "question" => "다음 행동: 승인되지 않았다. gil_open(kind: question)으로 무엇을 바꿔야 하는지 묻는다. 이전 제안의 승인은 재사용하지 않는다.".to_string(),
        ("verify", _) => "다음 행동: gil_open(kind: analysis)으로 저장된 증거를 해석하는 자리를 연다. 새 조사는 하지 않는다. 이 Verify가 시각 자료를 첨부했다면 Analysis와 Cycle Report의 close에서 그 자료(owner·asset_id·sha256)를 stored source로 인용한다.".to_string(),
        ("outcome", Some(Next::CloseCycle)) if said.contains("Interview") => "다음 행동: gil_close로 Interview Cycle Report를 제출한다 (verdict · outcome_ref · handoff_summary · next_direction). Experiment·조사·보고서 작성은 그 성공 뒤에 시작한다.".to_string(),
        ("outcome", Some(Next::CloseCycle)) => "다음 행동: gil_close로 이 Cycle의 Report를 제출한다. Verify가 시각 자료를 첨부했다면 Cycle Report도 expected_owner {kind: cycle}와 visuals.uses(stored source)로 그 자료를 인용한다. 다음 문제는 그 뒤에 연다.".to_string(),
        ("interview", Some(Next::OpenCycle)) => "다음 행동: Interview Cycle까지 닫혔다. gil_open(kind: experiment)으로 합의한 첫 질문의 Experiment를 연다. 승인 계획 프로필이면 첫 미실행 항목이 자동으로 묶인다. gil_start를 다시 부르지 않는다.".to_string(),
        ("interview", Some(Next::ChainReport)) | ("experiment", Some(Next::ChainReport)) => "다음 행동: Chain Report를 gil_close로 제출한다. 새 Cycle을 열지 않는다.".to_string(),
        ("experiment", Some(Next::Revisit)) if cfg!(feature = "product-safety") => "다음 행동: 이 Cycle은 failure·revisit으로 닫혔다. Monitor를 열고 변경 목록과 보존 위치를 사용자가 확인하도록 안내한 뒤 ‘보존 후 되돌아가기’ 버튼을 기다린다. 채팅 동의를 승인으로 옮겨 적거나 gil_revisit을 반복하지 않는다. 복원 완료 후 gil_open으로 새 Cycle을 연다. 실패 기록·증거·학습은 남는다.".to_string(),
        ("experiment", Some(Next::Revisit)) => "다음 행동: 이 Cycle은 failure·revisit으로 닫혔다. Artifact 복원이 따르므로 사용자에게 알린 뒤 gil_revisit(인수 없음)을 한 번 호출하고, 그 다음 gil_open으로 새 Cycle을 연다.".to_string(),
        ("experiment", Some(Next::OpenCycle)) => "다음 행동: Experiment Cycle이 닫혔다. 남은 독립 질문이 있으면 gil_open(kind: experiment), 목표를 다시 합의해야 하면 gil_open(kind: interview)을 연다. 일이 모두 끝났어도 채팅 보고만으로 끝내지 않는다: Chain이 있으면 gil_open(kind: interview)으로 Closing Interview를 열어 최종 판정을 승인받고 Chain Report를 닫는다.".to_string(),
        (_, Some(Next::Open(k))) => format!("다음 행동: gil_open(kind: {k})에 행동 계약을 적어 연다. 그 전에 조사·파일 작성을 하지 않는다."),
        (_, Some(Next::OpenMany(kinds))) => format!("다음 행동: gil_open으로 다음 중 하나를 연다: {kinds}. 고르는 기준은 아래 영수증에 있다."),
        (_, Some(Next::CloseCycle)) => "다음 행동: gil_close로 이 Cycle의 Report를 제출한다. gil_cycle(action=close)는 사용하지 않는다.".to_string(),
        (_, Some(Next::Blocked)) => "다음 행동: 여기서 할 수 있는 것이 없다. gil_status를 한 번 읽고 막힌 이유를 사용자에게 알린다.".to_string(),
        _ => generic.to_string(),
    };
    format!("GIL 진행: 이 닫기가 성공했다. {specific}")
}

/// What the Core receipt's `다음` block says can actually be stepped. Read, not recomputed.
#[derive(Debug, PartialEq)]
enum Next {
    Open(String),
    OpenMany(String),
    CloseCycle,
    OpenCycle,
    ChainReport,
    Revisit,
    Blocked,
}

fn next_from_receipt(said: &str) -> Option<Next> {
    // Order matters: a Cycle-close skeleton may quote `gil revisit` inside a value description.
    if said.contains("Chain Report를 gil close로 제출한다") { return Some(Next::ChainReport); }
    if said.contains("이 Cycle 의 Report 를 적어 닫는다") { return Some(Next::CloseCycle); }
    if said.contains("적어 둔 조상으로 되돌아간다") { return Some(Next::Revisit); }
    if said.contains("여기서 할 수 있는 것이 없다") { return Some(Next::Blocked); }
    let mut lines = said.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("열 수 있는 것: ") {
            return Some(Next::Open(rest.trim().to_string()));
        }
        if line == "열 수 있는 것" {
            // `gil open <종류>`: one `name  description` line per kind (two spaces after the name).
            let kinds: Vec<&str> = lines.by_ref()
                .take_while(|l| !l.is_empty() && !l.starts_with("실행"))
                .filter_map(|l| l.split_whitespace().next())
                .collect();
            if kinds.iter().any(|k| *k == "interview" || *k == "experiment") { return Some(Next::OpenCycle); }
            return Some(Next::OpenMany(kinds.join(", ")));
        }
    }
    None
}

/// Plan tools answer with typed fields; the next call follows the recorded decision, not the OS.
fn plan(name: &str, result: &CallToolResult) -> Option<String> {
    let view = result.structured_content.as_ref()?;
    if view.get("supported") == Some(&Value::Bool(false)) {
        return Some("승인 계획: 이 프로젝트는 승인 계획 프로필이 아니다. gil_plan_* 를 요구하지 않고 일반 Synthesis로 사용자의 답을 기다린다.".into());
    }
    let proposal = view.get("proposal").filter(|p| !p.is_null());
    let decision = proposal.and_then(|p| p.pointer("/receipt/decision")).and_then(Value::as_str);
    let revision = proposal.and_then(|p| p.get("revision")).and_then(Value::as_u64);
    let reinterview = view.get("reinterview_requests").and_then(Value::as_array).is_some_and(|r| !r.is_empty());
    let pending: Vec<String> = view.get("pending").and_then(Value::as_array).map(|items|
        items.iter().filter_map(|i| i.get("id").and_then(Value::as_str)).map(str::to_string).collect()).unwrap_or_default();
    let current = view.get("current_item").filter(|i| !i.is_null()).and_then(|i| i.get("id")).and_then(Value::as_str);
    // A binding survives Cycle closure. B's read-only state is the witness; without it
    // an older server's binding cannot justify telling the model to resume Verify.
    let cycle_state = view.pointer("/current_cycle/state").and_then(Value::as_str);
    if name == "gil_plan_read"
        && (cycle_state == Some("closed") || (current.is_some() && cycle_state != Some("open")))
    {
        return Some(if cycle_state == Some("closed") {
            "승인 계획: 현재 Cycle은 이미 닫혔다. 항목 연결은 기록으로 남아 있다. 다음 호출은 gil_context다. 그 응답의 다음 행동에 따라 새 Cycle 또는 Chain Report를 이어간다. 닫힌 Step에서 조사·파일 작성을 계속하지 않는다.".into()
        } else {
            "승인 계획: 항목 연결만으로 현재 Cycle이 열려 있는지 확인할 수 없다. 다음 호출은 gil_context다. 실제 열린 Step과 다음 행동을 확인한 뒤 이어간다.".into()
        });
    }
    Some(match name {
        "gil_plan_request_interview" => "승인 계획: 재인터뷰 요청을 보존했다. 아무것도 닫히거나 열리지 않았다. 지금 열린 Step은 마지막 receipt의 다음 행동대로 이어가고, 남은 Step과 Cycle Report가 실제 순서대로 닫힌 뒤에 gil_open(kind: interview)으로 다시 승인받는다. 요청은 그동안 보존되며, 그 전에는 새 Experiment를 열 수 없다. 요청을 취소하거나 Cycle을 중단하는 호출은 없다.".into(),
        _ => match (proposal, decision) {
            (Some(_), None) => format!("승인 계획: 제안 판 {}이(가) 기록됐고 Monitor 버튼 응답을 기다린다. 턴을 끝내고 기다린다. 채팅 동의로 대체하거나 approved를 직접 적지 않는다. 그동안 조사·보고서 작성·재제안을 하지 않는다. 응답 여부는 gil_plan_read로만 확인한다.", revision.unwrap_or(0)),
            (Some(_), Some("approve")) => format!("승인 계획: 판 {}이(가) 승인됐다. 다음 호출은 close_report를 글자 그대로 넣은 gil_close(Synthesis)다. 그 뒤 gil_open(kind: outcome) → gil_close → Interview Cycle Report gil_close 순서로 닫은 다음에야 gil_open(kind: experiment)을 연다.", revision.unwrap_or(0)),
            (Some(_), Some(_)) => format!("승인 계획: 판 {}에 수정 요청이 왔다. close_report(approved: no)로 Synthesis를 gil_close하고 다음 Question에서 무엇을 바꿀지 묻는다. 수정한 제안은 새 판이며 이전 승인은 재사용되지 않는다.", revision.unwrap_or(0)),
            (None, _) if reinterview => "승인 계획: 재인터뷰 요청이 보존돼 있다. 현재 Cycle의 남은 Step을 receipt 순서대로 마치고 Cycle Report를 닫은 뒤 gil_open(kind: interview)으로 다시 승인받는다. 그 전에는 Experiment를 열 수 없다. 열린 Step이 있으면 그 Step의 close가 다음 호출이다.".into(),
            (None, _) => match current {
                Some(item) => format!("승인 계획: 이번 Experiment는 항목 {item}에 묶여 있다. Verify에서 document를 bytes 그대로 파일로 저장해 plan_document에 적고, 실제 작업 증거는 별도 파일로 evidence_files에 적는다."),
                None if !pending.is_empty() => format!("승인 계획: 열린 제안이 없다. 미실행 항목 {}. Interview가 닫혀 있으면 gil_open(kind: experiment)으로 첫 미실행 항목을 연다. 열린 Synthesis가 있으면 먼저 gil_plan_propose로 제안한다.", pending.join(", ")),
                None => "승인 계획: 열린 제안도 미실행 항목도 없다. 열린 Synthesis에서는 gil_plan_propose로 제안한다. 항목을 모두 실행했으면 gil_open(kind: interview)으로 Closing Interview를 열어 최종 판정(chain_verdict)을 승인받고 Chain Report를 닫는다. 채팅 보고만으로 끝내지 않고, 승인을 지어내지 않는다.".into(),
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn receipt(said: &str) -> CallToolResult {
        let mut out = CallToolResult::success(vec![ContentBlock::text(said.to_string())]);
        out.structured_content = Some(json!({"ok": true, "exit_code": 0, "said": said, "problem": ""}));
        out
    }
    fn text(result: &CallToolResult) -> String {
        result.content.iter().filter_map(|c| c.as_text().map(|t| t.text.clone())).collect::<Vec<_>>().join("\n")
    }

    fn refusal_of(problem: &str) -> CallToolResult {
        let mut out = CallToolResult::success(vec![ContentBlock::text(problem.to_string())]);
        out.structured_content = Some(json!({"ok": false, "exit_code": 1, "said": "", "problem": problem}));
        out.is_error = Some(true);
        out
    }

    #[test]
    fn a_dirty_close_refusal_adds_the_recovery_this_build_allows() {
        let mut dirty = refusal_of("거절: cycle:C2 을(를) 닫을 수 없다.\n\n이유\n  기준 세계 snapshot:A1 이후 프로젝트 파일이 바뀌었다.\n    새로 생김  report.md\n");
        after("gil_close", &mut dirty);
        let hint = text(&dirty);
        assert!(hint.starts_with("GIL 거절: 닫힌 것은 없다."), "{hint}");
        assert!(hint.contains("dirty 거절이면:"), "{hint}");
        if cfg!(feature = "product-safety") {
            assert!(!hint.contains("gil restore") && !hint.contains("gil_restore"), "{hint}");
            assert!(hint.contains("프로젝트 밖의 새 보존 위치로 옮기고") && hint.contains("다음 Experiment의 Verify"), "{hint}");
        } else {
            assert!(hint.contains("gil_restore"), "{hint}");
        }
        // An ordinary input refusal gets no dirty line.
        let mut plain = refusal_of("Report 에 verdict 가 없다\n");
        after("gil_close", &mut plain);
        assert!(!text(&plain).contains("dirty 거절이면"), "{}", text(&plain));
    }

    #[test]
    fn a_closed_hypothesis_names_verify_open_first_and_nothing_else() {
        let mut r = receipt("닫았다: step:C2/S2 · hypothesis\n기록됨: journey:J3\n\n다음\n  열 수 있는 것: verify\n\n실행\n  gil open\n");
        after("gil_close", &mut r);
        let hint = text(&r);
        assert!(hint.starts_with("GIL 진행: 이 닫기가 성공했다. 다음 행동: gil_open(kind: verify)"), "{hint}");
        assert!(!hint.contains("gil_cycle"), "{hint}");
    }

    #[test]
    fn a_closed_synthesis_follows_the_recorded_approval_not_the_sequence() {
        let mut yes = receipt("닫았다: step:C1/S3 · synthesis\n기록됨: journey:J4\n\nInterview 마무리\n  승인된 Synthesis만 닫혔다.\n\n다음\n  열 수 있는 것: outcome\n\n실행\n  gil open\n");
        after("gil_close", &mut yes);
        assert!(text(&yes).contains("gil_open(kind: outcome)"), "{}", text(&yes));
        assert!(!text(&yes).contains("gil_open(kind: question)"));
        let mut no = receipt("닫았다: step:C1/S3 · synthesis\n기록됨: journey:J4\n\n다음\n  열 수 있는 것: question\n\n실행\n  gil open\n");
        after("gil_close", &mut no);
        assert!(text(&no).contains("gil_open(kind: question)") && text(&no).contains("재사용하지 않는다"), "{}", text(&no));
    }

    #[test]
    fn interview_outcome_then_cycle_report_then_experiment_are_three_distinct_calls() {
        let mut outcome = receipt("닫았다: step:C1/S4 · outcome\n기록됨: journey:J5\n\nInterview 마무리\n  Outcome Step만 닫혔고 Interview Cycle은 아직 열려 있다.\n\n다음\n  이 Cycle 의 Report 를 적어 닫는다. MCP에서는 gil_close(project_root, report)를 사용한다.\n\n실행\n  gil close\n");
        after("gil_close", &mut outcome);
        assert!(text(&outcome).contains("Interview Cycle Report"), "{}", text(&outcome));
        assert!(!text(&outcome).contains("gil_open(kind: experiment)"));
        let mut cycle = receipt("닫았다: cycle:C1 · interview · success\n기록됨: journey:J6 (판은 늘지 않는다)\n\n다음\n  Interview Cycle까지 닫혔다.\n  열 수 있는 것\n  interview   사용자에게 질문해 의도와 판단을 확인한다\n  experiment  명확한 가설을 실행하고 결과를 검증한다\n\n실행\n  gil open <종류>\n");
        after("gil_close", &mut cycle);
        assert!(text(&cycle).contains("gil_open(kind: experiment)") && text(&cycle).contains("gil_start를 다시 부르지 않는다"), "{}", text(&cycle));
    }

    #[test]
    fn a_failed_experiment_is_told_to_revisit_once_and_warn_the_user() {
        let mut r = receipt("닫았다: cycle:C3 · experiment · failure\n기록됨: journey:J9 (판은 늘지 않는다)\n\n다음\n  적어 둔 조상으로 되돌아간다 — 그 자리의 Artifact 세계가 함께 복원된다\n\n실행\n  gil revisit\n");
        after("gil_close", &mut r);
        let hint = text(&r);
        if cfg!(feature = "product-safety") { assert!(hint.contains("버튼을 기다린다"), "{hint}"); }
        else { assert!(hint.contains("gil_revisit(인수 없음)") && hint.contains("사용자에게 알린"), "{hint}"); }
    }

    #[test]
    fn an_unknown_receipt_shape_falls_back_to_the_generic_sentence() {
        let mut r = receipt("typed close receipt without the usual first line\n");
        after("gil_close", &mut r);
        assert!(text(&r).contains("다음 행동") && text(&r).contains("gil_status"), "{}", text(&r));
        let mut o = receipt("something else\n");
        after("gil_open", &mut o);
        assert!(text(&o).contains("gil_close"), "{}", text(&o));
    }

    #[test]
    fn opened_steps_say_wait_or_act_without_shortcuts() {
        for (kind, must, must_not) in [
            ("question", "턴을 끝내", "조사한다"),
            ("synthesis", "명시적 동의", "approved: yes로 바로"),
            ("verify", "evidence_files", "채팅만으로 닫"),
            ("define", "Verify에서 한다", "검색"),
            ("experiment", "gil_open(kind: define)", "verify를 먼저"),
        ] {
            let lead = if kind == "experiment" { "열었다: cycle:C2 · experiment · 부모 cycle:C1\n" } else { "열었다: step:C1/S1 · " };
            let mut r = receipt(&format!("{lead}{}\n\n지금 할 일\n  …\n", if kind == "experiment" { "" } else { kind }));
            after("gil_open", &mut r);
            let hint = text(&r);
            assert!(hint.contains(must), "{kind}: {hint}");
            assert!(!hint.contains(must_not), "{kind}: {hint}");
            assert!(hint.contains("gil_close") || kind == "experiment", "{kind}: {hint}");
        }
    }

    #[test]
    fn refusals_say_nothing_closed_and_never_claim_success() {
        let mut r = CallToolResult::error(vec![ContentBlock::text("Report 에 다음 칸이 있어야 한다".to_string())]);
        r.structured_content = Some(json!({"ok": false, "exit_code": 1, "said": "", "problem": "Report 에 다음 칸이 있어야 한다\n"}));
        after("gil_close", &mut r);
        let hint = text(&r);
        assert!(hint.contains("닫힌 것은 없다") && !hint.contains("이 닫기가 성공"), "{hint}");
        let mut start = CallToolResult::error(vec![ContentBlock::text("이미 프로젝트가 있다".to_string())]);
        after("gil_start", &mut start);
        assert!(text(&start).contains("gil_start를 반복하지 않는다") && !text(&start).contains("show_gil_monitor"));
        let mut plain = CallToolResult::error(vec![ContentBlock::text("x".to_string())]);
        after("gil_status", &mut plain);
        assert_eq!(text(&plain), "x", "unlisted refusals keep their exact text");
    }

    #[test]
    fn plan_hints_follow_the_recorded_decision_and_never_require_plans_on_legacy_projects() {
        let mut legacy = CallToolResult::structured(json!({"supported": false}));
        after("gil_plan_read", &mut legacy);
        assert!(text(&legacy).contains("승인 계획 프로필이 아니다") && !text(&legacy).contains("gil_plan_propose로 제안"), "{}", text(&legacy));

        let waiting = json!({"supported": true, "proposal": {"revision": 2, "receipt": null}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut w = CallToolResult::structured(waiting);
        after("gil_plan_propose", &mut w);
        let hint = text(&w);
        assert!(hint.contains("판 2") && hint.contains("턴을 끝내고 기다린다") && !hint.contains("gil_close(Synthesis)"), "{hint}");

        let approved = json!({"supported": true, "proposal": {"revision": 2, "receipt": {"decision": "approve", "source": "monitor_button"}}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut a = CallToolResult::structured(approved);
        after("gil_plan_read", &mut a);
        let hint = text(&a);
        assert!(hint.contains("close_report를 글자 그대로") && hint.contains("gil_open(kind: outcome)") && hint.contains("Interview Cycle Report"), "{hint}");

        let changes = json!({"supported": true, "proposal": {"revision": 3, "receipt": {"decision": "request_changes", "source": "monitor_button"}}, "pending": [], "reinterview_requests": [], "current_item": null});
        let mut c = CallToolResult::structured(changes);
        after("gil_plan_read", &mut c);
        assert!(text(&c).contains("approved: no") && text(&c).contains("이전 승인은 재사용되지 않는다"), "{}", text(&c));

        let bound = json!({"supported": true, "proposal": null, "pending": [{"id": "B", "question": "q", "done_when": "d"}], "reinterview_requests": [], "current_item": {"id": "A", "question": "q", "done_when": "d"}, "current_cycle": {"ref": "cycle:C2", "state": "open"}, "document": "# GIL approved plan"});
        let mut b = CallToolResult::structured(bound);
        after("gil_plan_read", &mut b);
        assert!(text(&b).contains("항목 A") && text(&b).contains("plan_document") && text(&b).contains("evidence_files"), "{}", text(&b));

        let blocked = json!({"supported": true, "proposal": null, "pending": [{"id": "B", "question": "q", "done_when": "d"}], "reinterview_requests": [{"cycle_ref": "cycle:C2", "reason": "scope"}], "current_item": null});
        let mut r = CallToolResult::structured(blocked);
        after("gil_plan_request_interview", &mut r);
        assert!(text(&r).contains("아무것도 닫히거나 열리지 않았다") && text(&r).contains("gil_open(kind: interview)"), "{}", text(&r));
        let mut r2 = CallToolResult::structured(json!({"supported": true, "proposal": null, "pending": [], "reinterview_requests": [{"cycle_ref": "cycle:C2", "reason": "scope"}], "current_item": null}));
        after("gil_plan_read", &mut r2);
        assert!(text(&r2).contains("재인터뷰 요청이 보존") && !text(&r2).contains("gil_open(kind: experiment)"), "{}", text(&r2));
    }

    #[test]
    fn wave2_closed_or_unknown_cycle_never_directs_work_into_its_old_verify() {
        for state in [Some("closed"), None] {
            for requested in [false, true] {
                let mut view = json!({"supported": true, "proposal": null,
                    "current_item": {"id": "A"}, "pending": [{"id": "B"}],
                    "reinterview_requests": if requested {json!([{"reason":"scope"}])} else {json!([])}});
                if let Some(state) = state { view["current_cycle"] = json!({"ref":"cycle:C2", "state":state}); }
                let mut result = CallToolResult::structured(view);
                after("gil_plan_read", &mut result);
                let hint = text(&result);
                assert!(hint.contains("gil_context"), "{state:?}, request={requested}: {hint}");
                assert!(!hint.contains("Verify에서") && !hint.contains("남은 Step"), "{hint}");
            }
        }
    }

}
