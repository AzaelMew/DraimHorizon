use crate::game::claim_borders_ui_view as ui;
// Native chat cards. The original signed message/history path remains untouched.
use crate::game::claim_borders_chat_archive as archive;
use crate::game::claim_borders_chat_preferences as options;
use crate::game::{Game, Screen, claim_borders as claims, claim_borders_chat_feed as model};
use claim_borders_relay_client::translation::Translator;
use horizon_client::{
    hud::{HudBatch, rgba},
    text::Text,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub modern: bool,
    pub translate: bool,
    pub preferences: options::Preferences,
}
struct State {
    feed: model::Feed,
    archive: Option<archive::Worker>,
    archive_scope: Option<String>,
    archive_generation: u64,
    archive_loading: bool,
    archive_snapshot: Option<serde_json::Value>,
    archive_saved: Instant,
    tabs: model::Tabs,
    tab_hits: Vec<([f32; 4], model::Channel)>,
    nickname_query: Option<(String, usize, usize, Vec<String>)>,
    started: Instant,
    settings: Settings,
    loaded: bool,
    translator: Option<Translator>,
    requested: HashSet<u64>,
    retry: HashMap<u64, (u64, u8)>,
    translation_failed: bool,
    heads: HashMap<String, (u128, String)>,
    hits: Vec<options::Hit>,
    hit_viewport: [f32; 2],
    bounds: Option<[f32; 4]>,
    priority: crate::game::claim_borders_chat_priority::Preferences,
    friends: HashSet<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            feed: model::Feed::default(),
            archive: None,
            archive_scope: None,
            archive_generation: 0,
            archive_loading: false,
            archive_snapshot: None,
            archive_saved: Instant::now(),
            tabs: model::Tabs::default(),
            tab_hits: Vec::new(),
            nickname_query: None,
            started: Instant::now(),
            settings: Settings {
                modern: true,
                translate: true,
                preferences: options::Preferences::default(),
            },
            loaded: false,
            translator: None,
            requested: HashSet::new(),
            retry: HashMap::new(),
            translation_failed: false,
            heads: HashMap::new(),
            hits: Vec::new(),
            hit_viewport: [0.0, 0.0],
            bounds: None,
            priority: crate::game::claim_borders_chat_priority::Preferences::default(),
            friends: HashSet::new(),
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
}
pub(super) fn settings(game: &Game<'_>) -> Settings {
    state(|s| {
        if !s.loaded {
            s.settings.modern =
                game.native_setting(claims::ID, "chat_modern").as_deref() != Some("false");
            s.settings.translate =
                game.native_setting(claims::ID, "chat_translate").as_deref() != Some("false");
            s.settings.preferences = game
                .native_setting(claims::ID, "chat_preferences")
                .and_then(|raw| options::Preferences::decode(&raw).ok())
                .unwrap_or_default();
            s.priority = game
                .native_setting(claims::ID, "chat_priority")
                .and_then(|r| {
                    crate::game::claim_borders_chat_priority::Preferences::decode(&r).ok()
                })
                .unwrap_or_default();
            s.loaded = true;
        }
        s.settings
    })
}
pub(super) fn priority_settings(
    game: &Game<'_>,
) -> crate::game::claim_borders_chat_priority::Preferences {
    let _ = settings(game);
    state(|s| s.priority.clone())
}
pub(super) fn priority_configure(
    game: &Game<'_>,
    prefs: crate::game::claim_borders_chat_priority::Preferences,
) {
    game.native_set_setting(claims::ID, "chat_priority", &prefs.encode());
    state(|s| s.priority = prefs);
}
fn priority_rule(name: &str) -> Option<crate::game::claim_borders_chat_priority::Rule> {
    state(|s| {
        s.priority
            .rule(name, s.friends.contains(&name.to_ascii_lowercase()))
    })
}
pub(super) fn configure(game: &Game<'_>, preferences: options::Preferences) {
    let preferences = preferences.normalized();
    game.native_set_setting(claims::ID, "chat_preferences", &preferences.encode());
    state(|s| {
        s.settings.preferences = preferences;
        s.feed.reconfigure(preferences);
        s.feed.scroll = 0;
        s.hits.clear();
        s.bounds = None;
    });
}
pub(super) fn toggle(game: &Game<'_>, translation: bool) {
    let current = settings(game);
    let (key, value) = if translation {
        ("chat_translate", !current.translate)
    } else {
        ("chat_modern", !current.modern)
    };
    game.native_set_setting(claims::ID, key, if value { "true" } else { "false" });
    state(|s| {
        if translation {
            s.settings.translate = value;
            if !value {
                s.translator = None;
                s.requested.clear();
                s.retry.clear();
                s.translation_failed = false;
            }
        } else {
            s.settings.modern = value;
        }
    });
}
fn archive_scope(game: &Game<'_>) -> Option<String> {
    if let Some(remote) = &game.remote {
        return Some(format!(
            "server:{}#{:032x}",
            remote.session.address.to_ascii_lowercase(),
            remote.profile()
        ));
    }
    game.native_player()?;
    let uuid = game.level.player_uuid?;
    (!game.world_root.as_os_str().is_empty())
        .then(|| format!("world:{}#{uuid:?}", game.world_root.to_string_lossy()))
}
fn poll_archive(s: &mut State) {
    while let Some(result) = s.archive.as_ref().and_then(archive::Worker::poll) {
        if s.archive_scope.as_deref() != Some(&result.scope)
            || s.archive_generation != result.generation
        {
            continue;
        }
        s.archive_loading = false;
        match result
            .entries
            .and_then(|v| s.feed.restore(&v, s.settings.preferences))
        {
            Ok(()) => {}
            Err(error) => eprintln!("[Horizon] {error}"),
        }
    }
}
fn save_archive(s: &mut State, wait: bool) {
    if s.archive_loading {
        return;
    }
    let Some(scope) = s.archive_scope.clone() else {
        return;
    };
    let Some(worker) = &s.archive else {
        return;
    };
    s.archive_saved = Instant::now();
    if worker.failed() {
        s.archive_snapshot = None;
    }
    let snapshot = s.feed.archive();
    if s.archive_snapshot.as_ref() != Some(&snapshot) && worker.save(scope, snapshot.clone(), wait)
    {
        s.archive_snapshot = Some(snapshot);
        s.archive_saved = Instant::now();
    }
}
fn finish_archive(s: &mut State) {
    poll_archive(s);
    if s.archive_loading {
        if let (Some(worker), Some(scope)) = (&s.archive, &s.archive_scope)
            && !worker.merge(scope.clone(), s.feed.archive())
        {
            eprintln!("[Horizon] Cannot queue pending chat history merge");
        }
        return;
    }
    save_archive(s, true);
}
/// Called on every native frame, also when the modern UI/translation is disabled.
pub(super) fn frame(game: &Game<'_>) {
    if !game.native_on(claims::ID) {
        if state(|s| s.archive_scope.is_some()) {
            reset();
        }
        return;
    }
    let current = archive_scope(game);
    let changed = state(|s| s.archive_scope != current);
    if changed {
        reset();
        let _ = settings(game);
        state(|s| {
            if s.archive.is_none() {
                match Game::native_config_dir().and_then(|root| {
                    archive::Worker::new(root.join("claim_borders").join("chat-history"))
                }) {
                    Ok(worker) => s.archive = Some(worker),
                    Err(_) => eprintln!("[Horizon] Cannot initialize local chat history"),
                }
            }
            s.archive_generation = s.archive_generation.wrapping_add(1);
            s.archive_scope = current;
            if let (Some(worker), Some(scope)) = (&s.archive, &s.archive_scope) {
                s.archive_loading = worker.load(scope.clone(), s.archive_generation);
            }
        });
    }
    state(|s| {
        poll_archive(s);
        if s.archive_saved.elapsed().as_secs() >= 2 {
            save_archive(s, false);
        }
    });
}
/// Drain queued writes before the client's normal exit.
pub(super) fn flush() {
    state(|s| {
        finish_archive(s);
        if let Some(worker) = &s.archive
            && !worker.flush()
        {
            eprintln!("[Horizon] Chat archive flush did not complete successfully");
        }
    });
}
pub(super) fn reset() {
    state(|s| {
        finish_archive(s);
        s.archive_scope = None;
        s.archive_snapshot = None;
        s.archive_loading = false;
        s.feed.clear();
        s.tabs = model::Tabs::default();
        s.tab_hits.clear();
        s.nickname_query = None;
        s.hits.clear();
        s.bounds = None;
        s.requested.clear();
        s.heads.clear();
        s.translator = None;
        s.retry.clear();
        s.translation_failed = false;
    });
}
pub(super) fn capture_content(
    game: &Game<'_>,
    text: &horizon_client::text::Text,
    source: horizon_client::chat::ChatSource,
) {
    use horizon_client::chat::ChatSource;
    let component = match source {
        ChatSource::Server => Some((text.component(), true)),
        ChatSource::Player
            if text.translation_key().is_some_and(|key| {
                matches!(
                    key,
                    "commands.message.display.incoming" | "commands.message.display.outgoing"
                )
            }) =>
        {
            Some((text.component(), false))
        }
        _ => None,
    };
    capture_inner(game, &text.plain(), component.as_ref(), false);
}
pub(super) fn capture(game: &Game<'_>, text: &str) {
    capture_inner(game, text, None, false);
}
pub(super) fn capture_local(game: &Game<'_>, text: &str) {
    capture_inner(game, text, None, true);
}
fn capture_inner(
    game: &Game<'_>,
    text: &str,
    component: Option<&(serde_json::Value, bool)>,
    local: bool,
) {
    if !game.native_on(claims::ID) {
        return;
    }
    let _ = settings(game);
    frame(game);
    state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        let preferences = s.settings.preferences;
        let anchor = if s.feed.scroll > 0 {
            s.feed
                .entries
                .iter()
                .rev()
                .filter(|e| main_entry(e, preferences) && s.tabs.selected.shows(e))
                .nth(s.feed.scroll)
                .map(|e| e.id)
        } else {
            None
        };
        let added = match component {
            Some((component, true)) => s.feed.push_component(text, component, now, preferences),
            Some((component, false)) => {
                s.feed
                    .push_player_component(text, component, now, preferences)
            }
            None if local => s.feed.push_local(text, now, preferences),
            None => s.feed.push_with(text, now, preferences),
        };
        if added.is_some()
            && let Some(entry) = s.feed.entries.back()
            && entry.kind != model::Kind::Whisper(model::Whisper::Outgoing)
            && (entry.channel != model::Channel::Whisper
                || game.ui.chat.shows(horizon_client::chat::ChatSource::Player))
        {
            s.tabs.record(entry.channel);
        }
        if let Some(anchor) = anchor {
            if let Some(position) = s
                .feed
                .entries
                .iter()
                .rev()
                .filter(|e| main_entry(e, preferences) && s.tabs.selected.shows(e))
                .position(|e| e.id == anchor)
            {
                if position > 0 {
                    s.feed.scroll = position;
                }
            }
        }
        let retained: HashSet<_> = s.feed.entries.iter().map(|e| e.id).collect();
        s.requested.retain(|id| retained.contains(id));
        s.retry.retain(|id, _| retained.contains(id));
    });
}
fn event_entry(entry: &model::Entry, p: options::Preferences) -> bool {
    p.notifications != options::NotificationStyle::Off
        && match entry.kind {
            model::Kind::Presence => p.presence,
            model::Kind::Death => p.deaths,
            _ => false,
        }
}
fn main_entry(entry: &model::Entry, p: options::Preferences) -> bool {
    entry.kind.is_chat()
        || entry.kind == model::Kind::System
        || (p.notifications == options::NotificationStyle::Inline && event_entry(entry, p))
}
pub(super) fn reply(game: &mut Game<'_>, body: &str) {
    if body.is_empty() {
        game.native_chat("Использование /r <текст> или /reply <текст>");
        return;
    }
    let Some(name) =
        state(|s| s.feed.last_whisper_sender.clone()).filter(|_| game.remote.is_some())
    else {
        game.native_chat("В этом подключении ещё не было входящих ЛС.");
        return;
    };
    let message = format!("/msg {name} {body}");
    if message.encode_utf16().count() > 256 {
        game.native_chat("Слишком длинное личное сообщение.");
        return;
    }
    game.send_chat(&message);
}
pub(super) fn prepare_nickname_list(game: &mut Game<'_>, force: bool) {
    use horizon_client::screen_draw::Measure;
    if !game.native_on(claims::ID)
        || game.screen != Screen::Chat
        || !game.chat_abilities().send_commands
        || game.pvp.chat_search
    {
        return;
    }
    let Some(names) = game.remote_online_names() else {
        return;
    };
    let Some(screen) = game.ui.chat_screen.as_ref() else {
        return;
    };
    let input = &screen.input;
    let query = (input.value.clone(), input.cursor, input.highlight, names);
    let changed = state(|s| {
        let changed = s.nickname_query.as_ref() != Some(&query);
        s.nickname_query = Some(query.clone());
        changed
    });
    if (!changed && !force)
        || input.cursor != input.value.chars().count()
        || input.highlight != input.cursor
        || screen.suggestions_visible()
    {
        return;
    }
    let Some((start, names)) = model::nickname_suggestions(&query.0, &query.3) else {
        return;
    };
    if let Some(screen) = game.ui.chat_screen.as_mut() {
        screen.draimhorizons_nickname_list(&Measure(&game.gui.atlas.font), start, names, force);
    }
}
pub(super) fn complete_nickname(game: &mut Game<'_>, key: winit::keyboard::KeyCode) -> bool {
    if key == winit::keyboard::KeyCode::Tab {
        prepare_nickname_list(game, true);
    }
    // Let the native list handle Tab/Shift+Tab, arrows, clicks and Escape.
    false
}
fn label(entry: &model::Entry) -> Cow<'_, str> {
    if let Some(notice) = entry.auction {
        notice.label().into()
    } else if let model::Kind::Whisper(direction) = entry.kind {
        format!("{} {}", direction.label(), entry.name).into()
    } else if entry.name.is_empty() {
        "Система".into()
    } else {
        entry.name.as_str().into()
    }
}
fn in_chat_area(game: &Game<'_>, point: [f32; 2], viewport: [f32; 2]) -> bool {
    if game.screen != Screen::Chat || !game.native_on(claims::ID) || !settings(game).modern {
        return false;
    }
    state(|s| {
        s.hit_viewport == viewport && s.bounds.is_some_and(|rect| options::contains(rect, point))
    })
}
pub(super) fn scroll(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    amount: f64,
    canceled: &mut bool,
) {
    if !in_chat_area(game, screen.mouse, [screen.rect[2], screen.rect[3]]) {
        return;
    }
    state(|s| {
        let count = s
            .feed
            .entries
            .iter()
            .filter(|e| main_entry(e, s.settings.preferences) && s.tabs.selected.shows(e))
            .count();
        if amount > 0.0 {
            s.feed.scroll = (s.feed.scroll + 3).min(count.saturating_sub(1));
        } else if amount < 0.0 {
            s.feed.scroll = s.feed.scroll.saturating_sub(3);
        }
    });
    *canceled = true;
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if game.screen != Screen::Chat || !game.native_on(claims::ID) || !settings(game).modern {
        return;
    }
    let area = in_chat_area(game, screen.mouse, [screen.rect[2], screen.rect[3]]);
    let legacy = screen.mouse[1] >= (screen.rect[3] - 210.0).max(0.0)
        && screen.mouse[1] < screen.rect[3] - 32.0;
    // Suppress invisible vanilla links even when cards have moved to another corner.
    if area || legacy {
        *canceled = true;
    }
    if button == 0 && area {
        state(|s| {
            if let Some((_, tab)) = s
                .tab_hits
                .iter()
                .find(|(r, _)| options::contains(*r, screen.mouse))
            {
                s.tabs.selected = *tab;
                s.feed.scroll = 0;
                s.hits.clear();
            } else if let Some(id) = options::hit_at(&s.hits, screen.mouse) {
                s.feed.toggle_original(id);
            }
        });
    }
}

fn text(
    game: &Game<'_>,
    batch: &mut HudBatch,
    value: &str,
    position: [f32; 2],
    rgb: u32,
    alpha: f32,
) {
    let [_, r, g, b] = ui::ink(rgb).to_be_bytes();
    batch.text_styled(
        &game.gui.atlas,
        &Text::literal(value),
        position[0],
        position[1],
        rgba(r, g, b, (alpha * 255.0) as u8),
        false,
    );
}
fn fade(entry: &model::Entry, now: u64, focused: bool, duration: u16) -> f32 {
    if focused {
        return 1.0;
    }
    if entry.archived {
        return 0.0;
    }
    let age = now.saturating_sub(entry.at) as f32;
    let life = f32::from(duration) * 1000.0;
    ((life - age) / 1500.0).clamp(0.0, 1.0)
}
fn draw_card(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    entry: &model::Entry,
    rect: [f32; 4],
    rows: &[String],
    alpha: f32,
    head: Option<(u128, String)>,
    focused: bool,
    compact: bool,
    opacity: f32,
) {
    let [x, y, w, h] = rect;
    let color = entry
        .auction
        .map(|n| n.color())
        .or_else(|| matches!(entry.kind, model::Kind::Whisper(_)).then_some(0xd1a7ff))
        .or_else(|| priority_rule(&entry.name).map(|r| r.color))
        .unwrap_or(match entry.kind {
            model::Kind::Presence if entry.joined == Some(false) => 0xe1c69e,
            model::Kind::Presence => 0xa2d0b7,
            model::Kind::Death => 0xdfa8b5,
            _ => ui::preferences(game).accent.color(),
        });
    let background = ((alpha * 255.0 * opacity) as u32) << 24 | 0x151a26;
    ui::fill(game, batch, rect, background);
    ui::fill(
        game,
        batch,
        [x, y, 2.0, h],
        ((alpha * 255.0) as u32) << 24 | color,
    );
    let mut tx = x + 6.0;
    if entry.name.is_empty() {
        ui::icon_tinted(
            game,
            batch,
            ui::Icon::Info,
            [tx + 2.0, y + 6.0, 10.0, 10.0],
            ((alpha * 255.0) as u32) << 24 | color,
        );
        tx += 17.0;
    }
    if !entry.name.is_empty() {
        let (uuid, skin) =
            head.unwrap_or_else(|| (0, horizon_client::tab_list::default_skin(0).into()));
        let skin = game.face_skin(uuid, &skin);
        let mut face = Vec::new();
        horizon_client::tab_list::face(
            &mut face,
            &skin,
            (x + 6.0) as i32,
            (y + 5.0) as i32,
            12,
            true,
            matches!(entry.name.as_str(), "Dinnerbone" | "Grumm"),
            rgba(255, 255, 255, (alpha * 255.0) as u8),
        );
        horizon_client::hud_layout::paint(batch, &game.gui.atlas, &face);
        tx += 17.0;
    }
    let name = label(entry);
    if compact {
        if let Some(row) = rows.first() {
            text(
                game,
                batch,
                row,
                [tx, y + 6.0],
                if entry.kind == model::Kind::Death {
                    0xdfa8b5
                } else {
                    0xe7eaf2
                },
                alpha,
            );
        }
    } else if entry.kind.is_chat() {
        let prefix = format!(
            "{name}{}: ",
            if entry.show_original {
                " [ориг.]"
            } else {
                ""
            }
        );
        for (i, row) in rows.iter().enumerate() {
            let line_y = y + 6.0 + i as f32 * 10.0;
            if i == 0
                && let Some(body) = row.strip_prefix(&prefix)
            {
                text(game, batch, &prefix, [tx, line_y], color, alpha);
                text(
                    game,
                    batch,
                    body,
                    [tx + game.native_text_width(&prefix), line_y],
                    0xe7eaf2,
                    alpha,
                );
            } else {
                text(game, batch, row, [tx, line_y], 0xe7eaf2, alpha);
            }
        }
    } else {
        text(game, batch, &name, [tx, y + 4.0], color, alpha);
        if entry.show_original {
            text(
                game,
                batch,
                "ОРИГ",
                [x + w - 29.0, y + 4.0],
                0x7e899f,
                alpha,
            );
        } else if entry.translated.is_some() && !focused {
            text(game, batch, "RU", [x + w - 17.0, y + 4.0], 0x7e899f, alpha);
        }
        for (i, row) in rows.iter().enumerate() {
            text(
                game,
                batch,
                row,
                [tx, y + 15.0 + i as f32 * 10.0],
                0xe7eaf2,
                alpha,
            );
        }
    }
}
#[derive(Clone, Copy)]
struct PaintContext {
    preferences: options::Preferences,
    layout: options::Layout,
    now: u64,
    focused: bool,
    translate: bool,
    line_limit: usize,
}
fn paint_region<'a>(
    game: &mut Game<'_>,
    batch: &mut HudBatch,
    entries: impl Iterator<Item = &'a model::Entry>,
    region: [f32; 4],
    context: PaintContext,
    hits: &mut Vec<options::Hit>,
) {
    let p = context.preferences;
    let mut y = if context.layout.top {
        region[1]
    } else {
        region[1] + region[3]
    };
    for e in entries {
        let motion = ui::preferences(game).motion;
        let alpha = fade(e, context.now, context.focused, p.duration)
            * crate::game::claim_borders_ui_style::entrance(
                context.now.saturating_sub(e.created_at) as f32 / 1000.0,
                motion,
            );
        if alpha <= 0.0 {
            continue;
        }
        let compact = e.auction.is_none()
            && (e.kind == model::Kind::Death
                || !e.kind.is_chat()
                    && matches!(
                        p.notifications,
                        options::NotificationStyle::Compact | options::NotificationStyle::Inline
                    ));
        let inline = e.kind.is_chat() || compact;
        let count = if e.repetitions > 1 {
            format!(" ×{}", e.repetitions)
        } else {
            String::new()
        };
        let body = if compact {
            format!("{}{count}", model::event_summary(e, context.translate))
        } else if inline {
            let label = label(e);
            format!(
                "{label}{}: {}{count}",
                if e.show_original { " [ориг.]" } else { "" },
                e.body(context.translate)
            )
        } else {
            format!("{}{count}", e.body(context.translate))
        };
        let inset = 29.0;
        let mut rows = if compact {
            vec![model::single_line(&body, region[2] - inset, |s| {
                game.native_text_width(s)
            })]
        } else {
            model::wrap(&body, region[2] - inset, |s| game.native_text_width(s))
        };
        let available = if context.layout.top {
            region[1] + region[3] - y
        } else {
            y - region[1]
        };
        let padding = if inline { 10.0 } else { 20.0 };
        let max_rows = (((available - padding) / 10.0).floor() as usize).min(context.line_limit);
        if max_rows == 0 {
            break;
        }
        if rows.len() > max_rows {
            rows.truncate(max_rows);
            if let Some(last) = rows.last_mut() {
                last.push('…');
            }
        }
        let h = if compact {
            20.0
        } else {
            (padding + rows.len() as f32 * 10.0).max(if inline { 22.0 } else { 30.0 })
        };
        if h > available {
            break;
        }
        let card_y = if context.layout.top {
            let at = y;
            y += h + 4.0;
            at
        } else {
            y -= h;
            let at = y;
            y -= 4.0;
            at
        };
        // Stable cards while reading/clicking; appearance uses alpha only.
        let rect = [region[0], card_y, region[2], h];
        let head = state(|s| s.heads.get(&e.name).cloned());
        let start = batch.vertices.len();
        draw_card(
            game,
            batch,
            e,
            rect,
            &rows,
            alpha,
            head,
            context.focused || !context.translate,
            compact,
            p.opacity,
        );
        batch.clip_since(
            start,
            region[0],
            region[1],
            region[0] + region[2],
            region[1] + region[3],
        );
        if let Some(rect) =
            options::intersection(context.layout.screen(rect), context.layout.screen(region))
        {
            hits.push(options::Hit { id: e.id, rect });
        }
    }
}
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !game.native_on(claims::ID) || !settings(game).modern {
        return false;
    }
    let focused = game.screen == Screen::Chat;
    if (game.ui.hide_gui && !focused)
        || !game.ui.chat.shows(horizon_client::chat::ChatSource::Server)
    {
        state(|s| {
            s.hits.clear();
            s.bounds = None;
        });
        return true;
    }
    let config = settings(game);
    let friends = crate::game::claim_borders_client::friend_ids()
        .into_iter()
        .map(|(_, n)| n.to_ascii_lowercase())
        .collect();
    state(|s| s.friends = friends);
    let priority = priority_settings(game);
    let p = config.preferences;
    let show_players = game.ui.chat.shows(horizon_client::chat::ChatSource::Player);
    let (entries, now, scroll, channel, unread) = state(|s| {
        if focused && s.feed.scroll == 0 {
            s.tabs.read();
        }
        if config.translate {
            let now = s.started.elapsed().as_millis() as u64;
            let worker = s.translator.get_or_insert_with(Translator::default);
            while let Some((id, value)) = worker.poll() {
                if let Some(value) = value {
                    s.feed.translate(id, value);
                    s.translation_failed = false;
                    s.retry.remove(&id);
                } else {
                    s.requested.remove(&id);
                    let attempts = s.retry.get(&id).map_or(1, |(_, n)| n.saturating_add(1));
                    s.retry
                        .insert(id, (now + 15_000 * (1u64 << attempts.min(4)), attempts));
                    s.translation_failed = true;
                }
            }
            for e in s.feed.entries.iter().rev().take(64) {
                if !e.archived
                    && (e.kind == model::Kind::Message
                        || (show_players && matches!(e.kind, model::Kind::Whisper(_)))
                        || (e.kind == model::Kind::Death && event_entry(e, p)))
                    && e.translated.is_none()
                    && !s.requested.contains(&e.id)
                    && s.retry.get(&e.id).is_none_or(|(at, _)| now >= *at)
                    && worker.submit(e.id, &e.original)
                {
                    s.requested.insert(e.id);
                }
            }
        }
        (
            s.feed.entries.iter().cloned().collect::<Vec<_>>(),
            s.started.elapsed().as_millis() as u64,
            s.feed.scroll,
            if focused {
                s.tabs.selected
            } else {
                model::Channel::All
            },
            model::Channel::TABS.map(|tab| s.tabs.unread(tab)),
        )
    });
    let viewport = [batch.width, batch.height];
    let layout = options::Layout::new(
        p,
        if focused {
            [viewport[0], (viewport[1] - 18.0).max(60.0)]
        } else {
            viewport
        },
    );
    if !entries.is_empty()
        && let Some((players, _, _)) = game.remote_tab_players()
    {
        state(|s| {
            if s.heads.len() > 256 {
                s.heads.clear();
            }
            for player in players {
                s.heads.insert(player.name, (player.uuid, player.skin));
            }
        });
    }
    let context = PaintContext {
        preferences: p,
        layout,
        now,
        focused,
        line_limit: 24,
        translate: config.translate && !game.modifiers.alt_key(),
    };
    let mut hits = Vec::new();
    let start = batch.vertices.len();
    let main = entries
        .iter()
        .rev()
        .filter(|e| main_entry(e, p) && channel.shows(e) && (show_players || !e.kind.is_chat()))
        .skip(if focused { scroll } else { 0 });
    let mut region = layout.main;
    let mut tab_hits = Vec::new();
    if focused && region[3] >= 34.0 {
        let width = region[2] / 4.0;
        let mouse = game.native_cursor().map(|n| n / layout.scale);
        for (i, tab) in model::Channel::TABS.into_iter().enumerate() {
            let label = ["Все", "ЛС", "Пати", "Торги"][i];
            let caption = match unread[i] {
                0 => label.to_owned(),
                1..=99 => format!("{label} {}", unread[i]),
                _ => format!("{label} 99+"),
            };
            let r = [region[0] + i as f32 * width, region[1], width - 1.0, 14.0];
            ui::button(game, batch, r, &caption, tab == channel, mouse);
            tab_hits.push((layout.screen(r), tab));
        }
        region[1] += 16.0;
        region[3] -= 16.0;
    }
    if priority.tray && channel == model::Channel::All {
        let mut names = HashSet::new();
        let mut pinned: Vec<_> = entries
            .iter()
            .rev()
            .filter(|e| {
                !e.archived
                    && now.saturating_sub(e.at) < u64::from(priority.seconds) * 1000
                    && (e.auction.is_some()
                        || (show_players
                            && e.kind == model::Kind::Message
                            && priority_rule(&e.name).is_some_and(|r| r.pinned)
                            && names.insert(e.name.to_ascii_lowercase())))
            })
            .collect();
        pinned.sort_by_key(|e| e.auction.is_none());
        pinned.truncate(priority.rows as usize);
        if !pinned.is_empty() {
            let height = (pinned.len() as f32 * 42.0).min(region[3] * 0.45);
            let pin = [region[0], region[1], region[2], height];
            let mut pinned_context = context;
            pinned_context.layout.top = true;
            pinned_context.line_limit = 2;
            pinned_context.preferences.duration = priority.seconds;
            paint_region(
                game,
                batch,
                pinned.into_iter(),
                pin,
                pinned_context,
                &mut hits,
            );
            region[1] += height + 3.0;
            region[3] = (region[3] - height - 3.0).max(20.0);
        }
    }
    paint_region(game, batch, main, region, context, &mut hits);
    if channel == model::Channel::All
        && let Some(region) = layout.events
    {
        paint_region(
            game,
            batch,
            entries.iter().rev().filter(|e| event_entry(e, p)),
            region,
            context,
            &mut hits,
        );
    }
    batch.scale_since(start, 0.0, 0.0, layout.scale, layout.scale);
    let [x, y, w, h] = layout.bounds;
    batch.clip_since(start, x, y, x + w, y + h);
    if game.screen != Screen::Playing {
        let mouse = game.native_cursor();
        if let Some(id) = options::hit_at(&hits, mouse) {
            if let Some(e) = entries.iter().find(|e| {
                e.id == id && matches!(e.kind, model::Kind::Death | model::Kind::Presence)
            }) {
                ui::tooltip(
                    game,
                    batch,
                    &format!("{} · {}", e.name, e.body(context.translate)),
                    mouse,
                );
            }
        }
    }
    if focused {
        let hint = if scroll > 0 {
            format!("История ↑ {scroll} · ЛКМ: оригинал / перевод")
        } else {
            let status = state(|s| {
                if !config.translate {
                    "RU: выкл."
                } else if s.translation_failed {
                    "RU: сервис недоступен · повтор автоматически"
                } else {
                    "RU: автоперевод"
                }
            });
            format!("{status} · ЛКМ: оригинал / перевод")
        };
        let start = batch.vertices.len();
        text(game, batch, &hint, [x + 2.0, y + h + 4.0], 0xa99eff, 1.0);
        batch.clip_since(start, x, y + h, x + w, y + h + 14.0);
    }
    state(|s| {
        s.hits = hits;
        s.tab_hits = tab_hits;
        s.hit_viewport = viewport;
        s.bounds = Some(layout.bounds);
    });
    true
}
