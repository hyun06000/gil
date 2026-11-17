# Chain closing 오프라인 검사 도구

기준 `bcefbc4` (PR #142 통합 후). [합성 사례](../../scenarios/chain-closing/README.md)의
구조·참조·공개 메시지 순서·승인 판 결속·결과 파일 관측을 검사한다.
Node 표준 라이브러리만 사용한다. 의존성 설치, 모델 호출, Core 실행, 네트워크 접근이 없다.
입력은 데이터이며 명령·링크·파일 경로를 실행하거나 열지 않는다.

```sh
node tools/chain-closing-eval/check.mjs fixtures
node --test tools/chain-closing-eval/*.test.mjs
node tools/chain-closing-eval/check.mjs skeleton --out /tmp/closing-not-run.json
node tools/chain-closing-eval/check.mjs result --result /tmp/closing-not-run.json
node tools/chain-closing-eval/check.mjs result --result scenarios/chain-closing/results/examples/unknown-synthetic.json
# 아래는 의도적으로 종료 코드 1이어야 한다
node tools/chain-closing-eval/check.mjs result --result scenarios/chain-closing/results/examples/invalid-overclaim.json
```

`--cases <dir>`로 복사한 사례 묶음을 검사할 수 있다. `--out <새 JSON>`은 `wx`로만
저장한다. 기존 결과를 덮어쓰지 않는다. 결과 파일의 근거는 내장된 공개 메시지와 events이며,
검사기가 사용자 프로젝트를 다시 읽거나 파일 링크를 따라가지 않는다.

| 종료 코드 | 뜻 |
|---|---|
| 0 | 사례/결과의 일관성이 맞음. 관측된 행동이 fail이거나 평가가 unknown/not_run이어도 0일 수 있음 |
| 1 | fixture 또는 결과의 무결성·선언 불일치. 자동 판정 위조, 근거 인용 오류, 미실행 승격 등 |
| 2 | 사용법·입력 오류, JSON 해석 불가, 입력 파일 부재, 출력 덮어쓰기, 깨진 사례 묶음 위의 결과 검사 |

**종료 코드 0은 제품 UX 합격이 아니다.** 자동 판정은 `automatic`, 의미 판정은 `semantic`에
따로 남는다. 출력은 판정 개수와 검사 위치만 표시하며 공개 메시지 본문을 재출력하지 않는다.

## 재사용과 작은 별도 확장의 이유

- `tools/scenarios/lib/trace.mjs`: `okOf`, `saidOf`. JSON-RPC 오류·Core 거절을 성공으로 세지 않는다.
- `tools/scenarios/lib/report.mjs`: `parseReport`. 기존 `gil_close`의 텍스트 Report를 읽는다.
- `tools/scenarios/lib/markdown.mjs`: 표 출력.
- `tools/chain-interview-eval/lib/io.mjs`: `sha256`, `caseSetDigest`.
- 기존 events 형식(`seq/at/actor/synthetic/request/result/error`)과
  `pass/fail/unknown/not_run`, run/judges/cases/automatic/semantic 구성을 재사용한다.

기존 Interview checker의 출력은 질문·proposal·Define이며 메시지 내부 순서, 파일 관측 시점,
종료 후 인계를 표현하지 못한다. 기존 도구를 수정하지 않고
`gil.chain-closing.case/1`, `result/1`과 **평가 전용** timeline/span/file 관측을 추가했다.
제품 Report/저장 schema나 MCP 도구에 새 필드를 추가하지 않는다.
Core graph·Outcome lineage를 다시 구현하지 않는다. 그 검사는 기존 Core/W18에 맡긴다.

`lib/evidence.mjs`는 읽은 사실과 명시적으로 주석 처리한 주장만 비교한다.
`lib/package.mjs`는 사례와 결과 일관성을 검사한다. `check.mjs`는 CLI다.
`build-fixtures.mjs`는 합성 사례·예시 JSON의 결정적 작성 도구다. 새 도구/사례 디렉터리의
지정된 생성 파일만 덮어쓰므로 실제 평가 결과를 그 `results/examples` 경로에 저장하지 않는다.
fixture 내용·기준을 변경한 뒤에만 실행한다. 자체시험은 별도의 고정된 음성 기대값을 가져
생성기가 자기 판정을 정답으로 만드는 회귀를 잡는다.

```sh
node tools/chain-closing-eval/build-fixtures.mjs
node --test tools/chain-closing-eval/*.test.mjs
```

## 자동화의 한계

- 인용 역할(summary/result/limitation 등)은 평가자가 원문에 붙인다. 자동 검사는 정확한
  부분문자열과 순서만 확인한다. 역할이 의미적으로 맞는지는 사람/모델 판정 대상이다.
- 현재 파일은 **마지막 읽기 관측 시점**의 bytes다. 그 후의 미관측 변경이나 지금 파일의 존재는
  보장하지 않는다. `capture_complete`는 해당 구간의 도구 기록 완전성에 대한 수집자 선언이다.
- 기본 파일 쓰기 탐지는 proxy의 `save_project_file`과 `delete_project_file` 이름이다.
  shell·다른 파일 도구는 `artifact_write_events`에 해당 사건을 명시한다. 누락·불완전 수집이면
  `capture_complete:false`로 두고 “쓰기 없음”은 unknown이다. 새 OS 차단 장치가 아니다.
- 링크가 적혀 있고 bytes가 같아도 Host에서 열리는지는 모른다. 합성 SVG는 화면 사례의
  데이터일 뿐 실제 화면 캡처 증거가 아니다. 실제 평가에서는 앞선 Verify의 실제 PNG로 교체한다.
- proposal digest는 Core가 반환한 불투명 식별자로 비교한다. Core의 정규화·서명·저장 검증을
  재구현하거나 사람 신원을 인증하지 않는다. 이 fixture의 digest도 합성 식별자다.
- 현재 Chain close 영수증 식별은 기준 커밋의 한국어 `said` 형식에 맞춰져 있다. 다른 Host/언어의
  기록은 unknown으로 남겨 출처를 보존하고 수동 대조한다. Verify는 기존 typed confirmed/owner 응답도 지원한다. 알려지지 않은 응답을 번역해 원본처럼 기록하지 않는다.
- 전체 프로젝트·프로세스·실제 화면·모델 행동을 실행 검증하는 runner가 아니다.
  [후속 절차](../../scenarios/chain-closing/PROTOCOL.md)와 [검증표](../../scenarios/chain-closing/VALIDATION.md)를 따른다.

## 원 실행 기록 가져오기

`import.mjs`는 명시적으로 제공한 기록을 `gil.chain-closing.result/1`로 옮긴다.
기준 커밋은 `5d936765634c94b459cee7358b1e82eede49cd14`이며 기존 10개 사례·rubric·
`inspect`는 변경하지 않는다. 자동 점수는 `inspect(bundle)`만으로 계산한다.

```sh
# 합성 pending 예시: 실제 평가나 승인을 실행하지 않는다. 출력 경로는 새 파일이어야 한다.
node tools/chain-closing-eval/import.mjs \
  --input scenarios/chain-closing/imports/pending-synthetic.json \
  --out /tmp/closing-imported-pending.json
node tools/chain-closing-eval/check.mjs result --result /tmp/closing-imported-pending.json
```

[실행 가능한 전체 입력 예](../../scenarios/chain-closing/imports/pending-synthetic.json)는
승인 저장 전 연결이 끊겼고, 이미 수집한 재조회가 pending인 합성 기록이다.
실제 수집이나 입력 파일 생성은 이 명령이 하지 않는다. 수집자가 공개·동의된 자료만 내보내고
토큰·개인정보를 일관되게 가린 뒤 제공한다. 마스킹 뒤 인용 위치와 관측 해시도 해당 제공 자료에
맞아야 하며 원본과 마스킹본을 혼합하지 않는다.

### 입력 구조와 출처 연결

입력 JSON은 `schema: "gil.chain-closing.import/1"`, 명시적 `run`, `cases` 배열이다.
`run`은 [기존 결과 메타데이터](../../scenarios/chain-closing/results/FORMAT.md)를 그대로 쓴다.
`actor:model`이나 App 응답을 보고 `model_executed`·`host_executed`를 만들어 넣지 않는다.
실제 실행 선언은 수집자 책임이며 가져오기 자체가 그 선언의 진실성을 인증하지 않는다.

사례 한 건의 모양은 다음과 같다(아래 `events` 등 변수는 수집된 데이터이며 명령이 아니다).

```js
const identity = {
  case_id: 'CC-09',
  execution_id: 'run-09-attempt-1',
  project_root: '<redacted-project-root>',
};
const source = data => ({ ...identity, data });
const caseInput = {
  ...identity,
  events: { ...source(events), first_seq: 41, last_seq: 47 },
  messages: source(messages),
  timeline: source(timeline), // 수집된 전체 순서가 없으면 이 필드 자체를 생략
  annotations: source(annotations),
  files: source(files),
};
```

- 한 입력에는 사례별 실행 하나만 넣는다. 반복 실행은 별도 출력 파일을 사용한다.
  사례마다 고유한 `execution_id`를 쓰고 각 자료 묶음의 `case_id/execution_id/project_root`가
  일치해야 한다. 사건·메시지에 같은 식별 필드가 있으면 그것도 대조한다.
- `events.data`는 기존 proxy의 `seq/at/actor/synthetic/request/result/error` 사건 배열이다.
  JSONL 수집본은 각 줄을 JSON으로 해석해 배열로 내보내되 번호·actor·응답·시간을 보충하지 않는다.
  공용 `parseEvents`는 누락 번호/actor 기본값을 채우므로 여기서는 사용하지 않는다.
  `first_seq/last_seq`는 **선택한 수집 구간의 명시적 경계**다. 1부터 시작할 필요는 없지만
  구간 안은 연속 번호여야 한다. 빈 구간은 둘 다 null이다. 중복·역순·중간/양끝 누락을 거절한다.
  알려진 중간 누락을 재번호 부여로 숨기지 않는다. 연속 구간 밖의 수집 여부는 인증하지 않는다.
- `messages.data`는 기존 `{id,role,text,synthetic}` 공개 메시지 배열이다.
  `timeline.data`는 `['e41','m1','e42',…]`처럼 모든 사건·메시지를 정확히 한 번 담는다.
  timestamp, 배열 위치, 메시지 번호를 근거로 교차 순서를 추정하거나 정렬하지 않는다.
  제공된 순서의 누락·중복·사건 번호 순서와의 모순은 오류다.
- `annotations.data`에는 기존 bundle의 `presentation/interview_open_event/capture_complete/
  artifact_write_events/closing/resume/handoff`만 넣는다. 원문 역할·결과 주장·승인 대상 연결은
  수집자/평가자가 주석한다. 본문을 NLP로 해석해 주석을 생성하지 않는다.
  `capture_complete`가 없으면 수집 완전성을 주장하지 않는다.
- `files.data`는 기존 파일 관측 배열이다. `verified/current.event`는 원 사건,
  `pointer`는 그 사건의 `/result/…` 위치, `sha256/encoding/state`는 당시 bytes와 읽기 결과다.
  시점은 원 사건의 `at`와 명시적 timeline으로 보존하며 누락 시간을 만들지 않는다.
  해시·경로·상태 불일치는 기존 `file_consistency`의 fail/unknown으로 남는다.
  깨진 pointer·없는 사건·깨진 공개 인용은 가져오기 오류다.

`closing.proposal_event` → `shown`의 판·digest·Synthesis·판정 → `receipt_event` →
`report_event` → `handoff.open_event/read_event` 연결은 기존 필드를 그대로 보존한다.
scope 기반 App 호출은 같은 입력 구간의 앞선 `gil_monitor_prepare`에 연결되어야 한다.
사건 인수의 다른 `project_root` 또는 연결되지 않은 scope는 거절한다. 경로를 canonicalize하거나
현재 디스크에서 프로젝트 정체성을 조회하지 않는다. 동일하게 가려진 식별값을 사용해야 한다.

`resume.read_event`는 **이미 수집한** 재조회다. pending과 saved 응답을 기존 `resume_receipt`로
구별하고, 응답 실패·부재·낡은 판은 그 검사 계약대로 unknown/fail에 남긴다. 연결 끊김 자체는
저장 실패의 증거가 아니며, 원 사건에 없는 승인 저장/취소를 생성하지 않는다. 승인·닫기·조회·복구
요청을 전송하거나 재전송하는 코드 경로는 없다.

### 준비 자료와 평가 결과

- 명시적 timeline이 있으면 원자료를 그대로 bundle로 연결하고 `inspect`를 계산한다.
  status는 **unknown**, 의미 기준 7개도 모두 unknown, judge는 null이다. 자동 pass/fail은
  관측된 형식·관계에 관한 것이며 `evaluated`로 승격하지 않는다. 미제공 사례는 not_run이다.
- timeline이 없거나 null이면 교차 연결을 하지 않는다. 원자료 전체는 아래 출처 영역에 보존하고
  기존 unknown 표현인 빈 bundle을 검사한다. references의 빈 참조 유효성만 pass이고 나머지는
  unknown이다. 이 경우 `import_evidence.cases[].state`는 `preparation`이다.
  순서가 생기면 원 입력을 보완한 별도 입력으로 새 출력 파일을 만든다.
- 출력의 `import_evidence.source`는 제공한 입력 객체 전체다. 각 사례의 `source_pointer`는
  이 객체 안의 원 자료 위치를 가리킨다. `source_sha256`은 `JSON.stringify(input)`의 SHA-256,
  CLI의 `input_sha256`은 읽은 입력 파일 전체 bytes의 SHA-256이다. 서명·출처 인증은 아니다.
  bundle의 사건 번호·메시지 ID·인용·pointer를 변경하지 않아 원문으로 되짚을 수 있다.
- `import_evidence`는 오프라인 출처 보존용 부가 정보다. 기존 result checker는 이 부가 영역을
  검증하지 않고 bundle과 점수만 재검사한다. provenance는 보존한 원 입력 및 해시와 따로 대조한다.
  준비 자료를 기존 checker로 읽어도 실제 평가 완료를 뜻하지 않는다.
- 실제 내용의 충분성, 앱 표시, 결과 파일 열람, 물리적 버튼 클릭, 모델 실행은 추론하지 않는다.
  추후 별도 판정자가 기존 형식의 judge/basis/rationale을 채워 평가한 새 결과 파일을 만든다.

저장된 과거 증거가 존재하는 것, 마지막 읽기 관측에서 파일을 읽을 수 있었던 것, 사용자가
**지금** 결과 파일을 열 수 있는 것은 서로 다른 사실이다. 가져오기는 첫 두 사실의 제공된
관측만 보존한다. 입력의 파일 경로·URL을 열지 않으며 현재 파일을 과거 증거로 다시 읽지 않는다.
저장한 bytes가 있어도 현재 파일 변경·삭제·접근 실패를 덮어쓰지 않는다.

### 출력 안전성과 검증 범위

CLI가 여는 사용자 경로는 명시한 `--input`과 `--out`뿐이다. `--out`은 `wx`와 생성 권한
0600으로 새 파일에만 쓴다. 입력·기존 출력·그 hardlink/symlink를 덮어쓰지 않는다.
출력에는 제공한 원문이 포함되므로 비공개 평가 자료로 보관하며 저장소에 실제 기록을 커밋하지 않는다.
터미널에는 건수와 고정 오류 코드만 출력한다. 원문·토큰·개인 경로·예외 stack은 출력하지 않는다.
입력의 지시문은 문자열로만 복사한다. 코드·셸·Core·모델·Host·네트워크를 실행하지 않는다.

종료 코드 0은 가져오기/결과 형식의 성공이며 자동 fail과 unknown을 허용한다.
1은 출처 식별·범위·참조·메타데이터 불일치, 2는 CLI/JSON/I/O 오류다. 거절 시 결과를 쓰지 않는다.
출력 I/O 자체가 중단되면 부분 파일이 남을 수 있으므로 기존 파일을 덮어쓰지 말고 새 경로로 재시도한다.

합성 시험은 기존 10개 사례의 **모든 good/bad 변형**에서 원 bundle 및 inspect 결과의 정확한 일치를
검사한다. 정상 종료/인계·결과 없음·부분 실패·파일 변경/삭제/접근 실패·승인 저장 전후 유실·낡은 판,
번호/순서/인용/출처 오류, 원 입력 보존과 새 파일 전용 출력, 민감정보 미출력을 포함한다.
새 의존성은 없다. 실제 모델·앱·사용자 프로젝트 실행과 실제 수집 경로 인수는 수행하지 않았다.

통합 담당이 유지/추가할 자동 검사 명령(이 작업은 자동 검사 설정을 수정하지 않음):

```sh
node --test tools/chain-closing-eval/*.test.mjs
node tools/chain-closing-eval/check.mjs fixtures
git diff --check
```

기존 wildcard 명령이 `import.test.mjs`도 포함한다. CLI 왕복과 제공 입력 예 검증도 이 시험에 포함한다.
