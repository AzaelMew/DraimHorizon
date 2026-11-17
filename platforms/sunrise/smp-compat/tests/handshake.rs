#[allow(dead_code)]
#[path = "../src/handshake.rs"]
mod handshake;
use handshake::{Outcome, Rendezvous};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[test]
fn announce_waits_for_registration_then_queues_once_before_returning() {
    let rendezvous = Arc::new(Rendezvous::default());
    let count = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| rendezvous.announce(10, Duration::from_secs(1)));
        let writes = count.clone();
        assert!(rendezvous.register(
            10,
            1,
            Some(Arc::new(move || {
                writes.fetch_add(1, Ordering::SeqCst);
                true
            }))
        ));
        assert_eq!(worker.join().unwrap(), Outcome::Queued);
    });
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        rendezvous.announce(10, Duration::ZERO),
        Outcome::AlreadyAttempted
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn selection_and_channel_identity_are_separate_from_the_server_brand() {
    let rendezvous = Rendezvous::default();
    rendezvous.register(20, 1, None);
    rendezvous.register(21, 2, Some(Arc::new(|| true)));
    assert_eq!(
        rendezvous.announce(20, Duration::ZERO),
        Outcome::NotSelected
    );
    assert_eq!(rendezvous.announce(21, Duration::ZERO), Outcome::Queued);
    assert_eq!(rendezvous.announce(22, Duration::ZERO), Outcome::TimedOut);
}

#[test]
fn closed_connection_callback_is_not_reported_as_queued() {
    let rendezvous = Rendezvous::default();
    let live = Arc::new(());
    let weak = Arc::downgrade(&live);
    rendezvous.register(30, 1, Some(Arc::new(move || weak.upgrade().is_some())));
    drop(live);
    assert_eq!(
        rendezvous.announce(30, Duration::ZERO),
        Outcome::Unavailable
    );
}

#[test]
fn callback_runs_without_registry_lock_and_new_serial_can_replace_old_registration() {
    let rendezvous = Arc::new(Rendezvous::default());
    let weak = Arc::downgrade(&rendezvous);
    rendezvous.register(
        40,
        1,
        Some(Arc::new(move || {
            weak.upgrade().unwrap().register(41, 2, None)
        })),
    );
    assert_eq!(rendezvous.announce(40, Duration::ZERO), Outcome::Queued);
    assert!(!rendezvous.needs_registration(40, 1));
    assert!(rendezvous.needs_registration(40, 3));
    rendezvous.register(40, 3, Some(Arc::new(|| true)));
    assert_eq!(rendezvous.announce(40, Duration::ZERO), Outcome::Queued);
}

#[test]
fn pending_registrations_are_bounded() {
    let rendezvous = Rendezvous::default();
    for channel in 0..64 {
        assert!(rendezvous.register(channel, channel as u64, None));
    }
    assert!(!rendezvous.register(64, 64, None));
    assert!(rendezvous.register(0, 100, None));
}

#[test]
fn unpolled_connection_times_out_without_calling_another_sessions_sender() {
    let rendezvous = Rendezvous::default();
    let count = Arc::new(AtomicUsize::new(0));
    let writes = count.clone();
    rendezvous.register(
        50,
        1,
        Some(Arc::new(move || {
            writes.fetch_add(1, Ordering::SeqCst);
            true
        })),
    );
    assert_eq!(
        rendezvous.announce(51, Duration::from_millis(2)),
        Outcome::TimedOut
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
