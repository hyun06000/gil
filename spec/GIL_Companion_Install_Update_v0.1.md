# GIL Companion 설치·업데이트·연결 수명 v0.1

2026-10-26. **제품 결정 / Windows x64 구현 중. 자동 업데이트는 채택하지 않는다.**

현재 코드에는 current-user NSIS, 사용자가 누르는 서명 updater, 고정 Windows 런처, 버전별 Core,
선택 기록과 Codex 연결 버튼이 하나의 x64 제품 후보 경로로 연결돼 있다. 기존 Plugin 후보 manifest는
여전히 Plugin 안의 Core를 직접 실행하므로 제품 경로가 아니다. 서명된 산출물과 clean Windows
종단 인수 전에는 이 문서의 흐름을 지원 완료 기능처럼 설명하지 않는다.

## 1. 목적과 결정

이미 Codex를 사용하는 Windows·Mac 사용자가 명령어·MCP 설정·ACL을 몰라도 GIL을 설치하고
업데이트할 수 있어야 한다. 에이전트의 자기 업데이트보다 사용자가 완료할 수 있는 경로를 먼저 만든다.

- 기본 입구는 공식 다운로드 페이지다. OS/CPU에 맞는 Companion＋고정 실행 진입점＋Core를
  서명된 한 묶음으로 받는다.
- 보조 입구는 Codex 프롬프트다. Agent는 같은 배포물과 버전별 `INSTALL.md`를 사용한다.
- 업데이트의 실행 주체는 사용자용 Companion/설치 프로그램이다. Agent는 안내·상태 조회·결과
  확인만 맡는다. MCP에 설치·prepare·select·승인 대행 도구를 추가하지 않는다.
- Companion은 GIL Project를 실제 사용하는 연결이 있는 동안만 유지한다. 로그인 자동 시작·
  주기적 기상·Codex 종료 후 업데이트 확인은 기본 동작이 아니다.
- 진행 중인 사용자 요청 업데이트만 마지막 연결 종료 뒤 계속한다. 완료 또는 실패를 기록한 뒤
  종료한다. OS 종료·정전까지 막거나 미완료 작업을 성공으로 표시하지 않는다.
- Project, `.gil`, Host plugin cache, 기존 프로세스를 업데이트 편의상 변경·삭제·종료하지 않는다.
- 백그라운드 자동 다운로드·자동 설치·강제 재시작은 하지 않는다. 확인은 알림이고, 설치는
  사용자가 대상 version과 변경 내용을 본 뒤 native 버튼을 누른 별도 행위다.

이 문서는 Runtime Update Contract §3/§6, Design §2/§6, Implementation Plan의
**Agent 실행 업데이트 역할**을 향후 제품에서 대체한다. 기존 상태 판정·불변 버전·무결성·
호환성·실패 보존 계약은 유지한다. 아직 배포된 Plugin 동작을 바꾸었다는 뜻이 아니다.
Distribution 모델의 설치·업데이트 절은 이 결정에 맞춰 함께 갱신한다. Host UI Model의
fullscreen 우선도는 유지한다. 자동으로 띄우는 것은 관리 아이콘이며 Monitor 창을 강제로 열지 않는다.

## 2. 최초 설치의 사용자 흐름

1. 공식 다운로드 페이지에서 지원 OS/CPU, 버전, 시험판 여부, 출처·서명 상태를 확인한다.
2. 설치 프로그램을 다운로드한다. Companion·Core·고정 실행 진입점·필요한 Skill/manifest를
   하나의 검증된 release cohort로 제공한다. 사용자는 Rust/Node/npm을 설치하지 않는다.
3. 설치 프로그램이 새 설치/기존 설치를 구분하고 변경 내용을 표시한다. 기존 설치가 있으면
   조용히 덮어쓰지 않는다. 기존 cache를 실행 위치로 재사용하거나 수동 삭제하지 않는다.
4. 지원이 검증된 공식 Codex 연결 절차를 안내한다. 자동 연결은 실제 지원이 확인된 경우에만
   제공한다. 내부 App Server API나 사용자 `config.toml`의 무단 편집으로 대신하지 않는다.
5. 실제 Core 응답으로 설치 버전·빌드를 확인한다. 파일 존재나 등록 성공만으로 완료하지 않는다.
6. 사용자가 고른 GIL Project를 처음 성공적으로 열 때 관리 아이콘이 나타난다.

Windows의 설치 기준 위치는 사용자 단위의 비동기화 local app data 아래 GIL 전용 영역이다.
Host 등록은 그 영역의 고정 진입점을 가리킨다. Host Plugin cache와 버전별 Core 경로는 등록
대상이 아니다. 정확한 경로와 ACL은 installer 인수에서 확정하되, 구현이 임의 폴더를 검색하거나
Project 아래에 runtime을 두지 않는다.

Codex 연결은 사용자가 Companion에서 확인한 뒤 공식 `codex mcp add` CLI를 호출하도록 구현했다.
`codex.exe` 발견, Codex Desktop 반영과 설치 산출물은 아직 Windows 실기기에서 미검수다. 이 단계가 닫히기 전에는
페이지에 설치 가능 버튼·지원 완료 표시를 게시하지 않는다. 설치 프로그램을 직접 열었을 때의
설치/도움말 창 수명은 작업 중에 한정하며 무기한 상주 서비스로 바꾸지 않는다.

## 3. Companion 수명

MCP 프로세스 존재와 사람이 Project를 보고 있다는 것은 다르다. 기준은 **Project 열기에 성공한
살아 있는 MCP 연결**이다. 단순 server initialize/tools-list는 Companion을 시작하지 않는다.

- OS 사용자별 Companion 하나. 여러 Project/대화/버전 연결을 하나의 관리 아이콘에 등록한다.
- Core는 최초 Project 열기 성공 때 한 번 등록하고 native IPC로 heartbeat를 보낸다.
  모델 호출, MCP tools/call 반복, Project 폴더 polling은 사용하지 않는다.
- 초기 정책: 15초 heartbeat, 60초 미수신 만료, Companion 시작 후 첫 연결 대기 최대 30초.
  시간은 monotonic clock이다. sleep 복귀 때 만료된 연결은 재등록한다.
- 연결 EOF/정상 종료는 즉시 해제한다. 강제 종료는 lease 만료로 처리한다. 만료된 ID의 늦은
  heartbeat는 등록을 되살리지 않는다. PID 이름으로 타 프로세스를 종료하지 않는다.
- 마지막 연결이 끝나면 종료한다. heartbeat 만료 처리 tick 지연까지의 비용은 실제 인수에서 측정한다.
- 업데이트 실행 중에는 종료를 유예한다. 단순 업데이트 알림·적용 확인 대기는 상주 이유가 아니다.
- 사용자 명시 종료는 연결이 있어도 종료하되 진행 중 업데이트는 완료/실패 기록 뒤 종료한다.
  자동 재실행으로 사용자 종료 의도를 뒤집지 않는다. 다음 명시적 Project 사용에서 재시작한다.
- UI 숨김과 서버 종료는 다르다. 다른 Codex 대화를 보고 있어도 기존 연결이 살면 아이콘은 남는다.

IPC 구현은 같은 사용자 인증, 인스턴스별 challenge, 크기/시간 제한, 오래된 응답 거절과
single-instance 종료 경합 처리를 갖춰야 한다. 정책의 숫자 ID는 인증 token이 아니다.
Windows named-pipe와 macOS local IPC는 각각 검증한다. 현행 Unix handshake를 Windows
지원으로 간주하지 않는다. 인증되지 않은 IPC에는 업데이트 실행 권한을 부여하지 않는다.

## 4. 사용자 주도 업데이트

```text
업데이트 안내 → 사용자가 Companion에서 정확한 대상 확인·클릭
→ 다운로드·출처/무결성/호환 검증 → 새 버전 준비·실행 검수
→ 다음 연결용 선택 공개 → 새 연결의 실제 응답 확인
```

- 발견/알림은 설치 승인이 아니다. 시작 버튼의 확인값을 정확한 release version·전체 digest와
  결합한다. 목록 갱신으로 대상이 바뀌면 다시 표시한다. 에이전트 문장이나 `approved:true`로
  native 클릭을 대체하지 않는다. 모델 토큰 없이 확인 가능하다.
- 새 Core는 옆에 준비하고 실행 중 v1을 덮어쓰거나 지우지 않는다. 기존 연결은 v1을 유지한다.
- UI 상태를 `업데이트 있음`, `진행 중`, `다음 연결부터 적용`, `새 연결 적용 확인`, `실패`로 구분한다.
  선택 공개/프로세스 시작/reload 응답을 실제 도구 응답 확인으로 가장하지 않는다.
- 작업별 ID를 부여해 이전 작업의 늦은 완료가 새 작업을 완료시키지 못하게 한다.
- 새 버전 검증과 공개를 수행하는 worker가 내구성 있는 결과를 먼저 기록한 뒤 UI에 완료를 알린다.
  연결이 없으면 UI가 즉시 종료할 수 있기 때문이다. 다음 실행은 이 기록을 읽어 상태를 복원한다.
- worker 실패는 디스크 무변경을 뜻하지 않는다. 선택 공개 여부·잔존물·오류를 별도로 보고한다.
  자동 rollback·ACL 복원·보안 설정 변경·자동 재시도는 하지 않는다.
- 정상 완료 뒤 남은 구버전 정리는 별도 기능이다. 업데이트 성공의 선행 조건으로 삼지 않는다.
- 모든 개발 build를 사용자에게 권하지 않는다. 명시적으로 게시·검수된 채널만 안내한다.
- 정기 확인은 metadata 조회까지만 허용한다. 사용자가 버튼을 누르기 전에는 payload를 받거나
  installer를 실행하지 않는다. 알림을 끄는 설정도 제공한다.

### Core-only와 전체 설치 업데이트

동일 Skill/도구/저장/실행기 계약의 검증된 Core-only 쌍만 다음 연결 선택 방식으로 처리한다.
Companion·실행기·Skill·Host 등록이 바뀌는 판은 **전체 설치 업데이트**로 분기한다. 이 경우
native updater가 설치 프로그램으로 넘기고 필요한 종료/재연결을 설명한다. 실행 중 Companion
자신을 덮어쓰거나 다른 계약의 Core를 먼저 선택하지 않는다. 빈번한 초기 개발에서는 전체 설치
업데이트가 더 자주 필요할 수 있으며 Core-only를 유일한 경로로 가정하지 않는다.

## 5. 신뢰·실행 경계

사용자의 클릭과 OS 실행 권한은 별개다. Companion이 사용자의 앱으로 실행돼도 접근 가능성이
보장되지는 않는다. 거절되면 멈추며 관리자 실행·임의 경로 탐색·상속 해제를 유도하지 않는다.
기존 runtime 계층의 보호 검사를 완화하지 않는다. 이번 Windows ACL 변경 주체는 미확정이다.

publisher 신뢰, HTTPS 출처, digest, 호환성 검증을 구분한다. 서명 키/신뢰 anchor는 개발자가
별도 확보·검수해야 하며 테스트용 키를 제품 신뢰 키로 쓰지 않는다. 아직 내려받지 않은 후보를
descriptor 확인 명목으로 실행하지 않는다. 고정 공식 출처 외 URL/파일 실행 명령은 Agent가
임의로 만들 수 없다. 업데이트 화면은 웹 콘텐츠가 임의 명령을 실행하는 표면이 아니다.

## 6. 구현 상태와 순서

첫 구현은 `src/companion_session.rs`와 단위시험이다. 네트워크·파일·프로세스·UI·IPC와
분리한 공유 정책으로 연결 등록/만료, 마지막 연결 종료, 업데이트 종료 유예, 정확한 대상 확인,
작업 ID, 공개와 새 연결 응답 확인의 분리를 제공한다. 업데이트 worker는 아직 연결하지 않았다.
공개 API의 호출자 역할은 문서 계약이지 인증 구현이 아니다. 이 모듈 성공은 업데이트 성공이 아니다.

### 6.0 현재 제품 배선 판정

| 구성요소 | 코드 존재 | 제품에서 사용 | 판정 |
| --- | --- | --- | --- |
| 연결·업데이트 정책 상태기계 | 예 | 업데이트 worker/UI에서 사용 안 함 | 정책 시험만 완료 |
| 불변 Core 저장·선택·실행 | 예 | production manifest가 호출하지 않음 | testbed/기반 코드 |
| Companion 업데이트 버튼·다운로드 worker | 예 | x64 제품 후보 | 서명 release·Windows E2E 미검수 |
| Windows installer/uninstaller | 예 | x64 제품 후보 | 같은 후보 재실행은 불변 Core를 보존하며 repair, 서명 산출물·실기기 재검수 전 |
| Windows Companion 상태·tray 종단 | 일부 | 설치 경로와 연결 안 됨 | 미검수 |
| 고정 실행 진입점과 버전별 Core | 예 | NSIS 제품 후보 | digest·직전판 폴백·Job Object 구현, Windows E2E 미검수 |
| Codex 공식 등록 | 예 | Companion 버튼 | 공식 CLI 사용, 예전 GIL Plugin/충돌 등록은 사용자 확인 뒤 교체, Desktop 재검수 전 |

위 표가 모두 제품 경로로 연결되기 전에는 “수동 업데이트 지원”이라고 표시하지 않는다.

### 6.1 Mac opt-in 연결 구현

`src/companion_ipc`는 macOS Unix stream을 통해 연결 정책을 실행한다. 같은 effective UID를
양쪽에서 확인하고, 연결마다 OS 난수 challenge, 1 KiB 프레임 상한, 128 peer 상한, handshake
기한, 순서 번호를 사용한다. 원격 연결 ID나 Project 경로·내용을 받지 않으며 업데이트 승인·
설치·선택 명령 자체가 없다. 같은 사용자 악성 코드에 대한 인증 또는 publisher 인증은 아니다.
일반 heartbeat는 native thread에서 15초마다 전송한다. 정상 EOF는 다음 100 ms server tick에서
제거하고, 비정상 연결은 60초 lease로 만료한다. 종료 판정과 새 등록 차단은 같은 mutex 안에서
수행한다. Tauri 종료 전달은 별도 250 ms tick이며 실제 OS 스케줄링 시간 보장은 아니다.

명시적 연결은 MCP `GIL_COMPANION_SESSION_SOCKET`과 Companion `--gil-session-socket <절대경로>`로 켠다.
설치 기록 기반 자동 발견은 §6.2를 따른다. 이것은 사용자 수동 설치 명령이 아니다. Mac endpoint의 부모는 이미 존재하는
canonical 디렉터리이며 현재 UID 소유·mode 0700인지 검사한다. 확장 ACL 검사는 없으므로
이 비트 검사만으로 모든 주체의 접근 부재를 주장하지 않는다. 실제 peer의 UID를 따로 검사한다.
모드를 고치거나 ACL을 복원하지 않는다. 기존 socket도
제거/인계하지 않고 거절하며, 종료 후 endpoint 파일은 호출자 소유 시험 디렉터리에 남는다.
자동 발견에서는 새 세대 endpoint를 만들고 옛 socket은 삭제하지 않는다(§6.2).

MCP initialize/tools-list, help, 실패한 Project 동작은 등록하지 않는다. 성공한 Project action과
monitor prepare 뒤에만 해당 MCP 연결을 등록하고, 도구 실행 횟수와 무관하게 한 lease를 유지한다.
등록/heartbeat는 별도 native 스레드이며 모델 호출을 추가하지 않는다. IPC 실패는 이미 성공한
Project 결과를 되돌리거나 재실행하지 않는다. stderr에 오류 종류만 보고한다. stdio EOF 뒤에는
handler clone이 남아도 lease를 닫고, 뒤늦은 worker가 다시 등록하지 못한다.

Core identity는 현재 executable 경로의 hash와 crate version을 관측한다. 실행 중 매핑된 bytes,
출처·서명 인증의 증거로 세지 않는다. Response 이벤트도 Core의 성공 결과 생성이지 Host가 화면에
표시했거나 응답 수신을 확인했다는 뜻은 아니다. 실제 업데이트 적용 판정과의 연결은 후속 검수다.

Companion의 opt-in 모드는 Monitor 창을 생성하지 않고 tray만 시작한다. 새 모드의 요청으로 기존
legacy 인스턴스가 창을 앞으로 가져오거나 모드를 바꾸지 않는다. 기존 single-instance guard를
유지하며, 최초 30초 안에 등록이 없거나 마지막 lease가 닫히면 종료한다. 로그인 시작 토글은
새 모드에서 비활성이다(기존 사용자 로그인 항목은 변경하지 않음). 수동 실행의 legacy Monitor
동작은 그대로다. Windows named-pipe 구현은 §6.2에 추가했으며 native 실행 증거와 구분한다.

### 6.2 Windows IPC 및 설치된 cohort 자동 시작

`src/companion_ipc/wire.rs`는 두 OS의 동일한 session-only 프로토콜이다. Windows transport는
로컬 `\\.\pipe\gil-companion-v1-<token-user-SID의 SHA-256>`를 사용한다. 새 kernel object에만
사용자·SYSTEM DACL을 지정하고 원격 client를 거절한다. 양방향 pipe peer PID를 조회하고,
process handle을 보유한 채 토큰 사용자 SID 일치와 peer PID 재일치를 확인한다. 접근 불가면
거절하며 권한 확대·impersonation·ACL 복원은 없다. 동일 사용자 악성 프로세스 인증은 아니다.
바이트 모드 PIPE_NOWAIT polling으로 읽기·쓰기·handshake를 제한하고, 최초 server는
FILE_FLAG_FIRST_PIPE_INSTANCE로 이름 충돌을 거절한다. 최대 활성 peer는 기존 128개다.

PIPE_NOWAIT의 ConnectNamedPipe 성공은 연결 완료가 아니라 연결 가능 상태이므로
ERROR_PIPE_CONNECTED만 연결로 취급한다. 이것은 overlapped 비동기 I/O가 아니다.
근거: [ConnectNamedPipe 계약](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe),
[named-pipe 모드](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes).
AppContainer·Codex 제한 토큰의 조회/연결 허용 여부는 미검수이며 실패를 성공으로 바꾸지 않는다.

자동 발견은 **실행 중 Core와 같은 설치 cohort**의 옆 파일 `companion-install.json`만 읽는다.
PATH·레지스트리·Applications 폴더를 검색하거나 다른 위치를 순회하지 않는다. 파일이 없는
기존 Plugin은 자동 실행하지 않는다. installer가 제공할 형식은 다음과 같다(현재 installer 미구현).

```json
{"schema":"gil-companion-install/1","protocol":1,"companion_file":"gil-companion.exe","companion_sha256":"<전체 소문자 SHA-256 64자리>"}
```

Mac의 companion_file은 정확히 `GIL Companion`이다. Core·receipt·Companion executable을
같은 디렉터리에 두는 계약이며, 예를 들어 Mac `.app/Contents/MacOS` cohort에 둘 수 있다.
receipt는 4 KiB 이하·알려진 필드만, filename은 고정 basename·protocol은 1만 허용한다.
링크/reparse·하드링크, digest 불일치, 비 native image는 거절한다. 실행 직전에 보유 파일을
재hash하고 경로 identity를 재확인한다. Windows는 읽기 공유만 허용한 image handle을 spawn까지
유지한다. Unix는 execve 직전 dev/ino를 비교하며 PATH 검색·execvp 셸 fallback을 사용하지 않는다.
Unix의 비교~execve 창·같은 inode 내용 변경, 조상 교체 경쟁은 일반적으로 방어하지 못한다.

이 receipt는 **이미 설치된 cohort의 실행 동의/무결성 계약이지 publisher 인증이 아니다**.
Windows 설치 파일 DACL을 독립 검증하는 새 신뢰 gate가 아니며, 다운로드 파일을 receipt와
함께 만들어 실행해도 된다는 뜻이 아니다. 최초 설치의 서명/출처 승인은 installer 인수 대상이다.
runtime의 별도 선택 버전 폴더에 receipt가 없으면 자동 발견되지 않는다. 향후 Core-only 배포가
이 cohort 계약을 보존하는 방법은 아직 연결하지 않았으며 임의 상위 폴더 탐색으로 대신하지 않는다.

첫 성공 Project 동작에서 기존 endpoint 연결을 우선한다. 새 실행은 사용자별 비대기 launch
잠금(최대 10초 재획득 시도) 아래 다시 확인한 뒤 한 번만 한다. Windows는 Local named mutex,
Mac은 `/private/tmp/gil-cs-<euid>`의 새 0700 디렉터리·0600 flock 파일을 사용한다. 기존 모드가
다르면 복구하지 않는다. Mac은 OS 난수 세대별 socket과 잠금 아래 current 기록을 만들고,
종료된 socket을 unlink하지 않는다. 남은 세대의 GC는 미구현이다. Windows는 OS가 pipe 수명을
관리한다. protocol/인증/권한 오류는 "앱 없음"으로 취급하지 않으며 spawn하지 않는다.

spawn은 고정 session flag와 endpoint만 전달하고 표준 스트림은 null로 둔다. 자신이 시작한
Child만 reaper가 자연 종료를 회수하며, PID 종료·재실행·타 앱 종료는 없다. 시작/등록 실패는
이미 성공한 Project 결과를 되돌리지 않는다. 자동 background reconnect는 없고 이후 성공한
Project 요청에서만 다시 시도한다. 기존 legacy Companion과 single-instance 충돌이 나면
해당 앱의 모드·창을 바꾸지 않고 등록 실패로 끝난다. 실제 tray/GUI 인수는 별도다.
**자동 시작은 Core의 실행 컨텍스트/토큰을 상속하며 sandbox 밖 일반 사용자 권한을 얻는 방법이
아니다.** 화면에서 업데이트를 클릭해도 토큰이 바뀌지 않는다. installer/user가 시작한 기존 앱과
Core가 자동 시작한 앱의 실제 접근 가능성은 서로 구분해 인수해야 한다. 이번 IPC 성공이 기존
Windows ACL 변화나 runtime 저장소 쓰기 문제를 해결했다는 증거는 아니다.

다음 구현 순서는 사용자 완료 경로에 맞춘다.

1. 인증된 IPC + Project 열기/EOF adapter + Companion 단일 인스턴스 수명 연결.
2. 사용자 단위 installer + Host cache 밖 고정 실행 진입점 + 초기 Core cohort.
3. 사용자 업데이트 화면 + 서명된 다운로드/검증 worker의 Windows 설치·취소·실패 E2E와 내구성 결과.
4. 공식 Codex 연결 절차, 웹 다운로드 페이지와 버전별 INSTALL.md 확정.
5. Mac 검수 → Windows CI → 별도 승인한 prerelease → clean 사용자 기기에서 설치·업데이트.

게시 URL, publisher 키, OS별 실제 installer, 자동 Codex 연결, 전체 설치 업데이트 인계는
미완이다. 현재 개발용 `companion/make-app.sh`는 격리 표식 제거/설치 동작이 있어 사용자 경로로
재사용하지 않는다. 기존 설치본의 동작이나 로그인 항목을 이번 구현으로 자동 변경하지 않는다.

## 7. 인수 기준

| 범위 | 합격 조건 | 현재 상태 |
| --- | --- | --- |
| 정책 | 연결 0/1/다수, 종료·만료·지연 heartbeat, clock 역행, 유한 registry | 단위시험 구현 |
| 업데이트 정책 | 알림만으로 실행 안 함, 대상 바뀌면 거절, 중복·오래된 결과 거절, 종료 유예 | 단위시험 구현 |
| 적용 판정 | 기존 연결 v1 유지, 공개는 대기, 공개 후 새 연결의 일치하는 응답만 적용 확인 | 단위시험 구현 |
| IPC/수명 | 양 OS 실제 Core Project 연결→아이콘 하나→마지막 연결 종료→프로세스 종료 | transport/정책 코드와 Mac 합성 검증은 있으나 production 설치·Windows tray 종단 미검수 |
| 업데이트 | native 클릭→서명된 전체 설치→새 Core 응답, 기존 연결·Project 보존 | 코드 구현/Windows E2E 미검수 |
| 최초 설치 | 새 Mac/Windows에서 다운로드·설치·공식 연결, 터미널 없이 사용 | NSIS 구성/고정 Core 연결 미구현 |
| 전체 설치 | Companion/Skill 변경을 Core-only로 오판하지 않음, 종료·인계·재연결 | 미구현/미검수 |

검증 결과는 정책 시험, Core 회귀, Tauri 빌드, 실제 창/트레이, Windows native 실행, 사용자
인수를 각각 기록한다. 하위 단계 통과로 상위 단계를 완료라고 표시하지 않는다.

## 8. 2026-10-25 Mac 연결 adapter 검증

- 실제 소켓 시험 6건: 다중 연결/마지막 EOF, 잘못된·초과·재전송 프레임, handshake 미응답 격리,
  경로·모드 거절과 원본 보존, 명시 종료 뒤 자동 재연결 없음, 만료 lease의 재사용 거절.
- 실제 `gil mcp --serve` 통합시험 1건: initialize/list/실패한 status/help에서 0 연결,
  start 성공 뒤 1 연결, 반복 status에서도 1, 두 번째 MCP에서 2, 각각 EOF 뒤 1→종료.
  기록 바이트 유지와 Companion endpoint 종료 후에도 status 성공을 확인했다.
- Mac default: lib 586 passed/1 ignored, MCP 20 passed. visual-authoring: lib 597 passed/
  2 ignored, MCP 26 passed. `cargo build --locked -p gil --all-targets` 통과.
- Tauri 별도: `cargo check --locked -p gil-companion --tests` 통과, Companion 시험 96 passed.
  실제 tray 표시·단일 인스턴스 GUI 동작은 실행하지 않았다. 설치본과 충돌하는 인수를 만들지 않았다.
- Windows x64: Core lib/tests 두 프로필 교차 컴파일 통과. Windows IPC 구현·실행/CI 증거는 아니다.
- 첫 소켓 시험은 실행 도구 sandbox의 bind EPERM으로 4건 실패(2건 통과); 허용된 격리 소켓
  실행에서 6건 통과했다. 최초 visual 전체 회귀의 기존 Monitor artifact-change 시험 1건은
  시간 내 화면 반영 단언에 실패했다. 관련 제품 코드는 바꾸지 않았고 단독·전체 재실행은 통과했다.
- Companion 첫 실행은 95 passed/1 failed: 기존 시험이 두 번째 실행의 모든 argv 접근을 금지했다.
  새 모드의 고정 flag 확인 한 식만 예외로 단언하고 Project 추측/읽기 금지는 유지한 뒤 96 passed.
- 기존 `src/mcp/actions.rs`의 unused_mut 경고는 이번 변경과 무관하며 그대로다.
- commit·push·CI·게시·설치·Windows PC 변경 없음. 기존 미커밋 문서 변경 보존.

## 9. 2026-10-25 Windows transport·설치 발견 구현 검증

구현은 `companion_ipc/windows.rs`, 공유 `wire.rs`, `companion_discovery.rs`와 OS별
coordination 모듈이다. Windows API feature(Pipes/Cryptography)만 추가했으며 Cargo.lock은
변경하지 않았다. Cargo.toml 고지 정책 digest 3개를 함께 갱신했다. 기존 runtime 저장소의
권한 검사를 바꾸거나 ACL을 복원하지 않았다.

- Mac Core 회귀: default lib 586 passed/1 ignored + MCP 20 passed;
  visual-authoring lib 597 passed/2 ignored + MCP 26 passed. 기존 socket 시험 6개 포함.
- Mac Companion: 96 passed. 실제 설치된 Companion/Tauri GUI는 실행하지 않았다.
- 자동 발견/시작 통합시험: default+runtime-testbed 3/3, visual-authoring+runtime-testbed 3/3.
  동시 두 요청에서 한 번 시작·마지막 연결 종료 경로·다음 요청 새 인스턴스, 미설치/판 불일치/
  hash 불일치/검증 뒤 변경 거절, linked/초과 크기 receipt 거절과 원본 보존을 확인했다.
- `cargo build --locked -p gil --all-targets` 통과.
- 패키징/고지/Windows 패키징 JS: 108 passed/1 skipped. UI/MCP App/Plugin JS: 102 passed/4 skipped.
- Windows x64 `cargo check --locked -p gil --lib --tests --features runtime-testbed`
  및 `runtime-testbed,visual-authoring` 두 조합 통과(신규 경고 없음).
  named-pipe 네이티브 시험 5개(transport 3 + protocol 2)와 합성 설치 앱 시험을 컴파일했지만
  **Windows에서는 실행하지 않았다**. 로컬에 ARM64 Windows target은 없다.
- Windows Companion 전체 교차 check는 **차단**: `cargo check --offline --locked -p gil-companion
  --tests --target x86_64-pc-windows-msvc`가 Tauri build script의 `icons/icon.ico not found`
  오류로 종료했다. 앱 본체 전체의 Windows 컴파일 성공을 주장하지 않는다. 아이콘 생성·설치
  자산 변경·의존성 다운로드로 범위를 넓히지 않았다. Core의 위 교차 check 성공과 별개다.
- 합성 Companion은 feature-gated 테스트베드 사본이며, 각 시험의 독립 cohort/IPC namespace만
  사용한다. 제품의 기본 사용자 IPC namespace, 기존 GIL/Companion 설치·설정·Project에는 접근하지 않는다.
  실제 MCP의 Project 등록/EOF는 명시 endpoint 시험이며, 자동 발견은 별도 합성 설치 앱 시험이다.
  둘의 성공을 실제 Desktop/tray 설치 인수라고 합치지 않는다.
- 최초 sandbox 내 자동 시작 시험은 `Companion exited before registration`으로 1건 실패했다.
  소켓을 허용한 격리 실행에서는 통과했다. 최초 자식 상세 stderr는 수집되지 않아 상세 원인은 확정하지 않는다.
- 추가 거절 시험의 첫 visual 실행은 시간값만 쓴 시험 폴더 이름이 병렬 호출에서 겹쳐
  AlreadyExists로 실패했다. fixture 이름에 프로세스 내 atomic 순번을 추가했고 제품 코드는 바꾸지 않았다.
- 기존 `src/mcp/actions.rs:24`의 Mac unused_mut 경고는 범위 밖으로 유지했다.

다음 검수는 Windows CI의 native pipe·자동 시작 시험과 GUI build다. 그 뒤 별도 승인한
설치 묶음에서 실제 tray·Codex 제한 토큰·legacy 인스턴스 충돌을 확인해야 한다. 현재 installer,
업데이트 버튼/서명 검증 worker는 구현됐지만 고정 Core 진입점과 cohort 배치 연결은 미완이며 이번 변경을 사용자 설치 완료로
표시하지 않는다. commit·push·CI·게시·Windows PC 변경은 하지 않았다.

## 10. Windows 빌드 준비 및 native CI

2026-10-25 후속 승인 범위: 변경 검토·commit·push·CI. 게시·설치·실제 PC 변경은 제외한다.

- 기존 procedural 로고 생성기에서 16/32/48/256 px PNG frame을 가진 `icon.ico`를 생성해
  소스에 포함한다. `--windows-only`는 Mac 아이콘을 바꾸지 않고, `--check-windows`는 저장된
  ICO가 같은 도형 코드에서 생성됐는지 쓰기 없이 비교한다. 새 그림·외부 도구 의존성은 없다.
- Mac에서 Windows Companion 교차 check는 아이콘 부재 단계를 통과한 뒤 `llvm-rc`
  사용 불가(NotAttempted)로 중단했다. 검사 우회 없이 Windows 네이티브 build로 확인한다.
- Windows spike 네 조합에 정책/pipe 시험과 feature-gated `companion_discovery` 시험을
  명시적으로 추가한다. default/visual-authoring 각각 x64/ARM64에서 같은 시험을 실행한다.
- 별도 Companion workflow는 Mac ARM64, Windows x64/ARM64에서 실제 앱 executable을
  build하고 unit test를 실행한다. 앱 GUI를 열거나 설치·게시하지 않으며, testbed 성공을
  tray 표시나 Codex 제한 토큰 환경 인수로 세지 않는다.
- 관련 없는 Distribution 모델·Roadmap·Windows 계약의 기존 미커밋 변경은 커밋에서 제외한다.

## 11. 2026-10-26 portable 검수 묶음 준비 (미게시)

정식 installer 이전에 실제 Core + 실제 Companion tray를 확인하는 별도 review cohort를 준비한다.
`distribution/companion/package-review.mjs`는 같은 빌드의 Core·Companion·feature-gated 검수
드라이버를 PE CPU/hash로 대조해 receipt와 manifest·고지·INSTALL·평문 시작 파일을 조립한다.
Core/MCP 공개 도구나 제품 receipt schema는 바꾸지 않는다. runtime-testbed Core만 명시적인
review namespace 환경 변수를 읽는다. 기본 빌드에는 이 분기가 없다.

Companion은 별도 Tauri identifier/productName으로 빌드한다. 이 identifier일 때만 드라이버가
만든 새 폴더의 설정·readiness 기록을 사용한다. 실제 설치 앱의 설정·single-instance guard·
IPC namespace를 재사용하지 않는다. 일반 제품 app config와 기본 자동 발견 경로는 그대로다.
드라이버는 실제 Core MCP initialize→gil_start 성공→tray 수동 확인→stdin EOF→Core 종료와
보유 Companion 프로세스 핸들 종료→tray 소멸 수동 확인을 구분한다. 강제 종료·재시도는 없다.
사용자가 시작을 승인한 새 시험 Project만 생성한다. 실패 시 Project와 증거를 보존한다.

검수 묶음은 Node/Rust/npm/별도 CLI 다운로드 없이 동작하도록 설계한다. Codex Desktop 데몬·
제한 토큰·공식 Plugin 등록·설치/업데이트·실제 GUI는 이 로컬 준비 시험으로 검증하지 않는다.
기존 GIL/Windows PC는 접근하지 않는다. workflow는 관련 PR 변경 또는 수동 dispatch로 검수 artifact를 만들며
contents:read이고 release/설치 단계는 없다. Windows 빌드·고지 검토·artifact hash 확정·게시 및
실사용자 tray 검수는 별도 실행/승인 단계다. 임의 URL/hash를 안내에 넣지 않는다.
