# E — 통합 시나리오·승인 경계 자동시험

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: E — 통합 시나리오·승인 경계 자동시험
예상 branch: claude/wave1-scenarios
결과 파일: coordination/parallel-wave-1/results/E-SCENARIOS-RESULT.md

GIL 병렬 개발 Wave 1의 독립 세션이다. 분석만 하고 끝내지 말고 지정한 산출물과 자동시험까지 완료한다.
다른 세션도 동시에 작업하므로 파일 소유 경계와 기존 기록을 지킨다.

시작:
- 이 프롬프트에 지정한 전용 worktree에서만 작업한다. 다른 worktree의 checkout/reset/stash/clean을 실행하지 않는다.
- 현재 branch와 git status를 확인한다. 예상 branch가 다르거나 기존 변경이 있으면 덮어쓰지 말고 상황을 보고한다.
- 공통 코드 기준 fcd6e5d90363ab880b27ccbd85e7c6936609ef93이 HEAD의 조상인지 확인한다.
  이 커밋은 M6-D 개발 구현을 포함한다. 예전 main으로 바꾸거나 작업 중 다른 세션의 branch를 임의로 merge/rebase하지 않는다.
- AGENTS.md, CONTRIBUTING.md, coordination/parallel-wave-1/README.md, INTERFACES.md 및 담당 명세/코드를 읽는다.
- 설치된 GIL/개인 기록으로 bootstrap하지 않는다. 실제 사용자 프로젝트나 소스 저장소에서 gil start를 하지 않는다.

공통 경계:
- 프로젝트/Chain/Will/Journey/저장 transaction을 바꾸는 공유 파일은 통합 담당자가 관리한다.
  src/project.rs, src/chain.rs, src/store.rs, src/session.rs, src/session/**, src/cycle.rs, src/cycles.rs,
  src/approved_plan.rs, src/approved_plan/**, src/lib.rs, src/mcp.rs, src/mcp/plans.rs, src/mcp/tools.json,
  spec/gil-spec.yaml, .github/**, 배포/업데이트 설정을 수정하지 않는다.
- 담당 목록에 없는 파일은 읽을 수 있지만 수정하지 않는다. 새 의존성/모듈 export/공유 연결이 필요하면
  담당 범위 안에서 우선 진행하고 결과 문서에 필요한 정확한 인터페이스·연결 diff를 제안한다.
- 기존 format 4/5/6/7 자동 이전, product-safety 해제, restore 실행 정책 변경, 자동 승인·가짜 성공은 하지 않는다.
- spec/GIL_Roadmap.md, 다른 세션 결과 파일 및 이 공통 작업 지침은 수정하지 않는다.
- 병합/push/PR 생성/CI dispatch/릴리즈/설치본 업데이트는 이번 세션에서 하지 않는다.
  로컬 branch에 커밋해 통합 담당자에게 넘긴다. 소스 완료와 제품 반영을 구분한다.

검증:
- 작은 관련 자동시험부터 실행한다. Rust build/test 및 해당 공용 실행 파일을 사용하는 runner는 반드시
  python3 coordination/parallel-wave-1/cargo-queue.py -- COMMAND ...
  로 실행한다. 이 큐가 공유 target과 합성 TMPDIR을 준비하고 전체 명령 종료까지 잠근다.
- 직접 cargo를 병렬로 실행하거나 다른 세션의 target/debug/gil을 큐 밖에서 실행하지 않는다.
  큐 대기 중에는 다른 터미널/도구에서 문서 검토 등 독립 작업을 계속할 수 있다.
- JS만 쓰고 native Core를 띄우지 않는 시험은 독립 실행한다. 설치는 각 worktree의 lockfile을 따른다.
- AGENTS.md의 해당 필수 검사도 실행한다. 실패/skip/실행 불가를 통과로 기록하지 않는다.
  공용 생성물/고지 hash가 아직 통합되지 않아 실패한 경우 정확한 항목과 이유를 분리한다.
- 실제 Mac/Windows 설치본·Codex 화면·Luna의 사람 참여 시험은 모든 세션 통합 후에 한다.
  합성 모델 응답, 프로토콜 성공, DOM fixture를 실제 사용자 승인이나 화면 인수라고 주장하지 않는다.

완료 보고:
1. 담당 결과 파일에 구현 범위·변경 파일·검증 명령/결과·남은 연결·발견한 결함을 남긴다.
2. 결과 파일에는 secrets/개인 대화/실제 .gil/개인 절대경로를 넣지 않는다.
3. 로컬 commit SHA, branch, 결과 파일 경로, 다른 세션에 전달할 인터페이스, 막힌 조건을 알려 준다.
4. 요청 범위 밖 중요한 결함은 재현 조건과 영향만 보고하고 남의 소유 파일을 고치지 않는다.

목표: 통합 뒤 사람이 Mac/Windows/Luna로 바로 실검증할 수 있는 재현 가능한 시험 묶음과
그 전에 돌릴 수 있는 프로토콜 회귀 runner를 만든다. 기존 제품 코드를 고치지 않고 결함을 드러낸다.

먼저 읽을 곳:
Roadmap M7, spec/GIL_Approved_Plan_v0.1.md, src/mcp/plans.rs,
src/approved_plan/tests.rs, tests/mcp.rs, 기존 CI/packaging scripts(읽기만).

소유 파일:
- 신규 scenarios/wave1/** (Backend, Data Science, Frontend, Planning Document).
- 신규 tools/scenarios/** 및 그 안의 Node 표준 라이브러리 기반 runner/tests.
- 신규 tests/approved_plan_wire_wave1.rs 및 전용 tests/support_wave1/** (기존 common/mcp.rs는 읽기만).
- coordination/parallel-wave-1/results/E-SCENARIOS-RESULT.md.
제품 source, 기존 tests/mcp.rs, Cargo/Node dependency 파일, .github workflow는 변경하지 않는다.

구현:
1. 네 시나리오에 task, initial-artifacts, human-checkpoints, acceptance, expected evidence를 제공한다.
   Planning은 합성 자료를 사용한 조사 보고서로 만들어 사용자 사례인 'Interview 마지막 노드에서
   조사/작성까지 끝내고 채팅에만 출력'을 실패로 잡는다. 실시간 금융 데이터·실제 개인정보는 쓰지 않는다.
2. 정답 품질과 GIL 절차 준수 점수를 분리한다.
   Monitor 표시 미확인, 승인 없는 실행, Synthesis 열린 채 실행, 한 노드에 여러 목적 혼합,
   파일 증거 없음, 기존 승인 재사용, 재인터뷰 기록 손실을 각각 판정한다.
   관측 로그가 없는 내부 모델 생각/실행을 추정하지 말고 unknown으로 남긴다.
3. 실제 native gil mcp stdio를 띄워 JSON-RPC로 승인 계약을 시험한다.
   공유 target의 다른 branch 바이너리를 시험하지 않도록, Node runner는 같은 큐 명령 안에서
   현재 branch Core를 빌드한 직후 실행한다. runner 내부에서 이미 잡은 큐를 다시 잡지 않는다.
   tools/list, 제안/read, App review의 private _meta, decision, close, 프로세스 재시작을 실제 경로로 통과한다.
   bare approved yes, 다른 scope, 오래된 판/token, 중복 승인, 수정 후 재승인, 잘못된 문서,
   Closing/다음 Chain을 검사한다. 환경 변수로 실제 installed Core를 추측하지 않는다.
4. 사람이 누른 승인과 테스트 harness가 보낸 합성 App 입력을 명백히 구분한다.
   합성 승인 모드는 시험 전용이며 실제 사용자 동의/화면 인수 점수를 얻지 못한다.
   서버의 승인/증거 검증을 끄거나 테스트 전용 우회 tool을 제품에 넣지 않는다.
5. runner는 생성한 임시 폴더만 조작하고, 요청/응답은 token/path/private content를 가려 저장한다.
   process timeout과 자식 정리를 구현한다. 실패 시 출력·재현 절차와 JSON/Markdown 결과를 남긴다.
6. 제품 안전성으로 차단된 restore/revisit은 expected_blocked로 별도 표시한다.
   실행하지 않은 시나리오를 pass로 세거나 사람 확인 칸을 자동 채우지 않는다.
7. 실제 모델 검증용 동일 task/control/GIL 평가표와 반복 실행 절차를 준비한다.
   이 세션에서 비용을 쓰는 모델 호출·VM 실행·설치/배포를 시작하지 않는다.

합격:
- 소형 합성 시나리오의 runner 자체 자동시험, 실제 stdio 승인 경계 회귀 실행.
- 기능 flag/플랫폼 차이에 따른 unsupported/blocked/failed/pass/not_run을 구별한 결과.
- 결과만 보고 어느 단계가 깨졌는지 재현할 수 있다.
- 제품 결함을 찾으면 실패 재현 시험과 결과를 커밋하고 실패를 숨기지 않는다. 수정은 통합 담당자에게 넘긴다.
```

