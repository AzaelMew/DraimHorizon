// The seed map's found POIs in the game: kept for the world (server and account) and the seed,
// shared with friends. A click on the seed map marks or unmarks a POI (`toggle`); the player's own
// marks and unmarks are sent to every friend online over the relay as `[DHF1]` frames (a page at
// a time, again when they change or the friend comes online), and friends' frames are merged in
// (`receive`): the newer setting of a POI wins, and the map shows who marked it. Loaded when the
// world is joined, not when the map opens, so a friend's mark arrives while the map is closed too.
use crate::game::{
    Game, claim_borders as claims, claim_borders_client as client, claim_borders_seed_map as model,
    claim_borders_seed_poi as poi,
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
#[derive(Default)]
struct State {
    scope: String,
    found: Option<poi::Found>,
    error: Option<String>,
    /// Friends' changes not saved yet (the player's own clicks are saved at once).
    dirty: bool,
    last_save: u64,
    /// The frames of the player's own settings, made again when they change.
    frames: Vec<String>,
    revision: u64,
    built: bool,
    /// Per friend (relay code): the revision sent and the next page of it to send.
    sent: BTreeMap<String, (u64, usize)>,
    last_send: u64,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
fn now() -> u64 {
    crate::game::claim_borders_economy_client::now()
}
fn root() -> Option<std::path::PathBuf> {
    // The same folder as before (`native_data_dir`), kept by the map's settings namespace.
    ROOT.get().cloned()
}
static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
fn save(s: &mut State) {
    let (Some(found), Some(root)) = (&s.found, root()) else {
        return;
    };
    // A failed save keeps the edit in memory and shows the error on the map.
    s.error = found.save(&root).err();
    if s.error.is_none() {
        s.dirty = false;
        s.last_save = now();
    }
}
/// The found POIs of the world the seed map shows, with the error of loading or saving them.
pub(super) fn with<T>(scope: &str, f: impl FnOnce(Option<&poi::Found>, Option<&str>) -> T) -> T {
    state(|s| {
        if s.scope == scope {
            f(s.found.as_ref(), s.error.as_deref())
        } else {
            f(None, None)
        }
    })
}
/// Marks or unmarks a POI as this player, saved at once and sent to friends.
pub(super) fn toggle(scope: &str, key: poi::Poi) {
    state(|s| {
        if s.scope != scope {
            return;
        }
        let Some(found) = &mut s.found else {
            return;
        };
        match found.toggle(key, now()) {
            Ok(_) => save(s),
            Err(e) => s.error = Some(e),
        }
    });
}
/// A friend's frame (the relay names the friend).
pub(super) fn receive(peer: &claim_borders_relay_client::Peer, body: &str) {
    state(|s| {
        if let Some(found) = &mut s.found
            && let Ok(true) = found.receive(body, &peer.name, now())
        {
            s.dirty = true;
        }
    });
}
/// The relay connected again: everything is sent to every friend once more.
pub(super) fn network_reset() {
    state(|s| s.sent.clear());
}
/// Saves friends' changes not saved yet (leaving the world, closing the game).
pub(super) fn flush() {
    state(|s| {
        if s.dirty {
            save(s);
        }
    });
}
pub(super) fn reset() {
    flush();
    state(|s| *s = State::default());
}
/// Every tick: loads the found POIs of the world joined, saves friends' changes now and then,
/// and sends the player's own settings to friends, a frame at a time.
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    let _ = ROOT.set(game.native_data_dir(claims::SETTINGS_ID));
    if state(|s| s.scope != scope) {
        flush();
        let loaded = root()
            .ok_or_else(|| "no data folder".to_owned())
            .and_then(|root| poi::Found::load(&root, &scope, model::SEED));
        state(|s| {
            *s = State {
                scope: scope.clone(),
                ..State::default()
            };
            match loaded {
                Ok(found) => s.found = Some(found),
                // An unreadable save is left as it is: nothing is shared or written over it.
                Err(e) => s.error = Some(e),
            }
        });
    }
    let time = now();
    if state(|s| s.dirty && time.saturating_sub(s.last_save) >= 3000) {
        flush();
    }
    if game.remote.is_none() {
        return;
    }
    let peers = client::friend_peers(game);
    let out = state(|s| {
        let found = s.found.as_ref()?;
        if !s.built || found.revision() != s.revision {
            s.frames = found.frames();
            s.revision = found.revision();
            s.built = true;
        }
        s.sent
            .retain(|code, _| peers.iter().any(|p| p.code == *code));
        // One frame a second: marks are not urgent, and a relay with a message quota closes the
        // connection of a client over it (the claim pages and positions share that quota).
        if time.saturating_sub(s.last_send) < 1000 {
            return None;
        }
        peers.iter().find_map(|p| {
            let page = s
                .sent
                .get(&p.code)
                .filter(|(r, _)| *r == s.revision)
                .map_or(0, |(_, page)| *page);
            s.frames
                .get(page)
                .map(|body| (p.code.clone(), body.clone(), s.revision, page))
        })
    });
    if let Some((code, body, revision, page)) = out
        && client::claim_send(&code, &body)
    {
        state(|s| {
            s.sent.insert(code, (revision, page + 1));
            s.last_send = time;
        });
    }
}
