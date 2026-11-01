# C — Monitor 승인·진행 화면 결과

- branch: `claude/wave1-monitor-ux` (base `e89a32b`, M6-D `fcd6e5d` 포함)
- 결과 파일: `coordination/parallel-wave-1/results/C-MONITOR-UX-RESULT.md`
- 스크린샷·모의 Host: `coordination/parallel-wave-1/results/C-MONITOR-UX-screens/`
- 로컬 commit: 본문 끝 "커밋" 절 참조. push·PR·병합·배포는 하지 않았다.

## 요약

MCP App Monitor의 승인 패널(`mcp-app/plans.mjs`)을 상태 중심으로 다시 그렸다. 긴 그래프 위에
한 줄 배너가 "지금 무엇이 기다리는지"를 말하고, 버튼으로 승인 패널로 이동한다. 요청·새 판
재승인·승인 버튼 연결 없음·승인됨·수정 요청됨·승인된 계획·저장 결과 불확실·읽기 실패·승인
계획 없음을 구분한다. Closing proposal은 `chain_verdict`를 쉬운 한 문장으로 풀고 원문·참조·판·
digest는 접은 보조 정보로 보존한다. 기존 M6-D wire(`gil_plan_review`/`gil_plan_decide`)와 token
흐름은 그대로이며 새 서버 API나 schema 변경은 없다.

## 구현 범위 (지시 1–7 대응)

1. **상단 안내/바로가기.** `build.mjs`가 `<div class="notice">` 바로 뒤에 `#plan-banner`
   (`role="status"`, `aria-live="polite"`)를 둔다. 펼친 넓은 화면 grid에는 `plan-top` 행을 추가했다.
   배너의 "승인 화면으로" 버튼만이 이 adapter가 하는 유일한 scroll이다(`scrollIntoView` 후 첫 결정
   버튼 focus). polling은 wire 값·token 유무·전달 상태가 바뀔 때만 DOM을 다시 그리고 scroll하지 않는다.
2. **상태 구분.** `planSummary()`가 한 상태를 정한다: `requested`, `reapproval`(base_revision 또는
   active_revision이 있는 새 제안), `no_token`(제안은 있으나 `_meta` token 없음 → 버튼 비활성·자동 재조회),
   `approved`, `changes_requested`, `executing`(제안 없이 승인 판 존재), `uncertain`(decide 응답 유실),
   `unreadable`(review 실패), `idle`. `planItems()`가 항목을 실행함(cycle ref 표기)/지금 열린 Cycle/남음으로
   나누고 개수 요약을 붙인다. 제안이 없는 실행 상태에서는 `executed_cycles`·`current_item`·`pending`만으로
   목록을 만든다(제안 본문은 wire에 없음).
3. **chain_verdict 문장.** `verdictSentence()`: `success`/`failure`는 "전체 결과(Chain)를 …로 마치자는
   제안입니다. 승인하면 …로 끝내는 데 동의합니다", 없으면 "항목 N개를 담은 계획… Chain 종료 제안은
   아닙니다", 그 외 값은 따옴표로 보여 주고 해석하지 않는다. 제안문 전체(dl + JSON pre)와 판·제안 주소·
   SHA-256은 `<details>`로 보존한다.
4. **추측 금지.** 패널과 배너 문구에 "작업 중/진행 중"이 없다(시험으로 고정). 실행 상태는 "지금 열린
   Cycle에 묶인 항목", "제안 단계가 아직 열려 있습니다"처럼 기록된 사실만 말하고, 마지막 줄에 "에이전트가
   지금 작업 중인지는 여기서 알 수 없습니다"를 둔다.
5. **채팅 전달 실패.** 결정 저장 성공 후 `sendMessage` 실패는 `delivery={digest,'undelivered'}`로 이 화면에
   남아 다음 polling에도 지워지지 않는다(기존 코드는 다음 재조회에서 안내문이 사라졌다 — 아래 결함 1).
   수동 안내문("GIL 승인 결과를 확인하고 계속해 줘")과 "채팅에 계속 요청 보내기" 버튼을 제공한다. 이 버튼은
   trusted click에서만 `sendMessage`만 호출하고 `gil_plan_decide`를 다시 보내지 않는다. 재조회·렌더는
   어떤 메시지도 보내지 않는다. 이 화면이 아닌 곳에서 기록된 receipt에도 같은 안내와 버튼을 보인다.
6. **기존 계약 유지.** `event.isTrusted` 확인, token은 closure에만, `scope_id/revision/digest` 동일성,
   `current?.digest !== p.digest`인 stale 요청 거절, `busy`로 중복 클릭 차단, native `<button>`과
   `tabindex=-1` 제목으로 키보드 접근, 재렌더 시 같은 결정 버튼→제목 순으로 focus 복원(패널 밖 focus는
   건드리지 않음), `dead` 이후 늦은 응답 무시(패널·배너 비움). 결정 응답 유실 시 `token=null`로 두고
   `uncertain` 상태에서 "다시 읽기"(read-only)만 제공한다.
7. **합성 화면 점검.** 아래 "시각 점검" 절. Codex/Claude 실기 인수와 별개다.

접근성 변경: 패널의 `aria-live`를 제거하고(제안 전체가 매번 낭독되는 것을 막음) `role="region"`으로
두었다. 상태 낭독은 배너(`role="status"`)가 맡는다.

## 변경 파일

- `mcp-app/plans.mjs` — 재작성. export: `makePlanReview({app, scope, panel, banner, document, changed})`,
  `planSummary`, `planItems`, `verdictSentence`, `bannerLine`, `PLAN_STATE`, `MANUAL_CONTINUE`.
- `mcp-app/plans.test.mjs` — fixture Element 확장(dataset/focus/contains/scrollIntoView), 12개 시험.
- `mcp-app/app.js` — `banner: el('plan-banner')` 전달(한 줄).
- `mcp-app/build.mjs` — 배너 section 배치, 패널 `role="region"`.
- `mcp-app/monitor.css` — 배너·상태 배지·항목 상태·좁은 화면(≤480px) 전체 폭 버튼, fullscreen grid `plan-top`.
- `plugins/gil-companion-prototype/assets/monitor.html`, `monitor.json` — `node mcp-app/build.mjs` 생성물.
  sha256 `6ab64173f8a234a96d36f33eee285cee283c118b7761d889e010bf9b028ffc1d`.
- `coordination/parallel-wave-1/results/C-MONITOR-UX-screens/` — fixture 스크린샷 12장과 모의 Host
  (`mock-host/server.mjs`, `index.html`; 개발 점검용, 제품 코드 아님).

`ui/**`, Rust, `src/mcp/tools.json`, `package.json`/lock, notice-policy, `spec/`는 수정하지 않았다.

## 검증 명령과 결과

| 명령 | 결과 |
|---|---|
| `node --test mcp-app/plans.test.mjs` | 12 pass / 0 fail |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | 140 tests: 136 pass, 0 fail, 4 skipped (plugin core 시험 4개, "core 가 아직 실리지 않았다 — make-core.sh": 기존과 동일, 이번 변경과 무관) |
| `node mcp-app/build.mjs` | 701459 bytes · `6ab64173f8a234a96d36`; 두 번 실행해 생성물 동일(결정적) |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | 97 tests: 94 pass, **2 fail**, 1 skipped. 실패 2건은 모두 notice-policy의 입력 hash 불일치이며 아래에 정확히 적는다. 무력화하지 않았다. |
| `python3 coordination/parallel-wave-1/cargo-queue.py -- sh -c 'touch src/lib.rs src/mcp.rs src/mcp/surface.rs src/main.rs tests/mcp.rs plugins/gil-companion-prototype/assets/monitor.html && cargo test --locked -p gil && cargo build --locked -p gil --all-targets'` | `Compiling gil` 확인 후 43개 suite 모두 ok: **1438 passed, 0 failed, 2 ignored**(기존 ignored). 이어진 all-targets 빌드 Finished(exit 0). 실행 후 `target/claude-wave-1/debug/gil`에 `plan-banner` 26회, D 전용 문장 0회로 이 branch의 실행 파일임을 확인 |
| (참고) 강제 재빌드 없이 큐로 돌린 `cargo test --locked -p gil` | `tests/mcp.rs` 2 failed(아래 설명) — 남의 실행 파일을 시험한 결과라 이 branch의 증거가 아님 |
| `node plugins/gil-companion-prototype/sync-workflow.mjs` | 일치 (workflow.md 변경 없음) |
| `git diff --check` | 공백 문제 없음 |

Rust 시험은 `src/mcp/surface.rs`가 `monitor.html`을 `include_str!`로 포함하므로 새 번들로 컴파일·실행해야 한다.
**주의 — 공유 target의 stale 실행 파일(아래 결함 5).** 큐로 처음 돌린 `cargo test`는 전체 통과처럼 보였으나
전체 로그를 남긴 재실행에서 `tests/mcp.rs` 2건(`initialize_succeeds_and_says_who_is_answering`,
`the_app_resource_is_embedded_and_binds_both_metadata_keys`)이 실패했고, 원인은 시험이 띄운 `gil`
실행 파일이 **다른 worktree(D)의 소스**로 빌드된 것이었다(served instructions에 D의 workflow 문장이 있고
이 번들의 `plan-banner` 문자열이 없음). 그래서 이 branch의 소스를 `touch`해 강제로 다시 빌드한 뒤 같은 큐
슬롯 안에서 시험·빌드했고, 위 표의 Rust 결과는 그 강제 재빌드 실행의 값이다.

### packaging 실패 — 통합 담당자가 재생성할 hash

`distribution/compliance/notice-policy.json` `inputs`의 두 항목만 다르다. 의존성(Cargo.toml/Cargo.lock/
mcp-app/package-lock.json) hash는 동일하다.

| 입력 | 정책(기존) | 이 branch |
|---|---|---|
| `mcp-app/build.mjs` | `64d7c2ba06ab65b06211fe26fe617aeffbf8bb93158215a1c14f1e6e1913452c` | `7c388dc3c9c0ef5130b16ca21118f142785f55d471bd778d2997458659dc33b2` |
| `plugins/gil-companion-prototype/assets/monitor.html` | `186713fc28dc63a14e73b9b4eb7909be72bc6e745238711e837bb1554bbbb54e` | `6ab64173f8a234a96d36f33eee285cee283c118b7761d889e010bf9b028ffc1d` |

실패 시험: "real policy preserves MCP transition terms…"(build.mjs hash)와 "actual locked Core and bundled
UI match reviewed offline coverage"(위 두 hash). 번들에 새 npm 의존성은 들어가지 않았다(esbuild 입력은
기존 `@modelcontextprotocol/ext-apps` 뿐).

## 시각 점검 (합성 DOM · 모의 Host · Codex 실기 아님)

- 방법: `C-MONITOR-UX-screens/mock-host/`의 Node 정적 서버가 생성된 `monitor.html`을 iframe `srcdoc`로
  띄우고 MCP Apps postMessage(`ui/initialize`, `tool-result`, `tools/call`)를 흉내 낸다. View는
  `ui/fixtures/reading-one`, `gil_plan_review` 응답은 query `state=`로 고른 합성 값이다. Google Chrome
  `--headless=new --screenshot`으로 저장했다. 개인 정보·실제 `.gil`·절대경로는 없다.
- 관측 범위: 넓은 펼친 화면 1280×900(`mode=fullscreen`, grid 2열), 좁은 inline 420×900(iframe 폭 420,
  `max-width:480px` 규칙 적용). 실제 Host의 chrome·composer·테마 변수는 없다(중립 기본 토큰으로 렌더).
- 파일: `wide-requested-top`(배너+그래프), `wide-requested-panel`(판정 문장→제안문→범위→항목→버튼→접힌 원문),
  `wide-reapproval-panel`(v2, 이전 승인 판 v1, A 실행함), `wide-closing-panel`(chain_verdict success 문장),
  `wide-approved-panel`(승인 기록·전달 미확인 안내·재전송 버튼), `wide-executing-panel`(실행함/지금 열린
  Cycle/남음, 재인터뷰 기록, 배너 없음), `wide-unreadable-top`(읽기 실패 배너·버튼 없음·마지막 상태),
  `wide-notoken-top`(버튼 연결 없음), `narrow-*` 4장(배너 줄바꿈, 전체 폭 버튼).
- 확인한 것: 배너가 그래프 위에 보이고 상태별 색이 다르다. 900px 높이에서 제안 패널의 핵심 정보와 두 결정
  버튼이 한 화면에 들어온다(원문·digest는 접힘). 실행 상태는 현재 항목이 굵은 좌측 선으로 구분된다.
  trusted click이 필요한 `uncertain`·`undelivered`·focus 복원은 headless로 만들 수 없어 단위시험으로만
  확인했다(스크린샷 없음).
- 확인하지 않은 것: 실제 Codex/Claude Desktop 표시, 실제 버튼 클릭과 `_meta` token, Host 테마 적용,
  composer 겹침, 화면 낭독기 동작.

## 다른 세션·통합 담당자에게 전달할 인터페이스

- wire 변경 없음. `gil_plan_review`의 `supported/proposal/active_revision/current_item/pending/
  executed_cycles/reinterview_requests/close_report/document`와 `_meta['gil/planApprovalToken']`만 읽는다.
- D(안내 문구): 화면이 사용자에게 입력하라고 권하는 문장은 `MANUAL_CONTINUE` =
  "GIL 승인 결과를 확인하고 계속해 줘"다. 버튼이 보내는 채팅 문장은 기존과 같다("GIL Monitor에서 계획 vN
  (step:…)에 승인/수정 요청을 기록했습니다. gil_plan_read로 … close_report로 …"). 안내 문서가 이 두 문장을
  받아 `gil_plan_read`→`close_report` 흐름으로 이어 주면 된다.
- E(시나리오): App이 `gil_plan_decide`를 보내는 조건은 trusted click·token·동일 digest뿐이며 재조회로는
  보내지 않는다. 메시지 재전송 버튼은 `sendMessage`만 호출한다.
- 통합: A의 의존성 변경과 함께 notice-policy inventory/hash를 한 번 재생성한 뒤 packaging 시험을 다시 돌린다.
  `monitor.json`의 `resource_uri`가 바뀌므로 Plugin manifest 시험(`plugins/*.test.mjs`)은 이미 새 값으로 통과했다.

## 발견한 결함·제안 (범위 밖은 고치지 않음)

1. (이 파일에서 수정) 기존 `plans.mjs`는 결정 후 전달 실패 안내를 `panel.append`로 덧붙였는데, 다음
   `refresh()`가 `rendered=null` 상태에서 패널을 통째로 다시 그려 ≤30초 뒤 안내가 사라졌다. 지금은 전달
   상태를 closure에 보존해 재렌더에 포함한다.
2. (`ui/companion.css`, 소유 아님) iframe 폭 420px에서 머리줄 `.scope`의 버튼 네 개가 오른쪽으로 잘린다
   (`narrow-requested-top.png` 상단). `.scope { flex: none }`이라 줄어들지 않는다. 재현: 모의 Host
   `?w=420`. 영향: 좁은 inline 미리보기에서 "별도 창 열기"가 보이지 않음. 이번 변경과 무관한 기존 동작.
3. (wire 제안, 선택) 제안이 없는 실행 상태에서 항목의 `question/done_when`은 `current_item`과 `pending`
   에만 있어 이미 실행한 항목은 id와 cycle ref만 보여 준다. 승인 판 전체 항목(또는 `document`)을 함께
   주면 실행함 항목의 질문도 표시할 수 있다. 필요하면 `gil_plan_review`에 `active_items` 같은 읽기 전용
   필드를 제안한다. mock에만 두지 않았다.
4. (관찰) `gil_plan_review`는 Synthesis가 열려 있어야 `proposal`을 돌려주므로 "승인됨인데 제안 단계가
   아직 열림"을 화면이 추론한다. 명시 필드가 없어도 동작하지만 D의 안내가 같은 해석을 쓰는지 맞춘다.
5. **(공유 빌드 큐, 모든 세션에 영향·소유 아님) `cargo-queue.py`의 공유 `target/claude-wave-1`은 worktree를
   구분하지 못한다.** Cargo는 path package의 metadata hash에 경로를 넣지 않아 여섯 worktree의 `gil`
   artifact가 같은 `deps/gil-<hash>` 파일로 겹치고, fingerprint의 dep-info는 package 상대 경로라
   "마지막으로 빌드한 worktree"의 timestamp를 현재 worktree 파일 mtime과 비교한다. 결과: 다른 세션이
   더 늦게 빌드했으면 내 `cargo test`는 "fresh"로 판단해 **내 소스를 컴파일하지 않고 남의 실행 파일로
   시험**하며, 큐의 잠금은 이를 막지 못한다. 재현: 이 worktree에서 `cargo test` 후
   `target/claude-wave-1/debug/gil`에 `grep -a -c plan-banner` → 0, `grep -a -c 'Never repeat a call that
   already succeeded'`(D 소스에만 있는 문장) → 1. 영향: 어떤 세션의 Rust "통과"도 자기 소스에 대한 증거가
   아닐 수 있다. 임시 우회: 같은 큐 명령 안에서 `touch src/lib.rs …` 후 `cargo test`(이 결과 문서는 그렇게
   얻었다). 통합 담당자 제안: 큐가 worktree별 `CARGO_TARGET_DIR` 하위 폴더(예: `target/claude-wave-1/<branch>`)
   를 쓰거나, 명령 전에 `cargo clean -p gil` 없이도 재빌드를 강제하도록 `__CARGO_DEFAULT_LIB_METADATA`/
   `RUSTFLAGS=--cfg wave_<id>` 같은 worktree별 metadata 구분을 넣는다. 디스크 23 GiB 제약은 의존성 artifact를
   공유하고 `gil` crate만 분리하는 방식으로 완화할 수 있다.

## 미수행·막힌 조건

- 실제 Mac/Windows 설치본, Codex·Claude Desktop 화면, Luna 참여 시험은 통합 후 한 번에 한다.
- notice-policy hash 재생성은 통합 담당자 몫이라 packaging 시험 2건은 이 branch에서 실패한 채 둔다.
- 다른 세션 branch를 병합하지 않았고 push/PR/CI dispatch/배포를 하지 않았다.

## 커밋

로컬 commit 1개, branch `claude/wave1-monitor-ux`, base `e89a32b`. SHA는 이 파일을 담은 commit이므로
`git log -1 --format=%H -- coordination/parallel-wave-1/results/C-MONITOR-UX-RESULT.md`로 읽는다.
push·PR·병합·CI dispatch·배포 없음.
