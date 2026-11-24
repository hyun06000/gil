# 제품 빌드 — OS마다 하나의 사용자 설치물 (후보, 게시 아님)

제작자용 문서다. 사용자 설치 과정에서 Node·Cargo·소스 checkout을 요구하지 않는다.
계약은 [배포 명세 §6.2d](../../spec/GIL_Distribution_Model_v0.1.md)가 정본이고, 이 문서는 그 구현 후보의
기능 대응표·제작·검증 절차다. **이 문서와 그 안의 표는 빌드·시험·설치·게시가 수행됐다는 기록이 아니다.**
실행 결과는 실행한 사람이 명령·대상·통과/실패/미실행을 구분해 따로 남긴다.

| 대상 | target | 설치물 | 제작 |
|---|---|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin` | 기존 Plugin marketplace 묶음(같은 identity) | `../codex/build-preview.mjs <out> --product` |
| Linux x64 GNU | `x86_64-unknown-linux-gnu` | 사용자 단위 설치 묶음(`bin/gil` + 고지 + [INSTALL.md](../linux/INSTALL.md) + `release.json`) | `build.mjs <out>` |

두 설치물의 기능 집합은 같다: `tracking-policy,exploration-flow,visual-authoring,product-safety`.
`runtime-testbed`·`recovery-auto-approval`·`--all-features`는 쓰지 않는다. 기능별 edition·프로필·다운로드는 없다.

## 1. 기능 · 계약 · 플랫폼 대응표

「근거」는 코드와 계약의 자리다. 「Linux」 열의 「이식」은 이번 후보가 바꾼 것, 「공통」은 플랫폼 분기가 없는 것,
「제한」은 기존 계약이 플랫폼을 한정해 그대로 둔 것이다(이름만 내놓고 성공하는 척하지 않는다 — 도구는 거절로 답한다).

| 사용자 기능 | 도구 · 근거 | 저장 계약 | macOS arm64 | Linux x64 |
|---|---|---|---|---|
| Chain · Interview · Experiment · Step 기록 | `gil_open`·`gil_close`·`gil_status`·`gil_context`·`gil_story` (`src/mcp/actions.rs`) | 두 종류 모두 | 기존 | 공통 |
| 승인 계획(제안 → Monitor 결정 → 정확한 close report) | `gil_plan_propose`·`gil_plan_read`·`gil_plan_request_interview`, App 전용 `gil_plan_review`·`gil_plan_decide` (`src/mcp/plans.rs`, Approved Plan 계약) | 두 종류 모두 | 기존 | 공통 |
| 탐색: 승인 요구에 연결한 작은 Experiment | `gil_open(kind: experiment)`의 요구 연결 (`src/project.rs`, Cycle Model) | 두 종류 모두 | 기존 | 공통 |
| 원본 보고서 · 근거 조회 | `gil_report_read`·`gil_evidence_read` (`src/mcp/records.rs`) | 두 종류 모두 | 기존 | 공통 |
| 단일 Will 종료 이력 · 미달성 종료 | `gil_close`의 `unachieved_will` (`src/mcp/surface.rs`, 저장 전환 계약 §4) | 두 종류 모두(형식 9·10) | 기존 | 공통 |
| 명시적 형식 전환(6→8, 7→9) | `gil_format_transition_check`·`gil_format_transition_apply`(프로젝트별 `user_approved` 필요) (`src/mcp/transition.rs`) | 기존 6·7 기록만 | 기존 | 공통(시각 7→9의 비공개 저장 검사는 Unix 경로) |
| **이미지 보고서 새 시작** | `gil_start(start_kind: image_reports)` → `src/command.rs::start_visual_action` → `src/visual_start.rs` | 형식 9 | 기존 | **이식**: 예약(`mkdirat` 0700, 비추종 열기, dev/ino 정체성), 로컬 저장 검사(statfs 유형 목록), 배타 최초 공개(`renameat2` `RENAME_NOREPLACE`) |
| PNG 작성 · 보관 · 선언 · 인용 | `gil_close`의 `visuals`, `gil_report_visuals` (`src/mcp/authoring.rs`, 시각 작성 계약) | 형식 9 | 기존 | 공통(`src/visual_report/file.rs`는 Unix 공통) |
| Monitor에서 시각 자료 읽기 | App 전용 `gil_monitor_report_visuals`·`gil_monitor_report_image` | 형식 9 | 기존 | 공통 |
| 기존 형식 4 → 시각 형식 이관 | `gil_migration_*` (`src/visual_migration.rs`, 시각 이관 계약) | 기존 형식 4 기록 | 기존(macOS 전용) | **제한**: 도구는 보이지만 거절한다. 이번 범위가 아니다 |
| **추적 범위 승인 새 시작** | `gil_start(start_kind: tracked_scope)` → `gil_start_prepare`·`gil_start_status`·`gil_start_cancel`, App 전용 `gil_start_review`·`gil_start_decide`·`gil_start_apply`·`gil_start_list` (`src/start_prep`) | 형식 10 | 기존(개발 구성) | 공통(Unix) |
| 같은 Chain 범위 변경 · 보관 대기 · 명시 재시도 | `gil_scope_propose`·`gil_scope_status`, App 전용 `gil_scope_review`·`gil_scope_decide`·`gil_scope_retry` (`src/scope_change`, #323) | 형식 10 | 기존(개발 구성) | 공통(Unix) |
| O-2 즉시 보관 중단 · 명시 해제 | App 전용 `gil_scope_halt`·`gil_scope_release` | 형식 10 | 기존 | 공통(Unix) |
| O-3 일반 Interview 보존 · 최종 종료 보호 | 종료 Synthesis 승인의 거절(`src/scope_change`) | 형식 10 | 기존 | 공통(Unix) |
| 닫힌 실패의 보존·복구 승인(제품 보호) | App 전용 `gil_revisit_review`·`gil_revisit_apply` (`src/mcp/recovery.rs`) | 형식 9 | 기존 | 공통. **형식 10/11은 restore·revisit을 실행 전에 거절**(현재 제한 그대로) |
| 일반 restore · Step revisit | `gil_restore`·`gil_revisit` | — | 제품 보호로 차단(기존) | 같음 |
| Monitor(MCP App) | `show_gil_monitor`·`gil_monitor_*`, 내장 HTML(`src/mcp/surface.rs`) | 두 종류 모두 | 기존 | 공통. 화면 표시는 Host의 MCP App 지원에 달려 있고 이 빌드가 보증하지 않는다 |
| 지속형 Companion 창 | `gil_companion_status`·`show_gil_companion` (`src/launcher.rs`) | — | 선택 기능(별도 설치) | **없음**: `this_machine()`이 `None`, 도구는 미지원으로 답한다. Core 기능과 Monitor는 영향받지 않는다 |
| 사용자 설치 · 제거 | `gil install`·`gil doctor`·`gil uninstall` (`src/install`) | — | 사용자 채널은 Plugin 묶음(기존) | **이식**: `~/.local/share/gil/bin/gil`, 남의 항목은 덮지 않음, 기록된 파일만 제거, Codex만 등록, Claude Desktop은 찾지 않음 |
| 런처의 Core 실행(fork/execve) | `src/runtime_launch.rs` | — | 기존 | **이식**: errno 접근만 플랫폼별(`__error`/`__errno_location`) |

형식 11(추적 범위 + 시각 등록부)은 이 빌드가 읽을 수 있는 범위에 있지만 **어떤 시작도 만들지 않는다.**
영수증의 `compatibility.storage_format`(읽기 범위 4..11)과 `start_kinds`(새 기록 10·9)는 그래서 따로 적힌다.

## 2. 새 시작 선택 계약

입력은 `gil_start`의 선택 문자열 `start_kind` 하나다(`tracked_scope` | `image_reports`).
판정은 `src/mcp/start_kind.rs`가 호출마다 한 번 하고, 판정하는 동안 아무것도 만들지 않는다.

| 폴더의 상태 | 입력 없음 | `tracked_scope` | `image_reports` |
|---|---|---|---|
| `.gil` 없음 | 거절 `start_kind_required`(두 선택지 안내, 무기록) | 시작 준비 | 즉시 생성 |
| 시작 준비 또는 그것이 된 프로젝트(`.gil/start`) | 그 준비를 이어 감 / 이미 시작됨을 알림 | 같음 | 거절 `start_kind_conflict` |
| 다른 시작의 프로젝트(`state.yaml`, 준비 없음) | 읽기만 하고 거절 | 거절 `already_started` | 읽기만 하고 거절 |
| 출처를 가릴 수 없는 `.gil` 잔여물 | 거절(보존) | 거절 `start_record_unsettled` | 거절(보존) |

- 한 종류만 가진 빌드는 종전과 같다. 기존 배포 프로필의 `gil_start` schema는 바이트 그대로이고 `start_kind`는
  알 수 없는 입력으로 wire에서 거절된다. 추적 범위 전용 개발 구성은 `tracked_scope`만 받는다.
- 추적 범위 준비 도구(`gil_start_prepare`·`gil_start_cancel`)는 다른 종류의 기록을 향하면 잠금 전에 거절한다.
- 두 종류를 가진 빌드에서 준비 응답에는 「이 폴더는 이제 GIL 프로젝트다」 안내를 붙이지 않는다.
- **검토 요청 사항**: 두 종류를 가진 빌드에서 `.gil`만 있고 준비 기록이 없는 잔여물은 추적 범위 시작도 이어받지
  않는다(전용 구성에서는 종전대로 이어 간다). 두 계약의 잔여물 정책이 한 실행 파일에서 만나는 자리의 보수적 선택이다.

## 3. 제작

```sh
npm ci --prefix mcp-app
cargo fetch --locked
node distribution/product/build.mjs <소스 inventory 밖의 새 출력 폴더>
```

각 플랫폼에서 자기 target만 제작한다(교차 제작 없음). 공통으로: UI를 잠근 입력으로 다시 지어 commit된
Monitor와 바이트가 같은지 보고, 제작 중 소스가 바뀌면 거절하며, 고지를 target별 검토 inventory와 대조한다.
`provenance.json`에 source snapshot·Git head/tree·target·feature·도구 버전·CI 식별자·archive hash를 적는다.

- macOS: 기존 pipeline 그대로(Mach-O arm64, 시스템 라이브러리만, bsdtar 옵션, roundtrip, 무결성 선검증 smoke,
  timeout·no-retry) 뒤에 포장된 Core로 두 시작 종류를 확인한다. 서명·공증은 이 pipeline이 하지 않으며 자격
  증명을 읽지 않는다. `unsigned-preview.mjs`의 게시 준비는 `product` 프로필을 **거절한다** — 그 채널의 고지문과
  검토가 아직 없다.
- Linux: ELF64 x86-64, interpreter `/lib64/ld-linux-x86-64.so.2`, NEEDED는 glibc·libgcc 계열만, RPATH/RUNPATH
  없음, 버전 심볼은 `GLIBC_*`·`GCC_*`만. glibc 하한은 바이너리에서 읽어 `release.json`의 `elf.glibc_required`에
  적는다(Rust target의 하한을 옮겨 적지 않는다). GNU tar 옵션은 별도 목록이다.
- **Linux 고지 inventory가 아직 없다.** `distribution/linux/compliance/notice-policy.json`과 `texts/`는 Linux
  target의 Cargo graph를 조회하고 원문을 검토해 만들어야 한다. 그 전에는 Linux 포장이 실패한다(macOS inventory를
  대신 쓰지 않는다).

## 4. 검증 (exact review 뒤, 실행 범위·장소 지정 후)

```sh
node --test distribution/product/*.test.mjs
F=tracking-policy,exploration-flow,visual-authoring,product-safety
cargo test --locked -p gil --features $F --lib -- --test-threads=1
node tools/ci/expect-tests.mjs 5 cargo test --locked -p gil --features $F --test unified_start
node tools/ci/expect-tests.mjs 4 cargo test --locked -p gil --features $F --test tracking_start
node tools/ci/expect-tests.mjs 6 cargo test --locked -p gil --features $F --test tracking_scope_change
node tools/ci/expect-tests.mjs 20 cargo test --locked -p gil --features $F --test exploration_flow --test exploration_tools --test exploration_evidence --test exploration_plan_panel --test closing_handoff
cargo build --locked -p gil --all-targets --features $F
```

정확한 설치물의 검증은 그 설치물의 bytes로만 한다.

```sh
node distribution/product/verify-install.mjs linux-archive <archive.tar.gz> <SHA256SUMS> [monitor.html]
node distribution/product/verify-install.mjs kinds <설치된 또는 포장된 gil>
```

`linux-archive`: checksum → 멤버 검사(절대 경로·상위 이동·링크·특수 파일 거절) → 새 폴더에 풀기 → 영수증·
멤버·권한·ELF·고지 검사 → 격리된 HOME에 `gil install` → **설치된 binary**의 버전·probe·catalog·MCP·내장 HTML
(URI·MIME·bytes·SHA-256을 포장 자산과 비교) → 두 시작 종류 → 새 프로세스의 재시작 → `gil uninstall
--remove-binary` → 합성 Codex 설정 sentinel과 무관한 파일이 그대로인지 확인.

`kinds`가 확인하는 것: 빈 폴더의 무선택 거절과 무기록, 추적 범위(승인 전 무보관 → 제안 → App 전용 승인 →
형식 10 → 계획 승인 → 범위 변경 승인·보관 → 요구 연결 Experiment → 재시작), 이미지 보고서(형식 9 → 계획 승인 →
요구 연결 Experiment → PNG 첨부·선언 → Analysis·Cycle Report 인용 → 원본·Monitor 읽기 → 재시작), 두 종류가
서로를 대체하지 않음.

확인하지 **않는** 것: 사람의 승인(App 호출은 시험 입력이다), 실제 Host 화면, 전원 손실 내구성,
보관 실패·대기·명시 재시도·O-2 중단(이들은 debug 전용 failpoint가 필요하므로 `tracking_scope_change`의 debug
실행이 별도 증거이며 release bytes의 증거가 아니다). `GIL_INSTALLED_BINARY=<절대 경로>`는 stdio 시험 지원이
설치된 binary를 쓰게 하지만, failpoint가 필요한 시험은 release binary로 통과할 수 없다.

CI는 [product-candidate.yml](../../.github/workflows/product-candidate.yml)이 두 플랫폼에서 같은 head로 위 절차를
실행하도록 적혀 있다. 필수 check가 아니며 게시 단계가 없다.
