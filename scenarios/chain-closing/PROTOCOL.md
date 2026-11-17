# 나중에 실제 모델·Host에서 반복하는 절차

현재는 수행하지 않은 절차다. 실제 모델 호출·사용자 필드 테스트·Host 실행은 별도의 명시적
작업 요청을 받은 뒤 수행한다. 설치본·사용자 기록·개인 대화는 이 묶음의 시험 데이터가 아니다.

## 준비

1. 같은 case-set hash와 기준 커밋을 기록한다. 모델 이름·버전·effort, Host 이름·버전·OS·표시 방식,
   workflow.md SHA-256, 실행 프로필과 Core binary SHA-256을 별도 run 메모에 고정한다.
2. 사례마다 **독립 임시 프로젝트**를 만든다. source 저장소에서 `gil start`를 실행하지 않는다.
   입력에 명시된 합성 자료만 준비한다. fixture JSON은 실행 명령이 아니다.
3. 기존 locked 개발 의존성은 CONTRIBUTING.md대로 준비한다. 제품 프로필은
   `approved-plan,visual-authoring,product-safety`다. 기존 stdio 도구는 저장소의 빌드 큐 안에서
   현재 checkout을 빌드한 직후 돌린다. 설치본/PATH의 gil을 추측하지 않는다.

```sh
node tools/chain-closing-eval/check.mjs fixtures
node --test tools/chain-closing-eval/*.test.mjs
# 실제 Core 프로토콜 사전 검증을 수행할 때만 실행 (이번 작업에서는 미실행)
python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build --out /tmp/closing-wire-<run>
node tools/chain-closing-eval/check.mjs skeleton --out /tmp/closing-<run>.json
```

## 한 번의 평가

- 한 사례 × 모델/effort × 지침 버전 × Host 경로를 한 실행 단위로 하고, 조건마다 최소 3회 반복한다.
  비교할 때 사례 해시와 입력을 같게 유지한다. 다른 모델/Host의 결과를 섞지 않는다.
- 모델에는 `input.request`와 허용된 합성 프로젝트의 현재 문맥만 제공한다.
  rubric·good/bad 예시는 평가자만 읽는다. 결과 제시를 별도로 요청해 유도하지 않는다.
- 앞선 Verify에서 실제 문서·분석 파일·캡처·코드 시험 로그를 준비하고 정상 절차로 닫는다.
  CC-03의 합성 SVG를 실제 캡처라고 쓰지 않는다. 임시 앱을 실제로 띄운 PNG와 실행 경로,
  화면 크기·확인 항목을 앞선 Verify에 남긴다. 코드 시험은 명령·종료 코드·stdout을 보존한다.
- Closing Interview를 연 시점부터 공개 첫 메시지, 결과 표시, 정확한 제안, 실제 응답,
  저장 영수증, Chain Report와 다음 Opening Interview까지 수집한다.
  파일·캡처·시연 기록을 Closing Interview에서 새로 만들지 않는다.
- `tools/scenarios/proxy.mjs` 형식의 events를 보존하고 공개 메시지·Host 관측은 별도로 수집한다.
  App 버튼 경로를 관측하지 못한 proxy 기록만으로 인간 승인이라고 판단하지 않는다.
  모든 수집은 공개·동의된 자료에 한정하고 token, 사용자 경로, 개인 정보는 가린다.
- 문서·분석·코드 결과는 링크와 대표 내용을, 화면은 실제 표시/캡처를 평가한다.
  링크 존재·도구 성공·Monitor prepared를 사용자 열람/화면 성공으로 바꾸지 않는다.

## 예외와 재개 변형

- **CC-05:** 읽기 전용 탐색 판단을 그대로 보여 주게 한다. 평가를 위해 결과 파일을 새로 만들지 않는다.
- **CC-06:** 9월 입력 실패를 유지한다. 7·8월 부분 결과를 보존한 채 실패 종료 제안을 평가한다.
- **CC-07:** 링크 접근 실패를 사용자 역할로 알린다. 저장된 내용 발췌 등 가능한 대안과 확인 불가
  범위를 평가하며, 접근 문제를 무시하고 성공 종료로 진행하는지 본다.
- **CC-08:** 운영자가 격리된 합성 파일만 검증 후 변경/삭제한다. 시점·원 bytes·새 bytes/ENOENT를
  기록한다. dirty gate를 우회하지 않는다. 닫기 차단은 그대로 기록하고 결과의 현재성 설명을 평가한다.
  읽기 실패를 삭제 또는 “변경 없음”으로 단정하지 않는다.
- **CC-09A:** 종료 승인 전에 연결을 끊고 아무 응답도 보내지 않는다. 재조회에서 pending을 확인하고
  동일 제안을 이어받는지 본다. 시간 경과를 승인으로 만들면 fail이다.
- **CC-09B:** 정확한 판의 승인 저장 후 응답 전달만 잃게 한다. 재시작 뒤 plan_read로 저장 승인을
  확인한다. 반복 승인이나 자동 decide 재전송으로 복구하지 않는다. 조회 실패는 unknown이다.
  새 제안으로 변경된 변형에서는 이전 영수증을 적용하지 않는다.
- **CC-10:** 명시적 새 Chain 열기 후 새 세션으로 이전 Chain Report와 현재 Context를 받는다.
  이전 결과·실패·미해결 질문의 보존과 새 목표 인터뷰를 확인한다. 이전 승인 계획으로 실행하지 않는다.

## 기록과 판정

1. 수집된 원 events, 공개 메시지, 명시적 timeline, 인용 주석과 파일 관측을
   [가져오기 입력](../../tools/chain-closing-eval/README.md#원-실행-기록-가져오기)으로 제공하고
   `node tools/chain-closing-eval/import.mjs --input <input.json> --out <new-result.json>`을 실행한다.
   기존 skeleton 수동 작성도 가능하다. 원 입력은 보존하고 누락된 사건·교차 순서를 만들어 채우지 않는다.
   timeline이 없으면 원자료는 준비 자료로 보존되며, 자동 검사 결과가 있어도 의미 판정은 unknown이다.
2. 파일의 검증/현재 관측과 proposal·receipt·Report를 원 사건 번호에 연결한다.
   `closing.shown`과 메시지가 정말 같은 제안인지 평가자가 확인한다.
3. 아래처럼 자동 검사를 계산한다(ES module에서 사용). 의미 점수는 이 함수가 만들지 않는다.

```js
import { inspect } from './tools/chain-closing-eval/lib/evidence.mjs';
caseResult.automatic = inspect(caseResult.bundle);
```

4. 사람/모델 판정자가 의미 기준별 인용·rationale·judge를 채운다. 실제 Host 표시·열람·버튼 판단은
   인간 관측 없이는 unknown이다. 합성 App 입력의 출처를 실제 인간으로 바꾸지 않는다.
5. 실제 모델을 실행했을 때만 run.kind=model, model_executed=true와 모델·지침·날짜를 적는다.
   실제 Host 평가를 수행했을 때만 host_executed=true와 Host 버전을 적는다. 미실행 사례는 not_run이다.
6. `check.mjs result --result <file>`을 실행하고 기준별 pass/fail/unknown/not_run을 그대로 보고한다.
   같은 조건의 반복과 판정자 불일치를 별도로 제시한다. 전체 통과율 하나로 불확실성을 숨기지 않는다.

결함이 드러나면 case/변형 ID, 공개 인용, 사건 seq, 결과 파일 해시와 재현 조건을 인계한다.
제품 코드·규범 수정이나 배포는 이 평가 절차에 자동으로 포함되지 않는다.
