# 파일 보호 전용 훅 전환판 게시 장부 — 2026-11-06

macOS Apple Silicon `0.2.1-preview.23`을 unsigned prerelease로 게시했다.
사용자가 진행 차단 제거, 회귀시험·PR·CI·정상 머지·새 배포와 설치 적용까지 승인했다.
실제 Host 필드테스트는 사용자가 수행한다. Windows owner.14는 유지한다.
사용자 프로젝트 기록·승인·Snapshot·계획 실행 기록을 변환하거나 소급 수정하지 않는다.

## 변경과 출처

- [수정 PR #96](https://github.com/hyun06000/gil/pull/96), source/build commit `255ead7b35aca25adf4dfb66adc515b4e507bef0`.
- Stop·wrap_up은 registry나 프로젝트를 읽기 전에 빈 결과로 통과한다. UserPromptSubmit은 지시 주입·대기 초기화를 하지 않는다.
  복구 승인 대기·Cycle 경계·무관한 채팅을 위해 임시 Step/Cycle이나 정확한 마지막 질문을 만들 필요가 없다.
- 모든 등록 프로젝트의 실제 열린 Verify에 따른 파일 보호, 성공한 시작·재개·open·close의 자동 등록은 유지한다.
  세션 ID·외부 cwd·pause는 파일 보호의 면제 조건이 아니다.
- installer는 PreToolUse·PostToolUse를 새 Core로 갱신하고 구형 Stop·UserPromptSubmit의 정확한 GIL 생성 명령만 제거한다.
  같은 항목에 섞인 다른 훅·사용자 설정·권한·백업은 보존한다. 남은 구형 생성 명령은 status에서 갱신 필요로 표시한다.
- `gil_await_user`의 입출력은 유지하되 선택적 읽기 확인으로 안내한다. 기존 registry session/wait 값은 보존하고 진행 강제에 쓰지 않는다.
  실제 사용자 승인·Verify 증거·Chain 완료 요건은 유지한다. 공통 안내와 설치 절차도 같은 정책을 설명한다.
- [후보 CI 42195163054](https://github.com/hyun06000/gil/actions/runs/42195163054)의 원본 bytes다.
  `approved-plan,visual-authoring,product-safety`, 33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #22](https://github.com/hyun06000/gil-marketplace/pull/22)의 head
  `58c68737110483a8c1801896d84991a900c9e87f`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42195969749)가 통과한 뒤 정상 머지했다.
  marketplace/tag commit: `b3e9b6da119ff447a541e02c07281afa088a06ec`.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 범위

- 수정 PR 최신 head `d0cba27bf7fe3cd13e8f58c18b09e6a653602eac`의 CI 10개 성공:
  macOS Core·UI·패키징, Windows x64/arm64 default·visual-authoring, macOS/Windows Chain,
  Companion macOS arm64·Windows x64/arm64. 기존 workflow 검사 범위를 줄이지 않았다.
- 로컬 기본 Core 1,561 passed / 기존 2 ignored, 승인 계획 제품 프로필 선택 suite 744 passed / 기존 1 ignored.
  두 프로필 all-targets 빌드 통과. 최초 두 실행에서 옛 안내 문구를 기대하던 assertion이 각각 실패했고
  선택적 대기 확인·실제 사용자 답변 요건으로 수정한 뒤 전체 재검증했다.
- 후보 CI: 승인 프로필 732 passed / 기존 1 ignored, JS/UI/Plugin 182 passed / 기존 5 skipped,
  packaging 102 passed / archive 미지정 1 skipped. native 33 tools·내장 UI·압축 왕복 통과.
- clean source 726개 파일의 snapshot이 CI receipt와 정확히 일치했다. Core·Skill·manifest·고지·실행 권한·receipt는
  CI 원본 그대로이며 로컬 재빌드나 기존 릴리스 자산 교체를 하지 않았다.
- 배포 저장소 무결성·raw history 회귀시험 28개 통과.
- 정확한 게시 Core의 격리 HOME·합성 프로젝트 stdio wire: 20 passed / 1 expected product-safety block / 0 failed.
- 같은 Core의 보호 smoke: 반복 Stop·wrap_up·UserPromptSubmit 무동작과 legacy wait 보존, 외부 개발·설치 준비,
  보호 root의 절대경로·cd·symlink·이동 거절, Claude ask/Codex 안내, 구형 훅 정리·비-GIL 설정·백업 보존 통과.
- Rust 회귀는 실제 실패 Experiment의 복구 승인 대기와 복구 뒤 경계에서 가짜 Step 없이 턴이 끝나고
  파일 편집은 거절되며 기록·산출물이 그대로인 것을 확인한다. Verify 경계·상태 손상·잠금 경쟁 검사도 유지한다.
- 공개 HTTPS archive·SHA256SUMS·checks.json·update-record.json이 CI 원본과 byte 단위로 일치했다.
- 공개 archive를 지정한 Git 없는 준비·패키징·고지 회귀시험 103 passed / 0 skipped.
  JS/UI/Plugin 182 passed / 기존 5 skipped, workflow 동기화·diff 검사 통과.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [v0.2.1-preview.23](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.23) |
| Archive | `gil-codex-macos-arm64-0.2.1-preview.23.tar.gz` / 3,673,299 bytes |
| Archive SHA-256 | `b0172f58e4546e223678dd51b4115c0d7e9764ae6ace533caa996bc6d9e72fbc` |
| Core SHA-256 | `f157de7e2dfdd6af5461d76a2e926c92b180114d4344a24003a33de46a9c2f07` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `759d6d9b630bff632c65094e5e380fa2f0fa4d0edf87ee63bbd9736830857aab` |
| Source snapshot SHA-256 | `92bbede9f5ed168415f697eb75db9cac67aab4c64748442c87d5fde3da58144b` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 `publishable:false`·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리즈·이 장부에 기록한다.
preview.22와 compatibility 범위가 같고 `migration:requires_review` 및
`explicit_contract_and_migration_review_required` 정책을 유지한다. 자동 이전·실제 Host rollback 쌍의 성공을 주장하지 않는다.

## 설치와 필드테스트

[설치 안내](../../INSTALL.md)에 따라 새 폴더를 검증하고 공식 Plugin 명령과 완전히 종료된 Claude의
MCP 등록을 갱신한다. 사용하는 Host의 두 파일 보호 훅도 같은 새 Core로 갱신한다. 누락된 훅은 표준 설치 절차로
등록하고 옛 GIL 진행 훅만 정리한다. 다른 설정·백업·기존 폴더·프로젝트 기록은 보존한다.
`matches_current_core`는 설정 일치이며 Host 활성화·신뢰·화면 성공을 뜻하지 않는다.

사용자는 앱 재시작·새 세션의 정상 훅 검토 후, 복구 승인 대기와 복구 직후의 채팅 종료·주제 변경,
Verify 밖 직접 편집·셸 쓰기 거절, Verify 안 허용·닫힌 뒤 거절, 새 폴더 자동 등록을 확인한다.
실제 Codex/Claude 훅·승인 UI·fullscreen·사람 승인 클릭과 새 Mac 인수는 이 자동검증에 포함되지 않는다.
