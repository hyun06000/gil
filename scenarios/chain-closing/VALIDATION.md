# 검증표와 실행 기록

2026-11-17, macOS arm64, Node v26.5.0. 기준 `bcefbc4`, 별도
`codex/chain-closing-evaluation` worktree에서 실행했다. 아래 수치는 새 오프라인 도구와
재사용 라이브러리의 자체시험 결과이며 GIL 제품의 사용자 경험 합격 결과가 아니다.

## 기존 검사 대비 추가 범위

| 기존 검사/근거 | 기존 검사의 책임 | 이 묶음이 더 보는 것 | 이번 실행 |
|---|---|---|---|
| `tests/chain_closing.rs` | 정확한 종료 Synthesis·verdict·저장·중복 close·dirty 경계 | 그 종료 전에 공개한 결과와 질문 순서·승인 대상의 일치 | 읽기 검토, Core 재실행 not_run |
| `tests/chain_handoff.rs` | 직렬 새 Chain·전역 ID·세계·과거 PNG 증거 보존·새 Interview | 과거 증거와 현재 파일을 구분하는 설명, 인계 내용·새 목표 | 읽기 검토, Core 재실행 not_run |
| `tests/mcp.rs`의 closing/handoff 재시작 시험 | close/open 응답 유실 이후 실제 stdio 조회·중복 방지 | pending/saved 재개 메시지가 실제 조회와 일치하는가 | 매핑만, native stdio not_run |
| `tools/scenarios/wire-runner.mjs` W18 | Closing Interview 자체 승인·Chain close·새 Chain에서 이전 승인 거절 | 도구 기록에는 없는 공개 결과·한계, 실제 파일 관측과 비교 | 읽기 검토, W18 실행 not_run |
| 기존 `tools/scenarios` JS | trace·승인 연결·redaction·proxy·RPC 라이브러리 | 같은 사건 envelope, `okOf/saidOf/parseReport/table` 읽기 전용 import | 아래 64건에 포함 |
| 기존 `tools/chain-interview-eval` | 질문·proposal·Define 구조/결과 검사 | 종료 제시·파일 현재성·재개·인계 전용 schema | 아래 64건에 포함 |
| 새 `chain-closing-eval` | 10사례·12 좋은 예시·14 실패 예시의 구조 및 결과 무결성 | 8개 자동 기준 / 7개 별도 의미 기준 | 49건 통과 |
| 실제 모델·Host·사용자 필드 테스트 | 대표 결과 이해·한계 충실성·보이는 화면·열람·인간 승인 | 동일 사례·조건으로 나중에 반복 | 전부 not_run |

Core 시험은 추가하지 않았다. 현재 부족한 것은 사용자 공개 출력과 증거의 연결이며,
기존 Core 불변식을 다시 구현할 필요가 없기 때문이다. 따라서 Cargo 빌드·Core 실행·빌드 큐
점유·새 stdio 실행은 하지 않았다. 나중에 native 검사를 실행할 때는 반드시
`coordination/parallel-wave-2/cargo-queue.py`와 독립 임시 프로젝트를 사용한다.
기본 Core 성공을 제품 프로필·Tauri/Companion·실제 Host 성공으로 확대하지 않는다.

## 실행한 명령

| 명령 | 결과 |
|---|---|
| `node tools/chain-closing-eval/build-fixtures.mjs` | 합성 10사례·결과 예시 3개 결정적 생성. 모델/Core 호출 없음 |
| `node --test tools/chain-closing-eval/*.test.mjs` | **49 passed**, 0 failed/skipped |
| `node --test tools/chain-interview-eval/*.test.mjs tools/scenarios/*.test.mjs` | **64 passed**, 0 failed/skipped. mock 프로세스 시험 포함, 실제 native Core 아님 |
| `node tools/chain-closing-eval/check.mjs fixtures` | 사례 무결성 오류 0, 종료 0 |
| `node tools/chain-closing-eval/check.mjs result --result scenarios/chain-closing/results/examples/not-run.json` | 종료 0, 의미 70 not_run, 모델/Host not_run |
| `node tools/chain-closing-eval/check.mjs result --result scenarios/chain-closing/results/examples/unknown-synthetic.json` | 종료 0, 의미 7 unknown + 63 not_run, 모델/Host not_run |
| `node tools/chain-closing-eval/check.mjs result --result scenarios/chain-closing/results/examples/invalid-overclaim.json` | **기대한 종료 1**. 미실행 pass·근거/판정자 누락 거절 |
| `git diff --check` | whitespace 오류 없음 |

자체시험에서 잘못된 JSON·없는 파일·인수 누락·출력 덮어쓰기는 종료 2,
근거 bundle 누락·판정 위조·사례 누락은 종료 1, unknown/not_run 결과는 종료 0을 실제 subprocess로
확인했다. 별도의 고정 기대값으로 실패 유형을 검사하며, 생성기의 자동 판정을 그대로 정답으로
채택하지 않는다. 첫 41건 통과 후 typed Verify·새 관측·첫 메시지·파일 항목·JSON 키 순서와
미지원 영수증 형식·App review scope·닫기 영수증 판정 일치 검사를 추가해 최종 49건을 통과했다. 시험 실패를 숨긴 재실행은 없다.

파일 이름만 나열, 과도한 완성 주장, 접근 성공을 꾸민 문장은 자동 구조 검사를 통과할 수 있다.
이를 좋은 UX라고 집계하지 않는다. 의미 평가자는 원문·실제 결과를 대조해야 한다.

## 변경 파일과 인계

변경은 아래 두 새 디렉터리에만 있다.

- `scenarios/chain-closing/`: README.md, rubric.json, rubric.md, cases/CC-01.json … CC-10.json,
  PROTOCOL.md, VALIDATION.md, PR-DRAFT.md, results/FORMAT.md,
  results/examples/{not-run,unknown-synthetic,invalid-overclaim}.json.
- `tools/chain-closing-eval/`: README.md, check.mjs, build-fixtures.mjs, eval.test.mjs,
  lib/evidence.mjs, lib/package.mjs.

제품 코드·UI·workflow·기존 도구·규범 명세·공통 로드맵·CI는 수정하지 않았다.
실제 모델 호출·사용자 테스트·병합·CI dispatch·설치·배포는 하지 않았다.
평가 작성 후 별도 사용자 요청으로 이 변경의 커밋·브랜치 push·PR 게시를 진행한다.
PR 설명은 [PR-DRAFT.md](PR-DRAFT.md), 실제 평가 절차는 [PROTOCOL.md](PROTOCOL.md)다.

## PR 인계 시 확인한 제품 프로필 제한

`tests/chain_closing.rs`·`tests/chain_handoff.rs`의 공통 bootstrap/closing helper는
bare `approved: yes` fixture를 사용한다. `tests/common/mod.rs::bootstrap_from`은
승인 계획이 있는 새 `approved-plan` 프로젝트에 이 fixture를 쓰면 명시적으로 거절한다.
따라서 Chain 프로필 회귀를 제품 프로필(`approved-plan,visual-authoring,product-safety`)
회귀 통과로 간주할 수 없다. 제품 프로필은 기존 `approved_plan_wire_wave1`,
`support_wave1`, W18의 실제 App 승인 경로를 따로 실행해야 한다. 이번 작업은
해당 native 실행을 하지 않았으며 기존 시험의 프로필이나 skip 조건을 수정하지 않았다.
