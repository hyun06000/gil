# GIL MCP Monitor 인수 기록 — 2026-09-28

이 문서는 **이전 개발 단계의 제품 인수 근거와 미결**을 보존한 것이다. 개인 대화 원문·실제 Project
데이터는 포함하지 않는다. 새 저장소나 새 배포물의 CI·설치·화면 검수를 통과했다는 뜻은 아니다.
새 source 검증은 [SOURCE-MIGRATION.md](../SOURCE-MIGRATION.md)에 따로 기록한다.

## 확인된 것

| 항목 | 증거 | 범위 |
| --- | --- | --- |
| 계산기 → 공학용 계산기 작업 중 Graph 실시간 갱신 | 사용자 확인, 9/25 | macOS Codex |
| 실제 GIL Graph fullscreen | 사용자 확인, 9/27 | Claude Desktop 직접 등록; Plugin 업로드와 별개 |
| 가로 DAG와 기본 가로보기·자동 fullscreen | 사용자 확인, 9/27 | macOS Codex 설치본 `0.2.0+codex.20260927122346` |
| Rust 단일 MCP 설치본의 가로 DAG·fullscreen·노드 상세 | 확인 요청 후 사용자 동작 성공 보고, 9/28 | macOS Codex `0.2.0+codex.20260927134559`; 이전 JS 승인에서 추정한 것이 아님 |
| 다른 대화에 갔다 돌아와 선택·접힘·보고서 유지 | 사용자 “그대로 유지돼”, 9/28 | macOS Codex; Host 완전 종료나 새 App 복원은 포함하지 않음 |
| Host 완전 종료 후 수동 재연결 없이 Monitor·Graph 복원 | 사용자 “모니터를 열고 그래프를 보여준다…이정도면 충분히 ux적으로 훌륭”, 9/28 | macOS Codex 기본 재시작 UX 승인; 마지막 선택·접힘·표시 상태의 완전 복원까지 통과한 것은 아님 |
| App 시작 순서·모드 거절·수동 복귀·방향 유지 | `display.test.mjs`, `presentation.test.mjs` | 실제 App 진입점, Host·renderer 경계 주입 |
| 숨김/복귀·cached page 복원·teardown·backoff | `display.test.mjs` | 수명 이벤트 재현; Host가 그 이벤트를 보내는지까지 증명하지 않음 |
| scope 고정·재시작 세대·조용한 5분 재조회 | `host.test.mjs` | transport adapter |
| Plugin/Core 전체 재시작 후 같은 scope 복구·실제 파일 변화 | `monitor-stdio-check.mjs` | 임시 Project, 실제 설치본 stdio, 화면 렌더 증거 아님 |
| 상세 재조회 실패 시 마지막 보고서 보존·회복, 늦은 응답/오류 격리 | 공용 UI selftest 65–70 + 모의 Host 브라우저 시험 | 같은 Step만 보존; 다른 선택이나 떠난 화면의 상세를 섞지 않음; 선택 소멸 시 오류 안내도 정리 |
| 가로 revisit 상단→상단 두 꺾임 및 생성 edge와 동일 선분 겹침 제거 | `ui/layout.test.mjs`, 공용 UI selftest 64 | 모든 fixture 접힘 조합, 선택 강조 node 실제 상단 착지; 가까운 통로 52→28px, Cycle 경계 12px·접힌 원 화살촉 전 7px 이상 확보; 사용자 화면 인수는 별도 |
| 요약 카드 인디케이터의 몸통·화살촉 접합 | `ui/layout.test.mjs`, 공용 UI selftest 6·64 | 몸통은 밑변에서 종료, cap 돌출 없음, 가로·세로·대각선·접힌 원 및 선택 강조 반지름 검증 |
| fullscreen 하단 Report 여백 | `browser-check.mjs`의 iframe 밖 모의 composer | 1280×900·900×650·600×480에서 160/220px overlay 위로 마지막 줄 스크롤·hit test 통과; 실제 Claude 검수 아님 |

자동 fullscreen은 한 App에서 한 번만 요청한다. 사용자가 inline으로 돌아가거나 세로보기를 고르면
덮어쓰지 않는다. Host의 응답과 실제 화면을 구분하며, `show_gil_monitor`의 성공을 화면 표시로
단정하지 않는다. Native Companion은 유지하되 자동 실행하지 않는다.

## 현재 진행 기준

2026-09-28 사용자 결정으로 MCP App 통합을 우선하고 Tauri 전용 검증은 후속으로 미룬다.
기존 Companion 코드·설치물·설정은 보존한다. 공용 UI·read-only·Project 보존 시험은 유지하며,
Companion을 변경하거나 다시 배포할 때 해당 Tauri 검증을 재개한다. 유예를 시험 통과로 세지 않는다.
MCP App 사용에 Companion 설치나 실행을 선행 요구하지 않는다.

## Claude 설치 경로의 현재 차단점

2026-09-28의 Claude Desktop macOS 2.9939.2 대조 실험에서는 폴더가 연결된 작업 세션의
computer bridge를 거치는 새 카운터 Plugin과 기존 Desktop 카운터 모두 도구 데이터만
반환하고 화면을 표시하지 않았다. 같은 기존 카운터는 일반 대화의 Desktop 직접 연결에서
화면·fullscreen에 성공했다. 따라서 과거 Cowork 또는 직접 등록 화면 성공을 현재 Plugin
설치 경로의 성공으로 일반화하지 않는다. 원격 marketplace 설치 자체도 아직 별도 인수 전이다.

Host 함수 분리 시험의 UI metadata 소실은 유력한 메커니즘이지 실제 세션 전체 통신으로
확정한 원인은 아니다. 사용자 승인으로 [상위 프로젝트 이슈 #274에 재현 증거를 제출](https://github.com/anthropics/claude-ai-mcp/issues/274#issuecomment-5858816977)했다.
게시 후 본문 일치를 확인했으며, 제출 완료를 수정·담당자 답변 확인으로 세지 않는다.
직접 연결 진단이나 Companion 자동 실행을 marketplace 한 번 설치 UX의 대체 합격으로 삼지 않는다.

2026-09-28 저녁 대조에서는 폴더를 연결하지 않은 대화에서 Plugin과 직접 등록 모두 Monitor가 표시됐고,
폴더를 연결한 같은 대화에서는 공식 map 예제도 표시되지 않았다. 폴더 없이 Monitor를 보며 파일을
수정하는 흐름, fullscreen이 도구 권한 창을 가리는 새 차단점(#1081)과 열린 UX 문제는
[CLAUDE-DESKTOP-ROUTES.md](./CLAUDE-DESKTOP-ROUTES.md)에 기록한다.

## 다음 사용자 화면 확인

기존 프로젝트에서 하며 재초기화·기록 변경·터미널 명령은 필요 없다.

일반 대화 왕복의 선택·접힘·보고서 보존과 기본 Host 재시작 UX는 사용자 확인으로 통과했다.
완벽한 마지막 화면 복원은 이번 인수의 선행 조건이 아니다. 별도 남은 화면 확인은 다음과 같다.

1. 9/28 디자인 보정판을 새 Monitor로 열어 되돌아감이 node 위에서 나와 두 번만 꺾여 목표 위로
   들어오는지, fullscreen의 마지막 Report 줄을 실제 Host 채팅 입력창 위까지 스크롤할 수 있는지 확인한다.
2. `대화 안으로`와 `세로로 보기`를 선택한 뒤 다른 대화에 갔다 돌아온다. 자동으로 다시 fullscreen/가로형으로
   바뀌지 않아야 한다. 이후 사람이 펼치기 버튼을 누르는 것은 가능해야 한다.

다른 Host의 설치나 종료는 이 시험의 자동 단계가 아니다. Claude Plugin은 사용자가 업데이트를
설치한 다음 같은 절차로 별도 확인한다. Windows 결과를 macOS 결과로 대신하지 않는다.

## 남은 배포 단계

M5-F 화면 성공과 M5-E 배포 완료는 별개다. 17개 tool과 내장 UI를 Rust 실행점 하나로 옮긴
검증은 `NATIVE-ACCEPTANCE.md`에 기록한다. 새 Rust 설치본의 Codex 화면은 별도 사용자 확인을
받았다. 기본 Host 재시작 UX도 승인됐다. MCP App의 수동 표시 선택 보존, Cowork Plugin 설치 경로,
새 기계 설치·서명/공증·원격 배포·
Windows 검수는 각각 남아 있다. Tauri 검증 유예가 MCP 실행 파일의 배포 신뢰 검증을 면제하지 않는다.
새 저장소 생성, 공개 게시, commit/push는 별도 승인 없이 수행하지 않는다.
