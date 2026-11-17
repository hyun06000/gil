# tools/chain-interview-eval — Chain Interview 평가 묶음의 오프라인 검사 도구

> **2026-11-17 적용 범위:** 이 자료의 기존 사례·검사 결과는 당시 제품 계약 또는 미채택 설계
> 후보의 기록이다. 최신 합의는 [승인 계획 계약](../../spec/GIL_Approved_Plan_v0.1.md)의 구현 이행 절을
> 따른다. 안정된 요구 아래 여러 예/아니오 Define, 관측 기반 선택, 실패 시 무변경 세계에서도
> 명시적 전체 복원, 질문별 답·한계 인계를 새로 검증해야 한다. 현재 시험 통과를 새 계약의 구현
> 또는 의미 평가 완료로 주장하지 않는다. 과거 기대값을 새 설계에 맞았던 것처럼 소급 수정하지 않는다.


[`scenarios/chain-interview/`](../../scenarios/chain-interview/README.md)의 합성 사례 집합과 평가 결과 파일을 **오프라인**으로 검사한다.
Node 표준 라이브러리만 쓴다(의존성 설치 없음). 모델·Core·App·네트워크를 실행하지 않는다. 사례·결과 안의 문장은 데이터로만 다룬다.

| 파일 | 역할 |
|---|---|
| `check-fixtures.mjs` | 사례 집합 무결성: schema, id·파일 이름, 참조(기준·실패 유형·비교 파일·명세 파일·bound_item), 필수 입력(required 확인마다 합성 답변), 좋은/실패 예시의 구조 검사 선언 일치, 비교 파일 모양 |
| `check-results.mjs` | 결과 파일 일관성: 실행 메타데이터, 사례 커버리지, not_run/unknown 보존, 판정 근거, 판정자 종류, 합성 승인 구분, outputs에 구조 검사 재계산 |
| `make-skeleton.mjs` | 모든 사례가 not_run인 결과 뼈대 생성. `--events`로 `tools/scenarios` 형식 기록에서 제안 draft·Define·닫힌 Experiment를 뽑아 `unknown` 사례로 넣는다 |
| `import-evidence.mjs` · `lib/evidence.mjs` | 명시한 도구 기록·공개 대화·주석을 해시와 인용으로 검증해 기존 결과의 `prepared_evidence`에 보관. 실행·평가·승인 상태 불변 |
| `lib/structural.mjs` | 출력 구조 검사(제안·질문·Define·완료 주장). 문구·길이·개수를 보지 않는다 |
| `lib/cases.mjs` `lib/results.mjs` `lib/vocab.mjs` `lib/io.mjs` `lib/format.mjs` | 적재·검사·어휘·출력 |
| `*.test.mjs` | 기존 평가·새 가져오기 자체시험. 합성 입력만 사용한다 |

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

`make-skeleton --events`의 기존 출력은 `run.executed=false`에 `unknown`·outputs가 있어 그대로는 결과 검사를
통과하지 않는다. 그 검사를 완화하지 않았다. 실행 증거가 없는 자료의 연결·보관에는 아래 가져오기 경로를 쓴다.

## 공개 대화와 도구 기록 가져오기

```sh
# 저장소의 합성 예시만 사용한다. /tmp/chain-evidence.json은 아직 없는 파일이어야 한다.
node tools/chain-interview-eval/import-evidence.mjs \
  --result scenarios/chain-interview/results/examples/not-run.json \
  --events scenarios/chain-interview/import/examples/events.jsonl \
  --conversation scenarios/chain-interview/import/examples/conversation.json \
  --manifest scenarios/chain-interview/import/examples/manifest.json \
  --out /tmp/chain-evidence.json
node tools/chain-interview-eval/check-results.mjs --result /tmp/chain-evidence.json --quiet
```

`--result`, `--events`, `--manifest`, `--out`은 필수다. `--cases <dir>`은 선택이다.
대화가 없으면 manifest의 `conversation`을 `null`로 하고 `--conversation`을 생략한다.
그때 대화 인용이 필요한 questions·claims·links는 빈 배열이어야 한다. 빈 대화/도구 파일도 누락으로 표시한다.
파일 경로는 CLI에서만 받는다. 입력 본문의 ref·링크·경로·명령은 읽거나 실행하지 않는다.

기준 결과는 기존 검사에 통과해야 하며 현재 사례 집합의 해시와 일치해야 한다.
새 평가라면 `make-skeleton --run-id <id> --out <new.json>`으로 시작해 실제 평가 대상의
`run.guidance`(지침 해시·소스 커밋)를 명시한다. 자료를 가져오기 위해 `executed=true`로 바꾸지 않는다.
예시 결과의 model·guidance를 실제 실행의 정보로 복사하지 않는다.

가져오기는 지정 사례에 **`prepared_evidence` 하나만 추가**한다. run 전체, judges, status, approval,
observation, outputs, automatic, semantic은 그대로다. 기존 준비 자료가 있으면 거절하며 조용히 합치거나 덮어쓰지 않는다.
따라서 not_run은 미실행, unknown은 근거 부족, evaluated는 기존에 기록한 평가 자료와 판정 상태로 남는다.
모든 새 자료는 `purpose: preparation_only`다. importer가 평가 완료·모델 실행·인간 승인을 선언하는 경로는 없다.
기존 evaluated 사례에 준비 자료를 추가해도 기존 판정의 근거가 자동으로 교체되지는 않는다.

### 최소 대화 형식

```json
{
  "schema": "gil.chain-interview.conversation/1",
  "run_id": "run-1",
  "case_id": "CI-01",
  "messages": [
    {
      "id": "message-1",
      "role": "user",
      "text": "날짜 열부터 확인해 주세요.",
      "source": {"ref": "export:session-1/message-1", "origin": "synthetic"}
    }
  ]
}
```

UTF-8 JSON이며 role은 `user|assistant`, origin은 `synthetic|human|model|unknown`이다.
실제로 공개된 원문만 text에 넣는다. 비공개 추론을 입력하지 않는다. source.ref는 원본 메시지를 다시 찾기 위한
불투명 참조다. export의 식별자가 있으면 message.id에 보존한다. 없으면 준비자가 부여한 식별자임을 source.ref에 밝힌다.
각 메시지는 상위 run_id/case_id를 상속한다. 메시지에 같은 두 필드를 추가할 수 있지만 다른 값이면 거절한다.
`role:user`나 `origin:human`은 사람의 승인 버튼 클릭 증거가 아니다. 합성 사용자 답변은 synthetic으로 적는다.

### 출처 manifest와 명시적 주석

완전한 실행 가능 입력은 [manifest.json](../../scenarios/chain-interview/import/examples/manifest.json),
[conversation.json](../../scenarios/chain-interview/import/examples/conversation.json),
[events.jsonl](../../scenarios/chain-interview/import/examples/events.jsonl)에 있다. 모두 손으로 만든 합성 자료다.

manifest는 `schema: gil.chain-interview.import/1`, 자체 `id/ref`, `run_id/case_id`,
`events: {id,ref,sha256}`, `conversation: {id,ref,sha256}|null`, `questions[]`, `claims[]`, `links[]`를 갖는다.
sha256은 지정 파일의 **원본 UTF-8 바이트** 해시다(공백·개행 포함). 변환/마스킹했다면 변환한 파일의 해시와 출처를 적는다.
잘못된 UTF-8이나 JSON은 거절한다. manifest 자체의 해시와 원문도 결과에 보존한다.

- 메시지 인용은 `{message_id,start,end,quote}`다. start/end는 JS 문자열의 0부터 시작하는 UTF-16 인덱스이며
  끝은 포함하지 않는다. `text.slice(start,end) === quote`를 요구한다. 짧은 반복 문구도 위치로 구별한다.
- 질문은 `{id,cite,choices:[{cite,role}],multi_select:{value,cite}|null,response:cite|null}`다.
  질문·선택지·복수 선택 안내는 같은 assistant 메시지를, response는 user 메시지를 인용한다.
  선택지 role은 `option|unsure|free_text`이며 **준비자가 직접** 지정한다. 단어·길이로 의미를 추정하지 않는다.
  복수 선택 안내/응답이 없으면 null로 남긴다. 선택지가 없으면 빈 배열로 남긴다.
- 완료 주장은 `{id,cite,items:["A"]}`다. cite는 실제 assistant 주장이고, items는 준비자의 명시적 항목 대응이다.
  항목 id는 추출된 마지막 제안 또는 해당 사례의 prior approved plan에 있어야 한다. 이 대응은 실행/완료의 증명이 아니다.
- 대화–도구 연결은 `{id,message:cite,event:{seq,pointer,start,end,quote}}`다.
  pointer는 도구 이벤트 안의 문자열을 가리키는 JSON Pointer다(예: `/request/arguments/report`).
  해당 문자열의 구간도 인용과 정확히 같아야 하고, 양쪽 quote가 동일해야 한다.
  일치하는 원문 구간이 없으면 연결을 만들지 않는다. 시간·배열 인접성으로 연결하지 않는다.

모든 주석은 manifest 원문 안에 남는다. 주석을 생략해도 메시지는 보존하지만 빈 questions/claims를
“질문/완료 주장 없음”으로 판정하지 않는다(`annotation_coverage: unspecified`). `missing`에는 대화·도구·질문 주석·
응답·복수 선택 주석·대화–도구 연결의 관측 가능한 누락을 기록한다. 질문 수·질문의 깊이·선택지 역할의 의미·완료 주장의
정확성은 별도 판정자가 원문을 보고 평가해야 한다. importer는 점수·판정자·의미 판정의 인용 근거를 생성하지 않는다.

### 식별자·원문 검증과 기존 검사기 연결

도구 입력은 기존 proxy/wire의 JSONL이다. 각 비어 있지 않은 줄에 `request.name`과 양의 정수 `seq`가 필요하다.
seq는 파일 순서대로 증가해야 하며 중복/재시작은 거절한다. 여러 실행을 이어 붙이지 않는다.
선택적인 provider `id/call_id`는 비어 있지 않은 고유 문자열이어야 한다. 원래 `actor/synthetic`도 보존한다.
이벤트 자체에 run_id/case_id가 있다면 manifest와 같아야 한다. 없는 기존 기록은 **준비자의 파일 전체 범위 선언**으로
묶는다. 각 실행/사례마다 별도 기록을 명시해야 하며, 범위 정보가 없는 자료의 진짜 출처를 해시로 인증할 수는 없다.

결과의 `prepared_evidence.sources`는 세 입력의 id/ref/sha256/text를 보존한다.
`projection`은 메시지 해시, 이벤트별 seq/원본 줄/줄 해시/호출 성공 여부, 기존 추출기의 제안·Define·Experiment 종료,
명시적 질문·응답·완료 주장을 담는다. 도구의 Question contract는 `tool_question_pointers`에 따로 보존하며
사용자에게 보인 질문으로 만들지 않는다. 실패한 호출은 원문과 인덱스에 남고 성공 출력으로 추출하지 않는다.

기존 `check-results`는 준비 자료가 있으면 원문 해시·범위·식별자·인용을 재검증하고 projection을 다시 만들어 비교한다.
깨진 참조·중복 식별자·다른 사례/실행의 명시적 범위·변조한 추출 자료는 `prepared_evidence_integrity`로 거절한다.
준비 자료는 기존 pass/fail 개수에 포함하지 않으며 기존 구조/의미 기준을 완화하지 않는다.
나중에 평가하려면 실제 실행 메타데이터, 판정자, 정확한 인용과 판정을 별도로 기록해야 한다. 자동 승격 명령은 없다.

한계: 해시는 변조 대조 수단이며 수집자의 진실성·대화 완전성·물리적 인간 클릭을 인증하지 않는다.
인용의 일치는 인과관계나 의미적 일치의 증명이 아니다. 서로 다른 표현을 의역해 자동 연결하지 않는다.
Core 그래프의 basis_refs lineage·승인 revision의 진실성은 이 가져오기 도구가 재실행/인증하지 않는다.
기존 추출기처럼 마지막 성공 제안/Define만 투영하지만 이전 자료는 원문에 모두 보존한다.

입력은 읽기 전용이고 출력은 새 파일에만 `wx`, 권한 `0600`으로 쓴다. 부모 폴더는 미리 있어야 한다.
성공은 종료 코드 0, 거절은 2다. 터미널은 고정된 상태 안내만 출력하며 오류에도 원문·토큰·개인 경로를 표시하지 않는다.
원문은 새 출력 파일에 포함되므로 실제 사용자 자료·토큰이 든 결과를 저장소에 커밋하지 않는다.

## 재사용한 것

`tools/scenarios/lib/markdown.mjs`의 `table()`(읽기 전용 import)과 `events.jsonl` 형식, 판정 어휘(`pass/fail/unknown/not_run`).
기존 `tools/scenarios`·`tools/context-metrics` 파일은 바꾸지 않았다.

## 하지 않는 것

제품 코드(`src/`, `ui/`, `mcp-app/`)·Plugin Skill·workflow·기존 평가 도구와 fixture·로드맵·규범 명세 변경, 모델 호출, Core 실행, 자동 검사(CI) 편입, 로드맵 갱신.
CI에 넣으려면 `node --test tools/chain-interview-eval/*.test.mjs`와 `node tools/chain-interview-eval/check-fixtures.mjs`를 JS 검사 묶음에 추가하면 되며, 이는 통합 담당의 일이다.
기존 glob에 새 `import-evidence.test.mjs`가 자동 포함된다. 이 시험은 합성 입력의 CLI 가져오기와
`check-results` 연결까지 실행한다. 새 별도 workflow나 의존성 설치는 필요하지 않다.
