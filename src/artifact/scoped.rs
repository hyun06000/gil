//! **승인 정책 범위의 관측·보관** — 형식 10/11 프로젝트가 쓰는 유일한 세계 관측 경로.
//!
//! ```text
//! ① 정책 엔진의 트리 평가(evaluate_tree)   필요한 디렉터리만 연다. 완전히 제외된 폴더 안은 보지 않는다.
//!                                          연결 파일·특수 항목·중첩 .gil 은 엔진 규칙대로 거절, 하위
//!                                          .gilignore 는 차단 사유(읽지 않는다)
//! ② 추적 대상 일반 파일만 연다             루트 핸들부터 구성 요소마다 openat(O_NOFOLLOW) — 확인 뒤 연결
//!                                          파일로 바뀐 자리를 따라가지 않는다
//! ③ 한 번 흘려 읽으며 지문(+ capture 이면 blob 저장)
//! ④ ①–③ 을 한 번 더 — 저장하지 않고 지문만. 두 결과가 다르면 ChangedDuringObservation
//! ```
//!
//! 기존 whole-tree [`observe`](super::observe)·[`capture`](super::capture) 와 같은 「두 번 훑어 견준다」
//! 규칙을 지키며, 다른 점은 **무엇을 세계로 보는가**를 승인된 `.gilignore` 원문을 컴파일한
//! [`ApprovedPolicy`] 가 정한다는 것 하나다. 루트 `.gilignore` 자신은 엔진의 시스템 보호라 세계에 들어가지
//! 않는다. `.gitignore` 는 읽지 않는다. 제외된 파일은 열지도 담지도 않는다.
//!
//! 이 모듈은 상태를 쓰지 않는다. capture 는 객체 창고에 blob 을 놓을 뿐이며 Snapshot 이름 발급·상태 저장은
//! 부르는 쪽(시작 확정·Verify 닫기)의 일이다. 외부 프로세스의 쓰기를 막는다고 주장하지 않는다.
#![cfg(all(feature = "tracking-policy", unix))]

use std::ffi::CString;
use std::fs::File;

use super::gilignore::{ApprovedPolicy, TreeEvaluation, evaluate_tree};
use super::{ContentDigest, EntryPath, Manifest, ManifestEntry, ObjectStore, ObserveError, ObserveLocation};
use std::path::Path;

/// 한 번의 정책 범위 관측 결과 — manifest 와 그 manifest 를 고른 트리 평가.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedWorld {
    pub(crate) manifest: Manifest,
    pub(crate) evaluation: TreeEvaluation,
}

impl ScopedWorld {
    /// 추적 대상 경로 목록(파일 내용 없음) — 범위 재확인이 견주는 값.
    pub(crate) fn tracked_paths(&self) -> Vec<String> {
        self.evaluation.tracked().iter().map(|file| file.path.as_str().to_owned()).collect()
    }
}

/// 승인 정책으로 두 번 관측한다. 아무것도 쓰지 않는다.
pub(crate) fn observe_scoped(root: &Path, policy: &ApprovedPolicy) -> Result<ScopedWorld, ObserveError> {
    #[cfg(test)]
    probe::observe_called();
    let first = scan(root, policy, None)?;
    #[cfg(test)]
    probe::between_scans();
    let again = scan(root, policy, None)?;
    if first != again {
        return Err(ObserveError::ChangedDuringObservation);
    }
    Ok(first)
}

/// 승인 정책으로 관측하며 추적 파일의 바이트를 창고에 둔다. 두 번째 훑기는 저장 없이 검증만 한다.
///
/// 두 번째 관측이 다르면 첫 훑기의 blob 은 미참조로 남는다 — 어떤 Snapshot 도 가리키지 않으므로
/// 성공으로 보지 않는다(Artifact Model §3.5, 공유 계약 §5.3 「객체 일부가 남았다는 사실도 성공이 아니다」).
pub(crate) fn capture_scoped(root: &Path, policy: &ApprovedPolicy, store: &ObjectStore) -> Result<ScopedWorld, ObserveError> {
    #[cfg(test)]
    probe::capture_called();
    let first = scan(root, policy, Some(store))?;
    #[cfg(test)]
    probe::between_scans();
    let again = scan(root, policy, None)?;
    if first != again {
        return Err(ObserveError::ChangedDuringObservation);
    }
    Ok(first)
}

/// 시험만 쓰는 틈 — **시험에서만 컴파일된다.** 관측·보관이 실제로 몇 번 불렸는지 세고, 두 훑기 사이에 한 번 끼어든다.
///
/// 「새로 관측했다」는 결과만 봐서는 잴 수 없고(세계가 조용하면 앞선 결과와 같다), 두 훑기 사이의 변화는 잠을 재워 운에 맡기면 시험이
/// 깜빡인다. thread 마다 따로 세므로 나란히 도는 다른 시험의 호출이 섞이지 않는다. 실제 실행의 동작은 바꾸지 않는다.
#[cfg(test)]
pub(crate) mod probe {
    use std::cell::{Cell, RefCell};

    thread_local! {
        static OBSERVES: Cell<usize> = const { Cell::new(0) };
        static CAPTURES: Cell<usize> = const { Cell::new(0) };
        static BETWEEN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    }

    pub(super) fn observe_called() {
        OBSERVES.with(|count| count.set(count.get() + 1));
    }

    pub(super) fn capture_called() {
        CAPTURES.with(|count| count.set(count.get() + 1));
    }

    pub(super) fn between_scans() {
        let hook = BETWEEN.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    /// 이 thread 에서 `observe_scoped` 가 불린 횟수.
    pub(crate) fn observes() -> usize {
        OBSERVES.with(Cell::get)
    }

    /// 이 thread 에서 `capture_scoped` 가 불린 횟수.
    pub(crate) fn captures() -> usize {
        CAPTURES.with(Cell::get)
    }

    /// 이 thread 의 **다음** 관측·보관 한 번에서, 두 훑기 사이에 `hook` 을 부른다. 한 번 쓰이면 사라진다.
    pub(crate) fn between_next_scans(hook: impl FnOnce() + 'static) {
        BETWEEN.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    }
}

/// 정책 평가만 — 파일 내용을 읽지 않는다. 시작 준비의 추적 목록 제시·재확인이 쓴다.
pub(crate) fn evaluate_scope(root: &Path, policy: &ApprovedPolicy) -> Result<TreeEvaluation, ObserveError> {
    evaluate_tree(root, policy).map_err(policy_error)
}

fn scan(root: &Path, policy: &ApprovedPolicy, sink: Option<&ObjectStore>) -> Result<ScopedWorld, ObserveError> {
    let evaluation = evaluate_scope(root, policy)?;
    if let Some(blocker) = evaluation.blockers().first() {
        return Err(ObserveError::Policy { code: blocker.code(), said: blocker.to_string() });
    }
    let held = crate::internal_fs::dir_nofollow(root).map_err(|source| ObserveError::ReadFailed {
        at: ObserveLocation::Root,
        source: source.to_string(),
    })?;
    let mut entries = Vec::with_capacity(evaluation.tracked().len());
    for file in evaluation.tracked() {
        let opened = open_tracked(&held, &file.path)?;
        let content = read_into(opened, &file.path, sink)?;
        entries.push(ManifestEntry { path: file.path.clone(), content });
    }
    Ok(ScopedWorld { manifest: Manifest::new(entries), evaluation })
}

/// 루트 핸들에서 구성 요소마다 연결 파일을 따라가지 않고 내려가 그 일반 파일을 연다.
fn open_tracked(root: &File, path: &EntryPath) -> Result<File, ObserveError> {
    let failed = |source: std::io::Error| ObserveError::ReadFailed {
        at: ObserveLocation::Entry(path.clone()),
        source: source.to_string(),
    };
    let parts: Vec<&str> = path.as_str().split('/').collect();
    let (name, dirs) = parts.split_last().expect("Artifact 경로에는 이름이 하나 이상 있다");
    let mut owned: Option<File> = None;
    for dir in dirs {
        let c = CString::new(*dir).map_err(|_| failed(std::io::Error::other("NUL")))?;
        let parent = owned.as_ref().unwrap_or(root);
        owned = Some(crate::internal_fs::child_dir(parent, &c).map_err(failed)?);
    }
    let c = CString::new(*name).map_err(|_| failed(std::io::Error::other("NUL")))?;
    crate::internal_fs::regular_at(owned.as_ref().unwrap_or(root), &c).map_err(failed)
}

fn read_into(file: File, path: &EntryPath, sink: Option<&ObjectStore>) -> Result<ContentDigest, ObserveError> {
    match sink {
        Some(store) => store.put_blob(file).map_err(ObserveError::ObjectStore),
        None => ContentDigest::of(file).map_err(|source| ObserveError::ReadFailed {
            at: ObserveLocation::Entry(path.clone()),
            source: source.to_string(),
        }),
    }
}

fn policy_error(error: super::gilignore::EvaluationError) -> ObserveError {
    use super::gilignore::EvaluationError as E;
    match error {
        E::ChangedDuringEvaluation => ObserveError::ChangedDuringObservation,
        E::UnrepresentablePath { near, shown } => ObserveError::UnrepresentablePath { near, shown },
        E::ReadFailed { at, source } => ObserveError::ReadFailed { at, source },
        E::NestedGilBoundary { path } => ObserveError::NestedGilRepository { path },
        E::SpecialEntry { path, kind } => ObserveError::SpecialEntry { path, kind },
        other => ObserveError::Policy { code: other.code(), said: other.to_string() },
    }
}
