# E — 통합 시나리오·승인 경계 자동시험 결과

- branch: `claude/wave1-scenarios` (base `e89a32b`, 공통 코드 기준 `fcd6e5d` 포함 확인)
- 작업일: 2026-11-02 · 플랫폼: macOS arm64, Rust 1.97 toolchain(저장소 기준), Node 26.5
- 제품 source·기존 `tests/mcp.rs`·Cargo/Node 의존성 파일·`.github/**`·다른 세션 소유 파일은 수정하지 않았다.
- 모델 호출·VM·설치·배포·push·PR·병합은 하지 않았다. 모든 App 승인 입력은 harness의 합성 입력이며 사람의 동의나 화면 인수가 아니다.

## 1. 구현 범위

| 산출물 | 내용 |
|---|---|
| `scenarios/wave1/README.md`, `PROTOCOL.md`, `evaluation-sheet.md` | 시나리오 묶음 개요, 판정 어휘(pass/fail/unknown/expected_blocked/unsupported/not_run), control vs GIL 반복 실행 절차, `human.json` 양식 |
| `scenarios/wave1/{backend,data-science,frontend,planning-document}/` | 각각 `task.md`, `initial-artifacts/`, `human-checkpoints.md`, `acceptance.md`, `expected-evidence.md`, `rubric.json`. 정답 품질(answer_checks)과 GIL 절차(procedure)를 분리. Planning은 합성 출처 3개로 "Interview 마지막 노드에서 조사·채팅 전용 출력"을 실패로 잡는다 |
| `tools/scenarios/lib/rpc.mjs` | stdio JSON-RPC 클라이언트: 요청 timeout, 프로세스 그룹 종료, stdout 비-JSON 감지, 명시적 실행 파일만 사용(PATH/GIL_BINARY 불사용) |
| `tools/scenarios/lib/redact.mjs` | token(`_meta`·`token` 키·64hex secret)·절대경로·과대 문자열 가림, 누출 검사 |
| `tools/scenarios/lib/report.mjs`, `trace.mjs` | Report 텍스트 파서, tools/call 이벤트 재생(열림/닫힘 수신 문구, typed close `confirmed`, plan 조회, App/harness 결정, 재인터뷰, restore/revisit) |
| `tools/scenarios/lib/score.mjs` | 절차 판정 12개(Monitor 표시 미확인, 승인 없는 실행, Synthesis 열린 채 실행, 한 노드 여러 목적, 파일 증거 없음, 기존 승인 재사용, 재인터뷰 기록 손실, restore/revisit expected_blocked, 채팅 전용 산출물, Closing 전 Chain 종료)와 정답 검사(file/regex/final/command/human). 관측 없으면 unknown/not_run, 사람 칸은 `human.json`만 |
| `tools/scenarios/wire-runner.mjs` | 실제 `gil mcp --serve`를 큐 안에서 빌드 직후 실행하는 승인 경계 회귀 W01–W21 (아래 표). 임시 폴더만 조작, 가림된 frames/events JSONL + JSON/Markdown 결과 |
| `tools/scenarios/proxy.mjs` | 모델 평가용 MCP stdio proxy: 이벤트 기록, 시나리오 도구(read/save/허용 명령), App 전용 도구 숨김·거절, `--synthetic-app-decision`(시험 전용, harness 표기), `--control`(Core 없는 대조군) |
| `tools/scenarios/scorer.mjs` | events.jsonl + rubric(+프로젝트 폴더, human.json) → `score.json`/`score.md` |
| `tools/scenarios/*.test.mjs`, `fixtures/` | JS 전용 자체시험 21개(가짜 Core fixture 포함; 제품 증거가 아님) |
| `tests/approved_plan_wire_wave1.rs`, `tests/support_wave1/mod.rs` | `#![cfg(feature = "approved-plan")]` Rust 통합시험: 실제 stdio 프로세스로 같은 승인 계약 검사(재시작 포함) |

## 2. 검증 명령과 결과

모든 Rust/native 실행은 `python3 coordination/parallel-wave-1/cargo-queue.py -- …`로 공유 target(`target/claude-wave-1`)에서 수행했다.

| 명령 | 결과 |
|---|---|
| `node --test tools/scenarios/*.test.mjs` (JS 전용, 큐 밖) | 21 통과 |
| 큐: `node tools/scenarios/wire-runner.mjs --build` (features `approved-plan,visual-authoring,product-safety`, 같은 명령 안에서 빌드 후 즉시 실행) | **pass** — pass 20, expected_blocked 1, fail 0, not_run 0 (21 case, binary sha256 `ce8d446c8edc83dd…`, HEAD `e89a32b`, frames 166, events 163) |
| 큐: `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test approved_plan_wire_wave1` | 1 통과 (4.3s) |
| 큐: `cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety` | 통과 |
| 큐: `cargo build --locked -p gil --all-targets` | 통과 (기존 gil-launcher dead-code 경고 11개, 변경 없음) |
| 큐: `cargo test --locked -p gil` | 1446 통과, 0 실패, 2 ignored (기존과 동일한 ignored 2개) |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` (lockfile `npm ci` 후) | 128 통과, 4 skip (플러그인 번들 Core 필요 기존 시험) |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | 96 통과, 1 skip (실제 게시 archive 경로 필요 기존 시험) |
| `git diff --check` (신규 파일) | 통과 |

Windows 패키징 시험·실제 설치본·Codex 화면·Luna 사람 참여 시험은 수행하지 않았다(통합 후 항목).

### wire 회귀 case 표 (최종 실행)

| case | status | 내용 |
| --- | --- | --- |
| W01 | pass | tools/list exposes the approval contract with App/model visibility |
| W02 | pass | gil_start once, then the returned show_gil_monitor; display stays unconfirmed |
| W03 | pass | Interview Question → Interpretation → open Synthesis |
| W04 | pass | bare approved:yes close is refused; App review without a proposal carries no capability |
| W05 | pass | propose; read shows revision 1 without receipt; identical re-propose is idempotent |
| W06 | pass | App review returns the private capability only in _meta and writes nothing |
| W07 | pass | decide with another scope, a wrong token, or an extra field is refused/rejected |
| W08 | pass | editing the open proposal creates revision 2 and voids the earlier capability and close_report |
| W09 | pass | process restart keeps the scope, reissues the capability, refuses the old one |
| W10 | pass | duplicate approve is idempotent; flipping to request_changes is refused |
| W11 | pass | Synthesis closes only with the exact close_report; a tampered statement is refused |
| W12 | pass | Experiment is refused until Outcome and the Interview Cycle are closed, then binds item A |
| W13 | pass | Verify needs the bound plan document bytes plus separate fresh evidence |
| W14 | expected_blocked | gil_restore / gil_revisit are blocked in this profile (expected_blocked) — restore: 거절: 이 제품판에서는 `gil restore` 를 사용할 수 없다. \| revisit: GIL 거절: 되돌아가지 않았다. 이유가 dirty 세계·승인 계획 바인딩·적어 둔 방향 없음이면 그 조건을 사용자에게 알린다. gil_revisit을 반복하거나 restore를  |
| W15 | pass | reinterview request is preserved through the current Experiment and forces an Interview next |
| W16 | pass | request_changes closes with approved:no, re-proposal must keep approved items, then re-approval |
| W17 | pass | the inserted item D executes under revision 4 with the revision-4 document |
| W18 | pass | closing Interview needs its own approval; Chain closes; the next Chain cannot reuse it |
| W19 | pass | clean shutdown: EOF exits 0 and stdout carried only JSON-RPC frames |
| W20 | pass | proxy with explicit synthetic App decision: recorded as harness input, never as a person |
| W21 | pass | scorer replays this run: synthetic approval never scores as human consent or display |

가림된 실제 실행 산출물: `scenarios/wave1/runs/2026-11-02/` (`wire-run.json`, `wire-run.md`, `wire-events.jsonl`). 토큰은 `<redacted-token>`, 경로는 `<project_root>` 등으로 치환됐고 누출 검사를 통과했다.

### 실행 중 수정한 harness 결함 (제품 결함 아님)

1. harness가 미리 만든 0755 `GIL_MONITOR_STATE_DIR`을 Core가 비공개 디렉터리가 아니라고 거절해 Monitor 준비가 실패했다 → Core가 스스로 만들도록 `settings/bindings` 경로를 넘긴다(기존 `tests/mcp.rs`와 동일).
2. 영수증 없는 제안에는 `close_report`가 null이다 → "이전 판 승인 재사용" 검사는 1판 report를 harness가 직접 구성해 제출한다(Core가 거절함을 확인).
3. `close_report`는 모든 필드를 블록 형식(`approved: |\n  no`)으로 낸다 → 위조 fixture를 블록 형식으로 바꿨다. 그 전 실행의 "위조 승인이 통과" 표시는 치환이 안 돼 원본을 보낸 harness 오류였다.
4. typed Verify close 성공 응답에는 `닫았다:` 줄이 없다 → 재생기가 `structuredContent.confirmed`와 `visuals.owner.ref`로 닫힘을 인식한다.
5. Chain마다 판 번호가 1부터 다시 시작한다 → 결정의 오래됨 판정을 `synthesis_ref` 기준으로 바꿨다. 거절된 propose 응답은 plan 조회로 세지 않는다.

## 3. 발견 사항 (제품)

승인 경계 계약 자체에서는 실패를 찾지 못했다. bare `approved: yes`, 다른 scope, 틀린 token/digest, 추가 필드, 수정 후 이전 판 결정·report, 재시작 뒤 이전 capability, 중복 승인(멱등)·응답 뒤집기, 위조 report, Outcome/Cycle 닫기 전 Experiment, 문서 없음/문서만/틀린 bytes/증거 없음 Verify, 재인터뷰 대기 중 Experiment, 승인 항목 변경·삭제·체크리스트 순서 불일치, 이전 Chain의 digest/capability 재사용이 모두 거절됐고 거절 전후 `state.yaml` bytes가 같았다. restore/revisit는 product-safety·approved-plan 잠금으로 거절됐다(`expected_blocked`).

통합 담당자 검토 요청 사항(소유 파일 밖이라 고치지 않았다):

| # | 관측 | 재현 | 영향 | 제안 |
|---|---|---|---|---|
| O1 | typed Verify close(`expected_owner`+`visuals`) 성공 응답에 `said`/`닫았다: step:… · verify` 수신 줄과 다음 행동 안내가 없고 안내문이 "typed close라면 gil_status를 한 번 읽는다"로 대체된다 | wire 이벤트 중 `gil_close` with `expected_owner` 성공 응답 (`structuredContent`: `confirmed`, `report_sha256`, `visuals` 뿐) | 일반 close와 수신 형식이 달라 Monitor/안내/외부 채점기가 닫힌 step과 다음 행동을 응답만으로 알 수 없다. 모델이 한 번 더 `gil_status`를 읽어야 한다 | `src/mcp/authoring.rs` 성공 경로에 일반 close와 같은 `said` 수신(닫힌 step ref·다음 행동)을 포함 |
| O2 | `GIL_MONITOR_STATE_DIR`이 이미 0755로 존재하면 Monitor 준비가 "Monitor 연결 설정을 읽지 못했다"로 실패하고 원인(권한)이 드러나지 않는다 | 빈 0755 디렉터리를 가리킨 뒤 `gil_start` | 개발/CI harness에서 혼동. 제품 기본 경로에서는 Core가 직접 만들므로 영향 낮음 | 거절문에 "비공개 권한(0700) 필요"를 덧붙일지 검토 |
| O3 | `gil_plan_request_interview` 뒤 `gil_open experiment`는 "미실행 항목 없음"과 "재인터뷰 대기" 중 하나만 이유로 보여 준다 | W15 | 안내 품질 문제일 뿐 계약 위반 아님 | D(안내) 담당 참고 |

## 4. 남은 연결 (통합 담당자)

- CI: chain-core matrix에 `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test approved_plan_wire_wave1` 추가 검토. AGENTS.md JS 검사에 `node --test tools/scenarios/*.test.mjs` 추가 검토. 공용 target 없이 CI에서 wire runner를 돌리려면 `--gil <built path>` 또는 `--unqueued --build`.
- `tests/approved_plan_wire_wave1.rs`는 macOS/Linux 레이아웃(`.gil/state.yaml`가 project_root 아래)을 전제로 bytes 비교를 한다. Windows의 private app data 레이아웃은 `tests/mcp.rs`의 `product_state_path`와 같은 분기가 필요하며 이번에는 넣지 않았다(통합 시 공유 helper 재사용 여부 결정).
- 실제 모델 평가(PROTOCOL.md): Host에 `proxy.mjs`를 연결해 control/GIL arm을 각 3회 이상 실행하고 `scorer.mjs`로 채점. 비용·모델 선택은 사용자 승인 뒤.
- C(Monitor UX)·D(안내)와의 교차: O1의 typed close 수신, 승인 대기 표시는 각 결과 문서와 함께 통합에서 판단.

## 5. 하지 않은 것 / 한계

- 모델 실행 없음: 네 시나리오는 정의·fixture·평가표까지이며 모델로 돌린 결과는 없다(`not_run`).
- 합성 App 결정은 Core의 실제 App 경로(review→decide)를 통과하지만 사람 클릭이 아니다. `human_approval_real`·`monitor_display_confirmed`는 모든 자동 실행에서 unknown이다.
- 외부 도구 로그가 없는 실행에서는 `no_action_while_synthesis_open`·`one_purpose_per_node`가 unknown으로 남는다. 모델 내부 행동을 추정하지 않는다.
- 이미지 증거의 실제 표시·fullscreen·Companion은 범위 밖.
