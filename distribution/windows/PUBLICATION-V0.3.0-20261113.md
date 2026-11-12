# Windows v0.3.0 게시 장부 — 2026-11-13

첫 **단일 실행 파일** Windows 릴리스다. 설치 프로그램·Authenticode 서명·Store·Companion 없이 정적 링크한
`gil.exe` 하나를 GitHub Releases에 올렸다(2026-11-12 소유자 결정서, [SIMPLE-INSTALL](SIMPLE-INSTALL-20261112.md)).
설치는 루트 [INSTALL.md](../../INSTALL.md)(에이전트)와 `install.ps1`(사람)을 따른다.

## 배포물

| 항목 | 값 |
|---|---|
| Release | [v0.3.0](https://github.com/hyun06000/gil/releases/tag/v0.3.0) — 정식 release, `latest` |
| Tag/source commit | `v0.3.0` → `9f1e526` (PR #113 브랜치, 병합 commit `8c7ab86`) |
| Workflow run | [43240833076](https://github.com/hyun06000/gil/actions/runs/43240833076) (`windows-release.yml`) |
| `gil-x86_64-pc-windows-msvc.exe` | 11,857,920 bytes, SHA-256 `6120b15367f11d757e449de6a134f0cdbf08b19d41153f036363cc0afd5417bd` |
| `checksums.txt` | 97 bytes |
| Core 버전 | `gil 0.3.0` (`GIL_BUILD_VERSION`), 기능 프로필 `approved-plan,visual-authoring,product-safety`, `+crt-static` |
| 고정 핀 | [current-release.json](current-release.json) — `update-feed.json`과 일치해야 한다 |

## 검증

- 빌드 runner(windows-2025, Host 없음)에서 `gil --version` = `gil 0.3.0`, `gil install --no-wait`가
  `%LOCALAPPDATA%\Programs\gil\gil.exe`를 배치하고, 설치된 복사본의 `gil doctor`가 `binary: OK`,
  `claude-desktop: NOT_INSTALLED`, `codex: NOT_INSTALLED`로 종료 0.
- 게시 뒤 `releases/latest/download/` 주소로 다시 받은 exe의 SHA-256이 `checksums.txt`와 일치(macOS에서 다운로드 검증).
- PR #113 CI 13항목 성공(Rust 1,616 통과, JS 197, 패키징 105, Windows 배포 37). 동일 commit의 소스.
- 첫 두 태그 실행은 실패했다: ① 워크플로 YAML의 here-string 들여쓰기 오류(파일 자체가 무효), ② 자기 검증이
  고정 경로가 아닌 빌드 복사본에서 `doctor`를 실행해 `STALE`. 둘 다 PR #113에 수정 commit을 더하고 태그를
  옮겨 세 번째 실행에서 게시했다. 중간 실행은 release를 만들지 않았다.

## 사용자 안내와 이행

- 신규: PowerShell 한 줄 `irm https://raw.githubusercontent.com/hyun06000/gil/main/install.ps1 | iex`
  또는 에이전트에게 `https://raw.githubusercontent.com/hyun06000/gil/main/INSTALL.md 를 읽고 설치해줘`.
- 기존 `0.1.1-owner.15`(NSIS Companion) 사용자: Companion의 업데이트 버튼은 이 판을 받지 못한다. 같은 한 줄을
  실행하면 `gil install`이 `gil` 등록(구 런처 경로)을 새 고정 경로로 갱신한다. Companion·구 runtime·`.gil` 기록은
  지우지 않으며 Companion은 Windows 앱 제거로 지울 수 있다. 열린 대화는 구 Core를 끝까지 쓰고 새 연결부터 새 판이다.
- Monitor 업데이트 안내: `update-feed.json`의 `windows-x64`가 이 판을 가리키며 안내문에 위 한 줄을 담는다.
  이 판의 내장 Monitor는 Windows 안내를 Companion 버튼 대신 한 줄 설치 절차로 보여 준다(구 판의 내장 UI는
  Companion 절차를 보여 주되 안내문의 한 줄로 이행할 수 있다).

## 검수 범위와 한계

- **실기기 미검수.** 실제 Windows PC의 한 줄 설치, Claude Desktop(MSIX)의 `mcpServers` 인식, 실행 중 Desktop의
  설정 되돌림, Smart App Control이 켜진 PC의 차단 여부는 사용자 필드테스트 대기다. SAC 차단이 확인되면
  결정서 §3의 서명 재검토 조건이다.
- Windows 10, ARM64(x64 에뮬레이션)는 미검증이며 지원 범위 표기만 한다.
- 서명 없음. SHA-256은 바이트 무결성 확인이지 제작자 인증이 아니다. OS가 막으면 보안 설정을 바꾸지 않고 중단한다.
- `.mcpb` 보조 배포물과 `gil update` 서브커맨드는 후순위로 남는다.

## 실기기 결과 — 2026-11-13, 소유자 PC (Smart App Control 켜짐)

에이전트 경로(Codex, 루트 INSTALL.md)로 시험했다. 기존에 구형 Companion `0.1.1-owner.4`가 설치된 PC다.

| 단계 | 결과 |
|---|---|
| INSTALL.md 읽기·승인·폴더 생성 | 정상 |
| `gil-x86_64-pc-windows-msvc.exe`·`checksums.txt` 다운로드 | 정상 |
| SHA-256 대조 | `6120b153…5417bd` 일치 |
| `gil-new.exe install` 실행 | **차단.** Windows: "애플리케이션 제어 정책에서 이 파일을 차단했습니다". `Get-AuthenticodeSignature` = `NotSigned` |
| `doctor`·Host 등록 | 실행되지 못함. 설정 파일 변경 없음 |
| 에이전트 동작 | 우회·보안 설정 변경 없이 중단, 임시 파일 정리, 원인 보고 — 지침대로 |

판정: **Smart App Control이 켜진 PC에서는 미서명 `gil.exe`가 프로세스 생성 단계에서 차단된다.** 브라우저 다운로드나
SmartScreen과 무관하며, Host가 자식 프로세스로 띄워도 같다(SAC는 실행 자체를 막는다). 이는 결정서 §3의 **서명 재검토
조건**이 충족된 것이다. 미서명 단일 실행 파일 채널은 SAC가 꺼진(또는 평가 모드에서 꺼진) PC에서만 동작한다.
SAC가 꺼진 PC의 한 줄 설치·Claude MSIX 설정 인식·Desktop 되돌림은 아직 미관측이다.

