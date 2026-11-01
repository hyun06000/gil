# B — 실제 Cycle 상태를 반영하는 Monitor (Wave 2 결과)

branch: `claude/wave2-monitor-state` (base `15d6329`, `197a402` 조상 확인). 작업일 2026-11-02.
commit SHA는 보고 메시지에 적는다. push/PR/병합/배포는 하지 않았다.

## 재현 (수정 전)

- `node coordination/parallel-wave-1/review/probes.mjs` → `{"finding":"C-closed-cycle", "actual":"current"}`.
  닫힌 Cycle에 남은 `current_item` 바인딩만으로 `planItems`가 A를 `current`(지금 열린 Cycle)로 분류했다.
- 수정 전 `src/mcp/plans.rs::view`의 응답에는 Cycle 열림 여부 필드가 없었다(`current_item`만 있음).
  `regressions.rs::review_closed_cycle_still_has_current_item_binding`이 보인 대로 Core는 Cycle을 닫아도
  바인딩을 유지하므로, 이 조회 모양에서 UI가 열림을 알 방법이 없었다.

## 수정

| 파일 | 내용 |
|---|---|
| `src/mcp/plans.rs` | `view`의 `supported:true` 응답에 read-only `current_cycle {ref, state}` 추가. `ref`는 `p.current_cycle().id().to_ref()`, `state`는 실제 `is_closed()`에 따른 `open`/`closed`. `current_item`·pending·executed_cycles·grant·입력·저장 의미는 그대로. |
| `src/mcp/plans.rs` (tests) | `approved_plan_view_projects_real_current_cycle_state_beside_the_binding`: 실제 Core로 Interview 승인 → Experiment 열림(`open`, current_item=A) → 5 Step 닫고 Cycle 닫음(`closed`, current_item=A 유지, pending/executed/active_revision 불변) 확인. mock 없음. |
| `mcp-app/plans.mjs` | `currentCycle(value)` 추가(`open`/`closed`/`unknown`, ref 문자열 검증). `planItems`: 바인딩 항목은 `open`일 때만 `current`, `closed`면 `executed`, 필드 없음/알 수 없는 값이면 새 상태 `bound`("현재 Cycle에 연결됨 (열림 여부 미확인)"). 닫힌 Cycle의 바인딩 항목이 executed_cycles에 없어도 목록에 남긴다. `executing` 화면 문장을 open/closed/unknown으로 분리하고, 닫힘 문장에 "여기서 성공으로 보지 않는다"를 명시. `counts`는 unknown일 때 "연결됨(열림 미확인) n개". 하단 안내에 "실행함은 열렸다는 기록, 성공은 Cycle Report가 진실원" 추가. 승인 token closure·trusted click·중복 클릭 차단·불확실 응답·재전송 분리·focus 보존 로직은 변경하지 않았다. |
| `mcp-app/plans.test.mjs` | 기존 executing/재승인 fixture에 실제 wire와 같은 `current_cycle` 추가. 신규 5개: 닫힌 Cycle(R3 fixture), 필드 없음/알 수 없는 state/null, 다음 Cycle 열림→닫힘 폴링, 닫힌 Experiment 뒤 재인터뷰 제안, idle 불변. 순수 표 시험에 open/closed/absent/unknown 네 경우와 `currentCycle` 검증 추가. |
| `mcp-app/monitor.css` | `li[data-status="bound"]`(점선, 강조 없음)과 `.plan-current[data-cycle=closed|unknown]` 색만 추가. |
| `plugins/gil-companion-prototype/assets/monitor.html`, `monitor.json` | `node mcp-app/build.mjs`로 재생성. 새 sha256 `1b29af14e7cb55045098cefab940cdefff0f5e8fabf8ef7550cf636517a8cccd` (이전 `6ab64173…`). |
| `spec/GIL_Approved_Plan_v0.1.md` | 읽기 응답의 `current_cycle` 설명 한 줄 추가(저장·승인·입력 의미 불변, 필드 없으면 UI는 열림을 추정하지 않음). |
| `coordination/parallel-wave-2/results/B-screens.mjs`, `B-screens.md` | 7개 화면(제안 없음·실행 항목·Cycle 닫힘·다음 Cycle 열림·재인터뷰·Closing 이후·옛 Core)의 fixture DOM 렌더 텍스트. 실제 Host 화면 인수가 아니다. |

## 자동시험

모두 `coordination/parallel-wave-2/cargo-queue.py`(worktree 전용 target)로 실행했다. 실행 중 디스크 여유 14→11 GiB.

| 검사 | 결과 |
|---|---|
| `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib mcp::plans` | **3/3 통과** (신규 projection 시험 포함) |
| 같은 프로필 `--lib` 전체 | 664 통과 / **12 실패** / 2 ignored. 실패는 전부 E 소유 baseline: `mcp::tests::the_complete_twenty_tool_surface_is_registered`(tool 표가 gil_plan_* 5개를 모름, `src/mcp.rs` cfg(test))와 `visual_migration::tests::*` 11개(`tests/common/mod.rs:401` 합성 bootstrap이 승인 프로필에서 거절됨). 이 worktree는 `src/mcp.rs`·`tests/common/mod.rs`·`src/visual_migration.rs`를 건드리지 않았다. 통합 대기. |
| 같은 프로필 `cargo build --locked -p gil --all-targets` | 통과 (gil-launcher 기존 경고 11개) |
| 기본 프로필 `cargo build --locked -p gil --all-targets` | 통과 |
| 기본 프로필 `cargo test --locked -p gil` 전체 | **전부 통과** (lib 652, tests/mcp.rs 25 — monitor.json sha256 ↔ monitor.html 일치 검사 포함, monitor 110 등; 0 실패) |

- JS: `mcp-app` `npm test` **74/74 통과** (plans 17개 포함). `node coordination/parallel-wave-1/review/probes.mjs`
  R3 재현은 수정 후 `planItems`가 이 fixture(필드 없음)를 `bound`로 분류하며, closed 필드가 있으면 `executed`다
  (`plans.test.mjs` 신규 시험에서 고정). probes.mjs 자체는 검토 자료라 수정하지 않았다.
- `plugins/gil-companion-prototype` `npm test`: **62 통과 / 1 실패**. 실패 `Skill 이 부르는 도구 이름은 전부 실제 tool 표에 있다 — gil_plan_read`는
  `skill.test.mjs`(E 소유)의 baseline 실패이며 이번 변경과 무관하다(Skill·catalog·tool 표를 수정하지 않음). 통합 대기.
- `node distribution/compliance/notices.mjs check` (수정하지 않음, 보고용):

```text
+     'plugins/gil-companion-prototype/assets/monitor.html': '1b29af14e7cb55045098cefab940cdefff0f5e8fabf8ef7550cf636517a8cccd'
-     'plugins/gil-companion-prototype/assets/monitor.html': '186713fc28dc63a14e73b9b4eb7909be72bc6e745238711e837bb1554bbbb54e'
      'plugins/gil-companion-prototype/assets/monitor.html': '1b29af14e7cb55045098cefab940cdefff0f5e8fabf8ef7550cf636517a8cccd'
        name: 'tokio-macros',
        source: 'https://crates.io/crates/tokio-macros/2.7.2',
        name: 'tokio-util',
        source: 'https://crates.io/crates/tokio-util/0.7.19',
        name: 'tokio',
```

- 번들 재생성 일치: `node mcp-app/build.mjs` 재실행 결과가 커밋된 `monitor.html`/`monitor.json`과 같다.

## 미검증 · 통합 대기

- **notice-policy hash**: `distribution/compliance/notice-policy.json`과 `distribution/windows/compliance/notice-policy.json`의
  `monitor.html` 항목은 수정하지 않았다(통합 담당 소유). baseline에서 이미 `186713fc…` ≠ 당시 번들 `6ab64173…`로 고지 검사
  2개가 실패했고, 이번 번들은 `1b29af14e7cb55045098cefab940cdefff0f5e8fabf8ef7550cf636517a8cccd`다. A/B 결과 뒤 통합 담당자가 갱신한다.
- 실제 Codex/Claude Desktop 화면 인수, 물리적 승인 클릭, Luna 실사용은 하지 않았다. B-screens.md는 fixture 렌더다.
- `current_cycle`은 C/D/E가 이번 작업에서 의존하지 않는 필드다. 결합 회귀(예: wire runner에서 닫힌 Cycle 조회 모양)는 합류 뒤 추가한다.
- 옛 Core(필드 없음) 경로는 UI 시험으로만 고정했다. 실제 옛 바이너리와의 혼합 설치는 시험하지 않았다.

## 남은 연결

- 통합: notice-policy 두 파일의 `monitor.html` hash를 위 값으로 갱신 → 고지 검사 재실행.
- E의 `skill.test.mjs` gil_plan_* 예외 처리 뒤 plugin suite 전체 녹색 확인.
- 합류 후 wire runner/scenario에 `current_cycle` 결합 회귀 추가 여부는 D/통합 담당 결정.
