# Windows 계획 입력·Closing 안내 검증판 — 2026-11-03

사용자가 macOS와 Windows 모두 게시하도록 승인했다. macOS preview.20은
[기존 게시 장부](../codex/PREVIEW-20-PUBLICATION-20261103.md)를 따르며 이번 작업에서 bytes를 바꾸지 않았다.
Windows x64는 새 불변 버전 `0.1.1-owner.14`를 사용한다. 공개 owner.12와 미게시 후보 owner.13은 덮어쓰지 않는다.

## 변경과 출처

- [PR #86](https://github.com/hyun06000/gil/pull/86)과 [독립 검토 보완 #87](https://github.com/hyun06000/gil/pull/87)의
  draft 중첩·체크리스트 예시·synthesis 문자열 맵·Closing chain_verdict 및 승인 후 흐름을 포함한다.
- source/build: `860afe67bc44b14018b7dcd1380322358aedb911`.
  Mac preview.20의 `c5a8fe61da89c67777412521322c1011645b607e` 이후 차이는 #88의 게시 문서·배포 pin뿐이다.
- runtime: `approved-plan,visual-authoring,product-safety`.
- 후보 CI: [41578140901](https://github.com/hyun06000/gil/actions/runs/41578140901).
- 기존 updater 서명 경로를 사용하며 키·endpoint·전역 훅·권한·사용자 설치본·실제 Project는 변경하지 않는다.

## 검증 기록

- 소스 #86/#87 검토와 전체 10개 CI, Mac preview.20의 바이트·wire 검증은 위 기존 장부에 기록했다.
  Windows 빌드 기준 #88의 최신 head 전체 8개 해당 CI도 통과했다.
- Windows 배포 Node 검사 34 passed. 새 worktree의 첫 실행은 esbuild 의존성 부재로 2 failed였고,
  CONTRIBUTING의 잠긴 npm 의존성 설치 후 전체 재실행해 통과했다.
- JS/UI/Plugin 181 passed / 기존 5 skipped. 공통 workflow–생성 Skill 동기화 통과.
- 실제 pinned Tauri updater의 새 공개 manifest 역직렬화 시험 1 passed. Core-only 시험으로 대신하지 않았다.
- 문서와 피드를 순차 편집하던 중 packaging의 일치 검사는 옛 manifest를 감지해 실패했다.
  후보의 실제 서명으로 manifest를 완성한 뒤 최종 packaging/notices 103 passed, skip 없음을 확인했다.

- 후보 CI의 제품 계약과 updater 후보 설치 인수 모두 성공, 재실행 없음. 선택하지 않은 두 배포 경로만 skipped다.
- 실제 패키지 Core wire: 20 passed / 1 expected product-safety block / 0 failed.
- Windows 인수 13항목 모두 성공: 사용자 단위 설치, 같은 버전 복구 설치, 실행 중 MCP 연결을 유지한
  버전 전환, 다음 연결의 새 Core 선택, legacy runtime 보존, Companion 창 표시, launcher MCP,
  실행 중 Core 쓰기 거절, Codex CLI 등록·해제, Companion 제거와 runtime 보존.
- installer bytes·hash·소스·runtime 버전을 CI 영수증과 대조했고, wire가 시험한 Core digest도 일치했다.
  서명의 trusted comment는 `version:0.1.1-owner.14`와 원본 파일명에 결합되어 있다.
- GitHub 자산 digest를 대조하고 공개 HTTPS에서 인증 없이 다섯 자산을 재다운로드하여 모두 원본과
  바이트 단위로 일치함을 확인했다. 파일명만 공개 asset 규칙에 맞췄으며 installer·서명 bytes는 바꾸지 않았다.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [windows-owner-update-0.1.1-owner.14](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.14) |
| Tag/source commit | `860afe67bc44b14018b7dcd1380322358aedb911` |
| Installer | `GIL.Companion.Unsigned.Test._0.1.1-owner.14_x64-setup.exe` / 6,165,288 bytes |
| Installer SHA-256 | `f1be0b1bc0cd102a4045be9ddb0c4f5aaf572d8faa9b539cfe867dba32f86267` |
| Core SHA-256 | `456bf882ed78765fd3a8c4b2cdccc524ef415d9b66d9fa7da3e88186c459ee3c` |
| Signature SHA-256 | `aa9f7059784d6bb50d7b058d718d6179e532ae78a3fc86f580b9dbac283ed4ec` |
| Acceptance SHA-256 | `446f1b40939e91891c5f780b3607047a160ed6163a7ca24de177158536e66aef` |
| Wire report SHA-256 | `9fe960d9537637e9cf761a4dbf8a7b875a1e69d8cf783bfba22dbfcbd03acf78` |
| SHA256SUMS SHA-256 | `f87fa845f782efedb467b77d8602d34b272f48b5a919cde5d56813941e28f588` |

## 사용자 검수와 한계

Authenticode publisher 인증서가 없는 unsigned prerelease다. version-bound updater 서명은 publisher 서명이나
SmartScreen 평판을 대신하지 않는다. stable/latest로 올리지 않으며 보안 설정을 바꾸지 않는다.

이번 버전의 Host 화면·fullscreen·실제 승인 버튼·모델 실사용 검수는 사용자 확인 대기다.
Windows ARM64 native Core/Companion CI 범위는 유지하지만 제품 installer는 x64만 제공한다.
새 연결·새 빈 테스트 폴더에서 Opening 제안과 승인, Experiment 증거, Closing 제안과 승인 후 Chain 종료를 확인한다.
설치와 업데이트는 [통합 설치 안내](../../INSTALL.md)를 따른다.
