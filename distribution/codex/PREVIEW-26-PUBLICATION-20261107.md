# 훅 없는 새 세션 재개 보완판 게시 장부 — 2026-11-07

macOS Apple Silicon `0.2.1-preview.26`을 unsigned prerelease로 게시했다.
사용자는 프로젝트별 새 세션 재개 수정, 회귀시험, PR·CI·정상 머지와 새 배포본을 승인했다.
PR 단계에서 일시 중단한 뒤 CI 확인과 다음 단계 진행을 요청했다. 실제 Host 필드테스트는 별도이며
Windows owner.14 배포는 유지한다. 이 게시 작업은 실제 설치본과 사용자 프로젝트 기록을 변경하지 않는다.

## 변경과 출처

- [수정 PR #102](https://github.com/hyun06000/gil/pull/102), build/merge commit `fd1e78545454202acf96ffe99502af11585297b2`.
- 기존 프로젝트의 새 세션에서 GIL 이름을 생략한 산출물 수정 요청도 먼저 context·현재 계약을 읽도록
  공통 MCP 지침과 Skill을 보완했다. 선택 폴더만 읽으며 다른 프로젝트·HOME·registry를 검색하지 않는다.
- 새 CLI/MCP start는 프로젝트 내부 Codex/Claude 안내를 보존·추가한 뒤 최초 Snapshot을 만든다.
  기존 프로젝트는 다음 정상 Verify open 뒤 같은 잠금 아래 안내를 추가한다. context/status는 설치하지 않는다.
  과거 Snapshot·승인·실행 기록과 기존 안내 bytes·권한은 보존한다. 안내가 변경되거나 과대·링크·경합이면 검토 필요로 남긴다.
- 전역·프로젝트 로컬·Plugin·Skill 훅, 전역 지침 변경, 셸 가로채기는 없다. Core 전이·승인·증거 검사를 유지한다.
- [후보 CI 42360577732](https://github.com/hyun06000/gil/actions/runs/42360577732)의 원본 bytes를 게시했다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #25](https://github.com/hyun06000/gil-marketplace/pull/25) 최신 head `bee131e630c12d82db125bdbd1377ea298c15318`의
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42361017481) 통과 후 정상 머지했다.
  marketplace/tag commit: `364a24d7da0e770d0b29b8c20455f93fe4ba394a`. main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 한계

- 수정 PR 최종 head `36c436e4903db7092d63907e2a2112cf47748647`의 해당 CI 11개 모두 성공.
  macOS Core·UI·패키징, Windows x64/arm64 default·visual-authoring, macOS/Windows Chain,
  Companion macOS arm64·Windows x64/arm64, product contract 범위를 유지했다.
  별도 실행 조건의 서명/설치 인수 3개는 skipped이며 성공으로 세지 않았다.
- 최초 로컬 제한 실행의 소켓 거절과 이전 프로필의 임시 fixture 충돌을 분리했다. 이후 HOME/TMPDIR를 격리했다.
  MCP 시작 응답·macOS 경로 별칭 실패를 고치고 관련 MCP 27개, 프로젝트 보존 4개, 안내 처리 5개를 통과했다.
  실제 제품 프로필의 구형 프로젝트 재개→Verify→재연결 시험도 통과했다.
- 초기 CI에서 시각자료 fixture가 저장소에 없는 빈 Snapshot을 참조한 문제가 드러났다. 실제 초기 세계를 쓰도록
  고쳤으며, Windows 두 시험은 정확히 안내 두 파일만 추가되고 기존 bytes·외부 기록·clean Snapshot이 유지됨을 검사한다.
  로컬 MCP Chain+visual+product-safety 37개, visual-authoring 34개 통과 후 위 native CI도 통과했다.
- 로컬 JS/UI/MCP App/Plugin 185 passed / 기존 5 skipped, 패키징·고지 106 passed. 전체 성공은 위 최종 CI를 근거로 한다.
- 후보 CI의 CLI·probe·MCP 버전, SDK 계약, 압축 왕복·33 tools·내장 UI 검증 통과.
  clean source 735개 파일의 snapshot이 receipt와 일치한다. Core·Skill·manifest·고지·실행 권한은 원본 그대로다.
- 배포 저장소 무결성·raw history 회귀시험 29개 통과. 실제 후보 Core로 SDK 시작·재연결·일반 폴더 비등록,
  안내 보존, 무동작 훅·GIL 명령 제거·백업·비-GIL 설정·registry 보존을 임시 HOME에서 확인했다.
  stdio 시나리오 20 passed / 1 expected product-safety block / 0 failed.
- preview.25 → preview.26 → preview.25 → preview.26 SDK 읽기·재연결에서 프로젝트 전체 파일 bytes 보존.
  공개 비인증 HTTPS로 받은 archive·SHA256SUMS·checks.json·update-record.json은 후보 원본과 동일하다.

**안내가 없는 구형 프로젝트에서 Host가 MCP 지침을 전달하지 않으면 최초 자동 인지를 보장하지 않는다.**
한 번 명시적으로 재개해 정상 Verify에서 안내를 채택해야 한다. installer는 기존 프로젝트를 순회하지 않는다.
프롬프트는 파일 잠금·모델 준수 보장이 아니다. 실제 Codex/Claude Desktop Code 탭의 안내 로딩·모델 행동,
화면·fullscreen·사람 승인·새 Mac·Host 업데이트/rollback 인수는 미검증이다.

## 게시물

[릴리스 v0.2.1-preview.26](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.26)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.26.tar.gz` |
| 크기 | 3,653,973 bytes |
| Archive SHA-256 | `81a6401b9125c7f8d56f34a7ecdd8da18c5e01d442186cf70a9fa7401d3243fc` |
| Core SHA-256 | `b8110a290dc66200e8a43f45e5b61c8be5edd8b7129daa2ad940899ab94c7556` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `1ae55d7c2833f52b6b51b7826384f034a404fb0705038b5c0497ffba3cada5aa` |
| Source snapshot SHA-256 | `6af89f795037991ef92e8362a3656afdf62e5a38df76de3e44da9862537fe6a9` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
preview.25와 compatibility 범위가 같고 migration:requires_review 정책을 유지한다.

[설치 안내](../../INSTALL.md)에서 등록된 Core 버전·receipt·해시를 확인한다. 실제 설치본 적용은 이 게시와
구분한다. Claude 업데이트는 사용자가 보류한 상태다. 새 세션에서 GIL 이름 없이 산출물 수정을 요청하고,
첫 파일 도구 전에 context를 읽는지, 일반 프로젝트에는 간섭하지 않는지를 사용자가 필드테스트한다.
