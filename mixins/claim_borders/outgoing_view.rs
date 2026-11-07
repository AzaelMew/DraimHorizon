// Async outbound translation, scoped to one live connection; no delayed messages cross servers.
use crate::game::{Game, claim_borders as claims, claim_borders_outgoing as model};
use claim_borders_relay_client::translation::{Language, Translator};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
#[derive(Default)]
struct State {
    scope: String,
    queue: model::Queue,
    russian: Option<Translator>,
    english: Option<Translator>,
    parts: VecDeque<String>,
    clock: Option<Instant>,
    last: u64,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn refresh_translators() {
    state(|s| {
        for worker in [s.russian.take(), s.english.take()].into_iter().flatten() {
            while let Some((id, result)) = worker.poll() {
                s.queue.complete(id, result);
            }
        }
        for message in &mut s.queue.messages {
            if message.result.is_none() {
                message.result = Some(None);
            }
        }
    });
}
pub(super) fn enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::ID, "chat_outgoing_en")
        .as_deref()
        != Some("false")
}
fn scope(game: &Game<'_>) -> Option<String> {
    let r = game.remote.as_ref()?;
    Some(format!("{}#{:032x}", r.session.address, r.profile()))
}
pub(super) fn intercept(game: &mut Game<'_>, text: &str) -> bool {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return false;
    }
    let text = horizon_client::chat::normalize_chat_message(text);
    if text.is_empty() || text.starts_with('/') || !game.chat_abilities().allows(&text) {
        return false;
    }
    let target = crate::game::claim_borders_chat_view::language(game).other();
    let translate = enabled(game) && target.needs_translation(&text);
    let current = scope(game).unwrap();
    let result = state(|s| {
        if s.scope != current {
            *s = State {
                scope: current,
                ..State::default()
            };
        }
        if !translate && s.queue.messages.is_empty() && s.parts.is_empty() {
            return None;
        }
        let now = s
            .clock
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_millis() as u64;
        let id = match s.queue.push(&text, now, translate) {
            Ok(id) => id,
            Err(e) => return Some(Err(e)),
        };
        if translate {
            let worker = match target {
                Language::Russian => &mut s.russian,
                Language::English => &mut s.english,
            };
            if !worker
                .get_or_insert_with(|| {
                    crate::game::claim_borders_chat_view::translator(game, target)
                })
                .submit_until(
                    id,
                    &text,
                    Instant::now() + Duration::from_millis(model::WAIT_MS - 500),
                )
            {
                s.queue.complete(id, None);
            }
        }
        Some(Ok(()))
    });
    if let Some(result) = result {
        game.ui.chat.add_recent(&text);
        if let Err(error) = result {
            game.native_chat(error);
        }
        true
    } else {
        false
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || scope(game).is_none() {
        reset();
        return;
    }
    let current = scope(game).unwrap();
    let (message, failure) = state(|s| {
        if s.scope != current {
            *s = State::default();
            return (None, false);
        }
        let now = s
            .clock
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_millis() as u64;
        for worker in [&s.russian, &s.english].into_iter().flatten() {
            while let Some((id, result)) = worker.poll() {
                s.queue.complete(id, result);
            }
        }
        let mut failure = false;
        if s.parts.is_empty()
            && let Some((text, failed)) = s.queue.pop(now)
        {
            s.parts.extend(model::split(&text));
            failure = failed;
        }
        if now.saturating_sub(s.last) < 1000 {
            return (None, failure);
        }
        let part = s.parts.pop_front();
        if part.is_some() {
            s.last = now;
        }
        (part, failure)
    });
    if failure {
        game.native_chat(crate::game::claim_borders_chat_view::language(game).text(
            "[Перевод] Сервис недоступен: отправляется исходный текст.",
            "[Translation] Service unavailable. Sending the original text.",
        ));
    }
    if let Some(message) = message
        && game.chat_abilities().send_messages
    {
        game.send_signed_chat(&message);
    }
}

fn toggle_rect(size: [f32; 2]) -> [f32; 4] {
    [2.0, size[1] - 14.0, 64.0, 12.0]
}
pub(super) fn draw_toggle(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if game.screen != crate::game::Screen::Chat || !game.native_on(claims::ID) {
        return;
    }
    let rect = toggle_rect([batch.width, batch.height]);
    crate::game::claim_borders_ui_view::button(
        game,
        batch,
        rect,
        match (
            crate::game::claim_borders_chat_view::language(game),
            enabled(game),
        ) {
            (Language::Russian, true) => "RU → EN: вкл.",
            (Language::Russian, false) => "RU → EN: выкл.",
            (Language::English, true) => "EN → RU: on",
            (Language::English, false) => "EN → RU: off",
        },
        enabled(game),
        game.native_cursor(),
    );
}
static TOGGLE_PRESSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) fn toggle_mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !pressed && button == 0 && TOGGLE_PRESSED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        *canceled = true;
        return true;
    }
    if !pressed
        || button != 0
        || game.screen != crate::game::Screen::Chat
        || !game.native_on(claims::ID)
    {
        return false;
    }
    if crate::game::claim_borders_ui_style::contains(
        toggle_rect([screen.rect[2], screen.rect[3]]),
        screen.mouse,
    ) {
        let value = !enabled(game);
        game.native_set_setting(
            claims::ID,
            "chat_outgoing_en",
            if value { "true" } else { "false" },
        );
        TOGGLE_PRESSED.store(true, std::sync::atomic::Ordering::Relaxed);
        *canceled = true;
        true
    } else {
        false
    }
}
