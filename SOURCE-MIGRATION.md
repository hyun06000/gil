# GIL source migration

2026-09-28 · 비공개 준비 · 원격 push/공개/정식 배포 전

## 범위

새 정본은 `hyun06000/gil`이다. 기존 개발 저장소의 현재 commit
`741f7c05b933a61be48275afebb6964df41dbca9`에서 선별한 소스로 **독립적인 첫 이력**을 만든다.
이 식별자는 출처일 뿐이며 과거 commit, branch, tag, Git 객체를 이 저장소로 가져오지 않는다.
기존 저장소를 rename하거나 fork하지 않는다.

추적 파일 266개 중 260개를 선별 복사했다. Core/MCP, 시험, 공용 UI, 선택적 Companion,
Plugin adapter, 명세와 배포 도구, MIT LICENSE와 제3자 원문 고지를 보존했다.
복사 후 제품 동작·저장 형식·tool 계약은 바꾸지 않고 개발/공개 준비 문서와 ignore 규칙만 정리했다.

옮기지 않은 파일은 다음과 같다. 원본에서는 삭제하지 않았다.

- `CLAUDE.md`: 옛 개인 bootstrap 대신 이 저장소의 `AGENTS.md`를 읽는 지침으로 새로 작성.
- `distribution/codex/CI-CHECKPOINT-20260928.md`
- `distribution/compliance/AUDIT-20260928.md`
- `distribution/compliance/CHECKPOINT-20260928.md`
- `distribution/compliance/CI-CHECKPOINT-20260928.md`
- `distribution/compliance/DEPENDENCIES-20260928.jsonl`

과거 감사·CI 장부는 기존 저장소의 증거이며 새 저장소의 합격으로 승계하지 않는다.
기존 작업 트리의 미커밋 공개 준비 문서 3개도 원본에 그대로 남겼다.
개인 대화 원본, 실제 `.gil` Project, 사용자 설정, credential, 설치 cache, 생성 executable,
`node_modules`, `target`과 과거 Git 이력은 첫 source tree에 포함하지 않는다.
유출 방지 시험의 합성 경로와 제3자 고지의 저작권자·연락처는 실제 개인 작업 기록과 구분한다.

## 호환성과 이름

제품과 저장소 이름은 **GIL / gil**이다. 기존 Plugin id, manifest/catalog, Skill, application id,
handshake와 합성 fixture는 바이트 그대로 유지한다. 일부 내부의 `ariadne` 식별자를 일괄 치환하면
기존 설정·설치 연결을 끊을 수 있으므로 이름 이전과 별개의 변경으로 다룬다.
설치된 Plugin·Companion·cache·marketplace registry를 변경하거나 재설치하지 않는다.
옛 화면 인수 기록은 제품의 검수 이력으로 남기되, 이번 checkout/Host 설치 인수와 구분한다.

## 새 checkout 검증

첫 source tree는 **263개 파일**이다. 선별 복사한 260개 중 249개는 bytes·실행 권한 그대로이고,
11개는 문서/ignore 정리다. `AGENTS.md`, 새 `CLAUDE.md`, 이 장부 3개를 추가했다.
Core·시험·UI 코드, Cargo 잠금 파일, Plugin manifest/catalog/Skill, 기존 고지는 바꾸지 않았다.

| 검증 | 새 checkout 결과 |
|---|---|
| `cargo test --locked --offline -p gil` | **1,175 passed · 0 failed · 0 ignored**, doctest 포함; `gil-companion` package 전체 시험 아님 |
| `cargo build --locked --offline -p gil --all-targets` | 성공 · warning 0 |
| UI/App/Plugin/packaging/고지 Node 시험 | **104 passed · 0 failed · 4 skipped** (전체 108개) |
| 모의 Host 브라우저 회귀 | graph·detail·fullscreen·hint·재연결·보고서 보존/회복·입력창 여백 통과 |
| 공용 UI selftest | **70/70** 통과 |
| UI 재생성 | embedded HTML/URI가 복사 원본과 동일 |
| 공식 Plugin validator | 통과; marketplace/manifest 내용 및 설치본 변경 없음 |
| 고지 policy 검증 | Core 79 + UI 5 = **84 packages**, 표준 라이브러리 고지 포함 |
| 자동 비밀 탐지 | 선별 source tree, Gitleaks 8.30.1 기본 규칙·repository 예외 없음, 후보 0 |
| 복사/문서 감사 | 실제 개발 경로 패턴 후보 0, 끊어진 상대 파일 링크 0; binary는 기존 gil 아이콘 3개뿐 |
| 기존 저장소 보존 | HEAD·작업 트리 상태 및 미커밋 문서 3개의 SHA-256 변경 없음 |

Node의 skipped 4개는 **legacy JS bridge용 별도 Core sidecar/control release**가 없어서다.
이전 과정에서 생성 binary를 복사하거나 설치하지 않았으며, native MCP 경로는 위 Rust 시험에 포함된다.
이를 108/108 통과로 보고하지 않는다.

첫 sandbox 실행에서는 macOS 파일 감지 시험 2개가 실패했다. 같은 시험을 제한 밖에서 실행해
2개 모두 통과한 뒤 전체 Core 시험도 통과했다. timeout이나 제품 코드를 고쳐 숨기지 않았다.
브라우저 시험도 기본 Playwright browser 미설치 및 sandbox 실행 제한으로 시작하지 못한 시도를
구분한다. 이후 설치된 Chrome을 **임시 프로필/headless**로 실행해 로컬 fixture 시험을 통과했다.
이는 실제 Codex/Claude Host에 새 Plugin을 설치한 화면 검수가 아니다.

첫 commit의 `git diff --cached --check`에는 **원본 그대로인 공백 경고 4건**이 있다:
Host UI 명세의 Markdown hard break 2개, `src/restore/mod.rs` 끝 빈 줄 1개,
`tests/walk.rs` import 끝 공백 1개. source 동치 보존을 위해 재포맷하지 않았으며 clean이라고
보고하지 않는다. 새 문서/ignore 편집에 추가한 공백 오류는 없다.

자동 탐지 후보 0은 모든 비밀·개인정보·권리의 부재를 보증하지 않는다.
과거 Git 이력·외부 artifact·fork/clone은 이번 새 source tree 검사 범위가 아니다.

## 아직 하지 않은 일

- 새 원격 저장소로 push, GitHub CI 실행, 공개 전환, release/marketplace 게시.
- 새 사용자 기계 설치·업데이트·제거/재설치, 실제 Host 화면 재검수.
- Windows 및 Tauri 전용 회귀시험. Core/모의 Host 시험으로 대신 통과 처리하지 않는다.
- 기존 공개 이력 삭제·재작성·force push 또는 비공개 전환.
- 비공개 보안 보고 설정 변경과 확정되지 않은 SECURITY 연락처 게시.

새 저장소는 이미 공개됐던 개인 기록을 회수하는 수단이 아니다. 기존 공개 이력의 정리 대상과
영향 검토는 별도로 유지하며, 그 기록 자체를 이 저장소에 옮기지 않는다.
다음 공개 게이트는 [Open Source Readiness](spec/GIL_Open_Source_Readiness_v0.1.md)를 따른다.
