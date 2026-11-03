# M6-D 구현 및 로컬 검증 — 2026-11-01

기준: GitHub main `3b5341f544eb2baad7a7d7dca133c4522ac28f9f` (최종 조회에서도 동일).
작업 브랜치: `feat/approved-plan-m6d-20261101`. 병합·배포·실제 Host 인수와 구분한다.

## 구현된 경계

- 선택적 `approved-plan` 개발 프로필. 기존 프로젝트를 자동 이전하지 않는다.
- 열린 Synthesis의 전체 제안, 순차 revision, 기준 승인 판, SHA-256과 Monitor 응답을 저장한다.
- 사용자 버튼 응답은 scope/revision/digest/임시 capability로 검사한다. 토큰은 App `_meta`에만 있고 model content·Context·저장소·Debug 출력에는 없다.
- 편집은 새 판이며 기존 응답은 이력에 보존된다. 이전 승인은 편집된 판을 닫는 근거가 되지 못한다.
- Synthesis 승인 후에도 Outcome 및 Interview Cycle을 닫아야 Experiment를 열 수 있다.
- Experiment는 승인된 첫 미실행 항목에 고정된다. A 실행 뒤 D 추가 승인 시 남은 순서는 D,B,C이고 A의 기록은 그대로다.
- 열린 Experiment에서 재인터뷰 요청을 보존하며 다른 Cycle을 동시에 만들거나 성공으로 자동 종료하지 않는다.
- Verify는 바인딩된 승인 계획 문서의 실제 Snapshot bytes와 별도의 새/변경된 작업 증거를 요구한다.
- Closing Interview와 후속 Chain도 별도 제안/승인이 필요하다. 과거 Chain의 승인을 재사용하지 않는다.
- App 버튼 성공 후 Host 채팅에 계속 요청을 보낸다. 응답 유실/메시지 전달 실패는 저장된 결정 조회로 구분하며 재조회만으로 승인·메시지를 전송하지 않는다.

## 실행한 검사

macOS 로컬, 합성 임시 프로젝트. 기존 사용자 `.gil`은 시험하지 않았다.

| 검사 | 결과 |
|---|---|
| `cargo test --locked -p gil` | 1,438 통과, 2 ignored (부모 시험이 호출하는 crash fixture 1개, 명시적 구버전 바이너리가 필요한 시험 1개) |
| `cargo test --locked -p gil --features chain-core --test chains --test chain_closing --test chain_handoff --test cycles --test context --test will --test walk` | 154 통과 |
| `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib approved_plan` | 8 통과 |
| `cargo build --locked -p gil --all-targets` | 통과 |
| `cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety` | 통과 |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | 128 통과, 플러그인 번들 Core가 필요한 기존 시험 4 skip (새 승인 UI 4개 포함) |
| `node --test distribution/codex/*.test.mjs distribution/windows/*.test.mjs distribution/compliance/notices.test.mjs` | 127 통과, 실제 게시 archive 경로가 필요한 기존 시험 1 skip |
| `git diff --check` | 통과 |

전용 Core/MCP 시험은 무승인 거절·편집 시 승인 무효화·중복 응답·다른 scope·서버 재시작·수정 요청·저장 왕복·잘못된 바인딩·문서만으로 Verify 거절·Closing/후속 Chain을 포함한다.
UI 시험은 합성 DOM/이벤트 fixture다. 실제 물리적 버튼 클릭이나 Codex가 메시지를 받아 모델을 재개했다는 증거로 세지 않는다.

초기 UI 실행에서 새 worktree에 SDK 의존성이 없어 1개 파일을 불러오지 못했다. 동일 lockfile의 기존 개발 의존성을 준비한 뒤 전체 suite를 재실행해 통과했다. 실제 코드 결함으로 보고하지 않는다.
기본/개발 빌드에는 macOS에서 사용하지 않는 Windows launcher 항목의 기존 dead-code 경고가 있다.

## 남은 확인과 제한

- PR 검토, macOS/Windows GitHub CI, 병합은 아직 수행하지 않았다. Chain CI matrix에 이 프로필의 시험·빌드를 추가했다.
- 실제 Codex MCP App의 표시, 버튼 클릭, 서버 재연결, 채팅 계속 요청, Luna 실작업은 미검증이다. Windows 패키징 시험 통과를 Windows 네이티브 실행으로 표현하지 않는다.
- 기본 배포 프로필과 설치본·업데이트 피드는 바꾸지 않았다. 현재 설치된 사용자 프로젝트에는 자동 적용되지 않는다.
- 버튼 경로는 정상 MCP Host의 App/model 분리와 임시 토큰을 전제로 한다. 개인 신원 인증이나 악성 로컬 프로세스 방어가 아니다.
- Core는 GIL의 상태 전이·실행 연결·증거를 검증한다. 모델이 별도 외부 도구로 수행하는 모든 작업의 의미를 자동 판단하거나 차단하지는 않는다.
- Cycle 재시도 바인딩·항목 취소·abort는 후속 계약이다. 승인 계획 프로필에서는 Cycle revisit를 상태 변경 전에 거절한다. 기존 일반 Core의 반복 revisit와 제품 `product-safety` 차단은 유지한다.
- Claude 병렬 작업 범위는 [M3 프롬프트](https://github.com/hyun06000/gil/blob/a992efa43c10ef4dc3ad97699cb504663d07288a/coordination/CLAUDE-M3-SAFETY-PROMPT.md)에 정의했다. Claude가 실제 착수/완료했다고 주장하지 않는다.
