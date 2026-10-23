# GIL Runtime Update — 최소 구현 설계안 v0.1 (U2 CI 결과 기준)

2026-10-23. [계약](GIL_Runtime_Update_Contract_v0.1.md)·[U1 설계 결정안](GIL_Runtime_Update_Design_v0.1.md)의 구현 전 최소 설계.
**설계·코드 조사만. 제품 미구현, commit·게시·PC 실험 없음.** 정상 설치 경로에서 사용자에게 긴 코드·PowerShell·수동 권한 변경을 요구하지 않는다.

목표: 기존 GIL cache를 삭제하거나 실행 중 바이너리를 덮어쓰지 않고, 고정 실행기가 **승인된 새 Core를 다음 연결부터** 실행한다.

## 0. 근거 — 확인한 사실과 가정의 분리

| # | 사실(확인) | 출처 |
|---|---|---|
| F1 | 현재 Plugin manifest는 `mcpServers.gil-companion = {command: "./core/<os-arch>/gil", args: ["mcp","--serve"], cwd: "."}`. Windows 후보도 `candidate.mjs`가 같은 형식으로 생성 | `plugins/gil-companion-prototype/.codex-plugin/plugin.json`, `distribution/windows/candidate.mjs:149` |
| F2 | Host 설치 cache는 `<CODEX_HOME>/plugins/cache/<marketplace>/<plugin>/<version>/`. `plugin add`는 새 버전 디렉터리 생성 뒤 옛 항목 제거, `plugin remove`는 cache 삭제. 서버가 실행 중일 때 삭제가 거부된 현장 사례(os error 5) | 계약 E 장부, Mac 격리 관찰 H2~H6, 현장 결과 2 |
| F3 | Codex 앱은 MCP 서버를 최소 환경으로 띄운다(Mac 실측 8개 변수, `HOME` 포함). Windows 서버 환경의 `LOCALAPPDATA`/`USERPROFILE` 유무는 미확인 | 48차 조사 |
| F4 | GIL은 이미 사용자별 지속 저장소를 쓴다: Monitor 바인딩 `HOME/Library/Application Support/GIL/monitor-bindings-v1`(Mac), `LOCALAPPDATA\GIL\monitor-bindings-v1`(Windows), 환경 변수 없으면 저장소 없음(`unreadable`) | `src/mcp/bindings.rs:96-107` |
| F5 | U2 probe: Windows x64·ARM64·macOS runner에서 배타 생성·fsync·**파일 rename 교체**(`fs.renameSync`)·별도 프로세스의 dev/ino·marker 일치·새 프로세스 선택값 읽기·자식 종료 확인 통과(관리자 runner 토큰) | run 39845527374, 계약 E6 |
| F6 | 제품 Core에 Windows용 원시 연산이 있다: 핀 고정 핸들 아래 `FileRenameInfoEx`(POSIX, replace) rename, 배타 생성(`CREATE_NEW`), 디렉터리 `FlushFileBuffers`, 핸들 identity, 위치 분류(`classify_location`) | `src/windows_fs.rs` |
| F7 | Core는 `--gil-agent-descriptor`로 schema/protocol/action surface/storage 범위/os/arch를 자기 보고한다 | `src/agent.rs` |
| F8 | 사용자 PC(E3)에서 **디렉터리** rename EPERM이 관측됐다; **파일** rename은 미관측 | 계약 E3 |

| # | 가정(미확인) | 확인 방법 |
|---|---|---|
| A1 | Windows의 Codex 서버 환경에 `LOCALAPPDATA`가 있다 | 환경 변수에 의존하지 않고 토큰 기반 known folder API로 해결(§3); 실제 값은 `gil_runtime_status`가 보고 |
| A2 | Host가 Plugin cache 밖 경로의 실행 파일을 자식으로 실행하는 것을 막지 않는다(Codex 정책, AppLocker/WDAC) | prerelease로 실제 Windows Codex에서만 확인 |
| A3 | Agent 셸이 작업 폴더 밖 쓰기에 제한될 수 있다 | 저장소 쓰기는 Agent 셸이 아니라 **서버 프로세스**(도구)가 수행하도록 설계해 의존을 없앰 |
| A4 | Core 프로세스가 네트워크 다운로드를 할 수 있다 | 의존하지 않음: 다운로드는 Agent가 작업 폴더에 받고(현장에서 동작 확인), Core는 로컬 파일을 검증·복사 |
| A5 | 사용자 PC에서 파일 rename 교체가 된다(F8의 디렉터리 EPERM 환경) | `gil_runtime_status`의 capability probe 결과를 prerelease로 확인; 안 되면 "이 환경에서 업데이트 차단" |

## 1. 최초 설치에 포함할 구성 (고정 실행기 + 기본 Core)

- **같은 바이너리 두 역할**: 별도 launcher 실행 파일을 추가하지 않고 번들 `gil`(`gil.exe`)에 서브커맨드 `gil launch mcp` 를 둔다. manifest만 `args: ["launch","mcp"]`로 바뀐다(F1). 이유: 패키징·고지·수명 문제가 있는 실행 파일을 하나로 유지하고, bootstrap(선택 없음·저장소 없음)일 때는 **자기 자신을 in-process로 `mcp --serve`** 하여 현재 동작과 완전히 같다.
- **실행기 동작**(연결 시작 1회): 저장소 결정(§3) → 저장소 상태 판정(아래) → `selection.json` 읽기·검증 → 선택된 버전의 `complete` 마커·파일 digest 재검증(핀 고정 핸들 아래) → 그 Core를 `mcp --serve`로 spawn, stdio 전달, stdin EOF 전달, 자식 종료 대기·종료 코드 전달 → 진단은 stderr만. 임의 구판 탐색·"가장 새 디렉터리" 선택은 하지 않는다.
- **저장소 상태와 실행 판정(2026-10-23 2차 보정, §1.1 표가 정본)**: 번들 Core(bootstrap) 실행은 **선택 이력이 없는 상태**(`first_use`·`initializing`·`prepared`)에서만 허용한다. 선택 이력이 있는 저장소(`store.json.last_generation ≥ 1`)에서 selection이 없거나 손상됐거나 가리키는 버전이 없으면 `damaged`, 저장소·선택·버전 파일 접근 거부나 보호 검사 실패면 `blocked`이며, 두 상태에서는 일반 GIL Core(번들·선택판 모두)를 실행하지 않고 **진단 전용 상태**로 뜬다(§1.2). 자동 rollback·자동 복구·조용한 강등은 없다. 임의 구판 탐색·"가장 새 디렉터리" 선택은 하지 않는다.
- **최초 사용과 소실의 구분**: `store.json.last_generation`(마지막으로 **성공한 select**의 generation; 0이면 선택 이력 없음)으로 구분한다. 루트·`store.json`은 첫 prepare의 **시작 단계**에서 만들어지므로(§1.1 기록 순서) "루트 있음"만으로는 소실이 아니다. 한계(명시): 저장소 루트 전체를 지우면 최초 사용과 구별할 수 없다(의도적 초기화와 동치로 본다).
- 번들 Core = bootstrap. 번들 파일은 업데이트로 덮어쓰지 않는다(수명 문제 회피의 핵심).

### 1.1 저장소 상태 전이 (정본, 2026-10-23 3차 보정)
정상 전이: **최초 사용 → 초기화/준비 중 → 준비 완료·미선택 → 선택 완료**. 중단·손상·접근 거부는 별도 상태다.
번들 Core(bootstrap)는 **유효한 초기화 기록(`store.json`)으로 "선택을 시도한 적 없음"이 확인되는 경우에만** 허용한다. 불명확한 상태는 모두 진단 전용이다.

저장소 파일: `store.json`(초기화 기록: schema, created, owner, phase, `last_generation`=마지막으로 **공개가 완료된** selection의 generation), `intent.json`(선택 의도: generation, version, sha256, approved_at; selection 공개 **전에** 내구화하고 이력 갱신 **뒤에** 제거), `selection.json`(공개된 선택), `versions/<v>/complete`.

| 상태 | 판정 조건(순서대로 평가; 먼저 맞는 행) | 실행 | 노출 도구 |
|---|---|---|---|
| `blocked` | 루트·`store.json`·`intent.json`·`selection.json`·버전 파일 접근 거부, 또는 §3 보호 검사 실패 | **없음**(진단 전용) | `status`·`help`만 |
| `first_use` | 저장소 루트 없음 | 번들 Core | 일반 + runtime |
| `damaged` | 루트 있음이고 `store.json` 없음 또는 파싱·schema·소유자 불일치 — **selection 유무와 무관**(불명확 상태) | **없음**(진단 전용) | §1.2 |
| `damaged` | `intent.json` 손상(파싱·schema 불일치) | **없음**(진단 전용) | §1.2 |
| `damaged` | selection 무효(파싱·digest·버전 부재·`complete` 없음) | **없음**(진단 전용) | §1.2 |
| `selected` | selection 유효이고 **intent 없음**이고 `selection.generation == store.json.last_generation ≥ 1` | 선택된 Core | 일반 + runtime |
| `selected`(+`interrupted`) | selection 유효이고 **유효 intent**가 generation·대상 버전·전체 digest에서 selection과 일치하며, 정상 전이 관계 `intent.generation == last_generation + 1`(공개됨·이력 갱신 전, `history_lagging`) 또는 `intent.generation == last_generation`(이력 갱신됨·intent 정리 전) | 선택된 Core | 일반 + runtime(흔적 보고; 다음 도구 호출이 잠금 아래 이력·intent 정리) |
| `damaged` | selection 유효이나 위 두 행에 맞지 않음: intent 없음인데 `generation ≠ last_generation`(설명되지 않는 차이; `last_generation = 0`에 selection이 있는 경우 포함 — 유효 intent 일치 사례는 위 행의 예외), 또는 intent가 있으나 generation·버전·digest 불일치, 또는 intent generation이 `last_generation`·`last_generation+1` 어느 쪽도 아님 | **없음**(진단 전용) | §1.2 |
| `damaged` | selection 없음이고 `intent.json` 존재(선택 의도 기록 뒤 공개되지 않음; `last_generation` 값과 무관) | **없음**(진단 전용) | §1.2 |
| `damaged` | selection 없음이고 intent 없음이고 `store.json.last_generation ≥ 1`(과거 선택 소실) | **없음**(진단 전용) | §1.2 |
| `initializing` | `store.json` 유효, `phase="initializing"`, `last_generation=0`, `intent.json` 없음, `selection.json` 없음 | 번들 Core | 일반 + runtime(`status`가 미완성 준비본 보고) |
| `prepared` | `store.json` 유효, `phase="prepared"`, `last_generation=0`, `intent.json` 없음, `selection.json` 없음 | 번들 Core | 일반 + runtime |
| `interrupted`(수식) | 위 어느 기본 상태에 `*.new`·`complete` 없는 버전 디렉터리·이력 지연 같은 중단 흔적이 더해진 것 | 기본 상태의 실행 | 기본 상태 + `status`에 흔적 목록 |

**기록·갱신 순서**(각 쓰기는 배타 생성 또는 파일 rename 교체 + fsync + 디렉터리 flush):
1. prepare: 루트 배타 생성(보호 DACL) → `store.json{phase:"initializing", last_generation:0}` 배타 생성 → `versions/<v>/` 배타 생성 → 복사·fsync → 재hash → `complete` → `store.json.new{phase:"prepared"}`→`store.json` rename 교체.
2. select(잠금 아래): selection·store.json·intent 재읽기 → generation 검증 → **(a) `intent.json.new{generation:g+1, version, sha256, approved_at}`→`intent.json` rename 교체·fsync·flush(선택 의도 내구화)** → (b) `selection.json.new`→`selection.json` rename 교체·flush(공개) → (c) `store.json.new{last_generation:g+1}`→`store.json` rename 교체·flush(이력 갱신) → (d) `intent.json` 삭제·flush → 잠금 해제.
3. generation 경계: selection·intent의 generation 0은 어떤 경우에도 선택 상태가 아니다(`damaged`); `store.last_generation = 0`만 "선택 시도 없음"을 뜻한다. `last_generation + 1` 비교는 오버플로 없는 checked 연산으로 하며 `u64::MAX` 이력은 다음 전이가 없다(MAX→0 회귀는 `damaged`). Selected는 항상 generation ≥ 1이다.
4. 실행기 판정은 위 표 순서대로이며 **순수 판정기 `src/runtime_state.rs::judge`가 정본 구현**이다(파일 접근·실행 없음; 호출자가 관측값을 넘긴다). 보수 규칙: (a) 뒤 어느 지점에서 중단돼도, 유효 selection이 intent와 generation·버전·digest에서 일치하고 intent가 `last_generation+1`(공개 뒤 이력 갱신 전) 또는 `last_generation`(이력 갱신 뒤 정리 전)일 때만 `selected`(+`interrupted`)이고, 그 밖(의도만 있음·selection 소실·불일치·설명되지 않는 generation 차이)은 `damaged`다. 이력 갱신·intent 정리는 다음 runtime 도구 호출이 **잠금 아래**에서 완료한다(Core 실행 자체는 변경하지 않음).
5. 따라서 **정상 prepare 뒤 select 없이 재연결하면 `prepared`**(번들 Core)이고, **선택 의도가 기록된 뒤 유효 selection이 없으면 어떤 경우에도 번들 Core를 자동 실행하지 않는다**(보호 검사 통과 + 명시 승인 뒤 §1.2로 복구).
6. 한계(유지): 저장소 루트 전체 삭제는 최초 사용과 구별할 수 없다.

### 1.2 진단 전용 상태의 도구 표면과 복구 경로
| 상태 | 노출 도구 | 복구 |
|---|---|---|
| `damaged`(보호 검사 **통과**) | `gil_runtime_status`, `gil_help`, `gil_runtime_prepare`, `gil_runtime_select`(둘 다 명시적 승인 필요) — Project 도구 없음 | 사용자가 승인하면 `complete`인 기존 버전을 재선택(`select`)하거나 새 후보를 준비 뒤 선택. `store.json` 부재·손상이면 승인된 `select`가 잠금 아래에서 `store.json`을 재생성(초기화 기록 복구)한 뒤 공개한다. 미완성 intent는 승인된 select가 새 intent로 덮는다. 성공하면 다음 연결부터 `selected` |
| `blocked`(접근 거부·보호 검사 실패) | `gil_runtime_status`, `gil_help`만 | **복구 도구 없음**. 재준비·재선택으로 보호 검사를 우회하지 않는다. 원인(경로·거부된 권한·실패한 검사 항목)을 보고하고 멈춘다. 사용자 폴더 ACL 자동 수정·권한 확대 없음 |
Project 도구는 두 상태 모두에서 노출하지 않는다. 복구 도구는 새 기능이 아니라 `selected` 상태의 같은 도구를 같은 게이트(§4.1, 잠금, generation)로 제공하는 것이며, `blocked`에서는 제공하지 않는다.

## 2. 버전별 Core와 선택 기록의 저장소 후보

| 후보 | 위치 | 판정 |
|---|---|---|
| A (권장) | 사용자별 비동기화 앱 데이터: Windows `<LocalAppData>\GIL\runtime-v1\`, macOS `~/Library/Application Support/GIL/runtime-v1/` | F4와 같은 관례. Host cache 밖이라 제거/교체와 분리됨. 사용자 Project와 무관 |
| B | Plugin cache 안의 GIL 하위 폴더 | **기각**: Host 소유, 제거 시 삭제, 바로 그 잠금 문제의 위치(계약 §1) |
| C | `~/.gil-runtime` 등 프로필 루트 | 관례 밖, 사용자 눈에 띔. 알려진 폴더 리디렉션 영향은 없으나 A보다 이점 없음 |
| D | Host가 제공하는 지속 디렉터리 | 문서화된 것 없음(미확인) → 채택 불가 |

**Agent와 실행기의 동일 저장소 접근**: Agent는 저장소를 직접 읽고 쓰지 않는다. 준비·승인·선택은 **실행 중인 Core(서버 프로세스)의 MCP 도구**가 수행하므로 실행기와 같은 계정·같은 경로 결정 코드를 쓴다(A3 해소). 동일성은 경로 문자열이 아니라 실행기가 시작 시 기록하는 저장소 identity(dev/ino + marker)와 비교한다(F5 방식).

## 3. 후보 A의 경로 결정·토큰 경계·권한·지속성

| 항목 | Windows | macOS | 확인/가정 |
|---|---|---|---|
| 경로 결정 | `SHGetKnownFolderPath(FOLDERID_LocalAppData)`(프로세스 토큰 기준; 환경 변수 불사용) → `GIL\runtime-v1` | `HOME`(서버 환경에 존재 확인됨, F3) 또는 `getpwuid` → `Library/Application Support/GIL/runtime-v1` | Windows API 경로는 F3/A1을 우회하는 설계; 실제 값은 prerelease 확인 |
| 사용자/토큰 경계 | 저장소 루트 생성 시 소유자=현재 토큰 사용자 확인, 다른 소유자·reparse·동기화 루트 아래(`classify_location` 재사용)면 **거절**(권한 확대·다른 위치 순회 없음) | 소유자 uid 확인, symlink 거절 | 토큰이 대화 사용자와 같다는 보장은 하지 않음(location_check와 같은 표현) |
| 읽기/쓰기 권한(2026-10-23 2차 보정) | 두 검사를 분리한다. **(a) 현재 토큰 접근 가능성**: `AccessCheck`(프로세스 토큰, 객체 SD)로 루트·`versions/<v>/`·Core 파일·`selection.json`·`selection.lock`에 필요한 권한(읽기; 쓰기 경로에서는 쓰기·삭제; Core 파일은 FILE_EXECUTE)이 있는지. **(b) 허용되지 않은 주체의 권한 부재**: AccessCheck로는 증명하지 않고 **DACL 구조 검증**으로 한다 — 루트는 `.gil`과 같은 `private_dir` 기준(소유자=토큰 사용자, SE_DACL_PROTECTED, ACE 정확히 2개: 사용자·SYSTEM FA, OICI 상속 플래그), 그 아래 디렉터리·파일은 `inherited_only` 기준(소유자=사용자 또는 토큰 기본 소유자, DACL 존재, 비보호, 상속된 allow ACE 정확히 2개 FA 사용자·SYSTEM). 기존 `src/windows_fs.rs`의 검증 코드를 **그대로 재사용**하며 재사용 범위는 "허용 SID 2개·ACE 수·상속 플래그·보호 비트"다. 기본 상속 ACL·읽기 전용 속성·AccessCheck 결과만으로 (b)를 주장하지 않는다. 루트는 GIL이 새로 만들 때만 보호 DACL로 생성하고 기존 폴더 ACL은 수정하지 않는다 | **(a)** `access(2)`로 읽기/쓰기/실행; **(b)** 모드·소유자: 루트 0700, 데이터 파일(`store.json`·`selection.json`·receipt) 0600, **실행 파일(Core) 0500**(소유자 r-x; 쓰기 비트 없음), 소유자 uid=프로세스 euid, 확장 ACL(`com.apple.acl` xattr) **없음** 요구 | 어느 하나 불충족이면 `blocked`; 권한 확대·ACL/모드 자동 수정·다른 위치 순회 없음 |
| 조상 경로(2차 보정) | 루트까지의 각 구성 요소를 reparse 거부로 열어 reparse 아님·디렉터리임을 확인(`classify_location`과 같은 방식). 조상의 ACL은 검증하지 않는다(사용자 프로필 아래의 OS 기본 ACL을 전제; 한계) | 각 조상이 symlink 아님, 소유자 root 또는 사용자, group/other 쓰기 비트 없음. 조상의 확장 ACL은 검사하지 않는다(한계) | 조상 위반 시 `blocked` |
| 지속성 | 동기화 대상 아님(LocalAppData는 OneDrive KFM 대상 아님) | iCloud Desktop/Documents와 무관 | 레지스트리 동기화 루트 검사로 재확인 |
| 접근 거부 시 | `blocked`(진단 전용, 복구 도구 없음, §1.2) | 동일 | 우회 없음 |

### 3.1 보호 조건과 방어 범위 (2026-10-23 보정)
| 대상 | 보호 조건 | 검사 시점 |
|---|---|---|
| 저장소 루트 | 소유자=토큰 사용자, 보호 DACL(상속 차단), 실효 쓰기는 사용자·SYSTEM만, reparse 아님 | 실행기 시작, 모든 runtime 도구 호출 |
| `versions/<v>/` 와 Core 파일 | 같은 상속 ACE만(`.gil` 하위 기준과 동일), reparse 아님, **hardlink 수 1**(`nNumberOfLinks`), Core 파일은 현재 토큰에 FILE_EXECUTE(Windows)/소유자 실행 비트(macOS) 있음, `complete` 마커 뒤 읽기 전용 속성(방어 보조, 보안 근거 아님); `complete`는 **존재 마커**(내용 미해석)이며 데이터 파일과 같은 보호 조건(상속 ACE만/0600, reparse 아님, hardlink 1) | prepare 완료 시, 실행기 시작 시 |
| `selection.json` | 같은 상속 ACE, reparse 아님, hardlink 1, 내용 schema·generation·버전 digest 일치 | select 전후, 실행기 시작 |
| 검증 후 교체 경쟁(TOCTOU) | 실행기는 Core 파일을 **쓰기·삭제 공유 거부 핸들**로 열어 hash 한 뒤 그 핸들을 자식 시작까지 유지한다(시작 후에는 이미지 매핑이 보호). selection은 잠금 아래에서만 바뀐다 | 실행기 |
| 한계(명시) | 같은 사용자 권한의 악성 코드, 관리자의 ACL 변경, 볼륨 수준 접근은 방어 범위 밖. 하드링크 검사는 NTFS 링크 수에 의존. 보호 DACL이 외부 주체에 의해 바뀌는 현상(현장 결과 1·3)은 탐지·거절(`blocked`)하되 원인은 미확정 | — |

## 4. 최소 상태 흐름

```
[Agent] 후보 archive 다운로드(작업 폴더)  ──►  gil_runtime_prepare(path, expected_sha256)
   ▼ Core: digest 검증 → archive 안 receipt·descriptor 검사(§5) → versions/<ver>-<sha12>/ 배타 생성
          → 파일 복사+fsync → 전체 재hash → complete 마커(마지막) → 디렉터리 flush
   state: prepared(ver)                                   (실패: 부분 디렉터리는 complete 없음 → 무시·보고)
[사용자] 채팅에서 "ver, 전체 sha256, 변경 요약"을 보고 승인   ──►  gil_runtime_select(ver, sha256, previous_generation)
   ▼ Core: `selection.lock` OS 잠금 획득(배타; 실패=무변경 거절) → 잠금 아래 selection **재읽기** → generation 일치 확인
          (불일치=충돌 거절) → selection.json.new 배타 생성·fsync → selection.json 으로 **파일 rename 교체**
          (Windows FileRenameInfoEx POSIX / rename) → 디렉터리 flush → 잠금 해제
   state: selected(ver), applies_to: next_connection      기존 연결은 그대로
[다음 연결] 실행기가 selection 읽기 → 검증 → 새 Core 실행 → gil_runtime_status 가 running=ver 보고
```

- **중단**: prepare 중단 → `complete` 없는 디렉터리(다음 prepare가 같은 ver를 요청하면 EEXIST → "미완성 준비본 존재" 보고, 삭제는 승인 뒤 도구가 수행). select 중단은 §1.1 기록 순서의 지점에 따라: intent 기록 전 → 이전 상태 그대로; intent 기록 뒤 selection 공개 전 → `damaged`(번들 Core 없음); selection 공개 뒤 이력 갱신 전 → `selected`+`interrupted`; 이력 갱신 뒤 intent 정리 전 → `selected`+`interrupted`. rename은 파일 단위 원자 교체 원시 연산에 맡기되 **전원 차단 내구성은 주장하지 않음**(fsync·dir flush는 수행).
- **손상**: selection 파싱 실패/digest 불일치/가리키는 버전의 `complete` 없음이고 `store.json.last_generation ≥ 1` → `damaged`(§1.1): 일반 GIL Core를 실행하지 않고 진단 전용으로 뜬다. 보호 검사가 통과했을 때만 승인 아래 재선택·재준비로 복구한다(§1.2). 자동 복구·자동 rollback 없음.
- **동시 실행(2026-10-23 보정)**: 연결 2개 동시 시작은 읽기뿐이라 무관. prepare 2개는 디렉터리 배타 생성으로 한쪽 EEXIST. **select는 직렬화한다**: `selection.lock`에 대한 OS 잠금(Windows `LockFileEx` 배타 / POSIX `flock(LOCK_EX|LOCK_NB)`) 아래에서 selection 재읽기 → generation 검증 → `.new` 쓰기 → rename 교체 → flush까지 수행한다. 잠금 획득 실패(다른 writer 보유)는 **무변경 거절**이며 대기·재시도·stale lock 강제 회수를 하지 않는다. OS 잠금은 보유 프로세스 종료 시 OS가 해제하므로 pid 기록형 lock 파일의 stale 문제를 만들지 않는다(lock 파일 자체는 비워 두고 내용으로 판단하지 않는다). 같은 generation을 든 두 writer가 경쟁하면 잠금을 먼저 얻은 쪽만 성공하고 다른 쪽은 재읽기에서 generation 불일치로 거절된다(인수시험 AT-S3).
- **옛 버전**: 삭제하지 않는다(GC 없음). 실행 중 Core의 파일은 건드리지 않는다.

### 4.1 후보 실행 전 신뢰 게이트 (2026-10-23 보정)
descriptor 조회(`--gil-agent-descriptor`)도 **실행**이므로 게이트 통과 전에는 후보 바이너리를 어떤 인자로도 실행하지 않는다. archive 안의 receipt(`checks.json`)와 호출자가 건넨 hash만으로는 publisher 신뢰가 성립하지 않는다(둘 다 후보와 같은 출처이거나 Agent 입력).
unsigned 개발 채널의 게이트(모두 충족해야 함):
1. **고정 출처**: bootstrap에 내장된 출처 패턴(`https://github.com/hyun06000/gil/releases/download/<tag>/<file>`; tag 패턴 `windows-dev-visual-*`/`v0.*`)과 Agent가 보고한 다운로드 URL·tag가 일치해야 한다. URL은 Agent 보고값이므로 **그 자체로 신뢰가 아니라 기록**이다.
2. **전체 digest**: archive 전체 sha256(64자)이 (a) 사용자가 채팅에서 본 값, (b) `SHA256SUMS`(별도 자산)에 적힌 값, (c) archive 안 receipt의 `archive_sha256`과 모두 일치해야 한다. (b)·(c)는 (a)와 같은 출처에서 왔을 수 있으므로 "일관성 검사"이지 인증이 아니다.
3. **사용자 승인 결합**: `gil_runtime_prepare(path, sha256)` 호출의 `sha256`은 사용자가 승인한 전체 digest이며, 도구는 승인 문맥(버전·digest·시각·이전 generation)을 `approvals.log`에 추가 기록한다. 인간이 실제로 승인했는지는 Core가 검증할 수 없다(Agent 행동 검수 대상, W12-6과 같은 성격).
4. **실행 전 정적 검사**: 멤버가 정확히 8개 일반 파일, 경로 탈출 없음, 크기 상한, PE/Mach-O 헤더와 machine, import 테이블이 시스템 DLL allowlist(`candidate.mjs`와 동일 목록)뿐, 고지 파일 존재. 이 검사는 바이너리를 실행하지 않는다.
5. 1~4 뒤에만 후보를 `--gil-agent-descriptor`·`--gil-tool-catalog`로 실행한다(인자 고정, stdin 닫힘, 제한 시간, 출력 크기 상한, 작업 디렉터리=버전 디렉터리). 이것이 **첫 실행**이며, 실패하면 `complete`를 쓰지 않는다.
**명시**: 이 게이트는 출처 고정·일관성·승인 기록이지 publisher 인증이 아니다. 실제 신뢰는 서명 채널(예: bootstrap 내장 공개키로 `SHA256SUMS` 서명 검증)이 생겨야 성립하며, 그 전까지 unsigned 채널의 버전별 사용자 승인 정책을 유지한다.

## 5. 호환 범위 — Core만 교체해도 되는 조건

Core-only 교체는 다음이 **모두** 같을 때만 허용한다(descriptor v2로 Core가 자기 보고; §4.1 게이트 통과 뒤 실행기가 후보 Core를 `--gil-agent-descriptor`로 실행해 읽음):
1. `protocol`, `action_surface`, `storage_format` 범위, `os`, `arch`가 번들 bootstrap과 동일.
2. **도구 카탈로그 digest**(tools/list의 이름·inputSchema·annotations 정규화 sha256; 새 플래그 `--gil-tool-catalog`)가 동일 — Skill이 아는 도구 표면이 변하지 않음.
3. Monitor View/NodeDetail schema 버전 동일(descriptor v2에 추가).
4. Skill은 Plugin에 있으므로 Core-only 경로로 바뀌지 않는다 → Skill 변경이 필요한 Core는 **Core-only 불가**, 공식 Plugin 교체.
5. 저장 형식 변경·이관이 필요한 Core는 불가(storage migration은 별도 계약).
내장 Monitor UI bytes 변경은 허용(Core가 제공)하되 View schema는 3에 묶인다. hash 일치만으로 의미 호환을 주장하지 않는다. **합성 두 버전 시험**(CI, fixture 바이너리로 선택·실행·보존 기계 검증)과 **실제 GIL 두 버전의 의미·저장 호환 시험**(U3: 합성 Project에서 Core A로 쓴 기록을 Core B가 읽고 쓰기, 이미지·잠금·Monitor View, Skill 흐름)은 서로 다른 시험이며 전자가 후자를 대신하지 않는다.

## 6. 최초 도입 vs 이후 업데이트

- **최초 도입**: 실행기 서브커맨드가 든 Plugin을 **공식 Host 경로**로 설치/교체한다. 기존 direct-Core 설치본을 교체할 때는 지금의 cache 삭제 거부 문제가 그대로 남는다(현장 결과 2; D안/개발 우회로 별도). 이 설계는 그 문제를 풀지 않는다.
- **이후 Core 업데이트**: Host 조작 없이 §4 흐름만으로 다음 연결에 적용. 사용자 동작은 승인 한 번 + 새 대화(필요 시 앱 재실행; 재접속 시점은 prerelease 확인 항목).
- **미검증 유지(2026-10-23 보정)**: 저장소 쓰기를 Agent 셸 대신 서버 도구가 수행하더라도, 서버 프로세스의 실제 접근 권한(Codex 제한 토큰), 다운로드 파일(작업 폴더)을 서버가 읽을 수 있는지, 저장소의 지속성(앱 갱신·재부팅·Host 정책 후)은 **미검증**이다. prerelease 실측 전에는 가정으로 둔다.
- **실행기 자체의 교체**: 미해결로 남긴다. 번들 바이너리는 Host Plugin 교체로만 바뀌며, 그때 같은 수명 문제가 생길 수 있다.

## 7. 검증 분담

| Mac·Windows CI에서 확인 | 최종 prerelease로 실제 Windows Codex에서만 확인 |
|---|---|
| 저장소 경로 결정(API)·소유자·동기화 루트 거절·접근 거부 시 `blocked` 진단 전용 | Codex 서버 환경에서 실제 결정된 경로와 접근 가능 여부(A1), Host가 cache 밖 Core 실행을 허용하는지(A2) |
| prepare: digest 불일치 거절, 부분 디렉터리 무시, 재hash, complete 마커 순서 | 사용자 PC에서 파일 rename 교체 가능 여부(A5) |
| select: generation 충돌 거절, `.new` 배타 생성, rename 교체, 중단 잔존물 처리 | 새 연결 적용 시점(새 대화 vs 앱 재실행), 기존 연결 고정 |
| launch: §1.1 상태표 전부(first_use/initializing/prepared → 번들, selected → 선택판, damaged/blocked → 진단 전용), 자식 stdio 전달·EOF·종료 코드, 고아 없음 | 실제 Agent가 승인 전에 select를 부르지 않음(Skill 행동) |
| 합성 두 버전(Core A→B) 쌍 시험: descriptor·카탈로그 동일 시 허용, 다르면 거절; 옛 bytes 보존 | 비개발자 한 문장 요청의 완료 경로 |

## 8. 권장 결정 (하나, 2026-10-23 보정)

번들 `gil` 바이너리에 `launch mcp` 서브커맨드와 `gil_runtime_status / gil_runtime_prepare / gil_runtime_select` 도구를 추가한다. 저장소는 후보 A(토큰 기반 LocalAppData / HOME 아래 `GIL/runtime-v1`)를 GIL이 소유자 전용 보호 DACL로 **새로 만들고**, 매 사용 시 실효 접근 검사로 확인한다. 번들 Core는 선택 이력이 없는 상태(`first_use`·`initializing`·`prepared`)에서만 실행하고, 선택 이력 뒤의 소실·손상·부재는 `damaged`, 접근 거부·보호 검사 실패는 `blocked`로 **진단 전용 상태**로 뜬다(Project 도구 없음, 자동 rollback 없음). `damaged`는 보호 검사 통과 시 승인 아래 같은 prepare/select 도구로 복구하고, `blocked`는 복구 도구 없이 보고만 한다. select는 `selection.lock` OS 잠금 아래 재읽기→generation 검증→파일 rename 교체로 직렬화하고 잠금 실패는 무변경 거절이다. 후보는 고정 출처·전체 digest 3중 일치·사용자 승인 기록·정적 검사를 모두 통과한 뒤에만 descriptor 실행을 허용하며, 이는 publisher 인증이 아님을 명시한다. Core-only 호환은 descriptor v2+카탈로그 digest+View schema 동일로 제한한다. Agent는 다운로드만 하고 저장소는 건드리지 않는다. 옛 버전은 지우지 않는다. 최초 도입의 공식 Plugin 교체 문제와 실행기 자체 교체는 미해결로 유지한다.

## 8.1 인수시험 목록
| ID | 시험 | 어디서 |
|---|---|---|
| AT-B1 | 저장소 루트 없음 → 번들 Core 실행, `runtime_state: first_use` | CI |
| AT-B2 | `last_generation ≥ 1` + selection 없음/손상/digest 불일치/버전 부재/`complete` 없음 → `damaged` 진단 전용(Project 도구 미노출), 번들·선택판 Core 모두 미실행, 자동 rollback 없음 | CI |
| AT-B2a | **기존 루트에서 `store.json`과 `selection.json` 모두 부재 또는 손상** → `damaged`(진단 전용), 번들 Core 미실행; `store.json` 손상 + selection 유효도 `damaged` | CI |
| AT-B2b | `last_generation = 0`인데 selection 존재: intent 없음 → `damaged`(설명 없음); **유효 intent(generation 1, 같은 버전·digest)가 일치하면 `selected`+`history_lagging`**(첫 공개 뒤 이력 갱신 전의 예외) | 단위(순수 판정기) |
| AT-B3 | 루트·selection·버전 파일 접근 거부(deny ACL/0000) 또는 보호 검사 실패 → `blocked`, `status`·`help`만, 권한 변경 없음 | CI(Windows icacls·Mac chmod; pwsh 단계) |
| AT-B4 | 진단 전용 두 상태에서 Project 쓰기 시도가 불가능함(도구 부재) | CI |
| AT-B5 | **정상 prepare 성공 → select 없이 재연결** → `prepared`(유효 `store.json`, `last_generation=0`, intent 없음)로 번들 Core 실행, `damaged` 아님, `status`가 준비된 버전·미선택 보고 — 계속 허용 | CI |
| AT-B6 | **초기화 각 단계 중단**: 루트 생성 뒤·`store.json` 생성 전 → `damaged`(store.json 없음, 번들 Core 없음); `store.json` 뒤·버전 디렉터리 뒤·복사 중·`complete` 뒤·phase 갱신 전 → `initializing`/`prepared`(+`interrupted`), 번들 Core 실행, 재prepare가 흔적 보고(EEXIST) | CI(단계별 kill) |
| AT-B6a | **select 각 지점 중단**: intent 기록 전 → 이전 상태(prepared/selected 유지); intent 기록 뒤 selection 공개 전 → `damaged`, 번들 Core 미실행; selection 공개 뒤 이력 갱신 전 → `selected`+`history_lagging`; 이력 갱신 뒤 intent 정리 전 → `selected`+`intent_pending_cleanup`; 다음 runtime 도구 호출이 잠금 아래 이력·intent를 정리 | 단위(순수 판정기, 지점별 관측값) + CI(단계별 kill, 후속) |
| AT-B6b | **첫 selection 공개 뒤 이력 갱신 전 중단 + selection 소실**(`last_generation=0`, intent generation 1 존재, selection 없음) → `damaged`, **번들 Core 미실행**, 보호 통과 시 승인된 select로 복구 | CI |
| AT-B7 | **과거 선택 소실**: select 성공 뒤 `selection.json` 삭제(또는 손상) → `damaged`, 번들 Core 미실행; `store.json` 삭제(루트 존재) → selection 유무와 무관하게 `damaged`(불명확 상태); 복구는 보호 통과 + 승인된 select(이때 `store.json` 재생성) | CI |
| AT-B8 | **진단 상태 복구**: `damaged`+보호 통과 → 승인된 `select`(complete 버전) 성공 후 다음 연결 `selected`; `blocked`(보호 실패) → `prepare`/`select` 미노출·호출 시 거절, 검사 우회 없음 | CI |
| AT-S1 | prepare: digest 불일치·멤버 수/경로 탈출·import allowlist·헤더 불일치 각각 거절, 바이너리 미실행(실행 흔적 없음) | CI |
| AT-S2 | prepare 중단(복사 중 kill) → `complete` 없음, 실행기 무시, 재prepare EEXIST 보고 | CI |
| AT-S3 | **같은 generation의 writer 2개 동시 select → 정확히 하나 성공**, 다른 쪽 무변경 거절(잠금 실패 또는 generation 불일치), selection 최종 내용=성공 쪽 | CI(두 프로세스, 배리어) |
| AT-S4 | 잠금 보유 중 select → 즉시 무변경 거절, 대기·회수 없음; 보유 프로세스 종료 후 잠금 해제 확인 | CI |
| AT-S5 | select 중단(rename 직전 kill) → `.new` 잔존, selection 불변, 다음 연결 이전 버전 | CI |
| AT-P1 | 루트·버전·selection 실효 접근 검사: 다른 주체 쓰기 ACE 추가/소유자 변경/reparse/hardlink 2 → `blocked`, ACL 자동 수정 없음 | CI(Windows) / Mac(모드·symlink) |
| AT-P2 | 실행기의 핀 고정 핸들: hash 뒤 교체 시도가 공유 위반으로 실패, 자식은 검증된 bytes 실행 | CI(Windows) |
| AT-P3 | **실행 파일의 실행 권한**: Core 파일에 FILE_EXECUTE 거부 ACE(Windows)/실행 비트 없음(macOS) → 실행기 `blocked`, spawn 시도 없음; 데이터 파일에 실행 비트가 있으면(macOS) `blocked` | CI |
| AT-P4 | Windows (a)/(b) 분리: 사용자에게 접근은 있으나 Users에 쓰기 ACE가 추가된 경우 (a) 통과·(b) 실패 → `blocked`; macOS 확장 ACL 존재 → `blocked` | CI |
| AT-T1 | 출처 패턴 불일치·`SHA256SUMS`/receipt/승인 digest 불일치 → prepare 거절, 실행 없음 | CI |
| AT-T2 | 게이트 통과 뒤 descriptor 실행: 인자 고정·stdin 닫힘·제한 시간·출력 상한; 비정상 종료 시 `complete` 미기록 | CI |
| AT-C1 | 합성 두 버전(fixture Core A/B): descriptor·카탈로그·View schema 동일이면 허용, 하나라도 다르면 거절; A bytes 보존; 새 연결 B 실행·기존 연결 A 유지 | CI |
| AT-C2 | 실제 GIL 두 버전 의미·저장 호환(합성 Project로 A 기록을 B가 읽고 쓰기, 이미지·잠금·View) | U3(별도; CI fixture 시험이 대신하지 않음) |
| AT-R1 | Codex 서버 환경의 실제 저장소 경로·실효 접근·다운로드 파일 읽기·지속성(앱 갱신·재부팅 후) | prerelease, 실제 Windows Codex |
| AT-R2 | Host가 cache 밖 Core 실행을 허용하는지, 새 연결 적용 시점(새 대화 vs 앱 재실행), 기존 연결 고정 | prerelease |
| AT-R3 | 사용자 PC(E3 환경)에서 파일 rename 교체·잠금 동작 | prerelease |
| AT-R4 | Agent가 승인 전 select/prepare를 부르지 않고 digest·버전을 사용자에게 보이는 행동 | prerelease(행동 검수) |
| AT-R5 | 최초 도입: 공식 Plugin 교체(기존 direct-Core 설치본의 cache 삭제 거부 포함) — **미해결, 이 설계의 범위 밖** | 별도 |

## 9. 미결 사항
1. A1·A2·A5(서버 환경 경로, Host의 외부 Core 실행 허용, 사용자 PC의 파일 rename) — prerelease 전에는 답이 없다.
2. 진단 전용 상태의 도구 표면(`gil_runtime_status`·`gil_help`만)이 Host·Skill에서 어떻게 보이는지, 이 상태로 뜬 연결을 사용자에게 어떻게 알릴지.
3. 승인 결합 형식: `gil_runtime_select`에 넣는 승인 근거(사용자가 본 ver+sha12+generation)로 충분한지(인간 승인 검증은 여전히 Agent 행동 검수 대상).
4. 도구 3개 추가는 작성 표면 도구 수(25)와 Skill 문구를 바꾸므로 **첫 도입은 Plugin 교체**가 필요하다(Core-only로 도입 불가).
5. publisher 신뢰: 다운로드 digest 검증만으로는 출처 인증이 아니다(unsigned 승인 정책 유지).
6. 디렉터리 flush·rename의 Windows 내구성 범위.

## 10. 최소 구현 파일 범위 (예정; 2026-10-23 첫 구현 = 순수 판정기만)
- **구현됨(미커밋, 2026-10-23 두 번째, 보완 1회)**: `src/runtime_observe.rs` + `runtime_observe_tests_unix.rs`/`runtime_observe_tests_windows.rs` — 명시된 저장소 경로의 **읽기 전용 관측** `observe(&Path) -> Observed{observation, checks, owner}`와 `observe_and_judge`. 생성·ACL 변경·잠금·복구·정리·후보 실행 없음.
  - 경로 안전성: 볼륨 루트부터 **핸들 상대 탐색**(Unix `openat`+`O_NOFOLLOW|O_DIRECTORY`, Windows `NtCreateFile` `RootDirectory`=상위 핸들·reparse 거절). 세 가지를 구분한다: (i) **단계별 핸들 상대 탐색** — 각 조상은 직전 핸들 기준으로 열어 자기 핸들에서 검사한 뒤 직전 핸들은 버리며, 탐색이 끝나면 루트 핸들만 남는다(조상은 관측 동안 고정되지 않음); (ii) **관측 대상 핸들 유지** — 루트와 그 아래 읽은 모든 항목(기록·versions·버전 디렉터리·complete·Core)의 핸들은 최종 재검사까지 유지하고, 모든 항목은 보유한 상위 핸들 기준 상대 열기이며 경로 문자열 재해석이 없어 관측 도중 조상·중간 디렉터리가 교체되어도 처음 연 트리 밖은 읽지 않음; (iii) **마지막 루트 재탐색** — 종료 시 (i)을 한 번 더 수행해 보유한 루트 핸들과 같은 디렉터리가 아니면 `blocked("root replaced at its path")`. 디렉터리 핸들 복제(`try_clone`) 실패는 panic 없이 `blocked`(시험에서 주입). Windows 목록은 `NtQueryDirectoryFile` 응답을 `IO_STATUS_BLOCK.Information` 범위 안에서만 파싱(헤더·짝수 FileNameLength·NextEntryOffset 전진·overflow 검사, 불일치는 오류; 파서는 플랫폼 독립 단위시험 3건). 조상 검사 범위: Unix 링크·소유자(root/본인)·group/other 쓰기(sticky 제외), Windows reparse 거절만(ACL 미검사).
  - 분류: 루트는 부모가 "없음"이라 답할 때만 `root_exists=false`, 그 외 실패는 `blocked`; 기록은 "없음"일 때만 `Absent`, 접근 거부·I/O 오류·비정규·링크는 `blocked`; 64 KiB·schema·generation u64·버전 정규식·64 hex digest 위반은 `Invalid`.
  - 보호 검사: 루트(Windows `private_dir`/Unix 0700+소유자+확장 ACL 없음) 실패 시 아래를 읽지 않음; `versions/`·버전 디렉터리(`inherited_only(dir)`/0700) 실패 시 하위 미판독; `complete`(존재 마커, 내용 미해석; 데이터 파일 보호: `inherited_only(file)`+링크 1/0600)·Core(동일+Windows FILE_EXECUTE 열기/0500) 실패 시 Core 미해시. 통과한 검사만 `checks`에 기록, 미구현(예: Linux 확장 ACL)은 `blocked`.
  - 변경 감지(`COMPARED`): 루트 목록(이름) 전후; 루트 항목 store/intent/selection/versions 의 identity(dev/volume·inode/file id·크기·링크 수·mode/attributes·mtime·ctime/ChangeTime)를 **첫 읽기 전에 기록**해 마지막 읽기 뒤 재비교; 보유한 모든 핸들(루트·기록·versions·버전 디렉터리·complete·Core)의 open 시점 identity 재비교; 보유 핸들에 보호 검사 재수행(권한 변경 감지); 루트 경로 재탐색. 놓치는 것(`LIMITS`): 크기·mtime·ctime/ChangeTime 이 그대로인 내용 변경(소유자 수준 타임스탬프 조작), 읽지 않은 항목(다른 버전 디렉터리 등), 최종 재검사 이후 변경, 조상 ACL. 잠금 없는 관측은 원자적 스냅샷이 아니며 결과는 실행 허가가 아님.
  - 시험: 파서 단위시험 3건(모든 호스트) + 핸들 복제 실패 주입 1건(Unix·Windows 각각) + Unix 12건(fixture 구성과 관측기 분리; 관측 전후 이름·모드·링크 수·소유자·크기·내용 스냅샷 불변, atime 제외): first_use, prepared/selected/lagging/의도만, damaged 변형, 경로 이탈·식별자, 심볼릭/하드 링크(기록·complete·버전 디렉터리·루트), 보호 실패(기록 mode·group 쓰기 루트·mode 000 기록/루트·Core 실행 비트·versions/·버전 디렉터리·complete mode, 하위 미판독 단언), macOS 확장 ACL(store/versions/complete), 관측 중 변경(덮어쓰기·파일 추가·권한 변경·selection 교체), 중간 디렉터리(`versions`) 심볼릭 링크 교체(외부 canary 미판독), 조상 교체(심볼릭 링크/실제 디렉터리; A 기록 유지·B 미판독·재탐색 차단), 심볼릭 링크 조상. Windows 10건(`cfg(all(test, windows))`; 보호 DACL 루트 fixture, icacls grant/inheritance/deny, hard_link, PowerShell junction): 정상 상태, damaged, 보호 실패(기록·루트·versions/·complete·Core, 하위 미판독), reparse(버전 디렉터리·루트·조상)·하드링크(기록·complete), 접근 거부(deny 전제 단언; 루트 deny), 관측 중 변경(덮어쓰기·추가·DACL 변경·versions junction 교체), 조상 교체(OS가 rename 거절 시 그 결과도 기록). **Windows 시험은 Mac 교차 컴파일(x64 두 프로필 `cargo check --tests`)만 확인했고 실행 증거 없음.**
- **구현됨(미커밋, 2026-10-23 세 번째)**: `src/runtime_prepare.rs` + `runtime_prepare_tests.rs` — 명시 경로 저장소의 **초기화·후보 준비** `prepare(&Path, &Request{version, expected_sha256, source}) -> Result<Report, Refusal>`. 순서(§1.1 1.): 관측+판정(blocked/damaged → 거절, bootstrap·복구 없음; first_use만 초기화) → 부모를 핸들 상대 탐색해 루트 배타 생성(Unix `mkdirat` 0700 / Windows `create_private_dir_at` 보호 DACL) → 보호 재검사 → `store.json{initializing,last_generation 0,owner}` 배타 생성·flush·보호 검사·디렉터리 flush → `versions/`(없으면 배타 생성, 동시 생성은 열어서 검사) → 유일성 검사(같은 version의 다른 sha12 디렉터리 = "다른 bytes" 거절; 같은 이름이 complete이면 Core 재hash가 기대 digest와 같을 때만 AlreadyPrepared, 다르면 거절; complete 없으면 "미완성 준비본" 거절 — 모두 보존) → 버전 디렉터리 배타 생성(EEXIST = 경쟁 패배, 무변경 거절) → 소스(링크 미추적·정규·링크 1·256 MiB 상한)를 hash하며 복사 → flush → 재열기·재hash → 기대 digest 일치 시에만 실행 모드(Unix 0500; Windows 상속 ACE) → `complete` 배타 생성·flush·디렉터리 flush → phase initializing이면 `store.json.new` 배타 생성(있으면 거절)·flush·rename 교체·flush → 보유 디렉터리가 아직 그 이름의 항목인지 재확인(`versions/`·버전 디렉터리; 링크 교체 시 거절) → 재관측·판정이 예측 상태(Prepared, 또는 Selected generation 불변)와 같을 때만 Ok. 삭제·GC 없음: `Report.created`/`Refusal.created`·`remaining`(보유 핸들로 재확인)에 생성·잔존 객체를 순서대로 기록. 기대 digest 일치는 복사 무결성 검사이며 publisher 인증·후보 신뢰 판정이 아님(§4.1 별도).
  - 공용 원시 연산 추가(기존 호출 무변경): `internal_fs::{create_dir_at, create_file_at, chmod, rename_at, fsync}`, `windows_fs::{create_dir_at, create_file_at}`(`nt_open_at` 재사용), 관측기 `platform` 모듈 `pub(crate)`·`Dir::{as_file, from_file}`·`valid_version/valid_sha256/version_dirname`. 회귀 범위: `src/internal_fs.rs`·`src/windows_fs.rs`의 기존 함수 본문은 그대로이므로 기존 호출(.gil 제어 파일·바인딩·Windows 저장소)의 동작 변경 없음; Mac에서 lib 전체 두 프로필과 visual_start/visual_store/visual_attachment 통과, Windows는 교차 check만.
  - 관측기 보정 1건: `versions/`가 존재하면 selection 유무와 무관하게 디렉터리로 열어 보호 검사(링크·비보호 `versions/`는 selection 없어도 blocked). 발견 경위: prepare의 경로 교체 시험에서 selection 없는 저장소의 `versions` 심볼릭 링크 교체가 재관측에 걸리지 않음. 관측기 Unix 시험 1건 추가.
  - 보완(2026-10-23, 같은 날 2차): (1) **버전 단위 배타 예약** `versions/~reserve-<version>`(배타 생성 파일; `~`는 version 식별자 첫 글자가 될 수 없어 버전 디렉터리로 파싱되지 않음). 순서: 빠른 경로 검사(complete·검증 통과 → AlreadyPrepared, 예약 불필요) → 예약(EEXIST = 동시 진행 또는 중단 잔존 → 거절, 회수 없음) → 예약 아래 재검사(목록이 예약 전에 낡았을 수 있음) → 디렉터리 생성. **예약은 이 계층이 절대 제거하지 않음**(2026-10-23 3차 보정: Unix의 "identity 확인 뒤 unlink"는 조건부 원자 삭제가 아니므로 "자기 객체만 삭제" 보장을 줄 수 없어 보수적으로 보존). 잔존 예약의 의미: 검증된 완료 디렉터리와 함께 있으면 준비의 정상 종료 상태(빠른 경로는 그대로 AlreadyPrepared); 디렉터리 없이 있으면 중단된 준비이며 승인 복구가 제거하기 전까지 그 version은 거절. 종료 직전 자기 예약이 아직 같은 객체인지 재확인(제거·교체 시 Reobserve 거절; 작성한 객체는 보존·보고). 이 계층에는 unlink·delete 경로가 없음(시험이 소스에 해당 호출이 없음을 확인). (2) **AlreadyPrepared 검증**: 기존 디렉터리 보호(`check_dir`), `complete` 보호·링크 1(`check_data`), Core 보호·링크 1·실행 권한(`check_exec`; Windows는 FILE_EXECUTE 열기)·digest 재hash — 신규 준비본과 같은 검사. 어느 하나라도 실패하면 거절·보존(bytes·권한 미수정). (3) **생성·잔존 보고**: 생성 시 객체 identity(dev+inode / volume+file id)를 기록하고 재확인을 `Presence::{SameObject, Absent, Replaced, Unknown(err)}`로 구분(반환 직전 계산)(`Report.objects`/`Refusal.objects`); 조회 오류는 `Unknown`이지 부재가 아님; 보고용 상위 핸들 복제가 실패해도 생성 사실은 기록되고 presence만 `Unknown`; 교체된 initializing 기록은 `store.json (initializing record, replaced by this call)`=Replaced 로 보고.
  - 시험 15건(격리 fixture; 저장소는 prepare 자체로 생성; 스냅샷에 Unix 모드 포함): first_use 초기화·준비·반복 no-op, 요청 검증, 같은 version 다른 bytes 거절·complete인데 digest 다른 디렉터리 거절(무변경 스냅샷), 둘째 version 추가 시 첫 version·기록 불변, 기존 selection·last_generation 불변(fixture가 selection 기록), damaged/blocked 거절(bootstrap 없음), **경계별 실패 주입 9지점**(RootCreated·StoreWritten·VersionsCreated·VersionDirCreated·CoreWritten·CoreFlushed·Verified·CompleteWritten·NewStoreWritten: created==remaining 정확 일치, complete 존재 여부, 이후 상태, 재호출 결과 — 루트만 생성=damaged 거절, 미완성 디렉터리=거절·보존, complete 뒤 중단=AlreadyPrepared+prepared 기록, store.json.new 잔존=거절), 쓰기·flush·digest 불일치(부분 보존·complete 없음·같은 version 재시도 거절), **같은 목적지 경쟁 4 스레드**(Prepared 정확히 1, 패자는 무잔존 Observe/Reservation/VersionDir/Uniqueness 거절 또는 AlreadyPrepared), **first_use 경쟁**(루트 생성자 정확히 1, 패자 잔존물은 유효 초기화 상태·공용 `versions/`뿐), **같은 version·다른 digest 동시 2건×3라운드**(배리어로 둘 다 빠른 경로 검사를 통과한 뒤 예약 경쟁; Prepared 1·Reservation 거절 1·완료본 1), **예약 영속성**(성공 뒤 잔존·SameObject·AlreadyPrepared 유지, 예약 아래 거절 시 잔존, 디렉터리 생성 뒤 예약 제거=Absent·Reobserve 거절·작성물 보존, 이름 아래 교체=Replaced 거절, 소스에 unlink/delete 부재), **AlreadyPrepared 검증 실패 4경우**(complete 보호/하드링크, Core 보호/하드링크; Unix 추가로 실행 비트 없음) — 거절·bytes/권한 불변, **identity 보고**(이름 아래 교체=Replaced·제거=Absent·보고용 복제 실패=전부 Unknown이되 목록 완전·조회 거부(Unix)=Unknown(err)와 실제 상태 대조), `versions` 링크 교체(보유 디렉터리에만 기록·재확인 거절·전부 보고), 링크 소스 거절(Unix). Windows는 교차 check만(junction·보호 DACL fixture 포함, 실행 증거 없음).
- **구현됨(미커밋, 2026-10-23 네 번째)**: `src/runtime_lock.rs` + `src/runtime_select.rs` + `runtime_select_tests.rs` — **선택 전환** `select(&Path, &SelectRequest{version, sha256, expected_generation}) -> Result<SelectReport, SelectRefusal>`. 요청은 version·전체 digest·expected_generation에 결합되며 **인간 승인의 증명이 아님**(§4.1 승인 표면은 별도). 순서: 사전 관측(prepared/selected만) → 루트 핸들·보호 검사 → `selection.lock`(루트 아래 상대 열기/생성, 데이터 파일 보호 검사, Unix `flock(LOCK_EX|LOCK_NB)`/Windows `LockFileEx(EXCLUSIVE|FAIL_IMMEDIATELY)`; 잠금 실패=무변경 거절; 파일은 절대 삭제하지 않음) → 잠금 아래 재관측·판정(phase prepared, `store.last_generation == expected_generation`, Selected면 generation 일치·`history_lagging` 아님; 아니면 무변경 거절) → 후보 재검증(`verify_complete_candidate`: 준비 계층과 동일 검사 — 디렉터리 보호·complete 보호·링크 1·Core 보호·링크 1·실행 권한·digest) → `intent.json.new` 배타 생성·flush·rename→`intent.json`·flush → `selection.json.new`→`selection.json`(공개) → `store.json.new{prepared, last_generation=g+1}`→`store.json`(이력) → **intent.json 보존**(이름 기반 검사-후-삭제 경쟁을 만들지 않기 위해 정리 생략; 판정기는 `intent_pending_cleanup`인 selected로 읽고 다음 select가 rename으로 교체) → 잠금 해제 → 재관측이 `Selected{g+1, version, lagging=false}`일 때만 Ok. 보고: `Progress{intent_recorded, published, history_updated, intent_kept}`는 각 rename 뒤에만 true(공개 뒤 실패를 "미변경"으로 보고하지 않음), `previous_selection_preserved`(교체 전 실패 시 이전 selection 바이트 동일 여부), 생성 객체 presence. generation: `expected_generation.checked_add(1)` 실패(u64::MAX)=거절, 0은 요청될 수 없음(0+1≥1), 기록 중 `generation 0`은 판정기가 거절.
  - 두 계층의 직렬화 경계: prepare의 `initializing → prepared` 기록도 같은 `selection.lock` 아래에서 `store.json`을 다시 읽어 phase가 더 이상 initializing이 아니면 **기록을 건드리지 않음**(note "record left alone"; 재관측 Initializing→Selected 허용). 보정(2026-10-23 5차): 재읽기와 교체는 **하나의 잠금 수명** 안에서 수행(그 사이 다른 writer 불가; 시험이 잠금 아래에서 두 번째 획득 WouldBlock·select 무변경 거절을 결정적으로 확인); prepare도 잠금을 **기다리지 않고**(NonBlocking) 거절하되 이미 만든 객체(버전·initializing 기록)는 보존·보고하며 "무변경"이라 표현하지 않음; 관측은 **보유 루트 핸들**로 수행(`observe_with(held)`: 핸들 복제로 읽고 경로는 끝에 같은 객체를 가리키는지만 확인) — select의 잠금 아래 관측과 마지막 검증(잠금 수명 안), prepare의 최종 재관측 모두 해당, 루트가 경로에서 교체되면 다른 저장소의 값으로 원래 저장소를 갱신하지 않고 거절(시험); `selection.lock`은 배타 생성(이 호출이 만든 파일만 모드 설정) 또는 기존 파일 열기(유형·소유자·모드/DACL·링크 1 검증만, chmod 없음) — 잘못된 모드·하드링크·심볼릭 링크 잠금 파일은 거절하며 원본·연결 대상의 바이트·권한 불변(시험). 이것이 없으면 initializing을 일찍 관측한 prepare가 뒤늦게 `last_generation 0` 기록으로 select의 이력을 덮어쓸 수 있음(시험 (b)로 재현·차단). select는 phase initializing이면 거절하므로 prepare의 최초 기록과 select는 겹치지 않음; prepare는 버전 예약을, select는 잠금 파일을 지우지 않음.
  - 중단 지점↔판정기: intent 기록 뒤(공개 전) = `damaged`(이전 selection 바이트 보존, 승인 복구 필요); 공개 뒤(이력 전) = `selected{history_lagging}`(다음 select는 이 계층에서 거절 — lagging 완료는 범위 밖); 이력 뒤 = `selected` + `intent_pending_cleanup`(정상 종료와 동일). `*.new` 잔존은 다음 시도를 거절(보존). 손상 복구·rollback·GC·lagging 완료 미구현.
  - 시험 10건: 첫 선택·다음 세대(기록 내용·observer 일치·versions/ 바이트·모드 불변·잠금 파일 잔존), 요청 결합·경계(overflow·식별자·generation 충돌·미준비/다른 digest/미완성/보호 실패 후보·오래된 expected_generation·prepared 아닌 저장소), 잠금 보유 시 select 거절·prepare 기록 단계 대기(채널+600ms 관측 뒤 해제 시 진행), 같은 expected_generation 두 writer ×3라운드(성공 1, 패자 무변경), prepare↔select 경쟁 ×3(관측 충돌 시 보수적 거절 뒤 재시도 성공, 이력 미덮어쓰기)+늦은 기록 재현(b), 6지점 실패 주입×첫/다음 선택(판정기 상태·disk↔보고 일치·이전 selection 보존 여부·후보 바이트 불변·`*.new` 잔존 재시도 거절·lagging 재시도 거절)+쓰기/flush 실패, 소스에 실행·삭제 호출 부재, 잠금 보유 시 prepare 기록 단계 비대기 거절·객체 보고·해제 뒤 완료, 단일 잠금 수명(잠금 아래 두 번째 획득 WouldBlock·select 거절), 잠금 뒤 루트 교체(보유 객체 값만 사용·거절·두 저장소 불변; Windows는 OS rename 거절 분기 기록), 기존 잠금 파일 무변경(모드/ACL 오류·하드링크·심볼릭 링크 거절, 바이트·권한 불변, 재획득 시 내용 불변). 관측기 Unix 시험 1건 추가(보유 핸들 관측·경로 교체 차단). Windows는 교차 check만.
- **구현됨(미커밋, 2026-10-23 다섯 번째, crate 내부 전용; 같은 날 보정 1회)**: `src/runtime_launch.rs` + `runtime_launch_tests.rs` — **격리 실행 메커니즘**. 계약 확인: 실행 직전 검증 = §1 "selection 읽기·검증 → complete·digest 재검증(핀 고정 핸들 아래) → spawn·stdio·EOF·종료 코드 전달", §3.1 TOCTOU 행 "Core 파일을 쓰기·삭제 공유 거부 핸들로 열어 hash 한 뒤 자식 시작까지 유지", §1.1 "first_use/initializing/prepared → 번들, selected → 선택판, damaged/blocked → 일반 Core 미실행", AT-P2/P3. 구현: `prepare_target(root, bundled)`(보유 루트 핸들 관측→판정→대상 결정; **selected의 generation·version·전체 digest는 같은 관측의 유효 Selection 기록에서만** 취함, 별도 재읽기 없음; Run::None은 거절·fallback 없음; selected는 `verify_complete_candidate` + 핀 핸들 재hash·`check_exec`; 번들은 정규 파일·링크 1·실행 비트만) → `spawn(target, FixedArgs, IoMode)`(실행 파일과 고정 인자 분리: `FixedArgs::McpServe = ["mcp","--serve"]`, 시험 전용 `Synthetic`; 셸 문자열·임의 인자 없음; spawn 뒤 핀 해제) → `Child::{send_and_close, finish, send_eof_and_finish}`. **Unix spawn은 자체 fork+execve**(execvp/posix_spawnp 미사용 — libc execvp는 ENOEXEC 파일을 /bin/sh로 재실행하므로 제거; fork 뒤 자식 코드는 stat·dup2·execve·write·_exit만). 시작 이후 보고: `ExitReport{pid, exit: Code/Signal/NotConfirmed{waited}, stdout/stderr: Bytes/Failed(err)/NotCaptured, stdin_error}` — stdin 쓰기 실패여도 stdin을 닫아 EOF 전달, reader 실패는 Failed로(빈 출력 아님), 대기 한도 초과는 NotConfirmed(정상 결과 아님).
  - **방어 범위(구현 그대로)**: Windows — `FILE_EXECUTE|FILE_SHARE_READ` 핀을 CreateProcess까지 유지해 그 파일의 rename/삭제/쓰기(같은 객체 내용 변경)는 공유 위반; 아래 핸들이 있는 조상 rename은 OS가 거절(runner 관측); **막지 못함**: 상위 경로 reparse(관측 walk만 거절). Unix — 핀은 hash용 읽기 fd이며 자식이 execve 직전 경로 (dev,ino)와 핀 identity를 비교해 다른 객체면 ESTALE 거절; **막지 못함**: 비교와 execve 시스템콜 사이의 교체(한 시스템콜 폭, 0 아님), 쓰기 권한을 가진 writer의 **같은 inode 내용 변경**(Unix에 공유 잠금 없음; 0500/0700이 유일한 장벽), shebang 인터프리터의 경로 해석. 이 Mac에서 시도한 두 exec-by-handle 경로(`/dev/fd/N` devfs 거부, `/.vol/<inode>` 교체 inode 해석 불가)는 실패했고 다른 경로는 평가하지 않음("macOS에 exec-by-handle 없음"을 주장하지 않음). 번들 Core digest·서명 미검증. "검증되지 않은 bytes는 실행되지 않는다"는 일반 보장은 하지 않음. 공개 진입점 없음(모두 `pub(crate)`; §4.1 신뢰 게이트·승인 전까지 manifest·MCP·설치 미연결).
  - 보정(2026-10-23 2차): **유한 제한의 범위** — `send_eof_and_finish(input, limit)`는 하나의 deadline으로 입력 전달(Unix 비차단 파이프+`poll`, Windows writer 스레드; 미완은 `stdin_error`와 `left_behind`에 기록, stdin은 어떤 경우에도 닫아 EOF 전달)·종료 확인(`WNOHANG` 폴링, 초과는 `Exit::NotConfirmed`)·출력 수집(reader 스레드의 진행을 공유 버퍼로 보고 남은 시간만 기다림; 초과는 `Capture::Incomplete(지금까지의 bytes)`)을 모두 묶는다. 자식이 종료해도 후손이 파이프를 쥐면 "종료 확인 + Incomplete"로 구분되며, 남은 책임(`left_behind`: 미회수 자식 pid는 호출자가 종료·회수, reader 스레드는 마지막 writer가 파이프를 닫을 때 종료, Windows writer 스레드)을 보고한다. 제한 `None`(제품의 serving)은 무기한. **errno 채널**(Unix fork/exec): 성공 EOF / 완전한 4바이트 errno(자식 127 종료를 500 ms 폴링으로 회수·보고, `replaced`=ESTALE) / 부분 응답 / 읽기 오류를 구분하고 부분·오류는 성공으로 처리하지 않으며(`Started::Unknown`, `FailedChild{pid, exit}` 보존, 파이프 끝을 먼저 닫아 exec된 자식이 EOF로 끝날 수 있게 한 뒤 500 ms 폴링; 미회수면 호출자 책임 명시), EINTR 재시도, 실패 자식 `waitpid` 반환값 확인. 관측(이 Mac): 병렬 스위트에서는 fork된 자식이 `_dyld_start`에 1–2초 머물고 'N'(낮은 우선순위) 플래그를 가짐 → 시험 제한은 자식 시작 지연과 무관한 불변식(후손의 30초 파이프 보유가 보고를 막지 않음·미회수 자식은 pid로 회수 가능)만 단언.
  - 보정(2026-10-23 3차): 입력 결과를 `Delivery::{Complete, Failed, Timeout{delivered,total,stdin_closed}, Unknown}`로 구분. Unix는 매 반복 deadline 확인(EAGAIN뿐 아니라 꾸준한 부분 진행도 유계)·stdin은 항상 닫힘(`stdin_closed: true`). Windows writer 스레드는 deadline 뒤에도 stdin을 보유하며 강제 취소 없음 → `Timeout{stdin_closed: false}`로 "stdin NOT closed, EOF not delivered"와 `left_behind`(writer가 쓰기 종료/실패 시 닫음)로 정직 보고, 스레드 단절은 `Unknown`. 시험: Unix 꾸준한 1 KiB/10 ms 소비 자식 + 64 MiB·800 ms → deadline 안 보고·부분 전달 수 보고; Windows `FixedArgs::SyntheticIdle`(`ping` 30 s, stdin 미판독) + 8 MiB·3 s → NOT closed·writer left_behind·NotConfirmed·Incomplete, `taskkill /T /F` 뒤 writer 채널로 스레드 종료(쓰기 실패) 확인; Windows 선택 실행의 종료 확인 분기 출력. workflow에 실행 단계(`--nocapture`; 14건 중 공통 6·Unix 전용 6·Windows 전용 2 → Windows 8 실행).
  - 시험 14건(합성 v1/v2 스크립트: 시작 흔적·stdin 에코·고정 exit; Windows는 cmd.exe 사본+overlay): **유한 제한**(stdin 미판독·출력 미닫음 자식+8 MiB 입력·700 ms → 65536/8388608 bytes 전달·NotConfirmed·Incomplete·left_behind 3건, 시험이 SIGKILL·waitpid 회수 / 직접 자식 종료+후손이 출력 파이프 유지 → exit Code(0) 확인·`Incomplete("child start")`·reader만 left_behind, 시험이 후손 pid를 SIGKILL 후 소멸 확인), **errno 채널 주입**(Partial → "2 of 4 bytes"·Unknown·pid·회수 결과, ReadError → "read failed: injected"·Unknown, EintrThenReal → 정상 실행; 완전 응답은 거부 이미지 시험에서 `FailedChild{exit: Code(127)}`), 번들 3상태, selected 실행·stdio·EOF·exit·pid·핀 재hash, **같은 관측 identity**(관측 직후 v2 선택 주입 → 대상 전 필드 v1·실행도 v1, 다음 결정은 v2), damaged/blocked/digest 불일치 미실행·fallback 없음, **커널이 거부하는 이미지 5종**(매직 없음·Mach-O 매직+쓰레기·ELF 매직+쓰레기·존재하지 않는 shebang·빈 파일; 각 파일에 셸 명령 흔적 페이로드 포함 → 흔적 없음=셸 미실행, Spawn 거절), 핀 뒤 교체 → 자식 identity 검사 거절·미시작(Windows 변형은 rename 공유 위반), v1 연결 유지 → v2 선택 → v1 출력 유지·새 실행 v2, **시작 이후 오류**(조기 종료 자식에 8 MiB 쓰기 → stdin_error + exit Code(5) 확인 + pid; reader 실패 주입 → Capture::Failed; 30초 sleep 자식 400ms 한도 → NotConfirmed{pid} 뒤 시험이 SIGKILL·waitpid로 종료 확인), 고정 인자·비공개·execvp/posix_spawn/셸 호출 부재 소스 검사.
- **구현됨(커밋 e9ddb7c)**: `src/runtime_state.rs` — 순수 상태 판정기 `judge(&Observation) -> Verdict`와 단위시험 18건(generation 0·u64::MAX 경계 5건 포함)(표의 각 행, generation 경계, intent 손상·불일치, prepare/select 각 중단 지점, 유계 전수 조합에서 결과 유일성과 "선택 시도 없음일 때만 번들" 불변식). 파일 접근·잠금·교체·실행·MCP 도구·manifest 변경 없음.
- `src/runtime/mod.rs`(저장소 경로·identity·상태), `src/runtime/store.rs`(versions/·complete·재hash), `src/runtime/selection.rs`(selection.json·generation·rename 교체), `src/runtime/launch.rs`(자식 실행·stdio·종료), `src/runtime/verify.rs`(descriptor v2·카탈로그 digest 비교)
- `src/command.rs`(`launch mcp` 서브커맨드), `src/agent.rs`(descriptor v2 필드, `--gil-tool-catalog`), `src/mcp/runtime.rs`(도구 3개, 작성 표면·Windows/Mac), `src/mcp/surface.rs`·`src/mcp.rs`(dispatch), `src/windows_fs.rs`(기존 rename/flush 재사용; known folder API 추가)
- `plugins/gil-companion-prototype/.codex-plugin/plugin.json`·`distribution/windows/candidate.mjs`·`plugins/gil-companion-prototype/package-native.mjs`(args `launch mcp`), `distribution/codex/tool-contract.mjs`(도구 수), Skill(업데이트 절 1개)
- 시험: `src/runtime/*_tests.rs`, `tests/runtime_update.rs`(합성 두 버전 쌍), `tests/mcp.rs`(도구), `.github/workflows/windows-spike.yml`·`codex-preview.yml`(실행), `distribution/runtime-update/`(probe 유지)
