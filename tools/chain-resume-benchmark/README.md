# 실제 Core 재개 응답 기준선 v1

기준 제품 소스: `5d936765634c94b459cee7358b1e82eede49cd14`. 제품 변경 없이 새 시험만 추가했다.
`baseline-default.json`과 `baseline-product.json`은 실제 로컬 실행 결과다. 소스 HEAD와
시험·수집기·측정기·지침 내용 SHA-256, 실행 파일 SHA-256, OS/CPU/도구 버전이 들어 있다.
수집 당시 새 시험은 미커밋이므로 `source_dirty: true`; 시험 내용 해시가 정확한 입력을 식별한다.
실행 파일 해시는 같은 소스라도 컴파일러·디버그 경로·빌드 환경에 따라 달라질 수 있다.

## 재현 / 통합 담당이 추가할 검사

저장소 루트에서 실행한다. Node 표준 라이브러리와 잠금 Cargo 의존성만 사용한다.
설치된 Plugin, PATH의 gil, 개인 빌드 큐, 다른 세션 도구는 사용하지 않는다.
출력 디렉터리는 **새 경로**, target은 다른 세션과 겹치지 않는 경로로 지정한다.

```sh
node --test tools/chain-resume-benchmark/*.test.mjs
node --test tools/context-metrics/*.test.mjs
node tools/chain-resume-benchmark/run.mjs default /tmp/resume-default-run /tmp/resume-default-target
node tools/chain-resume-benchmark/run.mjs product /tmp/resume-product-run /tmp/resume-product-target
node tools/context-metrics/cli.mjs analyze --events /tmp/resume-product-run/events.jsonl

git diff --check
```

runner는 각각 아래 Cargo 명령을 실행한다. 출력 디렉터리를 지정하지 않은 직접 시험도 검사는
실행하며 자료를 저장하지 않는다. 통합 CI에는 위 runner 두 명령을 쓰면 수집·보고 경로까지 검사한다.
기존 자동 검사 설정은 수정하지 않았다.

```sh
cargo test --locked -p gil --no-default-features --test chain_resume_benchmark --target-dir <dedicated-default-target>
cargo test --locked -p gil --no-default-features --test chain_resume_benchmark --target-dir <dedicated-product-target> --features approved-plan,visual-authoring,product-safety
```

`tests/support_wave1/mod.rs`를 읽기 전용으로 import한다. 그 클라이언트의
`CARGO_BIN_EXE_gil`은 이 Cargo 실행에서 현재 소스로 빌드한 바이너리의 명시적 경로다.
프로젝트와 설정은 임시 폴더에만 만들고 Drop으로 삭제한다. 읽을 저장 위치도 기존 helper의
플랫폼별 계약을 따른다. 파일 생성은 열린 Verify의 합성 근거와 Core가 반환한 계획 문서뿐이다.
저장 레코드를 생성·편집하기 위해 파일을 직접 쓰거나 도메인 저장 API를 호출하지 않는다.

## 고정 입력 / 호출 범위

최소 입력은 새 Rust 시험의 시나리오와 기존 helper의 `CONTRACT`, `QUESTION`,
`INTERPRETATION`, `draft([A], basis)`다. 합성 질문/답변, 항목 A 하나, 근거 텍스트 하나이며
사람의 승인·모델 행동을 평가하지 않는다. 승인/수정 요청/복구 결정은 실제 App 전용 공개 도구에
넣는 합성 입력이다. 토큰은 Core에서 얻어 사용하고 기록하지 않는다.

언어 `ko`, 기본 기능 없음 / 제품 `approved-plan,visual-authoring,product-safety`를 구분한다.
각 checkpoint에서 기존 프로세스를 종료한 뒤 **서로 다른 새 프로세스 두 개**로 읽는다.
각 프로세스에서 `gil_context({project_root})`, `gil_help({project_root})` 순서로 각 1회 측정한다.
help topic은 생략한다. 제품에서는 이어 `gil_plan_read`를 1회 읽어 판/다이제스트를 확인하지만
그 응답은 측정 합계에 넣지 않는다. `tool_calls`는 설정·전이·검사 호출도 모두 센다.
초기화 handshake는 프로세스당 1회이며 측정에서 제외한다(기본 17, 제품 29개 프로세스).

기본 8 checkpoint(32 측정 호출), 제품 14 checkpoint(56 호출):

- 공통: 미응답 질문; 실패 시도 1·2 각각 열린 Verify/닫힌 Cycle; 긴 시도 3의 열린 Verify/닫힌 Cycle;
  누적 실패 재조회.
- 제품 추가: 제안 승인 대기(판 1), 수정 요청(판 1), 새 판 2 승인 저장 후 재연결,
  Chain Report 대기, 닫힌 Chain, 다음 Chain.
- 기본 구성에는 승인 계획·항목 연결·Chain이 없어 해당 상태는 미지원으로 명시한다.
- 실패는 실제 failure Outcome/Report를 닫고 cycle:C1로 복구해 형제 실험을 연다.
  제품은 `gil_revisit_review` → 합성 App `gil_revisit_apply` 승인 경로, 기본은 `gil_revisit`이다.
  같은 두 실패 이력이 다음 실험과 최종 재조회에서 보존되는지 검사한다.
- 긴 현재 실험의 hypothesis는 `Synthetic observation detail. `를 256번 반복한다.
  Core가 Report를 파싱할 때 끝 공백을 제거하므로 파싱된 본문 전체의 보존을 검사한다.
  짧은 시도와 긴 시도는 이력·상태도 다르므로 그 차이를 길이만의 인과 효과로 주장하지 않는다.

기대 문자열은 **검사 조건**으로만 쓴다. 응답 자료는 JSON-RPC result 그대로 수집하며
기대 문자열을 실제 엔진 응답으로 대체하지 않는다. 단위 시험의 작은 문자열은 보고 함수만 검증한다.

## 보존 검사와 측정 정의

각 checkpoint는 프로세스 간 context text 일치, 현재 Cycle/Step 참조, 실제 저장 판의 revision·
digest·결정, 실행 Cycle의 항목/revision 연결을 검사한다. 상태별 기대 결과/다음 호출,
실패 handoff, 다음 Chain의 이전 handoff 및 `gil_story`/`gil_status` 상세 조회 안내도 검사한다.
독립적으로 읽은 도메인 객체는 oracle일 뿐 응답을 만드는 데 쓰지 않는다.
전체 프로젝트 트리와 저장 디렉터리(객체 포함)를 조회 전후 바이트 단위로 비교한다.
제품 plan read도 이 불변성 검사 안에 있다. settings의 연결 메타데이터는 불변 대상이 아니다.

기존 context-metrics `analyzeRecording`·`measure`를 그대로 재사용한다.
text(content text를 개행 연결), structuredContent, request arguments, event line 크기는
서로 다른 표현이며 합산해서 모델 입력량이라 부르지 않는다. JSON-RPC envelope, 초기화 지침,
설정/전이 응답은 측정 합계에 포함하지 않는다. 지침은 내용 해시로 식별한다.

짧은 요약은 ko의 `ctx.resume.heading`부터 `ctx.resume.details` 끝까지 정확히 잘라 잰다.
앞의 MCP 안내·preamble과 뒤의 next-call/상세 보고서는 제외한다. 전체 text에는 모두 포함한다.
모든 크기는 UTF-8 bytes, Unicode code points(문자), UTF-16 units다.
반복은 기존 측정기의 NFC/공백 정규화, 40 code point 이상 단락 기준을 사용한다.
기준선은 반복 합계와 후보 개수, 상위 5개 후보 식별값을 담는다. 전체 반복 목록은 생성한
`events.jsonl`을 기존 CLI로 읽으면 얻는다. 반복 후보는 낭비 판정이 아니다.

`raw`는 가리기 전 실제 응답/직렬화 이벤트 크기이고 `normalized`는 정확한 임시 프로젝트 경로와
transport scope ID를 각각 `<project_root>`, `<scope_id>`로 바꾼 뒤의 크기다. 시간 가림이나 절단은
없다. 공개용 JSONL은 이 정규화된 실제 응답이다. raw event, 임시 절대 경로, 승인 토큰, .gil,
바이너리, 큰 원시 로그는 커밋하지 않는다. 임시 raw capture는 runner 종료 시 삭제된다.

## 관측 기준선

아래 합계는 고정된 **재조회 두 번을 모두 포함**한 context + help text 응답이다.
기본과 제품의 호출 수·기능이 달라 서로 개선 전후가 아니다.

| 구성 | 측정 호출 | UTF-8 bytes | 문자 | 반복 후보 bytes |
|---|---:|---:|---:|---:|
| 기본 | 32 | 151364 | 107462 | 116583 |
| 제품 | 56 | 253802 | 166892 | 194108 |

제품 context 한 응답의 예:

| 상태 | 전체 bytes | 문자 | 요약 bytes |
|---|---:|---:|---:|
| 미응답 | 5659 | 2978 | 746 |
| 첫 Verify | 6533 | 3725 | 656 |
| 실패 1개 후 Verify | 7291 | 4190 | 656 |
| 실패 2개 + 긴 현재 Verify | 15456 | 12253 | 790 |
| Chain Report 대기 | 7924 | 4895 | 556 |
| 닫힌 Chain | 7697 | 4727 | 556 |
| 다음 Chain | 5012 | 2734 | 500 |

요약이 짧아도 전체 상세 문맥이 함께 전달되는 현재 기준선이다. 실제 토큰 수, 모델 비용,
Host의 실제 입력 구성, 작업 품질, 실제 사용자 필드 테스트는 측정하지 않았다.
크기에 따른 합격 한도·자동 절단은 없으며, 실제 비교 자료 없는 개선율도 제시하지 않는다.

## 실행 기록 / 한계 (2026-11-17)

macOS arm64, Node v26.5.0, Rust/Cargo 버전은 JSON environment 참조.
위 두 runner 및 새 도구 자체시험 3개, 기존 context-metrics 시험 14개, `git diff --check` 통과.
새 Rust 통합시험은 구성별 1개 안에서 각각 8/14 상태를 검사한다.
독립 target 폴더로 실행했으며 네트워크·설치·게시·모델 호출은 하지 않았다.
기존 launcher의 dead-code 경고는 제품 수정 범위 밖이어서 유지했다.

초기 작성 중 실패: 기본 Synthesis의 다중행 basis_refs 문법, 제품 Verify에서 기본 구성의
`gil_close` 문구를 기대한 검사, 긴 hypothesis의 끝 공백까지 비교한 검사.
각각 합법적인 보고서 문법, 해당 상태 안내, 파싱 후 본문 검사로 수정한 뒤 두 구성 모두 재실행했다.
기존 시험·도우미·측정기·제품 코드를 바꾸거나 검사 기준을 낮춰 통과시키지 않았다.
전체 Core 회귀/Companion/Tauri, Windows, 다른 언어, 실제 Host 화면은 이번에 검증하지 않았다.
