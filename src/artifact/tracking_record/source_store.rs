//! 호출자가 **명시적으로 건넨** 정책 원문 바이트를 검증하고 **기존 객체 창고에 보관한다** — 구현 순서 3번 안의 작은
//! 후속(편의상 「3m」).
//!
//! 계약 §5.1 은 승인 원문을 「`.gilignore` 하나의 바이트이고, 기존 객체 창고에 내용 주소로 둔다. 상태보다 먼저
//! 확정한다」고 적는다. 3a 의 [`check_policy_sources`](super::check_policy_sources) 는 그 원문을 **읽어** 검사할 뿐이고,
//! 원문을 창고에 넣는 문은 범용 [`ObjectStore::put_blob`] 뿐이었다 — 그것은 아무 바이트나 받고 기대 주소·엔진을 모른다.
//! 이 모듈은 그 사이의 경계 하나다.
//!
//! ```text
//! 호출자가 고른 ObjectStore + 엔진 + 기대 주소 + 메모리의 원문 바이트
//!   ① 원문 바이트의 sha256 = 기대 주소인가          아니면 AddressMismatch — 창고에 아무것도 요청하지 않는다
//!   ② 그 엔진으로 해석되는가(ApprovedPolicy::compile) 아니면 Rejected      — 창고에 아무것도 요청하지 않는다
//!   ③ 같은 바이트를 ObjectStore::put_blob 에 건넨다
//!        창고 오류 → Store(그 ObjectError 그대로)
//!        창고가 기대 주소가 아닌 주소를 돌려줌 → StoredAtOtherAddress
//!   ④ PolicySourceStored — 그 바이트의 blob 객체가 기대 주소에 있음을 창고가 확인했다
//! ```
//!
//! ①·②·③ 은 **같은 불변 슬라이스**를 쓴다. 지문을 잰 바이트, 해석한 바이트, 창고에 흘려 넣은 바이트는 같은 것이다.
//! 주소부터 보는 순서는 3a 의 읽기 검사와 같다 — 주소와 다른 바이트를 해석해 보는 것은 승인 원문이 아닌 것을 읽는 일이다.
//!
//! # 새 저장기·새 형식이 아니다
//!
//! 창고의 배치·쓰기 방식·검증은 [`ObjectStore`] 의 것을 그대로 쓴다: 임시 자리에 흘려 쓰고, 덮어쓰지 않는 hard link 로
//! 건 뒤, 앉은 객체를 다시 읽어 주소와 견준다(이미 같은 주소가 있으면 건드리지 않고 다시 읽어 견준다). 그래서 같은 원문의
//! 재시도는 같은 결과이고 두 번째 객체를 만들지 않는다. 손상된 기존 객체(`Corrupt`), 확정 직후 다시 읽은 내용이 다른 객체
//! (`Corrupt`), hard link 를 못 거는 파일 시스템(`PublishUnsupported`), 읽기·쓰기 실패(`Io`)는 창고의 오류 그대로
//! [`SourceStoreProblem::Store`] 에 담아 돌려준다 — 성공으로 바꾸지 않고, 창고를 고쳐 쓰지 않는다. 창고의 접근 권한 계약
//! (`ObjectStore::at` / `owner_only`)은 호출자가 고른 값 그대로다.
//!
//! # 이 경계가 하지 않는 것
//!
//! [`SOURCE_STORE_DOES_NOT_SHOW`] 에 적었다. 요약하면 성공은 **「그 바이트의 객체가 기대 주소에 보관됐다」뿐**이다. 승인·적용,
//! 정책 판·관리 사건의 생성, `state.yaml`·`sync_base`·작업 파일·`.gilignore` 의 쓰기는 별도 단계이며 이 모듈은 하지 않는다.
//! 루트 `.gilignore`·`.gitignore` 를 읽어 입력을 대신하지 않고, 원문의 줄바꿈·주석·공백·인코딩을 정규화하지 않는다.
//! 제품 경로에 연결하지 않았다.

use std::fmt;

use crate::artifact::gilignore::{ApprovedPolicy, EngineVersion, PolicyIdentity, RulesRejected};
use crate::artifact::{ContentDigest, ObjectError, ObjectStore};

/// 이 경계의 성공이 **보이지 않는** 것 — 성공 값을 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const SOURCE_STORE_DOES_NOT_SHOW: [&str; 9] = [
    "승인 — 건넨 바이트가 모니터가 승인한 원문인지 확인하지 않는다. proposal_digest·제안·모니터 응답 원장을 받지 않는다",
    "적용 — 적용 정책은 원장의 마지막 판이다(§5.2). 이 경계는 정책 판도 관리 사건도 만들지 않고 원장을 받지도 바꾸지도 않는다",
    "상태 저장 — state.yaml·tracking 블록·next_policy_id·sync_base·archive_halt 를 읽거나 쓰지 않는다. 원문 객체를 상태보다 먼저 확정한다는 §5.1 의 순서를 실행하는 것은 연결 계층의 몫이다",
    "파일 동기화 — 디스크의 .gilignore 를 쓰거나 읽지 않는다. 루트 .gilignore·.gitignore·작업 파일을 입력으로 대신하지 않는다",
    "새로 썼는지 — 새 객체를 걸었는지, 같은 주소의 기존 객체를 확인했는지 구분해 돌려주지 않는다. 창고가 둘 다 같은 성공으로 답한다",
    "참조 — 이 호출은 정책 판·관리 사건·Snapshot 에 참조를 추가하지 않으며, 기존 참조의 유무도 검사하지 않는다. 같은 바이트의 객체는 이미 참조되고 있을 수 있다. 미참조 객체는 논리 손상이 아니다(Artifact Model §10.5)",
    "반환 뒤의 불변 — 창고가 확인한 순간 뒤에 창고 밖에서 객체가 바뀌는 것을 막지 않는다. 다음 읽기가 손상으로 드러낸다",
    "알 수 없는 엔진 번호 — 엔진은 이 빌드가 아는 EngineVersion 으로만 받는다. 기록의 번호 읽기와 거절은 원장(PolicyLedger)의 몫이다",
    "잠금·동시성 — 프로젝트 잠금을 잡지 않는다. 같은 주소의 동시 확정은 창고의 덮어쓰지 않는 확정과 재확인에 맡긴다",
];

/// 건넨 원문 바이트의 blob 객체가 기대 주소에 보관됐음을 창고가 확인했다 — **그것뿐이다.**
///
/// 보이지 않은 것은 [`SOURCE_STORE_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`store_policy_source`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicySourceStored {
    identity: PolicyIdentity,
}

impl PolicySourceStored {
    /// 건넨 엔진과 원문 바이트로 정책 엔진이 세운 정체성.
    pub(crate) fn identity(&self) -> &PolicyIdentity {
        &self.identity
    }

    /// 보관된 객체의 주소 — 기대 주소이자 정체성의 `rules_digest` 다.
    pub(crate) fn address(&self) -> &ContentDigest {
        self.identity.rules_digest()
    }
}

/// 보관하지 못한 이유. **어느 것이든 성공이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceStoreProblem {
    /// 건넨 바이트의 지문이 기대 주소와 다르다. 창고에 아무것도 요청하지 않았다.
    AddressMismatch { expected: ContentDigest, found: ContentDigest },
    /// 주소는 맞지만 그 엔진으로 해석되지 않는다. 창고에 아무것도 요청하지 않았다.
    Rejected(RulesRejected),
    /// 창고가 답한 오류 그대로 — 손상·확정 실패·지원하지 않는 파일 시스템·입출력 실패를 구분해 담는다.
    Store(ObjectError),
    /// 창고가 기대 주소가 아닌 주소에 확정했다고 답했다. 같은 바이트이므로 창고 계약이 깨진 경우이며 성공으로 보지 않는다.
    StoredAtOtherAddress { expected: ContentDigest, found: ContentDigest },
}

impl SourceStoreProblem {
    /// 창고에 쓰기를 요청하기 **전에** 거절했는가 — 이때 창고와 디스크에는 이 호출의 흔적이 없다.
    pub(crate) fn refused_before_write(&self) -> bool {
        matches!(self, SourceStoreProblem::AddressMismatch { .. } | SourceStoreProblem::Rejected(_))
    }
}

impl fmt::Display for SourceStoreProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceStoreProblem::AddressMismatch { expected, found } => write!(
                f,
                "건넨 정책 원문의 지문 sha256:{} 이 기대 주소 sha256:{} 와 다르다. 창고에 쓰지 않았다",
                found.hex(),
                expected.hex()
            ),
            SourceStoreProblem::Rejected(rejected) => {
                write!(f, "건넨 정책 원문을 해석할 수 없다. 창고에 쓰지 않았다: {rejected}")
            }
            SourceStoreProblem::Store(err) => write!(f, "정책 원문을 객체 창고에 보관하지 못했다: {err}"),
            SourceStoreProblem::StoredAtOtherAddress { expected, found } => write!(
                f,
                "객체 창고가 정책 원문을 기대 주소 sha256:{} 가 아닌 sha256:{} 에 확정했다고 답했다. 보관 성공으로 보지 않는다",
                expected.hex(),
                found.hex()
            ),
        }
    }
}

impl std::error::Error for SourceStoreProblem {}

/// 건넨 원문 바이트가 기대 주소·엔진과 맞는지 **쓰기 전에** 확인하고, 같은 바이트를 호출자가 고른 창고에 보관한다.
///
/// 순서와 결과의 뜻은 모듈 설명의 ①–④ 그대로다. 같은 원문으로 다시 부르면 같은 성공을 돌려준다.
pub(crate) fn store_policy_source(
    store: &ObjectStore,
    engine: EngineVersion,
    expected: &ContentDigest,
    source: &[u8],
) -> Result<PolicySourceStored, SourceStoreProblem> {
    // ① 주소부터 — 3a 의 읽기 검사와 같은 순서.
    let found = crate::artifact::digest_of(source).expect("메모리의 바이트는 언제나 읽힌다");
    if &found != expected {
        return Err(SourceStoreProblem::AddressMismatch { expected: expected.clone(), found });
    }
    // ② 기존 정책 엔진으로 해석한다. 정체성은 엔진이 같은 바이트로 세운다.
    let identity = ApprovedPolicy::compile(engine, source).map_err(SourceStoreProblem::Rejected)?.identity().clone();
    // ③ 기존 창고에 같은 바이트를 넣는다. 창고의 오류는 바꾸지 않고 담는다.
    let stored = store.put_blob(source).map_err(SourceStoreProblem::Store)?;
    if &stored != identity.rules_digest() {
        return Err(SourceStoreProblem::StoredAtOtherAddress { expected: expected.clone(), found: stored });
    }
    // ④
    Ok(PolicySourceStored { identity })
}
