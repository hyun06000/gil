# 계획 입력·Closing 안내 검증판 게시 장부 — 2026-11-03

macOS Apple Silicon `0.2.1-preview.20`을 unsigned prerelease로 게시했다.
사용자가 PR #86의 독립 검토·보완·머지 후 직접 검증할 수 있는 릴리즈를 요청했다.
이번 게시 대상은 macOS다. Windows owner.12, 사용자 설치본·설정·전역 훅·Project·`.gil`은 변경하지 않았다.

## 변경과 출처

- [PR #86](https://github.com/hyun06000/gil/pull/86)은 검토 시작 전에 `a5b950b`로 머지되어 있었다.
- 독립 검토의 [보완 PR #87](https://github.com/hyun06000/gil/pull/87)은 JSON 예시의 항목/체크리스트 불일치,
  synthesis 문자열 맵 설명, Closing의 chain_verdict와 승인 후 흐름, 실제 context 다음 호출을 바로잡았다.
- 새 stdio 시험은 문서 JSON 자체로 Opening 승인 → Experiment → Closing 승인 → Chain 종료까지 수행한다.
  잘못된 중첩·타입의 -32602, 기록 불변, 응답·stderr의 합성 비밀값 비노출도 검사한다.
- 공개 preview.13 이후의 시작→Monitor handoff, 프로젝트 파일 작업 범위, 사용자 대기 안내 보완도 포함한다.
  이번 두 PR 자체는 schema·역직렬화·저장 계약을 바꾸지 않았다.
- source/build: `c5a8fe61da89c67777412521322c1011645b607e`.
- runtime: `approved-plan,visual-authoring,product-safety`; 33 tools, action surface 2, readable formats 4..7.
- [후보 CI 41575368511](https://github.com/hyun06000/gil/actions/runs/41575368511): 해당 source에서
  명시적 approved-plan 프로필로 시험·빌드·압축 왕복·native 첫 시작 검증 성공.
- [배포 PR #18](https://github.com/hyun06000/gil-marketplace/pull/18): pinned-payload CI 통과 후 정상 merge.
  main 직접 push·force push·관리자 우회를 사용하지 않았다.
- marketplace/tag commit: `e7c0d0590e7edc357d2837a96a199271d85dfb01`.

## 검증

- 소스 검토: 기본 Core 1,552 passed / 기존 2 ignored, 승인 프로필 730 passed / 기존 1 ignored,
  Chain 관련 61 passed. 기본·Chain·승인 프로필 all-targets 빌드 성공. 원 PR과 후속 PR의 각 최신 head
  전체 10개 CI 성공(macOS·Windows x64/arm64·기능 프로필 범위 유지).
- JS/UI/Plugin 181 passed / 기존 5 skipped, packaging 102 passed / 기존 1 skipped.
- 로컬 clean source의 775개 파일 snapshot과 CI receipt가 정확히 일치했다.
- 배포 저장소 정적 무결성·raw history 28개 통과. payload·mode·고지·receipt는 CI 원본 그대로다.
- 게시할 Core 자체의 wire: 20 passed / 1 expected product-safety block / 0 failed.
- 공개 HTTPS에서 인증 없이 archive·SHA256SUMS·checks.json·update-record.json을 다시 받아
  CI 원본과 바이트 단위로 비교했다. 네 자산 모두 일치한다.
- 공개 원본과 동일한 archive로 Git 없는 준비를 포함한 packaging/notices 103개 통과, skip 없음.
  이 준비는 임시 폴더에서 이루어졌으며 사용자 Plugin 설치가 아니다.
- 새 worktree 첫 packaging 실행은 개발 의존성 부재로 실패했다. 잠긴 npm 의존성 설치 후,
  기존 preview.13을 기대하던 테스트 정규식 한 곳을 preview.20으로 맞추고 전체 재실행해 통과했다.
  최초 archive 검사는 상대경로 사용을 거절했으며 절대경로로 수정해 통과했다. 이때 원본 bytes는 바꾸지 않았다.
- 제한 sandbox의 보호 훅 실패는 변경 전 main과 원 PR head 모두 같은 sandbox_apply 거절이었다.
  정상 명령별 승인 경로와 CI에서는 통과했다. 전역 보안 설정을 바꾸지 않았다.
- 추가 탐색한 approved-plan 단독의 기존 stdio 시험은 visual 응답 필드를 가정해 변경 전후 모두 실패한다.
  정식 배포 조합은 전체 통과했으며 단독 조합의 기존 harness 제약을 배포 성공으로 포장하지 않는다.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [v0.2.1-preview.20](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.20) |
| Archive | `gil-codex-macos-arm64-0.2.1-preview.20.tar.gz` / 3,672,162 bytes |
| Archive SHA-256 | `6370135ce8eb1b40c788ad2f5ce98efe5e82e9df2bd0c21abf3317eba8689bd5` |
| Core SHA-256 | `fae7b5a898e1db360cad12229968182525118d784419206b06d41d2a4ab49342` |
| UI SHA-256 | `56a42d5fb569105237adb6c28018cf26b142352cc0c76ad652789ead46895426` |
| Receipt SHA-256 | `398756dfa50cec46735f1ad8876f7d4ef4c2bc05370a260a2721834f1b388359` |
| Source snapshot SHA-256 | `42489303385e081459eaa4d4d1805acb5dd1fa665f0194442b5bde359dec07b1` |

Apple Developer ID·공증은 없다. stable/latest로 지정하지 않았으며 기존 공개 버전의 bytes를 교체하지 않았다.
preview.14 후보와 사용자가 보고한 preview.19보다 높은 새 불변 버전으로 preview.20을 사용했다.
receipt의 `publishable:false`는 빌드 당시 권한 기록으로 보존하며 게시 승인은 PR·이 장부·릴리즈에 기록한다.

이전 공개 preview.13과 compatibility 범위는 같다. 두 receipt의 `migration:requires_review` 때문에
update-policy는 자동 승격 대신 명시적 계약·이관 검토를 요구한다. 해당 표시와 형식을 유지한다.
새 프로젝트는 format 7이며 기존 기록을 자동 이관하지 않는다. 모든 기록의 downgrade 안전성이나
실제 Host 업데이트 쌍 인수를 주장하지 않는다.

## 사용자 검증

새 버전 설치 후 새 연결·새 빈 테스트 폴더에서 다음을 확인한다.

1. 시작한 Monitor가 실제로 보이고 Interview가 진행되는가.
2. 계획이 한 번의 올바른 draft 입력으로 등록되고 Monitor에 전체 범위·항목이 보이는가.
3. 실제 승인 버튼을 누른 뒤 close_report로 Synthesis·Outcome·Interview가 닫히는가.
4. Experiment Verify가 계획 문서와 별도 실제 증거를 남기는가.
5. Closing 제안의 chain_verdict를 승인한 뒤 새 Experiment 대신 Chain Report로 종료하는가.

합성 승인·stdio 성공은 실제 화면·사람 승인 클릭·모델 실사용의 증거가 아니다. 이번 버전의 Host fullscreen,
실제 버튼 클릭, 새 Mac 설치는 사용자 검수 대기다. 설치 및 직접 MCP 등록 경로는 [설치 안내](../../INSTALL.md)를 따른다.
