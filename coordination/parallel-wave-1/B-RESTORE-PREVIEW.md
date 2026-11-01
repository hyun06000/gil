# B — 복원 미리보기·경로 안전성

아래 내용을 이 세션의 Claude Code에 전달한다.

```text
담당: B — 복원 미리보기·경로 안전성
예상 branch: claude/wave1-restore-preview
결과 파일: coordination/parallel-wave-1/results/B-RESTORE-PREVIEW-RESULT.md

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

목표: 파일을 바꾸지 않는 복원 미리보기 엔진과 경로/충돌 판정을 구현한다.
현재 restore/plan.rs::steps는 target에 없는 현재 파일을 Delete로 계획한다.
그 함수를 그대로 호출해 이름만 preview로 바꾸지 않는다.

먼저 읽을 곳:
spec/GIL_Artifact_Model_v0.1.md, spec/GIL_Storage_Model_v0.1.md,
src/restore/**, src/artifact/mod.rs, src/windows_fs.rs 및 기존 restore 시험.

소유 파일:
- 신규 src/restore/preview.rs, src/restore/preview/** 또는 전용 tests module.
- src/restore/mod.rs: 신규 읽기 전용 모듈 선언/좁은 연결만.
- 신규 spec/proposals/M3_Restore_Preview_v0.1.md.
- coordination/parallel-wave-1/results/B-RESTORE-PREVIEW-RESULT.md.
기존 plan.rs/apply/rollback/cleanup 코드는 읽기만 한다. artifact/**, windows_fs.rs, Cargo 파일은 A/통합 소유다.

구현:
1. before/target Manifest와 정책 식별자, 포함 여부 callback을 받아 결정적 미리보기를 만든다.
   A의 새 branch를 기다리거나 merge하지 않는다. INTERFACES.md의 Fn(&str)->Result<bool,String>으로
   정책을 주입하고 자신의 합성 callback으로 시험한다.
2. 유지, 생성 예정, 변경 예정, 현재에만 있음(보존 필요), 정책 제외, 충돌/거절을 구분한다.
   현재에만 있는 파일을 삭제 대상으로 만들지 않는다. quarantine 제안과 실제 파일 이동은 구분한다.
3. target/before의 정책 식별자가 없거나 다르면 정상 복원 가능하다고 승인하지 않는다.
   정보성 diff는 제공해도 apply-ready 판정은 거절한다. legacy 호환 문제를 결과에 표시한다.
4. 절대경로/드라이브 상대경로/UNC/device/ADS, .., NUL, 구분자 혼합과 대상 OS의 예약 이름을 검사한다.
   macOS에서 유효했던 과거 경로가 Windows에서 불가능한 경우를 따로 판정한다.
5. 대소문자 충돌·Unicode 동등성·파일/디렉터리 충돌·symlink/reparse·경로 topology 변경을
   알 수 있는 근거와 검사 시점을 구분한다. 문자열 검사만으로 Windows handle/TOCTOU 안전성을 주장하지 않는다.
   실제 FS 조회 adapter가 필요하면 읽기 전용으로 만들고 미지원 환경은 명시적으로 거절한다.
6. 출력에는 근거와 위험 이유를 담되 restore command/제품 복원 허용을 새로 만들지 않는다.
   관측/검증 사이 변동이 있었으면 해당 preview를 무효로 판단할 방법을 제안한다.

합격:
- 호출 전후 프로젝트 파일·state.yaml·객체 저장소가 byte-identical인 합성 시험.
- 현재에만 있는 파일 보존, .git 제외, callback 오류, 다른 정책 ID, unsafe path, case/Unicode collision.
- 같은 입력의 결과 순서가 안정적이며 기존 destructive Plan과 타입/진입점을 혼동할 수 없다.
- 실제 restore/revisit의 제품 차단을 유지한다. 이 엔진을 완성해도 안전 restore 전체 완료라고 보고하지 않는다.
```

