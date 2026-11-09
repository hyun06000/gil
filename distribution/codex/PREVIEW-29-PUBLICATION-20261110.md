# 계획 입력 진단·PNG 안내 보완판 게시 장부 — 2026-11-10

macOS Apple Silicon `0.2.1-preview.29`를 unsigned prerelease로 게시했다.
사용자는 수정·회귀시험·PR·최신 head CI·정상 머지·새 배포본 게시를 승인했고, 설치와 필드테스트는
직접 하겠다고 명시했다. 실제 Codex·Claude 설치본·설정과 사용자 프로젝트 기록은 변경하지 않았다.
Windows owner.14도 유지한다.

## 변경과 출처

- [수정 PR #109](https://github.com/hyun06000/gil/pull/109), build/merge source `e98b048b97fdf44bcaac0b9c70e5d7ef942d64d3`.
- Synthesis 스키마에 statement·basis_refs·exploration_checklist를 명시하고, 열기 영수증·Context·
  공통 workflow/Skill에 전체 참조 조건을 제공한다. 잘못된 모든 비어 있지 않은 줄을 검사해
  원래 줄 번호·필드 경로·이유를 함께 반환한다. 한 줄은 먼저 실패한 조건을 설명한다.
  현재 Interview의 닫힌 Question/Interpretation·부모 사슬·중복 금지·최소 하나의 조건은 유지한다.
  이전 Cycle 배경을 statement에 설명할 수 있지만 참조를 자동 이동하거나 보정하지 않는다.
- Verify 안내·도구 설명·Manual에 기존 PNG 한도를 수치로 제공한다. 가로·세로 각각 1–8192px,
  총 16,777,216픽셀, 파일당 16 MiB, 새 첨부 최대 8개·합계 32 MiB다. 디코더 자원·무결성 검사는
  유지하며, 거절문에 실제 치수와 제한값을 표시한다. bounded read의 초과 bytes는 관측 하한으로 표시한다.
- [후보 CI 42640140684](https://github.com/hyun06000/gil/actions/runs/42640140684)의 원본 bytes다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7,
  new format 7과 기존 승인·전이·증거·Snapshot 의미를 유지한다. 훅은 등록하지 않는다.
- [배포 PR #28](https://github.com/hyun06000/gil-marketplace/pull/28) head
  `38f5ac85f13b4a48458c18fe46c92a2a2d349635`에서
  [pinned-payload CI 42641479291](https://github.com/hyun06000/gil-marketplace/actions/runs/42641479291)를
  통과한 뒤 정상 머지했다. marketplace/tag commit `c82bd228cf98f6bcf9dacf9a42f76ca8b2d3ff0f`.
  main 직접 push·force push·관리자 우회는 사용하지 않았다.

## 검증과 한계

- 수정 PR 최종 head `ed0284aeef0a8cdc779d0ec18c4438053398cd16`의 CI 10개 모두 성공.
  macOS 기본·패키징, Windows x64/ARM64 기본·시각자료, macOS/Windows Chain·승인 계획,
  Companion 세 플랫폼 검사 범위를 유지했다.
- 기본 Core 전체 1,577 passed / 기존 2 ignored, all-targets build 성공. 이후 최종 PNG decoder
  subset 14 passed, 새 거절/복구 사례를 포함한 실제 Session 첨부 suite 24 passed.
  approved-plan·visual-authoring·product-safety library 711 passed / 기존 1 ignored,
  실제 stdio 승인·Context·guidance 36 passed, all-targets build 성공.
- JS/UI/MCP App/Plugin 192 passed / 기존 5 skipped, workflow/Skill 동기화 성공.
  패키징·고지 106 passed(수정 단계는 preview.28, 게시 안내 단계는 실제 preview.29 archive 지정).
  내장 UI는 동일한 hash다. 소스 변경 범위 밖의 전체 formatting이나 의존성 변경은 하지 않았다.
- 최초 sandbox 내부 Core 실행은 기존 IPC/loopback bind 제한으로 실패했다. 필요한 실행 권한으로
  전체 suite를 다시 실행해 통과했고 시험을 삭제·축소하지 않았다. 후보 압축 해제의 첫 시도는 로컬
  Python의 `extractall(filter=...)` 미지원으로 멈췄다. 빈 임시 폴더에서 상대 경로·일반 파일/폴더만
  허용하는 멤버 검사를 유지해 같은 원본을 풀었고, 이후 payload 검증을 통과했다.
- 후보 CI의 CLI·probe·MCP 버전, 압축 왕복·33 tools·내장 UI 검사 성공.
  clean source 738개 파일 snapshot과 receipt가 일치한다. Core·Skill·고지·권한은 원본 그대로다.
  배포 무결성·raw no-reply 이력 시험 29 passed이며 머지·tag 뒤 이력도 재확인했다.
- 실제 후보 SDK에서 보고된 세 가지 basis_refs 실패 형식과 여러 오류가 섞인 입력을 확인했다.
  잘못된 모든 줄의 번호·이유·필드 경로가 보이고, 거절 전후 프로젝트 파일 bytes가 같았다.
  수정한 제안·합성 App 승인·Verify 안내·높이 초과 PNG 거절과 수정 후 첨부 성공도 확인했다.
  이 합성 App 결정은 실제 사람의 승인이나 Host 화면 인수가 아니다.
- preview.28 → preview.29 → preview.28 → preview.29 SDK 읽기·재연결에서 프로젝트 파일 bytes가
  보존됐다. 같은 compatibility와 `migration: requires_review`를 명시적으로 검토했다. 자동 이관이나
  실제 Host Plugin 업데이트·rollback 허용/성공을 뜻하지 않는다.
- 공개 비인증 HTTPS의 archive·SHA256SUMS·checks.json·update-record.json이 후보 원본과 모두 같다.

실제 Codex·Claude Desktop Code 탭의 설치, 모델의 안내 활용·규칙 준수, Monitor 표시·클릭·승인,
새 Mac 설치와 Host 업데이트/rollback은 이번 판에서 미검증이다. 사용자가 설치 후 필드테스트한다.
프롬프트는 파일 잠금이나 모든 모델의 준수를 보장하지 않는다. 사용자 프로젝트·설치본은 변경하지 않았다.

## 게시물

[릴리스 v0.2.1-preview.29](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.29)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.29.tar.gz` |
| 크기 | 3,495,614 bytes |
| Archive SHA-256 | `31dca9788ee3b4cf5d392600c877e56de73073a4628e4d0567ca1854717b80dc` |
| Core SHA-256 | `560e38ca93c4a9182565ce297f73febe9a85b005876c287e704c321eba497929` |
| UI SHA-256 | `54ab7a558e085942d61dc697d4e8966394f5023d1171cd23ed1aa7b50a72cad4` |
| Receipt SHA-256 | `bf9ca1594a1c251bca5851c83cc7270cd869643e59f3b7b4fe9480b71d64b245` |
| Source snapshot SHA-256 | `2c17c191cdc187d3d3218f39a12645aeea14d664a477d944c47890871926e39d` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
[설치 안내](../../INSTALL.md)와 Monitor 업데이트 피드를 같은 게시 안내 PR에서 갱신한다.
새 안내는 preview.29부터 포함되고 preview.28 이하·Windows owner.14에는 없음을 구분한다.
