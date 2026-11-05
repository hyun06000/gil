# 프로젝트 파일 쓰기 보호 — 개발용 Host 어댑터

상태: native Core에 포함된 opt-in 어댑터. 훅 활성화는 별도 설치 단계이며, 소스 변경은 기존 설치본을 갱신하지 않는다.
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

## 새 프로젝트 자동 연결 (macOS)

설치된 native Core 하나로 동작한다. Node·Python·개발 checkout이 필요하지 않다. 사용자 요청으로
설치/갱신할 때 다음 명령을 한 번 실행한다. 프로젝트 생성 때마다 설정을 수정하지 않는다.

```sh
/absolute/path/to/gil write-guard install --auto --host both
```

`~/.codex/hooks.json`과 `~/.claude/settings.json`의 기존 항목을 보존해 PreToolUse·PostToolUse·
UserPromptSubmit·Stop 훅을 병합한다. 원본은 `~/.gil-host-guard`의 private 백업에 보존한다.
이 명령은 trust·permissions를 수정하지 않는다. **새 Host 세션에서** 설정을 읽고 Codex의 정상
`/hooks` 검토에서 이 실행 파일을 신뢰해야 한다. Claude Desktop의 **Code 탭**은 Claude Code
훅을 사용한다. 일반 Desktop MCP 채팅에는 이 훅의 강제를 주장하지 않는다.

연결 후 성공한 `gil_start`, `gil_context`, `gil_open`, `gil_close`의 PostToolUse가 실제 Core 상태를
검증하고 canonical root를 등록한다. 선택한 폴더는 채팅 cwd와 달라도 된다. 한글 NFC/NFD는 MCP와
같은 명시적 경로 해석을 쓴다. 실패한 start는 등록하지 않는다. 프로젝트 내부에 `.claude`·`.codex`를
만들지 않으므로 등록이 Artifact/Snapshot을 dirty로 만들지 않는다. 기존 프로젝트도 `gil_context`로
재개하면 등록된다. CLI로만 만든 프로젝트는 Host에서 `gil_context`로 연결한다.

설치 명령은 Host마다 `hook --auto --host codex|claude`를 기록한다. Host는 이 설치 argv로
구분하며 `Bash` 이름·cwd·모델이 준 인자로 추측하지 않는다(Codex도 Bash 이름을 전달한다).
진행 연결 키는 Host + session_id다. 기존 Host 미지정 연결 기록은 그대로 보존하되 새 Host에
상속하지 않는다. 갱신 후 기존 프로젝트는 `gil_context`로 재연결한다. 보호 root 등록은 유지된다.

`gil write-guard status --auto`는 등록 폴더 목록과 현재 Core, Host별 훅 일치 상태를 출력한다.
`matches_current_core`는 네 event의 훅 경로·Host·matcher가 현재 Core와 일치한다는 뜻이며,
`refresh_required`는 구버전·누락·중복 연결을 다시 확인해야 한다는 뜻이다. 등록 사실만으로 Host의 활성화·신뢰를
입증하지 않는다. 설정이 꺼졌거나 Host가 PostToolUse 결과를 전달하지 않으면 자동 연결은 보장되지 않는다.
기존 `gil-write-guard hook|config|run --root …`는 호환용으로 남지만 공통 훅과 중복 설치하지 않는다.

## 채팅으로만 종료하는 경로

- 성공한 GIL 호출로 연결한 **그 채팅**만 Stop 검사한다. 다른 채팅에는 진행 의무를 만들지 않는다.
- Chain이 실제 닫혔으면 종료를 허용한다. 열린 중간 단계·빈 Interview·미등록 승인 제안은 완료가 아니다.
- 사용자 입력이 필요하면 `gil_await_user(project_root, question)`를 호출한다. 실제 열린 Question 또는
  미승인 제안이 등록된 Synthesis만 허용한다. 응답 마지막에 그 질문을 그대로 적어 사용자가 답할 수 있게 한다.
- 대기는 그 상태의 digest에 묶이고 다음 UserPromptSubmit에서 만료한다. 승인이나 Step 변경도 대기를
  무효화한다. 옛 Question을 남겨 둔 채 다음 턴에서 웹 결과만 제시하면 다시 GIL로 돌아가도록 지시한다.
- `gil_await_user`는 Model 전용 읽기 도구다. 사용자 승인·Step 닫기·Domain 기록 변경을 하지 않는다.
- 사용자 중단은 항상 존중한다. `GIL 일시 중지`를 사용자 메시지로 보내면 그 채팅의 진행 검사만 일시 해제한다.
  파일 보호는 유지되고 `gil_context`로 재개한다. 이 명령을 Agent가 완료 우회로 사용하지 않는다.
- 파일 없는 웹 조사는 허용한다. 그것이 요청한 GIL 프로젝트의 최종 완료를 뜻하지는 않는다.
  주제를 바꿔 무관한 작업을 할 때는 별도 채팅 또는 명시적 pause를 쓴다.
- Stop 훅은 Host에게 계속 작업하도록 요청한다. 이미 표시된 답변을 지우거나 사람의 중단·Host의
  반복 상한을 무력화하지 않는다. 모든 모델의 행동을 절대적으로 보장하는 기능은 아니다.

## 여러 폴더 보호

공통 셸 래퍼는 `gil write-guard run --auto -- '명령'`다. 등록된 모든 폴더의 경로만 한 sandbox에서 쓰기 보호하며,
각 폴더의 실제 Verify 상태를 따로 검사한다. 기존 프로세스·다른 앱에는 소급 적용되지 않는다.
등록 정보도 이 셸과 직접 파일 편집 도구로는 수정하지 못하게 한다. Host 설정 갱신은 명시적인
설치 작업으로 수행한다. 사용자 파일·GIL 기록·Snapshot을 자동 이동·삭제·복구하지 않는다.

## 실행 방식

- `Write`, `Edit`, `MultiEdit`, `NotebookEdit`, `apply_patch`: 대상 경로를 해석하고 Verify 상태를
  검사한다. patch의 삭제·이동 원본과 목적지도 검사한다. 프로젝트 밖에서 안으로 향하는 symlink는
  해석해 검사한다. `.gil` 직접 편집과 프로젝트를 포함하는 상위 경로 편집은 거절한다.
- Claude `Bash`: `updatedInput`으로 정확히 인용된 `gil write-guard run --auto -- '원래 명령'`을
  제출하고 `permissionDecision: ask`로 Host 승인을 요청한다. 나머지 입력 필드는 보존한다.
  훅 프로세스가 명령을 대신 실행하지 않는다. 자동 `allow`나 sandbox 해제 필드를 넣지 않는다.
- Codex `Bash` / `exec_command`: 현재 계약은 입력 교체에 `allow`를 요구하고 `ask`를 지원하지
  않으므로 자동 교체를 쓰지 않는다. 정확한 래퍼 재호출과 Codex의 단일 명령 승인을 안내한다.
  Host 미지정 구형 훅에는 Host별 설치 갱신을 안내하며 Codex 승인 필드를 추측하지 않는다.
- 어느 경우도 명령 키워드로 읽기 전용을 판단하거나 외부 cwd·비연결 세션을 무조건 면제하지 않는다.
  경로가 명확한 외부 직접 편집과 웹 조사는 래퍼나 Verify를 요구하지 않는다.
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
  해당 Host에 승인 실행 경로가 없으면 그 셸 호출을 중단한다. 전역 sandbox 설정을 끄지 않는다.
- 보호 sandbox의 사전 실행 검사가 실패하면 `[guard_sandbox_unavailable]`로 **요청한 명령을 실행하기
  전에** 거절한다. 원래 명령을 보호 없이 재실행하지 않는다. Claude에서도 sandbox를 추가로 사용하는
  설정이라면 같은 플랫폼 제약이 적용되며, 일반 로컬 Code 탭의 검증과 구분한다. Claude에서는 지원하는 단일 명령의 sandbox 밖 실행
  승인만 사용할 수 있으며 Codex 필드를 안내하지 않는다. 해당 기능이 없거나 정책으로 금지됐으면
  보호 없는 재실행·전역 설정 해제 없이 한계를 보고한다.
- 정상 foreground 실행이 끝날 때까지 GIL 잠금을 유지하므로 그동안 Verify close는 거절된다.
  쓰기 상태를 읽지 못하거나 잠금이 바쁘면 프로젝트 전체를 쓰기 금지한 보호 셸로 실행한다. 이 경우
  외부 출력·읽기는 가능하고 프로젝트 쓰기는 항상 거절된다. 잠금 없는 writable 실행은 없다.
- GIL 자체 기록·승인·허용된 복구는 기존 MCP 도구로 수행한다. 잠금을 잡은 보호 셸 안에서 GIL
  명령을 중첩 실행하지 않는다. 파일을 밖으로 옮겨 dirty를 해소해도 최초 위반 사실이 없어지지 않는다.

## 세션·파일 보호 경계와 실패 정책

등록 목록은 파일 보호 범위이고 Host + session_id 연결은 채팅 진행 범위다. cwd는 상대 파일 경로
해석에만 쓰며 연결을 만들거나 면제하지 않는다. 성공한 GIL PostToolUse와 검증된 project_root만
연결을 만든다. 별도 채팅, 다른 Host의 동일 ID와 pause에는 Stop 의무가 없다. 다른 프로젝트가
연결된 세션도 모든 등록 root의 파일 보호를 받는다. 정상 연결된 프로젝트 상태가 손상·잠김이면
그 채팅의 완료는 거절한다. 공용 registry 자체가 손상·잠겨 연결 여부를 모르면 Stop/입력에 경고만
남긴다. 무관한 모든 채팅을 막지 않으며 GIL 완료를 확인한 것으로 처리하지 않는다. 이 경우 파일
도구와 셸은 보호 범위를 알 수 없어 거절하며 registry 원본을 보존한다. 무관한 읽기 도구는 통과한다.

**플랫폼 한계:** 현재 훅 입력에는 임의 셸의 실제 쓰기 대상 목록이나 기존 sandbox가 모든 GIL
root를 보호한다는 증명이 없다. 따라서 “모든 일반 셸을 제약 없이 통과”와 “외부 cwd에서 임의
보호 root 쓰기 거절”은 동시에 보장할 수 없다. Claude의 승인 가능한 자동 래핑으로 거절·재시도
순환은 제거하지만 등록 root가 있으면 셸에 경로 보호는 남는다. 이는 모든 세션에 GIL 진행을
강제하는 정책이 아니다. Codex의 명시적 재호출과 중첩 sandbox 불가 Host의 실행 제한은 남으며,
지원하지 않는 Host까지 일반 셸 문제가 완전히 해결됐다고 주장하지 않는다.

## 보장의 한계와 실제 Host 인수

이 어댑터는 **모든 외부 프로세스에 적용하는 OS 접근 통제 제품이 아니다.** 아래 제한 때문에
전체 우회 방지나 두 Host의 실제 적용 완료로 표시하지 않는다.

- 훅이 활성화·신뢰되어야 한다. 훅 호출 자체의 실패·timeout 처리는 Host에 달려 있다.
- 실행 중인 래퍼는 시작할 때의 등록 root 목록을 보호한다. 새로운 프로젝트 연결은 기존 writers가
  끝난 뒤 수행한다. 이후 등록을 이미 실행 중인 프로세스에 소급하지 않는다.
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

공식 근거(2026-11-03 재확인): [Codex Hooks](https://learn.chatgpt.com/docs/hooks),
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
