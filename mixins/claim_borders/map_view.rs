use crate::game::claim_borders_ui_view as ui;
// Native map presentation and controls. Terrain is supplied by Horizon's minimap.
use crate::game::{Game, claim_borders as claims, claim_borders_map as model};
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
    return_menu: bool,
    close_request: bool,
    dimension: String,
    view: model::View,
    follow: bool,
    drag: Option<Drag>,
    rect: Option<[f32; 4]>,
    published: Option<(String, model::View, f32)>,
    dimensions: Vec<String>,
    history_warning: Option<String>,
    left_click: Option<(std::time::Instant, [f32; 2], String)>,
    right_click: Option<(std::time::Instant, [f32; 2], String)>,
    probes: std::collections::BTreeSet<u64>,
    points: Vec<(String, [f64; 3])>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            preferences: model::Preferences::default(),
            loaded: false,
            source: None,
            open: false,
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
            dimensions: Vec::new(),
            history_warning: None,
            left_click: None,
            right_click: None,
            probes: std::collections::BTreeSet::new(),
            points: Vec::new(),
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
                .native_setting(claims::ID, "claim_map")
                .and_then(|raw| model::Preferences::decode(&raw).ok())
                .unwrap_or_default();
            s.loaded = true;
        }
        s.preferences
    })
}
pub(super) fn configure(game: &Game<'_>, preferences: model::Preferences) {
    let preferences = preferences.normalized();
    game.native_set_setting(claims::ID, "claim_map", &preferences.encode());
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
        s.drag = None;
        s.rect = None;
        s.published = None;
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
            s.dimensions.clear();
            s.history_warning = None;
            s.open = false;
        }
        if s.open && game.native_mod_screen() != Some(claims::ID) {
            s.open = false;
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
}
pub(in crate::game) fn request(game: &Game<'_>) -> Option<Request> {
    if !game.native_on(claims::ID) || game.title.is_some() || game.native_player().is_none() {
        return None;
    }
    let p = settings(game);
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
            draw: s.open || (p.minimap && !game.ui.hide_gui),
        })
    })
}
pub(in crate::game) fn published(dimension: String, view: model::View, aspect: f32) {
    state(|s| s.published = Some((dimension, view, aspect)));
}
pub(in crate::game) fn history_status(dimensions: Vec<String>, warning: Option<String>) {
    state(|s| {
        s.dimensions = dimensions;
        s.history_warning = warning;
    });
}
fn map_marks(
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
        for mob in source.mobs.iter().filter(|m| p.mobs != 2 || m.hostile) {
            let at = xy([mob.position[0], mob.position[2]]);
            if !contains(rect, at) {
                continue;
            }
            let alpha = if (mob.position[1] - source.player[1]).abs() > 16.0 {
                0x88
            } else {
                0xff
            };
            let color = (alpha << 24) | if mob.hostile { 0xf07876 } else { 0x86ce9a };
            ui::fill(
                game,
                batch,
                [at[0] - 2.5, at[1] - 2.5, 5.0, 5.0],
                0xd0101720,
            );
            ui::fill(game, batch, [at[0] - 1.5, at[1] - 1.5, 3.0, 3.0], color);
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
                    let text = format!("{} · {:.0} м", mark.label, distance);
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
                    "Последняя смерть",
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
    let (source, image) = state(|s| (s.source.clone(), s.published.clone()));
    let (Some(source), Some((dimension, view, _))) = (source, image) else {
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
        let (x, y) = corners.place(corner(p.corner), p.size as f32, 11.0);
        label(
            game,
            batch,
            &format!(
                "X {:.0}  Z {:.0}",
                source.player[0].floor(),
                source.player[2].floor()
            ),
            [x, y],
            p.size as f32,
            0xe0e7f0,
        );
    }
}
fn label(game: &Game<'_>, batch: &mut HudBatch, text: &str, pos: [f32; 2], width: f32, color: u32) {
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
    Dimension,
    Grid,
    Settings,
    Layer,
    Back,
}
fn map_rect(size: [f32; 2]) -> [f32; 4] {
    [
        6.0,
        30.0,
        (size[0] - 12.0).max(100.0),
        (size[1] - 88.0).max(80.0),
    ]
}
fn buttons(size: [f32; 2]) -> Vec<(Button, [f32; 4], String)> {
    let width = (size[0] - 12.0) / 10.0;
    [
        (Button::ZoomOut, "−"),
        (Button::ZoomIn, "+"),
        (Button::Center, "К себе"),
        (Button::Death, "Смерть"),
        (Button::Fit, "Зоны"),
        (Button::Dimension, "Мир"),
        (Button::Layer, "Рельеф"),
        (Button::Grid, "Сетка"),
        (Button::Settings, "Опции"),
        (Button::Back, "Закрыть"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (b, text))| {
        (
            b,
            [6.0 + i as f32 * width, size[1] - 24.0, width - 2.0, 18.0],
            text.into(),
        )
    })
    .collect()
}
fn map_frame(
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
            format!("{} км", blocks / 1000.0)
        } else {
            format!("{blocks:.0} м")
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
    if matches!(button, Button::Settings) {
        state(|s| {
            s.open = false;
            s.drag = None;
        });
        crate::game::claim_borders_menu_view::claims_settings();
        return;
    }
    state(|s| match button {
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
        Button::Dimension => {
            if let Some(source) = &s.source {
                let mut dimensions: BTreeSet<_> =
                    source.cells.iter().map(|c| c.dimension.clone()).collect();
                dimensions.extend(source.marks.iter().map(|m| m.dimension.clone()));
                dimensions.extend(s.dimensions.iter().cloned());
                dimensions.extend(game.level.last_death.iter().map(|(dim, _)| dim.clone()));
                dimensions.insert(source.dimension.clone());
                let dimensions: Vec<_> = dimensions.into_iter().collect();
                let current = dimensions
                    .iter()
                    .position(|d| d == &s.dimension)
                    .unwrap_or(0);
                s.dimension = dimensions[(current + 1) % dimensions.len()].clone();
                s.follow = false;
            }
        }
        Button::Back => {
            s.published = None;
            s.open = false;
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
        (
            s.source.clone(),
            s.dimension.clone(),
            s.view,
            image,
            s.history_warning.clone(),
        )
    });
    ui::fill(game, batch, screen.rect, 0xff101821);
    label(
        game,
        batch,
        &format!("КАРТА МИРА · {}", model::dimension_label(&dimension)),
        [8.0, 10.0],
        size[0] * 0.7,
        0xe3e9ed,
    );
    label(
        game,
        batch,
        &format!(
            "{} · {}",
            p.layer.label(),
            if p.remember {
                "Сохранение вкл."
            } else {
                "Без записи"
            }
        ),
        [size[0] * 0.7, 10.0],
        size[0] * 0.3 - 8.0,
        0x9fadb9,
    );
    ui::fill(game, batch, rect, 0xff18212c);
    if image.is_some() {
        if let Some(texture) = game
            .gui
            .atlas
            .sprites
            .get("claim_borders/world_map")
            .copied()
        {
            batch.rect_uv(rect[0], rect[1], rect[2], rect[3], texture, u32::MAX);
        }
    } else {
        label(
            game,
            batch,
            "Загрузка исследованной местности…",
            [rect[0] + 12.0, rect[1] + 12.0],
            rect[2] - 24.0,
            0x9fadb9,
        );
    }
    let (actual, aspect) = image.map_or((view, rect[2] / rect[3]), |(_, v, a)| (v, a));
    if let Some(source) = source {
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
                if contains(rect, xy) && owners.len() < 16 && owners.insert(cell.owner.clone()) {
                    let start = batch.vertices.len();
                    label(
                        game,
                        batch,
                        &cell.owner,
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
                .filter(|m| p.mobs != 2 || m.hostile)
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
                .map_or("—", |c| c.owner.as_str());
            format!(
                "X {:.0}  Z {:.0}  ·  чанк [{}, {}]  ·  Владелец: {owner}",
                world[0].floor(),
                world[1].floor(),
                cx.div_euclid(16),
                cz.div_euclid(16)
            )
        } else if let Some((dim, [x, y, z])) = &game.level.last_death {
            let distance = if dim == &source.dimension {
                let meters = crate::game::claim_borders_waypoints::distance(
                    source.player,
                    [f64::from(*x) + 0.5, f64::from(*y), f64::from(*z) + 0.5],
                );
                meters.map_or(String::new(), |m| format!(" · {m:.0} м"))
            } else {
                String::new()
            };
            format!(
                "Последняя смерть · {} · {x} {y} {z}{distance}",
                model::dimension_label(dim)
            )
        } else {
            "Тянуть ЛКМ — карта · 2×ЛКМ — удалить свою метку · 2×ПКМ — создать".into()
        };
        label(
            game,
            batch,
            &text,
            [8.0, size[1] - 52.0],
            size[0] - 16.0,
            0xc7d3df,
        );
        let info = warning.as_ref().map_or_else(
            || {
                format!(
                    "{} · свои приваты {} · чужие {}",
                    if !source.own_known {
                        "Свои приваты: обнови /claims"
                    } else if p.remember {
                        "Исследованная местность сохраняется на диск"
                    } else {
                        "Запись карты выключена"
                    },
                    source
                        .cells
                        .iter()
                        .filter(|c| c.own && c.dimension == dimension)
                        .count(),
                    source
                        .cells
                        .iter()
                        .filter(|c| !c.own && c.dimension == dimension)
                        .count()
                )
            },
            |error| format!("Не удалось сохранить/прочитать карту: {error}"),
        );
        label(
            game,
            batch,
            &info,
            [8.0, size[1] - 40.0],
            size[0] - 16.0,
            if warning.is_some() {
                0xe7ad75
            } else {
                0x8194a6
            },
        );
    }
    map_frame(game, batch, rect, actual, aspect, false);
    for (_, r, caption) in buttons(size) {
        ui::button(game, batch, r, &caption, false, screen.mouse);
    }
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
    if !pressed {
        state(|s| s.drag = None);
        return true;
    }
    if button == 1 {
        let point = state(|s| {
            let rect = s.rect?;
            if !contains(rect, screen.mouse) {
                return None;
            }
            let (dimension, view, aspect) = s.published.clone()?;
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
                let (dimension, view, aspect) = s.published.clone()?;
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
    match key {
        "key.keyboard.escape" | "key.keyboard.j" => operate(game, Button::Back),
        "key.keyboard.g" => operate(game, Button::Grid),
        "key.keyboard.home" => operate(game, Button::Center),
        "key.keyboard.equal" => operate(game, Button::ZoomIn),
        "key.keyboard.minus" => operate(game, Button::ZoomOut),
        _ => {}
    }
    true
}
