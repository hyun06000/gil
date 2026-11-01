# E — 공통 승인 프로필 시험·도구 계약·CI · 결과

- branch: `claude/wave2-profile-tests` (base `15d6329`, 검토 커밋 `197a402` 조상 확인)
- 작업일: 2026-11-02 · 플랫폼: macOS arm64, Rust 1.97.1, Node 26.5
- 모든 Rust 실행은 `python3 coordination/parallel-wave-2/cargo-queue.py -- …`(worktree 전용 target)로 했다.
  JS 시험은 각 lockfile대로 `npm ci` 한 뒤 큐 밖에서 실행했다.
- push/PR/병합/CI dispatch/릴리즈/설치/실제 모델 호출은 하지 않았다. 실제 Windows CI 실행은 없었다.
- 생산 코드(handler, migration 구현, Core 승인 검사, 저장 형식, workflow 원문, 생성 Skill)는 수정하지 않았다.

## 1. 재현한 baseline 실패

| 명령 | 결과 |
|---|---|
| 큐: `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib` | **663 pass / 12 fail / 2 ignored** — 검토와 동일 |
| `node --test plugins/gil-companion-prototype/skill.test.mjs` | 「Skill 이 부르는 도구 이름은 전부 실제 tool 표에 있다」 1 실패 (`gil_plan_*` 가 기본 `tools.json` 에 없음) |

12개 lib 실패의 원인:

1. `mcp::tests::the_complete_twenty_tool_surface_is_registered` — 기대 목록에 approved-plan 의 5개 계획 도구가 없었다.
2. `visual_migration::tests::*` 11개 — fixture 가 `ProjectSession::start` 로 **승인 계획 프로필 프로젝트**를 만든 뒤
   `common::bootstrap_from` 의 맨 `approved: yes` 로 Synthesis 를 닫으려 했고 Core 가 거절했다
   (`승인 계획: gil_plan_propose로 제안한 뒤 Monitor 버튼 응답을 기다린다`). 설령 Synthesis 를 지나도
   chain 프로필의 새 프로젝트는 format 6/7 이라 `candidate()` 의 "only format 4" 거절에 걸린다 —
   즉 이 fixture 는 **format-4(legacy) 전용**이다. chain-core 만의 빌드에서도 같은 이유로 성립하지 않는다.

## 2. 수정

### `src/mcp.rs` cfg(test) — 작업 1

- `the_complete_twenty_tool_surface_is_registered` → `the_registered_tool_names_match_the_enabled_features_exactly`:
  기본 20개 + `visual-authoring` 5개 + (`visual-authoring`+windows) `gil_prepare_workspace` + `approved-plan` 5개를
  feature 별로 **정확히** 비교한다(초과·누락 모두 실패).
- 신규 `the_checked_in_default_catalog_never_lists_plan_or_visual_tools`: 체크인된 `tools.json` 자체가 기본 20개
  그대로이며 `gil_plan_*`/시각·이관 도구를 담지 않는다(기본 profile 표와 feature 표 혼동 방지).
- 신규 `plan_tools_are_registered_only_in_the_approved_plan_profile_with_their_visibility`: approved-plan 에서만
  5개 등록, `gil_plan_propose/read/request_interview` = `["model"]`, `gil_plan_review/decide` = `["app"]`;
  그 밖의 빌드에서는 `gil_plan_*` 0개.
- 신규 `default_surface_visibility_matches_the_checked_in_catalog`: 등록된 기본 도구의 visibility 가 파일 선언과
  같고, Monitor 읽기/보고 도구는 App 전용, 작업 도구는 모델 전용임을 고정.

### `src/visual_migration.rs` cfg(test) — 작업 2·3

- `mod tests`(format-4 리허설)는 `#[cfg(all(test, not(feature = "chain-core")))]` 로 적용 범위를 명시했다.
  근거를 모듈 주석에 적었다: chain 프로필은 format 4 를 만들 수 없고 이 이관은 format 4 만 받는다.
  fixture 안에 **legacy 임을 명시하는 단정** 2개를 추가했다(`current_chain().is_none()`, 저장 파일 `format: 4`).
- 신규 `chain_profile_tests` (`#[cfg(all(test, feature = "chain-core"))]`):
  `a_new_chain_profile_project_declares_a_chain_format_not_four`, `chain_format_projects_are_refused_before_backup_and_source_stays_unchanged`
  — chain/approved 프로필에서 precheck·prepare 가 "only format 4" 로 거절하고 backup 을 만들지 않으며 source 를 바꾸지 않음을 고정.
- 제품 승인 검사를 끄거나 기록을 자동 이전하지 않았다. guard 없는 skip/ignore 는 없다.
- 커버리지 유지 증명: 기본 프로필 전체 시험에서 format-4 리허설 11개가 그대로 돈다(§3).

### `tests/common/mod.rs` — 작업 3

- `bootstrap`/`bootstrap_from` 의 의미는 바꾸지 않았다. 적용 범위(맨 `approved: yes` 는 legacy/기본·chain-core 전용)와
  승인 프로필의 올바른 fixture 위치(crate 안 `approved_plan::tests`, stdio `tests/support_wave1`)를 문서화했다.
- `bootstrap_from` 은 승인 계획 상태가 붙은 프로젝트를 받으면 Core 의 거절 **전에** 명시적 메시지로 실패한다.
  (성공하던 경로는 그대로다. 통과시키기 위한 자동 approve 는 없다.)
- 신규 explicit 선택 helper: `approved_plan_profile()`, `bare_approval_is_the_contract()`.
  C/D 는 baseline API 로 독립 작업하므로 사용을 요구하지 않는다.
- 통합 시험 밖에서 승인 결정을 기록할 공개 API 는 없다(`decide_plan` 은 crate 내부·App 전용). 그래서 common 에
  승인 프로필 bootstrap 을 넣지 않았다 — 넣으려면 Core 의 승인 경계를 시험용으로 여는 셈이다(통합 담당 결정).

### `plugins/gil-companion-prototype/skill.test.mjs` — 작업 4

- `planCatalog()` 가 `src/mcp/plans.rs::catalog` 의 실제 항목(이름·app/model·readOnly)을 읽는다.
  파서 결과: propose(model), request_interview(model), read(model, read-only), review(app, read-only), decide(app).
- 도구 이름 검사: `gil_plan_*` 는 **정확한 이름**이 plan catalog 에 있고 `app:false` 일 때만 통과. 접두어 통과 없음.
  기본 `tools.json` 에 있으면 실패(기본 profile 부재 고정, 20개 유지).
- 신규 시험: App 전용 `gil_plan_review/decide` 가 Skill 어디에도 없고, 모델용 셋은 전부 capability 조건문
  (`gil_context` 승인 계획 프로필 / `gil_plan_read supported:true` / `tools/list` 제공) 아래에만 등장하며, OS/설치
  종류로 추측하지 말라는 문장·채팅 동의 금지·`approved`/`plan_*` 직접 기입 금지·`close_report` 그대로 사용을 고정.
- C 의 workflow 원문/생성 Skill 은 수정하지 않았다(`sync-workflow.mjs` 일치 검사 통과).

### `.github/workflows/chain-core.yml` — 작업 5 (검토용 YAML, dispatch 없음)

- pinned actions·permissions 유지. 게시 단계 없음.
- 경로 필터에 `plugins/gil-companion-prototype/{skills/**,*.mjs,assets/**}`, `mcp-app/**`, `ui/**`, `tools/scenarios/**`, `scenarios/**` 추가.
- 승인 프로필 `--lib approved_plan` → `--lib` 전체; `--test guidance_wave1`(C), `--test approved_plan_wire_wave1`(D) 추가;
  순수 Node 단계(`skill.test.mjs`, `workflow.test.mjs`, `tools/scenarios/*.test.mjs` 6개, npm 의존성 불필요) 추가.
- macOS/Windows 양쪽 matrix 에 적용된다. Windows 에서 D 의 wire 시험은 검토 5항(상태 경로 가정)이 고쳐질 때까지
  실패할 것이다 — 합류 대기. Windows CI 성공은 주장하지 않는다.

## 3. 검증

| 검사 | 결과 |
|---|---|
| 큐: approved 프로필 `cargo test … --lib` | **669 pass / 0 fail / 1 ignored** (신규 6개 포함; ignored 1 감소는 format-4 전용 `crash_child` 가 이 프로필에서 컴파일되지 않기 때문) |
| 큐: `--features chain-core --lib visual_migration` | 2 pass (chain 프로필 거절 시험) |
| 큐: `--features chain-core,visual-authoring,product-safety --lib mcp::` | 42 pass |
| 큐: approved 프로필 `--test guidance_wave1 --test approved_plan_wire_wave1` | wire **1 pass**; guidance **1 pass / 6 fail** (C 합류 대기, §4) |
| 큐: `cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety` | 통과 |
| 큐: 기본 `cargo test --locked -p gil` (전체) | **1508 pass / 0 fail / 2 ignored** (format-4 migration 리허설 11개 포함) |
| 큐: 기본 `cargo build --locked -p gil --all-targets` | 통과 |
| 큐: chain-core CI 단계(`--lib visual_start`, `--test mcp`, `--test visual_start`) | 통과 |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | **142 pass / 0 fail / 4 skipped** (baseline 1 실패 해소) |
| `node --test tools/scenarios/*.test.mjs` (CI 에 넣은 6개) | 21 pass |
| `node plugins/gil-companion-prototype/sync-workflow.mjs` | 일치 |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | 94 pass / **2 fail** / 1 skipped — notice-policy 해시(`mcp-app/build.mjs`, monitor.html). **통합 대기**, 무력화하지 않음 |
| `git diff --check` | 통과 |

### 승인 프로필 전체 integration 시험(작업 6, 미발견 실패 노출)

큐: `cargo test --locked -p gil --no-fail-fast --features approved-plan,visual-authoring,product-safety`
→ **1042 pass / 509 fail / 2 ignored**. lib·wire 는 녹색이고 실패는 전부 integration 시험이다. 원인 분류:

| 원인 | 건수(패닉 메시지 기준) | 성격 |
|---|---|---|
| `common::bootstrap_from` 맨 `approved: yes` → 승인 프로필 거절 (이번 fail-fast 메시지로 드러남) | 414 | 시험 fixture 가 legacy 전용. 통합 시험 밖에는 승인 결정 공개 API 가 없음 |
| CLI `gil close`/`restore` 실패 (`cli.rs`) | 54 | 같은 fixture 계열 + product-safety 의 `gil restore` 비활성 |
| `이 제품판에서는 gil restore 를 사용할 수 없다` (product-safety) | 19 | approved-plan 이 아니라 product-safety 와의 조합 |
| 저장 형식 가정(`저장 형식은 4 다`, `cycles.next_id`, YAML null index 등) | ~8 | format-4 가정 integration 시험 |
| C 의 guidance_wave1 (아래) | 6 | 합류 대기 |

실패 suite: artifact 18, chain_closing 7, chain_handoff 6, chains 5, cli 43, companion 17, context 14, cycle 15, cycles 23,
evidence 4, guidance_wave1 6, help_ref 12, lock 2, manual 4, mcp 11, monitor 97, project 9, restore 24, revisit 53,
session_boundary 8, status 5, store 58, story 16, visual_attachment 22, visual_migration 7, visual_project 7, visual_start 1,
visual_store 8, will 7. 이 전체 integration 묶음은 지금까지 어떤 CI 도 실행하지 않았고 이번 CI 에도 넣지 않았다.
**최종 통합이 합의할 "실패 없는 검사 범위"** 제안: 기본 프로필 전체 + chain-core CI 단계 + 승인 프로필 `--lib`·
`guidance_wave1`·`approved_plan_wire_wave1` + 순수 JS 계약. 승인 프로필 integration 전체를 범위에 넣으려면
legacy fixture 의 분리(명시적 legacy 생성 경로 또는 승인 경로 helper)가 Core/통합 소유 결정으로 먼저 필요하다.

## 4. 소유 밖 실패 (재현·영향)

1. **C — `tests/guidance_wave1.rs` 6 실패** (approved 프로필). 3건은 `common::bootstrap` 계열(이제 fail-fast 메시지),
   1건은 Project::start 후 맨 `approved: yes` close unwrap, 1건은 stdio receipt 의 `gil_close` 거절 비교,
   1건은 열린 Synthesis 안내문 기대 차이. 검토 1항 그대로이며 C 수정 뒤 재검증.
2. **통합 — notice-policy 해시 2 실패**: 그대로 두었다(README 지시).
3. **D — Windows wire 경로**: 로컬에서 실행 불가. CI 에 연결만 했다.
4. **chain-core 단독 빌드의 `--lib visual_migration`** 은 이번 수정 전에도 실패했을 것이다(format 6 → "only format 4").
   CI 가 `--lib visual_start` 만 돌려 드러나지 않았다. 지금은 chain 프로필 거절 시험 2개로 대체·고정됐다.

## 5. 남은 연결

- 승인 프로필 integration 전체 녹색화: legacy fixture 와 승인 fixture 의 분리 결정(통합/Core 소유).
- notice-policy 해시 갱신(A/B 결과 뒤 통합 담당).
- Windows runner 에서 신규 CI 단계의 실제 실행 결과 확인(실행 전 성공 주장 없음).
- 공간: 작업 시작 14 GiB → 종료 9.2 GiB 가용. 다른 worktree target 은 지우지 않았다.

## 6. 변경 파일

`.github/workflows/chain-core.yml`, `plugins/gil-companion-prototype/skill.test.mjs`, `src/mcp.rs`(cfg(test)),
`src/visual_migration.rs`(cfg(test)), `tests/common/mod.rs`, 이 문서.
