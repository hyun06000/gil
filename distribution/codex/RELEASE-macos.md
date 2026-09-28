# Native MCP macOS release gates

상태: **배포 준비 중, 실제 서명·공증 미실행** (2026-09-28).
첫 인수 대상은 macOS Apple Silicon의 Codex Plugin이다. 이는 제작자 문서이며 사용자에게
Node·Cargo·clone·터미널 설치를 요구하지 않는다. Companion DMG는 별도 산출물이다.

## 1. 준비 → 공개 → 배포

유지관리자는 배포 준비를 마친 뒤 새 `gil`을 공개하고 배포하는 순서를 승인했다.
미완료 게이트를 무시하는 즉시 공개·출시 승인은 아니다. 기존 개발 저장소는 비공개로 유지한다.
작업 브랜치 → PR → 같은 head의 CI → 유지관리자 승인 후 병합 원칙을 계속 따른다.

| 게이트 | 필요한 증거 | 현재 |
|---|---|---|
| 선별된 source·MIT·고지·CI | 공개 표면 감사, 검수한 clean commit과 CI artifact 대응 | 부분 완료 |
| 서명 환경 | Apple Developer Program, 유효한 Developer ID Application identity, 명시적 선택 | 미준비 |
| native 코드 서명 | hardened runtime·secure timestamp·유효한 서명 검증 | 미실행 |
| 공증 | 정확한 제출물 hash와 Accepted 응답, 성공이어도 notary log 검토 | 미실행 |
| 실제 설치 | 개발 도구 없는 새 Mac, 검수용 marketplace 설치·첫 실행·fullscreen·재시작 | 미실행 |
| 수명·업데이트 | 업데이트·제거/재설치 뒤 Project 보존, 최초 실행 timeout 해소 | 미실행 |
| source 공개 | 공개 준비 감사, PVR 개통과 main 보호 적용·재조회, 운영 안내 | 미실행 |
| 정식 배포 | 불변 version·검증된 게시 경로·rollback 정책, 승인된 동일 bytes 게시 | 미실행 |

새 Mac 인수에는 실제 다운로드/quarantine와 Host 실행 경로를 기록한다. 이미 개발하며 허용한
Mac의 smoke 성공, `spctl` 명령 한 번 또는 gatekeeper 설정 변경을 설치 인수로 대신하지 않는다.
`xattr` 제거, Gatekeeper 해제, 설치 cache 손편집은 정식 설치 방법이 아니다.
비공개 검수용 marketplace의 설치 경로·접근 방식도 아직 인수하지 않았다.

서명 전에 version·Plugin identity·게시 경로를 확정해야 한다. 기존 개발 cachebuster를 stable
version으로 오인하지 않으며 임의의 이름·주소로 바꾸지 않는다. 서명 후 bytes가 바뀌면 이전
hash·공증 증거를 재사용하지 않는다. preview receipt의 `publishable: false`를 수동으로 뒤집어
release를 만드는 기능은 없다. 현재 빌더와 CI는 개발 preview만 만든다.

## 2. 지금 실행할 수 있는 읽기 전용 preflight

```sh
node distribution/codex/release-preflight.mjs environment
node distribution/codex/release-preflight.mjs candidate target/codex-preview/marketplace
```

- 환경: macOS arm64, codesign/notarytool/ditto, 유효한 Developer ID Application identity,
  `APPLE_SIGNING_IDENTITY`의 정확한 일치, `APPLE_NOTARY_PROFILE`의 지정 여부만 검사한다.
- `APPLE_SIGNING_IDENTITY`는 Keychain의 identity 이름 또는 SHA-1 지문이다. 부분 일치·모호한 선택은
  거절한다. `APPLE_NOTARY_PROFILE`은 나중에 notarytool에 사용할 Keychain profile 이름이며,
  이름이 있다는 사실을 credential 존재·인증 성공으로 세지 않는다.
- candidate는 기존 preview verifier로 allowlist·bytes·mode·고지를 확인하고, clean source의
  commit·파일 수·snapshot hash를 대조한다. binary와 source의 빌드 대응·publisher 인증은
  이 비교만으로 증명되지 않으므로 독립 CI/build evidence가 계속 필요하다.
- candidate binary를 실행하지 않고 서명·공증 제출·credential 등록·파일 수정·게시도 하지 않는다.
  도구가 출력한 경로, identity, profile 이름, 오류 원문은 보고서에 싣지 않는다.
- exit **0**은 준비 점검만 통과, **1**은 미준비/미검사, **2**는 사용법/실행 오류다.
  `environment`만 실행하면 candidate가 미검사여서 1이다. 모든 경우 `publishable: false`다.
  다른 OS/CPU에서 macOS 명령을 실행하지 않는다.

2026-09-28 현지 점검: 도구 세 가지 있음, Developer ID Application identity 없음,
서명 identity와 notary profile 미지정. 유지관리자는 Program 미가입 상태를 확인했다.
credential을 가져오거나 계정·결제를 생성하지 않았다. 회귀시험의 synthetic identity는
실제 Apple 서명·인증 증거가 아니다.

## 3. Native CLI와 Companion의 공증 차이

Apple은 ZIP, UDIF DMG, signed flat PKG를 공증 입력으로 받는다. 현재 preview의 tar.gz를
그대로 공증 제출물로 취급하지 않는다. native CLI는 Developer ID Application으로 서명하고
hardened runtime·secure timestamp를 확인한 뒤 ZIP에 담아 공증할 수 있다.
**단독 실행 파일과 ZIP에는 티켓을 staple할 수 없다.** 티켓은 온라인으로 조회되는 경로이며,
Companion `.app`/DMG의 `stapler` 절차를 단독 `gil`에 복사하면 안 된다.

따라서 예정 검수 순서는 다음과 같다. 아직 자동 서명/제출 구현 또는 성공 증거가 아니다.

1. 승인된 clean source/version에서 빌드하고 source 안정성·고지·테스트를 확인한다.
2. 새 staging 복사본만 서명한다. 개발 preview·설치본은 덮지 않는다. 최소 권한을 유지하고
   hardened runtime·secure timestamp·서명 유효성을 검증한다.
3. 정확한 서명 bytes를 ZIP으로 만들고 hash를 기록해 notarytool로 제출한다.
   인증은 Keychain profile 참조를 사용한다. 비밀번호·키 본문을 소스/명령 로그에 넣지 않는다.
4. Accepted를 확인하고 notary log의 warning/error도 검토한다. timeout은 성공이 아니다.
   제출 ID로 결과를 이어 확인하며 같은 파일을 무조건 재제출하지 않는다.
5. 공증한 실행 파일의 bytes를 그대로 marketplace 산출물에 포함해 전달 후 hash를 대조한다.
   원문 notary log의 경로·계정 식별자는 검수 없이 공개하지 않는다.
6. 새 Mac의 실제 marketplace에서 설치하고 최초 실행·fullscreen·재시작·업데이트를 검수한다.
   네트워크 사용 여부와 offline 제약도 기록한다. standalone CLI의 offline 첫 실행을 보장하지 않는다.

Host의 다운로드/캐시/실행 방식까지 통과해야 이 포맷을 정식으로 채택한다. 통과하지 못하면
그 실패를 먼저 보고하고 배포 컨테이너를 재설계한다. 별도 DMG 설치를 몰래 추가하거나
Plugin 한 번 설치라는 사용자 계약을 조용히 바꾸지 않는다.

Windows·Intel/universal·Claude 작업 Plugin의 화면 전달·Companion 재배포는 이번 인수와 별개다.
macOS/Codex를 닫기 전에 이들까지 지원한다고 표시하지 않는다.

## 근거 (2026-09-28 확인)

- [Apple Developer ID](https://developer.apple.com/developer-id/): 외부 배포의 서명·공증과 인증서 준비.
- [Apple custom notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow):
  제출 포맷, Keychain profile, Accepted 이후 log 검사, standalone binary/ZIP의 stapling 제한.
- 로컬 `xcrun stapler`의 지원 포맷과 `xcrun notarytool submit --help`도 읽기 전용으로 확인했다.
- [공개 준비 장부](../../spec/GIL_Open_Source_Readiness_v0.1.md), [preview 계약](README.md).
