# Windows owner.15 게시 장부 — 2026-11-10

사용자가 Windows도 최신 공통 기능으로 맞추도록 승인했다. 기존 Windows owner 채널의 새 불변 버전
`0.1.1-owner.15`를 게시하며 owner.14 자산·키·endpoint·제품 identity는 바꾸지 않는다.
macOS preview.29는 [기존 장부](../codex/PREVIEW-29-PUBLICATION-20261110.md)의 배포물 그대로 유지한다.
실제 설치와 사용자 필드테스트는 이번 작업에 포함하지 않는다.

## 변경과 출처

- source/build: `d0a13bc07b6937178e9488905604685002069125`. 최신 `github/main`에서 별도 작업 브랜치를 만들었다.
- macOS preview.29의 Core 변경을 포함하며, 그 빌드 이후 차이는 #110의 게시 문서·배포 pin뿐이다.
- [PR #109](https://github.com/hyun06000/gil/pull/109)의 계획 근거 schema·Synthesis 영수증·줄별 일괄 오류,
  PNG 수치 제한 안내를 포함한다. 참조 자동 이동이나 승인·기록·Snapshot 마이그레이션은 없다.
- owner.14 이후의 훅 제거, 프로젝트별 새 세션 재개, Core 버전 표기, typed close·계획 안내,
  Monitor 업데이트 안내와 [PR #107](https://github.com/hyun06000/gil/pull/107)의 fullscreen 응답 구분도 포함한다.
- runtime: `approved-plan,visual-authoring,product-safety`. Core CLI는 `gil 0.1.1-owner.15`로 빌드 중 검증한다.
  Companion의 embedded Core도 같은 기능 프로필이다.
- 후보 CI: [42669533790](https://github.com/hyun06000/gil/actions/runs/42669533790).
- 설치·Monitor feed·version-bound updater manifest는 같은 owner.15를 가리킨다. macOS feed entry는 보존한다.
  새 공개 manifest를 읽는 실제 pinned Tauri updater의 역직렬화 회귀시험도 갱신한다.

## 후보 검증

- 위 CI의 제품 계약과 updater 후보 설치 인수가 성공했다. 선택하지 않은 일반 unsigned 후보·Authenticode
  서명 후보 두 경로는 skipped이며 해당 인수를 통과했다고 세지 않는다.
- 패키지에 포함된 실제 Windows x64 Core wire: 20 passed / 1 expected product-safety block / 0 failed.
  합성 App 승인이며 실제 Monitor 화면·사람 승인·모델 실행 증거가 아니다.
- Windows CI 설치 인수 13항목 성공: 사용자 단위 설치, 같은 버전 복구 설치, 실행 중 MCP를 유지한
  버전 전환, 기존 연결 생존, 다음 연결의 새 Core 선택, legacy runtime 보존, Companion 창 핸들 확인,
  launcher MCP, 실행 중 Core 쓰기 거절, 공식 Codex CLI 등록·해제, Companion 제거와 runtime 보존.
- CI의 버전 전환은 합성 이전 버전 디렉터리에 같은 후보 Core bytes를 사용한다. 실제 owner.14→owner.15
  사용자 업데이트나 구버전 런처 전환·모델 실사용을 검증한 것으로 주장하지 않는다.
- installer bytes·hash·소스·runtime 버전과 Core digest를 acceptance/wire 영수증에 대조했다.
  updater 서명의 trusted comment는 `version:0.1.1-owner.15`와 원본 installer 파일명에 결합되어 있다.
  공개 asset 이름만 정규화하며 파일 bytes와 서명은 변경하지 않는다.
- Windows 배포 Node 36 passed, JS/UI/Plugin 192 passed / 기존 5 skipped.
- 임시 HOME의 Core `cargo test --locked -p gil`: 1,578 passed / 기존 2 ignored (doctest 포함).
  `cargo build --locked -p gil --all-targets` 성공.
- Companion 자체 112 passed와 새 공개 manifest의 pinned Tauri updater 역직렬화 1 passed.
  최종 packaging/notices 106 passed, skip 없음. Core-only 시험을 Companion 성공으로 대신하지 않았다.
- GitHub 공개 자산 digest를 대조한 뒤 인증 없는 공개 HTTPS에서 다섯 자산을 재다운로드했다.
  모두 CI 후보와 크기·SHA-256·전체 bytes가 일치했다.

## 고정 배포물

| 항목 | 값 |
|---|---|
| Release | [windows-owner-update-0.1.1-owner.15](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.15) |
| Tag/source commit | `d0a13bc07b6937178e9488905604685002069125` |
| Installer | `GIL.Companion.Unsigned.Test._0.1.1-owner.15_x64-setup.exe` / 6,113,782 bytes |
| Installer SHA-256 | `fefcd6c22cf787130db0a63151af869c64caac393d1a15d22dcfe9b11f3aeaf2` |
| Core SHA-256 | `2c0b2e6f73548b67f2a02ab787d6d53f0e22004cd2c2e6076ea66909bd689c9a` |
| Signature SHA-256 | `f0d4ccf4b49b41dc4f8936a7fd1b43edca4af8db700b4f935fa6276291e4a9c5` |
| Acceptance SHA-256 | `31a55785c46d3f6298129a7e5711b88ff366959d9ec7e534c7e7b0b14c8def08` |
| Wire report SHA-256 | `243e55afb1abab1ba3157577de77b02b68997dea430c51994c12dd0d5a406c2f` |
| SHA256SUMS SHA-256 | `3351ffbf59f35d8eba67b35375da7c96ad0ef29b3040253a7f414bf101316017` |

## 사용자 검수와 한계

이 배포는 unsigned publisher / signed updater prerelease다. updater 서명은 Authenticode publisher 신뢰나
SmartScreen 평판을 대신하지 않는다. stable/latest로 올리지 않으며 보안 설정이나 권한을 바꾸지 않는다.
Windows ARM64 native Core/Companion CI 범위는 유지하지만 제품 installer는 x64만 제공한다.

이번 owner.15의 실제 사용자 설치, 버튼 updater, Codex·Claude Desktop 화면·fullscreen·모델의 단계별
안내 준수는 사용자 필드테스트 대기다. macOS에서 Windows installer를 실행했다고 주장하지 않는다.
설치 절차는 [Windows 안내](INSTALL.md)를 따르며, 구형 Host 훅이 있다면 검증된 새 Core 또는 정확한
백업·diff 검토로 GIL 항목만 별도 정리한다. 실제 사용자 설정·프로젝트·기록·Snapshot은 이번 작업에서 변경하지 않았다.
