use crate::game::claim_borders_ui_view as ui;
// Optional server presentation, with separately labelled money and daily sale allowance.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_economy_client as data,
    claim_borders_economy_view::label,
};
use horizon_client::hud::HudBatch;
use std::sync::atomic::{AtomicUsize, Ordering};
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
fn card(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
    p: &crate::game::claim_borders_economy::Preferences,
    title: &str,
    icon: ui::Icon,
    lines: &[(String, u32)],
    key: &str,
) {
    let natural_height = 27.0 + lines.len() as f32 * 13.0;
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
    ui::fill(
        game,
        batch,
        [left, top, w, height / scale],
        ((p.get("sidebar_opacity") as u32 * 255 / 100) << 24) | 0x101c29,
    );
    ui::icon(
        game,
        batch,
        icon,
        [left + 8.0, top + 7.0, 11.0, 11.0],
        ui::preferences(game).accent.color(),
    );
    label(
        game,
        batch,
        title,
        [left + 25.0, top + 8.0],
        w - 33.0,
        0xbce8d7,
    );
    for (i, (text, color)) in lines.iter().enumerate() {
        label(
            game,
            batch,
            text,
            [left + 9.0, top + 25.0 + i as f32 * 13.0],
            w - 18.0,
            *color,
        );
    }
    ui::finish(game, batch, start, "sidebar-card", key);
    batch.scale_since(start, 0.0, 0.0, scale, scale);
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
        let mut performance = vec![(format!("FPS  {:.0}", game.fps), 0xdce5ed)];
        if p.flag("sidebar_tps") {
            performance.push((
                crate::game::claim_borders_server_tps_view::label(),
                0x9edccb,
            ));
        }
        performance.push((
            format!(
                "Онлайн  {}",
                value(&["Online", "Онлайн"]).unwrap_or_else(|| "—".into())
            ),
            0xdce5ed,
        ));
        performance.push((
            format!(
                "В игре  {}",
                value(&["Playtime", "Время игры"]).unwrap_or("—".into())
            ),
            0xdce5ed,
        ));
        card(
            game,
            batch,
            corners,
            p,
            "СЕРВЕР · КЛИЕНТ",
            ui::Icon::Signal,
            &performance,
            "performance",
        );
        let mut economy = vec![(format!("Баланс  {} монет", money(book.balance)), 0xf0d68a)];
        if p.flag("enabled") && p.flag("sidebar_limits") {
            economy.push(("Продажа серверу · осталось".into(), 0x9aacc0));
            economy.push((
                format!(
                    "{} / {} монет",
                    money(book.coin_left),
                    book.coin_cap
                        .map_or("—".into(), crate::game::claim_borders_economy::fmt)
                ),
                0xc3dfd3,
            ));
            if book.coin_left.is_some_and(|(n, _)| n == 0) {
                economy.push(("Сброс лимита · 00:00 UTC".into(), 0xd0a778));
            }
        }
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
            economy.push(("Богатейшие игроки".into(), 0x9aacc0));
            for (k, v) in richest {
                let budget = p.get("sidebar_width") as f32 - 28.0 - game.native_text_width(v);
                economy.push((
                    format!("{}  {}", ui::clip(game, k.trim(), budget), v.trim()),
                    0xb8c9d7,
                ));
            }
        }
        card(
            game,
            batch,
            corners,
            p,
            "ЭКОНОМИКА",
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
        let columns = (p.get("tab_columns") as usize)
            .min(((batch.width - 24.0) / 150.0).max(1.0) as usize)
            .max(1);
        let max_rows = (((batch.height - 78.0) / 22.0).floor() as usize).max(1);
        let per = columns * max_rows;
        let pages = players.len().div_ceil(per).max(1);
        let page = PAGE.load(Ordering::Relaxed).min(pages - 1);
        PAGE.store(page, Ordering::Relaxed);
        let count = players.len().saturating_sub(page * per).min(per);
        let rows = count.div_ceil(columns).max(1);
        let width = (columns as f32 * 230.0 + 20.0).min(batch.width - 24.0);
        let cw = (width - 20.0) / columns as f32;
        let height = 60.0 + rows as f32 * 22.0;
        let x = (batch.width - width) / 2.0;
        let y = 10.0;
        ui::fill(
            game,
            batch,
            [x, y, width, height],
            ((p.get("tab_opacity") as u32 * 255 / 100) << 24) | 0x0b1621,
        );
        ui::fill(game, batch, [x, y, width, 2.0], 0x9972cbb5);
        ui::icon(
            game,
            batch,
            ui::Icon::People,
            [x + 11.0, y + 8.0, 12.0, 12.0],
            ui::preferences(game).accent.color(),
        );
        let title = if columns == 1 {
            format!("Игроки · {}", players.len())
        } else {
            format!("DraimHorizons · {} онлайн", players.len())
        };
        label(
            game,
            batch,
            &title,
            [x + 29.0, y + 9.0],
            width - 139.0,
            0xa9e0d2,
        );
        if p.flag("tab_tps") {
            label(
                game,
                batch,
                &crate::game::claim_borders_server_tps_view::label(),
                [x + width - 106.0, y + 9.0],
                96.0,
                0xd0dfe8,
            );
        }
        for col in 0..columns {
            let left = x + 10.0 + col as f32 * cw;
            ui::fill(
                game,
                batch,
                [left - 2.0, y + 25.0, cw - 4.0, rows as f32 * 22.0 + 23.0],
                0x70111e2c,
            );
            label(
                game,
                batch,
                "ИГРОК",
                [left + 14.0, y + 27.0],
                cw - 57.0,
                0x8da5b8,
            );
            if p.flag("tab_ping") {
                label(
                    game,
                    batch,
                    "мс",
                    [left + cw - 25.0, y + 27.0],
                    24.0,
                    0x8da5b8,
                );
            }
        }
        let me = game.native_player();
        for (i, player) in players.iter().skip(page * per).take(per).enumerate() {
            let col = i / rows;
            let row = i % rows;
            let left = x + 10.0 + col as f32 * cw;
            let top = y + 42.0 + row as f32 * 22.0;
            let friend = is_friend(player.uuid);
            ui::fill(
                game,
                batch,
                [left + 1.0, top - 1.0, cw - 10.0, 19.0],
                if friend { 0x8033474a } else { 0x80233142 },
            );
            ui::fill(
                game,
                batch,
                [left + 1.0, top + 3.0, 2.0, 11.0],
                if friend { 0xff91cfac } else { 0xff7798b9 },
            );
            let skin = game.face_skin(player.uuid, &player.skin);
            crate::game::claim_borders_avatar_draw::face(
                game,
                batch,
                &skin,
                [left + 13.0, top + 8.0],
                12.0,
                0xffffffff,
            );
            if is_friend(player.uuid) {
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Check,
                    [left + 16.0, top + 11.0, 6.0, 6.0],
                    0xa2d0b7,
                );
            }
            let ping = if player.latency < 0 {
                "?".into()
            } else {
                player.latency.to_string()
            };
            let ping_width = if p.flag("tab_ping") {
                game.native_text_width(&ping).max(23.0) + 7.0
            } else {
                0.0
            };
            let mut distance = String::new();
            if p.flag("tab_distance")
                && is_friend(player.uuid)
                && let (Some(me), Some(pos)) = (
                    &me,
                    crate::game::claim_borders_client::entity_position(game, player.uuid),
                )
            {
                let d =
                    ((me.x - pos[0]).powi(2) + (me.y - pos[1]).powi(2) + (me.z - pos[2]).powi(2))
                        .sqrt();
                distance = format!("{d:.0}м");
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
                [left + 24.0, top + 5.0],
                cw - ping_width - dw - 38.0,
                if is_friend(player.uuid) {
                    0x9be6c2
                } else {
                    0xd0dfe8
                },
            );
            if dw > 0.0 {
                ui::text(
                    game,
                    batch,
                    &distance,
                    [left + cw - ping_width - dw - 9.0, top + 5.0],
                    0x8fb9c9,
                    false,
                );
            }
            if p.flag("tab_ping") {
                let color = if player.latency < 0 {
                    0x8da5b8
                } else if player.latency < 100 {
                    0x91cfac
                } else if player.latency < 250 {
                    0xdec184
                } else {
                    0xe39b87
                };
                ui::text(
                    game,
                    batch,
                    &ping,
                    [left + cw - ping_width - 7.0, top + 5.0],
                    color,
                    false,
                );
            }
        }
        if pages > 1 {
            label(
                game,
                batch,
                &format!("PgUp / PgDn  ·  Страница {}/{}", page + 1, pages),
                [x + 10.0, y + height - 10.0],
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
