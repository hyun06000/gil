# 복구 완료의 채팅 전달 시험판 게시 장부 — 2026-11-02

macOS Apple Silicon `0.2.1-preview.10`, Windows 11 x64 `0.1.1-owner.9`.
사용자가 소스 PR 병합, 양쪽 테스트 설치본 배포와 이 Mac의 Plugin 업데이트를 요청했다.
기존 배포물과 사용자 프로젝트·실패 기록·증거·승인 이력은 바꾸지 않는다.

## 변경과 경계

실제 Monitor 복구 버튼 클릭 뒤 Core가 `completed`를 반환하면 Host `sendMessage`로
같은 프로젝트의 `gil_context`를 다시 읽고 이어가라는 요청을 한 번 보낸다. token, 파일 경로,
보존본 세부 내용은 메시지에 넣지 않는다. 메시지는 승인 증거가 아니며 Core receipt가 기준이다.

전달 실패·불확실한 전달에는 **채팅에 계속 요청 보내기** 버튼으로 메시지만 재시도한다.
복구는 반복하지 않는다. 자동 polling·재열기·거절된 복구·응답 불명인 apply는 자동 전송하지 않는다.
공통 지침은 승인 대기 뒤 “계속/눌렀어/완료” 응답을 받으면 현재 기록을 먼저 다시 읽도록 한다.
특정 문구를 요구하거나 채팅을 실제 승인으로 바꾸지 않는다. 이미 복구된 과거 Cycle을 다시
복원할 필요는 없다. 기존 복구 계약은 [명세](../../spec/GIL_Revisit_Recovery_v0.1.md)를 따른다.

## 소스와 검증

- 소스 [PR #69](https://github.com/hyun06000/gil/pull/69), 검증·후보 빌드 head `44b8b7440495702efd9b8406c402ee95cb285ce6`.
- 소스 merge `c5248b61ec024ccb70cd69e2551d32e64c22dc35`. 빌드는 merge 전의 동일 PR head이며 merge에서 다시 빌드했다고 주장하지 않는다.
- Claude 호스트 지원 PR #66은 포함하지 않는다.
- 기본 Core 로컬 순차: 1,519 통과 / 2 기존 ignored. all-target 빌드 통과.
- 실제 MCP 복구 wire: 로컬 7 통과. 사후 `gil_context`가 반복 승인 대기가 아닌 다음 Cycle을 안내함을 검증한다.
- JS UI/Plugin: 153 통과 / 4 skipped. 메시지 실패·재전송·중복 클릭·polling·불확실한 응답을 검증한다.
- Mac 배포 저장소: 무결성·raw history 28 통과. exact downloaded Core 기존 wire 20 통과 / 1 예상 차단.
- 최종 배포·고지·Git 없는 준비 시험: 132 통과 / 0 실패 / 0 skipped. 실제 공개 Mac archive를 사용했다.

| 검증 | CI |
|---|---|
| Mac/Windows Chain 및 복구 | [41348986653](https://github.com/hyun06000/gil/actions/runs/41348986653) |
| Mac 기본 배포 | [41348986677](https://github.com/hyun06000/gil/actions/runs/41348986677) |
| Companion Mac/x64/ARM64 | [41348986717](https://github.com/hyun06000/gil/actions/runs/41348986717) |
| Windows x64/ARM64 default/visual | [41348986651](https://github.com/hyun06000/gil/actions/runs/41348986651) |
| Mac 실제 후보 | [41348988075](https://github.com/hyun06000/gil/actions/runs/41348988075) |
| Windows 제품 계약·실제 후보 | [41348989801](https://github.com/hyun06000/gil/actions/runs/41348989801) |

## Mac 배포물

- [배포 PR #14](https://github.com/hyun06000/gil-marketplace/pull/14), tag commit `77e845c02b13a883a61d70e86bd69847d06763ef`.
- [고정 prerelease](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.10).
- archive `gil-codex-macos-arm64-0.2.1-preview.10.tar.gz`, 3,537,685 bytes.
- SHA-256 `803395f40ca59dde28b9faa8e83c1b3bed82e7c423b41f2a490e10e8e68fefd6`.
- Core `f4a137c07591eb16fd45f671fc375676099e8444c9ae716eaf3d3a0acfdb0314`.
- UI `9ebd92f3de1c66e6c5a091c4608ffe1b1635992c01cdefc309c835a52c24d773`.
- receipt `3373432fa1dc702c72c98e2d88f0248d8c97315f0830fecc093b1818cb4d1faf`.
- CI 원본 archive·고지·실행 권한·receipt를 그대로 게시한다. receipt의 `publishable:false`는
  빌드 당시 기록이며 사용자 게시 승인은 PR/release에 별도 남긴다.
- 게시 뒤 인증 없는 공개 HTTPS 다운로드의 크기·해시를 원본과 대조했다.
- Apple 서명·공증은 없다. 보안 설정을 해제하지 않는다.
- 이 Mac에서는 공식 Codex CLI로 preview.9 등록을 제거한 뒤 preview.10 고정 tag를 등록·설치했다.
  설치 목록의 enabled 상태, tag commit, 설치된 payload 8개 해시와 Core 실행 권한을 확인했다.
  사용자 프로젝트나 기록을 열어 변경하지 않았다. 이 검증은 실제 Host 화면·Luna 시험과 별개다.

## Windows 배포물과 채널

- [고정 prerelease](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.9).
- installer `GIL.Companion.Unsigned.Test._0.1.1-owner.9_x64-setup.exe`, 5,995,976 bytes.
- SHA-256 `d54b470a1905d9e15a7c5f93579d762df65773b75e41549272c4afe34cba8ca4`.
- Core `98b5b3b01dbfa0c816f9a35c898ca02aa0a0acee11aab2fadc40ad2d9b7252a7`.
- 후보 CI 설치·재설치·이전 연결 유지·새 연결의 Core 선택·창 표시·Codex CLI 등록/해제·제거 후
  runtime 보존 등 13개 acceptance 항목이 통과했다. 실제 Codex Desktop 채팅은 검사 범위 밖이다.
- 설치된 Core와 wire 해시가 같다. 기존 승인 wire 20 통과 / 1 예상 차단.
- updater signature의 trusted comment는 `version:0.1.1-owner.9`를 포함한다.
  installer·signature bytes는 원본이며 GitHub용 파일명만 정규화했다.
- 인증 없는 공개 다운로드를 확인한 뒤 `distribution/windows/owner-update.json`을 새 버전으로 전환한다.
- Authenticode publisher 서명은 없다. updater 서명과 `requireSignedVersion`은 유지한다.
- 사용자가 업데이트 버튼을 눌러 설치한다. 기존 MCP 연결을 강제 종료하거나 실행 중인 Core를 덮어쓰지 않는다.

## 실제 사용자 시험

실제 Host의 메시지 수락이 새 모델 턴을 보장하지 않는다. 합성 App 입력과 프로토콜 성공은
사람의 클릭이나 화면 표시 증거가 아니다. 이 판의 Luna 실사용·Host 클릭 후 자동 이어가기는
사용자 확인 대상으로 남긴다. 새 대화에서 기존 프로젝트의 정확한 루트를 지정하고 Monitor를 연다.
이미 완료된 복구는 반복하지 않고 현재 기록에서 이어간다. 새 실패 Cycle에서 승인 복구를 시험할 때
채팅이 현재 상태를 읽는지, 이전 승인을 다시 요구하지 않는지, 다음 시도가 증거 파일·배움을 남기는지 확인한다.
