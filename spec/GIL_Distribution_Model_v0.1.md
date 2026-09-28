# GIL Distribution Model v0.1

> 상태: Draft
> 범위: 비개발자가 플러그인 설치에서 GIL Monitor 사용 가능 상태까지 도달하는 배포·설치 계약
> 비범위: 특정 Store 심사 양식, 서명 서비스 사업자, Host SDK의 구체 API

---

## 1. 목적

GIL의 주 사용자는 터미널과 패키지 관리자를 다루는 개발자가 아니다. 기본 인간 표면은 Codex와
Claude Desktop의 **Cowork** 모드다. 사용자는 이들
같은 Agent Host에서 Agent와 협업하고, GIL의 복잡한 버전 Graph는 인간용 Monitor로
이해한다.

따라서 다음 상태는 완전한 설치가 아니다.

- 플러그인은 설치됐지만 인간이 Monitor를 열 수 없음
- inline 카드만 있어 대화가 길어지면 Monitor가 밀려남
- 사용자가 terminal command, port 또는 capability URL을 기억해야 함
- Companion 설치 뒤 사용자가 대화로 돌아와 설치 완료를 다시 설명해야 함

> **GIL 설치는 Agent가 GIL을 호출할 수 있고, 인간이 지속형 Monitor를 열 수 있을 때 완료된다.**

---

## 2. 설치 완료 조건

```text
GilInstallation
├─ agent_surface       ready | unavailable
└─ monitor_surface     persistent_host | native_companion | unavailable
```

- `agent_surface=ready` — 현재 Host에서 GIL의 typed action과 Manual을 사용할 수 있다.
- `monitor_surface=persistent_host` — Host가 실제로 지속되는 panel 또는 PiP에 공용 Monitor UI를
  표시한다. MCP `fullscreen`도 실제로 채팅과 병행되는 지속 표면이면 이 후보에 포함된다.
- `monitor_surface=native_companion` — 설치된 GIL Companion이 같은 UI를 지속형 native window로
  표시한다.
- `monitor_surface=unavailable` — inline 미리보기나 text 출력만 가능한 상태다.

`agent_surface=ready`이면서 Monitor surface 둘 중 하나가 있어야 설치 완료다. inline UI, browser
fallback과 CLI text는 진단·복구 수단이며 지속형 Monitor를 대신하지 않는다.

---

## 3. 표면 선택 순서

2026-09-23 결정: **MCP App fullscreen을 기본 Monitor UX로 옮기고 Companion을 유지한다.**
Codex는 그대로 사용하고 Claude Desktop에서는 Cowork를 안내한다. 같은 Desktop의 Code 모드와
Cowork를 같은 capability로 취급하지 않는다. 이 날 macOS V3 진단판 실측은 Codex/Cowork에서
`inline + fullscreen`, Code에서 `inline`이었다. PiP는 셋 모두 광고하지 않았다.
fullscreen은 OS always-on-top/PiP라는 뜻이 아니며, 창의 실제 배치는 Host가 정한다.

```text
Plugin installed
  → persistent Host surface를 실제로 요청하고 결과를 확인
      ├─ persistent_host 확인됨 → 같은 Host에서 Monitor를 연다
      └─ 지원하지 않음·inline에 머묾·수명 계약 미달
           → 지원 표면(Codex / Claude Desktop Cowork) 또는 Companion을 안내
               └─ 사용자가 Companion을 선택하면 설치 상태를 확인
                   ├─ 호환판 설치됨 → 열거나 앞으로 가져온다
                   ├─ 설치됐으나 낡음 → update를 제안한다
                   └─ 없음 → 신뢰된 설치 경로를 제안한다
```

기능 이름이나 요청 성공만으로 PiP 지원을 추측하지 않는다. adapter는 실제 display mode와 수명
계약을 확인한다. PiP 요청 뒤 `inline`이면 `persistent_host`가 아니다.

PiP capability 판정은 실제 `ui://` MCP App에서 수행한다. 앱은 `ui/initialize`의
`appCapabilities.availableDisplayModes`에 자신이 지원하는 mode를 선언하고, adapter는 Host context가
광고한 목록, 사용자 동작에 묶인 mode 요청의 원문 반환값, mode 변경 event와 실제 surface 수명을
함께 기록한다. 이 네 증거가 없는 inline 진단은 Host의 PiP 지원 여부를 확정하지 못한다.

MCP fullscreen과 Companion은 별도 Monitor 제품이 아니다. 둘은 같은 `MonitorViewV1`, `NodeDetailV1`, 공용
UI bundle과 presentation 규칙을 사용한다. 바뀌는 것은 수명과 packaging을 맡는 Host뿐이다.

### 3.1 MCP App 우선 진행과 Companion 검증 유예 (2026-09-28)

새 Rust 설치본의 macOS Codex fullscreen 화면을 사용자가 확인했다. 현재 개발·인수의 우선 경로는
**Plugin 하나 → 로컬 Rust MCP/Core → 내장 MCP App → 가로 DAG·보고서·Host 채팅**이다.
MCP App이 검증된 Host에서 Companion 설치·실행이나 Tauri 전용 회귀시험 완료를 이 경로의
선행 조건으로 두지 않는다. 이는 현재 단계의 진행 기준이지 모든 Host·OS의 배포 완료 선언이 아니다.

Tauri 전용 창·tray·autostart·native packaging 검증은 후속으로 미룬다. 기존 Companion 코드·
설치물·설정과 명시적 fallback 도구는 보존하며, MCP App 실패를 이유로 자동 실행하지 않는다.
공용 UI·read model·Project 보존 회귀시험은 계속 수행한다. Companion 구현을 다시 변경하거나
새 Companion 배포물을 제공할 때는 영향을 받는 Tauri 검증을 재개한다.

현재 진행 순서는 MCP App의 작업 전환·재시작 수명 → Cowork Plugin 설치 경로 → self-contained
Plugin의 설치·업데이트 인수다. MCP 실행 파일의 서명·공증과 새 기계 설치 검수는 별도 배포 게이트로
남으며, Tauri 검증 유예가 그것을 면제하지 않는다.

---

## 4. 지속형 Host surface의 최소 계약

Native Companion을 생략하려면 Host surface가 다음을 모두 만족해야 한다.

1. 대화가 길어져도 메시지와 함께 밀려나지 않는다.
2. 사용자가 명시적으로 닫기 전까지 유지된다.
3. 다른 대화나 작업을 보고 돌아와도 같은 Project scope를 복구한다.
4. DAG 선택·접기·상세 Report interaction을 지원한다.
5. 완전한 `MonitorViewV1` 갱신을 받을 수 있다.
6. 현재 Project를 명시적으로 식별하며 cwd나 최근 폴더를 추측하지 않는다.
7. macOS와 Windows에서 의미상 같은 수명 계약을 제공한다.
8. 지원하지 않거나 전환에 실패하면 그 사실을 adapter가 판별할 수 있다.

하나라도 확인되지 않으면 **Companion을 완전히 제거하거나 모든 환경에서 불필요하다고 판정하지 않는다.**
이미 확인된 Host의 fullscreen에는 Companion 설치를 강제하지 않는다. 미지원·실패·독립 창 요청에는
Companion을 대안으로 안내하고 사용자가 선택한 경우에만 연다.
특히 Windows 및 작업 전환·재시작 수명은 이번 macOS 진단판 성공만으로 통과 처리하지 않는다.

MCP Monitor의 명시적 Project 선택은 Plugin cache와 독립된 사용자 전용 로컬 설정으로 보존한다.
서버 재시작 시 해당 화면의 scope만 검증·복원하며, 정상 재시작마다 수동 재연결을 요구하지 않는다.
Project 데이터·보고서는 이 설정에 복제하지 않는다. 설정의 OS identity·원자적 저장·실패 경계는
Host UI Model §9.2a가 소유한다. 업데이트 이전의 미저장 화면은 한 번 재열기가 필요하다.
자동 복원 구현을 Host fullscreen 위치나 선택·접힘의 재시작 복원까지 통과한 증거로 삼지 않는다.

---

## 5. Companion 설치 상태

```text
missing       설치되지 않았다
stopped       호환판이 설치됐지만 실행 중이 아니다
outdated      설치됐지만 Plugin·wire 계약과 호환되지 않는다
ready         호환판이 설치됐고 열 수 있다
```

경로 존재 여부 하나로 판정하지 않는다. 서명된 app identity, protocol handshake와 호환 version을
사용한다. `ready`가 아니면 기존 Project를 임의로 열거나 오래된 wire를 해석하지 않는다.

### 5.1 Handshake v1

설치된 Companion binary는 `--gil-companion-handshake`에 Project·창·Tauri runtime을 열지 않고
compact JSON 하나로 답한다.

```json
{"schema_version":1,"product":"gil_companion","bundle_id":"dev.ariadne.gil.companion","app_version":"0.1.0","protocol":{"min":1,"max":1},"monitor_view_schema":{"min":1,"max":1},"node_detail_schema":{"min":1,"max":1}}
```

경로·PID·Project scope·사용자 이름은 싣지 않는다. launcher가 요구하는 protocol과 두 wire schema가
모두 응답 범위 안에 있고 identity가 정확히 같아야 호환판이다. 응답이 없거나 모양·identity·범위가
다르면 실행 중처럼 보여도 `outdated`로 판정한다.

```text
bundle 없음                                  → missing
bundle 있음 + handshake 없음·불일치          → outdated
호환 handshake + 실행 중 아님                → stopped
호환 handshake + 실행 중 protocol challenge  → ready
```

binary descriptor가 identity와 호환 범위를 밝히고, 실행 중인 process는 사용자 전용 local IPC에서
매번 새 challenge를 그대로 돌려준다. `ready`는 두 응답이 같은 descriptor를 말할 때만 성립하며,
PID·process 이름만으로 대신하지 않는다. macOS의 첫 transport는 권한 `0600` Unix socket이고
포트·URL을 만들지 않는다. Windows transport는 달라도 challenge와 응답의 의미는 같아야 한다.

### 5.2 Monitor 요청의 조율

「GIL Monitor 열기」 하나를 끝까지 책임지는 자리는 하나다. 표면 선택과 Companion 상태를 따로
물으면 두 답이 어긋나고, 어긋난 자리에서 사람이 같은 말을 두 번 하게 된다.

조율의 결과는 값으로 가른다. 문장을 뜯어 뜻을 짐작하지 않는다.

```text
opened_persistent_host         확인된 지속형 Host surface에 열었다
focused_existing_companion     이미 열려 있던 창을 앞으로 가져왔다
started_and_opened_companion   꺼져 있던 것을 실행하고 handshake를 확인한 뒤 열었다
needs_companion_install        설치가 필요하다 — 승인 없이 설치하지 않는다
needs_companion_update         호환되지 않는 판이 있다 — 그 판을 열지 않는다
monitor_unavailable            지금은 어느 표면도 열 수 없다
```

`stopped`에서는 실행 명령의 성공을 완료로 삼지 않는다. 실행한 뒤 **새 challenge로 다시 확인**하고,
`ready`가 된 뒤에야 원래 요청을 이어서 수행한다. 확인되지 않으면 `monitor_unavailable`이며 원래
요청을 성공으로 표시하지 않는다.

재확인은 유한하다. 무한 polling이나 background busy loop를 만들지 않는다.

원래 요청은 **그 요청이 살아 있는 동안 메모리에만** 둔다. Graph·Journey·Memory·Will·Project 어디에도
적지 않는다. 적어 두면 다음 실행이 사람이 지금 원하지 않는 창을 열 수 있다.

Host surface는 `verified`와 `unverified` 둘로만 적는다. `unverified`는 미지원이라는 뜻이 아니라
§3이 요구한 네 증거로 아직 확인하지 않았다는 뜻이다. 어느 쪽이든 판정은 같다 — 확인되지 않았으면
`persistent_host`로 세지 않는다. 확인하지 못한 것을 미지원으로 단정하지도 않는다.

사용자에게 가는 문장에는 경로·PID·socket·port가 없다. 실패한 OS 명령의 오류 문구를 그대로 잇지
않는다. 그 안에 경로가 들어 있다.

### 5.3 Agent Core 의 자리

Plugin 과 Companion 은 **다른 것을 소유한다.**

```text
Plugin      Agent 용 GIL Core · MCP bridge · Manual/Skill · Companion coordinator
Companion   읽기 전용 Monitor · DAG·Report UI · Project watcher · 지속형 창
```

Companion과 MCP App에 Agent의 write 명령을 넣지 않는다. Plugin은 Host가 그릴 `ui://` resource를
제공할 수 있지만 자체 native 창 수명이나 두 번째 DAG renderer를 소유하지 않는다.

Agent Core 의 정본은 **Rust GIL** 이다. 옛 Go 도구는 packaging 대상이 아니다.

Plugin 은 platform 별 Core 실행 파일을 자기 안에 싣는다. 설치본만으로 서야 하며 저장소·Cargo·
전역 `gil` 에 기대지 않는다. 실린 파일이 거기 있다는 사실은 증거가 아니다 — **실행해 보고**
identity·protocol·action surface·platform 이 맞을 때만 쓴다.

```text
Core 없음 · 실행 불가 · 다른 기계        → unavailable
identity 불일치                          → unavailable
protocol·action surface 범위 불일치      → outdated_agent
fresh challenge 에 답하고 범위가 맞음    → ready
```

저장 format 범위는 descriptor 가 싣고 나르되 Plugin 이 문지기로 쓰지 않는다. 그것은 Core 가
Project 를 열 때 스스로 거절할 일이고, Plugin 이 따라 적으면 판정이 두 자리에 살게 된다.

`agent_surface` 는 이 probe 에서 **유도된다.** 호출자가 넘긴 값이나 상수로 정하지 않으며,
Monitor 판정과 서로 독립이다 — 한쪽이 없어도 다른 쪽은 그대로 돈다.

#### 경계의 모양

```text
typed MCP 인자
→ 공유 command 모듈에 명시적 root·본문 전달
→ 같은 프로세스의 Rust GIL domain 호출
→ Result로 성패 판정
→ 기존 ok·exit_code·said·problem 응답과 산문 유지
```

tool 하나가 허용된 domain command 하나에만 대응한다. 자유 형식 command 를 받는 문은 만들지
않으며, shell·자기 실행 파일 재호출·chdir를 쓰지 않는다. protocol stdin을 Report 입력으로
읽지 않는다. Core 산문을 파싱해 domain 상태로 해석하지 않으며, 기존 JS의 고정 argv·stdin·
종료 코드 경계와 응답은 동등성 시험으로 보존한다. 실제 CLI stdout/stderr는 얇은 binary가 맡는다.

Core action 의 stdout 은 **아직 공개 JSON 계약이 아니다.** typed 인 것은 MCP 입력과 handshake 다.

Project 자리는 Host 가 검증한 workspace 나 사람이 명시한 scope 에서만 온다. cwd·최근 폴더·
Companion 이 보여 주는 선택을 Agent action 의 대상으로 짐작하지 않는다 — 사람이 보는 것과
Agent 가 고치는 것이 달라도 되어야 한다.

### 5.4 하나의 MCP, 두 Host Plugin

GIL Agent surface의 실행 계약은 **하나의 로컬 Rust MCP server**다. Core, 열일곱 tool의
이름·입력·출력, 공용 UI resource, Bootstrap Capsule, Manual Topic과 Companion availability protocol을 Host마다
다시 구현하지 않는다.

```text
공통 GIL MCP
├─ Rust Core · 공유 command/renderer
├─ Rust MCP · Agent 10 + Companion 2 + Monitor 5 tools
├─ 내장 MCP App · 공용 DAG/Report UI
├─ Bootstrap · Skill · Manual
├─ Companion coordinator (명시적 fallback)
└─ Host adapter manifests
    ├─ Codex Plugin adapter
    └─ Claude Code Plugin adapter
```

Codex와 Claude Code의 차이는 **설치 manifest와 Host가 요구하는 경로 변수**뿐이다. Codex adapter는
`.codex-plugin/plugin.json`, Claude Code adapter는 `.claude-plugin/plugin.json`을 가지며, 둘 다
같은 `skills/`, server source와 platform Core를 싣는다. MCP 등록 JSON의 외형이 Host마다 다르면
얇은 adapter나 한 정본에서 만든 생성물로 두되, 한쪽의 server·tool 표·안내문을 복사해 두 번째
정본으로 만들지 않는다.

사용자에게 두 adapter는 모두 **GIL Plugin**이다. MCP namespace, sidecar, bridge와 package 확장자는
진단 문서의 말이지 정상 설치 UX의 말이 아니다. 두 Host에서 설치 뒤 보이는 명령의 의미, 거절,
다음 행동, Monitor 요청과 degraded mode가 같아야 한다.

**tool namespace 는 Host 내부 사실이며 공통 사용자 계약이 아니다.** 두 Host 가 같은 prefix 를
쓴다고 전제하지 않는다. 실측한 Claude Code 의 자리는 설치 식별자와 server 이름을 함께 엮은
`mcp__plugin_gil-companion-prototype_gil-companion__*` 이고, Codex 는 자기 규칙으로 다르게 짓는다.
동등성은 prefix 의 같음이 아니라 **tool 의 이름·입력·출력·거절·다음 행동의 같음**으로 정의한다.
문서·Skill·Manual 은 prefix 를 사용자 계약으로 적지 않으며, 시험도 prefix 문자열로 동등성을
판정하지 않는다.

---

## 6. AI가 조율하는 설치

AI는 설치를 수행하는 주체가 아니라 설치 과정을 끝까지 조율하는 주체다.

```text
감지 → 이유 설명 → 사용자 승인 → OS의 신뢰된 설치 표면 → 재감지 → 원래 요청 재개
```

- 사용자의 명시적 승인 없이 native binary를 내려받거나 실행하지 않는다.
- OS의 서명·공증·Store 확인을 우회하지 않는다.
- 설치 전에 제품명, publisher, 필요한 이유와 로컬 접근 범위를 짧게 설명한다.
- macOS에서는 Mac App Store 또는 Developer ID로 서명·공증된 배포물을 사용한다.
- Windows에서는 Microsoft Store 또는 서명된 installer를 사용한다.
- 설치 완료를 사용자가 채팅으로 다시 보고하게 하지 않는다. adapter가 재감지하고 원래 요청을
  자동으로 재개한다.
- 설치·업데이트가 실패하면 Project와 `.gil`을 바꾸지 않고 text 사용과 재시도 경로를 제공한다.
- 관리자 권한을 기본 전제로 두지 않는다. 꼭 필요하면 설치 전에 이유를 밝힌다.

“알아서 설치한다”는 OS 승인과 사용자 의사를 우회한다는 뜻이 아니다. 사용자가 download folder,
terminal, JSON 설정과 실행 파일 위치를 다루지 않아도 된다는 뜻이다.

### 6.1 macOS 배포물의 상태

배포물은 **이름으로** 상태를 말한다. 서명되지 않은 것을 배포 자리에 두지 않는다.

```text
development                 개발용 bundle — 격리 표식을 손으로 뗀다. 배포 불가
release_unsigned            packaging 확인용. 배포 불가
release_signed_unnotarized  공증 전. 배포 불가
release_signed_notarized    사용자에게 줄 수 있는 것
```

Native Companion의 최초 설치는 Developer ID Application 으로 서명하고 공증한 DMG 하나다. 업데이트는 장차 별도
서명 artifact 를 쓰며, Mac App Store 와 PKG 는 이 판의 범위가 아니다.

hardened runtime 을 켜고 **권한은 요구하지 않는다.** 편의로 넣은 권한은 쓰지 않아도 공격면이고
심사에서 근거를 대야 하는 빚이다. `get-task-allow` 만 명시적으로 거짓이다. App Sandbox 는
Developer ID 직접 배포에 필수가 아니므로 켜지 않으며, Store 심사는 별개로 판단한다.

서명 identity 와 공증 credential 은 **환경에서만** 온다. 설정 파일·저장소·로그·산출물 어디에도
값이 남지 않는다. 공증은 계정 비밀번호보다 App Store Connect API key 를 먼저 쓴다 — 권한이 좁고
회수가 사람 계정과 무관하기 때문이다.

배포물에는 제품 화면만 싣는다. 시험 장치와 fixture Host 는 제품 안에 가짜 사실을 만들 수 있는
문이므로 싣지 않는다. 의존성 source 경로는 remap 해서 만든 사람의 home 이 binary 에 남지 않게
한다.

지원한다고 적는 architecture 는 **실제로 지어 확인한 것**뿐이다. 확인하지 않은 판을 목록에
올리지 않는다.

### 6.2 Codex native Plugin의 배포 준비 (2026-09-28)

Companion DMG와 Rust MCP Plugin은 서로 다른 배포물이다. Plugin은 Host manifest·공용 Skill·
Core와 MCP App을 내장한 실행 파일·license만 싣는 built-dist tree를 만든다. catalog의 `source.path`는
그 tree root에서 상대경로로 풀고 source checkout이나 설치 시 빌드를 가리키지 않는다.

`distribution/codex/`는 macOS arm64의 **미등록 preview 생성·검증 도구**다. 기존 identity/version은
보존하고, 파일 allowlist·mode·SHA-256·source snapshot을 receipt에 기록한다. dirty source도 preview
증거에는 표시하지만 공개 release로 취급하지 않는다. 압축 왕복 뒤 Node 없는 PATH로 native
challenge·17 tools·embedded UI bytes를 확인한다. 이것은 Host 설치나 화면 인수를 대신하지 않는다.

현재 pipeline은 `development_unsigned`, `publishable: false`만 만들고 게시 명령은 거절한다.
서명·공증·새 기계 첫 실행·marketplace 설치 gate는 열린 채다. 첫 실행 timeout을 자동 재시도로
감추지 않는다. Actions workflow는 수동 실행·읽기 권한만 가지며, 검수 artifact 보관과 정식
배포 채널 게시를 구분한다. 원격 저장소 생성·공개 게시·서명 자격 증명 사용은 별도 승인 후 진행한다.

Git-backed marketplace와 public Plugins Directory 등재는 서로 다르다. 최종 사용자 UX는 계속
Plugin UI의 설치 한 번이며, preview용 Node·Cargo·압축 명령을 사용자에게 요구하지 않는다.
CLI 바이너리·archive의 Apple 신뢰 검수는 별도로 확정하고 Companion DMG의 stapling 성공을
Plugin 실행 파일의 서명/공증 성공으로 대체하지 않는다.

이전 source에서 원격 runner 검수를 수행했지만 새 `gil` 저장소의 CI 성공으로 간주하지 않는다.
[source 이전 경계](../SOURCE-MIGRATION.md)에 새 검증을 구분한다. 원격 runner의 빌드·압축 왕복·
17 tools·내장 UI bytes 검사는 새 사용자 기계의 설치 성공이나 공개 release/marketplace 게시와 다르다.

### 6.3 정식 배포 전 오픈소스 공개 준비 게이트

GIL은 GitHub 소스를 공개하는 오픈소스 프로젝트로 배포한다. 새 정본 `hyun06000/gil`은
2026-09-28 비공개 빈 저장소로 생성했다. 기존 MIT LICENSE와 저작권 표기는 유지하며,
source 검수 후 공개 전환과 정식 사용자 배포를 각각 승인받는다. 이전 저장소의 공개 이력은
복사하지 않으며 새 저장소 생성이 과거 공개 기록을 제거하는 조치는 아니다.

정식 release·사용자용 marketplace 게시 전에는 [Open Source Readiness](GIL_Open_Source_Readiness_v0.1.md)의
게이트를 반드시 닫는다. 전체 Git 이력/공개 refs의 민감정보, 배포 의존성의 라이선스·고지,
기여·보안 안내, 재현 가능한 build/검수 증거를 확인한다. 현재 working tree에 패턴 검색을 한 것을
전체 이력 감사 또는 제3자 라이선스 준수의 증거로 대체하지 않는다.

Public 저장소의 push·CI 로그·artifact 공유도 외부 공개다. 개발 preview 표시는 접근 통제가
아니므로 원격 작업 전에 포함 범위와 노출 정보를 검수한다. 정식 출시 승인, 저장소 visibility
변경, 공개 이력 재작성, 보안 설정 변경은 각각 별도 결정이며 이 문서를 썼다고 실행하지 않는다.

---

## 7. 비개발자용 설치 UX

정상 경로에서 사용자가 알아야 하는 공개 개념은 둘뿐이다.

```text
GIL Plugin       Agent가 GIL을 사용하게 한다
GIL Monitor      사람이 여정을 보게 한다
```

Companion은 Host가 지속형 Monitor를 제공하지 못할 때만 이름을 드러낸다. 설치 UI는 한 화면에서
다음 셋만 제공한다.

- `Monitor 설치`
- `지금은 text로 계속`
- `무엇이 설치되는지 보기`

설치 완료 뒤에는 “GIL Monitor를 사용할 준비가 됐다. 현재 Project의 여정을 열었다.”라고 답한다.
사용자에게 port, localhost URL, binary path, package format 또는 MCP transport를 보여 주지 않는다.

---

## 8. Host별 packaging

### 8.1 Codex와 Claude Code

현재 Agent 배포 adapter는 Codex Plugin과 Claude Code Plugin이다. 인간 Monitor의 권장 표면은
Codex와 Claude Desktop Cowork로 정한다. **Code Plugin 도구 검수와 Cowork MCP Apps 검수는 별개**다.
Cowork에서 같은 설치 단위가 실제 tool과 resource를 노출하는지는 따로 검수하며 Desktop 직접
등록 probe의 성공을 Plugin 배포 성공으로 대체하지 않는다. 두 Plugin은 각 Host의 Plugin UI에서
설치되며 Skill과 공통 MCP server를 한 설치 단위로 제공한다. 사용자는 runtime을 설치하거나 JSON을
편집하거나 server command를 등록하지 않는다.

Plugin은 platform Core와 server 의존성을 self-contained하게 싣는다. 설치 cache의 위치가 달라도
Host가 제공하는 Plugin root에서만 상대경로를 해석하며 개발자의 저장소·home·전역 `gil`에 기대지
않는다.

설치 경로는 둘이며 **같은 자리가 아니다.**

```text
local Plugin upload    개발 인수 경로 — 지은 묶음을 Host 에 직접 올린다. 받는 쪽도 개발자다
remote marketplace     사용자 배포 경로 — self-contained artifact 와 release pipeline 이 선다
```

source clone 은 어느 쪽에서도 비개발자 설치의 완성본이 아니다. `make-core.sh` 와 Rust·cargo 가
필요하기 때문이다. Desktop UI 에서 로컬 디렉터리를 marketplace 로 더할 수 있다고 전제하지
않는다 — 확인된 개발 경로는 local Plugin upload 하나다.

현재 개발 manifest는 함께 실은 Rust 실행 파일의 `mcp --serve`만 시작한다. 기존 Agent/Companion
12개와 Monitor 5개, 공용 UI resource가 한 프로세스에 있고 HTML도 binary에 내장된다.
Node·npm은 개발 시험과 bundle/zip 생성에만 필요하며 native 설치물의 실행 의존성이 아니다.
macOS arm64에서 Node 없는 PATH의 실제 MCP 동작을 확인하되, 이를 개발 도구 없는 새 기계의
설치 UX·Developer ID 서명·공증·원격 marketplace·Windows 검수 성공으로 일반화하지 않는다.

독립 창, tray, 로그인 시 시작과 Agent session 밖의 수명을 bundle Host가 보장하지 않으면
Companion은 별도 native surface로 남는다.

### 8.2 MCPB와 일반 Claude Desktop

MCPB는 별도의 GIL 구현이 아니라 **같은 MCP server를 Claude Desktop Extension으로 포장하는 후속
adapter**다. Claude Code Plugin 경로가 닫히기 전에는 제품의 필수 배포물이 아니며, Node probe나
MCPB manifest를 현재 설치 완료 조건으로 세지 않는다.

일반 Claude Desktop 대화, Extension Directory 또는 파일 더블클릭 설치를 지원할 필요가 확인되면
MCPB를 추가한다. 그때도 Rust Core·bridge·tool·Manual을 복제하지 않고 공통 Plugin 정본에서
package를 만든다. MCPB의 유무는 Codex와 Claude Code의 기능 동등성을 바꾸지 않는다.

### 8.3 원격 MCP가 기본인 Host

공개 HTTPS MCP만 배포할 수 있는 Host에서는 remote plugin이 Companion의 존재·호환성을 조율한다.
remote server가 사용자의 로컬 Project 파일을 대신 보관하거나 읽지 않는다. 로컬 Project 접근은
사용자 장치의 검증된 GIL Core와 Companion 경계에 남는다.

### 8.4 공용 계약

어느 Host에서도 GIL domain state, 저장 schema, `MonitorViewV1`, `NodeDetailV1`, 공용 UI bundle,
설치 상태의 의미와 사용자 승인 경계는 같다.

---

## 9. 업데이트와 복구

- Plugin, GIL Core, Companion과 wire schema의 호환 범위를 명시한다.
- `outdated`를 `missing`으로 말하지 않는다.
- 자동 업데이트는 서명된 동일 publisher의 배포물만 받는다.
- update 실패 시 마지막 검증 View와 Project 기록을 보존한다.
- 새 판이 저장 migration을 요구하면 rollback 가능 여부를 설치 전에 밝힌다.
- 제거는 Companion 설정과 설치물만 지우며 사용자의 Project와 `.gil`을 지우지 않는다.

---

## 10. 검증 가능한 불변식

1. terminal 없이 Plugin 설치에서 Monitor 열기까지 도달한다.
2. inline UI만 가능한 상태를 설치 완료로 판정하지 않는다.
3. fullscreen/PiP 요청 성공이 아니라 실제 persistent mode를 확인한다.
   정식 판정은 앱 선언·Host 광고·요청 반환값·mode 변경 event를 함께 보존한다.
4. MCP fullscreen과 Companion이 같은 fixture에서 같은 사실을 보여 준다.
5. Companion 없음·꺼짐·낡음·호환됨을 서로 다르게 판정한다.
6. 사용자 승인 없이 native binary를 설치하거나 실행하지 않는다.
7. 설치 완료 뒤 같은 대화에서 원래 요청을 자동 재개한다.
8. 설치·업데이트 거절과 실패 전후 Project bytes가 같다.
9. Companion 없이도 Agent의 text loop와 GIL 기록은 동작한다.
10. macOS와 Windows의 clean machine에서 설치·업데이트·제거를 재현한다.
11. 처음 보는 비개발자가 설명서 없이 한 문장 요청으로 3분 안에 Monitor를 연다.
12. Codex와 Claude Code에서 같은 인수 시나리오가 같은 GIL 사실과 다음 행동을 만든다.
13. Host별 Plugin을 제거하면 Agent surface만 사라지고 Companion과 Project 기록은 남는다.
14. 정식 release·사용자용 marketplace 게시 전에 오픈소스 공개 준비 게이트를 닫는다.
    소스가 Public이라는 사실만으로 비개발자 설치·보안·라이선스 검수가 끝났다고 판정하지 않는다.

---

## 11. 아직 정하지 않는 것

- 어떤 Agent Host가 언제 persistent PiP를 공개 지원하는가
- Mac App Store sandbox build와 Developer ID direct build 중 최종 기본 채널
- Windows Store package와 별도 signed installer 중 최종 기본 채널
- public Plugin directory의 심사 일정과 국가별 출시 순서
- 일반 Claude Desktop용 MCPB adapter의 구현·출시 시점
- remote marketplace artifact 의 호스팅 자리와 release pipeline 의 서명 단계
- Host 사이에서 Companion 설치 상태를 공유하는 방식
- remote MCP와 로컬 Companion 사이의 pairing protocol

---

## 12. 핵심 문장

> **GIL Monitor는 선택 기능이 아니다. Agent surface와 지속형 인간 Monitor가 모두 있어야 설치가
> 완료된다.**

> **persistent Host surface를 우선 사용하고, 실제 수명 계약을 만족하지 못하면 같은 UI를 Native
> Companion에 싣는다. inline UI는 미리보기와 설치 안내이지 지속형 Monitor가 아니다.**

> **AI는 설치를 조율하지만 사용자 승인과 운영체제의 신뢰 경계를 우회하지 않는다.**
