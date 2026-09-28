# gil-companion-prototype

**Agent 가 쓸 GIL 과 사람이 볼 Monitor 의 문**을 함께 소유하는 공용 Plugin 정본.
이 디렉터리가 **정본**이다.

Codex 원격 배포 준비용 catalog/단일 실행 파일 묶음·무결성 검사·수동 CI는
[`distribution/codex/`](../../distribution/codex/README.md)에 있다. 미등록·미서명 preview만 만들며
기존 설치 cache를 교체하거나 원격 게시하지 않는다. source clone은 사용자 설치 경로가 아니다.

```text
core/<platform>/gil   Rust MCP + Core + 내장 UI — 설치본의 실행 파일 하나 (생성물)
../../src/mcp/        17개 tool · resource · read-only Monitor · native fallback 배선
../../src/command.rs  CLI 와 MCP 가 공유하는 domain 호출·receipt
*.mjs                이전 JS adapter와 개발용 시험/패키징 — native 실행 의존성이 아님
assets/monitor.*      공용 UI에서 생성한 HTML · 내용 hash/URI (Rust 빌드 입력)
core.test.mjs         Core 판정과 동치 시험
capability.test.mjs   coordinator 시험
manifest.test.mjs     두 adapter 가 같은 하나를 가리키는지의 시험
make-core.sh          실린 Core 를 짓는 자리
package-native.mjs    Node 없는 macOS arm64 개발 검수 zip (Host 별 manifest만 다름)
skills/gil-companion  Agent가 언제 무엇을 부를지
.codex-plugin/        Codex adapter manifest — 여기 말고 아무것도 담지 않는다
.claude-plugin/       Claude Code adapter manifest — 같은 규칙
```

Codex와 Claude Code는 별도 GIL 제품이 아니다. 같은 Rust MCP·Core·tool 표·Skill을 쓰고,
Host가 요구하는 manifest와 Plugin root 표기만 나눈다. 사용자에게는 둘 다 **GIL Plugin**이다.
일반 Claude Desktop용 `.mcpb`는 후속 packaging adapter이며 이 정본을 복사한 두 번째 구현으로
만들지 않는다.

## Host adapter 는 manifest 하나뿐이다

MCP 등록은 **각 Host 의 manifest 안에** 적는다. plugin root 의 `.mcp.json` 은 두지 않는다 —
두 Host 모두 그 파일을 스스로 찾아내므로, inline 등록과 겹치면 server 가 둘이 된다.

```text
.codex-plugin/plugin.json    "command": "./core/darwin-arm64/gil",                     "cwd": "."
.claude-plugin/plugin.json   "command": "${CLAUDE_PLUGIN_ROOT}/core/darwin-arm64/gil", "cwd": "${CLAUDE_PLUGIN_ROOT}"
두 Host 모두                 "args": ["mcp", "--serve"]
```

다른 것은 **plugin root 를 부르는 이름 하나**뿐이다. 두 줄 모두 같은 Rust 실행 파일로 내려앉고,
`manifest.test.mjs` 가 그 자리를 풀어 같은 파일인지 확인한다. Skill·Core·tool 표는 나누지
않는다 — adapter 디렉터리는 자기 `plugin.json` 말고 **아무것도** 담지 않으며, 그것도 시험이
지킨다.

Codex 는 `skills/` 자리를 manifest 에 적고, Claude Code 는 `skills/` 를 언제나 스스로 훑으므로
적지 않는다. 같은 Skill 파일 하나를 두 Host 가 각자의 방식으로 찾아갈 뿐이다.

## 무엇을 소유하는가

```text
Plugin      Agent 용 GIL Core · MCP bridge · Manual/Skill · Companion coordinator
Companion   읽기 전용 Monitor · DAG·Report UI · Project watcher · 지속형 창
```

Companion과 MCP App에 Agent write 명령을 넣지 않는다. Plugin은 Host가 그리는 MCP App resource를
제공하지만 별도 native 창 수명은 Companion이 맡는다. 기본 인간 표면은 Codex / Claude Desktop
Cowork의 fullscreen이며 Companion은 Windows·독립 창·Host 미지원 fallback으로 유지한다.

## MCP fullscreen Monitor (개발 인수 단계)

`gil_monitor_prepare(project_root)` → `show_gil_monitor(scope_id)` → 가로보기 + fullscreen 자동 요청.
App 연결과 Monitor 준비가 끝난 뒤, Host가 지원을 광고하면 인스턴스당 한 번만 요청한다.
자동 요청이 거절·무시되거나 inline을 반환하면 **모니터 펼치기**를 직접 누를 수 있다.
사용자가 대화 안으로 돌아온 뒤에는 자동으로 다시 펼치지 않으며 Companion도 자동 실행하지 않는다.
준비 도구는 exact-root read-only 검증 후 scope와 이름만 돌려준다. 렌더 도구의 input·HTML·App
조회에는 경로가 없다. App-only `gil_monitor_read`·`gil_monitor_detail`·`gil_monitor_poll`은 기존
canonical View·Detail과 hint counter를 사용한다. 기존 12개 도구는 유지되며 전체 목록은 17개다.

`mcp-app`가 `ui/index.html`·`companion.css`·`companion.js`·`layout.js`를 재사용해 HTML 하나로 묶는다.
`npm ci --prefix mcp-app` 후 `npm run build --prefix mcp-app`로 재생성한다. SDK·esbuild는 개발용이고
Rust가 빌드할 때 `assets/monitor.html`을 내장한다. 설치본은 별도 HTML 파일을 읽지 않는다.
HTML hash가 URI이므로 변경 뒤 옛 resource cache를 쓰지 않는다.

MCP App의 DAG 기본값은 mode와 무관하게 가로형(시간: 왼쪽→오른쪽)이다.
방향 기본값이나 fullscreen 요청 자체는 실제 fullscreen 전환의 증거가 아니다.
형제는 아래 lane, 되돌아감은 왼쪽 과거로 향하는 점선이다. `세로로 보기` / `가로로 보기`로
방향을 바꿔도 선택·접힘·전체 Report는 유지된다. Companion의 기본 세로형은 변경하지 않는다.
`ui/layout.test.mjs`는 두 방향의 참조 동일성과 접기 압축을, `ui/selftest.html`은 실제 DOM을 검사한다.
렌더 도구는 표준 `_meta.ui.resourceUri`와 호환 alias `_meta["ui/resourceUri"]`에 같은 URI를 싣는다.
이는 직접 등록 성공본과 metadata를 맞춘 것이며, Plugin 경로 표시 성공의 증명은 아니다.

2초 App heartbeat는 변경 counter만 읽고 전체 Project를 훑지 않는다. hint·수동 refresh·복귀 시
완전 View, 조용할 때 5분 reconciliation, 실패 때 bounded backoff, 사라진 App의 watch lease 회수.
Project는 App 인스턴스 하나에 고정되며 새 scope는 Agent에게 명시하여 다시 연다.
App이 숨겨진 동안 heartbeat timer를 중단하고, 같은 화면으로 돌아오면 완전 View를 다시 읽는다.
보관된 page의 복원과 실제 종료를 구분하며, 느린 조회 중 여러 복귀가 와도 하나로 합친다.
복귀 때문에 fullscreen을 다시 강제하거나 Companion을 열지는 않는다.

명시적으로 준비한 Project는 Rust가 사용자 전용 로컬 설정에 기억한다. 서버를 재시작해도 App이
기존 scope만 보내면 같은 Project인지 확인한 뒤 View·Report·watcher를 자동 복원한다. macOS의
설정은 `~/Library/Application Support/GIL/monitor-bindings-v1/`에 0700/0600으로 저장되며
Project·Companion 설정·Plugin cache에는 쓰지 않는다. 같은 경로의 다른 `.gil`은 자동 연결하지 않는다.
서버 세대가 바뀌면 hint counter가 같아도 완전 View를 다시 읽는다. 폴더 없음·교체·설정 손상은
마지막 Graph를 남기고 이유를 표시한다. 저장 기능 이전의 화면은 업데이트 후 한 번 새로 열어야 한다.

개발 시험은 `GIL_MONITOR_STATE_DIR`의 절대경로로 설정 위치를 격리한다. 해당 위치가 Project 안이면
저장 전 거절한다. Windows identity는 아직 구현·검수되지 않았으며 native Companion을 유지한다.

Rust MCP 하나가 기존 12개 도구와 Monitor 5개·resource를 함께 제공한다. JS adapter는 이전
설치본과의 비교 시험을 위해 소스로 남지만 native manifest에서 실행하지 않는다.
설치 후 Node·npm·cargo·source checkout은 필요하지 않다. 새 Mac의 설치 UX·서명·공증은 별도 인수다.
`gil_companion_status`는 아직 개별 App의 fullscreen 성공을 판정하지 않는다.

macOS Codex 계산기 프로젝트의 실시간 Graph 갱신(9/25), Claude Desktop **직접 등록** 실제 GIL
fullscreen(9/27), 가로 DAG(9/27), Codex의 가로 기본값·fullscreen 자동 요청(9/27)은 사용자 확인을 받았다.
Cowork **Plugin 설치 경로**와 실제 Host 작업 전환·완전 종료 뒤 수명 검수는 별도 인수가 필요하다.
직접 등록 V3 진단판이나 fixture UI 시험으로 대신하지 않는다. `mcp-app/ACCEPTANCE.md`에 증거를 구분한다.
Windows는 미검증이다.

## Agent 가 부르는 문

`gil_start` · `gil_open` · `gil_close` · `gil_restore` · `gil_revisit` ·
`gil_status` · `gil_story` · `gil_context` · `gil_cycle` · `gil_help`

경계는 이렇게 생겼다.

```text
typed MCP 인자 → 공유 command 모듈 (명시적 root·본문)
→ Rust domain 직접 호출 → Result로 성패 → 기존 응답 wrapper와 산문 유지
```

shell·자기 실행 파일 재호출·chdir를 쓰지 않는다. 사용자 입력은 **값**이며 protocol stdin을
Report 입력으로 읽지 않는다. Core의 산문을 해석하지 않는다. `ok`·`exit_code`·`said`·`problem`은
이전 JS 응답과 같고, CLI의 실제 stdout/stderr는 얇은 binary wrapper만 소유한다.

Project 자리는 **명시적으로** 받는다. cwd 도, 최근 폴더도, Companion 이 사람에게 보여 주는
선택도 Agent action 의 대상으로 짐작하지 않는다.

## 실린 Core

```bash
./make-core.sh   # 짓고 확인만 한다
./package.sh     # 짓고 · 확인하고 · cachebuster 를 올리고 · 다시 설치한다
```

저장소의 Rust Core 를 `core/darwin-arm64/gil` 로 짓고, **지은 뒤 실제로 실행해** identity·
architecture·fresh challenge 를 확인한다. 새 파일을 검증한 뒤 원자 교체한다. 실패하면 이전 파일은
보존하되 script가 실패로 끝나므로 cachebuster·재설치로 넘어가지 않는다.

### 배포의 한계 — 이 sidecar 가 아직 아닌 것

이 binary 는 **로컬에서 만든 개발 artifact** 다. git 에 들어가지 않으므로:

- **source clone 만으로는 Plugin 설치가 완성되지 않는다.** 받은 사람은 `make-core.sh` 를 직접
  돌려야 하고, 그러려면 Rust 와 cargo 가 필요하다 — 비개발자용 경로가 아직 아니다.
- packaging 전에 `make-core.sh` 가 **반드시** 먼저 돌아야 한다. `package.sh` 가 그 순서를
  강제한다: 짓고 → identity·arch·fresh challenge·digest 를 확인하고 → 그제서야 cachebuster 를
  올리고 다시 설치한다.
- **Core 가 없거나 이 기계의 것이 아니면 cachebuster 와 재설치를 하지 않는다.** 확인되지 않은
  설치본을 만드느니 멈춘다.
- **public marketplace 용 binary publication 은 미결이다.** 서명·공증·배포 자리·platform 별
  artifact 가 모두 정해지지 않았다.
- 지금 실측한 것은 **macOS arm64 development packaging feasibility** 하나뿐이다.

서명도 공증도 하지 않았고 공개 배포판이 아니다. Companion 의 `release-macos.sh` 와 혼동하지
않는다 — 그쪽은 사람이 볼 창을 짓고, 여기는 Agent 가 부를 Core 를 짓는다.

지금 묶는 자리는 **macOS arm64 하나뿐**이다. 이 manifest를 다른 platform용으로 배포하지 않는다.
다른 OS/architecture는 해당 binary·manifest와 설치 검수가 필요하다. Windows를 지원한다고 주장하지 않는다.

## 정본과 설치본

```text
plugins/gil-companion-prototype          ← 정본 (여기). 고치는 자리는 여기 하나다.
~/plugins/gil-companion-prototype        → 위를 가리키는 symlink. **개발·설치 진입점일 뿐이다.**
~/.codex/plugins/cache/personal/...      ← `codex plugin add` 가 만든 설치본. 혼자 선다.
```

Claude Code adapter도 같은 정본에서 설치 cache를 만들며, 그 cache가 Codex cache를 참조하거나
반대쪽 cache를 정본으로 삼지 않는다. 두 설치본은 독립적으로 지울 수 있지만 담긴 MCP 동작과 Skill은
같아야 한다. 한쪽 Plugin을 끄면 그 Host의 Agent tool만 사라지고, Companion도 Project 기록도
그대로 남는다.

personal marketplace(`~/.agents/plugins/marketplace.json`)의 항목이 `./plugins/gil-companion-prototype`
를 가리키고, 그 자리가 이 디렉터리로 이어지는 symlink다. 그래서 편집하는 파일과 marketplace가
읽는 파일이 **같은 파일 하나**다. 손으로 복사해 두 정본을 만들지 않는다.

### symlink 는 runtime 의존이 아니다

설치가 끝나면 symlink도, 이 저장소도 **실행에 필요하지 않다.** `codex plugin add`가 설치본을
cache로 복사하고, 그 안의 manifest는 자기 root 를 기준으로만 말한다.

```json
{ "command": "./core/darwin-arm64/gil", "args": ["mcp", "--serve"], "cwd": "." }
```

절대 경로도, 홈 디렉터리도, 이 저장소의 자리도 적지 않는다 — Codex 번들 plugin이 쓰는 것과 같은
형태다. Claude Code 쪽도 같은 약속을 `${CLAUDE_PLUGIN_ROOT}` 로 말할 뿐이고, manifest 와
marketplace 에 기계에 매인 자리가 없다는 것을 시험이 지킨다. 저장소·cache·Node·Rust 도구를
읽을 수 없는 sandbox 안에서 최소 zip의 압축 해제본만으로 tool 목록·Agent walk·내장 UI 조회·
재시작 복원·파일 감시가 동작하는 것을 확인했다. Companion 창 실행과 Host 화면 검수는 별도다.

Rust는 로컬 Monitor binding 설정과 Companion 설치 위치를 읽는다. 둘 다 사용자 상태이며
Plugin source 경로가 아니다. source·설치 cache·Project 데이터의 소유를 섞지 않는다.

다른 개발 기계에서는 이 저장소를 받아 `npm ci` 와 `make-core.sh` 를 돌린 뒤 아래 절차를 그대로
밟는다. manifest 는 고칠 것이 없다. 이것은 **개발자 절차**다 — clone·Node·npm·Rust·cargo 가
모두 있어야 하므로 받는 사람의 설치 완성본이 아니다.

Node는 개발 시험과 UI bundle/zip 생성에만 쓴다. native 설치물은 Node 없이 실행한다.
`native-parity-check.mjs`는 승인된 JS 설치본과 17개 schema·UI bytes·실제 동작을 비교한다.
인수 결과와 미검증 범위는 `../../mcp-app/NATIVE-ACCEPTANCE.md`에 적는다.

## 고친 뒤 반영하기 — Codex

Codex의 공식 절차를 그대로 쓴다. marketplace 파일을 손으로 고치지 않는다.

```bash
S=~/.codex/skills/.system/plugin-creator/scripts
python3 "$S/read_marketplace_name.py"                     # → personal
python3 "$S/update_plugin_cachebuster.py" "$PWD"          # 판 번호의 cachebuster 만 교체
codex plugin add gil-companion-prototype@personal
```

그다음 **새 thread**에서 열어야 갱신된 skill과 tool이 붙는다. 옛 MCP server가 남아 있으면
그 프로세스만 정확한 PID로 끝낸다.

## 고친 뒤 반영하기 — Claude Code

개발 인수에 실제로 쓴 경로는 **local Plugin upload** 하나다. 이 정본을 묶어 Claude Desktop 에
올리면 Host 가 `local-desktop-app-uploads` marketplace 아래에 설치본을 둔다. `package.sh` 는
Codex 쪽 절차다. `node package-native.mjs claude /새/경로/gil-plugin.zip`은 Claude 검수용
묶음을 만든다(기존 zip은 덮지 않는다). 설치 registry(`~/.claude/plugins/…`)는 손으로 고치지 않는다. 갱신된 skill 과 tool 은
**새 session** 에서 붙는다.

저장소 최상위의 `.claude-plugin/marketplace.json` 은 그 묶음의 정본 descriptor 다. **Desktop UI
에서 로컬 디렉터리를 marketplace 로 더하는 자리는 확인되지 않았다** — 그 문을 전제로 절차를
적지 않는다. 사용자 배포 경로는 local upload 가 아니라 **remote marketplace** 이며, 그것은 아직
열려 있다(M5-E).

```text
local Plugin upload    개발 인수 경로 — 이 저장소에서 지어 올리고, 받는 쪽도 개발자다
remote marketplace     사용자 배포 경로 — self-contained artifact 와 release pipeline 이 필요하다
```

source clone은 **비개발자 설치의 완성본이 아니다.** 만드는 개발자에게는 `make-core.sh`와
Rust/cargo가 필요하다. 만들어진 native zip을 받는 쪽에는 필요하지 않지만, 아직 개발 검수물이고
공개 서명 배포판은 아니다. 일반 Claude Desktop 용 `.mcpb` 는 후속 adapter 이고 아직
구현하지 않았다.

## 시험

```bash
./make-core.sh   # Core 를 먼저 짓는다 — 동치 시험이 이것을 쓴다
npm test
```

바깥 효과는 전부 가짜로 끼운다 — launcher, handshake probe, 시계. production protocol을 약하게
만들거나 고정된 fixture 결과를 제품 코드에 넣지 않는다.

`node_modules`는 역사에 넣지 않는다. 새로 받으려면 `npm ci`.
