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
/// A file read for its preview: its blocks relative to its origin, and their box.
struct Shape {
    blocks: Vec<(places::Pos, String)>,
    min: places::Pos,
    max: places::Pos,
}
enum Loading {
    Pending(Receiver<Result<files::Response, String>>),
    Ready(Arc<Shape>),
    Failed,
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
    }
    f(&mut p)
}
fn save(game: &Game<'_>, p: &Places) {
    game.native_set_setting(claims::SETTINGS_ID, SETTING, &places::encode(&p.list));
}
fn here(game: &Game<'_>) -> Option<String> {
    walk::scope(game)
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
                (
                    i,
                    format!(
                        "{} · {x} {y} {z} · {}°",
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
    let mut guard = SHAPES.lock().unwrap_or_else(|e| e.into_inner());
    let shapes = guard.get_or_insert_with(HashMap::new);
    if !shapes.contains_key(file) {
        let loading = match files::spawn(workshop::folder(game), Some(file.to_owned()), [0; 3], 0) {
            Ok(rx) => Loading::Pending(rx),
            Err(_) => Loading::Failed,
        };
        shapes.insert(file.to_owned(), loading);
    }
    let entry = shapes.get_mut(file)?;
    if let Loading::Pending(rx) = entry {
        match rx.try_recv() {
            Ok(Ok(files::Response::Loaded(blueprint))) => {
                let blocks: Vec<(places::Pos, String)> = blueprint
                    .blocks
                    .into_iter()
                    .filter(|(_, s)| !s.name.ends_with(":air") && !s.name.ends_with("cave_air"))
                    .map(|(at, s)| (at, s.name))
                    .collect();
                let mut min = [i32::MAX; 3];
                let mut max = [i32::MIN; 3];
                for (at, _) in &blocks {
                    for i in 0..3 {
                        min[i] = min[i].min(at[i]);
                        max[i] = max[i].max(at[i]);
                    }
                }
                if blocks.is_empty() {
                    min = [0; 3];
                    max = [0; 3];
                }
                *entry = Loading::Ready(Arc::new(Shape { blocks, min, max }));
            }
            Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => *entry = Loading::Failed,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    match entry {
        Loading::Ready(shape) => Some(shape.clone()),
        _ => None,
    }
}
/// Draws the shown previews of this world: the box, and near the player each block still to be
/// placed (blue) or standing there wrongly (red); what is built is not drawn.
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let (Some(scope), Some(p)) = (here(game), game.native_player()) else {
        return;
    };
    let me = [p.x, p.y, p.z];
    let shown: Vec<(places::Placement, bool)> = with_places(game, |pl| {
        pl.list
            .iter()
            .enumerate()
            .filter(|(_, x)| x.scope == scope && x.visible)
            .map(|(i, x)| (x.clone(), pl.active == Some(i)))
            .collect()
    });
    let mut budget = 2000;
    for (pl, selected) in shown {
        let Some(shape) = shape(game, &pl.file) else {
            continue;
        };
        let (lo, hi) = places::bounds(shape.min, shape.max, pl.origin, pl.rotation);
        g.cuboid(
            lo.map(f64::from),
            hi.map(|v| f64::from(v) + 1.0),
            if selected { 0xffffd060 } else { 0xaa88aacc },
            if selected { 2.0 } else { 1.0 },
        );
        for (rel, name) in &shape.blocks {
            if budget == 0 {
                break;
            }
            let at = places::place(*rel, pl.origin, pl.rotation);
            if nav::distance(nav::center(at), me) > 40.0 {
                continue;
            }
            let live = game.native_block(at);
            if &live == name {
                continue;
            }
            let empty = matches!(live.as_str(), "minecraft:air" | "minecraft:cave_air")
                || crate::game::claim_borders_navigation::passable(&live);
            let color = if empty { 0xcc55ccff } else { 0xccff5555 };
            let min = at.map(|v| f64::from(v) + 0.08);
            let max = at.map(|v| f64::from(v) + 0.92);
            g.cuboid(min, max, color, 1.0);
            budget -= 1;
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
pub(super) fn hud(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::claim_borders_ui_view as ui;
    if !game.native_playing() || game.ui.hide_gui {
        return;
    }
    let Some(pl) = active(game).filter(|p| p.visible) else {
        return;
    };
    let key = |i: usize| {
        let (id, default, _, _) = KEYS[i];
        let name = game.native_mod_key(claims::ID, id, default);
        name.rsplit('.').next().unwrap_or("").to_uppercase()
    };
    let [x, y, z] = pl.origin;
    let lines = [
        format!("{}: {}", text("Схема", "Schematic"), pl.file),
        format!("XYZ {x} {y} {z} · {}°", u16::from(pl.rotation) * 90),
        format!(
            "{} {}/{}/{}/{} · {} {} · {} {}",
            text("двигать", "move"),
            key(4),
            key(5),
            key(6),
            key(7),
            text("выше", "up"),
            key(8),
            text("поворот", "turn"),
            key(2)
        ),
        format!(
            "{} {} · {} {} · {} {}",
            text("ко мне", "to me"),
            key(3),
            text("скрыть", "hide"),
            key(0),
            text("убрать", "remove"),
            key(1)
        ),
    ];
    let width = lines
        .iter()
        .map(|l| game.native_text_width(l))
        .fold(0.0f32, f32::max)
        + 16.0;
    let top = batch.height * 0.35;
    ui::fill(
        game,
        batch,
        [6.0, top, width, 12.0 * lines.len() as f32 + 8.0],
        0xcc1d2a3a,
    );
    for (i, line) in lines.iter().enumerate() {
        ui::text(
            game,
            batch,
            line,
            [14.0, top + 5.0 + 12.0 * i as f32],
            if i == 0 { 0xffd060 } else { 0xd8e2ec },
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
