# GIL Windows 이미지 Report 지원 계약 v0.1 (단계 1)

작성일 2026-10-18. 이 문서는 Windows에서 이미지 Report **작성**을 지원하기 위한 Core 경계와
인수 기준을 정한다. 작성 계약(`GIL_Visual_Report_Authoring_Contract_v0.1.md`) §6 항목 9
"안전한 첨부 구현이 없는 플랫폼은 명시적으로 거절. 빌드 성공을 Windows 쓰기 검수로 세지
않음"과 저장 계약(`GIL_Visual_Report_Storage_Contract_v0.1.md`) §13–§24의 Unix 경계,
이관 명세 §5.6의 capability 규칙, 배포 명세 §6.2b의 Windows 범위를 전제로 한다.
이 문서는 Unix 경계를 바꾸지 않으며, 기존 format-4 Windows 동작과 저장 형식·도구 schema·
도구 이름도 바꾸지 않는다.

## 1. 범위

단계 1이 지원하는 Windows 흐름은 넷이다. 각각은 macOS Apple Silicon에서 이미 검수된 같은
Core 경로를 쓰고, 플랫폼 원시 연산만 Windows 동등 구현으로 바꾼다.

| 흐름 | Core 진입 | 단계 1 결과 |
| --- | --- | --- |
| W1 새 format-5 Project 생성 | `ProjectSession::start_visual` → `visual_start::create` | 지원 |
| W2 Verify에 새 이미지 첨부 | `close_verify_with_images_and_commit` → `visual_report::file` | 지원 |
| W3 저장된 이미지를 다른 Report에서 인용 | `close_references_and_commit` + `read_stored` | 지원 |
| W4 프로세스 재시작 뒤 보고서·이미지 보존 | `ProjectSession::open` / `open_read_only` → `store::load` | 지원 |

범위 밖(이 문서로 바뀌지 않음):

- **format-4 → 5 이관은 Windows에서 계속 거절한다.** `gil_migration_*` 도구는
  `unsupported_platform`으로 답하고, `visual_migration` 모듈은 Unix 전용으로 남는다.
  사용자는 Windows에서 새 format-5 Project를 만들 수 있을 뿐 기존 기록을 이관할 수 없다.
- 복구(`restore`)·rollback·revisit의 Windows 핸들 기반 경계(저장 계약 §21–§24의 Windows 항목).
- `.gil/artifacts/tmp` 정리(`reap_tmp`)의 핸들 기반 열거. Windows는 기존 경로 기반 구현
  (`symlink_metadata`로 일반 파일만, GIL 이름만 삭제)을 유지한다.
- Monitor binding 저장(이미 Windows 검수됨, `bindings_windows.rs`), Host 화면 인수, 설치,
  공개 게시, 버전 표기 정책, MSVC runtime 재배포 검토.
- 같은 사용자 권한의 악의적 동시 writer가 모든 검사 직후에 쓰는 경합의 완전한 제거.
  Unix 경계와 같은 한계다(저장 계약 §18).

## 2. 기존 경계와 Windows 동등 구현

| Unix 경계 (현행) | 이번 Windows 동등 구현 |
| --- | --- |
| `openat` + `O_NOFOLLOW` 로 구성 요소마다 열기 | 조상부터 한 단계씩 `CreateFileW(FILE_FLAG_BACKUP_SEMANTICS \| FILE_FLAG_OPEN_REPARSE_POINT)` 로 열고 핸들의 `dwFileAttributes` 에 `FILE_ATTRIBUTE_REPARSE_POINT` 가 있으면 거절(symlink·junction·mount point·기타 reparse tag 전부). 조상 핸들은 작업이 끝날 때까지 보유해 이름 변경·삭제를 OS 공유 규칙으로 막는다. |
| `nlink() == 1` | `BY_HANDLE_FILE_INFORMATION.nNumberOfLinks == 1` |
| `dev()/ino()` identity | `FileIdInfo` 의 `VolumeSerialNumber` + 128-bit `FileId` (NTFS·ReFS 모두 유효) |
| `mtime/ctime` 안정성 비교 | `FileBasicInfo` 의 `LastWriteTime`·`ChangeTime`·`CreationTime` + 크기 + 링크 수 + 속성 |
| `O_CREAT \| O_EXCL` 임시 파일 | `CREATE_NEW` (이미 있는 일반 파일·링크·디렉터리 모두 실패) |
| `fsync` | `FlushFileBuffers` (파일 핸들). 디렉터리는 `GENERIC_WRITE` 로 다시 열어 `FlushFileBuffers`. |
| `renameat` / `renameatx_np(RENAME_EXCL)` | `SetFileInformationByHandle(FileRenameInfoEx)` + `RootDirectory` = 보유한 `.gil` 디렉터리 핸들, `FILE_RENAME_FLAG_POSIX_SEMANTICS`; 배타 생성은 `REPLACE_IF_EXISTS` 없이(존재하면 실패), 통상 저장은 `REPLACE_IF_EXISTS` 로 **항목을** 교체(목적지가 링크여도 가리키는 대상은 건드리지 않음) |
| `linkat` (no-clobber 객체 확정) | `CreateHardLinkW` (목적지가 있으면 어떤 종류든 실패) |
| `unlinkat` (자기 임시 항목만) | 보유한 임시 파일 핸들에 `FileDispositionInfoEx(DELETE \| POSIX_SEMANTICS)` — 이름을 다시 풀지 않으므로 바꿔치기된 항목을 지우지 않음 |
| `MNT_LOCAL` (생성 시 로컬 파일 시스템) | `GetDriveTypeW == DRIVE_FIXED` 이고 `GetVolumeInformationByHandleW` 가 NTFS (배포 명세 §6.2b 범위) |
| `0o700` 디렉터리 | 상위 폴더 ACL 상속(기존 Windows format-4 `.gil` 과 동일). 보호 DACL 적용 여부는 §8 미해결 항목 |

## 3. 위협 목록과 대응

1. **reparse point** (symlink, junction, volume mount point, cloud placeholder 등 모든 tag):
   어느 구성 요소든 핸들 속성으로 거절한다. 경로 문자열로 판정하지 않는다.
2. **hardlink**: 첨부 원본·임시 파일·상태 파일·잠금 파일·저장 객체는 링크 수 1만 허용한다.
   저장 객체 확정 중 동시 확정자가 잠시 두 번째 링크를 남기는 정상 경합은 Unix와 같이 허용한다.
3. **alternate data stream(ADS)**: 상대 경로 규칙이 `:` 를 거절하므로 스트림 이름을 지정할 수
   없다. 내부 객체 이름은 검증된 digest에서만 만들어진다. 스트림 추가는 `ChangeTime` 을 바꾸므로
   첨부 준비 중 변경으로 감지된다.
4. **경로 교체 경쟁**: Project root 와 `.gil` 핸들을 보유하는 동안 그 디렉터리는 이름 변경·삭제
   되지 않는다. 그래도 각 단계 뒤 이름으로 다시 열어 identity 를 비교하고, 다르면 거절한다.
5. **중단·디스크 고갈**: Unix 와 같은 checkpoint 이름(`reserved`, `captured`, `encoded`,
   `partial_write`, `file_synced`, `before_publish`, `after_publish`, `directory_synced`,
   `completed`)을 쓴다. 공개(rename) 전 실패는 `state.yaml` 이 없는 상태를 남기고, 공개 뒤
   실패는 완전한 format-5 상태를 남긴다. 어느 경우든 실패한 임시 파일·잔여물은 삭제하지 않고
   보존하며, 같은 폴더의 재시도는 거절된다.
6. **네트워크·removable·비-NTFS**: 새 생성만 `DRIVE_FIXED` + NTFS 를 요구한다. 기존 Project 의
   읽기/저장은 파일 시스템 종류를 추가로 제한하지 않는다(hardlink 를 못 거는 파일 시스템은
   기존대로 `PublishUnsupported` 로 거절).
7. **같은 사용자 악의 writer**: 검사 직후의 모든 경합을 막지 못한다. 핸들 보유로 Unix 보다
   줄어드는 창도 있으나 "완전 방어"로 적지 않는다.

## 4. 흐름별 절차 (Windows)

### W1 `start_visual`

1. 절대 경로(`C:\…` 또는 `\\?\C:\…`)만 받는다. canonicalize 뒤 폴더 핸들을 no-follow 로
   열고 reparse 가 아닌 디렉터리인지 확인한다.
2. 드라이브 종류 `DRIVE_FIXED`, 볼륨 NTFS 가 아니면 생성 전에 거절한다.
3. `CreateDirectoryW(root\.gil)` 로 배타 예약한다. 어떤 항목이든 이미 있으면 거절한다
   (빈 디렉터리·중단 잔여물·링크 포함). 생성 뒤 `.gil` 을 no-follow 로 열어 보유하고
   identity 를 기록한다.
4. 이후 단계는 macOS 와 같다: 잠금, `initializable`, world capture, `prepare_visual_upgrade`,
   encode, `state_file::write_new`, 완료 checkpoint. 각 checkpoint 에서 `.gil` 과 root 의
   identity 와 잠금 identity 를 다시 확인한다.
5. 실패 시 남은 저장소는 보존한다. 성공 응답이 유실돼도 완전한 상태만 남는다.

### state 저장 (`state_file::write` / `write_new`)

1. 부모 이름이 `.gil` 이어야 한다. Project root 를 canonicalize 하고 root → `.gil` 을 no-follow
   로 열어 보유한다(생성 없음).
2. `<name>.writing` 을 `CREATE_NEW` 로 만든다. 기존 일반 파일·symlink·hardlink 가 있으면 실패하고
   그 내용은 보존된다.
3. 쓰고 `FlushFileBuffers`, 처음부터 다시 읽어 바이트를 대조한다. 임시 이름을 no-follow 로
   다시 열어 보유 핸들과 identity·링크 수 1 을 확인한다. `.gil` 이 여전히 보유 핸들과 같은
   항목인지 확인한다.
4. `.gil` 핸들 상대 `FileRenameInfoEx` 로 공개한다. `write_new` 는 교체 없이(존재 시 실패),
   `write` 는 항목 교체로. 디렉터리를 flush 하고 identity 를 마지막으로 확인한다.
5. 실패한 임시 파일은 지우지 않는다.

### W2 첨부 열기 (`visual_report::file::open` / `same`)

1. 상대 경로 규칙(`relative`)은 플랫폼 공통이다(`..`, 절대 경로, `\`, `:`, 예약 장치 이름,
   `.gil`/`.git` 거절).
2. root 의 각 구성 요소와 상대 경로의 각 구성 요소를 no-follow 로 열어 보유한다. 마지막
   파일은 `GENERIC_READ`, 공유 `FILE_SHARE_READ` 로 열어 첨부 준비 동안 쓰기·삭제를 막는다.
   디렉터리·reparse·링크 수 ≠ 1 은 거절한다.
3. 준비 전후 `Stamp`(identity + 크기 + 시각 3종 + 링크 수 + 속성)를 비교하고, 이름으로 다시
   연 파일의 `Stamp` 도 같아야 한다. 재검증은 바이트도 다시 읽어 비교한다.
4. 저장 객체 읽기(`read_stored`)는 `.gil/artifacts/blobs/sha256/xx/…` 를 같은 방식으로 열고
   digest 와 metadata 를 대조한다.

### W2/W3 저장 객체 (`ObjectStore::put` / `read_blob` / `read_manifest`)

1. root → `.gil` → `artifacts` 를 no-follow 로 열어 보유한다(`put` 은 필요한 디렉터리를 만들되
   각 단계에서 reparse 를 거절한다). `tmp` 에 `<pid>-<n>` 을 `CREATE_NEW` 로 만든다.
2. 흘려 쓰며 digest 를 재고 `FlushFileBuffers` 한다. 목적지 디렉터리 체인을 다시 보유하고,
   `CreateHardLinkW(tmp, final)` 로 확정한다. 이미 있으면 그 객체를 건드리지 않고 digest 만
   재확인한다. 임시 항목은 보유 핸들로 삭제한다.
3. 확정된 객체를 no-follow 로 다시 열어 일반 파일인지와 digest 를 확인한다.
4. 읽기는 같은 체인으로 열고 reparse·디렉터리를 거절한 뒤 호출자가 digest 를 대조한다.

### W4 Session 입구 (`lock`, `preflight_state`, `store::load`)

1. 잠금 파일은 `.gil` 을 보유한 채 no-follow 로 열고 일반 파일·링크 수 1 을 확인한다.
   `validate` 는 이름으로 다시 열어 디렉터리·파일 identity 를 비교한다.
2. `preflight_state` 는 상태 파일이 링크·디렉터리·특수 항목이면 복구·정리 전에 거절한다.
3. `store::load` 는 no-follow 핸들로 읽고 읽기 전후 identity 를 비교한다.
4. 재시작 뒤 `open_read_only` 로 연 Project 의 보고서·선언·이미지 바이트는 저장 전과 같아야 한다.

## 5. capability 와 도구 표면

- `IMAGE_STORAGE_SUPPORTED` 는 `visual-authoring` feature 이면서 (macOS aarch64 또는 Windows)
  일 때만 참이다. Windows 포함은 §6 의 runner 실행 증거가 있을 때만 유지한다. 포함 전에는
  descriptor 가 format 4..4 를 말하므로 Skill/Host 가 이미지 가능 Project 로 보고하지 않는다.
- action surface 는 그대로 2(feature 빌드) / 1(기본 빌드). 도구 이름·schema 는 바뀌지 않는다.
  기본 빌드(19 도구)는 Windows 에서도 그대로이며, 작성 빌드(24 도구)는 별도 profile 로만 만든다.
- 이관 도구 4 개는 Windows 에서 `unsupported_platform` 을 유지한다. 도구 설명·Skill 은
  "새 생성은 지원, 이관은 미지원"을 한 문장으로 구분한다.

## 6. 인수 기준

각 항목은 **Windows runner(x64 `windows-2025`, ARM64 `windows-11-arm`) 에서 실제 실행**해야
통과다. Mac 의 교차 `cargo check` 는 컴파일 증거일 뿐이고, runner 통과도 실제 PC·Host 화면
인수가 아니다. 결과는 run id 와 함께 복귀 보고서에 적는다.

| ID | 시나리오 | 시험 | 결과 |
| --- | --- | --- | --- |
| W1-1 | 빈 폴더·기존 Git 폴더에서 최초 format 5, Interview 하나/Step 0, 기존 파일 불변, `.writing` 잔여 없음 | `session::visual_start::tests::first_publication_is_visual_and_existing_git_files_are_unchanged` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)) |
| W1-2 | 기존 format 4/5, 빈 `.gil`, 중단 잔여물, `.gil` 파일, junction, 미지 형식 → 거절·불변 | `…::existing_records_and_even_empty_or_partial_storage_are_not_reused` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); kind 5 는 junction |
| W1-3 | 동시 start 는 하나만 성공 | `…::exclusive_reservation_allows_only_one_concurrent_creator` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)) |
| W1-4 | 상대 경로·없는 폴더·파일·쓰기 거부 폴더 → `.gil` 없음 | `…::denied_or_invalid_folder_does_not_create_storage` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); 쓰기 거부는 `icacls` Everyone deny(AD,WD) |
| W1-5 | 9 개 checkpoint 각각에서 디스크 고갈 → 공개 전 부재/공개 후 완전, 작업 파일 보존, 재시도 거절 | `…::failures_before_and_after_publication_preserve_work_and_remaining_storage` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); 고갈은 checkpoint 에 주입한 `ERROR_DISK_FULL`(실제 디스크 고갈 아님) |
| W1-6 | 공개 직전 다른 writer 의 `state.yaml` 을 덮어쓰지 않음 | `…::concurrent_state_entry_is_not_overwritten_at_publication` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); 배타 rename 이 기존 항목에서 실패함을 관측 |
| W1-7 | `.gil` 이름 변경은 보유 중 거부, 잠금/임시 항목 교체는 감지·보존 | `…::changed_directory_lock_or_temporary_entry_is_preserved_and_refused` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); `.gil` rename 은 열린 잠금 때문에 OS 가 거부, 잠금 교체는 identity 로 감지 |
| W1-8 | capture 가 거절하는 링크가 있으면 커밋 없음·원본 보존 | `…::capture_refusal_preserves_source_and_does_not_commit_or_hide_the_problem` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); 링크는 junction |
| W1-9 | 5 개 checkpoint 에서 프로세스 강제 종료 → 부재 또는 완전 상태, 증거 보존 | `…::killed_creator_leaves_absent_or_complete_state_and_preserves_evidence` | 통과 x64·ARM64 (lib 필터, run 39126103310 (head 09882352)); 종료는 `std::process::abort`(전원 차단 아님) |
| W2-1 | 첨부 확정 후 blob·registry·state 바이트가 응답 전에 영속 | `tests/visual_attachment.rs::attachment_close_persists_report_provenance_and_bytes_before_receipt` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)) |
| W2-2 | 준비 중 파일 수정·동일 바이트 교체 → 논리 close 없음 | `…::file_changed_during_preparation_refuses_without_logical_close`, `…::replacing_file_with_identical_bytes_still_changes_identity` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)) |
| W2-3 | 파일 symlink·디렉터리 junction·hardlink·unsafe 이름 거절 | `…::source_links_parent_links_hardlinks_and_unsafe_names_are_refused` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)); 파일 symlink 는 runner 의 symlink 권한으로 생성 |
| W2-4 | 디렉터리·중복 입력 거절 | `…::directory_fifo_and_duplicate_input_do_not_become_attachments` (Windows 는 FIFO 대신 디렉터리·중복만) | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)) (FIFO 대신 junction·디렉터리·중복) |
| W2-5 | 내부 구성 요소마다 symlink/junction/hardlink 거절, 손상·다른 정상 PNG 거절 | `…::stored_read_refuses_links_at_every_internal_component`, `…::stored_read_rejects_missing_corrupt_and_valid_but_wrong_bytes` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)); 3차 run 39125709523 까지는 fixture 의 junction 제거 오류로 실패, 거절 자체는 그때도 관측 |
| W2-6 | state 임시 이름의 기존 파일/링크를 truncate 하지 않음, 링크된 `.gil` 부모 거절, 목적지 링크 교체 시 대상 보존 | `…::visual_state_temp_never_truncates_preexisting_files_or_links`, `…::visual_state_refuses_linked_internal_parent_without_writing_target`, `…::visual_state_replaces_target_link_not_its_referent` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)); 목적지 symlink 항목 교체 후 대상 보존 확인 |
| W3-1 | 저장 이미지를 다른 Report 에서 인용(보고서별 caption, origin 유지), 잘못된 sha256 거절 | `tests/mcp.rs::authoring_stdio_close_reuse_confirmation_and_rejections` | 통과 x64·ARM64 (`--test mcp` 24/24, run 39126103310 (head 09882352)) |
| W3-2 | 선언 3 종과 실패한 commit 이 Report 를 열어 둠, 경쟁 close 는 하나만 | `…::authoring_declarations_and_failed_commit_keep_reports_open`, `…::authoring_competing_closes_publish_only_one_report` | 통과 x64·ARM64 (`--test mcp` 24/24, run 39126103310 (head 09882352)) |
| W4-1 | 원본 삭제 뒤 재시작·읽기 전용 열기에서 projection 과 이미지 바이트 동일, state 불변 | `tests/visual_attachment.rs::display_projection_survives_reopen_and_source_deletion`, `…::read_only_session_reads_preserved_image_without_recovery_or_state_write` | 통과 x64·ARM64 (`visual_attachment` 22/22, run 39126103310 (head 09882352)) |
| W4-2 | 새 서버 프로세스가 기존 기록·잔여물을 복구·재시작 없이 읽음, 응답 유실 뒤 상태 보존 | `tests/mcp.rs::visual_start_preserves_existing_and_partial_records_and_survives_response_loss` | 통과 x64·ARM64 (`--test mcp` 24/24, run 39126103310 (head 09882352)) |
| W4-3 | 큰 이미지 바이트가 stdio 로 정확히 전달되고 손상 객체는 거절 | `tests/mcp.rs::visual_report_tools_read_exact_bytes_and_refuse_unlinked_assets_over_stdio` | 통과 x64·ARM64 (`--test mcp` 24/24, run 39126103310 (head 09882352)) (default profile 19/19 에서도 실행) |
| W5-1 | 작성 profile 네이티브 smoke: 24 도구 계약, 새 start → `storage_format: 5`, 재시작 뒤 status·Monitor View | `distribution/windows/spike.mjs` (`GIL_RUNTIME_PROFILE=visual-authoring`) | 통과 x64·ARM64 (`spike.mjs` visual-authoring: 24 도구, descriptor surface 2 / format max 5, `storage_format: 5`, `format: 5` 상태 파일, 재시작 뒤 status·View, run 39126103310 (head 09882352)) |
| W5-2 | 기본 profile 회귀 불변: 19 도구, 기존 binding/MCP 시험 | 기존 `windows-spike.yml` 기본 job | 통과 x64·ARM64 (default: binding 11, mcp 19, spike 19 도구·format 4·재시작, candidate 9, run 39126103310 (head 09882352)) |
| W5-3 | Mac 회귀 불변: `cargo test --locked -p gil` default·visual-authoring, JS·패키징 시험 | 기존 `codex-preview.yml` | 통과 (macOS run 39126103325, default 와 visual-authoring 전체 시험·JS·패키징) |
| W2-7 | 객체 저장 경계: no-clobber 확정, 확정 후 재판독 검증, tmp 잔여 없음, 동시 확정자 합의, 손상 객체 거절; 잠금 identity·경쟁·부재 | lib 필터 `artifact::store::tests::*`(31), `lock::tests::*`(7) | 통과 x64·ARM64 (lib 필터 118 통과 / 0 실패 / 1 ignored = `crash_child` 진입점, run 39126103310 (head 09882352)) |

### 6.1 runner 실행 결과 요약 (2026-10-18, PR hyun06000/gil#25)

- 1차 run 39125038859 (head 217376c4): 4 job 실패. 원인 1개 — `FileRenameInfoEx` 에 `RootDirectory`
  상대 이름을 넘겨 `ERROR_INVALID_PARAMETER`. 수정: 전체 경로 + 디렉터리 identity 재확인.
- 2차 run 39125352678 (head 2832e05d): default 2 job 통과, authoring 2 job 은 mcp 24/24 통과 뒤
  `visual_attachment` fixture(junction 제거) 오류로 실패. 수정: fixture 만.
- 3차 run 39125709523 (head 5ffd9d1d): authoring 2 job 이 lib 필터 1 개 실패 — **Core 결함**:
  junction root 가 `canonicalize` 로 풀려 생성 허용. 수정: 선택 경로를 먼저 no-follow 로 열어 거절.
- 4차 run 39126103310 (head 09882352): **4 job 모두 통과.** 건너뛴 시험 없음(ignored 1 = 자식
  진입점). 이 표의 통과는 모두 4차의 실제 실행이다.
- 실행되지 않은 것: 실제 디스크 고갈·전원 차단, ADS 를 직접 만드는 시험(이름 규칙 거절은
  `visual_report::file::tests::path_contract_is_shared_across_platforms` 로 실행), 비-NTFS·
  네트워크 드라이브 거절(runner 에 없음), §8 항목 1 의 ACL 시나리오, Host 화면·설치.

## 7. 증거 구분

- **교차 컴파일**: Mac 에서 `cargo check --target x86_64-pc-windows-msvc` (default·feature). 컴파일 증거.
- **runner 실행**: `windows-spike.yml` 의 `runtime_profile=visual-authoring` dispatch. 위 표의 통과 증거.
- **실제 PC**: 방법 B 검수(별도). Host 화면·설치·fullscreen 은 사람이 본다.
- 어떤 단계의 성공도 다음 단계를 대신하지 않는다.

## 8. 미해결·후속

1. **`.gil` 의 ACL — 미해결, 작성 후보 출시 차단 항목.** 단계 1 은 기존 Windows format-4
   `gil start` 와 같은 **상속 ACL** 로 `.gil` 을 만든다(`CreateDirectoryW` 기본 보안 속성).

   *계약이 보호하려는 대상.* Unix 의 `.gil` `0o700` 은 **같은 기계의 다른 사용자 계정**(그룹·
   기타)이 Project 기록을 읽거나 바꾸지 못하게 한다. root 와 같은 사용자의 다른 프로세스는
   보호 대상이 아니다. Windows 동등 계약은 "현재 사용자(및 SYSTEM·관리자)만 `.gil` 에 접근할
   수 있다"이며, 보호 범위는 다른 표준 사용자 계정까지다. 관리자는 Unix 의 root 에 대응한다.

   *현재 구현이 계약을 충족하는가.* **조건부로만 충족한다.** 사용자 프로필 아래 폴더
   (`C:\Users\<이름>\…`)는 상속 DACL 이 해당 사용자·SYSTEM·Administrators 로 제한되므로
   동등하다. 그러나 상속 ACL 이 넓은 위치(예: `C:\` 바로 아래에 만든 폴더는 기본적으로
   `Authenticated Users` 에 하위 폴더·파일 Modify 를 상속한다, 공용 폴더, 관리자가 넓게 연
   작업 폴더)에서는 **다른 로컬 계정이 `.gil` 을 읽고 바꿀 수 있어** Unix 계약보다 약하다.
   이미지 Report 는 화면 캡처를 담으므로 노출 범위가 텍스트 기록보다 넓다.

   *같은 성질의 기존 경로.* 이 약점은 단계 1 이 새로 만든 것이 아니다. 기존 Windows format-4
   `gil start`(`lock::bootstrap` 의 `create_dir_all`)와 이미 게시된 Windows tester preview
   (`windows-preview-f0ecfbd`)도 같은 상속 ACL 이다. Monitor binding 설정은 별도로 보호
   DACL(`bindings_windows.rs`)을 쓴다.

   *선택지(결정 전까지 구현하지 않음).* (a) 생성 시 `.gil` 에 소유자 전용 보호 DACL 적용
   (binding 설정과 같은 `O:<sid>D:P(A;;FA;;;<sid>)`; 백업·동기화 도구 호환과 기존 Project 의
   ACL 불일치 처리 필요), (b) 상속 ACL 이 다른 계정에 접근을 주면 생성 거절 + 안내,
   (c) 사용자 프로필 아래 폴더만 허용. 어느 쪽이든 Unix 코드와 저장 형식은 바뀌지 않는다.

   *판정.* 계약을 모든 폴더에서 충족하지 못하므로 **Windows 이미지 작성 후보의 게시 전
   차단 항목**으로 둔다. 기존 텍스트 전용 Windows preview 에도 같은 사실을 별도 보고한다.
   runner 시험은 이 항목을 검증하지 않는다(사용자 프로필 아래 임시 폴더에서만 실행).
2. 디렉터리 `FlushFileBuffers` 의 실제 내구성 범위(저장 컨트롤러 캐시)는 README 와 같이 단정하지
   않는다.
3. cloud placeholder(OneDrive 등) reparse tag 는 "reparse 거절" 규칙으로 함께 거절된다.
   사용자 안내 문구는 설치 문서 단계에서 다룬다.
4. 버전 표기(`candidate.mjs` 의 `0.2.1-preview.3+windows…`), MSVC runtime 재배포 검토, 작성
   profile 의 Windows 후보 패키징·게시는 별도 승인 항목이다.
5. `reap_tmp`·복구·rollback 의 Windows 핸들 기반 경계는 저장 계약 §20–§24 의 후속이다.
