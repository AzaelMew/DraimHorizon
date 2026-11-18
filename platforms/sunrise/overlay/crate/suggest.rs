//! Chat completions the game side hands to the chat screen's hook (a lower crate).
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The text up to the cursor in; where the word to complete starts (a byte index) and the candidates out.
pub type Provider = fn(&str) -> Option<(usize, Vec<String>)>;

static PROVIDER: OnceLock<Provider> = OnceLock::new();
static NICKNAMES: Mutex<Option<(Instant, usize, Vec<String>, bool)>> = Mutex::new(None);

/// A request older than this was for a chat screen that is gone.
const FRESH: Duration = Duration::from_secs(2);

pub fn set(provider: Provider) {
    let _ = PROVIDER.set(provider);
}

pub fn get() -> Option<Provider> {
    PROVIDER.get().copied()
}

/// Ask the open chat screen to list these names from `start` (its next update shows them).
pub fn request_nicknames(start: usize, names: Vec<String>, force: bool) {
    if let Ok(mut held) = NICKNAMES.lock() {
        *held = Some((Instant::now(), start, names, force));
    }
}

pub fn take_nicknames() -> Option<(usize, Vec<String>, bool)> {
    let taken = NICKNAMES.lock().ok()?.take()?;
    (taken.0.elapsed() <= FRESH).then_some((taken.1, taken.2, taken.3))
}
