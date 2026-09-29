# Native MCP macOS release gates

상태: **공개 HTTPS marketplace·preview.2 공식 설치·rollback·보호/PVR 검수 완료** (2026-09-30).
현재 [설치 안내](INSTALL.md)와 [배포 장부](PREVIEW-2-PUBLICATION-20260930.md)가 정본이다.
날짜가 붙은 준비 기록은 당시 상태이며 릴리스 게시 여부는 장부의 versioned release 링크로 확인한다.
첫 인수 대상은 macOS Apple Silicon의 Codex Plugin이다. 이는 제작자 문서이며 사용자에게
Node·Cargo·clone·터미널 설치를 요구하지 않는다. Companion DMG는 별도 산출물이다.

## 1. 첫 시험판의 준비 → 공개 → 배포

유지관리자는 배포 준비를 마친 뒤 새 `gil`을 공개하고 배포하는 순서를 승인했다.
이후 Apple 개발자 서명·공증 없는 시험판 진행과 새 Mac 검수 유예를 명시적으로 승인했다.
나머지 미완료 게이트까지 면제하는 즉시 공개·출시 승인은 아니다. 기존 개발 저장소는 비공개로 유지한다.
작업 브랜치 → PR → 같은 head의 CI → 유지관리자 승인 후 병합 원칙을 계속 따른다.

| 게이트 | 필요한 증거 | 현재 |
|---|---|---|
| 선별된 source·MIT·고지·CI | 공개 표면 감사, 검수한 clean commit과 CI artifact 대응 | 이번 target의 한정 감사 통과 |
| 시험판 신뢰 표시 | Developer ID·공증 없음, OS 차단 가능성·승인 경계 표시 | 정책 승인·후보에 포함 |
| 기존 Mac 설치 | 공식 Plugin 설치본과 fullscreen | 원격 prerelease 교체·제거·재설치 및 화면 사용자 확인 |
| 새 Mac 설치 | 개발 도구 없는 새 Mac의 최초 실행·fullscreen·재시작 | 유예, 미통과 |
| 수명·업데이트 | version 쌍의 업데이트·제거/재설치 뒤 Project 보존 | 공식 preview.2→preview.1→preview.2 및 합성 4회 쌍 통과 |
| source 공개 | 공개 준비 감사, PVR 개통과 main 보호 적용·재조회, 운영 안내 | 공개·필수 PR/CI·PVR 활성화 확인 |
| 시험판 게시 | 불변 version·검증된 게시 경로·복구 안내, 승인된 동일 bytes 게시 | 공개 Git marketplace 설치 통과; versioned release는 장부 링크 확인 |
| 후속 정식 신뢰 채널 | Developer ID 서명·공증·새 Mac 설치 | 미준비; 첫 시험판 선행 조건 아님 |

새 Mac 인수에는 실제 다운로드/quarantine와 Host 실행 경로를 기록한다. 이미 개발하며 허용한
Mac의 smoke 성공, `spctl` 명령 한 번 또는 gatekeeper 설정 변경을 설치 인수로 대신하지 않는다.
`xattr` 제거, Gatekeeper 해제, 설치 cache 손편집은 정식 설치 방법이 아니다.
기존 Mac의 marketplace 설치와 화면에 이어 공개 비인증 HTTPS 다운로드·공식 설치를 확인했다.
새 Mac은 미검수다. [초기 후보 증거](UNSIGNED-PREVIEW-CHECKPOINT-20260928.md)와
[Host 설치 장부](HOST-INSTALL-CHECKPOINT-20260929.md),
[독립 marketplace 출처 전환](CLEAN-MARKETPLACE-CHECKPOINT-20260929.md)을 구분한다.

기존 개발 cachebuster를 stable version으로 오인하지 않으며 Plugin identity와 게시 주소를 임의로
바꾸지 않는다. `unsigned`는 **Apple Developer ID 서명·공증을 제공하지 않음**을 뜻하며, 실행 파일의
ad-hoc 서명까지 제거한다는 뜻이 아니다. staging은 입력 binary·Skill·UI bytes를 보존한다.
source에서 Skill만 바꾸더라도 새 payload이므로 기존 후보를 덮지 않고 새 version으로 검수한다.

### 1.1 후보 제작 (유지관리자용)

먼저 `build-preview.mjs`가 만든 tree의 clean source commit·snapshot·Core hash를 검수한 CI 증거와
대조한다. 아래 도구는 로컬 receipt 일관성과 실제 응답을 검사할 뿐 CI provenance의 암호학적
인증 도구가 아니다. 출처가 불명확한 binary는 입력하지 않는다.

```sh
node distribution/codex/unsigned-preview.mjs prepare target/codex-preview/marketplace 0.2.1-preview.2 target/codex-unsigned-candidate-2
node distribution/codex/unsigned-preview.mjs verify target/codex-unsigned-candidate-2/marketplace
```

- 새 output만 사용한다. 입력·설치 cache를 덮거나 기존 `release.json`의 flag를 뒤집지 않는다.
- `development_unsigned`의 dirty source는 거절한다. `X.Y.Z-preview.N`만 허용하고 stable·개발
  cachebuster는 거절한다. 입력 개발판보다 version 순서가 높아야 한다.
  `0.2.1-preview.1`은 이미 검수·설치된 불변 후보다. 위 preview.2 예시는 새 Skill 후보의 제작
  절차이지 게시·설치 승인 또는 공개 tag가 아니다.
- 정본 manifest는 건드리지 않고 출력 manifest의 version만 바꾼다. catalog·Core·Skill·고지는
  byte 그대로 유지한다. 바뀐 설치 metadata는 Host에서 별도 확인해야 한다.
- 압축 왕복 뒤 Node 없는 PATH로 native challenge·MCP initialize·17 tools·내장 UI를 새로 확인한다.
  실패/timeout은 재시도로 숨기지 않는다. 실패한 output은 진단용으로 남기고 성공 checks를 쓰지 않는다.
- `release.json`은 `channel: preview_unsigned`, `publishable: false`다. `checks.json`, `SHA256SUMS`,
  `update-record.json`도 생성한다. 이 단계에는 다운로드·설치·Apple credential·보안 설정 변경·게시가 없다.
- `codex-preview.yml`의 선택 입력 `unsigned_preview_version`으로 같은 후보 제작을 CI에서 수행할 수
  있다. 수동·read-only workflow이며, 생성은 공개 release/marketplace 게시가 아니다.

### 1.2 사용자 설치와 복구 안내

최종 기본 UX는 **검증된 marketplace의 GIL Plugin 설치 → 작업 폴더 선택 → 자연어 요청**이다.
공개 고정 ref와 공식 명령은 [INSTALL.md](INSTALL.md)에 있다. 셸 설치를 도와야
한다면 Agent가 제품·출처·고정 version/hash·미공증 사실·로컬 접근 범위를 설명하고 승인을 받는다.
무검증 `curl | sh`, 보안 검사 해제, Homebrew/Node/Cargo 선행 설치를 기본 UX로 추가하지 않는다.

사용자에게 보여 줄 핵심 고지:

> macOS Apple Silicon의 Codex용 GIL 시험판입니다. Apple 개발자 서명·공증을 받지 않았고,
> 새 Mac에서의 설치는 아직 확인하지 않았습니다. macOS가 실행을 차단할 수 있습니다.
> 보안 설정을 끄지 말고 문제가 생기면 설치를 멈춰 주세요. 선택한 작업 폴더를 로컬에서 읽고
> 승인한 GIL 행동으로 기록하며, Monitor는 읽기 전용입니다. 별도 Companion은 필요하지 않습니다.

새 폴더에서는 “지금 폴더에서 GIL 프로젝트를 시작해 줘”라고 요청한다. 기존 Project에서는
“기록을 바꾸지 말고 GIL Monitor를 열어 줘”라고 요청한다. 그래프·fullscreen·대화 병행을 실제로
확인하며, tool의 성공 응답만으로 설치 완료라고 말하지 않는다. 미지원 Host에서 Companion을
자동 실행하지 않고 선택지를 안내한다.

업데이트는 자동 실행하지 않는다. 유지관리자는 이전/다음 `update-record.json`을 다음처럼 비교한다.

```sh
node distribution/codex/update-policy.mjs update previous-record.json next-record.json
node distribution/codex/update-policy.mjs rollback next-record.json previous-record.json
```

같은 version의 bytes/source/계약 변경, 잘못된 version 방향, 호환 범위 변경 또는 migration은
차단/별도 검토다. `pair_test_required`는 실제 업데이트 성공이 아니다. 동일 조건에서도 임시
Project로 이전→다음→이전 쌍과 Host 제거/재설치·기록 보존을 검수한다. Plugin downgrade에
`gil restore`를 쓰거나 `.gil`을 삭제하지 않는다. 실패하면 마지막 검증판/Project를 보존하고
Host의 공식 Plugin 관리 경로로 복구하며 cache를 손편집하지 않는다.

게시 시 검수한 HTTPS/Git 출처와 불변 version/ref/hash를 고정하고 사용자에게 미검증 범위를
계속 보인다. 해시와 receipt는 publisher 신원 확인·악성 코드 검사·새 Mac 인수의 대체가 아니다.
Git marketplace 등록은 universal public Plugins Directory 심사·등재와도 다르다.

### 1.3 비공개 원격 인수 상태 (2026-09-29)

사용자 승인으로 `hyun06000/gil-distribution`을 비공개 생성했다. 검수한 CI 후보를 PR로 옮겼고,
GitHub에서 새로 clone한 고정 commit의 파일·권한·native 17 tools·내장 UI를 확인했다.
[원격 검수 장부](REMOTE-STAGING-CHECKPOINT-20260928.md) 이후 별도 승인으로 공식 Host 경로에서
개발판을 preview로 교체하고 제거·재설치·Project 보존·실제 화면을 확인했다.
[Host 인수 장부](HOST-INSTALL-CHECKPOINT-20260929.md)에 자동 시험, 사용자 실측, sandbox 실패와
미확인 재시작/rollback 범위를 구분한다. 공개 배포 완료로 표시하지 않는다.
이후 배포 Git 작성자 개인정보를 공개하지 않기로 하고 독립 이력의 `gil-marketplace`로 전환했다.
PR #1 병합·공식 설치 출처 전환·preview.1의 같은 bytes와 화면 확인을
[새 경로 장부](CLEAN-MARKETPLACE-CHECKPOINT-20260929.md)에 기록한다. 옛 `gil-distribution`은
비공개 감사/복구 이력이다. 공개 전환/설치 안내는 [게시 계획](PUBLICATION-PLAN-20260929.md)을 따른다.
Skill 보정 뒤의 preview.2는 별도 후보이며 위 preview.1 인수를 그대로 승계하지 않는다.

## 2. 후속 서명 채널의 읽기 전용 preflight

아래 기존 점검은 **서명 채널 전용**이다. Apple 계정이 없는 unsigned 시험판에 이 점검의 성공을
요구하지 않으며, 실패를 숨기거나 synthetic identity를 넣어 통과시키지 않는다.

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

## 3. 후속 Native CLI와 Companion의 공증 차이

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

Claude Desktop의 [9/28 경로별 진단](../../mcp-app/CLAUDE-DESKTOP-ROUTES.md)은 폴더 없는 대화의
Plugin 표시를 확인했지만, 폴더 연결 뒤의 차단과 fullscreen의 승인 창 가림은 남겼다. 이 때문에
이번 Codex 후보의 UI나 실행 계약을 변경하지 않으며 Claude 전체 지원으로 배포 범위를 넓히지 않는다.

## 근거 (2026-09-28 확인)

- [Apple Developer ID](https://developer.apple.com/developer-id/): 외부 배포의 서명·공증과 인증서 준비.
- [Apple custom notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow):
  제출 포맷, Keychain profile, Accepted 이후 log 검사, standalone binary/ZIP의 stapling 제한.
- 로컬 `xcrun stapler`의 지원 포맷과 `xcrun notarytool submit --help`도 읽기 전용으로 확인했다.
- [Apple trusted execution 설명](https://developer.apple.com/forums/thread/706442): 다운로드 경로와
  quarantine의 차이, 그 밖에도 실행될 수 있는 Gatekeeper 검사. CLI 설치가 모든 보안 검사의 면제는 아니다.
- [OpenAI Plugin packaging](https://developers.openai.com/plugins/build/plugins): 공식 marketplace 등록과
  Git ref/sha 선택. 설치 cache를 손편집하는 절차로 바꾸지 않는다.
- [공개 준비 장부](../../spec/GIL_Open_Source_Readiness_v0.1.md), [preview 계약](README.md).
