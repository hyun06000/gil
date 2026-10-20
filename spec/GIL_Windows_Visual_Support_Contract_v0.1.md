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
| `0o700` 디렉터리 | `.gil` 생성 시 명시적 보호 DACL `O:<user>D:P(A;OICI;FA;;;<user>)(A;OICI;FA;;;SY)` — 현재 사용자와 SYSTEM 만 전체 접근, 하위 항목으로 상속, 상위 폴더 상속 차단. 생성 직후 핸들로 실효 DACL 을 검증하고, 재열기 때마다 다시 검증한다(§4 W1·W4, §8) |

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
8. **같은 기계의 다른 사용자 계정**: Unix `0o700` 에 대응해 `.gil` 은 소유자 전용 보호 DACL 로
   태어난다. 상위 폴더가 `Authenticated Users` 등에 넓게 상속해도 `.gil` 아래 기록(상태·보고서·
   artifact blob)은 다른 표준 계정이 읽거나 바꾸거나 열거하지 못한다. 관리자(SYSTEM·소유권
   탈취 권한)는 Unix 의 root 에 대응하므로 보호 대상이 아니다. 경로가 사용자 프로필 아래라는
   사실은 근거가 아니며, 판정은 언제나 핸들로 읽은 실효 DACL 이다.

## 4. 흐름별 절차 (Windows)

### W1 `start_visual`

0. **위치 지원 검사(2026-10-21, 기록 생성 전).** `gil_start` 와 `start_visual` 은 `.gil` 이나
   어떤 기록도 만들기 전에 선택 폴더를 `windows_fs::classify_location` 으로 가른다: UNC·원격
   드라이브(`network`), 고정 드라이브 아님(`not_fixed_drive`), NTFS 아님(`not_ntfs`), 경로 구성
   요소의 reparse point(`reparse_point`), cloud-files 속성(RECALL_ON_OPEN/PINNED/UNPINNED/
   RECALL_ON_DATA_ACCESS)을 가진 구성 요소(`cloud_files`), `OneDrive`/`OneDriveConsumer`/
   `OneDriveCommercial` 환경 변수가 가리키는 루트 아래(`onedrive`), 검사 자체 실패
   (`unverifiable`). 어느 것이든 `code: unsupported_location` 으로 거절하며 아무것도 만들지
   않는다. **탐지 불가는 안전 확인이 아니다.** 일반적인 부모 ACL 상속만으로는 거절하지 않는다
   (새 `.gil` 은 3 에서 자기 보호 DACL 을 받는다). 이 검사는 지원 범위 안내이며 어떤 위치가
   오류의 원인이라는 판정이 아니다(§8 1b). 거절 응답의 `said` 는 비개발자용 한 문장이고
   `suggested_parent` 는 승인 기반 작업 폴더 준비(아래)의 기본 상위 폴더다.
   거절 응답은 `evidence` 로 근거의 종류를 밝힌다: `observed`(OS 가 이 경로에 대해 보고한 속성·
   드라이브 종류·reparse·UNC), `declared_by_environment`(OneDrive 환경 변수가 선언한 루트; 변수가
   오래됐을 수 있다), `check_failed`(검사 실패). 선언·실패를 "OneDrive 다"로 격상하지 않는다.
   **승인 기반 로컬 작업 폴더 준비** `gil_prepare_workspace{name, parent?, confirm, approved_path?}`
   (Windows 작성 표면 전용). 계약:
   - 허용 범위: `parent` 는 `%USERPROFILE%` 아래여야 하고(기본 `%USERPROFILE%\gil-projects`), 한
     호출이 만드는 것은 없는 상위 한 단계와 폴더 자신뿐이다. 임의 경로 생성 도구가 아니다
     (`outside_allowed_range`).
   - `confirm:false` 는 만들 정확한 경로만 보여 주고 아무것도 만들지 않는다(쓰기 권한도 필요 없다).
   - `confirm:true` 는 계획이 보여 준 경로 문자열을 `approved_path` 로 함께 받아야 하며 다르면
     `path_mismatch` 로 무생성 거절한다. 이것은 요청을 사용자가 본 경로에 묶는 장치이지 인간 승인의
     검증이 아니다: Core 는 사용자가 실제로 승인했는지 알 수 없고, Agent 가 경로와 백업 범위를
     설명한 뒤 턴을 끝내고 기다렸는지는 Skill 회귀·화면 검수가 따로 확인한다(§6 W12-6).
   - 생성 직전 계획을 다시 도출해 위치·조상·충돌을 재검사하고, 계획과 다르면 `plan_changed` 로
     무생성 거절한다. 상위와 폴더는 각각 배타 생성한다(기존 항목 재사용·덮어쓰기·다른 이름 대체
     없음, 충돌 `exists`·권한 거절 `permission_denied`·위치 미확인은 무변경 거절, 관리자 실행·권한
     변경 없음).
   - 실패 시 기존 항목은 건드리지 않으며, 응답의 `created_by_this_call` 이 이번 호출이 만든 항목과
     각각의 잔존 여부를 그대로 보고한다(지우지 않는다).
   - 만든 폴더는 일반 사용자 폴더(상속 ACL)다. 응답은 "확인한 범위에서 OneDrive 가 선언한 백업
     위치 밖이고 클라우드 동기화 속성이 없다"고만 말하며 다른 동기화 제품의 부재를 보장하지 않는다.
   - 폴더 생성은 앱의 작업 폴더 선택이 아니다: 사용자가 그 폴더를 선택·연결한 뒤에만 `gil_start` 를
     부른다(Skill).
   **action surface 판단(2026-10-21).** `gil_prepare_workspace` 는 Windows 작성 표면에만 추가되는
   도구이고 기본 표면(action surface 1, 19 tools)에는 없다. 작성 표면은 2 를 유지한다: 추가는
   가산적이며 기존 도구의 schema·의미가 바뀌지 않고, 이 도구를 모르는 Agent 도 `gil_start` 의
   구조화 거절을 그대로 보고할 수 있다. 플랫폼별 도구 수는 Windows 작성 25 / macOS 작성 24 /
   기본 19 이며 catalog·`tool-contract.mjs`·`candidate.mjs` 라벨·`checks.json` receipt·smoke·Skill
   회귀가 같은 수를 본다.
1. 절대 경로(`C:\…` 또는 `\\?\C:\…`)만 받는다. canonicalize 뒤 폴더 핸들을 no-follow 로
   열고 reparse 가 아닌 디렉터리인지 확인한다.
2. 드라이브 종류 `DRIVE_FIXED`, 볼륨 NTFS 가 아니면 생성 전에 거절한다.
3. **핸들 우선 예약(설계안 A, 2026-10-19).** pin 된 Project root 핸들을 `RootDirectory` 로
   `NtCreateFile(".gil", FILE_CREATE, FILE_DIRECTORY_FILE|FILE_OPEN_REPARSE_POINT, 보호 DACL)` 을
   한 번 호출한다. 성공하면 **그 호출이 만든 객체의 핸들**이 돌아오므로 생성과 최초 핸들 획득
   사이에 이름 재해석이 없다. 어떤 항목이든 이미 있으면 그 항목을 건드리기 전에 실패한다(빈
   디렉터리·잔여물·파일·링크 포함; 그 항목의 ACL·내용 불변). 정규화(`SetSecurityInfo`)·검증
   (소유자 = 현재 사용자, `SE_DACL_PROTECTED`, 상속 가능한 전체 접근 허용 ACE 정확히 둘:
   현재 사용자·SYSTEM)·**검증 실패 시 삭제**는 모두 그 핸들로 수행한다 — 삭제되는 것은 구성상
   그 호출이 만든 비어 있는 객체뿐이며 이름은 다시 풀지 않는다.
   **핸들 전환(감지 후 거절, 방지 아님).** 그 핸들은 DELETE 접근을 가져 이후 잠금·상태 쓰기의
   pin 열기(삭제 미공유)와 충돌하므로, `ReOpenFile` 로 같은 객체의 읽기 핸들을 얻고 원래 핸들을
   닫은 뒤 root 핸들 상대 `NtCreateFile(".gil", FILE_OPEN)` 로 삭제 미공유 핸들을 다시 연다.
   세 핸들의 identity 가 같을 때만 진행한다. 다르면 거절하며, 이름으로 아무것도 지우지 않고
   이름 아래의 객체도 바꾸지 않는다(우리 예약 객체는 옮겨진 이름 아래 비어 있는 채 남는다).
   이 전환 동안 기록은 쓰지 않는다. 전환이 끝난 뒤에는 그 핸들이 생성 내내 `.gil` 의 이름
   변경·삭제를 OS 공유 규칙으로 막는다. 그 뒤의 어떤 실패도 저장소를 보존한다.
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
5. **재열기 순서와 "무수정"의 범위(2026-10-19 보완·2 차 보정).** `open`·`open_read_only` 는
   다음 순서다. (a) 잠금 전에 `state.yaml` 전체를 no-follow 제어 파일 핸들로 읽어 **로더와 같은
   판별**(`store::declared_format`: YAML 을 `StoredFormat` 으로 해석하므로 첫 줄 주석·빈 줄이
   있어도 같다)로 선언 형식을 가른다 — 생성 없음. **판별 방침:** `format: 4` 또는 `5` 만 통과한다.
   파일 부재·읽기 실패(링크·특수 파일·비-UTF-8 포함)·선언 없음·앞 형식·미지 형식은 어느 것도
   format 4 로 간주하지 않으며, 복구·정리 없이 기록을 보존한 채 거절한다(로더가 내는 것과 같은
   오류). 복구 대기 기록이 있어도 형식을 안전하게 판별할 수 없으면 복구를 시도하지 않는다 —
   헤더 판별(이 단계)과 복구 가능성 판단(복구 모듈)은 분리돼 있고 전자가 먼저다.
   (b) format 5 이면 `verify_storage_tree` 로 `.gil` 과 제어·구조 항목(`state.yaml`,
   `project.lock`, `artifacts`, `artifacts/blobs`, `artifacts/blobs/sha256`, `artifacts/manifests`,
   `artifacts/manifests/sha256`, `artifacts/tmp`, `restore` — 있는 것만)을 읽기만으로 검증한다;
   실패하면 잠금도 잡지 않고 거절. (c) 잠금을 잡는다 — 이때 `project.lock` 이 없으면 만들어진다
   (검증된 `.gil` 의 DACL 을 상속). (d) 잠금 아래에서 (a) 를 다시 하고 **잠금 전 판별과 같아야**
   한다(다르면 거절), format 5 이면 (b) 를 다시 한다(`project.lock` 과 판별–잠금 사이의 변경
   포함); 실패하면 잠금을 놓고 거절. (e) 그 뒤에만 `preflight_state` → 복구(`restore::recover`) →
   임시 객체 정리(`reap_tmp`) → 상태 읽기 → 로드된 Project 의 형식으로 마지막 게이트.
   format 4 는 (a)·(d) 의 판별만 거치고 ACL 검사 없이 기존 복구·정리 흐름으로 간다.
   **무수정의 범위:** (d) 를 통과하기 전에 `.gil` 아래의 어떤 항목도 만들거나 지우거나 이름을
   바꾸거나 쓰지 않는다 — 복구 기록(`restore/*`), 임시 객체(`artifacts/tmp/*`), `.writing`,
   `state.yaml` 모두 그대로다. 유일한 예외는 (c) 의 `project.lock` 생성(없을 때만)이며, 그 파일의
   OS advisory lock 은 내용 변경이 아니다. blob·manifest 객체는 읽을 때와 확정 직후 같은 조건으로
   검사하고, 소유자 전용 계약의 쓰기는 임시 파일을 만들기 전에 `.gil` 을 먼저 검증한다.
   어긋나면 거절하고 **아무것도 고치지 않는다**. format-4 Project 와 다른 플랫폼은 이 검사를
   받지 않는다. macOS 에서 만든 format-5 기록을 Windows 로 복사한 경우도 같은 이유로 열리지
   않는다(채택 기능 없음).

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

| W6-1 | 새 `.gil` 이 소유자 전용(소유자·보호·사용자/SYSTEM 상속 ACE 둘)이고 하위 기록이 그것만 상속 — **구조 검사**(핸들로 읽은 실효 DACL + `icacls` 목록) | `session::visual_start::tests::new_gil_is_owner_only_with_a_protected_inheritable_dacl_and_children_inherit_it` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)); 자식 `(I)` 표시는 설명자를 `SetSecurityInfo` 로 정규화한 뒤에만 나타났다(9·10 차) |
| W6-2 | 넓어진 `.gil` DACL 은 재열기에서 거절, 복구·재작성 없음; format-4 Project 는 영향 없음 | `…::weakened_gil_dacl_is_refused_on_reopen_and_never_repaired` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)) |
| W6-3 | **실제 접근 시험**: `Authenticated Users` Modify 를 상속하는 시스템 드라이브 루트 폴더에서, 임시 로컬 계정이 제어 파일·프로젝트 파일은 읽고 쓰지만 `.gil/state.yaml` 읽기·`.gil` 열거·쓰기·삭제는 거부 | `…::another_local_account_cannot_read_or_write_gil_records_under_a_broad_folder` (`net user` + `Start-Process -Credential`) | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)); 임시 계정이 제어 파일·프로젝트 파일은 읽고 썼으나 `state.yaml` type / `.gil` dir 은 exit≠0·Access is denied, `echo >` 는 파일 미생성·denied(cmd 는 ERRORLEVEL 0), del 뒤 state.yaml 잔존. **실제 접근 결과이며 ACL 문자열 검사(W6-1)와 구분** |
| W6-4 | UNC 경로와 매핑된 네트워크 드라이브(`net share` + `net use`)는 생성 거절·저장소 없음, 같은 폴더의 로컬 고정 경로는 허용 | `…::unc_and_mapped_network_roots_are_refused_without_creating_storage` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)); `net share` + `net use` 매핑, UNC 모두 거절·`.gil` 없음, 로컬 경로는 생성 |
| W6-5 | FAT/exFAT 등 비-NTFS 볼륨 거절 | runner 에 그런 볼륨이 없어 **실행 불가**(코드 경로: `local_fixed_ntfs` 의 파일 시스템 이름 검사) | 미실행 |
| W7-1 | pin 된 체인의 모든 구성 요소는 보유 중 이름 변경·삭제가 OS 에서 거부되고 해제 뒤 가능 | `windows_fs::tests::pinned_chain_blocks_rename_and_delete_of_every_component_until_released` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)) |
| W7-2 | pin 하지 않은 핸들 뒤에서 디렉터리가 교체되면 rename 은 identity 재확인으로 거절, 어느 쪽에도 쓰지 않음 | `…::rename_refuses_a_directory_replaced_behind_an_unpinned_handle` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)) |
| W7-3 | 배타 rename 은 파일·디렉터리·junction 어떤 기존 항목에서도 실패하고 임시 파일을 보존; 교체 rename 은 symlink 항목만 바꾸고 대상 보존 | `…::exclusive_rename_fails_on_any_existing_entry_and_replace_swaps_only_the_entry` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)); 파일 symlink 교체는 runner 의 symlink 권한으로 실행 |
| W7-4 | 공개 뒤 두 번째 hardlink 나 symlink 가 생긴 제어 파일은 읽기에서 거절 | `…::control_file_refuses_a_hard_linked_or_linked_record` | 통과 x64·ARM64 (lib 필터 126/126, ignored 1 = `crash_child`, run 39129885115 (head b84a6322)) |

| W8-1 | `NtCreateFile` 직후·정규화 전 실효 DACL 이 이미 소유자 전용(소유자·보호·user/SYSTEM ACE 둘) | `session::visual_start::tests::reservation_is_owner_only_before_normalization_and_nothing_else_is_written` (시험 훅, 제품 빌드 미노출) | 통과 x64·ARM64 (lib 필터 131/131, ignored 1 = `crash_child`, run 39162630706 (head 19508d50)); `NtCreateFile` 직후 `private_dir` 통과(소유자·보호 비트·ACE 둘) |
| W8-2 | 기존 파일·디렉터리·junction 충돌 시 그 항목의 내용·ACL 불변, 정리 분기 미진입 | `…::existing_entry_collision_leaves_that_entry_and_its_acl_untouched` | 통과 x64·ARM64 (lib 필터 131/131, ignored 1 = `crash_child`, run 39162630706 (head 19508d50)); 파일·디렉터리·junction 모두 내용·`icacls` 목록 불변, 정리 분기 미진입 |
| W8-3 | 검증 실패(주입) 시 방금 만든 예약만 제거, 형제 항목·작업 파일 불변, 재시도 가능 | `…::verification_failure_removes_only_the_fresh_reservation` | 통과 x64·ARM64 (lib 필터 131/131, ignored 1 = `crash_child`, run 39162630706 (head 19508d50)); 주입 실패 뒤 `.gil` 부재·형제/작업 파일 불변·재생성 성공 |
| W8-4 | 핸들 전환 중 교체(주입): 거절, 심어진 디렉터리 내용·ACL 불변, 기록 없음, 우리 예약은 옮겨진 이름 아래 보존 | `…::handle_transition_swap_is_detected_and_refused_without_touching_either_object` | 통과 x64·ARM64 (lib 필터 131/131, ignored 1 = `crash_child`, run 39162630706 (head 19508d50)); 심어진 디렉터리는 상속 ACL·내용 그대로, 기록 없음, 원 예약은 옮겨진 이름 아래 소유자 전용·빈 채 보존 |
| W8-5 | 넓어진 blob 은 재열기에서 미감지(한계)·읽기에서 거절, 넓어진 `state.yaml` 은 재열기 거절, 상속 끊긴 `artifacts/blobs` 는 재열기 거절, 어느 것도 복구 없음 | `…::widened_child_entries_are_refused_on_reopen_or_on_read_but_never_repaired` | 통과 x64·ARM64 (lib 필터 131/131, ignored 1 = `crash_child`, run 39162630706 (head 19508d50)); 넓어진 blob: 재열기 미감지(한계)·읽기 거절, 넓어진 `state.yaml`: 재열기 거절, 상속 끊긴 `artifacts/manifests`: 재열기 거절, 복구 없음 |

| W9-1 | 소유자 전용 계약에서 `.gil` 이 넓어지면 객체 읽기·쓰기가 검사를 건너뛰지 않고 거절(임시 파일 미생성); 같은 바이트를 상속 계약(format 4)으로 읽는 것은 영향 없음; 재열기 거절·무수정 | `session::visual_start::tests::widened_gil_never_skips_object_checks_under_the_owner_only_contract` | 통과 x64·ARM64 (lib 필터 135/135, ignored 1 = `crash_child`, run 39165028169 (head 7b538d7b)) |
| W9-2 | ACL 조회 오류(소유자를 SYSTEM 으로 바꾸고 READ_CONTROL 거부)는 거절이지 상속 계약으로의 전환이 아님; 읽기·쓰기·트리 검증·재열기 모두 거절, 저장소 불변 | `…::acl_query_error_is_a_refusal_not_a_downgrade_to_the_inherited_contract` | 통과 x64·ARM64 (lib 필터 135/135, ignored 1 = `crash_child`, run 39165028169 (head 7b538d7b)); `GENERIC_READ` 열기 자체가 거부돼 조회 오류로 거절됨, 저장소 inventory 불변 |
| W9-3 | 복구 대기 기록·임시 객체·`.writing` 이 있는 상태에서 ACL 거절 시 그 기록·파일·잠금 파일 불변; 넓힘을 걷으면 같은 열기가 복구 단계에 도달; 임시 객체만 있는 Project 는 거절 중 보존·통과 뒤 정리 | `…::acl_refusal_happens_before_recovery_and_reaping_and_leaves_them_untouched` | 통과 x64·ARM64 (lib 필터 135/135, ignored 1 = `crash_child`, run 39165028169 (head 7b538d7b)); 거절 메시지는 ACL 게이트, inventory(복구 기록·tmp·`.writing`·`project.lock`·state) 동일, 넓힘 제거 뒤 복구 단계 도달, tmp 전용 Project 는 통과 뒤 정리 |
| W9-4 | format-4 기존 흐름: `.gil` 이 넓어져도 열기·정리·객체 읽기/쓰기·읽기 전용 열기 그대로 | `…::format_four_projects_keep_the_inherited_contract_and_their_flow` + default job 전체 | 통과 x64·ARM64 (lib 필터 135/135, ignored 1 = `crash_child`, run 39165028169 (head 7b538d7b)); default job(mcp 19, spike format 4, candidate 9)도 통과 |

| W10-1 | 첫 줄 주석·빈 줄이 앞선 유효한 format-5 YAML 은 로더와 같이 열리고 읽기 전용 열기는 파일을 바꾸지 않음 | `session::visual_start::tests::format_five_state_with_leading_comment_and_blank_lines_opens_like_the_loader` | 통과 x64·ARM64 (lib 필터 139/139, ignored 1 = `crash_child`, run 39223870974 (head 49dcd9ba)) |
| W10-2 | 형식 미확인 4 종(파일 부재·선언 없음·미지 형식·비-UTF-8)은 복구 기록·tmp·`.writing`·잠금 파일을 둔 채 거절·불변 | `…::unclear_format_is_refused_before_recovery_and_reaping_with_everything_preserved` | 통과 x64·ARM64 (lib 필터 139/139, ignored 1 = `crash_child`, run 39223870974 (head 49dcd9ba)); 4 종 모두 inventory 동일 |
| W10-3 | 판별–잠금 사이 형식 변경(5→4, 4→5): 거절·불변, 4→5 는 정리 없이 거절 | `…::format_change_between_classification_and_lock_is_refused_untouched` (시험 훅) | 통과 x64·ARM64 (lib 필터 139/139, ignored 1 = `crash_child`, run 39223870974 (head 49dcd9ba)); 5→4·4→5 모두 "잠금 전" 거절, 4→5 는 tmp 보존 |
| W10-4 | format-4 정상 복구·정리 유지(주석 앞선 format-4 도 4 로 판별) | `…::format_four_recovery_and_reaping_still_run_after_classification` | 통과 x64·ARM64 (lib 필터 139/139, ignored 1 = `crash_child`, run 39223870974 (head 49dcd9ba)) |

| W12-1 | 합성 OneDrive 루트(환경 변수) 아래 `gil_start` → `unsupported_location`/`onedrive` 거절, `.gil`·기존 파일 불변; 같은 서버에서 루트 밖 폴더는 정상 시작 | `tests/mcp.rs::unsupported_location_is_refused_before_creation_and_workspace_prep_needs_approval` | 기록 대기(미push) |
| W12-2 | `gil_prepare_workspace` 계획은 무생성(기본 상위 포함), `approved_path` 없음/불일치는 무생성 거절, 승인 생성은 정확히 그 경로(상위 포함)·빈 폴더·`created_by_this_call` 정확, 같은 이름 재요청은 `exists` 무변경·대체 이름 없음, 동기화 루트 아래·프로필 밖·불안전 이름·`..` 은 무생성 거절, 한글·공백 상위/이름, 없는 조상 `parent_missing`, 계획 뒤 상위가 junction 으로 바뀌면 생성 직전 재검사가 `reparse_point`(observed) 거절·대상 무변경, 생성 거부 ACL 은 `permission_denied`·무생성, 준비된 폴더에서 명시 root 로 `gil_start` 성공 | 같은 시험 | 기록 대기(미push) |
| W12-6 | Agent 가 경로와 백업 확인 범위를 설명한 뒤 턴을 끝내고 기다리는지, 승인 뒤에도 앱 선택 전에는 `gil_start` 를 부르지 않는지 | Skill 회귀(`skill.test.mjs`, 문구) + 화면 검수 | 문구 회귀만 실행; 화면 미검수 |
| W12-7 | 생성 뒤 거절(상위만 만들고 폴더 실패 등)에서 `created_by_this_call` 잔존 보고 | 코드 검토 | 합성 재현 없음(미검수) |
| W12-3 | 탐지 한계 명시: 없는 경로를 가리키는 변수는 누락(허용), 동기화 안 된 폴더를 가리키는 변수는 오탐(거절·무생성) | `…::onedrive_variable_misses_and_false_positives_are_what_the_contract_says` | 기록 대기(미push) |
| W12-4 | Core 방어: `start_visual` 자체도 위치 검사로 거절(네트워크·reparse 는 기존 W6-4·W1-2 가 실행) | `visual_start.rs` reserve | W6-4·W1-2 로 부분 실행 |
| W12-5 | 실제 OneDrive 환경에서 사전 거절·안내·폴더 준비·선택 후 시작 | 사용자 PC | 미실행 |

### 6.1 runner 실행 결과 요약 (2026-10-18, PR hyun06000/gil#25)

- 1차 run 39125038859 (head 217376c4): 4 job 실패. 원인 1개 — `FileRenameInfoEx` 에 `RootDirectory`
  상대 이름을 넘겨 `ERROR_INVALID_PARAMETER`. 수정: 전체 경로 + 디렉터리 identity 재확인.
- 2차 run 39125352678 (head 2832e05d): default 2 job 통과, authoring 2 job 은 mcp 24/24 통과 뒤
  `visual_attachment` fixture(junction 제거) 오류로 실패. 수정: fixture 만.
- 3차 run 39125709523 (head 5ffd9d1d): authoring 2 job 이 lib 필터 1 개 실패 — **Core 결함**:
  junction root 가 `canonicalize` 로 풀려 생성 허용. 수정: 선택 경로를 먼저 no-follow 로 열어 거절.
- 4차 run 39126103310 (head 09882352): **4 job 모두 통과.** 건너뛴 시험 없음(ignored 1 = 자식
  진입점). 이 표의 통과는 모두 4차의 실제 실행이다.
- 보완 라운드(2026-10-18, ACL·실제 접근·rename 검토): 6 차 run 39127711288 — Core 결함
  (예약 `.gil` 을 DELETE 접근 핸들로 보유해 이후 pin 열기와 공유 위반) + fixture icacls 오류;
  7 차 run 39128080405 — fixture DACL 이 정확한 설명자와 불일치(`adopt_private_storage` 로 교체,
  거절 사유 메시지 추가); 8 차 run 39128571227 — 124/126, 자식 ACE inherited 미표시(`D:P` 만),
  다른 계정 launcher 인용 문제; 9·10 차 run 39129041207/39129436214 — `AI` 만으로는 부족,
  `SetSecurityInfo` 정규화로 해결, Windows PowerShell Security 모듈 로드 불가 → .NET
  SecureString; 11 차 run 39129490528 — 125/126, cmd 리다이렉션 ERRORLEVEL 판정 문제;
  **12 차 run 39129885115 (head b84a6322): 4 job 모두 통과**, authoring job 당 binding 11,
  mcp 24, `visual_attachment` 22, `visual_start` 1, `visual_store` 8, lib 필터 126(ignored 1),
  작성 smoke·재시작. 이 표의 W6/W7 과 다시 실행된 W1–W5 는 모두 12 차 실행 결과다.
- 설계안 A 라운드(2026-10-19): 15 차 run 39161287667 — 확정 직후 하위 ACL 검사를 모든 저장소에
  적용해 format-4 `gil start` 가 깨짐(관리자 토큰의 하위 항목 소유자 = Administrators) → 체제
  한정·토큰 기본 소유자 허용; 16 차 run 39161844632 — default 2 job 통과, authoring 은
  `reserve/reopen: Access is denied`(Win32 `ReOpenFile` 이 디렉터리 핸들 거부) → `NtCreateFile`
  빈 이름 재열기; 17 차 run 39162190894 — mcp 24/24, lib 130/131, W8-1…W8-4 통과, W8-5 3 부
  fixture 오류(빈 폴더 Project 에 `artifacts/blobs` 없음); **18 차 run 39162630706 (head
  19508d50): 4 job 모두 통과**, authoring job 당 binding 11, mcp 24, `visual_attachment` 22,
  `visual_start` 1, `visual_store` 8, lib 필터 131(ignored 1), 작성 smoke·재시작. W6/W7/W8 의
  13 개 시험이 두 CPU 에서 실행됐다.
- 명시적 계약·무수정 순서 라운드(2026-10-19): 20 차 run 39164657855 — Core 통과, `visual_store`
  fixture(상속 ACL `.gil` 에 저장한 format-5 상태)가 새 계약대로 거절됨 → fixture 보정;
  **21 차 run 39165028169 (head 7b538d7b): 4 job 모두 통과**, authoring job 당 binding 11,
  mcp 24, `visual_attachment` 22, `visual_start` 1, `visual_store` 8, lib 필터 135(ignored 1),
  작성 smoke·재시작. W9-1…W9-4 가 두 CPU 에서 실행됐다.
- 형식 판별 보정 라운드(2026-10-19): **23 차 run 39223870974 (head 49dcd9ba): 첫 실행에서 4 job
  모두 통과**, authoring job 당 binding 11, mcp 24, `visual_attachment` 22, `visual_start` 1,
  `visual_store` 8, lib 필터 139(ignored 1), 작성 smoke·재시작. W10-1…W10-4 가 두 CPU 에서 실행됐다.
- 실행되지 않은 것: 실제 디스크 고갈·전원 차단, ADS 를 직접 만드는 시험(이름 규칙 거절은
  `visual_report::file::tests::path_contract_is_shared_across_platforms` 로 실행), 비-NTFS·
  네트워크 드라이브 거절은 12 차에서 UNC·매핑 드라이브로 실행(비-NTFS 볼륨은 여전히 없음),
  Host 화면·설치, `icacls` 수작업 DACL 과 정확한 설명자의 차이 진단.

## 7. 증거 구분

- **교차 컴파일**: Mac 에서 `cargo check --target x86_64-pc-windows-msvc` (default·feature). 컴파일 증거.
- **runner 실행**: `windows-spike.yml` 의 `runtime_profile=visual-authoring` dispatch. 위 표의 통과 증거.
- **실제 PC**: 방법 B 검수(별도). Host 화면·설치·fullscreen 은 사람이 본다.
- 어떤 단계의 성공도 다음 단계를 대신하지 않는다.

## 8. 미해결·후속

1. **`.gil` 의 ACL — 계약 확정·구현·runner 검증(2026-10-18 보완 §6 W6-1…W6-4 run 39129885115; 2026-10-19 설계안 A §6 W8-1…W8-5 run 39162630706).**

   *보호 대상.* 같은 기계의 **다른 사용자 계정**(표준 사용자, 다른 관리자의 비승격 토큰 포함)
   이 Project 기록을 읽거나 바꾸거나 열거하지 못한다. 접근 주체는 현재 사용자 SID 와 SYSTEM
   둘뿐이다. 관리자 권한 승격·소유권 탈취·백업 권한은 Unix root 처럼 보호 범위 밖이다. 같은
   사용자의 다른 프로세스도 보호 대상이 아니다.

   *상속 정책.* `.gil` 은 `SE_DACL_PROTECTED` 로 상위 폴더 상속을 끊고, 두 ACE 는
   `OBJECT_INHERIT|CONTAINER_INHERIT` 로 하위 항목(상태 파일·잠금·artifact·복구 기록)에 전파된다.
   하위 항목은 보안 속성 없이 만들어져 그것만 상속한다.

   *판정 근거.* 경로 위치(사용자 프로필 여부)는 근거가 아니다. 생성 직후와 재열기 때마다 핸들로
   실효 보안 설명자를 읽어 소유자·보호 비트·ACE 집합을 대조한다(§4 W1 3, W4 5).

   *실패 시 동작.* 보호 DACL 로 생성하지 못하면 아무것도 만들지 않는다. 생성 뒤 검증에 실패하면
   방금 만든 빈 예약 디렉터리만 보유 핸들로 지우고 거절한다. 재열기 검증 실패는 거절이며
   기록·ACL 을 바꾸지 않는다. 거절 메시지는 어긋난 첫 조건(소유자, 상속, ACE 수·플래그·마스크·
   주체)을 말한다. **기존 사용자 Project 의 ACL 은 어떤 경우에도 자동 변경하지 않는다.**
   제품에는 기존 저장소의 ACL 을 바꾸는 API 가 없다(2026-10-19 에 공개 `adopt_private_storage`
   제거). 시험 fixture 는 시험 코드(`tests/common/windows_acl.rs`)에서 같은 설명자를 적용한다.
   `icacls` 로 손수 만든 DACL 은 설명자가 정확히 같을 때만 통과한다(6 차 runner 에서
   `icacls … (OI)(CI)F` 결과는 거절됐다 — 차이의 정밀 진단은 남은 항목).

   *범위 밖·남는 것.* (a) 기존 Windows format-4 `gil start` 경로(`lock::bootstrap` 의
   `create_dir_all`)와 이미 게시된 tester preview `windows-preview-f0ecfbd` 는 여전히 상속 ACL
   이다 — 영향 범위와 대응안은 `distribution/windows/ACL-IMPACT-20261018.md`. 그 preview 에
   대한 제품 변경은 별도 결정이다. (b) 비-NTFS 볼륨은 DACL 이 없으므로 새 생성이 거절된다
   (W6-5 는 runner 에서 실행 불가). (c) Monitor binding 설정은 전부터 별도 보호 DACL 이다.

1a. **하위 항목 합격 조건(2026-10-19 확정, `windows_fs::inherited_only`).** `.gil` 아래의 파일과
   디렉터리는 핸들로 읽은 실효 보안 설명자가 다음을 모두 만족해야 한다.
   - 소유자 = 현재 사용자 SID, 또는 이 프로세스 토큰의 기본 소유자(`TokenOwner`: 승격된 관리자
     계정에서는 `BUILTIN\Administrators` — OS 가 상속으로 만든 항목에 자동으로 붙이는 소유자).
     `.gil` 자신은 명시 설명자로 만들므로 소유자 = 현재 사용자만 허용한다(15 차 runner 에서
     관리자 계정의 하위 항목 소유자가 Administrators 로 확인됨).
   - DACL 이 존재한다(NULL DACL = 모두에게 전체 접근 → 거절). `SE_DACL_PROTECTED` 가 **없다**
     (상속을 끊은 항목은 거절).
   - ACE 가 정확히 둘이고 모두 `ACCESS_ALLOWED_ACE_TYPE`. deny·audit·object·callback ACE 는 거절.
   - 플래그: 디렉터리는 정확히 `OBJECT_INHERIT|CONTAINER_INHERIT|INHERITED_ACE`, 파일은 정확히
     `INHERITED_ACE`. `INHERIT_ONLY`·`NO_PROPAGATE` 나 명시(비상속) ACE 는 거절.
   - 마스크 = `FILE_ALL_ACCESS`. 주체는 현재 사용자와 SYSTEM 각 하나.
   - 실제 소유자의 접근 가능성: 위 조건에서 소유자 ACE 가 전체 접근을 주고 deny ACE 가 없으므로
     소유자는 언제나 읽고 쓸 수 있다. 소유자 ACE 가 빠지거나 deny 가 섞이면 거절한다.
   적용 범위(명시적 계약, 2026-10-19 보완): 객체 창고는 호출자가 `Protection::Inherited`(format 4)
   또는 `Protection::OwnerOnly`(format 5) 를 **명시**해 만든다(`ObjectStore::at` / `owner_only` /
   `for_project`). 보호 여부를 `.gil` 의 ACL 을 읽어 **추론하지 않는다**. `OwnerOnly` 에서는 `.gil`
   의 소유자 전용 검증, shard 디렉터리와 객체의 상속 검증이 모두 통과해야 하며, **ACL 조회 오류를
   포함한 어떤 실패도 거절**이다(상속 계약으로 내려가지 않는다). 저장 이미지 읽기(`read_stored`)도
   같은 계약을 인자로 받는다. `Inherited` 저장소는 어떤 하위 검사도 받지 않는다(기존 Windows
   format-4 동작 보존; 15 차 runner 에서 무조건 검사가 format-4 `gil start` 를 깨뜨린 것을 바로잡음).
   검사 시점: 재열기 때 제어·구조 항목, 객체는 읽을 때와 확정 직후. **보장하지 않는 것:** 외부에서
   권한을 넓힌 **미사용 blob 의 즉시 탐지**(다음 읽기까지 감지되지 않음), 이미 일어난 노출의
   복구, 소유자·관리자·`WRITE_DAC` 보유 프로세스가 검사 사이에 바꾸는 것의 방지. Windows 의
   기본 "Bypass traverse checking" 권한 때문에 넓어진 하위 파일은 디렉터리 보호와 무관하게
   경로를 아는 계정이 읽을 수 있다 — GIL 은 다음 사용에서 거절할 뿐이다.
1b. **위치 탐지의 누락과 오탐(2026-10-21).** 이것은 배포 명세 §6.2b(OneDrive·네트워크·removable·
   reparse 경로는 채널 지원 대상 아님)의 정합이지 "동기화 폴더는 반드시 권한을 바꾼다"는 원인
   설명이 아니다. 현장(2026-10-21)에서 확인된 것은 OneDrive 리디렉션 폴더에서 생성·첫 재열기는
   통과한 뒤 이후 재열기에서 `.gil` 의 상속 차단이 사라져 거절된 사실과, OneDrive 밖 새 폴더에서는
   정상이었다는 대조뿐이며 인과는 미확정이다.
   - 환경 변수 방식: 다른 동기화 제품, 변수 미설정, 다른 사용자 계정의 OneDrive, 변수와 다른 실제
     리디렉션 경로를 **놓친다**; 동기화를 끈 채 변수만 남은 경우를 **오탐**한다.
   - cloud-files 속성 방식: Files On-Demand 가 꺼졌거나 "항상 이 기기에 보관" 인 폴더를 **놓친다**;
     OneDrive 가 아닌 cloud filter 제품을 OneDrive 로 **오탐**한다(안내 문구는 "클라우드 동기화").
   - 드라이브 종류·NTFS·reparse 검사는 OS 답에 따른다; 질의 실패는 `unverifiable` 로 거절한다.
   실제 OneDrive 환경에서의 검증과 합성 탐지 시험(환경 변수·경로 fixture)은 구분한다.
2. 디렉터리 `FlushFileBuffers` 의 실제 내구성 범위(저장 컨트롤러 캐시)는 README 와 같이 단정하지
   않는다.
3. cloud placeholder(OneDrive 등) reparse tag 는 "reparse 거절" 규칙으로 함께 거절된다.
   사용자 안내 문구는 설치 문서 단계에서 다룬다.
4. 버전 표기(`candidate.mjs` 의 `0.2.1-preview.3+windows…`), MSVC runtime 재배포 검토, 작성
   profile 의 Windows 후보 패키징·게시는 별도 승인 항목이다.
5. `reap_tmp`·복구·rollback 의 Windows 핸들 기반 경계는 저장 계약 §20–§24 의 후속이다.

## 9. 전체 경로 rename 의 보안 검토 (2026-10-18 보완)

1 차 runner 에서 `FileRenameInfoEx` 의 `RootDirectory` 상대 이름이 Win32 wrapper 에 거절돼
(`ERROR_INVALID_PARAMETER`) 목적지를 pin 된 디렉터리 아래 **전체 경로**로 바꿨다. 이 변경이
"경로 교체 경쟁" 보장을 유지하는지 따로 검토한 결과다.

- **구성 요소 교체.** rename 이 도는 동안 볼륨 루트부터 `.gil` 까지 모든 디렉터리 핸들을
  `FILE_SHARE_READ|FILE_SHARE_WRITE`(삭제 미공유)로 보유한다. 어떤 구성 요소를 junction 이나
  다른 디렉터리로 바꾸려면 먼저 그 항목을 이름 변경·삭제해야 하는데, 보유 핸들이 있는 동안
  OS 가 그것을 공유 위반으로 거부한다. 그러므로 커널이 전체 경로를 다시 푸는 순간에도 각
  구성 요소는 보유 핸들과 같은 항목이다(W7-1 로 실제 거부를 확인).
- **identity 재확인과 경쟁 구간.** `rename` 은 호출 직전에 디렉터리를 이름으로 다시 열어
  보유 핸들과 identity 를 대조한다. pin 된 체인에서는 이 확인과 `SetFileInformationByHandle`
  사이에 디렉터리가 바뀔 수 없으므로 경쟁 구간이 없다. pin 하지 않은 핸들(잠금의 `.gil`
  핸들처럼 삭제를 공유하는 것)로 호출하면 identity 재확인만 남고 확인과 호출 사이에 창이
  있으나, 제품 코드는 항상 pin 된 체인 핸들로 부른다(W7-2 는 pin 하지 않은 경우의 거절만 본다).
- **목적지 항목.** 목적지 이름은 pin 할 수 없다(아직 없거나 교체 대상). 배타 모드는 어떤
  기존 항목에서도 실패하고(W7-3), 교체 모드는 그 순간 그 이름에 있는 항목을 교체한다 —
  Unix `renameat` 와 같은 의미이며 symlink 는 항목으로만 바뀌고 대상은 보존된다(W7-3).
- **임시 파일의 두 번째 링크.** 링크 수 검사 뒤 공개 전에 같은 사용자가 hardlink 를 걸면
  공개된 상태 파일의 링크 수가 2 가 된다. 이것은 막지 않고, 다음 읽기에서 `control_file`
  이 링크 수 1 을 요구해 거절한다(W7-4). Unix 와 같은 fail-closed 다.
- **남는 한계.** 같은 사용자 권한의 writer 가 목적지 항목을 두고 벌이는 경쟁과, 전원 차단
  시 디렉터리 flush 의 실제 내구성은 이 검토로 닫히지 않는다(§3 7, §8 2).

2026-10-19 보완: 예약이 `NtCreateFile` 핸들 우선 방식으로 바뀌어 생성→최초 핸들 구간은 이름
재해석 없이 한 호출이 됐다. 핸들 전환(DELETE 핸들 → 읽기 핸들 → root 상대 pin 핸들)은 identity
대조로 **감지 후 거절**하며 방지가 아니다(§4 W1 3). 상태 쓰기의 rename 경계(이 절)는 바뀌지 않았다.
