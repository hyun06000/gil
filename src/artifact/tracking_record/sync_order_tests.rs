//! 정책 파일 동기화 순서 제어의 시험 — **합성 보고**만 쓴다.
//!
//! 여기의 `Stored`·`Replaced`·`SameLock` 은 시험이 지어낸 작업 결과다. 실제 파일 교체·상태 저장·잠금을
//! 하지 않으며, 이 시험의 통과는 실제 파일 시스템의 충돌 복구나 원자적 저장을 검증한 것이 아니다.
//! 검증하는 것은 **건네받은 보고에 대해 순서 제어가 계약 §5.2 대로 다음 걸음을 고르는가**뿐이다.

use super::*;
use crate::artifact::digest_of;

fn address(bytes: &[u8]) -> ContentDigest {
    digest_of(bytes).expect("메모리의 바이트는 언제나 읽힌다")
}

const NEW: &[u8] = b"target/\nnode_modules/\n";
const OLD: &[u8] = b"target/\n";
const EDITED: &[u8] = b"edited by hand\n";

fn new() -> ContentDigest {
    address(NEW)
}
fn old() -> ContentDigest {
    address(OLD)
}
fn edited() -> ContentDigest {
    address(EDITED)
}
fn on_disk(digest: ContentDigest) -> DiskReading {
    DiskReading::Present(digest)
}
fn unreadable() -> DiskReading {
    DiskReading::Unreadable { reason: "합성 읽기 오류".into() }
}
fn failed() -> StoreResult {
    StoreResult::Failed { reason: "합성 저장 실패".into() }
}
fn unknown() -> StoreResult {
    StoreResult::Unknown { reason: "합성 결과 불명".into() }
}
fn stored() -> SyncReport {
    SyncReport::Store(StoreResult::Stored)
}
fn store(result: StoreResult) -> SyncReport {
    SyncReport::Store(result)
}
fn replace(result: ReplaceResult) -> SyncReport {
    SyncReport::Replace(result)
}
fn replaced(reread: DiskReading) -> SyncReport {
    replace(ReplaceResult::Replaced { reread })
}
fn not_replaced(reread: Reread) -> SyncReport {
    replace(ReplaceResult::NotReplaced { reason: "합성 권한 오류".into(), reread })
}
fn request(request: SyncRequest) -> SyncStep {
    SyncStep::Request(request)
}
fn mark() -> SyncStep {
    request(SyncRequest::StoreBase(SyncBase::Unconfirmed))
}
fn write() -> SyncStep {
    request(SyncRequest::ReplaceFile { rules: new() })
}
fn settle() -> SyncStep {
    request(SyncRequest::StoreBase(SyncBase::Present(new())))
}
fn stop(stop: SyncStop) -> SyncStep {
    SyncStep::Stop(stop)
}

/// 쓰기 후보 두 가지 — 확인된 기준과 같은 구판, 또는 기준 `absent` 에 파일 없음.
fn stale_starts() -> [(SyncBase, DiskReading); 2] {
    [(SyncBase::Present(old()), on_disk(old())), (SyncBase::Absent, DiskReading::Absent)]
}

/// ①을 저장하고 ②를 기다리는 시도.
fn awaiting_write(base: &SyncBase, disk: &DiskReading) -> PolicyFileSync {
    let (mut sync, first) = PolicyFileSync::begin(&new(), base, disk);
    assert_eq!(first, mark());
    assert_eq!(sync.advance(stored()), Ok(write()));
    sync
}

// ── 정상 순서 ───────────────────────────────────────────────────────────────

#[test]
fn the_normal_order_marks_then_writes_then_settles() {
    for (base, disk) in stale_starts() {
        let (mut sync, first) = PolicyFileSync::begin(&new(), &base, &disk);
        assert_eq!(first, mark(), "쓰기 전에 unconfirmed 저장을 먼저 요청한다 ({base:?})");
        assert_eq!(sync.step(), mark(), "요청은 보고가 올 때까지 그대로다");
        assert_eq!(sync.advance(stored()), Ok(write()));
        assert_eq!(sync.advance(replaced(on_disk(new()))), Ok(settle()));
        assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::InSync)));
    }
}

// ── 1: 재확인이 쓰기 후보가 아니면 시작하지 않는다 ────────────────────────────

#[test]
fn a_recheck_that_is_not_the_confirmed_base_never_reaches_a_write() {
    use PolicyFileReading as R;
    let cases = [
        // 사용자 편집·삭제 — 확인된 기준과도 새 원문과도 다르다.
        (SyncBase::Present(old()), on_disk(edited()), stop(SyncStop::NotWritable(R::UserChanged { deleted: false }))),
        (SyncBase::Present(old()), DiskReading::Absent, stop(SyncStop::NotWritable(R::UserChanged { deleted: true }))),
        (SyncBase::Absent, on_disk(edited()), stop(SyncStop::NotWritable(R::UserChanged { deleted: false }))),
        // 기준 미확인 — 원인 미확인이므로 자동으로 쓰지 않는다.
        (SyncBase::Unconfirmed, on_disk(old()), stop(SyncStop::NotWritable(R::BaseUnconfirmed))),
        (SyncBase::Unconfirmed, DiskReading::Absent, stop(SyncStop::NotWritable(R::BaseUnconfirmed))),
        // 읽지 못함 — 없음으로 보지 않는다.
        (SyncBase::Absent, unreadable(), stop(SyncStop::NotWritable(R::Unreadable { reason: "합성 읽기 오류".into() }))),
        (SyncBase::Present(old()), unreadable(), stop(SyncStop::NotWritable(R::Unreadable { reason: "합성 읽기 오류".into() }))),
        // 디스크도 기준도 이미 원문이다 — 파일도 기준도 쓰지 않는다. (기준이 다른 값인 일치는 아래 기준 갱신 시험.)
        (SyncBase::Present(new()), on_disk(new()), stop(SyncStop::AlreadyMatches)),
    ];
    for (base, disk, wanted) in cases {
        let (mut sync, first) = PolicyFileSync::begin(&new(), &base, &disk);
        assert_eq!(first, wanted, "{base:?} / {disk:?}");
        // 멈춘 시도는 어떤 보고로도 쓰기로 넘어가지 않는다.
        for report in [stored(), replaced(on_disk(new()))] {
            assert!(matches!(sync.advance(report), Err(SyncOutOfOrder::AlreadyStopped { .. })));
        }
        assert_eq!(sync.step(), wanted);
    }
}

fn all_bases() -> [SyncBase; 5] {
    [SyncBase::Present(old()), SyncBase::Present(new()), SyncBase::Present(edited()), SyncBase::Absent, SyncBase::Unconfirmed]
}
fn all_disks() -> [DiskReading; 5] {
    [on_disk(old()), on_disk(new()), on_disk(edited()), DiskReading::Absent, unreadable()]
}

/// 관측 하나만으로 나오는 첫 요청은 ①(쓰기 후보일 때) 또는 기준 저장(디스크 = 원문이고 기준이 아직 그 원문이
/// 아닐 때)뿐이다. 관측만으로 파일 쓰기나, 원문이 아닌 디스크 위의 새 `present` 기준 저장을 요청하지 않는다.
#[test]
fn an_observation_alone_never_requests_a_write_or_a_settled_base() {
    for base in &all_bases() {
        for disk in &all_disks() {
            let reading = classify_policy_file(&new(), base, disk);
            let (_, first) = PolicyFileSync::begin(&new(), base, disk);
            match &first {
                SyncStep::Request(SyncRequest::ReplaceFile { .. }) => panic!("관측만으로 쓰기를 요청했다: {base:?} / {disk:?}"),
                SyncStep::Request(SyncRequest::StoreBase(SyncBase::Unconfirmed)) => {
                    assert_eq!(reading, PolicyFileReading::StaleMatchesBase, "{base:?} / {disk:?}")
                }
                SyncStep::Request(SyncRequest::StoreBase(other)) => {
                    assert_eq!(other, &SyncBase::Present(new()));
                    assert_eq!(disk, &on_disk(new()), "원문이 아닌 디스크 위에 새 기준을 요청했다: {base:?}");
                    assert_ne!(base, &SyncBase::Present(new()), "이미 같은 기준을 다시 저장하려 했다");
                }
                SyncStep::Stop(_) => assert_ne!(reading, PolicyFileReading::StaleMatchesBase, "{base:?} / {disk:?}"),
            }
            // 명시적 동기화가 아닌 변경 명령: 요청은 unconfirmed 정리 하나뿐이다.
            let (_, first) = PolicyFileSync::begin_cleanup(&new(), base, disk);
            match (base, disk) {
                (SyncBase::Unconfirmed, found) if found == &on_disk(new()) => assert_eq!(first, settle()),
                _ => assert_eq!(first, stop(SyncStop::NoCleanup(reading)), "{base:?} / {disk:?}"),
            }
        }
    }
}

// ── 2·3: ① 저장이 확인되기 전에는 쓰지 않는다 ─────────────────────────────────

#[test]
fn a_failed_or_unknown_mark_never_requests_the_file_write() {
    for (base, disk) in stale_starts() {
        for result in [failed(), unknown()] {
            let (mut sync, _) = PolicyFileSync::begin(&new(), &base, &disk);
            let wanted = stop(SyncStop::MarkNotStored(result.clone()));
            assert_eq!(sync.advance(store(result)), Ok(wanted.clone()));
            assert!(matches!(sync.advance(replaced(on_disk(new()))), Err(SyncOutOfOrder::AlreadyStopped { .. })));
            assert_eq!(sync.step(), wanted);
        }
    }
}

#[test]
fn reports_out_of_order_are_refused_and_change_nothing() {
    let (base, disk) = (SyncBase::Present(old()), on_disk(old()));

    // ①을 건너뛰고 쓰기 결과를 보고한다.
    let (mut sync, _) = PolicyFileSync::begin(&new(), &base, &disk);
    let before = sync.clone();
    for report in [replaced(on_disk(new())), not_replaced(Reread::SameLock(on_disk(old())))] {
        assert!(matches!(sync.advance(report), Err(SyncOutOfOrder::UnexpectedReport { .. })));
        assert_eq!(sync, before, "거절한 보고가 제어 값을 바꿨다");
    }

    // ②를 건너뛰고 저장 결과로 ③에 가려 한다.
    let mut sync = awaiting_write(&base, &disk);
    let before = sync.clone();
    assert_eq!(
        sync.advance(stored()),
        Err(SyncOutOfOrder::UnexpectedReport { awaiting: SyncRequest::ReplaceFile { rules: new() }, got: stored() })
    );
    assert_eq!(sync, before);
    assert_eq!(sync.step(), write());

    // ③을 기다리는데 쓰기 결과가 다시 온다.
    let mut sync = awaiting_write(&base, &disk);
    assert_eq!(sync.advance(replaced(on_disk(new()))), Ok(settle()));
    assert!(matches!(sync.advance(replaced(on_disk(new()))), Err(SyncOutOfOrder::UnexpectedReport { .. })));
    assert_eq!(sync.step(), settle());

    // 멈춘 뒤의 보고.
    assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::InSync)));
    assert!(matches!(sync.advance(stored()), Err(SyncOutOfOrder::AlreadyStopped { .. })));
}

// ── 4: 교체·재조회 일치 뒤에만 새 present 기준 ────────────────────────────────

#[test]
fn a_reread_that_is_not_the_new_rules_never_settles_the_base() {
    for (base, disk) in stale_starts() {
        for reread in [on_disk(old()), on_disk(edited()), DiskReading::Absent, unreadable()] {
            let mut sync = awaiting_write(&base, &disk);
            let wanted = stop(SyncStop::WriteUnsettled(Unsettled::RereadDiffers(reread.clone())));
            assert_eq!(sync.advance(replaced(reread.clone())), Ok(wanted.clone()), "{reread:?}");
            assert!(matches!(sync.advance(stored()), Err(SyncOutOfOrder::AlreadyStopped { .. })));
        }
    }
}

// ── 5: 이전 기준 복귀의 조건 ────────────────────────────────────────────────

#[test]
fn the_previous_base_comes_back_only_after_a_clear_pre_replace_failure_and_an_unchanged_same_lock_reread() {
    for (base, disk) in stale_starts() {
        let mut sync = awaiting_write(&base, &disk);
        assert_eq!(sync.advance(not_replaced(Reread::SameLock(disk.clone()))), Ok(request(SyncRequest::StoreBase(base.clone()))));
        assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::WriteFailedBaseRestored)));

        for result in [failed(), unknown()] {
            let mut sync = awaiting_write(&base, &disk);
            assert_eq!(sync.advance(not_replaced(Reread::SameLock(disk.clone()))), Ok(request(SyncRequest::StoreBase(base.clone()))));
            assert_eq!(
                sync.advance(store(result.clone())),
                Ok(stop(SyncStop::WriteUnsettled(Unsettled::RestoreNotStored(result))))
            );
        }
    }
}

#[test]
fn no_restore_when_replacement_is_unclear_or_the_disk_moved() {
    let reason = || "합성 권한 오류".to_string();
    for (base, disk) in stale_starts() {
        let other_rereads = [on_disk(old()), on_disk(edited()), on_disk(new()), DiskReading::Absent, unreadable()]
            .into_iter()
            .filter(|reread| reread != &disk);
        let mut cases: Vec<(SyncReport, Unsettled)> = other_rereads
            .map(|reread| {
                (not_replaced(Reread::SameLock(reread.clone())), Unsettled::DiskNotAtPrevious { reason: reason(), reread })
            })
            .collect();
        cases.push((not_replaced(Reread::NotInSameLock), Unsettled::NotRereadInSameLock { reason: reason() }));
        cases.push((
            replace(ReplaceResult::Unknown { reason: "교체 여부 불명".into() }),
            Unsettled::ReplacementUnknown { reason: "교체 여부 불명".into() },
        ));
        for (report, why) in cases {
            let mut sync = awaiting_write(&base, &disk);
            let got = sync.advance(report.clone());
            assert_eq!(got, Ok(stop(SyncStop::WriteUnsettled(why))), "{base:?} / {report:?}");
            assert!(!matches!(got, Ok(SyncStep::Request(_))), "복귀나 다른 저장을 요청했다");
        }
    }
}

// ── 6: 파일 일치와 기준 미확인의 구분 ───────────────────────────────────────

#[test]
fn a_failed_settle_says_the_file_matches_but_the_base_is_unsettled() {
    for (base, disk) in stale_starts() {
        for result in [failed(), unknown()] {
            let mut sync = awaiting_write(&base, &disk);
            assert_eq!(sync.advance(replaced(on_disk(new()))), Ok(settle()));
            assert_eq!(sync.advance(store(result.clone())), Ok(stop(SyncStop::FileMatchesBaseUnsettled(result))));
        }
    }
    // 그 뒤의 읽기 전용 표시는 동기화됨이다 — 기준 `unconfirmed` 여도.
    assert_eq!(
        classify_policy_file(&new(), &SyncBase::Unconfirmed, &on_disk(new())),
        PolicyFileReading::MatchesApplied
    );
}

// ── 7·8: 재시작 뒤 ──────────────────────────────────────────────────────────

#[test]
fn after_a_restart_an_unconfirmed_base_with_a_different_disk_is_never_rewritten() {
    for disk in [on_disk(old()), on_disk(edited()), DiskReading::Absent] {
        let (_, first) = PolicyFileSync::begin(&new(), &SyncBase::Unconfirmed, &disk);
        assert_eq!(first, stop(SyncStop::NotWritable(PolicyFileReading::BaseUnconfirmed)), "{disk:?}");
    }
}

#[test]
fn after_a_restart_a_matching_disk_is_not_rewritten_and_only_a_change_command_settles_the_base() {
    // 읽기 전용 조회: 판정만 있다. 판정값에는 저장 요청이 들어갈 자리가 없다.
    let reading = classify_policy_file(&new(), &SyncBase::Unconfirmed, &on_disk(new()));
    assert_eq!(reading, PolicyFileReading::MatchesApplied);

    // 변경 명령(명시적 동기화든 아니든): 파일을 쓰지 않고 기준만 정리하도록 요청한다.
    type Begin = fn(&ContentDigest, &SyncBase, &DiskReading) -> (PolicyFileSync, SyncStep);
    for begin in [PolicyFileSync::begin as Begin, PolicyFileSync::begin_cleanup as Begin] {
        let (mut sync, first) = begin(&new(), &SyncBase::Unconfirmed, &on_disk(new()));
        assert_eq!(first, settle());
        assert!(matches!(sync.advance(replaced(on_disk(new()))), Err(SyncOutOfOrder::UnexpectedReport { .. })));
        assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::InSync)));
        for result in [failed(), unknown()] {
            let (mut sync, _) = begin(&new(), &SyncBase::Unconfirmed, &on_disk(new()));
            assert_eq!(sync.advance(store(result.clone())), Ok(stop(SyncStop::FileMatchesBaseUnsettled(result))));
        }
    }
}

#[test]
fn the_read_only_table_does_not_know_about_requests() {
    // 판정표의 소스는 순서 제어를 부르지도, 요청 자료형을 만들지도 않는다.
    let table = include_str!("sync.rs");
    for word in ["sync_order", "SyncRequest", "StoreBase", "ReplaceFile", "PolicyFileSync"] {
        assert!(!table.contains(word), "sync.rs 가 `{word}` 를 쓴다");
    }
}

// ── 9: 읽기 실패와 부재 ─────────────────────────────────────────────────────

#[test]
fn an_unreadable_file_is_never_taken_for_an_absent_one_at_any_step() {
    // 기준 absent: 없는 파일은 쓰기 후보이고 읽지 못한 파일은 아니다.
    assert_eq!(PolicyFileSync::begin(&new(), &SyncBase::Absent, &DiskReading::Absent).1, mark());
    assert!(matches!(
        PolicyFileSync::begin(&new(), &SyncBase::Absent, &unreadable()).1,
        SyncStep::Stop(SyncStop::NotWritable(PolicyFileReading::Unreadable { .. }))
    ));
    // 교체 전 실패 뒤 다시 읽지 못했다 — 이전 기준 absent 로 되돌리지 않는다.
    let mut sync = awaiting_write(&SyncBase::Absent, &DiskReading::Absent);
    assert!(matches!(
        sync.advance(not_replaced(Reread::SameLock(unreadable()))),
        Ok(SyncStep::Stop(SyncStop::WriteUnsettled(Unsettled::DiskNotAtPrevious { .. })))
    ));
}

// ── 10: 성공 뒤 사용자가 되돌린 것 ──────────────────────────────────────────

#[test]
fn reverting_the_file_after_a_successful_sync_is_a_user_edit() {
    let mut sync = awaiting_write(&SyncBase::Present(old()), &on_disk(old()));
    assert_eq!(sync.advance(replaced(on_disk(new()))), Ok(settle()));
    assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::InSync)));
    let after = SyncBase::Present(new());
    for (disk, deleted) in [(on_disk(old()), false), (DiskReading::Absent, true)] {
        let (_, first) = PolicyFileSync::begin(&new(), &after, &disk);
        assert_eq!(first, stop(SyncStop::NotWritable(PolicyFileReading::UserChanged { deleted })), "{disk:?}");
    }
}

// ── 명시적 동기화의 기준 갱신 (계약 §5.2, 2026-11-20 사용자 결정) ─────────────

/// 디스크가 이미 원문 B인데 기준이 B가 아닌 시작 — 기준 present(A), absent, unconfirmed, 그리고 다른 present.
fn matching_disk_bases() -> [SyncBase; 4] {
    [SyncBase::Present(old()), SyncBase::Absent, SyncBase::Unconfirmed, SyncBase::Present(edited())]
}

#[test]
fn an_explicit_sync_over_a_matching_disk_asks_only_for_the_new_base() {
    for base in matching_disk_bases() {
        let (mut sync, first) = PolicyFileSync::begin(&new(), &base, &on_disk(new()));
        assert_eq!(first, settle(), "파일 쓰기도 unconfirmed 저장도 없이 present(B) 저장만 ({base:?})");
        // 파일 쓰기 결과는 기다리지 않는다 — 들어와도 거절하고 바뀌지 않는다.
        let before = sync.clone();
        for report in [replaced(on_disk(new())), not_replaced(Reread::SameLock(on_disk(new())))] {
            assert!(matches!(sync.advance(report), Err(SyncOutOfOrder::UnexpectedReport { .. })), "{base:?}");
            assert_eq!(sync, before);
        }
        assert_eq!(sync.advance(stored()), Ok(stop(SyncStop::InSync)), "{base:?}");
        assert!(matches!(sync.advance(stored()), Err(SyncOutOfOrder::AlreadyStopped { .. })));
    }
}

#[test]
fn an_explicit_sync_over_a_fully_synced_file_writes_nothing() {
    let (mut sync, first) = PolicyFileSync::begin(&new(), &SyncBase::Present(new()), &on_disk(new()));
    assert_eq!(first, stop(SyncStop::AlreadyMatches));
    for report in [stored(), replaced(on_disk(new()))] {
        assert!(matches!(sync.advance(report), Err(SyncOutOfOrder::AlreadyStopped { .. })));
    }
}

#[test]
fn a_failed_or_unknown_new_base_is_never_reported_as_settled() {
    for base in matching_disk_bases() {
        for result in [failed(), unknown()] {
            let (mut sync, _) = PolicyFileSync::begin(&new(), &base, &on_disk(new()));
            let got = sync.advance(store(result.clone()));
            assert_eq!(got, Ok(stop(SyncStop::FileMatchesBaseUnsettled(result))), "{base:?}");
            assert_ne!(got, Ok(stop(SyncStop::InSync)));
            // 멈춘 뒤에는 어떤 보고로도 파일 쓰기로 넘어가지 않는다.
            assert!(matches!(sync.advance(replaced(on_disk(new()))), Err(SyncOutOfOrder::AlreadyStopped { .. })));
        }
    }
}

#[test]
fn other_change_commands_do_not_take_the_explicit_sync_base_update() {
    use PolicyFileReading as R;
    // 확인된 다른 기준 위의 일치 — 명시적 동기화가 아니면 기준을 바꾸지 않는다.
    for base in [SyncBase::Present(old()), SyncBase::Absent, SyncBase::Present(edited())] {
        let (mut sync, first) = PolicyFileSync::begin_cleanup(&new(), &base, &on_disk(new()));
        assert_eq!(first, stop(SyncStop::NoCleanup(R::MatchesApplied)), "{base:?}");
        assert!(matches!(sync.advance(stored()), Err(SyncOutOfOrder::AlreadyStopped { .. })));
    }
    // 구판 잔류여도 이 입구는 파일 쓰기를 시작하지 않는다.
    for (base, disk) in stale_starts() {
        let (_, first) = PolicyFileSync::begin_cleanup(&new(), &base, &disk);
        assert_eq!(first, stop(SyncStop::NoCleanup(R::StaleMatchesBase)), "{base:?}");
    }
}

/// B 관측 → 기준 B 저장 성공 → 사용자가 A로 되돌리거나 지움 → 다음 동기화.
#[test]
fn after_the_new_base_is_stored_a_revert_or_delete_is_a_user_change_and_is_not_overwritten() {
    for start in [SyncBase::Present(old()), SyncBase::Absent, SyncBase::Unconfirmed] {
        let mut model = Model { base: start.clone(), disk: on_disk(new()) };
        let (mut sync, mut step) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
        while let SyncStep::Request(request) = step {
            let report = perform(&mut model, &request, Fate::Ok);
            step = sync.advance(report).unwrap();
        }
        assert_eq!(step, stop(SyncStop::InSync), "{start:?}");
        assert_eq!(model, Model { base: SyncBase::Present(new()), disk: on_disk(new()) }, "파일은 그대로, 기준만 B");

        for (user_disk, deleted) in [(on_disk(old()), false), (DiskReading::Absent, true)] {
            let wanted = PolicyFileReading::UserChanged { deleted };
            assert_eq!(classify_policy_file(&new(), &model.base, &user_disk), wanted, "{start:?} / {user_disk:?}");
            let (_, next) = PolicyFileSync::begin(&new(), &model.base, &user_disk);
            assert_eq!(next, stop(SyncStop::NotWritable(wanted.clone())), "{start:?} / {user_disk:?}");
            let (_, next) = PolicyFileSync::begin_cleanup(&new(), &model.base, &user_disk);
            assert_eq!(next, stop(SyncStop::NoCleanup(wanted)), "{start:?} / {user_disk:?}");
        }
    }
}

/// 실패·불명 뒤의 재시도는 제어 값이 기억한 것이 아니라 실제로 저장된 기준과 새 관측으로 판단한다.
#[test]
fn a_retry_after_an_unsettled_new_base_judges_the_base_actually_stored() {
    for start in matching_disk_bases() {
        for fate in [Fate::Fail, Fate::UnknownNoEffect, Fate::UnknownTookEffect] {
            let mut model = Model { base: start.clone(), disk: on_disk(new()) };
            let (mut sync, first) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
            let SyncStep::Request(request) = first else { panic!("{start:?}") };
            let report = perform(&mut model, &request, fate);
            assert!(matches!(sync.advance(report), Ok(SyncStep::Stop(SyncStop::FileMatchesBaseUnsettled(_)))));

            let (_, retry) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
            match fate {
                // 저장이 실제로 닿았다면 이제 할 일이 없다.
                Fate::UnknownTookEffect => assert_eq!(retry, stop(SyncStop::AlreadyMatches), "{start:?}"),
                // 닿지 않았다면 앞 기준이 그대로이므로 다시 기준 저장만 요청한다(파일 쓰기 없음).
                _ => {
                    assert_eq!(model.base, start);
                    assert_eq!(retry, settle(), "{start:?} / {fate:?}");
                }
            }
        }
    }
}

/// 한계를 고정해 두는 시험 — 기준 저장이 실패해 이전 기준 A가 남은 채 사용자가 A로 되돌리면 구판 잔류와
/// 구별할 수 없다. 이 경우의 편집 보호는 주장하지 않는다(계약 §5.2).
#[test]
fn after_a_failed_new_base_a_revert_cannot_be_told_from_a_stale_file() {
    let mut model = Model { base: SyncBase::Present(old()), disk: on_disk(new()) };
    let (mut sync, first) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
    let SyncStep::Request(request) = first else { panic!() };
    let report = perform(&mut model, &request, Fate::Fail);
    assert!(matches!(sync.advance(report), Ok(SyncStep::Stop(SyncStop::FileMatchesBaseUnsettled(_)))));
    model.disk = on_disk(old());
    assert_eq!(classify_policy_file(&new(), &model.base, &model.disk), PolicyFileReading::StaleMatchesBase);
    assert_eq!(PolicyFileSync::begin(&new(), &model.base, &model.disk).1, mark());
}

/// 읽기 전용 조회는 어떤 조합에서도 판정만 낸다 — 디스크가 원문이면 기준이 무엇이든 동기화됨.
#[test]
fn a_read_only_query_is_only_a_reading_for_every_combination() {
    for base in &all_bases() {
        for disk in &all_disks() {
            let reading: PolicyFileReading = classify_policy_file(&new(), base, disk);
            if disk == &on_disk(new()) {
                assert_eq!(reading, PolicyFileReading::MatchesApplied, "{base:?}");
            }
            // 같은 조회를 몇 번 해도 같은 판정이다 — 조회가 남기는 것이 없다.
            assert_eq!(classify_policy_file(&new(), base, disk), reading);
        }
    }
}

/// 디스크가 이전 기준과도 적용 원문과도 다르면 어느 입구에서도 쓰지도 저장하지도 않는다.
#[test]
fn a_disk_unlike_both_the_base_and_the_rules_stays_protected_at_every_entry() {
    for (base, disk) in [
        (SyncBase::Present(old()), on_disk(edited())),
        (SyncBase::Present(old()), DiskReading::Absent),
        (SyncBase::Absent, on_disk(edited())),
        (SyncBase::Present(new()), on_disk(old())),
        (SyncBase::Present(new()), DiskReading::Absent),
    ] {
        let reading = classify_policy_file(&new(), &base, &disk);
        assert!(matches!(reading, PolicyFileReading::UserChanged { .. }), "{base:?} / {disk:?}");
        assert_eq!(PolicyFileSync::begin(&new(), &base, &disk).1, stop(SyncStop::NotWritable(reading.clone())));
        assert_eq!(PolicyFileSync::begin_cleanup(&new(), &base, &disk).1, stop(SyncStop::NoCleanup(reading)));
    }
}

// ── 11: 승인 저장 때의 기준 ─────────────────────────────────────────────────

#[test]
fn the_base_written_with_a_new_policy_keeps_an_unconfirmed_mismatch() {
    let rows = [
        // 디스크 = 새 원문 — 디스크를 확인한 결과.
        (SyncBase::Present(old()), on_disk(new()), Ok(SyncBase::Present(new()))),
        (SyncBase::Absent, on_disk(new()), Ok(SyncBase::Present(new()))),
        (SyncBase::Unconfirmed, on_disk(new()), Ok(SyncBase::Present(new()))),
        // 기준 unconfirmed 와 새 원문 불일치 — 그대로 unconfirmed.
        (SyncBase::Unconfirmed, on_disk(old()), Ok(SyncBase::Unconfirmed)),
        (SyncBase::Unconfirmed, on_disk(edited()), Ok(SyncBase::Unconfirmed)),
        (SyncBase::Unconfirmed, DiskReading::Absent, Ok(SyncBase::Unconfirmed)),
        (SyncBase::Unconfirmed, unreadable(), Ok(SyncBase::Unconfirmed)),
        // 확인된 기준과 디스크가 같다.
        (SyncBase::Present(old()), on_disk(old()), Ok(SyncBase::Present(old()))),
        (SyncBase::Absent, DiskReading::Absent, Ok(SyncBase::Absent)),
        // 읽지 못함 — 기준 그대로.
        (SyncBase::Present(old()), unreadable(), Ok(SyncBase::Present(old()))),
        (SyncBase::Absent, unreadable(), Ok(SyncBase::Absent)),
    ];
    for (base, disk, wanted) in rows {
        assert_eq!(base_for_new_policy(&new(), &base, &disk), wanted, "{base:?} / {disk:?}");
    }
    // 계약이 정하지 않은 조합 — 고르지 않는다.
    for (base, disk) in [
        (SyncBase::Present(old()), on_disk(edited())),
        (SyncBase::Present(old()), DiskReading::Absent),
        (SyncBase::Absent, on_disk(edited())),
    ] {
        assert_eq!(
            base_for_new_policy(&new(), &base, &disk),
            Err(BaseUndecided { base: base.clone(), disk: disk.clone() })
        );
    }
}

// ── 모든 중단 지점을 지나는 합성 모형 ───────────────────────────────────────

/// 메모리 안의 합성 모형 — 저장된 기준과 디스크 내용 두 칸뿐이다. 실제 파일 시스템이 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Model {
    base: SyncBase,
    disk: DiskReading,
}

/// 한 요청에 대해 모형이 겪는 일과 보고하는 결과.
#[derive(Debug, Clone, Copy)]
enum Fate {
    Ok,
    /// 효과가 없었고 실패라고 보고한다.
    Fail,
    /// 효과가 있었지만 알 수 없다고 보고한다.
    UnknownTookEffect,
    /// 효과가 없었고 알 수 없다고 보고한다.
    UnknownNoEffect,
    /// 교체했지만 재조회 전에 누군가 파일을 바꿨다(교체 요청에만).
    ReplacedThenEdited,
    /// 교체 전 실패, 같은 잠금 재조회 없음(교체 요청에만).
    FailNoSameLockReread,
}

const STORE_FATES: [Fate; 4] = [Fate::Ok, Fate::Fail, Fate::UnknownTookEffect, Fate::UnknownNoEffect];
const REPLACE_FATES: [Fate; 6] = [
    Fate::Ok,
    Fate::Fail,
    Fate::UnknownTookEffect,
    Fate::UnknownNoEffect,
    Fate::ReplacedThenEdited,
    Fate::FailNoSameLockReread,
];

fn perform(model: &mut Model, request: &SyncRequest, fate: Fate) -> SyncReport {
    match request {
        SyncRequest::StoreBase(value) => {
            if matches!(fate, Fate::Ok | Fate::UnknownTookEffect) {
                model.base = value.clone();
            }
            store(match fate {
                Fate::Ok => StoreResult::Stored,
                Fate::Fail => failed(),
                _ => unknown(),
            })
        }
        SyncRequest::ReplaceFile { rules } => {
            assert_eq!(model.base, SyncBase::Unconfirmed, "T13: 기준을 unconfirmed 로 저장하기 전에 쓰기를 요청했다");
            let unclear = || replace(ReplaceResult::Unknown { reason: "합성".into() });
            match fate {
                Fate::Ok => {
                    model.disk = on_disk(rules.clone());
                    replaced(model.disk.clone())
                }
                Fate::ReplacedThenEdited => {
                    model.disk = on_disk(edited());
                    replaced(model.disk.clone())
                }
                Fate::Fail => not_replaced(Reread::SameLock(model.disk.clone())),
                Fate::FailNoSameLockReread => not_replaced(Reread::NotInSameLock),
                Fate::UnknownTookEffect => {
                    model.disk = on_disk(rules.clone());
                    unclear()
                }
                Fate::UnknownNoEffect => unclear(),
            }
        }
    }
}

/// 확인된 기준은 언제나 디스크의 사실이어야 한다 — 기준이 덮어써도 된다고 말하는 상태가 실제와 다르면 안 된다.
fn assert_base_is_truthful(model: &Model, context: &str) {
    match &model.base {
        SyncBase::Present(digest) => assert_eq!(model.disk, on_disk(digest.clone()), "{context}: {model:?}"),
        SyncBase::Absent => assert_eq!(model.disk, DiskReading::Absent, "{context}: {model:?}"),
        SyncBase::Unconfirmed => {}
    }
}

#[test]
fn every_interruption_point_keeps_the_base_truthful_and_never_rewrites_on_restart() {
    let mut runs = 0;
    for (base, disk) in stale_starts() {
        for mark_fate in STORE_FATES {
            for replace_fate in REPLACE_FATES {
                for last_fate in STORE_FATES {
                    let mut model = Model { base: base.clone(), disk: disk.clone() };
                    let fates = [mark_fate, replace_fate, last_fate];
                    let context = format!("{base:?} / {fates:?}");
                    let (mut sync, mut step) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
                    let mut used = 0;
                    while let SyncStep::Request(request) = step {
                        let fate = fates[used];
                        used += 1;
                        let report = perform(&mut model, &request, fate);
                        step = sync.advance(report).unwrap_or_else(|err| panic!("{context}: {err}"));
                        assert_base_is_truthful(&model, &context);
                    }
                    let SyncStep::Stop(end) = step else { unreachable!() };
                    runs += 1;

                    // 멈춤의 이름이 모형의 사실과 맞는다.
                    match &end {
                        SyncStop::InSync => assert_eq!(model, Model { base: SyncBase::Present(new()), disk: on_disk(new()) }, "{context}"),
                        SyncStop::FileMatchesBaseUnsettled(_) => assert_eq!(model.disk, on_disk(new()), "{context}"),
                        SyncStop::WriteFailedBaseRestored => assert_eq!(model, Model { base: base.clone(), disk: disk.clone() }, "{context}"),
                        SyncStop::MarkNotStored(StoreResult::Failed { .. }) => {
                            assert_eq!(model, Model { base: base.clone(), disk: disk.clone() }, "{context}")
                        }
                        _ => {}
                    }

                    // 재시작: 메모리의 제어 값은 사라지고 기준과 디스크만 남는다. 원문과 다른 파일 위에서
                    // 기준이 unconfirmed 면 쓰지 않는다. 원문과 같은 파일은 다시 쓰지 않는다.
                    let (_, again) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
                    match (&model.base, model.disk == on_disk(new())) {
                        (SyncBase::Unconfirmed, false) => assert_eq!(
                            again,
                            stop(SyncStop::NotWritable(PolicyFileReading::BaseUnconfirmed)),
                            "{context}"
                        ),
                        (SyncBase::Unconfirmed, true) => assert_eq!(again, settle(), "{context}: 기준 정리만"),
                        (SyncBase::Present(kept), true) if kept == &new() => {
                            assert_eq!(again, stop(SyncStop::AlreadyMatches), "{context}")
                        }
                        (_, true) => assert_eq!(again, settle(), "{context}: 기준 갱신만"),
                        // 확인된 기준 위의 구판 — 처음부터 다시 할 수 있다.
                        (_, false) => assert_eq!(again, mark(), "{context}"),
                    }
                }
            }
        }
    }
    assert_eq!(runs, 2 * 4 * 6 * 4);
}

/// 디스크가 이미 원문인 모든 시작 × 기준 저장의 모든 결과 × 사용자의 다음 행동 → 다음 명시적 동기화.
#[test]
fn every_new_base_outcome_then_a_user_action_keeps_edits_protected_once_the_base_is_stored() {
    #[derive(Debug, Clone, Copy)]
    enum UserAction {
        Nothing,
        RevertToOld,
        Delete,
    }
    let mut runs = 0;
    for start in all_bases() {
        for fate in STORE_FATES {
            for action in [UserAction::Nothing, UserAction::RevertToOld, UserAction::Delete] {
                let context = format!("{start:?} / {fate:?} / {action:?}");
                let mut model = Model { base: start.clone(), disk: on_disk(new()) };
                let (mut sync, mut step) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
                while let SyncStep::Request(request) = step {
                    assert_eq!(request, SyncRequest::StoreBase(SyncBase::Present(new())), "{context}: 기준 저장 말고 다른 요청");
                    let report = perform(&mut model, &request, fate);
                    step = sync.advance(report).unwrap_or_else(|err| panic!("{context}: {err}"));
                    // 시작 기준(A·absent)은 이미 디스크 B와 다를 수 있다 — 제어가 새로 적게 한 기준만 사실이어야 한다.
                    if model.base != start {
                        assert_base_is_truthful(&model, &context);
                    }
                }
                assert_eq!(model.disk, on_disk(new()), "{context}: 파일이 바뀌었다");
                let reported_stored = matches!(step, SyncStep::Stop(SyncStop::InSync | SyncStop::AlreadyMatches));
                if reported_stored {
                    assert_eq!(model.base, SyncBase::Present(new()), "{context}");
                } else {
                    assert!(matches!(step, SyncStep::Stop(SyncStop::FileMatchesBaseUnsettled(_))), "{context}: {step:?}");
                }
                runs += 1;

                match action {
                    UserAction::Nothing => {}
                    UserAction::RevertToOld => model.disk = on_disk(old()),
                    UserAction::Delete => model.disk = DiskReading::Absent,
                }
                let (_, next) = PolicyFileSync::begin(&new(), &model.base, &model.disk);
                let reading = classify_policy_file(&new(), &model.base, &model.disk);
                match (&model.base, action) {
                    (SyncBase::Present(kept), UserAction::Nothing) if kept == &new() => {
                        assert_eq!(next, stop(SyncStop::AlreadyMatches), "{context}")
                    }
                    (_, UserAction::Nothing) => assert_eq!(next, settle(), "{context}: 실제 기준으로 다시 기준 저장만"),
                    // 기준 B가 실제로 저장됐다면 되돌림·삭제는 사용자 편집·삭제다 — 쓰지 않는다.
                    (SyncBase::Present(kept), _) if kept == &new() => {
                        assert!(matches!(reading, PolicyFileReading::UserChanged { .. }), "{context}");
                        assert_eq!(next, stop(SyncStop::NotWritable(reading)), "{context}");
                    }
                    // 기준 저장이 닿지 않았다 — 앞 기준에 따라 판정한다. 보호를 주장하지 않는 자리다.
                    _ => {
                        assert!(!reported_stored, "{context}");
                        assert_eq!(model.base, start, "{context}");
                    }
                }
            }
        }
    }
    assert_eq!(runs, 5 * 4 * 3);
}
