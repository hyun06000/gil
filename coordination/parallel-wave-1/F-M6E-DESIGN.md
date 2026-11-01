# F — M6-E 열린 단계 중단·재시도 계약

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: F — M6-E 열린 단계 중단·재시도 계약
예상 branch: claude/wave1-m6e-design
결과 파일: coordination/parallel-wave-1/results/F-M6E-DESIGN-RESULT.md

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

목표: 증거/Report를 완성하지 못하는 열린 단계에서 어떻게 기록을 보존하고 다음 시도로 갈지
구현 가능한 명세 제안을 만든다. 이것은 설계 세션이며 기존 Core에 새 상태를 몰래 넣는 작업이 아니다.

먼저 읽을 곳:
기본/Node/Time/Will/Existence/Cycle/Storage/Artifact 명세,
spec/GIL_Approved_Plan_v0.1.md, src/project.rs, cycles.rs, session.rs,
approved_plan.rs 및 restore 경계, Roadmap M6-E/M4-R.

소유 파일:
- 신규 spec/proposals/M6E_Open_Step_Interruption_v0.1.md.
- 신규 design-tests/m6e/** (순수 상태 전이 표·JSON fixtures·Node 표준 라이브러리 검사).
- coordination/parallel-wave-1/results/F-M6E-DESIGN-RESULT.md.
기존 명세를 확정본으로 바꾸거나 src/**, schema, CLI/MCP tool, runtime feature를 수정하지 않는다.

설계할 사례:
1. 사용자가 Verify 작업 도중 목적 변경/중단을 요구함.
2. 네트워크/권한/도구 실패로 증거를 완성할 수 없음.
3. agent/session/process가 종료되고 새 세션이 이어받음.
4. 부분 파일은 있지만 결론을 내릴 수 없음.
5. 승인된 항목 A가 실패했고 같은 항목을 다시 시도하거나 새 Interview가 필요함.
6. 승인 요청/수정 요청 중 앱이 끊기거나 사용자가 답하지 않음.

반드시 구분:
- 운영상 기다림/재개 vs 도메인 Node의 종료.
- 실패를 실제 근거로 정상 완결할 수 있는 경우 vs 필수 Report 자체를 만들 수 없는 경우.
- 현재 Artifact와 미확정 dirty 파일, snapshot 확정 권한, Active Will의 처리,
  Journey 증가 여부, 재시도 Cycle의 새 ID/기존 승인 항목 연결, 사람 동의의 재사용 범위.
- 기존 Closed 기록의 불변성과 단일 열린 Cycle, product-safety 차단, old reader/writer 호환.
- 사용자 파일 보존을 위한 임시 격리/운영 기록과 새로운 확정 Artifact의 차이.

산출물:
- 가능한 접근을 2~3개 비교하고 추천 1개를 선정한다.
- 각 전이의 전제·before/after·실패/재시작·증거·human input·금지 동작 표.
- schema/tool 영향과 작은 구현 단계, migration이 필요한지 여부.
- 가짜 success/빈 evidence/자동 승인 없이 진행되는 사례와 명시적으로 막혀야 하는 사례.
- 실행 가능한 순수 모델 fixture 검사로 전이표 자체의 모순을 확인한다.
  이 모델이 실제 GIL Core 구현이라고 표시하지 않는다.
- 사용자/통합 담당자가 결정할 핵심 쟁점은 최대 3개로 압축한다.
  일상적인 문서 선택 때문에 중간에 멈추지 말고 추천안을 끝까지 만든다.

합격:
- 기존 명세와 충돌하는 지점을 숨기지 않은 구현 가능한 제안 + 정상/오류/재시작 fixture.
- 생산 코드, 기존 상태 파일, 설치본은 그대로.
- 최종 상태는 '설계 제안 완료, 결정 및 Core 구현 미완료'다.
```

