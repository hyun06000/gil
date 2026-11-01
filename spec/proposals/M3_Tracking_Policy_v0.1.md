# M3 Tracking Policy v0.1 (proposal)

> 상태: **제안.** Wave 1 세션 A 의 산출물이며 저장 형식·제품 capture·restore 를 바꾸지 않는다.
> 정본 계약은 여전히 `spec/GIL_Artifact_Model_v0.1.md` 다. 이 문서는 §3.6a 의 1·2 항
> (루트 `.git/`·`.gil/` 항상 제외, `.gitignore` 존중)을 **읽기 전용 엔진**으로 어떻게 세우는지
> 제안하고, 통합 단계에 남는 연결을 적는다.

## 1. 문제

현행 `artifact::observe` 는 루트 `.gil/` 하나만 제외하고 모든 일반 파일을 세계로 본다.
`.git/`, `node_modules/`, build 출력, `.env` 같은 secret 이 Snapshot 에 들어가고, restore 는
목표 manifest 에 없는 파일을 삭제한다. 일반 사용자 프로젝트에 허용할 수 있는 범위가 아니다.

이번 제안은 기존 관측의 의미를 **조용히 바꾸지 않는다.** 대신 정책이 명시적으로 선택된
새 읽기 경로를 두고, 그 결과에 정책 식별자를 함께 묶는다.

## 2. 구성

```text
src/artifact/tracking_policy.rs
  IgnoreSource { directory, text }           명시적 입력: 상대 위치 + 원문
  PolicyVersion::V1                          해석 의미의 버전
  TrackingPolicy::from_sources(version, [IgnoreSource]) -> Result<TrackingPolicy, PolicyError>
  TrackingPolicy::includes_file(&str) -> Result<bool, String>     A → B 연결 (INTERFACES.md)
  TrackingPolicy::policy_id() -> String                            A → B 연결
  TrackingPolicy::evaluate(&EntryPath) -> Result<Decision, PolicyError>   이유까지
  discover_gitignore_sources(root) -> Result<Vec<IgnoreSource>, PolicyObserveError>
  ExcludedDirectories::{Prune, Inspect}

src/artifact/mod.rs
  observe_with_policy(root, &TrackingPolicy, ExcludedDirectories)
      -> Result<PolicyObservation { manifest, policy_id, excluded }, PolicyObserveError>
```

### 2.1 입력은 전부 명시적이다

정책은 주어진 원문·상대 위치·버전만으로 세운다. 다음은 **읽지 않는다.**

- 사용자 global gitignore (`core.excludesFile`, `~/.config/git/ignore`)
- `.git/info/exclude`, `core.ignorecase` 등 모든 Git 설정
- 환경 변수, 현재 작업 디렉터리, 네트워크

프로젝트 트리의 `.gitignore` 를 모으는 것은 `discover_gitignore_sources(root)` 가 **주어진
루트 아래에서만** 한다. Git 과 같은 순서로 위에서 아래로 읽고, 이미 읽은 규칙이 제외한
디렉터리 안으로는 들어가지 않는다(그 안의 `.gitignore` 는 Git 도 읽지 않는다). `.git/`·`.gil/`
안은 보지 않는다. `.gitignore` 라는 이름의 심볼릭 링크는 원문으로 읽지 않고 거절한다.

### 2.2 내장 제외와 거절 계약

| 자리 | 판정 | 이유 |
|---|---|---|
| `.git` 구성 요소 (어느 깊이든, 파일이든 디렉터리든) | 항상 제외, `!` 로 못 살린다 | Git 자신의 metadata |
| 루트 `.gil/` 아래 | 항상 제외 | 현재 프로젝트의 내부 저장소 (기존 경계 유지) |
| `a/.gil/` 디렉터리 (중첩) | **거절** (`PolicyError::NestedGilBoundary` / `ObserveError::NestedGilRepository`) | 기존 거절 계약을 약화하지 않는다. 「제외」라고 답하면 B 가 무시해도 되는 것으로 읽는다 |
| 일반 파일 `.gil` (루트·하위) | 보통 규칙으로 판정 | 기존 관측과 같다 |
| 심볼릭 링크·특수 항목·비UTF-8·`\` 이름 | 열거되는 즉시 거절 — 정책이 제외해도 | 조용한 제외 금지 (§3.4) |

### 2.3 gitignore 호환 **부분집합**

구현한 것:

- 빈 줄, `#` 주석, `\#` 리터럴, trailing space 무시(`\ ` 는 보존), 줄 끝 `\r` 제거
- `!` 부정과 `\!` 리터럴, 마지막으로 맞는 규칙이 이긴다
- 끝의 `/` 디렉터리 전용; 앞/중간의 `/` 는 원본 디렉터리에 anchoring; `/` 없는 패턴은 어느 깊이에서나
- `*`, `?` (구분자를 넘지 않음), `[abc]`, `[!abc]`/`[^abc]`, `[a-z]`, `[[:alpha:]]` 등 ASCII POSIX 클래스, `\x` escape
- `**/`, `/**` (안의 모든 것, 디렉터리 자신은 아님), `/**/` (0개 이상의 디렉터리); 구성 요소 안의 `**` 는 `*`
- 중첩 원본: 깊은 원본이 얕은 원본보다 우선; 원본은 자기 디렉터리의 **아래**에만 적용
- 제외된 부모 디렉터리 아래는 어떤 부정 규칙으로도 되살아나지 않는다 (Git 과 같은 제약)

구현하지 않은 것 (전체 호환을 주장하지 않는다):

- `core.ignorecase` 대소문자 무시 매칭 — 항상 대소문자 구분
- global/`info/exclude` 원본 자동 수집 (명시적으로 `IgnoreSource` 로 넘기면 같은 엔진이 평가한다)
- `.gitattributes`, sparse-checkout, submodule 경계 (`.git` 파일은 단순히 제외만 한다)
- 해석할 수 없는 원문 — 닫히지 않은 `[`, 줄 끝 `\`, `//`, 구분자만 있는 줄, 모르는 `[:name:]` — 은
  Git 처럼 「안 맞음」으로 넘기지 않고 `PolicyError::InvalidPattern` 으로 **정책 전체를 거절**한다.
  짐작한 정책으로 Snapshot 을 만들지 않기 위해서다.

### 2.4 정책 fingerprint 와 `policy_id`

```text
sha256(
  "GIL-TRACKING-POLICY\0"
  version u32 BE
  source_count u64 BE
  for each source, sorted by directory bytes:
    directory_len u32 BE, directory bytes
    text_len u64 BE,      text bytes (원문 그대로 — CRLF·주석·줄 순서 포함)
)
policy_id = "gil-tracking-policy/1:sha256:<64 hex>"
```

- 같은 (버전, 원본 집합)은 **입력 순서와 무관하게** 같은 ID 다. 같은 위치의 원본이 둘이면 거절한다.
- 원문 한 글자, 줄 순서, 위치, 버전 중 하나라도 다르면 다른 ID 다. 해석 결과가 같아도 그렇다 —
  이 값은 「어떤 규칙을 읽어 그 Snapshot 을 만들었는가」를 되짚는 provenance 이지 의미 동등성의 증명이 아니다.
- OS 경로 검증·symlink/reparse/TOCTOU 검사 결과는 여기 섞이지 않는다 (INTERFACES.md).

### 2.5 정책 적용 관측

`observe_with_policy` 는 기본 관측과 같은 규칙으로 두 번 훑고 견준다. 제외된 항목의 변화는
세계의 변화가 아니다. 결과는 `manifest` + `policy_id` + `excluded`(경로·종류·이유, canonical 순서)다.

제외된 **디렉터리** 안을 어떻게 할지는 부르는 쪽이 정한다 — 결정 없이 보안 검사를 건너뛰지 않는다:

| `ExcludedDirectories` | 하는 일 | 한계 |
|---|---|---|
| `Prune` | 들어가지 않는다 | 그 안의 링크·특수 항목·중첩 `.gil` 은 **검사되지 않는다.** `excluded` 에 그 디렉터리가 남아 무엇을 보지 않았는지는 안다 |
| `Inspect` | 들어가서 종류 검사만 하고 아무것도 기록하지 않는다 | `node_modules/.bin` 의 링크처럼 흔한 것에서 관측 전체가 거절될 수 있다 |

열거된 항목 자체(제외된 디렉터리 entry, 제외 규칙에 맞는 최상위 링크)는 어느 모드든 종류 검사를
먼저 지난다. 따라서 `.venv -> …` 같은 **최상위 ignored 링크**는 여전히 관측을 거절한다. 이것은
알려진 마찰이며, 완화하려면 Artifact Model §3.4 의 링크 계약을 먼저 바꿔야 한다 — 이 제안은 바꾸지 않는다.

## 3. 과거 Snapshot 과의 호환 제안

**현재 `.gitignore` 를 과거 Snapshot 에 소급 적용하지 않는다.** 과거 manifest 는 그때 관측한
세계의 정확한 집합이고, 거기서 파일을 빼면 복원 의미(보존/삭제/격리)가 바뀐다.

제안하는 규칙:

1. 정책 provenance 는 **앞으로 만드는** Snapshot 에만 붙는다. Snapshot 기록에 `policy_id` 필드를
   더하는 것은 저장 schema 변경이므로 통합 담당자가 format 결정과 함께 한다. 이 제안은 그 필드의
   값 형식(`gil-tracking-policy/1:sha256:…`)만 정한다.
2. `policy_id` 가 없는 기존 Snapshot 은 「정책 미상(legacy)」이다. 임의의 ID 를 지어 붙이지 않는다.
3. B 의 미리보기는 before/target 의 `policy_id` 를 따로 받고, 미상이거나 서로 다르면 apply-ready
   로 판정하지 않는다 (INTERFACES.md). 정보성 diff 는 제공할 수 있다.
4. 정책이 바뀐 뒤의 첫 Verify 는 「정책 변경으로 세계가 달라졌다」를 보통의 dirty 로 다룬다.
   정책 변경 자체를 자동으로 clean 으로 취급하지 않는다.
5. 제품 기본 capture 에 어떤 정책을 기본으로 둘지(`.gitignore` 자동 수집 + `Prune` 이 유력하나
   최상위 ignored 링크 마찰이 있다)는 통합 단계에서 실기 결과를 보고 정한다.

## 4. 통합 단계에 남긴 연결

- `capture` 의 정책 버전 — blob 저장과 함께 정책을 적용하는 `capture_with_policy` 는 만들지 않았다.
  `observe_with_policy` 는 아무것도 쓰지 않는다.
- Session 기본 capture·Verify close·`gil start` 와의 연결, Snapshot 기록의 `policy_id` 저장.
- B 연결: 통합 담당자가 `|path| policy.includes_file(path)` 로 callback 을 만든다. B 는 A 타입을 import 하지 않는다.
- 제품 기본 정책 결정과 최상위 ignored 링크 마찰의 처리.
- Windows 경로 규칙(§3.6a 5·6 항)은 이 엔진의 범위가 아니다 — 경로 문자열의 정책 판정만 한다.

## 5. 의존성

새 crate 를 더하지 않았다. 엔진은 표준 라이브러리와 이미 쓰던 `sha2` 만 쓴다. 라이선스 변경 없음.
`ignore`/`globset` 같은 기존 라이브러리를 쓰지 않은 이유: `--locked` 환경에서 lockfile·고지
inventory 가 바뀌고, Git 호환이라는 이름으로 global 설정 읽기 같은 암묵 입력이 들어오는 것을
막기 위해서다. 부분집합임을 §2.3 에 명시했다.
