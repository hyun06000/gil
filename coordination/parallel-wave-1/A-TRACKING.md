# A — 추적 정책

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: A — 추적 정책
예상 branch: claude/wave1-tracking
결과 파일: coordination/parallel-wave-1/results/A-TRACKING-RESULT.md

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

목표: .git/ignore 정책을 결정적으로 평가하는 엔진과 명시적인 정책 적용 관측 API를 만든다.
현재 artifact/mod.rs는 루트 .gil만 제외하고 is_usable은 Windows 드라이브 문자도 허용한다.
이번에는 기존 observe/capture 의미를 조용히 변경하지 않고 정책이 선택된 새 읽기 경로를 만든다.

먼저 읽을 곳:
spec/GIL_Artifact_Model_v0.1.md, spec/GIL_Storage_Model_v0.1.md,
src/artifact/mod.rs, codec.rs, store.rs 및 관련 관측/Manifest 시험.

소유 파일:
- 신규 src/artifact/tracking_policy.rs 및 해당 모듈 내부/하위 unit tests.
- src/artifact/mod.rs: 모듈 선언, 새로운 명시적 policy-aware 관측 API만. 기존 기본 observe/capture의 의미·EntryPath 규칙은 바꾸지 않는다.
- 신규 spec/proposals/M3_Tracking_Policy_v0.1.md.
- Cargo.toml / Cargo.lock: 새 정책 엔진에 꼭 필요한 의존성에 한해 A만 수정한다. 다른 기능 flag는 변경하지 않는다.
- coordination/parallel-wave-1/results/A-TRACKING-RESULT.md.
src/artifact/codec.rs와 store*.rs는 읽기만 한다. 저장 format/schema는 아직 변경하지 않는다.

구현:
1. 입력은 명시적으로 제공된 ignore 원문·상대 위치·정책 버전이다. 사용자 global gitignore,
   시스템 Git 설정, 네트워크, cwd 추측을 몰래 읽지 않는다.
2. .git은 항상 제외하고 루트 .gil은 기존 내부 저장 경계를 유지한다.
   중첩 .gil·링크·특수 파일은 기존 거절 계약을 약화하지 않는다.
3. 중첩 .gitignore, 상대/루트 anchoring, 디렉터리 규칙, **, 부정 패턴과 우선순위를 구현·시험한다.
   Git 호환을 주장하려면 제외된 부모 디렉터리를 부정 패턴으로 다시 포함하는 제약도 검사한다.
   기존 검증된 라이브러리를 써도 되지만 의존성과 라이선스 변경을 보고한다.
   부분 구현을 전체 gitignore 호환이라고 이름 붙이지 않는다.
4. 정책 fingerprint는 원문·순서·상대 위치·정책 버전에 결정적으로 묶는다. 과거 Snapshot에
   현재 .gitignore를 소급 적용하지 않는 호환 제안을 문서화한다.
5. 동결된 A→B 연결은 INTERFACES.md를 따른다:
   includes_file(relative_path: &str) -> Result<bool, String> 및 policy_id: String.
   B가 이 모듈을 미리 import해야 하는 구조로 만들지 않는다.
6. 정책 관측 결과는 Manifest와 적용 정책 식별자를 함께 돌려준다. 기존 Manifest를 임의 변경해
   과거 Snapshot으로 저장하거나 Session의 기본 capture에 자동 연결하지 않는다.
7. 제외한 경로의 하위 디렉터리 순회를 안전하게 줄이되 중첩 저장소/링크 거절 의미와 충돌하면
   계약에 드러내고 결정 없이 보안 검사를 건너뛰지 않는다.

합격:
- 같은 정책·파일 트리는 같은 결과/fingerprint, 정책 변경은 다른 fingerprint.
- .git, node_modules, 제외/재포함, 중첩 규칙, 한글 경로, 정책 오류, 중첩 .gil·링크 회귀.
- 예전 observe/capture 및 기존 저장 기록은 그대로.
- 새 API가 실제 합성 폴더를 관측하는 시험이 있어야 하며 미사용 타입만 추가하고 끝내지 않는다.
- 제품 기본 capture와 저장 정책 provenance 연결은 통합 단계에 남았다고 명시한다.
```

