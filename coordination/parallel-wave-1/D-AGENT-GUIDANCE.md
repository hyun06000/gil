# D — 가벼운 모델용 다음 행동 안내

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: D — 가벼운 모델용 다음 행동 안내
예상 branch: claude/wave1-agent-guidance
결과 파일: coordination/parallel-wave-1/results/D-AGENT-GUIDANCE-RESULT.md

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

목표: Luna 같은 가벼운 모델이 긴 규칙을 추론하지 않아도 현재 상태에서 다음 행동을 알게 한다.
서버 계약은 유지하고, 공통 지침·Context·tool receipt 안내를 짧고 일관되게 개선한다.

먼저 읽을 곳:
spec/GIL_Approved_Plan_v0.1.md, Cycle/Context/Will 명세,
src/mcp/guidance.rs, workflow.md, src/context.rs, src/manual/**,
src/mcp/plans.rs와 tools.json(읽기 전용), plugins/gil-companion-prototype/sync-workflow.mjs.

소유 파일:
- src/mcp/guidance.rs, src/mcp/workflow.md, src/context.rs.
- src/manual/topics/**의 설명만; router/schema/새 command는 추가하지 않는다.
- plugins/gil-companion-prototype/skills/gil-companion/SKILL.md의 생성 영역은 sync 도구로만 갱신.
- plugins/gil-companion-prototype/workflow.test.mjs, 전용 tests/guidance_wave1.rs 또는 context 내부 unit tests.
- coordination/parallel-wave-1/results/D-AGENT-GUIDANCE-RESULT.md.

구현:
1. 상태별로 가장 중요한 다음 호출/기다림을 하나 먼저 말하고, 성공한 동작을 반복하지 않게 한다.
   새 시작→Monitor 요청→Interview, Synthesis 제안→사용자 버튼 대기→close_report→Outcome→Interview close,
   Experiment 항목→Define/Hypothesis→Verify→파일 증거→Analysis→Outcome→Cycle close를 구분한다.
2. approved-plan 지원 여부는 프로젝트 Context/응답과 연결 도구 schema로 판단한다.
   옛 프로젝트에 gil_plan_*를 무조건 요구하거나 OS 이름으로 기능을 추측하지 않는다.
3. M6-D 제안은 scope 5개 필드, items(id/question/done_when), synthesis를 사용한다.
   exploration_checklist는 ID를 같은 순서로 한 줄씩 적고 approved/plan_revision/plan_digest는 모델이 만들지 않는다.
4. 버튼을 기다리는 동안 허용 범위를 넘는 조사·보고서 작성을 하지 않는다.
   승인 후에도 마지막 Synthesis에 머물지 않고 별도 Outcome와 Interview Cycle을 닫아야 함을 명시한다.
5. Verify의 계획 문서와 실제 작업 증거는 구분한다. UTF-8 문서 bytes 보존, plan_document/evidence_files,
   채팅만으로 완료 불가, Analysis/Outcome의 근거 연결을 짧은 성공/거절 예제로 설명한다.
6. 시작 중복, 이전 승인 재사용, 승인 후 제안 수정, 현재 Cycle의 재인터뷰 요청,
   blocked revisit, 저장 결과 불확실 등 실패 복구 안내를 현재 사실과 연결한다.
7. workflow.md를 정본으로 두고 sync-workflow.mjs --write 및 일치 시험을 실행한다.
   사실을 확인하기 위해 도구를 무한 반복시키거나 '항상 동작한다'는 보장을 만들지 않는다.

합격:
- 상태별 fixture가 올바른 다음 행동을 포함하고, 금지된 지름길을 제안하지 않는지 검사한다.
- 기존 legacy Context 의미/형식을 불필요하게 깨지 않는다. 기대 문자열만 새 코드에 맞춰 덮는 시험은 피한다.
- Mac/Windows/Plugin/직접 MCP가 같은 핵심 원문을 공유한다.
- 이 시험은 지침 회귀이며 실제 Luna 실행 결과로 보고하지 않는다. 실제 모델 평가는 E가 준비하고 통합 후 수행한다.
```

