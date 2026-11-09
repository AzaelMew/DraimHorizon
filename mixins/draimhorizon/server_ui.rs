use crate::game::claim_borders_ui_view as ui;
// Optional server presentation, with separately labelled money and daily sale allowance.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_economy_client as data,
    claim_borders_economy_view::label,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::hud::HudBatch;
use std::sync::atomic::{AtomicUsize, Ordering};
macro_rules! tr {
    ($ru:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match crate::game::claim_borders_chat_view::current_language() {
            Language::Russian => format!($ru $(, $arg)*),
            Language::English => format!($en $(, $arg)*),
        }
    };
}
fn lang() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
const TAB_CELL: f32 = 168.0;
const TAB_ROW: f32 = 16.0;
const TAB_HEAD: f32 = 24.0;
static PAGE: AtomicUsize = AtomicUsize::new(0);
pub(super) fn page(game: &Game<'_>, key: &str) -> bool {
    if game.native_on(claims::ID)
        && game.screen == Screen::Playing
        && game.keys.contains(&winit::keyboard::KeyCode::Tab)
        && data::read(|_, p, _, _| p.flag("custom_tab"))
    {
        match key {
            "key.keyboard.page.up" => {
                let _ = PAGE.try_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
                    Some(p.saturating_sub(1))
                });
                true
            }
            "key.keyboard.page.down" => {
                PAGE.fetch_add(1, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    } else {
        false
    }
}
fn money(value: Option<(i64, u64)>) -> String {
    value.map_or("—".into(), |(v, _)| {
        crate::game::claim_borders_economy::fmt(v)
    })
}
/// One line of a sidebar card: an icon, a muted name and a coloured value on the right.
struct Row {
    icon: Option<ui::Icon>,
    label: String,
    value: String,
    color: u32,
}
fn row(icon: ui::Icon, label: &str, value: String, color: u32) -> Row {
    Row {
        icon: Some(icon),
        label: label.into(),
        value,
        color,
    }
}
const ROW: f32 = 14.0;
const HEAD: f32 = 21.0;
#[allow(clippy::too_many_arguments)]
fn card(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
    p: &crate::game::claim_borders_economy::Preferences,
    title: &str,
    icon: ui::Icon,
    rows: &[Row],
    key: &str,
) {
    let natural_height = HEAD + 6.0 + rows.len() as f32 * ROW;
    let scale = (p.get("sidebar_scale") as f32 / 100.0)
        .min((batch.height - 8.0) / natural_height)
        .max(0.25);
    let width = (p.get("sidebar_width") as f32 * scale).min(batch.width - 12.0);
    let height = natural_height * scale;
    let preferred = p.get("sidebar_corner") as u8;
    let (mut x, mut y) = corners.place(
        crate::game::claim_borders_map_view::corner(preferred),
        width,
        height,
    );
    if y < 2.0 || y + height > batch.height - 2.0 {
        (x, y) = corners.place(
            crate::game::claim_borders_map_view::corner(preferred ^ 1),
            width,
            height,
        );
    }
    x = x.clamp(2.0, (batch.width - width - 2.0).max(2.0));
    y = y.clamp(2.0, (batch.height - height - 2.0).max(2.0));
    let start = batch.vertices.len();
    let left = x / scale;
    let top = y / scale;
    let w = width / scale;
    let accent = ui::preferences(game).accent.color();
    ui::fill(
        game,
        batch,
        [left, top, w, height / scale],
        ((p.get("sidebar_opacity") as u32 * 255 / 100) << 24) | 0x101c29,
    );
    // A short accent bar and a hairline under the title give the card a header.
    ui::rounded(
        game,
        batch,
        [left + 5.0, top + 6.0, 2.0, 10.0],
        1.0,
        0xff000000 | accent,
    );
    ui::icon(
        game,
        batch,
        icon,
        [left + 11.0, top + 5.0, 12.0, 12.0],
        accent,
    );
    label(
        game,
        batch,
        title,
        [left + 27.0, top + 7.0],
        w - 35.0,
        0xbce8d7,
    );
    ui::fill(
        game,
        batch,
        [left + 6.0, top + HEAD, w - 12.0, 1.0],
        0x22ffffff,
    );
    for (i, r) in rows.iter().enumerate() {
        let at = top + HEAD + 6.0 + i as f32 * ROW;
        if i % 2 == 1 {
            ui::rounded(
                game,
                batch,
                [left + 4.0, at - 2.0, w - 8.0, ROW],
                3.0,
                0x14ffffff,
            );
        }
        let mut text_x = left + 9.0;
        if let Some(icon) = r.icon {
            ui::icon(game, batch, icon, [left + 8.0, at, 9.0, 9.0], 0x8da5b8);
            text_x = left + 21.0;
        }
        let value_w = game.native_text_width(&r.value);
        let value_x = left + w - 8.0 - value_w;
        label(
            game,
            batch,
            &r.label,
            [text_x, at + 1.0],
            (value_x - text_x - 4.0).max(0.0),
            0x9aacc0,
        );
        ui::text(game, batch, &r.value, [value_x, at + 1.0], r.color, false);
    }
    ui::finish(game, batch, start, &format!("sidebar-{key}"), "card");
    batch.scale_since(start, 0.0, 0.0, scale, scale);
}
/// Green, amber or red by how healthy a value is.
fn health(good: bool, fair: bool) -> u32 {
    if good {
        0x9be6c2
    } else if fair {
        0xf0d68a
    } else {
        0xe39b87
    }
}
/// The number in a `TPS ≈ 18.3` label.
fn tps_value(label: &str) -> Option<f32> {
    label
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
}
pub(super) fn sidebar(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    if game.ui.hide_gui
        || game.native_player().is_none()
        || !matches!(game.screen, Screen::Playing | Screen::Chat)
    {
        return;
    }
    data::read(|book, p, status, _| {
        if !p.flag("custom_sidebar") {
            return;
        }
        let value = |keys: &[&str]| {
            status
                .sidebar
                .iter()
                .find(|(k, _)| {
                    keys.iter()
                        .any(|key| k.trim().trim_end_matches(':').eq_ignore_ascii_case(key))
                })
                .map(|(_, v)| v.trim().to_owned())
        };
        let fps = game.fps;
        let mut performance = vec![row(
            ui::Icon::Gauge,
            "FPS",
            format!("{fps:.0}"),
            health(fps >= 60.0, fps >= 30.0),
        )];
        if p.flag("sidebar_tps") {
            let label = crate::game::claim_borders_server_tps_view::label();
            let (value, color) = match tps_value(&label) {
                Some(t) => (format!("{t:.1}"), health(t >= 19.0, t >= 15.0)),
                None => (label.trim_start_matches("TPS:").trim().to_owned(), 0x9aacc0),
            };
            performance.push(row(ui::Icon::Activity, "TPS", value, color));
        }
        performance.push(row(
            ui::Icon::People,
            lang().text("Онлайн", "Online"),
            value(&["Online", "Онлайн"]).unwrap_or_else(|| "—".into()),
            0xe6edf5,
        ));
        performance.push(row(
            ui::Icon::Clock,
            lang().text("В игре", "Playtime"),
            value(&["Playtime", "Время игры"]).unwrap_or("—".into()),
            0xe6edf5,
        ));
        card(
            game,
            batch,
            corners,
            p,
            lang().text("СЕРВЕР · КЛИЕНТ", "SERVER · CLIENT"),
            ui::Icon::Signal,
            &performance,
            "performance",
        );
        let mut economy = vec![row(
            ui::Icon::Coins,
            lang().text("Баланс", "Balance"),
            money(book.balance),
            0xf0d68a,
        )];
        let richest: Vec<_> = status
            .sidebar
            .iter()
            .filter(|(k, _)| {
                let k = k.trim();
                k.starts_with("1.") || k.starts_with("2.") || k.starts_with("3.")
            })
            .take(3)
            .collect();
        if !richest.is_empty() && batch.height >= 230.0 {
            for (i, (k, v)) in richest.into_iter().enumerate() {
                let budget = p.get("sidebar_width") as f32 - 64.0 - game.native_text_width(v);
                economy.push(Row {
                    icon: (i == 0).then_some(ui::Icon::Star),
                    label: ui::clip(game, k.trim(), budget),
                    value: v.trim().to_owned(),
                    color: 0xb8c9d7,
                });
            }
        }
        card(
            game,
            batch,
            corners,
            p,
            lang().text("ЭКОНОМИКА", "ECONOMY"),
            ui::Icon::Wallet,
            &economy,
            "economy",
        );
    });
}
pub(super) fn tab(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !game.native_on(claims::ID)
        || game.remote.is_none()
        || game.screen != Screen::Playing
        || !game.keys.contains(&winit::keyboard::KeyCode::Tab)
        || !data::read(|_, p, _, _| p.flag("custom_tab"))
    {
        return false;
    }
    let render_start = batch.vertices.len();
    let mut players = crate::game::scoreboard::claim_borders_economy_sidebar::prepare_tab(game);
    let friends = crate::game::claim_borders_client::friend_ids();
    let is_friend = |id| friends.iter().any(|(uuid, _)| *uuid == id);
    data::read(|_, p, _, _| {
        players.sort_by(|a, b| {
            let friend = if p.flag("tab_friends") {
                is_friend(b.uuid).cmp(&is_friend(a.uuid))
            } else {
                std::cmp::Ordering::Equal
            };
            friend.then_with(|| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            })
        });
        // A crowded server (80+ players) is split into four columns, whatever the setting.
        let wanted = if players.len() >= 80 {
            (p.get("tab_columns") as usize).max(4)
        } else {
            p.get("tab_columns") as usize
        };
        let columns = wanted
            .min(((batch.width - 16.0) / TAB_CELL).max(1.0) as usize)
            .max(1);
        let max_rows = (((batch.height - 52.0) / TAB_ROW).floor() as usize).max(1);
        let per = columns * max_rows;
        let pages = players.len().div_ceil(per).max(1);
        let page = PAGE.load(Ordering::Relaxed).min(pages - 1);
        PAGE.store(page, Ordering::Relaxed);
        let count = players.len().saturating_sub(page * per).min(per);
        let rows = count.div_ceil(columns).max(1);
        let width = (columns as f32 * TAB_CELL + 16.0).min(batch.width - 16.0);
        let cw = (width - 16.0) / columns as f32;
        let height = TAB_HEAD + 8.0 + rows as f32 * TAB_ROW + if pages > 1 { 16.0 } else { 4.0 };
        let x = (batch.width - width) / 2.0;
        let y = 8.0;
        let accent = ui::preferences(game).accent.color();
        ui::fill(
            game,
            batch,
            [x, y, width, height],
            ((p.get("tab_opacity") as u32 * 255 / 100) << 24) | 0x0b1621,
        );
        ui::rounded(
            game,
            batch,
            [x + 8.0, y + 1.0, width - 16.0, 2.0],
            1.0,
            0xcc000000 | accent,
        );
        ui::icon(
            game,
            batch,
            ui::Icon::People,
            [x + 10.0, y + 7.0, 12.0, 12.0],
            accent,
        );
        let title = if columns == 1 {
            tr!("Игроки · {}", "Players · {}", players.len())
        } else {
            tr!(
                "DraimHorizons · {} онлайн",
                "DraimHorizons · {} online",
                players.len()
            )
        };
        label(
            game,
            batch,
            &title,
            [x + 27.0, y + 9.0],
            width - 130.0,
            0xa9e0d2,
        );
        if p.flag("tab_tps") {
            let text = crate::game::claim_borders_server_tps_view::label();
            let color = match tps_value(&text) {
                Some(t) => health(t >= 19.0, t >= 15.0),
                None => 0xd0dfe8,
            };
            let shown = text.replace("TPS ≈ ", "").replace("TPS: ", "");
            let tw = game.native_text_width(&shown);
            ui::text(
                game,
                batch,
                &shown,
                [x + width - 12.0 - tw, y + 9.0],
                color,
                false,
            );
            ui::icon(
                game,
                batch,
                ui::Icon::Activity,
                [x + width - 26.0 - tw, y + 7.0, 11.0, 11.0],
                color,
            );
        }
        ui::fill(
            game,
            batch,
            [x + 8.0, y + TAB_HEAD, width - 16.0, 1.0],
            0x22ffffff,
        );
        let me = game.native_player();
        for (i, player) in players.iter().skip(page * per).take(per).enumerate() {
            let col = i / rows;
            let row = i % rows;
            let left = x + 8.0 + col as f32 * cw;
            let top = y + TAB_HEAD + 6.0 + row as f32 * TAB_ROW;
            let friend = is_friend(player.uuid);
            ui::rounded(
                game,
                batch,
                [left, top, cw - 4.0, TAB_ROW - 2.0],
                3.0,
                if friend { 0x7033474a } else { 0x60233142 },
            );
            ui::rounded(
                game,
                batch,
                [left + 1.0, top + 3.0, 2.0, 8.0],
                1.0,
                if friend { 0xff91cfac } else { 0xff7798b9 },
            );
            let skin = game.face_skin(player.uuid, &player.skin);
            crate::game::claim_borders_avatar_draw::face(
                game,
                batch,
                &skin,
                [left + 11.0, top + 7.0],
                10.0,
                0xffffffff,
            );
            if friend {
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Check,
                    [left + 13.0, top + 9.0, 5.0, 5.0],
                    0xa2d0b7,
                );
            }
            // Ping: four bars that fill with connection quality, the number beside them.
            let mut right = left + cw - 9.0;
            if p.flag("tab_ping") {
                let (bars, color) = match player.latency {
                    l if l < 0 => (0, 0x8da5b8),
                    l if l < 60 => (4, 0x91cfac),
                    l if l < 120 => (3, 0x91cfac),
                    l if l < 200 => (2, 0xdec184),
                    _ => (1, 0xe39b87),
                };
                for n in 0..4 {
                    let h = 2.0 + n as f32 * 2.0;
                    ui::fill(
                        game,
                        batch,
                        [right - 11.0 + n as f32 * 3.0, top + 11.0 - h, 2.0, h],
                        if n < bars {
                            0xff000000 | color
                        } else {
                            0x40ffffff
                        },
                    );
                }
                right -= 14.0;
                if player.latency >= 0 && cw >= 150.0 {
                    let ping = player.latency.to_string();
                    let tw = game.native_text_width(&ping);
                    ui::text(game, batch, &ping, [right - tw, top + 3.0], 0x8da5b8, false);
                    right -= tw + 5.0;
                }
            }
            let mut distance = String::new();
            if p.flag("tab_distance")
                && friend
                && let (Some(me), Some(pos)) = (
                    &me,
                    crate::game::claim_borders_client::entity_position(game, player.uuid),
                )
            {
                let d =
                    ((me.x - pos[0]).powi(2) + (me.y - pos[1]).powi(2) + (me.z - pos[2]).powi(2))
                        .sqrt();
                distance = tr!("{:.0}м", "{:.0}m", d);
            }
            let dw = if distance.is_empty() {
                0.0
            } else {
                game.native_text_width(&distance) + 5.0
            };
            label(
                game,
                batch,
                &player.name,
                [left + 20.0, top + 3.0],
                right - left - 20.0 - dw,
                if friend { 0x9be6c2 } else { 0xd0dfe8 },
            );
            if dw > 0.0 {
                ui::text(
                    game,
                    batch,
                    &distance,
                    [right - dw + 2.0, top + 3.0],
                    0x8fb9c9,
                    false,
                );
            }
        }
        if pages > 1 {
            label(
                game,
                batch,
                &tr!(
                    "PgUp / PgDn  ·  Страница {}/{}",
                    "PgUp / PgDn  ·  Page {}/{}",
                    page + 1,
                    pages
                ),
                [x + 10.0, y + height - 12.0],
                width - 20.0,
                0x8da5b8,
            );
        }
    });
    ui::finish(
        game,
        batch,
        render_start,
        "player-tab",
        &PAGE.load(Ordering::Relaxed).to_string(),
    );
    true
}
