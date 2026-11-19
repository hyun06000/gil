//! 정책 원문의 실재·주소·해석 검사 — 계약 T2 의 나머지.
//!
//! 원문은 **명시적 입력**([`ProvidedSources`])이나 **읽기 전용 조회**([`ObjectStore`] 의 `read_blob`)로만
//! 받는다. 디스크의 `.gilignore`·`.gitignore` 를 읽지 않으며 원문을 창고에 쓰지 않는다.

use std::collections::BTreeMap;
use std::fmt;

use super::{PolicyLedger, PolicyName};
use crate::artifact::gilignore::{ApprovedPolicy, PolicyIdentity, RulesRejected};
use crate::artifact::{ContentDigest, ObjectError, ObjectStore};

/// 원문 조회 한 번의 결과. **실패와 없음은 다르고, 둘 다 성공이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceLookup {
    /// 그 주소라고 건네진 바이트.
    Found(Vec<u8>),
    /// 그 주소의 원문이 없다.
    Missing,
    /// 조회 자체가 실패했다(읽기 오류·창고 손상 등). 없음으로 바꾸지 않는다.
    Failed { reason: String },
}

/// 내용 주소로 승인 원문 바이트를 **읽기만** 하는 경계.
pub(crate) trait PolicySourceLookup {
    fn rules_source(&self, address: &ContentDigest) -> SourceLookup;
}

/// 호출자가 「이 주소의 원문은 이 바이트다」라고 명시적으로 건넨 묶음.
///
/// 바이트의 실제 지문은 검사가 다시 잰다 — 건넨 쪽의 주장을 믿지 않는다.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProvidedSources(BTreeMap<ContentDigest, Vec<u8>>);

impl ProvidedSources {
    pub(crate) fn new() -> ProvidedSources {
        ProvidedSources::default()
    }

    pub(crate) fn with(mut self, claimed: ContentDigest, bytes: impl Into<Vec<u8>>) -> ProvidedSources {
        self.0.insert(claimed, bytes.into());
        self
    }
}

impl PolicySourceLookup for ProvidedSources {
    fn rules_source(&self, address: &ContentDigest) -> SourceLookup {
        match self.0.get(address) {
            Some(bytes) => SourceLookup::Found(bytes.clone()),
            None => SourceLookup::Missing,
        }
    }
}

/// 객체 창고의 blob 을 **읽기만** 한다. 창고가 내용과 주소를 다시 견주므로 손상은 `Failed` 다.
impl PolicySourceLookup for ObjectStore {
    fn rules_source(&self, address: &ContentDigest) -> SourceLookup {
        let mut bytes = Vec::new();
        match self.read_blob(address, &mut bytes) {
            Ok(()) => SourceLookup::Found(bytes),
            Err(ObjectError::Missing { .. }) => SourceLookup::Missing,
            Err(other) => SourceLookup::Failed { reason: other.to_string() },
        }
    }
}

/// 이 검사가 성립을 보인 계약 조건 — 이것 말고는 아무것도 보이지 않았다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Condition {
    /// T1 가운데 정책 이름·정책 발급기 부분([`PolicyLedger`] 를 세울 때 확인).
    T1PolicyNames,
    /// T2: 원문 객체가 있고 주소와 맞으며, 그 판의 엔진은 아는 버전이고 그 엔진으로 해석된다.
    T2PolicySources,
}

/// 모든 판의 원문이 있고, 주소와 맞고, 지정된 엔진으로 해석됐다 — **그것뿐이다.**
///
/// 승인됐다·적용됐다·원장 전체가 유효하다는 뜻이 아니다. [`super::UNCHECKED_CONDITIONS`] 의 조건은
/// 확인하지 않았다. 이 값을 만드는 문은 [`check_policy_sources`] 하나뿐이다.
#[derive(Debug, Clone)]
pub(crate) struct PolicySourcesMatch {
    identities: Vec<(PolicyName, PolicyIdentity)>,
}

impl PolicySourcesMatch {
    /// 이 결과가 보인 조건.
    pub(crate) const CHECKED: [Condition; 2] = [Condition::T1PolicyNames, Condition::T2PolicySources];

    /// 판마다 원문을 해석해 얻은 #174 정책 정체성.
    pub(crate) fn identity_of(&self, name: PolicyName) -> Option<&PolicyIdentity> {
        self.identities.iter().find(|(at, _)| *at == name).map(|(_, identity)| identity)
    }

    pub(crate) fn len(&self) -> usize {
        self.identities.len()
    }
}

/// 원문 검사가 찾은 문제 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceProblem {
    pub(crate) name: PolicyName,
    pub(crate) kind: SourceProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceProblemKind {
    /// 원문이 제공되지 않았다.
    Missing,
    /// 원문을 조회하지 못했다.
    LookupFailed { reason: String },
    /// 건네진 바이트의 지문이 기록의 주소와 다르다.
    AddressMismatch { found: ContentDigest },
    /// 원문이 주소와 맞지만 그 엔진으로 해석되지 않는다.
    Rejected(RulesRejected),
}

impl fmt::Display for SourceProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name;
        match &self.kind {
            SourceProblemKind::Missing => write!(f, "판 {name} 의 승인 원문이 없다"),
            SourceProblemKind::LookupFailed { reason } => {
                write!(f, "판 {name} 의 승인 원문을 읽지 못했다(없음으로 보지 않는다): {reason}")
            }
            SourceProblemKind::AddressMismatch { found } => {
                write!(f, "판 {name} 의 원문으로 건네진 바이트가 기록의 주소와 다르다 (실제 sha256:{})", found.hex())
            }
            SourceProblemKind::Rejected(rejected) => write!(f, "판 {name} 의 원문을 해석할 수 없다: {rejected}"),
        }
    }
}

/// 모든 판의 원문을 조회해 주소와 해석을 검사한다. 문제가 하나라도 있으면 **모든** 문제를 돌려준다.
///
/// 판마다 원문을 정확히 한 번 조회한다. 같은 원문을 가진 서로 다른 판도 각자 검사한다.
pub(crate) fn check_policy_sources(
    ledger: &PolicyLedger,
    lookup: &impl PolicySourceLookup,
) -> Result<PolicySourcesMatch, Vec<SourceProblem>> {
    let mut identities = Vec::with_capacity(ledger.policies().len());
    let mut problems = Vec::new();
    for record in ledger.policies() {
        let name = record.name();
        let bytes = match lookup.rules_source(record.rules()) {
            SourceLookup::Found(bytes) => bytes,
            SourceLookup::Missing => {
                problems.push(SourceProblem { name, kind: SourceProblemKind::Missing });
                continue;
            }
            SourceLookup::Failed { reason } => {
                problems.push(SourceProblem { name, kind: SourceProblemKind::LookupFailed { reason } });
                continue;
            }
        };
        // 주소부터 본다 — 주소와 다른 바이트를 해석해 보는 것은 승인 원문이 아닌 것을 읽는 일이다.
        let found = crate::artifact::digest_of(bytes.as_slice()).expect("메모리의 바이트는 언제나 읽힌다");
        if &found != record.rules() {
            problems.push(SourceProblem { name, kind: SourceProblemKind::AddressMismatch { found } });
            continue;
        }
        match ApprovedPolicy::compile(record.engine(), &bytes) {
            Ok(policy) => {
                let identity = policy.identity().clone();
                // compile 이 같은 바이트로 정체성을 세우므로 어긋날 수 없다. 어긋나면 엔진 계약이 깨진 것이다.
                assert!(
                    identity.engine() == record.engine() && identity.rules_digest() == record.rules(),
                    "정책 엔진이 건넨 바이트와 다른 정체성을 세웠다"
                );
                identities.push((name, identity));
            }
            Err(rejected) => problems.push(SourceProblem { name, kind: SourceProblemKind::Rejected(rejected) }),
        }
    }
    match problems.is_empty() {
        true => Ok(PolicySourcesMatch { identities }),
        false => Err(problems),
    }
}
