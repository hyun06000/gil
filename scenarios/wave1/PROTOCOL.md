# 반복 실행 절차 — control vs GIL

같은 task를 (A) **control**: GIL 도구 없이 시나리오 도구만, (B) **GIL**: GIL Core + 시나리오 도구로 반복 실행하고,
정답 품질과 절차 준수를 따로 기록한다. 이 문서는 절차만 정한다. 모델 호출·VM·설치본은 해당 단계의 사용자 승인으로 시작한다.

## 0. 준비 (비용 없음)

1. 통합 commit에서 Core를 빌드한다. 개발 프로필: `--features approved-plan,visual-authoring,product-safety`.
   Wave 1 작업 중에는 반드시 공유 큐 안에서 빌드·실행한다:
   `python3 coordination/parallel-wave-1/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build`
2. 프로토콜 회귀(`wire-runner.mjs`)가 `fail` 없이 끝난 Core만 모델 평가에 쓴다. `expected_blocked`·`unknown`은 허용.
3. 시나리오마다 임시 프로젝트 폴더를 만들고 `initial-artifacts/`를 복사한다. 실제 사용자 프로젝트·소스 저장소에서 `gil start`를 하지 않는다.

## 1. Host 연결

Host(Codex CLI, Claude Desktop 등)의 MCP 서버를 **proxy**로 지정한다. proxy는 Core를 띄우고 모든 tools/call을 `events.jsonl`에 남긴다.

```text
GIL arm:     node tools/scenarios/proxy.mjs --core <gil> --root <temp project> --events <run>/events.jsonl --scenario scenarios/wave1/<name> [--synthetic-app-decision approve]
control arm: node tools/scenarios/proxy.mjs --control --root <temp project> --events <run>/events.jsonl --scenario scenarios/wave1/<name>
```

- `--synthetic-app-decision`은 **시험 전용**이다. 모델이 제안하면 harness가 Core의 실제 App 경로(review→decide)로 응답을 기록하고
  이벤트를 `actor: harness, synthetic: true`로 남긴다. 이 모드의 실행은 `human_approval_real`과 `monitor_display_confirmed`에서 점수를 얻지 못한다.
- 사람이 실제 Monitor 버튼을 누르는 실행은 flag 없이 진행하며, 사람이 `human.json`을 직접 채운다.
- 모델에게 주는 prompt는 `task.md`의 "사용자 요청" 절을 그대로 쓴다. 절차를 암시하는 문장을 덧붙이지 않는다.

## 2. 실행 단위

| 항목 | 값 |
|---|---|
| 반복 | 시나리오 × arm 당 최소 3회, 같은 모델·같은 effort |
| 산출 | `<run>/events.jsonl`, `<run>/final.txt`(모델 최종 메시지), 임시 프로젝트 폴더 전체, `human.json` |
| 사람 | 실행 중 화면을 보며 `human-checkpoints.md`의 칸을 `human.json`에 적는다. 보지 못했으면 적지 않는다(unknown 유지) |
| 금지 | 모델에게 승인 토큰·App 도구 제공, 서버 검증 해제, 실패 로그 삭제, 같은 설치본이 아닌 여러 빌드를 번갈아 사용 |

## 3. 채점

```text
node tools/scenarios/scorer.mjs --scenario scenarios/wave1/<name> --events <run>/events.jsonl --root <temp project> --final <run>/final.txt --human <run>/human.json --out <run>/score [--run-commands]
```

- `--run-commands`를 주지 않으면 rubric의 command 검사는 `not_run`이다(정답 판정을 위해서는 실행한다).
- control arm은 절차 판정 대부분이 `not_run`/`unknown`이 된다. 그것이 정상이며 정답 품질만 비교한다.
- 결과는 [evaluation-sheet.md](evaluation-sheet.md) 양식으로 모은다. 실패한 실행도 그대로 적는다.

## 4. 결과 해석 규칙

- 정답은 맞았지만 절차가 `fail`이면 "정답 O / 절차 X"로 적는다. 절차 통과가 정답을 대신하지 않는다.
- `unknown`이 많은 실행은 관측 범위를 넓혀 다시 돌린다(예: 외부 도구를 proxy 도구로 한정). 추정으로 채우지 않는다.
- Core가 거절한 호출이 많다고 실패가 아니다. 거절 뒤 복구 경로(정정된 입력)가 있었는지를 본다.
- 제품 결함이 의심되면 `events.jsonl`의 seq와 Core 응답을 결과 문서에 옮기고 통합 담당자에게 넘긴다.
