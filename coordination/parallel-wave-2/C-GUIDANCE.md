# C — 합법적인 다음 호출과 승인 프로필 지침 시험

Common instructions: `coordination/parallel-wave-2/README.md` and `coordination/parallel-wave-2/INTERFACES.md`.

예상 branch: `claude/wave2-guidance`.
전용 worktree에서 README.md와 INTERFACES.md를 읽고 수정·자동시험·로컬 커밋까지 완료한다.
검토 R4와 “D 신규 시험 6개 실패”를 먼저 재현한다. 해당 실패를 통과로 꾸미지 않는다.

소유 파일:
- src/context.rs, src/mcp/guidance.rs, src/mcp/workflow.md.
- tests/guidance_wave1.rs, 필요한 신규 tests/guidance_wave2.rs와 자기 전용 helper.
- plugins/gil-companion-prototype/workflow.test.mjs, sync-workflow.mjs로 생성하는 Skill 영역.
- src/manual/topics/current.md, src/manual/topics/step/verify/close.md의 안내 설명.
- coordination/parallel-wave-2/results/C-RESULT.md.
tests/common/mod.rs와 skill.test.mjs, src/mcp.rs는 E 소유다. 도메인 전이는 변경하지 않는다.

작업:
1. 재인터뷰 요청 보존이 현재 Step의 합법적인 진행을 가리지 않게 한다. Define 닫힘이면 Hypothesis,
   Hypothesis 닫힘이면 Verify, Verify 뒤 Analysis 등 실제 openable/can_close를 따른다.
   Cycle close는 가능한 때만 권하고, 요청은 그동안 보존됨을 알린다. 요청을 자동 취소하거나 현재
   일을 건너뛰어 새 Interview/Experiment를 열지 않는다. F의 중단 기능을 이미 있는 것처럼 안내하지 않는다.
2. context 첫 호출과 tool receipt가 모순되지 않게 관련 상태를 검사한다. 최종 항목 뒤 Closing
   Interview/Chain Report가 필요한 경우 채팅 보고만으로 완료하라고 안내하지 않는지도 점검한다.
3. tests/guidance_wave1.rs의 legacy 의도와 새 승인 프로필 의도를 명시적으로 분리한다.
   legacy를 검사하는 fixture는 해당 형식임을 증명한다. approved-plan 신규 프로젝트는 실제
   propose→합성 Monitor decide→정확한 close_report→Outcome→Cycle 순서로 시험한다.
   feature를 꺼서만 통과시키거나 테스트 전체를 ignore하는 방법은 쓰지 않는다.
4. stdio fixture에 응답 timeout/실패 시 child 정리를 넣고 Windows에서 필요한 환경을 보존하면서
   시험 저장소는 임시 폴더로 격리한다. 실제 사용자 설정·기록을 쓰지 않는다.
5. 정본 workflow가 바뀌면 sync 스크립트로 Skill을 갱신하고 일치를 확인한다. OS별 핵심 규칙을 나누지 않는다.

검증: R4 정식 회귀, guidance/context 관련 기본·chain-core·승인 전체 프로필 시험,
tests/guidance_wave1 및 새 시험을 세 프로필에서 실행, workflow JS·정본 일치 검사, 관련 Core 빌드.
작은 모델 실사용은 이번 세션에서 호출하지 않는다. 시험 결과는 안내 계약 검증이라고 표현한다.
