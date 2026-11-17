//! A bounded rendezvous between existing Session::poll and receive_brand replacements.
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_CONNECTIONS: usize = 64;
const REGISTRATION_LIFETIME: Duration = Duration::from_secs(30);
pub type SendBrand = Arc<dyn Fn() -> bool + Send + Sync>;

struct Registration {
    serial: u64,
    expires: Instant,
    sender: Option<SendBrand>,
    attempted: bool,
}

#[derive(Default)]
pub struct Rendezvous {
    connections: Mutex<HashMap<usize, Registration>>,
    registered: Condvar,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Queued,
    NotSelected,
    AlreadyAttempted,
    Unavailable,
    TimedOut,
}

impl Rendezvous {
    pub fn needs_registration(&self, channel: usize, serial: u64) -> bool {
        self.connections
            .lock()
            .unwrap()
            .get(&channel)
            .is_none_or(|r| r.serial != serial || r.expires <= Instant::now())
    }

    /// Called on the game thread. The callback owns only weak connection references.
    /// No network operation or wait is performed here.
    pub fn register(&self, channel: usize, serial: u64, sender: Option<SendBrand>) -> bool {
        let mut connections = self.connections.lock().unwrap();
        let now = Instant::now();
        connections.retain(|_, r| r.expires > now);
        if connections
            .get(&channel)
            .is_some_and(|r| r.serial == serial)
        {
            return true;
        }
        if connections.len() >= MAX_CONNECTIONS && !connections.contains_key(&channel) {
            return false;
        }
        connections.insert(
            channel,
            Registration {
                serial,
                sender,
                attempted: false,
                expires: now + REGISTRATION_LIFETIME,
            },
        );
        self.registered.notify_all();
        true
    }

    /// Called only on the receiving thread, while handling the server's Horizon brand.
    /// The callback is invoked before receive_brand returns to the configuration loop.
    pub fn announce(&self, channel: usize, timeout: Duration) -> Outcome {
        let deadline = Instant::now() + timeout;
        let mut connections = self.connections.lock().unwrap();
        let sender = loop {
            if let Some(registration) = connections.get_mut(&channel)
                && registration.expires > Instant::now()
            {
                if registration.attempted {
                    return Outcome::AlreadyAttempted;
                }
                registration.attempted = true;
                break match &registration.sender {
                    Some(sender) => sender.clone(),
                    None => return Outcome::NotSelected,
                };
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Outcome::TimedOut;
            }
            connections = self
                .registered
                .wait_timeout(connections, remaining)
                .unwrap()
                .0;
        };
        // Never hold the registry lock while acquiring the writer/queue locks.
        drop(connections);
        if sender() {
            Outcome::Queued
        } else {
            Outcome::Unavailable
        }
    }
}

pub fn rendezvous() -> &'static Rendezvous {
    static INSTANCE: OnceLock<Rendezvous> = OnceLock::new();
    INSTANCE.get_or_init(Rendezvous::default)
}
