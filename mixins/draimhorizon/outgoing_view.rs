// Async outbound translation and a native language-settings screen. No network in UI hooks.
use crate::game::claim_borders_ui_view as ui;
use crate::game::native_hooks::NativeScreenView;
use crate::game::{Game, Screen, claim_borders as claims, claim_borders_outgoing as model};
use claim_borders_relay_client::translation::Translator;
use std::{
    collections::VecDeque,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Default)]
struct State {
    scope: String,
    queue: model::Queue,
    worker: Option<Translator>,
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
#[derive(Default)]
struct EditorState {
    requested: bool,
    chat: Option<horizon_client::chat::ChatScreen>,
    draft_shown: bool,
    editor: Option<model::TranslationScreen>,
    action: Option<model::TranslationControl>,
    size: [f32; 2],
    error: bool,
}
fn editor<T>(f: impl FnOnce(&mut EditorState) -> T) -> T {
    static EDITOR: OnceLock<Mutex<EditorState>> = OnceLock::new();
    f(&mut EDITOR
        .get_or_init(|| Mutex::new(EditorState::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
static PRESSED: AtomicU8 = AtomicU8::new(0);
pub(super) fn reset() {
    state(|s| *s = State::default());
    editor(|s| *s = EditorState::default());
}
pub(super) fn refresh_translators() {
    state(|s| {
        if let Some(worker) = s.worker.take() {
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
    game.native_setting(claims::SETTINGS_ID, "chat_outgoing_en")
        .as_deref()
        != Some("false")
}
fn scope(game: &Game<'_>) -> Option<String> {
    let r = game.remote.as_ref()?;
    Some(format!("{}#{:032x}", r.session.address, r.profile()))
}
pub(super) fn targets(game: &Game<'_>) -> model::TranslationTargets {
    crate::game::claim_borders_chat_view::settings(game).translation
}
pub(super) fn intercept(game: &mut Game<'_>, text: &str) -> bool {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return false;
    }
    let text = horizon_client::chat::normalize_chat_message(text);
    if text.is_empty() || text.starts_with('/') || !game.chat_abilities().allows(&text) {
        return false;
    }
    let ui_language = crate::game::claim_borders_chat_view::language(game);
    let prefs = targets(game);
    let translate = enabled(game)
        && prefs
            .outgoing
            .should_translate(prefs.policy(true).source, &text);
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
        let id = match s.queue.push_in(&text, now, translate, ui_language) {
            Ok(id) => id,
            Err(e) => return Some(Err(e)),
        };
        if translate
            && !s
                .worker
                .get_or_insert_with(|| {
                    crate::game::claim_borders_chat_view::translator(
                        game,
                        prefs.outgoing,
                        prefs.policy(true),
                    )
                })
                .submit_until(
                    id,
                    &text,
                    Instant::now() + Duration::from_millis(model::WAIT_MS - 500),
                )
        {
            s.queue.complete(id, None);
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
fn close_editor(game: &mut Game<'_>) {
    let (chat, draft_shown) = editor(|s| {
        let saved = (s.chat.take(), s.draft_shown);
        *s = EditorState::default();
        saved
    });
    game.native_mod_screen_close(claims::ID);
    // Move the original native text field back, preserving its caret and selection.
    // Keeping it in UiState while Mod is open would let tick_in_bed_chat destroy it.
    game.ui.chat_screen = chat;
    game.ui.chat_draft_shown = draft_shown;
    if game.screen == Screen::Playing
        && game.native_player().is_some()
        && game.ui.chat_screen.is_some()
    {
        game.screen = Screen::Chat;
        game.capture(false);
    }
}
fn service_editor(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        if editing(game) {
            close_editor(game);
        }
        editor(|s| *s = EditorState::default());
        return;
    }
    let requested = editor(|s| std::mem::take(&mut s.requested));
    if requested && game.screen == Screen::Chat && !game.ui.in_bed_chat {
        let chat = game.ui.chat_screen.take();
        let draft_shown = game.ui.chat_draft_shown;
        game.screen = Screen::Playing;
        if game.native_mod_screen_open(claims::ID) {
            let draft = targets(game);
            editor(|s| {
                s.editor = Some(model::TranslationScreen::new(draft));
                s.chat = chat;
                s.draft_shown = draft_shown;
                s.error = false;
            });
        } else {
            game.ui.chat_screen = chat;
            game.screen = Screen::Chat;
        }
    }
    if editor(|s| s.editor.is_some()) && game.native_mod_screen() != Some(claims::ID) {
        close_editor(game);
        return;
    }
    let pending = editor(|s| {
        s.action
            .take()
            .map(|action| (action, s.editor.as_ref().map(|e| e.draft)))
    });
    if let Some((action, Some(draft))) = pending {
        match action {
            model::TranslationControl::Cancel => close_editor(game),
            model::TranslationControl::Save => {
                match crate::game::claim_borders_chat_view::set_translation_targets(game, draft) {
                    Ok(()) => close_editor(game),
                    Err(_) => editor(|s| s.error = true),
                }
            }
            _ => {}
        }
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    service_editor(game);
    if !game.native_on(claims::ID) || scope(game).is_none() {
        state(|s| *s = State::default());
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
        if let Some(worker) = &s.worker {
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
pub(super) fn draw_toggle(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if game.screen != Screen::Chat || !game.native_on(claims::ID) {
        return;
    }
    let rect = model::translation_button([batch.width, batch.height]);
    let label = targets(game).label();
    ui::button_base(
        game,
        batch,
        rect,
        &label,
        enabled(game),
        game.native_cursor(),
    );
    // The generic button reserves 11 px for padding; compact direction tags need 8.
    let label = ui::clip(game, &label, rect[2] - 8.0);
    ui::text(
        game,
        batch,
        &label,
        [rect[0] + 4.0, rect[1] + 2.0],
        0xe3ebf3,
        false,
    );
    if crate::game::claim_borders_ui_style::contains(rect, game.native_cursor()) {
        let language = crate::game::claim_borders_chat_view::language(game);
        ui::tooltip(
            game,
            batch,
            language.text(
                "ЛКМ: перевод исходящих вкл./выкл. · ПКМ: языки (? = авто)",
                "Left click: outgoing on/off · Right click: languages (? = auto)",
            ),
            game.native_cursor(),
        );
    }
}
pub(super) fn editing(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(claims::ID) && editor(|s| s.editor.is_some())
}
pub(super) fn render_menu(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &NativeScreenView<'_>,
) -> bool {
    if !editing(game) || !game.native_on(claims::ID) {
        return false;
    }
    let language = crate::game::claim_borders_chat_view::language(game);
    ui::fill(game, batch, screen.rect, 0xff111520);
    ui::text(
        game,
        batch,
        language.text("Языки перевода", "Translation languages"),
        [10.0, 12.0],
        0xe3ebf3,
        true,
    );
    editor(|s| {
        s.size = [screen.rect[2], screen.rect[3]];
        let Some(menu) = &s.editor else {
            return;
        };
        for widget in menu.layout(s.size, language) {
            if let Some((heading, value)) = widget.text.split_once('\n') {
                ui::button(game, batch, widget.rect, "", widget.selected, screen.mouse);
                for (line, y) in [
                    (heading, widget.rect[1] + 4.0),
                    (value, widget.rect[1] + 17.0),
                ] {
                    ui::text(
                        game,
                        batch,
                        &ui::clip(game, line, widget.rect[2] - 8.0),
                        [widget.rect[0] + 4.0, y],
                        0xe3ebf3,
                        false,
                    );
                }
            } else {
                let label = if widget.control == model::TranslationControl::CommonOnly {
                    format!(
                        "{}: {}",
                        widget.text,
                        if menu.draft.common_only {
                            language.text("вкл.", "on")
                        } else {
                            language.text("выкл.", "off")
                        }
                    )
                } else {
                    widget.text
                };
                ui::button(
                    game,
                    batch,
                    widget.rect,
                    &label,
                    widget.selected,
                    screen.mouse,
                );
            }
        }
        if s.error {
            let message = language.text(
                "Не сохранено. Проверьте translation_preferences; выбор сохранён в меню.",
                "Not saved. Check translation_preferences; your selections are retained.",
            );
            ui::text(
                game,
                batch,
                &ui::clip(game, message, screen.rect[2] - 20.0),
                [10.0, 24.0],
                0xff7777,
                false,
            );
        }
    });
    true
}
pub(super) fn key(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if !editing(game) {
        return false;
    }
    editor(|s| {
        let Some(menu) = &mut s.editor else {
            return;
        };
        match key {
            "key.keyboard.escape" => s.action = Some(model::TranslationControl::Cancel),
            "key.keyboard.enter" | "key.keyboard.kp.enter" => {
                s.action = Some(model::TranslationControl::Save)
            }
            "key.keyboard.tab" => {
                let field = menu.field.next();
                menu.apply(model::TranslationControl::Field(field), s.size);
            }
            "key.keyboard.left" | "key.keyboard.page.up" => {
                menu.apply(model::TranslationControl::Previous, s.size)
            }
            "key.keyboard.right" | "key.keyboard.page.down" => {
                menu.apply(model::TranslationControl::Next, s.size)
            }
            _ => {}
        }
    });
    *canceled = true;
    true
}
pub(super) fn scroll(game: &Game<'_>, amount: f64, canceled: &mut bool) -> bool {
    if !editing(game) {
        return false;
    }
    editor(|s| {
        if let Some(menu) = &mut s.editor {
            menu.apply(
                if amount > 0.0 {
                    model::TranslationControl::Previous
                } else {
                    model::TranslationControl::Next
                },
                s.size,
            );
        }
    });
    *canceled = true;
    true
}
pub(super) fn toggle_mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !(0..=1).contains(&button) {
        if editing(game) {
            *canceled = true;
            return true;
        }
        return false;
    }
    let bit = 1 << button;
    if !pressed && PRESSED.fetch_and(!bit, Ordering::Relaxed) & bit != 0 {
        *canceled = true;
        return true;
    }
    if editing(game) {
        if pressed {
            PRESSED.fetch_or(bit, Ordering::Relaxed);
            if button == 0 {
                let language = crate::game::claim_borders_chat_view::language(game);
                editor(|s| {
                    let size = [screen.rect[2], screen.rect[3]];
                    if let Some(menu) = &mut s.editor
                        && let Some(control) = menu.hit(size, language, screen.mouse)
                    {
                        match control {
                            model::TranslationControl::Save | model::TranslationControl::Cancel => {
                                s.action = Some(control)
                            }
                            _ => {
                                menu.apply(control, size);
                                s.error = false;
                            }
                        }
                    }
                });
            }
        }
        *canceled = true;
        return true;
    }
    if !pressed || game.screen != Screen::Chat || !game.native_on(claims::ID) {
        return false;
    }
    if !crate::game::claim_borders_ui_style::contains(
        model::translation_button([screen.rect[2], screen.rect[3]]),
        screen.mouse,
    ) {
        return false;
    }
    if button == 1 {
        editor(|s| s.requested = true);
    } else {
        let value = !enabled(game);
        game.native_set_setting(
            claims::SETTINGS_ID,
            "chat_outgoing_en",
            if value { "true" } else { "false" },
        );
    }
    PRESSED.fetch_or(bit, Ordering::Relaxed);
    *canceled = true;
    true
}
