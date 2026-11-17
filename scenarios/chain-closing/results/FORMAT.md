# 결과 형식 `gil.chain-closing.result/1`

기존 Interview 평가의 run/judges/cases/automatic/semantic 구성과 판정 어휘를 재사용한다.
종료 전후의 관측을 표현하기 위해 별도 schema를 쓴다. 제품 저장 형식이 아니다.

## 실행 메타데이터

`run`에는 id, example, kind(`synthetic|model`), model_executed, host_executed,
recorded_at, model(name/version), host(name/version), baseline_commit(40자리), guidance_sha256,
profile, case_set_sha256을 남긴다. 실제 모델 실행이면 날짜·모델·지침 해시가 필수다.
실제 Host 실행이면 Host·버전을 적는다. 합성 실행은 두 executed 값이 false이며 model/host는 null이다.
case-set 해시 불일치는 오류다. 사례를 고친 결과와 이전 결과를 같은 조건의 실행으로 합치지 않는다.

`judges`는 `{id,kind:"human"}` 또는 `{id,kind:"model",model:{name,version}}`다.
자동 검사기를 의미 판정자로 등록할 수 없다. 판정자를 여러 명 썼으면 독립된 결과 파일을 남긴다.

## 사례 결과

사례 10건이 정확히 한 번씩 들어간다.

| 필드 | 값 |
|---|---|
| case_id | CC-01 … CC-10 |
| status | not_run / unknown / evaluated |
| reason | 실행하지 않음·관측 부족·평가 범위를 설명하는 비어 있지 않은 문자열 |
| bundle | 아래의 공개 근거 묶음, not_run이면 null |
| automatic | `inspect(bundle)`가 산출한 `{check,status,detail}` 배열. 검사기가 다시 계산해 비교 |
| semantic | 모든 rubric 기준에 `{criterion,status,judge,basis,rationale}` 하나씩 |

`not_run`에는 bundle과 자동 검사가 없고 의미 기준도 모두 not_run이다.
`unknown`은 일부 자동 사실이 확인됐어도 의미 판단은 unknown/not_run이다.
`evaluated`는 근거를 평가한 상태이지 통과라는 뜻이 아니다. 의미 기준별 unknown을 허용한다.
자동 fail도 결과 형식이 정합하면 일관성 checker의 종료 코드는 0이다.

의미 pass/fail에는 등록된 judge ID, 원문 인용 basis와 rationale이 필수다.
`basis`는 `{message:"m1",start:0,quote:"원문의 정확한 부분"}` 배열이다.
start는 JavaScript UTF-16 문자열 인덱스이며 `text.slice(start,start+quote.length)`로 확인한다.
가상 인용·어림잡은 줄 번호·없는 판정자·중복 기준은 거절한다.

## bundle: 원 기록과 평가 주석

- `events`: 기존 tools/scenarios events의 도구 호출 envelope를 그대로 복사한다.
  seq는 양의 정수로 증가하고 actor/synthetic을 유지한다. 이 묶음의 fixture는 전부 synthetic:true다.
  실제 proxy 기록의 `actor:model`만 보고 모델 실행으로 판정하지 않는다(W18 harness도 그 actor를 쓴다).
- `messages`: `{id:"m1",role:"assistant|user|observer",text,synthetic}`. 공개 출력/동의된 관측만 넣는다.
  실제 사람의 화면 관측은 observer, synthetic:false로 구분한다.
- `timeline`: `['e1','e2','m1','e3',…]`. 모든 event/message를 정확히 한 번 포함한 공통 순서다.
  원 수집 자료로 확인하지 못한 교차 순서를 만들어 넣지 않는다. 부족하면 unknown으로 남긴다.
- `presentation`: `{kind,message,start,quote}` 인용 배열. kind는 summary/result/verification/limitation/question,
  필요한 경우 no_artifact/fallback/historical. no_artifact는 실제로 결과 파일이 없다고 설명하는 인용이다.
  의미 역할은 사람이 주석하며 자동 검사는 문장 의미를 추측하지 않는다.
- `interview_open_event`: 성공한 `gil_open(kind:interview)` 사건 번호.
- `capture_complete`: Closing Interview 구간의 공개 메시지/외부 작업 도구 수집이 완전한지 선언.
  shell 등 다른 도구의 파일 변경 사건은 `artifact_write_events:[seq,…]`로 표시한다.
- `closing`: proposal_event, receipt_event, report_event, scope_event 번호(관측 없으면 null).
  scope_event는 `gil_monitor_prepare`의 scope_id와 project_root 연결이다.
  `shown:{revision,digest,synthesis_ref,chain_verdict}`는 화면/공개 메시지에 대응하는 정확한 승인 대상의
  평가 주석이며 `span`이 그 문장을 인용한다. 내부 digest를 사용자 문장에 반드시 출력하라는 규칙이 아니다.
  저장 영수증은 성공한 plan_read/review/decide의 proposal.receipt로만 확인한다.
- `files`: 각 `{path,item_id,claim,span,verify_event,verified,current}`. item_id는 종료 제안에 포함된 승인 항목에 연결한다.
  claim은 current_verified/historical/unverified/unavailable.
  verify_event는 앞선 성공한 Verify close이며 evidence_files에 path가 있어야 한다.
  verified/current는 `{event,pointer,state,encoding,sha256}`다. pointer는 해당 원 도구 사건의
  `/result/…` JSON Pointer이며 UTF-8 텍스트 또는 base64 bytes를 가리킨다. 원 도구 인수의 path도 같아야 한다.
  present이면 encoding(utf8/base64)과 bytes의 SHA-256이 필요하다. missing/inaccessible이면 오류 응답의
  정확한 설명 위치를 가리킨다. 오류 종류의 의미적 분류는 판정자가 대조한다.
- `resume`: `{read_event,claim:"pending|saved",span}`. span은 재조회 이후 공개한 재개 메시지의 정확한 인용이다. 읽기 실패/다른 판이면 unknown.
- `handoff`: `{open_event,read_event,span}`. 새 Chain의 open, fresh plan_read와 공개 새 인터뷰를 연결한다.
  이 관측 구간은 다음 Opening Interview까지다. 그 이후 정상 승인·실행은 별도 실행 구간으로 평가한다.

검사기는 참조된 bytes만 해시로 비교한다. 파일 링크를 따라가거나 실제 파일을 다시 쓰지 않는다.
원래 도구가 bytes·path를 이 형태로 제공하지 않으면 형식에 맞는 응답을 꾸미지 않는다.
해당 파일 검사는 unknown으로 두고 원 로그를 별도 보존하거나, 앞선 Verify에서 실제 읽기 도구로
필요한 관측을 추가한 다음 별도 실행을 기록한다. 현재 캡처가 base64면 원 byte 인코딩을 보존한다.

합성 fixture는 **관측 구간의 최소 발췌**이며 실행 가능한 전체 Core trace가 아니다.
중간 Question/Outcome 전이의 정당성은 Core 시험이 담당한다. 발췌에서 실제로 기록되지 않은
승인을 복원하거나 필수 상태 전이를 성공했다고 생성하면 안 된다.

## 예시

| 파일 | 내용 | 기대 종료 코드 |
|---|---|---|
| [not-run.json](examples/not-run.json) | 10개 사례 모두 not_run, 70개 의미 기준 not_run, 모델/Host 미실행 | 0 |
| [unknown-synthetic.json](examples/unknown-synthetic.json) | CC-09의 합성 pending 관측만 있음. 의미 7 unknown, 나머지 63 not_run. 모델/Host 미실행 | 0 |
| [invalid-overclaim.json](examples/invalid-overclaim.json) | not_run인데 pass를 기록하고 판정 근거/판정자도 없음. 거절 예시 | 1 |

이 예시들은 실제 모델 성능 결과가 아니다. synthetic 자료에 대한 자체시험 성공도 제품 사용자
경험의 성공이 아니다.
