//! **명시된 시작 판부터 원장의 마지막 판까지, 건넨 원장에서 빠짐없이** — 3n 의 구간([`AppliedSpan`])을 호출자가 판을
//! 하나씩 나열해 세우는 대신 건넨 정책 원장에서 고른다(편의상 「3o」, 제품 미연결).
//!
//! 3n 은 구간을 **받기만** 한다. 그래서 호출자가 중간 판을 빠뜨리거나 원장에 없는 판(예: 승인 전 제안)을 끼워 넣으면 그
//! 잘못된 구간에 대한 답이 나왔다. 이 모듈은 구간의 **자리와 순서와 끝을 원장이 정하게** 한다. 호출자가 고르는 것은
//! 시작점 하나뿐이다.
//!
//! ```text
//! 입력: PolicyLedger(P1 … Pn) + 시작점 + 판 이름별 정책 자료(이미 해석된 ApprovedPolicy)
//!
//! 시작점 Board(Pk)            → 자리 0 = Pk (확정 정책),  자리 1.. = Pk+1 … Pn   (Pn = 적용 정책)
//! 시작점 BeforePolicyAdoption → 자리 0 = 정책 채택 전,     자리 1.. = P1 … Pn
//!
//! 판마다: 건넨 자료의 정체성(engine + 원문 sha256) = 그 판의 원장 기록  ─▶ AppliedSpan::new ─▶ classify_path_continuity(3n)
//! ```
//!
//! # 시작점은 판 이름이다
//!
//! 같은 원문을 다시 승인하면 같은 원문 주소의 **다른 판**이 된다(계약 §5.1). 그래서 시작점은 원문 지문이 아니라 판 이름
//! ([`PolicyName`])으로 받고, 원장의 자리(`P<n>` 은 n번째 자리, 3a 의 [`PolicyLedger`] 가 보장)로 찾는다. 같은 원문의 판이
//! 여러 자리에 있어도 각자의 자리를 지킨다 — 한 [`ApprovedPolicy`] 값을 여러 판 이름에 건네도 된다.
//!
//! 「정책 채택 전」은 [`LedgerStart::BeforePolicyAdoption`] 으로 **명시했을 때만** 쓴다. 빈 원장이나 자료의 없음에서 생기지
//! 않는다. 적용된 판이 없는 구간(빈 원장 + 정책 채택 전)은 3n 의 기존 경계([`SpanProblem::NoAppliedPolicy`])를 그대로 돌려준다.
//!
//! # 고를 수 없는 것
//!
//! 입력에는 판 목록을 넣을 자리가 없다. 중간 판을 생략하는 길은 「그 판의 자료를 건네지 않기」뿐이고, 그것은
//! [`BoardPolicyProblem::Missing`] 으로 실패한다 — 그 판을 건너뛴 구간으로 성공하지 않는다. 원장 밖의 이름(승인 전 제안이 받을
//! 다음 이름 등)으로 건넨 자료는 [`BoardPolicyProblem::NotInLedger`], 원장의 판 자리에 다른 정책을 건넨 자료는
//! [`BoardPolicyProblem::Mismatch`] 로 거절한다. 중간 판에 기준점이 보관됐는지는 조건이 아니다(보관 대기 판도 구간에 든다).
//!
//! # 다시 읽거나 해석하지 않는다
//!
//! 정책 자료는 호출자가 이미 해석한 [`ApprovedPolicy`] 다. 이 모듈은 원문을 읽거나 해석하지 않고, 엔진이 해석할 때 세운
//! 정체성([`ApprovedPolicy::identity`])을 원장 기록의 `engine`·`rules` 와 견줄 뿐이다. 그 정체성은 엔진이 실제 바이트로 잰 것이므로
//! 주소가 다른 바이트로 해석한 자료는 여기서 불일치로 걸린다. 경로 판정은 3n 의 [`classify_path_continuity`] 그대로다.
//!
//! # 성공은 「건넨 원장의 그 구간을 빠짐없이 썼다」까지다
//!
//! 건넨 원장이 실제 프로젝트의 최신 원장인지, 시작 판이 실제로 마지막 확정 세계를 확정한 판인지(§6.1 — 사건·사이클에서 유도하는
//! 연결은 후속 작업), 각 판의 모니터 승인이 있었는지는 증명하지 않는다. 목록은 [`LEDGER_SPAN_DOES_NOT_SHOW`] 에 있다.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

use super::path_continuity::{AppliedSpan, ConfirmedStart, PathContinuity, SpanProblem, classify_path_continuity};
use super::{PolicyLedger, PolicyName, PolicyRecord};
use crate::artifact::gilignore::{ApprovedPolicy, PathKind};
use crate::artifact::{ContentDigest, EntryPath};

/// 이 선택의 결과가 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const LEDGER_SPAN_DOES_NOT_SHOW: [&str; 8] = [
    "원장의 출처 — 건넨 원장이 실제 프로젝트의 최신 원장인지, state.yaml 에서 빠짐없이 뽑혔는지 보지 않는다. 성공은 건넨 원장의 그 구간을 빠짐없이 썼다는 뜻까지다",
    "시작 판의 유도 — 명시된 시작 판이 마지막 확정 세계의 확정 정책(§6.1)인지, 정책 채택 전이 정말 시작인지 사건·사이클·세계 등록부에서 유도하거나 검사하지 않는다. 그 선택은 호출자의 것이다",
    "승인·적용 사실 — 각 판의 모니터 승인(proposal_digest 의 응답)과 관리 사건의 적용(after) 연결을 확인하지 않는다",
    "기준점 보관 — 중간 판에 기준점이 보관됐는지는 조건이 아니며 보지 않는다. 보관 대기 판도 다른 판과 같이 구간에 든다",
    "원문의 실재 — 정책 자료는 이미 해석된 값이다. 객체 창고의 원문이 실재하는지 보지 않고 원문을 다시 읽거나 해석하지 않는다. 엔진이 세운 정체성만 원장 기록과 견준다",
    "구간 밖 판 — 시작 판보다 앞선 판의 자료는 건넸다면 정체성만 견주고 판정에 쓰지 않으며, 건네지 않아도 된다",
    "내용의 변경·승인 결정 — 3n 과 같다. 계속 추적한 경로는 마지막 확정 세계와 견줄 대상이라는 뜻일 뿐이며 파일·폴더·원장·state.yaml·객체 창고를 읽거나 쓰지 않고 제품 경로와 연결되지 않았다",
    "미결 결정 — 즉시 보관 중단의 요청·해제(O-2), 보관 대기 중 체인 종료·사건 이관(O-3), 승인 이후 변경의 결속·보존·재적용에 아무 의미도 정하지 않는다",
];

/// 구간의 시작점 — **호출자가 명시한다.** 기본값이 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerStart {
    /// 마지막 확정 세계가 정책 채택 전에 확정됐다 — 구간은 정책 채택 전 → 원장의 모든 판.
    ///
    /// 「확정 정책을 모른다」나 「이력이 불완전하다」의 대용이 아니다(3n 의 [`ConfirmedStart::BeforePolicyAdoption`] 과 같은 호출 계약).
    BeforePolicyAdoption,
    /// 마지막 확정 세계의 확정 정책이라고 호출자가 명시한 **판 이름** — 원문 지문이 아니다.
    Board(PolicyName),
}

impl fmt::Display for LedgerStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerStart::BeforePolicyAdoption => f.write_str("정책 채택 전"),
            LedgerStart::Board(name) => write!(f, "{name}"),
        }
    }
}

/// 판 이름별로 건넨 **이미 해석된** 정책 자료 — 메모리의 값만 담는다.
///
/// 같은 원문의 여러 판에 같은 [`ApprovedPolicy`] 를 건네도 된다. 판 하나에는 자료 하나만 건넨다.
#[derive(Debug, Clone, Default)]
pub(crate) struct BoardPolicies<'a> {
    given: Vec<(PolicyName, &'a ApprovedPolicy)>,
}

impl<'a> BoardPolicies<'a> {
    pub(crate) fn new() -> BoardPolicies<'a> {
        BoardPolicies::default()
    }

    /// 「판 `name` 의 정책은 이것이다」 — 맞는지는 [`select_ledger_span`] 이 원장 기록과 견준다.
    pub(crate) fn with(mut self, name: PolicyName, policy: &'a ApprovedPolicy) -> BoardPolicies<'a> {
        self.given.push((name, policy));
        self
    }

    /// 건넨 자료를 **한 번** 훑어 판 이름별로 색인한다.
    ///
    /// 조회표의 크기는 건넨 서로 다른 이름의 수다(건넨 판 번호의 크기와 무관하다). `first_seen` 은 이름이 처음 나온 입력 순서를
    /// 지킨다. 두 번 이상 나온 이름은 `repeated` 로만 표시하고 자료를 고르지 않는다.
    fn index(&self) -> GivenIndex<'a> {
        let mut by_name = BTreeMap::new();
        let mut first_seen = Vec::new();
        for (name, policy) in &self.given {
            match by_name.entry(*name) {
                Entry::Vacant(slot) => {
                    slot.insert(Given { policy: *policy, repeated: false });
                    first_seen.push(*name);
                }
                Entry::Occupied(mut slot) => slot.get_mut().repeated = true,
            }
        }
        GivenIndex { by_name, first_seen }
    }
}

/// 판 이름 하나에 건넨 자료 — 두 번 이상 건넸으면 `repeated` 이고 `policy` 는 쓰지 않는다.
struct Given<'a> {
    policy: &'a ApprovedPolicy,
    repeated: bool,
}

/// [`BoardPolicies::index`] 의 결과 — 자료 검사·누락 검사·구간 구성이 모두 이것만 본다.
struct GivenIndex<'a> {
    by_name: BTreeMap<PolicyName, Given<'a>>,
    first_seen: Vec<PolicyName>,
}

impl<'a> GivenIndex<'a> {
    /// 그 판에 자료를 정확히 한 번 건넸을 때만 돌려준다. 중복된 판에서는 어느 자료도 고르지 않는다.
    fn only(&self, name: PolicyName) -> Option<&'a ApprovedPolicy> {
        self.by_name.get(&name).filter(|given| !given.repeated).map(|given| given.policy)
    }
}

/// 원장에서 고른 구간 — [`select_ledger_span`] 만 만든다.
///
/// 자리와 순서와 끝은 원장의 것이다: 판 목록은 시작 판(또는 `P1`)부터 원장의 마지막 판까지 빈틈없이 오름차순이고, 마지막
/// 판이 적용 정책이다.
#[derive(Debug, Clone)]
pub(crate) struct LedgerSpan<'a> {
    start: LedgerStart,
    boards: Vec<PolicyName>,
    span: AppliedSpan<'a>,
}

impl<'a> LedgerSpan<'a> {
    /// 호출자가 명시한 시작점.
    pub(crate) fn start(&self) -> LedgerStart {
        self.start
    }

    /// 구간에 든 판 이름 — 원장 순서 그대로. 정책 채택 전 시작은 판이 아니므로 여기 없다.
    pub(crate) fn boards(&self) -> &[PolicyName] {
        &self.boards
    }

    /// 적용 정책의 판 이름 — 원장의 마지막 판.
    pub(crate) fn applied_board(&self) -> PolicyName {
        *self.boards.last().expect("구간에는 적용 정책이 언제나 있다")
    }

    /// 3n 의 자리 번호(0 = 시작)가 가리키는 판. 정책 채택 전 시작의 0번 자리와 범위 밖 번호는 `None`.
    pub(crate) fn board_at(&self, position: usize) -> Option<PolicyName> {
        let index = match self.start {
            LedgerStart::Board(_) => Some(position),
            LedgerStart::BeforePolicyAdoption => position.checked_sub(1),
        };
        index.and_then(|at| self.boards.get(at).copied())
    }

    /// 3n 에 건네는 구간.
    pub(crate) fn applied_span(&self) -> &AppliedSpan<'a> {
        &self.span
    }

    /// 경로 하나를 3n 그대로 판정한다.
    pub(crate) fn classify(&self, path: &EntryPath, kind: PathKind) -> PathContinuity {
        classify_path_continuity(&self.span, path, kind)
    }
}

/// 원장에서 구간을 고를 수 없다. **하나라도 있으면 경로 판정을 하지 않는다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LedgerSpanProblem {
    /// 명시된 시작 판이 건넨 원장에 없다. `boards` 는 원장의 판 수다.
    StartNotInLedger { start: PolicyName, boards: usize },
    /// 정책 자료의 문제 — 건넨 이름마다 처음 나온 입력 순서로 하나씩(원장 밖·중복·불일치), 그 뒤에 구간 순서의 누락이 온다.
    ///
    /// 이름 하나에는 문제를 하나만 알린다: 원장 밖이면 그것만, 여러 번 건넨 이름은 중복 한 번만 알리고 그 이름의 정체성 비교는
    /// 생략한다. 그래서 모든 자료의 모든 결함을 나열한 목록은 아니다.
    Policies(Vec<BoardPolicyProblem>),
    /// 3n 이 구간을 세우지 않았다 — 빈 원장 + 정책 채택 전 시작의 [`SpanProblem::NoAppliedPolicy`].
    Span(SpanProblem),
}

/// 판 하나의 정책 자료 문제.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoardPolicyProblem {
    /// 원장에 없는 판 이름으로 자료를 건넸다 — 승인 전 제안을 끼워 넣는 길이 여기서 막힌다.
    NotInLedger { name: PolicyName },
    /// 같은 판에 자료를 두 번 이상 건넸다. 어느 것을 쓸지 고르지 않는다.
    GivenTwice { name: PolicyName },
    /// 구간에 든 판의 자료가 없다. 그 판을 건너뛰고 판정하지 않는다.
    Missing { name: PolicyName },
    /// 건넨 자료의 정체성이 그 판의 원장 기록과 다르다. `fields` 는 다른 칸(`engine`·`rules`)이고 `found` 는 자료의 원문 주소다.
    Mismatch { name: PolicyName, fields: Vec<&'static str>, found: ContentDigest },
}

impl fmt::Display for BoardPolicyProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoardPolicyProblem::NotInLedger { name } => {
                write!(f, "판 {name} 은 건넨 원장에 없다. 원장 밖의 정책(승인 전 제안 등)은 구간에 넣지 않는다")
            }
            BoardPolicyProblem::GivenTwice { name } => {
                write!(f, "판 {name} 의 정책 자료를 두 번 이상 건넸다. 어느 것을 쓸지 고르지 않는다")
            }
            BoardPolicyProblem::Missing { name } => {
                write!(f, "구간에 든 판 {name} 의 정책 자료가 없다. 그 판을 건너뛰고 판정하지 않는다")
            }
            BoardPolicyProblem::Mismatch { name, fields, found } => write!(
                f,
                "판 {name} 으로 건넨 정책이 원장 기록과 다르다({}; 건넨 원문 sha256:{})",
                fields.join(", "),
                found.hex()
            ),
        }
    }
}

impl fmt::Display for LedgerSpanProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerSpanProblem::StartNotInLedger { start, boards } => {
                write!(f, "시작 판 {start} 이 건넨 원장(판 {boards}개)에 없다. 경로 판정을 하지 않는다")
            }
            LedgerSpanProblem::Policies(problems) => {
                let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
                write!(f, "구간의 정책 자료가 원장과 맞지 않는다. 경로 판정을 하지 않는다: {}", shown.join(" / "))
            }
            LedgerSpanProblem::Span(problem) => write!(f, "{problem}"),
        }
    }
}

impl std::error::Error for LedgerSpanProblem {}

/// 건넨 자료가 원장 기록과 다른 칸.
fn differing_fields(record: &PolicyRecord, policy: &ApprovedPolicy) -> Vec<&'static str> {
    let identity = policy.identity();
    let mut fields = Vec::new();
    if identity.engine() != record.engine() {
        fields.push("engine");
    }
    if identity.rules_digest() != record.rules() {
        fields.push("rules");
    }
    fields
}

/// 명시된 시작점부터 원장의 마지막 판까지 구간을 고른다 — 판 목록은 원장이 정한다.
///
/// 순서: ① 시작 판이 원장에 있는가. ② 건넨 자료를 한 번 색인한다. ③ 건넨 이름마다 처음 나온 입력 순서로 하나만 진단한다 —
/// 원장 밖 이름이면 그것만, 원장의 판인데 여러 번 건넸으면 중복만(정체성 비교 생략), 나머지는 정체성 불일치(구간 밖 판도 견준다).
/// ④ 구간의 판마다 자료를 건넸는가(구간 순서). ③·④ 의 진단을 한 목록으로 돌려준다. ⑤ 같은 색인에서 판마다 자료를 골라 3n 의
/// [`AppliedSpan::new`] 로 구간을 세운다.
pub(crate) fn select_ledger_span<'a>(
    ledger: &PolicyLedger,
    start: LedgerStart,
    policies: &BoardPolicies<'a>,
) -> Result<LedgerSpan<'a>, LedgerSpanProblem> {
    let records = ledger.policies();
    // P<n> 은 n번째 자리다(PolicyLedger 가 보장) — 이름으로 자리를 찾는다. 원문 지문으로 찾지 않는다.
    let first = match start {
        LedgerStart::BeforePolicyAdoption => 0,
        LedgerStart::Board(name) => {
            let at = name.number() as usize - 1;
            if at >= records.len() {
                return Err(LedgerSpanProblem::StartNotInLedger { start: name, boards: records.len() });
            }
            at
        }
    };

    let given = policies.index();
    let mut problems = Vec::new();
    for name in &given.first_seen {
        // P<n> 은 n번째 자리다 — 이름 하나에 원장 조회 한 번.
        let Some(record) = records.get(name.number() as usize - 1) else {
            problems.push(BoardPolicyProblem::NotInLedger { name: *name });
            continue;
        };
        let Some(policy) = given.only(*name) else {
            problems.push(BoardPolicyProblem::GivenTwice { name: *name });
            continue;
        };
        let fields = differing_fields(record, policy);
        if !fields.is_empty() {
            let found = policy.identity().rules_digest().clone();
            problems.push(BoardPolicyProblem::Mismatch { name: *name, fields, found });
        }
    }
    let in_span = &records[first..];
    for record in in_span {
        if !given.by_name.contains_key(&record.name()) {
            problems.push(BoardPolicyProblem::Missing { name: record.name() });
        }
    }
    if !problems.is_empty() {
        return Err(LedgerSpanProblem::Policies(problems));
    }

    let boards: Vec<PolicyName> = in_span.iter().map(PolicyRecord::name).collect();
    let mut chosen = boards.iter().map(|name| given.only(*name).expect("자료 검사를 통과했다"));
    let confirmed = match start {
        LedgerStart::BeforePolicyAdoption => ConfirmedStart::BeforePolicyAdoption,
        LedgerStart::Board(_) => ConfirmedStart::Policy(chosen.next().expect("시작 판은 구간의 첫 판이다")),
    };
    let span = AppliedSpan::new(confirmed, chosen.collect()).map_err(LedgerSpanProblem::Span)?;
    let positions = match start {
        LedgerStart::Board(_) => boards.len(),
        LedgerStart::BeforePolicyAdoption => boards.len() + 1,
    };
    debug_assert_eq!(span.len(), positions, "3n 의 자리 수가 원장 구간과 같아야 한다");
    Ok(LedgerSpan { start, boards, span })
}

/// 건넨 원장의 구간을 고르고 경로 하나를 3n 그대로 판정한다 — 구간을 고를 수 없으면 판정하지 않는다.
pub(crate) fn classify_path_over_ledger(
    ledger: &PolicyLedger,
    start: LedgerStart,
    policies: &BoardPolicies<'_>,
    path: &EntryPath,
    kind: PathKind,
) -> Result<PathContinuity, LedgerSpanProblem> {
    select_ledger_span(ledger, start, policies).map(|span| span.classify(path, kind))
}
