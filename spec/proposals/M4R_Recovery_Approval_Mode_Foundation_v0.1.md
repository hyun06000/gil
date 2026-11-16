# M4-R 제안 — 실패 복구 승인 모드의 Core 기반과 미결 입력 경로 (v0.1)

상태: **제품에서 비활성인 실험용 기반(2026-11-16). 사용자 설정 입력 경로는 미결이며 제품 기능 완료가 아니다.** 초기 초안의 로컬 회귀는 ENOSPC로 일부 미검증(§5)이었으며, 통합 검토에서 원격 회귀와 opt-in 전체 대상 빌드를 확인한다. 이 문서는 제안이며
사용자 결정이 아니다. 정본은 [실패 실험 복구 계약](../GIL_Revisit_Recovery_v0.1.md)의 「승인 모드」와
[Roadmap §9 M4-R](../GIL_Roadmap.md)의 2026-11-15 결정이다. 여기서 결정된 계약을 넓히지 않는다.

## 1. 이미 결정된 것과 아직 결정되지 않은 것

| 구분 | 내용 | 근거 |
| --- | --- | --- |
| 결정 | 기본은 수동 승인(Monitor 버튼). 자동 승인은 사용자가 **프로젝트별로** 켠다 | 복구 계약 「승인 모드」 표 |
| 결정 | 자동 모드도 수동과 **같은** 변경 목록·보호 경로·미기록 변경·경로 충돌·객체 손상 검사와 실행 직전 재검증을 통과해야 한다 | 「자동 승인 실행」 2 |
| 결정 | 설정이 없거나 읽을 수 없거나 유효성을 확인할 수 없으면 수동. 읽기·자동 갱신은 설정 변경이나 복구 실행을 유발하지 않는다 | 「선택과 적용 범위」 |
| 결정 | 복구마다 수동 클릭인지 자동 설정 승인인지 구분해 기록하고, 자동 기록에는 설정의 판·실패/대상·계획 식별값·보존 위치를 묶는다. 사람의 클릭·신원 인증으로 기록하지 않는다 | 「승인 기록·해제·중단」 |
| 결정 | 자동 모드 해제는 **새** 자동 승인만 막는다. 원본·승인 기록 저장 후 시작한 복구의 재개는 그 기록을 따르며 새 승인을 만들지 않는다 | 「승인 기록·해제·중단」 |
| 결정 | 재시작·응답 유실·중복 요청은 같은 복구를 중복 실행하거나 새 분기를 만들지 않는다 | 같은 절 |
| 결정 | 자동 모드는 계획 승인·계획 수정·추적 제외 변경·열린 단계 강제 종료·일반 restore·Step revisit을 대신하지 않는다 | 「선택과 적용 범위」, Cycle Model |
| 결정 | Monitor와 에이전트 상태 조회에 현재 모드를 표시한다 | 「선택과 적용 범위」 |
| **미결** | 사용자가 모드를 **켜고 끄는 입력 경로**(어느 도구·화면·명령이 설정을 쓰는가)와 그 경로가 보장하는 인간 출처 | 계약: “설정 입력 경로와 저장 형식은 구현 전에 정의하며” |
| **미결** | 설정 저장 형식의 **확정** | 같은 문장. 아래 §3은 시험용 잠정안이다 |
| 미결(범위 밖) | 시작 인터뷰에서의 선택 문구·에이전트 안내 문구(i18n), Monitor 화면의 모드 표시·토글 UI | 안내·화면 PR과의 충돌을 피해 이번에 넣지 않았다 |

## 2. 이번에 구현한 것 (코드 근거)

설정 파일 읽기와 자동 실행은 opt-in Cargo feature **`recovery-auto-approval`**(= `product-safety` 포함) 뒤에 있다.
현재 제품 프로필(`chain-core,visual-authoring,product-safety` / `approved-plan,visual-authoring,product-safety`)은
이 feature를 켜지 않으므로 **자동 복구를 활성화하지 않으며 설정 파일도 읽지 않는다.**
승인 모드·출처의 읽기 전용 응답과 수동 승인 시 `source.json` 기록은 제품 프로필에도 추가된다.

| 요소 | 위치 | 동작 |
| --- | --- | --- |
| 모드 읽기 | `src/recovery_mode.rs` | 기록 폴더(`.gil/`)의 `recovery-approval.json`을 **읽기만** 한다. 링크·하드링크·크기 초과·schema 불일치·모르는 필드·`revision 0`·모르는 모드는 전부 `manual`/`unreadable`. 파일 없음은 `manual`/`default`. feature가 없으면 디스크를 건드리지 않고 `manual`/`unavailable` |
| 상태 표시 | `src/mcp/monitor.rs` `gil_monitor_read.recovery_approval`, `gil_revisit_review.approval_mode` | `{"mode","basis","revision"?}`. digest·`approval.json`에는 넣지 않아 진행 중 승인의 재개를 깨지 않는다 |
| 승인 출처 기록 | `src/session/revisit_recovery.rs` `source.json` | 승인을 **만든** 호출만 `revisit-preserved/<key>/source.json`을 쓴다. `monitor_button`(`human_click: true`) 또는 `automatic_setting`(`human_click: false`, `setting.{file,schema_version,revision,sha256}`), 공통으로 `pending_state_sha256`·`digest`·`from`·`target`. 결정적 bytes라 중복 승인은 같은 내용으로 수렴한다. 재개는 기록을 다시 쓰지 않으며 기록이 없는 과거 승인은 `unrecorded`로 보고한다 |
| 자동 승인 진입점 | 같은 파일 `resume_revisit` → `automatic_revisit` | 에이전트의 `gil_revisit` 요청에서 **pending 승인이 없고** 유효한 자동 설정이 있을 때만 수동과 같은 `recovery_plan()`을 세우고, 승인 기록 직전에 설정을 다시 읽어 bytes가 같아야 `apply_recovery`로 간다. 토큰은 만들지도 받지도 않는다. pending이 있으면 설정과 무관하게 저장된 승인으로 재개한다 |
| 제품 프로필 | 자동 실행 비활성 유지 | `gil_revisit`은 여전히 저장된 승인만 재개한다. 수동 승인에 `source.json` 기록과 읽기 전용 승인 모드·출처 응답이 추가된다 |

### 왜 `gil_revisit`을 진입점으로 썼는가

계약은 “에이전트가 실패 뒤 복구를 요청하면 Core가 현재 모드와 유효한 실패 계보를 읽는다”고 하고,
`gil_revisit`이 그 요청이다. 새 도구를 추가하면 도구 카탈로그·Skill·안내 문구가 함께 바뀌어
병행 PR과 충돌한다. 대안(별도 `gil_revisit_request`)은 §4에 남긴다.

### 왜 feature로 막았는가

설정 파일은 `state.yaml`과 같은 신뢰 수준의 일반 파일이다. 사용자 입력 경로가 없는 상태에서
제품이 이 파일을 읽으면 “에이전트가 임의 값을 넣어 모드를 켤 수 없다”는 결정에 어긋나는 길이
생긴다. 입력 경로가 정해져 그 경로만 파일을 쓰게 되면 feature를 제품 프로필에 넣는다.

## 3. 잠정 저장 형식 (시험 전용, 확정 아님)

```json
{ "schema_version": 1, "mode": "automatic", "revision": 2, "note": "선택 사항" }
```

- 위치: 기록의 `.gil/recovery-approval.json`(locale 사이드카와 같은 자리). 기록 형식 번호는 바꾸지 않는다.
- `mode` ∈ {`manual`,`automatic`}, `revision` ≥ 1(변경마다 올린다), 4096 bytes 한도, 모르는 필드 거절.
- `revision`과 파일 bytes의 SHA-256이 “설정의 판”이며 자동 승인 기록이 이를 묶는다.
- 다른 프로젝트로 복사해도 그 프로젝트 기록 폴더에 **그 사용자가** 두지 않는 한 적용되지 않는다는
  보장은 입력 경로가 정해져야 생긴다(§4).

## 4. 입력 경로 대안과 권장안

| 대안 | 내용 | 장점 | 문제 |
| --- | --- | --- | --- |
| **A. Monitor App 전용 도구 (권장)** | `gil_recovery_mode_review/decide`(App 전용, `_meta` 토큰, M6-D의 `gil_plan_decide`와 같은 꼴). Monitor에 켜기/끄기 버튼 | 기존 “사람의 버튼” 채널과 같은 보장·같은 코드 패턴. 모델은 토큰을 보지 못한다. 설정 변경이 실제 사람 클릭 뒤에 온다 | Monitor UI·생성 HTML·도구 카탈로그 변경. 실화면 인수 필요 |
| B. 시작 인터뷰에서 에이전트가 기록 | `gil_start`/Interview Report 필드로 모드 선택 저장 | 계약의 “시작 인터뷰에서 선택” 문장과 직결 | 에이전트가 쓰는 값이라 “Report에 approved: yes로 켤 수 없다”와 충돌. 단독 채택 불가. A의 버튼을 인터뷰 중 안내하는 방식으로만 결합 |
| C. CLI `gil recovery-mode on/off` | 터미널의 사람이 직접 실행 | 구현이 작다 | Host 안의 에이전트도 같은 명령을 실행할 수 있어 인간 출처를 보장하지 못한다. Zero-terminal 배포 방향과도 어긋난다 |
| D. 별도 요청 도구 `gil_revisit_request` | 자동 승인 진입을 `gil_revisit`에서 분리 | 재개와 새 승인이 도구 이름으로 갈린다 | 도구 추가·안내 변경. 현재 Core 코드로 이미 pending 유무로 구분되므로 추가 이득이 작다 |

권장: **A**를 입력 경로로 확정하고, 그 PR에서 (1) Monitor 버튼 → App 전용 도구 → 사이드카 쓰기(ProjectLock 아래,
기존 `state_file::write` 게시), (2) `revision` 증가, (3) feature를 제품 프로필에 편입, (4) 에이전트 안내·i18n 키
(`ctx.next.can_revisit_product`에 모드 분기), (5) 실제 Host 인수를 함께 한다. B는 A의 버튼을 안내하는 문구로만 쓴다.

## 5. 시험

- 단위: `src/recovery_mode.rs` — 유효/무효 판정, 표시 투영, 읽기 무변경·링크 거절.
- 통합(실제 stdio, 합성 임시 프로젝트): `tests/recovery_approval_mode.rs`
  - 기본 수동·명시적 수동 설정에서 `gil_revisit` 거절, Monitor 경로는 `source.json: monitor_button`.
  - 손상·위조(`approved: yes` 필드)·`revision 0`·크기 초과·링크 설정 → 수동, 파일 bytes 무변경, 보존 영역 없음.
  - 유효한 자동 설정: 읽기·review·`gil_open`은 복구를 시작하지 않음, `gil_revisit`이 클릭 없이 완료,
    `source.json: automatic_setting` + 설정 revision·sha256, 보존 필드·객체 저장소 불변, 중복 요청 수렴,
    새 세션 유지, 두 번째 실패의 재승인, 계획 항목 진행.
  - 자동 모드에서 보호 경로 차이·미기록 변경·손상 객체 거절(수동과 동일), 해소 뒤 완료.
  - 상태 저장 뒤 주입 실패 → 설정을 수동으로 바꿔도 저장된 승인으로 재개(기록 bytes 불변), 이후 새 실패는 거절,
    파일 삭제도 같은 해제.
- 실행 명령(`.github/workflows/chain-core.yml`의 approved-plan 묶음 뒤에 두 OS 공통으로 등록했다):

```sh
cargo test --locked -p gil --features approved-plan,visual-authoring,recovery-auto-approval --lib recovery_mode
cargo test --locked -p gil --features approved-plan,visual-authoring,recovery-auto-approval --test recovery_approval_mode
```

### 2026-11-16 로컬 검증 상태 — 디스크 가득 참(ENOSPC)으로 일부 미검증

macOS arm64 로컬에서 **통과**: `--lib recovery_mode`(제품 프로필 3건, opt-in 프로필 4건), `--test recovery_approval_mode` 5건,
`--features approved-plan,visual-authoring,product-safety --test approved_plan_wire_wave1` 11건(기존 수동 복구 경로 + `source.json`).
기능별 라이브러리 빌드(제품 프로필·opt-in 프로필)는 통과했다.

로컬에서 **미검증(ENOSPC로 컴파일·실행 실패, 통과로 세지 않는다)**: `--features chain-core,visual-authoring,product-safety --test mcp`,
approved-plan 프로필 `--lib` 전체(install/locale/companion_ipc 시험이 임시 파일 쓰기 단계에서 실패했고 원인 분리 못 함),
all-targets 빌드 4종(chain-core 제품·approved-plan 제품·opt-in·기본), 기본 `cargo test --locked -p gil`, Windows 전체.
이 항목은 PR CI 결과로 대신 확인해야 하며 CI가 돌지 않았다면 미검증이다.

## 6. 검증하지 않은 것

- 설정 확인과 승인 기록 사이의 외부 동시 변경(경합)은 재읽기 비교로 막지만 주입 시험은 없다.
- 실제 Host에서의 모드 표시·자동 복구·후속 진행, Windows 실기기, 실제 모델의 재개 행동.
- 에이전트 안내 문구는 바뀌지 않았다. 제품 프로필에서는 과거와 같이 Monitor 버튼만 안내한다.
- 이 기반을 실제 사용자 프로젝트에서 켜라는 뜻이 아니다. 입력 경로 결정 전에는 켤 방법이 제품에 없다.
