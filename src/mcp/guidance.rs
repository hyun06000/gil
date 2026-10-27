//! Short host guidance shared by Plugin and direct-MCP installations.
//! These are instructions, not new Project facts or structured receipt fields.
use rmcp::model::{CallToolResult, ContentBlock};

pub const INSTRUCTIONS: &str = "GIL workflow: use only the user's selected project_root. Read the tool's text guidance as well as structuredContent. After successful gil_start, follow its Monitor handoff before opening the first Interview step. Never repeat start to open a screen. If start succeeded but Monitor preparation failed, report the display failure and continue the recorded Interview via gil_open; display failure alone does not block Project work. Work in small action contracts: open → perform that action → close with observed results → follow the next receipt. Interview is conversation, not implementation; only Verify permits deliverable edits and tests. Once done_when is met, the next GIL action is gil_close. A new user request does not extend an already completed contract: close the completed action first, then handle the new request at the next appropriate step. Multiple edits/tests within one unfinished Verify are valid. Synthesis needs the user's reply to the proposed statement before approved: yes. On a failed Project action, resolve the refusal before further Project work. In a new session read gil_context once and resume the recorded step. Report the current step and next action briefly to the user; do not fabricate past steps, hidden reasoning, or screen visibility.";

pub fn prepend(result: &mut CallToolResult, text: impl Into<String>) {
    result.content.insert(0, ContentBlock::text(text.into()));
}

pub fn after(name: &str, result: &mut CallToolResult) {
    if result.is_error == Some(true) { return; }
    let hint = match name {
        "gil_open" => "GIL 진행: 아래 '지금 할 일'만 수행한다. 완료 조건을 충족하면 관측한 결과로 gil_close한 뒤 다음 행동으로 간다. 다른 목적의 일을 이 노드에 계속 붙이지 않는다. Interview에서는 구현하지 않고, Verify에서는 계약 범위의 구현·테스트를 함께 할 수 있다.",
        "gil_close" => "GIL 진행: 이 닫기가 성공했다. 다음 작업 전에 아래 영수증의 다음 행동을 따른다. 다음 안내가 없는 typed close라면 gil_status를 한 번 읽는다. 기록되지 않은 다음 단계를 먼저 수행하지 않는다.",
        "gil_context" => "GIL 재개: 아래 현재 노드와 행동 계약을 이어간다. 완료 조건을 충족한 뒤 닫고 다음으로 간다. 이미 존재하는 프로젝트에 gil_start를 다시 부르지 않는다.",
        "show_gil_monitor" => "GIL 진행: Monitor 표시를 요청했다. 신규 시작 흐름이면 같은 project_root로 첫 Interview의 gil_open을 호출하고 질문한다. 기존 프로젝트라면 현재 열린 단계를 이어간다. 실제 화면 표시 여부는 아직 확인되지 않았다.",
        _ => return,
    };
    prepend(result, hint);
}
