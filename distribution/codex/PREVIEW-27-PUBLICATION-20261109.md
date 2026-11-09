# Mac 업데이트 안내·MCP 복구 보완판 게시 장부 — 2026-11-09

macOS Apple Silicon `0.2.1-preview.27`을 unsigned prerelease로 게시했다.
사용자는 PR #104·#105의 최신 CI 확인, 정상 머지와 두 수정을 포함한 새 배포본 게시를 승인했다.
Windows owner.14는 유지한다. 이 게시 작업은 실제 Host 설치본·설정과 사용자 프로젝트를 변경하지 않는다.

## 변경과 출처

- [PR #104](https://github.com/hyun06000/gil/pull/104): Mac Monitor에서 앱을 선택하고 업데이트 요청문을
  복사한다. 복사 거절 시 수동 복사를 안내하고, Claude 완전 종료가 필요한 작업은 외부 에이전트에 맡긴다.
  안내 열기·복사는 설치나 MCP 도구를 실행하지 않는다. merge `2b8d3fae21fb6204fa5c250ced9540b5902519e9`.
- [PR #105](https://github.com/hyun06000/gil/pull/105): 잘못 놓인 contract/report 입력을 실행 전에 거절하고,
  `active_plan`으로 실행된 항목까지 승인 계획 원문을 제공한다. 계획 오류는 누락 ID·불일치 필드와
  제한된 값 미리보기를 표시한다. 연결된 Core의 `gil_help(topic: close)`는 현재 Report를 안내한다.
  Windows private 기록도 제품의 프로젝트 resolver를 사용한다. 승인·증거·기록 형식은 유지한다.
  merge/build source `0e660397a70dd66dc5c24de2bcc9d253c3b30c4c`.
- [후보 CI 42584040647](https://github.com/hyun06000/gil/actions/runs/42584040647)의 원본 bytes를 게시했다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7,
  new format 7을 유지한다. 전역·프로젝트 로컬·Plugin·Skill 훅은 없다.
- [배포 PR #26](https://github.com/hyun06000/gil-marketplace/pull/26)의 최신 head
  `886aab6f7f86a339b3b3fb2f81f8eb8f78295b55`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42584638869)를 통과한 뒤 정상 머지했다.
  marketplace/tag commit `ab80e77ffce178c92199511ab6c865cebcc10da1`.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 한계

- #104 최신 head `2b959379a13652350462923284ae1a76cf0d9211`의 해당 CI 10개 성공.
  #105 최초 CI의 Windows Chain 검사는 inline 기록만 찾는 close-help 경로 때문에 실패했다.
  제품 resolver와 Windows 회귀시험을 보완하고 #104를 병합한 최종 head
  `ea00cb01e16b8472cbc7c5aee503d4d3fde9cf83`의 해당 CI 11개 모두 성공했다.
  macOS·Windows x64/arm64, default·visual-authoring·Chain, Companion, product contract 범위를 줄이지 않았다.
  별도 실행 조건의 서명/설치 인수 3개는 skipped이며 성공으로 세지 않는다.
- 결합 head 로컬 JS/UI/MCP App/Plugin 186 passed / 기존 5 skipped, 패키징·고지 105 passed / archive 미지정 1 skipped.
  Windows 수정 후 실제 MCP approved-plan wire 11개와 관련 default 도움말·MCP 검사를 통과했다.
  전체 제품 프로필 검증은 위 후보 CI 및 최종 PR CI를 근거로 한다.
- 후보 CI의 CLI·probe·MCP 버전, 압축 왕복·33 tools·내장 UI 검사 통과.
  clean source 736개 파일의 snapshot과 receipt가 일치한다. binary·Skill·고지·권한은 CI 원본 그대로다.
- 배포 저장소 무결성·raw history 회귀시험 29개 통과. 실제 후보 Core의 SDK 시작·재연결,
  일반 폴더 비등록, 기존 안내 보존, 무동작 훅·Question 안내를 임시 HOME·프로젝트에서 확인했다.
- 게시 안내 변경 후 JS/UI/MCP App/Plugin 186 passed / 기존 5 skipped,
  실제 게시 archive를 지정한 패키징·고지 106 passed. Git 없는 준비의 크기·해시·압축 해제·권한 검사도 포함한다.
- preview.26 → preview.27 → preview.26 → preview.27 SDK 읽기·재연결에서 프로젝트 전체 파일 bytes가 보존됐다.
  공개 비인증 HTTPS로 받은 archive·SHA256SUMS·checks.json·update-record.json은 후보 원본과 동일하다.
- 두 판의 compatibility와 `migration: requires_review`가 같음을 명시적으로 검토했다.
  일반 update/rollback 정책 도구는 이 migration 정책 때문에 계속 검토 필요로 차단한다.
  이를 자동 이관 허용이나 실제 Host Plugin 업데이트·rollback 성공으로 바꾸지 않는다.

실제 Codex·Claude Desktop Code 탭의 화면·fullscreen·클립보드·모델 행동·사람 승인,
새 Mac 설치와 Host 업데이트/rollback은 이번 배포에서 미검증이다. 프롬프트는 모델 준수나 파일 잠금을
보장하지 않는다. 안내 없는 구형 프로젝트의 최초 인지 한계는 [프로젝트 재개 계약](PROJECT-RESUME.md)을 따른다.

## 게시물

[릴리스 v0.2.1-preview.27](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.27)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.27.tar.gz` |
| 크기 | 3,665,233 bytes |
| Archive SHA-256 | `ca0f0c616630c0d6afc3ec136cf6887f542fec5cf7bcc2146678aaafad05e365` |
| Core SHA-256 | `aa9cd9ff6b199e6420a541af5f453540f34cea8300eb0e4acc60809666d404b8` |
| UI SHA-256 | `60535672b9a5bb06014ae6232e10569e5d9097001839adadb9a91659088fa397` |
| Receipt SHA-256 | `d68d5f89a2382cdd65935841fc5f892f65007bee691180b0461cb142f26b5418` |
| Source snapshot SHA-256 | `712eeb18801dfdfebfa76ec553421d31975a71a05f2097f4be6c5394c1ba0254` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
[설치 안내](../../INSTALL.md)와 Monitor 업데이트 메타데이터를 같은 게시 안내 PR에서 갱신한다.
사용자의 실제 설치본 적용·필드테스트는 이 게시와 별개다.
