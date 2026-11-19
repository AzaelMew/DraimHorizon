//! Chat completions the game side hands to the chat screen's hook (a lower crate).
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The text up to the cursor in; where the word to complete starts (a byte index) and the candidates out.
pub type Provider = fn(&str) -> Option<(usize, Vec<String>)>;

/// The chat input in, and the online names to filter: where the name starts and the matches.
pub type Nicknames = fn(&str, &[String]) -> Option<(usize, Vec<String>)>;

static PROVIDER: OnceLock<Provider> = OnceLock::new();
static ONLINE: Mutex<Option<Snapshot>> = Mutex::new(None);

struct Snapshot {
    at: Instant,
    names: Vec<String>,
    filter: Nicknames,
}

/// The game publishes the names every frame the chat is open and allowed to list them; a snapshot older
/// than this was for a chat screen that is gone, or for a mod that was switched off.
const FRESH: Duration = Duration::from_secs(1);

pub fn set(provider: Provider) {
    let _ = PROVIDER.set(provider);
}

pub fn get() -> Option<Provider> {
    PROVIDER.get().copied()
}

/// The online names and the game's filter for them; called from the game's per-frame chat code.
pub fn publish_nicknames(filter: Nicknames, names: &[String]) {
    if let Ok(mut held) = ONLINE.lock() {
        match held.as_mut() {
            Some(snapshot) if snapshot.names == names => {
                snapshot.at = Instant::now();
                snapshot.filter = filter;
            }
            _ => {
                *held = Some(Snapshot {
                    at: Instant::now(),
                    names: names.to_vec(),
                    filter,
                })
            }
        }
    }
}

/// The names to list for exactly this input, or None when the game does not list names for it (the
/// chat screen then updates as usual and asks the server). Computed from the input each time, so a
/// list is never one keystroke late or for another text.
pub fn nicknames(input: &str) -> Option<(usize, Vec<String>)> {
    let held = ONLINE.lock().ok()?;
    let snapshot = held.as_ref().filter(|s| s.at.elapsed() <= FRESH)?;
    (snapshot.filter)(input, &snapshot.names).filter(|(_, list)| !list.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The snapshot is process-wide: tests take turns.
    static TURN: Mutex<()> = Mutex::new(());

    // The same shape as the game's filter: `/tpa <prefix>`, matched case-insensitively and sorted.
    fn filter(input: &str, names: &[String]) -> Option<(usize, Vec<String>)> {
        let (command, partial) = input.strip_prefix('/')?.split_once(' ')?;
        if command != "tpa" {
            return None;
        }
        let needle = partial.to_ascii_lowercase();
        let mut list: Vec<String> = names
            .iter()
            .filter(|n| n.to_ascii_lowercase().starts_with(&needle))
            .cloned()
            .collect();
        list.sort();
        Some((input.len() - partial.len(), list))
    }

    fn names() -> Vec<String> {
        ["Mark", "Maria", "Marcus", "Mia", "Zed"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn every_keystroke_gets_its_own_fresh_list() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        publish_nicknames(filter, &names());
        let (start, list) = nicknames("/tpa m").unwrap();
        assert_eq!((start, list.len()), (5, 4));
        let (_, list) = nicknames("/tpa ma").unwrap();
        assert_eq!(list, ["Marcus", "Maria", "Mark"]);
        let (_, list) = nicknames("/tpa mar").unwrap();
        assert_eq!(list, ["Marcus", "Maria", "Mark"]);
        let (_, list) = nicknames("/tpa mari").unwrap();
        assert_eq!(list, ["Maria"]);
    }

    #[test]
    fn other_input_and_empty_matches_fall_through_to_the_server() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        publish_nicknames(filter, &names());
        assert!(nicknames("/gamemode c").is_none());
        assert!(nicknames("hello").is_none());
        assert!(nicknames("/tpa q").is_none());
    }

    #[test]
    fn a_snapshot_that_is_not_refreshed_expires() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        publish_nicknames(filter, &names());
        if let Ok(mut held) = ONLINE.lock() {
            held.as_mut().unwrap().at = Instant::now() - FRESH * 2;
        }
        assert!(nicknames("/tpa m").is_none());
    }
}
