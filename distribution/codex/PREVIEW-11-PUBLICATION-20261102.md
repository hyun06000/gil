# Host 지원·Monitor 시험판 게시 장부 — 2026-11-02

macOS Apple Silicon `0.2.1-preview.11`, Windows 11 x64 `0.1.1-owner.10`.
사용자가 PR #66 병합 뒤 두 OS 테스트 설치본 배포를 요청했다. 설치·Claude Desktop 실사용 시험은
사용자가 진행하며, 이번 게시 작업에서 사용자 설정·설치된 Plugin·Project·`.gil`을 바꾸지 않는다.

## 변경과 경계

- Windows Companion이 Codex와 Claude Desktop을 각각 연결·교체·해제한다. 두 Host는 같은 고정 런처를 쓴다.
- Claude Desktop은 완전히 종료한 상태에서만 설정을 바꾼다. 프로세스 상태를 모르면 쓰지 않는다.
  기존 설정 백업, GIL writer 잠금, 고유 임시 파일, 교체 직전 digest 검사를 사용한다. 다른 설정은 보존한다.
  외부 편집기는 GIL 잠금을 따르지 않으므로 설정을 동시에 편집하지 않는다.
- 공통 Monitor에 확대·미니맵·메타데이터 업데이트 안내가 추가된다. 자동 다운로드·설치는 하지 않는다.
- 이전 승인 계획·Chain·Verify 실제 파일 증거·실패 기록 보존·복구 완료 후 채팅 전달 계약은 유지한다.
- macOS에는 고정 런처·Companion 연결 버튼이 없다. Claude Desktop 직접 등록은 새 archive의 Core로
  경로를 갱신해야 한다. Codex Plugin만 업데이트하면 Claude 직접 등록은 바뀌지 않는다.
- 기존 프로젝트 형식을 자동 변환하지 않는다. 새 프로젝트는 형식 7이며 downgrade도 기록을 변환하지 않는다.

## 소스와 공통 검증

- [PR #66](https://github.com/hyun06000/gil/pull/66), build source merge `3353c91fddb9b2c2c6d468a9d1f0f4b2ba032532`.
- PR 최종 head `fb2d4e35c327b64424ba1c4fe05bf88e5eb9d4ed`에서 Mac/Windows 검사 13개 통과.
  dispatch 전용 installer 작업 3개는 PR에서 의도적으로 skipped였으며 실제 후보 검증과 구분한다.
- 병합 전 로컬 Core 1,524 통과 / 2 기존 ignored, all-target 빌드 통과. Companion 113 통과,
  JS UI·Plugin·배포·고지 295 통과 / 4 기존 skipped. 사용자 프로젝트를 시험 데이터로 사용하지 않았다.
- 확대·미니맵은 격리 예제 브라우저에서 확인했다. 실제 Host 화면이나 사용자의 승인을 대신하는 증거가 아니다.

## Mac 배포물과 검증

- [후보 CI 41354472592](https://github.com/hyun06000/gil/actions/runs/41354472592): approved-plan 프로필,
  회귀검사·고지·native build·archive 왕복·첫 실행 프로토콜 통과.
- [배포 PR #15](https://github.com/hyun06000/gil-marketplace/pull/15), tag commit `52e7699b8dfa444698afcdaf489a6805521d520e`.
- [고정 prerelease v0.2.1-preview.11](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.11).
- archive `gil-codex-macos-arm64-0.2.1-preview.11.tar.gz`, 3,565,880 bytes.
- archive SHA-256 `955cf16c566939543d02f96077f71a8eb186333187f9ccd3bcb708ad27537fdb`.
- Core `826a59198206d33aeee40cbdf10443f9f0e49d4c2f401bbb9f27d7bd37cd32bd`.
- UI `c25c2b43cd67739ba9c06f2fa2c8552f71a423782b55538f65dbf8dcb46b941f`.
- receipt `61ce56189bb84e5eadd4fe5a7a653d9e32fe1a9e7238432fe6839a8fe2d195cb`.
- CI archive·고지·receipt·실행 권한을 그대로 게시했다. `publishable:false`는 빌드 당시 기록으로 유지한다.
- 배포 저장소 무결성·raw history 28개 통과, PR CI 통과. 정확한 다운로드 Core의 approval wire
  20개 통과 / 1 예상 product-safety 차단. Git 없는 준비 절차 7개 통과.
- Git 없는 시험 첫 실행은 예전 버전을 명시한 assertion 두 곳을 발견했다. 새 버전 pin에 맞추고
  원래 검증 조건을 유지한 재실행이 통과했다. archive 내용이나 판정 기준을 완화하지 않았다.
- 인증 없는 공개 HTTPS 다운로드의 크기·해시가 CI 원본과 일치함을 확인했다.
- 공개 archive를 입력한 source 배포·고지·Git 없는 준비·updater manifest 작성 시험 102개가 통과했다.
  새 Windows 서명을 받은 뒤 실행하는 업데이트 feed 대조 검사는 별도다.
- Apple Developer ID·공증은 없다. 사용자 기기 설치·보안 설정 변경은 하지 않았다.

## Windows 배포물과 채널

- [후보 CI 41354479275](https://github.com/hyun06000/gil/actions/runs/41354479275): 제품 계약과 정확한 설치본 인수 통과.
- [고정 prerelease windows-owner-update-0.1.1-owner.10](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.10).
- installer `GIL.Companion.Unsigned.Test._0.1.1-owner.10_x64-setup.exe`, 6,048,931 bytes.
- SHA-256 `d24356e3329e827f51e1fe2f63c7916ebdc11bcb3a2b8486b6ab137d0d5a7247`.
- Core `1f409aae4798829f174da4b8acdb281931e173d499286c2f28a9d91bae196c0a`.
- 설치·재설치·실행 중 버전 선택·기존 연결 유지·새 연결 Core·창 표시·Codex CLI 등록/해제·제거 후
  runtime 보존 등 13개 인수 항목 통과. 설치된 Core의 hash가 wire에서 시험한 Core와 같다.
- 승인 wire 20개 통과 / 1 예상 product-safety 차단. 실제 Desktop 채팅·Claude 연결 버튼 클릭은 별도 사용자 시험이다.
- updater signature trusted comment에 `version:0.1.1-owner.10`이 있다. installer·signature 바이트는
  CI 원본이며 GitHub용 파일명만 정규화했다. Authenticode publisher 서명은 없다.
- 인증 없는 공개 다운로드를 CI 원본과 byte-for-byte 비교했다. 검증 뒤 `owner-update.json`과
  공통 `update-feed.json`을 함께 새 버전으로 전환한다. 업데이트 feed 대조 검사 3개도 통과했다.
- 설치는 사용자의 업데이트 버튼으로 시작한다. 실행 중인 Core를 덮어쓰거나 기존 연결을 강제 종료하지 않는다.
- 배포 PR 첫 CI에서는 Companion의 updater manifest 시험이 이전 `owner.9`를 기대해 실패했다.
  `cfg(test)`의 정확한 기대 버전을 `owner.10`으로 갱신했다. 런타임 코드·이미 게시한 설치본은 바꾸지 않았다.

## 사용자 인수

Windows에서는 업데이트 뒤 Companion에서 Claude Desktop 연결 버튼을 확인하고 Desktop을 완전히
종료한 상태에서 연결한다. Desktop 재실행 → Claude Code 탭의 새 대화 → 테스트 폴더 → GIL 시작을 확인한다.
macOS에서는 검증한 archive의 새 폴더 Core를 Desktop 설정에 직접 등록한다. 기존 다른 MCP 항목은 보존한다.
이전 Claude Plugin 경로에서는 App이 표시되지 않았으므로 직접 등록 경로를 사용한다.

Monitor 카드 표시 → 클릭 fullscreen → 확대/미니맵 → 승인 계획 → Interview 닫기 → 계획 항목별
Experiment → Verify 파일 증거·Analysis·Outcome 배움 순서로 시험한다. 창 표시, 클릭, 실제 모델 작업은
사용자 확인 대상으로 남기며 CI 통과나 이전 버전의 인수를 이번 판의 실사용 성공으로 쓰지 않는다.
기존 기록이 있는 프로젝트는 정확한 루트에서 현재 context를 먼저 읽는다. 이미 완료된 복구를 반복하지 않는다.
