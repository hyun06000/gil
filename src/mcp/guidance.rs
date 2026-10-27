//! Short host guidance shared by Plugin and direct-MCP installations.
//! These are instructions, not new Project facts or structured receipt fields.
use rmcp::model::{CallToolResult, ContentBlock};

// The Plugin skill embeds this exact text; CI rejects a stale generated copy.
pub const INSTRUCTIONS: &str = include_str!("workflow.md");

pub fn prepend(result: &mut CallToolResult, text: impl Into<String>) {
    result.content.insert(0, ContentBlock::text(text.into()));
}

pub fn after(name: &str, result: &mut CallToolResult) {
    if result.is_error == Some(true) { return; }
    let hint = match name {
        "gil_open" => "GIL 진행: 아래 '지금 할 일'만 수행한다. 완료 조건을 충족하면 관측한 결과로 gil_close한 뒤 다음 행동으로 간다. 다른 목적의 일을 이 노드에 계속 붙이지 않는다. Interview는 목표 합의까지만 한다. 조사·계산·보고서 작성(채팅 전달도 포함)·구현·테스트는 Experiment의 Verify에서 계약 범위만큼 수행한다.",
        "gil_close" => "GIL 진행: 이 닫기가 성공했다. 다음 작업 전에 아래 영수증의 다음 행동을 따른다. 다음 안내가 없는 typed close라면 gil_status를 한 번 읽는다. 기록되지 않은 다음 단계를 먼저 수행하지 않는다.",
        "gil_context" => "GIL 재개: 아래 현재 노드와 행동 계약을 이어간다. 완료 조건을 충족했다면 추가 작업보다 gil_close가 먼저다. 승인받은 Synthesis는 닫고, Interview Outcome의 판정도 닫은 뒤, Interview Cycle Report를 별도로 닫는다. 이미 닫힌 단계를 반복하지 않고 실제 위치의 다음 안내를 따른다. 조사·채팅 보고서도 열린 Interview에서 수행하지 않는다. 이미 존재하는 프로젝트에 gil_start를 다시 부르지 않는다.",
        "show_gil_monitor" => "GIL 진행: Monitor 표시를 요청했다. 신규 시작 흐름이면 같은 project_root로 첫 Interview의 gil_open을 호출하고 질문한다. 기존 프로젝트라면 현재 열린 단계를 이어간다. 실제 화면 표시 여부는 아직 확인되지 않았다.",
        _ => return,
    };
    prepend(result, hint);
}
