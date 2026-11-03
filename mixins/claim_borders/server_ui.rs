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
        if !p.flag("custom_sidebar") || status.sidebar.is_empty() {
            return;
        }
        let value = |key: &str| {
            status
                .sidebar
                .iter()
                .find(|(k, _)| k.trim().eq_ignore_ascii_case(key))
                .map(|(_, v)| v.trim().to_owned())
        };
        let mut lines: Vec<(String, u32)> = vec![
            ("КОШЕЛЁК".into(), 0x8fa9bb),
            (format!("{} монет", money(book.balance)), 0xf0d68a),
        ];
        if p.flag("enabled") && p.flag("sidebar_limits") {
            lines.push(("ЛИМИТ ПРОДАЖ СЕРВЕРУ".into(), 0x8fa9bb));
            lines.push((
                format!(
                    "Осталось {} / {} монет",
                    money(book.coin_left),
                    book.coin_cap
                        .map_or("?".into(), crate::game::claim_borders_economy::fmt)
                ),
                0xc3dfd3,
            ));
            if book.coin_left.is_some_and(|(n, _)| n == 0) {
                lines.push(("Обновится в 00:00 UTC".into(), 0xd0a778));
            }
        }
        let play = value("Playtime");
        let online = value("Online");
        if play.is_some() || online.is_some() {
            lines.push((
                format!(
                    "В игре {}  ·  Онлайн {}",
                    play.as_deref().unwrap_or("—"),
                    online.as_deref().unwrap_or("—")
                ),
                0xa8bdcf,
            ));
        }
        if p.flag("sidebar_tps") {
            lines.push((
                crate::game::claim_borders_server_tps_view::label(),
                0x9edccb,
            ));
        }
        let richest: Vec<_> = status
            .sidebar
            .iter()
            .filter(|(k, _)| {
                let t = k.trim();
                t.starts_with("1.") || t.starts_with("2.") || t.starts_with("3.")
            })
            .collect();
        if !richest.is_empty() {
            lines.push(("БОГАТЕЙШИЕ ИГРОКИ".into(), 0x8fa9bb));
            for (k, v) in richest {
                lines.push((format!("{}  {}", k.trim(), v.trim()), 0xb9cad7));
            }
        }
        // Keep unfamiliar server fields, but omit boilerplate, duplicates and the address.
        let own = game
            .remote
            .as_ref()
            .map(|r| r.profile_name.as_str())
            .unwrap_or("");
        for (k, v) in &status.sidebar {
            let k = k.trim();
            let low = k.to_ascii_lowercase();
            if k.is_empty()
                || k.eq_ignore_ascii_case(own)
                || [
                    "balance",
                    "balance:",
                    "баланс",
                    "playtime",
                    "online",
                    "richest",
                ]
                .contains(&low.as_str())
                || k.starts_with("1.")
                || k.starts_with("2.")
                || k.starts_with("3.")
                || low.contains("playhorizon")
                || low.contains("draimhorizons")
            {
                continue;
            }
            lines.push((format!("{k} {v}"), 0xa8bdcf));
        }
        let scale = p.get("sidebar_scale") as f32 / 100.0;
        let width = (p.get("sidebar_width") as f32 * scale).min(batch.width - 12.0);
        let max = (((batch.height - 60.0) / scale - 34.0) / 12.0).max(1.0) as usize;
        lines.truncate(max);
        let height = (34.0 + lines.len() as f32 * 12.0) * scale;
        let (x, y) = corners.place(
            crate::game::claim_borders_map_view::corner(p.get("sidebar_corner") as u8),
            width,
            height,
        );
        let start = batch.vertices.len();
        let left = x / scale;
        let top = y / scale;
        let w = width / scale;
        ui::fill(
            game,
            batch,
            [left, top, w, height / scale],
            ((p.get("sidebar_opacity") as u32 * 255 / 100) << 24) | 0x0c1721,
        );
        ui::icon(
            game,
            batch,
            ui::Icon::Home,
            [left + 9.0, top + 7.0, 12.0, 12.0],
            ui::preferences(game).accent.color(),
        );
        label(
            game,
            batch,
            "DraimHorizons",
            [left + 27.0, top + 7.0],
            w - 36.0,
            0xa9e0d2,
        );
        ui::fill(
            game,
            batch,
            [left + 9.0, top + 22.0, w - 18.0, 1.0],
            0x4472cbb5,
        );
        for (i, (text, color)) in lines.iter().enumerate() {
            let kind = if text == "КОШЕЛЁК" {
                Some(ui::Icon::Wallet)
            } else if text == "ЛИМИТ ПРОДАЖ СЕРВЕРУ" {
                Some(ui::Icon::Shield)
            } else if text == "БОГАТЕЙШИЕ ИГРОКИ" {
                Some(ui::Icon::Chart)
            } else if text.starts_with("В игре") {
                Some(ui::Icon::Clock)
            } else if text.starts_with("TPS") {
                Some(ui::Icon::Signal)
            } else {
                None
            };
            if let Some(kind) = kind {
                ui::icon(
                    game,
                    batch,
                    kind,
                    [left + 9.0, top + 29.0 + i as f32 * 12.0, 9.0, 9.0],
                    ui::preferences(game).accent.color(),
                );
            }
            let inset = if kind.is_some() { 23.0 } else { 9.0 };
            label(
                game,
                batch,
                text,
                [left + inset, top + 29.0 + i as f32 * 12.0],
                w - inset - 9.0,
                *color,
            );
        }
        ui::finish(game, batch, start, "sidebar", "sidebar");
        batch.scale_since(start, 0.0, 0.0, scale, scale);
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
        let max_rows = (((batch.height - 78.0) / 15.0).floor() as usize).max(1);
        let per = columns * max_rows;
        let pages = players.len().div_ceil(per).max(1);
        let page = PAGE.load(Ordering::Relaxed).min(pages - 1);
        PAGE.store(page, Ordering::Relaxed);
        let count = players.len().saturating_sub(page * per).min(per);
        let rows = count.div_ceil(columns).max(1);
        let width = (columns as f32 * 200.0 + 20.0).min(batch.width - 24.0);
        let cw = (width - 20.0) / columns as f32;
        let height = 54.0 + rows as f32 * 15.0;
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
            let top = y + 40.0 + row as f32 * 15.0;
            if row % 2 == 0 {
                ui::fill(game, batch, [left, top - 1.0, cw - 4.0, 14.0], 0x190d1c28);
            }
            let skin = game.face_skin(player.uuid, &player.skin);
            crate::game::claim_borders_avatar_draw::face(
                game,
                batch,
                &skin,
                [left + 5.0, top + 5.0],
                10.0,
                0xffffffff,
            );
            if is_friend(player.uuid) {
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Check,
                    [left + 6.0, top + 7.0, 6.0, 6.0],
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
                [left + 14.0, top + 1.0],
                cw - ping_width - dw - 22.0,
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
                    [left + cw - ping_width - dw - 5.0, top + 1.0],
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
                    [left + cw - ping_width, top + 1.0],
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
