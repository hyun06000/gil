# 시각자료·계획 진행 안내 수정판 게시 장부 — 2026-11-06

macOS Apple Silicon `0.2.1-preview.22`를 unsigned prerelease로 게시했다.
사용자가 PR #93의 머지 뒤 설치본 갱신까지 요청했고 실제 Host 필드테스트는 직접 수행하기로 했다.
Windows owner.14는 유지한다. 프로젝트 기록·승인·Snapshot·계획 실행 기록을 이관하거나 소급 수정하지 않는다.

## 변경과 출처

- [수정 PR #93](https://github.com/hyun06000/gil/pull/93), source/build commit `ebdde810f20c2e5e35e0d6ed6de6e04a4f7e864d`.
- `gil help visuals`에 PNG 두 개의 `attached` 선언과 새 첨부(`new` + `key`), 기존 시각자료 인용
  (`stored` + `owner`/`asset_id`/`sha256`)의 완전한 JSON 예시를 제공한다.
- 잘못된 입력은 값·개인 경로를 노출하지 않는 필드 경로를 안내한다. owner 불일치는 실제 열린 owner를 안내한다.
- `gil_plan_read.next`는 현재 단계에 맞게 안내한다. 승인 문서의 바인딩 시점, Experiment 하나당 계획 항목 하나,
  Step의 `close_cycle`과 Cycle의 `open_child` 의미를 설명한다. 닫힌 기록을 다시 쓰거나 pending을 완료로 만들지 않는다.
- 보고된 document의 뒤늦은 생성은 재현되지 않았다. 최초·후속 실제 응답 자료도 없어 이를 고친 것으로 주장하지 않는다.
- [후보 CI 42181287362](https://github.com/hyun06000/gil/actions/runs/42181287362)의 원본 bytes다.
  `approved-plan,visual-authoring,product-safety`, 33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #20](https://github.com/hyun06000/gil-marketplace/pull/20)의 head
  `af9e853c72f847bf4201c5bb48bac894c932b232`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42182071282)가 통과한 뒤 정상 머지했다.
  marketplace/tag commit: `10f89576df104362e3ff1007487f0dbf1c01c2af`.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 범위

- 수정 PR 최신 head의 CI 11개 성공: macOS 기본 Core·UI·패키징, Windows x64/arm64의
  default/visual-authoring, macOS/Windows Chain, Companion macOS arm64·Windows x64/arm64, 제품 계약.
  별도 설치·서명 인수 job 3개는 건너뛰었고 실제 설치·서명 성공으로 세지 않는다.
- 후보 CI: 승인 프로필 732 passed / 기존 1 ignored, JS/UI/Plugin 181 passed / 기존 5 skipped,
  packaging 102 passed / archive 미지정 1 skipped. native 33 tools·내장 UI·압축 왕복 통과.
- clean source 725개 파일의 snapshot이 CI receipt와 정확히 일치했다.
  Core·Skill·manifest·고지·실행 권한·receipt는 CI 원본 그대로이며 재빌드하지 않았다.
- 배포 저장소 무결성·raw history 회귀시험 28개 통과.
- 정확한 게시 Core의 격리 HOME·합성 프로젝트 stdio wire: 20 passed / 1 expected product-safety block / 0 failed.
- 같은 Core의 보호 smoke: 비연결 Stop·Host 구분, 외부 작업, 보호 root의 절대경로·cd·symlink·이동 거절,
  Claude ask/Codex 안내, 구형 훅 갱신·비-GIL 설정·백업 보존 통과. 실제 Host 이벤트·승인 화면 인수가 아니다.
- 공개 HTTPS의 archive·SHA256SUMS·checks.json·update-record.json을 CI 원본과 대조했다.
- 공개 archive를 지정한 Git 없는 준비·패키징·고지 회귀시험 103개 통과, skip 없음.
  새 worktree의 첫 실행은 개발 의존성 esbuild 누락으로 실패했고, 잠금 의존성 설치 후 전체 재실행했다.
  JS/UI/Plugin 181 passed / 기존 5 skipped, workflow 동기화·diff 검사 통과.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [v0.2.1-preview.22](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.22) |
| Archive | `gil-codex-macos-arm64-0.2.1-preview.22.tar.gz` / 3,498,578 bytes |
| Archive SHA-256 | `789d966ad85bc5108600966a09729d7c8a8b89f7572e02e88e498d8b656599fa` |
| Core SHA-256 | `a08ceec076b5916857076184fd94f108fe5758d74b117d7da2ebe0faef99e95c` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `0975e743b4f5e4967b60127805f4ff17d2c5a8dbe1203275975219f40de5074d` |
| Source snapshot SHA-256 | `8f45359d7e4d1d0ab145f0d701652fbe1577b8b5db48dd672d7e09efd131dfeb` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 `publishable:false`·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리즈·이 장부에 기록한다.
preview.21과 compatibility 범위가 같고 `migration:requires_review` 및
`explicit_contract_and_migration_review_required` 정책을 유지한다. 자동 이전·실제 Host rollback 쌍의 성공을 주장하지 않는다.

## 설치와 필드테스트

[설치 안내](../../INSTALL.md)를 따라 새 버전 폴더를 검증하고 공식 Plugin 명령과 종료된 Claude Desktop의
`mcpServers.gil` 등록을 갱신한다. 기존 보호 훅은 MCP와 같은 새 Core로 갱신하고 백업·비-GIL 설정을 보존한다.
훅이 없던 Host에는 설치 갱신이라는 이유만으로 훅을 추가하지 않는다. 기존 폴더와 프로젝트 기록은 보존한다.
`matches_current_core`는 설정 일치이며 Host 활성화·신뢰·화면 성공을 뜻하지 않는다.

사용자는 새 연결에서 두 PNG의 attached Verify, 필드 오류와 현재 owner, 승인 문서/계획 항목 안내,
Step/Cycle 종료 흐름, 기존 경로 보호와 Monitor 표시를 확인한다. 새 Mac, 실제 Codex·Claude의 훅·승인 UI·
fullscreen·사람 승인 클릭은 이 게시 검증에 포함되지 않는다.
