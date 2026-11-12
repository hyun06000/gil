# Windows 설치 신뢰 전략 — 2026-11-12

> 상태: **변형 B(Store MSIX) 진행 중 (2026-11-13 소유자 승인).** 구현·제출 절차는 [STORE.md](STORE.md). 재검토 조건 충족 경위: 소유자 PC(Smart App Control 켜짐)에서 미서명 `gil.exe`가 실행 단계에서 차단됐다([v0.3.0 장부](PUBLICATION-V0.3.0-20261113.md) 실기기 결과). 결정서 §3에 따라 결제·계정 개설은 소유자 결정 전에는 진행하지 않는다. 후보: §9 변형 B(Store MSIX, `gil.exe` + 실행 별칭). 현행 미서명 채널은 [SIMPLE-INSTALL-20261112.md](SIMPLE-INSTALL-20261112.md)이며 SAC가 꺼진 PC에만 해당한다.
> 범위: Windows 11 x64 사용자가 OS 보호 기능을 끄지 않고 GIL Companion을 설치·업데이트하는 경로.
> 비범위: macOS 채널, Core/Monitor 기능, 실제 Store 제출·계정 개설(별도 승인 작업).

## 1. 문제

현재 Windows 배포물 `GIL.Companion.Unsigned.Test._0.1.1-owner.15_x64-setup.exe`는 Tauri 2 + NSIS
사용자 단위 installer이며 **Authenticode 서명이 없다**. 저장소 정책
([README](README.md), [RELEASE-READINESS](RELEASE-READINESS.md), 배포 모델 §6.2b)은 SmartScreen·Defender·
Smart App Control 해제, `Unblock-File`, 관리자 실행을 설치 방법으로 쓰지 못하게 한다. 그래서 OS가
막으면 지금은 **중단**이 유일한 답이고, 이것이 친구·테스터 설치가 막히는 직접 원인이다.

차단은 두 종류이며 대응이 다르다. 사용자가 본 화면으로 먼저 구분해야 한다.

| 보호 기능 | 화면 | 미서명 exe에 대한 동작 | 사용자 우회 |
|---|---|---|---|
| **Microsoft Defender SmartScreen** | "Windows의 PC 보호" 파란 창, `추가 정보` 링크 | 다운로드 파일(Mark-of-the-Web) + 알 수 없는 게시자 → 경고 | `추가 정보 → 실행` 가능 (정책상 안내하지 않음) |
| **Smart App Control (SAC)** | "이 앱은 Smart App Control에 의해 차단됨" | 미서명 또는 평판 없는 파일을 **무조건 차단** | 없음. SAC를 영구 해제해야만 하며 다시 켤 수 없음 |

SAC는 Windows 11 **클린 설치** PC에서 평가 모드로 켜져 있으므로 새 노트북 사용자일수록 두 번째
경우일 가능성이 높다. 해시 안내나 "추가 정보" 안내로는 해결할 수 없는 사용자군이 이미 존재한다.

## 2. 선택지 비교

| 경로 | SmartScreen | SAC | 비용·자격 | GIL 적합성 |
|---|---|---|---|---|
| **A. Microsoft Store (MSIX)** | 없음 — Store가 Microsoft 인증서로 서명 | 허용 | Partner Center 개인 계정 무료 또는 소액(과거 표 ₩21,216), 회사 ₩108,000 수준. 1회성 | **채택.** 비개발자 입구(Store 검색·링크), 배포 모델 §6이 이미 "Windows 정식 신뢰 채널 = Store 또는 서명 installer"로 명시 |
| B. Store "EXE/MSI 링크형" (Win32 app) | 서명 필요 | 서명 필요 | 신뢰 CA 코드서명 인증서 **필수** (정책 10.2.9, 자체 서명 불가, Store가 재서명하지 않음) | 인증서 없이는 불가 → A로 수렴 |
| C. Authenticode OV/EV 인증서 + 현 NSIS | 2024년부터 EV도 즉시 평판 없음. 파일 해시마다 평판을 새로 쌓음 | 서명+평판 필요, 보장 없음 | 연 수십만 원 + 하드웨어 토큰/클라우드 HSM. 조직 검증 또는 개인 검증 절차 | GitHub 직접 다운로드를 유지하려면 **후속 병행** 가치. 단독 해법은 아님 |
| D. Azure Trusted Signing (Artifact Signing) | C와 같음 (즉시 신뢰 없음) | C와 같음 | **개인은 미국·캐나다만**, 조직은 US/CA/EU/UK만 | 한국 개인 개발자 **자격 없음** |
| E. winget 커뮤니티 저장소 | installer를 그대로 실행 → 미서명 경고 유지 | 차단 유지 | 무료 | CLI 전용이라 주 사용자에 부적합. 서명 뒤 보조 채널 |
| F. 현상 유지 + "추가 정보 → 실행" 안내 | 경고 유지 | **불가** | 0 | 정책 위반이고 SAC 사용자를 영구 배제 |

**결론: A(Store MSIX)를 주 신뢰 채널로 채택한다.** C는 "GitHub 직접 다운로드도 서명판으로 유지"를
원할 때만 별도 예산으로 병행한다.

## 3. 제약 조건 (조사 결과)

Store 경로는 공짜 서명을 주지만 다음 네 가지를 설계에 반영해야 한다.

1. **Tauri 2는 MSIX 번들 타깃이 없다.** NSIS/MSI만 만든다. `makeappx.exe`(Windows SDK, GitHub
   `windows-2025` runner에 포함)로 패키지를 직접 만드는 스크립트가 필요하다. Store 제출 패키지는
   **서명하지 않은 채** 업로드하고 Microsoft가 서명한다.
2. **MSIX 컨테이너는 `%LOCALAPPDATA%`·`%APPDATA%` 쓰기를 가상화한다.** 패키지 앱과 **그 자식
   프로세스**는 `%LOCALAPPDATA%\GIL\runtime`나 `%APPDATA%\Claude\claude_desktop_config.json`에 쓰면
   실제 경로가 아니라 `Packages\<id>\LocalCache` 아래에 기록된다. Codex/Claude Desktop은 실제 경로를
   읽으므로 지금의 **Claude Desktop 연결** 버튼과 고정 런처 배치가 조용히 깨진다. 해제하려면
   제한 capability `unvirtualizedResources` + `FileSystemWriteVirtualization=disabled`를 선언하고
   심사 노트에 사유를 적어야 한다(Windows 11은 폴더 단위 지정도 지원).
3. **패키지 안의 프로세스가 살아 있으면 업데이트가 보류된다**(`0x80073D02`, 사용 중). MCP Core가
   패키지 안에서 돌면 Codex/Claude 세션이 열려 있는 동안 Store 업데이트가 적용되지 않는다.
4. **`runFullTrust`는 제한 capability다.** Win32 앱 패키징 사유를 심사 노트에 적으면 통상 승인되지만
   첫 제출은 추가 심사 단계가 있고, 계정에 따라 거절 사례가 있다.

그리고 저장소 자체 계약도 지켜야 한다: 자동 업데이트 미채택(사용자 버튼), 기존 연결은 기존 Core 유지·
새 연결부터 새 Core, 고정 런처 등록 불변, 제거 시 runtime·`.gil` 보존, Project 자동 이관 금지.

## 4. 추천 설계 — "Store가 Companion을 나르고, runtime은 패키지 밖에 산다"

현 NSIS 설계의 핵심 자산(고정 런처 + 불변 버전 폴더 + `current.json` 원자 교체 + Job Object +
직전판 폴백)은 그대로 두고, **배달 수단만 NSIS → Store MSIX로 바꾼다.**

```text
[Microsoft Store]  GIL Companion (MSIX, runFullTrust, unvirtualizedResources)
   └─ GIL Companion.exe
   └─ bootstrap\gil.exe, gil-launcher.exe, current.json   ← 지금 NSIS resources와 동일

Companion 첫 실행 / Store 업데이트 뒤 실행 (runtime_sync, Rust)
   └─ %LOCALAPPDATA%\GIL\runtime\                          ← 가상화 해제된 실제 경로
        ├─ gil-launcher.exe        (있으면 보존, 없을 때만 복사)
        ├─ versions\<ver>\gil.exe  (불변, 같은 버전 재복사 안 함)
        ├─ current.next → current.json 원자 교체, current.previous 보존
        └─ (runtime-hooks.nsh의 POSTINSTALL 논리를 Rust로 이식)

Codex / Claude Desktop  →  %LOCALAPPDATA%\GIL\runtime\gil-launcher.exe mcp --serve
   (패키지 밖 일반 프로세스. 컨테이너·가상화·"사용 중" 보류와 무관)
```

이 설계가 §3의 제약을 어떻게 피하는지:

- **가상화(2)**: Companion이 `unvirtualizedResources`로 실제 `%LOCALAPPDATA%\GIL`·`%APPDATA%\Claude`에
  쓴다. Core는 패키지 밖에서 Host가 직접 띄우므로 컨테이너와 무관하다.
- **사용 중 보류(3)**: 패키지 안에서 오래 사는 프로세스는 Companion 창뿐이다. Store 업데이트는 Companion
  이 닫혀 있을 때 적용되고, 실행 중인 MCP 세션은 영향을 받지 않는다.
- **사용자 주도 업데이트 계약**: Store가 Companion **패키지**를 자동으로 바꿔도, Core 선택(`current.json`
  교체)은 Companion이 실행되어 "새 Core <버전> 준비됨 — 적용" 버튼을 사용자가 눌렀을 때만 한다.
  "기존 연결은 기존 Core, 새 연결부터 새 Core"가 글자 그대로 유지된다.
- **제거 보존**: Store 제거는 패키지만 지운다. 가상화 해제 경로에 쓴 runtime은 남는다(현 NSIS 정책과 동일).
  심사 노트에 "실행 중 MCP 세션 보호를 위해 runtime을 보존하며 Companion 안에 **runtime 정리** 버튼을 둔다"
  를 적고, 그 버튼을 실제로 구현한다(정책 10.2 "깨끗한 제거" 대응).

### 채택하지 않은 대안: 순수 MSIX + App Execution Alias

Core까지 패키지 안에 두고 Host가 `%LOCALAPPDATA%\Microsoft\WindowsApps\gil-launcher.exe` 별칭을 등록하는
방식은 파일 복사가 없어 깔끔하지만, (3) 세션이 열려 있는 한 업데이트가 영원히 보류되고, (2) Core가
컨테이너 안에서 `.gil`을 쓰며, 버전별 불변 폴더·직전판 폴백 설계를 버려야 한다. Store 심사가 §4의
runtime 배치를 거절할 경우의 **2차 안**으로만 둔다.

## 5. 단계별 계획

| 단계 | 내용 | 완료 기준 | 담당 |
|---|---|---|---|
| **P0 진단** | 차단 화면이 SmartScreen인지 SAC인지 확인. 설정 > 앱 및 브라우저 컨트롤 > Smart App Control 상태, 화면 캡처 | 테스터별 차단 종류 표 | 사용자(테스터 문의) |
| **P1 계정** | Partner Center 개발자 계정(개인/회사 결정), 게시자 표시 이름, 앱 이름 "GIL Companion" 예약, Publisher ID(`CN=<GUID>`) 확보 | Publisher ID·Package Identity Name 기록 | 사용자 |
| **P2 패키징** | `companion/AppxManifest.store.xml` 템플릿(Identity, `runFullTrust`, `unvirtualizedResources`, 한/영 리소스, 로고 44/150/310/50px, x64), `companion/build-windows-msix.ps1`(cargo build → 레이아웃 → `makeappx pack`), 로컬 사이드로드용 자체서명 테스트 서명 분리 | `windows-product.yml`에 `msix-store-candidate` job, `product-install.test.mjs`에 manifest 계약 시험 추가 | 구현 |
| **P3 Companion** | `runtime-hooks.nsh` POSTINSTALL 논리를 Rust `runtime_sync`로 이식(멱등, 같은 버전 보존, 원자 교체). 패키지 identity 감지(`GetCurrentPackageFullName`)로 업데이트 UI를 Store 모드로 전환. "새 Core 적용" 버튼, "runtime 정리" 버튼 | 기존 13항목 설치 인수를 사이드로드 MSIX로 통과(windows-2025 runner에서 `Add-AppxPackage`) | 구현 |
| **P4 제출** | 1차는 **비공개 대상(Private audience) 또는 Store 숨김 + 직접 링크**로 친구 테스터에게. 심사 노트에 runFullTrust·unvirtualizedResources·runtime 보존 사유. 통과 뒤 공개 전환 | 테스터 1명 이상이 SAC 켜진 PC에서 Store 설치 → Codex/Claude 연결 → Monitor 확인 | 사용자+구현 |
| **P5 문서** | [INSTALL.md](../../INSTALL.md) Windows 절을 Store 링크로 교체, 배포 모델 §6.2b를 "Store 채널로 대체됨"으로 갱신, RELEASE-READINESS의 Authenticode 게이트를 "Store 서명 패키지 인수"로 재정의, `update-feed.json` windows-x64 항목에 Store 링크 | 문서 PR | 구현 |

P2·P3는 사용자 계정 없이도 자체서명 사이드로드로 끝까지 검증할 수 있다. P1과 병행 가능하다.
기존 unsigned NSIS owner 채널은 Store 통과 전까지 유지하고 Store 통과 뒤 "개발자 검수용"으로 격하한다.

## 6. 리스크와 미확인 사항

- **심사 거절 가능성**: 패키지 밖 `%LOCALAPPDATA%\GIL` 배치와 제거 후 보존이 정책 10.2(깨끗한 제거)로
  지적될 수 있다. 대응은 "runtime 정리" 버튼 + 심사 노트. 거절 시 §4의 2차 안(별칭 방식)으로 전환한다.
- **Partner Center 자격·비용**: 한국 개인 계정의 현재 수수료는 가입 화면에서 확인한다(무료 또는 소액으로
  안내됨). 회사 계정은 사업자 검증이 추가된다.
- **WebView2**: Store 패키지는 설치 시 부트스트래퍼를 실행할 수 없다. Windows 11은 WebView2 Runtime을
  내장하므로 x64 Windows 11 범위에서는 문제없고, Windows 10은 지원 범위 밖임을 명시한다.
- **Claude Desktop 자체가 MSIX**: Claude Desktop이 띄우는 `gil-launcher.exe`는 Claude의 컨테이너를 상속한다.
  이는 **지금도 같은 조건**이며 이번 변경으로 나빠지지 않지만, Claude 경로의 `.gil` 쓰기 위치는 P4 실기기
  검수 항목에 포함한다.
- **첫 심사 기간**: runFullTrust 첫 제출은 추가 질의가 붙을 수 있어 수일이 걸릴 수 있다.
- **Store 자동 업데이트**: 패키지 교체는 Store 설정(사용자 전역)을 따른다. Core 선택은 §4대로 버튼이 결정
  하므로 저장 형식 7 기록은 자동 변환되지 않는다.
- **이 문서는 설계이며 검증이 아니다.** 실제 MSIX 빌드·사이드로드·Store 심사·SAC PC 설치는 아직
  수행하지 않았다.

## 7. 사용자 결정 필요

1. Partner Center 계정을 **개인**으로 열지 **회사**로 열지.
2. Store 단독으로 갈지, GitHub 직접 다운로드도 **OV 인증서 서명판**으로 병행할지(연간 비용 발생).
3. 1차 Store 노출을 비공개 대상(친구)으로 할지 처음부터 공개로 할지.
4. P2·P3 구현을 지금 이 브랜치(`claude/windows-installer-optimization-99eeb7`)에서 시작할지.

## 출처

- Microsoft Store 정책 변경 이력(10.2.9 서명 요건): https://learn.microsoft.com/en-us/windows/apps/publish/store-policies-change-history
- Windows 앱 코드 서명 선택지(Store 무료 서명, Trusted Signing 지역 제한, OV 권고): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- SmartScreen 평판(EV 즉시 평판 폐지, 파일 해시 단위): https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation
- Azure Artifact Signing 빠른 시작(개인은 미국·캐나다): https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart
- Tauri Microsoft Store 안내(EXE/MSI만 생성): https://v2.tauri.app/ja/distribute/microsoft-store/
- MSIX 유연한 가상화(`unvirtualizedResources`): https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization
- `FileSystemWriteVirtualization` 요소: https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-virtualization-filesystemwritevirtualization
- MSIX 사용 중 서비싱(업데이트 보류): https://devblogs.microsoft.com/insidemsix/msix-servicing-while-in-use/
- Smart App Control 개요(미서명 차단): https://learn.microsoft.com/en-ie/windows/apps/develop/smart-app-control/overview
- Smart App Control 사용자 안내: https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003
- Partner Center 계정 유형·수수료: https://learn.microsoft.com/windows/uwp/publish/account-types-locations-and-fees
- runFullTrust 제한 capability 심사: https://learn.microsoft.com/en-us/answers/questions/1424817/the-following-restricted-capabilities-require-appr
- Claude Desktop MSIX 가상화로 자식 프로세스가 깨진 사례: https://github.com/anthropics/claude-code/issues/47977

## 8. 역할 분담과 비용·시간 추정 (2026-11-12 추가)

### 구현자(Agent)가 맡는 것

| 산출물 | 내용 | 검증 수단 |
|---|---|---|
| `companion/AppxManifest.store.xml` | Identity(이름·Publisher는 사용자 계정 값으로 치환), `runFullTrust`, `unvirtualizedResources`, `FileSystemWriteVirtualization=disabled`, ko/en, 로고 자산 | `product-install.test.mjs` manifest 계약 시험 |
| `companion/icons/` Store 자산 | 기존 `icon.png`에서 44/50/150/310px 로고 생성 | 스크립트 |
| `companion/build-windows-msix.ps1` | Core·런처·Companion 빌드 → 레이아웃 → `makeappx pack`. Store용 무서명 `.msix`와 사이드로드용 자체서명 `.msix` 분리 | CI |
| `windows-product.yml` `msix-store-candidate` job | windows-2025 runner에서 자체서명 사이드로드 → runtime 동기화·런처 MCP·Codex 등록·제거 보존 인수 → 무서명 `.msix` artifact | 기존 13항목 인수를 MSIX로 재실행 |
| Companion `runtime_sync` (Rust) | `runtime-hooks.nsh` POSTINSTALL 논리 이식: 버전 폴더 멱등 복사, receipt 원자 교체, 직전판 보존 | OS 공통 단위시험(임시 폴더) |
| Companion Store 모드 UI | 패키지 identity 감지, "새 Core 적용" 버튼, "runtime 정리" 버튼(실행 중 Core·등록 없음 확인 뒤에만), Store 링크 | Companion 단위시험 + CI 인수 |
| 버전 체계 | MSIX는 숫자 4자리(`1.0.N.0`, Store는 revision 0)만 허용. 패키지 버전과 Core 문자열 버전(`current.json`) 매핑 규칙 | 시험 |
| 문서 | INSTALL.md Windows 절, README/RELEASE-READINESS 게이트, 배포 모델 §6.2b, update-feed, 심사 노트 초안 | PR |

### 사용자가 맡는 것

| 일 | 이유 | 소요 |
|---|---|---|
| P0 테스터 차단 화면 확인 | SmartScreen/SAC/Defender 구분 | 테스터 답변 대기 |
| Partner Center 계정 개설(개인/회사 결정, Microsoft 계정 2단계 인증, 결제) | 본인 신원·결제 수단 필요 | 30~60분 + 신원 확인 대기 |
| 앱 이름 예약, Publisher ID·Package Identity Name 전달 | manifest 치환 값 | 10분 |
| Store 등록 정보 | 설명문 검토, 스크린샷(실제 Windows 화면), 개인정보 처리방침 URL(GitHub 페이지로 충분), 연령 등급 설문 | 1~2시간 |
| 제출 버튼·심사 노트 제출 | 계정 소유자만 가능 | 10분 + 심사 대기 |
| 실기기 검수 | SAC 켜진 PC 1대 이상에서 Store 설치 → 연결 → Monitor | 1시간 |

### 비용

| 항목 | 금액 | 주기 |
|---|---|---|
| Partner Center 개인 계정 | 무료 또는 약 ₩21,000 (가입 화면에서 확정) | 1회 |
| Partner Center 회사 계정 | 약 ₩108,000 + 사업자 검증 | 1회 |
| Store 등록·서명·배포 | 무료 | — |
| GitHub Actions windows runner | 현재와 동일(공개 저장소 무료, 비공개면 분당 과금 2배) | 빌드당 |
| (선택) OV 코드서명 인증서 + 토큰/HSM | 연 ₩30~70만 | 매년 |

### 시간

| 단계 | 추정 | 병목 |
|---|---|---|
| P2 패키징 + CI | 작업 세션 1~2회 | Windows CI 1회 30~60분, 반복 횟수 |
| P3 Companion 변경 + 인수 | 작업 세션 2~3회 | MSIX 사이드로드 인수 디버깅 |
| P5 문서 | 세션 0.5회 | — |
| 계정 개설·신원 확인 | 개인 수 시간~1일, 회사 수일 | Microsoft 검증 |
| 1차 심사 | 1~3영업일, runFullTrust 첫 제출은 추가 질의 가능 | Microsoft |
| 거절 시 재제출 | 회당 1~3영업일 | — |

낙관 2주, 보통 3~4주(심사 질의·거절 1회 포함). P2/P3는 계정 없이 병행 가능하므로 사용자 작업은
계정 개설을 먼저 시작하는 것이 전체 기간을 줄인다.

## 9. 변형 B — Tauri Companion 없이 Store 패키지 (2026-11-12 검토)

사용자 전제: Codex와 Claude Desktop(Claude Code 탭)의 fullscreen Monitor가 확인되어 Windows에서
Companion 창의 필요가 사라졌다. 이 전제에서는 §4의 하이브리드보다 다음이 더 단순하다.

```text
[Microsoft Store]  GIL (MSIX, runFullTrust, unvirtualizedResources)
   └─ gil.exe  + App Execution Alias "gil.exe"  (%LOCALAPPDATA%\Microsoft\WindowsApps에 자동 등록)
   └─ 시작 메뉴 "GIL 연결" → gil.exe connect (콘솔 마법사)

Codex:           codex mcp add gil -- gil mcp --serve        (gil connect codex가 대신 실행)
Claude Desktop:  mcpServers.gil = { command: "gil", args: ["mcp","--serve"] }  (gil connect claude가 백업 뒤 기록)
업데이트:        Store. GIL 프로세스가 모두 종료된 뒤 적용(사용 중이면 보류)
```

제거되는 것: Tauri crate·WebView2·tauri-cli 핀, Tauri updater와 `TAURI_SIGNING_*`/`GIL_UPDATER_*` secrets,
NSIS와 `runtime-hooks.nsh`, **고정 런처·버전 폴더·`current.json`·직전판 폴백·Job Object 전체**
(`src/bin/gil-launcher.rs`). 불변 버전·서명 검증·원자 교체·제거는 MSIX가 OS 기능으로 제공한다.
Python Store 패키지가 같은 "콘솔 앱 + alias" 형태의 전례다.

옮겨 오는 것: `companion/src/claude.rs`·`clients.rs`의 연결·해제·이전 GIL 교체 논리(Tauri 무관한 Rust)를
`gil` crate의 `gil connect|disconnect <host>` 서브커맨드로 이동. `%APPDATA%\Claude` 실제 쓰기 때문에
`unvirtualizedResources`는 여전히 필요하다. 선택: Claude Desktop용 `.mcpb`(외부 명령 허용 여부 미확인).

계약 변경(사용자 결정): "기존 연결은 기존 Core, 새 연결부터 새 Core" → "GIL이 실행 중이 아닐 때 Store가 교체".
Monitor의 업데이트 안내 문구는 "Host를 재시작하면 적용"으로 바뀐다. Store 자동 업데이트를 앱 단위로 끌 수 없다.

| 비교 | A. Tauri 유지(§4) | B. Tauri 제외 |
|---|---|---|
| 패키지 | Companion + Core + 런처, WebView2 의존 | gil.exe 하나 |
| 빌드/CI | cargo tauri + makeappx, tauri-cli 설치 5~10분 | cargo build + makeappx |
| 유지 코드(Windows 제품 경로) | Companion ~6,700줄 + 런처 355줄 + NSIS 59줄 | connect 논리 ~1,300줄 이동 |
| 심사 리스크 | 패키지 밖 runtime 보존 지적 가능 | 낮음(모두 패키지 안) |
| 연결 UX | GUI 버튼 | 시작 메뉴 → 콘솔 마법사(Enter/y) 또는 `.mcpb` |
| 업데이트 | Companion 버튼이 Core 선택 | Store, Host 재시작 뒤 적용 |
| 구현 추정 | 세션 4~6회 | 세션 3~5회(이동·정리 포함) |

B의 검증 항목: MSIX인 Claude Desktop이 다른 패키지의 alias를 자식으로 띄우는 조합의 실기기 확인,
사용 중 보류 뒤 재시작 시 정상 교체, 콘솔 마법사의 비개발자 가독성.
Companion·런처 코드는 저장소에 남기고(macOS·향후 Companion용, 배포 모델 §4 조항) Windows 제품 경로에서만 제외한다.
