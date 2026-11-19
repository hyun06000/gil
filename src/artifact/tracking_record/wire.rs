//! 정책 기록 칸의 **저장 꼴** — 후속 저장 구현이 `tracking` 블록 안에서 쓸 자료형과 거절 규칙.
//!
//! 칸 이름과 뜻은 계약 §5.1 그대로다(`id`, `engine`, `rules: {algorithm, digest}`, `proposal_digest`,
//! `policy_file.sync_base: {state, digest}`). 이 자료형은 **`state.yaml` 의 읽기·쓰기 경로에 연결되지
//! 않았다.** 형식 10/11 을 지원한다는 선언이 아니며, 이것으로 쓰는 독립 파일도 없다. 직렬화가
//! 성공했다는 것은 이 칸들의 꼴이 맞는다는 뜻이지 프로젝트 기록 전체가 유효하다는 뜻이 아니다.
//!
//! 모르는 칸은 거절한다(`deny_unknown_fields`). 선택 칸은 `sync_base.digest` 하나이며, 그 칸이
//! **없음**과 **명시적 `null`** 을 구분해 읽는다 — `null` 이 「없음」으로 읽혀 꼴 검사를 지나가지 않게.

use serde::{Deserialize, Serialize};

use super::{LedgerShapeError, PolicyLedger, PolicyName, PolicyRecord, ProposalDigest, SyncBase};
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{DigestAlgorithm, sha256_from_hex};

/// 승인 정책 원장의 한 판.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredPolicy {
    /// **제 이름은 bare** — `P1`.
    pub(crate) id: String,
    pub(crate) engine: u32,
    pub(crate) rules: StoredRules,
    pub(crate) proposal_digest: String,
}

/// 승인 원문 객체의 내용 주소.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredRules {
    pub(crate) algorithm: String,
    /// 소문자 canonical 16진수.
    pub(crate) digest: String,
}

/// `policy_file` 자리 — 승인·적용과 별개인 동기화 기준 하나.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredPolicyFile {
    pub(crate) sync_base: StoredSyncBase,
}

/// `sync_base: {state: present | absent | unconfirmed, digest?}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredSyncBase {
    pub(crate) state: String,
    /// 바깥 `Option` 은 칸의 있음, 안쪽은 값이다. 명시적 `null` 은 「있는 칸」(`Some(None)`)이다.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    pub(crate) digest: Option<Option<String>>,
}

/// 있는 칸은 값이 `null` 이어도 `Some(_)` 이 된다. 없는 칸만 `#[serde(default)]` 의 `None` 이다.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

// ── 원장 ↔ 저장 꼴 ─────────────────────────────────────────────────────────

impl StoredPolicy {
    pub(crate) fn of(record: &PolicyRecord) -> StoredPolicy {
        StoredPolicy {
            id: record.name().id(),
            engine: record.engine().number(),
            rules: StoredRules {
                algorithm: record.rules().algorithm().as_str().to_string(),
                digest: record.rules().hex(),
            },
            proposal_digest: record.proposal().hex(),
        }
    }

    /// 한 판의 칸을 읽는다. 이름의 순서·발급기는 [`PolicyLedger::from_records`] 가 본다.
    fn into_record(self, position: usize) -> Result<PolicyRecord, LedgerShapeError> {
        let name = PolicyName::parse_bare(&self.id).map_err(|source| LedgerShapeError::BadName {
            position,
            value: self.id.clone(),
            source,
        })?;
        let shown = name.id();
        let engine = EngineVersion::from_number(self.engine).ok_or_else(|| LedgerShapeError::UnknownEngine {
            name: shown.clone(),
            found: self.engine,
        })?;
        if self.rules.algorithm != DigestAlgorithm::Sha256.as_str() {
            return Err(LedgerShapeError::UnknownDigestAlgorithm {
                name: shown,
                found: self.rules.algorithm,
            });
        }
        let rules = sha256_from_hex(&self.rules.digest).ok_or_else(|| LedgerShapeError::DigestNotCanonical {
            name: shown.clone(),
            field: "rules.digest",
            value: self.rules.digest.clone(),
        })?;
        let proposal = ProposalDigest::parse(&self.proposal_digest).ok_or_else(|| {
            LedgerShapeError::DigestNotCanonical {
                name: shown.clone(),
                field: "proposal_digest",
                value: self.proposal_digest.clone(),
            }
        })?;
        Ok(PolicyRecord::new(name, engine, rules, proposal))
    }
}

impl PolicyLedger {
    /// 저장 꼴의 발급기와 판 목록에서 원장을 세운다 — 칸의 꼴, 그다음 이름 순서와 발급기.
    pub(crate) fn from_stored(
        next_policy_id: u32,
        policies: Vec<StoredPolicy>,
    ) -> Result<PolicyLedger, LedgerShapeError> {
        let records = policies
            .into_iter()
            .enumerate()
            .map(|(position, stored)| stored.into_record(position))
            .collect::<Result<Vec<_>, _>>()?;
        PolicyLedger::from_records(next_policy_id, records)
    }

    /// 저장 꼴 — `(next_policy_id, policies)`.
    pub(crate) fn to_stored(&self) -> (u32, Vec<StoredPolicy>) {
        (self.next_policy_id(), self.policies().iter().map(StoredPolicy::of).collect())
    }
}

// ── 동기화 기준 ↔ 저장 꼴 ──────────────────────────────────────────────────

/// `sync_base` 의 꼴이 맞지 않는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncBaseShapeError {
    /// `present`·`absent`·`unconfirmed` 이 아니다.
    UnknownState { found: String },
    /// `present` 인데 지문이 없거나 `null` 이다.
    MissingDigest,
    /// `absent`·`unconfirmed` 인데 `digest` 칸이 있다(값이 `null` 이어도).
    UnexpectedDigest { state: &'static str },
    /// 지문 글자가 소문자 canonical 16진수 64자가 아니다.
    DigestNotCanonical { value: String },
}

impl std::fmt::Display for SyncBaseShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncBaseShapeError::UnknownState { found } => {
                write!(f, "sync_base.state {found:?} 는 present·absent·unconfirmed 가 아니다")
            }
            SyncBaseShapeError::MissingDigest => f.write_str("sync_base 가 present 인데 digest 가 없다"),
            SyncBaseShapeError::UnexpectedDigest { state } => {
                write!(f, "sync_base 가 {state} 인데 digest 칸이 있다")
            }
            SyncBaseShapeError::DigestNotCanonical { value } => {
                write!(f, "sync_base.digest {value:?} 는 소문자 16진수 64자가 아니다")
            }
        }
    }
}

impl std::error::Error for SyncBaseShapeError {}

impl StoredSyncBase {
    pub(crate) fn of(base: &SyncBase) -> StoredSyncBase {
        let (state, digest) = match base {
            SyncBase::Present(digest) => ("present", Some(Some(digest.hex()))),
            SyncBase::Absent => ("absent", None),
            SyncBase::Unconfirmed => ("unconfirmed", None),
        };
        StoredSyncBase { state: state.to_string(), digest }
    }

    pub(crate) fn into_sync_base(self) -> Result<SyncBase, SyncBaseShapeError> {
        let bare = |state: &'static str, base: SyncBase| match self.digest {
            None => Ok(base),
            Some(_) => Err(SyncBaseShapeError::UnexpectedDigest { state }),
        };
        match self.state.as_str() {
            "present" => {
                let Some(Some(text)) = self.digest.clone() else {
                    return Err(SyncBaseShapeError::MissingDigest);
                };
                sha256_from_hex(&text)
                    .map(SyncBase::Present)
                    .ok_or(SyncBaseShapeError::DigestNotCanonical { value: text })
            }
            "absent" => bare("absent", SyncBase::Absent),
            "unconfirmed" => bare("unconfirmed", SyncBase::Unconfirmed),
            _ => Err(SyncBaseShapeError::UnknownState { found: self.state.clone() }),
        }
    }
}
