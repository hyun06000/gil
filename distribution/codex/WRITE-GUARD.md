# 프로젝트 파일 쓰기 보호 — 개발용 Host 어댑터

상태: opt-in 소스 구현. 기존 Plugin/설치본에는 포함하거나 활성화하지 않았다.
대상은 macOS의 Codex 데스크톱과 Claude Desktop **Code 탭의 로컬 세션**이다.
Claude Desktop 일반 채팅·Cowork, 원격 세션과 Windows 셸 보호를 검수했다고 주장하지 않는다.

## 정책

프로젝트 안 파일의 생성·수정·삭제·이름 변경·이동·외부 결과 가져오기는 실제로 열린 Experiment
Verify에서만 수행한다. Closing Interview를 포함한 다른 모든 단계, 승인 대기, Step 사이 경계와
종료된 Chain에서는 허용하지 않는다. DOCX·PNG·PDF 제작, 렌더링, 테스트 출력도 파일 변경이다.
프로젝트 밖 작업과 프로젝트 파일을 남기지 않는 웹 조사·기존 증거 읽기는 GIL Step을 요구하지 않는다.
GIL 산출물·제작 스크립트·중간 렌더링은 추적 가능한 프로젝트 내부를 기본 저장 위치로 안내한다.
외부 생성이 꼭 필요하면 실행 전에 이유·경로와 “GIL Snapshot·증거로 추적되지 않는다”는 점을 알린다.
이 안내는 공통 Agent 지침이며 외부 쓰기를 차단하는 훅이 아니다. 프로젝트와 무관한 외부 작업은 계속 허용한다.
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
병합한다. **설정은 새 Host 세션을 열기 전에 준비한다.** 이미 열린 Codex 채팅에 파일만 추가해도
그 채팅이 새 훅을 읽었다고 간주하지 않는다. Codex의 정상 `/hooks` 화면에서 정확한 실행 파일과
root를 검토하고 활성화·Trusted 상태를 확인한 뒤, 같은 폴더의 새 세션에서 실제 도구 호출로 검사한다.
trust 파일이나 해시를 직접 수정하지 않는다. Claude Code 탭도 기존 설정의 훅을 사용하지만 일반
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
- Unix의 기존 잠금 파일은 읽기 전용 descriptor로 열고 **같은 배타 잠금**을 잡는다. `.gil` 쓰기
  권한이 없는 보호 훅도 상태를 읽을 수 있다. 없는 잠금 파일의 생성 및 경로·identity 검증은 유지한다.
- 보호 실행은 macOS `/usr/bin/sandbox-exec` 아래에서 `/bin/sh -c`로 실행한다. Verify 밖에서는
  프로젝트 경로의 `file-write*`를 거절하고, Verify 안에서는 `.gil` 직접 쓰기만 거절한다. 밖의 파일
  쓰기와 읽기에는 추가 제한을 두지 않는다.
- macOS는 이미 제한된 sandbox에서 두 번째 sandbox 적용을 거절한다. Codex의 `exec_command`는
  **원래 명령이 아닌 정확한 GIL 래퍼 전체**에 `sandbox_permissions: require_escalated`와 구체적인
  `justification`을 붙여 정상 Host 승인을 요청한다. 이 한 명령은 Host sandbox 밖에서 GIL sandbox를
  적용한다. 이는 Host의 모든 파일·네트워크 제한을 복제하는 방식이 아니므로 Host 승인 판단은 여전히
  필요하다. 훅은 승인을 자동 허용하거나 입력의 권한 필드를 변경하지 않는다. 승인이 거절되거나
  해당 Host에 승인 실행 경로가 없으면 중단한다. 전역 sandbox 설정을 끄지 않는다.
- 보호 sandbox의 사전 실행 검사가 실패하면 `[guard_sandbox_unavailable]`로 **요청한 명령을 실행하기
  전에** 거절한다. 원래 명령을 보호 없이 재실행하지 않는다. Claude에서도 sandbox를 추가로 사용하는
  설정이라면 같은 플랫폼 제약이 적용되며, 일반 로컬 Code 탭의 검증과 구분한다.
- 정상 foreground 실행이 끝날 때까지 GIL 잠금을 유지하므로 그동안 Verify close는 거절된다.
  쓰기 상태를 읽지 못하거나 잠금이 바쁘면 프로젝트 전체를 쓰기 금지한 보호 셸로 실행한다. 이 경우
  외부 출력·읽기는 가능하고 프로젝트 쓰기는 항상 거절된다. 잠금 없는 writable 실행은 없다.
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

## Closing 회귀 검증

`cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test approved_plan_wire_wave1`
에는 실제 stdio Core와 guard 프로세스로 Opening 승인 대기 → Define/Hypothesis → Verify → 종료 후
경계 → Closing Question/Interpretation/Synthesis → 최종 제안·승인 → Chain 종료까지 검사하는 합성 시험이 있다.
각 경계에서 직접 파일 도구의 훅 판단과 보호 셸의 실제 쓰기 결과를 확인하며, 외부 출력·`.gil` 보호·
Verify 후 기존 파일 불변을 함께 검사한다. `.gil` 읽기 전용 훅과 정상 승인 경로의 보호 셸은 나누어
실행한다. App 승인 입력은 시험 fixture이며 사람의 승인이나 실제 Host 화면 인수로 세지 않는다.

`write_guard` 시험은 상태 잠금 경쟁·손상 시 외부 출력 허용, 프로젝트 쓰기 거절과 중첩 sandbox의
명확한 사전 거절도 검사한다. 실제 Codex/Claude 필드테스트는 이 자동검증과 별도다.

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
