# tools/chain-interview-eval — Chain Interview 평가 묶음의 오프라인 검사 도구

[`scenarios/chain-interview/`](../../scenarios/chain-interview/README.md)의 합성 사례 집합과 평가 결과 파일을 **오프라인**으로 검사한다.
Node 표준 라이브러리만 쓴다(의존성 설치 없음). 모델·Core·App·네트워크를 실행하지 않는다. 사례·결과 안의 문장은 데이터로만 다룬다.

| 파일 | 역할 |
|---|---|
| `check-fixtures.mjs` | 사례 집합 무결성: schema, id·파일 이름, 참조(기준·실패 유형·비교 파일·명세 파일·bound_item), 필수 입력(required 확인마다 합성 답변), 좋은/실패 예시의 구조 검사 선언 일치, 비교 파일 모양 |
| `check-results.mjs` | 결과 파일 일관성: 실행 메타데이터, 사례 커버리지, not_run/unknown 보존, 판정 근거, 판정자 종류, 합성 승인 구분, outputs에 구조 검사 재계산 |
| `make-skeleton.mjs` | 모든 사례가 not_run인 결과 뼈대 생성. `--events`로 `tools/scenarios` 형식 기록에서 제안 draft·Define·닫힌 Experiment를 뽑아 `unknown` 사례로 넣는다 |
| `lib/structural.mjs` | 출력 구조 검사(제안·질문·Define·완료 주장). 문구·길이·개수를 보지 않는다 |
| `lib/cases.mjs` `lib/results.mjs` `lib/vocab.mjs` `lib/io.mjs` `lib/format.mjs` | 적재·검사·어휘·출력 |
| `eval.test.mjs` | 자체시험. 깨진 fixture는 임시 폴더에 만든다 |

## 실행

```sh
# 사례 집합 검사 (기본 scenarios/chain-interview). 끝에 case_set 해시를 출력한다
node tools/chain-interview-eval/check-fixtures.mjs [--cases <dir>] [--json <out.json>] [--quiet]

# 결과 파일 검사
node tools/chain-interview-eval/check-results.mjs --result <result.json> [--cases <dir>] [--json <out.json>] [--quiet]

# 결과 뼈대 (저장소 밖 경로 권장)
node tools/chain-interview-eval/make-skeleton.mjs --out <result.json> [--run-id <id>] [--events <events.jsonl> --case CI-NN]

# 자체시험
node --test tools/chain-interview-eval/*.test.mjs
```

종료 코드: `0` 문제 없음(not_run·unknown만 있는 결과도 0이다) · `1` 무결성/일관성 문제 · `2` 사용법·입력 오류(없는 파일, JSON 아님, 깨진 사례 집합 위에서 결과 검사).
`--json`은 새 파일에만 쓴다(`wx`). 기존 파일을 덮어쓰지 않으며 같은 경로를 다시 쓰려면 새 이름을 준다.
터미널 출력은 식별자·개수만 적고 예시·제안·메시지 본문을 재출력하지 않는다. 문제의 파일 경로는 사례 폴더 기준 상대 경로다.

## 검사의 범위와 한계

| 검사한다 | 검사하지 않는다 |
|---|---|
| 사례 파일의 schema·열거값·id·참조 무결성 | 사례 문장이 좋은 평가 사례인지(사람이 검토한다) |
| required 확인 항목마다 합성 답변이 있는지 | 그 답변이 현실적인지 |
| 제안 draft의 필드·항목·checklist·basis_refs 형식 | basis_refs가 실제 닫힌 Question을 가리키는지(기록이 있어야 알 수 있고, Core가 거절한다) |
| 질문 보기의 role과 두 탈출 경로, multi_select 명시 | 질문이 필요했는지, 깊이가 적절했는지(S1·S2 — 사람/모델 판정) |
| Define이 승인 항목 하나에 연결되는지 | Define 범위가 항목과 같은지(S7) |
| 완료 주장 항목마다 success Experiment 종료 기록이 있는지(기록 없으면 unknown) | 완료 메시지의 정직함 전반(S8) |
| 실패 예시가 선언한 구조 위반과 실제 위반의 일치 | 의미 실패 유형(`failure_modes`)의 타당성 |
| 결과 파일의 not_run·unknown 보존, 판정 근거·판정자·승인 출처 | 판정 자체의 옳고 그름 |

**자동 검사를 통과한 제안이 좋은 제안은 아니다.** CI-02의 묶음 제안, CI-03의 조용한 제약 삭제, CI-06의 손실 있는 간결화, CI-07의 아홉 조각 제안은 모두 구조 검사를 통과한다.
그것이 의미 평가를 따로 두는 이유다. 반대로 구조 검사 fail(빈 basis_refs, 탈출 경로 없는 질문)은 그 자체로 계약 위반이므로 의미 판정 전에 알 수 있다.

## 입력 형식

- 사례 집합: `<dir>/rubric.json`, `<dir>/cases/CI-NN.json`, `<dir>/comparisons/*.json`. 구성은 [시나리오 README](../../scenarios/chain-interview/README.md).
- 결과 파일: [`results/FORMAT.md`](../../scenarios/chain-interview/results/FORMAT.md).
- `--events`: `tools/scenarios/proxy.mjs`·`wire-runner.mjs`가 남기는 JSONL(한 줄 = 도구 호출 한 건). JSON이 아닌 줄·`request.name`이 없는 줄은 `unsupported_lines`로 세고 뺀다.
  `gil_plan_propose`의 성공한 마지막 `draft`, `gil_open(kind: define)` 뒤 `gil_close`의 `problem`/`success_condition`, `gil_plan_read`의 `current_item`, Experiment 안 `verdict`+`lesson` 종료를 뽑는다.
  채팅 본문은 도구 기록에 없으므로 `visible_messages`는 사람이 옮긴다. 뽑힌 사례는 판정 전이므로 `unknown`이다.

## 재사용한 것

`tools/scenarios/lib/markdown.mjs`의 `table()`(읽기 전용 import)과 `events.jsonl` 형식, 판정 어휘(`pass/fail/unknown/not_run`).
기존 `tools/scenarios`·`tools/context-metrics` 파일은 바꾸지 않았다.

## 하지 않는 것

제품 코드(`src/`, `ui/`, `mcp-app/`)·Plugin Skill·workflow·기존 평가 도구와 fixture·로드맵·규범 명세 변경, 모델 호출, Core 실행, 자동 검사(CI) 편입, 로드맵 갱신.
CI에 넣으려면 `node --test tools/chain-interview-eval/*.test.mjs`와 `node tools/chain-interview-eval/check-fixtures.mjs`를 JS 검사 묶음에 추가하면 되며, 이는 통합 담당의 일이다.
