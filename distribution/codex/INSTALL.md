# GIL macOS preview 설치

GIL은 Host 훅을 설치하지 않는다. 선택한 GIL 프로젝트에는 공통 프롬프트와 단계별 MCP 응답으로 안내한다.
preview.24는 기존 GIL 명령의 정확한 제거·백업과 구형 훅 호출의 무동작 호환 처리를 제공한다.
새 훅을 만들지 않으며 구형 훅 installer도 실행하지 않는다.
Core 승인·전이·증거 검사는 유지하지만 다른 세션의 도구나 파일 쓰기를 가로막지 않는다.

전체 OS 안내의 시작점은 [저장소 설치 안내](../../INSTALL.md)다. 이 문서는 macOS 전용 상세 절차다.

대상은 **macOS Apple Silicon(`arm64`)의 로컬 Codex 작업**이다. 현재 배포판은
`0.2.1-preview.24`이며 2026-11-06 게시한 unsigned prerelease다. Core, MCP server, Skill과
fullscreen Monitor UI가 Plugin 하나에 들어 있으므로 Node·npm·Cargo·Homebrew·별도 Companion을
설치하지 않는다. Intel Mac, Windows에는 이 파일을 사용하지 않는다. Claude Code(Claude Desktop)는
같은 archive의 Core를 Desktop 설정에 직접 등록하는 경로를 사용한다. `.claude-plugin` catalog 설치는 가능하지만
이전 시험에서는 Monitor가 표시되지 않았다. 직접 등록·업데이트 절차는 [저장소 설치 안내](../../INSTALL.md)를 따른다.

- marketplace: `https://github.com/hyun06000/gil-marketplace.git`
- 고정 tag: `v0.2.1-preview.24`
- 고정 commit: `d36f6ba35f9e4e6ee43481580af51f98c0207f29`
- Plugin: `gil-companion-prototype@gil-preview-macos-arm64`
- 공개 archive: `gil-codex-macos-arm64-0.2.1-preview.24.tar.gz`
- archive 크기: `3,647,594` bytes
- archive SHA-256: `6e2d0dc92fefe73eb103c4d85cb1df6bf193146d899294113cb62a7bc534abc5`
- 게시·검증 장부: [PREVIEW-24-PUBLICATION-20261106.md](PREVIEW-24-PUBLICATION-20261106.md)

Apple Developer ID 서명과 공증은 없다. macOS가 정상적인 사용자 확인 뒤에도 실행을 차단하면 멈춘다.
Gatekeeper를 끄거나 quarantine·권한·Plugin cache를 변경해 우회하지 않는다.


## Core 버전 확인과 구형 표기

새 버전 표기 수정판부터는 실제 MCP가 실행하는 Core의 `--version`, MCP initialize의
`serverInfo.version`, agent probe의 `core_version`이 배포 버전과 일치해야 한다.
아래에 고정된 preview.24와 그 이전 판은 이 수정 전이므로 `gil 0.1.0`을 출력할 수 있다.
이는 Cargo 패키지 버전이며 **그 한 줄만으로 설치 실패나 구버전이라고 단정하지 않는다.**
Windows owner.14도 기존 배포이므로 같은 구형 확인 절차를 따른다.

설치 Agent는 PATH의 임의 `gil` 대신 공식 설치 응답·manifest 또는 직접 MCP 등록에서 확인한
실행 파일을 검사한다. macOS는 공개 archive의 크기·SHA-256, `release.json`의 배포 버전과
실제 Core 파일 해시, Codex·Claude manifest 버전을 대조한다. Core 해시가 receipt와 다르면
정상 설치로 보고하지 않는다. Windows는 선택된 `current.json`과 실제 Core 해시를 대조한다.

새 표기 수정판에서 `--version`만 구버전이면 실제 실행 경로를 다시 확인한다. 디스크의 버전은
맞지만 MCP 서버 정보만 다르면 기존 연결이 살아 있는지 확인하고 앱 재시작·새 세션으로 연결한다.
버전 문구는 무결성·제작자 인증을 대신하지 않으므로 해시 검증을 생략하지 않는다.
이 확인을 위해 프로젝트를 초기화하거나 기록·설정·보안 정책을 임의로 바꾸지 않는다.

## 사용자에게 보장할 설치 경험

사용자는 터미널 명령을 직접 입력하지 않는다. Codex에서 빈 폴더를 선택해 **로컬 명령을 실행할 수 있는
새 대화**를 열고 아래 요청문을 붙여넣는다. Agent가 사용하는 앱 내장 `codex` CLI는 Plugin 관리 MCP
도구와 별개다. 관리 도구에 검색·권한·제거만 있다는 이유로 설치 불가라고 결론내리지 않는다.

일반 ChatGPT/Work 대화처럼 로컬 셸이 없는 환경에서는 설치하지 않는다. Agent가 별도 Codex CLI,
Git, Xcode Command Line Tools나 개발 도구 설치를 사용자에게 요구해서도 안 된다.

## Agent가 먼저 판단할 분기

다음 항목만 읽기 전용으로 확인한다.

```sh
uname -s
uname -m
command -v codex
codex --version
codex plugin list --json
codex plugin marketplace list --json
xcode-select -p
git --version
```

1. Darwin/arm64가 아니면 중단한다.
2. 앱 번들 `codex`가 없거나 `plugin marketplace add`/`plugin add`를 지원하지 않으면 중단한다.
3. 기존 GIL이 있으면 출처와 버전을 보고하고 교체 승인을 기다린다.
4. Git과 활성 Command Line Tools가 실제로 동작하면 고정 Git tag 경로를 사용한다.
5. `git clone`이 Command Line Tools 부재로 실패하면 개발 도구 설치를 제안하지 않고 Git 없는 경로로
   전환한다. 실패한 등록이 남았는지 `codex plugin marketplace list --json`으로 다시 확인한다.

## Codex에 붙여넣을 통합 요청

```text
GIL unsigned preview 0.2.1-preview.24를 훅 없이 설치해 줘. 내가 터미널을 직접 조작하게 하지 마.
플러그인 관리 MCP 도구만 보고 설치 불가라고 결론내리지 말고, 이 로컬 Codex 작업의 셸에서
앱에 포함된 codex CLI를 사용해. 프로젝트를 초기화·이관하거나 .gil 기록을 바꾸지 마.

먼저 uname -s, uname -m, command -v codex, codex --version,
codex plugin list --json, codex plugin marketplace list --json,
xcode-select -p, git --version으로 OS·CPU·CLI·기존 설치·Git 상태를 확인해.
Darwin/arm64가 아니면 중단해. 기존 GIL이 있으면 출처와 버전을 알려주고 교체 전에 물어봐.

Git이 실제로 동작하면 다음 고정 tag를 사용해:
codex plugin marketplace add https://github.com/hyun06000/gil-marketplace.git --ref v0.2.1-preview.24 --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

Git 또는 Xcode Command Line Tools가 없거나 clone이 그 이유로 실패하면 개발 도구를 설치하지 말고
Git 없는 경로로 전환해. 다음 파일을 임시 위치에 다운로드해:
https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.24/gil-codex-macos-arm64-0.2.1-preview.24.tar.gz

압축 해제 전에 크기가 정확히 3647594 bytes이고 SHA-256이
6e2d0dc92fefe73eb103c4d85cb1df6bf193146d899294113cb62a7bc534abc5인지 확인해.
다르면 아무것도 풀거나 등록하지 말고 중단해.

일치하면 tar 목록에서 절대경로, .. 경로 이동, 심볼릭 링크, 하드 링크와 특수 파일이 없는지 확인해.
문제가 없으면 기존 폴더를 덮어쓰지 말고
$HOME/Library/Application Support/GIL/preview.24-local 이라는 새 사용자 전용 폴더에 풀어.
.agents/plugins/marketplace.json, release.json, Plugin version 0.2.1-preview.24와
core/darwin-arm64/gil 실행 권한을 확인한 뒤 다음 명령을 사용해:
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.24-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json

네트워크나 Codex 설정 저장 권한이 필요하면 무엇이 필요한지 설명하고 정상 승인 창으로 요청해.
승인 뒤에는 plugin list와 marketplace list로 상태를 다시 읽고 미완료 단계만 이어가.
이미 검증·해제한 파일을 다시 받거나 같은 marketplace를 무조건 재등록하지 마.
Plugin cache를 직접 편집하거나 Gatekeeper·quarantine·권한을 강제로 변경하지 마.
Git·Git 없는 경로 모두 Plugin 설치 뒤 실제 설치 응답의 installedPath와 manifest에서 Core 절대경로를 확인해.
아래 훅 제거 절차에 따라 기존 GIL 훅만 백업·정리하고 새 훅은 등록하지 마. 다른 Host는 바꾸지 마.
비-GIL 설정·권한·sandbox를 보존해. 앱 재시작·새 세션에서 일반 프로젝트 비간섭과 GIL 단계별 안내를
확인하도록 안내해. 실제 Host를 시험하지 못하면 확인 대기라고 보고하고 파일 쓰기의 강제 차단을 주장하지 마.

마지막으로 codex plugin list --json에서 설치 버전이 정확히 0.2.1-preview.24인지 확인해 알려줘.
화면이 실제로 보이기 전에는 Monitor가 열렸다고 말하지 마.
```

Git 없는 경로의 archive member·목적지·권한 검사와 유지관리자 스크립트는
[GITLESS-INSTALL.md](GITLESS-INSTALL.md)에 더 자세히 기록한다.

## 이번 시험판의 범위

`approved-plan` + `visual-authoring` 프로필에서 새 프로젝트는 Chain과 Monitor 승인 계획을 쓴다.
새 빈 폴더에서 승인 버튼, Interview 닫기, 항목별 Experiment와 실제 증거 파일을 확인한다.
저장 형식은 새 프로젝트에서 7이며 기존 기록의 형식은 자동 변환하지 않는다. 이전 Plugin으로
되돌려도 기록 형식은 내려가지 않는다. 닫힌 실패 Cycle의 revisit만 Monitor 승인으로 진행하며 일반 restore와 Step revisit는 계속 차단한다. Luna 실사용은 사용자 검수다.

기존 실패 프로젝트는 [공통 복구 절차](../../INSTALL.md#실패-후-revisit가-막힌-기존-프로젝트-이어가기)를 따른다.

## 설치 후 실제 동작 확인

1. 아래 기존 GIL 훅 제거 확인 뒤 앱을 재시작하고 **새 Codex 대화**를 연다. 설치 전 대화는 이전 MCP 연결을 유지할 수 있다.
2. 빈 테스트 폴더를 선택한다.
3. **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말한다.
4. 실제 Monitor, 첫 Interview와 graph가 보이는지 확인한다. 도구 성공 문장만으로 UI 성공을 판정하지 않는다.
5. [공통 확인 절차](../../INSTALL.md#훅-없는-설치와-기존-훅-제거)에 따라 일반 프로젝트 비간섭과 선택한 GIL 프로젝트의 단계별 안내를 확인한다. 훅의 파일 쓰기 차단은 제공하지 않는다.
6. 기존 Project라면 **“기록을 바꾸지 말고 GIL Monitor를 열어 줘”**라고 말한다.

이전 preview.6의 깨끗한 Mac VM에서 Git/Xcode Command Line Tools 부재 → Git 없는 archive 검증 → 로컬 marketplace 등록 →
Plugin 설치 → GIL 동작을 사용자가 확인했다. 별도의 fullscreen 전환과 앱 완전 종료 후 복원을 그 VM에서
확인하지 않았다면 그 항목까지 통과했다고 기록하지 않는다. 이번 preview.24는 별도 실사용 검수 대상이다.

## 기존 설치·업데이트·제거

기존 GIL 또는 같은 marketplace가 있으면 Agent는 `codex plugin list --json`과
`codex plugin marketplace list --json` 결과를 먼저 보여 주고 교체 승인을 기다린다. 승인 전에는
`plugin remove`나 `marketplace remove`를 실행하지 않는다. 같은 버전 bytes를 덮어쓰지 않는다.
사용자가 현재 대화에서 버전 교체를 이미 승인했다면 같은 승인을 다시 묻지 않는다.

교체가 승인되면 새 고정 tag 또는 검증된 Git 없는 폴더를 준비한 뒤 공식 CLI로 기존 등록을 제거하고
위의 새 버전 등록·설치를 진행한다. 제거 전 확인한 출처와 버전은 복구용으로 남긴다.

```sh
codex plugin remove gil-companion-prototype@gil-preview-macos-arm64 --json
codex plugin marketplace remove gil-preview-macos-arm64 --json
```

각 명령 뒤 조회하여 완료한 단계는 반복하지 않는다. 새 설치가 끝나면 `plugin list`에서 preview.24를
확인하고 아래 기존 GIL 훅 제거를 확인한 뒤 새 대화를 연다. Plugin 제거는 프로젝트 폴더나 `.gil` 기록 삭제를 포함하지 않는다.

<a id="별도로-설치한-전역-write-guard-갱신"></a>

### 훅 없는 설치와 기존 훅 제거

[공통 제거 절차](../../INSTALL.md#훅-없는-설치와-기존-훅-제거)가 정본이다. 전역·프로젝트 로컬·Plugin·Skill
훅은 등록하지 않는다. 구형 설정의 GIL 명령만 백업·제거하고 비-GIL 훅·권한·sandbox는 보존한다.
훅 제거 기능이 있는 새 배포본에서, 공식 설치 응답·manifest로 확인한 Core 절대경로를 사용한다.
preview.23 이하에는 아래 명령이 없으므로 정확한 설정 diff와 백업을 준비해 GIL 명령만 정리한다.

```sh
"$GIL_CORE" write-guard uninstall --auto --host codex
"$GIL_CORE" write-guard status --auto
```

Claude만 요청받았으면 `--host claude`, 두 Host 모두 요청받았으면 `--host both`를 쓴다.
`not_installed`는 검사한 설정의 결과이며 실제 Host 메모리·사용자 스크립트 검증을 대신하지 않는다.
`custom_review_required`는 사용자 정의 명령의 정확한 검토가 필요하다는 뜻이다.
프로젝트 registry·기록·승인·Snapshot은 읽거나 변경하지 않는다. 없는 설정은 만들지 않는다.

구형 훅이 설치 셸을 막으면 반복 실행하지 않는다. 정확한 제거 diff와 원본 백업을 먼저 준비하고,
Host가 지원하는 파일 편집 도구로 승인된 GIL 항목만 제거한다. 도구가 없으면 그 제한과 남은 변경을
보고한다. 전체 훅 삭제·보안 해제·자동 승인이나 사용자의 수동 터미널 실행을 기본 해결책으로 삼지 않는다.
새 세션에서 MCP 버전, 일반 프로젝트 비간섭과 선택한 GIL 프로젝트의 단계별 안내를 확인한다.
임시 HOME 자동검증·실제 Host의 모델 준수·Monitor 화면은 구분한다. [범위와 한계](WRITE-GUARD.md)를 따른다.

직전 공개 배포판은 `0.2.1-preview.23`, tag `v0.2.1-preview.23`, marketplace commit
`b3e9b6da119ff447a541e02c07281afa088a06ec`다. preview.23에는 전역 파일 보호 훅이 남아 있다.
이전 Plugin으로 복구하더라도 폐기한 훅을 다시 등록하지 않는다. 복구는 Plugin 버전 교체이며 Project Artifact를
되돌리는 `gil restore`가 아니다. Plugin 제거·교체로 Project 폴더나 `.gil`을 삭제하지 않는다.
preview.4 이후 새 이미지 Project는 저장 형식 5를 사용할 수 있고 preview.3는 이를 읽지 못한다.

## 실패별 대응

| 실패 | Agent의 대응 |
|---|---|
| 관리 도구에 설치 기능이 없음 | 로컬 셸의 앱 번들 `codex` CLI를 확인. 로컬 셸이 없으면 중단 |
| Xcode Command Line Tools를 찾지 못함 | 개발 도구를 설치하지 않고 Git 없는 로컬 marketplace 경로로 전환 |
| 네트워크·설정 저장 권한 거절 | 필요한 권한을 설명해 정상 승인 요청; 승인 뒤 현재 상태를 재조회 |
| 기존 marketplace/Plugin 발견 | 출처·버전 보고 후 사용자 승인 대기 |
| 다운로드 크기/hash/member 불일치 | 압축 해제·등록 금지, 오류 원문과 생성된 임시 항목만 보고 |
| CLI 등록 성공, 화면 없음 | 새 대화·선택 폴더·실제 Monitor를 별도로 확인 |
| OneDrive의 `.gil` 권한 거절 | ACL을 강제로 바꾸지 않고 동기화 밖 로컬 작업 폴더 사용을 안내 |

오류 보고에는 OS/CPU, Codex 버전, Plugin 버전, 실패 단계와 오류 원문만 포함한다. 개인 경로·대화·
token·`.gil` 내용은 첨부하지 않는다. 보안 취약점은 공개 issue 대신 [보안 안내](../../SECURITY.md)를 따른다.
