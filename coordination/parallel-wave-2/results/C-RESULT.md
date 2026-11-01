# C — 합법적인 다음 호출과 승인 프로필 지침 시험 (Wave 2)

branch `claude/wave2-guidance` · base `15d6329` (조상 `197a402` 확인) · 전용 worktree에서만 작업.
push/PR/병합/CI dispatch/설치/실제 모델 호출은 하지 않았다. 시험 결과는 **안내 계약 검증**이며
작은 모델 실사용·실제 사람 승인·화면 표시의 증거가 아니다.

## 재현 (수정 전, 같은 commit)

승인 프로필 `approved-plan,visual-authoring,product-safety`, Wave 2 큐(worktree별 target).

| 재현 | 결과 |
|---|---|
| 원본 `tests/guidance_wave1.rs`(HEAD 사본) | **1 pass / 6 fail** — 검토 보고서의 "D 신규 시험 6개 실패"와 동일. 원인 전부 `Project::start`·MCP start로 만든 승인 프로필 프로젝트를 `approved: yes` fixture로 닫으려 함(`tests/common/mod.rs:401`, `guidance_wave1.rs:75/31/275`) |
| R4 Core 재현 `context::plan_guidance_tests::a_reinterview_request_is_preserved_without_hiding_the_legal_next_call` | FAIL — Define이 열린 채 재인터뷰 요청 후 첫 줄이 `재인터뷰 요청이 보존돼 있다. 현재 Step과 Cycle을 사실대로 닫는다.`로 고정되어 열린 Define의 close 안내를 가림 |
| R4 실제 binary 재현 `tests/guidance_wave2.rs` | FAIL — 같은 줄이 `gil_context`에서 나왔고, Define 닫힘 뒤 실제 허용 수(Hypothesis)와 어긋남 |
| 최종 항목 뒤 안내 `all_items_executed_leads_to_a_closing_interview_not_a_chat_report` | FAIL — `완료 보고는 사용자에게 한다`로 끝나 Closing Interview/Chain Report를 말하지 않음 |

## 수정

소유 파일만 변경. 도메인 전이·저장 형식·승인/증거 정책은 바꾸지 않았다.

- `src/context.rs`
  - `next_call` = 실제 위치의 합법적 호출(`next_call_here`) + 보존 알림(`reinterview_notice`).
    열린 Step이면 그 Step의 close, Step 사이면 `openable_here`(Define→Hypothesis, Hypothesis→Verify, Verify→Analysis…),
    `can_close`일 때만 Cycle Report. 알림은 "요청은 Cycle이 닫힌 뒤 Interview를 열어 다시 승인받는다 · 그때까지 보존된다"만
    말하고 `gil_open(kind: interview)`를 지금 호출로 제시하지 않는다. 취소/중단 호출(F의 D1/D2/D3)은 안내하지 않는다.
  - `plan_next_call`·`plan_next_call_between_steps`의 재인터뷰 override 제거(R4 원인).
  - 모든 항목 실행 뒤: `gil_open(kind: interview)`으로 Closing Interview → `chain_verdict` 승인 → Chain Report. "채팅 보고만으로 끝내지 않는다".
  - 본문 "보존된 재인터뷰 요청" 줄: "현재 Cycle을 사실대로 닫은 후"를 남은 Step을 실제 순서대로 마친 뒤로 교정.
- `src/mcp/guidance.rs`: `gil_plan_request_interview` 성공/거절 안내가 "현재 Step과 Cycle을 닫는다" 대신 "마지막 receipt의
  다음 행동을 이어간다 · 요청은 보존된다 · 취소/중단 호출은 없다"를 말한다. `gil_plan_read`의 재인터뷰·항목 소진 안내,
  Experiment Cycle 닫힘 receipt에 Closing Interview/Chain Report 문장 추가.
- `src/mcp/workflow.md`(정본) §3 표·§7·§8 표 갱신 → `sync-workflow.mjs --write`로 `SKILL.md` 재생성, 일치 검사 통과. OS별 규칙 분리 없음.
- `src/manual/topics/current.md` [흔한 실패] 1항, `step/verify/close.md` [불변식] 1줄(topic 60줄 한도 유지).
- 시험
  - `tests/guidance_wave1.rs`: in-memory 시험은 전부 **증명된 legacy fixture**(format 4로 다시 쓴 기록을 실제 store로 load,
    `chains().is_empty() && approved_plan().is_none()` 단언)를 쓴다. 프로필에 따라 다른 계약을 몰래 재지 않는다.
    stdio 시험은 `tools/list`와 `gil_plan_read.supported`로 프로필을 읽고 둘의 일치를 단언한 뒤, 승인 프로필에서는
    맨손 `approved: yes` 거절 → `gil_plan_propose` → `gil_plan_review`(_meta 능력) → `gil_plan_decide`(합성 Monitor 응답) →
    `close_report` 그대로 → Outcome → Interview Cycle → Experiment를 걷는다. feature off나 ignore로 통과시키지 않았다.
  - `tests/guidance_wave2.rs`(신규): R4를 실제 `gil mcp --serve`에서 재현/고정. 매 close 뒤 receipt의 `다음 행동`과 `gil_context`
    첫 줄이 같은 `gil_open(kind)`을 말하는지(`agree`), 재인터뷰 요청 뒤 Define close·Hypothesis·Verify 안내, 조기 Cycle Report
    거절, Interview 열기 거절을 검사. legacy 프로필에서는 같은 일치 검사 + 승인 도구 비언급을 검사(skip 아님).
  - `tests/guidance_support/mod.rs`(C 전용 helper): legacy fixture, 응답 30초 deadline·timeout/패닉/drop 시 child kill·stderr 수집,
    `env_clear` 후 필요한 변수만 전달(Windows: `LOCALAPPDATA`를 scratch로 격리, SystemRoot/USERPROFILE 등 보존), 기록은 임시 폴더만.
    `tests/common`은 `super::common`으로 공유(사본 두 개가 `first_world()` 폴더를 두고 경쟁하던 문제 제거).
  - `src/context.rs` Core 시험 2개 추가/교정(위 재현 항목). `tests/common/mod.rs`·`skill.test.mjs`·`src/mcp.rs`는 건드리지 않았다.

## 수정 후 시험 (Wave 2 큐, worktree별 target)

| 검사 | 결과 |
|---|---|
| 승인 프로필 `--lib plan_guidance_tests guidance::` | **13 pass / 0 fail** |
| 승인 프로필 `--test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1` | **7 / 1 / 1 pass, 0 fail** |
| chain-core `--test guidance_wave1 --test guidance_wave2 --test context --test manual` | 7 / 1 / 15 / 32 pass, 0 fail |
| 기본 `cargo test --locked -p gil` 전체 | 모든 target 통과, `tests/manual` 1 fail → 내 topic 추가로 60줄 초과였음. 1줄로 줄여 재실행 **32 pass / 0 fail**. 나머지 target은 변경 없음(lib 652 pass) |
| `cargo build --locked -p gil --all-targets` 기본 · 승인 프로필 | 둘 다 통과(기존 launcher dead-code 경고 11개) |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | **140 pass / 1 fail / 4 skipped**. 실패 1 = `skill.test.mjs` "Skill 이 부르는 도구 이름…"(검토 항목 3, E 소유, baseline과 동일) |
| `sync-workflow.mjs`(일치 검사) · `workflow.test.mjs` | 통과 · 7 pass |

### 내 수정이 고치지 않은 기존 실패 (통합 대기 · 다른 소유)

- 승인 프로필 `tests/context.rs` **14 fail**, `tests/manual.rs` **4 fail**: 모두 `tests/common::bootstrap`의 `approved: yes` 닫기
  거절(`common/mod.rs:401`) 또는 product-safety에서의 restore CLI 거절(`manual.rs:398/792`). E의 공통 fixture 정비·통합 계약 범위.
  검토 보고서 "전체 approved-plan 시험 12개 실패"와 같은 뿌리. 숨기지 않았고 수정하지 않았다.
- chain-core `--lib` `visual_migration::tests` **8 fail**(`NotFound` at `visual_migration.rs:825` 등): 수정 전 같은 commit에서
  stash 후 재실행해도 동일 8건 실패 → 내 변경과 무관. `runtime_launch` 1건은 lock 경합으로 한 번 실패 후 재실행에서 통과(flaky).
- 고지/패키징 2건 실패는 README대로 통합 담당 갱신 대기(이번에 실행하지 않음).

## 미검증

- Windows 실제 실행(`LOCALAPPDATA` 격리 경로는 코드로만 준비, macOS에서는 분기 미실행). Tauri/Companion 화면. 실제 Codex/Luna/작은 모델 실사용.
- Verify close까지의 stdio 승인 흐름은 E의 `approved_plan_wire_wave1`(통과)에 맡기고 wave2는 Verify 열기까지만 검사했다.

## 남은 연결 (통합 담당)

- `tests/common::bootstrap`에 명시적 legacy/승인 fixture 분리(E) → 승인 프로필 `tests/context.rs`·`manual.rs` 재검증.
- `skill.test.mjs`가 feature별 catalog를 대조하도록(E). 고지 해시 갱신(A/B 뒤). F의 중단 계약(D1/D2/D3) 확정 전까지
  context/guidance는 "취소/중단 호출은 없다"고 안내한다 — 확정되면 `reinterview_notice`와 workflow §7 한 문장을 바꾼다.
