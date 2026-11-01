# Claude 병렬 작업: M3 추적 경계와 읽기 전용 복원 계획

아래 내용을 Claude에게 전달한다. 이 문서는 업무 요청 초안이며 Claude가 실제 작업을 시작했다는 기록은 아니다.

```text
GIL의 최신 main(최소 3b5341f; PR #62/#63 포함)에서 별도 worktree와 topic branch를 만들어 작업해 줘.
대상 저장소: https://github.com/hyun06000/gil (현재 사용하는 GIL checkout에서 remote를 확인한다).
먼저 AGENTS.md, CONTRIBUTING.md, spec/GIL_Artifact_Model_v0.1.md와 restore 코드를 읽어 줘.

Codex는 M6-D 승인 계획 원장, Monitor 승인 버튼, MCP 승인 통로, 저장/Context 및 로드맵 정리를 맡고 있다.
너는 독립적인 M3 안전성의 선행 조각을 맡아 줘: 추적 경계 계약과 읽기 전용 복원 계획 엔진.

목표:
1. 현재 capture/restore에서 .git, .gitignore, 사용자 미추적 파일, 링크와 Windows 경로가 어떻게 처리되는지 코드로 확인한다.
2. spec/GIL_M3_Tracking_Safety_v0.1.md에 추적 정책 및 구형 Snapshot과의 호환 계약을 제안한다.
   .git 고정 제외, ignore 해석/우선순위, 제외 규칙 변경 시 과거 기록 보존, 현재에만 있는 파일의 보관·격리 정책을 명시한다.
   gitignore 호환을 주장하려면 중첩 규칙·부정 패턴·디렉터리 패턴의 실제 동작을 검증한다.
3. src/restore/preview.rs 등 독립 모듈에 읽기 전용 복원 계획 산출을 구현한다.
   대상 Snapshot과 현재 관측을 비교해 유지/복원/현재에만 존재/제외/충돌·거절을 구분한다.
   현재에만 있는 파일을 자동 삭제하지 않으며, 이 단계는 어떤 파일도 쓰거나 지우지 않는다.
4. tests/restore_preview.rs와 합성 fixture로 원본·저장소 바이트 보존, 경로 탈출·symlink/reparse 거절,
   Windows 대소문자 충돌, 중첩 .gil, ignore 정책 변경 사례를 검사한다. 지원하지 않는 플랫폼 경계는 명시적으로 거절한다.
5. 실제 restore 실행·제품 revisit 차단 해제·설치본 배포는 이번 작업에서 하지 않는다.
   저층 모델만 구현하고 연결이 남았다면 그 상태를 정확히 보고한다.

파일 소유 경계:
- 우선 사용: 신규 src/restore/preview.rs, src/artifact/tracking_policy.rs, 독립 tests, 위 신규 명세.
- src/restore/mod.rs 또는 src/artifact/mod.rs의 모듈 선언/좁은 연결은 필요하면 변경 가능하며 정확한 diff를 보고한다.
- 수정 금지: src/project.rs, src/chain.rs, src/store.rs, src/session.rs, src/cycle.rs, src/cycles.rs,
  src/context.rs, src/mcp/**, ui/**, mcp-app/**, spec/GIL_Roadmap.md 및 M6-D 명세.
- 공유 파일 변경이 반드시 필요하면 임의로 진행하지 말고 필요한 인터페이스와 패치를 별도 메모로 제안한다.

기존 .gil 파일을 변환하거나 실제 사용자 프로젝트를 시험 대상으로 쓰지 않는다.
보안/저장 계약을 조용히 완화하지 않는다. 기존 테스트와 AGENTS.md의 관련 필수 검사를 실행한다.
완료하면 PR을 만들되 자동 병합·배포하지 않는다. 변경 파일, 검증 결과, 미구현 연결과 M6-D와의 충돌 가능성을 보고해 줘.
```
