# 프로젝트 파일 쓰기 보호 — 개발용 Host 어댑터

상태: opt-in 소스 구현. 기존 Plugin/설치본에는 포함하거나 활성화하지 않았다.
대상은 macOS의 Codex 데스크톱과 Claude Desktop **Code 탭의 로컬 세션**이다.
Claude Desktop 일반 채팅·Cowork, 원격 세션과 Windows 셸 보호를 검수했다고 주장하지 않는다.

## 정책

프로젝트 안 파일의 생성·수정·삭제·이름 변경·이동·외부 결과 가져오기는 실제로 열린 Experiment
Verify에서만 수행한다. Closing Interview를 포함한 다른 모든 단계, 승인 대기, Step 사이 경계와
종료된 Chain에서는 허용하지 않는다. DOCX·PNG·PDF 제작, 렌더링, 테스트 출력도 파일 변경이다.
프로젝트 밖 작업과 프로젝트 파일을 남기지 않는 웹 조사·기존 증거 읽기는 GIL Step을 요구하지 않는다.
외부 결과를 가져올 때는 실제 출처와 검증을 기록하며, 그 제작을 소급해 Verify 실행으로 적지 않는다.

`gil-write-guard`는 기존 Core의 읽기 전용 세션과 검증된 Graph를 사용한다. 형식 변환, 승인 생성,
Snapshot 채택, 자동 복구를 하지 않는다. 상태가 없거나 손상됐거나 잠겼으면 프로젝트 편집은 거절한다.
경로가 명확한 프로젝트 밖 편집은 GIL 상태를 읽을 필요도 없다.

## 연결 준비

개발용 바이너리를 빌드한다:

```sh
cargo build --locked -p gil --bin gil-write-guard
```

사용자가 선택한 **별도 GIL 프로젝트**의 절대경로를 지정하면 두 Host가 읽을 수 있는 hooks JSON을
stdout으로 출력한다. source 저장소에 `gil start`를 실행하지 않는다.

```sh
/absolute/path/to/gil-write-guard config --root /absolute/path/to/project
```

이 명령은 설정 파일을 생성·수정하지 않는다. 출력에는 바이너리와 프로젝트의 절대경로가 들어가므로
개발 로그·공개 PR에 실제 출력값을 넣지 않는다. 연결 시 기존 훅을 보존해 해당 Host의 hooks 설정에
병합한다. Codex는 훅 신뢰 검토가 필요하다. Claude Code 탭도 기존 설정의 훅을 사용하지만 일반
Desktop 채팅의 MCP 설정과는 다른 표면이다. Plugin 설치·설정 변경은 별도 작업이다.

프로젝트 경로는 설정에 고정되며 cwd에서 추측하지 않는다. 프로젝트를 옮기거나 다른 프로젝트로
전환할 때 설정을 갱신해야 한다. 한 실행 환경에는 이 보호 대상 하나만 연결한다. 여러 root의 셸
래퍼를 겹쳐 설치하는 구성은 지원하지 않는다.

## 실행 방식

- `Write`, `Edit`, `MultiEdit`, `NotebookEdit`, `apply_patch`: 대상 경로를 해석하고 Verify 상태를
  검사한다. patch의 삭제·이동 원본과 목적지도 검사한다. 프로젝트 밖에서 안으로 향하는 symlink는
  해석해 검사한다. `.gil` 직접 편집과 프로젝트를 포함하는 상위 경로 편집은 거절한다.
- `Bash` / `exec_command`: 명령 문자열에서 파일 쓰기를 추측하지 않는다. 원래 호출을 거절하고
  정확히 인용된 `gil-write-guard run --root … -- '원래 명령'` 재호출을 안내한다. 이 재호출은 기존
  Host 권한 검사에 맡긴다. `permissionDecision: allow`로 승인 절차를 생략하지 않는다.
- 보호 실행은 macOS `/usr/bin/sandbox-exec` 아래에서 `/bin/sh -c`로 실행한다. Verify 밖에서는
  프로젝트 경로의 `file-write*`를 거절하고, Verify 안에서는 `.gil` 직접 쓰기만 거절한다. 밖의 파일
  쓰기와 읽기에는 추가 제한을 두지 않는다. Host 자체 sandbox 제한은 계속 적용된다.
- 정상 foreground 실행이 끝날 때까지 GIL 잠금을 유지하므로 그동안 Verify close는 거절된다.
  쓰기 상태를 모르면 실행하지 않으며 sandbox 실행 실패 시 원래 명령으로 fallback하지 않는다.
- GIL 자체 기록·승인·허용된 복구는 기존 MCP 도구로 수행한다. 잠금을 잡은 보호 셸 안에서 GIL
  명령을 중첩 실행하지 않는다. 파일을 밖으로 옮겨 dirty를 해소해도 최초 위반 사실이 없어지지 않는다.

## 보장의 한계와 실제 Host 인수

이 어댑터는 **모든 외부 프로세스에 적용하는 OS 접근 통제 제품이 아니다.** 아래 제한 때문에
전체 우회 방지나 두 Host의 실제 적용 완료로 표시하지 않는다.

- 훅이 활성화·신뢰되어야 한다. 훅 호출 자체의 실패·timeout 처리는 Host에 달려 있다.
- 등록하지 않은 MCP 파일 도구, GUI, 기존 프로세스·기존 interactive shell은 보호하지 않는다.
  Codex `write_stdin`은 새 PreToolUse를 거치지 않는다. 연결 전의 실행 세션은 종료해야 한다.
- 백그라운드 실행은 지원하지 않는다. 명시적 `run_in_background`는 거절하지만 스크립트 내부의
  detach/daemonize까지 입증하는 파서는 없다. Verify에서 시작한 자손을 남긴 채 close하면 안 된다.
- 직접 편집 훅의 검사와 실제 편집은 Host의 별도 동작이다. 그 사이의 동시 상태 전이·symlink 교체,
  프로젝트 밖 hard link 별칭, 별도 프로세스의 변경까지 원자적으로 차단한다고 주장하지 않는다.
- 셸은 `/bin/sh`로 실행하므로 기존 Host의 zsh/bash 전용 문법은 필요하면 명시적으로 해당 셸을
  호출한다. shell wrapper는 해당 프로세스 트리에 한정된다.

연결 후에는 두 Host에서 각각 합성 프로젝트로 확인한다: Closing Synthesis에서 DOCX 생성 거절,
밖의 파일 생성과 웹 조사 허용, Verify에서 DOCX/PNG 생성 허용, Verify 종료 뒤 재수정 거절,
`.gil` 직접 쓰기 거절, 거절 뒤 정상 GIL 승인·기록 진행, 훅 비활성화 상태를 보호 중으로 오인하지
않는지. Rust의 JSON/실제 sandbox 시험은 이 Host 인수를 대신하지 않는다.

공식 근거(2026-11-02 확인): [Codex Hooks](https://learn.chatgpt.com/docs/hooks),
[Claude Code Hooks](https://code.claude.com/docs/en/hooks),
[Claude Code Desktop 공유 설정](https://code.claude.com/docs/en/desktop#shared-configuration).

## 로컬 검증 기록 (2026-11-02, macOS arm64)

- `cargo test --locked -p gil`: 통과. 최초 sandbox 실행의 로컬 IPC 거절은 권한을 허용한 실행으로
  재검사했다. 변경 전 안내 문구에 의존한 assertion과 신규 fixture 초기 저장 오류는 수정 후 재검증했다.
- `cargo build --locked -p gil --all-targets`: 통과. Companion/Tauri 전체 workspace 검사가 아니다.
- `--features approved-plan,product-safety`의 `guidance_wave1`, `guidance_wave2`, `write_guard`:
  각각 7·1·6개 통과. 기본 프로필의 보호 어댑터 검사는 unit 1개와 integration 7개 통과.
- 지정된 JS 회귀 명령: 174개 통과, 4개 건너뜀(패키지에 native Core 미탑재).
- 지정된 패키징·고지 회귀 명령: 102개 통과, 1개 건너뜀.
- workflow 동기화, `git diff --check`, 수정한 평가 스크립트의 `node --check`: 통과.
- 실제 sandbox 파일 쓰기·읽기·외부 출력·폴더 이동 거절·잠금 수명은 격리 임시 프로젝트에서 확인했다.
  실제 모델 workflow 평가, 두 Host의 설치/신뢰/훅 실행, 화면 인수, Windows와 원격 환경은 미실행이다.
