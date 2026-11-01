# E — 공통 승인 프로필 시험·도구 계약·CI

Common instructions: `coordination/parallel-wave-2/README.md` and `coordination/parallel-wave-2/INTERFACES.md`.

예상 branch: `claude/wave2-profile-tests`.
전용 worktree에서 README.md와 INTERFACES.md를 읽고 수정·자동시험·로컬 커밋까지 완료한다.
검토의 승인 lib 12개 실패, Skill catalog 1개 실패를 먼저 재현한다. C/D의 별도 수정 대상과 구분한다.

소유 파일:
- src/mcp.rs의 cfg(test) 모듈만 (production handler 수정 금지).
- src/visual_migration.rs의 cfg(test) 모듈만 (실제 migration 구현 수정 금지).
- tests/common/mod.rs 및 필요한 신규 전용 test fixture/helper.
- plugins/gil-companion-prototype/skill.test.mjs 및 신규 전용 catalog 검사.
- .github/workflows/chain-core.yml의 검증 job/필터만. pinned actions/permissions 유지, 게시 기능 금지.
- 다른 기존 Rust integration test는 프로필 fixture 오류를 직접 재현한 경우 그 fixture/feature 분기만
  수정 가능. C의 guidance_wave1/2, D의 approved_plan_wire_wave1/support_wave1, B의 plans.rs는 제외.
- coordination/parallel-wave-2/results/E-RESULT.md.

작업:
1. 실제 feature별 등록 도구 이름을 정확히 검사한다. approved-plan이 켜질 때만 5개 계획 도구가
   등록됨과 App/model visibility를 검사한다. 기본 profile 표와 feature profile 표를 혼동하지 않는다.
2. 구형 migration fixture는 명시적으로 구형 프로젝트임을 보존한다. 새 프로젝트를 승인 프로필로
   만들고 bare approved:yes로 닫는 모순을 해결한다. fixture를 바꾸려고 제품 승인 검사를 끄거나
   기존 기록을 자동 이전하지 않는다. common의 bootstrap 의미를 전역으로 바꾸지 않는다.
3. 새 승인 프로필에 맞는 fixture가 필요한 곳은 explicit helper를 추가한다. C/D는 baseline API로
   독립 작업하므로 새 helper 사용을 요구하지 않는다. guard 없는 테스트 skip/ignore, 단정 삭제,
   모든 approve 자동 허용으로 통과시키지 않는다. 의도상 해당하지 않는 프로필은 적용 범위를
   문서화하고 다른 프로필에서 해당 행위의 커버리지가 유지됨을 증명한다.
4. Skill 검사에서 gil_plan_*를 통째로 허용하지 않는다. 실제 catalog에 존재하는 정확한 모델 도구,
   capability 조건, App 전용 결정 도구를 모델에게 사용하라고 하지 않음을 검사한다. C의 workflow
   원문/생성 Skill은 수정하지 않는다. 테스트와 제품 catalog가 함께 틀릴 수 있는 self-confirming
   단정만 만들지 말고 이름/visibility/기본 profile 부재를 고정한다.
5. macOS/Windows CI에 승인 lib, C의 guidance 시험, D의 실제 wire 시험과 관련 JS 시나리오 검사를
   연결한다. 경로 필터에 새 테스트/지침/UI/scenario 영향을 포함한다. 검토 가능한 YAML만 변경하고
   dispatch/push는 하지 않는다. C/D가 아직 고치지 않은 baseline 실패는 합류 대기로 정확히 보고한다.
6. 기본 전체 Core 시험과 승인 프로필 전체 검사를 가능한 범위까지 돌려 미발견 실패를 드러낸다.
   자신의 소유 밖 실패는 재현·영향을 기록한다. 최종 통합은 실패 없는 합의된 검사 범위를 명시해야 한다.

검증: 수정한 catalog/visual migration 시험, 기본 전체 Core 시험/빌드, 승인 lib 전체,
기존 chain acceptance 명령, skill/workflow JS 계약. 새 CI의 호출 명령을 로컬 가능한 OS에서 실행한다.
notice-policy hash는 A/B 결과 확정 뒤 통합 담당자가 갱신한다. 이번 baseline의 2개 고지 실패를
무력화하거나 미완성 UI hash를 고정하지 않는다. 실제 Windows CI 성공은 실행 전 주장하지 않는다.
