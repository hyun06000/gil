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

# 기준선: 저장소 빌드 큐 안에서 현재 소스를 빌드하고 그 실행 파일로 측정한다
python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/chain-context-bench/cli.mjs run --build --out <새 폴더>

# 이미 빌드한 실행 파일로 (기능 조합은 호출자가 선언한다)
node tools/chain-context-bench/cli.mjs run --gil <gil 경로> --features approved-plan,visual-authoring --out <새 폴더>

# 같은 조건의 두 결과 비교 (조건이 다르면 caution을 붙인다)
node tools/chain-context-bench/cli.mjs compare --before <a/bench.json> --after <b/bench.json> [--json <새 파일>]

# 자체시험 (Core 없음). 실제 Core 시험은 CHAIN_CONTEXT_BENCH_GIL=<gil 경로>일 때만 실행된다
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

`--build`는 큐가 설정하는 `GIL_PARALLEL_TARGET_DIR`가 없으면 거절한다(큐가 없는 환경에서만 `--unqueued`).
`--gil`로 받은 실행 파일은 sha256만 확인하며, 빌드 commit·기능은 검증하지 않았다고 결과에 적는다.

## 비교하는 축

모든 사례는 같은 **기준점**에서 축 하나만 늘린다. 기준점은 현재 Chain의 열린 Experiment에서
Define·Hypothesis를 닫고 Verify를 연 자리다(재개가 흔한 위치). 기준점 사례는 한 번만 생성해 모든 축이 공유한다.

| 축 | 늘리는 것 | 기본 level | 상한 |
|---|---|---|---|
| `chains` | 닫힌 이전 Chain (Opening Interview → Experiment 1개 → Closing Interview → Chain Report) | 0,1,2 | 6 |
| `cycles` | 현재 Chain에서 성공으로 닫힌 Experiment Cycle | 0,1,2 | 8 |
| `failures` | 같은 분기점에서 실패 → `gil_revisit` → 재시도로 쌓인 실패 Cycle | 0,1,2 | 8 |
| `steps` | 현재 Experiment에서 닫힌 Step 수(다음 Step이 열림, 4 = Outcome 열림) | 0,2,4 | 4 |
| `report_lines` | 현재 Cycle의 자유 서술 칸마다 줄 수(서로 다른 번호 문장) | 1,4,16 | 32 |
| `plan_items` | 승인 계획의 미실행 항목 수(현재 항목 포함) | 1,2,4 | 16 |

생성 경로: `gil_start`(locale) → Question·Interpretation → Synthesis에서 `gil_plan_propose` → App 경로의
`gil_plan_review`·`gil_plan_decide`(시험 harness의 **합성 승인**, 사람 승인이 아님) → `gil_plan_read`의 `close_report`로 닫기 →
Outcome·Cycle close → Experiment(Verify는 승인 문서 bytes와 별도 증거 파일) … 실패 Cycle은
`next_direction.action: revisit`으로 닫고 `gil_revisit`을 밟는다. `.gil`을 직접 쓰거나 Core가 거절할 상태를 만들지 않는다.
`product-safety` 프로필은 revisit을 거절하므로 `failures` 축은 `unsupported`로 기록된다(설계상 차단).

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
| `readonly` | 읽기 전후 프로젝트 폴더(`.gil` 포함)와 설정 폴더의 전체 내용 digest 비교 |
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

## 결과 파일

`--out`은 존재하지 않는 새 폴더여야 한다. 모든 파일은 배타적 생성(`wx`)으로 쓰며 기존 결과를 덮어쓰지 않는다.

- `bench.json` — 실행 조건(commit·dirty 항목·binary 위치와 sha256·선언/관측 기능·locale·읽기 호출·격리 방식), 계획,
  사례별 실행 결과, 축별 요약·증가 구간, 미완료 목록, 측정 한계. 사용자 홈·임시 경로는 표식으로 바꾸고 남아 있으면 쓰지 않는다.
- `summary.json` — 저장소에 남길 만한 축약본: 조건·요약·미완료 목록과 실행별 읽기 호출 크기·내용 hash. `compare` 입력으로 충분하다.
- `bench.md` — 같은 내용의 한국어 요약표. 응답 원문은 넣지 않는다.
- `recordings/<case>-r<n>.jsonl` — 읽기 호출의 가림 처리된 응답(context-metrics 입력 형식, 잘림 없음).
  `node tools/context-metrics/cli.mjs analyze --events <파일>`로 다시 읽을 수 있다.

상태 어휘: `complete` / `generation_failed`(어느 생성 단계에서 무엇이 거절됐는지) / `unsupported`(프로필이 막는 전이) /
`measurement_missing`(응답 없음·시간 초과·요청보다 적은 기록) / `record_changed_by_read` / `interrupted` / `not_run`.
SIGINT·SIGTERM을 받으면 Core 프로세스를 끝내고 임시 프로젝트를 지운 뒤 그때까지의 결과를 `interrupted`로 쓴다.
종료 코드: 0 모두 완료 · 1 미완료 실행 있음 · 2 사용법·입력 오류 · 130 중단.

## 측정 한계

- 토큰 수·Host의 모델 입력 구성·전체 비용은 관측하지 않는다. "읽기 text 합"은 고른 호출 text의 합일 뿐이다.
- 합성 기록이라 문장 길이와 항목 구성이 실제 작업과 다르다. 절대값보다 축별 증가 모양을 본다.
- 축은 하나씩 늘린다. 축 사이 상호작용(예: 이전 Chain이 많고 실패도 많은 경우)은 이번 기본 계획에 없다.
- 해상도 점검은 표식이 응답에 **문자열로** 있는지만 본다. 의미 보존·요약 품질은 판정하지 않는다.
- 단락 반복은 후보다. 고정 안내·상태별 안내가 필요한지는 판단하지 않는다.

## 하지 않는 것

`src/**`, UI, workflow, 명세, 로드맵, CI, `tools/context-metrics`, `tools/scenarios` 수정. source 저장소에서의 `gil start`,
실제 사용자 기록·설치본 사용, 모델 호출, push·병합·배포. 자동 검사 편입은 통합 담당의 판단이다.
