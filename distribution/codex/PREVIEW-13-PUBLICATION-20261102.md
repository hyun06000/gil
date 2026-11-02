# 시각 증거·한글 경로 시험판 게시 장부 — 2026-11-02

macOS Apple Silicon `0.2.1-preview.13`, Windows x64 `0.1.1-owner.12`.
사용자가 남은 PR 검토·병합과 최신 테스트 설치본 배포를 요청했다.
실제 사용자 설치·설정·Project·`.gil`은 이 게시 작업에서 변경하지 않았다. Luna 실사용은 사용자 검수 대상이다.

## 변경과 경계

- PR #72 시각 증거 정책: 새 Verify의 부실한 `not_needed`/`missing` 이유와 image evidence_files 모순을 거절한다.
  Verify가 첨부한 자료는 Analysis 및 Experiment Cycle Report에서 저장된 asset으로 인용해야 한다.
  기존 닫힌 기록은 다시 판정하거나 변환하지 않는다. 이 계약은 내용의 진실성이나 모델의 자발적 시각화를 증명하지 않는다.
- Monitor의 시각 자료 배지·개수·상세 패널과 누락 상태를 개선한다. 승인 성공·일시적 조회 실패 구분도 유지한다.
- PR #74 한글 경로: 입력 경로가 실제로 없을 때만 NFC/NFD 후보를 확인한다. 접근 오류나 복수 후보를 임의로 우회하지 않는다.
  보정 경로는 Monitor 준비·typed 보고서 요청에도 같다. 상하위 폴더 탐색은 하지 않는다.
- product-safety의 dirty 안내는 작업 내용을 별도로 보존한 뒤 기준 상태로 정리하도록 한다.
  일반 restore 및 Step revisit 차단은 유지한다. 모든 복구 경로를 새로 구현했다는 주장이 아니다.
- Mac·Windows의 공통 Core/Skill/UI 변경이다. Mac 직접 MCP 등록은 새 실행 파일 경로로 갱신해야 한다.

## 검토와 CI

- 원본 PR: [#72](https://github.com/hyun06000/gil/pull/72), [#74](https://github.com/hyun06000/gil/pull/74).
- 통합 검토/회귀 보완: [#76](https://github.com/hyun06000/gil/pull/76). 원본 커밋 이력을 보존해 병합.
- 빌드 source: `9e00303d6b59634e84b7f6bbe03becb26685e2c4`.
- 소스 통합 merge: `a4c2557b0d0d8bb905b71aa779baf88b29451b16`. 모든 적용 가능한 Mac/Windows PR CI 통과 후 병합.
- Mac candidate: [41363986809](https://github.com/hyun06000/gil/actions/runs/41363986809).
- Windows installer candidate: [41363989951](https://github.com/hyun06000/gil/actions/runs/41363989951).
- Marketplace PR [#17](https://github.com/hyun06000/gil-marketplace/pull/17), merge `311fa99fecb09dac019dca26014f1e186522bf90`.

## 실제 검증 범위

- Core 기본 전체: 1,535 passed / 0 failed / 2 ignored. all-targets 빌드 성공.
- 배포 프로필 lib/context/context_profiles/guidance_wave1/guidance_wave2/approved_plan_wire_wave1: 724 passed / 0 failed / 1 ignored.
- visual-authoring MCP·visual attachment: 56 passed. 실패한 close가 열린 단계와 저장 기록을 보존하고 수정한 요청으로 이어지는지 검증.
- JS/UI/Plugin/패키징/시나리오: 314 passed / 0 failed / 4 skipped. Windows 패키지·고지: 11 passed.
- Marketplace 정적 무결성·원시 Git history: 28 passed. 원본 CI payload, mode, 고지와 receipt를 그대로 보존.
- Mac 실제 배포 Core 승인 wire: 20 passed, 1 expected blocked, fail/unknown/unsupported/not_run 모두 0.
- Windows CI installer acceptance 13항목 true. 해당 Core wire도 20 passed, 1 expected blocked, 나머지 0.
- 공개 고정 URL에서 파일을 다시 내려받아 CI 원본과 바이트 단위 비교.
- 새 공개 archive와 동일한 바이트로 Git 없는 설치·패키징·업데이트 매니페스트: 105 passed / 0 failed.
- Companion의 실제 updater 설정에 대한 공개 매니페스트 검증: 1 passed. 공개 exe·서명·Mac archive는 원본과 byte-identical.
- 테스트 작성 중 assertion 위치·메서드 인자 오류는 수정 후 재실행했다. 최종 커밋 CI 결과를 판정 근거로 삼았다.
- Windows Chain 첫 실행은 기존 `an_artifact_change_reaches_the_screen`에서 복원 후 5초 내 clean 표시를 관측하지 못했다.
  감시 코드는 이번 변경에 포함되지 않았다. 같은 커밋의 실패 작업을 재실행해 결과를 확인했으며, 최초 실패를 삭제하거나 테스트를 완화하지 않았다.
- 실제 Host fullscreen·사람 승인 버튼·모델 실행은 이번 자동 시험에서 수행하지 않았다. 이전 사용자 검수를 이번 판의 인수로 승계하지 않는다.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Mac release | [v0.2.1-preview.13](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.13) |
| Mac archive | `gil-codex-macos-arm64-0.2.1-preview.13.tar.gz` / 3,585,903 bytes |
| Mac archive SHA-256 | `c346d27053c3bb638727dbfc4ce3859b1c83873052007f883b0f99c7b6f95d10` |
| Mac Core SHA-256 | `6ea86cd248a81e1473fb7a82fa4133b3467ad7a4a1068cd4f3466d1da6619e5f` |
| Mac receipt SHA-256 | `4fd98e63f3bfb994f59428917461decfcfcd427f59b47bdae3f14b0ab6b83adb` |
| 공통 UI SHA-256 | `df19d4e16aabe95b2579e0969c8173badfb0d924b9e4d51c4a9187ba4b3e8cf9` |
| Windows release | [windows-owner-update-0.1.1-owner.12](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.12) |
| Windows installer | `GIL.Companion.Unsigned.Test._0.1.1-owner.12_x64-setup.exe` / 6,110,897 bytes |
| Windows installer SHA-256 | `b678ed008c4a09ea41ce475326ad56ea38181c5f9d9c2a92a9db7b312b1f0fde` |
| Windows Core SHA-256 | `40d0f89d923f24f94762ddefd53a997def685ac8b8f8b90a9390c61ceb26921a` |

Windows는 게시자 인증서 서명 없는 시험판이며 업데이트 payload에는 `version:0.1.1-owner.12`가 결합된
서명을 사용한다. `requireSignedVersion`을 끄지 않았다. Mac은 Developer ID 서명·공증을 주장하지 않는다.
모두 prerelease이며 stable/latest로 지정하지 않는다. 새로운 버전으로 게시하며 기존 공개 버전의 바이트는 교체하지 않는다.

## Luna 인수

업데이트 후 새 연결에서 한글 경로의 테스트 프로젝트를 연다. 작은 데이터 분석을 요청하고 Verify 증거 파일·시각 첨부,
Analysis와 Cycle Report의 인용, Monitor 배지·그림·누락 표시를 확인한다. 승인 성공 뒤 조회 재시도가 승인 실패로
오인되지 않는지도 확인한다. 모델이 규칙을 어기면 거절 뒤 기록을 다시 읽고 올바른 입력으로 이어갈 수 있어야 한다.
