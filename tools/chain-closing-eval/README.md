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
