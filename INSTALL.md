# GIL 설치 안내

이 문서가 친구·외부 시험 사용자를 위한 설치 정본이다. 사용자는 터미널 사용법을 알 필요가 없다.
설치 도움을 받는 경우 **로컬 폴더를 연 Codex 대화**에 아래 요청문을 붙여넣고, Agent가 앱에 포함된
`codex` CLI와 OS 기본 도구를 사용한다. 일반 ChatGPT/Work 대화처럼 로컬 명령을 실행할 수 없는
환경에서는 설치하지 않는다.

## 현재 제공하는 시험판

| 환경 | 버전 | 상태 |
|---|---|---|
| Windows 11 x64 | `0.1.1-owner.7` | Monitor 승인·Chain 시험판, 실기기 Luna 검수 대기 |
| macOS Apple Silicon | `0.2.1-preview.8` | Monitor 승인·Chain 시험판, 실사용 검수 대기 |

이번 두 배포본은 같은 `approved-plan` + `visual-authoring` 시험 프로필이다. **새 대화·새 빈 폴더**에서
Monitor 승인 버튼 → Interview 닫기 → 계획 항목별 Experiment → Verify 파일 증거를 시험한다.
새 프로젝트는 저장 형식 7을 사용한다. 기존 기록의 형식은 자동 변환하지 않으며, 이전 설치본으로
되돌려도 형식 7 기록은 이전 형식으로 바뀌지 않는다. 제품의 restore/revisit는 여전히 차단된다.
검증 범위와 정확한 배포물은 [게시 장부](distribution/codex/PREVIEW-8-PUBLICATION-20261102.md)에 남긴다.

둘 다 정식 서명판이 아니다. Windows Authenticode와 Apple Developer ID/공증을 제공하지 않는다.
파일 hash는 손상·바이트 일치를 확인하지만 제작자 서명을 대신하지 않는다. OS가 정상적인 사용자 확인
절차를 제공하면 대상·버전을 확인하고 선택할 수 있지만, 보안 기능을 끄거나 quarantine·SmartScreen·
Defender·Gatekeeper·ACL을 변경해 실행을 강제하지 않는다.

설치와 업데이트는 Project 폴더나 `.gil` 기록을 삭제·이관하지 않는다. 기존 설치가 발견되면 Agent는
출처와 버전을 먼저 보여 주고 사용자 승인 전에는 제거·교체하지 않는다. Plugin cache, Codex 설정 파일,
registry를 손으로 편집하지 않는다.

## Windows 11 x64

릴리스: [windows-owner-update-0.1.1-owner.7](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.7)

- 설치 파일: `GIL.Companion.Unsigned.Test._0.1.1-owner.7_x64-setup.exe`
- 크기: `5,919,030` bytes
- SHA-256: `0b690218e67370495544174a45b2658c2a99e3baa2a8040e70c61268be0cee96`
- 지원: x64만. ARM64 Windows에는 설치하지 않는다.

사용자 절차:

1. 위 릴리스에서 정확한 x64 설치 파일을 받는다.
2. 파일명·크기·SHA-256을 확인하고 실행한다. 별도 관리자 권한은 요구하지 않는다.
3. 설치 후 시작 메뉴에서 **GIL Companion (Unsigned Test)**를 연다.
4. Companion에서 **Codex 연결**을 누른다. 이전 GIL이 있으면 표시되는 교체 내용을 확인한 뒤 승인한다.
5. Codex를 다시 열고 새 대화에서 빈 테스트 폴더를 선택한다.
6. **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말하고 실제 Monitor와 첫 단계가 보이는지 확인한다.

업데이트는 자동 적용하지 않는다. Companion의 **업데이트 확인** 후 사용자가 **업데이트**를 눌렀을 때만
설치 창이 열린다. 기존 Codex 대화와 Core는 종료하지 않아도 되며 기존 연결은 기존 Core를 계속 쓰고,
업데이트 뒤 만든 새 연결부터 새 Core를 사용한다. 설치 창이 실패하면 기존 선택과 Project는 보존되어야
하며, 같은 버전 재설치는 정상적인 복구 설치로 끝나야 한다.

제거 순서는 **Companion의 Codex 연결 해제 → 열린 GIL 대화 닫기 → Windows 앱 제거**다. 현재 제거기는
실행 중 연결과 Project를 보호하기 위해 `%LOCALAPPDATA%\GIL\runtime`과 모든 `.gil` 기록을 보존한다.

상세한 Agent 대응은 [Windows 설치 안내](distribution/windows/INSTALL.md)를 따른다.

## macOS Apple Silicon

릴리스: [v0.2.1-preview.8](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.8)

- Plugin: `gil-companion-prototype@gil-preview-macos-arm64`
- marketplace tag: `v0.2.1-preview.8`
- marketplace commit: `116bc49f4a464f75c81400fb2ff3d18091b50706`
- Git 없는 설치 archive: `gil-codex-macos-arm64-0.2.1-preview.8.tar.gz`
- archive 크기: `3,435,833` bytes
- archive SHA-256: `5fcfbb2a328ae65c71d6b5aa4432a556d89429d0fe4c98cacb855e9c2a88d266`
- 지원: Apple Silicon(`arm64`)만. Intel Mac에는 설치하지 않는다.

### Codex에 붙여넣을 요청

```text
GIL unsigned preview 0.2.1-preview.8을 설치해 줘. 사용자가 터미널을 직접 조작하게 하지 마.
플러그인 관리 MCP 도구만 보고 설치 불가라고 결론내리지 말고, 이 로컬 Codex 작업의 셸에서
앱에 포함된 codex CLI를 사용해. 프로젝트를 초기화·이관하거나 .gil 기록을 바꾸지 마.

먼저 다음을 읽기 전용으로 확인해:
- uname -s / uname -m 이 Darwin / arm64인지
- command -v codex, codex --version
- codex plugin list --json
- codex plugin marketplace list --json
- xcode-select -p 와 git --version이 성공하는지

기존 GIL이나 gil-preview-macos-arm64가 있으면 자동으로 제거·교체하지 말고 출처와 버전을
알려준 뒤 내 승인을 기다려. 설정 파일이나 Plugin cache를 직접 편집하지 마.

Git이 실제로 동작하면 다음 고정 tag 경로를 사용해:
codex plugin marketplace add https://github.com/hyun06000/gil-marketplace.git --ref v0.2.1-preview.8 --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

Git 또는 Xcode Command Line Tools를 찾지 못하면 개발 도구 설치를 제안하지 말고 Git 없는 경로로 전환해.
다음 archive를 임시 파일로 받은 뒤 압축 해제 전에 크기와 SHA-256을 모두 확인해:
https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.8/gil-codex-macos-arm64-0.2.1-preview.8.tar.gz
크기: 3435833 bytes
SHA-256: 5fcfbb2a328ae65c71d6b5aa4432a556d89429d0fe4c98cacb855e9c2a88d266

값이 다르면 아무것도 풀지 말고 중단해. 일치하면 tar 목록에서 절대경로, .. 경로 이동,
심볼릭 링크, 하드 링크와 특수 파일이 없는지 확인해. 문제가 없으면 기존 폴더를 덮어쓰지 말고
$HOME/Library/Application Support/GIL/preview.8-local 이라는 새 사용자 전용 폴더에 풀어.
그 폴더의 .agents/plugins/marketplace.json, release.json, Plugin version 0.2.1-preview.8,
core/darwin-arm64/gil 실행 권한을 확인한 뒤 다음 공식 CLI 명령을 사용해:
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.8-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

네트워크나 Codex 설정 저장 권한이 필요하면 정상 승인 창으로 요청하고, 승인 뒤에는
codex plugin list --json으로 상태를 다시 읽어 끝나지 않은 단계만 이어가. 다시 다운로드·압축 해제하거나
무조건 재등록하지 마. Gatekeeper를 끄거나 quarantine·권한을 강제로 변경하지 마.

마지막으로 codex plugin list --json에서 설치 버전이 0.2.1-preview.8인지 확인해 알려줘.
화면이 실제로 보이기 전에는 Monitor가 열렸다고 말하지 마.
```

설치 후 새 대화에서 빈 테스트 폴더를 선택하고 **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고
말한다. 이미 기록이 있는 폴더라면 **“기록을 바꾸지 말고 GIL Monitor를 열어 줘”**라고 말한다.

이전 preview.6의 깨끗한 Mac VM에서 Git/Xcode Command Line Tools 부재를 확인한 뒤 Git 없는 경로로 설치했고 GIL 동작까지
사용자가 확인했다. 이 확인은 Apple 서명·공증이나 모든 Mac 호환성을 의미하지 않는다. fullscreen 전환과
앱 완전 종료 후 복원이 그 VM에서 별도로 확인되지 않았다면 성공 범위에 포함하지 않는다.

상세한 실패·복구 분기는 [macOS 설치 안내](distribution/codex/INSTALL.md)와
[Git 없는 설치](distribution/codex/GITLESS-INSTALL.md)를 따른다.

### Claude Desktop(Claude Code 탭)에서 쓰기

Claude Desktop의 **Claude Code** 탭에서도 같은 Core와 Monitor UI를 쓴다. Codex와 별도 제품이 아니다.
2026-10-31 Desktop 4.4212.0 실측에서 **Desktop 설정에 직접 등록한 MCP 서버**의 Monitor는 inline 카드와
**모니터 펼치기** fullscreen까지 사용자가 확인했다. 반면 **Plugin으로 설치한 서버**는 설치와 도구 호출이
성공해도 Host가 App을 화면에 그리지 않았다(세 빌드·두 설치 방식 모두). 따라서 지금 Claude에서 Monitor를
보는 경로는 직접 등록이다. 자세한 근거는 [호출 경로별 진단](mcp-app/CLAUDE-DESKTOP-ROUTES.md)을 본다.

**1단계 — archive 준비(공통).** 위 Git 없는 경로 그대로 archive를 받아 크기·SHA-256·멤버 목록을 검증하고
사용자 전용 새 폴더(`$HOME/Library/Application Support/GIL/<버전>-local`, 0700)에 푼다. preview.8 archive는 최상위에 Codex용 `.agents/plugins/marketplace.json`과 Claude Code용
`.claude-plugin/marketplace.json`을 함께 담는다. 이전 `0.2.1-preview.7` archive에는 `.claude-plugin`이 없다.

**2단계 — 직접 등록(현재 화면이 성립하는 경로).** 먼저 Claude Desktop을 **완전히 종료**한다. Desktop은
실행 중에 자체 설정을 같은 파일에 다시 쓰므로, 켜진 상태에서 고친 내용은 되돌아갈 수 있다(10/31 실측).
종료한 뒤 `~/Library/Application Support/Claude/claude_desktop_config.json`의 `mcpServers`에 아래 항목을
더하고 Desktop을 다시 연다.
`<폴더>`는 1단계의 실제 절대경로다. 공용 작업 규칙은 Rust MCP 서버에 포함되어 Plugin Skill 없이도
도구 설명과 `gil_help`로 전달된다.

```json
"gil": {
  "command": "<폴더>/plugins/gil-companion-prototype/core/darwin-arm64/gil",
  "args": ["mcp", "--serve"]
}
```

**2단계(대안) — Plugin 설치.** 아래 절차로 설치는 되지만, 2026-10-31 현재 Claude Code 탭은 Plugin 서버의
Monitor App을 화면에 그리지 않는다. Host가 고쳐졌는지 확인하는 용도로만 쓰고, 설치 성공을 화면 성공으로
보고하지 않는다. Plugin과 직접 등록을 같은 Desktop에 동시에 두면 같은 이름의 도구가 둘이 되므로 한 경로만 둔다.

```text
위 Git 없는 경로 그대로 archive를 검증해서 $HOME/Library/Application Support/GIL/<버전>-local 에 풀어 줘.
그다음 Claude Code 플러그인 명령으로 그 폴더를 marketplace로 더하고
gil-companion-prototype@gil-preview-macos-arm64 를 설치해 줘. 설정 파일이나 플러그인 cache를
직접 편집하지 말고, 설치 뒤 플러그인 목록에서 버전을 확인해 알려 줘.
```

```bash
claude plugin marketplace add "$HOME/Library/Application Support/GIL/<버전>-local"
```

```bash
claude plugin install gil-companion-prototype@gil-preview-macos-arm64
```

설치 뒤 **새 세션**을 열고 빈 테스트 폴더에서 **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말한다.
Agent의 도구 성공 문장과 실제 화면을 구분한다. Claude Code 탭은 사용자 동작 없이 보낸 표시 모드 요청을 적용하지
않으므로(10/31 실측) Monitor는 inline 카드로 먼저 뜨고, **카드를 한 번 클릭하면** fullscreen으로 펼쳐진다.
Codex처럼 자동으로 펼쳐지지는 않는다. Monitor의 **연결 정보**에는 Host가 보고한 이름·버전과
광고한 표시 모드가 그대로 적히므로, 문제를 제보할 때 그 값을 함께 적는다. fullscreen 중 다른 도구의
승인 창이 보이지 않으면 **대화 안으로**를 눌러 inline으로 돌아간 뒤 승인한다.

## 설치가 멈췄을 때

| 상황 | 대응 |
|---|---|
| Plugin 관리 도구에 검색·권한·제거만 있음 | 로컬 Codex 셸에서 앱 번들 `codex` CLI 사용. 셸이 없는 대화면 설치 중단 |
| Mac에서 `git clone`이 Xcode 도구 오류로 실패 | Command Line Tools를 설치하지 않고 검증 archive의 로컬 marketplace 경로로 전환 |
| 네트워크/설정 쓰기가 거절됨 | 정상 승인 절차로 필요한 권한을 설명해 요청하고, 승인 뒤 현재 상태부터 재조회 |
| 같은 marketplace 또는 Plugin이 이미 있음 | 출처·버전을 보여 주고 사용자 승인 전에는 교체하지 않음 |
| archive 크기/hash/member 검사가 실패 | 압축 해제·등록하지 않고 원문 오류와 생성된 임시 파일만 보고 |
| 설치 명령은 성공했지만 화면이 없음 | 설치 성공과 UI 성공을 구분하고 새 대화·선택 폴더·실제 Monitor를 확인 |
| OneDrive 폴더의 `.gil` 권한 거절 | Project를 자동 이동하거나 ACL을 강제 변경하지 않고 동기화 밖 로컬 폴더 선택을 안내 |

오류 보고에는 OS/CPU, Codex 버전, GIL 버전, 설치 단계와 오류 원문만 포함한다. 개인 경로·대화·token·
`.gil` 내용은 첨부하지 않는다. 보안 문제는 공개 issue가 아니라 [보안 안내](SECURITY.md)를 따른다.
