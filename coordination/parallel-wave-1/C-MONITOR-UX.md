# C — Monitor 승인·진행 화면

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: C — Monitor 승인·진행 화면
예상 branch: claude/wave1-monitor-ux
결과 파일: coordination/parallel-wave-1/results/C-MONITOR-UX-RESULT.md

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

목표: 비개발자가 MCP App Monitor에서 지금 무슨 상태이며 무엇을 눌러야 하는지 즉시 알 수 있게 한다.
이미 M6-D의 plans.mjs에 승인 버튼과 토큰 검증 흐름이 있다. 이것을 다시 만들거나 다른 서버 API를 만들지 않는다.

먼저 읽을 곳:
spec/GIL_Monitor_Model_v0.1.md(파일명이 다르면 실제 Monitor spec 검색),
spec/GIL_Approved_Plan_v0.1.md, mcp-app/app.js, plans.mjs, host.mjs, monitor.css,
ui/index.html, ui/companion.js, ui/progress.mjs와 현재 테스트.

소유 파일:
- mcp-app/plans.mjs, app.js, monitor.css 및 신규/관련 mcp-app/*test.mjs.
- mcp-app/build.mjs는 필요한 배치 변경만.
- 생성물 plugins/gil-companion-prototype/assets/monitor.html 및 monitor.json.
- coordination/parallel-wave-1/results/C-MONITOR-UX-RESULT.md.
ui/**와 Rust/MCP schema, package.json/lock, notice-policy 파일은 변경하지 않는다.

구현:
1. 승인 대기를 긴 그래프 하단에서 놓치지 않게 상단 안내/바로가기 등 접근 가능한 UI를 제공한다.
   polling이 화면을 강제로 계속 스크롤하거나 사용자가 보던 상세를 뺏지 않게 한다.
2. 요청/승인됨/수정 요청/새 판 재승인 필요/읽기 실패/저장 결과 불확실 상태를 구분한다.
   현재 실행 항목과 남은 항목, 이미 실행한 항목을 읽기 쉽게 보여 준다.
3. Closing proposal의 chain_verdict가 success/failure 중 무엇을 승인하는지 쉬운 문장으로 보인다.
   원문 전체와 참조/판/digest는 검증 가능한 보조 정보로 보존한다.
4. 실제 agent 실행 신호가 없으면 '작업 중'이라고 추측하지 않는다.
   열린 Step, 최근 앱 표시 신호, 사용자 승인 대기 등 관측 가능한 사실만 말한다.
5. 승인 결과를 채팅에 전달하지 못해도 이미 저장된 승인을 다시 보내지 않으며,
   사용자가 이어갈 수 있는 안내를 제공한다. 재조회로 메시지/승인을 자동 전송하지 않는다.
6. trusted click, private token closure, 동일 scope/revision/digest, stale request 거절,
   중복 클릭 방지, 키보드 접근, focus 유지, stop 후 늦은 응답 무시 계약을 유지한다.
7. 실제 host가 없어도 합성 DOM 또는 로컬 fixture의 작은 화면/넓은 화면에서 표시를 점검한다.
   이 시각 검사는 Codex 실기 인수와 별도로 기록한다.

합격:
- UI 상태/재연결/유실 응답/버전 변경 회귀시험과 기존 mcp-app tests.
- 가능하면 개인정보 없는 fixture screenshot을 결과에 연결하고 관측한 화면 범위를 적는다.
- node mcp-app/build.mjs로 생성물을 갱신한다.
- notice-policy의 bundle hash 재생성은 통합 담당자가 A의 의존성 변경과 함께 한다.
  그 차이 때문에 packaging 검사가 실패했다면 정확하게 따로 기록하고 검사를 무력화하지 않는다.
```

