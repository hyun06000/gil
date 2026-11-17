# tools/chain-context-bench — Chain·Cycle·실패 규모별 문맥 전달량 벤치

GIL 기록이 길어질 때 **새 세션의 읽기 호출이 받는 문맥이 어떻게 늘어나는지**를 재현 가능하게 측정하는
개발용 도구다. 실제 Core(`gil mcp --serve`)의 공개 MCP 도구만으로 규모가 통제된 임시 프로젝트를 만들고,
새 Core 프로세스에서 `gil_context` 등 읽기 호출을 한 번씩 보내 그 응답을 [`tools/context-metrics`](../context-metrics/README.md)의
공개 함수로 측정한다. 측정 도구를 다시 만들지 않으며, 제품 context를 줄이거나 재개 안내를 고치지 않는다.

- 크기는 UTF-8 바이트·코드 포인트·UTF-16 단위다. **토큰 수가 아니다.**
- 모델을 호출하지 않는다. 전체 실행 비용·재조회·작업 정확성 향상은 측정하지도 주장하지도 않는다.
- 측정값에 따른 성공·실패 기준은 없다. 결과는 기준선 기록일 뿐 CI 판정이 아니다.

## 실행

```sh
# 계획만 보기 (Core 실행 없음): 사례·예상 호출 수
node tools/chain-context-bench/cli.mjs plan [--axes chains,failures] [--levels failures=0,1,2] [--repeat 2]

# 기준선: 현재 소스를 빌드하고 그 실행 파일로 측정한다
node tools/chain-context-bench/cli.mjs run --build --out <새 폴더>

# 이미 빌드한 실행 파일로 (기능 조합은 호출자가 선언한다)
node tools/chain-context-bench/cli.mjs run --gil <gil 경로> --features approved-plan,visual-authoring --out <새 폴더>

# 제품 구성: 보호 기능을 켠 채 빌드하고 측정한다 (실패 뒤 복구는 아래 "실패 뒤 복구 경로")
CARGO_TARGET_DIR=<전용 출력 폴더> node tools/chain-context-bench/cli.mjs run --build \
  --features approved-plan,visual-authoring,product-safety --out <새 폴더>

# 같은 조건의 두 결과 비교 (조건이 다르면 caution을 붙인다)
node tools/chain-context-bench/cli.mjs compare --before <a/bench.json> --after <b/bench.json> [--json <새 파일>]

# 자체시험 (Core 없음). 실제 Core 시험은 아래 환경 변수로 실행 파일을 줄 때만 실행된다
#   CHAIN_CONTEXT_BENCH_GIL=<approved-plan,visual-authoring로 빌드한 gil>
#   CHAIN_CONTEXT_BENCH_GIL_PRODUCT=<approved-plan,visual-authoring,product-safety로 빌드한 gil>
node --test tools/chain-context-bench/*.test.mjs
```

| 옵션 | 기본값 | 제한 |
|---|---|---|
| `--axes` | 여섯 축 모두 | 아래 축 이름 |
| `--levels axis=n,n,…` | 축별 기본값 | 축별 상한, 정수만 |
| `--repeat` | 2 | 1–3 |
| `--reads` | `gil_context,gil_status,gil_plan_read` | `gil_context`·`gil_status`·`gil_plan_read`·`gil_story`·`gil_help`만 |
| `--features` | `approved-plan,visual-authoring` | `approved-plan` 필수, 알려진 제품 기능만 |
| `--locale` | `ko-KR` | `gil_start`에 그대로 전달 |
| `--timeout-ms` | 20000 | 1000–120000, 요청 하나마다 |
| `--max-calls` | 3000 | 최대 10000. 예상 호출이 넘으면 Core를 띄우기 전에 거절 |
| `--keep` / `--no-recordings` / `--work-root` | 끔 / 기록 저장 / OS 임시 폴더 | 임시 프로젝트 보존, 응답 기록 생략, 임시 폴더 위치 |

`--build`는 일반 Cargo 환경 변수를 따른다. 빌드를 공유할 때는 별도 출력 폴더나 선택적 로컬 큐를 사용한다.
추적되지 않은 큐 스크립트는 필수 의존성이 아니다. 기존 `--unqueued` 옵션은 호환을 위해 허용한다.
`--gil`로 받은 실행 파일은 sha256만 확인하며, 빌드 commit·기능은 검증하지 않았다고 결과에 적는다.

통합 검토에서 실제 저장 레코드 폴더도 읽기 전후 비교하도록 보완했다. 프로젝트 밖 레코드나
설정이 바뀌어도 성공으로 표시하지 않는다. 과거 결과에 레코드 관측이 없으면 전체 불변 여부는
미확인으로 표시한다. 기존 Mac 기준선 파일은 당시 관측 이력으로 보존한다.
자체시험의 가짜 실행 파일·신호 시험은 Unix 전용이므로 자동 검사는 Mac에서 실행한다.
제품 안전 구성(`product-safety`)의 누적 실패 사례는 2026-11-17부터 Mac에서 측정한다(아래).
Windows 실제 실행은 여전히 별도 범위이며, Mac 결과를 Windows 실행 성공으로 읽지 않는다.

## 비교하는 축

모든 사례는 같은 **기준점**에서 축 하나만 늘린다. 기준점은 현재 Chain의 열린 Experiment에서
Define·Hypothesis를 닫고 Verify를 연 자리다(재개가 흔한 위치). 기준점 사례는 한 번만 생성해 모든 축이 공유한다.

| 축 | 늘리는 것 | 기본 level | 상한 |
|---|---|---|---|
| `chains` | 닫힌 이전 Chain (Opening Interview → Experiment 1개 → Closing Interview → Chain Report) | 0,1,2 | 6 |
| `cycles` | 현재 Chain에서 성공으로 닫힌 Experiment Cycle | 0,1,2 | 8 |
| `failures` | 같은 분기점에서 실패 → 복구(Core가 내놓는 경로) → 재시도로 쌓인 실패 Cycle | 0,1,2 | 8 |
| `steps` | 현재 Experiment에서 닫힌 Step 수(다음 Step이 열림, 4 = Outcome 열림) | 0,2,4 | 4 |
| `report_lines` | 현재 Cycle의 자유 서술 칸마다 줄 수(서로 다른 번호 문장) | 1,4,16 | 32 |
| `plan_items` | 승인 계획의 미실행 항목 수(현재 항목 포함) | 1,2,4 | 16 |

생성 경로: `gil_start`(locale) → Question·Interpretation → Synthesis에서 `gil_plan_propose` → App 경로의
`gil_plan_review`·`gil_plan_decide`(시험 harness의 **합성 승인**, 사람 승인이 아님) → `gil_plan_read`의 `close_report`로 닫기 →
Outcome·Cycle close → Experiment(Verify는 승인 문서 bytes와 별도 증거 파일) … 실패 Cycle은
`next_direction.action: revisit`으로 닫고 아래 복구 경로를 밟는다. `.gil`을 직접 쓰거나 Core가 거절할 상태를 만들지 않는다.

### 실패 뒤 복구 경로

닫힌 실패 Cycle에서 분기점으로 돌아가는 방법은 **Core가 도구 목록에 내놓은 것**으로 정한다. 기능 선언이 아니라 관측으로 고른다.

| Core의 도구 목록 | 경로 (`generation.recovery.path`) | 밟는 호출 | 실패 하나당 호출 |
|---|---|---|---|
| `gil_revisit_review`·`gil_revisit_apply` 있음 (`product-safety` 구성) | `app_approval` | `gil_revisit_review` → `gil_revisit_apply` | 13 + 2 |
| `gil_revisit`만 있음 (보호 없는 개발 구성) | `model_revisit` | `gil_revisit` | 13 + 1 |
| 둘 다 없음 | `none` | — (`unsupported`) | — |

제품 구성은 모델의 `gil_revisit`을 거절하고 Monitor의 승인을 요구한다([M4-R](../../spec/GIL_Revisit_Recovery_v0.1.md)).
이 도구는 보호 기능을 끄거나 거절 조건을 완화하지 않고, [상태별 재개 기준선 시험](../chain-resume-benchmark/README.md)과 같은
기존 App 전용 계약을 그대로 쓴다. `gil_revisit_review`가 돌려준 계획의 `digest`와 비공개 승인 정보(`_meta`)를
`gil_revisit_apply`에 그대로 넣는다. 계획이 `awaiting_approval`이 아니거나 실패·대상 Cycle이 방금 닫은 것과 다르면 승인을 보내지 않는다.

- 이 승인은 **시험 harness의 합성 입력**이다. 사람이 Monitor 버튼을 누른 것이 아니며 결과는 `synthetic_approvals`와
  `human_approvals: 0`으로 적는다. 화면 표시·사람의 판독·실제 Host 버튼의 증거가 아니다.
- Core는 App 전용 도구로 들어온 승인을 `source.json`에 `monitor_button`으로 적는다. 이 벤치에서 그 입력은 harness에서 왔으므로
  결과는 그 값을 `core_source_records`(Core가 적은 것)로만 옮기고 사람 클릭으로 세지 않는다. `--keep`으로 남긴 임시 프로젝트도 인수 증거가 아니다.
- 승인 정보(토큰)는 요청에만 쓰고 결과·기록 파일·오류 문구에 남기지 않는다. 읽기 응답이나 결과에 들어 있으면 쓰지 않고 실패한다.
  저장 레코드에 들어 있는지도 읽기 전용으로 확인한다(`capability_in_storage`).
- 새 복구 정책·자동 승인 기능을 만들지 않는다. `recovery-auto-approval` 구성의 자동 승인 진입은 이 도구가 쓰지 않는다.

## 측정

각 실행은 독립 임시 폴더(HOME·LOCALAPPDATA·Monitor 설정 격리)에서 한 Core 프로세스로 기록을 만든 뒤 **새 Core 프로세스**
(새 세션)에서 읽기 호출을 한 번씩 보낸다. 그 응답만 측정한다.

| 결과 필드 | 뜻 |
|---|---|
| `reads[].text_response` / `structured_response` | text content와 structuredContent(키 정렬 JSON)를 **따로** 잰다. 더하지 않는다 |
| `said_in_text` | structuredContent.said가 text에 그대로 들어 있는지(같은 내용의 두 표현) |
| `raw_text_utf8_bytes` | 경로 가림 전 text 크기(가림 뒤 크기와 다르면 경로가 응답에 있었다는 뜻) |
| `context_sections` | `gil_context` text를 Core locale 표(`src/i18n/<lang>.json`)의 절 제목으로 나눈 크기. 본문 앞의 receipt 안내는 `text_part_N` |
| `resolution` | Report마다 넣은 `ccbT-…` 표식이 어느 응답에 도착했는지로 본 해상도 점검(아래) |
| `blocks` | context-metrics의 단락 단위로 본 한 응답 안의 동일 단락 반복과 같은 틀(번호·표식만 다른) 단락 |
| `metrics` | context-metrics `analyzeRecording` 결과 요약: 상태별 수, 호출 사이 동일 단락 반복 후보 |
| `readonly` | 읽기 전후 프로젝트·설정·실제 저장 레코드 폴더의 전체 내용 digest 비교(외부 저장 위치 포함, 승인 복구의 보존본 포함) |
| `generation.recovery` | 실패 뒤 복구 경로, 복구 횟수, 합성 승인 수, 복구마다 실패·대상 Cycle과 변경 수(create/replace/preserve). 파일 이름·승인 정보는 없다 |
| `lineage` | 실패 0·1·2…회 뒤 새 세션에 남아 있는지 본 보존 점검(아래) |
| `storage.preserved` | 승인 복구가 남긴 `revisit-preserved/<상태 해시>/`의 승인 기록·원본 목록·출처 기록 유무와 원본 bytes의 SHA-256 일치 수 |
| `conditions.recovery` | 실행 전체의 복구 경로·횟수·합성 승인 수, 선언한 기능 조합이 가리키는 경로와 관측한 경로의 일치 여부 |
| `repeat` | 같은 조건 재실행의 tool별 크기·내용(Monitor scope id만 정규화) 일치 여부, context-metrics `compareAnalyses` 합계 |
| `generation` / `storage` | 생성 호출 수·거절 수·소요 시간, `.gil` 파일 수·바이트 |

해상도 점검의 기대(Context Model §2·§5·§7, M6-C):

| 계층 | `gil_context` 기대 |
|---|---|
| 이전 Chain의 Chain Report 인계 | 있음 |
| 이전 Chain 내부(Cycle·Step·Cycle Report) | 없음 |
| 이전 Cycle의 Define 목적·Outcome 교훈·Cycle Report 인계 | 있음 |
| 이전 Cycle의 Hypothesis·Verify·Analysis·Interview Step | 없음 |
| 누적 실패 Cycle의 같은 투영 | 모두 있음 |
| 누적 실패 Cycle의 내부 Step | 없음 |
| 현재 Cycle의 닫힌 Step Report(가드레일 포함) | 모두 있음 |
| 이번 Experiment의 승인 항목 | 있음 |
| 미실행 항목 본문·승인 범위 제약 | 관측만: 읽기 호출 중 **어딘가**에는 있어야 한다 |

보존 점검(`lineage`)은 생성한 사례가 아는 사실과 새 Core 프로세스의 읽기 응답을 맞춰 본다. 크기와 무관하다.

| 점검 | 보는 것 |
|---|---|
| `failure_refs` · `failure_tags` | 실패 Cycle마다 그 참조(`cycle:Cn`)와 투영 표식 네 개(Define 목적·교훈·인계·revisit 이유)가 `gil_context` text에 있다 |
| `failure_section` | 실패가 있을 때만 실패 Cycle 절이 있다(실패 0회 사례에 없는 실패가 나타나지 않는다) |
| `current_item` | `gil_plan_read`의 현재 항목·현재 Cycle이 생성한 사례와 같다 |
| `retry_links` · `retry_links_in_context` | `gil_plan_read.executed_cycles`에 실패한 시도와 현재 시도가 **같은 항목**으로 모두 남아 있고, 그 참조가 `gil_context`에도 있다 |

해당 읽기 호출을 요청하지 않았거나 응답이 없으면 `not_measured`이며 통과로 세지 않는다. `gil_status`가 마지막 실패 Cycle을
가리키는지는 관측만 한다(`status_names_revisit_source`). 표식·참조의 존재만 보며 의미 보존은 판정하지 않는다.

## 결과 파일

`--out`은 존재하지 않는 새 폴더여야 한다. 모든 파일은 배타적 생성(`wx`)으로 쓰며 기존 결과를 덮어쓰지 않는다.

- `bench.json` — 실행 조건(commit·dirty 항목·binary 위치와 sha256·선언/관측 기능·locale·읽기 호출·격리 방식), 계획,
  사례별 실행 결과, 축별 요약·증가 구간, 미완료 목록, 측정 한계. 사용자 홈·임시 경로는 표식으로 바꾸고 남아 있으면 쓰지 않는다.
- `summary.json` — 저장소에 남길 만한 축약본: 조건·요약·미완료 목록과 실행별 읽기 호출 크기·내용 hash. `compare` 입력으로 충분하다.
- `bench.md` — 같은 내용의 한국어 요약표. 응답 원문은 넣지 않는다.
- `recordings/<case>-r<n>.jsonl` — 읽기 호출의 가림 처리된 응답(context-metrics 입력 형식, 잘림 없음).
  `node tools/context-metrics/cli.mjs analyze --events <파일>`로 다시 읽을 수 있다.

상태 어휘: `complete` / `generation_failed`(어느 생성 단계에서 무엇이 거절됐는지) / `unsupported`(이 Core·OS가 제공하지 않는 전이) /
`measurement_missing`(응답 없음·시간 초과·요청보다 적은 기록) / `record_changed_by_read` / `interrupted` / `not_run`.

`unsupported`와 실행 실패는 섞지 않는다. 미완료 실행의 `error.kind`가 이유를 가른다.

| `error.kind` | 상태 | 뜻 |
|---|---|---|
| `unsupported_condition` | `unsupported` | Core가 승인 복구를 이 OS에서 제공하지 않는다고 답했거나(`supported: false`), 복구 도구가 없거나, 선언한 `product-safety` 구성인데 승인 복구 도구가 없는 Core가 `gil_revisit`을 거절했다 |
| `core_refusal` | `generation_failed` | Core가 호출을 거절했다(예: 기록되지 않은 파일 변경으로 복구 계획 거절, 승인 거절, 보호 없는 구성의 `gil_revisit` 거절) |
| `harness` | `generation_failed` | 응답은 왔지만 진행에 필요한 것이 없다(참조·승인 정보 없음, 승인 대기 상태가 아님, 다른 Cycle의 계획) |
| `transport` | `generation_failed` | 시간 초과·프로세스 종료 |
SIGINT·SIGTERM을 받으면 Core 프로세스를 끝내고 임시 프로젝트를 지운 뒤 그때까지의 결과를 `interrupted`로 쓴다.
종료 코드: 0 모두 완료 · 1 미완료 실행 있음 · 2 사용법·입력 오류 · 130 중단.

## 측정 한계

- 토큰 수·Host의 모델 입력 구성·전체 비용은 관측하지 않는다. "읽기 text 합"은 고른 호출 text의 합일 뿐이다.
- 합성 기록이라 문장 길이와 항목 구성이 실제 작업과 다르다. 절대값보다 축별 증가 모양을 본다.
- 축은 하나씩 늘린다. 축 사이 상호작용(예: 이전 Chain이 많고 실패도 많은 경우)은 이번 기본 계획에 없다.
- 해상도 점검은 표식이 응답에 **문자열로** 있는지만 본다. 의미 보존·요약 품질은 판정하지 않는다.
- 단락 반복은 후보다. 고정 안내·상태별 안내가 필요한지는 판단하지 않는다.
- 복구 승인은 합성 입력이다. 실제 Monitor 화면·사람의 변경 목록 확인·Host 버튼·복구 뒤 채팅 이어가기는 측정하지 않는다.
- 복구는 닫힌 실패 Cycle에서 같은 분기점으로 돌아가는 한 가지 모양만 만든다. 보호 경로 차이·미기록 변경·중단 뒤 재개 같은
  거절·복구 사례는 Core 시험(`tests/`)의 범위이며 여기서 다시 만들지 않는다.
- 제품 구성과 보호 없는 구성은 도구 수와 안내 문구가 달라 크기가 다르다. 두 결과를 개선 전후로 비교하지 않는다(`compare`가 조건 차이를 표시한다).
- 결과는 `environment`에 적힌 OS·CPU에서의 실행 기록이다. Mac 결과는 Windows 실행 성공이 아니다.

## 기준선

| 폴더 | 구성 | 실패 뒤 복구 |
|---|---|---|
| [`baseline/2026-11-17`](baseline/2026-11-17/REPORT.md) | Mac · `approved-plan,visual-authoring` (source `bcefbc4`) | `gil_revisit` |
| [`baseline/2026-11-17-product-safety`](baseline/2026-11-17-product-safety/REPORT.md) | Mac · `approved-plan,visual-authoring,product-safety` | App 승인 복구(합성 승인) |

과거 기준선은 그 시점의 관측 이력이며 다시 쓰지 않는다. 새 결과는 실행 환경·소스·기능 조합이 드러나는 새 폴더에 둔다.

## 하지 않는 것

`src/**`, `tests/**`, UI, workflow, 명세, 로드맵, CI, `tools/context-metrics`, `tools/scenarios`, `tools/chain-resume-benchmark` 수정.
제품 보호 기능 끄기, Core 거절 조건 완화, 새 복구 정책·자동 승인 기능. source 저장소에서의 `gil start`,
실제 사용자 기록·설치본 사용, 모델 호출, 병합·설치·배포. 자동 검사 편입은 통합 담당의 판단이다.
