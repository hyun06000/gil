# D — 승인 평가 정확성 및 Windows 시험 격리 (결과)

branch `claude/wave2-scenario-validation`, 출발점 `15d6329`(`197a402` 포함). 소유 파일만 수정했다.
제품 Core(`src/**`), `tests/common/**`, 다른 lane 파일, 기존 `scenarios/wave1/runs/2026-11-02/`는 손대지 않았다.
모델 호출·push·PR·병합·설치·릴리즈는 하지 않았다.

## 재현한 원래 결함

| 항목 | 재현 |
|---|---|
| R5 채점기 Chain 경계 | `node coordination/parallel-wave-1/review/probes.mjs`: 수정 전 `approval_before_execution: pass`, 수정 후 `fail` ("no proposal in Chain #2; only an earlier Chain's approval (revision 1) exists") |
| 수정 전 승인 | 기존 `trace-score.test.mjs` 위반 fixture가 superseded rev1 영수증을 pass로 셌다 → 이제 fail |
| 기록 경로 하드코딩 | `tests/approved_plan_wire_wave1.rs:19` `".gil/state.yaml"`, `wire-runner.mjs:26` `STATE_PATH` |
| null 통과 | `sameState(before, state()) !== false`: 기록이 없으면 `null !== false`로 통과 |
| 환경 | JS/Rust 모두 실제 `HOME`을 자식에 넘겼고 Windows 필수 변수·LOCALAPPDATA 격리 없음; Rust `read()`는 `read_line` 블로킹이라 30초 timeout이 실제로 작동하지 않음 |

## 수정

### 1. 승인 연결 채점 (`tools/scenarios/lib/score.mjs`, `lib/trace.mjs`)
- `trace.mjs`: 제안·영수증·Synthesis close·Cycle/Chain 이벤트에 `chainIndex`를 붙인다. 응답 없는 decide는 `answered:false`로 남기고 영수증이 아니다.
- `approvalLink(facts, open)` (export): Experiment open마다 **같은 Chain**에서 (a) approve 영수증이 open 전에 관측된 제안, (b) 그 Synthesis의 최신 판(수정으로 superseded면 무효), (c) 그 판으로 `approved: yes` 닫힌 Synthesis, (d) 닫힌 Interview Cycle, (e) Experiment 안의 `gil_plan_read`가 그 판을 `active_revision`으로, `current_item`을 바인딩으로 보여줌 — 모두 있어야 pass. 이전 Chain 승인·request_changes·응답 유실·다른 active 판·바인딩 없음은 fail. Synthesis close/제안이 로그에 없거나(중간 시작) Experiment 안 조회가 없으면 **unknown**.
- `human_approval_real`: 사람 기록이 클릭을 주장해도 로그의 결정이 전부 합성이면 fail, 결정이 하나도 없으면 unknown. 합성 입력은 어떤 경우에도 pass가 아니다.
- `no_reused_approval`: 판·digest·Chain·영수증 시점을 모두 대조한다. `closing_interview_before_chain_close`: 자기 Chain의 종료 Synthesis와 그 자체 영수증을 요구한다.
- fixture: `tools/scenarios/fixtures/approval/NN-<기대>-<사례>.jsonl` 8개(양성 2 Chain, 이전 Chain 승인, 수정 전 승인, request_changes, 응답 유실, 중간 시작, 조회 없음, 다른 active 판) + `approval-link.test.mjs` 12개 시험(정상 다중 Chain·중복 decide·유실 뒤 read 복구 포함).
- `scorer.mjs`/`wire-run.json`: `claims.core_contract`(Core 거절 관측)와 `claims.model_task`(모델 평가; wire runner는 항상 not_run)를 분리해 적는다.

### 2. 기록 경로·환경 격리
- `tools/scenarios/lib/state-path.mjs`: `src/project_state.rs::state_for_root`와 `tests/mcp.rs::product_state_path`의 계약을 그대로 옮겼다. Windows: `LOCALAPPDATA\GIL\project-state-v1\<sha256(UTF-16LE canonical, `\\?\` 접두 포함)>\.gil\state.yaml`, 기존 external → 기존 inline → external 순. 그 외: `<root>/.gil/state.yaml`.
- `lib/rpc.mjs::isolatedEnv`: PATH는 시스템 폴더만, HOME(Unix)/USERPROFILE·LOCALAPPDATA(Windows)는 작업 폴더 아래 격리 폴더, Windows는 SystemRoot 등 필수 변수만 복사, 필수 변수 없으면 실패. uncaughtException/unhandledRejection에서도 자식 process group을 정리한다. `proxy.mjs --private-root`로 proxy가 띄우는 Core도 같은 격리를 받는다.
- `wire-runner.mjs`: 기록이 없으면 `state()`가 던진다(“변경 없음” 비교에 두 snapshot 필수). W02는 제품 배치 경로에 기록이 있는지, Windows면 `record_storage: private_app_data`인지, 비Windows면 LOCALAPPDATA 저장소가 생기지 않았는지 확인한다. W21은 승인 연결 detail이 합성임을 말하고 사람 기록의 클릭 주장이 fail로 끝나는지도 확인한다.
- `tests/support_wave1/mod.rs`: `product_state_path(root, local_app_data)`, `record_bytes` (없으면 panic), `isolated_command` (env_clear + 격리 HOME/USERPROFILE/LOCALAPPDATA + Windows 필수 변수), reader thread + `recv_timeout`(30초)로 실제 timeout, EOF 대기도 bounded. `tests/approved_plan_wire_wave1.rs`: STATE 상수 제거, 시작 뒤 계약 경로에 기록이 있는지 확인.

## 시험 결과 (Mac, darwin-arm64, 이 worktree의 Wave 2 큐 target)

| 검사 | 결과 |
|---|---|
| `node --test tools/scenarios/*.test.mjs` | **37 pass / 0 fail** (approval-link 12, state-path 4 신규) |
| `node coordination/parallel-wave-1/review/probes.mjs` (R5) | `approval_before_execution: fail` (기대 fail) — R3 항목은 B 소유, 여전히 `current` |
| wire runner W01–W21, `approved-plan,visual-authoring,product-safety` | **20 pass / 1 expected_blocked / 0 fail**, 격리 HOME, record layout inline → `scenarios/wave1/runs/2026-11-02-wave2-D/` |
| wire runner W01–W21, `approved-plan,product-safety` (plain Verify close) | **20 pass / 1 expected_blocked / 0 fail** (plain text Verify close) → `scenarios/wave1/runs/2026-11-02-wave2-D-plain/` |
| `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test approved_plan_wire_wave1` | **1 passed / 0 failed** (4.95s; 격리 env, reader thread timeout, 계약 경로 기록 확인 포함) |

산출물은 runner의 누출 검사(토큰·절대경로)를 통과한 뒤에만 기록됐고 `wire-frames.jsonl`은 크기 때문에 커밋하지 않았다.
기록된 `HEAD`는 실행 당시 worktree HEAD(`15d6329`)이며 이 커밋 자체가 아니다(커밋 전에 실행). 재현 명령은 각 `wire-run.md`.
전체 `cargo test` / JS 전체 묶음 / 고지 검사는 D 범위가 아니라 돌리지 않았다; 통합 담당자가 합류 뒤 다시 돌린다.

## 미검증·한계

- **Windows native 실행 없음.** 이 Mac에서는 `state-path.mjs`와 Rust `product_state_path`의 **경로 문자열 계약**만 시험했다(`\\?\` 접두 UTF-16LE sha256, 선택 순서). 실제 `LOCALAPPDATA` 아래 기록 생성·`record_storage: private_app_data`·DACL은 Windows 호스트/CI(E 소유)에서 확인해야 한다. `windowsCanonical`의 UNC(`\\?\UNC\`) 분기는 실제 경로로 확인하지 못했다.
- 비visual Windows 프로필의 기록 위치: `tests/mcp.rs` helper는 inline, `state_for_root`는 cfg(windows)에서 external을 선호한다. `state-path.mjs`는 존재하는 쪽을 따르므로 둘 다 다루지만 실제 어느 쪽에 생기는지는 Windows에서만 알 수 있다.
- 사람 클릭·Monitor 표시·모델 작업 평가는 이번에도 전부 `unknown`/`not_run`이다. 모든 App 결정은 합성 harness 입력이다.
- 채점기는 로그에 있는 것만 본다. Experiment 안에 `gil_plan_read`가 없는 실행은 이제 pass 대신 unknown이 되므로, 모델 평가 프로토콜은 Experiment를 연 뒤 한 번 읽도록 안내해야 한다(PROTOCOL/Skill 문구는 C/통합 소유).

## 남은 연결 (통합 담당자)

- E의 Windows CI에서 `approved_plan_wire_wave1`과 `wire-runner.mjs --features ...`를 실행해 위 한계를 닫는다.
- B의 `current_cycle` 필드가 합류되면 `one_purpose_per_node`·바인딩 판정에 닫힌 Cycle 구분을 더할 수 있다(이번에는 의존하지 않았다).
- `scenarios/wave1/runs/2026-11-02/`(Wave 1 기록)는 그대로 두었다. 새 기록은 `-wave2-D`, `-wave2-D-plain` 폴더다.
