//! 독립 추적 정책 엔진 — **시스템 보호와 승인된 `.gilignore` 원문 하나로** 추적 범위를 정한다.
//!
//! 구현 순서 2번([구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md) §3)의 독립 검사
//! 단위다. 정본은 [Artifact Model §3.6](../../../spec/GIL_Artifact_Model_v0.1.md)과
//! [공유 저장·전이 계약 §5.1·§10](../../../spec/GIL_Storage_Transition_Contract_v0.1.md)이다.
//!
//! ```text
//! 승인된 원문 바이트 ──compile──▶ ApprovedPolicy ──identity──▶ engine + sha256(원문)
//!   (호출자가 준다)                  │
//!                                    ├─ decide(path, kind) ──▶ Decision     경로 글자만 본다
//!                                    └─ evaluate_tree(root) ──▶ TreeEvaluation
//!                                          추적 / 제외(이유·내부 미확인) / 재포함 탐색 근거 / 차단 사유
//! ```
//!
//! # 이 엔진이 하지 않는 것
//!
//! - **승인 여부를 정하지 않는다.** `compile` 에 들어온 바이트를 「승인된 원문」으로 다루는 것은
//!   호출자의 계약이다. 디스크의 `.gilignore` 를 읽었다는 이유로 그것을 승인 정책으로 삼지 않으며,
//!   이 모듈은 어떤 `.gilignore`·`.gitignore` 파일도 읽지 않는다.
//! - **아무것도 쓰지 않고 파일 내용을 읽지 않는다.** 디렉터리 목록과 항목 종류만 본다. 보관·지문
//!   계산·복원·`.gilignore` 생성/동기화는 후속 계층(3–6번)의 일이다.
//! - **제품 경로에 연결되지 않았다.** 기본 `observe`/`capture`, 복원, M4-R 승인 복구(옛
//!   [`TrackingPolicy`](super::TrackingPolicy) V1 사용)는 이 엔진을 부르지 않는다.
//!
//! # 옛 엔진(`tracking_policy`, `gil-tracking-policy/1`)과 다른 점
//!
//! | | 옛 V1 | 이 엔진 (`gil-gilignore/1`) |
//! |---|---|---|
//! | 입력 | `.gitignore` 여러 개(위치별) | 승인된 루트 `.gilignore` 원문 하나 |
//! | 제외된 부모 아래의 `!` | 효과 없음(Git 과 같음) | 다시 포함한다 — 경로와 조상 중 어디에든 맞는 **마지막 규칙**이 결정 |
//! | 제외된 폴더 | 부르는 쪽이 Prune/Inspect 선택 | 최종 정책상 완전히 제외면 생략, 재포함 가능성이 있으면 탐색 |
//! | 제외된 링크 | 열거되면 거절 | 대상을 따라가지 않고 건너뜀. 재포함·추적 영역·재포함 경로 위의 링크는 거절 |
//! | 정체성 | 위치·원문을 묶은 구조 해시 | `engine` + 원문 바이트의 sha256 (= 객체 창고 주소) |
//!
//! 옛 V1 은 바뀌지 않는다. 같은 문법 해석기(토큰·`**`·문자 클래스·매칭)를 재사용할 뿐이다.

use std::fmt;
use std::path::Path;

use super::tracking_policy::{self as grammar, ClassItem, Component, Rule, Token};
use super::{ContentDigest, EntryPath, GIL_DIR};

mod recommend;
mod walk;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use recommend::{
    GitignoreReview, Impact, Reason, Recommendation, RecommendationCategory,
    RecommendationSource, SkippedGitignoreLine, SystemProtectionNotice, catalog,
    project_specific, review_gitignore, system_protections, DEPENDENCY_MANIFESTS,
};

/// 정책 파일의 이름. **프로젝트 루트의 이것 하나만** 정책 원문의 자리다.
pub(crate) const GILIGNORE: &str = ".gilignore";

/// Git metadata 의 이름 — 어느 깊이든 시스템 보호.
const GIT_DIR: &str = super::tracking_policy::GIT_DIR;

// ── 엔진 버전과 정책 정체성 ────────────────────────────────────────────────

/// 규칙 해석 **의미**의 버전 — 공유 계약 §5.1 의 `engine`.
///
/// 옛 [`PolicyVersion`](super::PolicyVersion) 과 **다른 이름공간**이다. 옛 V1 은 `.gitignore`
/// 입력 의미이고, 이 열거형의 1 은 승인된 `.gilignore` 하나의 의미다. 번호가 같아도 섞이지
/// 않도록 정체성 글자의 머리(`label`)가 다르다. 프로젝트 저장 형식 번호(8/9, 10/11)와도 무관하다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EngineVersion {
    /// 이 모듈이 처음 정의한 의미 (2026-11-20).
    V1,
}

impl EngineVersion {
    pub(crate) fn number(self) -> u32 {
        match self {
            EngineVersion::V1 => 1,
        }
    }

    /// 기록에서 읽은 번호 — 모르는 번호는 `None` 이다(계약 T2: 아는 버전만).
    pub(crate) fn from_number(number: u32) -> Option<EngineVersion> {
        match number {
            1 => Some(EngineVersion::V1),
            _ => None,
        }
    }

    /// 정체성 글자의 머리. 옛 `gil-tracking-policy/1` 과 겹치지 않는다.
    pub(crate) fn label(self) -> &'static str {
        match self {
            EngineVersion::V1 => "gil-gilignore/1",
        }
    }
}

/// 정책의 정체성 — **엔진 버전과 승인된 원문 바이트, 그 둘뿐이다**(계약 §5.1).
///
/// `.gitignore` 의 내용·존재, 사용자 전역 Git 설정, `.git/info/exclude`, 환경 변수, 프로젝트의
/// 절대 위치, 파일 시스템 관측 결과는 어디에도 들어가지 않는다(T10). `rules` 는 원문 바이트의
/// 평범한 sha256 이므로 객체 창고가 같은 바이트에 주는 주소와 같다.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PolicyIdentity {
    engine: EngineVersion,
    rules: ContentDigest,
}

impl PolicyIdentity {
    pub(crate) fn engine(&self) -> EngineVersion {
        self.engine
    }

    /// 승인된 원문 바이트의 sha256 — 계약 §5.1 의 `rules.digest`.
    pub(crate) fn rules_digest(&self) -> &ContentDigest {
        &self.rules
    }

    /// 사람과 기록이 비교하는 한 줄 — `gil-gilignore/1:sha256:<64 hex>`.
    pub(crate) fn id(&self) -> String {
        format!("{}:sha256:{}", self.engine.label(), self.rules.hex())
    }
}

impl fmt::Display for PolicyIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id())
    }
}

// ── 승인 정책 ──────────────────────────────────────────────────────────────

/// 컴파일된 승인 정책. 만들어진 순간 정체성이 정해지고 바뀌지 않는다.
#[derive(Debug, Clone)]
pub(crate) struct ApprovedPolicy {
    identity: PolicyIdentity,
    /// 원문 순서 그대로 — 아래 줄이 위 줄보다 우선한다.
    rules: Vec<Rule>,
}

impl ApprovedPolicy {
    /// 승인된 원문 바이트를 해석한다.
    ///
    /// **승인은 여기서 정하지 않는다.** 이 함수는 「호출자가 승인됐다고 건넨 바이트」를 받는다.
    /// 해석할 수 없는 줄이 하나라도 있으면 정책을 세우지 않고 **모든** 문제 줄을 위치·이유와
    /// 함께 돌려준다 — 조용히 무시하거나 고쳐서 짐작한 정책을 만들지 않는다.
    pub(crate) fn compile(
        engine: EngineVersion,
        approved: &[u8],
    ) -> Result<ApprovedPolicy, RulesRejected> {
        let identity = PolicyIdentity {
            engine,
            rules: super::digest_of(approved).expect("메모리의 바이트는 언제나 읽힌다"),
        };
        let rules = parse(approved).map_err(|problems| RulesRejected {
            identity: identity.clone(),
            problems,
        })?;
        Ok(ApprovedPolicy { identity, rules })
    }

    pub(crate) fn identity(&self) -> &PolicyIdentity {
        &self.identity
    }

    /// 한 경로의 최종 판정 — **경로 글자만** 본다. 파일 시스템을 열지 않는다.
    ///
    /// 연결 파일처럼 대상 종류를 모르는 항목은 [`evaluate_tree`] 가 따로 다룬다.
    pub(crate) fn decide(&self, path: &EntryPath, kind: PathKind) -> Decision {
        let parts: Vec<&str> = path.as_str().split('/').collect();
        self.decide_parts(&parts, kind)
    }

    fn decide_parts(&self, parts: &[&str], kind: PathKind) -> Decision {
        if let Some(fixed) = name_boundary(parts, kind) {
            return fixed;
        }
        match self.deciding_rule(parts, kind) {
            None => Decision::Tracked { by: None },
            Some(rule) if rule.negated => Decision::Tracked {
                by: Some(RuleRef::of(rule)),
            },
            Some(rule) => Decision::Excluded {
                rule: RuleRef::of(rule),
            },
        }
    }

    /// 경로 **자신과 모든 조상** 가운데 어디에든 맞는 규칙 중 원문의 가장 아래 줄.
    ///
    /// 조상은 디렉터리로, 마지막 구성 요소는 `kind` 로 본다. 그래서 `data/` 다음 줄의
    /// `!data/keep.csv` 는 `data/keep.csv` 를 다시 포함하고, 반대 순서면 `data/` 가 이긴다.
    fn deciding_rule(&self, parts: &[&str], kind: PathKind) -> Option<&Rule> {
        let last = parts.len();
        self.rules.iter().rev().find(|rule| {
            (1..=last).any(|depth| {
                let is_dir = depth < last || kind == PathKind::Directory;
                (!rule.dir_only || is_dir) && grammar::match_components(&rule.components, &parts[..depth])
            })
        })
    }

    /// 제외된 디렉터리의 안을 **생략해도 되는가.**
    ///
    /// 그 디렉터리를 제외한 규칙보다 아래 줄에 있는 포함 예외(`!`) 가운데 그 **아래의 어떤
    /// 경로에라도** 맞을 수 있는 것이 하나도 없을 때만 생략한다. 맞을 수 있는지는 규칙을 그
    /// 디렉터리까지 따라가 본 뒤 남은 패턴이 있는가로 판정한다 — 남은 패턴이 실제 이름에 맞을지는
    /// 모른다고 보고 **탐색 쪽으로** 기운다. 잘못된 생략보다 불필요한 탐색이 낫다.
    fn interior(&self, dir: &[&str], deciding: &RuleRef) -> Interior {
        let because: Vec<RuleRef> = self
            .rules
            .iter()
            .filter(|rule| rule.line > deciding.line && rule.negated)
            .filter(|rule| may_match_below(&rule.components, dir))
            .map(RuleRef::of)
            .collect();
        match because.is_empty() {
            true => Interior::Skip,
            false => Interior::Explore { because },
        }
    }

    /// 시스템 보호 자리에 닿는 포함 예외 — 그 자리 자체에 맞거나(디렉터리면) 그 아래에 맞을 수 있는 `!` 줄.
    ///
    /// 보호 전용 줄은 해석 단계에서 이미 거절했다. 여기 남는 것은 다른 경로에도 맞을 수 있어 받아들인
    /// 넓은 줄이다. 판정은 보호가 이기며, 평가 결과가 이 사실을 알림으로 돌려준다.
    fn reinclusions_into(&self, parts: &[&str], kind: PathKind) -> Vec<RuleRef> {
        self.rules
            .iter()
            .filter(|rule| rule.negated)
            .filter(|rule| {
                let here = (!rule.dir_only || kind == PathKind::Directory)
                    && grammar::match_components(&rule.components, parts);
                here || (kind == PathKind::Directory && may_match_below(&rule.components, parts))
            })
            .map(RuleRef::of)
            .collect()
    }
}

/// 규칙이 판정하기 전에 **이름과 위치만으로** 정해지는 자리.
///
/// 순서는 왼쪽 구성 요소부터다: `.git` → 시스템 보호, 루트 `.gil` 디렉터리 → 내부 저장소,
/// 더 깊은 `.gil` 디렉터리 → 중첩 경계, 루트 `.gilignore` → 정책 파일, 더 깊은 `.gilignore` →
/// 하위 정책 파일 자리. 마지막 구성 요소가 `.gil` 인 **일반 파일**은 보통 경로다(§3.4).
///
/// **이름 비교는 사용자 패턴과 다르다.** 사용자 규칙은 대소문자를 구분하지만, 시스템 이름은
/// [`same_system_name`] 으로 대소문자 변형까지 같은 이름으로 본다 — macOS·Windows 의 기본 파일
/// 시스템에서 `.GIL/state.yaml` 은 실제 `.gil/state.yaml` 이다. 대소문자를 구분하는 파일 시스템의
/// 별개 항목(`.GIL/`)도 같은 규칙으로 다룬다: 그 기록이 다른 기계에서는 보호 대상과 겹치고, 같은
/// 트리의 평가가 운영체제마다 달라지지 않게 하기 위해서다. 그런 변형은 결과의 알림에 따로 남는다.
fn name_boundary(parts: &[&str], kind: PathKind) -> Option<Decision> {
    let last = parts.len() - 1;
    for (index, part) in parts.iter().enumerate() {
        let prefix = || prefix_path(parts, index);
        if same_system_name(part, GIT_DIR) {
            return Some(Decision::Protected {
                protection: Protection::GitMetadata,
                at: prefix(),
            });
        }
        if same_system_name(part, GIL_DIR) && (index < last || kind == PathKind::Directory) {
            return Some(match index {
                0 => Decision::Protected {
                    protection: Protection::InternalStore,
                    at: prefix(),
                },
                _ => Decision::NestedGilBoundary { at: prefix() },
            });
        }
        if same_system_name(part, GILIGNORE) {
            return Some(match index {
                0 => Decision::Protected {
                    protection: Protection::PolicyFile,
                    at: prefix(),
                },
                _ => Decision::NestedPolicyFile { at: prefix() },
            });
        }
    }
    None
}

/// `name` 이 시스템 이름 `canonical`(ASCII 소문자)과 **파일 시스템에서 같은 이름일 수 있는가.**
///
/// 글자마다 같거나, 소문자 변환 또는 대문자 변환 한 글자가 같으면 같다고 본다. ASCII 대소문자
/// (`.GIL`, `.Git`)와 대문자가 ASCII 로 가는 글자(`ı` → `I`, NTFS 대문자 표)가 여기 걸린다. 정규형
/// 차이는 이 이름들에 없다(분해되는 글자가 없다).
pub(super) fn same_system_name(name: &str, canonical: &str) -> bool {
    let mut got = name.chars();
    let mut want = canonical.chars();
    loop {
        match (got.next(), want.next()) {
            (None, None) => return true,
            (Some(c), Some(w)) if char_aliases(c, w) => {}
            _ => return false,
        }
    }
}

fn char_aliases(c: char, want: char) -> bool {
    fn single(mut chars: impl Iterator<Item = char>) -> Option<char> {
        let first = chars.next()?;
        chars.next().is_none().then_some(first)
    }
    c == want
        || single(c.to_lowercase()) == Some(want)
        || single(c.to_uppercase()) == Some(want.to_ascii_uppercase())
}

fn prefix_path(parts: &[&str], index: usize) -> EntryPath {
    EntryPath::parse(&parts[..=index].join("/")).expect("경로의 앞부분도 경로 계약을 지킨다")
}

/// 규칙이 `dir` **아래**(한 단계 이상 더 깊은 곳)의 어떤 경로에 맞을 수 있는가 — 보수적으로.
///
/// 패턴을 `dir` 의 구성 요소로 따라간 뒤 패턴이 남는 자리가 하나라도 있으면 참이다. 남은 패턴이
/// 실제로 어떤 이름에 맞는지는 묻지 않는다(언제나 맞을 수 있다고 본다).
fn may_match_below(pattern: &[Component], dir: &[&str]) -> bool {
    let mut states = closure(pattern, vec![0]);
    for part in dir {
        let chars: Vec<char> = part.chars().collect();
        let mut next = Vec::new();
        for &at in &states {
            match pattern.get(at) {
                Some(Component::Any) => next.push(at),
                Some(Component::Glob(tokens)) if grammar::match_glob(tokens, &chars) => {
                    next.push(at + 1)
                }
                _ => {}
            }
        }
        states = closure(pattern, next);
        if states.is_empty() {
            return false;
        }
    }
    states.iter().any(|&at| at < pattern.len())
}

/// `**` 는 0개의 구성 요소에도 맞는다 — 그 뒤 자리도 함께 도달한다.
fn closure(pattern: &[Component], mut states: Vec<usize>) -> Vec<usize> {
    let mut index = 0;
    while index < states.len() {
        let at = states[index];
        if matches!(pattern.get(at), Some(Component::Any)) && !states.contains(&(at + 1)) {
            states.push(at + 1);
        }
        index += 1;
    }
    states.sort_unstable();
    states.dedup();
    states
}

// ── 원문 해석 ──────────────────────────────────────────────────────────────

/// `\` 뒤에 올 수 있는 글자 — 문법상 특별한 글자와 공백뿐이다.
///
/// `src\gen` 처럼 Windows 구분자로 쓴 `\` 는 Git 문법에서는 조용히 `srcgen` 이 된다. Artifact 경로에는
/// `\` 가 들어올 수 없으므로(§3.3) 그런 escape 는 의도와 다른 규칙이 될 뿐이다. 거절한다.
const ESCAPABLE: &[char] = &['\\', '*', '?', '[', ']', '!', '#', ' ', '-', '^'];

fn parse(bytes: &[u8]) -> Result<Vec<Rule>, Vec<RuleProblem>> {
    let mut rules = Vec::new();
    let mut problems = Vec::new();
    // 줄마다 따로 읽는다 — 한 줄의 인코딩 문제가 다른 줄의 문제를 가리지 않게.
    for (index, raw) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let number = index + 1;
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let raw = match (number, raw.strip_prefix(b"\xEF\xBB\xBF")) {
            (1, Some(rest)) => {
                problems.push(RuleProblem {
                    line: 1,
                    text: String::from_utf8_lossy(rest).into_owned(),
                    reason: ProblemReason::ByteOrderMark,
                });
                rest
            }
            _ => raw,
        };
        let Ok(line) = std::str::from_utf8(raw) else {
            problems.push(RuleProblem {
                line: number,
                text: String::from_utf8_lossy(raw).into_owned(),
                reason: ProblemReason::NotUtf8,
            });
            continue;
        };
        let problem = |reason| RuleProblem {
            line: number,
            text: line.to_string(),
            reason,
        };
        // 줄 가운데의 CR — CR 만으로 줄을 나눈 파일은 한 줄이 되어 조용히 아무것에도 맞지 않는다.
        if line.contains('\r') {
            problems.push(problem(ProblemReason::CarriageReturn));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(found) = bare_backslash(line) {
            problems.push(problem(ProblemReason::BackslashNotEscape { found }));
            continue;
        }
        let rule = match grammar::compile_line(line, number) {
            Ok(Some(rule)) => rule,
            Ok(None) => continue,
            Err(reason) => {
                problems.push(problem(ProblemReason::Grammar(reason)));
                continue;
            }
        };
        if rule.components.iter().any(is_dot_component) {
            problems.push(problem(ProblemReason::DotComponent));
            continue;
        }
        if rule.negated && reincludes_only_protected(&rule) {
            problems.push(problem(ProblemReason::ReincludesProtectedOnly));
            continue;
        }
        rules.push(rule);
    }
    match problems.is_empty() {
        true => Ok(rules),
        false => Err(problems),
    }
}

/// 특별한 글자가 아닌 것을 escape 한 `\` 를 찾는다. 줄 끝의 `\` 는 문법 해석기가 거절한다.
fn bare_backslash(line: &str) -> Option<char> {
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if ESCAPABLE.contains(&next) => {}
                Some(next) => return Some(next),
                None => return None,
            }
        }
    }
    None
}

/// 와일드카드 없이 글자만으로 된 구성 요소의 글자.
fn literal(component: &Component) -> Option<String> {
    match component {
        Component::Any => None,
        Component::Glob(tokens) => tokens
            .iter()
            .map(|token| match token {
                Token::Literal(c) => Some(*c),
                _ => None,
            })
            .collect(),
    }
}

/// `.` 이나 `..` 구성 요소는 Artifact 경로에 없다 — 그런 줄은 아무것도 가리키지 않는다.
fn is_dot_component(component: &Component) -> bool {
    matches!(literal(component).as_deref(), Some(".") | Some(".."))
}

/// 포함 예외가 **시스템 보호나 중첩 경계만** 가리키는가.
///
/// 그런 `!` 줄은 어떤 경로도 추적 대상으로 만들 수 없다. 「포함한다고 보여 준 파일이 실제로는
/// 제외되는 모순」(§3.6)을 승인 전에 드러내려고 거절한다. 판정은 **구성 요소가 맞을 수 있는 이름의
/// 집합**으로 한다: 글자·한 글자 클래스(`[t]`, `[tT]`)·좁은 범위만으로 된 구성 요소가 보호 이름(과 그
/// 대소문자 변형)에만 맞으면 보호 전용이다. `?`·`*`·부정 클래스·이름 클래스가 섞이면 다른 이름에도
/// 맞을 수 있으므로 받아들이고, 트리 평가가 그 줄이 보호 대상에 닿았음을 알림으로 남긴다.
fn reincludes_only_protected(rule: &Rule) -> bool {
    let last = rule.components.len() - 1;
    rule.components.iter().enumerate().any(|(index, component)| {
        only_names(component, GIT_DIR)
            || (only_names(component, GIL_DIR) && (index < last || rule.dir_only))
            || only_names(component, GILIGNORE)
    })
}

/// 구성 요소가 `canonical` 과 같은 이름(대소문자 변형 포함)**에만** 맞는가.
fn only_names(component: &Component, canonical: &str) -> bool {
    let Component::Glob(tokens) = component else {
        return false;
    };
    let want: Vec<char> = canonical.chars().collect();
    tokens.len() == want.len()
        && tokens.iter().zip(&want).all(|(token, w)| match token {
            Token::Literal(c) => char_aliases(*c, *w),
            Token::Class {
                negated: false,
                items,
            } => {
                !items.is_empty()
                    && items.iter().all(|item| match item {
                        ClassItem::Char(c) => char_aliases(*c, *w),
                        ClassItem::Range(lo, hi) => {
                            (*hi as u32) - (*lo as u32) < 64
                                && (*lo..=*hi).all(|c| char_aliases(c, *w))
                        }
                        ClassItem::Named(_) => false,
                    })
            }
            _ => false,
        })
}

// ── 판정 ───────────────────────────────────────────────────────────────────

/// 판정을 묻는 경로의 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathKind {
    File,
    Directory,
}

/// 사용자 규칙으로 되살릴 수 없는 시스템 보호.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Protection {
    /// `.git` — 어느 깊이든, 파일이든 디렉터리든.
    GitMetadata,
    /// 루트 `.gil/` — 현재 프로젝트의 GIL 내부 기록.
    InternalStore,
    /// 루트 `.gilignore` — 정책 파일. 원문 보관은 승인 기록(3번 작업)의 일이다.
    PolicyFile,
}

impl Protection {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Protection::GitMetadata => "git_metadata",
            Protection::InternalStore => "gil_internal_store",
            Protection::PolicyFile => "policy_file",
        }
    }
}

impl fmt::Display for Protection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Protection::GitMetadata => "Git 내부 기록(.git)",
            Protection::InternalStore => "GIL 내부 기록(루트 .gil)",
            Protection::PolicyFile => "정책 파일(루트 .gilignore)",
        })
    }
}

/// 판정을 내린 승인 규칙 — 원문의 몇째 줄인지.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuleRef {
    /// 원문 안의 줄 번호 (1부터).
    pub(crate) line: usize,
    /// 그 줄의 글자 (끝의 `\r` 만 벗긴 것). 규칙은 패턴이며 파일 내용이 아니다.
    pub(crate) pattern: String,
    pub(crate) negated: bool,
}

impl RuleRef {
    fn of(rule: &Rule) -> RuleRef {
        RuleRef {
            line: rule.line,
            pattern: rule.shown.clone(),
            negated: rule.negated,
        }
    }
}

impl fmt::Display for RuleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".gilignore {}행 `{}`", self.line, self.pattern)
    }
}

/// 한 경로의 최종 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    /// 추적 대상이다. `by` 는 그렇게 정한 포함 예외 — 아무 규칙도 맞지 않았으면 `None`.
    Tracked { by: Option<RuleRef> },
    /// 승인 규칙이 제외했다.
    Excluded { rule: RuleRef },
    /// 시스템 보호 — 어떤 규칙으로도 되살릴 수 없다. `at` 은 보호가 시작되는 자리.
    Protected { protection: Protection, at: EntryPath },
    /// 루트가 아닌 `.gilignore` 의 자리 — 적용하지도, 일반 파일로 다루지도 않는다.
    NestedPolicyFile { at: EntryPath },
    /// 중첩 GIL 저장소 — 기존 거절 경계. 「제외」라고 답하지 않는다.
    NestedGilBoundary { at: EntryPath },
}

impl Decision {
    pub(crate) fn is_tracked(&self) -> bool {
        matches!(self, Decision::Tracked { .. })
    }
}

/// 제외된 디렉터리의 안을 어떻게 했는가.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Interior {
    /// 최종 정책상 완전히 제외 — **들어가지 않았다.** 안의 항목은 확인하지 않았다.
    Skip,
    /// 아래 포함 예외가 그 안에 맞을 수 있어 들어가 보았다.
    Explore { because: Vec<RuleRef> },
}

// ── 해석 오류 ──────────────────────────────────────────────────────────────

/// 원문을 정책으로 세우지 못했다 — **모든** 문제 줄과 함께.
///
/// `identity` 는 거절된 원문의 정체성이다. 승인 화면이 「어느 원문이 거절됐는가」를 묶어 말할 수
/// 있게 하려는 것이며, 적용 가능한 정책이라는 뜻이 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RulesRejected {
    pub(crate) identity: PolicyIdentity,
    pub(crate) problems: Vec<RuleProblem>,
}

impl fmt::Display for RulesRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            ".gilignore 원문의 {}개 줄을 해석할 수 없어 정책을 세우지 않았다. 짐작해서 넘기지 않는다.",
            self.problems.len()
        )?;
        for problem in &self.problems {
            writeln!(f, "- {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for RulesRejected {}

/// 해석할 수 없는 줄 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleProblem {
    /// 1부터 센 줄 번호.
    pub(crate) line: usize,
    /// 그 줄의 글자(UTF-8 이 아니면 대체 문자로 보인다).
    pub(crate) text: String,
    pub(crate) reason: ProblemReason,
}

impl fmt::Display for RuleProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}행 `{}` — {}", self.line, self.text, self.reason)
    }
}

/// 왜 해석하지 않았는가 — 안정된 코드와 사람의 말.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProblemReason {
    /// 파일 머리의 UTF-8 BOM. 첫 규칙에 보이지 않는 글자가 붙어 조용히 맞지 않게 된다.
    ByteOrderMark,
    NotUtf8,
    /// 줄 끝이 아닌 자리의 CR(`\r`) — CR 만으로 줄을 나눈 파일 등.
    CarriageReturn,
    /// 기존 문법 해석기의 거절(닫히지 않은 `[`, 줄 끝 `\`, `//`, 구분자만 있는 줄, 모르는 `[:name:]` 등).
    Grammar(&'static str),
    /// 특별한 글자가 아닌 것을 escape 한 `\` — Windows 경로 구분자로 쓴 경우가 많다.
    BackslashNotEscape { found: char },
    /// `.` 이나 `..` 구성 요소.
    DotComponent,
    /// 시스템 보호(`.git`, 루트 `.gil`, `.gilignore` 와 그 대소문자 변형)나 중첩 경계만 가리키는 포함 예외.
    ReincludesProtectedOnly,
}

impl ProblemReason {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            ProblemReason::ByteOrderMark => "byte_order_mark",
            ProblemReason::NotUtf8 => "not_utf8",
            ProblemReason::CarriageReturn => "carriage_return",
            ProblemReason::Grammar(_) => "grammar",
            ProblemReason::BackslashNotEscape { .. } => "backslash_not_escape",
            ProblemReason::DotComponent => "dot_component",
            ProblemReason::ReincludesProtectedOnly => "reincludes_protected_only",
        }
    }
}

impl fmt::Display for ProblemReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProblemReason::ByteOrderMark => f.write_str(
                "파일 머리에 UTF-8 BOM 이 있다. 첫 규칙에 보이지 않는 글자가 붙으므로 BOM 없이 저장한다",
            ),
            ProblemReason::NotUtf8 => f.write_str("UTF-8 로 읽을 수 없다"),
            ProblemReason::CarriageReturn => f.write_str(
                "줄 가운데에 CR 이 있다. 줄바꿈은 LF 나 CRLF 로 쓴다",
            ),
            ProblemReason::Grammar(why) => f.write_str(why),
            ProblemReason::BackslashNotEscape { found } => write!(
                f,
                "`\\{found}` — `\\` 는 특별한 글자(`* ? [ ] ! # \\` 와 공백)의 escape 에만 쓴다. \
                 경로 구분자는 `/` 다"
            ),
            ProblemReason::DotComponent => {
                f.write_str("`.` 이나 `..` 구성 요소는 프로젝트 상대 경로에 없다")
            }
            ProblemReason::ReincludesProtectedOnly => f.write_str(
                "이 포함 예외는 시스템 보호 대상(.git, 루트 .gil, .gilignore)이나 중첩 GIL 경계만 \
                 가리켜 아무것도 추적하게 만들 수 없다",
            ),
        }
    }
}

// ── 트리 평가 ──────────────────────────────────────────────────────────────

#[allow(unused_imports)]
pub(crate) use walk::{
    Blocker, EntryKind, EvaluationError, ExcludedEntry, ExclusionReason, Notice, TrackedFile,
    TreeEvaluation,
};

/// 승인 정책으로 프로젝트 폴더를 **읽기 전용으로** 평가한다.
///
/// 디렉터리 목록과 항목 종류만 본다(파일 내용은 읽지 않는다). 최종 정책상 완전히 제외된
/// 폴더는 들어가지 않고, 재포함 가능성이 있는 제외 폴더만 들어간다. 연결 파일은 따라가지 않는다.
/// 두 번 훑어 같을 때만 결과를 돌려준다 — 기본 관측의 안정 관측 규칙과 같다(§6).
pub(crate) fn evaluate_tree(
    root: &Path,
    policy: &ApprovedPolicy,
) -> Result<TreeEvaluation, EvaluationError> {
    walk::evaluate_twice(root, policy, || {})
}
