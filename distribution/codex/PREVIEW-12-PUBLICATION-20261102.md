# 승인 상태 조회 구분 시험판 게시 장부 — 2026-11-02

macOS Apple Silicon `0.2.1-preview.12`, Windows 11 x64 `0.1.1-owner.11`.
사용자가 승인 성공과 일시적인 조회 실패의 구분, 시험, CI 통과 후 PR 병합과 양 OS 테스트 배포를 요청했다.
사용자 설정·설치된 Plugin·Project·`.gil`은 변경하지 않는다. 실제 Host와 Luna 작업은 사용자 인수 대상이다.

## 동작과 경계

- 최초 조회는 확인 중, 처음 두 번의 조회 실패는 회색 재시도, 연속 세 번째부터는 조회 실패로 표시한다.
- 마지막으로 확인한 승인/수정 요청의 판을 유지하되 최신 상태와 같다고 단정하지 않는다.
  이전 승인을 새 제안에 적용하지 않으며 조회 실패 시 이전 capability와 승인 버튼을 무효화한다.
- 실제 저장 응답에 같은 판·digest·decision receipt가 있어야 성공과 채팅 전달을 표시한다.
  응답이 없거나 맞지 않으면 저장 결과 불확실로 남기고 읽기로 재확인한다. 쓰기 자동 재시도는 없다.
- 성공한 조회만 30초 간격을 적용한다. 실패는 2/4/8/15초 간격으로 재시도하며 busy는 성공으로 세지 않는다.
  진행 중 새로 도착한 갱신 힌트는 유지한다. 숨겨진 App의 폴링 중단 계약은 유지한다.
- Codex·Claude Desktop과 OS에 같은 공통 UI를 적용한다. 프로젝트 형식·backend 잠금·승인 계약은 바꾸지 않는다.
- 사용자 화면의 정확한 최초 실패 원인은 로그로 확정하지 않았다. 화면 갱신 오류를 승인 저장 실패처럼
  보여 주고 30초 기다리던 경로를 고쳤다. 모든 Host 통신 실패 자체가 없어졌다는 뜻은 아니다.

## 소스와 검증

- [소스 PR #73](https://github.com/hyun06000/gil/pull/73), build head `bbd7db3b29d69ec0ca6661336ecc2ed5a87aa12b`.
- Mac/Windows CI 10개 통과 뒤 merge `1aaea711406609d4cf3f3d3ef7971d9b373501b4`로 병합했다.
- 로컬 Core 1,524 통과 / 2 기존 ignored, all-target 빌드 통과. 첫 sandbox 실행은 loopback socket
  생성이 차단돼 실패했으며 동일 시험을 로컬 소켓 접근 허용 상태로 재실행해 통과했다.
- 승인 controller/폴링 회귀 24개, UI·Plugin·배포·고지 합계 268 통과 / 4 기존 skipped.
- 격리된 합성 브라우저 화면에서 승인 전/후 일시적 조회 재시도와 지속 실패를 구분했다.
  실제 Host의 승인 클릭이나 모델 후속 작업을 대신하는 증거는 아니다.
- 세 플랫폼 고지 inventory는 다시 수집했고 UI hash 외 dependency/license/toolchain은 동일했다.

## Mac 원본 배포물

- [후보 CI 41359559965](https://github.com/hyun06000/gil/actions/runs/41359559965), approved-plan 프로필.
- [배포 PR #16](https://github.com/hyun06000/gil-marketplace/pull/16), CI 통과 후 병합.
- tag commit `1132114d299c896e9879c3b9f108a8349c5ff691`.
- [고정 prerelease v0.2.1-preview.12](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.12).
- archive `gil-codex-macos-arm64-0.2.1-preview.12.tar.gz`, 3,566,625 bytes.
- archive SHA-256 `41f1bb5bfafac4912efc8251d5dc31ec462221e8e3805dc3fdfb890bc18943f1`.
- Core SHA-256 `c43cb57d3c7f786713e3599f2d3e90afe4cc097ff8bcd9f6d5963f9f9bd5b543`.
- UI SHA-256 `e7830fa2bc588072d60f83f4df3729db587320c15a186bc9a8f84afa14bc43bc`.
- receipt SHA-256 `594db0a80056c8d66f004ae60364fe566daecd3a028e543f2d7a5f282ae401b8`.
- CI 원본 바이트·실행 권한·고지를 유지했다. `publishable:false`는 빌드 당시 기록으로 남긴다.
- 배포 저장소 무결성/raw history 28개 통과. 정확한 배포 Core wire 20개 통과 / 1 예상 안전 차단.
- 인증 없는 공개 HTTPS 다운로드를 CI 원본과 byte-for-byte 비교했다. 공개 archive 기반 Git 없는
  준비 시험 7개 통과. 실제 설치·설정 변경은 하지 않았다. Apple Developer ID·공증은 없다.

## Windows 원본 배포물

- [후보 CI 41359562028](https://github.com/hyun06000/gil/actions/runs/41359562028), 제품 계약·설치 인수 통과.
- [고정 prerelease windows-owner-update-0.1.1-owner.11](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.11).
- installer `GIL.Companion.Unsigned.Test._0.1.1-owner.11_x64-setup.exe`, 6,051,801 bytes.
- SHA-256 `140be96ad41107da9f227b7ebe4a3af54121052511ec79f7ca86ce82d8827e78`.
- Core SHA-256 `b1da44fbe4f93cebbe543d941050d9978ed39db31bb66f5429db2b08e8c32200`.
- 설치·재설치·실행 중 버전 선택·기존 연결 유지·새 연결 Core·창 표시·Codex CLI 등록/해제·제거 후
  runtime 보존 등 인수 13개가 통과했다. 설치 Core와 wire 시험 Core hash가 같다.
- 정확한 Core wire 20개 통과 / 1 예상 안전 차단. 실제 Desktop 채팅·승인 클릭은 사용자 시험이다.
- updater signature trusted comment의 `version:0.1.1-owner.11`을 확인했다. `requireSignedVersion`은 유지한다.
  Authenticode publisher 서명은 없다. 설치본·서명 바이트는 CI 원본, 공개 파일명만 정규화했다.
- 인증 없는 공개 HTTPS 설치본·서명 다운로드를 CI 원본과 byte-for-byte 비교했다.
  검증 뒤 `owner-update.json`과 공통 `update-feed.json`을 같은 게시 PR로 전환한다.
- 사용자가 버튼을 눌러 설치하며 기존 실행 중 Core를 덮어쓰거나 연결을 강제 종료하지 않는다.

## 게시 채널 검증

공개 Mac archive를 입력한 배포·고지·Git 없는 준비·updater manifest 시험 105개가 통과했다.
Windows manifest의 새 버전 기대값은 Companion의 cfg(test)에서만 갱신했다. 이미 게시한 설치본이나
런타임 updater 동작은 바꾸지 않았다. 업데이트 feed와 설치 안내는 같은 PR로 전환하고 CI 통과 후 병합한다.

## 사용자 확인

업데이트 뒤 새 Host 연결과 새 Monitor에서 제안 → 사용자 승인 → 승인 기록 → Interview 닫기를 확인한다.
작업 중 일시적 조회 실패가 생겨도 확인한 승인 판을 보여 주는지, 상태를 다시 읽으면 안내가 사라지는지 본다.
새 제안에는 새 승인이 필요하다. 결과를 못 받은 저장은 성공이라 단정하지 말고 읽기로 확인한다.
실제 실패를 만들려고 프로젝트 기록이나 권한을 바꾸지 않는다. 이전 배포의 실기기 결과를 이번 판의 성공으로
간주하지 않는다. 기존 실패 복구가 끝난 프로젝트는 context를 먼저 읽고 복구를 반복하지 않는다.

Windows의 기존 고정 런처 등록은 유지되며 새 연결부터 새 Core를 사용한다. Mac Codex Plugin 업데이트와
Claude Desktop 직접 Core 경로 갱신은 별개다. Mac Claude 경로는 Desktop을 완전히 종료한 뒤 갱신한다.
