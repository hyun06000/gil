# Rust 단일 MCP 실행점 — 개발 인수

아래 날짜별 수치·설치 hash는 이전 source 개발 단계의 기술 인수 기록이다. 새 저장소의 실행
결과로 재사용하지 않는다. 새 검증은 [SOURCE-MIGRATION.md](../SOURCE-MIGRATION.md)에 구분한다.

## 구현 경계

`gil mcp --serve` 하나가 기존 Agent 10개, Companion 2개, Monitor 5개 도구와
`ui://gil-monitor/<HTML digest>.html` 리소스를 제공한다. domain action은 CLI와 공유하는
`command` 모듈을 직접 호출한다. 자기 자신을 자식 프로세스로 실행하지 않으며 protocol stdin을
Report 입력으로 읽거나 프로세스의 작업 디렉터리를 바꾸지 않는다.

MCP의 Project는 명시한 정확 root다. CLI의 조상 탐색은 유지한다. 경로가 필요한 receipt도
요청 root를 기준으로 표시한다. Agent action의 `said`·`problem`·`exit_code`·`ok`와 도구별
쓰기 권한을 유지한다. Monitor는 여전히 읽기 전용이고 Companion 실행은 명시적 도구에만 있다.

HTML은 기존 공용 bundle과 byte 단위로 같고 Rust binary에 내장된다. 기본 가로보기와 fullscreen
자동 요청을 다시 구현하지 않는다. Host의 실제 렌더링 성공은 stdio 도구 성공과 별도로 검수한다.

## 재현 가능한 검증

저장소 root에서:

```sh
cargo test -p gil
node --test mcp-app/*.test.mjs ui/layout.test.mjs plugins/gil-companion-prototype/*.test.mjs
```

현재 MCP App 경로의 Core 검증은 `gil` package를 대상으로 한다. `cargo test --workspace`는
Tauri Companion까지 포함한 별도 검증이며 2026-09-28 사용자 결정으로 후속 유예한다.
공용 UI/read model 시험은 계속하고, Companion 변경·재배포 때 Tauri 검증을 재개한다.

Plugin source 디렉터리에서, `GIL_JS_BASELINE`에는 변경하지 않은 이전 승인 JS 설치본의 경로를
명시하고 `GIL_NATIVE_MCP`에는 새로 지은 Rust 실행 파일의 절대경로를 지정한다:

```sh
node native-parity-check.mjs
node monitor-stdio-check.mjs
```

- `native-parity-check.mjs`: 17개 도구의 전체 schema·metadata, UI resource bytes·MIME·CSP,
  실제 열기/닫기/복원/되돌아감과 거절을 이전 JS 표면과 비교한다. Rust 자식의 PATH에는 Node가 없다.
- 유일한 protocol 정규화는 `execution.taskSupport: forbidden`의 생략이다. 기존 JS SDK는
  생략의 기본값을 `forbidden`으로 정의하고 rmcp의 Tool은 이 구형 필드를 직렬화하지 않는다.
  다른 실행 정책은 시험이 거절하며 제품 필드 차이는 숨기지 않는다.
- `monitor-stdio-check.mjs`: 실제 server 재시작 뒤 scope만으로 View·Detail 복구, Project bytes
  불변, 실제 파일 변경 hint→View 갱신, Agent 잠금 해제를 검사한다. macOS watcher는 정상 실행
  환경에서 검사한다. 샌드박스에서 OS 이벤트가 차단된 결과와 제품 결함을 구분한다.

## 설치 전 게이트

응답 동등성·회귀시험을 통과하기 전에는 동작 중인 Plugin을 교체하지 않는다. 패키징은 새 binary의
architecture·descriptor·fresh challenge를 확인한 뒤 원자적으로 교체하고, Codex는 공식
cachebuster/reinstall 절차를 사용한다. cache·registry는 손으로 편집하지 않는다.

`package-native.mjs <codex|claude> <새 zip 경로>`는 각 Host manifest, binary, 공용 Skill,
LICENSE, 개발 상태 안내만 싣는다. Node는 개발 시험/패키징 도구일 뿐 설치물의 실행 의존성이 아니다.
일반 사용자를 위한 원격 marketplace·자동 업데이트·서명·공증 완료를 의미하지 않는다.

## 아직 별도로 확인할 것

- Claude Plugin 업로드 경로의 표시, Host 작업 전환·완전 종료 뒤 화면 수명.
- 개발 도구가 없는 새 Mac의 설치 경험과 서명·공증된 공개 배포물.
- Windows 실행 파일·manifest·권한·파일 identity·fullscreen/fallback.

## 2026-09-27 실행 증거

- debug와 packaged release binary 모두: 17개 tool contract·내장 UI bytes 일치, JS/Rust 실제
  응답 70개 일치. restore·revisit 성공과 잘못된 입력/Report 거절 포함.
- release Core SHA-256: `d18c7c38fe65e1c6b2ae1d31ab10f475101fd5746452d2de16ad06084b0aa0c0`.
  최적화 빌드 warning 0. 개발자 home 문자열 없음, 동적 의존성은 macOS 시스템 라이브러리뿐.
- Claude zip 5개 파일, 2,710,465 bytes, SHA-256
  `11c16f5e022870139180ec571be55d71d2595361729af5e19fbcad6f5fd95c39`.
  새 임시 폴더에 풀어 실행 비트 0755 확인. ZIP에는 Node·npm package·UI 외부 파일이 없다.
- 그 압축 해제본을 macOS sandbox에서 실행: source repo·Codex cache·Homebrew(Node 포함)·
  cargo·rustup·전역 gil 읽기 차단 + Node 없는 PATH. 17 tools, Agent walk, 내장 resource 조회,
  서버 완전 재시작 후 scope-only View/Report 복구, 실제 파일 hint→dirty View 모두 통과.
  Project fingerprint 동일, watcher 동안 Agent status 잠금도 풀려 있음.
- 기존 JS fake Core 분류시험에서 새 임시 shell script의 첫 실행이 5초를 넘기는 경우를 관측했다.
  같은 실패 fixture의 재호출은 4ms에 정확한 `identity_mismatch`로 응답했다. 분류시험만 fixture
  사전 실행을 거친다. 제품의 5초 probe·실제 bundled Core 첫 호출·실행 횟수 시험은 바꾸지 않았다.

### 회귀시험과 설치 완료

- `gil` package: **1,175 passed · 0 failed · 0 ignored**, 28 targets. library 408,
  integration 766, doctest 1 (main test target는 0). `cargo test -p gil`과 나머지 target/doc
  실행 로그를 target별로 한 번만 집계했다.
- 첫 실행의 MCP 시험 하나는 malformed scope를 domain의 unknown scope로 취급하던 옛 기대 때문에
  실패했다. malformed 입력은 JSON-RPC `-32602`, 올바른 모양의 미등록 scope는
  `reconnect_required`임을 각각 검사하도록 고쳐 MCP 16/16 재실행 통과. 제품 코드는 바꾸지 않았다.
- UI/Plugin Node 시험 **69/69**, 공식 Plugin validator 통과, `git diff --check` clean.
- `cargo test --workspace`는 Companion test binary 컴파일 지연으로 중단했으므로 전체 workspace
  통과로 세지 않는다. 위 수치는 변경한 `gil` package의 전체 unit/integration/doc 범위이며,
  별도 Tauri `gil-companion` package 시험은 이번에 완료하지 않았다.
- Codex 공식 cachebuster/reinstall 완료: **`0.2.0+codex.20260927134559`**.
  설치된 manifest는 `./core/darwin-arm64/gil mcp --serve`를 실행한다. 설치 binary는 위 release
  digest와 같고 실행 권한은 0755다. cache·registry 수동 편집은 없다.
- 그 **설치본**으로 `monitor-stdio-check.mjs` 재실행 통과: 17 tools, 실제 Agent walk, 내장 resource,
  프로세스 완전 재시작 뒤 old scope만으로 View/Report 복원, 파일 hint→dirty View,
  Agent status 잠금 해제, Project fingerprint 불변.

## 2026-09-28 사용자 화면 인수와 우선순위

새 Codex 대화에서 기존 Project를 열어 가로 기본값·fullscreen 실제 표시·노드 상세를 확인해 달라는
요청 뒤 사용자가 “잘된다”로 승인했다. 새 Rust 설치본의 macOS Codex 화면 인수를 통과로 기록한다.
프로토콜 시험이나 이전 JS 설치본의 승인을 옮겨 적은 것이 아니다. Cowork Plugin·Windows 및
작업 전환·완전 종료 후 화면 수명은 이 최초 승인에 포함하지 않는다. 이후 대화 왕복의 선택·접힘·
보고서 보존은 별도 사용자 확인을 받았으며 아래에 구분한다.

사용자는 Tauri 검증을 미루고 MCP App 통합에 집중하기로 결정했다. 따라서 다음 인수는
`ACCEPTANCE.md`의 MCP App 대화/작업 전환·재시작 절차와 Cowork Plugin 설치 경로다.
Companion은 삭제·자동 실행하지 않고 fallback으로 보존한다. 미완료 Tauri 시험은 그대로 미완료이며
MCP App 진행을 막는 선행 조건에서는 제외한다. Claude zip은 준비만 했고 업로드하지 않았다.
이 우선순위 기록 당시에는 실행 코드·설치본·Project 데이터를 변경하지 않았다.

## 2026-09-28 대화 복귀 확인·상세 조회 실패 보정

사용자가 다른 대화로 이동했다 돌아온 뒤 선택·접힘·보고서가 “그대로 유지돼”라고 확인했다.
macOS Codex의 대화 왕복 인수로 기록한다. 수동 inline/세로 선택, Host 완전 종료 후 화면 수명,
Cowork Plugin·Windows 인수는 이 결과에 포함하지 않는다.

추가 브라우저 시험에서 같은 Step의 상세 재조회 실패 시 마지막 보고서가 빈 안내문으로 바뀌는
결함을 먼저 재현했다. 공용 renderer만 보정했다:

- 같은 Step의 마지막 보고서는 보존하고 최신 내용은 확인하지 못했다고 알린다.
- 다른 Step 선택 시 이전 보고서를 즉시 비운다. 같은 Step의 중복 요청은 최신 요청만 반영한다.
- Project를 떠난 요청은 같은 scope로 돌아와도 응답·오류를 새 화면에 적용하지 않는다.
- 성공하면 상세 오류만 걷고 다른 동작의 오류는 남긴다. 선택 소멸·목록에서 빼기 시 상세 안내도 정리한다.

최종 검증:

- 공용 브라우저 UI **70/70**, 신규 65–70 여섯 사례. 모의 Host의 실패→보고서 보존→자동 회복 통과.
- UI/Plugin Node **69/69**, 공식 Plugin validator 통과, `git diff --check` clean.
- arm64 release Core 재빌드 warning 0. Rust 소스·domain·wire schema는 이번 턴에 바꾸지 않았고,
  이전 1,175개 Rust suite를 이번에 재실행한 것으로 세지 않는다. Tauri build/test도 실행하지 않았다.
- 실제 최종 Core stdio: 17 tools, 서버 재시작 후 old scope로 View/Report 자동 복구,
  실제 파일 hint→dirty View, Agent 잠금 해제, 시험 Project 전체 파일 지문 불변 통과.
- release Core SHA-256: `5cc5b19657c98e81757613bc7b5c5c8c5734eb9a9309c019d660ffccf96332e8`.
- 내장 UI SHA-256: `919fae5c9ac31c3e436b6d21373b1cf2fb87bd18b872ee40c80a75195aea00d9`.
  resource: `ui://gil-monitor/919fae5c9ac31c3e436b.html`.
- Codex 공식 cachebuster/reinstall: `0.2.0+codex.20260927171057`. 설치 Core와 내장 resource를
  source 산출물과 byte 단위로 대조했고, 이 최종 설치본으로도 위 stdio 재시작·파일 감시 시험이
  통과했다. marketplace/cache/registry 수동 편집은 하지 않았다.

기존 사용자 변경과 Companion 설치물은 유지한다. Claude zip은 이번 보정을 반영해 다시 만들거나
업로드하지 않았다. 커밋·push·공개 배포는 하지 않았다. 갱신된 Codex Plugin은 새 대화에서 사용한다.

## 2026-09-28 추가 디자인 보정 — 위쪽 두 꺾임·composer 여백

사용자 요청에 따라 가로 revisit을 상단→상단의 두 꺾임으로 단순화했다. 기존 선이 실제 생성 edge와
같은 선분을 타는 실패를 먼저 시험으로 재현했고, 모든 fixture 접힘 조합에서 이를 제거했다.
선택된 Step의 커진 원과 접힌 Cycle의 반지름도 반영한다. 세로형의 경로 규칙은 바꾸지 않았다.
MCP fullscreen에만 200–320px + 기기 safe-area 하단 스크롤 여백을 두었다. 보고서를 줄이거나
Host 채팅창을 조작하지 않는다.

- Node UI/Plugin **70/70**, 공용 브라우저 UI **70/70** 통과. UI selftest 64는 실제 node 상단과
  아래 화살촉의 착지, 두 꺾임, 접기 control 비겹침을 세 폭에서 검증한다.
- iframe **밖**의 모의 채팅 입력창(160/220px)을 올리고 1280×900·900×650·600×480에서
  긴 Report 마지막 줄을 가리지 않는 위치까지 스크롤·hit test 확인. inline 여백은 0px다.
  스크린샷도 확인했으나 실제 Claude Host의 겹침 해결 인수로 대신하지 않는다.
- release Core build warning 0, source 및 최종 설치본의 실제 stdio 재시작·파일 감시·Project 지문
  보존 시험 통과. Rust 소스·domain·schema는 무변경이며 전체 Rust/Tauri suite는 재실행하지 않았다.
- Core SHA-256: `3aaa2348527381e78320c73a4bb86a210b3982b3f048a94aabdfc40ae24a3302`.
- UI SHA-256: `0087beb63eafd344d5bd012572ed9fcbf8af8f640620bfb13860bcd1c3215426`.
  resource: `ui://gil-monitor/0087beb63eafd344d5bd.html`.
- Codex 공식 검증·재설치: `0.2.0+codex.20260927172317`. 설치 Core와 실제 반환 resource가 source
  산출물과 byte 단위로 같음을 확인. cache·registry 수동 편집, Host 강제 종료 없음.
- Claude 개발 ZIP을 새로 만들었다: 5파일, 2,711,166 bytes,
  SHA-256 `f57d50c83bee70cd96ad1f19d5e54de0f9d2a0fc5d41f634128205b7b73abccc`.
  압축 검사 및 새 임시 폴더 왕복 후 실행 권한·descriptor·Core digest 일치 확인.
  이 ZIP은 준비만 했으며 Claude에 업로드·설치하지 않았다. 서명·공증된 공개 배포물이 아니다.

사용자 실제 화면 확인은 열린 인수 항목으로 둔다. Companion 설치물·사용자 Project 기록은
건드리지 않았고, 커밋·push는 하지 않았다.

## 2026-09-28 통로 간격 후속 보정

사용자 요청으로 가로 Graph 첫 lane 중심→가장 가까운 return 통로를 52px에서 28px로 줄였다.
일반 node의 위쪽 직선은 45.5→21.5px다. 두 꺾임·상단 착지·겹친 경로의 별도 통로와 세로형은
유지했다. 모든 fixture 접힘 조합에서 Cycle 경계 12px, 접힌 원 화살촉 전 7px 이상을 확인했다.

- Node UI/Plugin **71/71**, 공용 브라우저 **70/70**, 모의 composer 여백 시험 통과. 축소된 Graph
  스크린샷 확인; 실제 Host 검수와 구분한다. release build warning 0, Rust/Tauri 전체 suite 미재실행.
- 공식 Codex 재설치 `0.2.0+codex.20260927173113`; 설치본 stdio 재시작·실제 파일 감시·읽기 전용
  시험 통과. 설치 binary 및 반환 resource가 source 산출물과 동일하다.
- Core SHA-256 `2b8f56cd6b8bdcb2ae09707afa790394e6b6ea8ac50532e0b980dd220d97a12e`.
  UI SHA-256 `56e0d10e71bf2325ad152a91b47c9f60112cd113a6f17e46a9db2eb1518e5710`.
- Claude 개발 ZIP `gil-claude-macos-arm64-compact.zip`, 2,711,107 bytes,
  SHA-256 `bf302b588e87e3262a95861ac3ef4694b88fb35d787e9ffb92f26adacfb012cb`.
  압축 무결성 통과, 준비만 했으며 Claude 업로드·설치 및 공개 배포는 하지 않았다.

## 2026-09-28 카드 연결 화살촉 보정

몸통이 화살촉 끝까지 이어지고 round cap이 2.5px 더 돌출하던 오류를 고쳤다. 공용 `cardArrow()`가
몸통을 촉의 밑변에서 끝내고, 촉은 몸통 방향에 맞춰 9px 돌출한다. butt cap과 group opacity로
끝점 돌출·반투명 중첩을 없앴다. 가로·세로·대각선 및 접힌 원에 같은 계산을 사용한다.
선택된 Step의 실제 반지름에서 시작하며, 카드와 촉 사이 5px 간격은 유지한다.

- Node UI/Plugin **73/73**, 공용 브라우저 **70/70**. selftest 6·64가 실제 SVG 몸통 끝·촉 밑변·
  선택 node 가장자리·카드 간격을 잰다. MCP App 모의 Host 스크린샷과 하단 여백 회귀도 확인했다.
- release build warning 0. Codex 공식 재설치 `0.2.0+codex.20260927174145`, 설치본 stdio 재시작·
  실제 파일 감시·읽기 전용 시험 통과. binary와 반환 UI resource가 source 산출물과 동일하다.
- Core SHA-256 `b06ede5e632d1fdd9a52ef8840a3fa2f17650e1080be383aab873ce58db81048`.
  UI SHA-256 `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688`.
- Claude 개발 ZIP `gil-claude-macos-arm64-card-arrow.zip`, 2,710,860 bytes,
  SHA-256 `7b025190e6a03bc447eda243d1cff40c37956fe4e1b63452325b43804ce915f5`.
  압축 무결성 통과; Claude에 업로드·설치하지 않았다. 이전 ZIP을 덮어쓰지 않았다.

Rust source·domain·사용자 Project·Companion 설치물은 무변경이며 전체 Rust/Tauri suite는
재실행하지 않았다. 실제 Host의 새 화살표 인수는 별도다. 커밋·push하지 않았다.

## 2026-09-28 개발 배포 후보 체크포인트 — 공개 배포 전

이번 검수는 위 구현 변경을 추가하지 않고 기존 변경을 감사하고 회귀시험을 다시 실행했다.
기준 HEAD는 `77b796a8d575110d656253b530b9c46b78dac9fa`이며, 시작 시 변경 경로 65개와
194개 tracked/untracked 파일의 지문을 보관했다. 이번 저장소 편집 범위는 인수 문서 두 개와
Roadmap뿐이다. 기존 소스·설치본·사용자 Project·Companion을 교체하지 않았다.

| 이번에 실행한 검증 | 결과 |
| --- | --- |
| `cargo test --locked --offline -p gil` | **1,175 passed · 0 failed · 0 ignored**, 28개 결과 집계, doctest 포함 |
| `cargo build --locked --offline -p gil --all-targets` | 성공, warning 0 |
| 경로 remap을 적용한 macOS arm64 release build 확인 | 성공, warning 0; 기존 release cache 사용 |
| Node UI·Plugin·layout 시험 | **73/73**, skip 0 |
| 모의 Host 브라우저 및 공용 UI selftest | **70/70** + 재연결·Report 보존·외부 composer 여백 통과 |
| 공식 Plugin validator | 통과 |
| 실제 UI builder 재실행의 byte 비교 | HTML·metadata 모두 현재 산출물과 동일; 산출물 덮어쓰기 없음 |
| 설치본 stdio | 17 tools·old scope 재시작 복구·실제 파일 hint·Project 지문 불변 통과 |
| 개발 ZIP 압축 검사·해제 | 5파일, 실행 권한 0755, descriptor·Core digest 일치 |

정상 실행 환경의 결과다. 최초 sandbox 실행은 loopback `EPERM`으로 Rust 358 pass / 50 fail에서
중단됐고 브라우저 bind와 OS watcher도 제한을 받았다. 같은 시험의 정상 환경 재실행 결과와
분리했으며 실패 로그를 보존했다. `gil` package의 시험이지 Tauri package 또는 전체 workspace
통과가 아니다. 기존 JS/Rust 70개 응답 동등성은 과거 인수 기록이며 이번에 다시 센 숫자가 아니다.

개발 ZIP 해제본의 최초 MCP 시험에서 **30초 요청 timeout 1회**가 있었다. 같은 binary로
요청별 시간만 기록한 재시험과 계측 없는 원본 재시험은 모두 통과했다. 최초 실패 요청과 원인을
특정하지 못했으므로 수정 완료로 닫지 않는다. 별도 테스트 실행 파일의 1초 스택 샘플에는
`_dyld_start` 대기가 있었고 `--list` 실행에도 시작 지연이 관찰됐지만, ZIP timeout의 원인 확정은
아니다. 새 기계의 최초 다운로드·실행 시간과 서명·공증 검증을 공개 배포 게이트로 유지한다.

고정한 개발 산출물:

- Codex macOS arm64 ZIP: `gil-codex-macos-arm64-development.zip`, 2,711,147 bytes.
- ZIP SHA-256: `e925b8fd19ce143cb665c52c06c18e351e11e8d9a32e4158364116c2fe79e386`.
- Core SHA-256: `b06ede5e632d1fdd9a52ef8840a3fa2f17650e1080be383aab873ce58db81048`.
  release 산출물·source Plugin Core·기존 Codex 설치본·해제본이 동일하다.
- UI SHA-256: `0aaf30e3c62d63fdc45919d01ece01c1ea2ac2464bf572e72b4a4d4b87b52688`.
  URI: `ui://gil-monitor/0aaf30e3c62d63fdc459.html`.
- ad-hoc signature, TeamIdentifier 없음. **Developer ID 서명·공증된 공개 배포판이 아니다.**
  ZIP은 검증용으로만 생성했으며 설치본을 재설치하지 않았다.

사용자가 Host 재시작 뒤 수동 재연결 없이 Monitor·Graph가 다시 보임을 확인했고, 마지막 상태가
완전히 같지 않아도 충분한 UX라고 승인했다. 이를 기본 재시작 인수로 반영하며 완벽한 선택·접힘
복원을 주장하지 않는다. Claude 작업 Plugin 화면은 별도 차단점으로 남는다:
[상위 이슈 #274에 제출한 대조 증거](https://github.com/anthropics/claude-ai-mcp/issues/274#issuecomment-5858816977)는
게시 후 원문 일치 확인까지 끝났지만 수정 확인이 아니다. 직접 연결 fullscreen 성공을 Plugin
설치 경로의 성공으로 대체하지 않는다.

커밋·push·release tag·공개 게시·원격 저장소 생성은 하지 않았다. 다음 조각은 Codex의 자족형
원격 marketplace artifact와 검증 pipeline이며, 최초 실행 지연·서명/공증·새 기계 인수를
닫기 전 공개 배포 완료로 표시하지 않는다. macOS 배포 후 Windows로 진행한다.
