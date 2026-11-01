# tools/scenarios — Wave 1 scenario harness

Node 표준 라이브러리만 쓴다(의존성 설치 없음). 네 부분으로 이루어진다.

| 파일 | 역할 | native Core 필요 |
|---|---|---|
| `wire-runner.mjs` | 실제 `gil mcp --serve`를 띄워 M6-D 승인 계약을 JSON-RPC로 회귀 시험 (W01–W21) | 예 — **큐 안에서** 현재 branch를 빌드한 직후 실행 |
| `proxy.mjs` | Host ↔ Core 사이에서 모든 tools/call을 기록하고 시나리오 도구를 더한다. `--control`은 Core 없이 시나리오 도구만 | GIL arm은 예 |
| `scorer.mjs` | `events.jsonl` + rubric(+ 프로젝트 폴더, 사람 기록) → 절차/정답 분리 채점 | 아니오 |
| `lib/*.mjs`, `*.test.mjs`, `fixtures/` | 클라이언트·가림·재생·채점 라이브러리와 JS 전용 자체시험 | 아니오 |

## 실행

```sh
# JS 전용 자체시험 (독립 실행 가능)
node --test tools/scenarios/*.test.mjs

# 실제 stdio 회귀: 공유 큐가 target과 TMPDIR을 준비하고 전체 명령 동안 잠근다. runner는 큐를 다시 잡지 않는다.
python3 coordination/parallel-wave-1/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build --out <dir>

# 이미 빌드된 특정 실행 파일로 (통합 후 CI 등): 큐 밖에서는 --gil 경로를 명시한다. PATH/GIL_BINARY는 읽지 않는다.
node tools/scenarios/wire-runner.mjs --gil target/debug/gil --out <dir>

# 채점
node tools/scenarios/scorer.mjs --scenario scenarios/wave1/backend --events <run>/events.jsonl --root <project> --final <run>/final.txt --human <run>/human.json --out <run>/score --run-commands
```

- `--build`는 `GIL_PARALLEL_TARGET_DIR`(큐가 설정)이 없으면 거부한다. 큐가 없는 환경에서만 `--unqueued`를 쓴다.
- 기본 feature: `approved-plan,visual-authoring,product-safety`. `--features`로 바꾸면 Verify close 형식(typed/plain)을 맞춘다.
- 결과: `wire-run.json`, `wire-run.md`, `wire-frames.jsonl`(가림된 요청/응답, 프로세스 세대별), `wire-events.jsonl`(채점기 입력 형식).
  가림 검사에 실패하면 frames/events 파일을 쓰지 않고 `redaction_gap`을 기록한다.
- 실패 시 임시 작업 폴더를 남기고 경로를 stderr에 적는다. 각 case의 `repro`가 재현 명령을 가리킨다.

## 상태 어휘

`pass` / `fail` / `unknown` / `expected_blocked` / `unsupported` / `not_run`. runner는 선행 case가 실패하면 뒤 case를 `not_run`으로 둔다.
`expected_blocked`(restore/revisit 거절)는 의존성 판단에서만 통과로 취급하고 보고서에는 그대로 적는다.

## 이벤트 형식 (`events.jsonl`)

```json
{"seq":1,"at":"…","actor":"model|app|harness","synthetic":false,"request":{"name":"gil_open","arguments":{}},"result":{},"error":null,"ms":12}
```

`actor: harness, synthetic: true`는 시험 harness가 Core의 실제 App 경로로 보낸 합성 입력이다. 채점기는 이를 영수증 관측으로는 세지만
`human_approval_real`·`monitor_display_confirmed`에서는 절대 pass를 주지 않는다. 토큰은 기록 전에 `<redacted-token>`으로, 절대경로는 `<project_root>` 등으로 바뀐다.

## 하지 않는 것

- 설치된 Core 추측, 서버 검증 해제, 제품에 시험 전용 우회 도구 추가, 임시 폴더 밖 수정, 자동 병합·push·배포.
- 모델 호출: `proxy.mjs`는 Host가 연결할 서버일 뿐 모델을 호출하지 않는다. 비용이 드는 실행은 사용자 승인 뒤 [PROTOCOL](../../scenarios/wave1/PROTOCOL.md)대로 한다.
