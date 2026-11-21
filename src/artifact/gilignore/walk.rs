//! 승인 정책의 **읽기 전용 트리 평가** — 필요한 디렉터리만 열고, 파일 내용은 읽지 않는다.
//!
//! ```text
//! 디렉터리 항목 하나
//!   ① 이름이 UTF-8·Artifact 경로인가          아니면 거절 (제외 영역이어도 — 정책을 물을 수 없다)
//!   ② 종류를 따라가지 않고 본다               Unix fstatat(AT_SYMLINK_NOFOLLOW), Windows reparse 속성
//!   ③ 디렉터리   추적 → 들어간다
//!               제외 → 재포함 가능성이 없으면 들어가지 않는다(내부 미확인), 있으면 들어간다
//!               루트 .gil·.git·루트 .gilignore → 시스템 보호, 들어가지 않는다
//!               더 깊은 .gil → 중첩 경계, 거절
//!     파일       추적 / 제외 / 시스템 보호
//!     연결 파일  최종 정책상 제외 → 대상을 따라가지 않고 건너뛴다
//!               추적·재포함 / 재포함 경로 위 → 거절
//!     특수 항목  거절 (합의는 특수 파일 거절의 완화가 아니다)
//!     하위 .gilignore (어떤 종류든) → 차단 사유. 읽지 않는다
//! ```
//!
//! # 항목이 바뀌는 경우
//!
//! 디렉터리는 **부모의 핸들 기준으로** 연다 — Unix 는 `openat(O_DIRECTORY | O_NOFOLLOW)`,
//! Windows 는 조상을 고정한 [`Chain`](crate::windows_fs::Chain)(모든 reparse point 거절). 그래서
//! 확인 뒤 디렉터리가 연결 파일로 바뀌어도 그 대상의 목록을 읽지 않고 거절한다. 파일 내용은 읽지
//! 않으므로 프로젝트 밖의 내용을 읽을 일이 없다. 전체를 두 번 훑어 다르면 거절한다(§6 안정 관측).
//! 외부 프로세스의 쓰기를 막는다고 주장하지 않는다.

use std::fmt;
use std::io;

use super::{ApprovedPolicy, Decision, Interior, PathKind, PolicyIdentity, Protection, RuleRef};
use crate::artifact::{EntryPath, GIL_DIR, ObserveLocation};

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 평가한 폴더 — **어떤 정책으로, 무엇을 보았고 무엇을 보지 않았는가.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeEvaluation {
    policy: PolicyIdentity,
    tracked: Vec<TrackedFile>,
    excluded: Vec<ExcludedEntry>,
    blockers: Vec<Blocker>,
    notices: Vec<Notice>,
}

impl TreeEvaluation {
    pub(crate) fn policy(&self) -> &PolicyIdentity {
        &self.policy
    }

    /// 추적 대상 일반 파일 — canonical 경로 순서. 내용은 읽지 않았다.
    pub(crate) fn tracked(&self) -> &[TrackedFile] {
        &self.tracked
    }

    /// 만난 항목 가운데 추적하지 않는 것과 그 이유 — canonical 경로 순서.
    pub(crate) fn excluded(&self) -> &[ExcludedEntry] {
        &self.excluded
    }

    /// 시작 준비를 보류해야 하는 발견 — 비어 있지 않으면 이 정책으로 시작할 수 없다.
    pub(crate) fn blockers(&self) -> &[Blocker] {
        &self.blockers
    }

    pub(crate) fn is_blocked(&self) -> bool {
        !self.blockers.is_empty()
    }

    /// 시작을 막지는 않지만 승인 전에 보여 줘야 하는 진단 — canonical 경로 순서.
    pub(crate) fn notices(&self) -> &[Notice] {
        &self.notices
    }

    /// 들어가지 않은 디렉터리 — **안의 항목은 확인하지 않았다.**
    pub(crate) fn unexplored(&self) -> impl Iterator<Item = &ExcludedEntry> {
        self.excluded
            .iter()
            .filter(|entry| entry.interior == Some(Interior::Skip))
    }

    /// 제외됐지만 재포함 때문에 들어간 디렉터리와 그렇게 만든 포함 예외.
    pub(crate) fn explored_for_reinclusion(&self) -> impl Iterator<Item = (&EntryPath, &[RuleRef])> {
        self.excluded.iter().filter_map(|entry| match &entry.interior {
            Some(Interior::Explore { because }) => Some((&entry.path, because.as_slice())),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackedFile {
    pub(crate) path: EntryPath,
    /// 추적하게 만든 포함 예외. 아무 규칙도 맞지 않았으면 `None`.
    pub(crate) by: Option<RuleRef>,
}

/// 항목의 종류 — 따라가지 않고 본 그대로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    File,
    Directory,
    /// 심볼릭 링크, Windows 의 모든 reparse point(junction 포함). **대상을 따라가지 않았다.**
    Link,
}

impl EntryKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            EntryKind::File => "file",
            EntryKind::Directory => "directory",
            EntryKind::Link => "link",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExclusionReason {
    Rule(RuleRef),
    System(Protection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcludedEntry {
    pub(crate) path: EntryPath,
    pub(crate) kind: EntryKind,
    pub(crate) reason: ExclusionReason,
    /// 디렉터리만 값이 있다. 연결 파일은 언제나 대상을 따라가지 않았다.
    pub(crate) interior: Option<Interior>,
}

/// 시작 준비를 보류해야 하는 발견.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Blocker {
    /// 루트가 아닌 `.gilignore`. 적용하지도 일반 파일로 다루지도 않았고 **읽지 않았다.**
    NestedPolicyFile { path: EntryPath, kind: EntryKind },
}

impl Blocker {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Blocker::NestedPolicyFile { .. } => "nested_policy_file",
        }
    }

    pub(crate) fn path(&self) -> &EntryPath {
        match self {
            Blocker::NestedPolicyFile { path, .. } => path,
        }
    }
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocker::NestedPolicyFile { path, kind } => write!(
                f,
                "{path} ({}) — 정책 파일은 프로젝트 루트의 .gilignore 하나뿐이라 이 파일은 적용되지 않는다. \
                 만든 이유와 규칙의 의도를 확인하고, 필요한 규칙은 이 폴더 기준의 의미를 유지해 루트 \
                 규칙으로 옮기거나 파일을 정리해야 시작할 수 있다. GIL 은 이 파일을 읽거나 자동으로 \
                 삭제·이동·병합하지 않는다.",
                kind.code()
            ),
        }
    }
}

/// 시작을 막지 않는 진단. 판정은 이미 보호 쪽으로 내려졌고, 그 사실을 감추지 않으려는 것이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Notice {
    /// 다른 경로에도 맞을 수 있어 받아들인 포함 예외가 시스템 보호 자리에 닿았다 — 보호가 이겼다.
    ReinclusionOverriddenByProtection {
        path: EntryPath,
        protection: Protection,
        rules: Vec<RuleRef>,
    },
    /// 시스템 이름의 대소문자 변형(`.GIL`, `.Git` 등)을 시스템 보호로 다뤘다.
    ProtectedNameVariant { path: EntryPath, protection: Protection },
}

impl Notice {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Notice::ReinclusionOverriddenByProtection { .. } => {
                "reinclusion_overridden_by_protection"
            }
            Notice::ProtectedNameVariant { .. } => "protected_name_variant",
        }
    }

    pub(crate) fn path(&self) -> &EntryPath {
        match self {
            Notice::ReinclusionOverriddenByProtection { path, .. }
            | Notice::ProtectedNameVariant { path, .. } => path,
        }
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Notice::ReinclusionOverriddenByProtection {
                path,
                protection,
                rules,
            } => write!(
                f,
                "{path} 은(는) {protection} 이라 {} 이(가) 맞아도 추적하지 않는다. 포함 예외로 시스템 보호를 \
                 되살릴 수 없다 — 그 줄이 다른 경로를 위한 것인지 확인한다.",
                rules.iter().map(|rule| rule.to_string()).collect::<Vec<_>>().join(", ")
            ),
            Notice::ProtectedNameVariant { path, protection } => write!(
                f,
                "{path} 은(는) {protection} 의 대소문자 변형이라 시스템 보호로 다뤘다. 대소문자를 무시하는 \
                 파일 시스템에서는 같은 항목이고, 구분하는 파일 시스템에서도 추적하지 않는다."
            ),
        }
    }
}

// ── 거절 ───────────────────────────────────────────────────────────────────

/// 평가를 끝내지 못한 이유. 어느 것이든 **결과를 돌려주지 않았고 아무것도 쓰지 않았다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvaluationError {
    /// 프로젝트 루트를 디렉터리로 열 수 없다(없음·연결 파일·권한 등).
    RootUnavailable { source: String },
    ReadFailed { at: ObserveLocation, source: String },
    /// 이름이 UTF-8 이 아니거나 `\` 를 담아 Artifact 경로로 적을 수 없다.
    UnrepresentablePath { near: ObserveLocation, shown: String },
    /// 추적 대상(포함 예외로 다시 포함했거나 추적 영역)인 연결 파일.
    TrackedLink { path: EntryPath, by: Option<RuleRef> },
    /// 제외된 연결 파일이지만 그 아래를 포함 예외가 가리킬 수 있다 — 따라가서 포함을 실현하지 않는다.
    LinkOnReinclusionPath {
        path: EntryPath,
        excluded_by: RuleRef,
        because: Vec<RuleRef>,
    },
    /// 루트 `.gil` 이 연결 파일이다 — 내부 기록 경계를 확인할 수 없다.
    InternalStoreLink { path: EntryPath },
    SpecialEntry { path: EntryPath, kind: &'static str },
    NestedGilBoundary { path: EntryPath },
    /// 훑는 동안 항목이 바뀌었다.
    ChangedDuringEvaluation,
}

impl EvaluationError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            EvaluationError::RootUnavailable { .. } => "root_unavailable",
            EvaluationError::ReadFailed { .. } => "read_failed",
            EvaluationError::UnrepresentablePath { .. } => "unrepresentable_path",
            EvaluationError::TrackedLink { .. } => "tracked_link",
            EvaluationError::LinkOnReinclusionPath { .. } => "link_on_reinclusion_path",
            EvaluationError::InternalStoreLink { .. } => "internal_store_link",
            EvaluationError::SpecialEntry { .. } => "special_entry",
            EvaluationError::NestedGilBoundary { .. } => "nested_gil_boundary",
            EvaluationError::ChangedDuringEvaluation => "changed_during_evaluation",
        }
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = |rules: &[RuleRef]| {
            rules.iter().map(|rule| rule.to_string()).collect::<Vec<_>>().join(", ")
        };
        match self {
            EvaluationError::RootUnavailable { source } => {
                write!(f, "프로젝트 루트를 디렉터리로 열 수 없다 — {source}.")
            }
            EvaluationError::ReadFailed { at, source } => {
                write!(f, "{at} 을(를) 읽지 못했다 — {source}. 평가를 멈췄다.")
            }
            EvaluationError::UnrepresentablePath { near, shown } => write!(
                f,
                "{near} 아래의 {shown:?} 는 Artifact 경로로 적을 수 없다(UTF-8 이 아니거나 `\\` 를 담았다). \
                 정책을 물을 수 없어 평가를 멈췄다."
            ),
            EvaluationError::TrackedLink { path, by } => match by {
                Some(rule) => write!(
                    f,
                    "{path} 은(는) 연결 파일인데 {rule} 이(가) 추적 대상으로 다시 포함한다. GIL 은 연결 \
                     대상을 따라가 포함을 실현하지 않는다. 그 규칙을 고치거나 연결 파일을 정리해야 한다."
                ),
                None => write!(
                    f,
                    "{path} 은(는) 추적 영역의 연결 파일이다. GIL 은 연결을 따라가지도 저장하지도 않는다. \
                     제외 규칙에 넣거나 연결 파일을 정리해야 한다."
                ),
            },
            EvaluationError::LinkOnReinclusionPath {
                path,
                excluded_by,
                because,
            } => write!(
                f,
                "{path} 은(는) {excluded_by} 로 제외된 연결 파일이지만 그 아래를 포함 예외({})가 가리킬 수 \
                 있다. 연결을 따라가 포함을 실현하지 않으므로 요청한 추적을 수행할 수 없다.",
                lines(because)
            ),
            EvaluationError::InternalStoreLink { path } => write!(
                f,
                "{path} 은(는) 연결 파일이다. GIL 내부 기록의 경계를 확인할 수 없어 평가를 멈췄다."
            ),
            EvaluationError::SpecialEntry { path, kind } => write!(
                f,
                "{path} 은(는) {kind} 라 Artifact 로 다룰 수 없다. 평가를 멈췄다."
            ),
            EvaluationError::NestedGilBoundary { path } => write!(
                f,
                "현재 프로젝트 안에서 다른 GIL 저장소를 발견했다: {path}. 중첩 프로젝트의 경계는 정의되지 \
                 않았으므로 평가를 멈췄다."
            ),
            EvaluationError::ChangedDuringEvaluation => f.write_str(
                "평가 중 폴더 항목이 바뀌었다. 서로 다른 순간을 한 결과로 섞지 않으려고 멈췄다.",
            ),
        }
    }
}

impl std::error::Error for EvaluationError {}

// ── 훑기 ───────────────────────────────────────────────────────────────────

/// 두 번 훑고 견준다. `between` 은 시험만 쓰는 틈이다.
pub(super) fn evaluate_twice(
    root: &std::path::Path,
    policy: &ApprovedPolicy,
    between: impl FnOnce(),
) -> Result<TreeEvaluation, EvaluationError> {
    let first = scan(root, policy)?;
    between();
    let second = scan(root, policy)?;
    match first == second {
        true => Ok(first),
        false => Err(EvaluationError::ChangedDuringEvaluation),
    }
}

#[derive(Default)]
struct Collect {
    tracked: Vec<TrackedFile>,
    excluded: Vec<ExcludedEntry>,
    blockers: Vec<Blocker>,
    notices: Vec<Notice>,
}

fn scan(root: &std::path::Path, policy: &ApprovedPolicy) -> Result<TreeEvaluation, EvaluationError> {
    let dir = os::root(root).map_err(|source| EvaluationError::RootUnavailable {
        source: source.to_string(),
    })?;
    let mut out = Collect::default();
    visit(&dir, &mut Vec::new(), policy, &mut out)?;
    out.tracked.sort_by(|a, b| a.path.key().cmp(b.path.key()));
    out.excluded.sort_by(|a, b| a.path.key().cmp(b.path.key()));
    out.blockers.sort_by(|a, b| a.path().key().cmp(b.path().key()));
    out.notices.sort_by(|a, b| a.path().key().cmp(b.path().key()).then(a.code().cmp(b.code())));
    Ok(TreeEvaluation {
        policy: policy.identity().clone(),
        tracked: out.tracked,
        excluded: out.excluded,
        blockers: out.blockers,
        notices: out.notices,
    })
}

fn here(trail: &[String]) -> ObserveLocation {
    match EntryPath::from_components(trail) {
        Some(path) => ObserveLocation::Entry(path),
        None => ObserveLocation::Root,
    }
}

fn read_failed(trail: &[String], source: io::Error) -> EvaluationError {
    EvaluationError::ReadFailed {
        at: here(trail),
        source: source.to_string(),
    }
}

fn visit(
    dir: &os::Dir,
    trail: &mut Vec<String>,
    policy: &ApprovedPolicy,
    out: &mut Collect,
) -> Result<(), EvaluationError> {
    #[cfg(test)]
    listed::note(trail);

    let mut names = Vec::new();
    for raw in os::names(dir).map_err(|source| read_failed(trail, source))? {
        match raw.into_string() {
            Ok(name) => names.push(name),
            Err(raw) => {
                return Err(EvaluationError::UnrepresentablePath {
                    near: here(trail),
                    shown: raw.to_string_lossy().into_owned(),
                });
            }
        }
    }
    names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    for name in names {
        trail.push(name);
        let Some(path) = EntryPath::from_components(trail) else {
            let shown = trail.join("/");
            trail.pop();
            return Err(EvaluationError::UnrepresentablePath {
                near: here(trail),
                shown,
            });
        };
        let name = trail.last().expect("방금 얹었다").clone();
        let kind = os::kind(dir, &name).map_err(|source| EvaluationError::ReadFailed {
            at: ObserveLocation::Entry(path.clone()),
            source: source.to_string(),
        })?;
        let parts: Vec<String> = trail.clone();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        match kind {
            os::Kind::Missing => return Err(EvaluationError::ChangedDuringEvaluation),
            os::Kind::Special(kind) => return Err(EvaluationError::SpecialEntry { path, kind }),
            os::Kind::File => file(policy, &parts, path, out)?,
            os::Kind::Link => link(policy, &parts, path, out)?,
            os::Kind::Directory => {
                if directory(policy, &parts, path, out)? {
                    let child = match os::child(dir, &name) {
                        Ok(child) => child,
                        // 확인한 뒤 바뀌었으면 그 대상을 열지 않았다 — 다시 보고 말한다.
                        Err(source) => {
                            return Err(match os::kind(dir, &name) {
                                Ok(os::Kind::Directory) => EvaluationError::ReadFailed {
                                    at: here(trail),
                                    source: source.to_string(),
                                },
                                _ => EvaluationError::ChangedDuringEvaluation,
                            });
                        }
                    };
                    visit(&child, trail, policy, out)?;
                }
            }
        }
        trail.pop();
    }
    Ok(())
}

/// 디렉터리 하나의 판정. 참이면 들어간다.
fn directory(
    policy: &ApprovedPolicy,
    parts: &[&str],
    path: EntryPath,
    out: &mut Collect,
) -> Result<bool, EvaluationError> {
    match policy.decide_parts(parts, PathKind::Directory) {
        Decision::NestedGilBoundary { at } => Err(EvaluationError::NestedGilBoundary { path: at }),
        Decision::Protected { protection, at } => {
            protected(policy, parts, PathKind::Directory, &path, protection, &at, out);
            out.excluded.push(ExcludedEntry {
                path,
                kind: EntryKind::Directory,
                reason: ExclusionReason::System(protection),
                interior: Some(Interior::Skip),
            });
            Ok(false)
        }
        Decision::NestedPolicyFile { .. } => {
            out.blockers.push(Blocker::NestedPolicyFile {
                path,
                kind: EntryKind::Directory,
            });
            Ok(false)
        }
        Decision::Excluded { rule } => {
            let interior = policy.interior(parts, &rule);
            let descend = matches!(interior, Interior::Explore { .. });
            out.excluded.push(ExcludedEntry {
                path,
                kind: EntryKind::Directory,
                reason: ExclusionReason::Rule(rule),
                interior: Some(interior),
            });
            Ok(descend)
        }
        Decision::Tracked { .. } => Ok(true),
    }
}

fn file(
    policy: &ApprovedPolicy,
    parts: &[&str],
    path: EntryPath,
    out: &mut Collect,
) -> Result<(), EvaluationError> {
    match policy.decide_parts(parts, PathKind::File) {
        Decision::Tracked { by } => out.tracked.push(TrackedFile { path, by }),
        Decision::Excluded { rule } => out.excluded.push(ExcludedEntry {
            path,
            kind: EntryKind::File,
            reason: ExclusionReason::Rule(rule),
            interior: None,
        }),
        Decision::Protected { protection, at } => {
            protected(policy, parts, PathKind::File, &path, protection, &at, out);
            out.excluded.push(ExcludedEntry {
                path,
                kind: EntryKind::File,
                reason: ExclusionReason::System(protection),
                interior: None,
            });
        }
        Decision::NestedPolicyFile { .. } => out.blockers.push(Blocker::NestedPolicyFile {
            path,
            kind: EntryKind::File,
        }),
        // 조상 디렉터리에서 이미 거절했어야 한다 — 그래도 「제외」로 답하지 않는다.
        Decision::NestedGilBoundary { at } => {
            return Err(EvaluationError::NestedGilBoundary { path: at });
        }
    }
    Ok(())
}

/// 시스템 보호로 내린 판정의 진단 — 변형 이름과, 그 자리에 닿은 넓은 포함 예외.
fn protected(
    policy: &ApprovedPolicy,
    parts: &[&str],
    kind: PathKind,
    path: &EntryPath,
    protection: Protection,
    at: &EntryPath,
    out: &mut Collect,
) {
    let canonical = match protection {
        Protection::GitMetadata => ".git",
        Protection::InternalStore => GIL_DIR,
        Protection::PolicyFile => super::GILIGNORE,
    };
    if at.as_str().rsplit('/').next() != Some(canonical) {
        out.notices.push(Notice::ProtectedNameVariant {
            path: path.clone(),
            protection,
        });
    }
    let rules = policy.reinclusions_into(parts, kind);
    if !rules.is_empty() {
        out.notices.push(Notice::ReinclusionOverriddenByProtection {
            path: path.clone(),
            protection,
            rules,
        });
    }
}

/// 연결 파일 하나. **대상을 보지 않으므로 종류를 모른다** — 디렉터리로 읽은 판정과 파일로 읽은
/// 판정 가운데 원문의 더 아래 줄이 정한 쪽을 따른다(마지막 규칙이 이긴다). 그래서 `.venv/` 같은
/// 디렉터리 전용 제외 규칙도 연결 파일 `.venv` 에 맞는다.
fn link(
    policy: &ApprovedPolicy,
    parts: &[&str],
    path: EntryPath,
    out: &mut Collect,
) -> Result<(), EvaluationError> {
    if parts.len() == 1 && super::same_system_name(parts[0], GIL_DIR) {
        return Err(EvaluationError::InternalStoreLink { path });
    }
    let as_dir = policy.decide_parts(parts, PathKind::Directory);
    let as_file = policy.decide_parts(parts, PathKind::File);
    let readings = [&as_dir, &as_file];

    if readings
        .iter()
        .any(|reading| matches!(reading, Decision::NestedPolicyFile { .. }))
    {
        out.blockers.push(Blocker::NestedPolicyFile {
            path,
            kind: EntryKind::Link,
        });
        return Ok(());
    }
    for reading in readings {
        if let Decision::Protected { protection, at } = reading {
            protected(policy, parts, PathKind::Directory, &path, *protection, at, out);
            out.excluded.push(ExcludedEntry {
                path,
                kind: EntryKind::Link,
                reason: ExclusionReason::System(*protection),
                interior: None,
            });
            return Ok(());
        }
    }

    // 중첩 경계는 디렉터리의 규칙이다 — `.gil` 이라는 이름의 연결 파일은 연결 파일 규칙을 따른다.
    let line = |decision: &Decision| match decision {
        Decision::Tracked { by: Some(rule) } | Decision::Excluded { rule } => Some(rule.line),
        Decision::Tracked { by: None } => Some(0),
        _ => None,
    };
    let chosen = match (line(&as_dir), line(&as_file)) {
        (Some(dir), Some(file)) if dir >= file => &as_dir,
        (Some(_), None) => &as_dir,
        _ => &as_file,
    };
    match chosen {
        Decision::Excluded { rule } => match policy.interior(parts, rule) {
            Interior::Skip => {
                out.excluded.push(ExcludedEntry {
                    path,
                    kind: EntryKind::Link,
                    reason: ExclusionReason::Rule(rule.clone()),
                    interior: None,
                });
                Ok(())
            }
            Interior::Explore { because } => Err(EvaluationError::LinkOnReinclusionPath {
                path,
                excluded_by: rule.clone(),
                because,
            }),
        },
        Decision::Tracked { by } => Err(EvaluationError::TrackedLink {
            path,
            by: by.clone(),
        }),
        Decision::NestedGilBoundary { at } => Err(EvaluationError::NestedGilBoundary { path: at.clone() }),
        Decision::Protected { .. } | Decision::NestedPolicyFile { .. } => {
            unreachable!("위에서 이미 처리했다")
        }
    }
}

/// 연 디렉터리가 어디였는지 시험이 확인하는 자리 — **시험에서만 컴파일된다.**
///
/// 「완전히 제외된 폴더는 들어가지 않는다」는 결과만으로는 잴 수 없다(안이 조용하면 결과가 같다).
/// 그래서 목록을 읽은 디렉터리 자체를 적는다. thread 마다 따로 센다.
#[cfg(test)]
pub(crate) mod listed {
    use std::cell::RefCell;

    thread_local! {
        static DIRS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn note(trail: &[String]) {
        DIRS.with(|dirs| dirs.borrow_mut().push(trail.join("/")));
    }

    pub(crate) fn take() -> Vec<String> {
        DIRS.with(|dirs| std::mem::take(&mut *dirs.borrow_mut()))
    }
}

// ── 운영체제별 읽기 ────────────────────────────────────────────────────────

#[cfg(unix)]
mod os {
    use std::ffi::{CString, OsString};
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStringExt;
    use std::path::Path;

    /// 열어 둔 디렉터리 — 목록과 자식은 **이 핸들 기준으로만** 연다.
    pub(super) struct Dir(File);

    pub(super) enum Kind {
        File,
        Directory,
        Link,
        Special(&'static str),
        Missing,
    }

    fn cname(name: &str) -> io::Result<CString> {
        CString::new(name).map_err(|_| io::Error::other("이름에 NUL 이 있다"))
    }

    /// 루트 자신이 연결 파일이면 열지 않는다(`O_NOFOLLOW`). 조상 경로의 연결은 운영체제가 푼다.
    /// 끝의 `/` 가 `O_NOFOLLOW` 를 무력화하지 않도록 구성 요소로 다시 세워 연다(`internal_fs::dir_nofollow`,
    /// 루트 `.gilignore` 관측과 같은 문).
    pub(super) fn root(path: &Path) -> io::Result<Dir> {
        crate::internal_fs::dir_nofollow(path).map(Dir)
    }

    pub(super) fn names(dir: &Dir) -> io::Result<Vec<OsString>> {
        Ok(crate::internal_fs::names(&dir.0)?
            .into_iter()
            .map(|name| OsString::from_vec(name.into_bytes()))
            .collect())
    }

    pub(super) fn kind(dir: &Dir, name: &str) -> io::Result<Kind> {
        let name = cname(name)?;
        // SAFETY: zeroed stat 은 유효한 출력 버퍼다.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::fstatat(
                dir.0.as_raw_fd(),
                name.as_ptr(),
                &mut st,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            let error = io::Error::last_os_error();
            return match error.kind() {
                io::ErrorKind::NotFound => Ok(Kind::Missing),
                _ => Err(error),
            };
        }
        let format = st.st_mode & libc::S_IFMT;
        Ok(if format == libc::S_IFREG {
            Kind::File
        } else if format == libc::S_IFDIR {
            Kind::Directory
        } else if format == libc::S_IFLNK {
            Kind::Link
        } else if format == libc::S_IFIFO {
            Kind::Special("FIFO")
        } else if format == libc::S_IFSOCK {
            Kind::Special("socket")
        } else if format == libc::S_IFBLK {
            Kind::Special("block device")
        } else if format == libc::S_IFCHR {
            Kind::Special("character device")
        } else {
            Kind::Special("지원하지 않는 종류")
        })
    }

    /// `openat(O_DIRECTORY | O_NOFOLLOW)` — 그사이 연결 파일로 바뀌었으면 열리지 않는다.
    pub(super) fn child(dir: &Dir, name: &str) -> io::Result<Dir> {
        crate::internal_fs::child_dir(&dir.0, &cname(name)?).map(Dir)
    }
}

#[cfg(windows)]
mod os {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use crate::windows_fs::Chain;

    /// WinNT.h `FILE_ATTRIBUTE_REPARSE_POINT`.
    const REPARSE_POINT: u32 = 0x400;

    /// 볼륨 뿌리부터 조상을 고정한 디렉터리. 매 단계가 reparse point 가 아님을 확인하고 연다.
    pub(super) struct Dir {
        root: PathBuf,
        below: Vec<OsString>,
        chain: Chain,
    }

    pub(super) enum Kind {
        File,
        Directory,
        Link,
        Special(&'static str),
        Missing,
    }

    pub(super) fn root(path: &Path) -> io::Result<Dir> {
        Ok(Dir {
            root: path.to_path_buf(),
            below: Vec::new(),
            chain: Chain::open(path)?,
        })
    }

    pub(super) fn names(dir: &Dir) -> io::Result<Vec<OsString>> {
        std::fs::read_dir(dir.chain.path())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect()
    }

    /// 모든 reparse point(symbolic link, junction, mount point, 그 밖의 제공자)를 연결로 본다 —
    /// Windows 저장 계약(`windows_fs`)과 같다. `symlink_metadata` 는 reparse point 자신의 속성을 준다.
    pub(super) fn kind(dir: &Dir, name: &str) -> io::Result<Kind> {
        let at = dir.chain.child(OsStr::new(name))?;
        let data = match std::fs::symlink_metadata(&at) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Kind::Missing),
            Err(error) => return Err(error),
        };
        Ok(if data.file_attributes() & REPARSE_POINT != 0 {
            Kind::Link
        } else if data.is_dir() {
            Kind::Directory
        } else if data.is_file() {
            Kind::File
        } else {
            Kind::Special("지원하지 않는 종류")
        })
    }

    /// 뿌리부터 다시 고정해 내려간다 — 그사이 reparse point 로 바뀐 단계가 있으면 열리지 않는다.
    pub(super) fn child(dir: &Dir, name: &str) -> io::Result<Dir> {
        let mut chain = Chain::open(&dir.root)?;
        let mut below = dir.below.clone();
        below.push(OsString::from(name));
        for part in &below {
            chain.push(part, false)?;
        }
        Ok(Dir {
            root: dir.root.clone(),
            below,
            chain,
        })
    }
}

#[cfg(not(any(unix, windows)))]
mod os {
    use std::ffi::OsString;
    use std::io;
    use std::path::Path;

    pub(super) struct Dir;

    pub(super) enum Kind {
        File,
        Directory,
        Link,
        Special(&'static str),
        Missing,
    }

    fn unsupported() -> io::Error {
        io::Error::other("이 운영체제의 핸들 기준 읽기 전용 탐색은 아직 없다")
    }

    pub(super) fn root(_: &Path) -> io::Result<Dir> {
        Err(unsupported())
    }

    pub(super) fn names(_: &Dir) -> io::Result<Vec<OsString>> {
        Err(unsupported())
    }

    pub(super) fn kind(_: &Dir, _: &str) -> io::Result<Kind> {
        Err(unsupported())
    }

    pub(super) fn child(_: &Dir, _: &str) -> io::Result<Dir> {
        Err(unsupported())
    }
}
