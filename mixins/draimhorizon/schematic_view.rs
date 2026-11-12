// Schematic previews in the world: a hologram of each placed schematic (what is still to be
// built, and what stands there wrongly), kept per world across sessions, moved and turned with
// keys while an on-screen panel shows where it stands. The builder builds the selected one.
use crate::game::{
    Game, claim_borders as claims, claim_borders_navigation as nav,
    claim_borders_navigation_view as walk, claim_borders_schematic_files as files,
    claim_borders_schematic_places as places, claim_borders_workshop_view as workshop,
};
use horizon_client::gizmos::Gizmos;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc::Receiver},
};
const SETTING: &str = "schematic_places";
/// The previews (all worlds) and which one is selected, loaded from the settings once.
struct Places {
    loaded: bool,
    list: Vec<places::Placement>,
    active: Option<usize>,
}
static PLACES: Mutex<Places> = Mutex::new(Places {
    loaded: false,
    list: Vec::new(),
    active: None,
});
/// A file read for its preview: its blocks relative to its origin (an index into its distinct
/// states), and their box.
pub(super) struct Shape {
    pub(super) cells: HashMap<places::Pos, u32>,
    pub(super) states: Vec<crate::game::claim_borders_schematic::BlockState>,
    pub(super) min: places::Pos,
    pub(super) max: places::Pos,
}
enum Loading {
    Pending(Receiver<Result<files::Response, String>>),
    Ready(Arc<Shape>),
    /// The file could not be read, and why (shown in the list and on the panel).
    Failed(String),
}
static SHAPES: Mutex<Option<HashMap<String, Loading>>> = Mutex::new(None);
fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn with_places<T>(game: &Game<'_>, f: impl FnOnce(&mut Places) -> T) -> T {
    let mut p = PLACES.lock().unwrap_or_else(|e| e.into_inner());
    if !p.loaded {
        p.loaded = true;
        p.list = game
            .native_setting(claims::SETTINGS_ID, SETTING)
            .map(|s| places::decode(&s))
            .unwrap_or_default();
        // Previews kept by an older version under a world key that changed every session.
        for preview in &mut p.list {
            preview.scope = places::stable_world(&preview.scope);
        }
    }
    f(&mut p)
}
fn save(game: &Game<'_>, p: &Places) {
    game.native_set_setting(claims::SETTINGS_ID, SETTING, &places::encode(&p.list));
}
/// This player's choice of layers to draw (every preview, friends' too), kept in the settings.
const LAYERS: &str = "schematic_layers";
static LAYER_CHOICE: Mutex<Option<(places::LayerMode, i32)>> = Mutex::new(None);
pub(super) fn layers(game: &Game<'_>) -> (places::LayerMode, i32) {
    *LAYER_CHOICE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| {
            game.native_setting(claims::SETTINGS_ID, LAYERS)
                .map(|v| places::LayerMode::decode(&v))
                .unwrap_or_default()
        })
}
pub(super) fn set_layers(game: &Game<'_>, mode: places::LayerMode, layer: i32) {
    let layer = layer.clamp(0, places::MAX_LAYER);
    *LAYER_CHOICE.lock().unwrap_or_else(|e| e.into_inner()) = Some((mode, layer));
    game.native_set_setting(claims::SETTINGS_ID, LAYERS, &mode.encode(layer));
}
/// The world a preview belongs to: kept across reconnects and restarts.
fn here(game: &Game<'_>) -> Option<String> {
    walk::world_key(game)
}
/// The selected preview of this world.
pub(super) fn active(game: &Game<'_>) -> Option<places::Placement> {
    let scope = here(game)?;
    with_places(game, |p| {
        p.active
            .and_then(|i| p.list.get(i))
            .filter(|pl| pl.scope == scope)
            .cloned()
    })
}
/// The Preview key on the Schematics tab: the chosen file at the chosen place becomes the
/// selected preview (the selected one of the same file is moved there instead of a second one).
pub(super) fn preview(game: &Game<'_>, file: String, origin: places::Pos, rotation: u8) {
    let Some(scope) = here(game) else {
        return;
    };
    with_places(game, |p| {
        let same = p.active.filter(|i| {
            p.list
                .get(*i)
                .is_some_and(|pl| pl.scope == scope && pl.file == file)
        });
        let index = match same {
            Some(i) => i,
            None => {
                if p.list.len() >= places::MAX {
                    p.list.remove(0);
                }
                p.list.push(places::Placement {
                    scope: scope.clone(),
                    file: file.clone(),
                    origin,
                    rotation,
                    visible: true,
                });
                p.list.len() - 1
            }
        };
        if let Some(pl) = p.list.get_mut(index) {
            pl.origin = origin;
            pl.rotation = rotation % 4;
            pl.visible = true;
        }
        p.active = Some(index);
        save(game, p);
    });
}
/// The previews of this world for the menu: (index, label, shown, selected).
pub(super) fn rows(game: &Game<'_>) -> Vec<(usize, String, bool, bool)> {
    let Some(scope) = here(game) else {
        return Vec::new();
    };
    with_places(game, |p| {
        p.list
            .iter()
            .enumerate()
            .filter(|(_, pl)| pl.scope == scope)
            .map(|(i, pl)| {
                let [x, y, z] = pl.origin;
                let error = load_error(&pl.file)
                    .map(|e| format!(" · {}: {e}", text("не читается", "unreadable")))
                    .unwrap_or_default();
                (
                    i,
                    format!(
                        "{} · {x} {y} {z} · {}°{error}",
                        pl.file,
                        u16::from(pl.rotation) * 90
                    ),
                    pl.visible,
                    p.active == Some(i),
                )
            })
            .collect()
    })
}
/// Selects a preview (from the menu list).
pub(super) fn select(game: &Game<'_>, index: usize) {
    with_places(game, |p| {
        if index < p.list.len() {
            p.active = Some(index);
            p.list[index].visible = true;
            save(game, p);
        }
    });
}
/// Shows or hides a preview.
pub(super) fn toggle(game: &Game<'_>, index: usize) {
    with_places(game, |p| {
        if let Some(pl) = p.list.get_mut(index) {
            pl.visible = !pl.visible;
            save(game, p);
        }
    });
}
/// Forgets a preview for good.
pub(super) fn remove(game: &Game<'_>, index: usize) {
    with_places(game, |p| {
        if index < p.list.len() {
            p.list.remove(index);
            p.active = match p.active {
                Some(a) if a == index => None,
                Some(a) if a > index => Some(a - 1),
                other => other,
            };
            save(game, p);
        }
    });
}
/// The file's blocks for drawing, read in the background the first time.
fn shape(game: &Game<'_>, file: &str) -> Option<Arc<Shape>> {
    shape_in(workshop::folder(game), file, file)
}
/// A file of `folder` read for drawing, kept under `key` (a friend's file is kept apart from the
/// player's own of the same name).
fn shape_in(folder: std::path::PathBuf, key: &str, file: &str) -> Option<Arc<Shape>> {
    let mut guard = SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    let shapes = guard.get_or_insert_with(HashMap::new);
    if !shapes.contains_key(key) {
        let loading = match files::spawn(folder, Some(file.to_owned()), [0; 3], 0) {
            Ok(rx) => Loading::Pending(rx),
            Err(e) => Loading::Failed(e),
        };
        shapes.insert(key.to_owned(), loading);
    }
    let entry = shapes.get_mut(key)?;
    if let Loading::Pending(rx) = entry {
        match rx.try_recv() {
            Ok(Ok(files::Response::Loaded(blueprint))) => {
                let mut cells = HashMap::new();
                let mut states = Vec::new();
                let mut index = std::collections::BTreeMap::new();
                let mut min = [i32::MAX; 3];
                let mut max = [i32::MIN; 3];
                for (at, state) in blueprint.blocks {
                    if state.air() {
                        continue;
                    }
                    for i in 0..3 {
                        min[i] = min[i].min(at[i]);
                        max[i] = max[i].max(at[i]);
                    }
                    // Looked up first: a large file repeats a few states many times over.
                    let i = match index.get(&state) {
                        Some(i) => *i,
                        None => {
                            let i = states.len() as u32;
                            index.insert(state.clone(), i);
                            states.push(state);
                            i
                        }
                    };
                    cells.insert(at, i);
                }
                if cells.is_empty() {
                    min = [0; 3];
                    max = [0; 3];
                }
                *entry = Loading::Ready(Arc::new(Shape {
                    cells,
                    states,
                    min,
                    max,
                }));
            }
            Ok(Err(e)) => *entry = Loading::Failed(e),
            Ok(_) => *entry = Loading::Failed("unexpected answer".into()),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                *entry = Loading::Failed("reading stopped".into());
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    match entry {
        Loading::Ready(shape) => Some(shape.clone()),
        _ => None,
    }
}
/// Forgets the files that could not be read, so they are read again when next shown.
pub(super) fn forget_failed() {
    let mut guard = SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(shapes) = guard.as_mut() {
        shapes.retain(|_, l| !matches!(l, Loading::Failed(_)));
    }
}
/// Why a preview's file could not be read, if it could not.
fn load_error(file: &str) -> Option<String> {
    let guard = SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref()?.get(file)? {
        Loading::Failed(e) => Some(e.clone()),
        _ => None,
    }
}
/// The shown previews of this world whose files are read, the selected one marked.
pub(super) fn shown(game: &Game<'_>) -> Vec<(places::Placement, Arc<Shape>, bool)> {
    let Some(scope) = here(game) else {
        return Vec::new();
    };
    let list: Vec<(places::Placement, bool)> = with_places(game, |pl| {
        pl.list
            .iter()
            .enumerate()
            .filter(|(_, x)| x.scope == scope && x.visible)
            .map(|(i, x)| (x.clone(), pl.active == Some(i)))
            .collect()
    });
    let mut shown: Vec<_> = list
        .into_iter()
        .filter_map(|(pl, selected)| shape(game, &pl.file).map(|s| (pl, s, selected)))
        .collect();
    // Friends' previews of this world (shown on this screen, their files kept): drawn the same way,
    // never selected (only their owner edits them).
    let folder = crate::game::claim_borders_schematic_share_view::shared_folder(game);
    for (file, pl) in crate::game::claim_borders_schematic_share_view::shown(game) {
        if let Some(shape) = shape_in(folder.clone(), &format!("shared:{file}"), &file) {
            shown.push((pl, shape, false));
        }
    }
    shown
}
/// The player's own shown previews of this world, in the order they were placed (shared with
/// friends).
pub(super) fn own_shown(game: &Game<'_>) -> Vec<places::Placement> {
    let Some(scope) = here(game) else {
        return Vec::new();
    };
    with_places(game, |pl| {
        pl.list
            .iter()
            .filter(|x| x.scope == scope && x.visible)
            .cloned()
            .collect()
    })
}
/// Draws the box of each shown preview of this world; the blocks themselves are ghosts
/// (`schematic_ghosts`): what is still to be placed as its own translucent model, what stands
/// there wrongly in red.
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let (mode, layer) = layers(game);
    for (pl, shape, selected) in shown(game) {
        let (lo, hi) = places::bounds(shape.min, shape.max, pl.origin, pl.rotation);
        g.cuboid(
            lo.map(f64::from),
            hi.map(|v| f64::from(v) + 1.0),
            if selected { 0xffffd060 } else { 0xaa88aacc },
            if selected { 2.0 } else { 1.0 },
        );
        // The layer drawn (or the top one drawn) is outlined in the box.
        if mode != places::LayerMode::All {
            let y = lo[1] + layer.min(hi[1] - lo[1]);
            g.cuboid(
                [f64::from(lo[0]), f64::from(y), f64::from(lo[2])],
                [
                    f64::from(hi[0]) + 1.0,
                    f64::from(y) + 1.0,
                    f64::from(hi[2]) + 1.0,
                ],
                0xff60d0ff,
                1.5,
            );
        }
    }
}
/// The editor's keys: (id, default key, Russian name, English name).
pub(super) const KEYS: [(&str, &str, &str, &str); 9] = [
    (
        "schematic_toggle",
        "key.keyboard.insert",
        "Схема: показать/скрыть",
        "Schematic: show/hide",
    ),
    (
        "schematic_remove",
        "key.keyboard.delete",
        "Схема: убрать предпоказ",
        "Schematic: remove preview",
    ),
    (
        "schematic_rotate",
        "key.keyboard.home",
        "Схема: повернуть",
        "Schematic: rotate",
    ),
    (
        "schematic_here",
        "key.keyboard.end",
        "Схема: перенести ко мне",
        "Schematic: move to me",
    ),
    (
        "schematic_forward",
        "key.keyboard.up",
        "Схема: вперёд",
        "Schematic: forward",
    ),
    (
        "schematic_back",
        "key.keyboard.down",
        "Схема: назад",
        "Schematic: back",
    ),
    (
        "schematic_left",
        "key.keyboard.left",
        "Схема: влево",
        "Schematic: left",
    ),
    (
        "schematic_right",
        "key.keyboard.right",
        "Схема: вправо",
        "Schematic: right",
    ),
    (
        "schematic_up_down",
        "key.keyboard.page.up",
        "Схема: вверх (с Shift вниз)",
        "Schematic: up (Shift: down)",
    ),
];
static DOWN: Mutex<[bool; 9]> = Mutex::new([false; 9]);
/// Every tick while playing: the editor's keys act on the selected preview of this world.
pub(super) fn keys(game: &mut Game<'_>) {
    let playing = game.native_on(claims::ID)
        && game.native_playing()
        && !game.screen_over_playing()
        && game.player.is_some();
    let mut down = DOWN.lock().unwrap_or_else(|e| e.into_inner());
    let pressed: Vec<bool> = KEYS
        .iter()
        .map(|(id, default, _, _)| {
            let key = game.native_mod_key(claims::ID, id, default);
            horizon_client::keybinds::key_code(&key)
                .is_some_and(|code| game.physical_keys.contains(&code))
        })
        .collect();
    let mut fresh = [false; 9];
    for i in 0..9 {
        fresh[i] = playing && pressed[i] && !down[i];
        down[i] = pressed[i];
    }
    drop(down);
    if !fresh.iter().any(|f| *f) {
        return;
    }
    let Some(scope) = here(game) else {
        return;
    };
    let yaw = game.yaw;
    let feet = game.native_player().map(|p| {
        nav::standing_block(
            [p.x, p.y, p.z],
            &game.native_block(nav::block([p.x, p.y, p.z])),
        )
    });
    let shift = game.modifiers.shift_key();
    let message = with_places(game, |p| {
        let index = p
            .active
            .filter(|i| p.list.get(*i).is_some_and(|pl| pl.scope == scope))?;
        let mut removed = false;
        {
            let pl = &mut p.list[index];
            // Moving keys only act on a shown preview; showing and removing always.
            let shown = pl.visible;
            if fresh[0] {
                pl.visible = !pl.visible;
            }
            if shown {
                if fresh[2] {
                    pl.rotation = (pl.rotation + 1) % 4;
                }
                if fresh[3]
                    && let Some(feet) = feet
                {
                    pl.origin = feet;
                }
                let forward = i32::from(fresh[4]) - i32::from(fresh[5]);
                let right = i32::from(fresh[7]) - i32::from(fresh[6]);
                let up = if fresh[8] {
                    if shift { -1 } else { 1 }
                } else {
                    0
                };
                pl.origin = places::nudge(pl.origin, yaw, forward, right, up);
            }
            if fresh[1] {
                removed = true;
            }
        }
        let label = p.list[index].file.clone();
        if removed {
            p.list.remove(index);
            p.active = None;
        }
        save(game, p);
        Some(if removed {
            format!("{}: {label}", text("Предпоказ убран", "Preview removed"))
        } else {
            String::new()
        })
    });
    if let Some(message) = message.filter(|m| !m.is_empty()) {
        game.native_chat(&message);
    }
}
/// The editor panel: while the selected preview of this world is shown, where it stands, how it
/// is turned and the keys that move it.
/// The panel of the selected preview: a card with the file, where it stands, how much is left
/// and the editor's keys as key caps.
pub(super) fn hud(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::{claim_borders_ui_style as style, claim_borders_ui_view as ui};
    if !game.native_playing() || game.ui.hide_gui {
        return;
    }
    let Some(pl) = active(game).filter(|p| p.visible) else {
        return;
    };
    let key = |i: usize| {
        let (id, default, _, _) = KEYS[i];
        places::key_cap(&game.native_mod_key(claims::ID, id, default))
    };
    let accent = ui::preferences(game).accent.color();
    let [x, y, z] = pl.origin;
    let error = load_error(&pl.file);
    let blocks = SHAPES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| match m.get(&pl.file) {
            Some(Loading::Ready(shape)) => Some(shape.cells.len()),
            _ => None,
        });
    let (todo, wrong) = crate::game::claim_borders_schematic_ghosts::nearby_counts();
    let layer = match layers(game) {
        (places::LayerMode::All, _) => String::new(),
        (places::LayerMode::Single, n) => format!(" · {} {}", text("слой", "layer"), n + 1),
        (places::LayerMode::Below, n) => format!(" · {} 1–{}", text("слои", "layers"), n + 1),
    };
    let place = format!("XYZ {x} {y} {z} · {}°{layer}", u16::from(pl.rotation) * 90);
    // Status chips: (colour, text).
    let mut chips: Vec<(u32, String)> = Vec::new();
    match (&error, blocks) {
        (Some(e), _) => chips.push((
            style::BAD,
            format!("{}: {e}", text("Не читается", "Unreadable")),
        )),
        (None, None) => chips.push((
            style::MUTED,
            text("Файл читается…", "Reading the file…").into(),
        )),
        (None, Some(0)) => chips.push((
            style::WARN,
            text("В файле нет блоков", "No blocks in the file").into(),
        )),
        (None, Some(n)) => {
            chips.push((style::MUTED, format!("{n} {}", text("блоков", "blocks"))));
            if todo == 0 && wrong == 0 {
                chips.push((
                    style::GOOD,
                    text("Рядом всё совпадает", "Nearby all matches").into(),
                ));
            } else {
                if todo > 0 {
                    chips.push((
                        style::WARN,
                        format!("{} {todo}", text("Поставить", "To place")),
                    ));
                }
                if wrong > 0 {
                    chips.push((style::BAD, format!("{} {wrong}", text("Неверных", "Wrong"))));
                }
            }
        }
    }
    // The four move keys get a row of their own; the rest sit in two columns below.
    let moves = [key(4), key(5), key(6), key(7)];
    let move_label = text("Двигать", "Move");
    let keys = [
        (key(2), text("Повернуть", "Turn")),
        (key(3), text("Ко мне", "To me")),
        (key(0), text("Скрыть", "Hide")),
        (key(1), text("Убрать", "Remove")),
        (key(8), text("Выше · Shift ниже", "Up · Shift down")),
    ];
    // Sizes.
    let pad = 8.0;
    let line = 12.0;
    let cap_h = 11.0;
    let width_of = |s: &str| game.native_text_width(s);
    let cap_w = |s: &str| width_of(s) + 8.0;
    let column = |c: usize| {
        // A last key on its own row does not widen the columns.
        keys[..keys.len() / 2 * 2]
            .iter()
            .skip(c)
            .step_by(2)
            .map(|(k, l)| cap_w(k) + 4.0 + width_of(l))
            .fold(0.0f32, f32::max)
    };
    let moves_w = moves.iter().map(|k| cap_w(k) + 3.0).sum::<f32>() + 1.0 + width_of(move_label);
    let lone = keys
        .get(keys.len() / 2 * 2)
        .map_or(0.0, |(k, l)| cap_w(k) + 4.0 + width_of(l));
    let keys_w = (column(0) + 14.0 + column(1)).max(moves_w).max(lone);
    let chips_w: f32 = chips.iter().map(|(_, t)| width_of(t) + 14.0).sum::<f32>()
        + 4.0 * chips.len().saturating_sub(1) as f32;
    let title_w = 16.0 + width_of(&pl.file);
    let inner = keys_w
        .max(chips_w)
        .max(title_w)
        .max(width_of(&place))
        .clamp(200.0, (batch.width * 0.45).max(200.0));
    let rows = 1.0 + keys.len().div_ceil(2) as f32;
    let height =
        pad + line + 2.0 + line + 4.0 + cap_h + 6.0 + 1.0 + 5.0 + rows * (cap_h + 3.0) + pad - 3.0;
    let left = 6.0;
    let top = (batch.height * 0.35)
        .min(batch.height - height - 6.0)
        .max(6.0);
    let card = [left, top, inner + 2.0 * pad, height];
    ui::rounded(game, batch, card, 6.0, 0xe6000000 | style::BACKGROUND);
    ui::rounded(
        game,
        batch,
        [left, top + 6.0, 2.0, height - 12.0],
        1.0,
        0xff000000 | accent,
    );
    let x0 = left + pad;
    let mut cy = top + pad;
    // Title: icon and file.
    ui::icon(
        game,
        batch,
        style::Icon::Layers,
        [x0, cy - 1.0, 10.0, 10.0],
        accent,
    );
    ui::text(
        game,
        batch,
        &ui::clip(game, &pl.file, inner - 16.0),
        [x0 + 14.0, cy],
        style::TEXT,
        false,
    );
    cy += line + 2.0;
    ui::text(
        game,
        batch,
        &ui::clip(game, &place, inner),
        [x0, cy],
        style::MUTED,
        false,
    );
    cy += line + 4.0;
    // Status chips.
    let mut cx = x0;
    for (color, label) in &chips {
        let label = ui::clip(game, label, (x0 + inner - cx - 14.0).max(0.0));
        if label.is_empty() {
            break;
        }
        let w = width_of(&label) + 14.0;
        ui::rounded(
            game,
            batch,
            [cx, cy, w, cap_h],
            3.0,
            0xff000000 | style::SURFACE,
        );
        ui::rounded(
            game,
            batch,
            [cx + 4.0, cy + 4.0, 3.0, 3.0],
            1.5,
            0xff000000 | color,
        );
        ui::text(game, batch, &label, [cx + 10.0, cy + 2.0], *color, false);
        cx += w + 4.0;
    }
    cy += cap_h + 6.0;
    // Divider, then the keys in two columns: key cap, then what it does.
    game.native_fill(batch, [x0, cy, inner, 1.0], 0x40000000 | style::BORDER);
    cy += 5.0;
    let mut kx = x0;
    for cap in &moves {
        let w = cap_w(cap);
        ui::rounded(
            game,
            batch,
            [kx, cy, w, cap_h],
            2.5,
            0xff000000 | style::RAISED,
        );
        game.native_fill(
            batch,
            [kx + 1.0, cy + cap_h - 1.0, w - 2.0, 1.0],
            0x80000000 | style::BORDER,
        );
        ui::text(game, batch, cap, [kx + 4.0, cy + 2.0], style::TEXT, false);
        kx += w + 3.0;
    }
    ui::text(
        game,
        batch,
        move_label,
        [kx + 1.0, cy + 2.0],
        style::MUTED,
        false,
    );
    cy += cap_h + 3.0;
    let second = x0 + column(0) + 14.0;
    for (i, (cap, label)) in keys.iter().enumerate() {
        let kx = if i % 2 == 0 { x0 } else { second };
        let ky = cy + (i / 2) as f32 * (cap_h + 3.0);
        let w = cap_w(cap);
        ui::rounded(
            game,
            batch,
            [kx, ky, w, cap_h],
            2.5,
            0xff000000 | style::RAISED,
        );
        game.native_fill(
            batch,
            [kx + 1.0, ky + cap_h - 1.0, w - 2.0, 1.0],
            0x80000000 | style::BORDER,
        );
        ui::text(game, batch, cap, [kx + 4.0, ky + 2.0], style::TEXT, false);
        ui::text(
            game,
            batch,
            label,
            [kx + w + 4.0, ky + 2.0],
            style::MUTED,
            false,
        );
    }
}
/// The editor's buttons on the selected preview of this world.
pub(super) fn edit_active(
    game: &mut Game<'_>,
    edit: crate::game::claim_borders_menu::SchematicEdit,
) -> Result<(), String> {
    use crate::game::claim_borders_menu::SchematicEdit as E;
    let scope = here(game).ok_or("Join a world first")?;
    let feet = game.native_player().map(|p| {
        nav::standing_block(
            [p.x, p.y, p.z],
            &game.native_block(nav::block([p.x, p.y, p.z])),
        )
    });
    let mut look = None;
    with_places(game, |p| {
        let index = p
            .active
            .filter(|i| p.list.get(*i).is_some_and(|pl| pl.scope == scope))
            .ok_or_else(|| {
                text(
                    "Сначала поставьте или выберите предпоказ",
                    "Place or select a preview first",
                )
                .to_owned()
            })?;
        match edit {
            E::Move(d) => {
                let pl = &mut p.list[index];
                pl.origin = [
                    pl.origin[0] + d[0],
                    pl.origin[1] + d[1],
                    pl.origin[2] + d[2],
                ];
                pl.visible = true;
            }
            E::Rotate => {
                let pl = &mut p.list[index];
                pl.rotation = (pl.rotation + 1) % 4;
                pl.visible = true;
            }
            E::Here => {
                if let Some(feet) = feet {
                    p.list[index].origin = feet;
                    p.list[index].visible = true;
                }
            }
            E::Toggle => p.list[index].visible = !p.list[index].visible,
            E::Remove => {
                p.list.remove(index);
                p.active = None;
            }
            E::Look => look = Some(p.list[index].origin),
            E::Build => {}
        }
        save(game, p);
        Ok::<(), String>(())
    })?;
    if let (Some(at), Some(eye)) = (look, game.eye_position()) {
        let middle = nav::center(at);
        game.yaw = nav::yaw_to(eye, middle);
        game.pitch = nav::pitch_to(eye, middle);
    }
    Ok(())
}
/// The selected preview's file, origin and turn, for the builder.
pub(super) fn build_target(game: &Game<'_>) -> Option<(String, places::Pos, u8)> {
    active(game).map(|p| (p.file, p.origin, p.rotation))
}
