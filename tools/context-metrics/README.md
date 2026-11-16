# tools/context-metrics — 문맥 전달량과 반복 측정 도구

저장된 **합성 도구 호출 기록**(`tools/scenarios`의 `events.jsonl` / `wire-events.jsonl` 형식)을 읽어
`gil_context`·`gil_help` 응답의 크기, 같은 도움말의 반복 조회, 응답에 반복해서 등장하는 동일한 설명 블록,
개선 전후 두 기록의 전달량 차이를 **오프라인**으로 비교한다. Node 표준 라이브러리만 쓴다.

이 도구는 **무엇을 측정했는지 구분하는 것**이 목적이다. 설명서가 짧아졌다는 사실만으로 전체 비용이
줄었다고 말하지 않으며, 제품의 성공·실패를 판정하는 크기 기준을 두지 않는다.

## 실행

```sh
# 기록 하나의 측정 (터미널 요약; --json을 주면 그 위치에만 JSON을 쓴다)
node tools/context-metrics/cli.mjs analyze --events <events.jsonl> [--json <out.json>] [--min-block-chars 40] [--quiet]

# 개선 전후 두 기록의 비교
node tools/context-metrics/cli.mjs compare --before <before.jsonl> --after <after.jsonl> [--json <out.json>] [--min-block-chars 40] [--quiet]

# 자체시험과 합성 입력 재생성 확인
node --test tools/context-metrics/*.test.mjs
node tools/context-metrics/fixtures/generate.mjs --check
```

- 명시한 파일만 읽는다. 사용자 프로젝트, 대화 기록, `.gil`, 환경 변수를 탐색하지 않는다.
- 모델 호출, Core 실행, 앱 연결, 네트워크 접속을 하지 않는다. 기록 안의 명령·문구는 데이터로만 다룬다.
- 파일 쓰기는 `--json <경로>`에만 한다. 종료 코드는 성공 0, 사용법·입력 오류 2이며 측정값에 따른 실패 코드는 없다.
- 출력은 새 파일에만 생성한다. 입력 기록·기존 보고서·링크를 덮어쓰지 않으며, 같은 출력 경로를 다시 쓰려면 새 이름을 지정한다.

## 입력 형식

`tools/scenarios/proxy.mjs`와 `wire-runner.mjs`가 남기는 JSONL 한 줄 = 도구 호출 한 건:

```json
{"seq":1,"at":"…","actor":"model|app|harness","synthetic":false,"request":{"name":"gil_help","arguments":{"topic":"…"}},"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{…},"isError":false},"error":null,"ms":12}
```

`request.name`이 없는 줄, JSON이 아닌 줄, 객체가 아닌 줄은 `unsupported_lines`에 줄 번호와 이유로 남기고
모든 측정에서 제외한다. 빈 파일은 0건 기록으로 정상 처리한다. 응답 상태는 네 가지로 나누고 합치지 않는다.

| 상태 | 뜻 |
|---|---|
| `ok` | `error` 없음, `result` 있음, `isError`·`structuredContent.ok=false` 아님 |
| `error_response` | 도구가 오류 응답을 돌려줌 (`isError:true` 또는 `structuredContent.ok:false`) |
| `rpc_error` | JSON-RPC 오류 (`error` 필드) |
| `missing_result` | `result`와 `error`가 모두 없음 (응답 누락) |

## 측정 단위와 계산 기준

모든 크기는 세 단위를 **함께** 보고하며 서로 더하지 않는다.

| 필드 | 뜻 |
|---|---|
| `utf8_bytes` | `Buffer.byteLength(s, 'utf8')` |
| `code_points` | 유니코드 코드 포인트 수. 이 도구가 **"문자 수"** 라 부르는 값이다. 결합 이모지(ZWJ 열)는 보이는 글자 하나라도 코드 포인트 여러 개로 센다. 자소 묶음(grapheme) 단위는 세지 않는다 |
| `utf16_units` | JavaScript `string.length` |

**토큰 수는 보고하지 않는다.** 토큰 계산기를 쓰지 않았고 실제 모델의 입력 구성을 알 수 없으므로 어떤 필드도 토큰이 아니다.

응답 하나에서 서로 다른 표현을 따로 측정한다. 이들을 더해 "모델에 전달된 양"이라 부르지 않는다.

| 필드 | 대상 |
|---|---|
| `text_response` | `result.content[]` 중 `type:"text"`의 `text`를 `"\n"` 하나로 이어 붙인 문자열. text 부분이 없으면 `null`(0이 아니다). `text_parts`는 부분 수 |
| `structured_response` | `result.structuredContent`를 키 정렬 JSON으로 직렬화한 문자열. 없으면 `null` |
| `said` | `structuredContent.said`. `said_in_text`는 그 문장이 text 응답에 그대로 포함됐는지(같은 내용의 두 표현) |
| `request_arguments` | `request.arguments`의 키 정렬 JSON |
| `record_line` | 기록 파일의 해당 줄 원문 전체 |

기록기가 남긴 표식은 세어서 `markers`와 `limits`에 적는다. `…<truncated N chars>`로 끝나는 문자열은 **남아 있는 부분만**
측정하고 원문을 복원하지 않는다. `<redacted-token>`, `<abs-path>`, `<project_root>` 등 가림 표식이 있으면 측정값은
가림이 적용된 기록의 크기다.

## 반복 측정

- **같은 도움말의 반복 조회**: `gil_help`를 `arguments.topic`(앞뒤 공백 제거)으로 묶는다. topic이 없으면 `(current-state)`.
  `repeated_lookups = 호출 수 − 1`, `distinct_text_responses`는 서로 다른 text 응답 수, `identical_text_repeats`는 응답까지 동일한 반복 수.
  `close`와 `(current-state)`는 **상태 의존**(`state_dependent:true`)으로 표시한다. 상태가 바뀌면 다른 응답이 정상이므로 반복 조회가 그대로 중복은 아니다.
- **동일한 설명 블록**: 단위는 *text 응답의 content 부분 하나를 경계로, 그 안을 빈 줄로 나눈 단락*이다. 블록이 두 content 부분에 걸치지 않는다.
  정규화 규칙은 결과 JSON의 `repetition_candidates.normalization`에 그대로 적는다: NFC 정규화, 앞뒤 공백 제거, 공백 연속(공백·탭·줄바꿈)을 한 칸으로. 대소문자 보존, 토큰화 없음.
  `--min-block-chars`(기본 40 코드 포인트)보다 짧은 단락은 후보에서 뺀다. 두 번 이상 나타난 블록만 보고한다.
- 보고 내용은 `id`(정규화 블록의 sha256 앞 16 hex), `occurrences`, 등장한 기록 건수·`seqs`·도구 이름, 블록 크기, `repeated_size`(첫 등장을 제외한 반복분 = 크기 × (횟수 − 1))다. **블록 본문은 보고서에 넣지 않는다.**
- 전체 text 응답이 동일한 호출 묶음은 `identical_text_responses`에 따로 적는다.

이들은 모두 **반복 후보**다. 승인 경고·거절 안내처럼 필요한 반복을 낭비로 판정하거나 제거를 제안하지 않는다. 판단은 읽는 사람이 한다.

## 비교 (`compare`)

두 기록을 각각 측정한 뒤 **도구별**(`by_tool`), **gil_help 주제별**(`gil_help_topics`), **전체 합계**(`totals`)로 `before / after / delta / ratio`를 적는다.
`ratio`는 기준이 0이면 `null`이다. 모든 행에 호출 수를 함께 두고, 호출 수가 다르면 `call_count_differs:true`와 주의 문구를 붙인다.
**호출 수가 다른 두 기록은 같은 작업의 성능 비교로 단정하지 않는다.** 호출당 평균(`per_call_text_response`)은 참고용이다.
ok가 아닌 응답 수가 다르면 크기 차이가 거절·누락에서 올 수 있다고 적는다.

## 측정할 수 있는 것과 없는 것

| 측정한다 | 측정하지 않는다 / 알 수 없다 |
|---|---|
| 기록에 남은 각 응답의 text·구조화·기록 줄 크기 (바이트·코드 포인트·UTF-16) | 토큰 수, 실제 모델 입력 구성, 전체 실행 비용 |
| `gil_context`·`gil_help` 호출 수와 응답 크기, 주제별 반복 조회 | Host가 응답 중 무엇을 모델에 실제로 넣었는지 |
| 정규화 후 동일한 단락의 반복 횟수와 반복분 크기 | 그 반복이 필요했는지(승인·경고 등)의 가치 판단 |
| 오류 응답·RPC 오류·결과 누락·지원하지 않는 줄의 수 | 절차의 옳고 그름 (기존 `tools/scenarios/scorer.mjs`의 역할) |
| 두 기록의 도구별·주제별·합계 차이와 호출 수 차이 | 호출 수가 다른 두 기록을 같은 작업으로 본 성능 차이 |

## 출력

- 터미널: 한국어 요약. 숫자·식별값·횟수만 적고 응답 원문, 승인 토큰, 개인 경로는 적지 않는다. 입력은 파일 이름(basename)과 내용 sha256으로만 식별한다.
- JSON(`--json`): 키는 영어, 설명 문구(`limits`, `caveats`)는 한국어. 생성 시각이 없어 같은 입력이면 바이트까지 같은 결과가 나온다.

## 합성 입력 (`fixtures/`)

`fixtures/generate.mjs`가 결정적으로 생성한다. 어떤 실제 프로젝트·사람·세션에서도 가져오지 않았다.
`before.jsonl`에는 보고서가 원문·비밀값·경로를 재출력하지 않음을 시험하기 위한 표식 문자열(`SYNTHETIC_SECRET_TOKEN_…`, `/Users/synthetic-user/…`)과
잘림 표식, 오류·누락 응답, 같은 도움말의 동일 반복과 상태 의존 반복, 여러 도구에 걸친 동일한 안내 단락이 들어 있다.
`after.jsonl`은 안내 단락을 뺀 짧은 응답이지만 `gil_context`를 더 자주 다시 읽는 기록이다. `malformed.jsonl`과 `empty.jsonl`은 입력 처리 시험용이다.

예시 결과(`compare --before fixtures/before.jsonl --after fixtures/after.jsonl`): 호출 14 → 8, text 응답 합계 5396 → 1832 B, `gil_help` 5 → 2회(반복 조회 2 → 0),
`gil_context` 2 → 3회로 **늘었고**, 호출 수가 달라 같은 작업 비교로 단정하지 않는다는 주의가 함께 나온다.

## 하지 않는 것

제품 코드(`src/`, `ui/`, `mcp-app/`)와 기존 `tools/scenarios/` 변경, 절차 채점의 재구현, 실제 모델·Core 실행, 자동 검사 편입, 로드맵 갱신.
자동 검사에 넣으려면 `node --test tools/context-metrics/*.test.mjs`를 JS 검사 묶음에 추가하면 되며, 이는 통합 담당의 작업이다.
