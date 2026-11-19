# M3 `.gilignore` Interview Design v0.1 — 설계 제안

> 상태: **제안.** 2026-11-16, 로드맵 §2 「2026-11-15 합의한 후속 구현 순서」 3번
> 「인터뷰 기반 추적 제외」의 **구현 전 설계**다. 제품 코드·저장 형식·설치본·규범 명세를
> 바꾸지 않는다. 규범은 여전히 [Artifact Model §3.6](../GIL_Artifact_Model_v0.1.md#36-인터뷰에서-정하는-명시적-추적-제외)·§3.6a·§4,
> [Storage Model](../GIL_Storage_Model_v0.1.md), [Cycle Model](../GIL_Cycle_Model_v0.1.md),
> [승인 계획 계약](../GIL_Approved_Plan_v0.1.md), [M4-R 복구 계약](../GIL_Revisit_Recovery_v0.1.md)이다.
> 이 문서의 「권장안」은 비교 결과이지 확정된 사용자 결정이 아니다. 새 명령·필드·저장 구조는
> 모두 **(제안)** 으로 표시한다. 기준 main: `e923552` (2026-11-16).

> **2026-11-19 부분 채택:** 시작 대화에서 범위 승인 → 승인 정책으로 최초 보관 → 정식 체인·첫 인터뷰 개설 순서를 채택했다.
> 범위 승인은 정식 인터뷰 밖의 시작 준비 기록에 남긴다. 정본은 [Artifact Model 시작 계약](../GIL_Artifact_Model_v0.1.md#시작-대화의-범위-승인과-최초-보관--2026-11-19-확정)이다.
> 아래 A5의 사용자 흐름을 채택한 것이며 제안된 명령·필드·추천 기본값·취소 방식·저장 형식 전체를 승인한 것은 아니다. 구현은 대기다.

> **2026-11-19 일반 추천 기본값 채택:** 일반 제외 추천은 미리 선택해 보여 주고 사용자가 수정·승인한 뒤 적용한다. 미리 선택은 승인이 아니다. 시스템 보호는 해제 가능한 추천과 구분한다. 비밀 파일 추천·프로젝트별 추천 기본값·세부 패턴·승인 채널은 이번 승인에 포함하지 않는다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#일반-추천의-기본-선택--2026-11-19-확정)이다.

> **2026-11-19 비밀값 파일 추천 채택:** 비밀값 파일도 미리 선택한 제외 추천으로 제시하고 사용자 확인·수정·승인 뒤 적용한다. 설정 예시 파일의 일괄 제외와 실제 비밀값의 추천/승인 기록 노출은 하지 않는다. 아래 `.env.*`·키 확장자 등 구체적 패턴 전체는 미승인이다. 정본은 Artifact Model §3.6이다.

> **2026-11-19 후속 독립 정책 합의:** 이전 `.gitignore` 기본 존중·우선순위 합의는 대체됐다. `.gitignore`는 추천 참고 자료일 뿐이며 시스템 보호 경계 안에서 승인된 `.gilignore`만 GIL 범위를 결정한다. Git 규칙 자동 적용·원문 동결·변경 시 정책 재승인 후보는 폐기한다. 구현은 대기다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#제외-규칙의-우선순위--2026-11-19-확정)이다.

> **2026-11-19 폴더 탐색 채택:** 최종 승인 정책상 완전히 제외된 폴더는 내부 탐색을 생략한다. 명시적으로 재포함한 파일에는 필요한 탐색·검사를 수행한다. 아래 과거의 부모 제외 시 하위 재포함 불가 후보는 이 결정으로 대체한다. D5 중 직접 만난 제외 연결 파일의 처리까지 승인한 것은 아니다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#제외-폴더의-내부-탐색--2026-11-19-확정)이다.

> **2026-11-19 제외 연결 파일 처리 채택:** 승인 정책상 제외된 연결 파일은 대상에 접근하지 않고 건너뛰며 복원에서도 변경하지 않는다. 재포함한 연결 파일은 기존대로 거절한다. 경로 종류 확인과 대상 탐색을 구분하며 시스템 보호 검사는 우선한다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#제외된-연결-파일--2026-11-19-확정)이다. D5 방향은 확정, 구현은 대기다.

> **2026-11-19 정책 파일 저장 방향 채택:** `.gilignore`는 일반 보관·복원에서 제외하고 규칙 원문·적용 판은 정책 승인 기록에 보관한다. 파일 직접 편집은 제안이며 실제 적용 기준은 승인 기록이다. D7 방향은 승인됐지만 아래 상세 필드·파일 쓰기 실패 정책·D10의 과거 정책 자동 재적용은 미승인이다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#정책-파일과-승인-기록--2026-11-19-확정)이다.

> **2026-11-19 승인 채널 채택:** 추적 범위의 최종 승인은 모든 구성에서 모니터로 받는다. 대화는 설명·수정에 사용하며 에이전트 전달 승인으로 대체하지 않는다. 정식 체인 이전의 준비용 승인 화면이 필요하고 화면 부재 시 대기한다. D3의 과거 프로필별 대안은 채택하지 않았다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#추적-범위의-최종-승인-경로--2026-11-19-확정)이다.

> **2026-11-19 최초 적용 전 재확인 채택:** 승인 후 최초 적용 전에 실제 추적 대상 목록이나 규칙이 바뀌면 변경점을 보여 주고 모니터에서 재승인받는다. 완전 제외 폴더 내부·기존 파일 내용만의 변화는 범위 재승인 사유가 아니다. 규칙 입력 내용 변경은 규칙 변경이며 보관 일관성·경계 검사는 별도 유지한다. 정본은 [Artifact Model](../GIL_Artifact_Model_v0.1.md#승인-후-최초-적용-전-재확인--2026-11-19-확정)이다.

> **2026-11-19 복원 정책 결정:** 모든 복원은 사용자가 승인한 최신 정책을 따른다. 정책은 세계 밖에서 앞으로만 갱신한다. D10의 과거 정책 재적용·정책 차이만으로 거절·교집합에 따른 조건부 허용은 채택하지 않는다. 정본은 Artifact Model §3.6 「범위 변경과 과거 기록」이다. 결과 세계 참조·과거 기록 부재의 세부 계약은 후속이며 제품 복원 차단을 해제하지 않는다.

> **2026-11-19 추적 확대·복원 기준 채택:** 복원은 과거 전체의 정확한 재현이 아니라 복원 시점의 최신 승인 명시에 입각한다. 새로 추적한 파일은 최초 보관 시점부터 복원 가능하며 이전 미추적 상태는 추측하지 않는다. 기록 부족 시 실행 전 알리고 적용하지 않으며, 추적 확대 승인 화면에 이전 전체 복원의 제한 가능성을 안내한다. 정본은 Artifact Model §3.6 「범위 변경과 과거 기록」이다.

> **2026-11-19 파일 갱신 실패 처리 채택:** 승인 기록이 정상 저장되면 해당 정책을 적용하고 `.gilignore` 파일 갱신 실패는 별도로 표시한다. 그 실패만으로 재승인하지 않는다. 재시도는 사용자 편집·삭제를 덮어쓰지 않으며 승인 기록 저장 실패·불확실을 완료로 처리하지 않는다. 정본은 Artifact Model §3.6의 정책 파일과 승인 기록 절이다. 세부 저장·안전한 동기화 구현은 후속이다.

용어: Snapshot(보관본) = 한 시점의 추적 파일 전체를 이름 하나(`snapshot:A1`)로 확정한 불변 세계.
Verify = Experiment Cycle 안에서 파일 변경을 수행하고 닫을 때 세계를 확정하는 유일한 Step.
dirty gate = Verify가 아닌 자리를 닫을 때 추적 파일이 기준 Snapshot과 다르면 거절하는 검사.
정책(policy) = 어떤 파일을 세계에 넣고 빼는지 정하는 규칙 묶음과 그 해석 버전.
`policy_id` = 규칙 원문·위치·버전에 결정적으로 묶인 식별 문자열(기존 엔진이 정의).

---

## 1. 목적과 범위

### 1.1 목적

`.gilignore`로 관리하는 **인터뷰 기반 추적 제외**를 다음 다섯 가지와 모순 없이 연결하는
구현 설계를 만든다.

1. 최초 시작 — 범위 승인 **전에** 전체 트리를 저장하지 않으면서, 「Snapshot 없는 열린 Cycle」을
   만들지 않는 시작 순서.
2. 규칙 — `.gilignore` 문법·위치·`.gitignore`와 시스템 보호의 우선순위.
3. 승인된 정책과 파일 — 정책 원문·판·적용 시점의 보존, 직접 편집·삭제의 감지, 실패·재시작.
4. 작업 중 범위 변경 — 단일 열린 Cycle 규칙 안에서의 재인터뷰와 적용 시점.
5. 관측·저장·복원 연결과 기존 프로젝트 — 모든 판정이 같은 승인 정책을 쓰고, 과거 기록은
   이관 없이 보존하는 호환 전략.

### 1.2 이 문서가 하지 않는 것

- 기능 구현, 실제 프로젝트 시험, 설치본 변경, 설명서·에이전트 안내 본문 수정, 번역.
- 일반 `gil restore`의 차단 해제, 복구 자동 승인, 중첩 프로젝트 지원, Managed Dataset.
- `.gilignore`로 파일의 불변성·읽기 전용·접근 금지·영구 보존을 제공하는 것(§2).
- 모든 과거 기록의 호환성 선언. 과거 Snapshot은 그대로 보존하며 새 의미를 소급하지 않는다.

### 1.3 다른 세션과의 경계

설명서·에이전트 안내(순서 2번)와 Monitor 간결화(순서 1번)는 다른 세션이 진행 중이다.
이 문서는 그 결과에 **의존하는 자리**(안내 문구, Monitor 카드)를 §8의 마지막 단계로 두고,
공통 지침 원문 `src/mcp/workflow.md`·Monitor 소스·규범 명세 수정은 통합 담당에게 맡긴다.

---

## 2. 확정된 합의 (바꾸지 않는다)

출처: Artifact Model §3.6, 로드맵 §7·§15 「2026-11-15 — 인터뷰 기반 추적 제외」, 이번 작업 지시.

| # | 합의 | 이 설계에서의 귀결 |
|---|---|---|
| C1 | 기존 폴더 구조를 유지하고 루트 `.gilignore`로 제외를 명시한다. 별도 workplace 개념 없음 | 루트 하나만 정책 파일로 읽는다(§5.1) |
| C2 | GIL은 `.git/`·`node_modules/`·캐시·빌드 출력 등 일반 대상을 **추천**한다 | 추천 목록은 이유와 함께 보여 주는 **선택지**다. 강제 보호와 구분(§5.2) |
| C3 | 작업 에이전트는 목적·프로젝트 특성에 맞는 제외를 추천한다 | 에이전트 추천은 도구 입력으로 받아 사람이 승인한다(§4) |
| C4 | 사용자는 대상·이유·보관·복원 영향을 확인하고 승인·수정한다 | 승인 대상은 「정책 원문 + 보여 준 효과」이며 묶음 digest에 바인딩(§6.2) |
| C5 | 제외는 불변성·읽기 전용·접근 금지·영구 보존이 아니다. 제외 파일도 읽기·수정·삭제 가능 | 제외 목록은 **관측 범위**만 바꾼다. 어떤 OS 권한도 바꾸지 않는다 |
| C6 | 제외 파일은 적용 정책의 새 Snapshot에 들어가지 않고, 그 파일만 바뀌어도 추적 대상 변경으로 판정하지 않는다 | 관측·dirty 판정·Verify 확정이 같은 정책을 쓴다(§6.4) |
| C7 | GIL 복원은 현재 제외된 파일을 생성·덮어쓰기·삭제·격리 이동하지 않는다 | 복원 계획은 제외 경로를 계획에 넣지 않으며 충돌은 무변경 거절(§6.5) |
| C8 | `.git`과 GIL 내부 기록의 시스템 보호는 사용자가 해제할 수 없다 | 내장 제외(`.git` 어느 깊이든, 루트 `.gil/`)는 `!`로 되살릴 수 없다(기존 엔진) |
| C9 | Git과 GIL의 추적 정책 독립 | `.gitignore`는 추천 참고만, 승인된 `.gilignore`가 적용 정책. 두 파일 자동 상호 수정 없음 |
| C10 | 과거 보관본·보고서·증거를 다시 쓰거나 삭제하지 않는다 | 과거 Snapshot에 `policy`를 소급 기록하지 않는다. 미상은 미상(§6.6) |
| C11 | 필수 검증 증거를 제외 파일만으로 대신하지 않는다 | `evidence_files`는 확정 Snapshot 안의 파일이어야 한다(기존 `require_verify_evidence`) |
| C12 | 일반 복원 차단 해제·복구 자동 승인·중첩 프로젝트는 범위 밖 | §3.6a의 3·4항(격리·계획 승인)은 이 설계가 닫지 않는다 |

---

## 3. 현재 코드와의 차이 — 파일·함수 근거

### 3.1 이미 있는 것 (재사용 가능)

| 자리 | 근거 | 재사용 판단 |
|---|---|---|
| gitignore 부분집합 엔진 | `src/artifact/tracking_policy.rs` — `IgnoreSource`, `PolicyVersion::V1`, `TrackingPolicy::{from_sources, evaluate, includes_file, policy_id}`, `compile_line`, `verdict` | **재사용.** 문법·부정·`**`·문자 클래스·마지막 규칙 우선·제외된 부모 아래 되살림 금지·잘못된 패턴의 전체 거절(`PolicyError::InvalidPattern`)이 있다. 다만 같은 디렉터리의 원본 둘은 `DuplicateSourceLocation`으로 거절하므로 루트 `.gitignore`와 루트 `.gilignore`를 **같은 층**으로는 넣을 수 없다(§5.1) |
| 내장 제외 | `builtin_exclusion` — `.git` 구성 요소 어느 깊이든 제외, 루트 `.gil/` 제외, 중첩 `.gil/`은 `NestedGilBoundary` 오류 | **재사용.** C8 그대로 |
| `.gitignore` 수집 | `discover_gitignore_sources(root)` — 루트 아래만, Git과 같은 순서, 제외된 디렉터리 안은 들어가지 않음, 심볼릭 링크 `.gitignore` 거절, `.git/`·`.gil/` 안 안 봄 | **재사용.** 전역 Git 설정·`info/exclude`·환경 변수를 읽지 않는다는 성질이 재현성의 근거 |
| 정책 적용 관측 | `observe_with_policy`/`observe_twice(root, policy, ExcludedDirectories, between)` → `PolicyObservation { manifest, policy_id, excluded }` | **재사용.** 두 번 훑어 견주는 안정 관측 규칙을 지킨다. 그러나 **바이트를 저장하지 않는다**(`capture_with_policy` 없음) |
| fingerprint | `fingerprint_of` — 버전·원본 수·(디렉터리, 원문) 정렬 순서로 SHA-256 | **재사용.** 원문 한 글자가 달라도 다른 ID. 층(layer)을 넣으면 **버전 2**가 필요(§5.1) |
| 복원 미리보기 | `src/restore/preview.rs` — `Request { before_policy, target_policy }`, `check_policy` → `Matched/Mismatch/Unknown`, `Disposition::Excluded{by: Policy}`, `Readiness::Refused(PolicyUnknown|PolicyMismatch)` | **부분 재사용.** 기존 정책 ID 거절 규칙은 코드 현황이며 새 최신 정책 계약과 다르다. §6.5에 맞춰 계획·검증을 보완해야 한다 |
| 승인·보존 선례 | `src/session/revisit_recovery.rs` — `approval.json`·`files.json`을 `.gil/revisit-preserved/<key>/`에 durable 기록 → 상태 저장 → 복원. `policy(store, manifest)`가 Snapshot 안의 `.gitignore` blob을 읽어 정책을 세움 | **선례로 재사용.** 「승인 기록이 먼저, 실행이 나중」·「에이전트 입력은 저장된 승인을 재개만」의 틀. Snapshot 안의 원문 blob으로 과거 정책을 재구성하는 수법은 `.gilignore`가 내장 제외가 되면 쓸 수 없으므로(§6.1) 정책 원문을 **별도 참조**로 보존해야 한다 |
| 승인 원장 선례 | `src/approved_plan.rs` — 제안 판·digest·Monitor 응답·`request_plan_interview(reason)`(Experiment 중에만, 재인터뷰 이유 보존) | **선례로 재사용.** 범위 변경 요청의 입구가 이미 있다(approved-plan 프로필) |

### 3.2 아직 연결되지 않은 것 (이번 설계가 다루는 공백)

| 공백 | 근거 | 설계 절 |
|---|---|---|
| 제품 capture는 정책을 쓰지 않는다 | `src/session.rs` `capture(root, store)` → `artifact::capture` (루트 `.gil/`만 제외, 전 트리 blob 저장). `close_verify`와 `start_at_internal` 둘 다 이것을 부른다. `artifact/mod.rs` 주석: 「아직 crate 안의 아무도 이 경로를 부르지 않는다」 | §6.4 |
| 시작은 인터뷰 전에 저장한다 | `start_at_internal`: 잠금 → `restore::recover` → `reap_tmp` → `initializable` → `check_object_store` → `project_resume::ensure` → `capture` → `Project::start(rules, world)`. `Project::start`는 `ManifestAddress`를 **요구**하며 `Cycles::start(rules, existence, first)`가 뿌리 Interview Cycle의 `entry_snapshot_ref`에 A1을 넣는다 | §4.1 |
| dirty 판정은 전 트리를 본다 | `world_state`: `artifact::observe(root)` 결과를 기준 manifest와 그대로 비교 | §6.4 |
| 복원 계획은 전 트리를 본다 | `restore::run_with_expected`: `observe(root)` → `plan::steps(root, before, target)`가 목표에 없는 현재 파일을 `Delete`로 계획 | §6.5 |
| Snapshot 기록에 정책이 없다 | `state.yaml` `artifacts.snapshots[{id, manifest}]`(Storage Model §3). 정책 원장·`policy` 필드 없음 | §6.1 |
| `.gilignore`를 읽는 자리가 없다 | `GITIGNORE` 상수만 있다. 추천 목록·효과 요약·승인 도구 없음 | §5·§4 |
| 승인 바인딩 없음 | start는 어떤 승인도 받지 않는다. 범위 변경 재인터뷰의 정책 전용 입력 없음 | §4·§6.2 |
| 안내 파일 생성 순서 | `project_resume::ensure`가 capture 직전에 `AGENTS.md`/`CLAUDE.md`에 블록을 덧붙인다(PROJECT-RESUME: 「최초 Snapshot을 만들기 전에」) | §4.3 |
| 시작 잔해 분류 | `initializable`: `.gil/`에 `INTERNAL_ONLY`(`project.lock`·`artifacts/`·`restore/`) 밖의 항목이 있으면 `NotEmpty` 거절 | §4.4 |
| 제품 차단 | `src/command.rs` `product-safety`: `restore`는 전면 거절, `revisit`은 `resume_revisit`(저장된 승인 재개)만 | §6.5·§1.2 |

### 3.3 이번에 결정해야 하는 설계 vs 후순위

이번 결정: 시작 순서(§4.1), 층 우선순위와 버전(§5.1), 추천 기본 선택(§5.2), 제외 항목의 종류 검사(§5.3),
정책 저장 위치와 Snapshot 연결(§6.1), 승인 바인딩 범위(§6.2), 정책 파일 쓰기 순서(§6.3),
적용 시점(§6.4), 정책 경계를 넘는 복원·revisit 규칙(§6.5), 기존 프로젝트 채택(§6.6).

후순위 유지: 중첩 `.gilignore`의 의미(루트만), `core.ignorecase` 대소문자 무시, 전역 Git 설정,
격리 영역(§3.6a 3항), 사람 승인 wire가 있는 일반 restore(§3.6a 4항), 자동 승인 복구와의 결합,
중첩 프로젝트, Chain revisit.

---

## 4. 최초 시작 흐름과 정책 변경 흐름

### 4.1 최초 시작 — 대안 비교

제약: (a) 승인 전에 전체 바이트를 저장하지 않는다. (b) 실제 파일이 있는 프로젝트를 가짜 빈
세계로 시작하지 않는다. (c) 「Snapshot 없이 열린 Cycle」(`every_cycle_has_an_existing_entry_snapshot`)과
`start_writes_baseline_and_first_cycle_in_one_transaction`을 깨지 않는다. (d) 인터뷰에서 정한
제외가 **최초 보관에도 적용**된다.

| 대안 | 내용 | (a) | (b) | (c) | (d) | 비용·영향 |
|---|---|---|---|---|---|---|
| A0 현행 | 전 트리 capture → Interview | ✗ | ✓ | ✓ | ✗ | 지시로 배제 |
| A1 Snapshot 없는 Interview | `gil start`가 Cycle만 열고 승인 뒤 A1 확정 | ✓ | ✓ | **✗** | ✓ | format·`Project::start`·load 검증·World Current 유도·dirty gate 전부 Option 처리. 가장 큰 계약 변경 |
| A2 임시 기본 정책으로 capture 뒤 재기준 | 내장+`.gitignore`+일반 추천으로 A1, 승인 뒤 A2 | △ 승인 밖 바이트가 A1에 남음 | ✓ | ✓ | ✗ | 「나중에 제외」 금지에 걸린다 |
| A3 manifest만 기록, blob 미저장 | 지문만 적고 바이트는 승인 뒤 | ✓ | ✓ | 형식상 ✓ | ✓ | 복원 불가능한 Snapshot이 생겨 `restore_blobs_are_verified_before_the_first_artifact_change`와 충돌. 배제 |
| A4 빈 세계로 시작 | A1 = 빈 manifest | ✓ | **✗** | ✓ | – | 지시로 배제 |
| **A5 두 단계 시작 (권장)** | 1단계: 잠금·`.gil/` 생성·**읽기 전용 목록 관측**·범위 제안·보류 원장. 사람 승인. 2단계: 안내 파일 → `.gilignore` → **정책 적용 capture** → 기존의 한 번 저장(X1·J0·C1·A1) | ✓ | ✓ | ✓ | ✓ | `gil start` 의미가 둘로 나뉜다. 보류 원장(제안)과 승인 도구(제안)가 필요. 범위 승인이 Interview Cycle **밖**에서 일어난다 |

**권장: A5.** 이유:

- (c)를 지키면서 (a)·(d)를 동시에 만족하는 유일한 대안이다. A1만이 같은 효과를 내지만 format
  전체의 「처음부터 완전」 원칙(Artifact Model §10.7)을 포기한다.
- 범위 승인이 Cycle 밖에서 일어나는 것은 새 선례가 아니다. M6-D는 Monitor 버튼 응답을 Report가
  아닌 원장(`approved_plan`)에 적고, M4-R은 `approval.json`을 `.gil/revisit-preserved/`에 먼저 둔다.
  A5의 1단계 보류 원장은 그 틀을 **시작 전**에 적용한 것이다.
- 「시작 인터뷰에서 범위를 승인한다」는 §3.6 1항의 사용자 흐름은 유지된다. 사용자가 보는 것은
  같은 대화 안의 범위 제안·승인·시작 완료다. 다만 Core 기록상 그 승인은 Interview Cycle의
  Synthesis가 아니라 **시작 승인 원장**에 남는다. 이 점은 사용자 결정 사항(§10 Q1)으로 남긴다.

### 4.2 권장 시작 흐름 (A5)

```text
[1단계] gil start  (기존 명령, 기록 없음 → 「범위 제안」으로 끝난다)              (제안)
  잠금·.gil/ 생성 ─ restore::recover ─ reap_tmp ─ initializable(§4.4 완화) ─ check_object_store
  읽기 전용 목록 관측: 경로·종류만, 바이트·지문 없음                                (제안: inventory)
    적용 정책 초안 = 시스템 보호 + 승인 후보 .gilignore; .gitignore는 추천 참고만
    출력 = 폴더 단위 효과 요약(추적 n / 제외 m / 이유), 일반 추천 후보와 기본 선택(§5.2)
  보류 원장 .gil/start/pending.json 기록(§4.4)                                     (제안)
  receipt: 효과 요약 + 「에이전트 추천을 더하거나 그대로 승인」 다음 호출 안내

[대화] 에이전트가 프로젝트를 읽고(읽기는 허용) 프로젝트별 추천을 gil_scope_propose로 더한다   (제안)
  Core가 효과를 다시 계산해 새 제안 digest를 돌려준다. 사람은 효과를 보고 승인·수정한다.
  승인 채널: 모든 구성에서 Monitor 버튼. 정확한 제안 판·화면 입력을 검증한다.
             에이전트의 대화 응답 전달은 최종 승인이 아니다 (2026-11-19 확정)

[2단계] gil start  (승인된 보류 원장이 있으면 완료 단계로 들어간다)
  바인딩 재검증(§6.2): 승인 대상 규칙과 실제 추적 대상 목록의 동일성 확인(제외 내부 변화는 재승인 사유 아님)
  project_resume::ensure (AGENTS.md / CLAUDE.md 블록, 기존 순서 유지: capture 직전)
  .gilignore 쓰기 (디스크 원문 == 제안 원문이면 쓰지 않음, 다르면 §6.3 규칙)
  capture_with_policy(root, store, P1) → A1                                        (제안)
  한 번의 저장: X1 · J0 · current_existence · 뿌리 Interview Cycle(entry=A1) · A1(policy=P1) · 정책 원장 P1
  보류 원장 제거 (상태 저장 뒤, 다음 명령이 잔해로 정리할 수 있음)
  receipt: 기존 start receipt + 「적용 정책 P1, 추적 n, 제외 m」 + show_gil_monitor 인계(기존)
```

이후 Interview Cycle(호칭·목표 Question → … → Synthesis)은 **현행과 같다.** 범위 승인은
Interview의 선행 조건이지 Interview Step이 아니다.

**승인 전에 무엇을 읽는가.** 1단계는 디렉터리 이름·항목 종류·`.gitignore`/`.gilignore`
원문만 읽는다. 파일 바이트를 읽거나 지문을 계산하지 않는다(용량 표시가 필요하면 metadata
길이만). 제외 규칙에 맞는 디렉터리 안은 들어가지 않는다(`Prune`). 에이전트는 추천을 위해
파일을 읽을 수 있다 — 읽기는 어느 단계에서나 허용된 행위다. 쓰기는 `.gil/`뿐이다.

**승인 전 보존해야 할 최소 상태(보류 원장).** 제안 digest, 정책 원문(원본별 디렉터리·종류·
텍스트 digest; 텍스트는 객체 창고 blob으로), 추천 목록과 선택 상태와 이유, 에이전트 추천과
이유, 효과 요약의 경로 집합 digest(추적 집합·제외 집합), 승인 상태(`proposed|approved`), 승인
채널. **X1·J0·Cycle·Snapshot은 만들지 않는다.** 보류 원장만 있는 `.gil/`은 프로젝트가 아니다.

**최초 보관본 참조와 인터뷰의 연결.** A1 기록이 `policy: P1`을 갖고(§6.1), 정책 원장 P1이
`applied_from: snapshot:A1`, `approval.channel: start`를 갖는다. 뿌리 Cycle `entry_snapshot_ref`는
현행대로 A1이다. `baseline_snapshot_ref`를 뿌리에 두지 않는 결정(§4.1 규범)은 유지한다.

### 4.3 취소·중단·재시작

**2026-11-19 취소 방향 채택, 구현 대기:** 명시적 취소는 준비·승인 기록을 보존하고 취소로
표시한다. 앱 종료·연결 끊김은 취소가 아니다. 아래 파일명·필드명은 후보이며 취소 기록 삭제와
취소 후 자동 시작이라는 기존 후보는 폐기한다. 정본은 Artifact Model §3.6의 시작 계약이다.

| 상황 | 남는 것 | 이어지는 방식 |
|---|---|---|
| 승인 전 앱 종료·연결 끊김 | 준비 기록 보존, 취소 아님 | 재개 시 현재 범위 제안을 보여 주고 승인 대기를 유지한다. 달라진 제안과 기존 기록을 구분한다 |
| 사용자가 시작 준비를 명시적으로 취소 | 준비·승인 기록 보존 + 취소 표시 | 재실행·재연결·이전 요청 재시도로 자동 시작하지 않는다. 새 시작 의사가 있을 때 현재 규칙·추적 목록을 재확인하고 필요한 모니터 재승인을 받는다. 작업 파일·정책을 되감지 않는다 |
| 승인 뒤 정식 시작 전 앱 종료·연결 끊김 | 승인과 준비 상태 보존, 취소 아님 | 재개 시 확정 상태와 범위를 재확인한다. 규칙·추적 목록 변경은 모니터 재승인, 내용만 변경은 기존 검사에 따른다 |
| 2단계 capture 뒤 state 저장 전 중단 | 미참조 blob·manifest, `.gilignore`, 안내 블록 | 기존 규칙과 같다: 미참조 객체는 손상이 아니고, 재시도가 같은 객체를 공유한다(`retrying_a_start_shares_the_objects_the_first_try_left`). `.gilignore`는 승인 원문과 같으면 그대로 둔다. 안내 블록은 PROJECT-RESUME 규칙대로 중복 추가하지 않는다 |
| 정식 시작 저장 뒤 중단 또는 취소 요청 도착 | 완성된 프로젝트 + 준비·승인 이력 | 확정 상태를 안내하고 중복 체인 생성·준비 취소로의 소급 처리를 하지 않는다. 준비 기록은 단순 잔해로 삭제하지 않는다 |

**승인과 최초 보관 사이의 변경.**

- `.gilignore` 승인 후보 규칙 변경은 최초 적용 재승인 대상이다. `.gitignore` 내용만 변경은 정책 변경이 아니다.
- **2026-11-19 확정:** 규칙 또는 실제 추적 대상 목록이 승인 시점과 다르면 최초 적용을 보류하고 변경점을 보여 준 뒤 모니터에서 새 판을 승인받는다. 제외 목록 전체의 동일성을 요구하는 과거 후보는 채택하지 않는다.
- 추적 파일의 **내용**만 바뀐 경우 → 허용. 승인 대상은 범위이고, A1은 §4 규범대로 「capture 시점의
  실제 상태」다.
- 완전히 제외된 폴더 내부 변화는 재승인 사유가 아니며 내부를 탐색하지 않는다. 재포함 경로·규칙 입력 변경은 별도로 판정한다.

### 4.4 시작 잔해와 안내 파일 순서

- `initializable`의 `INTERNAL_ONLY`에 `start/`를 더한다(제안). `state.yaml`이 없고 `start/pending.json`이
  Core가 아는 모양이면 보류 상태로 읽고, 모르는 항목·링크·디렉터리면 기존처럼 거절한다.
- 안내 파일(`AGENTS.md`/`CLAUDE.md`) 블록은 **승인 뒤 2단계에서, capture 직전**에 덧붙인다. 현행
  순서(「최초 Snapshot을 만들기 전」)를 지킨다. 취소 자체는 사용자 파일을 변경하지 않으며,
  이미 적용한 정책·파일을 소급해서 되감는다는 뜻은 아니다.
  `.gilignore`가 안내 파일을 제외하도록 승인됐다면 그대로 따른다 — PROJECT-RESUME는 「안내는 일반
  Artifact이며 제외 목록 예외를 만들지 않는다」고 정했다. 1단계 효과 요약이 그 사실을 표시한다.
- 승인된 `.gilignore` 자체는 내장 제외(§6.1)라 A1에 들어가지 않는다. 그 원문은 정책 원장이 보존한다.

### 4.5 작업 중 정책 변경 흐름

단일 열린 Cycle 규칙과 dirty gate를 그대로 쓴다. 새 전이는 없다.

```text
열린 Experiment 중 범위 변경 필요
  approved-plan 프로필: gil_plan_request_interview(reason)  — 기존, 요청만 보존
  기본 프로필: 에이전트가 사용자에게 알리고 현재 Cycle을 규칙대로 닫는다
  ↓
현재 Experiment를 사실대로 닫는다
  열린 Verify가 있으면 그 변경을 확정(evidence 포함) → Analysis → Outcome → Cycle Report
  dirty gate: 추적 파일이 바뀐 채로는 Interview를 열 수 없다  ← 「미확정 변경을 정책 변경으로 숨길 수 없다」의 실제 근거
  ↓
Interview Cycle open (기존 전이)
  gil_scope_propose: 현재 적용 정책 P_k + 변경(추가/해제) + 이유 → 효과 요약(추가로 빠지는 n, 새로 들어오는 m) + digest   (제안)
  승인 채널은 §4.2와 같다 (모든 구성: Monitor 버튼, 에이전트 승인 우회 불가)                         (2026-11-19 확정)
  승인 → 정책 원장에 P_{k+1} = approved, pending activation 기록 → .gilignore 갱신(§6.3)
  Interview의 나머지(Synthesis·Outcome)는 현행과 같다. Interview는 세계를 확정하지 않는다.
  ↓
다음 Experiment의 Verify close
  capture_with_policy(P_{k+1}) → 새 Snapshot(policy=P_{k+1}) → P_{k+1}.applied_from 기록
  이 시점부터 해제된 파일의 현재 바이트가 보관되고, 추가 제외된 파일은 더 보관되지 않는다
```

적용 시점이 **다음 세계 확정(Verify close)**인 이유는 §6.4에 있다. 승인 즉시 적용하면 Interview
자신의 Outcome·Cycle close가 dirty gate에 걸린다(해제된 파일이 「추가」로 보인다). 세계를 확정할
권한이 `gil start`와 Verify close에만 있다는 §5 규범과 같은 지점에 정책 전환을 둔다.

**이력과 증거.** 변경 전 Snapshot들은 P_k로, 뒤는 P_{k+1}로 기록된다. 원장은 P_k→P_{k+1}의
승인 근거·Cycle·채널을 남긴다. 추가 제외된 파일의 과거 바이트는 과거 Snapshot에 그대로 있다.
해제 기간의 이력을 만들어 넣지 않는다(C10).

---

## 5. 규칙 우선순위와 상태 전이 표

### 5.1 입력 층과 우선순위

| 층 | 입력 | 위치 | 되살림 | 비고 |
|---|---|---|---|---|
| L0 내장 보호 | `.git` 구성 요소(어느 깊이, 파일/디렉터리), 루트 `.gil/`, **루트 `.gilignore` 자체(2026-11-19 승인)** | 고정 | `!`로 불가 | 중첩 `.gil/`은 제외가 아니라 **거절**(기존) |
| 적용 규칙 | 승인된 루트 `.gilignore` | 승인 기록 원문 | 시스템 보호는 재포함 불가 | Git 규칙은 입력하지 않음 |

이전 L1/L2 층 순서 설계는 폐기한다. `.gitignore` 수집기는 추천 참고에만 사용할 수 있으며
평가·정책 식별값·복원 정책 입력으로 연결하지 않는다. 기존 엔진의 문법 구현 재사용은 별도 검토다.
승인된 하위 파일 재포함은 부모 제외를 이유로 무시하지 않고 필요한 탐색·검사를 수행한다.
정책 버전·식별값 형식과 기존 기록 호환은 후속 설계이며 자동 이전하지 않는다.

**문법(제안).** 익숙한 gitignore 부분집합 문법을 사용할 수 있으나 Git 정책을 상속한다는 뜻은 아니다.
주석 `#`, 빈 줄, `!`, `/` 앵커, `*`·`?`·`[...]`·`**`, 끝 `/` 디렉터리 전용. 지원하지 않는 것은
Tracking Policy 제안 §2.3과 같다(`core.ignorecase`, 전역 설정, `.gitattributes`). **해석할 수 없는
줄은 정책 전체를 거절**한다(기존 `InvalidPattern`). 1단계·재인터뷰 제안 단계에서 줄 번호와 이유를
보여 주고 수정 뒤 재제안한다. 승인된 정책에는 해석 불가 줄이 있을 수 없다.

**루트가 아닌 `.gilignore`.** 권장: 정책으로 읽지 않고 일반 Artifact로 취급하며 효과 요약에
「무시된 정책 파일」로 경고한다. 대안: 거절(§10 Q9).

**`.gilignore`라는 이름의 링크·디렉터리.** 원문으로 읽지 않고 **거절**한다(`.gitignore` 링크와 같은
규칙). `.git`이 파일인 경우(worktree·submodule)는 기존대로 내장 제외다.

**전역 Git 설정.** 적용 정책 입력으로 읽지 않는다(독립 정책 합의). 같은 저장소를 다른 PC에서 열어도 같은 `policy_id`가
나와야 하며, 사용자 홈의 설정은 프로젝트 기록이 아니다. 사용자가 전역에 둔 규칙을 쓰려면
`.gilignore`에 옮겨 승인받는다.

### 5.2 추천과 강제의 구분

일반 추천과 비밀값 파일 추천의 미리 선택·사용자 확인 후 승인 적용은 2026-11-19 확정됐다. 정책 파일의 일반 보관·복원 제외도 후속 승인됐다. 아래 표의 세부 패턴·프로젝트별 추천 기본값 등 나머지 권장안은 미승인이다.

| 종류 | 예 | 기본 선택(권장) | 사용자 해제 | 표시 근거 |
|---|---|---|---|---|
| 강제 보호 (L0) | `.git/`, 루트 `.gil/`, 루트 `.gilignore` | 해당 없음 | 불가 | 「시스템 경계 — 복원이 쓰거나 지우지 않음」 |
| 일반 추천 — 버전 관리·도구 내부 | `.svn/`, `.hg/`, `.idea/`, `.vscode/`(설정 파일은 제외 안 함 선택지) | 선택됨 | 가능 | 「다른 도구의 내부 기록」 |
| 일반 추천 — 의존성 설치 폴더 | `node_modules/`, `.venv/`, `venv/`, `vendor/`(언어별), `target/`(Rust), `__pycache__/` | 선택됨 | 가능 | 「설치본은 목록·고정 파일에서 재생성 가능」 |
| 일반 추천 — 빌드·캐시 출력 | `dist/`, `build/`, `out/`, `.cache/`, `.next/`, `coverage/`, `*.log` | 선택됨 | 가능 | 「재생성 가능한 출력」 |
| 일반 추천 — 비밀 값 | `.env`, `.env.*`, `*.pem`, `*.key`, `.ssh/`, `.aws/` | 선택됨 | 가능 | 「보관본에 비밀을 복제하지 않음」. M4-R의 보호 목록과 같은 집합에서 출발 |
| `.gitignore` 참고 | 작업 목적에 맞는 개별 제외 후보 | Git에서 제외했다는 이유만으로 자동 선택·적용하지 않음 | 가능 | 추천 이유·출처를 표시하고 채택한 규칙을 `.gilignore`로 승인 |
| 에이전트 추천 | 프로젝트별 (예: 생성한 기준 자료 폴더) | 선택됨으로 제안 | 가능 | 에이전트가 적은 이유를 그대로 표시 |

**절대 추천하지 않는 것(제안 목록의 금지 집합).** 의존성 목록·버전 고정 파일:
`package.json`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `Cargo.toml`, `Cargo.lock`,
`requirements*.txt`, `pyproject.toml`, `poetry.lock`, `Pipfile*`, `go.mod`, `go.sum`, `Gemfile*`,
`composer.*`. 일반 추천은 **디렉터리 패턴**으로만 쓰고 이 파일들에 맞는 파일 패턴을 넣지 않는다.
사용자가 직접 넣으면 효과 요약이 「의존성 목록이 제외됨」을 경고하되 막지는 않는다(C5: 제외는
금지가 아니다). 일반 추천과 비밀값 파일 포함은 2026-11-19 확정됐다. 비밀값 파일의 세부 패턴·예시 파일 구분은 추가 설계한다.

추천 목록은 Core의 상수 카탈로그(제안)이며 이유 문구는 다국어 테이블을 따른다. 프로젝트 유형
감지(예: `Cargo.toml` 있음 → `target/` 추천 강조)는 이유 표시에만 쓰고 자동 선택 범위를 넓히지 않는다.

### 5.3 제외된 항목의 탐색·링크·특수 파일·중첩 `.gil`

| 자리 | 현행 §3.4 | 권장 | 근거 |
|---|---|---|---|
| 최종 정책상 **완전히 제외된 디렉터리** 안 | 기본 관측은 내부를 검사 | **2026-11-19 채택:** 내부 탐색 생략 | 내부 미확인으로 표시. 재포함 파일이 있으면 필요한 경로는 탐색·검사 |
| 직접 만난 **제외 연결 파일** (`.venv -> …`) | 기존 관측은 거절 | **2026-11-19 채택:** 대상에 접근하지 않고 건너뜀, 복원 비간섭 | 경로 종류·정책은 확인하고 대상은 따라가지 않음. 경로·제외 규칙·미탐색 사실 표시. 재포함 시 거절 |
| 제외되지 않은 링크·특수 항목 | 거절 | 거절 유지 | 변경 없음 |
| 규칙에 **직접** 맞는 중첩 `.gil/` | 거절(`NestedGilBoundary`) | 거절 유지 | 중첩 프로젝트 미지원, 숨은 제외 영역 금지 |
| `Prune`된 디렉터리 **안**의 중첩 `.gil/` | – | 보지 않음, `excluded`에 부모 디렉터리가 남음 | Tracking Policy 제안 §2.5의 한계를 그대로 채택. 중첩 프로젝트 지원이 아니다 |
| `.gil`이라는 이름의 일반 파일 | 일반 파일 | 일반 규칙 | 변경 없음 |
| 비UTF-8·`\` 포함 이름 | 거절 | 제외 디렉터리 안(`Prune`)이면 보지 않음, 그 밖은 거절 | 열거되는 즉시 거절은 유지 |

완전 제외 폴더의 내부 탐색 생략·재포함 경로 검사·직접 만난 제외 연결 파일의 건너뛰기는
2026-11-19 승인됐다. 연결 대상 접근·추적 지원은 승인하지 않았으며 시스템 보호 경계는 유지한다.

**운영체제 차이.** 정책은 `/` 구분 `EntryPath` 문자열만 판정한다(기존). 대소문자는 구분한다 —
macOS·Windows의 대소문자 접기 볼륨에서 `Build/` 규칙은 `build/`에 맞지 않는다. 효과 요약이 실제로
맞은 경로를 보여 주므로 사용자가 확인할 수 있다. Windows 예약 이름·대체 스트림·역슬래시는 기존
관측·preview가 이미 거절한다. Windows에서 기록이 `LOCALAPPDATA` 아래로 외부화된 프로젝트
(`project_state::state_for_root`)도 `.gilignore`는 **작업 폴더 루트**에 두고 정책 원장은 기록
저장소(`state.yaml` 옆)에 둔다. 두 위치의 차이는 §6.6.

### 5.4 상태 전이 표

정책에는 세 가지 상태가 있다. 셋을 섞지 않는다.

| 상태 | 뜻 | 저장 자리 |
|---|---|---|
| **초안(draft)** | 디스크의 `.gilignore`와 에이전트 추천으로 작성한 후보. `.gitignore` 참고 규칙은 개별 채택이 필요하며 아직 효력 없음 | 디스크 + 보류 제안(digest) |
| **승인됨·적용 대기(approved, pending)** | 사람이 승인했지만 아직 어떤 Snapshot도 이 정책으로 확정되지 않음 | 정책 원장(`applied_from: null`) |
| **적용 중(applied)** | 현재 서 있는 세계 Snapshot을 만든 정책. 관측·dirty·복원이 쓰는 정책 | Snapshot 기록의 `policy` |

| 현재 | 사건 | 다음 | 비고 |
|---|---|---|---|
| (프로젝트 없음) | 1단계 `gil start` | 초안 제안(보류 원장) | 기록 없음 |
| 초안 제안 | `gil_scope_propose`(에이전트 추천 추가·수정) | 새 초안 제안 | 이전 제안은 승인이 아니므로 교체 |
| 초안 제안 | 파일 변경으로 경로 집합 변경 | 새 초안 제안 | 재표시 |
| 초안 제안 | 승인 | 승인됨(시작용) | 보류 원장 `approved` |
| 승인됨(시작용) | 2단계 성공 | 적용 중 P1 (A1) | 원장 `applied_from: snapshot:A1`, `channel: start` |
| 승인됨(시작용) | 바인딩 실패 | 초안 제안 | 승인 무효, 사유 표시 |
| 적용 중 P_k | 디스크 `.gilignore` 편집·삭제·교체 | 적용 중 P_k + **드리프트 표시** | 효력 변화 없음. `.gitignore` 변경은 정책 드리프트가 아니며 추적 대상일 때 일반 작업물 변경 |
| 적용 중 P_k | Interview에서 제안 승인 | 적용 중 P_k + 승인·대기 P_{k+1} | `.gilignore` 갱신(§6.3) |
| 승인·대기 P_{k+1} | 또 다른 Interview 승인 P_{k+2} | 승인·대기 P_{k+2} | P_{k+1}은 원장에 `superseded`로 남고 적용 안 됨 |
| 승인·대기 P_{k+1} | Verify close | 적용 중 P_{k+1} | 새 Snapshot `policy: P_{k+1}`, `applied_from` 기록 |
| 승인·대기 P_{k+1} | revisit / Interview만 열림 | 그대로 대기 | 다음 세계 확정까지 |
| 적용 중 P_k (현 세계 S) | `gil restore` | 적용 중 P_k | 목표 S의 정책 = P_k. 제외 경로 무변경 |
| 적용 중 최신 승인 P_k | revisit → 과거 Exit S'(출처 정책 P_j) | 적용 정책은 **P_k 유지** | §6.5 최신 정책 원칙. 결과 세계 참조 상세는 후속 |
| 적용 중 P_k | 정책 없는 과거 세계(legacy)로 revisit | 적용 중 **legacy whole-tree** | §6.5·§6.6 |

---

## 6. 저장·승인·실패 복구·호환성 설계

### 6.1 정책 원장과 Snapshot 연결 (방향 채택, 상세 저장 구조는 제안)

```yaml
artifacts:
  next_snapshot_id: 3
  snapshots:
    - id: A1
      manifest: {algorithm: sha256, digest: …}
      policy: P1                      # (제안) 없으면 「정책 미상 = legacy whole-tree」
    - id: A2
      manifest: {…}
      policy: P1
  tracking:                           # (제안) 선택적 확장 블록. 없으면 정책 원장 없음
    next_policy_id: 2
    approved_pending: null            # 또는 P2
    policies:
      - id: P1
        version: 2                    # 후보 값, 실제 버전은 후속 설계
        policy_id: gil-tracking-policy/2:sha256:…
        sources:                      # 승인 시점에 동결된 원문. 디스크를 다시 읽지 않는다
          - directory: ""
            kind: gilignore
            text: {algorithm: sha256, digest: …}   # 객체 창고의 blob
        approval:
          proposal_digest: …
          channel: start | monitor | agent      # 사람 클릭 / 에이전트 전달을 구분, 신원 인증 아님
          in_cycle: null | cycle:C3
          recommendations:                      # 보여 준 선택과 이유의 요약(표시 재현용)
            - {rule: "node_modules/", selected: true, source: gil|agent|gitignore, reason_key: …}
        applied_from: snapshot:A1 | null
        superseded_by: null | P3
```

- **원문은 blob.** 승인된 `.gilignore` 원문을 승인 기록에 보관한다. `.gitignore`는 정책
  평가 원문으로 동결하지 않는다. 추천 출처 기록과 실제 정책 입력을 구분한다. `.gitignore` 자체는
  승인 범위에 따라 일반 Artifact로 보관·복원되며 그 내용은 적용 정책을 바꾸지 않는다.
- **`policy_id`는 유도값이자 검증값.** load 시 `sources`로 다시 계산해 `policy_id`와 같은지 검사한다.
  다르면 손상으로 거절(고치지 않는다).
- **Snapshot의 `policy`가 없으면 legacy.** 기존 format 4·5·6·7 기록은 필드가 없다. 그 Snapshot의
  관측 의미는 현행 `artifact::observe`(루트 `.gil/`만 제외)로 **알려져** 있으나 이름표가 없다.
  이것을 「정책 미상(legacy whole-tree)」이라 부르고 임의의 ID를 붙이지 않는다(Tracking Policy 제안 §3 2항).
- **형식 번호.** 두 길이 있다. (가) `approved_plan`·`pending_cycle_revisit`과 같은 **선택적 확장**:
  필드를 모르는 구형 writer는 unknown-field로 거절하므로 안전하다. 형식 번호 상승 없음.
  (나) 새 형식 번호(4/5/6/7 → 각 +4 등). 권장은 (가)이되 결정은 통합 담당·사용자(§10 Q2). 어느 쪽이든
  기존 기록을 **자동 변환하지 않는다.**
- **시작 보류 원장**(`.gil/start/pending.json`, 제안)은 `state.yaml`이 아니다. 2단계가 끝나면 그 내용이
  `tracking.policies[P1]`로 들어가고 보류 원장은 사라진다. 형식: JSON, `schema_version: 1`, 위의
  `policies[]` 항목 하나와 `inventory_digest`(추적 집합·제외 집합의 경로 digest), `state: proposed|approved`.
  모르는 필드·링크·크기 초과는 거절한다(`locale.json`과 같은 보수적 읽기).

### 6.2 승인 바인딩 — 승인 대상과 적용 대상이 같은가

승인은 다음 묶음의 SHA-256 digest에 바인딩된다(제안, M6-D 제안 digest와 같은 방식).

```text
proposal_digest = sha256(canonical JSON {
  version, sources[{directory, kind, text_digest}],      # 정책 원문
  inventory: {tracked_paths_digest},  # 실제 추적 대상 목록, 내용 바이트 제외; 필드명·해시 상세는 제안
  recommendations[{rule, selected, source}],             # 보여 준 선택
  base_policy: P_k | null                                # 변경 전 정책 (시작은 null)
})
```

적용 직전(2단계 또는 Verify close에서의 활성화) 재검증:

1. 승인된 정책 원문과 실제 적용 대상을 확인한다. 확인된 파일 갱신 실패로 `.gilignore`가 구판인
   경우에는 승인 기록의 원문을 사용하고 동기화 실패를 표시한다(2026-11-19 채택). 별도 사용자
   편집으로 규칙이 달라진 경우와 구별하며 최초 적용 전에는 기존 변경점 표시·모니터 재승인 규칙을 따른다.
2. `.gitignore` 원문 집합의 동일성은 적용 조건이 아니다. Git 규칙 내용 변화로 범위 재승인을 요구하지 않는다.
3. 최초 적용 전 실제 추적 대상 목록과 규칙을 비교한다(2026-11-19 채택). 달라지면 적용 보류·변경점 표시·모니터 재승인. 완전 제외 폴더 내부나 기존 파일 내용만의 변화는 범위 재승인을 요구하지 않는다.

승인 채널은 저장하되 **신원 인증으로 해석하지 않는다**(M6-D와 같은 한계 문구). 2026-11-19 결정으로
모든 구성에서 모니터 최종 승인이 필요하며 과거 `channel: agent` 승인 후보는 채택하지 않는다.

### 6.3 정책 파일 쓰기와 승인 기록의 순서·실패 — 방향 채택, 상세 구현은 제안

승인의 진실은 원장이고 `.gilignore`는 그 **투영**이다.

```text
1. 원문 blob 확정 (객체 창고)
2. 원장 기록 저장  (시작: pending.json approved / 재인터뷰: state.yaml tracking.approved_pending)
3. .gilignore 동기화 — 쓰기 전 확인된 기준에서 사용자 편집·삭제가 없음을 검사한다.
   이미 이번 승인 원문과 같으면 쓰지 않는다. 사용자 변경 또는 불확실이 있으면 덮어쓰지 않고 차이를 표시한다.
   파일 부재만으로 신규 생성이 안전하다고 단정하지 않는다(사용자 삭제와 구별 필요).
   쓰기 방식: locale.json과 같은 pinned-directory·임시 파일 배타 생성·fsync·교체. 링크·디렉터리면 거절.
```

- 2 뒤 3 전 중단: 승인 기록은 유효하며 구판 파일·동기화 미완료를 표시한다. 재시도 직전 사용자 변경을 검사하며 그 변경을 덮어쓰지 않는다.
- 3만 실패(권한 등): 승인 유효, 표시 유지. 적용(활성화)은 원장 원문으로 하므로 막히지 않는다.
- 원장 저장 실패: 새 승인을 완료로 처리하지 않는다. 기존 확인된 승인 상태를 보존한다. 결과가 불확실하면 기록을 조회해 확인하며 중복 승인을 추측하지 않는다. blob은 미참조로 남을 수 있다.
- **에이전트는 `.gilignore`를 직접 쓰지 않는다.** Interview 중 Core의 이 쓰기는 내장 제외 파일에 대한
  것이라 「Interview는 Artifact를 바꾸지 않는다」와 충돌하지 않는다. 그 밖의 파일 쓰기는 Interview에서
  여전히 금지다. 에이전트가 직접 편집한 `.gilignore`는 드리프트(초안)일 뿐이다.
- 삭제·교체·손상: 디스크 `.gilignore`가 없어져도 적용·승인 정책은 그대로다. 「파일이 없으니 전부
  추적」으로 넓히지 않는다(§3.6 5항). 해석 불가로 손상되면 초안으로서 거절되고 드리프트 표시.
  `.gitignore` 삭제는 추적 대상이면 일반 Artifact 변경이며 정책 드리프트는 아니다.

### 6.4 관측·dirty 판정·Verify 확정에 같은 정책을 쓰는 방법

**규칙 1 — 판정 기준은 기준 Snapshot의 정책이다.** `world_state`는 `project.world_snapshot()`이 유도한
기준 Snapshot S의 `policy`(없으면 legacy)를 읽어, 그 정책으로 `observe_with_policy`하고 S의 manifest와
비교한다. 제외 경로의 변화는 차이가 아니다(C6). 정책 미상(legacy)이면 현행 `observe`를 그대로 쓴다 —
기존 프로젝트의 동작이 **한 글자도 바뀌지 않는다.**

**규칙 2 — 활성화는 세계 확정 지점에서만.** `close_verify`는 `tracking.approved_pending`이 있으면 그
정책으로, 없으면 기준 Snapshot의 정책으로 `capture_with_policy`(제안: `artifact::capture`의 정책판,
첫 훑기에 blob 저장·두 번째 훑기 검증은 동일)한다. 새 Snapshot 기록에 `policy`를 쓰고, 활성화된 원장
항목에 `applied_from`을 적고 `approved_pending`을 비운다. 모두 같은 한 번의 state 교체다.

**규칙 3 — 증거.** `require_verify_evidence`는 변경 없이 그대로 동작한다. 제외된 경로를 `evidence_files`에
적으면 `evidence.not_in_snapshot`으로 거절된다. 거절문에 「이 경로는 현재 정책이 제외한다 — 다른 위치에
두거나 재인터뷰로 해제」 Topic을 연결한다(안내 세션 범위). 제외 파일만으로 증거를 대신할 길은 없다(C11).

**규칙 4 — status 표시.** 「적용 중 P_k(추적 n·제외 m)」, 「승인·대기 P_{k+1} — 다음 Verify 확정 시 적용」,
「정책 파일 드리프트 — 디스크 원문이 승인본과 다름」을 구분해 보인다. 전체 목록·내부 digest는
기본 출력에 내지 않는다(기존 receipt 원칙).

왜 승인 즉시가 아닌가: 해제(새로 포함)된 경로는 기준 Snapshot에 없으므로 즉시 적용하면 Interview의
Outcome·Cycle close가 「추가됨」으로 dirty 거절된다. 추가 제외는 즉시 적용해도 가능하지만 두 방향을
다르게 두면 규칙이 갈린다. Verify close에 두면 둘 다 자연스럽다: 해제된 파일은 그 Verify가 처음
보관하고, 제외된 파일은 그 Verify부터 빠진다.

### 6.5 복원 계획과 정책 경계 — 2026-11-19 방향 확정, 구현 대기

현재 관측·변경 감지·보관·복원은 항상 최신 유효 승인 정책을 따른다. 과거 정책은 보관본의
관측 출처로 보존하되 현재 복원 범위를 결정하거나 자동 재적용하지 않는다.

과거 R1(정책 판 차이만으로 전면 거절), R2(두 정책의 교집합에 따른 조건부 허용 및 대상 정책
재적용)는 채택하지 않는다. R3 후보를 그대로 채택한 것도 아니다. 최신 정책의 추적 세계 전체를
복원하고 현재 제외 파일은 일관되게 건드리지 않는 원칙을 따른다. 복원 결과를 과거 원본 manifest와
동일한 것으로 간주하는 구현은 허용하지 않는다.

기존 preview의 PolicyMismatch나 before/target 두 정책 교집합 검사를 그대로 새 제품 규칙으로
재사용하지 않는다. 최신 승인 정책으로 계획·실행·재관측을 일치시키고 정책 판 차이와 실제
저장 객체·권한·경로 충돌을 구분해야 한다. 과거 보관본의 식별자·내용은 변경하지 않는다.

**2026-11-19 기록 부재 처리 확정:** 새로 추적하는 파일은 최초 보관 시점부터 복원 가능하다.
그 이전 미추적 상태를 추측하거나 현재/후속 내용으로 대신하지 않는다. 최신 추적 세계 전체의
복원 기록이 부족하면 적용 전에 경로·근거 부족·최초 보관 기준점을 설명하고 거절한다.
관측된 파일 부재와 미추적 기록 부재는 구분한다. 추적 확대 승인 화면에는 이전 전체 복원에
제한이 생길 수 있음을 표시한다. 최초 보관 전 기준점은 미확정으로, 보관 후 실제 기준점은 조회 가능하게 한다.

**남은 설계:** 복원 근거 보관본·최신 정책·결과 세계의 참조 관계, 최초 보관 및 관측 범위의
검증 구현, 기존 저장 형식의 읽기·쓰기 경계. 이는 최신 정책 선택에 예외를 추가하는 작업이 아니다.
없는 과거 내용을 만들어 내거나 현재 파일을 근거 없이 삭제하는 규칙은 승인하지 않았다.
기존 보존·승인·실제 경로 안전 검사는 유지하며 일반 restore 제품 차단 해제는 별도다.

### 6.6 기존 프로젝트의 채택과 호환 전략

- **자동 이관 없음.** 기존 기록은 `tracking` 블록도 `policy` 필드도 없이 그대로 읽힌다. 모든 판정은
  legacy 의미(현행 `observe`)로 동작한다. 설치본 업데이트가 사용자 프로젝트를 순회하거나 고치지 않는다.
- **명시적 채택 절차.** 기존 프로젝트에서 「추적 제외를 정하자」 → 현재 열린 Cycle을 규칙대로 닫고
  Interview를 연다(열려 있는 Interview면 그 안에서) → `gil_scope_propose`(base_policy: legacy) → 효과 요약
  (처음 적용이므로 「새로 빠지는 n」만 있다) → 승인 → `tracking` 블록 생성·P1 승인·대기 → 다음 Verify close에서
  활성화. 그 전까지 dirty·복원은 legacy 의미. 채택은 **Interview 안에서만** 제안·승인한다(§10 Q10).
- **Windows.** 기록이 작업 폴더 밖(`LOCALAPPDATA`)에 있는 프로젝트도 `.gilignore`는 작업 폴더 루트에
  둔다. 사용자가 보고 고치는 파일이기 때문이다. 원장·blob은 기록 저장소에 있으므로 작업 폴더를 동기화
  도구가 건드려도 승인은 보존된다. 두 위치의 볼륨이 다르면 M4-R과 같은 이유로 복구는 거절되지만 정책
  승인·관측에는 영향이 없다. `.gilignore` 쓰기는 `project_resume`가 쓰는 핸들 기반 경로(`windows_fs`)를 쓴다.
- **구형 바이너리.** `tracking`·`policy`를 모르는 구형 writer는 unknown-field로 거절한다. 조용히 무시하고
  전 트리로 쓰는 것보다 안전하다. 사용자에게는 「새 버전이 필요하다」로 보인다.
- **호환성 선언의 한계.** 이 설계는 「정책 있는 Snapshot」과 「legacy Snapshot」이 한 프로젝트에 공존하는
  것을 허용하고 그 경계는 §6.5의 최신 승인 정책 원칙으로 설계한다. legacy Snapshot으로의 일반 restore를 안전하다고 선언하지 않는다.

---

## 7. 핵심 미결 사항의 대안과 권장안

| # | 쟁점 | 대안 | 권장 | 비용 / 기존 계약 영향 |
|---|---|---|---|---|
| D1 | 최초 시작 순서 | A0–A5 (§4.1) | **A5 두 단계 시작** | `gil start` 의미 분리, 보류 원장(제안), 승인 도구(제안). Artifact Model §4 서술·Existence Model §4 흐름의 **앞 단계 추가**(한 번 저장 규칙은 유지) |
| D2 | 범위 승인의 기록 자리 | Interview Synthesis 안 / **별도 승인 원장** | 별도 원장 (시작은 Cycle이 없어 불가피, 재인터뷰도 같은 도구로 통일) | Cycle Model §16 Synthesis 필드는 변경 없음. 새 제안 도구와 모니터 전용 승인 경로(이름 미정) |
| D3 | 승인 채널 | 모든 구성에서 Monitor 최종 승인 | **2026-11-19 채택** | 대화는 설명·수정. 시작 준비 화면·제안 판 검증 필요, 실제 개인 신원 인증은 아님 |
| D4 | 독립 추적 정책 | 시스템 보호 + 승인된 `.gilignore` | **2026-11-19 후속 합의**. `.gitignore`는 추천에만 참고, 기존 층 우선순위 폐기 | 상세 문법·탐색·버전은 미결 |
| D5 | 제외 항목 탐색 | 완전 제외 내부 생략, 재포함 경로 검사, 제외 연결 파일 건너뛰기 | **2026-11-19 방향 채택** | 대상 비접근·복원 비간섭, 추적 연결 파일 거절. 구현 대기 |
| D6 | 정책 저장 | 선택적 확장 블록 / 새 형식 번호 | 선택적 확장(`approved_plan` 선례) | Storage Model §3 Snapshot record 필드 추가, §2 배치에 `start/` 추가. 구형 writer 거절 |
| D7 | `.gilignore` 자체 | 일반 보관·복원 제외 + 정책 승인 기록에 원문·적용 판 보존 | **2026-11-19 방향 채택** | 실제 blob·필드·쓰기 실패 계약은 후속. 과거 정책 자동 재적용은 미승인 |
| D8 | 적용 시점 | 승인 즉시 / **다음 세계 확정(Verify close·start)** | 다음 확정 | 새 전이 없음. Interview close의 dirty 교착 회피 |
| D9 | 최초 적용 전 재확인 | 규칙 + 실제 추적 대상 목록 | **2026-11-19 채택** | 차이 표시 후 모니터 재승인, 제외 내부·기존 파일 내용만의 변화는 제외 |
| D10 | 복원 적용 정책 | 모든 복원에 최신 승인 정책 적용 | **2026-11-19 확정** | R1/R2 후보 철회. 결과 세계 참조·과거 기록 부재 처리는 후속 |
| D11 | 전역 Git 설정 | 적용 정책 입력에서 제외 | 독립 정책 합의로 확정 | 추천과 적용 분리 |
| D12 | 일반 추천 기본 선택 | 미선택(opt-in) / **선택(opt-out)** | 선택됨, 해제 가능 | 일반·비밀값 파일 추천 기본 선택 승인, 세부 패턴·프로젝트별 기본값은 미결 |
| D13 | 루트 외 `.gilignore` | 거절 / **경고 + 일반 파일** | 경고 | 없음 |

---

## 8. 구현 순서와 단계별 완료 조건

앞 단계가 뒤 단계의 저장 형식·승인·전이를 임의로 도입하지 않는다. 각 단계는 PR 하나 이상이며
공유 파일(`session.rs`·`store.rs`·Monitor)은 소유자를 나눠 동시에 수정하지 않는다.

| 단계 | 범위 | 완료 조건 |
|---|---|---|
| S0 설계 승인 | 이 문서의 §10 결정, 규범 명세 보정(§3.4·§4·Storage §2·§3·M4-R·Tracking Policy §2.4) | 사용자 결정 기록, 통합 담당의 명세 PR 병합 |
| S1 엔진 확장 | `tracking_policy.rs`: 독립 `.gilignore` 평가와 정책 버전(미정), 루트 `.gilignore` 내장 제외·수집, 제외 항목 종류 검사 완화(D5 승인 시), 목록 관측(`inventory`, 바이트 없음), 효과 요약(폴더 묶음·효과 없는 규칙·경고), 추천 카탈로그와 금지 집합(§5.2) | 단위 시험 §9 E-군 통과. V1 ID와 V2 ID가 다름을 시험으로 고정. 기본 `observe`/`capture` 무변경 |
| S2 저장 | `tracking` 블록·Snapshot `policy`·blob 원문·load 검증(`policy_id` 재계산)·구형 writer 거절·`start/` 잔해 분류·보류 원장 읽기/쓰기 | 왕복 시험, 손상 거절 시험, 기존 format 4–7 fixture가 변경 없이 읽힘(§9 S-군) |
| S3 두 단계 시작 | `start_at_internal` 분리, 보류 원장, 제안 도구·모니터 전용 승인 경로(이름 미정, 모든 구성), 바인딩 재검증, `capture_with_policy`, `project_resume` 순서, 취소·재시작·kill point | §9 T-군 통과. 기존 `tests/artifact.rs` 시작 시험이 「2단계 승인 fixture」로 갱신되되 의미 보존 |
| S4 관측·dirty·Verify | `world_state`/`require_clean`/`close_verify`의 정책 인식, 활성화, status 3상태, 드리프트 감지, 증거 거절 Topic 연결 | §9 W-군 통과. legacy 프로젝트 동작 비트 동일(기존 시험 전부 녹색) |
| S5 재인터뷰 범위 변경 | Interview 안의 제안·승인·`.gilignore` 쓰기 순서·대체(superseded)·단일 열린 Cycle 유지 | §9 C-군 통과 |
| S6 복원·revisit 경계 | `restore::run*`의 정책 관측, 최신 승인 정책의 복원 계획·재관측·결과 세계 참조 연결 | §9 R-군 통과. `product-safety`의 일반 restore 차단 **유지** 확인 |
| S7 기존 프로젝트 채택 | 채택 흐름, legacy 표시, Windows 외부 기록 배치 | §9 L-군 통과. 자동 이관 없음 확인 |
| S8 안내·화면 | workflow.md·Manual Topic(`artifact/scope/*`)·Monitor 카드(승인 버튼·효과 요약) — 안내·Monitor 세션 결과와 정합 | 안내 계약 시험, Monitor JS 시험. 실제 Host 화면 인수는 별도 보류 항목으로 기록 |

S1·S2는 서로 독립이라 병행 가능하다. S3는 S1·S2 뒤, S4는 S3 뒤, S5·S6·S7은 S4 뒤에 병행 가능하다.

---

## 9. 자동 검사에 사용할 사례 목록

이름은 기존 관례(검사 가능한 불변식 문장)를 따른다. 실제 프로젝트·설치본 시험이 아니다.

**E — 엔진**
- `a_gilignore_rule_is_evaluated_after_every_gitignore_rule`
- `a_gilignore_negation_reincludes_what_gitignore_excluded_but_not_builtin_or_excluded_parents`
- `a_root_gilignore_is_never_part_of_the_world`
- `a_non_root_gilignore_is_an_ordinary_artifact_and_a_warning`
- `a_symlinked_or_directory_gilignore_is_refused_as_policy_text`
- `an_unparseable_gilignore_line_refuses_the_whole_policy_with_line_and_reason`
- `v2_policy_ids_differ_from_v1_ids_for_the_same_text`
- `the_recommendation_catalog_never_matches_lockfiles_or_manifests` (package.json·Cargo.lock·requirements.txt·go.sum…)
- `an_ignored_top_level_symlink_is_skipped_and_listed_not_followed` (D5 승인 시) / `…_still_refuses_under_inspect` (미승인 시)
- `a_nested_gil_inside_a_pruned_directory_is_unseen_and_the_excluded_list_says_so`
- `a_directly_matched_nested_gil_is_still_refused`
- `inventory_reads_names_and_kinds_but_no_bytes_and_stores_nothing`
- `korean_and_case_sensitive_patterns_match_as_written_on_every_os`

**S — 저장**
- `a_snapshot_record_without_policy_is_legacy_and_never_relabelled`
- `tracking_sources_are_blobs_and_policy_id_is_recomputed_on_load`
- `a_tracking_block_with_a_wrong_policy_id_is_refused_not_repaired`
- `an_old_writer_refuses_a_state_with_tracking_or_policy_fields`
- `format_4_5_6_7_fixtures_load_unchanged_without_tracking`
- `a_start_pending_ledger_is_initializable_and_anything_else_in_gil_is_not`
- `a_start_pending_ledger_beside_a_state_file_is_cleaned_as_a_remnant`

**T — 두 단계 시작**
- `start_phase_one_writes_only_under_gil_and_no_guide_block`
- `a_new_project_proposes_builtin_gitignore_and_general_recommendations_with_reasons`
- `a_project_with_an_existing_gilignore_proposes_its_text_and_refuses_invalid_lines`
- `an_agent_recommendation_changes_the_proposal_digest`
- `approval_binds_to_text_and_shown_path_sets` / `a_content_only_change_keeps_the_approval`
- `a_gilignore_edit_after_approval_drops_the_start_back_to_proposed`
- `a_new_folder_after_approval_requires_reapproval` (D9)
- `cancel_before_approval_leaves_the_working_tree_untouched`
- `phase_two_writes_guides_then_gilignore_then_captures_with_policy_then_saves_once`
- `a1_excludes_approved_paths_and_carries_policy_p1_with_channel_start`
- `dying_after_approval_resumes_the_same_approval_on_the_next_start`
- `dying_after_capture_shares_objects_and_does_not_duplicate_guides`
- `dying_after_state_save_cleans_the_pending_ledger_on_the_next_command`
- `a_bare_approved_yes_cannot_approve_scope_in_the_approved_plan_profile`

**W — 관측·dirty·Verify**
- `a_change_inside_an_excluded_path_is_not_dirty`
- `a_legacy_project_judges_dirty_exactly_as_before`
- `an_approved_pending_policy_does_not_change_the_dirty_judgement_until_verify_closes`
- `verify_close_activates_the_pending_policy_and_records_applied_from`
- `an_unexcluded_file_is_first_kept_by_the_activating_verify_and_has_no_backfilled_history`
- `an_excluded_evidence_path_is_refused_as_not_in_snapshot`
- `editing_or_deleting_gilignore_on_disk_changes_nothing_but_shows_drift`
- `restoring_an_old_gitignore_shows_drift_and_does_not_change_the_applied_policy`
- `status_distinguishes_applied_pending_and_drift`

**C — 재인터뷰 범위 변경**
- `scope_cannot_be_proposed_while_an_experiment_is_open`
- `a_dirty_tracked_file_must_be_confirmed_by_verify_before_the_interview_that_excludes_it`
- `a_second_approval_before_activation_supersedes_the_first_and_keeps_it_in_the_ledger`
- `core_writes_gilignore_in_interview_but_the_agent_may_write_nothing_else`
- `a_user_edited_gilignore_is_not_overwritten_by_approval_and_is_shown_as_drift`
- `ledger_first_then_file_a_failed_file_write_keeps_the_approval`

**R — 복원·revisit 경계**
- `restore_plans_nothing_for_excluded_paths_and_touches_none`
- `restore_uses_the_target_snapshots_policy_not_the_disk_files`
- `revisit_across_policies_is_refused_before_the_logical_move_when_a_differing_path_is_excluded_by_either`
- `revisit_across_policies_proceeds_when_every_differing_path_is_included_by_both`
- `after_a_cross_policy_revisit_the_applied_policy_is_the_targets_and_the_pending_approval_survives`
- `revisit_to_a_legacy_ancestor_treats_legacy_as_whole_tree`
- `a_currently_excluded_file_present_in_the_target_is_never_overwritten`
- `product_safety_still_refuses_general_restore_after_policy_integration`
- `preview_marks_mismatch_with_all_differences_included_as_a_separate_readiness` (제안 값)

**L — 기존 프로젝트·플랫폼**
- `an_existing_project_adopts_a_policy_only_through_an_interview_approval`
- `adoption_leaves_every_old_snapshot_legacy_and_activates_at_the_next_verify`
- `a_windows_external_record_project_keeps_gilignore_in_the_working_root_and_the_ledger_beside_state`
- `gilignore_write_refuses_links_and_reparse_points_on_windows`
- `no_installer_or_update_path_touches_user_project_policies`

---

## 10. 사용자 결정이 필요한 질문

1. **시작 방식(D1·D2): 2026-11-19 방향 채택.** 범위 승인 뒤 최초 보관과 정식 체인을 생성하며, 승인은 체인 밖의 시작 준비 기록에 남긴다. 명시적 취소는 기록 보존·취소 표시·자동 시작 방지로 확정했고 앱 종료·연결 끊김과 구분한다. 구체적인 명령·저장 형식은 후속이다. `.gil/` 삭제를 승인된 취소 절차로 간주하지 않는다.
2. **저장 형식(D6):** `tracking` 블록·`policy` 필드를 선택적 확장으로 두는가(구형 writer 거절), 새 형식
   번호를 올리는가?
3. **일반 추천 기본 선택(D12): 2026-11-19 채택.** 일반 추천은 미리 선택해 보여 주고 사용자 수정·승인 뒤 적용한다. 시스템 보호는 해제 불가다. 비밀값 파일도 미리 선택한 추천에 포함하도록 후속 승인됐다. **남은 질문:** 예시 파일을 일괄 제외하지 않을 세부 패턴과 프로젝트별 추천 기본 선택은 어떻게 둘 것인가?
4. **§3.4 변경(D5): 2026-11-19 방향 채택.** 완전 제외 폴더 내부 탐색 생략, 재포함 경로 검사, 제외 연결 파일의 대상 비접근·복원 비간섭을 따른다. 추적 대상 연결 파일은 거절한다. 구현·시험은 대기다.
5. **승인 바인딩(D9): 2026-11-19 채택.** 최초 적용 전 규칙·실제 추적 대상 목록 변화는 모니터 재승인, 완전 제외 내부·기존 파일 내용만의 변화는 범위 재승인 불필요. 보관 일관성·안전 검사 유지.
6. **`.gilignore` 자체(D7): 2026-11-19 방향 채택.** 일반 보관·복원에서 제외하고 원문·적용 판은 정책 승인 기록에 보관한다. 파일 쓰기 실패 처리는 후속 합의로 확정됐으며 상세 저장·재시도 구현은 후속이다.
7. **정책 경계 revisit(D10): 2026-11-19 확정.** 모든 복원에 최신 승인 정책을 적용한다. 새로 추적한 파일의 과거 기록 부재 처리도 후속 합의로 확정됐다. 남은 것은 결과 세계 참조·기록 근거 검사 구현이다.
8. **전역 Git 설정(D11): 방향 확정.** 프로젝트 내부·전역 Git 규칙을 적용 정책으로 읽지 않는다. 추천 참고는 적용 승인이 아니다.
9. **루트 외 `.gilignore`(D13):** 경고 + 일반 파일인가, 거절인가?
10. **기존 프로젝트 채택:** Interview 안에서만 제안·승인하는 것으로 충분한가, `gil status`/`context`가
    「추적 제외 미설정」을 상시 안내해야 하는가(안내 세션과 조율)?
11. **승인 채널(D3): 2026-11-19 채택.** 모든 구성에서 모니터 최종 승인을 받는다. 대화는 설명·수정만이며 화면 부재 시 승인 대기로 남긴다. 도구명·입력 검증·준비용 화면은 후속 구현이다.

---

## 부록 A. 기존 계약에서 변경 승인이 필요한 부분 (요약)

| 문서·코드 | 변경 종류 | 이 설계의 요구 |
|---|---|---|
| Artifact Model §3.4 | 규범 변경 | 제외 규칙에 맞는 디렉터리 `Prune`, 제외 규칙에 맞는 최상위 링크 비거절(D5) |
| Artifact Model §4 | 서술 보정 | `gil start` 앞에 범위 제안·승인 단계 추가. 한 번 저장 규칙·A1 = 실제 상태는 유지 |
| Artifact Model §3.6 2항 | 후속 합의 반영 | 시스템 보호 + 승인된 `.gilignore` 독립 평가, Git 규칙은 추천 참고만 |
| Storage Model §2·§3 | 배치·record 필드 | `.gil/start/`, `artifacts.tracking`, `snapshots[].policy` |
| Existence Model §4 | 서술 보정 | 시작 save의 선행 조건으로 범위 승인 |
| Cycle Model §6 「Interview Cycle의 Artifact」 | 한 문장 추가 | Core의 `.gilignore` 쓰기는 내장 제외 파일이므로 Artifact 변경이 아니다 |
| M4-R 복구 계약 | 규칙 일반화 | 최신 승인 정책 기준 범위·사전검사·화면 표시 |
| M3 Tracking Policy 제안 §2.4·§3 | 버전 상승 | `PolicyVersion::V2`(층), Snapshot `policy` 값 형식 |
| M3 Restore Preview 제안 §5 | 값 추가(제안) | `CandidateAcrossPolicies` |
| 로드맵 §7 M3-F 「`.gilignore` 공개 계약은 지금 만들지 않는다」 | 결정 로그 갱신 | §3.6으로 대체됨을 명시(통합 담당) |
| `src/session.rs` `initializable`·`start_at_internal`·`close_verify`·`world_state`·`restore` | 코드 | §3.2의 공백 전부 |
| `src/mcp/tools.json`·`workflow.md`·Manual Topic | 도구·안내(제안) | `gil_scope_propose`/`gil_scope_decide`, `artifact/scope/*` Topic |

## 부록 B. 이 설계가 하지 않는 주장

- 설계 검토 통과는 제품 시험 통과가 아니다. 어떤 실제 프로젝트에서도 실행하지 않았다.
- 일반 `gil restore`·Step revisit의 제품 차단은 그대로다. §3.6a의 격리·계획 승인 항목은 열려 있다.
- 복구 자동 승인 모드는 추적 범위 변경을 승인하지 않는다(M4-R 계약 그대로).
- 과거 Snapshot의 안전성·호환성을 새로 선언하지 않는다. legacy는 legacy로 남는다.

## 부록 C. 시나리오 점검표

| 시나리오 | 처리 | 절 | 검사 이름 |
|---|---|---|---|
| 새 프로젝트 | 1단계 제안(시스템 보호+독립 `.gilignore` 후보·추천) → 승인 → 2단계 정책 적용 capture → A1(policy P1) | §4.2 | T `a_new_project_proposes_…`, `a1_excludes_approved_paths_…` |
| 기존 `.gilignore`가 있는 프로젝트 | 디스크 원문을 초안으로 읽어 효과를 보여 준다. 해석 불가 줄은 줄 번호와 함께 거절. 승인 원문과 같으면 다시 쓰지 않는다 | §4.2·§5.1·§6.3 | T `a_project_with_an_existing_gilignore_…` |
| 시작 준비 취소·중단·재시작 | 명시적 취소는 기록 보존·취소 표시·자동 시작 방지. 앱 종료·연결 끊김은 재개 가능한 중단. 새 시작 의사는 현재 범위 재확인, 정식 시작 경합은 확정 상태 확인 | §4.3 | 후속 검사: 취소 보존, 자동 시작 방지, 중단 재개, 범위 재확인, 취소/시작 경합, 저장 실패 |
| 승인 직후 규칙 변경 | 승인 정책 규칙·실제 추적 목록 변경은 모니터 재승인. `.gitignore` 내용만 변경은 정책 변경 아님 | §4.3·§6.2 | 후속 검사: Git 규칙 독립성, 최초 범위 재확인 |
| 승인 직후 파일 변경 | 추적 경로 집합 변경 → 재승인(권장, Q5). 내용만 변경 → 허용. 제외 경로 변경 → 무관 | §4.3 | T `approval_binds_to_text_and_shown_path_sets`, `a_new_folder_after_approval_requires_reapproval` |
| 기존 프로젝트의 명시적 채택 | Interview에서 base=legacy로 제안·승인 → 다음 Verify close에서 활성화. 과거 Snapshot은 legacy 유지, 자동 이관 없음 | §6.6 | L `an_existing_project_adopts_…`, `adoption_leaves_every_old_snapshot_legacy_…` |
| 변경된 추적 파일을 새로 제외 | 열린 Experiment를 규칙대로 닫아야 Interview를 열 수 있고, 그 Verify가 변경을 먼저 확정한다. 정책 변경이 미확정 변경을 숨길 수 없다 | §4.5·§6.4 | C `a_dirty_tracked_file_must_be_confirmed_by_verify_before_the_interview_that_excludes_it` |
| 제외 해제와 과거 이력 보존 | 해제는 다음 Verify close부터 현재 바이트를 보관. 제외 기간 이력은 만들지 않는다. 과거 Snapshot·Report는 그대로 | §4.5·§6.4 | W `an_unexcluded_file_is_first_kept_by_the_activating_verify_and_has_no_backfilled_history` |
| 정책 파일 삭제·손상·외부 편집 | 적용·승인 정책 불변, 드리프트 표시. 「없으니 전부 추적」으로 넓히지 않음. 손상은 초안으로서 거절 | §6.3 | W `editing_or_deleting_gilignore_on_disk_changes_nothing_but_shows_drift`; C `a_user_edited_gilignore_is_not_overwritten_…` |
| 저장 도중 실패 | 원장 먼저, 파일 나중. 원장 실패 = 미승인(blob 미참조). 파일 실패 = 승인 유효·표시. 2단계 kill point별 재개 | §4.3·§6.3 | T `dying_after_*`; C `ledger_first_then_file_a_failed_file_write_keeps_the_approval` |
| 현재 제외 파일과 과거 복원 대상의 충돌 | 최신 승인 정책에서 제외된 경로는 복원에서 변경하지 않는다. 정책 판 차이 자체로 거절하지 않으며 실제 경로 충돌은 사전 검사한다 | §6.5 | R `revisit_across_policies_is_refused_before_the_logical_move_…`, `a_currently_excluded_file_present_in_the_target_is_never_overwritten` |
| 필수 증거가 제외 대상인 경우 | `evidence.not_in_snapshot` 거절 유지, 복구 Topic만 추가. 우회 없음 | §6.4 규칙 3 | W `an_excluded_evidence_path_is_refused_as_not_in_snapshot` |
| 중첩 `.gil` | 규칙에 직접 맞아도 거절. `Prune`된 디렉터리 안은 보지 않고 `excluded`에 부모를 남김 | §5.3 | E `a_directly_matched_nested_gil_is_still_refused`, `a_nested_gil_inside_a_pruned_directory_…` |
| 링크·특수 파일 | 제외되지 않은 링크는 거절 유지. 승인 정책상 제외 링크는 대상을 따라가지 않고 건너뛰며 복원에서도 변경하지 않음(D5 채택) | §5.3 | E `an_ignored_top_level_symlink_…` |
| Mac·Windows 경로 차이 | 정책은 `/` 경로 문자열만, 대소문자 구분. Windows 외부 기록 배치에서도 `.gilignore`는 작업 루트, 원장은 기록 저장소. 링크·reparse 쓰기 거절 | §5.3·§6.6 | E `korean_and_case_sensitive_patterns_…`; L `a_windows_external_record_project_…`, `gilignore_write_refuses_links_and_reparse_points_on_windows` |
