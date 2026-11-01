# Claude Wave 1 검토 — 2026-11-02

**판정: 여섯 작업 모두 제출됨. 로컬 통합은 가능하지만 병합·배포 전 수정이 필요하다.**

실제 사용자 데이터·설치본은 변경하지 않았다. 검토 브랜치 `review/claude-wave1-20261102`에만
각 커밋을 cherry-pick했다. 충돌은 없었고 main 병합·push·PR·릴리즈는 하지 않았다.

## 제출과 담당 경계

공통 base `e89a32bfd8235286621ce2c00de6737aa5038a94`. 여섯 원본 worktree는 모두 clean이었다.
새 결과 문서·코드·시험을 읽고 소유 범위를 대조했다. 공유 도메인/저장/배포 코드를 무단 변경한
커밋은 발견하지 않았다. C의 모의 Host/스크린샷은 자체 검증 산출물이다.

| 세션 | 원본 커밋 | 검토 판정 |
|---|---|---|
| A 추적 정책 | `844dc14` | 새 API/부분집합 정책 구현과 시험 확인. 이번 검토에서 추가 차단 결함은 찾지 못함. 제품 capture·저장 provenance 연결은 미완료 |
| B 복원 미리보기 | `2ae7855` | 읽기 전용·기존 파일 보존 구조 확인. 아래 경로 판정 결함 2건 수정 필요. Windows probe는 미구현이며 명시적으로 거절 |
| C Monitor | `a97c1fb` | 배너·승인 상태·재전송 UI 확인. 닫힌 Cycle의 표시 오판 수정 필요 |
| D 안내 | `a01e739` | 정본 workflow/생성 Skill 일치 확인. 재인터뷰 도중 다음 행동 및 approved-plan 시험 수정 필요 |
| E 시나리오 | `7e8a3e3` | 실제 stdio 승인 회귀 재실행 통과. 채점기 Chain 경계와 Windows 시험 경로 수정 필요 |
| F M6-E 설계 | `b966e30` | 순수 설계 모델 25개 시험 통과. Core 기능 구현 완료가 아님. D1/D2/D3 결정이 남음 |

## 재현한 결함

### R1 — B: 루트 심볼릭 링크를 검사하고도 무시한다 (P2, 제품 연결 전 필수)

- 위치: `src/restore/preview.rs:1200`.
- `probe.kind("")`에서 Unsupported만 처리한다. Symlink, Missing, Other, Io 결과는 여기서 무시한다.
  이후 조상 목록에는 루트가 없으므로 루트가 바깥 폴더를 가리키는 링크여도 자식 조회를 진행한다.
- 재현: 임시 폴더의 `project -> outside`, before는 빈 Manifest, target은 `new.txt`.
  실제 결과는 `Readiness::Candidate`와 `막힘 0`이었다.
- 필요 수정: 루트가 실제 디렉터리임을 확인하고 그 외 종류/실패를 전체 차단에 넣는다.
  상위 경로·조회 시점과 apply 시점의 차이는 별도 handle 검사 계약으로 유지한다.
- 이 코드는 아직 apply에 연결되지 않았다. 이번 재현에서 파일 덮어쓰기·삭제는 발생하지 않았다.

### R2 — B: 디스크의 Unicode 별칭을 없는 파일로 취급한다 (P2, 제품 연결 전 필수)

- 위치: `src/restore/preview.rs:1259`, `1280`.
- 이름 목록에 바이트가 같은 이름이 없으면 `probe.kind(text)`를 건너뛰고 Missing을 만든다.
  대소문자 별칭은 검사하지만 정규화 별칭은 검사하지 않는다.
- 실제 Mac 파일 시스템에서 NFD `가.txt`를 만든 뒤 NFC `가.txt`로도 접근됨을 확인했다.
  before가 빈 Manifest이고 target이 NFC 이름인 경우, 기존 파일이 있는데도 `생성 예정`,
  `막힘 0`, `Readiness::Candidate`가 나왔다.
- 필요 수정: manifest끼리뿐 아니라 디스크 이름에도 정규화 충돌을 확인한다. 확실하지 않은
  정규화는 명시적으로 미확인/거절하고 실제 파일 조회를 Missing으로 대체하지 않는다.

### R3 — C: 닫힌 Cycle도 “지금 열린 Cycle”로 표시한다 (P2)

- 위치: `mcp-app/plans.mjs:39`, `40`, `277`.
- `current_item`은 현재 Cycle의 계획 바인딩이다. `Project::current_plan_item`
  (`src/approved_plan.rs:592`)은 Cycle이 닫혀도 같은 항목을 반환한다.
- 실제 도메인 fixture에서 A의 Experiment를 완전히 닫은 뒤에도 current_item=A임을 확인했다.
  그 조회 모양을 `planItems`에 전달하면 A를 `current`로 분류한다. UI는 이를 “지금 열린 Cycle”로 번역한다.
- 필요 수정: 조회 계약에 실제 Cycle 상태를 추가해 사용하거나, 그 전까지 문구를 “현재 Cycle에
  연결된 항목”처럼 확인 가능한 사실로 제한한다. current_item만으로 열림을 추정하지 않는다.
- 이 검증은 Core 상태 + UI 함수 재현이며 실제 Codex 화면 인수는 아니다.

### R4 — D: 재인터뷰 요청 후 실행할 수 없는 다음 행동을 안내한다 (P2)

- 위치: `src/context.rs:214`.
- 승인된 Experiment의 Define에서 재인터뷰를 요청한 다음 Define을 닫으면,
  실제 허용 행동은 Hypothesis 열기이며 Cycle은 아직 닫을 수 없다.
- 하지만 첫 안내는 “현재 Cycle을 사실대로 닫는다”로 고정되고 정상 다음 Step 안내를 가린다.
  작은 모델이 이 줄을 따르면 gil_close 거절을 만나 다시 막힐 수 있다.
- 필요 수정: 재인터뷰 요청은 보존 알림으로 유지하되 현재 단계의 합법적인 다음 호출을 먼저
  안내한다. can_close인 때만 Cycle close를 권한다. 중단 계약 자체는 F의 별도 결정 대상이다.

### R5 — E: 다른 Chain의 승인으로도 승인 절차에 pass를 준다 (P2)

- 위치: `tools/scenarios/lib/score.mjs:51`.
- 검사가 “과거 어느 제안이든 approve receipt가 있었는가”만 본다. Experiment가 속한 Chain,
  활성 판, 실제 승인된 항목과의 연결을 검사하지 않는다.
- 이전 Chain에서 승인한 로그 뒤 새 Chain의 무승인 Experiment를 넣은 음성 fixture에서
  `approval_before_execution: pass`가 나왔다.
- 필요 수정: 승인 사실에 scope/Chain/판/항목과 유효 구간을 연결해 해당 실행의 근거만 인정한다.
  연결 관측이 없다면 unknown을 사용한다. 승인되지 않은 실행의 음성 fixture를 추가한다.
- **평가기의 결함이다. 실제 Core의 승인 우회를 발견했다는 뜻은 아니다.** Core wire 회귀는 통과했다.

## 시험 및 통합 차단 사항

1. **D 신규 시험 6개가 approved-plan 프로필에서 실패한다.**
   `tests/guidance_wave1.rs`가 `Project::start` 또는 CLI start로 새 승인 계획 프로젝트를 만들고
   기존 `approved: yes` fixture로 Synthesis를 닫는다. 실제 계약은 이를 거절한다.
   legacy 시험에는 명시적인 legacy fixture를 쓰고 승인 프로필에는 propose→decide→close 흐름을
   넣어야 한다. 실패 검사를 단순 제거하거나 모든 프로필에서 skip하는 해결은 부적절하다.
2. **전체 approved-plan lib 시험도 12개 실패한다.** 도구 목록 예상값에 승인 도구가 빠진 1건,
   visual migration 시험의 구형 승인 fixture 11건이다. 해당 시험/공통 helper는 이번 여섯 lane이
   변경하지 않은 파일이다. 기반 M6-D 계약을 포함해 정비해야 한다. 기반 커밋에서 동일 명령을
   별도로 재실행하지는 않았으므로 본 검토의 수치는 통합 후보에 대한 것이다.
3. **JS 지침 계약 검사 1개 실패.** 생성 Skill은 승인 계획 도구를 설명하지만 `skill.test.mjs`가
   기본 tools.json만 대조한다. feature별 실제 catalog와 조건부 안내를 함께 검증해야 한다.
   `gil_plan_*` 접두어 전체를 무조건 허용하는 예외로 검사를 약화하지 않는다.
4. **고지/패키징 2개 실패.** C의 build.mjs와 생성 monitor.html 해시가 notice-policy와 다르다.
   의존성 변경은 없지만 통합 소유자가 입력·고지 범위를 검토한 뒤 해시를 갱신해야 한다.
5. **E의 Windows 시험 경로 미지원.** `tests/approved_plan_wire_wave1.rs:19`와 wire runner는
   프로젝트 안 `.gil/state.yaml`을 가정한다. Windows 외부 private storage 경로와 시험 환경 변수를
   기존 Windows helper에 맞춰야 한다. 이번에 Windows 실행은 하지 않았다.
6. **공유 빌드 캐시 오염은 준비 도구의 결함이다.** 원래 `cargo-queue.py`의 잠금은 동시 쓰기는
   막지만 worktree 전환 뒤 stale `target/debug/gil` 재사용까지 막지는 못한다. B/C가 보고했다.
   이번 검증은 처음부터 별도 target을 써서 그 영향을 제거했다. 앞으로는 worktree별 target 또는
   신뢰할 수 있는 artifact 격리를 적용해야 한다. 단순히 큐 안에서 cargo를 호출했다는 이유로
   현재 branch 바이너리임을 보증하는 기존 README 문구는 더 이상 근거로 쓰지 않는다.

## 이번에 직접 실행한 검증

통합 코드: `90cf763` (여섯 원본 커밋의 검토용 cherry-pick 결과).
Core는 다른 세션이 쓰지 않는 새 target, incremental 비활성, debug info 0, build jobs 2로 빌드했다.
Node 의존성은 각 lockfile로 offline npm ci 했다. 고지 검사를 우회하지 않았다.

| 검사 | 결과 |
|---|---|
| 기본 `cargo test --locked -p gil` | **1,505 pass / 0 fail / 2 ignored** |
| 기본 `cargo build --locked -p gil --all-targets` | 통과 |
| 승인 계획 프로필 `cargo build --locked -p gil --all-targets` | 통과. 두 빌드 모두 기존 launcher dead-code 경고 11개 |
| 최초 sandbox 안 기본 시험 | lib 598 pass / 54 fail / 1 ignored. 로컬 소켓·루프백의 Operation not permitted 확인 후 sandbox 밖에서 위와 같이 재실행 |
| 전체 `approved-plan,visual-authoring,product-safety` 시험 | lib **663 pass / 12 fail / 2 ignored**에서 중단. 뒤 integration suite를 전체 실행했다고 세지 않음 |
| 같은 프로필 `--test guidance_wave1 --test approved_plan_wire_wave1` | wire **1 pass**; guidance **1 pass / 6 fail** |
| UI/Plugin + E JS + F 설계 시험 | **186 pass / 1 fail / 4 skipped**. F의 25개와 E의 21개는 모두 통과 |
| distribution/codex + compliance | **94 pass / 2 fail / 1 skipped** |
| 실제 native stdio wire runner W01–W21 | **20 pass / 1 expected_blocked / 0 fail** (합성 App 승인, 실제 사람 클릭 아님) |
| 정본 workflow와 생성 Skill 일치 검사 | 통과 |
| 추가 Rust 재현 시험 | 정상 기대값을 둔 **3개 실패**로 R1/R2/R4 확인. 닫힌 Cycle 바인딩 관측 1개 통과(R3 근거) |
| 추가 JS 음성 fixture | R3는 current로 표시, R5는 pass로 오판함을 확인 |

신규 재현의 최초 컴파일은 비공개 ContentDigest::of 호출로 실패했다. 시험용 digest 생성 API로
고친 뒤 위 3개 실제 실패를 확인했다. 이를 제품 컴파일 오류로 세지 않았다.

`regressions.rs`는 검토 재현 자료이며 기본 Cargo 시험에 연결하지 않았다. 검토 중 lib.rs에 임시
test module로 연결해 실행한 뒤 원상 복구했다. `probes.mjs`는 이 폴더에서 읽는 E fixture와 UI
함수만 사용하며 `node coordination/parallel-wave-1/review/probes.mjs`로 재현한다.
Rust 재현은 별도 검토 worktree에서 아래 선언을 lib.rs에 임시 추가하고 전용 target으로 실행한다.

```rust
#[cfg(all(test, unix))]
#[path = "../coordination/parallel-wave-1/review/regressions.rs"]
mod wave1_review_regressions;
```

```sh
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib wave1_review_regressions -- --nocapture
```

실제 Codex 표시·물리적 승인 클릭·Luna 작업·Windows native·Tauri 설치/업데이트는 이번 검증에 포함하지 않았다.

## 다음 합류 순서

1. R1–R5와 시험/캐시 문제를 회귀시험과 함께 수정한다.
2. A/B의 정책 provenance·capture·미리보기 표면 연결을 구현한다. 기존 스냅샷에 정책을 지어 붙이지
   않고 restore 차단을 유지한다. F의 세 결정을 확정한 뒤 M6-E는 별도 구현한다.
3. 양 OS CI·전체 승인 프로필 검사·고지 검사를 통과시킨다.
4. 그 다음 한 테스트 설치본으로 실제 Monitor 승인·인터뷰 종료·항목별 Verify 증거·재개·종료를
   Luna에서 확인하고 디버깅한다. 현재 결과를 그대로 릴리즈하지 않는다.
