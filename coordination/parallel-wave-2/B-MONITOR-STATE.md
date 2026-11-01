# B — 실제 Cycle 상태를 반영하는 Monitor

Common instructions: `coordination/parallel-wave-2/README.md` and `coordination/parallel-wave-2/INTERFACES.md`.

예상 branch: `claude/wave2-monitor-state`.
전용 worktree에서 README.md와 INTERFACES.md를 읽고 수정·자동시험·로컬 커밋까지 완료한다.
검토 R3와 regressions.rs의 closed-cycle binding 관측, probes.mjs의 current 오판을 먼저 재현한다.

소유 파일:
- src/mcp/plans.rs: view의 read-only current_cycle 추가 및 그 projection 전용 시험만.
  catalog/입력/grant/승인 결정/저장 동작은 수정하지 않는다.
- mcp-app/plans.mjs, plans.test.mjs, 필요한 monitor.css. 다른 UI 구조 변경은 하지 않는다.
- 생성 plugins/gil-companion-prototype/assets/monitor.html 및 monitor.json.
- spec/GIL_Approved_Plan_v0.1.md의 읽기 응답 설명만.
- coordination/parallel-wave-2/results/B-RESULT.md 및 합성 화면 검증 산출물.

작업:
1. INTERFACES에 고정한 current_cycle {ref,state}를 실제 Core 상태로 투영한다. current_item은
   닫힌 Cycle에도 남는 binding이므로 기존 의미를 바꾸지 않는다.
2. UI는 실제 open/closed를 구분한다. 닫힌 Cycle을 “지금 열린 Cycle”로 세거나 강조하지 않는다.
   옛 서버/지원하지 않는 응답/알 수 없는 state에서는 열림을 추정하지 않는다.
3. 제안 없음, 실행 항목 있음, Cycle 닫힘, 다음 Cycle 열림, 재인터뷰, Closing 이후의 화면을 점검한다.
   실행됨과 성공함은 다르며 성공 verdict가 없는 항목을 성공 완료로 표시하지 않는다.
4. 기존 승인 token의 closure 보관, trusted click, 중복 클릭 차단, 불확실 응답, 재전송의 읽기/쓰기
   분리, focus와 좁은 화면을 보존한다. 작업 중이라는 상태를 모델 활동으로 추측하지 않는다.
5. 실제 Core의 열린/닫힌 상태 projection 시험과 UI 소비자 시험을 모두 추가한다. mock에만 새 필드가
   있고 실제 Core는 다른 값을 보내는 구조를 금지한다. 생성 bundle은 정해진 build.mjs로 재생성한다.

검증: plans 시험 및 전체 JS 회귀, 새 서버 projection 시험을 approved-plan 프로필로 실행,
기본/승인 프로필 Core build --all-targets, 생성물 재생성 일치. Rust/native는 Wave 2 큐 사용.
fixture 화면 검증과 실제 Codex 인수는 구분한다. notice-policy는 수정하지 말고 변경 hash를 보고한다.
