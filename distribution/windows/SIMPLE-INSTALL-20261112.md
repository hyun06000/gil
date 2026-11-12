# Windows 배포 시나리오 v2 — 결정서 적용판 (2026-11-12)

> 기준 문서: 소유자의 **「MCP 서버 배포 설계 결정서」(2026-11-12)**. 이 문서는 그 결정서를 GIL 저장소의
> 실제 코드·배포 상태에 대응시킨 것이다. 결정서와 어긋나는 곳은 §12 "열린 질문"에 모아 소유자 확인을 기다린다.
> 상태: **구현·게시됨 (v0.3.0). SAC 켜진 PC는 차단 확인(2026-11-13) → Microsoft Store 채널을 [STORE.md](STORE.md)로 병행한다.** `src/install/`, `install.ps1`, 루트 `INSTALL.md`, `windows-release.yml`. 실기기 검증·첫 태그 게시는 남았다. [TRUST-STRATEGY-20261112.md](TRUST-STRATEGY-20261112.md)(서명·Store)는
> 결정서 §3에 따라 **보류**한다.

## 0. 플레이스홀더 대응과 전제 확인

| 결정서 | GIL |
|---|---|
| `my-server` | `gil` (MCP 서버 이름 `gil`, 실행 파일 `gil.exe`) |
| `<OWNER>/<REPO>` | `hyun06000/gil` |
| 전송 방식 stdio | **맞음.** `rmcp` `transport-io`, 현재 명령 `gil mcp --serve` |
| 사용자별 설정(API 키 등) | **없음.** 프로젝트 루트는 대화마다 Host가 넘기고, 언어는 자동 결정 |
| Rust 단일 exe | **맞음.** Core·MCP·내장 Monitor UI가 `gil.exe` 하나. 기능 프로필 `approved-plan,visual-authoring,product-safety` |
| 대상 OS | Windows 10/11 x64 (결정서). 현재 문서는 11만 표기 → 10은 미검증으로 표기해 확장 |
| 대상 Host | Claude Desktop(Claude Code 탭)과 Codex(CLI·IDE·ChatGPT 앱, 같은 `config.toml`) |

## 1. 중단할 것과 현재 상태 보고 (결정서 §3·§12-1)

| 항목 | 저장소의 현재 상태 | 조치 |
|---|---|---|
| 코드 서명 인증서·클라우드 서명 | **구매·가입 없음.** CI에 서명 secrets 이름만 정의돼 있고 한 번도 실행된 적 없음(RELEASE-READINESS "NO-GO") | `windows-product.yml`의 서명 후보 job 제거 |
| Microsoft Store·MSIX | 전략 문서만 작성, 계정·패키지 없음 | TRUST-STRATEGY를 보류로 표시 |
| NSIS 설치 프로그램 | `0.1.1-owner.15` unsigned NSIS가 게시돼 있음(현재 사용자 채널) | 새 경로 게시 뒤 "구형 채널"로 격하. 기존 설치는 §9로 이행 |
| Tauri Companion·고정 런처·updater | 코드·CI 존재 | Windows 제품 경로에서 분리. 코드는 저장소에 보존(macOS·향후) |
| winget·npm·Registry·디렉터리 제출 | 없음 | 하지 않음 |

결제·계정 생성은 이번 세션 범위에서 **없었다**. GitHub Environment `windows-product-signing`에 실제 secret이
들어 있는지는 소유자가 확인한다(§12 Q5).

## 2. 릴리스 구성 (결정서 §9)

| 자산 | 이름(버전 없음) | 비고 |
|---|---|---|
| 실행 파일 | `gil-x86_64-pc-windows-msvc.exe` | 정적 CRT: `RUSTFLAGS=-C target-feature=+crt-static` (현재 CI에 없음 → 추가) |
| 체크섬 | `checksums.txt` | 자산별 SHA-256 |
| 보조 | `gil.mcpb` | 선택(결정서 §10). `mcpb sign` 없음 |

- 주소는 `https://github.com/hyun06000/gil/releases/latest/download/<자산>`. **`latest`는 정식 release만 가리키므로**
  Windows 배포는 정식 release로, `runtime-verify-*`·`windows-dev-visual-*` 같은 검수 자산은 prerelease로 유지한다.
- 버전은 `GIL_BUILD_VERSION`으로 스탬프한다(`gil --version` → `gil <ver>`). 체계는 §12 Q4.
- 태그 push → GitHub Actions가 빌드·`checksums.txt`·업로드. `chain-core.yml`이 이미 `x86_64-pc-windows-msvc`를
  빌드·시험하므로 release job만 더한다. 서명 단계 없음.
- ARM64는 별도 빌드 없이 x64 에뮬레이션(결정서). 현재 "ARM64 미지원" 문구를 "에뮬레이션, 미검증"으로 바꾼다.

## 3. 고정 경로와 설치 상태 (결정서 §5·§6)

```text
%LOCALAPPDATA%\Programs\gil\
  gil.exe                 ← Host가 등록하는 고정 경로
  gil.exe.old-<ver>       ← update 직전 판. 실행 중인 세션이 끝까지 사용. 다음 update 때 정리
  install.json            ← {version, sha256, installed_at}. Monitor 업데이트 안내가 설치 버전을 읽는 자리
```

`src/mcp/update.rs`의 `installed_version`은 지금 `versions/<v>/` 런타임과 Plugin `plugin.json`만 안다.
`install.json` 레이아웃을 세 번째 출처로 추가해야 Monitor의 "새 버전 X 있음" 안내가 이 설치본에서도 동작한다.
기존 `%LOCALAPPDATA%\GIL\runtime`은 건드리지 않는다(§9).

## 4. 서브커맨드 명세 (결정서 §6)

| 명령 | GIL 대응 | 상태 |
|---|---|---|
| `serve` | `gil mcp --serve`의 별칭으로 추가. 등록 args는 §12 Q2 | 추가 |
| `install` | 신규. Companion `claude.rs`·`clients.rs`의 백업·원자 쓰기·동시 변경 검사·"GIL이 아닌 `gil` 보존" 논리를 이식 | 추가 |
| `doctor` | 신규. 결정서 출력 형식·종료 코드 그대로 | 추가 |
| `uninstall` | 신규. exe 삭제는 `--remove-binary` 옵션 | 추가 |
| `update` | 후순위. `checksums.txt` 검증 → 이름 바꾸기 교체 | 후순위 |
| `--version` | 있음 (`gil 0.1.1-owner.15` 형식) | 유지 |
| 인자 없음 | 현재는 프로젝트 안내 `help_here`. 결정서대로 **사용법만 출력**하고 서버를 띄우지 않음 — 지금도 서버는 띄우지 않으므로 문구만 정리 | 확인 |

**`install` 순서 — 결정서 §6에 GIL 규칙을 더한 것**

1. 자기 경로가 고정 경로가 아니면 고정 경로로 복사(실행 중 exe는 이름 바꾸기 규칙). 이후 등록은 고정 경로.
2. Host 탐지(§5). 탐지되지 않으면 `NOT_INSTALLED`, 오류 아님.
3. 설정 파일마다: `.bak-<타임스탬프>` 백업(최근 5개 보관) → 파싱 실패 시 중단·보고 → 자기 항목만 추가/갱신 →
   임시 파일 후 이름 바꾸기. 다른 서버·다른 키·주석 보존.
4. **`gil` 항목이 이미 있을 때**: command가 GIL 실행 파일(새 고정 경로, 구 런처 `GIL\runtime\gil-launcher.exe`,
   구 Plugin cache의 `gil.exe`)이면 갱신. GIL이 아닌 `gil`이면 바꾸지 않고 `STALE`로 보고하며 `--replace`로만 교체.
5. **Claude Desktop이 실행 중이면** 파일을 쓰지 않는다(실행 중 Desktop이 바깥 편집을 되돌림, Mac 2026-10-31 관측).
   기본 동작은 분리 프로세스 `gil install --after-desktop-exit`를 띄워 `Claude.exe` 종료를 최대 10분 기다린 뒤
   쓰고, 결과를 `install.json` 옆 `install-log.txt`에 남기는 것이다. `--no-wait`면 안내만 출력한다. 도우미는 Desktop을
   다시 열지 않는다(결정서 §5: 마지막 재시작은 사용자가 한다). 사용자 동작은 **Desktop 종료 → 자동 등록 → Desktop 다시 열기**다.
   Windows MSIX Desktop도 같은 되돌림이 있는지는 **미확인**(§10).
6. Host별 결과와 재시작 안내 출력. 멱등: 같은 항목이면 파일을 바꾸지 않는다.

## 5. Host 설정 파일 명세 (결정서 §7)

### 5.1 Claude Desktop

- 탐지: `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\`(MSIX, 패키지 ID 하드코딩 금지)와
  `%APPDATA%\Claude\` **둘 다** 대상. 둘 다 없으면 `NOT_INSTALLED`. 파일이 없거나 0바이트면 새로 만든다.
- 내용: `mcpServers.gil = { "command": "<고정 경로>", "args": ["mcp", "--serve"] }`. 다른 최상위 키·다른 서버 보존.
  **구현 메모(2026-11-12):** `serde_json`의 `preserve_order`는 Core가 내보내는 모든 JSON(Monitor wire fixture가 바이트
  비교)의 키 순서를 바꾸므로 켜지 않았다. 키는 정렬되어 저장되며 값은 모두 보존된다. Claude Desktop도 자기 순서로 다시 쓴다.
- **현재 GIL 코드와의 차이**: Companion은 `%APPDATA%\Claude`만 썼고 Windows 실기기 확인이 없었다("사용자 확인 대기").
  결정서의 두 경로 규칙을 채택한다. 이슈 #29100(MSIX가 `mcpServers` 무시)은 **미확인** → 재현되면 `.mcpb`를
  Claude용 대체 경로로 올린다.

### 5.2 Codex

- 파일: `CODEX_HOME\config.toml`, 없으면 `%USERPROFILE%\.codex\config.toml`. 폴더가 있으면 설치된 것. 파일 없으면 생성.
- 내용: `[mcp_servers.gil]`, `command = '<고정 경로>'`(작은따옴표), `args = [...]`. `toml_edit`로 서식·주석 보존.
- **현재 GIL 정책과의 차이**: 저장소는 "공식 `codex mcp add`만 쓰고 `config.toml`을 손으로 고치지 않는다"였다.
  결정서는 CLI 없는 ChatGPT 앱 사용자를 위해 **직접 편집**을 결정했다. 채택하되 §12 Q3로 확인한다.
  `toml_edit`는 새 의존성이다(`Cargo.lock` 갱신, `--locked` CI).

### 5.3 공통

공백·한글 사용자 이름 경로, UTF-8(BOM 허용), 경로 API 사용. 결정서 §7.3 그대로.

## 6. 진입점 (결정서 §5·§8)

**A. 에이전트** — 사용자가 한 문장을 말한다.

```text
https://raw.githubusercontent.com/hyun06000/gil/main/INSTALL.md 를 읽고 설치해줘
```

**B. 사람** — PowerShell 한 줄.

```powershell
irm https://raw.githubusercontent.com/hyun06000/gil/main/install.ps1 | iex
```

둘 다 같은 다섯 단계: 폴더 생성 → exe 다운로드(`latest` 고정 URL) → `checksums.txt` 대조(다르면 삭제·중단)
→ `gil.exe install` → `gil.exe doctor`. 완료 조건은 `doctor` 종료 코드 0.

**GIL 저장소와의 충돌**: 지금 루트 `INSTALL.md`는 Mac·Windows·훅·버전 확인을 모두 담은 긴 통합 안내다.
결정서는 루트 `INSTALL.md`를 **짧은 에이전트 절차서**로 요구하고 "설치와 무관한 지시"를 금한다.
제안: 루트 `INSTALL.md`를 결정서 템플릿(Windows 절 + macOS 절)으로 교체하고, 현재 통합 안내는
`distribution/INSTALL-GUIDE.md`로 옮겨 링크한다. **§12 Q1.**

`install.ps1`은 템플릿의 1~5단계를 그대로 실행한다. 원격 스크립트를 통째로 파이프하는 명령은 `INSTALL.md`에
넣지 않는다(에이전트는 단계를 읽고 승인받는다).

## 7. 설치 후·Monitor·업데이트

- 사용자는 Claude Desktop을 **트레이에서 완전 종료** 후 다시 열고, Codex를 재시작한다. 재시작 전에는 도구가 안 보인다.
- Monitor는 Host fullscreen(MCP App)이 담당한다. 변경 없음. Claude Code 탭의 fullscreen 클릭 요구(PR #107)도 그대로.
- `distribution/update-feed.json`의 `windows-x64` 항목을 새 채널로 바꾼다: `install_doc`은 raw `INSTALL.md`,
  안내 문구는 "한 줄 설치를 다시 실행하거나 에이전트에게 요청". 자동 업데이트 없음(현 정책 유지).
- `gil update`(후순위)는 `checksums.txt` 검증 뒤 §3의 이름 바꾸기로 교체. 경로 고정이라 재등록 없음.
  **기존 세션은 옛 파일을 계속 쓰고 새 연결부터 새 판** — 현재 런처 계약과 같은 결과를 런처 없이 얻는다.

## 8. 저장소 정책 조정

- 배포 모델 §6.2b·`distribution/windows/README.md`·INSTALL.md의 "PowerShell 스크립트 안내 금지", "`config.toml` 손편집 금지"를
  결정서 기준으로 고친다. 금지의 본뜻(보안 해제·실행 정책 변경·cache 손편집·관리자 권한 금지)은 유지한다.
- RELEASE-READINESS의 Authenticode·SmartScreen 게이트를 "결정서 §3 재검토 조건"으로 대체한다.
- `windows-product.yml`(NSIS·서명 후보)과 Companion Windows 빌드 스크립트를 제품 경로에서 제거하거나 비활성화한다.

## 9. 기존 owner.15 사용자 이행

새 `install`은 `gil` 항목의 command가 `%LOCALAPPDATA%\GIL\runtime\gil-launcher.exe`이면 GIL로 인식해 새 고정
경로로 갱신한다(§4-4). 열린 대화는 옛 런처·Core를 끝까지 쓰고 새 연결부터 새 exe를 쓴다. 옛 runtime·Companion은
지우지 않으며, Companion은 사용자가 Windows 앱 제거로 지울 수 있다. Project·`.gil` 기록은 변경하지 않는다.

## 10. 위험과 테스트 (결정서 §11 + GIL 추가)

| 위험 | 상태 | 대응 |
|---|---|---|
| 미서명 exe를 SmartScreen·Defender·**Smart App Control**이 차단 | **SAC 차단 확인됨 (2026-11-13 소유자 PC, [장부](PUBLICATION-V0.3.0-20261113.md))**. SmartScreen·Defender는 미관측 | 깨끗한 Windows 11에서 두 진입점 테스트. SmartScreen은 더블클릭 경로가 아니므로 개입하지 않을 것으로 예상. SAC는 Microsoft 문서상 미서명 프로세스 생성을 막으므로 **SAC 켜진 PC는 차단될 가능성이 높다** → 확인되면 결정서 §3 재검토 조건 보고 |
| Claude Desktop MSIX가 `mcpServers` 무시(#29100) | 미확인 | 실기기 확인. 재현 시 `.mcpb` 대체 |
| Windows Desktop이 실행 중 외부 편집을 되돌림 | 미확인(Mac에서 관측) | §4-5 대기 도우미로 회피. 되돌리지 않으면 `--no-wait` 기본으로 완화 |
| Claude Desktop(MSIX) 자식 프로세스의 `.gil` 쓰기 위치 | 현 상태와 동일 | 테스트 폴더에서 `.gil` 실제 위치 확인 |
| Host 업데이트로 경로·형식 변경 | 상존 | `doctor` 감지 → `install` 수정 → 새 release |

체크리스트는 결정서 §11의 13항목에 다음을 더한다: owner.15 설치 PC에서 이행 후 Codex·Claude 새 대화 정상,
Monitor fullscreen과 "새 버전 있음" 안내가 새 레이아웃에서 표시, SAC 켜진 PC 1대 결과.

## 11. 작업 순서와 담당 (결정서 §12)

| # | 작업 | 담당 | 추정 |
|---|---|---|---|
| 1 | 중단·보고(이 문서 §1) | 완료 | — |
| 2 | 열린 질문 답(§12) | 소유자 | — |
| 3 | `serve` 별칭·인자 없음 사용법 정리 | 구현 | 0.3 세션 |
| 4 | `install` — Codex TOML 병합 → Claude JSON 병합(두 경로) → 대기 도우미 | 구현 | 1.5 세션 |
| 5 | 조기 검증: SmartScreen/SAC, #29100, Desktop 되돌림 | 소유자 실기기 | 1시간 |
| 6 | `doctor`·`uninstall` | 구현 | 0.5 세션 |
| 7 | release 자동화(정적 CRT·checksums·latest), `windows-product.yml` 정리 | 구현 | 0.5 세션 |
| 8 | `INSTALL.md`·`install.ps1`·README·정책 문구·update-feed | 구현 | 0.5 세션 |
| 9 | 전체 테스트(§10 체크리스트) | 소유자 실기기 + CI | 1~2시간 |
| 10 | `.mcpb`·`update` | 구현(필요 시) | 0.5 세션 |

구현 세션 3~4회, 비용 0원, 계정 없음.

## 12. 열린 질문 (소유자 확인)

- **Q1 루트 `INSTALL.md`**: 결정서 템플릿의 짧은 절차서로 교체하고 현재 통합 안내를 `distribution/INSTALL-GUIDE.md`로
  옮기는 안에 동의하는가? (대안: 루트는 그대로 두고 `INSTALL-WINDOWS.md`를 따로 두되 결정서의 raw 주소와 달라짐)
- **Q2 MCP 명령 이름**: 등록 args를 지금 계약 `["mcp","--serve"]`로 유지하고 `serve`는 별칭만 둘지, 결정서대로
  `["serve"]`로 바꿀지. 추천은 **유지 + 별칭** (Mac marketplace manifest·기존 등록과 동일, 변경 범위 최소).
- **Q3 Codex 등록 방식**: 결정서대로 `config.toml` 직접 편집(`toml_edit` 의존성 추가, 저장소 정책 변경)으로 확정하는가?
- **Q4 버전·태그 체계**: Windows 채널 `0.1.1-owner.N`을 계속 쓸지, Mac `0.2.1-preview.N`과 합류할지. `latest`를 쓰므로
  Windows release는 정식 release로 표시해야 한다.
- **Q5 결제·계정**: GitHub Environment `windows-product-signing`에 실제 인증서·키가 들어 있는가? 이 세션에서 아는 한 없다.
- **Q6 시작 시점**: Q1~Q4 답을 받은 뒤 이 브랜치(`claude/windows-installer-optimization-99eeb7`)에서 3번부터 시작해도 되는가?
