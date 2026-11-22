//! **이 경로는 마지막 확정 세계 때부터 지금까지 계속 추적 대상이었는가** — 계약 §5.4 의 「계속 추적한 경로」를
//! 경로 하나에 대해 판정한다(편의상 「3n」, 제품 미연결).
//!
//! 계약 §5.4 는 세 정책을 섞지 않는다: 비교 근거(마지막 확정 세계와 그 **확정 정책**, §6.1 로 유도), **적용 정책**
//! (원장의 마지막 판), **제안 정책**(승인 저장 전에는 어떤 판정에도 쓰지 않는다). 내용을 비교하는 대상은 확정 정책부터
//! 적용 정책까지 원장에 있는 **모든 판**의 범위에 속한 경로뿐이다. 이 모듈은 호출자가 메모리로 건넨 그 구간
//! ([`AppliedSpan`])과 경로 하나로 그 분류만 돌려준다.
//!
//! ```text
//! 구간의 시작                     그 뒤 적용된 판 …          적용 정책(구간의 끝)
//!   Policy(확정 정책)  ──  P2 ──  …  ──  Pn               → 판마다 ApprovedPolicy::decide
//!   BeforePolicyAdoption ─ P1 ──  …  ──  Pn               → 시작은 기존 전체 관측(루트 .gil/ 만 제외)
//!
//! 적용 정책이 추적하지 않는다            → OutsideCurrentScope(그 판정 그대로: 제외 / 시스템 보호 / 하위 정책 파일 / 중첩 경계)
//! 적용 정책이 추적 + 구간 모든 자리 추적  → ContinuouslyTracked          (마지막 확정 세계와 내용을 견줄 대상)
//! 적용 정책이 추적 + 어느 자리에서 아님  → NewlyTracked { 그 자리들 }   (처음 포함, 또는 제외됐다가 다시 포함)
//! ```
//!
//! # 성공은 「건네받은 구간에 대한 판정」이다
//!
//! 이 함수는 구간을 **받는다.** 원장·`state.yaml`·사건·세계 등록부를 읽어 구간을 찾지 않는다. 그래서 결과는 건넨 판들에
//! 대해서만 참이다: 호출자가 중간 판을 빠뜨렸거나, 원장의 판이 아닌 것을 넣었거나, 확정 정책이 아닌 판을 시작으로 줬다면
//! 그 잘못된 구간에 대한 답이 나온다. 빠진 판을 알아내거나 원장의 완전성·각 판의 승인 사실을 증명하지 않는다.
//! 보이지 않는 것은 [`CONTINUITY_DOES_NOT_SHOW`] 에 적었다.
//!
//! # 규칙 해석은 엔진의 것이다
//!
//! 판마다 [`ApprovedPolicy::decide`] 를 그대로 부른다. 규칙 일치·재포함(`!`)·마지막 줄 우선·시스템 이름의 대소문자 변형은
//! 엔진이 정하고 이 모듈은 다시 해석하지 않는다. 적용 정책이 추적하지 않는 경로는 엔진의 [`Decision`] 을 바꾸지 않고
//! 돌려준다 — 「제외」와 시스템 보호·하위 정책 파일·중첩 GIL 경계를 한 갈래로 뭉치지 않는다.
//!
//! # 정책 채택 전 기록
//!
//! 마지막 확정 세계가 정책이 생기기 전에 확정됐다면 그 세계의 범위는 **기존 전체 관측**이다(§5.4 「정책이 없던 기록의
//! 범위는 기존 전체 관측(루트 `.gil/`만 제외)」). 이것은 빈 원문을 컴파일한 정책과 다르다 — 빈 정책도 `.git`·루트
//! `.gilignore` 를 시스템 보호로 빼지만, 기존 관측은 루트의 `.gil` 디렉터리 하나만 건너뛴다(`artifact::walk`).
//! 그래서 [`ConfirmedStart::BeforePolicyAdoption`] 은 정책이 아닌 따로 이름 붙은 시작이고, 호출자가 명시적으로 골라야만
//! 생긴다. `Option` 의 없음이나 기본값으로 생기지 않는다. 확정 정책을 모르는 경우·이력이 불완전한 경우를 이것으로
//! 대신하면 안 된다 — 이 모듈은 그 차이를 알 수 없으므로 호출 계약으로 둔다.
//!
//! # 제안 정책은 입력이 아니다
//!
//! 구간은 확정 정책과 **적용된** 판만 받는다. 승인 저장 전의 제안을 넣을 자리가 따로 없고, 넣으면 그것은 호출자가
//! 적용 정책이라고 주장한 판이 된다. 그래서 지금까지 계속 추적한 경로는 다음 제안이 제외를 요청해도
//! `ContinuouslyTracked` 로 남는다(§5.4 ② 「제안 정책으로 범위를 좁히지 않는다」). 이 함수는 **비교 대상**만 정한다.
//! 파일 내용이 바뀌었는지 보거나, 미확정 변경을 판정하거나, 승인을 허락·거절하지 않는다.

use std::fmt;

use crate::artifact::gilignore::{ApprovedPolicy, Decision, PathKind};
use crate::artifact::{EntryPath, GIL_DIR};

/// 이 판정의 결과가 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const CONTINUITY_DOES_NOT_SHOW: [&str; 9] = [
    "구간의 완전성 — 건넨 판들이 원장의 그 구간 전부인지 보지 않는다. 빠진 판·순서가 바뀐 판·원장에 없는 판을 알아내지 못하며, 결과는 건넨 구간에 대한 판정일 뿐이다",
    "승인·적용 사실 — 각 판이 모니터가 승인해 적용된 판인지, 마지막 판이 지금의 적용 정책인지 확인하지 않는다. ApprovedPolicy 를 받았다는 것은 그 바이트가 해석된다는 뜻뿐이다",
    "확정 정책의 유도 — 시작 판이 마지막 확정 세계의 확정 정책(§6.1)인지, 정책 채택 전 기록이 정말 시작인지 검사하지 않는다. 그 선택은 호출자의 것이다",
    "내용의 변경 — 파일 내용·삭제·새 파일을 보지 않는다. 계속 추적한 경로라는 것은 마지막 확정 세계와 견줄 대상이라는 뜻이지, 같다거나 다르다는 뜻이 아니다",
    "승인 결정 — 미확정 변경이 있는지, 승인을 적용해도 되는지 정하지 않는다(§5.4 ② 의 검사는 연결 계층의 몫이다)",
    "제외 기간의 이력 — 새로 추적하는 경로가 제외돼 있던 동안 무엇이었는지, 그때 있었는지 추측하지 않는다. 알려 주는 것은 구간의 어느 자리가 추적하지 않았는가뿐이다",
    "트리와 파일 시스템 — 폴더를 훑거나 항목 종류를 보지 않는다. 경로와 호출자가 건넨 종류(PathKind)의 글자만 본다. 연결 파일·특수 항목·관측 거절은 트리 평가의 몫이다",
    "기록 — 원장·state.yaml·사건·세계 등록부·.gilignore 를 읽거나 쓰지 않고, 보관·복원·Monitor 와 연결되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 체인 종료·사건 이관(O-3), 승인 이후 변경의 결속·보존·재적용에 아무 의미도 정하지 않는다",
];

/// 구간의 시작 — 마지막 확정 세계를 **어떤 범위로** 관측해 확정했는가.
///
/// 기본값이 없다. 호출자가 둘 중 하나를 골라 이름으로 적어야 한다.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfirmedStart<'a> {
    /// 정책 채택 전의 기록 — 그 세계는 기존 전체 관측(루트 `.gil/` 만 제외)으로 확정됐다.
    ///
    /// 「확정 정책을 모른다」나 「이력이 불완전하다」의 대용이 아니다. 그런 입력이면 부르지 말아야 한다.
    BeforePolicyAdoption,
    /// 마지막 확정 세계의 확정 정책(§6.1).
    Policy(&'a ApprovedPolicy),
}

/// 확정 정책부터 적용 정책까지 — **호출자가 건넨** 순서 있는 구간.
///
/// 자리 0 은 시작([`ConfirmedStart`]), 자리 1.. 은 그 뒤 적용된 판을 적용된 순서대로 담는다. 마지막 자리가 적용 정책이다.
/// 시작이 확정 정책이고 뒤의 판이 없으면 확정 정책이 곧 적용 정책인 한 판짜리 구간이다. 같은 원문의 판이 여러 자리에 있어도
/// 된다 — 같은 원문의 다른 판은 오류가 아니다(3a).
#[derive(Debug, Clone)]
pub(crate) struct AppliedSpan<'a> {
    start: ConfirmedStart<'a>,
    later: Vec<&'a ApprovedPolicy>,
}

/// 구간을 세울 수 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpanProblem {
    /// 시작이 정책 채택 전인데 그 뒤 적용된 판이 없다 — 적용 정책이 없는 구간이다.
    ///
    /// 정책이 없는 프로젝트의 변경 검사는 현행 규칙 그대로이며(§6.2) 이 판정의 대상이 아니다.
    NoAppliedPolicy,
}

impl fmt::Display for SpanProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanProblem::NoAppliedPolicy => f.write_str(
                "구간이 정책 채택 전 기록에서 시작하는데 그 뒤 적용된 정책 판이 없다. 적용 정책이 없으면 계속 추적 \
                 판정을 하지 않는다 — 정책이 없는 프로젝트의 변경 검사는 현행 규칙 그대로다",
            ),
        }
    }
}

impl std::error::Error for SpanProblem {}

impl<'a> AppliedSpan<'a> {
    /// 시작과 그 뒤 적용된 판들로 구간을 세운다. 판의 순서·완전성·승인 여부는 검사하지 않는다(호출자의 계약).
    pub(crate) fn new(
        start: ConfirmedStart<'a>,
        later: Vec<&'a ApprovedPolicy>,
    ) -> Result<AppliedSpan<'a>, SpanProblem> {
        match (start, later.is_empty()) {
            (ConfirmedStart::BeforePolicyAdoption, true) => Err(SpanProblem::NoAppliedPolicy),
            _ => Ok(AppliedSpan { start, later }),
        }
    }

    /// 구간의 자리 수 — 시작을 포함한다.
    pub(crate) fn len(&self) -> usize {
        1 + self.later.len()
    }

    /// 적용 정책 — 구간의 마지막 자리. `new` 가 언제나 하나를 보장한다.
    pub(crate) fn applied(&self) -> &'a ApprovedPolicy {
        match (self.later.last(), self.start) {
            (Some(last), _) => last,
            (None, ConfirmedStart::Policy(only)) => only,
            (None, ConfirmedStart::BeforePolicyAdoption) => unreachable!("new 가 적용 정책 없는 구간을 거절했다"),
        }
    }

    /// 적용 정책보다 **앞**의 자리 가운데 그 경로를 범위에 넣지 않은 자리 번호(오름차순).
    ///
    /// 한 판짜리 구간(시작이 곧 적용 정책)에는 앞자리가 없다.
    fn earlier_not_containing(&self, path: &EntryPath, kind: PathKind) -> Vec<usize> {
        let Some((_, between)) = self.later.split_last() else {
            return Vec::new();
        };
        let start_contains = match self.start {
            ConfirmedStart::BeforePolicyAdoption => legacy_observation_contains(path, kind),
            ConfirmedStart::Policy(policy) => policy.decide(path, kind).is_tracked(),
        };
        let mut missing = Vec::new();
        if !start_contains {
            missing.push(0);
        }
        for (index, policy) in between.iter().enumerate() {
            if !policy.decide(path, kind).is_tracked() {
                missing.push(index + 1);
            }
        }
        missing
    }
}

/// 기존 전체 관측(정책 채택 전)이 그 경로를 범위에 넣는가 — `artifact::walk` 의 건너뛰기 규칙 그대로.
///
/// 기존 관측은 **루트 바로 아래의 `.gil` 디렉터리 하나만** 건너뛴다(이름은 글자 그대로 비교한다). 더 깊은 `.gil` 디렉터리는
/// 관측 전체를 거절하므로 그 아래는 관측된 적이 없다. `.git`·`.gilignore` 는 기존 관측에서 일반 경로였다. 이 판정은 적용
/// 정책이 추적한다고 한 경로에만 쓰이므로(그런 경로에는 시스템 이름 구성 요소가 없다) 실제로 거짓이 되는 일은 없지만,
/// 빈 정책으로 대신하지 않으려고 기존 규칙을 따로 적는다.
pub(super) fn legacy_observation_contains(path: &EntryPath, kind: PathKind) -> bool {
    let parts: Vec<&str> = path.as_str().split('/').collect();
    let last = parts.len() - 1;
    !parts
        .iter()
        .enumerate()
        .any(|(index, part)| *part == GIL_DIR && (index < last || kind == PathKind::Directory))
}

/// 한 경로의 분류 — 계약 §5.4 의 표 그대로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathContinuity {
    /// 구간의 모든 자리가 범위에 넣었다 — **계속 추적한 경로.** 마지막 확정 세계와 내용을 견줄 대상이다.
    ///
    /// `current` 는 적용 정책의 판정(언제나 [`Decision::Tracked`])이다.
    ContinuouslyTracked { current: Decision },
    /// 적용 정책은 추적하지만 구간의 앞자리 가운데 범위에 넣지 않은 곳이 있다 — **새로 추적하는 경로.**
    ///
    /// 처음 포함(자리 0 부터 빠졌다)과 제외됐다가 다시 포함(중간 자리가 빠졌다)이 모두 여기다. 비교할 과거 기준이 없다.
    /// `not_tracked_at` 은 범위에 넣지 않은 자리 번호(0 = 시작)를 오름차순으로 담는다. 그 동안의 내용·존재는 말하지 않는다.
    NewlyTracked { current: Decision, not_tracked_at: Vec<usize> },
    /// 적용 정책이 추적하지 않는다 — 현재 범위 밖. 엔진의 판정을 그대로 담는다(언제나 `Tracked` 가 아니다):
    /// 승인 규칙의 제외, 시스템 보호, 하위 정책 파일 자리, 중첩 GIL 경계.
    OutsideCurrentScope { current: Decision },
}

impl PathContinuity {
    /// 마지막 확정 세계와 내용을 견줄 대상인가 — 계속 추적한 경로일 때만 참이다.
    pub(crate) fn is_comparison_target(&self) -> bool {
        matches!(self, PathContinuity::ContinuouslyTracked { .. })
    }

    /// 적용 정책의 판정.
    pub(crate) fn current(&self) -> &Decision {
        match self {
            PathContinuity::ContinuouslyTracked { current }
            | PathContinuity::NewlyTracked { current, .. }
            | PathContinuity::OutsideCurrentScope { current } => current,
        }
    }
}

impl fmt::Display for PathContinuity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathContinuity::ContinuouslyTracked { .. } => f.write_str(
                "건넨 구간의 모든 판이 추적했다 — 계속 추적한 경로이며 마지막 확정 세계와 견줄 대상이다",
            ),
            PathContinuity::NewlyTracked { not_tracked_at, .. } => {
                let shown: Vec<String> = not_tracked_at.iter().map(|at| format!("{at}번 자리")).collect();
                write!(
                    f,
                    "적용 정책은 추적하지만 건넨 구간의 {}가 추적하지 않았다 — 새로 추적하는 경로이며 견줄 과거 기준이 없다",
                    shown.join(", ")
                )
            }
            PathContinuity::OutsideCurrentScope { current } => {
                write!(f, "적용 정책의 범위 밖이다 — {}", describe(current))
            }
        }
    }
}

fn describe(decision: &Decision) -> String {
    match decision {
        Decision::Tracked { .. } => "추적".to_string(),
        Decision::Excluded { rule } => format!("{rule} 이 제외했다"),
        Decision::Protected { protection, at } => format!("시스템 보호 {protection} ({})", at.as_str()),
        Decision::NestedPolicyFile { at } => format!("하위 정책 파일 자리 ({})", at.as_str()),
        Decision::NestedGilBoundary { at } => format!("중첩 GIL 경계 ({})", at.as_str()),
    }
}

/// 건넨 구간에서 경로 하나가 계속 추적 대상이었는지 판정한다 — **건넨 구간에 대한 판정**일 뿐이다.
///
/// 순서: ① 적용 정책의 판정이 추적이 아니면 그대로 [`PathContinuity::OutsideCurrentScope`]. ② 추적이면 앞자리를 차례로
/// 보고, 범위에 넣지 않은 자리가 없으면 계속 추적, 있으면 새로 추적. 판마다 [`ApprovedPolicy::decide`] 하나만 부르고,
/// 정책 채택 전 시작만 기존 관측의 건너뛰기 규칙을 쓴다. 파일 시스템·기록을 보지 않는다.
pub(crate) fn classify_path_continuity(span: &AppliedSpan<'_>, path: &EntryPath, kind: PathKind) -> PathContinuity {
    let current = span.applied().decide(path, kind);
    if !current.is_tracked() {
        return PathContinuity::OutsideCurrentScope { current };
    }
    let not_tracked_at = span.earlier_not_containing(path, kind);
    match not_tracked_at.is_empty() {
        true => PathContinuity::ContinuouslyTracked { current },
        false => PathContinuity::NewlyTracked { current, not_tracked_at },
    }
}
