# Microsoft Store 채널 — 소유자 절차 (2026-11-19)

Smart App Control(SAC)이 켜진 PC는 미서명 `gil.exe`를 실행 단계에서 막는다([v0.3.0 장부](PUBLICATION-V0.3.0-20261113.md)).
Store 심사를 통과한 MSIX는 Microsoft가 서명한다. 일반 사용자 설치·실행과 SAC 환경은 Store 배포본으로 검증한다. 같은 `gil.exe`를 그대로 싸며, 설치 프로그램·
Companion·별도 런처는 없다([전략 §9 변형 B](TRUST-STRATEGY-20261112.md)).

```text
[Microsoft Store]  GIL (MSIX, runFullTrust + unvirtualizedResources)
   └─ gil.exe + App Execution Alias  →  %LOCALAPPDATA%\Microsoft\WindowsApps\gil.exe
사용자:  Store에서 설치 → 터미널에서 `gil install` (또는 에이전트에게 요청) → Host 재시작
Host 등록: command = %LOCALAPPDATA%\Microsoft\WindowsApps\gil.exe, args = ["mcp", "--serve"]
업데이트: Store 자동/수동. GIL 프로세스가 모두 종료된 뒤 교체된다. 재등록 없음.
```

CLI에는 별도 GUI가 필요하지 않으며, 시작 메뉴 항목은 숨기고 터미널 실행 별칭을 제공한다.

## 저장소가 이미 하는 것 (구현 완료)

| 항목 | 위치 |
|---|---|
| manifest 템플릿(capability 사유 주석 포함) | [msix/AppxManifest.xml](msix/AppxManifest.xml) |
| 타일·Store 로고 | `msix/Assets/` |
| 패키징 스크립트 (`makeappx`, 선택 서명) | [msix/build-msix.ps1](msix/build-msix.ps1) |
| 릴리스 워크플로: 자체서명 사이드로드 검증(등록된 manifest·도움말/무인자 종료·MCP/Monitor 리소스·`gil install`·`doctor [store]`) + **무서명 Store 업로드 아티팩트** | `.github/workflows/windows-release.yml` |
| 패키지 모드의 `gil install`/`doctor` (별칭 경로 등록, 복사 없음) | `src/install/` |
| Monitor의 Store 채널 업데이트 안내 | `ui/i18n/*.js` `upd.store_*` |
| 계약 시험 | [msix.test.mjs](msix.test.mjs) |

## 소유자가 해야 하는 것

1. **Partner Center 개발자 계정** — https://partner.microsoft.com/dashboard 에서 개인 계정으로 등록한다
   (무료 또는 소액 1회, 가입 화면에서 확정). 결제·신원 확인은 소유자만 할 수 있다.
2. **앱 이름 예약** — Apps and games → New product → MSIX or PWA app. `gil`·`GIL`은 예약할 수 없었고 **`gil-companion`**으로
   예약했다(2026-11-13). manifest의 `DisplayName` 두 곳은 이 예약 이름과 같아야 한다.
3. **Product identity 값 복사** — Product management → Product identity 의 세 값:
   `Package/Identity/Name`, `Package/Identity/Publisher`(`CN=…` 형식), `Package/Properties/PublisherDisplayName`.
4. **GitHub repository variables 설정** — Settings → Secrets and variables → Actions → Variables:
   `GIL_MSIX_IDENTITY_NAME`, `GIL_MSIX_PUBLISHER`, `GIL_MSIX_PUBLISHER_DISPLAY_NAME`. 비밀이 아니므로 variables다.
5. **태그 푸시** — 다음 릴리스(`vX.Y.Z`)의 워크플로가 Store 업로드 패키지를 아티팩트
   `gil-msix-store-upload-<버전>`으로 남긴다. 변수 없이 돌면 dev identity로 만들어져 업로드할 수 없다고 README에 적힌다.
6. **제출** — Partner Center 제출에서 아티팩트의 `gil-store-upload.msix` 업로드. 서명하지 않는다(Store가 서명).
   - 가시성: 처음은 **Private audience**(테스터 이메일) 또는 **Hidden**(직접 링크)로 두고, 통과 뒤 공개로 바꾼다.
   - 연령 등급 설문, 스크린샷 1장 이상(Monitor 화면), 설명문.
   - **개인정보 처리방침 URL(필수)**: `https://github.com/hyun06000/gil/blob/main/PRIVACY.md`. GIL이 데이터를 수집하지 않아도
     Partner Center는 `runFullTrust`처럼 개인정보에 접근할 **수 있는** capability를 선언한 패키지에 방침 URL을 요구한다
     (정책 10.5.1). 방침은 수집하지 않는다는 사실과 유일한 네트워크 접근(하루 1회 공개 JSON 읽기)을 적는다.
   - **인증 노트**: CLI 실행 절차, 권한 사유와 실제 검증 증거를 적는다.
7. **심사 결과 반영** — 통과하면 Store 링크를 `INSTALL.md`·README·`update-feed.json`에 추가하고 게시 장부를 쓴다.
   거절되면 사유를 장부에 적고 보고한다. 거절 사유가 "패키지 밖 쓰기"면 `unvirtualizedResources` 사유를 보강한다.

## 인증 노트와 재제출

Notes for certification에는 사용 절차와 권한 사유, 실제 Windows에서 확인한 증거만 적는다. `AppListEntry="none"` 변경을 충돌 원인 해결의 증거로 쓰지 않는다.

## 알려진 미확인 사항 (첫 심사·실기기에서 확인)

- Claude Desktop(MSIX)이 다른 패키지의 실행 별칭을 자식 프로세스로 띄우는 조합.
- Store 업데이트가 "사용 중"으로 보류되는 동작과 Monitor 안내의 재시작 지시가 충분한지.
- `runFullTrust` 첫 제출의 추가 질의 기간, `unvirtualizedResources` 승인 여부.
- Windows 10 1903(build 18362) 이상이 manifest의 기술적 최소값이다. 실제 최소 버전 장치 검증은 별도이며, OS 지원 수명을 보증하지 않는다.
- Windows ARM64는 검증하지 않았으며 이번 재제출의 지원 완료 범위로 주장하지 않는다.

미서명 단일 실행 파일 채널(루트 INSTALL.md)은 SAC가 꺼진 PC용으로 그대로 유지한다. 두 채널은 같은 `gil.exe`다.
