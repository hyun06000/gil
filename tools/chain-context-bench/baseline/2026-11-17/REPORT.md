# Chain context 기준선 — 2026-11-17

[`tools/chain-context-bench`](../../README.md)로 만든 첫 기준선이다. 실제 Core 공개 MCP 경로로 규모가 통제된 합성
프로젝트를 만들고, 새 Core 프로세스(새 세션)의 읽기 호출 응답을 [`tools/context-metrics`](../../../context-metrics/README.md)
함수로 측정했다. 크기는 바이트(B)·코드 포인트이며 **토큰 수가 아니다.** 모델을 호출하지 않았으므로 전체 실행 비용·
재조회·작업 정확성에 대해서는 아무것도 주장하지 않는다. 임계값·CI 기준도 만들지 않는다.

같은 폴더의 [`bench.md`](bench.md)는 도구가 생성한 전체 표, [`summary.json`](summary.json)은 `compare` 입력용 축약본이다.
읽기 호출의 응답 원문(`recordings/`)과 큰 `bench.json`은 저장소에 넣지 않았다.

## 재현

```sh
python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/chain-context-bench/cli.mjs run --build --out <새 폴더>
node tools/chain-context-bench/cli.mjs compare --before tools/chain-context-bench/baseline/2026-11-17/summary.json --after <새 폴더>/summary.json
```

| 조건 | 값 |
|---|---|
| source | `bcefbc4` (미추적 파일은 `tools/chain-context-bench/` 아래 10개뿐, `src/` 변경 없음) |
| Core | 이 실행이 빌드 큐에서 `cargo build --locked -p gil --features approved-plan,visual-authoring`로 빌드한 debug `gil` · sha256 `eda298b6…d7874` |
| 관측 | tools 31개, plan 도구·`gil_revisit` 있음 (`product-safety` 없음: 실패 축에 revisit이 필요하다) |
| locale | `ko-KR` (`gil_start` 인자) |
| 측정 호출 | 새 Core 프로세스에서 `gil_context`·`gil_status`·`gil_plan_read` 각 1회 |
| 규모 | 13개 사례 × 2회 반복 = 26개 독립 임시 프로젝트, 생성 호출 960 + 읽기 78 = 1038 (예상과 같음), 27초 |
| 환경 | macOS arm64, Node v26.5.0 |

## 기준점과 축별 측정

기준점(모든 축의 출발점): 이전 Chain 0 · 닫힌 Experiment 0 · 실패 0 · 현재 Experiment의 Define·Hypothesis 닫힘, Verify 열림 ·
Report 칸당 1줄 · 승인 항목 1개. 이때 `gil_context` text 6230 B(3606자), structuredContent 5246 B, `gil_status` text 1476 B,
`gil_plan_read` text 3350 B, `.gil` 11362 B.

| 축 | level | context text B | context 문자 | context structured B | gil_status B | gil_plan_read B | 읽기 text 합 B | .gil B | 생성 호출 |
|---|---|---|---|---|---|---|---|---|---|
| 이전 Chain | 0 / 1 / 2 | 6230 / 6403 / 6574 | 3606 / 3720 / 3832 | 5246 / 5423 / 5598 | 1476 / 1478 / 1478 | 3350 / 3350 / 3350 | 11056 / 11231 / 11402 | 11362 / 30461 / 49623 | 21 / 63 / 105 |
| 닫힌 Experiment | 0 / 1 / 2 | 6230 / 6687 / 7142 | 3606 / 3917 / 4226 | 5246 / 5731 / 6214 | 1476 / 1478 / 1478 | 3350 / 3733 / 4116 | 11056 / 11898 / 12736 | 11362 / 18812 / 25447 | 21 / 34 / 47 |
| 누적 실패 | 0 / 1 / 2 | 6230 / 7155 / 7702 | 3606 / 4156 / 4521 | 5246 / 6205 / 6781 | 1476 / 1531 / 1531 | 3350 / 3403 / 3456 | 11056 / 12089 / 12689 | 11362 / 18527 / 24860 | 21 / 35 / 49 |
| 닫힌 Step | 0 / 2 / 4 | 6188 / 6230 / 6991 | 3456 / 3606 / 4227 | 5194 / 5246 / 6032 | 1476 / 1476 / 1477 | 3861 / 3350 / 3182 | 11525 / 11056 / 11650 | 9466 / 11362 / 14729 | 17 / 21 / 25 |
| Report 줄 | 1 / 4 / 16 | 6230 / 6784 / 9026 | 3606 / 3884 / 5022 | 5246 / 5808 / 8074 | 1476 / 1476 / 1476 | 3350 / 3350 / 3350 | 11056 / 11610 / 13852 | 11362 / 12018 / 14548 | 21 / 21 / 21 |
| 승인 항목 | 1 / 2 / 4 | 6230 / 6232 / 6240 | 3606 / 3608 / 3616 | 5246 / 5248 / 5256 | 1476 / 1476 / 1476 | 3350 / 3816 / 4750 | 11056 / 11524 / 12466 | 11362 / 11596 / 11992 | 21 / 21 / 21 |

"읽기 text 합"은 세 호출 text의 합일 뿐 모델 입력량이 아니다. text와 structuredContent는 더하지 않았다.

## 증가가 큰 구간 (`gil_context` text, 단위당)

| 축 · 구간 | Δ/단위 | 늘어난 절 | 해석에 필요한 관측 |
|---|---|---|---|
| 누적 실패 0→1 | **+925 B** | failed_cycles +869, plan +53 | 첫 실패가 실패 절 머리말(약 375 B)을 함께 연다 |
| 누적 실패 1→2 | +547 B | failed_cycles +494, plan +53 | 실패 Cycle 투영 하나 약 494 B |
| 닫힌 Experiment 0→1, 1→2 | +457, +455 B | previous_cycles +402, plan +53 | 이전 Cycle 투영 하나 약 402 B. `gil_plan_read`도 Cycle당 +383 B |
| 닫힌 Step 2→4 | +380 B/Step | rules +640, current_cycle +458, next_call −168, plan −170 | Outcome 자리의 필요 칸 안내가 커진다(상태별 안내) |
| Report 줄 1→4→16 | +185~187 B/줄 | current_cycle만 | 현재 Cycle은 Step Report 원문 해상도이므로 선형으로 는다(설계대로) |
| 이전 Chain 0→1, 1→2 | +173, +171 B | chain_previous +170 | Chain Report 해상도. `.gil`은 Chain당 약 19 KB 늘지만 context는 170 B |
| 승인 항목 1→2→4 | +2~4 B/항목 | plan(항목 ID만) | 항목 본문은 `gil_plan_read`에서만 늘어난다(+467 B/항목) |

## 해상도·누락 점검

각 Report 칸의 `ccbT-…` 표식이 어느 응답에 도착했는지 26개 실행 모두에서 확인했다. 위반 0건.

- 이전 Chain: Chain Report 인계 3/3이 `gil_context`에 있고, 내부 Cycle·Step·Cycle Report 표식 57개는 어느 읽기 호출에도 없다.
- 이전 Cycle: Define 목적·Outcome 교훈·Cycle Report 인계 35/35가 있고, 내부 Step 표식 54개는 `gil_context`에 없다
  (16개는 `gil_plan_read`의 승인 문서에 있다 — 승인된 Synthesis 문장·완료 항목 질문).
- 누적 실패: 실패 Cycle 투영 12/12(Define·교훈·인계·revisit 이유)가 모두 있고, 내부 Step 12개는 없다.
- 현재 Cycle: 닫힌 Step Report 38/38(가드레일 포함)이 있다. 이번 Experiment의 승인 항목 13/13도 있다.
- **승인 범위 제약(13/13)과 미실행 항목 본문(4/4)은 `gil_context`에 없고 `gil_plan_read`에만 있다.** `gil_context`는 승인 판·미실행 ID와
  `gil_plan_read`로 읽으라는 안내만 싣는다.
- 읽기 전후 프로젝트 폴더(`.gil` 포함)와 설정 폴더의 전체 digest가 26개 실행 모두 같았다.

## 동일 설명 반복과 상태별 안내의 근거

- **모든 사례에 똑같은 단락 10개, 2921 B** (기준점 `gil_context` text의 47%). 크기가 모든 사례에서 같은 절:
  receipt 앞부분 두 text part(329 B, 934 B)·preamble 358·file_scope 720·chain_current 376·next_moves 108·report_principles 477·resume 113 B.
  상태와 무관하게 매 재개에 실리는 고정 안내 후보다.
- **읽기 호출 사이의 같은 줄 947 B**: Monitor 미표시 안내(329 B)가 `gil_context`와 `gil_status`의 첫 text part에, 파일 작업 범위
  규칙 한 줄(479 B)이 두 호출의 둘째 text part에 똑같이 있다. 같은 규칙의 다른 표현이 `gil_context` 본문의 file_scope 절(720 B)에도 있다.
  단락 단위(context-metrics) 반복 후보로는 Monitor 안내 329 B만 잡힌다 — 줄 단위 측정을 이 도구에 따로 둔 이유다.
- **항목마다 반복되는 틀**: 이전/실패 Cycle 투영이 2개가 되면 같은 틀(번호·표식만 다른) 단락이 12개(712 B, 실패 축 892 B) 생긴다.
  "실험 목적·성공 기준·판정의 교훈·다음 Cycle 에 넘긴 것·다음 방향" 제목과 들여쓰기가 투영마다 반복된다.
  같은 응답 안 완전 동일 단락(+54 B, +65 B)은 합성 기록의 같은 문장 때문이며 Core의 반복이 아니다.
- **상태별 안내**: next_call(161~840 B)·rules(69~709 B)·plan(523~799 B)은 자리에 따라 크기가 바뀐다. 닫힌 Step 0에서 `gil_plan_read`가
  더 큰 것(3861 B)도 자리별 안내 때문이다. 이것들의 필요성은 판정하지 않았다.
- **두 표현의 중복**: `gil_context`의 structuredContent.said(4965 B)는 text의 셋째 part와 같다. `gil_plan_read` text도 structuredContent와 거의 같은
  JSON을 싣는다. Host가 둘 다 모델에 넣는지는 관측하지 않았다.

## 재현성

- 같은 실행 안 2회 반복: 13개 사례 모두 세 호출의 text·structured 크기와 내용(Monitor scope id만 정규화)이 같았다.
- 별도 실행 3회(이 기준선 포함): `compare` 결과 조건 차이 없음, 같은 binary, 13개 사례 모두 크기·내용 차이 0.
- 소요 시간·`.gil` 바이트는 기록했지만 시간은 재현 대상이 아니다.

## 측정 한계

- 바이트·문자 수이며 토큰 수가 아니다. Host의 모델 입력 구성, 재조회, 전체 실행 비용, 작업 정확성은 측정하지 않았다.
- 합성 기록이다. 문장이 짧고 항목이 단순하므로 절대 크기보다 축별 증가 모양을 본다.
- 축은 하나씩만 늘렸고 규모가 작다(최대 Chain 2·Cycle 2·실패 2·Report 16줄·항목 4). 큰 규모와 축 사이 상호작용은 측정하지 않았다.
- `product-safety`가 없는 개발 프로필이다. 제품 프로필은 revisit을 막으므로 실패 축을 같은 경로로 만들 수 없다. 그 외 문구 차이는 측정하지 않았다.
- 해상도 점검은 표식 문자열의 존재만 본다. 요약의 의미 보존은 판정하지 않는다.
- 단락·줄 반복은 후보다. 필요한 경고·승인 안내인지 판단하지 않았다.

## 후속 최적화 후보 (관측 근거가 있는 것만)

제품 코드는 이번에 고치지 않았다. 아래는 측정이 보여 준 자리이며, 줄일지 여부는 명세와 실제 모델 평가로 정한다.

1. **실패·Experiment마다 커지는 plan 실행 연결 목록** — Experiment Cycle 하나(성공·실패 모두)마다 `gil_context` plan 절이 +53 B 는다.
   실패 2개 사례에서는 실패한 `cycle:C2`·`cycle:C3`도 현재 `cycle:C4`와 같은 항목 `I1`의 실행 연결 JSON으로 모두 나열된다.
   긴 Chain에서 선형으로 커질 자리다.
2. **이전·실패 Cycle 투영의 반복 틀** — 투영당 약 402 B(실패 494 B) 중 제목·들여쓰기 틀이 투영마다 반복된다(2개일 때 같은 틀 단락 12개).
   해상도(어떤 칸을 싣는가)를 바꾸지 않고 배치만 바꿀 수 있는지 볼 자리다.
3. **새 세션 첫 읽기에 겹치는 고정 안내** — 모든 사례에 같은 2921 B, 그중 파일 작업 범위 규칙은 `gil_context` 두 곳과 `gil_status`에 함께 있다.
   Monitor 미표시 안내 329 B도 두 호출에 같다. 한 세션에서 몇 번 필요한지는 모델 평가로 확인할 질문이다.
4. **승인 제약의 전달 위치** — 승인 범위 제약과 미실행 항목 본문이 `gil_context`에 없다. Context Model §1.1은 "승인된 목표와 제약"을 이어받게
   하므로, 재개가 항상 `gil_plan_read`로 이어지는지 또는 짧은 제약을 context에 싣는지 설계 확인이 필요하다(누락이라고 단정하지 않는다).
5. **text와 structuredContent의 같은 본문** — `gil_context` 본문 약 5 KB가 두 표현에 모두 있다. 줄이기 전에 각 Host가 무엇을 모델에 넣는지부터 측정해야 한다.

누적 실패의 첫 증가(+925 B)는 실패 절 머리말을 포함하며, 같은 실패를 되풀이하지 않기 위해 명세가 반드시 전달하라고 한 내용이다(Cycle Model §10).
현재 Cycle Report의 선형 증가와 Outcome 자리의 필요 칸 안내도 설계대로의 증가로 보고 후보에서 뺐다.
