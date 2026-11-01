# A — 복원 미리보기 안전 판정 수정 결과

branch: `claude/wave2-restore-safety` (base `coordination/parallel-wave-2-base`, `197a402` 조상 확인).
작업일: 2026-11-02. 수정 범위는 소유 목록 안(`src/restore/preview.rs`, `src/restore/preview/tests.rs`,
`spec/proposals/M3_Restore_Preview_v0.1.md`, 이 문서). Cargo.toml/Cargo.lock은 바꾸지 않았다 — 의존성 추가 없음,
라이선스/고지 변경 없음.

## 수정

### R1 — 뿌리 종류 조회를 무시하던 경로
- 종전: `probe.kind("")`에서 `Unsupported`만 처리하고 `Symlink`/`Missing`/`File`/`Other`/`Io`는 버렸다. 뿌리가 바깥을
  가리키는 링크여도 자식 조회로 넘어가 `Candidate`가 났다.
- 지금: 뿌리가 `Directory`로 확인되지 않으면 `ProbeStatus::RootNotDirectory { adapter, found }` 또는
  `RootUnreadable { adapter, why }`로 끝내고 **자식 자리를 하나도 조회하지 않는다**. `Readiness`에
  `Block::RootNotDirectory { found }` / `Block::RootUnreadable { why }`가 들어가며 Manifest가 비어 있어도 거절한다.
  경로별 정보성 diff(`Disposition`)는 그대로 보인다. `Unsupported`는 종전대로 `FileSystemNotProbed`.
- 추가: `..`·빈 조각·`\`·NUL을 담은 경로는 글자 검사가 이미 모든 OS에서 `Blocks`이므로 디스크를 묻지 않는다
  (`escapes_root`). 뿌리에 이어 붙여 조회하면 밖에 닿을 수 있어서다. 판정은 느슨해지지 않는다(시험으로 확인).

### R2 — 목록 부재를 즉시 Missing으로 대체하던 경로
- 종전: 폴더 목록에 글자 그대로의 이름이 없으면 `lstat`을 생략하고 `Missing`으로 적었다. NFD로 적힌 `가.txt`가
  있어도 NFC target은 `생성 예정 · 막힘 0 · Candidate`였다.
- 지금: 목록에서 대소문자 변형(`CaseVariantOnDisk`, 기존)과 **한글 음절↔자모 정규화 변형**
  (`NormalizationVariantOnDisk`, 신규; 정규화를 접는 OS에서 Blocks, 그 밖 `ElsewhereOnly(macOS)`)을 찾고,
  목록 유무와 무관하게 **항상** `lstat`을 한다. 목록에 없는데 `lstat`이 닿았고 변형도 못 찾았으면
  `UnidentifiedAliasOnDisk { found }`(모든 OS Blocks, 「미확인 별칭」)로 거절한다. 검증하지 못한 동등성을
  통과로 치지 않는다. 라틴 결합 문자의 `UnverifiedNormalization`(미확인) 계약은 그대로다.
- 정규화 표를 추가하지 않았다. 한글 알고리즘 외의 정규화를 「지원」이라 부르지 않는다(모듈 머리말·명세 §4.3).

### 설명(작업 3)
- 모듈 머리말에 뿌리 → 조상 → 잎 차례와 읽기 시점 한계를 적었다. Windows handle/TOCTOU 안전성·apply 허용이
  아님을 명시했다. 제품 restore/revisit 차단과 apply 코드는 손대지 않았다.
- 명세 §4.2 표에 두 finding, §4.3 조회 차례, §5 Readiness의 두 Block, §8 시험 범위를 추가했다.

## 자동시험 (모두 Wave 2 큐 `cargo-queue.py` 사용, target hash `53a9aa4a16a845b7653d0a42`)

| 명령 | 결과 |
|---|---|
| `cargo test --locked -p gil --lib restore::preview` | 27 passed (기존 20 + 신규 7) |
| `cargo test --locked -p gil` (기본) | 1차: 657 passed / 1 failed — `runtime_launch::tests::damaged_blocked_and_unverifiable_targets_run_nothing_and_never_fall_back` (`SelectRefusal { stage: Lock, … }`, 선택 잠금 경합). 2차 `--lib` 전체 재실행: 658 passed / 0 failed / 1 ignored. 단독 재실행도 통과. 제 범위 밖 간헐 실패로 기록 |
| `cargo build --locked -p gil --all-targets` (기본) | 성공. `src/bin/gil-launcher.rs`의 dead_code warning은 baseline 그대로 |
| `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib restore::preview` | 27 passed |
| `cargo build --locked -p gil --features approved-plan,visual-authoring,product-safety --all-targets` | 성공 |
| `git diff --check` | 통과 |

신규 시험(`src/restore/preview/tests.rs` 끝):
- `a_root_that_is_not_a_verified_directory_refuses_the_whole_preview_and_asks_nothing_below` — 대본 adapter.
  Symlink/Missing/File/Other × (빈·빈, 빈·있음, 있음·있음) Manifest, Io 실패. 조회 기록이 `kind ""` 하나뿐임을 확인.
- `a_leaf_that_lstat_reaches_under_an_unlisted_name_is_an_unidentified_alias_not_missing` — 모르는 접기(라틴 결합)를
  미확인 별칭으로 거절, 잎 조회 생략 없음.
- `hangul_normalization_variants_in_a_listing_are_identified_and_weighed_per_os` — macOS Blocks, Unix/Windows 정보성.
- `paths_that_could_escape_the_root_are_refused_before_any_disk_lookup` — `escapes_root` 집합과 글자 검사 Blocks 일치.
- `review_r1_a_symlink_root_is_refused_and_the_outside_folder_is_not_read` (`cfg(unix)`) — 검토 재현 그대로 +
  없음/파일 뿌리, 전후 bytes 동일.
- `review_r2_an_existing_nfd_file_blocks_an_nfc_target_on_a_normalization_insensitive_volume`
  (`cfg(target_os = "macos")`) — 실제 볼륨에 NFD 파일 생성, NFC로 닿음 확인, `NormalizationVariantOnDisk`·`Refused`·
  `blocked 1`, 미추적 파일 내용과 폴더 bytes 전후 동일.
- 기존 보존·거절 계약 시험(바이트 동일성, 현재에만 있음 보존, 대소문자 충돌, stale, symlink, Plan 분리)은 수정 없이 통과.

원래 실패 재현: Wave 1 `regressions.rs`의 두 시험을 수정 전 코드에 돌리면 `Candidate`가 났다(검토 보고서 기록).
수정 후 같은 조건의 정식 시험이 `Refused`를 준다.

## 미검증 / 환경 한계
- Windows·비Unix에서는 `LstatProbe`가 종전대로 `Unsupported` → `FileSystemNotProbed`. 이번 수정은 Windows handle
  조회를 추가하지 않았고, 합성 시험으로 Windows 통과를 주장하지 않는다. R2 재현은 macOS 전용이며 정규화를 접는
  볼륨(TMPDIR 이 APFS/HFS+)을 전제한다 — 다른 볼륨이면 전제 assert에서 멈춘다.
- JS/Plugin/고지 시험은 이번 수정이 건드린 파일과 무관해 실행하지 않았다. baseline의 고지 2개 실패는 통합 대기.
- 실제 설치본·Monitor 표면·제품 `gil restore`/`revisit` 연결은 없다(차단 유지).
- 디스크 가용 공간: 시작 14 GiB → 종료 약 10 GiB(큐 target 빌드 포함). 다른 세션 cache는 지우지 않았다.

## 남은 연결 (통합 담당자)
1. 미리보기를 Session/명령/Monitor에 연결할 때 `ProbeStatus::RootNotDirectory`/`RootUnreadable`과
   `Block::RootNotDirectory`/`RootUnreadable`를 표면에 노출한다. `Readiness::Refused`만 보면 이미 포함된다.
2. Windows 읽기 전용 `Probe` 구현(§7.3) 때 뿌리 판정도 같은 계약(`Directory` 아니면 전체 거절, 자식 미조회)을 따른다.
3. 라틴 결합 문자 등 전체 정규화 동등성은 여전히 미확인 거절이다. 정규화 표/의존성 추가(§7.4)는 별도 결정.
4. 고지 inventory hash 갱신 불필요(의존성 변경 없음).
