# B — 복원 미리보기·경로 안전성 결과

branch: `claude/wave1-restore-preview` (base `e89a32b`, 공통 코드 기준 `fcd6e5d` 포함 확인)
작성: 2026-11-02. 로컬 commit SHA는 세션 보고에 적는다(이 파일은 그 commit 안에 있다).

이 결과는 **소스 완료**다. 제품 반영·병합·push·PR·CI dispatch는 하지 않았다. 제품 `gil restore`/
`gil revisit`의 `product-safety` 차단은 그대로이며, 이 엔진이 서도 안전 restore 전체가 끝난 것이 아니다.

## 1. 구현 범위

파일을 바꾸지 않는 복원 미리보기 엔진과 경로/충돌 판정. `restore/plan.rs::steps`를 부르지 않고
별도 타입(`Preview`/`Entry`/`Disposition`/`Risk`)으로 세웠다. `Plan`/`Step`으로 가는 변환이 없다.

- **입력**: before/target `Manifest`, before/target 정책 식별자 `Option<&str>`, `TargetOs`,
  `impl Fn(&str) -> Result<bool, String>`(INTERFACES의 A→B 모양 그대로, A 타입 import 없음),
  읽기 전용 `Probe`.
- **역할**: `Unchanged` / `WouldCreate` / `WouldReplace` / `CurrentOnly { QuarantineProposed }` /
  `Excluded { GitAlways | Policy, side }`. 현재에만 있는 파일은 **삭제 대상이 아니라 보존 필요**다.
  격리는 제안이며 이동 코드는 없다. `.git`은 callback에 묻지 않고 언제나 제외한다.
- **정책**: `Matched` / `Unknown`(legacy) / `Mismatch`. 미상·불일치면 정보성 diff는 내되
  `Readiness::Candidate`로 판정하지 않는다. 같은 정책인데 목표가 제외 경로를 담으면 `PolicyInconsistent`.
- **경로 글자 검사** `inspect_text`: NUL, `\`, 빈 조각(절대/`//`/UNC·device 접두), `.`/`..`,
  255 B 초과, `X:` 드라이브, `:`(ADS), `< > " | ? *`, 제어 문자, 끝 `.`/공백, 예약 이름
  (`CON PRN AUX NUL COM1–9 LPT1–9`, `¹²³` 포함, 확장자·대소문자 무시), UTF-16 255 단위 초과,
  결합 문자. 무게는 `Risk::severity(os)`가 정한다 — Windows 전용 위험은 macOS/Unix 규칙에서
  `ElsewhereOnly(Windows)`(정보성, legacy 표식), Windows 규칙에서 `Blocks`.
- **겹침**(결과 세계 = 목표 ∪ 현재): 대소문자(`to_lowercase` 근사), Unicode 정규화(한글 음절↔자모
  알고리즘; 라틴 결합 문자는 `UnverifiedNormalization`으로 **미확인**), 파일/디렉터리 topology.
  접는 OS에서만 막히는 것은 `ElsewhereOnly`로 구분한다.
- **디스크 조회** `Probe`(읽기 전용 trait): `NoProbe`(명시적 미조회 → 거절), `LstatProbe`
  (Unix `lstat`/`readdir`; 비Unix는 `Unsupported`로 **명시적 거절**, 흉내 내지 않음).
  조상 symlink/비디렉터리, 잎 symlink/비파일, `Stale`(관측 뒤 변동), `CaseVariantOnDisk`.
  근거는 `Basis::Manifest` / `Basis::FileSystem`으로 나눠 적고, 조회 시점 ≠ 적용 시점임을 출력에 적는다.
- **무효화**: `fingerprint()`(입력 전체 지문), `still_matches(&Manifest)`(재관측 비교).
- **출력** `Display`: 근거·무게·OS 표식을 담고 복원 명령·내부 digest·「삭제」를 적지 않는다.

## 2. 변경 파일

| 파일 | 변경 |
|---|---|
| `src/restore/preview.rs` | 신규 — 엔진 |
| `src/restore/preview/tests.rs` | 신규 — unit tests 21개 |
| `src/restore/mod.rs` | `pub(crate) mod preview;` 선언 두 줄(주석 포함)만 |
| `spec/proposals/M3_Restore_Preview_v0.1.md` | 신규 — 제안 |
| `coordination/parallel-wave-1/results/B-RESTORE-PREVIEW-RESULT.md` | 이 파일 |

`plan.rs`/apply/rollback/cleanup/`destination.rs`/`guard.rs`, `artifact/**`, `windows_fs.rs`,
Cargo 파일, 공유 transaction/저장/Session 파일은 읽기만 했다. 의존성 추가 없음.

## 3. 검증 명령과 결과

모든 Rust 명령은 `python3 coordination/parallel-wave-1/cargo-queue.py -- …`로 실행했다(공유
target `target/claude-wave-1`, 합성 TMPDIR). macOS arm64, Rust 1.97.1, Node 26.5.0.

| 명령 | 결과 |
|---|---|
| `cargo test --locked -p gil --lib restore::preview` | **21 passed, 0 failed** (첫 실행은 byte-string 리터럴 컴파일 오류, 둘째는 아래 §5-1 결함으로 1 실패 → 수정 후 통과) |
| `cargo test --locked -p gil` | lib **600 passed, 0 failed, 1 ignored**(기존 ignore) + 통합 시험 전부 통과, 단 `tests/mcp.rs` **2 failed**(`initialize_succeeds_and_says_who_is_answering`, `the_app_resource_is_embedded_and_binds_both_metadata_keys`). 원인은 §5-5의 공유 target stale 바이너리이며 B 변경과 무관. 같은 큐 안에서 `touch src/main.rs && cargo test --locked -p gil --test mcp`로 이 branch의 `gil`을 다시 빌드하자 **25 passed, 0 failed**. 첫 실행의 실패를 숨기지 않고 둘 다 적는다 |
| `cargo build --locked -p gil --all-targets` | **성공**(exit 0). 경고 11개는 모두 `gil-launcher` bin의 기존 dead-code 경고로 B 변경과 무관 |
| `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` (큐 밖) | 132 tests: **128 pass, 0 fail, 4 skipped** — skip은 Plugin에 Core가 실리지 않은 환경 조건(`make-core.sh` 미실행), B 변경과 무관. 통과로 세지 않음 |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` (큐 밖) | 97 tests: **96 pass, 0 fail, 1 skipped** — `GIL_GITLESS_ARCHIVE` 미설정 조건. 통과로 세지 않음 |
| `git diff --check` | whitespace 문제 없음 |

JS 의존성은 이 worktree의 lockfile로 `npm ci`했다(mcp-app, plugins/gil-companion-prototype).

**실행하지 않은 것**: 실제 Mac/Windows 설치본, Codex/Claude Desktop 화면, Luna 사람 참여 시험,
Windows 빌드/시험(`#[cfg(not(unix))]` 가지의 `on_this_os_the_lstat_probe_refuses_instead_of_pretending`
은 이 Mac에서 컴파일되지 않았다), Tauri/Companion 검사.

## 4. 합격 기준 대응

| 기준 | 시험 |
|---|---|
| 호출 전후 프로젝트 파일·state.yaml·객체 저장소 byte-identical | `a_probed_preview_changes_nothing_on_disk_byte_for_byte` (`.gil/state.yaml`, `.gil/objects/…`, `.git/HEAD` 포함 전체 폴더 비교, Unix) + `a_file_that_appeared_after_observation_makes_the_preview_stale`, `symlinks_and_case_variants_on_disk_are_seen_not_followed`(바깥 폴더까지 비교) |
| 현재에만 있는 파일 보존 | `a_current_only_file_is_preserved_never_deleted` (출력에 「삭제」·`Delete`·`backup`·명령 없음) |
| `.git` 제외 | `git_is_always_excluded_without_asking_the_policy` (`.gitignore`는 정책에 묻는다) |
| callback 오류 | `a_policy_error_refuses_that_path_and_the_whole_candidate` |
| 다른 정책 ID | `different_or_unknown_policy_ids_still_give_a_diff_but_never_a_candidate` (불일치·미상 3조합, 출력에 legacy 표시) |
| unsafe path | `unsafe_text_is_caught_even_where_entry_path_already_refuses_it`, `a_path_that_was_fine_on_macos_but_impossible_on_windows_is_judged_separately` |
| case/Unicode collision | `case_collisions_block_only_where_the_file_system_folds_case`, `hangul_syllables_and_jamo_are_the_same_name_on_macos`, `latin_combining_marks_are_unverified_not_silently_passed`, `a_file_and_a_directory_cannot_share_a_name` |
| 같은 입력 → 안정된 차례 | `the_same_input_gives_the_same_preview_in_ascending_path_order` (UTF-8 바이트 오름차순, 지문 동일/상이) |
| 기존 destructive Plan과 타입·진입점 혼동 불가 | `a_preview_carries_no_backup_ordinal_no_delete_and_no_world_number` (같은 입력에 `plan::steps`는 Delete, 미리보기는 CurrentOnly; Debug에 Delete/backup/world 없음) |
| 제품 차단 유지 | `src/command.rs`의 `product-safety` 가지를 건드리지 않았다. 미리보기는 어떤 명령/도구에도 연결되지 않았다 |

## 5. 발견한 결함·주의

1. **(B 자체, 수정함)** 대소문자를 접는 파일 시스템(이 Mac의 APFS)에서 `lstat("dup.txt")`가
   `Dup.txt`에 닿아 처음 구현은 `Stale`과 `CaseVariantOnDisk`를 둘 다 냈다. 폴더 이름 목록을 먼저
   보고 글자 그대로의 이름이 없으면 잎을 `Missing`으로 보도록 고쳤다. 재현: 시험
   `symlinks_and_case_variants_on_disk_are_seen_not_followed`.
2. **(범위 밖, 보고만)** `artifact/mod.rs::is_usable`은 `C:`·`CON`·`a:b` 같은 조각을 허용하므로
   기존 `observe`는 macOS에서 그런 이름을 Manifest에 담을 수 있다. 미리보기는 이를 Windows 규칙에서
   거절하지만 관측 자체는 A/통합 소유라 바꾸지 않았다.
3. **(한계)** `str::to_lowercase`는 NTFS upcase 표/APFS 접기의 근사다. 겹침을 「찾는」 데만 쓰고
   「없다」를 증명하지 않는다. 라틴 결합 문자의 NFC/NFD 동등성은 표/의존성 없이 판정하지 않고
   `Unverified`로 남겼다(Cargo는 A/통합 소유).
4. **(한계)** `LstatProbe`는 경로 이름으로 보는 조회라 TOCTOU를 막지 않는다. 결과에 「조회 시점의
   사실」로 적히고, 적용 직전 재관측(`still_matches`)과 handle 기준 검사는 통합 단계의 일이다.
   Windows handle 기반 읽기 전용 조회는 구현하지 않았고 비Unix에서는 명시적으로 `Unsupported`다.

5. **(공유 큐, 모든 세션에 영향)** 공유 `target/claude-wave-1/debug/gil`은 **마지막으로 bin unit을
   다시 빌드한 worktree의 것**이다. worktree 경로가 다르면 cargo unit hash가 달라 각 branch의
   `deps/gil-<hash>`는 따로 있지만, 이 branch의 unit이 fresh면 uplift된 `debug/gil`을 갱신하지 않는다.
   그래서 `tests/mcp.rs`가 `CARGO_BIN_EXE_gil`로 띄운 바이너리에는 D branch의 `workflow.md`와
   C branch의 `monitor.html`이 들어 있었다(served 문구 "Each receipt's text guidance names…"는
   이 worktree에 없고 `claude-wave1-D/src/mcp/workflow.md`에만 있음; `monitor.html`은 `claude-wave1-C`만
   다름). README의 「`cargo test`는 같은 잠금 안에서 자체 빌드하므로 괜찮다」는 가정이 이 경우에
   깨진다. 재현: 다른 worktree가 `gil` bin을 빌드한 뒤 이 worktree에서 큐로 `cargo test --locked -p gil`.
   우회: 같은 큐 명령 안에서 `touch src/main.rs`(내용 변경 없음) 뒤 실행. README/큐 스크립트는
   공통 지침이라 수정하지 않았다 — 통합 담당자가 큐에 bin 강제 재빌드를 넣거나 worktree별
   `--target-dir` 하위 폴더를 쓰는 것을 제안한다.

## 6. 남은 연결 (통합 담당자)

제안 문서 §7과 같다. 요약:

1. A의 정책 연결: `preview(&Request { before_policy: Some(&before_id), target_policy: Some(&target_id), .. },
   |p| policy.includes_file(p), &probe)`. 과거 Snapshot의 정책 ID는 저장 provenance가 서야 생긴다.
   그때까지 기존 Snapshot은 `None` → `PolicyUnknown`(legacy)으로 거절된다 — 의도한 동작이다.
2. `QuarantineProposed`의 실제 격리 자리·receipt·복구 — 저장 transaction과 함께 결정.
3. `windows_fs.rs` handle 열기로 `Probe` 구현(Windows 읽기 전용 조회).
4. Unicode 정규화 표 또는 의존성 결정.
5. 미리보기를 보여 주는 표면(`gil status`/Monitor/MCP)과 사람 승인 wire, 승인 기록에 `fingerprint`
   와 before digest를 함께 두고 apply 직전 `observe()` → `still_matches()`가 거짓이면 승인 무효.
6. `#![allow(dead_code)]`는 통합이 호출 자리를 만들 때 지운다.

## 7. 막힌 조건

없음. 요청 범위는 전부 소스로 완료했고 위 §3의 미실행 항목은 분업 지침대로 통합 후 실기 단계다.
