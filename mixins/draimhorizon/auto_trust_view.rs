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
/// Which automatic `/trust` replies may be hidden: kept apart from `State`, which is rebuilt when
/// the scope loads, so a typed command's protection survives that. Same bounded policy as the
/// other automatic checks: the next two incoming messages after a send that really went out, and
/// only the reply that acknowledges that very request.
static REPLIES: Mutex<crate::game::claim_borders_claim_replies::Replies> =
    Mutex::new(crate::game::claim_borders_claim_replies::Replies::new());
fn replies<T>(f: impl FnOnce(&mut crate::game::claim_borders_claim_replies::Replies) -> T) -> T {
    f(&mut REPLIES.lock().unwrap_or_else(|e| e.into_inner()))
}
fn clock() -> u64 {
    crate::game::claim_borders_economy_client::now()
}
/// The capture hook, once per shown incoming message: true when it is the acknowledgement of the
/// mod's own automatic `/trust` and the routine-reply hiding is on.
pub(super) fn deliver(text: &str) -> bool {
    let quiet = crate::game::claim_borders_economy_client::quiet_enabled();
    replies(|r| r.deliver(&claims::strip_formatting(text), quiet, clock()))
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::SETTINGS_ID, "auto_trust")
        .as_deref()
        != Some("false")
}
pub(super) fn reset() {
    state(|s| *s = State::default());
    replies(|r| r.leave());
}
/// A typed /trust or /untrust ends any automatic attribution and holds the next automatic one,
/// so its reply is shown; only /untrust also changes the grants.
fn note_typed(command: &str) {
    if command.starts_with("trust ") || command.starts_with("untrust ") {
        replies(|r| r.manual(clock()));
    }
}
pub(super) fn manual(game: &Game<'_>, command: &str) {
    let command = command.trim();
    note_typed(command);
    if !command.starts_with("untrust ") {
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
        // The grant is recorded whatever happens; whether the line is hidden is decided when it
        // is shown, and only if it answers an automatic request that is still attributable.
        replies(|r| r.recognize(&text));
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() || !enabled(game) {
        // No connection, the mod or auto trust is off: no attribution carries over.
        replies(|r| r.leave());
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
        return;
    };
    replies(|r| r.bind(&scope));
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
    // One automatic /trust at a time, and none while a typed one may still be answered.
    let busy = replies(|r| r.busy(clock()));
    let command = state(|s| {
        let now = s.clock.elapsed().as_millis() as u64;
        if busy || now.saturating_sub(s.last) < 1500 {
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
        // Hide the reply only if the command really went out.
        let allowed = game.remote.is_some()
            && game.command_prompt.is_none()
            && game.chat_abilities().allows(&format!("/{c}"));
        game.send_unattended_command(&c);
        let sent = allowed && game.command_prompt.is_none();
        state(|s| s.issuing = false);
        if sent {
            replies(|r| r.arm(clock()));
        }
    }
}
#[cfg(test)]
mod reply_tests {
    //! The seam the chat hooks use on the real module state: `server` (chat line hook), then
    //! `deliver` (chat-capture hook) once per shown message.
    use super::*;
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    const FRIEND: u128 = 0xabc;
    fn fresh() -> std::sync::MutexGuard<'static, ()> {
        let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        // The test state has no cache file: never write one.
        state(|s| s.writable = false);
        guard
    }
    /// An automatic /trust Steve that really went out.
    fn automatic() {
        state(|s| {
            s.trust.pending = Some(model::Pending {
                uuid: FRIEND,
                name: "Steve".into(),
                at: 0,
            })
        });
        assert!(replies(|r| r.arm(clock())));
    }
    fn shown(text: &str) -> bool {
        server(text);
        deliver(text)
    }
    const GRANT: &str = "Steve may now build and open things on your land.";
    const ALREADY: &str = "You already trust Steve.";
    #[test]
    fn an_automatic_trusts_acknowledgement_is_hidden_once_and_the_grant_is_recorded() {
        for reply in [GRANT, ALREADY] {
            let _g = fresh();
            automatic();
            assert!(shown(reply), "{reply}");
            assert!(
                state(|s| s.trust.granted.contains(&FRIEND)),
                "grant recorded"
            );
            // No stale exact-text list: the same line later is a new delivery.
            assert!(!shown(reply));
            assert!(!shown(reply));
        }
    }
    #[test]
    fn a_typed_trust_during_a_pending_automatic_one_is_visible_and_still_recorded() {
        let _g = fresh();
        automatic();
        note_typed("trust Steve");
        assert!(!shown(GRANT), "the typed command's reply is shown");
        assert!(
            state(|s| s.trust.granted.contains(&FRIEND)),
            "the grant is still recorded"
        );
        // The next automatic send waits while the typed reply may still be coming.
        assert!(replies(|r| r.busy(clock())));
        assert!(
            !shown(ALREADY),
            "and a later identical line is not hidden either"
        );
    }
    #[test]
    fn a_typed_untrust_cancels_the_automatic_attribution() {
        let _g = fresh();
        automatic();
        note_typed("untrust Steve");
        assert!(!shown(GRANT));
    }
    #[test]
    fn errors_other_requests_and_resets_never_hide() {
        let _g = fresh();
        automatic();
        assert!(!shown("Unknown command"));
        assert!(!shown("Claims are off on this server."));
        assert!(
            !shown("Alex may now build and open things on your land."),
            "another player"
        );
        let _g2 = ();
        reset(); // disconnect, account or scope change
        assert!(!shown(GRANT), "nothing is pending any more");
        // A request that was never sent arms nothing.
        state(|s| {
            s.trust.pending = Some(model::Pending {
                uuid: FRIEND,
                name: "Steve".into(),
                at: 0,
            })
        });
        assert!(!shown(GRANT), "no window was armed");
    }
}
