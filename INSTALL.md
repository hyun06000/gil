# GIL 설치 안내

이 문서가 친구·외부 시험 사용자를 위한 설치 정본이다. 사용자는 터미널 사용법을 알 필요가 없다.
설치 도움을 받는 경우 **로컬 폴더를 연 Codex 대화**에 아래 요청문을 붙여넣고, Agent가 앱에 포함된
`codex` CLI와 OS 기본 도구를 사용한다. 일반 ChatGPT/Work 대화처럼 로컬 명령을 실행할 수 없는
환경에서는 설치하지 않는다.

## 현재 제공하는 시험판

| 환경 | 버전 | 상태 |
|---|---|---|
| Windows 11 x64 | `0.1.1-owner.5` | 친구용 증거 필수 시험판·CI 설치 검수 완료, 실기기 Luna 검수 대기 |
| macOS Apple Silicon | `0.2.1-preview.6` | 친구용 unsigned Codex Plugin·Git 없는 VM 설치와 동작 확인 |

둘 다 정식 서명판이 아니다. Windows Authenticode와 Apple Developer ID/공증을 제공하지 않는다.
파일 hash는 손상·바이트 일치를 확인하지만 제작자 서명을 대신하지 않는다. OS가 정상적인 사용자 확인
절차를 제공하면 대상·버전을 확인하고 선택할 수 있지만, 보안 기능을 끄거나 quarantine·SmartScreen·
Defender·Gatekeeper·ACL을 변경해 실행을 강제하지 않는다.

설치와 업데이트는 Project 폴더나 `.gil` 기록을 삭제·이관하지 않는다. 기존 설치가 발견되면 Agent는
출처와 버전을 먼저 보여 주고 사용자 승인 전에는 제거·교체하지 않는다. Plugin cache, Codex 설정 파일,
registry를 손으로 편집하지 않는다.

## Windows 11 x64

릴리스: [windows-owner-update-0.1.1-owner.5](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.5)

- 설치 파일: `GIL.Companion.Unsigned.Test._0.1.1-owner.5_x64-setup.exe`
- 크기: `5,545,097` bytes
- SHA-256: `ce4b91c56164e4705cd054e486b54def06d9e86f174fc08fe49632599ef9a866`
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

릴리스: [v0.2.1-preview.6](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.6)

- Plugin: `gil-companion-prototype@gil-preview-macos-arm64`
- marketplace tag: `v0.2.1-preview.6`
- marketplace commit: `2a1ec23576eb35e05210dda1d8b5400a577ec30f`
- Git 없는 설치 archive: `gil-codex-macos-arm64-0.2.1-preview.6.tar.gz`
- archive 크기: `3,189,503` bytes
- archive SHA-256: `477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5`
- 지원: Apple Silicon(`arm64`)만. Intel Mac에는 설치하지 않는다.

### Codex에 붙여넣을 요청

```text
GIL unsigned preview 0.2.1-preview.6을 설치해 줘. 사용자가 터미널을 직접 조작하게 하지 마.
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
codex plugin marketplace add https://github.com/hyun06000/gil-marketplace.git --ref v0.2.1-preview.6 --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

Git 또는 Xcode Command Line Tools를 찾지 못하면 개발 도구 설치를 제안하지 말고 Git 없는 경로로 전환해.
다음 archive를 임시 파일로 받은 뒤 압축 해제 전에 크기와 SHA-256을 모두 확인해:
https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.6/gil-codex-macos-arm64-0.2.1-preview.6.tar.gz
크기: 3189503 bytes
SHA-256: 477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5

값이 다르면 아무것도 풀지 말고 중단해. 일치하면 tar 목록에서 절대경로, .. 경로 이동,
심볼릭 링크, 하드 링크와 특수 파일이 없는지 확인해. 문제가 없으면 기존 폴더를 덮어쓰지 말고
$HOME/Library/Application Support/GIL/preview.6-local 이라는 새 사용자 전용 폴더에 풀어.
그 폴더의 .agents/plugins/marketplace.json, release.json, Plugin version 0.2.1-preview.6,
core/darwin-arm64/gil 실행 권한을 확인한 뒤 다음 공식 CLI 명령을 사용해:
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.6-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

네트워크나 Codex 설정 저장 권한이 필요하면 정상 승인 창으로 요청하고, 승인 뒤에는
codex plugin list --json으로 상태를 다시 읽어 끝나지 않은 단계만 이어가. 다시 다운로드·압축 해제하거나
무조건 재등록하지 마. Gatekeeper를 끄거나 quarantine·권한을 강제로 변경하지 마.

마지막으로 codex plugin list --json에서 설치 버전이 0.2.1-preview.6인지 확인해 알려줘.
화면이 실제로 보이기 전에는 Monitor가 열렸다고 말하지 마.
```

설치 후 새 대화에서 빈 테스트 폴더를 선택하고 **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고
말한다. 이미 기록이 있는 폴더라면 **“기록을 바꾸지 말고 GIL Monitor를 열어 줘”**라고 말한다.

깨끗한 Mac VM에서 Git/Xcode Command Line Tools 부재를 확인한 뒤 Git 없는 경로로 설치했고 GIL 동작까지
사용자가 확인했다. 이 확인은 Apple 서명·공증이나 모든 Mac 호환성을 의미하지 않는다. fullscreen 전환과
앱 완전 종료 후 복원이 그 VM에서 별도로 확인되지 않았다면 성공 범위에 포함하지 않는다.

상세한 실패·복구 분기는 [macOS 설치 안내](distribution/codex/INSTALL.md)와
[Git 없는 설치](distribution/codex/GITLESS-INSTALL.md)를 따른다.

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
