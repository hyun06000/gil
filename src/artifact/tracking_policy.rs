//! 추적 정책 — **무엇이 세계에 속하는가를 명시적인 규칙으로 정한다.**
//!
//! 기본 [`observe`](super::observe) 는 루트 `.gil/` 하나만 빼고 **모든** 일반 파일을 세계로
//! 본다. `.git/`, `node_modules/`, build 출력, secret 까지 한 세계에 담기므로 일반 사용자
//! 프로젝트에 그대로 쓸 수 없다(Artifact Model §3.6a). 이 모듈은 그 기본 의미를 **건드리지
//! 않고** 정책이 선택된 **새 읽기 경로**를 세운다.
//!
//! ```text
//! IgnoreSource[]  ──compile──▶  TrackingPolicy ──▶ policy_id (fingerprint)
//!                                   │
//!   observe_with_policy(root, &policy, ExcludedDirectories) ──▶ PolicyObservation
//!                                   │                             manifest + policy_id + excluded
//!                                   └─ includes_file(path) → Result<bool, String>   (A → B 연결)
//! ```
//!
//! # 입력은 전부 명시적이다
//!
//! 정책은 **주어진** ignore 원문·상대 위치·정책 버전만으로 만든다. 사용자 global gitignore,
//! `.git/info/exclude`, `core.excludesFile`, `core.ignorecase` 같은 Git 설정, 환경 변수, 현재
//! 작업 디렉터리는 **읽지 않는다.** 프로젝트 트리에서 `.gitignore` 를 모으는 일은
//! [`discover_gitignore_sources`] 가 따로, 주어진 루트 아래에서만 한다.
//!
//! # gitignore 호환 부분집합
//!
//! 이 엔진은 Git 의 ignore 문법 가운데 아래를 구현한다. **전체 gitignore 호환이라고 부르지
//! 않는다** — 구현하지 않은 것은 proposal 문서(`spec/proposals/M3_Tracking_Policy_v0.1.md`)에
//! 적혀 있고, 해석할 수 없는 원문은 조용히 넘기지 않고 [`PolicyError`] 로 거절한다.
//!
//! ```text
//! 빈 줄 · `#` 주석 · `\#` 리터럴         trailing space 무시 (`\ ` 는 보존)
//! `!` 부정 · `\!` 리터럴                  마지막으로 맞는 규칙이 이긴다
//! 끝의 `/` 디렉터리 전용                  앞/중간의 `/` 는 원본 위치에 anchoring
//! `/` 없는 패턴은 어느 깊이에서나         `*` `?` `[...]` `[!...]` `[a-z]` `[[:alpha:]]`
//! `**/` · `/**` · `/**/`                  깊은 `.gitignore` 가 얕은 것보다 우선
//! 제외된 부모 아래는 `!` 로 못 살린다     `.git` 은 어느 깊이든 항상 제외
//! ```
//!
//! # 기존 거절 계약은 약해지지 않는다
//!
//! 정책이 제외한다고 해서 심볼릭 링크·특수 항목·비UTF-8 이름·중첩 `.gil` 을 **받아들이지
//! 않는다.** 열거된 항목은 정책보다 먼저 종류 검사를 지난다. 다만 제외된 **디렉터리 안으로
//! 들어갈지**는 부르는 쪽이 [`ExcludedDirectories`] 로 결정한다 — 들어가지 않으면 그 안의
//! 링크·중첩 저장소는 검사되지 않으며, 그것을 숨기지 않고 결과의 `excluded` 목록에 남긴다.

use std::fmt;
use std::fs::{self, File};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    ContentDigest, EntryPath, GIL_DIR, Manifest, ManifestEntry, ObserveError, ObserveLocation,
};

/// Git metadata 디렉터리(또는 worktree/submodule 의 `.git` 파일). **어느 깊이든 항상 제외한다.**
pub(crate) const GIT_DIR: &str = ".git";

/// Git 이 ignore 원문을 두는 파일 이름. [`discover_gitignore_sources`] 만 이 이름을 찾는다.
pub(crate) const GITIGNORE: &str = ".gitignore";

// ── 정책 버전 ─────────────────────────────────────────────────────────────

/// 규칙을 해석하는 **의미**의 버전. 같은 원문이라도 해석이 달라지면 다른 정책이다.
///
/// 열거형이므로 모르는 번호를 만들 수 없다 — 과거 기록에서 읽은 버전은 여기 없으면 거절한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PolicyVersion {
    /// 이 모듈이 처음 정의한 gitignore 부분집합 의미.
    V1,
}

impl PolicyVersion {
    pub(crate) fn number(self) -> u32 {
        match self {
            PolicyVersion::V1 => 1,
        }
    }

    /// 정책 ID 의 머리. 버전이 바뀌면 이 글자도 바뀐다.
    pub(crate) fn label(self) -> &'static str {
        match self {
            PolicyVersion::V1 => "gil-tracking-policy/1",
        }
    }
}

// ── 입력 ───────────────────────────────────────────────────────────────────

/// 한 자리의 ignore 원문.
///
/// `directory` 는 프로젝트 루트 기준 상대 디렉터리다 — 루트는 빈 글자 `""`, 그 아래는
/// `EntryPath` 와 같은 계약(`/` 구분, `.`/`..` 없음, 절대경로 아님)을 따른다. 원문은
/// **글자 그대로** 보관한다; 줄 끝의 `\r` 도 fingerprint 에 들어가고, 해석할 때만 벗긴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IgnoreSource {
    pub(crate) directory: String,
    pub(crate) text: String,
}

impl IgnoreSource {
    pub(crate) fn new(directory: impl Into<String>, text: impl Into<String>) -> IgnoreSource {
        IgnoreSource {
            directory: directory.into(),
            text: text.into(),
        }
    }

    /// 루트의 원문 — 가장 흔한 꼴.
    pub(crate) fn at_root(text: impl Into<String>) -> IgnoreSource {
        IgnoreSource::new("", text)
    }
}

// ── 정책 ───────────────────────────────────────────────────────────────────

/// 컴파일된 추적 정책. **만들어진 순간 fingerprint 가 정해지고 바뀌지 않는다.**
#[derive(Debug, Clone)]
pub(crate) struct TrackingPolicy {
    version: PolicyVersion,
    /// 상대 위치의 바이트 순서로 정렬된 원본들 — fingerprint 와 평가 순서의 근거.
    sources: Vec<CompiledSource>,
    fingerprint: String,
}

impl TrackingPolicy {
    /// 원문들을 해석해 정책을 세운다.
    ///
    /// 같은 (버전, 원본 집합)은 **주어진 순서와 무관하게** 같은 정책·같은 fingerprint 다 —
    /// 원본은 상대 위치로 canonical 정렬한다. 같은 위치의 원본이 둘이면 어느 것이 이기는지
    /// 정할 수 없으므로 거절한다. 원문 **안의** 줄 순서는 의미(마지막 규칙이 이긴다)이자
    /// fingerprint 의 일부다.
    pub(crate) fn from_sources(
        version: PolicyVersion,
        sources: Vec<IgnoreSource>,
    ) -> Result<TrackingPolicy, PolicyError> {
        let mut compiled: Vec<CompiledSource> = Vec::with_capacity(sources.len());
        for source in sources {
            compiled.push(compile_source(&source)?);
        }
        compiled.sort_by(|left, right| left.key().cmp(right.key()));
        for pair in compiled.windows(2) {
            if pair[0].key() == pair[1].key() {
                return Err(PolicyError::DuplicateSourceLocation {
                    directory: pair[0].shown_directory(),
                });
            }
        }
        let fingerprint = fingerprint_of(version, &compiled);
        Ok(TrackingPolicy {
            version,
            sources: compiled,
            fingerprint,
        })
    }

    /// 규칙이 하나도 없는 정책 — 내장 제외(`.git`·루트 `.gil`)만 적용된다.
    pub(crate) fn builtin_only(version: PolicyVersion) -> TrackingPolicy {
        TrackingPolicy::from_sources(version, Vec::new()).expect("빈 원본 목록은 언제나 해석된다")
    }

    pub(crate) fn version(&self) -> PolicyVersion {
        self.version
    }

    /// 원문·순서·상대 위치·버전에 결정적으로 묶인 SHA-256 — 소문자 16진수.
    pub(crate) fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// 정책의 공개 식별자 — INTERFACES.md 의 `policy_id: String`.
    ///
    /// `gil-tracking-policy/1:sha256:<hex>` 꼴이다. OS 경로 검증 결과와는 무관하며, 같은
    /// 글자는 같은 규칙·같은 해석을 뜻한다.
    pub(crate) fn policy_id(&self) -> String {
        format!("{}:sha256:{}", self.version.label(), self.fingerprint)
    }

    /// 이 **파일** 경로가 추적 후보인가 — INTERFACES.md 의 A → B 연결.
    ///
    /// `Ok(true)` 추적 후보, `Ok(false)` 정책 제외, `Err` 해석/경로 거절. 경로는 `/` 를 쓰는
    /// 프로젝트 상대 파일 경로다. symlink/reparse/TOCTOU 검증이나 파일 조작 권한을 대신하지
    /// 않는다 — 그것은 실제 파일 시스템을 보는 자리의 일이다.
    pub(crate) fn includes_file(&self, relative_path: &str) -> Result<bool, String> {
        let Some(path) = EntryPath::parse(relative_path) else {
            return Err(PolicyError::InvalidPath {
                shown: relative_path.to_string(),
            }
            .to_string());
        };
        self.evaluate(&path)
            .map(|decision| decision.is_included())
            .map_err(|err| err.to_string())
    }

    /// 파일 경로 하나의 판정 — **왜** 그런지까지.
    ///
    /// 부모 디렉터리가 제외되면 그 아래는 어떤 부정 규칙으로도 되살아나지 않는다(Git 과 같다).
    pub(crate) fn evaluate(&self, path: &EntryPath) -> Result<Decision, PolicyError> {
        let parts: Vec<&str> = path.as_str().split('/').collect();
        if let Some(builtin) = builtin_exclusion(&parts)? {
            return Ok(builtin);
        }
        for depth in 1..parts.len() {
            if let Verdict::Ignored(rule) = verdict(&self.sources, &parts[..depth], true) {
                let directory = EntryPath::parse(&parts[..depth].join("/"))
                    .expect("경로의 앞부분도 경로 계약을 지킨다");
                return Ok(Decision::ExcludedByParent { directory, rule });
            }
        }
        Ok(match verdict(&self.sources, &parts, false) {
            Verdict::Ignored(rule) => Decision::Excluded { rule },
            Verdict::Reincluded(_) | Verdict::Unmatched => Decision::Included,
        })
    }

    /// 열거 중인 **한 항목**의 판정 — 부모가 이미 포함됐음을 부르는 쪽이 안다.
    ///
    /// 관측 walk 가 쓴다: 디렉터리에 들어가기 전에 그 디렉터리를 묻고, 들어갔다면 그 안의
    /// 항목은 자기 이름만 다시 묻는다.
    fn verdict_for_entry(&self, parts: &[&str], is_dir: bool) -> Verdict {
        verdict(&self.sources, parts, is_dir)
    }
}

/// `.git` 과 루트 `.gil` 의 내장 판정. 규칙 원문이 무엇이든 먼저 본다.
///
/// - `.git` 구성 요소가 어디에 있든 → 항상 제외.
/// - `.gil/...` (루트 바로 아래 디렉터리) → 현재 프로젝트의 내부 저장소, 제외.
/// - `a/.gil/...` → 중첩 GIL 경계. 기본 관측이 **거절**하는 세계이므로 여기서도 「제외」라고
///   답하지 않는다 — 제외라고 답하면 B 가 그것을 무시해도 되는 것으로 읽는다.
/// - 마지막 구성 요소가 `.gil` 인 것은 일반 파일일 수 있으므로 보통 규칙으로 본다.
fn builtin_exclusion(parts: &[&str]) -> Result<Option<Decision>, PolicyError> {
    let last = parts.len() - 1;
    for (index, part) in parts.iter().enumerate() {
        if *part == GIT_DIR {
            return Ok(Some(Decision::ExcludedBuiltin {
                prefix: prefix_path(parts, index),
                reason: Builtin::GitMetadata,
            }));
        }
        if *part == GIL_DIR && index < last {
            if index == 0 {
                return Ok(Some(Decision::ExcludedBuiltin {
                    prefix: prefix_path(parts, index),
                    reason: Builtin::InternalStore,
                }));
            }
            return Err(PolicyError::NestedGilBoundary {
                path: prefix_path(parts, index),
            });
        }
    }
    Ok(None)
}

fn prefix_path(parts: &[&str], index: usize) -> EntryPath {
    EntryPath::parse(&parts[..=index].join("/")).expect("경로의 앞부분도 경로 계약을 지킨다")
}

/// 한 경로에 대한 정책의 답.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    /// 추적 후보다.
    Included,
    /// 규칙이 이 경로를 제외했다.
    Excluded { rule: RuleRef },
    /// 부모 디렉터리가 제외됐다 — 이 경로를 되살리는 부정 규칙은 효력이 없다.
    ExcludedByParent { directory: EntryPath, rule: RuleRef },
    /// 규칙 원문과 무관하게 언제나 제외되는 자리다.
    ExcludedBuiltin { prefix: EntryPath, reason: Builtin },
}

impl Decision {
    pub(crate) fn is_included(&self) -> bool {
        matches!(self, Decision::Included)
    }
}

/// 원문 없이도 제외되는 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    /// `.git` — Git 자신의 metadata. 어느 깊이에서든.
    GitMetadata,
    /// 루트 `.gil/` — 현재 프로젝트의 GIL 내부 저장소.
    InternalStore,
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Builtin::GitMetadata => f.write_str("Git metadata (.git)"),
            Builtin::InternalStore => f.write_str("GIL 내부 저장소 (루트 .gil)"),
        }
    }
}

/// 판정을 내린 규칙 — 사람이 어느 줄을 고치면 되는지 알도록.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleRef {
    /// 원본의 상대 디렉터리. 루트는 `""`.
    pub(crate) directory: String,
    /// 원문 안의 줄 번호 (1부터).
    pub(crate) line: usize,
    /// 그 줄의 원문 (trailing `\r` 만 벗긴 것).
    pub(crate) pattern: String,
    pub(crate) negated: bool,
}

impl fmt::Display for RuleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = match self.directory.as_str() {
            "" => "(루트)".to_string(),
            dir => dir.to_string(),
        };
        write!(f, "{at} {}행 `{}`", self.line, self.pattern)
    }
}

// ── 컴파일 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CompiledSource {
    /// `""` 이면 루트.
    directory: String,
    /// 디렉터리 구성 요소 수 — 루트는 0.
    depth: usize,
    /// 원문 그대로 — fingerprint 의 재료.
    text: String,
    rules: Vec<Rule>,
}

impl CompiledSource {
    fn key(&self) -> &[u8] {
        self.directory.as_bytes()
    }

    fn shown_directory(&self) -> String {
        self.directory.clone()
    }
}

#[derive(Debug, Clone)]
struct Rule {
    line: usize,
    /// 줄의 원문 (CR 제거). 진단용.
    shown: String,
    negated: bool,
    dir_only: bool,
    components: Vec<Component>,
}

#[derive(Debug, Clone)]
enum Component {
    /// `**` 하나로 된 구성 요소 — 0개 이상의 디렉터리.
    Any,
    Glob(Vec<Token>),
}

#[derive(Debug, Clone)]
enum Token {
    Literal(char),
    /// `?`
    AnyChar,
    /// `*` — `/` 를 넘지 않는다 (구성 요소 안이므로 저절로).
    AnyRun,
    Class { negated: bool, items: Vec<ClassItem> },
}

#[derive(Debug, Clone)]
enum ClassItem {
    Char(char),
    Range(char, char),
    Named(NamedClass),
}

#[derive(Debug, Clone, Copy)]
enum NamedClass {
    Alnum,
    Alpha,
    Blank,
    Cntrl,
    Digit,
    Graph,
    Lower,
    Print,
    Punct,
    Space,
    Upper,
    Xdigit,
}

impl NamedClass {
    fn parse(name: &str) -> Option<NamedClass> {
        Some(match name {
            "alnum" => NamedClass::Alnum,
            "alpha" => NamedClass::Alpha,
            "blank" => NamedClass::Blank,
            "cntrl" => NamedClass::Cntrl,
            "digit" => NamedClass::Digit,
            "graph" => NamedClass::Graph,
            "lower" => NamedClass::Lower,
            "print" => NamedClass::Print,
            "punct" => NamedClass::Punct,
            "space" => NamedClass::Space,
            "upper" => NamedClass::Upper,
            "xdigit" => NamedClass::Xdigit,
            _ => return None,
        })
    }

    /// ASCII 범위의 POSIX 클래스. Git 의 wildmatch 와 같은 범위다.
    fn matches(self, c: char) -> bool {
        match self {
            NamedClass::Alnum => c.is_ascii_alphanumeric(),
            NamedClass::Alpha => c.is_ascii_alphabetic(),
            NamedClass::Blank => c == ' ' || c == '\t',
            NamedClass::Cntrl => c.is_ascii_control(),
            NamedClass::Digit => c.is_ascii_digit(),
            NamedClass::Graph => c.is_ascii_graphic(),
            NamedClass::Lower => c.is_ascii_lowercase(),
            NamedClass::Print => c.is_ascii_graphic() || c == ' ',
            NamedClass::Punct => c.is_ascii_punctuation(),
            NamedClass::Space => matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'),
            NamedClass::Upper => c.is_ascii_uppercase(),
            NamedClass::Xdigit => c.is_ascii_hexdigit(),
        }
    }
}

fn compile_source(source: &IgnoreSource) -> Result<CompiledSource, PolicyError> {
    let depth = match source.directory.as_str() {
        "" => 0,
        dir => match EntryPath::parse(dir) {
            Some(path) => {
                let parts: Vec<&str> = path.as_str().split('/').collect();
                // `.git`·`.gil` 안의 원문은 Git 도 GIL 도 읽지 않는 자리다. 받으면 뜻이 없는
                // 규칙이 fingerprint 에 섞인다.
                if parts.iter().any(|part| *part == GIT_DIR || *part == GIL_DIR) {
                    return Err(PolicyError::InvalidSourceLocation {
                        shown: dir.to_string(),
                        reason: "`.git` 이나 `.gil` 안의 원문은 읽지 않는다",
                    });
                }
                parts.len()
            }
            None => {
                return Err(PolicyError::InvalidSourceLocation {
                    shown: dir.to_string(),
                    reason: "루트는 빈 글자, 그 아래는 `/` 로 잇는 상대 디렉터리여야 한다",
                });
            }
        },
    };

    let mut rules = Vec::new();
    for (index, raw_line) in source.text.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if let Some(rule) = compile_line(line, index + 1).map_err(|reason| {
            PolicyError::InvalidPattern {
                directory: source.directory.clone(),
                line: index + 1,
                pattern: line.to_string(),
                reason,
            }
        })? {
            rules.push(rule);
        }
    }
    Ok(CompiledSource {
        directory: source.directory.clone(),
        depth,
        text: source.text.clone(),
        rules,
    })
}

/// 한 줄을 규칙으로. 빈 줄·주석은 `None`.
fn compile_line(line: &str, number: usize) -> Result<Option<Rule>, &'static str> {
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let shown = line.to_string();
    let mut pattern = trim_trailing_spaces(line);
    if pattern.is_empty() {
        return Ok(None);
    }

    let mut negated = false;
    if let Some(rest) = pattern.strip_prefix('!') {
        negated = true;
        pattern = rest;
    }

    let mut dir_only = false;
    if let Some(rest) = pattern.strip_suffix('/') {
        dir_only = true;
        pattern = rest;
    }
    if pattern.is_empty() {
        return Err("구분자만 남은 패턴은 아무것도 가리키지 않는다");
    }

    // 앞이나 중간의 `/` 는 원본 위치에 anchoring 한다. 없으면 어느 깊이에서나 맞는다.
    let anchored = pattern.starts_with('/') || pattern.contains('/');
    let body = pattern.strip_prefix('/').unwrap_or(pattern);
    if body.is_empty() {
        return Err("구분자만 남은 패턴은 아무것도 가리키지 않는다");
    }

    let mut components = Vec::new();
    if !anchored {
        components.push(Component::Any);
    }
    for part in body.split('/') {
        if part.is_empty() {
            return Err("빈 경로 구성 요소(`//`)는 해석하지 않는다");
        }
        if part == "**" {
            components.push(Component::Any);
        } else {
            components.push(Component::Glob(tokenize(part)?));
        }
    }

    Ok(Some(Rule {
        line: number,
        shown,
        negated,
        dir_only,
        components,
    }))
}

/// 뒤따르는 공백을 지운다 — `\ ` 로 escape 한 것은 남긴다.
fn trim_trailing_spaces(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b' ' {
        let mut backslashes = 0;
        let mut at = end - 1;
        while at > 0 && bytes[at - 1] == b'\\' {
            backslashes += 1;
            at -= 1;
        }
        if backslashes % 2 == 1 {
            break;
        }
        end -= 1;
    }
    &line[..end]
}

/// 구성 요소 하나의 glob 을 token 으로.
fn tokenize(part: &str) -> Result<Vec<Token>, &'static str> {
    let chars: Vec<char> = part.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                let Some(&next) = chars.get(i + 1) else {
                    return Err("줄 끝의 `\\` 는 무엇을 escape 하는지 알 수 없다");
                };
                tokens.push(Token::Literal(next));
                i += 2;
            }
            '*' => {
                // 연속한 `*` 는 하나의 `*` 다 — 구성 요소 안의 `**` 는 특별하지 않다.
                while chars.get(i) == Some(&'*') {
                    i += 1;
                }
                tokens.push(Token::AnyRun);
            }
            '?' => {
                tokens.push(Token::AnyChar);
                i += 1;
            }
            '[' => {
                let (class, used) = parse_class(&chars[i..])?;
                tokens.push(class);
                i += used;
            }
            c => {
                tokens.push(Token::Literal(c));
                i += 1;
            }
        }
    }
    Ok(tokens)
}

/// `[` 에서 시작하는 문자 클래스. 돌려주는 수는 소비한 글자 수.
fn parse_class(chars: &[char]) -> Result<(Token, usize), &'static str> {
    let mut i = 1;
    let mut negated = false;
    if matches!(chars.get(i), Some('!') | Some('^')) {
        negated = true;
        i += 1;
    }
    let mut items = Vec::new();
    let mut first = true;
    loop {
        let Some(&c) = chars.get(i) else {
            return Err("`[` 로 연 문자 클래스가 닫히지 않았다");
        };
        if c == ']' && !first {
            i += 1;
            break;
        }
        first = false;
        if c == '[' && chars.get(i + 1) == Some(&':') {
            let rest = &chars[i + 2..];
            let Some(end) = rest
                .windows(2)
                .position(|pair| pair[0] == ':' && pair[1] == ']')
            else {
                return Err("`[:` 로 연 이름 클래스가 `:]` 로 닫히지 않았다");
            };
            let name: String = rest[..end].iter().collect();
            let Some(named) = NamedClass::parse(&name) else {
                return Err("모르는 이름 클래스다");
            };
            items.push(ClassItem::Named(named));
            i += 2 + end + 2;
            continue;
        }
        let mut lower = c;
        if c == '\\' {
            let Some(&escaped) = chars.get(i + 1) else {
                return Err("클래스 안의 `\\` 뒤에 글자가 없다");
            };
            lower = escaped;
            i += 1;
        }
        i += 1;
        if chars.get(i) == Some(&'-') && chars.get(i + 1).is_some_and(|next| *next != ']') {
            let mut upper = chars[i + 1];
            i += 2;
            if upper == '\\' {
                let Some(&escaped) = chars.get(i) else {
                    return Err("클래스 안의 `\\` 뒤에 글자가 없다");
                };
                upper = escaped;
                i += 1;
            }
            if lower > upper {
                return Err("범위의 시작이 끝보다 크다");
            }
            items.push(ClassItem::Range(lower, upper));
        } else {
            items.push(ClassItem::Char(lower));
        }
    }
    Ok((Token::Class { negated, items }, i))
}

// ── 평가 ───────────────────────────────────────────────────────────────────

enum Verdict {
    Ignored(RuleRef),
    Reincluded(RuleRef),
    Unmatched,
}

/// 경로 구성 요소들에 대해 **어느 규칙이 마지막으로 맞는가.**
///
/// 깊은 원본이 얕은 원본보다 우선하고, 한 원본 안에서는 아래 줄이 위 줄보다 우선한다.
/// 원본은 자기 디렉터리의 **아래**에만 적용된다 — `a/.gitignore` 는 `a` 자신을 판정하지 않는다.
fn verdict(sources: &[CompiledSource], parts: &[&str], is_dir: bool) -> Verdict {
    for source in sources.iter().rev() {
        if source.depth >= parts.len() {
            continue;
        }
        let in_scope = source.depth == 0
            || parts[..source.depth].join("/") == source.directory;
        if !in_scope {
            continue;
        }
        let relative = &parts[source.depth..];
        for rule in source.rules.iter().rev() {
            if rule.dir_only && !is_dir {
                continue;
            }
            if match_components(&rule.components, relative) {
                let reference = RuleRef {
                    directory: source.directory.clone(),
                    line: rule.line,
                    pattern: rule.shown.clone(),
                    negated: rule.negated,
                };
                return match rule.negated {
                    true => Verdict::Reincluded(reference),
                    false => Verdict::Ignored(reference),
                };
            }
        }
    }
    Verdict::Unmatched
}

fn match_components(pattern: &[Component], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((Component::Any, rest)) => {
            if rest.is_empty() {
                // 끝의 `/**` 는 「안의 모든 것」 — 그 디렉터리 자신은 아니다.
                return !path.is_empty();
            }
            let mut skip = 0;
            loop {
                if match_components(rest, &path[skip..]) {
                    return true;
                }
                if skip == path.len() {
                    return false;
                }
                skip += 1;
            }
        }
        Some((Component::Glob(tokens), rest)) => match path.split_first() {
            None => false,
            Some((head, tail)) => {
                let chars: Vec<char> = head.chars().collect();
                match_glob(tokens, &chars) && match_components(rest, tail)
            }
        },
    }
}

fn match_glob(tokens: &[Token], text: &[char]) -> bool {
    match tokens.split_first() {
        None => text.is_empty(),
        Some((Token::AnyRun, rest)) => {
            (0..=text.len()).any(|taken| match_glob(rest, &text[taken..]))
        }
        Some((Token::AnyChar, rest)) => !text.is_empty() && match_glob(rest, &text[1..]),
        Some((Token::Literal(c), rest)) => {
            text.first() == Some(c) && match_glob(rest, &text[1..])
        }
        Some((Token::Class { negated, items }, rest)) => match text.first() {
            None => false,
            Some(&c) => {
                let hit = items.iter().any(|item| match item {
                    ClassItem::Char(want) => c == *want,
                    ClassItem::Range(lo, hi) => (*lo..=*hi).contains(&c),
                    ClassItem::Named(named) => named.matches(c),
                });
                hit != *negated && match_glob(rest, &text[1..])
            }
        },
    }
}

// ── fingerprint ────────────────────────────────────────────────────────────

/// 버전·원본 수·(정렬된) 상대 위치·원문 바이트를 고정 폭 길이와 함께 해시한다.
///
/// 길이를 앞세우므로 `("a", "b")` 와 `("", "ab")` 가 섞이지 않는다. 원문은 **바이트
/// 그대로**다 — CRLF 와 LF, 주석 한 글자도 다른 정책이다. 해석 결과가 같아도 원문이 다르면
/// 다른 ID 인 것은 의도다: 어떤 규칙을 읽어 그 Snapshot 을 만들었는지를 되짚기 위한 값이지,
/// 의미 동등성의 증명이 아니다.
fn fingerprint_of(version: PolicyVersion, sources: &[CompiledSource]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"GIL-TRACKING-POLICY\0");
    hasher.update(version.number().to_be_bytes());
    hasher.update((sources.len() as u64).to_be_bytes());
    for source in sources {
        let directory = source.directory.as_bytes();
        hasher.update((directory.len() as u32).to_be_bytes());
        hasher.update(directory);
        let text = source.text.as_bytes();
        hasher.update((text.len() as u64).to_be_bytes());
        hasher.update(text);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ── 정책 오류 ──────────────────────────────────────────────────────────────

/// 정책을 세우거나 경로를 판정하지 못한 이유. 어느 것이든 **판정을 지어내지 않았다**는 뜻이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyError {
    InvalidSourceLocation {
        shown: String,
        reason: &'static str,
    },
    DuplicateSourceLocation {
        directory: String,
    },
    InvalidPattern {
        directory: String,
        line: usize,
        pattern: String,
        reason: &'static str,
    },
    /// ignore 원문이 UTF-8 이 아니다.
    UnreadableSourceText {
        directory: String,
    },
    /// 판정을 요청한 경로가 Artifact 경로 계약을 어긴다.
    InvalidPath {
        shown: String,
    },
    /// 경로가 중첩 GIL 저장소를 지난다 — 기본 관측이 거절하는 세계라 정책도 답하지 않는다.
    NestedGilBoundary {
        path: EntryPath,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::InvalidSourceLocation { shown, reason } => write!(
                f,
                "ignore 원문의 위치 {shown:?} 를 받을 수 없다 — {reason}."
            ),
            PolicyError::DuplicateSourceLocation { directory } => write!(
                f,
                "같은 위치 {:?} 의 ignore 원문이 둘이다. 어느 것이 이기는지 정할 수 없어 \
                 정책을 세우지 않았다.",
                shown_directory(directory)
            ),
            PolicyError::InvalidPattern {
                directory,
                line,
                pattern,
                reason,
            } => write!(
                f,
                "{} {line}행의 패턴 `{pattern}` 을 해석할 수 없다 — {reason}. \
                 짐작해서 넘기지 않고 정책을 세우지 않았다.",
                shown_directory(directory)
            ),
            PolicyError::UnreadableSourceText { directory } => write!(
                f,
                "{} 의 ignore 원문이 UTF-8 이 아니다. 정책을 세우지 않았다.",
                shown_directory(directory)
            ),
            PolicyError::InvalidPath { shown } => write!(
                f,
                "{shown:?} 는 Artifact 경로가 아니다 — `/` 로 잇는 프로젝트 상대 파일 \
                 경로만 판정한다."
            ),
            PolicyError::NestedGilBoundary { path } => write!(
                f,
                "{path} 는 중첩 GIL 저장소다. 기본 관측이 거절하는 세계이므로 정책은 \
                 제외라고 답하지 않는다."
            ),
        }
    }
}

impl std::error::Error for PolicyError {}

fn shown_directory(directory: &str) -> String {
    match directory {
        "" => "(루트)".to_string(),
        dir => dir.to_string(),
    }
}

// ── 원문 수집 ──────────────────────────────────────────────────────────────

/// 프로젝트 트리의 `.gitignore` 들을 **주어진 루트 아래에서만** 모은다.
///
/// Git 과 같은 순서로 읽는다: 위에서 아래로, 이미 읽은 규칙이 제외한 디렉터리 안으로는
/// 들어가지 않는다. `.git/`·`.gil/` 안은 보지 않는다. `.gitignore` 라는 이름의 심볼릭
/// 링크는 **읽지 않고 거절한다** — 정책 원문이 프로젝트 밖을 가리킬 수 있어서다.
///
/// 이것은 수집이지 관측이 아니다. 다른 심볼릭 링크·특수 항목·중첩 `.gil` 은 여기서 거절하지
/// 않고 지나친다; 그것은 [`observe_with_policy`](super::observe_with_policy) 의 일이다.
/// 돌려주는 목록은 상대 위치의 바이트 순서로 정렬되어 있다.
pub(crate) fn discover_gitignore_sources(
    root: &Path,
) -> Result<Vec<IgnoreSource>, PolicyObserveError> {
    let mut found = Vec::new();
    let mut compiled = Vec::new();
    discover_in(root, &mut Vec::new(), &mut found, &mut compiled)?;
    found.sort_by(|left, right| left.directory.as_bytes().cmp(right.directory.as_bytes()));
    Ok(found)
}

fn discover_in(
    dir: &Path,
    trail: &mut Vec<String>,
    found: &mut Vec<IgnoreSource>,
    compiled: &mut Vec<CompiledSource>,
) -> Result<(), PolicyObserveError> {
    let mut names: Vec<(String, std::path::PathBuf)> = Vec::new();
    let listing = fs::read_dir(dir).map_err(|source| {
        PolicyObserveError::Observe(ObserveError::ReadFailed {
            at: here(trail),
            source: source.to_string(),
        })
    })?;
    for item in listing {
        let item = item.map_err(|source| {
            PolicyObserveError::Observe(ObserveError::ReadFailed {
                at: here(trail),
                source: source.to_string(),
            })
        })?;
        let Some(name) = item.file_name().to_str().map(str::to_string) else {
            return Err(PolicyObserveError::Observe(ObserveError::UnrepresentablePath {
                near: here(trail),
                shown: item.file_name().to_string_lossy().into_owned(),
            }));
        };
        names.push((name, item.path()));
    }
    names.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));

    // 먼저 이 디렉터리의 원문을 읽는다 — 아래로 내려갈지는 그 규칙이 정한다.
    if let Some((_, at)) = names.iter().find(|(name, _)| name == GITIGNORE) {
        let kind = fs::symlink_metadata(at)
            .map_err(|source| {
                PolicyObserveError::Observe(ObserveError::ReadFailed {
                    at: here(trail),
                    source: source.to_string(),
                })
            })?
            .file_type();
        let directory = trail.join("/");
        if kind.is_symlink() {
            let mut parts = trail.clone();
            parts.push(GITIGNORE.to_string());
            let path = EntryPath::from_components(&parts)
                .expect("자취의 조각들은 이미 경로 계약을 지난다");
            return Err(PolicyObserveError::Observe(ObserveError::Symlink { path }));
        }
        if kind.is_file() {
            let bytes = fs::read(at).map_err(|source| {
                PolicyObserveError::Observe(ObserveError::ReadFailed {
                    at: here(trail),
                    source: source.to_string(),
                })
            })?;
            let text = String::from_utf8(bytes).map_err(|_| {
                PolicyObserveError::Policy(PolicyError::UnreadableSourceText {
                    directory: directory.clone(),
                })
            })?;
            let source = IgnoreSource::new(directory, text);
            compiled.push(compile_source(&source).map_err(PolicyObserveError::Policy)?);
            found.push(source);
        }
        // 디렉터리나 특수 항목인 `.gitignore` 는 원문이 아니다 — 관측이 따로 판단한다.
    }

    for (name, at) in &names {
        let kind = fs::symlink_metadata(at)
            .map_err(|source| {
                PolicyObserveError::Observe(ObserveError::ReadFailed {
                    at: here(trail),
                    source: source.to_string(),
                })
            })?
            .file_type();
        if !kind.is_dir() {
            continue;
        }
        if name == GIT_DIR || name == GIL_DIR {
            continue;
        }
        trail.push(name.clone());
        let parts: Vec<&str> = trail.iter().map(String::as_str).collect();
        let usable = EntryPath::from_components(trail).is_some();
        let excluded = matches!(verdict(compiled, &parts, true), Verdict::Ignored(_));
        if usable && !excluded {
            discover_in(at, trail, found, compiled)?;
        }
        trail.pop();
    }
    Ok(())
}

fn here(trail: &[String]) -> ObserveLocation {
    match EntryPath::from_components(trail) {
        Some(path) => ObserveLocation::Entry(path),
        None => ObserveLocation::Root,
    }
}

// ── 정책 적용 관측 ─────────────────────────────────────────────────────────

/// 제외된 **디렉터리** 안을 어떻게 할 것인가 — 부르는 쪽이 결정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExcludedDirectories {
    /// 들어가지 않는다. 빠르지만 그 안의 심볼릭 링크·특수 항목·중첩 `.gil` 은 **검사되지
    /// 않는다.** 결과의 `excluded` 에 그 디렉터리가 남으므로 무엇을 보지 않았는지는 안다.
    Prune,
    /// 들어가서 종류 검사(링크·특수·중첩 `.gil`·UTF-8)만 하고 아무것도 기록하지 않는다.
    /// `node_modules/.bin` 의 링크처럼 흔한 것에서 관측 전체가 거절될 수 있다.
    Inspect,
}

/// 정책을 적용해 관측한 결과 — **manifest 와 그것을 만든 정책의 ID 가 함께 간다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyObservation {
    manifest: Manifest,
    policy_id: String,
    /// manifest 에 담지 않은 항목과 그 이유 — canonical 경로 순서.
    excluded: Vec<ExcludedEntry>,
}

impl PolicyObservation {
    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub(crate) fn policy_id(&self) -> &str {
        &self.policy_id
    }

    pub(crate) fn excluded(&self) -> &[ExcludedEntry] {
        &self.excluded
    }

    /// manifest 와 정책 ID 를 떼어 간다 — 저장 연결은 통합 단계의 일이다.
    pub(crate) fn into_parts(self) -> (Manifest, String, Vec<ExcludedEntry>) {
        (self.manifest, self.policy_id, self.excluded)
    }
}

/// 정책 때문에 세계에 담지 않은 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcludedEntry {
    pub(crate) path: EntryPath,
    pub(crate) kind: ExcludedKind,
    pub(crate) reason: ExclusionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExcludedKind {
    File,
    /// 디렉터리 — [`ExcludedDirectories::Prune`] 이면 안을 보지 않았다.
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExclusionReason {
    Rule(RuleRef),
    Builtin(Builtin),
}

/// 정책 적용 관측이 멈춘 이유.
///
/// 기존 [`ObserveError`] 에 variant 를 더하지 않는다 — 그 열거형은 restore 등 공유 코드가
/// 받으므로, 새 경로의 실패를 거기 섞으면 기존 호출자의 의미가 조용히 넓어진다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyObserveError {
    /// 트리 자체가 기존 거절 계약에 걸렸다 — 링크·특수·중첩 `.gil`·읽기 실패·변경.
    Observe(ObserveError),
    /// 정책을 세우거나 적용하지 못했다.
    Policy(PolicyError),
}

impl fmt::Display for PolicyObserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyObserveError::Observe(err) => write!(f, "{err}"),
            PolicyObserveError::Policy(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PolicyObserveError {}

/// 두 번 훑고 견준다 — 기본 관측과 같은 규칙(Artifact Model §6).
pub(super) fn observe_twice(
    root: &Path,
    policy: &TrackingPolicy,
    mode: ExcludedDirectories,
    between: impl FnOnce(),
) -> Result<PolicyObservation, PolicyObserveError> {
    let candidate = scan(root, policy, mode)?;
    between();
    let again = scan(root, policy, mode)?;
    match candidate == again {
        true => Ok(candidate),
        false => Err(PolicyObserveError::Observe(
            ObserveError::ChangedDuringObservation,
        )),
    }
}

fn scan(
    root: &Path,
    policy: &TrackingPolicy,
    mode: ExcludedDirectories,
) -> Result<PolicyObservation, PolicyObserveError> {
    #[cfg(test)]
    super::scan_count::note();
    let mut entries = Vec::new();
    let mut excluded = Vec::new();
    walk(root, &mut Vec::new(), true, true, policy, mode, &mut entries, &mut excluded)?;
    excluded.sort_by(|left, right| left.path.key().cmp(right.path.key()));
    Ok(PolicyObservation {
        manifest: Manifest::new(entries),
        policy_id: policy.policy_id(),
        excluded,
    })
}

/// 한 디렉터리를 훑는다. `recording` 이 거짓이면 제외된 디렉터리 안을 **검사만** 하는 중이다.
#[allow(clippy::too_many_arguments)]
fn walk(
    dir: &Path,
    trail: &mut Vec<String>,
    at_root: bool,
    recording: bool,
    policy: &TrackingPolicy,
    mode: ExcludedDirectories,
    entries: &mut Vec<ManifestEntry>,
    excluded: &mut Vec<ExcludedEntry>,
) -> Result<(), PolicyObserveError> {
    fn read_failed(trail: &[String], source: std::io::Error) -> PolicyObserveError {
        PolicyObserveError::Observe(ObserveError::ReadFailed {
            at: here(trail),
            source: source.to_string(),
        })
    }
    let listing = fs::read_dir(dir).map_err(|source| read_failed(trail, source))?;

    for item in listing {
        let item = item.map_err(|source| read_failed(trail, source))?;
        let raw = item.file_name();

        // ① 이름이 UTF-8 로 적히지 않으면 정책을 물을 수도 없다 — 관측 전체를 거절한다.
        let Some(name) = raw.to_str().map(str::to_string) else {
            return Err(PolicyObserveError::Observe(ObserveError::UnrepresentablePath {
                near: here(trail),
                shown: raw.to_string_lossy().into_owned(),
            }));
        };
        trail.push(name);

        // ② 경로 계약 — `\` 를 담은 이름은 정책 이전에 걸린다.
        let Some(path) = EntryPath::from_components(trail) else {
            let shown = trail.join("/");
            let parent = &trail[..trail.len() - 1];
            return Err(PolicyObserveError::Observe(ObserveError::UnrepresentablePath {
                near: here(parent),
                shown,
            }));
        };

        // ③ 종류는 정책보다 먼저다. 제외됐다고 링크를 받아들이지 않는다.
        let kind = fs::symlink_metadata(item.path())
            .map_err(|source| {
                PolicyObserveError::Observe(ObserveError::ReadFailed {
                    at: ObserveLocation::Entry(path.clone()),
                    source: source.to_string(),
                })
            })?
            .file_type();

        if kind.is_symlink() {
            return Err(PolicyObserveError::Observe(ObserveError::Symlink { path }));
        }

        let parts: Vec<&str> = trail.iter().map(String::as_str).collect();

        if kind.is_dir() {
            let name = trail.last().expect("방금 얹었다").as_str();
            if name == GIL_DIR {
                if !at_root {
                    return Err(PolicyObserveError::Observe(
                        ObserveError::NestedGilRepository { path },
                    ));
                }
                // 루트 `.gil/` — 내부 저장소. 어느 모드든 들어가지 않는다.
                if recording {
                    excluded.push(ExcludedEntry {
                        path,
                        kind: ExcludedKind::Directory,
                        reason: ExclusionReason::Builtin(Builtin::InternalStore),
                    });
                }
                trail.pop();
                continue;
            }

            let reason = if !recording {
                None
            } else if name == GIT_DIR {
                Some(ExclusionReason::Builtin(Builtin::GitMetadata))
            } else {
                match policy.verdict_for_entry(&parts, true) {
                    Verdict::Ignored(rule) => Some(ExclusionReason::Rule(rule)),
                    Verdict::Reincluded(_) | Verdict::Unmatched => None,
                }
            };

            match reason {
                Some(reason) => {
                    excluded.push(ExcludedEntry {
                        path,
                        kind: ExcludedKind::Directory,
                        reason,
                    });
                    if mode == ExcludedDirectories::Inspect {
                        walk(&item.path(), trail, false, false, policy, mode, entries, excluded)?;
                    }
                }
                None => {
                    walk(&item.path(), trail, false, recording, policy, mode, entries, excluded)?;
                }
            }
            trail.pop();
            continue;
        }

        if !kind.is_file() {
            return Err(PolicyObserveError::Observe(ObserveError::SpecialEntry {
                path,
                kind: super::kind_name(&kind),
            }));
        }

        if !recording {
            trail.pop();
            continue;
        }

        // ④ 일반 파일 — 이름이 `.git` 이면 내장 제외, 아니면 정책에 묻는다.
        let name = trail.last().expect("방금 얹었다").as_str();
        let reason = if name == GIT_DIR {
            Some(ExclusionReason::Builtin(Builtin::GitMetadata))
        } else {
            match policy.verdict_for_entry(&parts, false) {
                Verdict::Ignored(rule) => Some(ExclusionReason::Rule(rule)),
                Verdict::Reincluded(_) | Verdict::Unmatched => None,
            }
        };
        if let Some(reason) = reason {
            excluded.push(ExcludedEntry {
                path,
                kind: ExcludedKind::File,
                reason,
            });
            trail.pop();
            continue;
        }

        let file = File::open(item.path()).map_err(|source| {
            PolicyObserveError::Observe(ObserveError::ReadFailed {
                at: ObserveLocation::Entry(path.clone()),
                source: source.to_string(),
            })
        })?;
        let content = ContentDigest::of(file).map_err(|source| {
            PolicyObserveError::Observe(ObserveError::ReadFailed {
                at: ObserveLocation::Entry(path.clone()),
                source: source.to_string(),
            })
        })?;
        entries.push(ManifestEntry { path, content });
        trail.pop();
    }
    Ok(())
}

// ── 시험 ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn policy(sources: &[(&str, &str)]) -> TrackingPolicy {
        TrackingPolicy::from_sources(
            PolicyVersion::V1,
            sources
                .iter()
                .map(|(dir, text)| IgnoreSource::new(*dir, *text))
                .collect(),
        )
        .unwrap_or_else(|err| panic!("정책을 세우지 못했다: {err}"))
    }

    fn root_policy(text: &str) -> TrackingPolicy {
        policy(&[("", text)])
    }

    fn included(policy: &TrackingPolicy, path: &str) -> bool {
        policy
            .includes_file(path)
            .unwrap_or_else(|err| panic!("{path}: 판정이 거절됐다: {err}"))
    }

    fn decision(policy: &TrackingPolicy, path: &str) -> Decision {
        policy
            .evaluate(&EntryPath::parse(path).expect("시험 경로는 계약을 지킨다"))
            .unwrap_or_else(|err| panic!("{path}: 판정이 거절됐다: {err}"))
    }

    // ── 문법 ──────────────────────────────────────────────────────────────

    #[test]
    fn an_empty_policy_includes_everything_but_git_and_the_internal_store() {
        let p = TrackingPolicy::builtin_only(PolicyVersion::V1);
        assert!(included(&p, "src/main.rs"));
        assert!(included(&p, "node_modules/x/index.js"));
        assert!(!included(&p, ".git/HEAD"));
        assert!(!included(&p, "sub/.git/config"));
        assert!(!included(&p, ".git"), ".git 파일(worktree 포인터)도 제외한다");
        assert!(!included(&p, ".gil/state.yaml"));
        assert!(included(&p, ".gil"), "루트의 일반 파일 .gil 은 평범한 Artifact 다");
        assert!(included(&p, "sub/.gil"), "하위의 일반 파일 .gil 도 평범한 Artifact 다");
    }

    #[test]
    fn a_nested_gil_boundary_is_an_error_not_an_exclusion() {
        // 「제외」라고 답하면 B 가 그 자리를 무시해도 되는 것으로 읽는다.
        let p = root_policy("sub/\n");
        let err = p.includes_file("sub/.gil/state.yaml").expect_err("중첩 .gil 이 판정됐다");
        assert!(err.contains("중첩"), "{err}");
        assert!(
            matches!(
                p.evaluate(&EntryPath::parse("a/b/.gil/x").unwrap()),
                Err(PolicyError::NestedGilBoundary { .. })
            )
        );
    }

    #[test]
    fn builtin_exclusions_win_over_negation() {
        let p = root_policy("!.git\n!.git/**\n!.gil/**\n");
        assert!(!included(&p, ".git/HEAD"));
        assert!(!included(&p, ".gil/state.yaml"));
        assert_eq!(
            decision(&p, ".git/HEAD"),
            Decision::ExcludedBuiltin {
                prefix: EntryPath::parse(".git").unwrap(),
                reason: Builtin::GitMetadata,
            }
        );
    }

    #[test]
    fn blank_lines_comments_and_escapes() {
        let p = root_policy("\n# comment\n\\#literal\n   \nfoo \nbar\\ \n\\!bang\n");
        assert!(!included(&p, "#literal"));
        assert!(included(&p, "# comment"));
        assert!(!included(&p, "foo"), "뒤따르는 공백은 무시한다");
        assert!(included(&p, "foo "));
        assert!(!included(&p, "bar "), "escape 한 공백은 패턴의 일부다");
        assert!(included(&p, "bar"));
        assert!(!included(&p, "!bang"), "`\\!` 는 리터럴 느낌표다");
    }

    #[test]
    fn crlf_lines_are_read_like_lf_lines() {
        let crlf = root_policy("*.log\r\n!keep.log\r\n");
        let lf = root_policy("*.log\n!keep.log\n");
        for p in [&crlf, &lf] {
            assert!(!included(p, "a.log"));
            assert!(included(p, "keep.log"));
        }
        assert_ne!(crlf.policy_id(), lf.policy_id(), "원문이 다르면 다른 정책이다");
    }

    #[test]
    fn unanchored_patterns_match_at_any_depth_and_anchored_only_at_the_source() {
        let p = root_policy("*.log\n/build\ndoc/frotz\n");
        assert!(!included(&p, "a.log"));
        assert!(!included(&p, "deep/er/b.log"));
        assert!(!included(&p, "build"));
        assert!(!included(&p, "build/out.bin"));
        assert!(included(&p, "sub/build/out.bin"), "앞의 `/` 는 루트에 anchoring 한다");
        assert!(!included(&p, "doc/frotz"));
        assert!(!included(&p, "doc/frotz/x"));
        assert!(included(&p, "a/doc/frotz"), "중간의 `/` 도 anchoring 한다");
    }

    #[test]
    fn a_trailing_slash_matches_only_directories() {
        let p = root_policy("logs/\n");
        assert!(!included(&p, "logs/today.txt"), "디렉터리 logs 아래는 제외된다");
        assert!(included(&p, "logs"), "같은 이름의 파일은 디렉터리 규칙에 안 걸린다");
        assert!(!included(&p, "a/logs/x"), "anchoring 없는 디렉터리 규칙은 어느 깊이든");
        assert!(
            matches!(decision(&p, "logs/today.txt"), Decision::ExcludedByParent { .. }),
            "부모 디렉터리 때문임을 말해야 한다"
        );
    }

    #[test]
    fn double_star_forms() {
        let p = root_policy("**/foo\nabc/**\na/**/b\n");
        assert!(!included(&p, "foo"));
        assert!(!included(&p, "x/y/foo"));
        assert!(!included(&p, "abc/x"));
        assert!(!included(&p, "abc/x/y"));
        assert!(included(&p, "abc"), "`abc/**` 는 abc 안의 것이지 abc 자신이 아니다");
        assert!(!included(&p, "a/b"));
        assert!(!included(&p, "a/x/b"));
        assert!(!included(&p, "a/x/y/b"));
        assert!(included(&p, "a/xb"));
    }

    #[test]
    fn star_and_question_do_not_cross_a_separator() {
        let p = root_policy("a*z\nq?\n");
        assert!(!included(&p, "abz"));
        assert!(!included(&p, "sub/abcz"));
        assert!(included(&p, "a/z"), "`*` 는 `/` 를 넘지 않는다");
        assert!(!included(&p, "qa"));
        assert!(p.includes_file("q/").is_err(), "빈 구성 요소는 경로가 아니다");
    }

    #[test]
    fn character_classes() {
        let p = root_policy("[abc].txt\n[!x]y\n[0-9][[:alpha:]]\n[]]\n");
        assert!(!included(&p, "a.txt"));
        assert!(included(&p, "d.txt"));
        assert!(!included(&p, "zy"));
        assert!(included(&p, "xy"));
        assert!(!included(&p, "7q"));
        assert!(included(&p, "7가"), "POSIX 클래스는 ASCII 범위다");
        assert!(!included(&p, "]"), "첫 자리의 `]` 는 리터럴이다");
    }

    #[test]
    fn the_last_matching_rule_wins_and_deeper_sources_beat_shallower_ones() {
        let p = policy(&[
            ("", "*.log\n!keep.log\n"),
            ("sub", "keep.log\n!*.log\nagain.log\n"),
        ]);
        assert!(!included(&p, "a.log"));
        assert!(included(&p, "keep.log"));
        assert!(included(&p, "sub/a.log"), "깊은 원본의 `!*.log` 가 루트를 이긴다");
        assert!(included(&p, "sub/keep.log"), "같은 원본에서 아래 줄이 이긴다");
        assert!(!included(&p, "sub/again.log"));
        assert!(!included(&p, "other/a.log"), "sub 의 규칙은 sub 아래에만 적용된다");
    }

    #[test]
    fn a_source_does_not_judge_its_own_directory() {
        let p = policy(&[("", ""), ("sub", "sub\n")]);
        assert!(included(&p, "sub"), "sub/.gitignore 의 `sub` 는 sub 자신이 아니라 sub/sub 다");
        assert!(!included(&p, "sub/sub"));
    }

    #[test]
    fn a_file_under_an_excluded_parent_cannot_be_reincluded() {
        // Git: "It is not possible to re-include a file if a parent directory of that file is excluded."
        let p = root_policy("build/\n!build/keep.txt\n");
        assert!(!included(&p, "build/keep.txt"));
        let Decision::ExcludedByParent { directory, rule } = decision(&p, "build/keep.txt") else {
            panic!("부모 제외가 아니라고 한다");
        };
        assert_eq!(directory.to_string(), "build");
        assert_eq!(rule.line, 1);
        assert_eq!(rule.pattern, "build/");

        // 반면 부모가 살아 있으면 되살릴 수 있다.
        let p = root_policy("*.txt\n!keep.txt\n");
        assert!(included(&p, "deep/keep.txt"));
        assert!(!included(&p, "deep/other.txt"));
    }

    #[test]
    fn a_nested_source_cannot_reinclude_what_its_parent_excluded() {
        let p = policy(&[("", "vendor/\n"), ("vendor", "!*\n!**\n")]);
        assert!(!included(&p, "vendor/lib.rs"));
    }

    #[test]
    fn korean_paths_and_patterns() {
        let p = root_policy("*.로그\n자료/\n!자료중요.txt\n[가-힣].txt\n");
        assert!(!included(&p, "오늘.로그"));
        assert!(!included(&p, "자료/표.csv"));
        assert!(included(&p, "자료중요.txt"));
        assert!(!included(&p, "나.txt"));
        assert!(included(&p, "a.txt"));
        assert!(included(&p, "자료.txt"), "`?` 하나처럼 클래스도 글자 하나다");
    }

    #[test]
    fn node_modules_and_git_in_a_typical_root_policy() {
        let p = root_policy("node_modules/\ndist/\n.env\n!.env.example\n*.log\n");
        assert!(!included(&p, "node_modules/left-pad/index.js"));
        assert!(!included(&p, "packages/a/node_modules/x/y.js"));
        assert!(!included(&p, "dist/app.js"));
        assert!(!included(&p, ".env"));
        assert!(included(&p, ".env.example"));
        assert!(included(&p, "src/index.js"));
        assert!(!included(&p, ".git/HEAD"));
    }

    // ── 정책 오류 ───────────────────────────────────────────────────────────

    #[test]
    fn policy_errors_are_refusals_not_guesses() {
        let bad = |sources: Vec<IgnoreSource>| {
            TrackingPolicy::from_sources(PolicyVersion::V1, sources).expect_err("통과했다")
        };
        assert!(matches!(
            bad(vec![IgnoreSource::at_root("[abc\n")]),
            PolicyError::InvalidPattern { line: 1, .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::at_root("ok\ntrailing\\\n")]),
            PolicyError::InvalidPattern { line: 2, .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::at_root("a//b\n")]),
            PolicyError::InvalidPattern { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::at_root("/\n")]),
            PolicyError::InvalidPattern { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::at_root("[[:nope:]]\n")]),
            PolicyError::InvalidPattern { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::new("../x", "")]),
            PolicyError::InvalidSourceLocation { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::new("/abs", "")]),
            PolicyError::InvalidSourceLocation { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::new(".git/info", "")]),
            PolicyError::InvalidSourceLocation { .. }
        ));
        assert!(matches!(
            bad(vec![IgnoreSource::new("a", "x"), IgnoreSource::new("a", "y")]),
            PolicyError::DuplicateSourceLocation { .. }
        ));
        let p = root_policy("");
        assert!(p.includes_file("/abs").is_err());
        assert!(p.includes_file("a/../b").is_err());
        assert!(p.includes_file("a\\b").is_err());
        assert!(p.includes_file("").is_err());
    }

    // ── fingerprint ──────────────────────────────────────────────────────

    #[test]
    fn the_same_policy_has_the_same_id_whatever_the_source_order() {
        let a = policy(&[("", "*.log\n"), ("sub", "x\n")]);
        let b = policy(&[("sub", "x\n"), ("", "*.log\n")]);
        assert_eq!(a.policy_id(), b.policy_id());
        assert_eq!(a.fingerprint().len(), 64);
        assert!(a.policy_id().starts_with("gil-tracking-policy/1:sha256:"));
    }

    #[test]
    fn any_change_to_text_order_location_or_version_changes_the_id() {
        let base = policy(&[("", "*.log\n!keep.log\n")]);
        let reordered = policy(&[("", "!keep.log\n*.log\n")]);
        let moved = policy(&[("sub", "*.log\n!keep.log\n")]);
        let comment = policy(&[("", "*.log\n!keep.log\n# note\n")]);
        let extra = policy(&[("", "*.log\n!keep.log\n"), ("sub", "")]);
        let ids = [
            base.policy_id(),
            reordered.policy_id(),
            moved.policy_id(),
            comment.policy_id(),
            extra.policy_id(),
        ];
        for (i, left) in ids.iter().enumerate() {
            for right in &ids[i + 1..] {
                assert_ne!(left, right);
            }
        }
        // 길이를 앞세우므로 경계가 옮겨 가도 같은 해시가 되지 않는다.
        let split_a = policy(&[("a", "b"), ("", "")]);
        let split_b = policy(&[("", "ab")]);
        assert_ne!(split_a.policy_id(), split_b.policy_id());
    }

    // ── 실제 폴더 관측 ─────────────────────────────────────────────────────

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gil-tracking-{label}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("시험이 쓸 자리를 만들 수 있어야 한다");
        dir
    }

    fn write(root: &Path, path: &str, bytes: &[u8]) {
        let at = root.join(path);
        if let Some(parent) = at.parent() {
            fs::create_dir_all(parent).expect("부모 디렉터리를 만들 수 있어야 한다");
        }
        fs::write(&at, bytes).expect("파일을 쓸 수 있어야 한다");
    }

    /// 전형적인 사용자 프로젝트 — Git metadata, 의존성, build 출력, 중첩 규칙, 한글.
    fn typical_project(label: &str) -> PathBuf {
        let root = scratch(label);
        write(&root, ".gitignore", b"node_modules/\nbuild/\n*.log\n!keep.log\n.env\n");
        write(&root, ".git/HEAD", b"ref: refs/heads/main\n");
        write(&root, ".git/objects/ab/cdef", b"blob");
        write(&root, ".gil/state.yaml", b"format: 7\n");
        write(&root, "src/main.rs", b"fn main() {}");
        write(&root, "src/.gitignore", b"*.tmp\n");
        write(&root, "src/scratch.tmp", b"tmp");
        write(&root, "src/deep/also.tmp", b"tmp");
        write(&root, "node_modules/left-pad/index.js", b"module.exports = 1");
        write(&root, "build/out.bin", b"\x00\x01");
        write(&root, "debug.log", b"...");
        write(&root, "keep.log", b"keep");
        write(&root, ".env", b"SECRET=1");
        write(&root, "자료/표.csv", b"a,b");
        write(&root, "자료/메모.log", b"log");
        root
    }

    fn observed_paths(observation: &PolicyObservation) -> Vec<String> {
        observation.manifest().paths().map(|p| p.to_string()).collect()
    }

    fn excluded_paths(observation: &PolicyObservation) -> Vec<String> {
        observation.excluded().iter().map(|e| e.path.to_string()).collect()
    }

    fn observe_typical(root: &Path, mode: ExcludedDirectories) -> PolicyObservation {
        let sources = discover_gitignore_sources(root)
            .unwrap_or_else(|err| panic!("원문 수집이 거절됐다: {err}"));
        let policy = TrackingPolicy::from_sources(PolicyVersion::V1, sources)
            .unwrap_or_else(|err| panic!("정책을 세우지 못했다: {err}"));
        super::super::observe_with_policy(root, &policy, mode)
            .unwrap_or_else(|err| panic!("정책 관측이 거절됐다: {err}"))
    }

    #[test]
    fn a_policy_observation_of_a_real_folder_keeps_only_tracked_files() {
        let root = typical_project("typical");
        let seen = observe_typical(&root, ExcludedDirectories::Prune);

        assert_eq!(
            observed_paths(&seen),
            [".gitignore", "keep.log", "src/.gitignore", "src/main.rs", "자료/표.csv"],
            "정책 적용 세계가 틀렸다"
        );
        assert_eq!(
            excluded_paths(&seen),
            [
                ".env",
                ".gil",
                ".git",
                "build",
                "debug.log",
                "node_modules",
                "src/deep/also.tmp",
                "src/scratch.tmp",
                "자료/메모.log",
            ],
            "무엇을 뺐는지 전부 말해야 한다"
        );
        assert!(seen.policy_id().starts_with("gil-tracking-policy/1:sha256:"));

        // 어느 규칙이 뺐는지도.
        let env = seen.excluded().iter().find(|e| e.path.to_string() == ".env").unwrap();
        let ExclusionReason::Rule(rule) = &env.reason else {
            panic!(".env 가 규칙이 아닌 이유로 빠졌다");
        };
        assert_eq!((rule.directory.as_str(), rule.line, rule.pattern.as_str()), ("", 5, ".env"));
        let tmp = seen.excluded().iter().find(|e| e.path.to_string() == "src/scratch.tmp").unwrap();
        let ExclusionReason::Rule(rule) = &tmp.reason else { panic!() };
        assert_eq!(rule.directory, "src", "중첩 .gitignore 의 규칙이다");
        let git = seen.excluded().iter().find(|e| e.path.to_string() == ".git").unwrap();
        assert_eq!(git.reason, ExclusionReason::Builtin(Builtin::GitMetadata));
        assert_eq!(git.kind, ExcludedKind::Directory);
    }

    #[test]
    fn the_same_tree_and_policy_observe_identically_and_the_plain_observer_is_untouched() {
        let one = typical_project("same-a");
        let two = typical_project("same-b");
        let seen_one = observe_typical(&one, ExcludedDirectories::Prune);
        let seen_two = observe_typical(&two, ExcludedDirectories::Prune);
        assert_eq!(seen_one, seen_two, "같은 트리·같은 정책이 다른 결과를 냈다");
        assert_eq!(seen_one.policy_id(), seen_two.policy_id());

        // 기본 관측은 **여전히** .git 과 node_modules 를 세계로 본다 — 의미를 바꾸지 않았다.
        let plain = super::super::observe(&one).expect("기본 관측은 그대로 동작한다");
        let plain_paths: Vec<String> = plain.paths().map(|p| p.to_string()).collect();
        assert!(plain_paths.contains(&".git/HEAD".to_string()));
        assert!(plain_paths.contains(&"node_modules/left-pad/index.js".to_string()));
        assert!(plain_paths.contains(&".env".to_string()));
        assert!(!plain_paths.contains(&".gil/state.yaml".to_string()));
        assert!(plain.len() > seen_one.manifest().len());

        // 정책이 다르면 같은 트리도 다른 결과·다른 ID 다.
        let other = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root("node_modules/\n")],
        )
        .unwrap();
        let seen_other =
            super::super::observe_with_policy(&one, &other, ExcludedDirectories::Prune).unwrap();
        assert_ne!(seen_other.policy_id(), seen_one.policy_id());
        assert_ne!(seen_other.manifest(), seen_one.manifest());
        assert!(observed_paths(&seen_other).contains(&".env".to_string()));
    }

    #[test]
    fn policy_observation_also_looks_twice() {
        let root = typical_project("twice");
        let policy = TrackingPolicy::builtin_only(PolicyVersion::V1);
        let err = observe_twice(&root, &policy, ExcludedDirectories::Prune, || {
            write(&root, "src/new.rs", b"late")
        })
        .expect_err("관측 중 파일이 생겼는데 통과했다");
        assert_eq!(
            err,
            PolicyObserveError::Observe(ObserveError::ChangedDuringObservation)
        );

        // 제외된 자리의 변화는 세계를 바꾸지 않는다 — 두 번 봐도 같다.
        let policy = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root("*.log\n")],
        )
        .unwrap();
        observe_twice(&root, &policy, ExcludedDirectories::Prune, || {
            write(&root, "debug.log", b"changed")
        })
        .expect("제외된 파일의 변화는 세계의 변화가 아니다");
    }

    #[test]
    fn a_source_discovered_inside_an_excluded_directory_is_not_read() {
        let root = scratch("discover-pruned");
        write(&root, ".gitignore", b"vendor/\n");
        write(&root, "vendor/.gitignore", b"!*\n");
        write(&root, "vendor/lib.rs", b"x");
        write(&root, "src/.gitignore", b"*.tmp\n");
        write(&root, "src/.git/config", b"never");
        write(&root, "src/.git/.gitignore", b"never");

        let sources = discover_gitignore_sources(&root).unwrap();
        let dirs: Vec<&str> = sources.iter().map(|s| s.directory.as_str()).collect();
        assert_eq!(dirs, ["", "src"], "제외된 vendor 와 .git 안의 원문을 읽었다");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_gitignore_is_refused_as_policy_text() {
        let root = scratch("discover-link");
        write(&root, "real", b"*.log\n");
        std::os::unix::fs::symlink("real", root.join(".gitignore")).unwrap();
        let err = discover_gitignore_sources(&root).expect_err("링크를 원문으로 읽었다");
        assert!(matches!(
            err,
            PolicyObserveError::Observe(ObserveError::Symlink { .. })
        ));
    }

    // ── 거절 계약은 약해지지 않는다 ─────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn an_excluded_symlink_is_still_refused_when_enumerated() {
        let root = scratch("excluded-link");
        write(&root, ".gitignore", b".venv\n");
        write(&root, "a.txt", b"x");
        std::os::unix::fs::symlink("a.txt", root.join(".venv")).unwrap();
        let policy = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root(".venv\n")],
        )
        .unwrap();
        for mode in [ExcludedDirectories::Prune, ExcludedDirectories::Inspect] {
            let err = super::super::observe_with_policy(&root, &policy, mode)
                .expect_err("제외됐다고 링크를 받아들였다");
            assert!(matches!(
                err,
                PolicyObserveError::Observe(ObserveError::Symlink { .. })
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn pruning_skips_what_is_inside_but_inspecting_refuses_it() {
        let root = scratch("prune-vs-inspect");
        write(&root, "src/main.rs", b"x");
        write(&root, "node_modules/pkg/index.js", b"x");
        std::os::unix::fs::symlink("../pkg/index.js", root.join("node_modules/.bin-link")).unwrap();
        let policy = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root("node_modules/\n")],
        )
        .unwrap();

        let pruned = super::super::observe_with_policy(&root, &policy, ExcludedDirectories::Prune)
            .expect("들어가지 않으면 안의 링크를 보지 않는다");
        assert_eq!(observed_paths(&pruned), ["src/main.rs"]);
        assert_eq!(excluded_paths(&pruned), ["node_modules"]);

        let err = super::super::observe_with_policy(&root, &policy, ExcludedDirectories::Inspect)
            .expect_err("들어갔는데 링크를 받아들였다");
        let PolicyObserveError::Observe(ObserveError::Symlink { path }) = err else {
            panic!("다른 이유로 거절됐다: {err}");
        };
        assert_eq!(path.to_string(), "node_modules/.bin-link");
    }

    #[test]
    fn a_nested_gil_is_refused_even_if_a_rule_would_exclude_it() {
        let root = scratch("nested-gil");
        write(&root, "src/main.rs", b"x");
        write(&root, "sub/.gil/state.yaml", b"format: 7\n");
        write(&root, "sub/lib.rs", b"y");
        let policy = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root("**/.gil/\n")],
        )
        .unwrap();
        for mode in [ExcludedDirectories::Prune, ExcludedDirectories::Inspect] {
            let err = super::super::observe_with_policy(&root, &policy, mode)
                .expect_err("중첩 .gil 이 제외 규칙으로 통과했다");
            let PolicyObserveError::Observe(ObserveError::NestedGilRepository { path }) = err
            else {
                panic!("다른 이유로 거절됐다: {err}");
            };
            assert_eq!(path.to_string(), "sub/.gil");
        }
    }

    #[test]
    fn a_nested_gil_inside_a_pruned_directory_is_not_seen_and_the_contract_says_so() {
        // Prune 의 알려진 한계 — 결과의 excluded 가 「vendor 안은 보지 않았다」를 남긴다.
        let root = scratch("nested-gil-pruned");
        write(&root, "src/main.rs", b"x");
        write(&root, "vendor/.gil/state.yaml", b"format: 7\n");
        let policy = TrackingPolicy::from_sources(
            PolicyVersion::V1,
            vec![IgnoreSource::at_root("vendor/\n")],
        )
        .unwrap();
        let pruned = super::super::observe_with_policy(&root, &policy, ExcludedDirectories::Prune)
            .expect("Prune 은 들어가지 않는다");
        assert_eq!(excluded_paths(&pruned), ["vendor"]);
        assert_eq!(pruned.excluded()[0].kind, ExcludedKind::Directory);

        let err = super::super::observe_with_policy(&root, &policy, ExcludedDirectories::Inspect)
            .expect_err("Inspect 는 중첩 .gil 을 거절한다");
        assert!(matches!(
            err,
            PolicyObserveError::Observe(ObserveError::NestedGilRepository { .. })
        ));
    }

    #[test]
    fn the_root_gil_directory_is_never_entered_in_either_mode() {
        let root = scratch("root-gil");
        write(&root, "a.txt", b"x");
        write(&root, ".gil/state.yaml", b"format: 7\n");
        #[cfg(unix)]
        std::os::unix::fs::symlink("../a.txt", root.join(".gil/link")).unwrap();
        let policy = TrackingPolicy::builtin_only(PolicyVersion::V1);
        for mode in [ExcludedDirectories::Prune, ExcludedDirectories::Inspect] {
            let seen = super::super::observe_with_policy(&root, &policy, mode)
                .expect("루트 .gil 안은 어느 모드든 보지 않는다");
            assert_eq!(observed_paths(&seen), ["a.txt"]);
            assert_eq!(excluded_paths(&seen), [".gil"]);
        }
    }

    #[test]
    fn a_regular_file_named_gil_is_an_ordinary_artifact_under_policy_too() {
        let root = scratch("gil-file");
        write(&root, "sub/.gil", b"file");
        write(&root, ".gil", b"file");
        let policy = TrackingPolicy::builtin_only(PolicyVersion::V1);
        let seen = super::super::observe_with_policy(&root, &policy, ExcludedDirectories::Prune)
            .unwrap();
        assert_eq!(observed_paths(&seen), [".gil", "sub/.gil"]);
    }

    #[test]
    fn every_included_entry_agrees_with_includes_file() {
        // 관측이 담은 경로는 경로 API 도 포함이라 하고, 뺀 경로는 제외라 한다 — 두 자리가
        // 같은 규칙을 쓴다.
        let root = typical_project("agree");
        let seen = observe_typical(&root, ExcludedDirectories::Prune);
        let sources = discover_gitignore_sources(&root).unwrap();
        let policy = TrackingPolicy::from_sources(PolicyVersion::V1, sources).unwrap();
        for path in seen.manifest().paths() {
            assert!(included(&policy, path.as_str()), "{path} 는 담겼는데 경로 API 는 제외라 한다");
        }
        for entry in seen.excluded() {
            if entry.kind == ExcludedKind::File {
                assert!(!included(&policy, entry.path.as_str()), "{} 는 뺐는데 경로 API 는 포함이라 한다", entry.path);
            }
        }
        assert!(!included(&policy, "node_modules/left-pad/index.js"));
        assert!(!included(&policy, "build/out.bin"));
        assert!(!included(&policy, ".git/HEAD"));
    }
}
