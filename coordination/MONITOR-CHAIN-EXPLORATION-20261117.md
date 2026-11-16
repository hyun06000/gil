# Monitor Chain 탐색 — 변경 기록과 병합 요청 초안 (2026-11-17)

작업 브랜치 `claude/gil-monitor-chain-exploration-bc269c`, 기준 main `e7f3a7a`.
목표: 기존 읽기 응답(`current.chain_ref`·`chain`·`chains`, [Chain 저장 계약 「읽기와 UI」](../spec/GIL_Chain_Storage_v0.1.md))만으로
사람이 Monitor에서 **지금 어느 Chain에서 일하는지, 사이클 결과와 Chain 종료가 어떻게 다른지, 이전 Chain의 결과·인계 요약과
그 안의 사이클 기록을 어떻게 보는지**를 읽을 수 있게 한다. 서버 계약·저장 형식·`src/**`·공통 지침·규범 명세는 바꾸지 않았다.

## 변경 전

- Chain 사실은 `progress.mjs`의 「기록 상세」 한 줄(`Chain chain:C1 열림/닫힘 (verdict)`)과, 닫힌 Chain일 때 상단 상태 칩
  「완료 · 판정」 및 완료 조건 자리의 인계 요약으로만 보였다.
- 이전 Chain 목록·보고서·소유 사이클을 화면에서 볼 길이 없었다. `chains` 배열은 읽었지만 쓰지 않았다.
- 열린 Chain 안에서 사이클이 닫혔을 때 그 결과가 Chain의 결과처럼 읽힐 수 있었다(대비 문장 없음).

## 변경 후

- **현재 Chain 한 줄**(`.chain-now`)이 「어디까지 왔는가」 아래에 선다. 예: `지금 작업 중인 Chain: C3 (열림) · 이전 Chain 2개`,
  닫힌 Chain이면 `Chain C3 닫힘 · 성공 (success) · 이전 Chain 2개`. 보고서 없는 닫힘은 「보고서 없음」으로 적는다.
- **대비 한 줄**(`.chain-contrast`)은 뜻이 있을 때만 선다: 열린 Chain 안의 닫힌 사이클(「사이클 결과는 Chain의 결과가 아닙니다」),
  `next_actions`에 `close_chain`이 있을 때(「지금 닫을 수 있는 상태입니다. 아직 닫히지 않았습니다」), 닫힌 현재 Chain에
  `open_chain`이 있을 때(「다음 Chain은 새 인터뷰로 열 수 있습니다」). 가능성은 가능성으로만 말하고 버튼은 만들지 않는다.
- **Chain 목록**(`details.chain-list`)은 접혀 있고 펼치면 발급 순서의 버튼이 선다(`Chain C1 · 닫힘 · 성공 (success)`, 현재는 `· 현재`).
  누르면 아래 상세 패널에 그 Chain의 **보고서 원문 칸**(판정은 낱말+저장값, 인계 요약은 자르지 않음, 종료 근거, 저장된 그 밖의 키는
  이름 그대로), **소유 사이클 목록**(종류·상태·판정·현재 위치 표식; timeline에 없는 사이클은 「기록 조회 불가」), 기록 식별자가 선다.
  사이클을 누르면 기존 사이클 보고서 선택으로 넘어간다. 읽기 전용이며 Current·저장 기록은 바뀌지 않는다.
- Chain이 없는 기록(format 4/5)은 세 요소가 모두 `hidden`으로 남고 기존 화면이 그대로다.
- 선택 상태는 scope별 seat(`selectedChain`)에 산다. 새로고침은 새 View에서 상세를 다시 만들고 없어진 Chain은 선택을 비운다.
  조회 실패는 마지막 확인 기록을 유지하며 기존 낡음 표시가 선다. Project 전환은 화면을 먼저 비운다. 모든 글은 `textContent`다.

## 수정 파일

| 파일 | 내용 |
| --- | --- |
| `ui/chain.mjs` (신규) | 순수 투영: `chainModel`·`chainLine`·`chainContrast`·`chainListEntries`·`chainDetail`·`chainOfCycle` |
| `ui/companion.js` | 모듈 연결, `renderChain`/`renderChainDetail`/`selectChain`, seat·새로고침·전환 처리, `GIL_COMPANION.selectChain` 노출 |
| `ui/index.html`, `ui/companion.css` | `.chain-now`·`.chain-contrast`·`details.chain-list` 자리와 스타일(좁은 화면 줄바꿈 포함) |
| `ui/i18n/{en,ko,ja,zh-Hans,zh-Hant}.js` | `chain.*` 25개 문장 |
| `mcp-app/chain.test.mjs` (신규) | 합성 데이터 11개 시험: Chain 없음·하나·여러 개·열림·성공/실패 종료·긴 인계·보고서 없음·목록 밖 chain_ref·미지 판정·5개 언어 |
| `ui/selftest.js` | 화면 검사 76–80: legacy 무영향, 목록·상세·주입 글자 보존·사이클 이동·현재 위치 불변, 대비 문장, 새 View/실패/전환, 360px |
| `plugins/gil-companion-prototype/assets/monitor.{html,json}` | `node mcp-app/build.mjs` 공식 화면 빌드 결과 |
| `distribution/compliance/notice-policy.json`, `distribution/windows/compliance/notice-policy{,-arm64}.json` | 빌드된 `monitor.html` 입력 hash를 명시적으로 갱신(의존성 변화 없음, 컴플라이언스 README 절차) |

과거 게시 장부(`PREVIEW-30/31-PUBLICATION-*.md`)는 당시 게시본의 hash이므로 바꾸지 않았다.

## 검증 결과 (macOS arm64, Node 26.5.0)

| 명령 | 결과 |
| --- | --- |
| `npm ci --prefix mcp-app`, `npm ci --prefix plugins/gil-companion-prototype` | 설치 완료 |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | 변경 전 199 통과/5 skip → 변경 후 **210 통과/0 실패/5 skip** |
| `node mcp-app/build.mjs` | `monitor.html` 969,943 bytes · `f0f99323e91dcca25da7` |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | hash 갱신 전 2 실패(policy stale) → 갱신 후 **통과**; `node distribution/compliance/notices.mjs check` 통과 |
| `node --test distribution/windows/*.test.mjs` | 37 통과 |
| `ui/selftest.html` (로컬 정적 서버, 앱 내장 브라우저, 숨은 pane) | 새 검사 76–80 **5개 통과**. 전체 76/77; 실패 1건(64)은 변경 전 원본도 같은 환경에서 7·31·33·34·35·64가 실패하는 hit-test·viewport 검사로, 이번 변경과 무관 |
| 모의 화면 확인 | 합성 Chain 3개를 fixture 위에 얹어 데스크톱·375px에서 줄·목록·상세·가로 넘침 없음 확인 (스크린샷은 세션 안에서만 확인) |
| `git diff --check` | 공백 문제 없음 |
| `cargo test` | **미실행** — `src/**`·fixture 미변경, 공유 target 충돌 회피. CI에서 확인 |

실제 Codex·Claude Desktop 화면 인수, preview.31 필드 테스트, 실제 chain-core 프로젝트 데이터로의 확인은 하지 않았다.

## 남은 한계·의존 사항

- 그래프 자체에는 Chain 경계를 그리지 않았다(디자인 개편 아님). 이전 Chain의 사이클은 기존 timeline 그래프에서 `other` 관계로 보인다.
- `ui/fixtures`는 Rust 시험(`tests/companion.rs`)이 실제 Core 출력과 바이트 비교하므로 Chain fixture를 추가하지 않았다. chain-core
  프로필로 만든 실제 fixture가 생기면 selftest의 합성 View를 그 fixture로 바꿀 수 있다.
- Chain 자체의 시각 자료 조회는 계약에 없어 상세에서 호출하지 않는다. 종료 제안(Synthesis)의 진행 상태는 View에 없으므로
  `next_actions`의 `close_chain` 유무만 말한다.
- 다국어 문장은 작성자 번역이며 품질 검수가 필요하다.

## 병합 요청 설명 초안

제목: `Monitor: Chain 탐색 — 현재 Chain 한 줄·대비 문장·이전 Chain 보고서/사이클 읽기`

본문:

> 기존 Monitor 읽기 응답(`current.chain_ref`·`chain`·`chains`)만으로 Chain을 큰 작업 단위로 읽을 수 있게 한다.
> 기본 화면에는 현재 Chain 한 줄만 더하고, 사이클 결과·종료 가능성이 Chain 종료와 다를 때만 대비 한 줄이 선다.
> Chain 목록은 접혀 있으며 고른 Chain의 보고서 원문·소유 사이클·기록 식별자를 상세 패널에서 읽기 전용으로 본다.
>
> - 서버 계약·저장 형식·`src/**`·공통 지침·규범 명세·Chain 열기/닫기 버튼: 변경 없음
> - 새 모듈 `ui/chain.mjs` + 합성 데이터 시험 `mcp-app/chain.test.mjs`(11), selftest 76–80, 5개 언어 문장 25개
> - 공식 화면 빌드 갱신과 notice policy의 `monitor.html` 입력 hash 명시적 갱신(의존성 동일)
> - 검증: JS 210 통과, 배포·컴플라이언스·Windows 패키징 시험 통과, 앱 내장 브라우저 selftest 새 검사 5개 통과
> - 미실행: `cargo test`(CI), 실제 Codex/Claude 화면 인수, preview.31 필드 테스트(사용자 진행)
>
> 🤖 Generated with [Claude Code](https://claude.com/claude-code)
