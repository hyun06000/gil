# Claude Desktop 표시 경로 진단 — 2026-09-28

2026-09-28 저녁 Claude Desktop(macOS)에서 MCP App이 어떤 호출 경로에서 화면에 표시되는지
대조한 기록이다. 코드, 도구 계약, 설치물, Codex 동작은 바꾸지 않았다. 사용자가 화면을 보고
보고한 결과와, protocol 응답·파일 검사로 확인한 것을 구분해 적는다. 도구 호출 성공만으로
화면 표시를 판정하지 않았다. Claude Desktop 버전은 같은 날 오전 기록(2.9939.2)과 같다고 보고
저녁에 다시 확인하지 않았다.

## 결론

- GIL Monitor의 HTML과 Rust surface는 Claude Desktop에서 표시된다. 폴더를 연결하지 않은
  대화에서는 Plugin 설치 경로와 Desktop 직접 등록 모두 Monitor가 떴다.
- 대화에 폴더를 연결하면 이후 도구 호출이 computer bridge를 거친다(도구 줄의 컴퓨터 아이콘).
  그 대화에서는 GIL뿐 아니라 공식 예제 map도 표시되지 않았다. GIL 고유 렌더러보다 Host 호출
  경로의 제약을 가리키는 대조 증거다. 정확한 내부 원인이나 모든 앱 측 완화책의 불가능까지
  증명하지는 않는다. [ACCEPTANCE.md](./ACCEPTANCE.md)의 "Claude 설치 경로의 현재 차단점"과
  상위 이슈 #274의 관찰과 일치한다.
- 폴더를 연결하지 않은 한 대화에서 Monitor를 보며 Project 파일을 수정하는 흐름은 Filesystem
  Desktop 확장으로 성립했다. 새 차단점은 fullscreen Monitor가 다른 도구의 권한 창을 가리는
  것이며 상위 이슈 #1081로 보고했다.

## 경로별 관찰

| 조건 | 대상 | 결과 | 근거 |
| --- | --- | --- | --- |
| 원격 작업 세션이 컴퓨터에 연결된 대화 | 공식 map, 최소 프로브 | 도구 데이터만 반환, 화면 없음 | 사용자 확인 |
| 폴더 없는 일반 대화, Desktop 직접 등록 | `@modelcontextprotocol/server-map` 2.0.3 | 표시 | 사용자 확인 |
| 같은 조건 | 최소 프로브 v4 | 카드 표시 | 사용자 확인. fullscreen 버튼 동작은 따로 기록하지 않음 |
| 같은 대화에 폴더를 연결한 뒤 | 공식 map (시험 A) | 표시 안 됨 | 사용자 확인 |
| 폴더를 연결한 대화 | GIL Plugin `show_gil_monitor` | scope·view 데이터만 반환, 화면 없음, 도구 줄에 컴퓨터 아이콘 | 사용자 화면 캡처 |
| 폴더 없는 새 대화 | GIL Desktop 직접 등록 check 서버, 경로를 인자로 전달 (시험 B) | 표시 | 사용자 확인 |
| 폴더 없는 새 대화 | GIL Plugin, 경로를 대화로 전달 (시험 C) | 표시 | 사용자 확인 |
| 폴더 없는 대화 + Filesystem 확장(허용 폴더는 예제 Project) | GIL Plugin Monitor와 파일 수정 | inline 실시간 갱신. fullscreen에서는 권한 창이 가려짐 | 사용자 확인 |

## 진단 방법과 배제한 원인

- 도구: Help → Troubleshooting → Enable Developer Mode 후 Developer Tools(`Cmd+Option+I`)의 Console.
- 최소 프로브 v1–v3에서 `Uncaught SyntaxError: Failed to execute 'write' on 'Document' ... at injectInnerHtml`가
  관측됐다. 프로브 빌드가 `String.prototype.replace`의 문자열 치환으로 JS 번들을 HTML에 넣어
  `$&` 같은 치환 패턴이 코드를 손상시킨 탓이었다. 함수 치환으로 고친 v4는 표시됐다. 도구 결과의
  `_meta.viewUUID` 추가(v2)와 신형 SDK 스택(v3)은 원인이 아니었다.
- 폴더를 연결한 대화의 GIL 호출 Console에는 이 오류가 없었다(사용자 캡처). 이는 그 특정
  script 오류의 증거가 없다는 뜻이다. 로그 부재만으로 resource 요청이나 초기화가 없었다고
  단정하지 않는다. Host 경로 제약이라는 해석은 같은 대화의 공식 map 대조 결과와 함께 판단한다.
- GIL은 이 결함에 해당하지 않는다. `mcp-app/build.mjs`는 함수 치환과 `</script` escape를 쓴다.
  `main`(8fcddfa)과 `release/distribution-readiness`(da7fa6f)의 동일한
  `plugins/gil-companion-prototype/assets/monitor.html`(sha256 `0aaf30e3c62d63fdc459…`)을 검사해
  inline script 1개, 본문 안 `</script` 0개, 추출한 script의 `node --check` 통과를 확인했다.
  파일 검사이며 화면 검수가 아니다.
- `MCP server … went away before the attach completed` 경고는 표시되는 map과 프로브에도 나타나
  원인으로 보지 않았다.

## fullscreen과 도구 권한 창

- 관찰: 다른 도구(Filesystem의 Edit File)의 승인 창이 fullscreen Monitor 뒤에 가려져, 작업이 멈춘
  것처럼 보인다. inline에서는 승인 창이 보이고 흐름이 이어진다.
- 현재 사용하는 MCP App 계약에는 Host 권한 창을 제어하거나 다른 도구의 승인 대기를 구독하는
  API가 없다. 검증된 자동 회피 방법이 없으며, 권한 창을 App 안으로 복제하거나 승인 절차를
  우회하는 방식은 해결책으로 채택하지 않는다.
- 상위 보고: [anthropics/claude-ai-mcp#1081](https://github.com/anthropics/claude-ai-mcp/issues/1081).
  사용자 승인으로 2026-09-28 제출했고 게시된 본문이 입력과 같음을 확인했다. 제출을 수정이나
  담당자 답변으로 세지 않는다.
- 결정 전 후보(미구현):
  - 처음 한 번 inline에서 "항상 허용". 새 대화에서도 유지되는지 확인하지 않았다.
  - Claude Host에서만 자동 fullscreen을 생략한다. `app.getHostVersion()`로 구분하되 불확실하면
    기존 동작을 유지해 Codex 자동 fullscreen을 보존한다. Claude가 보고하는 Host 이름을 먼저
    기록해야 한다.

"항상 허용"은 사용자의 권한 범위를 넓히므로 GIL의 기본 설치 지침으로 채택하지 않는다.
관측된 임시 수동 대응은 inline으로 돌아가 Host의 실제 승인 창을 확인하는 것뿐이며,
이를 fullscreen과 작업의 동시 사용 인수 통과로 세지 않는다.

## 열린 UX 문제

- 비개발자의 경로 입력: `gil_monitor_prepare`는 절대경로를 받고, 폴더 연결은 표시를 막는다.
  후보는 Monitor 안의 폴더 탐색과 최근 Project 목록이며 도구 계약 추가가 필요해 결정 전이다.
  sandbox iframe은 브라우저 파일 선택기로 실제 경로를 얻을 수 없다.
- 파일 접근: 이번에 성립한 흐름은 Filesystem 확장을 따로 설치하는 것을 전제한다. GIL이 Project
  범위 파일 도구를 직접 제공하는 안은 도구 계약과 보안 검토가 필요한 미결정 사항이다.
- 사이드 패널: MCP Apps 표시 모드는 `inline`, `fullscreen`, `pip`뿐이고 사이드 패널 모드는 없다.
  Claude의 `pip` 지원은 확인하지 않았다. 내장 브라우저 패널로 localhost Monitor를 여는 안은
  검증하지 않았고 MCP App 우선 배포 원칙과 충돌할 수 있다.
- 작업 대화와 Monitor 대화를 나누는 안은 "Monitor를 보며 대화한다"는 목표 UX와 맞지 않아
  채택하지 않았다.

## 바꾸지 않은 것

코드, 도구 계약, 설치물, Codex 자동 fullscreen, Companion을 바꾸지 않았다. Filesystem 확장 설치와
허용 폴더 설정은 사용자 환경에서 한 시험 조건이며 저장소의 요구사항이 아니다.

## 배포 판단

이 결과로 Claude Plugin 전체가 미지원이라는 주장은 좁혀졌지만, 비개발자의 폴더 선택·파일 작업·
권한 승인·Monitor 지속 표시를 한 흐름으로 인수한 것은 아니다. 첫 unsigned 시험판은 기존대로
Codex/macOS Apple Silicon만 대상으로 한다. Codex 자동 fullscreen과 공통 UI·domain 계약은 유지한다.
Claude 전용 표시 완화는 Host 식별 실측과 회귀시험 뒤 별도 조각으로 검토한다. 폴더 탐색·파일 쓰기
도구 추가는 별도 권한/경로 격리 설계이며, 화면 진단만으로 도구 표면을 확장하지 않는다.

## 2026-10-30 추가 — Claude Desktop 4.4212.0, Claude Code 탭

위 9/28 기록은 Desktop 2.9939.x의 관측이며 바꾸지 않는다. 아래는 같은 Mac에서 Desktop 4.4212.0의
**Claude Code** 탭(폴더가 연결된 작업 세션)에서 확인한 것이다. 사용자 보고와 protocol 실측을 구분한다.

- **사용자 보고:** 소유자가 "Claude Desktop의 Claude Code에서 MCP App fullscreen이 정상 지원된다"고
  보고했다. 이 문서는 그 보고를 그대로 적으며, 아래 실측이 화면을 대신 증명하지는 않는다.
- **protocol 실측 (Plugin 설치 경로):** 2026-10-07 local upload로 설치된 Plugin(Core sha256 `f83e7000…`)에
  대해, 폴더가 연결된 Claude Code 세션 안에서 임시 Project에 `gil_monitor_prepare` → `show_gil_monitor`를
  호출했다. 두 호출 모두 성공했고 `show_gil_monitor`는 `watching: true`인 완전 View를 반환했다.
  9/28에는 같은 조건(폴더 연결)에서 데이터만 반환되고 화면이 없었다. 화면·fullscreen 전환·채팅 병행은
  이 세션에서 스스로 관측할 수 없어 **사용자 화면 확인 대기**로 둔다.
- **Host 광고:** 같은 세션에 직접 등록된 진단 서버(`gil-monitor-direct-check`)의 `client_initialized`
  이벤트가 `ui_capability: true`를 기록했다. 광고는 표시의 증거가 아니다.

이 결과로 바꾼 것:

- App의 **연결 정보**에 Host가 `getHostVersion()`으로 보고한 이름·버전을 그대로 적는다(`host`).
  표시 정책은 Host 이름으로 분기하지 않는다. 9/28 "결정 전 후보"의 Host 식별 기록을 이것으로 닫고,
  Claude 전용 자동 fullscreen 생략은 채택하지 않는다.
- inline-only 안내 문구에서 "Codex 또는 Claude Desktop Cowork"라는 Host 이름을 제거했다.
- preview archive에 Claude Code catalog(`.claude-plugin/marketplace.json`)와 adapter manifest
  (`plugins/gil-companion-prototype/.claude-plugin/plugin.json`)를 함께 싣는다. 설치 절차는 저장소
  `INSTALL.md`에 적었다. 게시는 별도 승인 사항이며 이 변경은 기존 preview.7 archive를 바꾸지 않는다.

### 사용자 화면 확인 (2026-10-31)

- **Plugin 설치 경로(10/07 upload):** 위 `show_gil_monitor` 호출에 대해 사용자가 "inline/안 보여"로
  답했다. 폴더가 연결된 Claude Code 세션에서 Plugin 경로는 View를 반환하지만 fullscreen 화면이
  성립하지 않았다. inline 카드가 있었는지 전혀 없었는지는 아직 구분하지 못했다.
- **직접 등록 진단 서버(`gil-monitor-direct-check`, 9/25 JS 빌드) 실측:** 같은 세션·같은 임시 Project에
  `show_gil_monitor`를 호출하자 서버 진단에 `resource_requested` → App 쪽 `ui_initialize_response
  {fullscreen: true}` → `ui_connected {mode: inline}` → `renderer_ready` → `graph_dom_ready {ok: true}`가
  순서대로 기록됐다. 즉 이 세션의 Host는 직접 등록 App의 iframe을 실제로 띄우고 fullscreen 광고까지
  했다. 이 빌드는 자동 fullscreen을 요청하지 않으며 사용자가 **모니터 펼치기**를 눌러야 한다. 9/28·10/07의
  "폴더 연결 세션은 데이터만 반환"과 다른 결과다. 사용자 화면 확인은 별도다.
- 임시 Project는 Step이 없어 `graph_dom_ready {nodes: 0}`였다. 빈 그래프가 "안 보임"으로 읽힐 수 있어
  확인용 Question Step을 하나 열었다.
- 최소 프로브 `fullscreen-probe` v4 카드도 같은 세션에서 호출했다.
- **사용자 확인 (10/31):** 직접 등록 Monitor 카드가 보였고 **모니터 펼치기**로 펼쳐졌다고 답했다. 서버
  진단에도 `fullscreen_requested` → `mode_event {fullscreen}` → `mode_response {fullscreen}`가 두 번
  기록됐고, 각각 1.5–2.6초 뒤 `mode_event {inline}`이 뒤따랐다(사용자 복귀인지 Host 복귀인지는 미구분).
  이것이 **폴더가 연결된 Claude Code 세션에서 실제 GIL Graph fullscreen을 사용자가 확인한 첫 기록**이다.
  범위는 직접 등록 경로·Desktop 4.4212.0·9/25 JS 진단 빌드이며, Plugin 설치 경로의 성공은 아니다.

- **Claude Code marketplace 설치 경로 실측 (10/31):** 이 브랜치의 `build-preview.mjs` 산출 트리를
  `~/Library/Application Support/GIL/dev-20261031-local`(0700)에 풀고 `artifact.mjs verify`를 통과시킨 뒤,
  Desktop 내장 Claude Code CLI 2.1.309로 10/07 Plugin `plugin uninstall` → `plugin marketplace add <그 폴더>`
  → `plugin install gil-companion-prototype@gil-preview-macos-arm64`가 모두 `outcome: ok`였다. 설치 cache에
  `.claude-plugin/plugin.json`, 실행 비트가 있는 Core(sha256 `0777f800…`), Skill이 들어갔고 Core descriptor가
  응답했다. 이것으로 INSTALL.md의 Claude Code 절차(압축 해제 폴더를 marketplace로 등록)가 실제 CLI에서
  성립함을 확인했다. 화면 재검증은 새 세션에서 사용자 확인으로 한다.

- **marketplace 설치 Plugin의 화면 (10/31):** 새 Claude Code 세션에서 위 설치본(이 브랜치 빌드)의
  `show_gil_monitor`를 열었고, 사용자는 "안 보여"로 답했다. 도구 응답 유무와 연결 정보 값은 받지 못했다.

**판정.** Plugin 경로는 9/25 JS 빌드(10/07 upload), 10/07 Core(upload), 10/31 Core(marketplace 설치)
세 빌드와 두 설치 방식에서 모두 화면에 서지 않았고, 같은 Host·같은 세션 조건의 직접 등록 서버는 App을
띄우고 fullscreen까지 사용자가 확인했다. 차이는 GIL 빌드나 설치 방식이 아니라 **Host가 Plugin 제공 MCP
서버의 App을 그리지 않는 것**이다. 10/07의 관측과 같은 모양이며 상위 이슈 #274의 범위로 본다.
그러므로 Claude Desktop에서 지금 Monitor를 보는 유일한 확인 경로는 **Desktop 설정(`claude_desktop_config.json`)에
Core를 직접 등록**하는 것이다. 공용 작업 규칙(`src/mcp/workflow.md`)은 Rust MCP 서버에 포함되어 도구 설명과
`gil_help`로 전달되므로 Plugin Skill 없이도 Agent 지침은 유지된다. Plugin 경로의 `.claude-plugin` 동봉은
Host가 고쳐지면 그대로 쓰도록 archive에 남긴다.

- **직접 등록 Core 교체 (10/31):** 사용자 승인으로 `claude_desktop_config.json`의 10/07 Core 항목을 제거하고
  이 브랜치 빌드 Core(`…/GIL/dev-20261031-local/plugins/gil-companion-prototype/core/darwin-arm64/gil`)를
  `gil` 이름으로 등록했다(INSTALL.md의 2단계 snippet 그대로). 도구 이름 중복을 피하기 위해 화면에 서지 않는
  marketplace Plugin은 제거했다. 적용은 Desktop 완전 재시작 뒤 새 세션에서 확인한다: 자동 fullscreen 요청
  1회, **연결 정보**의 `host` 이름·버전, 채팅 병행. 결과는 사용자 확인으로만 적는다.

- **설정 편집이 되돌아감 (10/31):** Desktop이 실행 중인 상태에서 `claude_desktop_config.json`을 편집했는데,
  재시작 뒤 파일은 편집 전 내용(10/07 Core의 `gil-monitor-direct-20261007`)으로 돌아와 있었다(mtime 갱신).
  Desktop은 폴더 권한 등 자체 설정을 같은 파일에 쓰므로 실행 중 외부 편집은 덮어써진다. **직접 등록 변경은
  Desktop을 완전히 종료한 뒤 편집하고 다시 켜야 한다.** 따라서 아래 "재시작 후 새 세션" 관측은 교체한 Core가
  아니라 10/07 Core의 App일 가능성이 높다. 두 빌드의 처음 fullscreen 정책(준비 뒤 1회 자동 요청)은 같으므로
  Host 관측으로는 유효하지만, 이 브랜치 Core의 화면 확인으로 세지 않는다.
- **직접 등록 App의 화면 (10/31, Desktop 재시작 후 새 세션):** 사용자 보고 "인라인 카드까지는 뜨지만
  풀스크린으로 뜨지는 않아". 즉 직접 등록 경로에서 App은 그려지지만, 준비 뒤 1회 보내는
  **자동** `requestDisplayMode({mode:'fullscreen'})`는 화면을 바꾸지 못했다. 같은 Host에서 9/25 빌드의
  **클릭** 요청은 fullscreen이 됐으므로, 유력한 가설은 "Claude Code Host는 사용자 동작 없이 보낸 표시 모드
  요청을 적용하지 않는다"이다. 광고 모드가 fullscreen을 포함하지 않아 요청 자체가 나가지 않았을 가능성도
  남아 있다. 두 가설은 **연결 정보**의 `advertised`·`response`·`event` 값과 **모니터 펼치기** 클릭 결과로
  가른다(미수집). Codex의 자동 fullscreen 동작은 이 관측과 무관하게 유지한다.
- **사용자 확인 (10/31):** 같은 카드의 **모니터 펼치기** 버튼은 fullscreen으로 전환됐다. 광고 모드에
  fullscreen이 있고(버튼이 활성이었으므로) 클릭 요청은 적용되므로, 남는 설명은 "자동 요청의 타이밍" 또는
  "사용자 동작 안에서만 적용"이다. 사용자는 기본 fullscreen을 원한다.
- **보정:** App의 처음 의도를 "1회 요청"에서 "1회 요청 → Host가 inline으로 두면 1.5초·4초 뒤 두 번 재요청 →
  그래도 inline이면 사용자의 첫 클릭(어디든)에 마지막 요청"으로 바꿨다. 수동 inline 선택, Host의 fullscreen
  이벤트, teardown은 모두 의도를 끝낸다. 두 가설 중 어느 쪽이든 덮으며, 자동 요청이 바로 적용되는 Codex에서는
  재요청·클릭 요청이 발생하지 않는다(회귀시험 `display.test.mjs`). 보정된 Core는
  `~/Library/Application Support/GIL/dev-20261031b-local`에 풀어 두었고(무결성 검사 통과), Desktop을 완전히
  종료한 상태에서 설정을 그 Core(`gil`)로 바꾼 뒤 새 세션에서 확인했다.
- **사용자 확인 (10/31, 보정 Core · 직접 등록 · Desktop 재시작 후 새 세션):** "클릭해야 펼쳐져". 자동 요청과
  1.5초·4초 재요청은 적용되지 않았고, Monitor 안 첫 클릭의 요청은 fullscreen으로 전환됐다.

**판정 (Claude Code 탭, Desktop 4.4212.0).** Host는 `requestDisplayMode`를 **사용자 동작 안에서 보낸 경우에만**
적용한다. 타이밍 가설(늦게 보내면 적용)은 재요청 두 번이 모두 무시되어 기각한다. 따라서 Claude Code에서
"기본 fullscreen"은 App 혼자서는 만들 수 없고, 현재 최선은 inline 카드가 뜬 뒤 **첫 클릭에 펼쳐지는 것**이다.
안내 문구는 그 상태를 그대로 말한다("Monitor 를 한 번 클릭하면 펼칩니다"). Codex는 자동 요청이 바로 적용되므로
영향이 없다. Host가 사용자 동작 없는 요청을 받아 주거나 도구/resource `_meta`로 초기 표시 모드를 선언하는 길이
생기면 그때 기본 fullscreen으로 돌아간다(상위 제안 후보, 미제출).

남은 것: Claude Code에서 (1) 설치 경로 Plugin의 실제 화면(inline 카드 유무)과 fullscreen, (2) 직접 등록
Monitor의 **모니터 펼치기** 결과와 채팅 병행, (3) fullscreen 중 다른 도구 승인 창 노출(#1081),
(4) 세션 전환·Desktop 완전 종료 뒤 복원, (5) 일반 대화·Cowork 경로의 재검증. 각각 사용자 화면
확인으로만 닫는다.

## 2026-11-01 Host 테마 변수 실측과 토큰 계층

- **프로토콜:** ext-apps 2.0(2026-01-26)의 hostContext는 `theme`, `styles.variables`(표준 CSS 변수),
  `styles.css.fonts`를 정의하고 SDK가 `applyDocumentTheme`·`applyHostStyleVariables`·`applyHostFonts`를 제공한다.
- **Claude Code 탭(Desktop 4.4212.0) 번들 분석:** 렌더러가 표준 변수 전체와 `anthropic-sans` `@font-face`를
  App에 보낸다. 테마는 `prefers-color-scheme`에서 정한다. 값 요약은 [README](README.md)에 있다. 설치본
  번들을 읽은 것이며 화면에서 확인한 값은 아니다.
- **Codex(사용자 제공 연결 정보, 11/01):** `host` `chatgpt` 26.1027.51404, `theme: dark`, 변수 76개, 글꼴 CSS
  없음, 자동 fullscreen 적용. 번들 검색으로는 보이지 않았으므로 Host 판정은 연결 정보로만 한다.
- **조치:** `ui/companion.css`를 Host 변수 우선·중립 fallback 토큰 계층으로 바꾸고, App이 연결·변경 시 Host 값을
  적용하며, 연결 정보에 `styles`(유무·개수)를 적는다. 맨 위 표시 모드 줄은 맨 아래로 내리고 보조 손잡이는
  머리줄로 옮겼다. 정책은 여전히 Host 이름으로 갈라지지 않는다.
- **사용자 확인 (11/01, 직접 등록 설치본 `dev-20261101-local`, Core SHA-256 `755fc55e…`):** Claude Code 탭의
  inline 카드가 주변 대화와 어울린다고 답했다. 설정 적용은 Desktop 완전 종료 뒤 스크립트로 했고, 이번에는
  설정이 되돌아가지 않았다. 적용 스크립트의 실행 중 검사는 주 프로세스 `Claude`만 봐야 한다 — Chrome 확장이
  띄우는 `chrome-native-host` 등 보조 프로세스는 Desktop 종료 뒤에도 남는다.
- **Codex Plugin 교체 (11/01):** 기존 GitHub marketplace 등록·Plugin을 제거하고 `dev-20261101-local`을 같은
  이름의 로컬 marketplace로 등록해 재설치했다. 버전 문자열이 공개판과 같아 Core 지문(755fc55e…)으로만 구분된다.
- **사용자 확인 (11/01, Codex, dark):** 펼친 Monitor가 Codex 화면과 어울린다고 답했다.
- **판정:** 두 Host 모두 표준 변수를 보내므로 Host 이름 분기는 필요 없다. Codex fallback은 넣지 않는다.
- **남은 것:** 보지 않은 쪽 테마(두 Host), Claude Code 연결 정보 `styles` 수. 사용자 화면 확인으로만 닫는다.

## 2026-11-02 등록 자리 확인과 Host 동등 지원 정리

- **Claude Code 탭이 읽는 설정 (읽기 전용 확인, 소유자 Mac):** 이 세션은 Claude Desktop의 Claude Code 탭 안에서
  돌았고 `gil` 서버(`dev-20261101-local` Core)를 쓸 수 있었다. 그 항목은
  `~/Library/Application Support/Claude/claude_desktop_config.json`의 `mcpServers.gil`에만 있었고, Claude Code 사용자
  범위 설정 `~/.claude.json`의 `mcpServers`는 비어 있었다. 따라서 Claude Code 탭의 MCP 서버 등록 자리는 Desktop
  설정 파일이다. `claude mcp add --scope user`가 Desktop의 Claude Code 탭에도 보이는지는 확인하지 않았다.
- **Companion Claude Desktop adapter (코드, 화면 미확인):** Windows Companion에 **Claude Desktop 연결** 버튼을
  추가했다(`companion/src/claude.rs`). Codex와 같은 고정 런처를 `claude_desktop_config.json`에 적되, Desktop이 실행
  중이면 거절하고(위 10/31 되돌아감 관측 때문), 백업·임시 파일·rename·읽은 뒤 변경 없음 확인을 거치며, 다른 서버
  항목과 사용자가 만든 동명 `gil` 등록은 바꾸지 않는다. Rust 단위 시험과 Windows 제품 계약 시험만 통과했다.
  **실제 Windows Claude Desktop에서 버튼 → 새 대화 → Monitor 표시는 사용자 확인 대기**다.
- **Windows Plugin 후보:** `candidate.mjs`가 `.claude-plugin/marketplace.json`과 `.claude-plugin/plugin.json`
  (`${CLAUDE_PLUGIN_ROOT}/core/windows-x64/gil.exe`)을 함께 싣는다. Mac archive와 같은 모양이며, 10/31 판정대로
  Claude Code 탭이 Plugin 서버의 App을 그리기 전에는 화면 경로가 아니다.
- **문구:** Monitor의 inline-only 안내에서 “Codex 또는 Claude Desktop Cowork”를 “fullscreen 을 지원하는
  Host(Codex, Claude Desktop 의 Claude Code 탭)”로, Windows 작업 폴더 도구의 “Codex 앱에서 선택”을 “사용 중인 Host
  앱(Codex 또는 Claude Desktop)에서 선택”으로 바꿨다. 표시 정책은 여전히 Host 이름으로 갈라지지 않는다.
- **이 세션에서 확인하지 못한 것:** 이 세션은 Desktop을 재시작하거나 새 대화를 열 수 없어 (1) Windows Companion
  버튼 경로의 실제 화면, (2) 업데이트 뒤 두 Host의 다음 연결이 같은 새 Core를 받는지, (3) fullscreen 중 승인 창
  병행(#1081), (4) Desktop 완전 종료 뒤 Monitor 복원은 모두 사용자 확인으로만 닫는다. 최소 재현 절차는 저장소
  `INSTALL.md`의 Windows 절과 PR 본문에 적었다.

## 2026-11-02 Claude Code 탭 — 시작 직후 Monitor 미표시·inline 기본 점검

보고된 두 문제(① `gil_start` 성공 뒤 Agent가 `show_gil_monitor`를 부르지 않음, ② 뜨더라도 inline 카드)를
Host 이름으로 분기하지 않고 지침·계약·App 로직에서 점검했다. 사용자 화면 확인은 아직 없다.

- **Host가 모델에 보여 주는 도구:** 폴더가 연결된 Claude Code 세션에서 직접 등록 `gil` 서버의 `gil_start`·
  `show_gil_monitor`를 모델 쪽에서 읽었다. 두 도구 모두 이름·설명·inputSchema가 그대로 전달됐고
  (`_meta.ui.visibility`·`outputSchema`·`pattern`은 Host가 제거), 설명은 잘리지 않았다. 따라서 ①의 원인은
  가시성이나 설명 길이가 아니다.
- **① 원인 — 지침·계약의 모호함:** 시작 receipt의 `said` 본문은 `다음 … 실행 gil open`으로 끝나고,
  `show_gil_monitor({scope_id})` 안내는 그 앞에 붙는 별도 텍스트 블록에만 있었다. `structuredContent.said`만
  읽거나 본문의 `실행` 줄을 우선하는 Host/Agent는 Monitor 호출을 건너뛸 수 있다. 고침: Core가 성공한
  `gil_start` 응답의 마지막 텍스트 블록에 `Monitor / 다음 호출은 하나다: show_gil_monitor({...})`를 넣었다.
  `structuredContent.said`는 CLI receipt와 동일하게 유지하므로 이 필드만 읽는 소비자는 추가 안내를 받지 못한다.
  앞 힌트도 "다음 호출은 하나다 · Host 종류와 무관 · 인자는 그대로 복사"로 바꿨다. `workflow.md` 규칙 1과
  `gil_start`·`show_gil_monitor` 도구 설명도 같은 문장으로 맞췄다. Plugin Skill은 동기화 스크립트로 같은 원문을 받는다.
- **② 원인 — App 로직의 시점:** 10/31까지 App의 첫 자동 `requestDisplayMode({mode:'fullscreen'})`은 Monitor가
  **렌더된 뒤**에만 나갔고(1.5초·4초 재요청 포함) Claude Code 탭은 그것을 적용하지 않았다. Host가 카드를 막
  올리는 시점의 요청은 시도된 적이 없다. 고침: `availableDisplayModes`에 fullscreen이 있으면 **연결 직후**에
  한 번 요청하고, 렌더 뒤 한 번, 거절되면 1.5초·4초 재요청, 그래도 inline이면 첫 클릭 — 같은 순서를 모든
  Host에 보낸다. Codex처럼 첫 요청이 적용되면 나머지는 나가지 않는다(회귀시험 `display.test.mjs`).
- **확인 가능성:** **연결 정보**의 `automatic`에 자동 시도마다 `{phase: connect|ready|retry|gesture, at_ms,
  applied}`가 쌓인다. Claude Code 탭에서 `connect` 항목이 있고 `applied`가 `inline`이면 "요청은 나갔으나 Host가
  적용하지 않음", 항목이 없으면 "광고 모드에 fullscreen이 없어 요청 자체가 나가지 않음"으로 가른다.
- **Codex와의 차이(남는 것):** Codex는 첫 자동 요청이 적용된다. Claude Code 탭에서 연결 직후 요청이 적용되는지는
  사용자 화면 확인 대기다. 적용되지 않으면 동작은 10/31과 같다(inline 카드 → 첫 클릭에 fullscreen).
- **바꾸지 않은 것:** 저장 형식, 공개 schema, `presence.rs`의 표시 재시도 예산(미응답 2회에서 중단), Host 이름 분기 없음.

사용자 확인 절차(Claude Code 탭, Desktop 완전 재시작 뒤 새 세션, 빈 테스트 폴더): "지금 폴더에서 GIL 프로젝트를
시작해 줘" → Agent가 `gil_start` 바로 다음에 `show_gil_monitor`를 한 번 부르는지, Monitor가 시작 직후 자동으로
fullscreen으로 뜨는지, 아니라면 inline 카드가 뜨고 첫 클릭에 펼쳐지는지, 그리고 **연결 정보**의 `automatic` 값을
적는다. 도구 응답 성공은 화면 성공이 아니다.

## 2026-11-09 추가 — Claude Desktop 4.23118.0, Claude Code 탭: 자동 fullscreen 거부 로그

증상 보고: "`gil_start` → `show_gil_monitor` 뒤 Monitor가 전혀 뜨지 않는다(이전에는 inline 카드는 떴지만
자동 fullscreen이 되지 않았다)". 아래는 같은 Mac에서 Desktop 설정 `mcpServers.gil`에 직접 등록된
preview.27 Core(`gil 0.2.1-preview.27`, serverInfo 동일)를 Claude Code 탭 세션에서 호출해 확인한 것이다.
자동 시험·합성 측정·사용자 화면 확인을 구분한다.

- **자동 시험 (저장소):** `mcp-app` 102개, `plugins/gil-companion-prototype` 80개, `distribution/compliance/notices`
  15개 JS 시험 통과. 번들 `monitor.html`은 `node mcp-app/build.mjs`로 재현되며 inline script 1개, `node --check`
  통과, `</script` 문자열 없음. Core `presence` 단위 시험과 `tests/mcp.rs`의 presence 통합 시험 통과.
- **합성 측정 (protocol, stdio):** 설치판 Core의 `tools/list`에서 `show_gil_monitor`의 `_meta.ui.resourceUri`와
  `_meta["ui/resourceUri"]`가 같은 `ui://gil-monitor/<sha20>.html`을 가리키고, `resources/list`·`resources/read`가
  `text/html;profile=mcp-app`로 HTML을 돌려준다. 공식 `@modelcontextprotocol/server-map` 2.0.4(같은 ext-apps 2.0.0)의
  응답 모양과 `_meta`·mimeType·csp 키가 일치한다. 계약층의 회귀는 찾지 못했다.
- **합성 측정 (실제 Host, 이 세션):** scratch Project에 `gil_start` → `show_gil_monitor` 뒤 `gil_monitor_status`가
  `state: visible, display_mode: inline, last_seen_ms: 103`을 돌려줬다. App iframe이 부팅해 폴링 중이라는 뜻이며
  "표시 자체가 안 됨"은 이 경로에서 재현되지 않았다. 같은 시각 Desktop의 `claude.ai-web.log`에는
  `[MCP] [Epitaxy MCP App] fullscreen request refused: it needs a fresh press in this app's own frame, not elsewhere on the page`
  가 App의 자동 요청 수(연결·준비·재시도 2회)만큼 4번 적혔다. 즉 이 Host는 프로그램 fullscreen 요청을 거부하고
  **App 프레임 안의 새 사용자 클릭**에서만 펼친다. 이것이 "inline은 뜨는데 자동 fullscreen이 안 됨"의 원인이다.
- **같은 Mac의 Plugin 경로:** Claude Code marketplace `gil-preview-macos-arm64`가 `dev-20261031-local`(Core `gil 0.1.0`,
  #78 이전 빌드)을 가리킨다. 그 서버에 preview.27이 만든 Project로 `gil_monitor_prepare`를 호출하면
  `unsupported_format`으로 거절되어 `show_gil_monitor`까지 가지 못한다. 구형 Plugin과 신형 직접 등록이 함께 켜진
  환경에서는 Agent가 어느 서버를 부르느냐에 따라 "아무것도 뜨지 않음"이 재현될 수 있다. 사용자 확인 대상이다.

이 결과로 바꾼 것(Host 이름으로 분기하지 않는다):

- App은 자동 요청이 **오류(거부)**로 끝나면 타이머 재시도를 하지 않고 즉시 클릭 경로를 무장한다. 거부된 클릭
  요청은 세 번까지 다시 무장한다. Host가 `inline`을 **적용**한 경우의 기존 동작(유한 재시도 뒤 클릭 한 번)은 그대로다.
- App은 presence 보고에 `automatic_fullscreen`(`pending|applied|declined|refused|unavailable|manual`)을 싣고, Core는
  `gil_monitor_status`로 그대로 노출하며 inline 안내문을 그 값에 맞춘다(`refused`면 "Monitor 카드를 한 번 클릭").
  Core는 fullscreen을 스스로 요청하지 않는다. 구형 App은 값을 보내지 않으며 `null`로 남는다.
- Node 어댑터(`plugins/gil-companion-prototype/monitor.mjs`)의 `gil_monitor_poll` 스키마가 presence 필드를 거부하던
  것을 통과시킨다.
- inline-only 안내문에서 Host 이름을 뺐다.

확인하지 않은 것: 실제 화면의 inline 카드·클릭 뒤 fullscreen 전환은 이 세션이 스스로 볼 수 없어 사용자 확인
대기다. Desktop Console 캡처는 받지 못했다. Host의 "fresh press" 판정이 `pointerdown`을 인정하는지는 사용자의
첫 클릭 결과(연결 정보의 `automatic` 마지막 항목 `gesture`의 `applied`)로 확인한다.
