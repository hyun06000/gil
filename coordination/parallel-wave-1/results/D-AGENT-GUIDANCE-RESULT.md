# D — 가벼운 모델용 다음 행동 안내 · 결과

- branch: `claude/wave1-agent-guidance` (준비 commit `e89a32b` 위, 공통 기준 `fcd6e5d` 포함)
- 소유 파일만 수정했다. 공유 Core/wire/schema/`tools.json`/로드맵/CI는 건드리지 않았다.
- 이 문서의 시험은 **지침 회귀**다. 실제 Luna·Mac/Windows 설치본·Codex 화면 실행 결과가 아니며,
  합성 Monitor 응답(`decide_plan` fixture)은 실제 사람의 승인 증거가 아니다.

## 구현 범위

목표: 현재 기록 상태에서 **다음 호출(또는 기다림) 하나**를 먼저 말하고, 성공한 동작을 반복하지
않게 한다. 서버 계약은 그대로 두고 안내 원문만 바꿨다.

1. **Context 첫 블록 `[지금 바로 할 호출]`** — `src/context.rs`의 새 `next_call(&Project)`가
   preamble 바로 뒤에 한 줄을 쓴다. 기존 절(이전 Cycle·현재 Cycle·지금 자리·다음에 할 수 있는 것·
   닫는 데 필요한 칸·Report 원칙)과 `next_moves`는 그대로다. 판정은 `openable_here`/`can_close`/
   `pending_revisit`/`can_revisit`/`why_not_open_child`/Chain/`approved_plan` 기록에서만 읽는다.
   - 새 시작 → `gil_open(kind: question)`; Question/Interpretation/Synthesis 열림 → 사용자 답 대기 후
     `gil_close`; 승인된 Synthesis 닫힘 → `gil_open(kind: outcome)`; Outcome 닫힘 → Cycle Report
     `gil_close`; Interview 닫힘 → `gil_open(kind: experiment)`.
   - Experiment: Define → Hypothesis → **Verify `gil_open` 성공 전 조사 금지** → 파일 증거 +
     `evidence_files` → Analysis → Outcome → Cycle Report → 다음 Experiment/Interview.
   - 실패 Cycle: `gil_revisit` 한 번(사용자에게 복원 고지) → `gil_open`; 되돌아온 뒤 `gil_revisit` 반복 금지.
   - 승인 계획 프로필(기록에 `approved_plan`이 있을 때만): 제안 없음 → `gil_plan_propose`
     (scope 5칸 · items · synthesis · `exploration_checklist` 순서 · `approved`/`plan_revision`/
     `plan_digest` 금지); 응답 없음 → **턴을 끝내고 버튼 대기, 조사·보고서·재제안 금지**;
     approve → `close_report` 그대로 `gil_close`; request_changes → `approved: no`로 닫고 Question;
     Interview 닫힘 → 승인 판·다음 항목·질문·확인 조건; Verify 열림 → `plan_document` bytes 보존 +
     별도 `evidence_files`; 재인터뷰 요청 보존 → 현재 Cycle 사실대로 닫기 → Interview.
   - legacy 프로젝트의 Context에는 `gil_plan` 문구가 나오지 않는다(시험으로 고정).
2. **tool receipt 안내** — `src/mcp/guidance.rs`.
   - `gil_open`: 영수증 첫 줄의 종류별로 "기다림/행동" 한 줄(Question·Synthesis는 턴 종료 후 대기,
     Verify만 실제 작업, Define/Hypothesis는 조사 금지, Cycle 열림은 다음 Step 이름).
   - `gil_close`: 영수증 첫 줄 종류 + Core가 쓴 `다음` 절을 읽어 한 줄(`gil_open(kind: …)`,
     Cycle Report `gil_close`, Interview Cycle Report, `gil_revisit` 한 번, Chain Report, 막힘 →
     `gil_status`). 영수증 꼴을 알아보지 못하면 기존 일반 문장으로 되돌아간다. 상태를 다시 계산하지 않는다.
   - `gil_context`: 첫 블록 한 줄을 먼저 따르라고 안내. `gil_status`·`gil_revisit` 안내 추가.
   - 거절(`is_error`) 안내 추가: `gil_close`/`gil_open`/`gil_revisit`/`gil_start`/`gil_plan_*`에
     "아무것도 바뀌지 않았다 + 복구 호출 하나". 그 밖의 거절 본문은 글자 그대로 유지(시험으로 고정).
   - `gil_plan_propose`/`gil_plan_read`/`gil_plan_request_interview`: typed `structuredContent`
     (`supported`, `proposal.receipt.decision`, `pending`, `current_item`, `reinterview_requests`)로
     다음 호출을 고른다. `supported: false`면 `gil_plan_*`를 요구하지 않는다.
3. **공통 지침 정본** `src/mcp/workflow.md` — 상태→다음 호출 표(Interview 10행·Experiment 9행),
   §7 승인 계획(기록/`supported`/`tools/list`로 판단, OS 추측 금지, M6-D 제안 형식, 버튼 대기,
   `close_report` 그대로, 문서 bytes 보존 vs 실행 증거 성공/거절 예), §8 복구 표(시작 중복·응답 유실·
   승인 결과 불확실·승인 후 수정·재인터뷰·blocked revisit·증거 거절·Monitor 실패), "도구 반복으로
   사실 확인 금지 · '항상 동작' 보장 금지". §1 표시 규칙과 §4~6 증거/close 계약은 사실을 유지했다.
   `node plugins/gil-companion-prototype/sync-workflow.mjs --write`로 Skill 생성 영역을 갱신했다.
4. **Manual 설명** — `src/manual/topics/current.md` [흔한 실패]에 `gil start` 반복,
   `step/verify/close.md` [불변식]에 `plan_document`와 실행 증거 구분. router/schema/command 변경 없음.

## 변경 파일

- `src/context.rs` (+`next_call`, plan 보조 3개, approved-plan 전용 unit test 4개)
- `src/mcp/guidance.rs` (상태별 안내, 거절 안내, plan 안내, unit test 8개)
- `src/mcp/workflow.md` (정본), `plugins/gil-companion-prototype/skills/gil-companion/SKILL.md` (생성 영역만)
- `plugins/gil-companion-prototype/workflow.test.mjs` (+5 지침 회귀)
- `src/manual/topics/current.md`, `src/manual/topics/step/verify/close.md` (설명만)
- `tests/guidance_wave1.rs` (신규: in-memory 상태 fixture 6개 + 실제 `gil mcp --serve` receipt 1개)
- 이 결과 문서

## 검증

실행 명령과 결과는 아래 표와 같다. Rust는 전부 `python3 coordination/parallel-wave-1/cargo-queue.py --`
안에서 실행했고(공유 target `target/claude-wave-1`), JS는 독립 실행했다.
Plugin JS 시험을 위해 `npm ci --prefix plugins/gil-companion-prototype`(lockfile)를 실행했다.

| # | 명령 (큐 안: `python3 coordination/parallel-wave-1/cargo-queue.py -- …`) | 결과 |
|---|---|---|
| 1 | `cargo test --locked -p gil` (기본 프로필, AGENTS.md Core 검사) | 통과 · lib 600 passed / 1 ignored(기존), 모든 integration test 통과 |
| 2 | `cargo build --locked -p gil --all-targets` | 통과 (기존 `gil-launcher` warning 11개는 변경 전과 같음) |
| 3 | `cargo test --locked -p gil --test guidance_wave1 --test context --test mcp` | 통과 · guidance_wave1 7 / context 15 / mcp 33 |
| 4 | `cargo test --locked -p gil --lib mcp::guidance` | 통과 · 8 |
| 5 | `cargo test --locked -p gil --features chain-core --test context --test guidance_wave1 --test cycles` | 통과 · 15 / 7 / 25 |
| 6 | `cargo test --locked -p gil --features chain-core,visual-authoring,product-safety --test mcp --test guidance_wave1` | 통과 · 34 / 7 |
| 7 | `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib context::plan_guidance_tests` | 통과 · 4 (첫 두 번은 시험 자체의 `must_not` 부분 문자열이 "…하지 않는다" 문장에 걸린 false negative였고, 안내 원문은 바꾸지 않고 단정만 좁혔다) |
| 8 | `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib approved_plan` / `--lib mcp::guidance` | 통과 · 8 / 8 |
| 9 | `cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety` | 통과 |
| 10 | `node plugins/gil-companion-prototype/sync-workflow.mjs --write` 후 인자 없이 재실행 | 일치(stale 아님) |
| 11 | `node --test plugins/gil-companion-prototype/workflow.test.mjs` (JS만, 독립 실행) | 통과 · 7 |
| 12 | `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | **136 통과 / 1 실패** — `skill.test.mjs` 「Skill 이 부르는 도구 이름은 전부 실제 tool 표에 있다」(아래 남은 연결 1). 그 밖의 처음 실패 3건(`monitor.test.mjs`, `mcp-app/display`, `mcp-app/security-dependencies`)은 worktree에 `node_modules`가 없어서였고, lockfile대로 `npm ci --prefix plugins/gil-companion-prototype` · `npm ci --prefix mcp-app` 뒤 통과했다(준비 commit에서도 같은 원인으로 실패함) |
| 13 | `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | 통과 · 96 (처음 1건 실패도 mcp-app `esbuild` 미설치가 원인, 설치 후 통과) |
| 14 | `git diff --check` | 통과 |

실행하지 않은 것: 실제 Mac/Windows 설치본, Codex/Claude Desktop 화면, Luna 실사용, Companion/Tauri workspace,
Windows native 시험. 이 branch의 어떤 결과도 그것들의 통과를 뜻하지 않는다.

## 남은 연결 (통합 담당자)

1. **`plugins/gil-companion-prototype/skill.test.mjs` 「Skill 이 부르는 도구 이름은 전부 실제 tool 표에 있다」**
   가 실패한다. 공통 지침 §7이 `gil_plan_propose`/`gil_plan_read`/`gil_plan_request_interview`를
   backtick으로 부르는데 이 세 도구는 `approved-plan` feature의 `src/mcp/plans.rs` catalog에만 있고
   `tools.json`(20개)에는 없다. 이 시험은 다른 세션 소유가 아니므로 통합 단계에서 다음 예외를 추가한다
   (`gil_report_visuals` 예외와 같은 패턴):

   ```js
   if (name.startsWith("gil_plan_")) {
     // M6-D development profile (src/mcp/plans.rs catalog, feature approved-plan).
     // The shared workflow allows these only when the connected tools/list offers them.
     assert.ok(SKILL.slice(SKILL.indexOf("**Approved plans"), SKILL.indexOf("**Recover and keep")).includes(name));
     continue;
   }
   ```
   대안으로 §7에서 세 이름의 backtick을 빼면 시험은 통과하지만 문서 일관성이 깨지므로 권하지 않는다.
2. **CI 연결**: `.github/workflows/chain-core.yml`의 `--test context` 목록에 `--test guidance_wave1`을
   추가하면 chain-core 프로필에서도 같은 fixture를 돈다(기본 프로필은 `cargo test -p gil`이 포함한다).
   approved-plan 프로필은 `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib context::plan_guidance_tests`를 추가한다.
3. **공개 export(선택)**: `src/lib.rs`에 `pub use context::next_call;`를 더하면 Monitor/status가 같은
   한 줄을 쓸 수 있다. 지금은 `context()` 출력으로만 노출된다(mcp.rs 수정 없이 동작).
4. 설치본 반영은 별도다. 소스 변경은 기존 Mac/Windows/Codex 설치본을 갱신하지 않는다.
   실제 Luna 평가는 E의 시나리오와 통합 후 한 번의 설치본으로 수행한다.

## 발견한 결함 / 관찰 (범위 밖, 고치지 않음)

- `plugins/gil-companion-prototype/monitor.test.mjs`, `mcp-app/display.test.mjs`,
  `mcp-app/security-dependencies.test.mjs`, `distribution/compliance/notices.test.mjs`의 coverage 시험은
  worktree에 `node_modules`가 없으면 import 실패한다(준비 commit 기준 동일). 각 lockfile로 `npm ci` 후 통과했다.
  새 worktree 준비 절차에 두 `npm ci`를 적어 두면 같은 혼동을 막는다.
- `gil_close` 영수증의 `다음` 절은 Core(`src/command.rs`) 소유다. guidance는 그 첫 줄과 고정 문구만
  읽고, 못 알아보면 일반 문장으로 돌아간다. 영수증 문구를 바꾸면 `src/mcp/guidance.rs` unit test와
  `tests/guidance_wave1.rs`가 먼저 빨개진다.
- 승인 계획 프로필에서 Synthesis가 `approved: no`로 닫힌 뒤 Context 첫 줄은 일반 문법대로
  `gil_open(kind: question)`을 말한다. 수정 요청 내용은 `gil_plan_read`의 `proposal`에만 있다.
