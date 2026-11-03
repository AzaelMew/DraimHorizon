// One presentation layer for all DraimHorizons screens. Interactive geometry stays fixed.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_ui_style as style,
    native_hooks::NativeScreenView,
};
use horizon_client::hud::{HudBatch, rgba};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
pub(super) use style::Icon;
struct Hover {
    over: f32,
    selected: f32,
    last: Instant,
}
struct Scene {
    variant: u64,
    began: Instant,
    last: Instant,
}
#[derive(Default)]
struct State {
    loaded: bool,
    writable: bool,
    prefs: style::Preferences,
    error: Option<String>,
    hover: BTreeMap<u64, Hover>,
    scenes: BTreeMap<u64, Scene>,
    editor: Option<style::Preferences>,
    hits: Vec<([f32; 4], u8)>,
    gear: Option<[f32; 4]>,
    pressed: BTreeSet<i32>,
    focus: Option<u8>,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
}
pub(super) fn preferences(game: &Game<'_>) -> style::Preferences {
    state(|s| {
        if !s.loaded {
            match std::fs::read_to_string(game.native_data_dir(claims::ID).join("appearance")) {
                Ok(raw) => match style::Preferences::decode(&raw) {
                    Ok(p) => {
                        s.prefs = p;
                        s.writable = true;
                    }
                    Err(e) => s.error = Some(e),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => s.writable = true,
                Err(e) => s.error = Some(format!("Не удалось прочитать оформление: {e}")),
            }
            s.loaded = true;
        }
        s.prefs
    })
}
fn key(text: &str, rect: [f32; 4]) -> u64 {
    text.bytes()
        .chain(rect.into_iter().flat_map(|n| n.to_bits().to_le_bytes()))
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        })
}
pub(super) fn reset() {
    state(|s| {
        if let Some(p) = s.editor.take() {
            s.prefs = p;
        }
        s.hover.clear();
        s.scenes.clear();
        s.hits.clear();
        s.gear = None;
        s.pressed.clear();
        s.focus = None;
    });
}
pub(super) fn screen_begin(game: &Game<'_>) {
    let _ = preferences(game);
    state(|s| {
        s.gear = None;
        if s.hover.len() > 1024 {
            s.hover
                .retain(|_, v| v.last.elapsed() < Duration::from_secs(2));
        }
        if s.scenes.len() > 64 {
            s.scenes
                .retain(|_, v| v.last.elapsed() < Duration::from_secs(2));
        }
        if s.editor.is_some()
            && game.native_mod_screen() != Some(claims::ID)
            && game.screen != Screen::Inventory
        {
            if let Some(p) = s.editor.take() {
                s.prefs = p;
            }
            s.hits.clear();
        }
    });
}
pub(super) fn rounded(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    radius: f32,
    argb: u32,
) {
    let points = style::perimeter(rect, radius);
    if points.is_empty() {
        return;
    }
    let [a, r, g, b] = argb.to_be_bytes();
    let color = rgba(r, g, b, a);
    // A fan of triangles using a degenerate fourth corner; blending never overlaps.
    let center = [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5];
    for i in 0..points.len() {
        batch.fill_quad(
            &game.gui.atlas,
            [center, points[i], points[(i + 1) % points.len()], center],
            color,
        );
    }
}
fn dark(rgb: u32) -> bool {
    let [_, r, g, b] = rgb.to_be_bytes();
    r < 90 && g < 100 && b < 115
}
pub(super) fn ink(rgb: u32) -> u32 {
    match rgb & 0xffffff {
        0xffffff | 0xe6edf5 | 0xe0e8f0 | 0xe4edf6 | 0xe3e9ed | 0xe6edf3 | 0xd0dfe8 | 0xe1e9f5
        | 0xe7eaf2 | 0xe0e7f0 => style::TEXT,
        0x8995a5 | 0x9fadb9 | 0x8194a6 | 0x8da5b8 | 0x8fa9bb | 0x94b5ca | 0x7e899f | 0x879bad
        | 0x8196aa | 0x8ea3b7 => style::MUTED,
        _ => rgb & 0xffffff,
    }
}
pub(super) fn fill(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], argb: u32) {
    let p = preferences(game);
    let rgb = argb & 0xffffff;
    let a = (argb >> 24) & 255;
    let argb = if matches!(
        rgb,
        0x8b7bff | 0xb2a5ff | 0x739eac | 0x6bbdb1 | 0x72cbb5 | 0x87b6ea
    ) {
        ((a * 3 / 5) << 24) | p.accent.color()
    } else {
        argb
    };
    if !rect.iter().all(|n| n.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return;
    }
    if matches!(rgb, 0x080b12 | 0x071017) {
        game.native_fill(batch, rect, 0x300b111b);
        return;
    }
    if dark(rgb) && rect[2] >= 24.0 && rect[3] >= 10.0 {
        let tone = if matches!(
            rgb,
            0x354e62 | 0x354c62 | 0x45647b | 0x40546a | 0x465970 | 0x494d5d | 0x34364c
        ) {
            style::mix(style::SURFACE, p.accent.color(), 0.24)
        } else if rgb == 0x111520 || rgb == 0x101a25 {
            0x172230
        } else {
            style::SURFACE
        };
        let alpha = ((a as f32 * p.alpha()).round() as u32).clamp(0, 255);
        if p.shadows && rect[2] > 120.0 && rect[3] > 60.0 {
            rounded(
                game,
                batch,
                [rect[0] - 2.0, rect[1] + 2.0, rect[2] + 4.0, rect[3] + 3.0],
                7.0,
                0x1b050b13,
            );
        }
        rounded(
            game,
            batch,
            rect,
            if rect[3] > 40.0 { 7.0 } else { 4.0 },
            (alpha << 24) | tone,
        );
        return;
    }
    // Keep chart lines, user colours, atlas sprites and map geometry unchanged.
    game.native_fill(batch, rect, argb);
}
pub(super) fn text(
    game: &Game<'_>,
    batch: &mut HudBatch,
    value: &str,
    at: [f32; 2],
    rgb: u32,
    _shadow: bool,
) {
    game.native_text(batch, value, at, ink(rgb), false);
}
pub(super) fn clip(game: &Game<'_>, value: &str, width: f32) -> String {
    if width <= 0.0 {
        return String::new();
    }
    if game.native_text_width(value) <= width {
        return value.into();
    }
    let mut out = String::new();
    for c in value.chars() {
        out.push(c);
        if game.native_text_width(&out) + game.native_text_width("…") > width {
            out.pop();
            break;
        }
    }
    out.push('…');
    out
}
pub(super) fn icon(game: &Game<'_>, batch: &mut HudBatch, kind: Icon, rect: [f32; 4], rgb: u32) {
    icon_tinted(game, batch, kind, rect, 0xff000000 | rgb);
}
pub(super) fn icon_tinted(
    game: &Game<'_>,
    batch: &mut HudBatch,
    kind: Icon,
    rect: [f32; 4],
    argb: u32,
) {
    let color = {
        let [a, r, g, b] = argb.to_be_bytes();
        rgba(r, g, b, a)
    };
    for &[x0, y0, x1, y1] in style::strokes(kind) {
        let a = [rect[0] + x0 / 12.0 * rect[2], rect[1] + y0 / 12.0 * rect[3]];
        let b = [rect[0] + x1 / 12.0 * rect[2], rect[1] + y1 / 12.0 * rect[3]];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = (dx * dx + dy * dy).sqrt().max(0.001);
        let width = 0.52;
        let nx = -dy / length * width;
        let ny = dx / length * width;
        batch.fill_quad(
            &game.gui.atlas,
            [
                [a[0] + nx, a[1] + ny],
                [b[0] + nx, b[1] + ny],
                [b[0] - nx, b[1] - ny],
                [a[0] - nx, a[1] - ny],
            ],
            color,
        );
    }
}
pub(super) fn button_base(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    label: &str,
    selected: bool,
    mouse: [f32; 2],
) {
    let p = preferences(game);
    let target = style::contains(rect, mouse);
    let (hover, active) = state(|s| {
        if s.hover.len() >= 2048 && !s.hover.contains_key(&key(label, rect)) {
            s.hover.clear();
        }
        let h = s.hover.entry(key(label, rect)).or_insert_with(|| Hover {
            over: 0.0,
            selected: 0.0,
            last: Instant::now(),
        });
        let dt = h.last.elapsed().as_secs_f32();
        h.last = Instant::now();
        h.over = style::approach(h.over, if target { 1.0 } else { 0.0 }, dt, p.motion);
        h.selected = style::approach(h.selected, if selected { 1.0 } else { 0.0 }, dt, p.motion);
        (h.over, h.selected)
    });
    let danger = label.contains("Удалить") || label == "Отклонить" || label.contains("Очистить");
    let accent = if danger {
        style::BAD
    } else if matches!(label, "Принять" | "Готово") {
        style::GOOD
    } else {
        p.accent.color()
    };
    let base = style::mix(style::SURFACE, style::RAISED, hover * 0.35);
    let rgb = style::mix(base, accent, (active * 0.28 + hover * 0.12).min(0.4));
    let a = ((p.alpha() + 0.04).min(0.98) * 255.0) as u32;
    rounded(game, batch, rect, 4.0, (a << 24) | rgb);
    if selected {
        rounded(
            game,
            batch,
            [rect[0] + 1.0, rect[1] + 4.0, 2.0, (rect[3] - 8.0).max(2.0)],
            1.0,
            0xff000000 | accent,
        );
    }
    if target {
        rounded(
            game,
            batch,
            [
                rect[0] + 4.0,
                rect[1] + rect[3] - 1.0,
                (rect[2] - 8.0).max(1.0),
                1.0,
            ],
            0.5,
            0x50000000 | accent,
        );
    }
}
pub(super) fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    label: &str,
    selected: bool,
    mouse: [f32; 2],
) {
    button_base(game, batch, rect, label, selected, mouse);
    let title = match label {
        "Пати" => "Друзья",
        "Клеймы" => "Приваты",
        _ => label,
    };
    let only = rect[2] <= 28.0 && style::icon_for(label).is_some();
    let kind = style::icon_for(label);
    let with_icon = only || kind.is_some() && rect[2] >= game.native_text_width(title) + 24.0;
    let size = if only {
        10.0f32.min(rect[3] - 4.0)
    } else {
        10.0
    };
    if let Some(kind) = kind.filter(|_| with_icon) {
        let x = if only {
            rect[0] + (rect[2] - size) * 0.5
        } else {
            rect[0] + 6.0
        };
        icon(
            game,
            batch,
            kind,
            [x, rect[1] + (rect[3] - size) * 0.5, size, size],
            preferences(game).accent.color(),
        );
    }
    if !only {
        let pad = if with_icon { 21.0 } else { 6.0 };
        let value = clip(game, title, rect[2] - pad - 5.0);
        text(
            game,
            batch,
            &value,
            [rect[0] + pad, rect[1] + (rect[3] - 8.0) * 0.5],
            style::TEXT,
            false,
        );
    }
}
/// Fade a whole completed render range. Vertex positions and hit regions never move.
pub(super) fn finish(
    game: &Game<'_>,
    batch: &mut HudBatch,
    start: usize,
    scope: &str,
    variant: &str,
) {
    let p = preferences(game);
    let opacity = state(|s| {
        let id = key(scope, [0.0; 4]);
        let variant = key(variant, [0.0; 4]);
        if s.scenes.len() >= 128 && !s.scenes.contains_key(&id) {
            s.scenes.clear();
        }
        let scene = s.scenes.entry(id).or_insert_with(|| Scene {
            variant,
            began: Instant::now(),
            last: Instant::now(),
        });
        if scene.variant != variant || scene.last.elapsed() > Duration::from_millis(750) {
            scene.variant = variant;
            scene.began = Instant::now();
        }
        scene.last = Instant::now();
        style::entrance(scene.began.elapsed().as_secs_f32(), p.motion)
    });
    if opacity < 1.0 {
        for v in &mut batch.vertices[start..] {
            v.color = style::alpha(v.color, opacity);
        }
    }
}
pub(super) fn gear(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], mouse: [f32; 2]) {
    state(|s| s.gear = Some(rect));
    button(game, batch, rect, "Опции", false, mouse);
}
pub(super) fn tooltip(game: &Game<'_>, batch: &mut HudBatch, value: &str, mouse: [f32; 2]) {
    let max = (batch.width - 24.0).clamp(80.0, 260.0);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        let next = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && game.native_text_width(&next) > max {
            lines.push(line);
            line = word.into();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let w = lines
        .iter()
        .map(|s| game.native_text_width(s))
        .fold(0.0, f32::max)
        + 12.0;
    let h = lines.len() as f32 * 11.0 + 10.0;
    let x = (mouse[0] + 8.0).min(batch.width - w - 4.0).max(4.0);
    let y = (mouse[1] + 14.0).min(batch.height - h - 4.0).max(4.0);
    rounded(game, batch, [x, y, w, h], 4.0, 0xf2263446);
    for (i, line) in lines.iter().enumerate() {
        text(
            game,
            batch,
            line,
            [x + 6.0, y + 5.0 + i as f32 * 11.0],
            style::TEXT,
            false,
        );
    }
}
pub(super) fn open_settings(game: &Game<'_>) {
    let p = preferences(game);
    state(|s| {
        if s.editor.is_none() {
            s.editor = Some(p);
            s.focus = None;
            s.hits.clear();
        }
    });
}
pub(super) fn editing() -> bool {
    state(|s| s.editor.is_some())
}
fn act(game: &Game<'_>, id: u8) {
    state(|s| match id {
        0 => s.prefs.opacity = s.prefs.opacity.saturating_sub(5).max(50),
        1 => s.prefs.opacity = (s.prefs.opacity + 5).min(98),
        2 => s.prefs.accent = s.prefs.accent.next(),
        3 => s.prefs.motion = !s.prefs.motion,
        4 => s.prefs.shadows = !s.prefs.shadows,
        5 => s.prefs.contrast = !s.prefs.contrast,
        6 => s.prefs = style::Preferences::default(),
        7 => {
            if let Some(p) = s.editor.take() {
                s.prefs = p;
            }
            s.hits.clear();
        }
        8 => {
            if !s.writable {
                s.error = Some("Оформление не сохранено: проверьте файл appearance".into());
                return;
            }
            let dir = game.native_data_dir(claims::ID);
            let saved = std::fs::create_dir_all(&dir).and_then(|()| {
                let path = dir.join("appearance.tmp");
                std::fs::write(&path, s.prefs.encode())?;
                std::fs::rename(path, dir.join("appearance"))
            });
            match saved {
                Ok(()) => {
                    s.editor = None;
                    s.error = None;
                    s.hits.clear();
                }
                Err(e) => s.error = Some(format!("Не удалось сохранить: {e}")),
            }
        }
        _ => {}
    });
}
pub(super) fn settings_render(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
) {
    if !editing() {
        if state(|s| s.gear.is_some_and(|r| style::contains(r, screen.mouse))) {
            tooltip(game, batch, "Оформление всего мода · Ctrl+,", screen.mouse);
        }
        return;
    }
    let start = batch.vertices.len();
    let p = preferences(game);
    let w = (batch.width - 24.0).min(360.0);
    let h = (batch.height - 24.0).min(246.0);
    let x = (batch.width - w) * 0.5;
    let y = (batch.height - h) * 0.5;
    let row = ((h - 78.0) / 6.0).min(24.0);
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x7a09111c);
    rounded(
        game,
        batch,
        [x - 2.0, y + 3.0, w + 4.0, h + 3.0],
        9.0,
        0x38030910,
    );
    rounded(game, batch, [x, y, w, h], 8.0, 0xf21d2a3a);
    icon(
        game,
        batch,
        Icon::Settings,
        [x + 12.0, y + 12.0, 14.0, 14.0],
        p.accent.color(),
    );
    text(
        game,
        batch,
        "Оформление",
        [x + 34.0, y + 12.0],
        style::TEXT,
        false,
    );
    let error = state(|s| s.error.clone());
    let hint = error
        .as_deref()
        .unwrap_or("Единый стиль меню, карты, HUD и инвентаря");
    text(
        game,
        batch,
        &clip(game, hint, w - 24.0),
        [x + 12.0, y + 28.0],
        if error.is_some() {
            style::WARN
        } else {
            style::MUTED
        },
        false,
    );
    let on = |v| if v { "Вкл." } else { "Выкл." };
    let labels = [
        format!("Плотность фона: {}%", p.opacity),
        format!("Акцент: {}", p.accent.label()),
        format!("Плавные эффекты: {}", on(p.motion)),
        format!("Мягкие тени: {}", on(p.shadows)),
        format!("Высокий контраст: {}", on(p.contrast)),
        "Сбросить оформление".into(),
    ];
    let focus = state(|s| {
        s.hits.clear();
        s.focus
    });
    for (i, label) in labels.iter().enumerate() {
        let yy = y + 45.0 + i as f32 * row;
        let rect = [x + 12.0, yy, w - 24.0, row - 3.0];
        if i == 0 {
            text(
                game,
                batch,
                label,
                [rect[0] + 4.0, yy + (row - 11.0) * 0.5],
                style::TEXT,
                false,
            );
            for (id, xx, title) in [(0, x + w - 62.0, "−"), (1, x + w - 36.0, "+")] {
                let r = [xx, yy, 22.0, row - 3.0];
                button(game, batch, r, title, focus == Some(id), screen.mouse);
                state(|s| s.hits.push((r, id)));
            }
        } else {
            let id = (i + 1) as u8;
            button(game, batch, rect, label, focus == Some(id), screen.mouse);
            state(|s| s.hits.push((rect, id)));
        }
    }
    for (id, xx, title) in [
        (7, x + 12.0, "Отмена [Esc]"),
        (8, x + w * 0.5 + 3.0, "Готово"),
    ] {
        let r = [xx, y + h - 26.0, w * 0.5 - 15.0, 19.0];
        button(game, batch, r, title, focus == Some(id), screen.mouse);
        state(|s| s.hits.push((r, id)));
    }
    finish(game, batch, start, "theme-editor", "theme-editor");
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !pressed && state(|s| s.pressed.remove(&button)) {
        *canceled = true;
        return true;
    }
    if editing() {
        *canceled = true;
        if pressed {
            state(|s| {
                s.pressed.insert(button);
            });
            if button == 0 {
                let id = state(|s| {
                    s.hits
                        .iter()
                        .rev()
                        .find(|(r, _)| style::contains(*r, screen.mouse))
                        .map(|(_, i)| *i)
                });
                if let Some(id) = id {
                    state(|s| s.focus = Some(id));
                    act(game, id);
                }
            }
        }
        return true;
    }
    let gear = game.native_mod_screen() == Some(claims::ID)
        && !crate::game::claim_borders_economy_view::visible()
        && !crate::game::claim_borders_map_view::visible()
        && state(|s| s.gear.is_some_and(|r| style::contains(r, screen.mouse)));
    if gear && pressed && button == 0 {
        open_settings(game);
        state(|s| {
            s.pressed.insert(button);
        });
        *canceled = true;
        return true;
    }
    false
}
pub(super) fn key_event(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if editing() {
        *canceled = true;
        match key {
            "key.keyboard.escape" => act(game, 7),
            "key.keyboard.tab" => state(|s| {
                s.focus = Some(if game.modifiers.shift_key() {
                    s.focus.unwrap_or(0).wrapping_add(8) % 9
                } else {
                    s.focus.map_or(0, |i| (i + 1) % 9)
                })
            }),
            "key.keyboard.enter" | "key.keyboard.space" => {
                if let Some(id) = state(|s| s.focus) {
                    act(game, id);
                }
            }
            _ => {}
        }
        return true;
    }
    if key == "key.keyboard.comma"
        && game.modifiers.control_key()
        && game.native_on(claims::ID)
        && (game.native_mod_screen() == Some(claims::ID) || game.screen == Screen::Inventory)
    {
        open_settings(game);
        *canceled = true;
        return true;
    }
    false
}
