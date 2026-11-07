// Bootstrap once, remember confirmed ownership, and announce land to every mod peer in this realm.
use crate::game::{
    Game, claim_borders as claims, claim_borders_claim_directory as model,
    claim_borders_client as client,
};
use claim_borders_relay_client::translation::Language;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct State {
    scope: String,
    own: u128,
    name: String,
    path: PathBuf,
    directory: model::Directory,
    writable: bool,
    clock: Instant,
    pending: VecDeque<(bool, String, i32, i32, u64)>,
    sent: BTreeMap<String, (u64, usize)>,
    frames: Vec<String>,
    revision: u64,
    warning: Option<String>,
    cursor: usize,
    pull: bool,
    wanted: BTreeSet<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: String::new(),
            own: 0,
            name: String::new(),
            path: PathBuf::new(),
            directory: model::Directory::default(),
            writable: true,
            clock: Instant::now(),
            pending: VecDeque::new(),
            sent: BTreeMap::new(),
            frames: Vec::new(),
            revision: 0,
            warning: None,
            cursor: 0,
            pull: false,
            wanted: BTreeSet::new(),
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
fn save(s: &mut State) {
    if s.writable
        && let Err(e) = s.directory.save(&s.path, &s.scope)
    {
        s.warning = Some(match language() {
            Language::Russian => format!("[Клеймы] Кеш не сохранён: {e}"),
            Language::English => format!("[Claims] Cache not saved: {e}"),
        });
    }
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn network_reset() {
    state(|s| s.sent.clear());
}
pub(super) fn own() -> Option<Vec<claims::Claim>> {
    state(|s| s.directory.records.get(&s.own).map(|r| r.claims.clone()))
}
pub(super) fn known(uuid: u128) -> bool {
    state(|s| s.directory.records.contains_key(&uuid))
}
pub(super) fn others() -> Vec<(String, Vec<claims::Claim>)> {
    state(|s| {
        s.directory
            .records
            .iter()
            .filter(|(id, _)| **id != s.own)
            .map(|(_, r)| (r.name.clone(), r.claims.clone()))
            .collect()
    })
}
pub(super) fn clear_others() {
    state(|s| {
        let own = s.own;
        s.directory.forget_others(own);
        s.pull = true;
        save(s);
    });
}
pub(super) fn command(game: &Game<'_>, command: &str) {
    let add = match command.trim() {
        "claim" => true,
        "unclaim" => false,
        _ => return,
    };
    let Some(p) = game.native_player() else {
        return;
    };
    state(|s| {
        let now = s.clock.elapsed().as_millis() as u64;
        if s.pending.len() >= 16 {
            s.pending.pop_front();
        }
        s.pending.push_back((
            add,
            p.dimension.into(),
            p.x.floor() as i32 & !15,
            p.z.floor() as i32 & !15,
            now,
        ));
    });
}
pub(super) fn server(text: &str) {
    if text.len() > 262144 {
        return;
    }
    let text = claims::strip_formatting(text);
    let text = text.trim();
    let complete = claims::with_state(|s| s.shared_claims());
    state(|s| {
        if s.scope.is_empty() {
            return;
        }
        if text.starts_with("Your chunks (")
            && let Some(land) = complete
        {
            let own = s.own;
            let key = s
                .directory
                .records
                .get(&own)
                .map_or(String::new(), |r| r.key.clone());
            match s.directory.local(own, &s.name, &key, land) {
                Ok(true) => save(s),
                Err(e) => s.warning = Some(e),
                _ => {}
            }
            return;
        }
        let Some((add, x, z)) = model::confirmation(text) else {
            return;
        };
        let now = s.clock.elapsed().as_millis() as u64;
        s.pending.retain(|p| now.saturating_sub(p.4) < 15000);
        let Some(i) = s
            .pending
            .iter()
            .position(|p| p.0 == add && p.2 == x && p.3 == z)
        else {
            return;
        };
        let (_, dimension, _, _, _) = s.pending.remove(i).unwrap();
        let own = s.own;
        let Some(record) = s.directory.records.get(&own).cloned() else {
            return;
        };
        let mut land = record.claims;
        let claim = claims::Claim { dimension, x, z };
        if add {
            if !land.contains(&claim) {
                land.push(claim);
            }
        } else {
            land.retain(|c| *c != claim);
        }
        match s.directory.local(own, &s.name, &record.key, land.clone()) {
            Ok(true) => {
                save(s);
                claims::with_state(|r| r.restore(land));
            }
            Err(e) => s.warning = Some(e),
            _ => {}
        }
    });
}
pub(super) fn receive(peer: &claim_borders_relay_client::Peer, body: &str, tab: &[(u128, String)]) {
    if !tab
        .iter()
        .any(|(id, name)| *id == peer.uuid && name.eq_ignore_ascii_case(&peer.name))
    {
        return;
    }
    state(|s| {
        if body == "[DHC1] ?" {
            s.sent.remove(&peer.code);
            return;
        }
        if s.scope.is_empty() || peer.uuid == s.own {
            return;
        }
        let now = s.clock.elapsed().as_millis() as u64;
        match s
            .directory
            .receive(peer.uuid, &peer.name, &peer.code, body, now)
        {
            Ok(true) => save(s),
            Err(e) => {
                s.warning = Some(format!(
                    "{} {}",
                    language().text("[Клеймы]", "[Claims]"),
                    model::Directory::error_text(&e, language())
                ))
            }
            _ => {}
        }
    });
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() || game.native_player().is_none() {
        return;
    }
    let Some((scope, own)) = client::scope(game) else {
        return;
    };
    let cold = state(|s| s.scope != scope);
    if cold {
        let root = match Game::native_config_dir() {
            Ok(p) => p.join("claim_borders"),
            Err(_) => return,
        };
        let hash = crate::game::claim_borders_map_history::hash(scope.as_bytes());
        let path = root.join(format!("land-{hash:016x}.json"));
        let loaded = model::Directory::load(&path, &scope);
        let name = game.remote.as_ref().unwrap().session.name.clone();
        let (land, error) = state(|s| {
            *s = State {
                scope,
                own,
                name,
                path,
                ..State::default()
            };
            match loaded {
                Ok(d) => s.directory = d,
                Err(e) => {
                    s.writable = false;
                    s.warning = Some(match language() {
                        Language::Russian => format!("[Клеймы] Кеш повреждён: {e}"),
                        Language::English => format!("[Claims] Cache is corrupted: {e}"),
                    });
                }
            }
            (
                s.directory.records.get(&own).map(|r| r.claims.clone()),
                s.warning.take(),
            )
        });
        if let Some(error) = error {
            game.native_chat(&error);
        }
        if let Some(land) = land {
            claims::with_state(|s| s.restore(land));
        } else {
            claims::with_state(|s| s.begin_refresh());
            game.send_unattended_command("claims");
        }
    }
    let identity = client::claim_identity();
    let peers = client::claim_peers(game);
    let (out, warning) = state(|s| {
        if identity.len() == 24
            && let Some(record) = s.directory.records.get(&s.own).cloned()
            && (record.key != identity || record.name != s.name)
        {
            if let Ok(true) = s.directory.local(s.own, &s.name, &identity, record.claims) {
                save(s);
            }
        }
        if let Some(record) = s.directory.records.get(&s.own)
            && record.revision != s.revision
        {
            match model::Directory::frames(record) {
                Ok(frames) => {
                    s.frames = frames;
                    s.revision = record.revision;
                }
                Err(e) => s.warning = Some(e),
            }
        }
        s.sent
            .retain(|code, _| peers.iter().any(|p| p.code == *code));
        if s.pull {
            s.wanted.extend(peers.iter().map(|p| p.code.clone()));
            s.pull = false;
        }
        s.wanted
            .retain(|code| peers.iter().any(|p| p.code == *code));
        if let Some(code) = s.wanted.iter().next().cloned() {
            return (Some((code, "[DHC1] ?".into(), 0, 0)), s.warning.take());
        }
        let mut out = None;
        if !s.frames.is_empty() && !peers.is_empty() {
            for n in 0..peers.len() {
                let i = (s.cursor + n) % peers.len();
                let p = &peers[i];
                let page = s
                    .sent
                    .get(&p.code)
                    .filter(|(r, _)| *r == s.revision)
                    .map_or(0, |(_, p)| *p);
                if page < s.frames.len() {
                    out = Some((p.code.clone(), s.frames[page].clone(), s.revision, page));
                    s.cursor = if page + 1 == s.frames.len() { i + 1 } else { i };
                    break;
                }
            }
        }
        (out, s.warning.take())
    });
    if let Some(warning) = warning {
        game.native_chat(&warning);
    }
    if let Some((code, body, revision, page)) = out
        && client::claim_send(&code, &body)
    {
        state(|s| {
            if revision == 0 {
                s.wanted.remove(&code);
            } else {
                s.sent.insert(code, (revision, page + 1));
            }
        });
    }
}

fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
