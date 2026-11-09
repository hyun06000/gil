# Monitor fullscreen 오류 복구 보완판 게시 장부 — 2026-11-09

macOS Apple Silicon `0.2.1-preview.28`을 unsigned prerelease로 게시했다.
사용자는 PR #107을 포함한 새 배포본 제작·게시를 요청했고, 설치와 필드테스트는 직접 하겠다고 명시했다.
실제 Codex·Claude 설치본·설정과 사용자 프로젝트 기록은 변경하지 않았다. Windows owner.14는 유지한다.

## 변경과 출처

- [수정 PR #107](https://github.com/hyun06000/gil/pull/107), build/merge source
  `33a3eae4f82db5c878845de3fbd822cadc71817f`.
- App 응답에 명시적인 새 클릭 요구가 있을 때만 `refused`로 분류한다. 타임아웃·연결 종료·알 수 없는
  실패는 `error`이며 1.5초·4초의 제한된 재시도와 수동 재요청을 유지한다. Host 이름으로 분기하지 않는다.
  Host 로그만으로 실제 App 응답을 추정하거나 클릭 성공을 보장하지 않는다. 수동 표시 선택도 보존한다.
- 선택 presence 필드 `automatic_fullscreen`을 Core status와 모델 안내까지 전달한다. 구형 App은 null이다.
  Node 어댑터 poll schema, 공통 workflow·Skill과 내장 UI를 함께 갱신했다. 기록 형식·승인·전이 정책은 유지한다.
- [후보 CI 42619965436](https://github.com/hyun06000/gil/actions/runs/42619965436)의 원본 bytes를 사용했다.
  approved-plan·visual-authoring·product-safety, 33 tools, action surface 2, readable formats 4..7,
  new format 7을 유지한다. 전역·프로젝트 로컬·Plugin·Skill 훅을 등록하지 않는다.
- [배포 PR #27](https://github.com/hyun06000/gil-marketplace/pull/27) head
  `9e65eb1a4e610bb03e2c283e2e5f0e1d3998c0a8`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/42620705767)를 통과한 뒤 정상 머지했다.
  marketplace/tag commit `8bcf893a0af179bffe59659fb7bcd3cfe5229c3a`.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.

## 검증과 한계

- 원 수정 PR의 최종 head `68366b4ddfeb95604f79cf91496303f6db12e2c1`에서 해당 CI 10개 모두 성공.
  macOS Core·UI·패키징, Windows x64/ARM64 기본·시각자료, macOS/Windows Chain과 Companion 세 플랫폼을 유지했다.
- 수정 검증에서 Core 1,574 passed / 기존 2 ignored, all-targets build 성공, JS/UI/MCP App/Plugin
  192 passed / 기존 5 skipped, 실제 preview.27 archive를 지정한 패키징·고지 106 passed.
  검토 당시 타임아웃이 클릭 전용 정책으로 오판되는 재현을 추가했고, 일반 오류·명시적 클릭 요구,
  제한된 재시도·일시 오류 복구·수동 취소를 실제 App 진입점 모의 Host 시험으로 검사했다.
- 후보 CI의 CLI·probe·MCP 버전, 압축 왕복·33 tools·내장 UI 검사 통과.
  clean source 737개 파일 snapshot과 receipt가 일치하며 Core·Skill·고지·권한은 원본 그대로다.
- 배포 저장소 무결성·raw history 회귀시험 29개 통과. 실제 후보 Core의 SDK 시작·재연결,
  일반 폴더 비등록·안내 보존·무동작 훅 검사를 임시 HOME·프로젝트에서 통과했다.
- 실제 후보 MCP에서 `error`와 `refused`를 각각 poll → status → context로 전달하고 안내가 구분됨을 확인했다.
  이 읽기·관측 시험 전후 프로젝트 전체 파일 bytes는 같았다. 실제 Host 표시 증거로 세지 않는다.
- preview.27 → preview.28 → preview.27 → preview.28 SDK 읽기·재연결에서도 프로젝트 bytes가 보존됐다.
  두 판의 compatibility와 `migration: requires_review`가 같음을 명시적으로 검토했다. 이 검토는 자동 이관이나
  실제 Host Plugin 업데이트·rollback 허용/성공을 뜻하지 않는다.
- 공개 비인증 HTTPS의 archive·SHA256SUMS·checks.json·update-record.json은 후보 원본과 동일하다.
- 게시 안내 변경 후 JS/UI/MCP App/Plugin 192 passed / 기존 5 skipped,
  실제 preview.28 archive를 지정한 패키징·고지 106 passed. Git 없는 준비의 크기·해시·멤버·권한 검사도 포함한다.

실제 Codex·Claude Desktop Code 탭의 inline 카드·클립보드·클릭·fullscreen·모델 행동·사람 승인,
새 Mac 설치와 Host 업데이트/rollback은 이번 배포에서 미검증이다. 사용자가 설치 후 필드테스트한다.
Host가 명시적인 클릭 요구 문구를 App 응답에 전달하지 않으면 `error`로 남는다.
프롬프트는 모델 준수나 파일 잠금을 보장하지 않으며 구형 프로젝트의 최초 인지 한계는
[프로젝트 재개 계약](PROJECT-RESUME.md)을 따른다.

## 게시물

[릴리스 v0.2.1-preview.28](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.28)

| 항목 | 값 |
|---|---|
| Archive | `gil-codex-macos-arm64-0.2.1-preview.28.tar.gz` |
| 크기 | 3,666,609 bytes |
| Archive SHA-256 | `71c5565009c8cc5f178abe9dad95fc1343d78e5d4fa0cdc449c494994c882a1c` |
| Core SHA-256 | `96e08272356f0bdc454e2b91537ed17117e79b1d3c496a957f5d968e11ae6913` |
| UI SHA-256 | `54ab7a558e085942d61dc697d4e8966394f5023d1171cd23ed1aa7b50a72cad4` |
| Receipt SHA-256 | `0901917dfcb6ef2aab9fbafabe0e7af0826d9393faafff67e2c01e5513fee959` |
| Source snapshot SHA-256 | `6fe044d6bb1a44e11e7e8a242ce9671760a5281d9aa115a0e6366ed4222755b5` |

Apple Developer ID·공증은 없다. stable/latest로 지정하거나 기존 자산을 교체하지 않았다.
receipt의 publishable:false·trust는 빌드 당시 기록이며 게시 승인은 PR·릴리스·이 장부에 기록한다.
[설치 안내](../../INSTALL.md)와 Monitor 업데이트 피드를 같은 게시 안내 PR에서 갱신한다.
macOS preview.28에 포함된 개선과 이전 preview.27·Windows owner.14의 범위를 명시적으로 구분한다.
