# 훅 없는 프로젝트 안내판 게시 장부 — 2026-11-06

macOS Apple Silicon `0.2.1-preview.24`를 unsigned prerelease로 게시했다.
사용자가 설치본·설치 안내·현재 Host의 모든 GIL 훅 제거, PR·CI·정상 머지·배포·설치까지 승인했다.
실제 Host 필드테스트는 사용자가 수행한다. Windows owner.14 배포는 유지한다.
사용자 프로젝트 기록·승인·Snapshot·계획 실행 기록을 변경하거나 소급 보정하지 않는다.

## 변경과 출처

- [수정 PR #98](https://github.com/hyun06000/gil/pull/98), build commit `1ccb7a4c48b45717b95983cdc2e6db2df79ac297`.
- 전역·프로젝트 로컬·Plugin·Skill 훅을 설치하지 않는다. 구형 `hook` 호출도 HOME·cwd·registry·프로젝트 상태를
  읽지 않고 빈 결과로 끝낸다. `config`는 빈 hooks이며 `install --auto`와 자동 셸 래퍼는 폐기했다.
- 새 `uninstall --auto --host …`는 GIL 생성 명령만 제거하고 원본을 private 백업에 남긴다.
  같은 행의 다른 훅·matcher·권한·sandbox를 보존한다. 손상 설정·symlink·잠금 경쟁은 보고하며
  사용자 정의 shell 조합은 자동 삭제하지 않는다. 없는 설정은 만들지 않고 재실행은 변경하지 않는다.
- 성공한 GIL 호출도 Host registry에 프로젝트를 등록하지 않는다. 기존 registry·session/wait·프로젝트 기록은 보존한다.
  다른 세션·프로젝트의 일반 도구 호출·파일 작업·턴 종료에 GIL이 개입하지 않는다.
- 선택한 GIL 프로젝트에는 공통 프롬프트와 시작·재개·Verify 전후 MCP 응답으로 다음 행동을 안내한다.
  파일 작업은 열린 Verify에서 수행하도록 안내하지만 OS 파일 차단이나 모든 모델의 준수를 보장하지 않는다.
  Core 승인·전이·Report·증거·dirty 검사와 `gil_await_user`의 선택적 읽기 계약은 유지한다.
- [후보 CI 42213756425](https://github.com/hyun06000/gil/actions/runs/42213756425)의 원본 bytes다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #23](https://github.com/hyun06000/gil-marketplace/pull/23)의 head
  `ad41657803b9c431b7f5068d0b3ec160fba150d7`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42214489736)가 통과한 뒤 정상 머지했다.
  marketplace/tag commit: `d36f6ba35f9e4e6ee43481580af51f98c0207f29`. main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 한계

- 수정 PR 최신 head `39649495ebed5e0d5f05206e72d2c8b4558f173a`의 CI 10개 모두 성공.
  macOS Core·UI·패키징, Windows x64/arm64 default·visual-authoring, macOS/Windows Chain,
  Companion macOS arm64·Windows x64/arm64 검사 범위를 줄이지 않았다.
- 최종 head의 macOS CI 기본 Core: 1,561 passed / 기존 2 ignored. 로컬 전체 1,560 passed 뒤
  최종 설정 검증을 더하고 관련 22개(제거 13·명시적 진단 9)를 재검증했다. 두 프로필 all-targets 빌드 통과.
  첫 진단 시험의 두 실패는 폐기한 hook 호출과 inspect 오류 반환 기대를 분리해 수정한 뒤 통과했다.
- 로컬 승인 계획 제품 프로필 선택 suite: 745 passed / 기존 1 ignored. 실제 MCP SDK output 계약 통과.
- 후보 CI: 승인 프로필 732 passed / 기존 1 ignored, JS/UI/Plugin 185 passed / 기존 5 skipped,
  packaging 102 passed / archive 미지정 1 skipped, SDK 계약 3 passed. 33 tools·내장 UI·압축 왕복 통과.
- clean source 727개 파일의 snapshot이 CI receipt와 일치했다. Core·Skill·manifest·고지·실행 권한은
  CI 원본 그대로이며 로컬 재빌드나 기존 릴리스 자산 교체를 하지 않았다.
- 배포 저장소 무결성·raw history 회귀시험 28개 통과.
- 정확한 게시 Core의 격리 HOME 정리 smoke: 구형 훅·손상 registry 무동작, 새 훅 등록 거절,
  혼합 설정의 GIL 명령만 제거, 원본 백업·다른 설정·registry 보존과 반복 실행 통과.
- 같은 Core의 실제 MCP SDK 시작·재연결·단계별 안내·선택적 대기 계약 통과.
  stdio 실행 시나리오 20 passed / 1 expected product-safety block / 0 failed.
- 기존 절대경로·cd·symlink·Verify·손상·잠금·sandbox 시험은 개발자 명시적 inspect/run 진단에 남겼다.
  이 시험을 설치된 Host의 파일 보호 성공으로 표시하지 않는다.
- 공개 HTTPS에서 인증 없이 받은 archive·SHA256SUMS·checks.json·update-record.json은 후보 원본과 동일하다.
  새 공개 archive를 지정한 최종 패키징·고지 시험 103개와 JS/UI/Plugin 185개가 통과했다(기존 JS skip 5개).

## 게시물

[릴리스 v0.2.1-preview.24](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.24)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.24.tar.gz` |
| 크기 | 3,647,594 bytes |
| Archive SHA-256 | `6e2d0dc92fefe73eb103c4d85cb1df6bf193146d899294113cb62a7bc534abc5` |
| Core SHA-256 | `2007b3e63aba045193282c51e820936baa90538e24ac6aa42ce46947d7fa3949` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `0b3dfa478f98882005da95cf12fdee971be16c01c636d4d0b6a7e6d3002f1e98` |
| Source snapshot SHA-256 | `f574f2aecf933622e9f5e2f32ad2845949fa3ed24758a453d31b8b7d7bec09ce` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
preview.23과 compatibility 범위가 같고 migration:requires_review 정책을 유지한다.
기존 기록의 자동 이전이나 실제 Host rollback 쌍의 성공을 주장하지 않는다.

## 설치와 필드테스트

[설치 안내](../../INSTALL.md)는 훅 등록 대신 기존 GIL 명령만 백업·제거한다. 다른 사용자 훅·권한·sandbox·
기존 폴더·프로젝트 기록은 보존한다. Codex는 공식 Plugin CLI, Claude는 완전히 종료한 Desktop의 MCP 경로를 갱신한다.
not_installed는 검사한 설정의 결과이며 실행 중 Host의 메모리·모델 동작·화면 인수를 대신하지 않는다.

사용자는 앱 재시작·새 세션에서 일반 프로젝트 개발·설치·웹 조사의 비간섭, 선택한 GIL 프로젝트의
시작·재개·Verify 전후 안내와 Monitor 표시를 확인한다. 파일 쓰기의 강제 거절을 기대하는 시험은 아니다.
실제 Host·승인 UI·fullscreen·사람 버튼 클릭·새 Mac·모델 준수는 이 자동검증에서 성공으로 표시하지 않았다.
