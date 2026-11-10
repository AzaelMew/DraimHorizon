use crate::game::claim_borders_ui_view as ui;
// Native map presentation and controls. Terrain is supplied by Horizon's minimap.
use crate::game::{Game, claim_borders as claims, claim_borders_map as model};
use claim_borders_relay_client::translation::Language;
use horizon_client::hud::HudBatch;
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};
struct Drag {
    mouse: [f32; 2],
    center: [f64; 2],
}
struct State {
    preferences: model::Preferences,
    loaded: bool,
    source: Option<model::Snapshot>,
    open: bool,
    mob_filter: bool,
    mob_page: usize,
    return_menu: bool,
    close_request: bool,
    dimension: String,
    view: model::View,
    follow: bool,
    drag: Option<Drag>,
    rect: Option<[f32; 4]>,
    published: Option<(String, model::View, f32)>,
    mini_published: Option<(String, model::View)>,
    displayed: Option<(String, model::View, f32)>,
    dimensions: Vec<String>,
    history_warning: Option<String>,
    left_click: Option<(std::time::Instant, [f32; 2], String)>,
    right_click: Option<(std::time::Instant, [f32; 2], String)>,
    probes: std::collections::BTreeSet<u64>,
    points: Vec<(String, [f64; 3])>,
    seed_patch: Option<horizon_client::font::Patch>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            preferences: model::Preferences::default(),
            loaded: false,
            source: None,
            open: false,
            mob_filter: false,
            mob_page: 0,
            return_menu: true,
            close_request: false,
            dimension: String::new(),
            view: model::View {
                center: [0.0, 0.0],
                blocks_per_pixel: 2.0,
            },
            follow: true,
            drag: None,
            rect: None,
            published: None,
            mini_published: None,
            displayed: None,
            dimensions: Vec::new(),
            history_warning: None,
            left_click: None,
            right_click: None,
            probes: std::collections::BTreeSet::new(),
            points: Vec::new(),
            seed_patch: None,
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
pub(super) fn settings(game: &Game<'_>) -> model::Preferences {
    state(|s| {
        if !s.loaded {
            s.preferences = game
                .native_setting(claims::SETTINGS_ID, "claim_map")
                .and_then(|raw| model::Preferences::decode(&raw).ok())
                .unwrap_or_default();
            s.loaded = true;
        }
        s.preferences
    })
}
pub(super) fn configure(game: &Game<'_>, preferences: model::Preferences) {
    let preferences = preferences.normalized();
    game.native_set_setting(claims::SETTINGS_ID, "claim_map", &preferences.encode());
    state(|s| {
        if s.preferences.profile != preferences.profile
            || s.preferences.remember != preferences.remember
        {
            s.published = None;
            s.dimensions.clear();
            s.history_warning = None;
        }
        s.preferences = preferences;
    });
}
pub(super) fn reset() {
    crate::game::claim_borders_seed_view::reset();
    crate::game::extras::claim_borders_map_backend::flush();
    state(|s| {
        s.right_click = None;
        s.left_click = None;
        s.probes.clear();
        s.points.clear();
        s.source = None;
        s.dimensions.clear();
        s.history_warning = None;
        s.open = false;
        s.displayed = None;
        s.drag = None;
        s.rect = None;
        s.published = None;
        s.mini_published = None;
        s.close_request = false;
    });
}
pub(super) fn sync(game: &Game<'_>, source: model::Snapshot) {
    let _ = settings(game);
    state(|s| {
        if s.source
            .as_ref()
            .is_some_and(|old| old.scope != source.scope)
        {
            s.published = None;
            s.mini_published = None;
            s.dimensions.clear();
            s.history_warning = None;
            s.open = false;
        }
        if s.open && game.native_mod_screen() != Some(claims::ID) {
            s.open = false;
            s.mob_filter = false;
            s.drag = None;
        }
        if s.follow {
            s.dimension = source.dimension.clone();
            s.view.center = [source.player[0], source.player[2]];
        }
        s.source = Some(source);
    });
}
pub(super) fn open(source: model::Snapshot, focus: Option<(String, i32, i32)>, return_menu: bool) {
    crate::game::claim_borders_seed_view::open(&source, &focus);
    state(|s| {
        s.dimension = source.dimension.clone();
        s.view = model::View {
            center: [source.player[0], source.player[2]],
            blocks_per_pixel: 2.0,
        };
        s.follow = focus.is_none();
        if let Some((dimension, x, z)) = focus {
            s.dimension = dimension;
            s.view.center = [f64::from(x) + 8.0, f64::from(z) + 8.0];
            s.view.blocks_per_pixel = 0.5;
        }
        s.source = Some(source);
        s.published = None;
        s.displayed = None;
        s.open = true;
        s.return_menu = return_menu;
        s.drag = None;
        s.close_request = false;
    });
}
pub(in crate::game) fn waypoint_ready(token: u64, dimension: String, position: [f64; 3]) {
    state(|s| {
        if s.probes.remove(&token) {
            s.points.push((dimension, position));
        }
    });
}
pub(super) fn take_points() -> Vec<(String, [f64; 3])> {
    state(|s| std::mem::take(&mut s.points))
}
pub(super) fn take_close() -> bool {
    state(|s| std::mem::take(&mut s.close_request))
}
pub(super) fn visible() -> bool {
    state(|s| s.open)
}
pub(super) fn corner(value: u8) -> horizon_client::extras::Corner {
    use horizon_client::extras::Corner;
    match value {
        0 => Corner::TopLeft,
        1 => Corner::TopRight,
        2 => Corner::BottomLeft,
        _ => Corner::BottomRight,
    }
}
pub(super) fn wanted(game: &Game<'_>) -> Option<crate::game::native_hooks::NativeMinimap> {
    let p = settings(game);
    if !game.native_on(claims::ID) || !p.minimap || visible() {
        return None;
    }
    Some(crate::game::native_hooks::NativeMinimap {
        corner: corner(p.corner),
        size: p.size as f32,
        blocks_per_pixel: p.zoom,
        rotate: false,
        round: false,
    })
}
#[derive(Clone)]
pub(in crate::game) struct Request {
    pub source: model::Snapshot,
    pub preferences: model::Preferences,
    pub view: model::View,
    pub dimension: String,
    pub full: bool,
    pub aspect: f32,
    pub draw: bool,
    pub refresh: bool,
}
pub(in crate::game) fn request(game: &Game<'_>) -> Option<Request> {
    if !game.native_on(claims::ID) || game.title.is_some() || game.native_player().is_none() {
        return None;
    }
    let p = settings(game);
    let seed_active = crate::game::claim_borders_seed_view::active();
    state(|s| {
        let source = s.source.clone()?;
        Some(Request {
            view: if s.open {
                s.view
            } else {
                model::View {
                    center: [source.player[0], source.player[2]],
                    blocks_per_pixel: p.zoom,
                }
            },
            dimension: if s.open {
                s.dimension.clone()
            } else {
                source.dimension.clone()
            },
            source,
            preferences: p,
            full: s.open,
            aspect: if s.open {
                s.rect.map_or(1.0, |r| r[2] / r[3])
            } else {
                1.0
            },
            draw: (s.open && !seed_active) || (!s.open && p.minimap && !game.ui.hide_gui),
            refresh: s
                .published
                .as_ref()
                .is_none_or(|(dim, _, _)| dim != &s.dimension),
        })
    })
}
pub(in crate::game) fn published(dimension: String, view: model::View, aspect: f32) {
    state(|s| s.published = Some((dimension, view, aspect)));
}
pub(in crate::game) fn mini_published(dimension: String, view: model::View) {
    state(|s| s.mini_published = Some((dimension, view)));
}
pub(in crate::game) fn history_status(dimensions: Vec<String>, warning: Option<String>) {
    state(|s| {
        s.dimensions = dimensions;
        s.history_warning = warning;
    });
}
pub(super) fn map_marks(
    game: &Game<'_>,
    batch: &mut HudBatch,
    source: &model::Snapshot,
    dimension: &str,
    view: model::View,
    rect: [f32; 4],
    p: model::Preferences,
    full: bool,
    aspect: f32,
) {
    let xy = |world| {
        let point = view.pixel_aspect(world, 128.0, aspect);
        [
            rect[0] + point[0] / 128.0 * rect[2],
            rect[1] + point[1] / 128.0 * rect[3],
        ]
    };
    let start = batch.vertices.len();
    let mut marks: Vec<_> = source
        .marks
        .iter()
        .filter(|m| m.dimension == dimension)
        .collect();
    marks.sort_by_key(|m| (m.player.is_some(), m.friend));
    if p.mobs > 0 && source.dimension == dimension {
        let entities: std::collections::BTreeMap<_, _> = game
            .mobs
            .iter()
            .filter(|m| !m.removed)
            .map(|m| (m.id, m))
            .collect();
        for mob in source
            .mobs
            .iter()
            .filter(|m| p.mob_visible(&m.type_id, m.hostile))
        {
            let at = xy([mob.position[0], mob.position[2]]);
            if !contains(rect, at) {
                continue;
            }
            let alpha = if (mob.position[1] - source.player[1]).abs() > 16.0 {
                0x88
            } else {
                0xff
            };
            if let Some(entity) = entities.get(&mob.entity) {
                crate::game::claim_borders_mob_heads::draw(
                    game,
                    batch,
                    entity,
                    at,
                    if full { 12.0 } else { 9.0 },
                    alpha,
                );
            }
        }
    }
    if p.marks {
        for mark in marks {
            let at = xy(mark.position);
            if !contains(rect, at) {
                continue;
            }
            if let Some(uuid) = mark.player {
                let size = if full { 10.0 } else { 8.0 };
                let color = if mark.friend {
                    source.friend_color
                } else {
                    0xe9c66b
                };
                ui::fill(
                    game,
                    batch,
                    [
                        at[0] - size * 0.5 - 1.0,
                        at[1] - size * 0.5 - 1.0,
                        size + 2.0,
                        size + 2.0,
                    ],
                    0xff000000 | color,
                );
                let skin = if mark.skin.is_empty() {
                    horizon_client::tab_list::default_skin(uuid)
                } else {
                    &mark.skin
                };
                crate::game::claim_borders_avatar_draw::face(game, batch, skin, at, size, u32::MAX);
                let mouse = game.native_cursor();
                if mark.friend
                    && (mouse[0] - at[0]).abs() <= size * 0.6
                    && (mouse[1] - at[1]).abs() <= size * 0.6
                {
                    let distance = (mark.position[0] - source.player[0])
                        .hypot(mark.position[1] - source.player[2]);
                    let text = format!(
                        "{} · {:.0} {}",
                        mark.label,
                        distance,
                        language().text("м", "m")
                    );
                    let width = (game.native_text_width(&text) + 8.0).min(rect[2] - 4.0);
                    let x = (at[0] - width / 2.0).clamp(
                        rect[0] + 2.0,
                        (rect[0] + rect[2] - width - 2.0).max(rect[0] + 2.0),
                    );
                    let y = (at[1] + size / 2.0 + 3.0).min(rect[1] + rect[3] - 12.0);
                    ui::fill(game, batch, [x, y, width, 11.0], 0xee101b28);
                    label(
                        game,
                        batch,
                        &text,
                        [x + 4.0, y + 2.0],
                        width - 4.0,
                        0xbce8d7,
                    );
                }
            } else {
                let kind = if mark.friend {
                    crate::game::claim_borders_waypoints::MarkerKind::Pin
                } else {
                    crate::game::claim_borders_client::waypoint_kind(dimension, mark.position)
                };
                ui::icon(
                    game,
                    batch,
                    crate::game::claim_borders_waypoint_view::icon(kind),
                    [at[0] - 5.0, at[1] - 5.0, 10.0, 10.0],
                    mark.color,
                );
            }
            if full && p.labels {
                label(
                    game,
                    batch,
                    &mark.label,
                    [at[0] + 8.0, at[1] - 4.0],
                    100.0,
                    mark.color,
                );
            }
        }
    }
    if let Some((_, [x, _, z])) = game
        .level
        .last_death
        .as_ref()
        .filter(|(dim, _)| dim == dimension)
    {
        let at = xy([f64::from(*x) + 0.5, f64::from(*z) + 0.5]);
        if contains(rect, at) {
            ui::fill(
                game,
                batch,
                [at[0] - 6.0, at[1] - 6.0, 12.0, 12.0],
                0xef351b27,
            );
            ui::icon(
                game,
                batch,
                ui::Icon::Close,
                [at[0] - 5.0, at[1] - 5.0, 10.0, 10.0],
                0xff879b,
            );
            if full && p.labels {
                label(
                    game,
                    batch,
                    language().text("Последняя смерть", "Last death"),
                    [at[0] + 8.0, at[1] - 4.0],
                    105.0,
                    0xff879b,
                );
            }
        }
    }
    if source.dimension == dimension {
        let at = xy([source.player[0], source.player[2]]);
        if contains(rect, at) {
            let yaw = source.yaw.to_radians();
            let dir = [-yaw.sin(), yaw.cos()];
            let right = [dir[1], -dir[0]];
            batch.fill_quad(
                &game.gui.atlas,
                [
                    [at[0] + dir[0] * 14.0, at[1] + dir[1] * 14.0],
                    [at[0] + right[0] * 4.0, at[1] + right[1] * 4.0],
                    [at[0] - right[0] * 4.0, at[1] - right[1] * 4.0],
                    [at[0] + dir[0] * 14.0, at[1] + dir[1] * 14.0],
                ],
                0xfff0cc73,
            );
            let size = 10.0;
            ui::fill(
                game,
                batch,
                [at[0] - 6.0, at[1] - 6.0, 12.0, 12.0],
                0xffe7f8ff,
            );
            let skin = if source.player_skin.is_empty() {
                horizon_client::tab_list::default_skin(source.player_uuid)
            } else {
                &source.player_skin
            };
            crate::game::claim_borders_avatar_draw::face(game, batch, skin, at, size, u32::MAX);
        }
    }
    batch.clip_since(
        start,
        rect[0] + 1.0,
        rect[1] + 1.0,
        rect[0] + rect[2] - 1.0,
        rect[1] + rect[3] - 1.0,
    );
}
pub(super) fn hud(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let p = settings(game);
    if wanted(game).is_none() || game.ui.hide_gui || !game.native_corner_shown(corner(p.corner)) {
        return;
    }
    let (source, image) = state(|s| (s.source.clone(), s.mini_published.clone()));
    let (Some(source), Some((dimension, view))) = (source, image) else {
        return;
    };
    let (x, next_y) = corners.place(corner(p.corner), p.size as f32, 0.0);
    let y = if p.corner < 2 {
        next_y - p.size as f32 - 2.0
    } else {
        next_y + 2.0
    };
    map_marks(
        game,
        batch,
        &source,
        &dimension,
        view,
        [x, y, p.size as f32, p.size as f32],
        p,
        false,
        1.0,
    );
    map_frame(
        game,
        batch,
        [x, y, p.size as f32, p.size as f32],
        view,
        1.0,
        true,
    );
    if p.coordinates {
        coordinate_chip(game, batch, &dimension, source.player, &p, corners);
    }
}
/// The coordinates under the minimap: a tag, then each axis as a dim letter and a bright value.
/// In the Nether a second row gives where that is in the overworld (the game's 8:1 rule).
fn coordinate_chip(
    game: &Game<'_>,
    batch: &mut HudBatch,
    dimension: &str,
    player: [f64; 3],
    p: &model::Preferences,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let language = crate::game::claim_borders_chat_view::language(game);
    let block = |v: f64| (v.floor() as i64).to_string();
    let mut rows: Vec<(&str, u32, Vec<(&str, String)>)> = Vec::new();
    if dimension == "minecraft:the_nether" {
        rows.push((
            language.text("Незер", "Nether"),
            0xe39ca8,
            vec![
                ("X", block(player[0])),
                ("Y", block(player[1])),
                ("Z", block(player[2])),
            ],
        ));
        rows.push((
            language.text("Мир", "Overworld"),
            0x9ce3ba,
            vec![("X", block(player[0] * 8.0)), ("Z", block(player[2] * 8.0))],
        ));
    } else {
        rows.push((
            "",
            0,
            vec![
                ("X", block(player[0])),
                ("Y", block(player[1])),
                ("Z", block(player[2])),
            ],
        ));
    }
    let tag_w = rows
        .iter()
        .map(|(tag, _, _)| {
            if tag.is_empty() {
                0.0
            } else {
                game.native_text_width(tag) + 8.0
            }
        })
        .fold(0.0, f32::max);
    let row_w = |axes: &[(&str, String)]| {
        axes.iter()
            .map(|(label, value)| {
                game.native_text_width(label) + 3.0 + game.native_text_width(value) + 9.0
            })
            .sum::<f32>()
    };
    let inner = rows
        .iter()
        .map(|(_, _, axes)| tag_w + row_w(axes))
        .fold(0.0, f32::max);
    let width = (inner + 14.0).min(batch.width - 12.0);
    let height = if rows.len() > 1 { 30.0 } else { 16.0 };
    let (x, y) = corners.place(corner(p.corner), width, height + 2.0);
    ui::rounded(game, batch, [x, y, width, height], 4.0, 0xe0182230);
    ui::fill(
        game,
        batch,
        [x + 5.0, y + 4.0, 2.0, height - 8.0],
        0xff000000 | ui::preferences(game).accent.color(),
    );
    for (i, (tag, tag_color, axes)) in rows.iter().enumerate() {
        let ty = y + 4.0 + i as f32 * 13.0;
        let mut tx = x + 10.0;
        if !tag.is_empty() {
            ui::text(game, batch, tag, [tx, ty], *tag_color, false);
        }
        tx += tag_w;
        for (label, value) in axes {
            ui::text(game, batch, label, [tx, ty], 0x8fa9bb, false);
            tx += game.native_text_width(label) + 3.0;
            ui::text(game, batch, value, [tx, ty], 0xf0f4f8, false);
            tx += game.native_text_width(value) + 9.0;
        }
    }
}
pub(super) fn label(
    game: &Game<'_>,
    batch: &mut HudBatch,
    text: &str,
    pos: [f32; 2],
    width: f32,
    color: u32,
) {
    let mut shown = String::new();
    for c in text.chars() {
        shown.push(c);
        if game.native_text_width(&shown) > width - 5.0 {
            shown.pop();
            shown.push('…');
            break;
        }
    }
    ui::text(game, batch, &shown, pos, color, false);
}
#[derive(Clone, Copy)]
enum Button {
    ZoomIn,
    ZoomOut,
    Center,
    Death,
    Fit,
    Dimension(usize),
    Seed,
    Grid,
    Settings,
    Mobs,
    Layer,
    Back,
}
fn map_rect(size: [f32; 2]) -> [f32; 4] {
    crate::game::claim_borders_seed_map::map_rect(size)
}
fn buttons(size: [f32; 2]) -> Vec<(Button, [f32; 4], String)> {
    let lang = language();
    let mut out: Vec<_> = [
        (Button::ZoomIn, lang.text("Приблизить", "Zoom in")),
        (Button::ZoomOut, lang.text("Отдалить", "Zoom out")),
        (Button::Center, lang.text("К себе", "Center")),
        (Button::Death, lang.text("Смерть", "Death")),
        (Button::Fit, lang.text("Показать приваты", "Fit claims")),
        (Button::Layer, lang.text("Рельеф", "Terrain")),
        (Button::Grid, lang.text("Сетка", "Grid")),
        (Button::Mobs, lang.text("Фильтр мобов", "Mob filter")),
        (Button::Settings, lang.text("Настройки", "Settings")),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (button, label))| {
        (
            button,
            crate::game::claim_borders_seed_map::tool_rect(size, i),
            label.into(),
        )
    })
    .collect();
    out.push((
        Button::Seed,
        [128.0, 4.0, 80.0, 22.0],
        lang.text("Сид-карта", "Seed map").into(),
    ));
    out.push((
        Button::Back,
        [size[0] - 28.0, 4.0, 22.0, 22.0],
        lang.text("Закрыть", "Close").into(),
    ));
    for (i, label) in [
        lang.text("Обычный мир", "Overworld"),
        lang.text("Незер", "Nether"),
        lang.text("Край", "End"),
    ]
    .into_iter()
    .enumerate()
    {
        out.push((
            Button::Dimension(i),
            crate::game::claim_borders_seed_map::dimension_rect(size, i),
            label.into(),
        ));
    }
    out
}
pub(super) fn tool(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    caption: &str,
    icon: ui::Icon,
    selected: bool,
    mouse: [f32; 2],
) {
    ui::button_base(game, batch, rect, caption, selected, mouse);
    ui::icon(
        game,
        batch,
        icon,
        [rect[0] + 5.0, rect[1] + (rect[3] - 14.0) * 0.5, 14.0, 14.0],
        if selected { 0x88d9bd } else { 0xd2dce4 },
    );
    if rect[2] > 40.0 {
        ui::text(
            game,
            batch,
            &ui::clip(game, caption, rect[2] - 28.0),
            [rect[0] + 24.0, rect[1] + (rect[3] - 8.0) * 0.5],
            crate::game::claim_borders_ui_style::TEXT,
            false,
        );
    }
    if contains(rect, mouse) {
        ui::tooltip(game, batch, caption, mouse);
    }
}
pub(super) fn map_frame(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: [f32; 4],
    view: model::View,
    aspect: f32,
    mini: bool,
) {
    let [x, y, w, h] = rect;
    for r in [
        [x - 1.0, y - 1.0, w + 2.0, 1.0],
        [x - 1.0, y + h, w + 2.0, 1.0],
        [x - 1.0, y, 1.0, h],
        [x + w, y, 1.0, h],
    ] {
        ui::fill(game, batch, r, 0xff6c7e91);
    }
    for (text, at) in [
        ("N", [x + w / 2.0 - 3.0, y + 3.0]),
        ("S", [x + w / 2.0 - 3.0, y + h - 10.0]),
        ("W", [x + 3.0, y + h / 2.0 - 4.0]),
        ("E", [x + w - 9.0, y + h / 2.0 - 4.0]),
    ] {
        ui::fill(
            game,
            batch,
            [at[0] - 1.0, at[1] - 1.0, 8.0, 10.0],
            0xc0101720,
        );
        ui::text(
            game,
            batch,
            text,
            at,
            if text == "N" { 0xf0cc73 } else { 0xc8d2dd },
            false,
        );
    }
    if let Some((blocks, pixels)) = model::scale_bar(
        f64::from(view.blocks_per_pixel) * 128.0 * f64::from(aspect),
        w,
        if mini { w * 0.28 } else { w * 0.16 },
    ) {
        let bx = x + 6.0;
        let by = y + h - 6.0;
        let text = if blocks >= 1000.0 {
            format!("{} {}", blocks / 1000.0, language().text("км", "km"))
        } else {
            format!("{blocks:.0} {}", language().text("м", "m"))
        };
        let label_width = (pixels + 5.0).max(game.native_text_width(&text) + 6.0);
        ui::fill(
            game,
            batch,
            [bx - 2.0, by - 13.0, label_width + 2.0, 17.0],
            0xc0101720,
        );
        ui::fill(game, batch, [bx, by, pixels, 1.0], 0xffe3e9ed);
        for xx in [bx, bx + pixels] {
            ui::fill(game, batch, [xx, by - 2.0, 1.0, 3.0], 0xffe3e9ed);
        }
        label(game, batch, &text, [bx, by - 11.0], label_width, 0xd9e1e8);
    }
}
fn operate(game: &Game<'_>, button: Button) {
    if matches!(button, Button::Seed) {
        crate::game::claim_borders_seed_view::show();
        return;
    }
    if matches!(button, Button::Layer) {
        let mut p = settings(game);
        p.layer = p.layer.next();
        configure(game, p);
        return;
    }
    if matches!(button, Button::Grid) {
        let mut p = settings(game);
        p.grid = !p.grid;
        configure(game, p);
        return;
    }
    if matches!(button, Button::Mobs) {
        state(|s| {
            s.mob_filter = !s.mob_filter;
            s.drag = None;
        });
        return;
    }
    if matches!(button, Button::Settings) {
        state(|s| {
            s.open = false;
            s.mob_filter = false;
            s.drag = None;
        });
        crate::game::claim_borders_menu_view::claims_settings();
        return;
    }
    state(|s| match button {
        Button::Mobs => {}
        Button::ZoomIn => s.view.zoom_at([64.0, 64.0], 128.0, 0.5),
        Button::ZoomOut => s.view.zoom_at([64.0, 64.0], 128.0, 2.0),
        Button::Center => {
            s.follow = true;
            if let Some(source) = &s.source {
                s.dimension = source.dimension.clone();
                s.view.center = [source.player[0], source.player[2]];
            }
        }
        Button::Death => {
            if let Some((dim, [x, _, z])) = &game.level.last_death {
                s.dimension.clone_from(dim);
                s.view.center = [f64::from(*x) + 0.5, f64::from(*z) + 0.5];
                s.follow = false;
                s.drag = None;
            }
        }
        Button::Fit => {
            if let Some(source) = &s.source {
                let cells: Vec<_> = source
                    .cells
                    .iter()
                    .filter(|c| c.own && c.dimension == s.dimension)
                    .collect();
                if !cells.is_empty() {
                    let minx = cells.iter().map(|c| c.x).min().unwrap_or(0) as f64;
                    let maxx = cells.iter().map(|c| c.x + 16).max().unwrap_or(16) as f64;
                    let minz = cells.iter().map(|c| c.z).min().unwrap_or(0) as f64;
                    let maxz = cells.iter().map(|c| c.z + 16).max().unwrap_or(16) as f64;
                    s.view.center = [(minx + maxx) * 0.5, (minz + maxz) * 0.5];
                    s.view.blocks_per_pixel = (((maxx - minx).max(maxz - minz) + 32.0) / 110.0)
                        .clamp(0.25, 524288.0) as f32;
                    s.follow = false;
                }
            }
        }
        Button::Dimension(i) => {
            s.dimension = crate::game::claim_borders_seed_map::DIMENSIONS[i].into();
            s.follow = false;
            s.drag = None;
            s.published = None;
        }
        Button::Back => {
            s.published = None;
            s.open = false;
            s.mob_filter = false;
            s.drag = None;
            s.close_request = !s.return_menu;
        }
        _ => {}
    });
}
pub(super) fn render(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    if crate::game::claim_borders_seed_view::render(game, batch, screen) {
        return true;
    }
    let render_start = batch.vertices.len();
    let size = [screen.rect[2], screen.rect[3]];
    let rect = map_rect(size);
    let p = settings(game);
    let (source, dimension, view, image, warning) = state(|s| {
        s.rect = Some(rect);
        if let Some(drag) = &s.drag {
            if (screen.mouse[0] - drag.mouse[0]).hypot(screen.mouse[1] - drag.mouse[1]) > 5.0 {
                s.left_click = None;
            }
            s.view.center = [
                drag.center[0]
                    - f64::from(
                        (screen.mouse[0] - drag.mouse[0]) / rect[3]
                            * 128.0
                            * s.view.blocks_per_pixel,
                    ),
                drag.center[1]
                    - f64::from(
                        (screen.mouse[1] - drag.mouse[1]) / rect[3]
                            * 128.0
                            * s.view.blocks_per_pixel,
                    ),
            ];
            s.view.constrain();
            s.follow = false;
        }
        let image = s
            .published
            .as_ref()
            .filter(|(dim, _, _)| dim == &s.dimension)
            .cloned();
        s.displayed = image
            .as_ref()
            .map(|_| (s.dimension.clone(), s.view, rect[2] / rect[3]));
        (
            s.source.clone(),
            s.dimension.clone(),
            s.view,
            image,
            s.history_warning.clone(),
        )
    });
    ui::fill(game, batch, screen.rect, 0xff101821);
    ui::fill(game, batch, rect, 0xff18212c);
    if let Some((_, old, old_aspect)) = &image {
        if let Some(texture) = game
            .gui
            .atlas
            .sprites
            .get("claim_borders/world_map")
            .copied()
        {
            let aspect = rect[2] / rect[3];
            let corners = [[0.0, 0.0], [128.0, 128.0]].map(|xy| {
                let world = old.world_aspect(xy, 128.0, *old_aspect);
                let at = view.pixel_aspect(world, 128.0, aspect);
                [
                    rect[0] + at[0] / 128.0 * rect[2],
                    rect[1] + at[1] / 128.0 * rect[3],
                ]
            });
            let first = batch.vertices.len();
            batch.rect_uv(
                corners[0][0],
                corners[0][1],
                corners[1][0] - corners[0][0],
                corners[1][1] - corners[0][1],
                texture,
                u32::MAX,
            );
            batch.clip_since(
                first,
                rect[0],
                rect[1],
                rect[0] + rect[2],
                rect[1] + rect[3],
            );
        }
    } else {
        label(
            game,
            batch,
            language().text(
                "Загрузка исследованной местности…",
                "Loading explored area…",
            ),
            [rect[0] + 12.0, rect[1] + 12.0],
            rect[2] - 24.0,
            0x9fadb9,
        );
    }
    ui::fill(
        game,
        batch,
        [0.0, size[1] - 32.0, size[0], 32.0],
        0xf0151e27,
    );
    let (actual, aspect) = (view, rect[2] / rect[3]);
    if let Some(source) = source {
        for overlay in model::overlay_rects(actual, &dimension, &source, p, rect) {
            game.native_fill(batch, overlay.rect, overlay.color);
        }
        map_marks(
            game, batch, &source, &dimension, actual, rect, p, true, aspect,
        );
        if p.labels && actual.blocks_per_pixel <= 4.0 {
            let mut owners = BTreeSet::new();
            for cell in source
                .cells
                .iter()
                .filter(|c| c.dimension == dimension && ((c.own && p.own) || (!c.own && p.friends)))
            {
                let at =
                    actual.pixel_aspect([cell.x as f64 + 8.0, cell.z as f64 + 8.0], 128.0, aspect);
                let xy = [
                    rect[0] + at[0] / 128.0 * rect[2],
                    rect[1] + at[1] / 128.0 * rect[3],
                ];
                if contains(rect, xy)
                    && owners.len() < 16
                    && owners.insert((cell.owner.clone(), cell.reporter.clone()))
                {
                    let start = batch.vertices.len();
                    label(
                        game,
                        batch,
                        &cell.owner_label(language()),
                        xy,
                        80.0,
                        if cell.foreign {
                            0xe45b65
                        } else if cell.own {
                            source.own_color
                        } else {
                            source.friend_color
                        },
                    );
                    batch.clip_since(
                        start,
                        rect[0],
                        rect[1],
                        rect[0] + rect[2],
                        rect[1] + rect[3],
                    );
                }
            }
        }
        if p.mobs > 0 && dimension == source.dimension && contains(rect, screen.mouse) {
            if let Some(mob) = source
                .mobs
                .iter()
                .filter(|m| p.mob_visible(&m.type_id, m.hostile))
                .find(|m| {
                    let xy = actual.pixel_aspect([m.position[0], m.position[2]], 128.0, aspect);
                    let at = [
                        rect[0] + xy[0] / 128.0 * rect[2],
                        rect[1] + xy[1] / 128.0 * rect[3],
                    ];
                    (at[0] - screen.mouse[0]).abs() < 5.0 && (at[1] - screen.mouse[1]).abs() < 5.0
                })
            {
                let x = screen.mouse[0].clamp(
                    rect[0] + 2.0,
                    (rect[0] + rect[2] - 132.0).max(rect[0] + 2.0),
                );
                let y = (screen.mouse[1] - 24.0).max(rect[1] + 2.0);
                ui::fill(game, batch, [x, y, 130.0, 20.0], 0xed101821);
                label(
                    game,
                    batch,
                    &format!("{} · Y {:.0}", mob.label, mob.position[1]),
                    [x + 4.0, y + 6.0],
                    122.0,
                    if mob.hostile { 0xf07876 } else { 0x86ce9a },
                );
            }
        }
        let text = if contains(rect, screen.mouse) {
            let world = actual.world_aspect(
                [
                    (screen.mouse[0] - rect[0]) / rect[2] * 128.0,
                    (screen.mouse[1] - rect[1]) / rect[3] * 128.0,
                ],
                128.0,
                aspect,
            );
            let (cx, cz) = (model::origin(world[0]), model::origin(world[1]));
            let owner = source
                .cells
                .iter()
                .find(|c| c.dimension == dimension && c.x == cx && c.z == cz)
                .map_or_else(|| "—".to_owned(), |c| c.owner_label(language()));
            format!(
                "X {:.0}  Z {:.0}  ·  {} [{}, {}]  ·  {}: {owner}",
                world[0].floor(),
                world[1].floor(),
                language().text("чанк", "chunk"),
                cx.div_euclid(16),
                cz.div_euclid(16),
                language().text("Владелец", "Owner")
            )
        } else if let Some((dim, [x, y, z])) = &game.level.last_death {
            let distance = if dim == &source.dimension {
                let meters = crate::game::claim_borders_waypoints::distance(
                    source.player,
                    [f64::from(*x) + 0.5, f64::from(*y), f64::from(*z) + 0.5],
                );
                meters.map_or(String::new(), |m| {
                    format!(" · {m:.0} {}", language().text("м", "m"))
                })
            } else {
                String::new()
            };
            format!(
                "{} · {} · {x} {y} {z}{distance}",
                language().text("Последняя смерть", "Last death"),
                model::dimension_label_in(dim, language())
            )
        } else {
            language()
                .text(
                    "Тянуть ЛКМ — карта · 2×ЛКМ — удалить свою метку · 2×ПКМ — создать",
                    "Drag LMB — pan map · 2×LMB — delete your waypoint · 2×RMB — create",
                )
                .into()
        };
        label(
            game,
            batch,
            &text,
            [8.0, size[1] - 25.0],
            size[0] - 16.0,
            0xc7d3df,
        );
        let info = warning.as_ref().map_or_else(
            || {
                format!(
                    "{} · {} {} · {} {}",
                    if !source.own_known {
                        language().text(
                            "Свои приваты: обнови /claims",
                            "Own claims: refresh with /claims",
                        )
                    } else if p.remember {
                        language().text(
                            "Исследованная местность сохраняется на диск",
                            "Explored area is saved to disk",
                        )
                    } else {
                        language().text("Запись карты выключена", "Map recording is off")
                    },
                    language().text("свои приваты", "own claims"),
                    source
                        .cells
                        .iter()
                        .filter(|c| c.own && c.dimension == dimension)
                        .count(),
                    language().text("чужие", "others"),
                    source
                        .cells
                        .iter()
                        .filter(|c| !c.own && c.dimension == dimension)
                        .count()
                )
            },
            |error| {
                format!(
                    "{}: {error}",
                    language().text(
                        "Не удалось сохранить/прочитать карту",
                        "Could not save/read the map"
                    )
                )
            },
        );
        label(
            game,
            batch,
            &info,
            [8.0, size[1] - 12.0],
            size[0] - 16.0,
            if warning.is_some() {
                0xe7ad75
            } else {
                0x8194a6
            },
        );
    }
    map_frame(game, batch, rect, actual, aspect, false);
    ui::fill(game, batch, [0.0, 0.0, size[0], rect[1]], 0xf0151e27);
    ui::button(
        game,
        batch,
        [6.0, 4.0, 118.0, 22.0],
        language().text("Исследованная", "Explored"),
        true,
        screen.mouse,
    );
    for (button, r, caption) in buttons(size) {
        match button {
            Button::Dimension(i) => ui::button(
                game,
                batch,
                r,
                &caption,
                dimension == crate::game::claim_borders_seed_map::DIMENSIONS[i],
                screen.mouse,
            ),
            Button::Seed => ui::button(game, batch, r, &caption, false, screen.mouse),
            _ => {
                let icon = match button {
                    Button::ZoomIn => ui::Icon::Plus,
                    Button::ZoomOut => ui::Icon::Minus,
                    Button::Center => ui::Icon::Locate,
                    Button::Death => ui::Icon::Skull,
                    Button::Fit => ui::Icon::Shield,
                    Button::Layer => ui::Icon::Layers,
                    Button::Grid => ui::Icon::Grid,
                    Button::Settings => ui::Icon::Settings,
                    Button::Mobs => ui::Icon::People,
                    _ => ui::Icon::Close,
                };
                let caption = if matches!(button, Button::Layer) {
                    format!(
                        "{}: {}",
                        language().text("Рельеф", "Terrain"),
                        p.layer.label_in(language())
                    )
                } else if matches!(button, Button::Grid) {
                    format!(
                        "{}: {}",
                        language().text("Сетка", "Grid"),
                        if p.grid {
                            language().text("вкл.", "on")
                        } else {
                            language().text("выкл.", "off")
                        }
                    )
                } else {
                    caption
                };
                tool(
                    game,
                    batch,
                    r,
                    &caption,
                    icon,
                    (matches!(button, Button::Grid) && p.grid)
                        || (matches!(button, Button::Layer) && p.layer == model::Layer::Cave),
                    screen.mouse,
                );
            }
        }
    }
    draw_mob_filter(game, batch, size, screen.mouse);
    ui::finish(
        game,
        batch,
        render_start,
        "world-map",
        &format!("{}:{}", dimension, p.layer.label()),
    );
    true
}

fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] < rect[0] + rect[2]
        && point[1] < rect[1] + rect[3]
}
fn mob_filter_rect(size: [f32; 2]) -> [f32; 4] {
    [
        size[0] - (size[0] - 16.0).min(230.0) - 8.0,
        34.0,
        (size[0] - 16.0).min(230.0),
        (size[1] - 72.0).max(60.0),
    ]
}
fn mob_filter_rows(size: [f32; 2]) -> usize {
    ((mob_filter_rect(size)[3] - 66.0) / 20.0).max(1.0) as usize
}
fn mob_filter_hits(game: &Game<'_>, size: [f32; 2]) -> Vec<([f32; 4], usize, String)> {
    let r = mob_filter_rect(size);
    let per = mob_filter_rows(size);
    let page = state(|s| s.mob_page.min(model::MOB_TYPES.len().div_ceil(per) - 1));
    model::MOB_TYPES
        .iter()
        .enumerate()
        .skip(page * per)
        .take(per)
        .enumerate()
        .map(|(slot, (index, id))| {
            (
                [
                    r[0] + 6.0,
                    r[1] + 28.0 + slot as f32 * 20.0,
                    r[2] - 12.0,
                    18.0,
                ],
                index,
                game.translate(&format!("entity.minecraft.{id}"), &id.replace('_', " ")),
            )
        })
        .collect()
}
fn draw_mob_filter(game: &Game<'_>, batch: &mut HudBatch, size: [f32; 2], mouse: [f32; 2]) {
    if !state(|s| s.mob_filter) {
        return;
    }
    let r = mob_filter_rect(size);
    let p = settings(game);
    ui::fill(game, batch, r, 0xf21a2330);
    let lang = language();
    let mode = [
        lang.text("Выключены", "Off"),
        lang.text("Все категории", "All categories"),
        lang.text("Только враждебные", "Hostile only"),
    ][p.mobs as usize];
    ui::button(
        game,
        batch,
        [r[0] + 6.0, r[1] + 4.0, r[2] - 38.0, 20.0],
        mode,
        false,
        mouse,
    );
    tool(
        game,
        batch,
        [r[0] + r[2] - 26.0, r[1] + 4.0, 20.0, 20.0],
        lang.text("Закрыть фильтр", "Close filter"),
        ui::Icon::Close,
        false,
        mouse,
    );
    for (rect, index, name) in mob_filter_hits(game, size) {
        ui::button_base(game, batch, rect, &name, p.mob_selected(index), mouse);
        ui::icon(
            game,
            batch,
            if p.mob_selected(index) {
                ui::Icon::Check
            } else {
                ui::Icon::Minus
            },
            [rect[0] + 4.0, rect[1] + (rect[3] - 10.0) * 0.5, 10.0, 10.0],
            0xc6d3df,
        );
        ui::text(
            game,
            batch,
            &ui::clip(game, &name, rect[2] - 22.0),
            [rect[0] + 18.0, rect[1] + (rect[3] - 8.0) * 0.5],
            0xc6d3df,
            false,
        );
    }

    ui::button(
        game,
        batch,
        [r[0] + 6.0, r[1] + r[3] - 24.0, 28.0, 18.0],
        "←",
        false,
        mouse,
    );
    ui::button(
        game,
        batch,
        [r[0] + 38.0, r[1] + r[3] - 24.0, r[2] - 76.0, 18.0],
        language().text("Выбрать все", "Select all"),
        false,
        mouse,
    );
    ui::button(
        game,
        batch,
        [r[0] + r[2] - 34.0, r[1] + r[3] - 24.0, 28.0, 18.0],
        "→",
        false,
        mouse,
    );
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    *canceled = true;
    let size = [screen.rect[2], screen.rect[3]];
    if state(|s| s.mob_filter) && !crate::game::claim_borders_seed_view::active() {
        let r = mob_filter_rect(size);
        if contains(r, screen.mouse) {
            if pressed && button == 0 {
                if screen.mouse[1] < r[1] + 26.0 && screen.mouse[0] >= r[0] + r[2] - 28.0 {
                    state(|s| s.mob_filter = false);
                    return true;
                }
                let mut p = settings(game);
                if screen.mouse[1] < r[1] + 26.0 {
                    p.mobs = (p.mobs + 1) % 3;
                } else if screen.mouse[1] >= r[1] + r[3] - 26.0 {
                    if screen.mouse[0] < r[0] + 36.0 {
                        state(|s| s.mob_page = s.mob_page.saturating_sub(1));
                    } else if screen.mouse[0] > r[0] + r[2] - 36.0 {
                        state(|s| {
                            s.mob_page = (s.mob_page + 1)
                                .min(model::MOB_TYPES.len().div_ceil(mob_filter_rows(size)) - 1)
                        });
                    } else {
                        p.mob_hidden = [0; 2];
                        p.mobs = 1;
                    }
                } else if let Some((_, index, _)) = mob_filter_hits(game, size)
                    .into_iter()
                    .find(|(r, _, _)| contains(*r, screen.mouse))
                {
                    p.toggle_mob(index);
                }
                configure(game, p);
            }
            return true;
        }
    }
    if crate::game::claim_borders_seed_view::mouse(game, screen, button, pressed) {
        return true;
    }
    if !pressed {
        state(|s| s.drag = None);
        return true;
    }
    if button == 1
        && buttons([screen.rect[2], screen.rect[3]])
            .iter()
            .any(|(_, r, _)| contains(*r, screen.mouse))
    {
        return true;
    }
    if button == 1 {
        let point = state(|s| {
            let rect = s.rect?;
            if !contains(rect, screen.mouse) {
                return None;
            }
            let (dimension, view, aspect) = s.displayed.clone()?;
            if dimension != s.dimension {
                return None;
            }
            let double = s.right_click.as_ref().is_some_and(|(at, old, dim)| {
                at.elapsed().as_millis() <= 400
                    && dim == &dimension
                    && (old[0] - screen.mouse[0]).abs() < 5.0
                    && (old[1] - screen.mouse[1]).abs() < 5.0
            });
            s.right_click = Some((std::time::Instant::now(), screen.mouse, dimension.clone()));
            if !double {
                return None;
            }
            s.right_click = None;
            let world = view.world_aspect(
                [
                    (screen.mouse[0] - rect[0]) / rect[2] * 128.0,
                    (screen.mouse[1] - rect[1]) / rect[3] * 128.0,
                ],
                128.0,
                aspect,
            );
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let token = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            s.probes.insert(token);
            Some((token, dimension, world))
        });
        if let Some((token, dim, point)) = point
            && !crate::game::extras::claim_borders_map_backend::waypoint(token, dim, point)
        {
            state(|s| {
                s.probes.remove(&token);
            });
        }
    }
    if button == 0 {
        if let Some((button, _, _)) = buttons([screen.rect[2], screen.rect[3]])
            .into_iter()
            .find(|(_, r, _)| contains(*r, screen.mouse))
        {
            operate(game, button);
        } else {
            let target = state(|s| {
                let rect = s.rect?;
                if !contains(rect, screen.mouse) {
                    return None;
                }
                let (dimension, view, aspect) = s.displayed.clone()?;
                if dimension != s.dimension {
                    return None;
                }
                let double = s.left_click.as_ref().is_some_and(|(at, old, dim)| {
                    at.elapsed().as_millis() <= 400
                        && dim == &dimension
                        && (old[0] - screen.mouse[0]).hypot(old[1] - screen.mouse[1]) < 5.0
                });
                s.left_click = Some((std::time::Instant::now(), screen.mouse, dimension.clone()));
                s.drag = Some(Drag {
                    mouse: screen.mouse,
                    center: s.view.center,
                });
                s.follow = false;
                if !double {
                    return None;
                }
                s.left_click = None;
                s.drag = None;
                let world = view.world_aspect(
                    [
                        (screen.mouse[0] - rect[0]) / rect[2] * 128.0,
                        (screen.mouse[1] - rect[1]) / rect[3] * 128.0,
                    ],
                    128.0,
                    aspect,
                );
                Some((
                    dimension,
                    world,
                    7.0 * 128.0 * f64::from(view.blocks_per_pixel) / f64::from(rect[3]),
                ))
            });
            if let Some((dim, point, radius)) = target {
                if let Some(w) = crate::game::claim_borders_client::own_waypoints()
                    .into_iter()
                    .filter(|w| w.dimension == dim)
                    .map(|w| {
                        let d = (w.position[0] - point[0]).hypot(w.position[2] - point[1]);
                        (w.id, d)
                    })
                    .filter(|(_, d)| *d <= radius)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    crate::game::claim_borders_waypoint_view::queue_delete(w.0);
                }
            }
        }
    }
    true
}
pub(super) fn scroll(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    amount: f64,
    canceled: &mut bool,
) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    *canceled = true;
    let size = [screen.rect[2], screen.rect[3]];
    if state(|s| s.mob_filter) && contains(mob_filter_rect(size), screen.mouse) {
        if amount.is_finite() && amount != 0.0 {
            state(|s| {
                s.mob_page = s
                    .mob_page
                    .saturating_add_signed(if amount > 0.0 { -1 } else { 1 })
                    .min(model::MOB_TYPES.len().div_ceil(mob_filter_rows(size)) - 1)
            });
        }
        return true;
    }
    if crate::game::claim_borders_seed_view::scroll(game, screen, amount) {
        return true;
    }
    state(|s| {
        if let Some(rect) = s.rect
            && contains(rect, screen.mouse)
        {
            let pixel = [
                (screen.mouse[0] - rect[0]) / rect[2] * 128.0,
                (screen.mouse[1] - rect[1]) / rect[3] * 128.0,
            ];
            let aspect = rect[2] / rect[3];
            let before = s.view.world_aspect(pixel, 128.0, aspect);
            s.view.blocks_per_pixel = (s.view.blocks_per_pixel
                * if amount > 0.0 { 0.5 } else { 2.0 })
            .clamp(0.25, 524288.0);
            let after = s.view.world_aspect(pixel, 128.0, aspect);
            s.view.center[0] += before[0] - after[0];
            s.view.center[1] += before[1] - after[1];
            s.view.constrain();
            s.follow = false;
        }
    });
    true
}
pub(super) fn key(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    *canceled = true;
    if key == "key.keyboard.escape" && state(|s| s.mob_filter) {
        state(|s| s.mob_filter = false);
        return true;
    }
    if crate::game::claim_borders_seed_view::key(game, key) {
        return true;
    }
    if key == game.native_mod_key(claims::ID, "map", "key.keyboard.m") {
        operate(game, Button::Back);
        return true;
    }
    if key == game.native_mod_key(claims::ID, "menu", "key.keyboard.j") {
        operate(game, Button::Settings);
        return true;
    }
    match key {
        "key.keyboard.escape" => operate(game, Button::Back),
        "key.keyboard.g" => operate(game, Button::Grid),
        "key.keyboard.home" => operate(game, Button::Center),
        "key.keyboard.equal" => operate(game, Button::ZoomIn),
        "key.keyboard.minus" => operate(game, Button::ZoomOut),
        _ => {}
    }
    true
}

pub(super) fn seed_source() -> Option<model::Snapshot> {
    state(|s| s.source.clone())
}
pub(super) fn seed_point(dimension: String, position: [f64; 3]) {
    state(|s| s.points.push((dimension, position)));
}
pub(super) fn seed_close(game: &Game<'_>) {
    crate::game::claim_borders_seed_view::reset();
    operate(game, Button::Back);
}
pub(super) fn seed_settings(game: &Game<'_>) {
    crate::game::claim_borders_seed_view::reset();
    operate(game, Button::Settings);
}
pub(super) fn seed_patch(game: &Game<'_>, rect: horizon_client::hud::Rect, rgba: Vec<u8>) {
    state(|s| {
        s.seed_patch = Some(horizon_client::font::Patch {
            x: (rect.u0 * game.gui.atlas.width as f32).round() as u32,
            y: (rect.v0 * game.gui.atlas.height as f32).round() as u32,
            width: 256,
            height: 256,
            rgba,
        })
    });
}
pub(super) fn take_seed_patch() -> Option<horizon_client::font::Patch> {
    state(|s| s.seed_patch.take())
}

fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
