# GIL 설치 안내

macOS preview.22는 시각자료 JSON 예시·필드별 오류·현재 owner와 계획·Cycle 진행 안내를 보완한 검증판이다.
Windows owner.14는 기존 계획 입력·Closing 안내 검증판을 유지한다. 새 대화·새 빈 폴더에서 확인하며 기존 기록은 자동 변환하지 않는다.

이 문서가 친구·외부 시험 사용자를 위한 설치 정본이다. 사용자는 터미널 사용법을 알 필요가 없다.
GIL은 **Codex**와 **Claude Desktop의 Claude Code 탭**을 같은 지원 대상으로 둔다. 두 Host는 같은 Core,
같은 Monitor UI, 같은 작업 규칙(`src/mcp/workflow.md`)을 쓰며, 다른 것은 설치·연결 방법뿐이다.
여기서 “Claude”는 **Claude Desktop 안의 Claude Code 탭**을 뜻한다. 터미널 Claude Code, 일반 Chat 탭,
Cowork의 결과를 이 환경의 결과로 대신 적지 않는다.

설치 도움을 받는 경우 **로컬 폴더를 연 Codex 대화 또는 Claude Code 탭**에 아래 요청문을 붙여넣고,
Agent가 앱에 포함된 CLI와 OS 기본 도구를 사용한다. 일반 ChatGPT/Work 대화처럼 로컬 명령을 실행할 수
없는 환경에서는 설치하지 않는다.

## 네 조합의 지원 상태 (2026-11-06)

| OS | Codex | Claude Desktop (Claude Code 탭) |
|---|---|---|
| Windows 11 x64 | **이전 판 실기기 확인 / 이번 판 사용자 확인 대기** — Companion installer `0.1.1-owner.14`, **Codex 연결** 버튼 | **사용자 확인 대기** — `0.1.1-owner.14`의 **Claude Desktop 연결** 버튼(자동 테스트 통과) |
| macOS Apple Silicon | **이전 판 실기기 확인 / 이번 판 사용자 확인 대기** — Plugin `0.2.1-preview.22` 또는 Git 없는 archive | **이전 판 사용자 확인(11/01) / 이번 판 확인 대기** — 같은 archive의 Core를 Desktop 설정에 직접 등록. Plugin 경로는 화면 미표시 |

상태의 뜻:

- **실기기 확인** — 사용자가 실제 화면(Monitor, fullscreen, 파일 작업)을 보고 확인했다.
- **사용자 확인** — 특정 날짜·버전·경로에서 사용자가 화면을 확인했다. 아래 절에 범위를 적는다.
- **자동 테스트만 통과** — 단위·계약·패키징 시험은 통과했지만 실제 앱 화면은 아직 아무도 보지 않았다.
- **사용자 확인 대기** — 설치본과 절차는 있고, 사용자의 실제 확인만 남았다.

두 Host 모두에서 Interview 종료, Experiment, Verify 파일 증거, Analysis, Outcome 학습, Monitor 승인
규칙은 같은 Core 계약이 강제한다. Host별 workflow 사본은 없다.

## 현재 제공하는 시험판

| 환경 | 버전 | 상태 |
|---|---|---|
| Windows 11 x64 | `0.1.1-owner.14` | 계획 입력·Closing 안내 검증판, 실사용 검수 대기 |
| macOS Apple Silicon | `0.2.1-preview.22` | 시각자료·계획 진행 안내 수정판, 실제 Host 검수 대기 |

이번 판은 승인 저장과 최신 상태 조회를 구분한다. 승인 뒤 읽기가 잠시 실패하면 확인된 승인 판을 유지하고
회색 **최신 상태 다시 확인 중** 안내를 보인다. 연속 실패 때는 조회 실패로 알리며 **다시 읽기**를 제공한다.
저장 응답을 확인하지 못한 경우에는 **저장 결과 불확실**로 표시하고 승인을 자동 재전송하지 않는다.
이전 판의 승인을 새 제안에 적용하지 않는다. 조회는 2/4/8/15초 간격으로 재시도한다.

이번 두 배포본은 같은 `approved-plan` + `visual-authoring` 시험 프로필이다. **새 대화·새 빈 폴더**에서
Monitor 승인 버튼 → Interview 닫기 → 계획 항목별 Experiment → Verify 파일 증거를 시험한다.
새 프로젝트는 저장 형식 7을 사용한다. 기존 기록의 형식은 자동 변환하지 않으며, 이전 설치본으로
되돌려도 형식 7 기록은 이전 형식으로 바뀌지 않는다. 닫힌 실패 Cycle의 revisit만 Monitor에서 변경 목록 확인·보존 승인을 거쳐 진행한다. 일반 restore와 Step revisit는 계속 차단한다.
macOS 검증 범위와 배포물은 [preview.22 장부](distribution/codex/PREVIEW-22-PUBLICATION-20261106.md), Windows owner.14는 [게시 장부](distribution/windows/PUBLICATION-OWNER-14-20261103.md)를 따른다.

> Windows **Claude Desktop 연결** 버튼은 `0.1.1-owner.10`부터, 공통 Monitor의 **업데이트 안내·확대·미니맵**은 owner.10/preview.11부터 포함된다.
> 이전 owner.8/owner.9에는 Claude Desktop 연결 버튼이 없다. 업데이트 뒤 새 연결에서 확인한다.

둘 다 정식 서명판이 아니다. Windows Authenticode와 Apple Developer ID/공증을 제공하지 않는다.
파일 hash는 손상·바이트 일치를 확인하지만 제작자 서명을 대신하지 않는다. OS가 정상적인 사용자 확인
절차를 제공하면 대상·버전을 확인하고 선택할 수 있지만, 보안 기능을 끄거나 quarantine·SmartScreen·
Defender·Gatekeeper·ACL을 변경해 실행을 강제하지 않는다.

설치와 업데이트는 Project 폴더나 `.gil` 기록을 삭제·이관하지 않는다. 기존 설치가 발견되면 Agent는
출처와 버전을 먼저 보여 주고 사용자 승인 전에는 제거·교체하지 않는다. Plugin cache, Codex 설정 파일,
Claude Desktop 설정 파일, registry를 Agent가 손으로 편집하지 않는다. Claude Desktop 설정 파일은
Companion의 버튼(Windows) 또는 아래 절차의 사용자 승인 뒤 **Desktop을 완전히 종료한 상태**에서만 바꾼다.
보호 훅의 승인된 등록·갱신은 아래 **보호 훅 등록과 갱신** 절차를 따른다.
기존 비-GIL 설정·권한·sandbox를 보존하고 원본 백업과 정확한 변경 내용을 확인한다.

## Windows 11 x64

릴리스: [windows-owner-update-0.1.1-owner.14](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.14)

- 설치 파일: `GIL.Companion.Unsigned.Test._0.1.1-owner.14_x64-setup.exe`
- 크기: `6,165,288` bytes
- SHA-256: `f1be0b1bc0cd102a4045be9ddb0c4f5aaf572d8faa9b539cfe867dba32f86267`
- 지원: x64만. ARM64 Windows에는 설치하지 않는다.

Windows에서는 Host가 어느 쪽이든 Core는 한 곳에 있다. installer가 `%LOCALAPPDATA%\GIL\runtime\`에
**고정 런처** `gil-launcher.exe`와 버전별 Core `versions\<버전>\gil.exe`, 선택 기록 `current.json`을 둔다.
Codex와 Claude Desktop 모두 **고정 런처**를 등록하므로 업데이트 뒤에도 등록을 바꾸지 않으며,
새로 시작한 연결은 같은 선택 버전의 Core를 사용한다. 이미 열린 연결은 기존 Core를 유지한다.

사용자 절차:

1. 위 릴리스에서 정확한 x64 설치 파일을 받는다.
2. 파일명·크기·SHA-256을 확인하고 실행한다. 별도 관리자 권한은 요구하지 않는다.
3. 설치 후 시작 메뉴에서 **GIL Companion (Unsigned Test)**를 연다.
4. 쓰는 Host의 버튼을 누른다. 두 버튼은 각각 그 Host만 바꾼다.
   - **Codex 연결** — 공식 `codex mcp add` 명령으로 고정 런처를 등록한다. 이전 GIL이 있으면 표시되는
     **이전 GIL 연결 교체** 내용을 확인한 뒤 승인한다.
   - **Claude Desktop 연결** — `%APPDATA%\Claude\claude_desktop_config.json`의 `mcpServers.gil`에 고정
     런처를 적는다. **Claude Desktop을 완전히 종료한 뒤** 눌러야 한다. 실행 중이면 Companion이 거절한다
     (Desktop은 실행 중에 자체 설정을 같은 파일에 다시 써서 바깥 편집을 되돌릴 수 있다). 다른 MCP 서버
     항목과 사용자가 직접 만든 동명 `gil` 등록은 건드리지 않으며, 이전 GIL 실행 파일을 가리키는 `gil`
     항목만 **이전 GIL 연결 교체**로 바꿀 수 있다. 바꾸기 전 같은 폴더에 백업
     `claude_desktop_config.json.gil-backup-<시각>`을 남긴다(최근 5개 보관).
5. Host를 (다시) 열고 새 대화에서 빈 테스트 폴더를 선택한다.
6. **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말하고 실제 Monitor와 첫 단계가 보이는지 확인한다.
   Claude Code 탭에서는 App이 연결 직후와 Monitor 준비 뒤 fullscreen을 자동 요청한다. 자동으로 펼쳐지지
   않으면 **카드를 한 번 클릭하면** 펼쳐진다(아래 macOS Claude 절의 Host 동작과 같다).

**직접 등록(임시, owner.8/owner.9 사용자용).** owner.8/owner.9 Companion에는 Claude Desktop 버튼이 없다. Claude Desktop을
완전히 종료한 뒤 `%APPDATA%\Claude\claude_desktop_config.json`의 `mcpServers`에 다음 항목을 더하고 Desktop을
다시 연다. `<사용자>`는 실제 Windows 사용자 이름이다. 버전 폴더가 아니라 **고정 런처**를 적는다.

```json
"gil": {
  "command": "C:\\Users\\<사용자>\\AppData\\Local\\GIL\\runtime\\gil-launcher.exe",
  "args": ["mcp", "--serve"]
}
```

업데이트는 자동 적용하지 않는다. Companion의 **업데이트 확인** 후 사용자가 **업데이트**를 눌렀을 때만
설치 창이 열린다. Codex·Claude Desktop과 기존 GIL 대화는 종료하지 않아도 되며, 기존 연결은 기존 Core를
계속 쓰고 업데이트 뒤 만든 새 연결부터 새 Core를 사용한다. 두 Host가 같은 고정 런처를 쓰므로 업데이트
뒤 등록을 다시 할 필요가 없다. 설치 창이 실패하면 기존 선택과 Project는 보존되어야 하며, 같은 버전
재설치는 정상적인 복구 설치로 끝나야 한다. Companion 머리줄의 `Core <버전>`은 **다음 새 연결**이 받을
Core이고, 이미 열린 대화는 열 때의 Core를 끝까지 쓴다.

제거 순서는 **Companion의 Codex 연결 해제·Claude Desktop 연결 해제 → 열린 GIL 대화 닫기 → Windows 앱
제거**다. 현재 제거기는 실행 중 연결과 Project를 보호하기 위해 `%LOCALAPPDATA%\GIL\runtime`과 모든
`.gil` 기록을 보존한다.

상세한 Agent 대응은 [Windows 설치 안내](distribution/windows/INSTALL.md)를 따른다.

## macOS Apple Silicon

릴리스: [v0.2.1-preview.22](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.22)

- Plugin: `gil-companion-prototype@gil-preview-macos-arm64`
- marketplace tag: `v0.2.1-preview.22`
- marketplace commit: `10f89576df104362e3ff1007487f0dbf1c01c2af`
- Git 없는 설치 archive: `gil-codex-macos-arm64-0.2.1-preview.22.tar.gz`
- archive 크기: `3,498,578` bytes
- archive SHA-256: `789d966ad85bc5108600966a09729d7c8a8b89f7572e02e88e498d8b656599fa`
- 지원: Apple Silicon(`arm64`)만. Intel Mac에는 설치하지 않는다.

archive 하나가 Codex용 `.agents/plugins/marketplace.json`과 Claude Code용 `.claude-plugin/marketplace.json`,
같은 Core(`plugins/gil-companion-prototype/core/darwin-arm64/gil`)를 함께 담는다.
macOS에는 아직 고정 런처와 Companion 버튼이 없다. Codex는 Plugin 관리자가 버전을 관리하고, Claude Desktop은
**풀어 둔 버전 폴더의 Core를 직접 등록**한다. 그래서 macOS Claude 사용자는 **업데이트 뒤 등록 경로를
새 버전 폴더로 다시 가리켜야 한다**(아래 “업데이트” 참고). 이 차이는 macOS Companion 런처가 들어올 때까지
남는 제한이다.

<a id="기존-전역-write-guard-갱신"></a>

### 보호 훅 등록과 갱신 — 신규 설치에도 포함

macOS의 표준 설치는 **Plugin/MCP 연결 → 사용하는 Host의 보호 훅 등록 → 설정 일치 확인 →
새 세션 활성화 확인**까지다. Plugin 설치나 MCP 등록 자체가 훅을 자동 생성하지는 않는다.
아래 요청문은 설치 Agent에게 보호 훅 등록도 요청한다. 훅이 없거나 일부를 지웠어도 같은 명령으로
등록·복구하며, 사용자가 보호 훅을 원하지 않으면 강제로 설치하지 않고 “MCP만 연결, 보호 미설정”으로 보고한다.
이 절차는 macOS 로컬 Codex·Claude Desktop Code 탭용이다. Windows owner.14에는 적용하지 않는다.

Agent는 실제 설치 결과와 MCP manifest/직접 등록에서 실행 파일의 절대경로·버전을 확인한다.
Codex는 공식 `plugin add --json`의 `installedPath`와 manifest의 상대 실행 경로를 대조하고,
Claude는 `mcpServers.gil.command`를 사용한다. 캐시 경로를 추측하거나 전역 PATH의 다른 `gil`을 쓰지 않는다.
다음 `GIL_CORE`는 그 검증된 절대경로를 담은 변수다. **사용하는 Host만** 등록한다.

```sh
# Codex 설치를 돕는 Agent
"$GIL_CORE" write-guard install --auto --host codex
"$GIL_CORE" write-guard status --auto
```

```sh
# Claude Desktop Code 탭 설치를 돕는 Agent
"$GIL_CORE" write-guard install --auto --host claude
"$GIL_CORE" write-guard status --auto
```

두 Host 모두를 설치하도록 요청받았고 같은 Core를 쓸 때만 `--host both`를 쓴다. 각각 다른 설치 경로라면
각 Host의 Core로 별도 등록하고, 같은 배포 버전·Core bytes인지 대조한다. 다른 Host까지 임의로 바꾸지 않는다.
installer가 PreToolUse·PostToolUse·UserPromptSubmit·Stop을 병합하며 원본을 private 백업에 남긴다.
권한·sandbox·훅 신뢰를 자동 허용하지 않는다. Host 설정이나 조직 정책이 훅을 막으면 임의로 해제하지 않는다.

MCP/Plugin 설치만 바꾸면 `~/.codex/hooks.json`·`~/.claude/settings.json`의 절대 훅 경로가
구 Core에 남을 수 있다. 업데이트도 위 명령으로 새 Core에 맞추고, 훅이 없다는 이유로 생략하지 않는다.
이전 Core는 갱신·검증이 끝날 때까지 보존한다. 비-GIL 설정과 원본 백업은 보존한다.
새 Core의 `write-guard status --auto`에서 해당 Host가 `matches_current_core`인지 확인한다.
이 값은 설정 일치이며 Host 활성화 증명이 아니다. 세 결과를 나누어 보고한다.

1. **연결·설정 준비:** 설치 버전·MCP 경로·선택 Host의 네 훅·`matches_current_core`와 백업을 확인한다.
   실패하면 완료로 표시하지 않고 성공한 부분과 미완료 단계를 적는다.
2. **Host 보호 활성화:** 앱을 재시작하고 새 세션에서 정상 훅 검토를 진행한다(Codex는 `/hooks`).
   사용자가 선택한 버려도 되는 테스트 폴더에서 `gil_start` 또는 `gil_context`를 호출하고,
   `status --auto`의 해당 root 등록과 실제 Host의 Verify 밖 편집 거절·Verify 안 편집 허용을 확인한다.
   정책·비활성화·신뢰 미완료 또는 필드테스트 미실시면 “설정 준비 완료, 실제 보호 확인 대기”로 보고한다.
   실제 사용자 프로젝트에 시험 파일을 쓰거나 시험용 승인을 생성하지 않는다.
3. **Monitor 표시:** 실제 화면 표시·fullscreen 여부를 따로 확인한다. 위 두 결과로 UI 성공을 대신하지 않는다.

기존 프로젝트는 새 연결에서 `gil_context`로 재개한다. 성공한 시작·재개 뒤의 프로젝트 등록은 자동이므로
새 폴더마다 설정을 편집하지 않는다. 설치 단계에서는 프로젝트 등록 목록·기록·Snapshot을 바꾸지 않는다.

기존 훅이 설치 셸을 막으면 원래 명령을 보호 없이 다시 실행하지 않는다.
[정확한 설정 diff·백업을 준비하는 갱신 절차](distribution/codex/INSTALL.md#별도로-설치한-전역-write-guard-갱신)를
따른다. 전역 훅 삭제, 권한 자동 허용, 보안 설정 해제로 해결하지 않는다.

비연결 채팅에는 GIL 완료 의무가 없지만 등록된 프로젝트 경로는 계속 보호한다. Claude는 보호
셸로 교체한 입력의 승인을 요청하고 Codex는 지원하는 래퍼 재호출을 안내한다. 임의 셸을 cwd나
문자열만으로 면제하지 않는다. 중첩 sandbox로 보호 실행이 불가능한 Host의 제한은 남는다.
[보호 범위·한계](distribution/codex/WRITE-GUARD.md)를 읽고 두 Host의 실제 동작을 별도로 검증한다.

### Codex에 붙여넣을 요청

```text
GIL unsigned preview 0.2.1-preview.22와 Codex용 보호 훅을 함께 설치해 줘. 사용자가 터미널을 직접 조작하게 하지 마.
플러그인 관리 MCP 도구만 보고 설치 불가라고 결론내리지 말고, 이 로컬 Codex 작업의 셸에서
앱에 포함된 codex CLI를 사용해. 프로젝트를 초기화·이관하거나 .gil 기록을 바꾸지 마.

먼저 다음을 읽기 전용으로 확인해:
- uname -s / uname -m 이 Darwin / arm64인지
- command -v codex, codex --version
- codex plugin list --json
- codex plugin marketplace list --json
- xcode-select -p 와 git --version이 성공하는지

기존 GIL이나 gil-preview-macos-arm64가 있으면 자동으로 제거·교체하지 말고 출처와 버전을
알려준 뒤 내 승인을 기다려. 이미 승인했다면 다시 묻지 마. Plugin cache는 직접 편집하지 마.
보호 훅이 없거나 지워졌어도 위 등록 절차를 포함해. 설정 변경은 검토한 GIL 항목만 적용해.

Git이 실제로 동작하면 다음 고정 tag 경로를 사용해:
codex plugin marketplace add https://github.com/hyun06000/gil-marketplace.git --ref v0.2.1-preview.22 --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

Git 또는 Xcode Command Line Tools를 찾지 못하면 개발 도구 설치를 제안하지 말고 Git 없는 경로로 전환해.
다음 archive를 임시 파일로 받은 뒤 압축 해제 전에 크기와 SHA-256을 모두 확인해:
https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.22/gil-codex-macos-arm64-0.2.1-preview.22.tar.gz
크기: 3498578 bytes
SHA-256: 789d966ad85bc5108600966a09729d7c8a8b89f7572e02e88e498d8b656599fa

값이 다르면 아무것도 풀지 말고 중단해. 일치하면 tar 목록에서 절대경로, .. 경로 이동,
심볼릭 링크, 하드 링크와 특수 파일이 없는지 확인해. 문제가 없으면 기존 폴더를 덮어쓰지 말고
$HOME/Library/Application Support/GIL/preview.22-local 이라는 새 사용자 전용 폴더에 풀어.
그 폴더의 .agents/plugins/marketplace.json, release.json, Plugin version 0.2.1-preview.22,
core/darwin-arm64/gil 실행 권한을 확인한 뒤 다음 공식 CLI 명령을 사용해:
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.22-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

네트워크나 Codex 설정 저장 권한이 필요하면 정상 승인 창으로 요청하고, 승인 뒤에는
codex plugin list --json으로 상태를 다시 읽어 끝나지 않은 단계만 이어가. 다시 다운로드·압축 해제하거나
무조건 재등록하지 마. Gatekeeper를 끄거나 quarantine·권한을 강제로 변경하지 마.

Git·Git 없는 경로 모두 Plugin 설치 뒤에는 실제 설치 결과의 Core 절대경로를 확인해서
그 Core로 write-guard install --auto --host codex 와 write-guard status --auto 를 실행해.
다른 Host는 바꾸지 마. 네 훅과 codex의 matches_current_core, 원본 백업·비-GIL 설정 보존을 확인해.
훅 신뢰·권한·sandbox를 자동 허용하거나 비활성화 설정을 임의로 해제하지 마.
마지막으로 codex plugin list --json에서 설치 버전이 0.2.1-preview.22인지 확인해 알려줘.
앱 재시작·새 세션의 정상 /hooks 검토와 버려도 되는 테스트 폴더의 보호 확인을 안내해.
연결·설정 준비, 실제 Host 보호 활성화, Monitor 표시를 구분해서 보고해.
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
Claude Desktop(Claude Code 탭 포함)은 MCP 서버를 자기 설정 파일
`~/Library/Application Support/Claude/claude_desktop_config.json`에서 읽는다(2026-11-02 확인: 이 Mac의 Claude
Code 탭이 그 파일의 `gil` 항목을 서비스로 띄웠고, 사용자 범위 `~/.claude.json`에는 서버가 없었다).
2026-10-31 Desktop 4.4212.0 실측에서 **Desktop 설정에 직접 등록한 MCP 서버**의 Monitor는 inline 카드와
**모니터 펼치기** fullscreen까지 사용자가 확인했고, 2026-11-01에는 이 저장소 빌드(`dev-20261101-local`)의
inline 카드가 주변 대화와 어울린다고 확인했다. 반면 **Plugin으로 설치한 서버**는 설치와 도구 호출이
성공해도 Host가 App을 화면에 그리지 않았다(세 빌드·두 설치 방식 모두). 따라서 지금 Claude에서 Monitor를
보는 경로는 직접 등록이다. 자세한 근거는 [호출 경로별 진단](mcp-app/CLAUDE-DESKTOP-ROUTES.md)을 본다.

**1단계 — archive 준비(공통).** 위 Git 없는 경로 그대로 archive를 받아 크기·SHA-256·멤버 목록을 검증하고
사용자 전용 새 폴더(`$HOME/Library/Application Support/GIL/<버전>-local`, 0700)에 푼다. preview.22 archive는 최상위에 Codex용 `.agents/plugins/marketplace.json`과 Claude Code용
`.claude-plugin/marketplace.json`을 함께 담는다. 이전 `0.2.1-preview.7` archive에는 `.claude-plugin`이 없다.

**2단계 — 직접 등록(현재 화면이 성립하는 경로).** 먼저 Claude Desktop을 **완전히 종료**한다(Dock에서
종료, 주 프로세스 `Claude`가 없어야 한다. 브라우저 확장이 띄우는 보조 프로세스는 남아 있어도 된다).
Desktop은 실행 중에 자체 설정을 같은 파일에 다시 쓰므로, 켜진 상태에서 고친 내용은 되돌아갈 수 있다
(10/31 실측). 종료한 뒤 설정 파일의 `mcpServers`에 아래 항목을 더한다. 다른 서버
항목은 그대로 둔다. `<폴더>`는 1단계의 실제 절대경로다. 공용 작업 규칙은 Rust MCP 서버에 포함되어 Plugin
Skill 없이도 도구 설명과 `gil_help`로 전달된다.

```json
"gil": {
  "command": "<폴더>/plugins/gil-companion-prototype/core/darwin-arm64/gil",
  "args": ["mcp", "--serve"]
}
```

**3단계 — 보호 훅 등록과 확인.** Desktop을 다시 열기 전에 위에 등록한 같은 Core로
`write-guard install --auto --host claude`와 `write-guard status --auto`를 실행한다.
신규 설치·삭제된 훅 복구·업데이트 모두 포함한다. 위 **보호 훅 등록과 갱신** 절차로 백업과
`installed_hooks.claude.status`가 `matches_current_core`인지 확인한 뒤 Desktop을 다시 열고,
새 Code 세션에서 실제 보호를 확인한다.
직접 MCP 등록 파일과 훅 설정 파일(`~/.claude/settings.json`)은 서로 다르다.
Claude 안에서 설치를 돕는 중이라 종료로 작업을 이어갈 수 없으면 준비한 경로·백업·남은 단계를 남기고,
Desktop 종료 뒤 로컬 설치 Agent에서 이어간다. 실행 중 Desktop 설정을 바꾸거나 완료로 보고하지 않는다.

Claude Code 탭에 붙여넣어 Agent에게 맡길 수 있는 요청문:

```text
GIL unsigned preview 0.2.1-preview.22를 Claude Desktop에 직접 등록하고 Claude용 보호 훅도 설치해 줘. 내가 터미널을 직접 조작하게 하지 마.
위 Git 없는 경로 그대로 archive를 받아 크기·SHA-256·tar 멤버를 검증하고
$HOME/Library/Application Support/GIL/preview.22-local 에 풀어. 기존 폴더는 덮어쓰지 마.
그다음 ~/Library/Application Support/Claude/claude_desktop_config.json 을 읽어 지금 mcpServers 에 무엇이
있는지 보여 주고, gil 항목이 이미 있으면 어떤 실행 파일을 가리키는지 알려 준 뒤 내 승인을 기다려.
내가 승인하면: Claude Desktop 이 완전히 종료됐는지(주 프로세스 Claude 가 없는지) 확인하고, 설정 파일을
같은 폴더에 백업한 뒤, 다른 항목은 그대로 두고 mcpServers.gil 만 위 Core 경로와 ["mcp","--serve"] 로 적어.
Desktop 이 실행 중이면 쓰지 말고 종료를 요청해. 이미 교체를 승인했다면 다시 묻지 마.
직접 등록한 Core로 write-guard install --auto --host claude 와 write-guard status --auto 를 실행해.
훅이 없거나 지워졌어도 생략하지 마. Codex 설정은 바꾸지 마.
네 훅과 claude의 matches_current_core, 백업·비-GIL 설정 보존을 확인해.
훅 신뢰·권한·sandbox를 자동 허용하거나 비활성화 설정을 임의로 해제하지 마.
새 Code 세션에서 버려도 되는 테스트 폴더로 실제 보호를 확인하도록 안내하고,
연결·설정 준비, 실제 Host 보호 활성화, Monitor 표시를 나눠 보고해.
종료 때문에 현재 Agent가 이어갈 수 없으면 남은 단계를 적고, 종료 뒤 로컬 설치 Agent에서 이어가.
쓰고 나서 파일을 다시 읽어 결과를 보여 줘.
Gatekeeper 나 quarantine, 권한은 바꾸지 마. 화면이 실제로 보이기 전에는 Monitor 가 열렸다고 말하지 마.
```

**업데이트(macOS Claude).** 새 preview가 나오면 1단계를 새 `<버전>-local` 폴더에 반복하고, Desktop을
완전히 종료한 뒤 `gil` 항목의 `command`를 새 폴더의 Core로 바꾸고 **3단계의 훅 등록·확인도 반복한다**. 이전 폴더는 지우지 않아도 된다(열린
대화가 그 Core를 쓸 수 있다). 등록을 바꾸지 않으면 **Claude는 구버전 Core를 계속 쓰고 Codex Plugin만 새
버전이 되므로** 두 Host의 버전이 어긋난다. Monitor **연결 정보**의 `host`·GIL 버전과 `gil_help`의 서버
버전으로 어느 Core에 연결됐는지 확인한다.

**2단계(대안) — Plugin 설치.** 아래 절차로 설치는 되지만, 2026-10-31 현재 Claude Code 탭은 Plugin 서버의
Monitor App을 화면에 그리지 않는다. Host가 고쳐졌는지 확인하는 용도로만 쓰고, 설치 성공을 화면 성공으로
보고하지 않는다. Plugin과 직접 등록을 같은 Desktop에 동시에 두면 같은 이름의 도구가 둘이 되므로 한 경로만 둔다.

```text
위 Git 없는 경로 그대로 archive를 검증해서 $HOME/Library/Application Support/GIL/<버전>-local 에 풀어 줘.
그다음 Claude Code 플러그인 명령으로 그 폴더를 marketplace로 더하고
gil-companion-prototype@gil-preview-macos-arm64 를 설치해 줘. 설정 파일이나 플러그인 cache를
직접 편집하지 말고, 설치 뒤 플러그인 목록에서 버전을 확인해 알려 줘.
이 대안 경로도 같은 Core로 write-guard install --auto --host claude 와 write-guard status --auto 를 수행해.
백업·비-GIL 설정 보존과 claude의 matches_current_core를 확인하고 실제 Host 보호·화면 확인은 구분해.
```

```bash
claude plugin marketplace add "$HOME/Library/Application Support/GIL/<버전>-local"
```

```bash
claude plugin install gil-companion-prototype@gil-preview-macos-arm64
```

설치 뒤 **새 세션**을 열고 빈 테스트 폴더에서 **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말한다.
Agent의 도구 성공 문장과 실제 화면을 구분한다. 10/31 실측에서 Claude Code 탭은 Monitor 준비 뒤에 보낸 자동
표시 모드 요청을 적용하지 않았고 Monitor는 inline 카드로 뜬 뒤 **카드를 한 번 클릭하면** fullscreen으로
펼쳐졌다. 2026-11-02 빌드부터 App은 연결 직후에도 fullscreen을 요청한다. 이 요청이 Claude Code 탭에서
적용되는지는 사용자 화면 확인 대기이며, 적용되지 않으면 첫 클릭이 그대로 펼친다. fullscreen 지원과 “항상
자동으로 열림”은 다른 것이다. Monitor의 **연결 정보**에는 Host가 보고한 이름·버전, 광고한 표시 모드, 그리고
`automatic`(자동 시도의 단계·시각·Host 응답)이 그대로 적히므로, 문제를 제보할 때 그 값을 함께 적는다. fullscreen 중 다른 도구의 승인 창이 보이지 않으면 **대화 안으로**를 눌러 inline으로 돌아간 뒤
승인한다.

## Monitor의 업데이트 안내

설치된 Core는 저장소의 [업데이트 피드](distribution/update-feed.json)를 **하루 한 번, metadata만** 읽어
(OS 내장 `curl`, HTTPS만, 64KB 한도) 새 버전이 있으면 Monitor 맨 아래 표시 모드 줄에 **“새 버전 X 있음 · 업데이트
안내”** 버튼을 띄운다. 버튼은 OS별 절차를 글로 보여 줄 뿐 다운로드·설치·등록 변경을 하지 않는다.

- Windows: Companion → **업데이트 확인** → **업데이트** 절차와 **Companion 열기** 버튼. 두 Host가 같은 고정 런처를
  쓰므로 재연결은 필요 없다.
- macOS: Codex에 붙여넣을 업데이트 요청문과, Claude Desktop 직접 등록의 경로 재지정 절차(위 “업데이트(macOS Claude)”).

설치 버전은 Core가 놓인 자리(Windows `runtime\versions\<버전>\`, Mac Plugin의 `.codex-plugin/plugin.json`)에서
읽는다. 개발용 빌드는 설치본이 아니므로 확인하지 않는다. 설치 버전을 못 읽으면 “최신 X · 업데이트 안내”로만 표시한다.
확인을 끄려면 Host의 MCP 서버 환경에 `GIL_UPDATE_CHECK=off`를 두거나 Monitor 상태 폴더
(macOS `~/Library/Application Support/GIL/monitor-bindings-v1`, Windows `%LOCALAPPDATA%\GIL\monitor-bindings-v1`)에
`update-check.off` 파일을 둔다. 결과와 마지막 확인 시각은 Monitor **연결 정보**의 `update`에 적힌다.
새 버전을 게시할 때는 게시 장부와 같은 PR에서 피드 파일을 갱신한다.

## 실패 후 revisit가 막힌 기존 프로젝트 이어가기

이번 버전은 macOS와 Windows에 같은 절차를 제공한다. 새 프로젝트로 옮기거나 기존 `.gil`을
지우지 않는다. 업데이트 후 **새 GIL 연결·새 대화**에서 기존 프로젝트의 정확한 루트를 지정해 Monitor를 연다.
이미 복구가 완료됐다면 아래 복원을 반복하지 않고 `gil_context`로 현재 기록을 읽고 이어간다.

1. Agent에게 **“실패 기록과 증거를 유지하고, Monitor에서 되돌아갈 변경 목록을 보여 줘”**라고 요청한다.
2. 실패 Cycle·대상 Cycle·파일 변경 목록·보존 위치를 읽는다. 채팅의 동의나 자동 새로고침은 승인으로 처리하지 않는다.
3. 사용자가 Monitor의 **보존 후 되돌아가기**를 누르면 원본 파일을 실제 기록 저장소의
   `.gil/revisit-preserved/<승인 식별자>/`에 별도로 보존한 뒤 선택한 과거 파일 상태로 돌아간다.
4. 기존 실패 Report·Will·Snapshot·승인 이력을 유지하고 새 Cycle에서 같은 항목을 다시 시도한다.
   실패한 작업을 성공으로 바꾸거나 옛 증거를 덮어쓰지 않는다.
5. 복구 완료 뒤 Monitor가 채팅으로 계속 요청을 보낸다. 전달되지 않았다면 **채팅에 계속 요청 보내기**를 누른다. 이 버튼은 복원을 반복하지 않는다. 에이전트는 `gil_context`로 현재 기록을 확인한 뒤 진행한다.
6. 승인 후 중단됐다면 Agent가 상태를 다시 읽고 기존 승인에 한해서 복구를 재개한다.
   승인 없는 `gil revisit` 반복 호출로 우회하지 않는다.

현재 파일이 승인된 실패 상태와 다르거나, `.git`·비밀 파일·빌드 산출물·ignore 대상의 변경,
링크·경로 충돌·손상된 증거가 있으면 전체 복구를 거절한다. Windows는 작업 폴더와 기록 저장소가
같은 볼륨이어야 한다. 거절 시 파일이나 추적 메타데이터를 고쳐 통과시키지 말고 원문 오류를 남긴다.
사용자의 실제 막힌 프로젝트에 적용된 결과는 아직 검수 전이며, 합성 시험 통과와 구분한다.

## 설치가 멈췄을 때

| 상황 | 대응 |
|---|---|
| Plugin 관리 도구에 검색·권한·제거만 있음 | 로컬 Codex 셸에서 앱 번들 `codex` CLI 사용. 셸이 없는 대화면 설치 중단 |
| Mac에서 `git clone`이 Xcode 도구 오류로 실패 | Command Line Tools를 설치하지 않고 검증 archive의 로컬 marketplace 경로로 전환 |
| 네트워크/설정 쓰기가 거절됨 | 정상 승인 절차로 필요한 권한을 설명해 요청하고, 승인 뒤 현재 상태부터 재조회 |
| 같은 marketplace 또는 Plugin이 이미 있음 | 출처·버전을 보여 주고 사용자 승인 전에는 교체하지 않음 |
| archive 크기/hash/member 검사가 실패 | 압축 해제·등록하지 않고 원문 오류와 생성된 임시 파일만 보고 |
| 설치 명령은 성공했지만 화면이 없음 | 설치 성공과 UI 성공을 구분하고 새 대화·선택 폴더·실제 Monitor를 확인 |
| Claude Desktop 설정을 고쳤는데 다시 열면 되돌아감 | Desktop이 실행 중이었다. 완전히 종료(주 프로세스 없음)한 뒤 다시 고치고 연다 |
| Companion의 Claude Desktop 연결이 “실행 중” 거절 | Claude Desktop을 완전히 종료한 뒤 다시 누른다 |
| Claude Desktop에 다른 `gil` 등록이 있음 | GIL은 사용자가 만든 동명 등록을 바꾸지 않는다. 그 등록을 직접 정리한 뒤 다시 연결 |
| Claude에서 Monitor가 inline 카드로만 뜸 | 카드를 한 번 클릭하면 펼쳐진다. 연결 정보의 `automatic`에 자동 시도가 적혀 있는지 보고, 펼쳐지지 않으면 그 값과 함께 제보 |
| OneDrive 폴더의 `.gil` 권한 거절 | Project를 자동 이동하거나 ACL을 강제 변경하지 않고 동기화 밖 로컬 폴더 선택을 안내 |

오류 보고에는 OS/CPU, Host 이름과 버전(Codex 버전 또는 Claude Desktop 버전과 탭 이름), 연결 방식(Plugin /
직접 등록 / Companion 버튼), GIL 버전, 설치 단계와 오류 원문만 포함한다. Monitor가 떴다면 **연결 정보**의
`host`·`advertised`·`current` 값을 함께 적는다. 개인 경로·대화·token·`.gil` 내용은 첨부하지 않는다.
보안 문제는 공개 issue가 아니라 [보안 안내](SECURITY.md)를 따른다.
