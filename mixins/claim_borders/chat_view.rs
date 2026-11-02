// Native chat cards. The original signed message/history path remains untouched.
use crate::game::{Game, Screen, claim_borders as claims, claim_borders_chat_feed as model};
use claim_borders_relay_client::translation::Translator;
use horizon_client::{
    hud::{HudBatch, rgba},
    text::Text,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub modern: bool,
    pub translate: bool,
}
struct State {
    feed: model::Feed,
    started: Instant,
    settings: Settings,
    loaded: bool,
    translator: Option<Translator>,
    requested: HashSet<u64>,
    heads: HashMap<String, (u128, String)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            feed: model::Feed::default(),
            started: Instant::now(),
            settings: Settings {
                modern: true,
                translate: true,
            },
            loaded: false,
            translator: None,
            requested: HashSet::new(),
            heads: HashMap::new(),
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
            s.loaded = true;
        }
        s.settings
    })
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
            }
        } else {
            s.settings.modern = value;
        }
    });
}
pub(super) fn reset() {
    state(|s| {
        s.feed.clear();
        s.requested.clear();
        s.heads.clear();
        s.translator = None;
    });
}
pub(super) fn capture(game: &Game<'_>, text: &str) {
    if !game.native_on(claims::ID) {
        return;
    }
    let _ = settings(game);
    state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        if s.feed.scroll > 0 {
            s.feed.scroll = s.feed.scroll.saturating_add(1);
        }
        s.feed.push(text, now);
        let retained: HashSet<_> = s.feed.entries.iter().map(|e| e.id).collect();
        s.requested.retain(|id| retained.contains(id));
    });
}
fn in_chat_area(game: &Game<'_>, point: [f32; 2], size: [f32; 2]) -> bool {
    game.screen == Screen::Chat
        && game.native_on(claims::ID)
        && settings(game).modern
        && point[0] >= 4.0
        && point[0] < size[0] - 4.0
        && point[1] >= (size[1] - 210.0).max(0.0)
        && point[1] < size[1] - 32.0
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
            .filter(|e| matches!(e.kind, model::Kind::Message | model::Kind::System))
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
    canceled: &mut bool,
) {
    // Vanilla hit boxes describe the old layout. Never run an invisible link/command.
    if in_chat_area(game, screen.mouse, [screen.rect[2], screen.rect[3]]) {
        *canceled = true;
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
    let [_, r, g, b] = rgb.to_be_bytes();
    batch.text_styled(
        &game.gui.atlas,
        &Text::literal(value),
        position[0],
        position[1],
        rgba(r, g, b, (alpha * 255.0) as u8),
        false,
    );
}
fn fade(entry: &model::Entry, now: u64, focused: bool) -> f32 {
    if focused {
        return 1.0;
    }
    let age = now.saturating_sub(entry.at) as f32;
    let life = if entry.kind == model::Kind::Presence {
        7000.0
    } else {
        12000.0
    };
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
) {
    let [x, y, w, h] = rect;
    let color = match entry.kind {
        model::Kind::Presence if entry.joined == Some(false) => 0xf3b777,
        model::Kind::Presence => 0x74d9a4,
        model::Kind::Death => 0xf17e98,
        _ => 0xaaa2ff,
    };
    let background = ((alpha * 220.0) as u32) << 24 | 0x151a26;
    game.native_fill(batch, rect, background);
    game.native_fill(
        batch,
        [x, y, 2.0, h],
        ((alpha * 255.0) as u32) << 24 | color,
    );
    let mut tx = x + 6.0;
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
    let name = if entry.name.is_empty() {
        "Система"
    } else {
        &entry.name
    };
    if entry.kind == model::Kind::Message {
        let prefix = format!("{name}: ");
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
        text(game, batch, name, [tx, y + 4.0], color, alpha);
        if entry.translated.is_some() && !focused {
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
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !game.native_on(claims::ID) || !settings(game).modern {
        return false;
    }
    let focused = game.screen == Screen::Chat;
    if game.ui.hide_gui && !focused {
        return true;
    }
    if !game.ui.chat.shows(horizon_client::chat::ChatSource::Server) {
        return true;
    }
    let mut config = settings(game);
    if game.modifiers.alt_key() {
        config.translate = false;
    }
    let show_players = game.ui.chat.shows(horizon_client::chat::ChatSource::Player);
    let (entries, now, scroll) = state(|s| {
        if config.translate {
            let worker = s.translator.get_or_insert_with(Translator::default);
            while let Some((id, value)) = worker.poll() {
                if let Some(value) = value {
                    s.feed.translate(id, value);
                }
            }
            for e in s.feed.entries.iter().rev().take(64) {
                if matches!(e.kind, model::Kind::Message | model::Kind::Death)
                    && e.translated.is_none()
                    && !s.requested.contains(&e.id)
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
        )
    });
    if entries.is_empty() {
        return true;
    }
    // Use the same cached skin slots as the tab list, including downloaded skins.
    if let Some((players, _, _)) = game.remote_tab_players() {
        state(|s| {
            if s.heads.len() > 256 {
                s.heads.clear();
            }
            for p in players {
                s.heads.insert(p.name, (p.uuid, p.skin));
            }
        });
    }
    let wide = batch.width >= 420.0;
    let main_w = if wide {
        (batch.width * 0.60).min(320.0)
    } else {
        batch.width - 16.0
    };
    let bottom = batch.height - 42.0;
    let top = (bottom - if focused { 180.0 } else { 150.0 }).max(4.0);
    let mut y = bottom;
    let mut shown = 0;
    for e in entries
        .iter()
        .rev()
        .filter(|e| matches!(e.kind, model::Kind::Message | model::Kind::System))
        .skip(if focused { scroll } else { 0 })
    {
        let alpha = fade(e, now, focused);
        if alpha <= 0.0 {
            continue;
        }
        let text_body = if e.kind == model::Kind::Message {
            format!("{}: {}", e.name, e.body(config.translate))
        } else {
            e.body(config.translate).to_owned()
        };
        let inset = if e.name.is_empty() { 12.0 } else { 29.0 };
        if e.kind == model::Kind::Message && !show_players {
            continue;
        }
        let mut rows = model::wrap(&text_body, main_w - inset, |s| game.native_text_width(s));
        let max_rows =
            (((y - top - 24.0) / 10.0).floor() as usize).min(if focused { 12 } else { 6 });
        if max_rows == 0 {
            break;
        }
        if rows.len() > max_rows {
            rows.truncate(max_rows);
            if let Some(last) = rows.last_mut() {
                last.push('…');
            }
        }
        let h = if e.kind == model::Kind::Message {
            (10.0 + rows.len() as f32 * 10.0).max(22.0)
        } else {
            20.0 + rows.len() as f32 * 10.0
        };
        if y - h < top {
            break;
        }
        y -= h;
        let slide = (1.0 - (now.saturating_sub(e.at) as f32 / 160.0).clamp(0.0, 1.0)) * 8.0;
        let head = state(|s| s.heads.get(&e.name).cloned());
        draw_card(
            game,
            batch,
            e,
            [8.0 - slide, y, main_w, h],
            &rows,
            alpha,
            head,
            focused || !config.translate,
        );
        y -= 4.0;
        shown += 1;
        if shown >= if focused { 12 } else { 5 } {
            break;
        }
    }
    if focused && scroll > 0 {
        text(
            game,
            batch,
            &format!("История ↑ {scroll}"),
            [10.0, bottom + 4.0],
            0xa99eff,
            1.0,
        );
    }
    let side_x = if wide { main_w + 14.0 } else { 8.0 };
    let side_w = if wide {
        (batch.width - side_x - 8.0).min(170.0)
    } else {
        main_w
    };
    let mut sy = if wide { bottom } else { (y - 4.0).max(top) };
    for kind in [model::Kind::Presence, model::Kind::Death] {
        for e in entries.iter().rev().filter(|e| e.kind == kind).take(3) {
            let alpha = fade(e, now, focused);
            if alpha <= 0.0 {
                continue;
            }
            let rows = model::wrap(e.body(config.translate), side_w - 29.0, |s| {
                game.native_text_width(s)
            });
            let h = 20.0 + rows.len() as f32 * 10.0;
            if sy - h < top {
                break;
            }
            sy -= h;
            let head = state(|s| s.heads.get(&e.name).cloned());
            draw_card(
                game,
                batch,
                e,
                [side_x, sy, side_w, h],
                &rows,
                alpha,
                head,
                focused || !config.translate,
            );
            sy -= 4.0;
        }
        sy -= 4.0;
    }
    true
}
