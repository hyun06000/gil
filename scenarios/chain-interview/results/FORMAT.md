# 결과 파일 형식 — `gil.chain-interview.result/1`

실제 평가를 나중에 같은 모양으로 반복하기 위한 형식이다. 특정 모델의 내부 형식에 의존하지 않으며, 모델·지침 버전·사례 집합·도구 기록·판정 근거를 한 파일에 남긴다.
[`tools/chain-interview-eval/check-results.mjs`](../../../tools/chain-interview-eval/README.md)가 일관성을 검사한다.

```jsonc
{
  "schema": "gil.chain-interview.result/1",
  "run": {
    "id": "…",                       // 실행 식별자
    "recorded_at": "2026-11-17",     // 기록 시점 (executed면 필수)
    "executed": false,               // 모델을 실제로 실행했는가. false면 모든 사례가 not_run이어야 한다
    "example": false,                // 형식 예시 파일이면 true — 검사기가 머리말에 표시한다
    "model": { "name": "…", "version": "…", "provider": "…" } | null,   // executed면 name·version 필수
    "guidance": { "workflow_md_sha256": "…", "repo_commit": "…", "profile": "…" },  // 지침 버전. 항상 필수
    "case_set": { "name": "chain-interview", "sha256": "…" },           // check-fixtures가 출력하는 해시
    "harness": { "name": "proxy|manual-transcript|…", "version": "…" } | null,
    "notes": "…"
  },
  "judges": [ { "id": "H1", "kind": "human" }, { "id": "M1", "kind": "model", "model": { "name": "…", "version": "…" } } ],
  "cases": [ /* 사례 집합의 모든 사례, 하나씩 */ ]
}
```

## 사례 항목

| 필드 | 뜻 |
|---|---|
| `case_id` | 사례 id. 집합에 없는 id는 거절. 집합에 있는데 결과에 없는 사례는 not_run으로 보고된다 |
| `status` | `not_run`(실행 안 함) · `unknown`(실행했지만 관측 부족; `reason` 필수) · `evaluated`(관측 있음) |
| `observation` | `visible_messages[{role,text}]`(사용자가 본 대화), `tool_log_ref`, `transcript_ref`, `tool_log_summary{calls, proposals, closed_experiments[{item,verdict}]}` |
| `outputs` | `questions[]`(보기에 role 포함), `proposal`(gil_plan_propose의 draft), `define{bound_item,problem,success_condition}`, `claimed_completed_items[]` |
| `approval` | `kind`: `none` · `synthetic`(harness/시험자 입력) · `human_click`(evidence 필수) · `unknown` |
| `automatic[]` | `{check, status, detail}`. 구조 검사는 outputs에서 다시 계산해 비교한다 |
| `semantic[]` | `{criterion, status, judge{id}, basis[{quote, from}], rationale}`. pass/fail은 인용 basis·rationale·등록된 judge가 필수 |

## 상태 규칙 (검사기가 강제한다)

- `run.executed`가 false면 어떤 사례도 `evaluated`/`unknown`일 수 없고, 어디에도 pass/fail이 없다.
- `not_run` 사례에는 outputs도 verdict도 없다. `unknown` 사례의 판정은 unknown/not_run뿐이다.
- 의미 판정의 judge는 `judges`에 등록된 human 또는 model이다. `auto`·휴리스틱은 거절된다.
- `approval.kind`가 synthetic/none/unknown이면 `human_approval_real`은 pass가 될 수 없다.
- 사례가 요구한 의미 기준(`judgement.semantic`)에 항목이 없으면 not_run으로 세어 보고한다.
- `case_set.sha256`이 현재 집합과 다르면 **주의**로 표시한다(문제는 아니다). 전후 비교에는 같은 해시가 필요하다.

## 예시 파일

| 파일 | 뜻 | 검사기 종료 코드 |
|---|---|---|
| [examples/not-run.json](examples/not-run.json) | 모델을 실행하지 않은 결과. 8건 모두 not_run. **미실행은 성공이 아니다**를 형식으로 보여 준다 | 0 |
| [examples/partial-synthetic.json](examples/partial-synthetic.json) | 작성자가 손으로 만든 합성 예시. CI-01 evaluated(합성 승인 → human_approval_real unknown), CI-04 unknown(기록 끊김), CI-08 evaluated(일부 완료를 전체 완료로 주장 → fail), 나머지 not_run | 0 |
| [examples/invalid-overclaim.json](examples/invalid-overclaim.json) | 검사기가 **거절해야 하는** 결과. 미실행인데 pass, 자동 판정자, 합성 승인을 사람 승인으로, 구조 검사 거짓 기록, 사례 누락 | 1 |

세 파일의 `run.example`은 true이고, model은 `example-model@0.0-example`처럼 실제 모델이 아니다. 어떤 모델의 성능도 나타내지 않는다.
`guidance`·`case_set` 해시는 예시를 쓴 시점의 저장소 값이며, 사례를 고치면 `case_set` 해시가 달라져 주의 문구가 나온다.
