# A — 추적 정책 결과

- branch: `claude/wave1-tracking`
- 기준 commit: `e89a32b` (`fcd6e5d9…` 이 HEAD 의 조상임을 확인)
- 결과 commit: (아래 「커밋」 절)
- 제안 문서: `spec/proposals/M3_Tracking_Policy_v0.1.md`

## 구현 범위

`.git`/ignore 정책을 결정적으로 평가하는 엔진과, 정책이 **명시적으로 선택된** 새 관측 API 를 만들었다.
기존 `observe`/`capture` 의 의미, `EntryPath` 규칙, 저장 format/schema, Session 연결은 바꾸지 않았다.

| 구성 | 내용 |
|---|---|
| `TrackingPolicy::from_sources(PolicyVersion::V1, Vec<IgnoreSource>)` | 명시적 원문(상대 위치 + 텍스트)으로 정책 컴파일. 해석 불가 원문은 `PolicyError` 로 거절 |
| `includes_file(&str) -> Result<bool, String>` / `policy_id() -> String` | INTERFACES.md 의 A → B 연결. `Ok(true)` 추적 후보, `Ok(false)` 제외, `Err` 경로/해석 거절 |
| `evaluate(&EntryPath) -> Result<Decision, PolicyError>` | 이유까지 — `Excluded{rule}`, `ExcludedByParent{directory, rule}`, `ExcludedBuiltin{prefix, reason}` |
| fingerprint / `policy_id` | `gil-tracking-policy/1:sha256:<hex>`. 버전·원본 수·(정렬된) 상대 위치·원문 바이트를 길이와 함께 해시 |
| `discover_gitignore_sources(root)` | 주어진 루트 아래의 `.gitignore` 를 Git 순서로 수집. 제외된 디렉터리·`.git`·`.gil` 안은 읽지 않고, 링크 `.gitignore` 는 거절 |
| `observe_with_policy(root, &policy, ExcludedDirectories)` | 두 번 훑어 견주는 정책 적용 관측. `PolicyObservation { manifest, policy_id, excluded[] }` |
| `ExcludedDirectories::{Prune, Inspect}` | 제외된 디렉터리 안을 들어갈지 부르는 쪽이 결정. 한계는 proposal §2.5 |

구현한 gitignore 문법 부분집합과 구현하지 않은 항목은 proposal §2.3 에 적었다. **전체 gitignore 호환이라고
부르지 않는다.** 대소문자 무시(`core.ignorecase`), global/`info/exclude` 자동 수집, `.gitattributes` 는 없다.

내장 판정:
- `.git` 구성 요소는 어느 깊이든 항상 제외(`!` 로 못 살림). 루트 `.gil/` 아래는 항상 제외.
- 중첩 `.gil/` 은 「제외」가 아니라 **거절**(`PolicyError::NestedGilBoundary`, 관측에서는 기존
  `ObserveError::NestedGilRepository`). 심볼릭 링크·특수 항목·비UTF-8·`\` 이름은 정책이 제외해도 열거되는 즉시 거절.

## 변경 파일

- 신규 `src/artifact/tracking_policy.rs` — 엔진·수집·정책 적용 walk·unit tests
- `src/artifact/mod.rs` — `mod tracking_policy;`, `pub(crate) use` 재수출, `observe_with_policy` 하나
- 신규 `spec/proposals/M3_Tracking_Policy_v0.1.md`
- 신규 `coordination/parallel-wave-1/results/A-TRACKING-RESULT.md` (이 문서)

`Cargo.toml`/`Cargo.lock` 은 **변경하지 않았다.** 새 의존성 없음, 라이선스 변경 없음. `sha2` 와 표준 라이브러리만 쓴다.
`codec.rs`/`store*.rs`/`registry.rs` 는 읽기만 했다.

## 검증

실행 환경: macOS arm64, Rust 1.97.1, Node 26.5.0. Rust 는 모두 `python3 coordination/parallel-wave-1/cargo-queue.py -- …` 로 실행.

| 명령 | 결과 |
|---|---|
| `cargo-queue.py -- cargo test --locked -p gil artifact::` | 통과 — 151 passed, 0 failed (신규 31개 포함, 첫 실행에서 통과) |
| `cargo-queue.py -- cargo test --locked -p gil` | 통과 — lib 623 passed / 0 failed / 1 ignored(기존 `visual_migration::tests::crash_child`), 통합 시험 전부 통과, 1 ignored(기존 `actual_old_writer_refuses_format_five_without_mutating_it`, 옛 release 바이너리 필요). 경고는 기존 `gil-launcher` bin 의 dead_code 11건뿐 |
| `cargo-queue.py -- cargo build --locked -p gil --all-targets` | 통과 — Finished dev, 이번 변경에서 나온 경고 없음 |
| `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` (`npm ci --prefix mcp-app` 후) | 96 pass, 0 fail, 1 skipped — skip 은 기존 `gitless.test.mjs` 의 `GIL_GITLESS_ARCHIVE` 미설정 조건이며 이번 변경과 무관 |
| `git diff --check` | 통과 |

실행하지 않은 것: `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/…` (JS/UI 변경 없음), Tauri/Companion,
실제 Mac/Windows 설치본·Codex 화면·사람 참여 시험(통합 후).

새 unit tests (`src/artifact/tracking_policy.rs::tests`, 31 개):
- 문법: 주석/escape/trailing space/CRLF, anchoring, 디렉터리 전용, `**` 세 형태, `*`/`?` 가 `/` 를 넘지 않음, 문자 클래스
- 우선순위: 마지막 규칙 승, 깊은 원본 승, 원본은 자기 디렉터리를 판정하지 않음, **제외된 부모는 `!` 로 못 살림**(단일·중첩 원본 모두)
- 한글 경로·패턴, node_modules/.git/.env 전형 정책
- 정책 오류: 닫히지 않은 `[`, 줄 끝 `\`, `//`, `/`, 모르는 `[:name:]`, 잘못된/중복/`.git` 안 위치, 경로 계약 위반
- fingerprint: 입력 순서 무관 동일, 원문·줄 순서·위치·주석·원본 수 변경 시 상이, 길이 prefix 로 경계 이동 방지
- 실제 합성 폴더: `.gitignore`+중첩 `src/.gitignore`+`.git`+`.gil`+node_modules+build+`.env`+한글 — 정책 관측 manifest 와
  excluded 목록(이유·줄 번호 포함) 검증; 같은 트리·정책 → 같은 결과/ID; 다른 정책 → 다른 결과/ID; **기본 `observe` 는 여전히
  `.git/HEAD`·node_modules·`.env` 를 담는다**(회귀 없음)
- 두 번 보기: 포함 파일 생성 시 거절, 제외 파일 변경은 세계 변화 아님
- 거절 계약: 제외 규칙에 맞는 최상위 링크 거절(양 모드), Prune 은 안을 안 보고 Inspect 는 안의 링크 거절, `**/.gil/` 규칙이
  있어도 중첩 `.gil` 거절(양 모드), Prune 아래 중첩 `.gil` 은 excluded 에 디렉터리로 남음, 루트 `.gil` 안은 어느 모드든 안 봄,
  일반 파일 `.gil` 은 평범한 Artifact
- 관측이 담은/뺀 경로와 `includes_file` 의 답이 일치

## 다른 세션에 전달할 인터페이스

B (restore preview) 연결 — 통합 담당자가 다음처럼 callback 을 만든다. B 는 A 타입을 import 하지 않는다.

```rust
let sources = artifact::discover_gitignore_sources(root)?;           // 또는 명시적 Vec<IgnoreSource>
let policy = artifact::TrackingPolicy::from_sources(artifact::PolicyVersion::V1, sources)?;
let includes = |path: &str| policy.includes_file(path);              // impl Fn(&str) -> Result<bool, String>
let policy_id = policy.policy_id();                                  // "gil-tracking-policy/1:sha256:…"
```

- `includes_file` 은 파일 경로만 판정한다. 디렉터리 경로를 넘기면 「그 이름의 파일」로 판정된다(디렉터리 전용 규칙은 안 맞는다).
- 중첩 `.gil` 을 지나는 경로는 `Err` 다. B 는 이를 「제외」로 읽지 말고 거절로 다룬다.
- `policy_id` 는 OS 경로 검증과 무관하다. 과거 Snapshot 에 ID 를 지어 붙이지 않는다(proposal §3).

## 남은 연결 (통합 단계)

1. **제품 기본 capture 와 저장 정책 provenance 연결은 하지 않았다.** `capture_with_policy`(blob 저장 포함)도 없다.
   Snapshot 기록에 `policy_id` 를 넣는 것은 저장 schema 변경이므로 통합 담당자의 format 결정과 함께 한다.
2. Session/Verify close/`gil start` 는 여전히 기본 `observe`/`capture` 를 쓴다. 자동 연결하지 않았다.
3. 제품 기본 정책(`.gitignore` 자동 수집 + `Prune` 유력)은 실기 결과를 보고 정한다.
4. `src/lib.rs` 의 공개 export 는 건드리지 않았다. 지금은 `pub(crate)` 재수출뿐이다.

## 발견한 결함·마찰 (범위 밖, 수정하지 않음)

- **최상위 ignored 심볼릭 링크 마찰**: `.venv -> …` 처럼 루트 바로 아래의 링크가 ignore 규칙에 맞아도 기존 §3.4 링크 계약에
  따라 관측 전체를 거절한다(시험 `an_excluded_symlink_is_still_refused_when_enumerated`). 실제 Python/Node 프로젝트에서 흔하다.
  완화하려면 Artifact Model §3.4 를 먼저 바꿔야 하며 이 세션은 바꾸지 않았다.
- `Prune` 모드는 제외된 디렉터리 안의 링크·중첩 `.gil` 을 검사하지 않는다. 계약에 드러냈고 결과 `excluded` 에 남지만,
  통합 담당자가 기본 모드를 정할 때 알아야 한다.
- 기존 `ObserveError` 에 variant 를 더하지 않으려고 `PolicyObserveError { Observe, Policy }` 를 따로 두었다.
  restore 가 `From<ObserveError>` 를 갖고 있어 공유 열거형을 넓히면 호출자 의미가 바뀌기 때문이다.

## 커밋

로컬 commit 1개 — SHA 는 아래 세션 보고에 적는다. push/PR/병합 없음.
