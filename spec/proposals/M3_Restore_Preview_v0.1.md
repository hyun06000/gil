# M3 Restore Preview v0.1 — 제안

상태: **제안.** Wave 1 세션 B의 산출물이다. 저장 형식·제품 restore 정책·`product-safety` 차단을
바꾸지 않으며, 그 변경을 승인하는 문서도 아니다. 규범은 여전히 `GIL Artifact Model v0.1`과
`GIL Storage Model v0.1`이다. 이 제안은 Artifact Model §3.6a 3·4·5·6항의 **읽기 전용 부분**을
Core 안에서 어떻게 세우는지 적는다.

## 1. 문제

현재 `src/restore/plan.rs::steps(root, before, target)`는 target에 없는 현재 파일을 `Delete`로
계획한다. Artifact Model §8.0이 적듯 그 삭제 대상에 Git metadata와 추적하지 않은 사용자 파일이
들어갈 수 있어 제품 restore/revisit은 `product-safety`에서 거절된다.

§3.6a 4항은 「restore 전에 교체·생성·격리 대상을 보여 주는 read-only plan」을 요구한다. 그 plan을
`steps`에 이름만 바꿔 붙이면 **삭제 계획을 미리보기라고 부르는 것**이 된다. 그래서 다른 타입,
다른 진입점이 필요하다.

## 2. 범위

구현 위치: `src/restore/preview.rs`(+ `preview/tests.rs`). `restore/mod.rs`에는 모듈 선언 한 줄만.

| 하는 것 | 하지 않는 것 |
|---|---|
| before/target Manifest의 결정적 diff | 파일 쓰기·이동·삭제 |
| 정책 식별자 비교(일치/미상/불일치) | Manifest에 정책 ID 소급 |
| 주입된 `Fn(&str) -> Result<bool, String>` 호출 | 추적 정책 타입 import |
| 경로 글자 검사(OS별 무게) | Windows handle/TOCTOU 안전성 주장 |
| 읽기 전용 디스크 조회(Unix lstat) | 비Unix 조회를 흉내 내기 |
| 결과 세계의 대소문자·정규화·topology 겹침 | 「겹침 없음」 증명 |
| 보존 필요 자리의 **격리 제안** | 격리 실행, receipt 기록 |
| apply 후보/거절 판정과 근거 | restore 명령, 제품 허용, `Plan` 변환 |

## 3. 입력

```rust
Request { before: &Manifest, target: &Manifest,
          before_policy: Option<&str>, target_policy: Option<&str>, os: TargetOs }
preview(&request, includes: impl Fn(&str) -> Result<bool, String>, probe: &dyn Probe) -> Preview
```

- `before`는 지금 관측한 세계, `target`은 돌아가려는 세계다.
- 정책 식별자는 각 Manifest가 **어느 정책으로 관측됐는가**다. 과거 Manifest에 없으면 `None`.
  INTERFACES의 A→B 합의대로 식별자는 A가 정하며, B는 글자 둘을 견줄 뿐이다.
- `includes`는 `/` 구분 프로젝트 상대 경로를 받아 `Ok(true)` 추적 후보 / `Ok(false)` 제외 /
  `Err` 해석 거절을 돌려준다. 통합 담당자가 `|p| policy.includes_file(p)`로 연결한다.
- `TargetOs { Unix, MacOs, Windows }`는 **어느 OS 규칙으로 재는가**다. `TargetOs::host()`가
  지금 OS를 준다. 과거 macOS 경로를 Windows 규칙으로 미리 재는 데 쓴다.
- `Probe`는 읽기 전용 adapter다. `NoProbe`(조회 안 함), `LstatProbe`(Unix `lstat`/`readdir`;
  비Unix에서는 명시적 `Unsupported`).

## 4. 경로마다 적는 것

### 4.1 역할 `Disposition`

| 역할 | 조건 | 뜻 |
|---|---|---|
| `Unchanged` | 양쪽 같은 내용 | 건드리지 않는다 |
| `WouldCreate` | 목표에만 | 복원하면 만들어질 자리 |
| `WouldReplace` | 양쪽, 내용 다름 | 복원하면 바뀔 자리 |
| `CurrentOnly { proposal: QuarantineProposed }` | 현재에만, 정책 포함 | **보존 필요.** 삭제 대상 아님 |
| `Excluded { by: GitAlways, side }` | `.git` 또는 `.git/…` | 정책을 묻지 않고 언제나 제외 |
| `Excluded { by: Policy, side }` | callback `Ok(false)` | 복원이 건드리지 않는다 |

`CurrentOnly`의 `QuarantineProposed`는 §3.6a 3항의 「복구 가능한 격리 영역 + receipt」 **제안**이다.
격리 자리의 이름·형식·receipt·실제 이동은 이 제안에 없다(§7).

### 4.2 위험 `Risk`와 무게 `Severity`

위험은 OS와 무관하게 **전부** 모으고, 무게는 `severity(os)`가 정한다.

| 무게 | 뜻 | 판정 영향 |
|---|---|---|
| `Blocks` | 이 OS에서 복원 불가 | 거절 |
| `Unverified` | 판단 근거 부족 | 거절(통과로 치지 않는다) |
| `ElsewhereOnly(os)` | 이 OS는 무해, `os`에서만 막힘 | 정보성 — legacy 호환 표식 |

글자 검사(`Basis::Manifest`):

| 위험 | 검사 | Unix | macOS | Windows |
|---|---|---|---|---|
| `Nul`·`Backslash`·`EmptyComponent`·`DotComponent`·`ComponentTooLong` | NUL, `\`, 앞뒤 `/`·`//`·`//?/`, `.`/`..`, 255 B 초과 | Blocks | Blocks | Blocks |
| `WindowsDrive` | 첫 조각 `X:` | Elsewhere(Win) | Elsewhere(Win) | Blocks |
| `WindowsAlternateStream` | 조각 안 `:` | 〃 | 〃 | Blocks |
| `WindowsForbiddenChar` | `< > " \| ? *` | 〃 | 〃 | Blocks |
| `WindowsControlChar` | 0x01–0x1F | 〃 | 〃 | Blocks |
| `WindowsTrailingDotOrSpace` | 조각 끝 `.`/공백 | 〃 | 〃 | Blocks |
| `WindowsReservedName` | `CON PRN AUX NUL COM1–9 LPT1–9 (¹²³ 포함)`, 확장자 무시, 대소문자 무시 | 〃 | 〃 | Blocks |
| `WindowsComponentTooLong` | UTF-16 255 단위 초과 | 〃 | 〃 | Blocks |
| `UnverifiedNormalization` | 결합 문자(U+0300–036F 등) 존재 | Elsewhere(mac) | **Unverified** | Elsewhere(mac) |
| `CaseCollision` | 결과 세계에 `to_lowercase` 같은 다른 경로 | Elsewhere(mac) | Blocks | Blocks |
| `NormalizationCollision` | 한글 음절↔자모 알고리즘 정규화 뒤 같은 다른 경로 | Elsewhere(mac) | Blocks | Elsewhere(mac) |
| `TopologyConflict` | 파일 자리 = 다른 경로의 디렉터리 조상 | Blocks | Blocks(접기) | Blocks(접기) |
| `PolicyError`·`PolicyInconsistent`·`InternalStore` | callback Err / 같은 정책인데 목표에 제외 경로 / `.gil` | Blocks | Blocks | Blocks |

**macOS에서 유효했던 과거 경로가 Windows에서 불가능한 경우**는 `ElsewhereOnly(Windows)`로
따로 적힌다. macOS 규칙으로 재면 정보성, Windows 규칙으로 재면 거절이다.

겹침 검사의 결과 세계는 **목표 ∪ 현재**다. 현재에만 있는 파일은 제외든 보존이든 디스크에 남으므로
목표가 만들 자리와 다툴 수 있다.

디스크 조회(`Basis::FileSystem`, 조회 시점의 사실):

| `ProbeFinding` | 뜻 | 무게 |
|---|---|---|
| `SymlinkAncestor`·`SymlinkLeaf`·`AncestorNotDirectory`·`LeafNotFile` | 경로가 밖으로 향하거나 종류가 다르다 | Blocks |
| `Stale { expected, found }` | before Manifest와 디스크가 다르다 — **미리보기가 낡았다** | Blocks |
| `CaseVariantOnDisk { name }` | 같은 폴더에 대소문자 변형 | 접는 OS에서 Blocks |
| `NormalizationVariantOnDisk { name }` | 같은 폴더에 한글 음절↔자모 정규화 변형 | 정규화를 접는 OS(macOS)에서 Blocks |
| `UnidentifiedAliasOnDisk { found }` | 글자 그대로의 이름은 목록에 없는데 `lstat`이 닿았다 — 모듈이 모르는 접기 | Blocks(모든 OS) |
| `Failed(error)` | 조회 실패 | Unverified |

### 4.3 디스크 조회 차례(Wave 2 A 수정, 2026-11-02)

Wave 1 검토 R1/R2를 고친 뒤의 조회 계약이다. 세 단계 모두 **경로 이름으로** 보는 조회 시점의 사실이다.

1. **뿌리 `""`** — 실제 `Directory`로 확인되어야 자식 조회로 넘어간다. `Symlink`·`Missing`·`File`·
   `Other`는 `ProbeStatus::RootNotDirectory`, `Io` 실패는 `RootUnreadable`로 **전체 거절**이며 자식 자리를
   하나도 묻지 않는다(바깥을 가리키는 링크를 따라가지 않기 위해). `Unsupported`는 종전대로 `FileSystemNotProbed`.
   이 판정은 Manifest가 비어 있어도 `Readiness`에 들어간다. 경로별 정보성 diff는 그대로 보인다.
2. **조상** — 디렉터리가 아니면(`SymlinkAncestor`/`AncestorNotDirectory`) 그 경로를 막고 더 내려가지 않는다.
3. **잎** — 폴더 목록에 글자 그대로 있는지를 본 뒤 **항상** `lstat`도 한다. 목록에 없는데 `lstat`이 닿으면,
   찾아낸 변형(`CaseVariantOnDisk`/`NormalizationVariantOnDisk`)이 막거나, 변형을 찾지 못했으면
   `UnidentifiedAliasOnDisk`로 막는다. 목록 부재를 `Missing`으로 **대체하지 않는다**(R2).

`..`·빈 조각·`\`·NUL을 담은 경로는 글자 검사가 이미 모든 OS에서 막으므로 디스크를 묻지 않는다 —
뿌리에 이어 붙여 조회하면 프로젝트 밖에 닿을 수 있기 때문이다.

이 차례는 **Windows handle 안전성·TOCTOU 보장·apply 허용이 아니다.** 조회 뒤 변동은 `still_matches`
재관측과 apply 직전 handle 검사(§7.3)가 잡는 자리이고, 제품 restore/revisit의 `product-safety` 차단은 그대로다.
Unicode 정규화는 한글 음절↔자모 알고리즘만 판정하며, 그 밖의 결합 문자는 `UnverifiedNormalization`(미확인)으로
거절한다. 일부 문자만 알고리즘으로 판정하는 것을 「정규화 지원」이라 부르지 않는다(§7.4).

## 5. 전체 판정 `Readiness`

```text
Candidate            정책 일치 ∧ 조회함 ∧ 뿌리가 실제 디렉터리 ∧ 막는 위험 없음
Refused(Vec<Block>)  그 밖 — PolicyUnknown / PolicyMismatch / FileSystemNotProbed /
                     RootNotDirectory{found} / RootUnreadable{why} / Path{path, risk}
```

- 정책 식별자가 **없거나 다르면** 정보성 diff는 그대로 내되 `Candidate`가 되지 않는다. 출력에
  「legacy 호환 문제」로 표시한다. 과거 Snapshot에 지금 정책을 소급하면 「같은 파일이 없다」가
  삭제인지 제외인지 가를 수 없기 때문이다.
- `NoProbe`나 비Unix `LstatProbe`는 `FileSystemNotProbed`다. 글자 검사만 하고 통과시키지 않는다.
- `Candidate`는 「이 미리보기가 아는 한 막는 것이 없다」이다. **복원 허용이 아니다.** 사람의
  승인(§3.6a 4항), apply 연결, `product-safety` 해제는 모두 이 제안 밖이다.

## 6. 결정성·무효화·타입 경계

- 항목은 경로 UTF-8 바이트 오름차순이다. 겹침 목록은 정렬된다. 같은 입력은 같은 `Preview`다.
- `fingerprint()`는 두 Manifest·정책 식별자·OS·callback의 경로별 답을 묶은 SHA-256이다.
- `still_matches(&now_manifest)`는 다시 관측한 현재가 미리보기가 본 현재와 같은지 답한다.
  **관측과 적용 사이의 변동**은 글자 검사가 아니라 이 재관측 비교와 적용 직전 handle 검사가 잡는다.
  통합 시 권고: 승인된 미리보기의 `fingerprint`와 before digest를 승인 기록에 함께 두고, apply
  직전에 `observe()` → `still_matches()`가 거짓이면 그 승인을 무효로 하고 다시 미리보기를 만든다.
- `Preview`에서 `Plan`/`Step`으로 가는 변환이 없다. 보관 서수·세계 번호·Delete가 없다.
  시험 `a_preview_carries_no_backup_ordinal_no_delete_and_no_world_number`가 Debug 출력에서 이를 잰다.

## 7. 통합 담당자에게 남기는 연결

이 제안이 구현하지 **않은** 것이며, 각각 별도 결정이다.

1. **정책 연결** — A의 `includes_file`/`policy_id`를 `|p| policy.includes_file(p)`와 식별자 둘로
   넣는다. 과거 Snapshot의 식별자는 저장 provenance가 서야 생긴다(Storage Model 변경, 통합 소유).
2. **격리 영역** — `QuarantineProposed`의 실제 자리·receipt·복구 명령. 저장 transaction과 함께
   정해야 하므로 B가 만들지 않았다.
3. **Windows 읽기 전용 조회** — `windows_fs.rs`의 handle 기반 열기로 `Probe`를 구현하면
   `LstatProbe`의 비Unix `Unsupported`를 대체할 수 있다. 그래도 조회 시점 ≠ 적용 시점이다.
4. **Unicode 정규화 표** — 라틴 결합 문자의 NFC/NFD 동등성은 의존성(예: `unicode-normalization`)
   또는 표가 필요하다. Cargo는 A/통합 소유라 B는 한글 알고리즘 + 「미확인」으로 멈췄다.
5. **대소문자 접기** — `str::to_lowercase`는 NTFS upcase 표·APFS 접기의 근사다. 찾아내는 데 쓰고
   「없다」를 증명하는 데 쓰지 않는다. 디스크 조회의 `CaseVariantOnDisk`가 실제 변형을 본다.
6. **표면** — `gil status`/Monitor/MCP에 미리보기를 보여 주는 자리와 사람 승인 wire(§3.6a 4항).
   이 제안은 `Display`로 사람이 읽는 꼴만 제공하고 명령을 만들지 않는다.

## 8. 시험 범위(2026-11-02)

`src/restore/preview/tests.rs`(Wave 2 A 추가, 2026-11-02): 뿌리 종류별(링크·없음·파일·그 밖·Io) 전체 거절과
자식 미조회(대본 adapter + 실제 Unix 디스크, 빈 Manifest 포함), 모르는 별칭의 미확인 거절, 목록의 한글 정규화
변형 OS별 무게, 뿌리 탈출 글자의 조회 생략, 검토 R1(심볼릭 링크 뿌리) 재현, 검토 R2(실제 macOS 볼륨의 NFD 파일 vs
NFC target — 미추적 파일 내용과 폴더 bytes 보존 확인) 재현. R2 재현은 `cfg(target_os = "macos")`이며 정규화를 접는
볼륨을 전제한다.

기존: 바이트 동일성(Unix, `.gil`·`.git` 포함 전체 폴더), 현재에만 있는
파일 보존, `.git` 제외와 callback 미호출, 정책 제외/불일치/미상/오류, 내부 저장소, unsafe 글자,
Windows 전용 위험의 OS별 무게, 한글·ASCII 무위험, 대소문자/한글 정규화/라틴 결합 문자/topology
겹침, 결정성·지문·무효화, 디스크 조회(stale·symlink·대소문자 변형), `Plan`과의 타입 분리.

이 시험 전부가 통과해도 **안전 restore 전체 완료가 아니다.** 제품 차단은 그대로이며 실제 Mac/Windows
설치본 인수는 통합 후 한 버전으로 한다.
