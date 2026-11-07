use crate::game::{
    Game, claim_borders as claims, claim_borders_auto_trust as model,
    claim_borders_client as client,
};
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct State {
    scope: String,
    path: PathBuf,
    trust: model::Trust,
    clock: Instant,
    issuing: bool,
    writable: bool,
    last: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: String::new(),
            path: PathBuf::new(),
            trust: model::Trust::default(),
            clock: Instant::now(),
            issuing: false,
            writable: true,
            last: 0,
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::ID, "auto_trust").as_deref() != Some("false")
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn manual(game: &Game<'_>, command: &str) {
    if !command.trim().starts_with("untrust ") {
        return;
    }
    let friends = client::friend_ids();
    state(|s| {
        if s.issuing {
            return;
        }
        s.trust.manual(command, &friends);
        if s.writable && !s.scope.is_empty() {
            let _ = s.trust.save(&s.path, &s.scope);
        }
    });
    let _ = game;
}
pub(super) fn server(text: &str) {
    let text = claims::strip_formatting(text);
    let accepted = state(|s| {
        let accepted = s.trust.reply(text.trim());
        if accepted && s.writable {
            let _ = s.trust.save(&s.path, &s.scope);
        }
        accepted
    });
    if accepted {
        crate::game::claim_borders_economy_client::silence(&text);
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() || !enabled(game) {
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    if state(|s| s.scope != scope) {
        let Ok(root) = Game::native_config_dir() else {
            return;
        };
        let hash = crate::game::claim_borders_map_history::hash(scope.as_bytes());
        let path = root
            .join("claim_borders")
            .join(format!("trust-{hash:016x}.json"));
        let loaded = model::Trust::load(&path, &scope);
        let error = state(|s| {
            *s = State {
                scope,
                path,
                ..State::default()
            };
            match loaded {
                Ok(t) => {
                    s.trust = t;
                    None
                }
                Err(e) => {
                    s.writable = false;
                    s.trust.failed = true;
                    Some(e)
                }
            }
        });
        if let Some(e) = error {
            game.native_chat(
                &match crate::game::claim_borders_chat_view::current_language() {
                    claim_borders_relay_client::translation::Language::Russian => {
                        format!("[Друзья] Авто /trust остановлен: {e}")
                    }
                    claim_borders_relay_client::translation::Language::English => {
                        format!("[Friends] Auto /trust stopped: {e}")
                    }
                },
            );
        }
    }
    let friends = client::friend_ids();
    let online = game
        .remote_tab_players()
        .map(|(p, _, _)| p.into_iter().map(|p| (p.uuid, p.name)).collect::<Vec<_>>())
        .unwrap_or_default();
    let command = state(|s| {
        let now = s.clock.elapsed().as_millis() as u64;
        if now.saturating_sub(s.last) < 1500 {
            return None;
        }
        let c = s.trust.next(&friends, &online, now);
        if c.is_some() {
            s.last = now;
            s.issuing = true;
        }
        c
    });
    if let Some(c) = command {
        game.send_unattended_command(&c);
        state(|s| s.issuing = false);
    }
}
