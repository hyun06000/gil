//! 정책 기록 자료형과 **부분** 검증 — 구현 순서 3번의 첫 조각(이하 「3a」).
//!
//! 「3a」는 이 PR 의 편의상 이름이다. **구현 순서 3번 전체의 완료가 아니다.** 3번에는 형식 10/11 의
//! 제품 저장, 관리 사건, 원문 객체 쓰기·파일 동기화 실행, 명시적 전환(§12 O-1 결정 필요)이 남아 있다.
//!
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1·§5.2·§5.7 이고,
//! 정책 정체성은 2번 독립 정책 엔진([`super::gilignore`])의 `engine` + `rules_digest` 를 그대로 쓴다.
//!
//! ```text
//! StoredPolicy ─from_stored─▶ PolicyLedger            T1 의 정책 이름 부분, T2 의 엔진·주소 구조
//!                                  │
//!     명시적 원문 / 읽기 전용 조회 ─┤ check_policy_sources ─▶ PolicySourcesMatch   T2 의 원문 실재·주소 일치·해석
//!                                  │
//!     앞선 원장 ───────────────────┘ check_append_only     기존 판의 이름·내용·순서 보존(순수 비교)
//!
//! 호출자가 명시한 절대 경로의 루트 ─read_root_policy_file─▶ DiskReading   루트 `.gilignore` 한 파일의 읽기 전용 실제 관측
//!                                          (policy_file_read — 잠금·저장·판정 없음, 승인·쓰기 허가 아님)
//!
//! 적용 원문 주소 + SyncBase + DiskReading ─classify_policy_file─▶ PolicyFileReading   §5.2 판정표
//!                                      └─ PolicyFileSync::begin/advance ─▶ 다음 요청 또는 멈춤  §5.2 쓰기 순서
//!
//! PolicyLedger + 뽑아 건넨 사건 연결 ─check_policy_links─▶ PolicyLinksAgree   T1 사건 이름, T3, T4 (event_links, 「3c」)
//!
//! 두 시점의 (원장, 사건 연결) ─check_policy_history─▶ HistoryKept   3c 두 번 + check_append_only +
//!                                                                   사건 정책 연결 칸의 보존 (link_history, 「3d」)
//!
//! 뽑아 건넨 기준점·시도 칸 + SnapshotRegistry ─check_baselines─▶ BaselinesAgree   T6 의 한 시점 부분 (baseline_record, 「3e」)
//!
//! 두 시점의 (기준점·시도 칸, 등록부) ─check_baseline_history─▶ BaselineHistoryKept   3e 두 번 + 입력 대응 +
//!                                                                   기준점 칸·시도 결과 순서의 보존 (baseline_history, 「3f」)
//!
//! 뽑아 건넨 기준점·시도 칸 + 등록부 ─classify_baseline_positions─▶ BaselinePositions   목록상 보관 대기·대체 구분
//! 두 시점의 (기준점·시도 칸, 등록부) ─check_replaced_baselines─▶ ReplacedBaselinesKept   3f 그대로 + 변경 전부터 대체된
//!                                                                   사건의 기준점 추가 거절 (baseline_position, 「3g」)
//!
//! 뽑아 건넨 사건의 사이클 칸(3e 입력 + chain_ref·after_cycle_ref·next_cycle_id) + &[Chain] + 등록부
//!     ─check_cycle_refs─▶ CycleRefsAgree   3e 그대로 + 이름 자리 + 실제 사이클 참조의 존재·소속·닫힘과
//!                                          발급 번호 관계 — T7 의 일부 (event_cycles, 「3h」)
//!
//! 두 시점의 (사건의 사이클 칸, &[Chain], 등록부) ─check_event_cycle_history─▶ EventCycleHistoryKept   3h 두 번 + 3g 그대로 +
//!                                          앞선 사건의 chain_ref·after_cycle_ref·next_cycle_id 보존 (event_cycle_history, 「3i」)
//!
//! 한 시점의 (PolicyLedger, EventPolicyLinks) + (사건의 사이클 칸, &[Chain], 등록부) ─check_event_inputs─▶ EventInputsAgree
//!                                          3c 그대로 + 3h 그대로 + 두 입력의 사건 수·자리별 이름·같은 사건의 종류 대응
//!                                          (event_inputs, 「3j」 — 실제 원장 추출·제품 연결 아님)
//! ```
//!
//! # 「형식이 맞다」는 「승인됐고 적용됐다」가 아니다
//!
//! - **승인 여부를 판단하지 않는다.** [`ProposalDigest`] 는 해석하지 않는 참조다. 값이 있다는 사실은
//!   모니터 승인을 확인했다는 뜻이 아니다. 제안·모니터 응답 원장과의 연결은 3·4번의 후속 과제다.
//! - **정책을 적용하지 않는다.** 어느 판이 적용 정책인지(T4: 마지막 사건의 after)는 사건 원장 없이
//!   증명할 수 없으므로 그런 함수를 두지 않는다. [`classify_policy_file`] 은 적용 원문 주소를 **호출자가**
//!   건네받는다.
//! - **저장하지 않는다.** `state.yaml` 의 읽기·쓰기 경로, 형식 상수·관문, 객체 창고 쓰기, `.gilignore`
//!   쓰기에 연결하지 않았다. 루트 `.gilignore` 를 실제로 **읽는** 문은 [`read_root_policy_file`] 하나이며 제품 경로가
//!   부르지 않는다 — 읽은 결과는 3a 의 [`DiskReading`] 값일 뿐 판정·승인·쓰기 허가가 아니다. 직렬화 자료형([`wire`])은 후속 저장 구현이 쓸 칸의 꼴과 거절 규칙을
//!   시험하는 데만 쓰며, 독립 파일이나 두 번째 원장 형식을 만들지 않는다.
//! - **세계·사이클을 모른다.** 사건에 대해서는 호출자가 뽑아 건넨 이름·종류·앞뒤 정책만 본다
//!   ([`event_links`], 「3c」 — T1 의 사건 이름 부분, T3, T4 를 **그 입력 안에서만**). 기준점·보관 시도는 뽑아 건넨
//!   칸과 기존 등록부의 이름 조회로 T6 의 **한 시점 부분**만 보고([`baseline_record`], 「3e」), 건넨 두 시점 사이에서
//!   기준점 칸과 시도 **결과 순서**가 보존됐는지 본다([`baseline_history`], 「3f」). 건넨 목록 안의 보관 대기·대체
//!   위치를 유도하고 변경 전부터 대체된 사건의 기준점 추가를 거절한다([`baseline_position`], 「3g」 — T5 의 일부).
//!   건넨 실제 사이클 참조는 호출자가 건넨 `Chain` 목록의 읽기 전용 조회로 존재·소속·닫힘을, 발급 번호는 T7 의 비교를 본다
//!   ([`event_cycles`], 「3h」 — T7 의 일부. 참조 부재의 종류별 의미는 판단하지 않는다). 건넨 두 시점 사이에서 앞선 사건의
//!   승인 당시 세 칸(`chain_ref`·`after_cycle_ref`·`next_cycle_id`)이 그대로인지는 3h 를 각 시점에 그대로 부르고 본다
//!   ([`event_cycle_history`], 「3i」 — 사건 기록 전체의 보존이 아니다). 한 시점에 따로 건넨 3c 입력과 3h 입력이 같은 사건 목록을
//!   나타내는지(사건 수·자리별 이름·같은 사건의 종류)는 두 검사를 그대로 부르고 본다([`event_inputs`], 「3j」 — 입력이 실제 원장에서
//!   빠짐없이 뽑혔는지는 보지 않는다).
//!   T5·T6·T7 의 나머지와 T8–T13 은 이 모듈이 검사하지 않는다([`UNCHECKED_CONDITIONS`]). 현재 세계·보관 대기·복원은 3·5·6번이 공유하는 §6 규칙의 몫이다.
//!
//! 이 모듈의 시험 자료는 모두 **합성 자료**다. 모니터가 실제로 승인한 기록이 아니다.

use std::fmt;

use super::ContentDigest;
use super::gilignore::EngineVersion;
use crate::refs::RefSyntaxError;

mod baseline_history;
mod baseline_position;
mod baseline_record;
mod event_cycle_history;
mod event_cycles;
mod event_inputs;
mod event_links;
mod link_history;
mod policy_file_read;
mod source;
mod sync;
mod sync_order;
mod wire;

#[cfg(test)]
mod baseline_history_tests;
#[cfg(test)]
mod baseline_position_tests;
#[cfg(test)]
mod baseline_record_tests;
#[cfg(test)]
mod event_cycle_history_tests;
#[cfg(test)]
mod event_cycles_tests;
#[cfg(test)]
mod event_inputs_tests;
#[cfg(test)]
mod event_links_tests;
#[cfg(test)]
mod link_history_tests;
#[cfg(test)]
mod policy_file_read_tests;
#[cfg(test)]
mod sync_order_tests;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use baseline_history::{
    BASELINE_HISTORY_DOES_NOT_SHOW, BaselineChange, BaselineHistoryKept, BaselineHistoryProblem, BaselineUpdate,
    MatchProblem, Side, check_baseline_history,
};
#[allow(unused_imports)]
pub(crate) use baseline_position::{
    BaselinePosition, BaselinePositions, POSITIONS_DO_NOT_SHOW, PositionProblem, ReplacedBaselineProblem,
    ReplacedBaselinesKept, check_replaced_baselines, classify_baseline_positions,
};
#[allow(unused_imports)]
pub(crate) use baseline_record::{
    AttemptSeen, BASELINES_DO_NOT_SHOW, BaselineFacts, BaselineField, BaselineProblem, BaselineSlot, BaselinesAgree,
    check_baselines,
};
#[allow(unused_imports)]
pub(crate) use event_cycle_history::{
    APPROVAL_HISTORY_DOES_NOT_SHOW, ApprovalChange, ApprovalField, EventCycleHistoryKept, EventCycleHistoryProblem,
    check_event_cycle_history,
};
#[allow(unused_imports)]
pub(crate) use event_cycles::{
    CYCLE_REFS_DO_NOT_SHOW, CycleField, CycleRefProblem, CycleRefsAgree, EventCycleFacts, EventRefs, LookupProblem,
    RefReading, check_cycle_refs,
};
#[allow(unused_imports)]
pub(crate) use event_inputs::{
    EVENT_INPUTS_DO_NOT_SHOW, EventInputsAgree, EventInputsProblem, InputMismatch, InputSide, check_event_inputs,
};
#[allow(unused_imports)]
pub(crate) use event_links::{
    EventKind, EventName, EventNamesError, EventPolicyLink, EventPolicyLinks, LINKS_DO_NOT_SHOW, LinkField,
    LinkProblem, PolicyLinksAgree, check_policy_links,
};
#[allow(unused_imports)]
pub(crate) use link_history::{
    HISTORY_DOES_NOT_SHOW, HistoryKept, HistoryProblem, LinkAppendViolation, LinkSlot, check_link_append_only,
    check_policy_history,
};
#[allow(unused_imports)]
pub(crate) use policy_file_read::read_root_policy_file;
#[allow(unused_imports)]
pub(crate) use source::{
    Condition, PolicySourceLookup, PolicySourcesMatch, ProvidedSources, SourceLookup,
    SourceProblem, SourceProblemKind, check_policy_sources,
};
#[allow(unused_imports)]
pub(crate) use sync::{DiskReading, PolicyFileReading, SyncBase, classify_policy_file};
#[allow(unused_imports)]
pub(crate) use sync_order::{
    BaseUndecided, CALLER_DUTIES, HOLD_CALLER_DUTIES, NewPolicyBase, PolicyFileSync, ReplaceResult, Reread,
    StoreResult, SyncOutOfOrder, SyncReport, SyncRequest, SyncStep, SyncStop, ThreeWayDifference, Unsettled,
    base_for_new_policy,
};
#[allow(unused_imports)]
pub(crate) use wire::{StoredPolicy, StoredPolicyFile, StoredRules, StoredSyncBase, SyncBaseShapeError};

/// 이 모듈이 **검사하지 않는** 공유 계약 §5.7 의 조건.
///
/// [`PolicySourcesMatch`] 를 손에 넣었다고 이 조건들이 성립하는 것이 아니다. 사건·세계·사이클·보관
/// 중단 표식·파일 쓰기 순서와의 관계는 후속 저장 구현이 같은 계약으로 검증한다.
pub(crate) const UNCHECKED_CONDITIONS: [&str; 12] = [
    "T1 사건 이름(M1…)과 사건 발급기 — event_links 가 건넨 입력 안에서만 검사한다. 실제 원장 전체에서 뽑혔는지는 모른다. 한 시점에 따로 건넨 정책 연결 입력과 사이클 입력이 같은 사건 목록(사건 수·자리별 이름·같은 사건의 종류)인지만 event_inputs 가 본다. 「추가만 한다」는 link_history 가 건넨 두 시점의 정책 판과 사건의 정책 연결 칸에만, baseline_history 가 건넨 두 시점의 기준점 입력에서 앞선 사건이 같은 자리·이름·종류로 남았는지와 기준점 칸·시도 결과 순서에만, event_cycle_history 가 건넨 두 시점에서 앞선 사건의 chain_ref·after_cycle_ref·next_cycle_id 에만 검사한다",
    "T3 사건의 정책 사슬 — event_links 가 건넨 입력 안에서만 검사한다",
    "T4 각 정책이 정확히 한 start·scope_change 의 after, 마지막 정책 = 마지막 사건의 after — event_links 가 건넨 입력 안에서만 검사한다. 적용 여부는 판단하지 않는다",
    "T5 기준점이 없는 사건의 위치 — baseline_position 이 건넨 한 시점 목록 안에서 보관 대기·대체를 구분하고, 건넨 두 시점에서 변경 전부터 목록상 대체된 사건의 기준점 추가만 거절한다. 실제 원장의 끝 잘림, 두 시점 사이의 실행 순서, 대체된 사건의 실패 기록 추가의 의미, 보관 실행 권한은 보지 않는다",
    "T6 archived 시도와 기준점 — baseline_record 가 뽑아 건넨 한 시점의 기준점 칸·시도 결과 순서와 등록부의 이름 조회만 검사하고, baseline_history 가 건넨 두 시점 사이의 기준점 칸과 시도 결과 순서의 보존만 검사한다. 실패 사유·원본 시도 레코드 전체, 두 시점 사이의 되돌림, 등록부 매핑의 보존, 기준점 객체의 실재·내용은 보지 않는다",
    "T7 사건의 사이클 경계와 발급기 값 — event_cycles 가 건넨 사건의 실제 사이클 참조(after_cycle_ref·baseline.after_cycle_ref)만 건넨 Chain 목록의 읽기 전용 조회로 존재·체인 소속·지금 기록의 닫힘을 보고, 번호 < 견주는 발급 번호, next_cycle_id 의 비감소(이름 자리가 섰을 때), baseline.next_cycle_id ≥ next_cycle_id 를 검사한다. null·칸 없음의 종류별 의미, 저장 당시의 닫힘, 입력·체인 목록의 완전한 추출, review_cycle_ref 는 보지 않는다. 건넨 두 시점 사이에서 앞선 사건의 chain_ref·after_cycle_ref·next_cycle_id 가 그대로인지는 event_cycle_history 가 본다 — 두 관측 사이의 되돌림과 null·칸 없음의 허용은 보지 않는다",
    "T8 world_before_ref 와 마지막 확정 세계",
    "T9 실험 Entry 와 보관 대기",
    "T10 .gitignore 비유래 — 원문 출처는 기록만으로 증명할 수 없다",
    "T11 사건이 약속·판·보고서·단계를 만들지 않음",
    "T12 보관 중단 표식의 전이 차단",
    "T13 정책 파일 쓰기 순서 — 판정표와 순서 제어 로직(sync_order)만 있고, 실제 잠금·쓰기·저장 실행과 그 보고의 진위는 없다",
];

// ── 정책 판의 이름 ─────────────────────────────────────────────────────────

/// 승인 정책 원장의 **판 이름** — 기록 자신의 칸에는 bare `P1`, 남이 가리킬 때는 `policy:P1`.
///
/// 판의 이름은 정책 내용의 정체성([`super::PolicyIdentity`])이 아니다. 같은 원문을 다시 승인하면
/// 같은 내용 주소를 가진 **다른 판**이 된다(계약 §5.1). 수의 규칙(선행 0 없음, 0 없음)은
/// [`crate::refs`] 의 것을 그대로 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PolicyName(u32);

impl PolicyName {
    /// 기록에 적히는 종류의 이름.
    pub(crate) const KIND: &'static str = "policy";
    /// 판 이름의 글자.
    pub(crate) const LETTER: &'static str = "P";

    /// 수 하나로 만든다. `P0` 은 없다.
    pub(crate) fn new(number: u32) -> Option<PolicyName> {
        (number != 0).then_some(PolicyName(number))
    }

    /// 기록 자신의 칸에 적힌 bare 이름(`P1`)을 읽는다.
    pub(crate) fn parse_bare(text: &str) -> Result<PolicyName, RefSyntaxError> {
        if text.chars().any(char::is_whitespace) {
            return Err(RefSyntaxError::Whitespace { text: text.to_string() });
        }
        crate::refs::id_of(text, Self::LETTER, false).map(PolicyName)
    }

    pub(crate) fn number(self) -> u32 {
        self.0
    }

    /// bare 이름 — `P1`.
    pub(crate) fn id(self) -> String {
        format!("{}{}", Self::LETTER, self.0)
    }
}

impl fmt::Display for PolicyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}{}", Self::KIND, Self::LETTER, self.0)
    }
}

// ── 제안 판의 참조 ─────────────────────────────────────────────────────────

/// 모니터가 보여 준 제안 판을 가리키는 `proposal_digest` — **해석하지 않는 참조**다.
///
/// 이 단계에서 확인하는 것은 꼴(sha256 소문자 16진수 64자)뿐이다. 이 값으로 모니터 승인 응답을 찾거나
/// 승인 사실을 확인하는 기능은 없다 — 제안·모니터 응답 원장이 아직 없기 때문이다(계약 §10 의 3·4번).
/// 그래서 이 타입에는 승인 여부를 묻는 메서드가 없다.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProposalDigest(ContentDigest);

impl ProposalDigest {
    /// 저장 글자에서 읽는다. canonical 이 아니면 `None`.
    pub(crate) fn parse(text: &str) -> Option<ProposalDigest> {
        super::sha256_from_hex(text).map(ProposalDigest)
    }

    /// 저장에 적는 글자.
    pub(crate) fn hex(&self) -> String {
        self.0.hex()
    }
}

// ── 정책 기록 하나 ─────────────────────────────────────────────────────────

/// 승인 정책 원장의 한 판 — 계약 §5.1 에 정해진 네 칸만 지닌다.
///
/// 이 값이 있다는 것은 「그런 기록이 적혀 있다」는 뜻일 뿐이다. 원문이 실재하고 주소와 맞는지는
/// [`check_policy_sources`] 가, 모니터 승인과 적용 사건은 후속 계층이 확인한다. 추천 출처·`.gitignore`
/// 참고 여부 같은 정보는 제안 원장의 몫이라 여기 칸이 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyRecord {
    name: PolicyName,
    engine: EngineVersion,
    rules: ContentDigest,
    proposal: ProposalDigest,
}

impl PolicyRecord {
    pub(crate) fn new(
        name: PolicyName,
        engine: EngineVersion,
        rules: ContentDigest,
        proposal: ProposalDigest,
    ) -> PolicyRecord {
        PolicyRecord { name, engine, rules, proposal }
    }

    pub(crate) fn name(&self) -> PolicyName {
        self.name
    }

    /// 규칙 해석 의미의 버전 — 알려진 버전만 이 타입에 들어온다(T2).
    pub(crate) fn engine(&self) -> EngineVersion {
        self.engine
    }

    /// 승인 원문 객체의 내용 주소 — #174 의 `PolicyIdentity::rules_digest` 와 같은 값이어야 한다.
    pub(crate) fn rules(&self) -> &ContentDigest {
        &self.rules
    }

    /// 해석하지 않는 제안 판 참조.
    pub(crate) fn proposal(&self) -> &ProposalDigest {
        &self.proposal
    }
}

// ── 정책 원장의 꼴 ─────────────────────────────────────────────────────────

/// 승인 정책 원장의 **이름 꼴이 맞는** 판 목록과 발급기.
///
/// 만들어졌다면 T1 의 정책 부분이 참이다: 이름이 `P1` 부터 빈틈없이 오름차순이고, 발급기는 마지막
/// 이름 다음이다. 발급기를 「판 수 + 1」과 같게 요구하는 것은 [`super::SnapshotRegistry`] 와
/// `next_will_id` 의 기존 규칙과 같다 — 이름은 기록을 붙일 때만 발급되므로 빈 번호가 생길 수 없다.
/// T1 의 사건 부분, 원문 실재(T2 의 나머지), T3 이후는 여기서 성립하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyLedger {
    next_policy_id: u32,
    policies: Vec<PolicyRecord>,
}

impl PolicyLedger {
    /// 아직 정책을 채택하지 않은 원장 — `next_policy_id: 1`, `policies: []`.
    pub(crate) fn empty() -> PolicyLedger {
        PolicyLedger { next_policy_id: 1, policies: Vec::new() }
    }

    /// 판 목록과 발급기를 받아 이름 꼴을 검사한다.
    pub(crate) fn from_records(
        next_policy_id: u32,
        policies: Vec<PolicyRecord>,
    ) -> Result<PolicyLedger, LedgerShapeError> {
        for (position, record) in policies.iter().enumerate() {
            let expected = u32::try_from(position)
                .ok()
                .and_then(|at| at.checked_add(1))
                .ok_or(LedgerShapeError::Exhausted)?;
            if record.name.number() != expected {
                let repeated = policies[..position].iter().any(|seen| seen.name == record.name);
                return Err(match repeated {
                    true => LedgerShapeError::DuplicateName { name: record.name, position },
                    false => LedgerShapeError::OutOfSequence {
                        found: record.name,
                        expected: PolicyName(expected),
                        position,
                    },
                });
            }
        }
        let wanted = u32::try_from(policies.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(LedgerShapeError::Exhausted)?;
        if next_policy_id != wanted {
            return Err(LedgerShapeError::WrongIssuer { found: next_policy_id, wanted });
        }
        Ok(PolicyLedger { next_policy_id, policies })
    }

    pub(crate) fn next_policy_id(&self) -> u32 {
        self.next_policy_id
    }

    pub(crate) fn policies(&self) -> &[PolicyRecord] {
        &self.policies
    }
}

/// 원장의 꼴이 맞지 않는 이유. **하나라도 있으면 원장을 세우지 않는다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LedgerShapeError {
    /// 판 이름이 `P<n>` 으로 읽히지 않는다.
    BadName { position: usize, value: String, source: RefSyntaxError },
    /// 같은 이름이 두 번 실렸다. 같은 원문의 다른 판은 이것이 아니다.
    DuplicateName { name: PolicyName, position: usize },
    /// 빈틈이 있거나 순서가 바뀌었다.
    OutOfSequence { found: PolicyName, expected: PolicyName, position: usize },
    /// 발급기가 마지막 이름 다음이 아니다.
    WrongIssuer { found: u32, wanted: u32 },
    /// 이 빌드가 모르는 엔진 버전이다(T2).
    UnknownEngine { name: String, found: u32 },
    /// 원문 주소의 알고리즘이 sha256 이 아니다.
    UnknownDigestAlgorithm { name: String, found: String },
    /// 지문 글자가 소문자 canonical 16진수 64자가 아니다.
    DigestNotCanonical { name: String, field: &'static str, value: String },
    /// 이름을 더 발급할 수 없다. 감싸 돌지 않는다.
    Exhausted,
}

impl fmt::Display for LedgerShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerShapeError::BadName { position, value, source } => {
                write!(f, "정책 원장의 {}번째 판 이름 {value:?} 를 읽을 수 없다: {source}", position + 1)
            }
            LedgerShapeError::DuplicateName { name, position } => {
                write!(f, "정책 원장의 {}번째 판이 앞의 이름 {name} 을 다시 쓴다", position + 1)
            }
            LedgerShapeError::OutOfSequence { found, expected, position } => write!(
                f,
                "정책 원장의 {}번째 판 이름이 {found} 이다. 빈틈없는 순서라면 {expected} 이어야 한다",
                position + 1
            ),
            LedgerShapeError::WrongIssuer { found, wanted } => {
                write!(f, "next_policy_id 가 {found} 이다. 마지막 판 다음인 {wanted} 이어야 한다")
            }
            LedgerShapeError::UnknownEngine { name, found } => {
                write!(f, "판 {name} 의 engine {found} 은 이 빌드가 아는 정책 엔진이 아니다")
            }
            LedgerShapeError::UnknownDigestAlgorithm { name, found } => {
                write!(f, "판 {name} 의 원문 주소 알고리즘 {found:?} 은 sha256 이 아니다")
            }
            LedgerShapeError::DigestNotCanonical { name, field, value } => {
                write!(f, "판 {name} 의 {field} {value:?} 는 소문자 16진수 64자가 아니다")
            }
            LedgerShapeError::Exhausted => f.write_str("정책 판 이름을 더 발급할 수 없다"),
        }
    }
}

impl std::error::Error for LedgerShapeError {}

// ── 추가 전후 비교 ─────────────────────────────────────────────────────────

/// 앞선 원장 뒤에 판이 **덧붙기만 했는가** — 원장을 갱신하는 명령이 아니라 두 값의 순수 비교다.
///
/// 두 값 모두 [`PolicyLedger`] 이므로 이름은 자리에 묶여 있고(`P<n>` 은 n번째 자리) 발급기는 판 수 + 1 이다.
/// 그래서 앞선 판마다 같은 자리의 엔진·원문 주소·제안 참조가 그대로이고 사라진 판이 없으면, 순서·이름·
/// 발급기도 보존된 것이다. 순서를 바꾸면 앞 자리의 내용이 바뀐 것으로 걸린다. 덧붙은 판의 수를 돌려준다.
/// 덧붙은 판이 승인됐는지·적용 사건이 있는지는 모른다.
pub(crate) fn check_append_only(
    before: &PolicyLedger,
    after: &PolicyLedger,
) -> Result<usize, AppendViolation> {
    for (at, earlier) in before.policies.iter().enumerate() {
        let Some(later) = after.policies.get(at) else {
            return Err(AppendViolation::Removed { name: earlier.name });
        };
        let mut fields = Vec::new();
        if later.engine != earlier.engine {
            fields.push("engine");
        }
        if later.rules != earlier.rules {
            fields.push("rules");
        }
        if later.proposal != earlier.proposal {
            fields.push("proposal_digest");
        }
        if !fields.is_empty() {
            return Err(AppendViolation::Changed { name: earlier.name, fields });
        }
    }
    Ok(after.policies.len() - before.policies.len())
}

/// 추가 전후 비교가 찾은 과거 기록의 변경.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppendViolation {
    /// 앞선 판이 뒤 원장에 없다.
    Removed { name: PolicyName },
    /// 앞선 판의 자리에 다른 값이 있다 — 내용 수정과 순서 변경이 여기 걸린다.
    Changed { name: PolicyName, fields: Vec<&'static str> },
}

impl fmt::Display for AppendViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppendViolation::Removed { name } => write!(f, "앞선 판 {name} 이 사라졌다. 원장은 추가만 한다"),
            AppendViolation::Changed { name, fields } => {
                write!(f, "앞선 판 {name} 의 자리가 바뀌었다({}). 원장은 추가만 한다", fields.join(", "))
            }
        }
    }
}

impl std::error::Error for AppendViolation {}
