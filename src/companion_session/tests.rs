use super::*;

fn release(n: u8) -> Release {
    Release::new(&format!("1.0.{n}"), &format!("{n:x}").repeat(64)).unwrap()
}
fn controller() -> (Controller, Instant) {
    let now = Instant::now();
    (Controller::new(now), now)
}

#[test]
fn idle_start_has_a_bounded_grace_and_no_autostart_registration() {
    let (mut c, t) = controller();
    assert!(!c.snapshot(t).unwrap().should_exit);
    assert!(c.snapshot(t + STARTUP_GRACE).unwrap().should_exit);
}

#[test]
fn connections_share_one_lifetime_and_the_last_disconnect_ends_it() {
    let (mut c, t) = controller();
    let a = c.project_connected(release(1), t).unwrap();
    let b = c.project_connected(release(1), t).unwrap();
    c.disconnected(a, t).unwrap();
    assert_eq!(c.snapshot(t).unwrap().active_connections, 1);
    assert!(!c.snapshot(t).unwrap().should_exit);
    c.disconnected(b, t).unwrap();
    assert!(c.snapshot(t).unwrap().should_exit);
}

#[test]
fn heartbeat_is_local_and_expiration_cannot_be_reversed_by_a_late_message() {
    let (mut c, t) = controller();
    let a = c.project_connected(release(1), t).unwrap();
    assert!(c.heartbeat(a, t + HEARTBEAT_INTERVAL).unwrap());
    assert!(!c.snapshot(t + LEASE_TIMEOUT).unwrap().should_exit);
    let expired = t + HEARTBEAT_INTERVAL + LEASE_TIMEOUT;
    assert!(!c.heartbeat(a, expired).unwrap());
    assert!(c.snapshot(expired).unwrap().should_exit);
    let b = c.project_connected(release(1), expired).unwrap();
    assert_ne!(a, b);
}

#[test]
fn clock_regression_is_rejected_without_refreshing_the_lease() {
    let (mut c, t) = controller();
    let id = c.project_connected(release(1), t).unwrap();
    c.snapshot(t + LEASE_TIMEOUT).unwrap();
    assert_eq!(c.heartbeat(id, t), Err(Error::ClockWentBackwards));
    assert!(c.snapshot(t + LEASE_TIMEOUT).unwrap().should_exit);
}

#[test]
fn offers_never_start_an_update_or_keep_an_idle_process_alive() {
    let (mut c, t) = controller();
    c.offer(release(2)).unwrap();
    let s = c.snapshot(t + STARTUP_GRACE).unwrap();
    assert!(matches!(s.update, UpdateState::Offered { .. }));
    assert!(s.should_exit);
}

#[test]
fn confirmation_is_bound_to_the_exact_displayed_release() {
    let (mut c, _) = controller();
    assert_eq!(c.user_confirmed(&release(2)), Err(Error::NoOffer));
    c.offer(release(2)).unwrap();
    let altered = Release::new("1.0.2", &"f".repeat(64)).unwrap();
    assert_eq!(c.user_confirmed(&altered), Err(Error::OfferChanged));
    c.offer(release(3)).unwrap();
    assert_eq!(c.user_confirmed(&release(2)), Err(Error::OfferChanged));
    let ticket = c.user_confirmed(&release(3)).unwrap();
    assert_eq!(ticket.release(), &release(3));
    assert_eq!(c.offer(release(4)), Err(Error::Busy));
    assert_eq!(c.user_confirmed(&release(3)), Err(Error::Busy));
}

#[test]
fn an_update_outlives_the_last_connection_but_only_until_it_finishes() {
    let (mut c, t) = controller();
    let a = c.project_connected(release(1), t).unwrap();
    c.offer(release(2)).unwrap();
    let ticket = c.user_confirmed(&release(2)).unwrap();
    c.disconnected(a, t).unwrap();
    assert!(!c.snapshot(t + LEASE_TIMEOUT).unwrap().should_exit);
    c.published(&ticket, &release(2)).unwrap();
    let s = c.snapshot(t + LEASE_TIMEOUT).unwrap();
    assert!(s.should_exit);
    assert!(matches!(s.update, UpdateState::AwaitingConnection { .. }));
}

#[test]
fn failure_and_user_quit_do_not_leave_a_permanent_update_hold() {
    let (mut c, t) = controller();
    c.project_connected(release(1), t).unwrap();
    c.offer(release(2)).unwrap();
    let ticket = c.user_confirmed(&release(2)).unwrap();
    c.request_exit();
    assert!(!c.snapshot(t).unwrap().should_exit);
    c.failed(&ticket).unwrap();
    assert!(c.snapshot(t).unwrap().should_exit);
    assert!(matches!(
        c.snapshot(t).unwrap().update,
        UpdateState::Failed { .. }
    ));
    assert_eq!(c.user_confirmed(&release(2)), Err(Error::ShuttingDown));
    assert_eq!(c.offer(release(3)), Err(Error::ShuttingDown));
    assert_eq!(c.project_connected(release(1), t), Err(Error::ShuttingDown));
}

#[test]
fn stale_or_mismatched_worker_results_cannot_publish() {
    let (mut c, t) = controller();
    c.offer(release(2)).unwrap();
    let old = c.user_confirmed(&release(2)).unwrap();
    c.failed(&old).unwrap();
    c.offer(release(2)).unwrap();
    let current = c.user_confirmed(&release(2)).unwrap();
    assert_eq!(c.published(&old, &release(2)), Err(Error::WrongOperation));
    assert_eq!(c.failed(&old), Err(Error::WrongOperation));
    assert_eq!(
        c.published(&current, &release(3)),
        Err(Error::ReceiptMismatch)
    );
    assert!(matches!(
        c.snapshot(t).unwrap().update,
        UpdateState::Running { .. }
    ));
    c.published(&current, &release(2)).unwrap();
    assert_eq!(
        c.published(&current, &release(2)),
        Err(Error::WrongOperation)
    );
}

#[test]
fn publication_is_not_application_and_old_connections_keep_their_release() {
    let (mut c, t) = controller();
    let v1 = c.project_connected(release(1), t).unwrap();
    let already_v2 = c.project_connected(release(2), t).unwrap();
    c.offer(release(2)).unwrap();
    let ticket = c.user_confirmed(&release(2)).unwrap();
    c.published(&ticket, &release(2)).unwrap();
    assert!(!c.response_confirmed(v1, t).unwrap());
    assert!(!c.response_confirmed(already_v2, t).unwrap());
    let wrong = c.project_connected(release(3), t).unwrap();
    assert!(!c.response_confirmed(wrong, t).unwrap());
    let v2 = c.project_connected(release(2), t).unwrap();
    assert!(matches!(
        c.snapshot(t).unwrap().update,
        UpdateState::AwaitingConnection { .. }
    ));
    assert!(c.response_confirmed(v2, t).unwrap());
    assert_eq!(c.connections[&v1].release, release(1));
    assert!(matches!(
        c.snapshot(t).unwrap().update,
        UpdateState::Applied { .. }
    ));
}

#[test]
fn expired_connections_cannot_confirm_application() {
    let (mut c, t) = controller();
    c.offer(release(2)).unwrap();
    let ticket = c.user_confirmed(&release(2)).unwrap();
    c.published(&ticket, &release(2)).unwrap();
    let id = c.project_connected(release(2), t).unwrap();
    assert!(!c.response_confirmed(id, t + LEASE_TIMEOUT).unwrap());
}

#[test]
fn identity_validation_and_capacity_are_bounded() {
    for v in ["", "../v2", "v2\n", "v2 beta"] {
        assert_eq!(Release::new(v, &"a".repeat(64)), Err(Error::InvalidRelease));
    }
    for hash in ["a".repeat(63), "G".repeat(64), "A".repeat(64)] {
        assert_eq!(Release::new("1.0.0", &hash), Err(Error::InvalidRelease));
    }
    let (mut c, t) = controller();
    for _ in 0..MAX_CONNECTIONS {
        c.project_connected(release(1), t).unwrap();
    }
    assert_eq!(c.project_connected(release(1), t), Err(Error::Capacity));
    c.next_id = u64::MAX;
    assert_eq!(
        c.project_connected(release(1), t + LEASE_TIMEOUT),
        Err(Error::IdExhausted)
    );
}
