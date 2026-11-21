//! 건네받은 관리 사건의 **실제 사이클 참조**가 존재·닫힘·체인 소속을 지키는지와 **발급 번호 관계** 검사 — 구현 순서 3번 안의 작은
//! 후속(편의상 「3h」).
//!
//! 「3h」는 이 PR 의 편의상 이름이다. **계약 T7 전체의 완료가 아니고, 3번 전체의 완료도 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 사건 칸 표, §5.7 의 T7, §6.3·§6.5·§6.6 이다.
//! 범위 설명은 [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3h」 기록에 있다.
//!
//! ```text
//! EventCycleFacts* ─┬─ 이름 자리(3f·3g 와 같은 규칙) ─────────────────────▶ NameOutOfPlace 갈래
//!   (BaselineFacts  ├─ check_baselines (3e, 그대로) ───────────────────────▶ Point 갈래
//!    + chain_ref    ├─ 이름 자리가 섰을 때만: next_cycle_id 감소 비교 ─────▶ IssuerDecreased
//!    + after_cycle  ├─ baseline.next_cycle_id ≥ next_cycle_id ─────────────▶ BaselineIssuerBelowEvent
//!    + next_cycle)  └─ 실제 사이클 참조만, 조회가 모호하지 않을 때만:
//! &[Chain] ── Chain::id · Chain::owns · Cycles::node · Cycle::is_closed (읽기 전용)
//!                       존재 · 체인 소속 · 닫힘 · 번호 < 발급 번호 ─────────▶ Unknown*/OtherChain/OpenCycle/CycleNotBeforeIssuer
//!                                                     check_cycle_refs ─▶ CycleRefsAgree | [CycleRefProblem]
//! ```
//!
//! # 입력은 「뽑아 건넨 값」과 기존 조회 모델이다
//!
//! [`EventCycleFacts`] 는 미래의 전체 사건 기록에서 이 검사에 필요한 칸만 뽑은 값이다 — 3e 의 [`BaselineFacts`] 를 그대로 지니고
//! (그래서 이 검사가 쓰는 `baseline` 경계·발급기 칸은 3e 가 검사한 바로 그 값이다), 그 사건의 `chain_ref`·`after_cycle_ref` 칸·
//! `next_cycle_id` 를 덧붙인다. 새 저장 꼴·직렬화·영구 상태가 없다. `review_cycle_ref`·`world_before_ref` 는 받지 않는다.
//!
//! 사이클의 존재·닫힘·소속은 호출자가 건넨 [`Chain`] 목록(`Project::chains()` 가 돌려주는 꼴)에 **읽기 전용으로 묻는다**:
//! [`Chain::id`], [`Chain::owns`], [`Cycles::node`](crate::Cycles::node), [`Cycle::is_closed`](crate::Cycle::is_closed). 같은 정보를
//! 따로 뽑은 bool 목록으로 믿지 않는다. 이 검사가 그 목록에 대해 **스스로 확인하는 것**은 조회가 모호하지 않다는 것뿐이다 — 같은
//! 체인 이름이 둘이거나 같은 사이클을 두 체인이 소유하면 첫 항목을 고르지 않고 [`LookupProblem`] 으로 돌려주며 참조 조건을 평가하지
//! 않는다. 한 체인 안의 사이클 이름이 겹치지 않는 것은 [`Cycles`](crate::Cycles) 의 생성·복원 규칙이 이미 보장한다. 직렬 계보·
//! Entry/Exit 인계 같은 그 밖의 체인 불변식, 그리고 그 목록이 사건을 뽑은 **같은 상태**의 것이라는 사실은 호출자가 보장한다.
//!
//! # 실제 참조와 참조 부재
//!
//! `after_cycle_ref` 와 `baseline.after_cycle_ref` 는 3e 의 [`BaselineSlot`] 으로 받는다 — 칸 없음 / `null` / 실제 [`CycleRef`]. 실제
//! 참조가 적힌 칸만 T7 의 참조 조건을 검사한다. `null` 과 칸 없음은 **거절하지도, 유효하다고 승인하지도 않고** 성공 값에 사건마다
//! 구분해 남긴다([`RefReading`]). 시작 사건의 `null` 경계는 누락이나 잘못된 참조로 취급하지 않는다. 일반 사건·기존 프로젝트 첫 채택의
//! `null` 경계를 허용할지는 정본이 정하지 않았고 이 검사도 정하지 않는다. 기준점 칸의 누락·부분 기록·동반 존재는 3e 의 일이다.
//!
//! # 일부러 요구하지 않는 것
//!
//! - 발급 번호가 이전 값 + 1 이거나 늘 증가해야 한다는 것 — 같은 경계의 연속 사건은 같은 값을 쓴다(§5.3·§6.3).
//! - 사이클 번호·경계가 사건 순서대로 증가해야 한다는 것 — 과거 경계로 되돌아갈 수 있다(§6.4·§6.5).
//! - 보관 경계·발급 번호가 승인 경계·발급 번호와 같아야 한다는 것 — 대기 중 인터뷰가 닫히면 그 경계에서 보관한다(§6.3).
//! - 과거 사건의 발급 번호가 현재 발급기와 같아야 한다는 것, 가리킨 체인이 지금 열려 있어야 한다는 것.
//! - 소속을 현재 갈래의 조상 관계나 번호의 같음으로 대신하는 것 — `chain:C1` 과 `cycle:C1` 은 다른 참조다.
//!
//! 검사하지 않은 것은 [`CYCLE_REFS_DO_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_history::first_name_out_of_place;
use super::baseline_record::{BaselineFacts, BaselineProblem, BaselineSlot, BaselinesAgree, check_baselines};
use super::event_links::EventName;
use crate::cycles::CycleId;
use crate::refs::{ChainRef, CycleRef};
use crate::{Chain, Cycle};

/// 이 검사가 성공해도 **보이지 않은** 것.
pub(crate) const CYCLE_REFS_DO_NOT_SHOW: [&str; 12] = [
    "참조 부재의 종류별 의미 — after_cycle_ref·baseline.after_cycle_ref 의 null 과 칸 없음은 거절하지도 유효하다고 승인하지도 않는다(성공 값의 not_judged 에 드러난다). 일반 사건·기존 프로젝트 첫 채택의 null 경계 허용 여부는 정하지 않았다",
    "scope_change 의 review_cycle_ref 존재 의무·종류별 필수 칸과, review_cycle_ref 와 승인 경계의 관계 — review_cycle_ref 는 입력에 없다",
    "실제 사이클 참조가 하나도 없는 사건의 chain_ref 실재 — 조회하지 않으며 성공 값의 chains_not_looked_up 에 드러난다",
    "사건 저장 당시의 닫힘 — 지금 기록의 닫힘만 본다. 닫힌 사이클은 다시 열리지 않으므로 지금 열린 사이클은 거절 근거지만, 지금 닫혔다는 사실이 저장 당시의 닫힘을 증명하지는 않는다",
    "입력이 실제 사건 원장에서 빠짐없이 뽑혔는가, 3c–3g 입력과 같은 사건에서 뽑혔는가, 건넨 Chain 목록이 같은 상태의 것인가 — 후속 연결 계층의 책임. 목록의 끝이 잘려도 감소 비교는 남은 앞부분만 본다",
    "건넨 Chain 목록의 체인 불변식 — 직렬 계보·Entry/Exit 인계·발급기 경계는 Project 의 기존 검사 몫이다. 이 검사는 조회의 모호함(같은 체인 이름, 두 체인이 소유한 사이클)만 거절한다",
    "사건 발급 번호와 현재 프로젝트 발급기의 관계, 사건 순서와 체인 순서의 관계(체인 사이의 미완료 사건 이관 — §12 O-3 미결)",
    "보관 대기 중 인터뷰 뒤의 경계라는 세부 전이(§6.3), 보관 경계가 그때 서 있던 경계였는가",
    "world_before_ref 와 마지막 확정 세계(T8), 현재 세계와 다음 Entry(§6.2), 보관 대기 뒤의 실험(T9)",
    "실제 승인·보관 실행·재시도, 사건을 만들 권한, 다음 실험을 열어도 되는가",
    "기준점·보관 시도의 시간 측면과 위치(3f·3g 의 일), 정책 연결(3c·3d 의 일)",
    "형식 전환·활성화·제품 연결 — 이 검사를 부르는 제품 경로가 없다",
];

// ── 뽑아 건넨 칸 ───────────────────────────────────────────────────────────

/// 사건 하나에서 이 검사가 뽑아 받은 칸.
///
/// `next_cycle_id` 는 정본에 `null` 꼴이 없으므로(§5.1) 수로만 받는다 — 칸이 빠진 기록은 뽑는 계층이 거절할 일이다.
/// `after_cycle_ref` 는 `null` 일 수 있으므로(`start`, §5.1 예시) 칸 없음과 `null` 이 섞이지 않게 [`BaselineSlot`] 으로 받는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventCycleFacts {
    baseline: BaselineFacts,
    chain: ChainRef,
    boundary: BaselineSlot<Option<CycleRef>>,
    issuer: u32,
}

impl EventCycleFacts {
    pub(crate) fn new(
        baseline: BaselineFacts,
        chain: ChainRef,
        boundary: BaselineSlot<Option<CycleRef>>,
        issuer: u32,
    ) -> EventCycleFacts {
        EventCycleFacts { baseline, chain, boundary, issuer }
    }

    /// 3e 의 입력 그대로 — 이름·종류·기준점·`baseline` 경계/발급기 칸·시도 결과 순서.
    pub(crate) fn baseline(&self) -> &BaselineFacts {
        &self.baseline
    }

    pub(crate) fn event(&self) -> EventName {
        self.baseline.event()
    }

    /// `chain_ref`.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 사건의 `after_cycle_ref` 칸.
    pub(crate) fn boundary(&self) -> BaselineSlot<Option<CycleRef>> {
        self.boundary
    }

    /// 사건의 `next_cycle_id`.
    pub(crate) fn issuer(&self) -> u32 {
        self.issuer
    }
}

/// 사이클 참조가 적히는 두 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleField {
    /// 사건의 `after_cycle_ref` — 비교하는 발급 번호는 사건의 `next_cycle_id`.
    AfterCycleRef,
    /// `baseline.after_cycle_ref` — 비교하는 발급 번호는 `baseline.next_cycle_id`.
    BaselineAfterCycleRef,
}

impl CycleField {
    /// 계약 §5.1 의 칸 이름.
    pub(crate) fn name(self) -> &'static str {
        match self {
            CycleField::AfterCycleRef => "after_cycle_ref",
            CycleField::BaselineAfterCycleRef => "baseline.after_cycle_ref",
        }
    }

    /// 그 칸과 견주는 발급 번호 칸의 이름.
    pub(crate) fn issuer_name(self) -> &'static str {
        match self {
            CycleField::AfterCycleRef => "next_cycle_id",
            CycleField::BaselineAfterCycleRef => "baseline.next_cycle_id",
        }
    }
}

// ── 조회 ───────────────────────────────────────────────────────────────────

/// 건넨 `Chain` 목록으로는 조회가 하나로 정해지지 않는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LookupProblem {
    /// 같은 체인 이름이 목록에 두 번 이상 있다(`count` 번).
    DuplicateChain { chain: ChainRef, count: usize },
    /// 같은 사이클을 둘 이상의 체인이 소유한다(목록 순서).
    CycleInSeveralChains { cycle: CycleRef, chains: Vec<ChainRef> },
}

impl fmt::Display for LookupProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupProblem::DuplicateChain { chain, count } => {
                write!(f, "건넨 체인 목록에 {chain} 이 {count}번 있다. 어느 체인인지 정할 수 없다")
            }
            LookupProblem::CycleInSeveralChains { cycle, chains } => write!(
                f,
                "{cycle} 을 여러 체인({})이 소유한다. 소속을 정할 수 없다",
                chains.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

/// 건넨 `Chain` 목록에 대한 읽기 전용 조회. [`ChainLookup::new`] 가 모호함이 없음을 확인한 뒤에만 선다.
struct ChainLookup<'a> {
    chains: &'a [Chain],
}

impl<'a> ChainLookup<'a> {
    fn new(chains: &'a [Chain]) -> Result<ChainLookup<'a>, Vec<LookupProblem>> {
        let mut problems = Vec::new();
        let mut names: Vec<ChainRef> = Vec::new();
        for chain in chains {
            if names.contains(&chain.id()) {
                continue;
            }
            names.push(chain.id());
            let count = chains.iter().filter(|other| other.id() == chain.id()).count();
            if count > 1 {
                problems.push(LookupProblem::DuplicateChain { chain: chain.id(), count });
            }
        }
        let mut cycles: Vec<CycleRef> = Vec::new();
        for chain in chains {
            for cycle in chain.cycles().nodes() {
                let cycle = cycle.id().to_ref();
                if cycles.contains(&cycle) {
                    continue;
                }
                cycles.push(cycle);
                let owners: Vec<ChainRef> =
                    chains.iter().filter(|other| other.owns(cycle)).map(Chain::id).collect();
                if owners.len() > 1 {
                    problems.push(LookupProblem::CycleInSeveralChains { cycle, chains: owners });
                }
            }
        }
        match problems.is_empty() {
            true => Ok(ChainLookup { chains }),
            false => Err(problems),
        }
    }

    fn has_chain(&self, chain: ChainRef) -> bool {
        self.chains.iter().any(|known| known.id() == chain)
    }

    /// 그 사이클을 소유한 체인과 사이클 — 기존 소유 관계([`Chain::owns`])와 [`crate::Cycles::node`] 로 읽는다.
    fn owner(&self, cycle: CycleRef) -> Option<(ChainRef, &'a Cycle)> {
        self.chains
            .iter()
            .find(|chain| chain.owns(cycle))
            .and_then(|chain| chain.cycles().node(CycleId::from(cycle)).map(|node| (chain.id(), node)))
    }
}

// ── 모순 ───────────────────────────────────────────────────────────────────

/// 건넨 사건의 사이클 참조·발급 번호가 맞지 않는 이유. 자리는 0 부터 센다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CycleRefProblem {
    /// 자리의 이름이 `M<자리>` 가 아니다 — 빈틈·재배열(`repeated: false`) 또는 중복(`true`). 사건 순서가 서지 않아 발급 번호의
    /// 감소 비교를 하지 않았다. 첫 자리 하나만 돌려준다.
    NameOutOfPlace { position: usize, found: EventName, repeated: bool },
    /// 건넨 기준점 기록이 한 시점 검사(3e)를 통과하지 못했다 — 그대로 담는다.
    Point(Vec<BaselineProblem>),
    /// 건넨 `Chain` 목록으로는 조회가 하나로 정해지지 않는다. 참조 조건을 평가하지 않았다.
    AmbiguousLookup(Vec<LookupProblem>),
    /// 실제 사이클 참조가 있는 사건의 `chain_ref` 가 건넨 목록에 없다.
    UnknownChain { event: EventName, chain: ChainRef },
    /// 가리킨 사이클을 건넨 목록의 어느 체인도 소유하지 않는다.
    UnknownCycle { event: EventName, field: CycleField, cycle: CycleRef },
    /// 가리킨 사이클이 사건의 `chain_ref` 가 아닌 다른 체인에 속한다(T7, §6.6).
    OtherChain { event: EventName, field: CycleField, cycle: CycleRef, chain: ChainRef, owner: ChainRef },
    /// 가리킨 사이클이 열려 있다(T7 「닫힌 사이클」).
    OpenCycle { event: EventName, field: CycleField, cycle: CycleRef },
    /// 가리킨 사이클의 번호가 견주는 발급 번호보다 작지 않다(T7).
    CycleNotBeforeIssuer { event: EventName, field: CycleField, cycle: CycleRef, issuer: u32 },
    /// 사건의 `next_cycle_id` 가 바로 앞 사건보다 작다(T7 「감소하지 않는다」).
    IssuerDecreased { event: EventName, issuer: u32, previous: EventName, previous_issuer: u32 },
    /// `baseline.next_cycle_id` 가 사건의 `next_cycle_id` 보다 작다(T7).
    BaselineIssuerBelowEvent { event: EventName, baseline_issuer: u32, issuer: u32 },
}

impl fmt::Display for CycleRefProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CycleRefProblem::NameOutOfPlace { position, found, repeated } => {
                let place = position + 1;
                match repeated {
                    true => write!(
                        f,
                        "목록의 {place}번째 사건이 앞의 이름 {found} 을 다시 쓴다. 사건 순서를 정할 수 없어 next_cycle_id 의 감소를 비교하지 않았다"
                    ),
                    false => write!(
                        f,
                        "목록의 {place}번째 사건 이름이 {found} 이다. 빈틈없는 순서라면 event:M{place} 이어야 하며, 순서가 서지 않아 next_cycle_id 의 감소를 비교하지 않았다"
                    ),
                }
            }
            CycleRefProblem::Point(found) => write!(
                f,
                "건넨 기준점 기록이 한 시점으로 맞지 않는다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            CycleRefProblem::AmbiguousLookup(found) => write!(
                f,
                "건넨 체인 목록으로 사이클을 하나로 조회할 수 없어 사이클 참조를 검사하지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            CycleRefProblem::UnknownChain { event, chain } => {
                write!(f, "{event} 의 chain_ref {chain} 이 건넨 체인 목록에 없다")
            }
            CycleRefProblem::UnknownCycle { event, field, cycle } => {
                write!(f, "{event} 의 {} {cycle} 을 건넨 체인 목록의 어느 체인도 소유하지 않는다", field.name())
            }
            CycleRefProblem::OtherChain { event, field, cycle, chain, owner } => write!(
                f,
                "{event} 의 {} {cycle} 은 {owner} 의 사이클이다. 사건의 chain_ref {chain} 의 사이클이어야 한다",
                field.name()
            ),
            CycleRefProblem::OpenCycle { event, field, cycle } => {
                write!(f, "{event} 의 {} {cycle} 이 열려 있다. 닫힌 사이클이어야 한다", field.name())
            }
            CycleRefProblem::CycleNotBeforeIssuer { event, field, cycle, issuer } => write!(
                f,
                "{event} 의 {} {cycle} 의 번호가 {} {issuer} 보다 작지 않다",
                field.name(),
                field.issuer_name()
            ),
            CycleRefProblem::IssuerDecreased { event, issuer, previous, previous_issuer } => write!(
                f,
                "{event} 의 next_cycle_id {issuer} 가 앞 사건 {previous} 의 {previous_issuer} 보다 작다. 사건 순서대로 감소하지 않는다"
            ),
            CycleRefProblem::BaselineIssuerBelowEvent { event, baseline_issuer, issuer } => write!(
                f,
                "{event} 의 baseline.next_cycle_id {baseline_issuer} 가 사건의 next_cycle_id {issuer} 보다 작다"
            ),
        }
    }
}

// ── 성공 값 ────────────────────────────────────────────────────────────────

/// 사이클 참조 칸 하나를 이 검사가 어떻게 다뤘는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefReading {
    /// 실제 참조가 적혀 있어 존재·소속·닫힘·발급 번호 관계를 검사했다(견줄 발급 번호가 적혀 있을 때 그 비교까지).
    Checked(CycleRef),
    /// 값이 `null` 로 적혀 있다 — **판단하지 않았다.** 거절하지 않았지만 그 종류의 사건에서 유효하다고 승인한 것도 아니다.
    NullNotJudged,
    /// 칸이 없다 — **판단하지 않았다.** 기준점이 없는 사건의 `baseline` 칸이 없는 것이 정상인지는 3e 가 본다.
    UnwrittenNotJudged,
}

impl RefReading {
    fn of(slot: BaselineSlot<Option<CycleRef>>) -> RefReading {
        match slot {
            BaselineSlot::Written(Some(cycle)) => RefReading::Checked(cycle),
            BaselineSlot::Written(None) => RefReading::NullNotJudged,
            BaselineSlot::Unwritten => RefReading::UnwrittenNotJudged,
        }
    }

    fn is_checked(self) -> bool {
        matches!(self, RefReading::Checked(_))
    }
}

/// 사건 하나의 두 칸을 어떻게 다뤘는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventRefs {
    event: EventName,
    chain: ChainRef,
    after: RefReading,
    baseline_after: RefReading,
}

impl EventRefs {
    pub(crate) fn event(&self) -> EventName {
        self.event
    }

    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 사건의 `after_cycle_ref`.
    pub(crate) fn after(&self) -> RefReading {
        self.after
    }

    /// `baseline.after_cycle_ref`.
    pub(crate) fn baseline_after(&self) -> RefReading {
        self.baseline_after
    }

    /// 실제 사이클 참조가 있어 `chain_ref` 의 실재를 조회했는가.
    pub(crate) fn chain_looked_up(&self) -> bool {
        self.after.is_checked() || self.baseline_after.is_checked()
    }
}

/// 건넨 사건의 **실제 사이클 참조**가 건넨 체인 목록에서 존재·소속·닫힘을 지키고, 발급 번호 관계(T7 의 네 비교)가 맞으며, 3e 의
/// 한 시점 검사와 이름 자리 규칙을 통과했다 — **그것뿐이다.** 경계 전체가 유효하다는 뜻이 아니다. 참조 부재는 판단하지 않았고
/// ([`CycleRefsAgree::not_judged`]), 보이지 않은 것은 [`CYCLE_REFS_DO_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`check_cycle_refs`]
/// 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CycleRefsAgree {
    events: Vec<EventRefs>,
    baselines: BaselinesAgree,
}

impl CycleRefsAgree {
    /// 사건 순서대로 두 칸의 처리.
    pub(crate) fn events(&self) -> &[EventRefs] {
        &self.events
    }

    /// 3e 의 성공 값 그대로.
    pub(crate) fn baselines(&self) -> &BaselinesAgree {
        &self.baselines
    }

    /// 판단하지 않은 칸 — `null` 또는 칸 없음. 사건 순서, 사건 안에서는 `after_cycle_ref` 다음 `baseline.after_cycle_ref`.
    pub(crate) fn not_judged(&self) -> Vec<(EventName, CycleField, RefReading)> {
        self.events
            .iter()
            .flat_map(|refs| {
                [(CycleField::AfterCycleRef, refs.after), (CycleField::BaselineAfterCycleRef, refs.baseline_after)]
                    .into_iter()
                    .filter(|(_, reading)| !reading.is_checked())
                    .map(move |(field, reading)| (refs.event, field, reading))
            })
            .collect()
    }

    /// 실제 사이클 참조가 하나도 없어 `chain_ref` 의 실재를 조회하지 않은 사건.
    pub(crate) fn chains_not_looked_up(&self) -> Vec<EventName> {
        self.events.iter().filter(|refs| !refs.chain_looked_up()).map(|refs| refs.event).collect()
    }
}

// ── 검사 ───────────────────────────────────────────────────────────────────

/// 실제 참조 하나의 존재·소속·닫힘과, 견줄 발급 번호가 있으면 번호 비교.
fn check_reference(
    lookup: &ChainLookup<'_>,
    facts: &EventCycleFacts,
    field: CycleField,
    cycle: CycleRef,
    issuer: Option<u32>,
    problems: &mut Vec<CycleRefProblem>,
) {
    let event = facts.event();
    match lookup.owner(cycle) {
        None => problems.push(CycleRefProblem::UnknownCycle { event, field, cycle }),
        Some((owner, node)) => {
            // 체인 이름이 목록에 없으면 그 사실은 사건마다 한 번 따로 돌려준다. 여기서는 있는 체인과 다른 경우만 본다.
            if owner != facts.chain && lookup.has_chain(facts.chain) {
                problems.push(CycleRefProblem::OtherChain { event, field, cycle, chain: facts.chain, owner });
            }
            if !node.is_closed() {
                problems.push(CycleRefProblem::OpenCycle { event, field, cycle });
            }
        }
    }
    if let Some(issuer) = issuer.filter(|issuer| cycle.number() >= *issuer) {
        problems.push(CycleRefProblem::CycleNotBeforeIssuer { event, field, cycle, issuer });
    }
}

/// 사건마다 건넨 사이클 참조·발급 번호를 건넨 체인 목록과 견준다. **순수 함수**다.
///
/// 3e 의 [`check_baselines`] 와 이름 자리 규칙을 그대로 부르고 그 실패를 다른 갈래로 담는다. 이름 자리가 서지 않으면 발급 번호의
/// 감소를 비교하지 않는다. 체인 목록의 조회가 모호하면 사이클 참조 조건을 평가하지 않는다. 그 밖의 조건(사건별 발급 번호 비교)은
/// 서로 독립이라 함께 평가하고, 찾은 문제를 모두 돌려준다. 입력·체인·등록부를 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_cycle_refs(
    events: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<CycleRefsAgree, Vec<CycleRefProblem>> {
    let mut problems = Vec::new();
    let baselines: Vec<BaselineFacts> = events.iter().map(|facts| facts.baseline.clone()).collect();

    let ordered = match first_name_out_of_place(&baselines) {
        Some((position, found, repeated)) => {
            problems.push(CycleRefProblem::NameOutOfPlace { position, found, repeated });
            false
        }
        None => true,
    };
    let point = check_baselines(&baselines, registry);
    if let Err(found) = &point {
        problems.push(CycleRefProblem::Point(found.clone()));
    }
    let lookup = match ChainLookup::new(chains) {
        Ok(lookup) => Some(lookup),
        Err(found) => {
            problems.push(CycleRefProblem::AmbiguousLookup(found));
            None
        }
    };

    if ordered {
        for pair in events.windows(2) {
            let (previous, later) = (&pair[0], &pair[1]);
            if later.issuer < previous.issuer {
                problems.push(CycleRefProblem::IssuerDecreased {
                    event: later.event(),
                    issuer: later.issuer,
                    previous: previous.event(),
                    previous_issuer: previous.issuer,
                });
            }
        }
    }

    let mut readings = Vec::with_capacity(events.len());
    for facts in events {
        let event = facts.event();
        let baseline_issuer = match facts.baseline.issuer() {
            BaselineSlot::Written(issuer) => Some(issuer),
            BaselineSlot::Unwritten => None,
        };
        if let Some(baseline_issuer) = baseline_issuer.filter(|written| *written < facts.issuer) {
            problems.push(CycleRefProblem::BaselineIssuerBelowEvent { event, baseline_issuer, issuer: facts.issuer });
        }

        let after = RefReading::of(facts.boundary);
        let baseline_after = RefReading::of(facts.baseline.boundary());
        if let Some(lookup) = &lookup {
            if (after.is_checked() || baseline_after.is_checked()) && !lookup.has_chain(facts.chain) {
                problems.push(CycleRefProblem::UnknownChain { event, chain: facts.chain });
            }
            if let RefReading::Checked(cycle) = after {
                check_reference(lookup, facts, CycleField::AfterCycleRef, cycle, Some(facts.issuer), &mut problems);
            }
            if let RefReading::Checked(cycle) = baseline_after {
                // 발급기 칸이 없으면 견줄 값이 없다 — 그 부분 기록은 3e 가 거절한다.
                check_reference(lookup, facts, CycleField::BaselineAfterCycleRef, cycle, baseline_issuer, &mut problems);
            }
        }
        readings.push(EventRefs { event, chain: facts.chain, after, baseline_after });
    }

    match (problems.is_empty(), point) {
        (true, Ok(baselines)) => Ok(CycleRefsAgree { events: readings, baselines }),
        _ => Err(problems),
    }
}
