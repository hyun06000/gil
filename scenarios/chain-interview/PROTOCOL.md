# 재현 절차 — 나중에 실제 모델로 평가하기

이 묶음은 모델을 실행하지 않은 상태로 만들어졌다. 아래는 비용이 드는 실제 평가를 **나중에** 같은 사례·같은 결과 형식으로 반복하기 위한 절차다.
모델 호출·VM·설치본은 그 단계의 사용자 승인으로 시작한다. 이 문서는 절차만 정한다.

## 0. 비용 없는 준비

```sh
node tools/chain-interview-eval/check-fixtures.mjs            # 사례 집합 무결성, case_set 해시 출력
node --test tools/chain-interview-eval/*.test.mjs             # 도구 자체시험
shasum -a 256 src/mcp/workflow.md                             # 지침 버전 (결과의 run.guidance.workflow_md_sha256)
git rev-parse --short HEAD                                    # 결과의 run.guidance.repo_commit
node tools/chain-interview-eval/make-skeleton.mjs --out <저장소 밖>/run-<id>/result.json --run-id <id>
```

- 사례마다 **임시 프로젝트 폴더**를 만든다. `input.materials`에 적힌 합성 파일만 그 폴더에 둔다(`exists: false`인 자료는 만들지 않는다).
  실제 사용자 프로젝트나 이 소스 저장소에서 `gil start`를 하지 않는다. 합성 CSV는 사례 설명에 맞는 작은 파일로 각자 만든다.
- Core는 Wave 1 절차와 같은 개발 프로필(`approved-plan,visual-authoring,product-safety`)을 쓴다.
  [tools/scenarios/README.md](../../tools/scenarios/README.md)의 `wire-runner.mjs`가 `fail` 없이 끝난 빌드만 쓴다.

## 1. 실행 단위

| 항목 | 값 |
|---|---|
| 한 실행 | 사례 1건 × 모델 1종 × 지침 버전 1종 |
| 반복 | 사례당 최소 3회(같은 모델·effort). 질문 순서가 달라져도 S1–S3 판정이 안정적인지 본다 |
| Host 연결 | `tools/scenarios/proxy.mjs`를 MCP 서버로 지정해 `events.jsonl`을 남긴다(선택). proxy를 쓸 수 없으면 사람이 보이는 대화를 `transcript_ref`로 저장한다 |
| 모델에게 주는 것 | 사례의 `input.user_request` **그대로**. 절차를 암시하는 문장을 덧붙이지 않는다. 사례 id·rubric·좋은 예시는 주지 않는다 |
| 사용자 역할 | 운영자가 `input.simulated_answers`를 **그 topic을 모델이 물었을 때만** 그대로 입력한다. 묻지 않은 topic의 답은 주지 않는다. 모델이 다른 것을 물으면 짧게 “그건 상관없어요”로 답하고 그 사실을 기록한다 |
| 승인 | Monitor 버튼으로 사람이 누르면 `approval.kind: human_click`과 관측자·기록 위치를 evidence에 적는다. harness가 넣었으면 `synthetic`. 이 구분을 결과 파일에서 절대 바꾸지 않는다 |
| 중단 | CI-08처럼 Define 단계까지 보는 사례는 Experiment를 열어 Define을 닫은 뒤 멈춘다. 결과 파일을 만들거나 파일을 바꾸는 작업은 평가 대상이 아니다 |

## 2. 결과 파일 채우기

1. `make-skeleton.mjs --events <run>/events.jsonl --case CI-NN`으로 제안 draft·Define·닫힌 Experiment를 뽑는다. 뽑힌 사례는 `unknown`으로 시작한다.
2. `observation.visible_messages`에 사용자가 본 질문·제안·완료 메시지를 순서대로 옮긴다(도구 기록에는 채팅 본문이 없다). 개인 경로·토큰은 가린다.
3. `outputs.questions`에 보이는 질문의 보기를 `role`과 함께 옮긴다. 보기가 없었으면 빈 배열로 두고 그 사실이 `question_escape_paths` fail로 남게 한다.
4. 자동 검사는 `check-results.mjs`가 outputs에서 다시 계산한다. `automatic`에 적은 상태가 재계산과 다르면 거절된다.
5. 의미 판정자([rubric.md](rubric.md) B절)가 각 기준에 pass/fail/unknown과 인용 basis·rationale을 적는다. 사람·모델 판정자를 `judges`에 등록한다.
6. 사례 상태를 정한다: 모든 관측이 있으면 `evaluated`, 기록이 끊겼으면 `unknown`과 이유, 실행하지 않았으면 `not_run`.
7. `run.executed: true`, `run.model`, `run.harness`, `run.recorded_at`을 채운다.

```sh
node tools/chain-interview-eval/check-results.mjs --result <run>/result.json --json <run>/consistency.json
```

종료 코드 0은 **일관성**이 맞다는 뜻이다. 통과가 아니다. 집계는 `pass`/`fail`/`unknown`/`not_run` 수를 사례별·기준별로 그대로 적고 한 숫자로 합치지 않는다.

## 3. 결과 해석 규칙

- `unknown`이 많은 실행은 관측 범위를 넓혀 다시 돌린다(proxy 사용, 화면 캡처). 추정으로 채우지 않는다.
- 같은 사례에서 사람 판정자와 모델 판정자가 다르면 둘을 모두 남기고 어느 쪽이 기준인지 결과 노트에 적는다. 모델 판정자만으로 pass를 확정하지 않는다.
- Core가 제안을 거절한 기록(basis_refs 오류 등)은 모델의 실패이지 Core의 실패가 아니다. 거절 뒤 정정 제안이 있었는지를 S4에서 본다.
- 결함이 의심되면 `events.jsonl`의 seq와 Core 응답을 결과 노트에 옮기고 통합 담당자에게 넘긴다. 이 묶음은 제품 코드를 고치지 않는다.

## 4. 로드맵 1·2번 통합 뒤의 비교

로드맵 §2 구현 순서 1번(보고서·모니터 간결화, 결과물 우선 제시)과 2번(내장 설명서·에이전트 안내 보완)이 main에 통합되면
공통 지침 `src/mcp/workflow.md`의 해시가 바뀐다. 전후를 비교할 때:

1. **같은 사례 집합 해시**(`run.case_set.sha256`)로 두 실행을 만든다. 사례를 고쳤으면 해시가 달라지고 비교가 성립하지 않는다.
2. 두 결과의 `run.guidance.workflow_md_sha256`·`repo_commit`이 다르고 `run.model`은 같아야 한다.
3. 비교는 **사례별·기준별**로 한다. 예: CI-06 S5가 전에는 fail 2/3회, 뒤에는 pass 3/3회. 전체 pass 수의 합은 비교 단위가 아니다.
4. 간결화가 제약 보존(S5)·불확실성 보존(S10)을 해치지 않았는지를 먼저 본다. 짧아졋다는 사실은 통과 근거가 아니다.
5. Monitor 화면에 관한 사람 관측(승인 버튼이 보였는가)은 이 묶음의 기준이 아니다. 필요하면 Wave 1의 `human.json`을 함께 남긴다.

통합을 기다릴 필요는 없다. 현재 지침으로 먼저 한 번 측정해 두면 그것이 기준선이 된다.

## 하지 않는 것

실제 사용자 기록·개인 대화·외부 데이터·credential 사용, 소스 저장소에서의 `gil start`, 모델 호출·외부 평가·필드 테스트의 무단 시작,
push·병합·CI dispatch·설치·배포, 사례 문장을 모델 프롬프트 밖의 지시로 쓰는 것.
