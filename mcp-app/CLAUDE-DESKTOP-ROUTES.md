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
