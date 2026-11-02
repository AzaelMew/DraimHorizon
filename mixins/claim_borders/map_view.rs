// Native map presentation and controls. Terrain is supplied by Horizon's minimap.
use crate::game::{Game, claim_borders as claims, claim_borders_map as model};
use horizon_client::hud::{HudBatch, MINIMAP_SPRITE};
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
    published: Option<(String, model::View)>,
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
    state(|s| s.preferences = preferences);
}
pub(super) fn reset() {
    state(|s| {
        s.source = None;
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
        s.open = true;
        s.return_menu = return_menu;
        s.drag = None;
        s.close_request = false;
    });
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
}
pub(in crate::game) fn request(game: &Game<'_>) -> Option<Request> {
    if !game.native_on(claims::ID) || game.title.is_some() || game.native_player().is_none() {
        return None;
    }
    let p = settings(game);
    if game.ui.hide_gui && !visible() {
        return None;
    }
    state(|s| {
        if !s.open && !p.minimap {
            return None;
        }
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
        })
    })
}
pub(in crate::game) fn published(dimension: String, view: model::View) {
    state(|s| s.published = Some((dimension, view)));
}
pub(super) fn hud(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let p = settings(game);
    if wanted(game).is_none()
        || !p.coordinates
        || game.ui.hide_gui
        || !game.native_corner_shown(corner(p.corner))
    {
        return;
    }
    if let Some(player) = game.native_player() {
        let (x, y) = corners.place(corner(p.corner), p.size as f32, 11.0);
        label(
            game,
            batch,
            &format!("X {:.0}  Z {:.0}", player.x.floor(), player.z.floor()),
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
    game.native_text(batch, &shown, pos, color, false);
}
#[derive(Clone, Copy)]
enum Button {
    ZoomIn,
    ZoomOut,
    Center,
    Fit,
    Dimension,
    Grid,
    Settings,
    Back,
}
fn buttons(size: [f32; 2]) -> Vec<(Button, [f32; 4], String)> {
    let side = (size[1] - 60.0).min(size[0] - 160.0).max(100.0);
    let x = side + 18.0;
    let w = (size[0] - x - 8.0).max(80.0);
    vec![
        (Button::ZoomOut, [x, 38.0, w / 2.0 - 2.0, 18.0], "−".into()),
        (
            Button::ZoomIn,
            [x + w / 2.0, 38.0, w / 2.0, 18.0],
            "+".into(),
        ),
        (
            Button::Center,
            [x, 60.0, w / 2.0 - 2.0, 18.0],
            "К себе".into(),
        ),
        (
            Button::Fit,
            [x + w / 2.0, 60.0, w / 2.0, 18.0],
            "К приватам".into(),
        ),
        (Button::Dimension, [x, 82.0, w, 18.0], "Измерение".into()),
        (Button::Grid, [x, 104.0, w, 18.0], "Сетка чанков".into()),
        (Button::Settings, [x, 126.0, w, 18.0], "Настройки".into()),
        (Button::Back, [x, 148.0, w, 18.0], "Назад · Esc".into()),
    ]
}
fn operate(game: &Game<'_>, button: Button) {
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
    let size = [screen.rect[2], screen.rect[3]];
    let side = (size[1] - 60.0).min(size[0] - 160.0).max(100.0);
    let rect = [8.0, 38.0, side, side];
    let p = settings(game);
    let (source, dimension, view, image) = state(|s| {
        s.rect = Some(rect);
        if let Some(drag) = &s.drag {
            s.view.center = [
                drag.center[0]
                    - f64::from(
                        (screen.mouse[0] - drag.mouse[0]) / side * 128.0 * s.view.blocks_per_pixel,
                    ),
                drag.center[1]
                    - f64::from(
                        (screen.mouse[1] - drag.mouse[1]) / side * 128.0 * s.view.blocks_per_pixel,
                    ),
            ];
            s.view.constrain();
            s.follow = false;
        }
        let image = s
            .published
            .as_ref()
            .filter(|(dim, _)| dim == &s.dimension)
            .cloned();
        (s.source.clone(), s.dimension.clone(), s.view, image)
    });
    game.native_fill(batch, screen.rect, 0xfa0f1420);
    label(
        game,
        batch,
        &format!("КАРТА · {}", model::dimension_label(&dimension)),
        [8.0, 12.0],
        size[0] - 16.0,
        0xdfe5f2,
    );
    game.native_fill(batch, [7.0, 37.0, side + 2.0, side + 2.0], 0xff48546a);
    game.native_fill(batch, rect, 0xff1a2130);
    if image.is_some() {
        if let Some(texture) = game.gui.atlas.sprites.get(MINIMAP_SPRITE).copied() {
            batch.rect_uv(rect[0], rect[1], rect[2], rect[3], texture, u32::MAX);
        }
    } else {
        label(
            game,
            batch,
            "Загрузка карты…",
            [14.0, 46.0],
            side - 12.0,
            0xaab7cc,
        );
    }
    let actual = image.map_or(view, |(_, v)| v);
    for (button, r, caption) in buttons(size) {
        let hover = contains(r, screen.mouse);
        game.native_fill(batch, r, if hover { 0xff49516b } else { 0xff293348 });
        label(
            game,
            batch,
            &caption,
            [r[0] + 4.0, r[1] + 5.0],
            r[2] - 8.0,
            0xe4eaf6,
        );
        let _ = button;
    }
    label(
        game,
        batch,
        "N ↑",
        [rect[0] + side / 2.0 - 8.0, rect[1] + 3.0],
        25.0,
        0xffffff,
    );
    if let Some(source) = source {
        let x = side + 18.0;
        let width = size[0] - x - 8.0;
        let own = source
            .cells
            .iter()
            .filter(|c| c.own && c.dimension == dimension)
            .count();
        let friends = source
            .cells
            .iter()
            .filter(|c| !c.own && c.dimension == dimension)
            .count();
        label(
            game,
            batch,
            &format!("Свои: {own}"),
            [x, 176.0],
            width,
            source.own_color,
        );
        label(
            game,
            batch,
            &format!("Друзья онлайн: {friends}"),
            [x, 188.0],
            width,
            source.friend_color,
        );
        if contains(rect, screen.mouse) {
            let world = actual.world(
                [
                    (screen.mouse[0] - rect[0]) / side * 128.0,
                    (screen.mouse[1] - rect[1]) / side * 128.0,
                ],
                128.0,
            );
            let (cx, cz) = (model::origin(world[0]), model::origin(world[1]));
            let owner = source
                .cells
                .iter()
                .find(|c| c.dimension == dimension && c.x == cx && c.z == cz);
            label(
                game,
                batch,
                &format!("Чанк [{},{}]", cx.div_euclid(16), cz.div_euclid(16)),
                [rect[0], rect[1] + side + 5.0],
                side,
                0xd6dfef,
            );
            label(
                game,
                batch,
                &format!(
                    "X {cx}  Z {cz} · {}",
                    owner.map_or("нет данных", |c| c.owner.as_str())
                ),
                [rect[0], rect[1] + side + 16.0],
                side,
                owner.map_or(0xaab4c8, |c| {
                    if c.own {
                        source.own_color
                    } else {
                        source.friend_color
                    }
                }),
            );
        } else {
            label(
                game,
                batch,
                &format!("Масштаб {:.2} б/п · колесо / ±", view.blocks_per_pixel),
                [rect[0], rect[1] + side + 5.0],
                side,
                0xaab7cc,
            );
            label(
                game,
                batch,
                "Перетаскивание ЛКМ · Home — к себе",
                [rect[0], rect[1] + side + 16.0],
                side,
                0xaab7cc,
            );
        }
        label(
            game,
            batch,
            if dimension != source.dimension {
                "Другой мир: только приваты"
            } else if source.own_known {
                "Рельеф: загруженные чанки"
            } else {
                "Свои приваты: обнови /claims"
            },
            [x, 208.0],
            width,
            0x8999af,
        );
        if p.labels && actual.blocks_per_pixel <= 2.0 {
            let mut owners = BTreeSet::new();
            for cell in source
                .cells
                .iter()
                .filter(|c| c.dimension == dimension && ((c.own && p.own) || (!c.own && p.friends)))
            {
                let at = actual.pixel([cell.x as f64 + 8.0, cell.z as f64 + 8.0], 128.0);
                let xy = [
                    rect[0] + at[0] / 128.0 * side,
                    rect[1] + at[1] / 128.0 * side,
                ];
                if contains(rect, xy) && owners.len() < 16 && owners.insert(cell.owner.clone()) {
                    let start = batch.vertices.len();
                    label(
                        game,
                        batch,
                        &cell.owner,
                        xy,
                        80.0,
                        if cell.own {
                            source.own_color
                        } else {
                            source.friend_color
                        },
                    );
                    batch.clip_since(start, rect[0], rect[1], rect[0] + side, rect[1] + side);
                }
            }
            if p.marks {
                let mut shown = 0;
                for mark in source.marks.iter().filter(|m| m.dimension == dimension) {
                    let at = actual.pixel(mark.position, 128.0);
                    let xy = [
                        rect[0] + at[0] / 128.0 * side + 3.0,
                        rect[1] + at[1] / 128.0 * side,
                    ];
                    if contains(rect, xy) && shown < 32 {
                        let start = batch.vertices.len();
                        label(game, batch, &mark.label, xy, 90.0, mark.color);
                        batch.clip_since(start, rect[0], rect[1], rect[0] + side, rect[1] + side);
                        shown += 1;
                    }
                }
            }
        }
    }
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
    if button == 0 {
        if let Some((button, _, _)) = buttons([screen.rect[2], screen.rect[3]])
            .into_iter()
            .find(|(_, r, _)| contains(*r, screen.mouse))
        {
            operate(game, button);
        } else {
            state(|s| {
                if s.rect.is_some_and(|r| contains(r, screen.mouse)) {
                    s.drag = Some(Drag {
                        mouse: screen.mouse,
                        center: s.view.center,
                    });
                    s.follow = false;
                }
            });
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
            s.view
                .zoom_at(pixel, 128.0, if amount > 0.0 { 0.5 } else { 2.0 });
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
        "key.keyboard.home" => operate(game, Button::Center),
        "key.keyboard.equal" => operate(game, Button::ZoomIn),
        "key.keyboard.minus" => operate(game, Button::ZoomOut),
        _ => {}
    }
    true
}
