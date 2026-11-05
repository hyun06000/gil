# 세션별 진행·경로별 보호 수정판 게시 장부 — 2026-11-05

macOS Apple Silicon `0.2.1-preview.21`을 unsigned prerelease로 게시했다.
사용자가 write-guard 수정의 PR·CI·머지에 이어 새 배포본 게시를 요청했고 실제 Host 검증은 직접 수행하기로 했다.
이번 게시 대상은 macOS다. Windows owner.14와 실제 사용자 설치본·설정·프로젝트·승인·Snapshot은 변경하지 않았다.

## 변경과 출처

- [수정 PR #91](https://github.com/hyun06000/gil/pull/91), source/build commit
  `74e6c2d024abd9a23ca43c91b1635997b815f1a1`.
- 등록 root가 하나라도 있으면 모든 셸에 보호 실행을 요구하던 구조에서 Host·세션 진행 의무와
  경로 보호를 구분했다. Stop/wrap_up은 성공한 GIL 호출로 연결된 Host+session_id에만 적용한다.
  비연결·다른 Host의 같은 ID·pause는 진행 의무를 만들지 않지만 등록된 프로젝트의 파일 보호는 유지한다.
- Claude Bash는 `updatedInput`과 `permissionDecision: ask`로 정상 승인을 요청하는 보호 래퍼를 제출한다.
  Codex는 해당 Host가 지원하는 명시적 래퍼/단일 명령 승인을 안내한다. 자동 allow·권한 필드 주입은 없다.
  cwd·명령 키워드·비연결 세션을 무조건 면제하지 않으며 절대경로·cd·symlink 접근도 보호한다.
- 프로젝트 내부 쓰기는 실제 열린 Verify에서만 가능하다. 외부 직접 편집·파일 없는 웹 조사는 단계를 요구하지 않는다.
  등록·상태 손상 및 잠금의 정책과 중첩 sandbox의 실행 한계를 [보호 계약](WRITE-GUARD.md)에 명시했다.
- 기존 생성 훅의 Core 경로를 정확히 교체하면서 비-GIL 설정과 백업을 보존한다.
  `status --auto`의 `matches_current_core`로 MCP와 훅의 설정 일치를 확인하고 새 Host 연결은 별도로 검증한다.
- [후보 CI 41974673935](https://github.com/hyun06000/gil/actions/runs/41974673935)는 위 source에서
  `approved-plan,visual-authoring,product-safety` 프로필로 시험·빌드·압축 왕복·native 첫 시작을 통과했다.
  33 tools, action surface 2, readable formats 4..7, new format 7을 유지한다.
- [배포 PR #19](https://github.com/hyun06000/gil-marketplace/pull/19)의 head
  `ec0c20db63692314017164fbee393280583b0da1`에서
  [pinned-payload CI](https://github.com/hyun06000/gil-marketplace/actions/runs/41975525318)가 통과한 뒤 정상 merge했다.
  marketplace/tag commit은 `6000b9c65f19001708159f78222649903f1d5a65`다.
  main 직접 push·force push·관리자 우회는 사용하지 않았다.

## 검증과 범위

- 수정 PR 최신 head의 10개 CI 전부 성공: macOS 기본 Core·UI·packaging, Windows x64/arm64 ×
  default/visual-authoring, macOS/Windows Chain·승인 프로필, Companion macOS arm64·Windows x64/arm64.
  기본 Core 결과는 1,559 passed / 기존 2 ignored였다. 기존 플랫폼·기능 프로필 범위를 줄이지 않았다.
- 후보 CI: 승인 프로필 730 passed / 기존 1 ignored, JS/UI/Plugin 181 passed / 기존 5 skipped,
  packaging 102 passed / archive 미지정 1 skipped. native 33 tools와 내장 UI 검사 통과.
- clean source 723개 파일의 snapshot이 CI receipt와 정확히 일치했다.
  배포 binary·Skill·manifest·고지·실행 권한·receipt는 CI 원본 그대로이며 재빌드하지 않았다.
- 배포 저장소 정적 무결성·raw history 시험 28개 통과. 최초 실행에서 테스트의 명시적 preview.20
  기대값이 실패했고 preview.21로 갱신해 전체 재실행했다. 검사 범위는 유지했다.
- 게시한 Core 자체의 stdio wire: 20 passed / 1 expected product-safety block / 0 failed.
- 같은 binary의 격리 HOME·임시 프로젝트 smoke: 비연결 Stop·Host별 같은 ID 분리,
  외부 직접 편집·설치 준비 허용, 보호 경로 직접 편집·절대경로·cd·symlink·이동 거절,
  Claude ask 입력 교체·Codex 안내, 구형 훅 경로 갱신·비-GIL 설정·백업 보존을 통과했다.
  이때 Host 이벤트와 승인은 합성 입력이었다. 실제 앱 승인 창이나 설치된 Host 훅 인수로 표시하지 않는다.
- 공개 HTTPS에서 인증 없이 archive·SHA256SUMS·checks.json·update-record.json을 다시 받아
  CI 원본과 바이트 단위로 비교했다. 네 자산 모두 일치한다.
- 공개본과 같은 archive로 Git 없는 준비를 포함한 packaging/notices 103개 통과, skip 없음.
  임시 폴더에서 수행한 준비이며 사용자 Plugin 설치가 아니다.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [v0.2.1-preview.21](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.21) |
| Archive | `gil-codex-macos-arm64-0.2.1-preview.21.tar.gz` / 3,666,511 bytes |
| Archive SHA-256 | `30661f0748ea40476f7714f5496ba329e61ee2439c0eeaa956412eb3b1fb7bd3` |
| Core SHA-256 | `3f1b4777abe581c717e7314b0f78cfd6918d61b614b3af4c033b81db7a7ed7e6` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `3fe97e51b8583fc8779da40068c93b7b0cd2acad28c57dfe59dbb1a0e86d3765` |
| Source snapshot SHA-256 | `5448be711f75ddb4112387a6b577fe31e6e9fecce25557f65b9c44354f4b815b` |

Apple Developer ID·공증은 없다. stable/latest로 지정하지 않았고 기존 공개 버전의 자산을 교체하지 않았다.
receipt의 `publishable:false`와 trust 선언은 빌드 당시 기록으로 보존하며 게시 승인은 PR·이 장부·릴리즈에 기록한다.
preview.20과 compatibility 범위는 같고 `migration:requires_review`도 유지한다. update-policy는
`explicit_contract_and_migration_review_required`로 자동 승격을 막는다. 이번 변경이 저장 형식을 이관하지
않음을 검토했으며 실제 Host 업데이트/rollback 쌍의 성공이나 모든 기록의 downgrade 안전성을 주장하지 않는다.

## 설치 적용과 사용자 검증

실제 설치본·사용자 설정은 이번 게시로 갱신하지 않았다. [설치 안내](../../INSTALL.md)의 새 버전을 준비한 뒤
사용자 승인된 적용 단계에서 MCP와 기존 보호 훅을 같은 Core로 맞춘다. 구형 훅이 갱신을 막으면
[검토한 설정 diff·백업을 통한 좁은 갱신 절차](INSTALL.md#별도로-설치한-전역-write-guard-갱신)를 따른다.
전역 훅 삭제·보호 없는 재시도·보안 설정 해제를 사용하지 않는다.

두 Host의 새 연결과 버려도 되는 테스트 폴더에서 각각 확인한다.

1. MCP와 훅이 같은 preview.21 Core를 가리키고 해당 Host 상태가 `matches_current_core`인가.
2. GIL 미연결 채팅에서 외부 파일 작업·일반 개발·설치 준비와 완료가 가능한가.
3. 등록된 다른 프로젝트가 있어도 무관한 채팅에 Stop/진행 의무가 생기지 않는가.
4. 보호 프로젝트는 Verify 밖에서 직접 편집·셸·외부 cwd의 절대경로·cd·symlink 쓰기가 거절되는가.
5. 실제 Monitor 승인을 거쳐 Verify가 열리면 파일 작업이 되고 닫힌 뒤에는 다시 보호되는가.
6. 새 프로젝트 자동 등록·기존 프로젝트 재연결·새 세션·pause가 진행 의무와 파일 보호를 올바르게 구분하는가.

실제 Claude/Codex 훅 실행·승인 화면·fullscreen·사람 승인 클릭·새 Mac 설치는 사용자 확인 대기다.
중첩 sandbox를 지원하지 못하는 Host에서는 실행 제한이 남는다. 기존 프로세스·미등록 파일 도구와 직접 편집의
동시 상태 전이 등 [보장 한계](WRITE-GUARD.md)를 유지하며 전역 OS 접근 통제나 완전한 우회 방지로 표현하지 않는다.
