# Git 없는 Mac 설치 — preview.7 경로 (실사용 검수 대기)

대상은 macOS Apple Silicon의 Codex다. Windows에 이 파일을 사용하지 않는다.
GIL 실행 파일은 바꾸지 않는다. Git clone 대신 **이미 공개한 고정 압축파일**을 HTTPS로 받고,
크기·SHA-256을 검증한 뒤 안전하게 풀어 로컬 마켓플레이스로 등록한다. 사용자에게 Git·Apple
Command Line Tools·Node·Rust·Homebrew·Companion 설치를 요구하지 않는다. Apple 서명·공증은
여전히 없으며 보안 차단은 우회하지 않는다.

**현재 상태 (2026-10-28):** 게시된 preview.7(tag `v0.2.1-preview.7`, source `64de8e5c`)의
고정 archive를 사용한다. 공개 다운로드의 크기/hash, 공식 CLI의 격리 설치, preview.6↔preview.7
교체와 형식 4·5 기록 보존을 확인했다. 이번 버전의 새 Mac·VM·실제 fullscreen 검수는 별도다.
이전 preview.6의 Git 없는 VM 설치·GIL 동작을 사용자가 확인했지만 이번 버전의 실사용 인수로
승계하지 않는다. 권한 요청과 미완료 단계 이어가기 규칙은 유지한다.

**이전 상태 (2026-10-16, preview.4):** 아래 preview.4 경로는 개발 Mac의 격리 profile에서 다운로드 → 검증 →
해제 → 공식 설치 → 손상·중단·기존 설치 분기까지 로컬 검수했다. 개발 도구 없는 Mac VM에서는
**추가 권한 요청 뒤 설치·Monitor 표시 성공, 최초 프롬프트만으로는 설치 미완**이었다(아래 기록).
현재 판 요청문 단독 완료는 아직 확인하지 않았다. 이전 preview.3 VM 인수는 아래 기록으로 남긴다.

| 항목 | 값 |
|---|---|
| version | `0.2.1-preview.7` (unsigned prerelease, 2026-10-28 게시) |
| archive URL | https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.7/gil-codex-macos-arm64-0.2.1-preview.7.tar.gz |
| archive SHA-256 | `ecd045e3a043e820bb5ff59a6d8b6d3ca57e68dfa3d8a8426a598f70682dd8ee` |
| archive 크기 | 3,218,986 bytes |
| Git 경로 기준 | tag `v0.2.1-preview.7` / commit `b81151830a4d723be95705b8f9274b1ff3ed3adb` (Git이 있는 Mac은 [INSTALL.md](INSTALL.md)) |
| rollback | preview.6 archive `477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5` (tag `v0.2.1-preview.6`). preview.3는 format 5 Project를 읽지 못한다 |

## 사용자가 할 일

1. 빈 작업 폴더를 만들고 Codex에서 선택한 뒤 **로컬 명령을 실행할 수 있는 새 대화**를 연다.
2. 아래 요청을 그대로 붙여넣는다. Agent가 개발 도구 설치나 보안 해제를 제안하면 진행하지 않는다.
3. 설치 뒤 새 대화에서 GIL을 시작한다. Agent의 성공 문장과 실제 그래프 표시를 구분한다.

### Codex에 보낼 요청 (붙여넣기 하나)

```text
Git이나 Apple Command Line Tools 없이 GIL unsigned preview 0.2.1-preview.7을 설치해 줘.
앱에 포함된 Codex CLI와 macOS 기본 도구(curl, shasum, tar, mkdir)만 사용해 줘.

[0] 먼저 시스템과 현재 상태를 확인해서 알려 줘:
  - uname -s / uname -m 이 Darwin / arm64 인지. 아니면 여기서 멈춰.
  - sw_vers 의 macOS 버전.
  - codex plugin list 로 gil-preview-macos-arm64 마켓플레이스와 gil-companion-prototype 플러그인의
    등록 여부·출처(local/git)·버전. 이미 등록돼 있으면 자동으로 교체·제거하지 말고 출처와 버전을
    알려준 뒤 내 승인을 기다려. 승인 전에는 아래 단계로 넘어가지 마.
  - 아래 목적지 폴더가 아직 없는지.

[1] 승인하는 배포물은 macOS Apple Silicon용 unsigned preview 0.2.1-preview.7 하나뿐이야.
  아래 HTTPS 파일을 임시 파일로 내려받고, 압축 해제 전에 크기가 정확히 3218986 bytes 이고
  SHA-256 이 정확히 일치하는지 검사해 줘. 다운로드가 끊기거나 값이 다르면 아무것도 풀지 말고 멈춰.
  https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.7/gil-codex-macos-arm64-0.2.1-preview.7.tar.gz
  SHA-256: ecd045e3a043e820bb5ff59a6d8b6d3ca57e68dfa3d8a8426a598f70682dd8ee

[2] 일치하면 압축을 풀기 전에 tar -tzf 로 멤버 이름 전체 목록을, tar -tvzf 로 항목 종류를 보여 줘.
  절대 경로(/로 시작), 상위 경로 이동(..), 심볼릭/하드 링크나 특수 파일 항목이 하나라도 있으면 멈춰.
  문제없으면 내 사용자 Library/Application Support/GIL/preview.7-local 이라는 **새 폴더**에만 풀어 줘.
  목적지 경로의 상위 폴더 어디에도 심볼릭 링크가 없어야 해(/tmp 아래는 사용하지 마). 기존 폴더는 덮어쓰지 마.
  풀 때는 archive 에 기록된 파일 권한(일반 파일 0644, 실행 파일 0755)을 그대로 보존하고 소유자는 적용하지 마.
  새 폴더 자체는 나만 접근하는 권한(0700)으로 둬.
  .agents/plugins/marketplace.json 과 release.json 이 들어 있는 폴더가 등록할 루트야.
  plugins/gil-companion-prototype/.codex-plugin/plugin.json 의 version 이 0.2.1-preview.7 인지,
  plugins/gil-companion-prototype/core/darwin-arm64/gil 에 실행 권한이 있는지 확인해 줘.

[3] 그 절대 경로를 codex plugin marketplace add 에 전달하고,
  codex plugin add gil-companion-prototype@gil-preview-macos-arm64 로 설치해 줘.
  마켓플레이스 등록에는 GitHub 주소나 --ref 를 사용하지 마. 로컬 폴더 경로를 사용해야 해.
  기존 GIL 등록이 있었고 내가 교체를 승인했다면, 먼저 기존 마켓플레이스 등록을 공식 명령으로
  제거한 뒤 새 폴더를 등록해. 기존 폴더나 설치 cache 는 삭제하거나 손으로 수정하지 마.
  이 공식 명령들은 Codex 자신의 설정 파일(~/.codex/config.toml)에 등록 내용을 저장해. 그 저장은
  공식 CLI가 하는 일이고, 네가 설정 파일을 직접 열어 편집하는 것은 금지야. 명령이 쓰기 권한
  (Operation not permitted 등) 때문에 실패하면, 보안을 끄거나 권한을 강제로 바꾸지 말고 이 앱의
  정상 승인 절차로 필요한 권한(네트워크·홈 폴더/설정 쓰기)을 나에게 요청해. 요청 전에 어떤 권한이
  왜 필요한지 설명해 줘.

[3-1] 권한 오류 뒤 이어가기: 승인을 받은 뒤에는 먼저 codex plugin list 로 현재 등록 상태를 확인하고,
  아직 끝나지 않은 단계만 이어가. 이미 받아서 검증·해제한 폴더는 그대로 쓰고, 다시 내려받거나
  다시 풀거나 등록된 마켓플레이스를 무조건 다시 등록하지 마. 승인이 거절되거나 승인 뒤에도 같은
  오류가 나면 거기서 멈추고 원문 오류와 현재 상태를 알려 줘. 준비한 폴더는 지우지 마.

[4] 설치 후 codex plugin list 로 등록 출처가 local 이고 버전이 0.2.1-preview.7 인지 확인해 줘.

규칙: Git·Node·Rust·Homebrew·Companion 을 설치하지 마. 설정 파일이나 설치 cache 를 손으로 수정하지 마
(공식 CLI 가 자기 설정을 저장하는 것은 허용, 네가 편집하는 것은 금지). 필요한 권한은 정상 승인 절차로
요청하고, 보안 검사를 끄거나 quarantine 을 제거하거나 Gatekeeper 나 권한을 우회·강제 변경하지 마.
curl 결과를 셸로 바로 실행하지 마.
네트워크 승인이 필요하면 요청해 줘. 해시·크기 불일치나 설치 오류가 나면 멈추고 원문 오류를 알려줘.
기존 설치본과 프로젝트 기록은 그대로 둬. 프로젝트를 초기화하거나 이관하거나 기존 기록을 바꾸지 마.
화면이 보이기 전에는 Monitor 가 열렸다고 말하지 마.
```

## 유지관리자용 준비 스크립트

Agent가 사용자 대신 위 순서를 수행하는 것이 목표다. 수동 shell 입력은 사용자 필수 절차가 아니다.
저장소의 `prepare-gitless-macos.sh`는 고정 URL·크기·SHA-256·대상 CPU를 확인하고, 같은 폴더의
`check-archive-members.sh`(읽기 전용, 이름은 `tar -tzf`·종류는 `tar -tvzf` 첫 글자만 사용)로 압축 해제 전에
절대 경로·`..`·링크·특수 파일을 거절하며, 조상에 링크가 없는 새 폴더(0700)에만 검수된 권한(0644/0755)으로 푼다.
해시 검증을 건너뛰는 옵션은 없다. 멤버 검사기는 해시와 무관하게 단독 실행할 수 있어 악성 멤버 거절을 회귀시험한다.
다운로드는 부분 파일로 받아 완료·검증 후에만 사용한다. 임의 archive를 받는 `--archive` 시험 옵션도
**동일한 고정 크기·해시**만 허용한다. 준비 스크립트 자체는 실행 파일을 시작하거나 Plugin을 설치하지 않는다.

```sh
# 부모 폴더를 준비하고, 아직 존재하지 않는 전용 목적지를 지정한다. 조상에 심볼릭 링크가 없어야 한다(/tmp 불가).
mkdir -p "$HOME/Library/Application Support/GIL"
/bin/sh prepare-gitless-macos.sh --destination "$HOME/Library/Application Support/GIL/preview.7-local"
# 성공 후에만, 기존 등록과 충돌하지 않는지 확인하고 공식 명령으로 설치한다.
codex plugin list --json
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.7-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json
```

공식 `codex plugin marketplace add`/`plugin add`는 Codex의 `config.toml`에 등록을 **CLI가 직접 저장**한다.
이는 "설정 직접 편집 금지"의 예외가 아니라 공식 경로다: 유지관리자·Agent가 편집기로 설정을 고치는 것은
여전히 금지다. Host sandbox에서 이 저장이 `Operation not permitted`로 거부되면 설치는 미완이다.
그때는 Host의 정상 승인 절차로 필요한 권한을 요청한다. 승인 뒤에는 `codex plugin list`로 현재 등록
상태를 읽고 **미완료 단계만** 이어간다(재다운로드·재추출·무조건 재등록 금지). 승인이 거절되거나 승인
뒤에도 실패하면 중단하고 준비 폴더를 보존한다. 보안 해제·권한 강제 변경으로 우회하지 않는다.

받아 둔 마켓플레이스 폴더는 임시 다운로드가 아니라 **설치 출처**이므로 유지한다.
업데이트는 검증한 새 버전·새 폴더를 준비한 후 사용자 승인과 공식 명령으로 출처를 바꾼다.
로컬 출처에 Git ref 갱신을 적용하거나 같은 버전의 파일을 덮어쓰지 않는다.
제거도 공식 Plugin 관리 기능으로 한다. `.gil`과 사용자 작업 폴더는 삭제하지 않는다.
실패한 준비 폴더나 다운로드는 자동 삭제하지 않는다. 무엇이 생성되었는지 확인한 뒤 별도로 정리한다.
기존 설치(preview.3 등)가 있을 때 교체 전 반드시 사용자 승인을 받는다. preview.3로 되돌리면 format 5
Project는 읽지 못하므로, 이관·복구는 [INSTALL.md](INSTALL.md)의 주의를 따른다.

## 확인한 범위와 남은 검수

### preview.4 로컬 격리 검수 — 2026-10-16 (개발 Mac, VM 아님)

- 회귀시험 `gitless.test.mjs`: 핀(URL·크기·해시·version)·목록 선검사·금지 명령 부재, 잘못된/같은 크기 위조/
  1바이트 부족 archive 거절, 기존 목적지·링크 archive·링크 조상·상대 경로 거절, 도달 불가 proxy로 끊긴
  다운로드에서 목적지 미생성, 실제 공개 archive로 준비한 루트의 권한·version·`publishable:false` 확인,
  멤버 검사기 단독 시험(공백·한글 이름 수용, 절대 경로·`..`·심볼릭/하드 링크 archive 실제 거절).
- 격리 `CODEX_HOME`(저장된 Git 인증정보·Codex 로그인 없음)에서 공개 URL 실제 다운로드 → 검증 → 해제 →
  공식 `codex plugin marketplace add <로컬 경로>` → `plugin add` → 설치 파일 7개 bytes/권한이 검수본과 동일,
  24 tools·format 4/5 합성 Project 읽기. 손상 archive·중단 다운로드에서 아무것도 등록되지 않음.
  기존 preview.3 등록이 있는 격리 profile에서 `codex plugin list`로 발견 → 승인 전 중단 →
  승인 후 공식 제거·새 로컬 루트 등록·설치로 preview.4 전환. 상세는 유지관리자 로컬 장부에 있다.
- 이 검수는 Git이 설치된 개발 Mac에서 Git을 **사용하지 않고** 수행한 것이며, Command Line Tools가
  없는 Mac에서의 설치 인수를 대신하지 않는다.

### Mac VM 사용자 검수 — 2026-10-16 (preview.4, 부분 성공)

개발 도구가 없는 Mac VM에서 사용자가 위 붙여넣기 요청(권한 문구 추가 전 판)으로 시험했다.
다운로드·크기/해시 검증·멤버 검사·해제까지는 성공했다. 공식 `codex plugin marketplace add`가
Codex `config.toml` 저장 단계에서 `Operation not permitted`로 실패해 **최초 프롬프트만으로는 설치가
미완**이었다. 사용자가 Agent에게 필요한 권한을 정상 절차로 요청하도록 지시한 뒤 등록·설치가 완료됐고,
새 대화에서 Monitor 표시까지 성공했다. 이 결과를 반영해 요청문 [3]/[3-1]에 권한 요청·미완료 단계
이어가기·중단 조건을 추가했다. 보안 해제나 설정 직접 편집은 사용되지 않았다.

이 VM에서 이어진 실제 작업에서는 별개의 결함(열린 Interview 단계에서 `gil_close` 입력 거절 뒤 절차
이탈)이 관측됐다. 그 조사·보정은 설치 경로와 분리된 별도 변경으로 진행한다. VM의 HTML과 기록은
보존하며, 지나간 절차를 사후 기록하지 않는다.

### 남은 검수 (preview.4)

- 권한 문구가 추가된 **현재 판** 붙여넣기 요청만으로 깨끗한 Mac VM에서 설치가 끝나는지(최초 프롬프트
  단독 완료). 위 10-16 결과는 추가 지시가 필요했던 부분 성공이다.
- fullscreen 전환·앱 완전 종료 뒤 복원, 업데이트·제거/재설치의 사용자 실측.
- 업데이트·제거/재설치의 사용자 실측, Windows VM·Intel 실기기, Apple 서명·공증.

### Mac VM 사용자 인수 — 2026-10-07 (preview.3)

개발 도구가 없는 UTM Mac-test에서 사용자가 preview.3 Git 없는 경로로 설치 성공을 보고했다.
새 대화에서 계산기를 만들고 수정하는 동안 fullscreen의 가로 그래프와 채팅을 함께 사용했고,
노드 갱신·요약/상세·되돌아감과 이후 분기의 정상 동작을 확인했다. 당시 깨끗한 VM에서 Git 경로는
Apple Command Line Tools 부재로 실패했고, 그 결함이 이 Git 없는 경로를 만든 이유다.
이는 사용자 관측에 근거한 해당 VM의 preview.3 설치·사용 인수이며 preview.4로 승계되지 않는다.
앱을 완전히 종료하지 않았으므로 완전 종료 뒤 복원은 미검증이었다.

## 설치 안내 결함 기록

빈 Codex 대화에 로컬 실행 도구가 없을 수 있고, Git-backed 마켓플레이스는 Git 없는 Mac에서
복제하지 못한다. GIL runtime이 개발 도구를 요구하지 않는다는 사실을 설치 경로 전체의 무의존성으로
일반화하면 안 된다. 설치 완료 기준에는 이 최초 다운로드/등록 단계와 실제 화면을 함께 포함한다.
