// Collect only /claim info replies matched to the requested chunk and dimension.
use crate::game::{
    Game, claim_borders as claims, claim_borders_claim_observations as model,
    claim_borders_client as client, claim_borders_economy_persistence as disk,
};
use claim_borders_relay_client::translation::Language;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
#[derive(Default)]
struct State {
    scope: String,
    name: String,
    path: PathBuf,
    book: model::Observations,
    pending: VecDeque<(claims::Claim, u64)>,
    breaks: model::BreakEvidence,
    sent: BTreeMap<String, (u64, usize)>,
    frames: Vec<String>,
    revision: u64,
    last_send: u64,
    last_save: u64,
    dirty: bool,
    writable: bool,
    worker: Option<disk::Worker>,
    /// The `/claim info` reply the book just accepted, hidden only if the player asked for that.
    hide_line: Option<String>,
}
/// "Hide /claim info replies" (`hide_claim_info`, off unless the player turned it on). Read each
/// frame and set when the switch is flipped, so it applies at once and never starts on.
static HIDE_INFO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) const HIDE_KEY: &str = "hide_claim_info";
pub(super) fn set_hide_info(on: bool) {
    HIDE_INFO.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub(super) fn hide_info() -> bool {
    HIDE_INFO.load(std::sync::atomic::Ordering::Relaxed)
}
/// The capture hook, once per shown incoming message: true when it is the accepted `/claim info`
/// reply of this delivery and the player chose to hide those. Consumed once, so an identical later
/// line is decided on its own.
pub(super) fn deliver(text: &str) -> bool {
    let text = claims::strip_formatting(text);
    state(|s| s.hide_line.take().is_some_and(|line| line == text)) && hide_info()
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
pub(super) fn flush() {
    state(|s| {
        if s.dirty
            && s.writable
            && let Some(w) = &s.worker
        {
            let _ = w.save(s.path.clone(), s.scope.clone(), s.book.encode(&s.scope));
            w.finish();
        }
    });
}
pub(super) fn reset() {
    flush();
    state(|s| *s = State::default());
}
pub(super) fn records() -> Vec<model::Record> {
    state(|s| {
        s.book
            .records
            .values()
            .filter(|r| r.owner.is_some())
            .cloned()
            .collect()
    })
}
pub(super) fn network_reset() {
    state(|s| s.sent.clear());
}
pub(super) fn clear() {
    state(|s| {
        s.book = model::Observations::default();
        s.frames.clear();
        s.sent.clear();
        s.dirty = true;
    });
}
pub(super) fn command(game: &Game<'_>, command: &str) {
    if command.split_whitespace().collect::<Vec<_>>() != ["claim", "info"] {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let dimension = if p.dimension.contains(':') {
        p.dimension.into()
    } else {
        format!("minecraft:{}", p.dimension)
    };
    let c = claims::Claim {
        dimension,
        x: (p.x.floor() as i32).div_euclid(16) * 16,
        z: (p.z.floor() as i32).div_euclid(16) * 16,
    };
    state(|s| {
        if s.scope.is_empty() {
            return;
        }
        if s.pending.len() >= 8 {
            s.pending.pop_front();
        }
        s.pending.push_back((c, now()));
    });
}
pub(super) fn server(text: &str) {
    let text = claims::strip_formatting(text);
    state(|s| s.hide_line = None);
    let Some((x, z, owner)) = model::reply(text.trim()) else {
        return;
    };
    let time = now();
    let accepted = state(|s| {
        s.pending.retain(|(_, at)| time.saturating_sub(*at) < 10000);
        let Some(i) = s.pending.iter().position(|(c, _)| c.x == x && c.z == z) else {
            return false;
        };
        let (claim, _) = s.pending.remove(i).unwrap();
        let reporter = s.name.clone();
        match s.book.merge(
            model::Record {
                claim,
                owner,
                at: time,
                reporter,
            },
            time,
        ) {
            Ok(changed) => {
                s.dirty |= changed;
                true
            }
            Err(_) => false,
        }
    });
    if accepted && hide_info() {
        // Recorded either way; hidden only when asked for, by this delivery alone.
        state(|s| s.hide_line = Some(text));
    }
}
pub(super) fn receive(peer: &claim_borders_relay_client::Peer, body: &str) {
    state(|s| {
        if !s.scope.is_empty()
            && let Ok(true) = s.book.receive(body, &peer.name, now())
        {
            s.dirty = true;
        }
    });
}
pub(super) fn frame(game: &mut Game<'_>) {
    set_hide_info(
        game.native_setting(claims::SETTINGS_ID, HIDE_KEY)
            .as_deref()
            == Some("true"),
    );
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    let time = now();
    if state(|s| s.scope != scope) {
        flush();
        let Ok(root) = Game::native_config_dir() else {
            return;
        };
        let hash = crate::game::claim_borders_map_history::hash(scope.as_bytes());
        let path = root
            .join("claim_borders")
            .join(format!("observed-land-{hash:016x}.json"));
        let loaded = disk::load(&path, &scope).and_then(|raw| {
            raw.map(|raw| model::Observations::decode(&raw, &scope, time))
                .transpose()
        });
        let name = game.remote.as_ref().unwrap().profile_name.clone();
        let warning = state(|s| {
            *s = State {
                scope,
                path,
                name,
                writable: true,
                worker: disk::Worker::spawn().ok(),
                ..State::default()
            };
            match loaded {
                Ok(Some(book)) => {
                    s.book = book;
                    None
                }
                Ok(None) => None,
                Err(e) => {
                    s.writable = false;
                    Some(e)
                }
            }
        });
        if let Some(e) = warning {
            game.native_chat(&match language() {
                Language::Russian => format!("[Клеймы] Наблюдения не загружены: {e}"),
                Language::English => format!("[Claims] Observations not loaded: {e}"),
            });
        }
    }
    let peers = client::claim_peers(game);
    let (out, warning) = state(|s| {
        if s.book.revision != s.revision {
            s.frames = s.book.frames();
            s.revision = s.book.revision;
        }
        s.sent
            .retain(|code, _| peers.iter().any(|p| p.code == *code));
        let mut warning = None;
        if let Some(w) = &s.worker {
            if let Ok(e) = w.errors.try_recv() {
                s.writable = false;
                warning = Some(e);
            }
        }
        if s.dirty
            && s.writable
            && time.saturating_sub(s.last_save) >= 3000
            && let Some(w) = &s.worker
            && w.save(s.path.clone(), s.scope.clone(), s.book.encode(&s.scope))
        {
            s.dirty = false;
            s.last_save = time;
        }
        if time.saturating_sub(s.last_send) < 350 {
            return (None, warning);
        }
        let out = peers.iter().find_map(|p| {
            let page = s
                .sent
                .get(&p.code)
                .filter(|(r, _)| *r == s.revision)
                .map_or(0, |(_, page)| *page);
            s.frames
                .get(page)
                .map(|body| (p.code.clone(), body.clone(), s.revision, page))
        });
        (out, warning)
    });
    if let Some(e) = warning {
        game.native_chat(&match language() {
            Language::Russian => format!("[Клеймы] Не удалось сохранить наблюдения: {e}"),
            Language::English => format!("[Claims] Could not save observations: {e}"),
        });
    }
    if let Some((code, body, revision, page)) = out
        && client::claim_send(&code, &body)
    {
        state(|s| {
            s.sent.insert(code, (revision, page + 1));
            s.last_send = time;
        });
    }
}

pub(super) fn break_attempt(game: &Game<'_>, pos: Option<[i32; 3]>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    let dimension = if p.dimension.contains(':') {
        p.dimension.into()
    } else {
        format!("minecraft:{}", p.dimension)
    };
    let time = now();
    state(|s| {
        if s.scope != scope {
            s.breaks = model::BreakEvidence::default();
            return;
        }
        if let Some(pos) = pos {
            s.breaks.attempt(&dimension, pos, time);
        } else {
            s.breaks.interrupt(time);
        }
    });
}
pub(super) fn action_bar(game: &Game<'_>, text: &str) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return;
    }
    let text = claims::strip_formatting(text);
    if model::denied_owner(&text).is_none() {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    let dimension = if p.dimension.contains(':') {
        p.dimension.into()
    } else {
        format!("minecraft:{}", p.dimension)
    };
    let time = now();
    state(|s| {
        if s.scope != scope {
            s.breaks = model::BreakEvidence::default();
            return;
        }
        let Some((claim, owner)) = s.breaks.denial(&dimension, &text, time) else {
            return;
        };
        if s.book.records.get(&claim).is_some_and(|r| {
            r.owner.as_deref() == Some(owner.as_str()) && time.saturating_sub(r.at) < 30_000
        }) {
            return;
        }
        if let Ok(changed) = s.book.merge(
            model::Record {
                claim,
                owner: Some(owner),
                at: time,
                reporter: s.name.clone(),
            },
            time,
        ) {
            s.dirty |= changed;
        }
    });
}

fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
#[cfg(test)]
mod hide_info_tests {
    //! `/claim info` replies through the real seam: the chat hook's `server`, then the capture
    //! hook's `deliver`. They are visible unless the player turned the hiding on.
    use super::*;
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    const FREE: &str = "This chunk (x 0 to 15, z 0 to 15) is free: /claim it.";
    const OWNED: &str = "This chunk (x 16 to 31, z 0 to 15) is Alex's.";
    fn fresh() -> std::sync::MutexGuard<'static, ()> {
        let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        state(|s| {
            s.scope = "server#me".into();
            s.name = "Me".into();
        });
        set_hide_info(false);
        guard
    }
    fn ask(x: i32, z: i32) {
        state(|s| {
            s.pending.push_back((
                claims::Claim {
                    dimension: "minecraft:overworld".into(),
                    x,
                    z,
                },
                now(),
            ))
        });
    }
    fn shown(text: &str) -> bool {
        server(text);
        deliver(text)
    }
    #[test]
    fn replies_are_visible_by_default_and_still_recorded() {
        let _g = fresh();
        ask(0, 0);
        ask(16, 0);
        assert!(!shown(FREE));
        assert!(!shown(OWNED));
        assert_eq!(
            state(|s| s.book.records.len()),
            2,
            "both observations are kept"
        );
    }
    #[test]
    fn the_switch_hides_exactly_the_accepted_reply_and_works_at_once() {
        let _g = fresh();
        set_hide_info(true);
        ask(0, 0);
        assert!(shown(FREE), "hidden when asked for");
        assert_eq!(state(|s| s.book.records.len()), 1, "and still recorded");
        // The same text later is a different delivery with nothing asked: shown.
        assert!(!shown(FREE));
        // Switched off between the chat hook and the capture hook: visible immediately.
        ask(16, 0);
        server(OWNED);
        set_hide_info(false);
        assert!(!deliver(OWNED));
    }
    #[test]
    fn unrequested_malformed_or_other_scope_replies_are_never_hidden() {
        let _g = fresh();
        set_hide_info(true);
        assert!(!shown(FREE), "nobody asked for this chunk");
        ask(0, 0);
        assert!(
            !shown("This chunk (x 5 to 20, z 0 to 15) is free: /claim it."),
            "malformed"
        );
        assert!(
            !shown("This chunk (x 0 to 15, z 0 to 15) is free"),
            "partial"
        );
        reset(); // another server or account
        assert!(!shown(FREE), "no scope, nothing pending");
    }
}
