# GIL MCP fullscreen adapter

아래 날짜별 결과는 이전 개발 단계의 기술 기록이다. 새 source의 검증은
[SOURCE-MIGRATION.md](../SOURCE-MIGRATION.md)에 따로 기록하며 기존 설치본은 교체하지 않는다.

공용 `ui/`는 그대로 두고 이 개발 디렉터리에서 MCP App만 번들한다. Tauri의 `frontendDist`에
SDK, Node 의존성 또는 시험 HTTP server를 섞지 않는다. 설치본에는 생성 HTML과 Rust Core가 실린다.

```sh
npm ci --prefix mcp-app
npm run build --prefix mcp-app
npm test --prefix mcp-app
npm test --prefix plugins/gil-companion-prototype
cargo test --lib mcp::
cargo test --test mcp
```

`browser-check.mjs`는 **모의 Host**의 브라우저 회귀 시험이다. 개발 환경의 Playwright package를
`GIL_PLAYWRIGHT_MODULE`, 필요하면 Chrome 실행 파일을 `GIL_BROWSER_EXECUTABLE`로 지정한다.
공용 UI selftest도 이어서 실행한다. 이 시험의 fullscreen 성공을 실제 Host 지원으로 보고하지 않는다.

`plugins/gil-companion-prototype/monitor-stdio-check.mjs`는 `make-core.sh`로 지은 실제 묶음을
stdio로 연결해 임시 Project의 Agent walk, canonical View/Report, ui resource, 파일 변화 hint와
Project 전체 파일 지문 보존을 잰다. native 창을 열거나 사용자 Project를 바꾸지 않는다.

## 2026-09-23 개발 검증 기록

- MCP lib 7/7, 실제 stdio 11/11, Companion integration 31/31, Plugin 43/43,
  App adapter 5/5, 공용 브라우저 UI 62/62 통과.
- source 묶음과 최종 Codex 설치 cache 양쪽에서 17개 tool·Agent walk·resource·실제 파일 hint·
  complete View·전체 Project 파일 지문 보존을 실측했다. macOS arm64 Core release build 통과.
- 모의 Host에서 fullscreen/inline 응답, 지원 목록, 선택·Report 유지, 갱신과 native 버튼을 검증했다.
- 전체 Rust suite는 실행하지 않았다. 추가 `cargo build --workspace --all-targets`는
  장시간 컴파일 대기로 완료하지 못하고 해당 작업 프로세스만 종료했다. 전체 build 통과로 세지 않는다.
- 실제 Codex/Cowork의 제품 화면 인수는 아래 항목으로 남는다. 커밋·푸시는 하지 않았다.

## 실제 Host 인수 절차

최신 판정은 [ACCEPTANCE.md](./ACCEPTANCE.md)와 [NATIVE-ACCEPTANCE.md](./NATIVE-ACCEPTANCE.md)를
따른다. 아래는 공통 절차이며 모두 미검수라는 뜻이 아니다. 9/28 Codex의 Rust 설치본 화면과 대화
왕복의 선택·접힘·보고서 보존은 사용자 확인을 받았다. Tauri 전용 검증은 후속으로 미룬다.

1. 갱신된 Plugin의 새 대화에서 명시한 GIL Project에 `gil_monitor_prepare` → `show_gil_monitor`.
2. **모니터 펼치기** → 연결 정보의 Host 광고와 반환 mode가 fullscreen인지 확인.
3. Step 선택, 옆 요약 카드, 아래 전체 Report, Cycle 접기/펼치기를 확인.
4. Host 입력창에서 대화를 계속하고 scroll해도 화면·선택·접힘이 남는지 확인.
5. Agent가 그 Project를 진행하거나 작업 파일을 변경하면 완전 View와 선택 Report가 갱신되는지 확인.
6. **별도 창 열기**로 기존 Companion이 열리는지 확인. Project 선택은 각각 독립이다.
7. Codex와 Claude Desktop **Cowork**를 따로 기록. Code는 inline-only일 수 있다.

Cowork의 직접 등록 probe 성공은 Plugin 설치 경로 성공이 아니다. Windows, 수동 표시 선택의 대화 왕복,
Host 완전 재시작 및 `persistent_host` availability 통합은 별도 검수다. 이 조건들이 남아 있으므로
Companion을 삭제하지 않으며, PiP나 OS always-on-top을 지원한다고 주장하지 않는다.

## 재시작 연결 복원 (2026-09-25)

사용자가 계산기 제작→공학용 확장 중 실시간 Graph 갱신을 확인했다. 완전 재시작 후 상세가
`unknown_scope`로 거절되는 결함도 실측됐다. 서버 메모리만 쓰던 등록을 다음처럼 보정한다.

- `src/mcp/bindings.rs`: 명시적 준비 때만 사용자 설정에 canonical root와 `.gil` OS identity를 저장.
  scope별 원자적 파일, 0700/0600, 크기 제한, schema 검사, symlink 거절, 동시 연결 보존.
- 새 서버의 read/detail/poll은 요청 scope 한 개만 지연 복원한다. 새 root 입력이나 prepare 호출을
  요구하지 않는다. 폴더 교체는 새 scope이며 옛 화면이 새 Project로 연결되지 않는다.
- revision에 opaque 서버 세대를 넣어 재시작 직후 같은 counter 때문에 갱신을 놓치지 않는다.
- App은 복원 실패를 알리고 마지막 Graph를 남긴다. 영구 poll 중단 대신 bounded retry로 재확인한다.
- macOS 저장 위치: `~/Library/Application Support/GIL/monitor-bindings-v1/`. 개발 harness는
  `GIL_MONITOR_STATE_DIR`로 임시 설정을 쓴다. 프로젝트 안의 override는 디렉터리 생성 전 거절한다.
- 기존에 만들어진 화면은 저장 기록이 없으므로 업데이트 후 한 번 새로 연다. 그 이후 반복
  재시작에는 수동 재연결이 필요 없어야 한다. Windows identity는 별도 작업이다.

`cargo test --test mcp`는 **서로 다른 실제 프로세스** 사이에서 이름이 같은 두 Project의 Report와
View를 scope만으로 복원하고 폴더 교체를 거절한다. `monitor-stdio-check.mjs`는 Plugin 프로세스와
Rust 자식을 모두 종료·재실행한 뒤 재준비 없이 읽기·자동 변화 hint·Project 파일 불변을 검사한다.
`browser-check.mjs`는 과거 tool result를 재생하는 모의 Host에서 새 View와 재연결을 확인한다.
이들은 사용자 Codex 앱 자체를 종료한 실측을 대신하지 않는다.

### 이번 보정의 검증 결과

- Rust MCP 단위 15/15, 실제 stdio 13/13, Companion 통합 31/31.
- Plugin 43/43, App adapter 7/7, 공용 UI selftest 62/62 및 모의 Host 재시작·재연결 화면 검사.
- source와 Codex 설치본 양쪽에서 Plugin+Core 전체 재시작 → 기존 scope만으로 View·Report 복원 →
  실제 파일 변화 감지 → 완전 View 갱신 통과. 시험 Project 전체 파일 지문 불변.
- macOS arm64 release Core build 경고 없음. 전체 workspace build/test는 이번에 실행하지 않았다.
- 공식 cachebuster/reinstall 적용: `0.2.0+codex.20260925044007`.
  Core SHA-256 `0a5c30c4678ddbf362fc321b6b47a2ceb47f46b28b84e2de6c55396c36544da0`.
- 실제 Codex 완전 종료·재실행 검수와 Cowork·Windows 검수는 남긴다. 커밋·푸시하지 않았다.

## MCP App 표시 감지와 작업 재개 시 복구 (2026-10-30)

`gil_monitor_status(project_root)`는 이 MCP 연결에 있는 해당 Project App의 최근 표시 보고를 읽는다.
네이티브 Companion의 상태와 별개이며 Project 기록이나 설정을 쓰지 않는다. `gil_monitor_poll`의
선택 필드 `instance_id`, `sequence`, `presentation`은 App만 보내는 일시적 관측이다. 일반 poll/read,
View 준비나 파일 watcher 성공으로 표시를 추정하지 않는다. 표시 보고 자체는 인증된 OS 가시성 증거가 아니다.

- `visible`: renderer 준비 뒤 document가 숨겨지지 않았고 Host가 inline/fullscreen 모드를 보고했다.
- `hidden`/`closed`/`loading`/`error`: 해당 App의 마지막 상태. 종료 보고가 유실될 수 있다.
- `unknown`: 아직 보고가 없거나 마지막 신호가 15초보다 오래됨. 서버 재시작도 unknown부터 시작한다.
- `opening`: show 이후 최대 30초 동안의 표시 확인 대기. 화면 성공을 뜻하지 않는다.
- `unconfirmed`: 표시 응답 없는 show 두 번 이후. 추가 자동 카드를 만들지 않고 문제를 알린다.

성공한 Project 도구 호출(새 세션의 `gil_context` 포함)은 이 상태를 검사하고, 필요하면 정확한 scope의
`show_gil_monitor` 다음 호출을 응답에 넣는다. Agent는 사용자에게 별도 창을 요구하지 않고 이 호출을
따른다. 새 visible 보고는 재시도 예산을 회복하며, 정상 inline 화면을 fullscreen으로 강제하지 않는다.
다른 Project/다른 MCP 연결의 화면은 대신 세지 않는다. 임시 상태는 `.gil`에 저장하지 않는다.
Host가 한 MCP 연결을 여러 대화에서 공유한다면 현재 보고만으로 어느 대화의 App인지는 구분할 수 없다.

이 경로는 Agent가 GIL을 사용하거나 재개할 때 복구한다. Agent가 도구를 호출하지 않는 동안,
Codex가 종료된 동안의 강제 표시나 Host가 거절한 fullscreen을 보장하지 않는다. 숨김 이벤트가
Host에서 전달되지 않는 경우와 OS의 창 가림도 별도 실제 Host 검수가 필요하다.
