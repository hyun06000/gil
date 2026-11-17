# 실제 Core → Monitor → Chain 화면 fixture

검토 기준은 `bcefbc4`(PR #142 통합 후)다. 전용 브랜치는
`codex/chain-monitor-core-fixtures`다. 기존 Chain 화면을 다시 구현하지 않고,
실제 Core가 만든 응답과 기존 UI 투영 사이를 검사한다.

## 생성 경로와 데이터 출처

`tests/chain_monitor_fixtures.rs`는 기존 `tests/support_wave1`의 stdio client를
읽기 전용으로 재사용한다. 각 시나리오마다 독립 임시 프로젝트와 HOME/settings를 만들고
이번 소스에서 빌드한 `gil` 프로세스의 공개 MCP API를 호출한다.
source 저장소·사용자 프로젝트·설치본을 시작하거나 변경하지 않는다.

- `gil_start` → `gil_open` / `gil_close`로 Interview와 Experiment를 만든다.
- 제품 프로필에서는 `gil_plan_propose` → App의 `gil_plan_review` /
  `gil_plan_decide` → Core가 반환한 `close_report`로 승인 경계를 통과한다.
  이는 **합성 App 입력**이며 실제 사람이 클릭하거나 동의했다는 증거가 아니다.
  일회성 승인 capability는 메모리에서만 사용하고 fixture에 저장하지 않는다.
- Verify에서 합성 `plan.md`와 1×1 회색 PNG를 일반 작업 파일로 만든다.
  공개 typed close API로 PNG를 저장하고 Analysis/Cycle에서 같은 자산을 인용한다.
  저장 YAML이나 `.gil` 파일을 직접 수정해서 상태를 만들지 않는다.
- `gil_monitor_read`, `gil_monitor_detail`, `gil_plan_read`의 실제 응답을 수집한다.
  `gil::load`의 Chain 정의·Cycle 소유·보고서 원문과 별도로 대조한다.
  모든 checkpoint에서 프로세스를 종료·재시작하고 동일한 읽기 결과를 확인한다.
- 다음 Chain에서 과거 Verify의 `gil_monitor_report_visuals` /
  `gil_monitor_report_image`를 읽는다. 생성한 원본 PNG를 삭제하고 다시 시작해도
  저장된 PNG 바이트·SHA-256·소유 참조가 같은지 확인한다.
- legacy는 **default Core의 실제 새 기록**이다. 제품 기록에서 Chain 필드를 삭제하거나
  구형 저장 파일을 조작한 것이 아니다. 모든 역사적 저장 format의 호환을 증명하지 않는다.

`mcp-app/chain-core-fixtures.test.mjs`는 fixture의 `view`를 보정 없이 기존
`ui/chain.mjs`에 넣는다. 입력을 동결하고 순수 투영 뒤 원문 불변도 검사한다.
현재 Chain, 전역 Cycle 식별자, 소유 관계, 보고서와 승인 범위를 구분한다.

## 저장 형태와 정규화 규칙

`ui/fixtures/chain-core/`에는 JSON 13개만 둔다. raw `.gil`, 실행 파일, 실제 프로젝트,
사용자 대화, 개인 경로, 승인 token은 포함하지 않는다. 생성 fixture를 손으로 편집하지 않는다.

| 응답 | 보존 범위 / 유일한 정규화 |
| --- | --- |
| Monitor read | `structuredContent.view` 전체. `captured_at_unix_ms` 숫자만 `0`으로 변경 |
| Monitor detail | `structuredContent.detail` 전체를 원래 `step_ref` 키로 저장. 변경 없음 |
| Plan read | `structuredContent` 전체. 변경 없음 |
| Visuals / image read | `structuredContent` 전체. 실제 scope와 일치 확인 후 `scope_id`만 `project:` + 64개 `0`으로 변경 |

Monitor read/detail의 바깥 transport envelope(scope, label, revision/watch 메타데이터)는
UI가 받는 View/Detail의 일부가 아니므로 수집 대상에서 제외한다. 경로에서 파생되는 scope만
위와 같이 정규화하며 나머지 필드를 통째로 문자열 치환하지 않는다. 단계·Cycle·Chain 참조,
승인 revision/digest, 판정, 원문 report, snapshot/journey 식별자, 증거 SHA-256,
base64 내용은 변경하지 않는다. 두 번의 독립 생성에서 JSON 파일 목록과 모든 바이트가 같아야 한다.

## 상태별 검사

`success-*`와 `failure-*`는 별도 임시 프로젝트에서 생성한다. 두 경로의 Experiment는
성공하고 Closing Interview도 성공하지만, 승인된 Chain 판정만 서로 다르다.

| Fixture suffix | 실제 상태 | 검사 |
| --- | --- | --- |
| `first-interview` | 첫 Chain C1, Cycle C1 Interview, 열린 Question | Current·Chain 소유·열린 Detail |
| `experiment-closed` | 열린 Chain C1, 닫힌 Experiment C2 | Cycle 성공을 Chain 성공으로 표시하지 않음; 2개 Cycle 소유 |
| `closing-ready` | 닫힌 Closing Interview C3, 아직 열린 Chain | 3개 Cycle 소유; Chain close 전이 대기와 종료를 구분 |
| `chain-closed` | 성공/실패로 닫힌 Chain C1 | Core 원문 Chain Report, 여러 줄 handoff, closing Synthesis 참조 |
| `next-chain` | 새 Chain C2, Opening Interview C4 | 전역 Cycle ID·이전 Chain·과거 Detail 보존; 새 승인 계획은 없음 |
| `historical-evidence` | 새 Chain에서 과거 Verify 증거 조회 | 원본 삭제/재시작 후 저장 PNG, digest, 소유·출처 유지 |
| `legacy` | Chain 없는 기본 Core Interview | 기존 3개 fixture와 함께 Chain UI가 생기지 않음 |

과거 Chain/Cycle 선택은 View의 Current와 그래프의 현재 노드를 바꾸지 않는다.
이전 Synthesis의 승인 기록이 남아 있어도 새 Chain의 `active_plan`/`active_revision`은
없고 `executed_cycles`는 비어 있다. 새 Experiment 열기 거절 뒤 저장 상태 바이트도 같다.
읽기 API 전후에는 상태 파일과 프로젝트의 모든 일반 파일/저장 객체 바이트를 비교한다.

## 재현 명령

저장소 루트에서 실행한다. 도구는 두 Cargo 프로필을 순서대로 실행하며 일반 Cargo 환경 변수를 따른다.
동일 target을 다른 작업과 공유한다면 별도 target 또는 로컬 빌드 큐로 감쌀 수 있다.
추적되지 않은 로컬 큐 스크립트는 필수 의존성이 아니다. 임시 프로젝트는 시험 종료 때 제거된다.

```sh
npm ci --prefix mcp-app

# 생성: default → 제품 프로필을 두 번 실행, 파일별 byte equality 확인, JS 검사
node tools/chain-monitor-fixtures/run.mjs --write

# 검사: 새 임시 프로젝트에서 생성한 응답과 커밋 대상 JSON을 비교 (fixture 쓰기 없음)
node tools/chain-monitor-fixtures/run.mjs --check

# JS만 빠르게 재검사
node --test mcp-app/chain-core-fixtures.test.mjs mcp-app/chain.test.mjs

# 요청된 제품 프로필의 새 시험 및 관련 기존 시험 (아래 알려진 실패 포함)
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test chain_monitor_fixtures --test companion --test chain_handoff --test chains --test chain_closing --test approved_plan_wire_wave1 --no-fail-fast -- --test-threads=1

# 기존 bare-approval Chain 시험의 지원 프로필을 별도로 확인
cargo test --locked -p gil --features chain-core,visual-authoring,product-safety --test chain_handoff --test chains --test chain_closing -- --test-threads=1

# 기본 프로필 Companion 호환 (통합에서 fixture 목록 검사 충돌 수정)
cargo test --locked -p gil --test companion -- --test-threads=1
git diff --check
```

## 로컬 브라우저 검사

```sh
python3 -m http.server 8767 --bind 127.0.0.1
# http://127.0.0.1:8767/tools/chain-monitor-fixtures/selftest.html
```

전용 test Host가 실제 `ui/index.html`, CSS, `companion.js`를 그대로 불러온다.
11개 View의 Chain 표시와 legacy 숨김, 과거 Chain/Cycle 클릭 후 Current 불변,
여러 줄 handoff 원문을 검사한다. `<script>` 모양의 합성 report 텍스트가 실행되지
않는지도 확인한다. 마지막에는 실패한 과거 Chain을 선택해 원문을 눈으로 볼 수 있다.

이 페이지의 Host는 fixture를 읽는 가짜 Host다. 실제 MCP Host 승인 화면, fullscreen,
자동 연결/갱신, 재설치, Tauri, Windows 인수가 아니다. PNG는 실제 Core 읽기와 JS
바이트 검사를 했으며 이 브라우저 harness에서 이미지 렌더링은 검사하지 않는다.

## 원본 브랜치의 검증 기록과 초기 충돌

2026-11-17, macOS arm64, 기준 `bcefbc4`에서 수행했다.

- 제품 `gil` 바이너리를 locked 의존성과 Cargo 큐로 빌드했다.
- 독립 생성 두 번의 JSON 13개가 byte-identical이다.
- 새 Rust 시험: default 2개, 제품 프로필 2개 통과.
- 최종 `--check` 재실행도 default 2개/제품 2개와 JS 29개 통과.
- JS: 새 18개 + 기존 Chain 11개 = 29개 통과.
- 로컬 in-app browser: 실제 공통 UI로 11/11 통과, 콘솔 error/warn 없음.
- 기존 Chain 시험의 지원 프로필(`chain-core,visual-authoring,product-safety`):
  `chain_closing` 7개, `chain_handoff` 6개, `chains` 6개 = 19개 통과.
- `git diff --check` 통과. 요청된 전용 새 파일 19개만 추가했다.
- **전체 기존 Rust 시험 통과를 주장하지 않는다.** 아래 충돌은 기존 파일을 바꾸지 않고 남긴다.

### 필수 디렉터리와 기존 fixture 목록 검사의 충돌 (초기 관측)

`tests/companion.rs::the_switcher_lists_exactly_the_fixtures_that_exist`는 `ui/fixtures` 바로
아래 모든 디렉터리를 기존 `projects.json`의 3개 항목과 같다고 가정한다.
새 전용 `chain-core` 디렉터리가 생기면 실제 목록은 4개가 되어 이 검사가 실패한다.
이는 이번 추가가 드러내는 **통합 차단 사항**이다. 기존 fixture registry와 기존 시험은
이번 수정 금지 범위이므로 바꾸지 않았다. 기존 fixture를 새 형식으로 가장해 등록하지도 않는다.

최소 재현:

```sh
cargo test --locked -p gil --test companion the_switcher_lists_exactly_the_fixtures_that_exist -- --exact
```

기대: 기존 브라우저 fixture 3개와 별도 Core fixture corpus가 공존할 수 있음.
실제: `[dense, first-interview, reading-one]`과
`[chain-core, dense, first-interview, reading-one]` 불일치.
후속 통합에서는 유지관리자가 디렉터리 검사 범위를 실제 switcher fixture로 제한하는
별도 변경 등을 검토해야 한다. 이 작업은 기존 시험/registry를 변경하거나 green CI를 보장하지 않는다.

통합 검토에서 `chain-core`만 별도 응답 자료로 구분하여 목록 검사 충돌을 수정했다.
다른 디렉터리는 계속 기존 registry와 정확히 대조하며, 별도 자료는 실제 Core 생성·decoder·JS 시험으로 검사한다.
두 프로필 생성 도구의 로컬 큐 필수 조건도 제거해 일반 checkout에서 재현할 수 있다.
아래는 원본 브랜치의 최초 검증 이력이며 통합 후 결과는 병합 요청에 별도로 기록한다.

### 기존 시험의 승인 프로필 제한

제품 조합의 기존 `chain_closing` 7개, `chain_handoff` 6개, `chains` 5개와
`companion` 17개는 기존 `tests/common/mod.rs::bootstrap_from`이 명시한
bare `approved: yes` helper 제한에서 실패했다. `approved-plan` 프로젝트에는 해당 helper를
쓸 수 없다. 새 fixture는 이 helper를 사용하지 않고 실제 App 승인 경로를 거친다.
해당 기존 소스는 기준 커밋에서 변경되지 않았으며, 이 실패를 새 UI 투영 결함으로 분류하지 않는다.

최초 sandbox 실행에서는 `approved_plan_wire_wave1`의 guard 시험이
`sandbox-exec: sandbox_apply: Operation not permitted`로 실패했고, default Companion의
파일 감시 시험 2개가 알림 timeout으로 실패했다. 같은 명령의 실행 권한을 높여 격리된 임시
프로젝트에서 재실행했을 때 guard 시험 1개가 통과했다(최초 나머지 wire 11개 통과).
default Companion 재실행은 30개 통과/1개 실패였으며, 남은 실패는 위 fixture 목록 충돌이다.
제품 조합 첫 실행의 합계는 27개 통과/37개 실패였다(새 시험 2개 포함).
실패 37개는 bare-approval helper 35개, 목록 충돌 1개, 환경상 guard 실패 1개다.
환경 재실행을 전체 제품 suite의 green 결과로 대신하지 않는다.

## PR 설명 초안

**제목:** test: validate Chain Monitor projections with real Core fixtures

Chain 화면의 기존 검사는 합성 View에 의존해 Core 응답과의 연결을 확인하지 못했다.
독립 임시 프로젝트에서 공개 MCP API로 승인·실험·Chain 종료·다음 Chain을 수행하고,
정규화 규칙이 제한된 실제 Monitor 응답 13개를 재현 가능한 fixture로 추가한다.
기존 UI 투영과 공통 화면을 재사용하여 소유 관계, 판정, 원문 인계, 과거 선택의 읽기 전용성,
새 Chain 승인 분리, 저장 증거와 legacy 호환을 검사한다. 제품 코드/계약/저장 형식은 변경하지 않는다.

검증: 독립 생성 byte equality, 새 Rust default/제품 시험, JS 29개, 로컬 브라우저 11개.
알려진 통합 차단: 필수 `chain-core` 폴더가 기존 switcher fixture 목록 검사와 충돌한다.
기존 bare-approval 시험의 제품 프로필 제한과 환경별 첫 실패/재실행은 위 기록을 따른다.
실제 Host 인수·전체 Rust suite 성공은 주장하지 않는다. push/병합/CI dispatch/배포는 수행하지 않았다.
