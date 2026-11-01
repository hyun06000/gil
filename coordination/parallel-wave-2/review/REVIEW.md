# Wave 2 통합 검토 — 2026-11-02

## 결론

A–E의 제출을 로컬 `review/claude-wave2-20261102`에 합쳤다. 원래 R1–R5 결함은 재검증으로 해소를 확인했다. 통합 과정의 추가 결함과 누락을 보완한 소스는 `628da70`이다. **승인 프로필의 기존 통합 테스트 자료 정리는 아직 남아 있으므로, 모든 프로필 전체 테스트가 녹색이거나 배포 준비가 끝났다고 판정하지 않는다.**

main 변경, 원격 push/PR/CI 실행, 설치본 배포는 하지 않았다. 기존 Claude worktree는 수정하지 않았다. 실제 사용자 프로젝트도 열거나 변경하지 않았다.

## 제출과 통합

공통 기준: `15d6329c84c5e4e66a2ffaf41e153e83cd25d14c`.

| 세션 | 원본 제출 | 통합 커밋 | 검토 결과 |
|---|---|---|---|
| A restore preview | `0d8d575` | `116e4f4` | 링크/비디렉터리 루트 거절, 실제 디스크 별칭 확인. 기존 R1/R2 독립 재현 통과 |
| B Monitor 상태 | `a156aa6` | `7fc33aa` | 항목 연결과 실제 Cycle 열림/닫힘 분리. 상태 없는 구버전 응답은 미확인 |
| C 안내 | `b7c3ef1` | `13938f6` | 재인터뷰 요청이 합법적인 다음 Step을 덮지 않음. 실제 stdio 회귀 통과 |
| D 평가/시나리오 | `95c52a4` | `fc66947` | 승인·revision·실행 Chain 연결, 합성 입력/사람 입력 구분, 임시 환경 격리 |
| E 프로필 검증 | `9805374` | `28e9c08` | feature별 도구 표·가시성·Skill·라이브러리 검증 보완. 전체 통합 fixture 정리는 미완료 |

## 통합 담당 보완 (`628da70`)

1. **닫힌 Cycle의 에이전트 안내 오류를 재현하고 수정했다.** Monitor는 `current_cycle.state`를 읽지만 `gil_plan_read` 안내는 여전히 `current_item`만 보고 “Verify에서 파일로 저장”을 지시했다. binding은 Cycle이 닫힌 뒤에도 남는다. 닫힘 또는 binding만 있고 열림 여부 미확인인 경우 이제 `gil_context`를 다음 호출로 안내한다. 임의로 Cycle을 열거나 승인하지 않는다. 재인터뷰 요청 유무 × 닫힘/상태 누락 회귀를 추가했다. 수정 전 1 실패, 수정 후 통과.
2. **Windows plain wire 시험의 기록 위치를 시작 후 다시 조회한다.** visual-authoring은 개인 앱 데이터, plain 생성은 inline일 수 있다. 시작 전 존재하지 않는 경로를 확정해 버리지 않고 생성 후 실제 기존 기록 선택 규칙으로 재조회한다. private_app_data 응답 검사는 Windows visual-authoring에 한정한다. macOS typed/plain 실행은 통과했지만 Windows 네이티브 통과를 대신하지 않는다.
3. CI에 C의 `guidance_wave2`와 D의 `approval-link.test.mjs`, `state-path.test.mjs`를 추가했다. 양 OS matrix에 포함되지만 원격 실행하지 않았다.
4. UI bundle을 다시 생성해 B의 산출물과 동일함을 확인했다. macOS/Windows x64/arm64 고지 정책 3개의 입력 해시를 갱신했다. 각 target의 `collect()` 결과에서 **입력 해시 외 package·license·toolchain inventory가 그대로임을 assertCoverage로 검증**했다. 원문 라이선스는 바꾸지 않았다.

## 독립 검증

실행 명령과 수치는 [validation.json](validation.json)에 저장했다. Cargo는 공통 큐 + review worktree 전용 target, 임시 프로젝트/설정으로 실행했다. 소켓 시험에 필요한 로컬 실행 권한으로 검증했다.

| 검사 | 결과 / 범위 |
|---|---|
| 기본 Core 전체 | 1,515 통과 / 0 실패 / 2 ignored. 최종 안내 보완 전 전체 실행; 보완 후 해당 기본 안내 9개와 기본 all-target build 재검증 |
| 최종 승인 프로필 lib + guidance_wave1/2 + approved_plan_wire_wave1 | 687 통과 / 0 실패 / 1 ignored (lib 678, wire 1, guidance 7+1) |
| 원래 Rust 독립 재현 4개 | 4 통과. 검토 모듈을 임시 연결해 실행 후 lib.rs 원복 |
| 원래 JS 부정 사례 2개 | 닫힌 상태 증거 없는 binding은 `bound`; 다른 Chain의 이전 승인만 있는 실행은 `fail` |
| Chain 도메인/되돌림 | 154 + 1 통과 |
| Chain visual/MCP | integration 35 통과; visual_start lib 9 통과 / 1 ignored |
| 기본 / chain-product / approved-product all-target build | 모두 통과 |
| UI·Plugin·평가 JavaScript | 184 통과 / 0 실패 / 4 skipped; wire runner 보완 후 평가 시험 37개 재통과 |
| M6-E 설계 fixture | 25 통과. 설계 시험이며 M6-E 구현 완료 증거가 아님 |
| 배포·고지 | 96 통과 / 0 실패 / 1 skipped |
| 실제 native MCP wire (typed / plain) | 각각 20 통과 + restore/revisit 1 expected_blocked. [typed](wire/wire-run.md), [plain](wire-plain/wire-run.md) |
| 승인 프로필 기존 context integration | **1 통과 / 14 실패** — 아래 잔여 작업 |

첫 JS 실행은 새 worktree의 Plugin 의존성 미설치로 모듈 로드가 실패했다. 잠금 파일대로 `npm ci --offline` 후 재실행했다. Core 컴파일의 기존 dead-code/unused 경고는 오류로 숨기지 않았고, 무관한 전체 formatting도 하지 않았다.

## 남은 작업과 인수 경계

### 1. 승인 프로필의 기존 통합 fixture 정리

`cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test context`를 독립 실행하면 14개가 `tests/common/mod.rs::bootstrap_from`에서 실패한다. 새 승인 계획 프로젝트에 legacy `approved: yes` bootstrap을 적용하기 때문이다. **실제 도메인 assertion에 도달하지 못하는 coverage 공백**이다. 14개의 독립 제품 버그라고 부르지는 않지만, 이 상태를 전체 프로필 성공으로 감출 수도 없다.

E는 C 합류 전 전체 승인 프로필 실행에서 509 실패를 보고했다. 그 수치를 현재 통합본의 실패 수로 재사용하지 않는다. 이번에는 context의 14개를 직접 재현했고, 전체 승인 프로필의 모든 integration suite는 다시 실행하지 않았다.

다음 fixture 작업은 실제 legacy 프로젝트를 명시적으로 로드해 legacy 계약을 검사하는 시험과, 승인 프로필을 실제 App 경로로 부트스트랩하는 시험을 분리해야 한다. product-safety가 막는 restore/revisit 시험도 해당 프로필의 거절 계약으로 분리한다. 테스트를 통과시키려고 Core의 승인 검사나 App 전용 결정 경계를 완화하지 않는다. 일괄 ignore나 의미 없는 성공 처리도 하지 않는다.

### 2. 원격 CI와 실기

Windows native 테스트/실제 PC, 실제 Host의 fullscreen 표시·사람의 Monitor 버튼 클릭·Luna 작업은 미실행이다. 로컬 wire의 모든 App 승인은 명시된 합성 harness 입력이다. 표시 확인 상태는 unknown이며, 모델 작업 성공으로 기록하지 않는다. Tauri 전체 workspace 검사도 이번 범위가 아니다.

CI를 실제 실행한 뒤, 합의된 하나의 작업으로 Interview 승인 → 닫기 → 항목별 Experiment → 파일 증거 → Analysis/Outcome → Closing Interview/Chain Report를 사람이 관찰해야 한다. 아직 설치된 사용자 버전에 이번 로컬 소스 변경은 반영되지 않았다.

### 3. 개발 범위 유지

복원 preview는 read-only 후보 판정이다. 적용 직전 handle 검사/복구 가능한 격리/정책 저장과 제품 restore 연결이 완성된 것은 아니다. `product-safety` restore/revisit 차단은 그대로다. M6-E는 설계 fixture만 검증했으며 구현/승인된 의미론 변경을 대신하지 않는다.
