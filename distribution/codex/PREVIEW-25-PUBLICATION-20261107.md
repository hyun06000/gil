# Core 배포 버전 표기 수정판 게시 장부 — 2026-11-07

macOS Apple Silicon `0.2.1-preview.25`를 unsigned prerelease로 게시했다.
사용자는 Core 버전·설치 안내 수정, 회귀시험, PR·CI·정상 머지, 새 배포본과 설치본 적용을 승인했다.
실제 Host 필드테스트는 별도이며 Windows owner.14 배포는 유지한다.
사용자 프로젝트 기록·승인·Snapshot·계획 실행 기록은 변경하지 않는다.

## 변경과 출처

- [수정 PR #100](https://github.com/hyun06000/gil/pull/100), build commit `d8ab6866fb7bceba20b44e10aed6089f4a5560ef`.
- 기존 Core는 CLI·MCP·probe에 Cargo 패키지 버전 `0.1.0`을 표시해 실제 preview 배포 버전과 달랐다.
  배포 전에 선택한 버전을 컴파일하고 세 경로와 Companion의 Core 연결 정보에 같은 값을 쓴다.
  실행 시 환경 변수로 바뀌지 않으며 일반 Cargo 빌드는 `0.1.0-dev`로 명확히 구분한다.
- 최종 native 검증은 CLI·probe·MCP가 receipt 버전과 다르면 거절한다. staging은 Core bytes를 보존한다.
  구형 preview.24 이하와 기존 Windows owner.14는 `0.1.0` 한 줄로 설치 실패를 판정하지 않고
  등록된 실행 파일 경로·receipt·manifest·해시로 확인한다. 디스크와 MCP 연결 버전이 다르면 Host를 재시작한다.
- 훅 없는 설치, 기존 GIL 명령만 정확히 제거하는 백업 절차, 비-GIL 설정·권한·sandbox 보존은 유지한다.
  Core 승인·전이·Report·증거·dirty 검사, 저장 형식과 Plugin identity도 유지한다.
- [후보 CI 42351119873](https://github.com/hyun06000/gil/actions/runs/42351119873)의 원본 bytes다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #24](https://github.com/hyun06000/gil-marketplace/pull/24)의 head
  `33905b5873cd04d3b0e3742ac1762849fa5f14e7`에서 [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42351549195)가
  통과한 뒤 정상 머지했다. marketplace/tag commit: `5e03b8dcac8f659b61c35f6902db77de2e530334`.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 한계

- 수정 PR 최신 head `8011acaf7f291e0911cc67f0eae91caab8a39555`의 해당 CI 13개 모두 성공.
  macOS Core·UI·패키징, Windows x64/arm64 default·visual-authoring, macOS/Windows Chain,
  Companion macOS arm64·Windows x64/arm64, Windows 번들·product contract 범위를 줄이지 않았다.
  수동 서명/설치 인수 3개는 원래 조건에 따라 skipped이며 성공으로 세지 않았다.
- 로컬 기본 Core 1,562 passed / 기존 2 ignored, 제품 프로필 및 CLI 버전 회귀 746 passed / 기존 1 ignored.
  두 프로필 all-targets 빌드 통과. 최초 새 시험은 fixture가 미리 `.gil/artifacts`를 만든다는 점을 잘못
  가정해 실패했고, 별도 빈 디렉터리에서 무변경을 검사하도록 고친 뒤 전체 기본 suite를 다시 통과했다.
- 실제 MCP SDK 계약과 동일 Cargo target의 버전 변경·복원 빌드를 통과했다. 런타임 환경 변수로
  버전 위조가 불가능하며, 다른 stamp로 재빌드할 때 캐시가 올바르게 무효화됨을 확인했다.
- UI/MCP App/Plugin JS 185 passed / 기존 5 skipped. 게시 archive를 지정한 패키징·고지 106 passed.
  Windows/CI 계약도 통과했다. 이 로컬 검사는 Windows 실행을 대신하지 않으며 위 native CI를 따로 확인했다.
- 후보 CI의 실제 Core·SDK·버전 일치·압축 왕복·33 tools·내장 UI 검증이 통과했다.
  clean source 730개 파일의 snapshot이 CI receipt와 일치했다. Core·Skill·manifest·고지·실행 권한은
  CI 원본 그대로이며 로컬 재빌드나 기존 릴리스 자산 교체를 하지 않았다.
- 배포 저장소 무결성·raw history 회귀시험 29개 통과. CI probe의 버전이 pin과 다른 경우도 거절한다.
- 정확한 후보 Core의 격리 HOME 무동작 훅·기존 GIL 명령 제거·백업·비-GIL 설정·registry 보존 검증 통과.
  같은 Core의 실제 MCP SDK 시작·재연결 계약, stdio 시나리오 20 passed / 1 expected product-safety block / 0 failed.
- 격리된 SDK에서 preview.24 → preview.25 → preview.24 → preview.25로 읽기·재연결했고 모든 프로젝트
  파일 bytes가 보존됐다. 실제 Host의 Plugin 업데이트/rollback 인수와는 구분한다.
- 공개 HTTPS에서 인증 없이 받은 archive·SHA256SUMS·checks.json·update-record.json은 후보 원본과 동일하다.

## 게시물

[릴리스 v0.2.1-preview.25](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.25)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.25.tar.gz` |
| 크기 | 3,646,256 bytes |
| Archive SHA-256 | `cb9b448d2c5dd3370ae795eb778cbd155ea30df3d8c751658aedb5484ad4a9a4` |
| Core SHA-256 | `6b8bb1a5c5d37cdc2f7343c3dc612bda3c61b9f047c64676b06782c7b1a73ea2` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `4e9a8cebea4e17810adb742e51e2999926ea53553ec5729965065f1c44c846a1` |
| Source snapshot SHA-256 | `9a05a4b251376d3d9851b5863f284c682b7e108015673e69b75467345eed0201` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
preview.24와 compatibility 범위가 같고 migration:requires_review 정책을 유지한다.
기존 기록의 자동 이전이나 실제 Host rollback 쌍의 성공을 주장하지 않는다.

## 설치와 필드테스트

[설치 안내](../../INSTALL.md)는 실제 등록된 Core의 CLI·MCP·probe 버전과 archive·receipt·해시를 함께 확인한다.
Codex는 공식 Plugin CLI, Claude는 완전히 종료한 Desktop의 MCP 경로를 갱신한다.
전역·프로젝트 로컬·Plugin·Skill 훅을 등록하지 않는다. 기존 GIL 훅만 백업·제거하고 다른 설정을 보존한다.
설정 검사·SDK 프로토콜 성공은 실행 중 Host 메모리·모델 동작·화면 인수를 대신하지 않는다.

앱 재시작·새 세션에서 연결 버전, 일반 프로젝트 비간섭, 선택한 GIL 프로젝트의 단계별 안내와 Monitor를
사용자가 확인한다. 실제 Host·승인 UI·fullscreen·사람 버튼 클릭·새 Mac·모델 준수는 성공으로 표시하지 않았다.
