// Saving an area of the world as a schematic (MIT glue over `litematica/selection.rs` and
// `litematica/writer.rs`, LGPL): the player marks corners (the block aimed at, or the one stood
// on), adds boxes, and Save reads the blocks of the loaded chunks a few milliseconds a tick, then
// writes a `.litematic` file in the schematics folder on a background thread. Blocks only:
// containers' contents, signs' text and entities are not saved.
use crate::game::{
    Game, claim_borders_litematica_selection as sel, claim_borders_litematica_writer as writer,
    claim_borders_schematic_places as places, claim_borders_workshop_view as workshop,
};
use horizon_client::{gizmos::Gizmos, text::Text};
use std::{
    collections::HashMap,
    sync::{
        Mutex,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
/// A selection holds at most this many cells (the importer's limit on one file).
const MAX_CELLS: u64 = 4_194_304;
const BUDGET: Duration = Duration::from_millis(3);

struct Job {
    regions: Vec<writer::Region>,
    boxes: Vec<(sel::Box, [i32; 3])>,
    region: usize,
    cursor: usize,
    states: HashMap<u32, writer::State>,
    /// Chunks of the boxes that are not loaded were left as air.
    unloaded: usize,
}
#[derive(Default)]
struct Capture {
    scope: String,
    selection: sel::Selection,
    job: Option<Job>,
    writing: Option<Receiver<Result<String, String>>>,
}
static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);

fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn with<T>(game: &Game<'_>, f: impl FnOnce(&mut Capture) -> T) -> T {
    let scope = crate::game::claim_borders_navigation_view::scope(game).unwrap_or_default();
    let mut guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
    let c = guard.get_or_insert_with(Capture::default);
    // Another world: the old one's selection is dropped.
    if c.scope != scope {
        *c = Capture {
            scope,
            ..Capture::default()
        };
    }
    f(c)
}
/// The block aimed at, or else the one stood on.
fn target(game: &Game<'_>) -> Option<[i32; 3]> {
    if let Some(hit) = game.pick() {
        return Some(hit.pos);
    }
    game.native_player()
        .map(|p| [p.x, p.y, p.z].map(|v| v.floor() as i32))
}
/// What a button does to the selection.
pub(super) fn edit(
    game: &mut Game<'_>,
    what: crate::game::claim_borders_menu::SelectionEdit,
) -> Result<String, String> {
    use crate::game::claim_borders_menu::SelectionEdit as E;
    if what == E::Save {
        return save(game);
    }
    let at = target(game).ok_or_else(|| text("Сначала зайдите в мир", "Join a world first"))?;
    with(game, |c| {
        match what {
            E::Corner1 => c.selection.set_corner(sel::Corner::One, at),
            E::Corner2 => c.selection.set_corner(sel::Corner::Two, at),
            E::NewBox => {
                c.selection
                    .new_box(at, "Region")
                    .ok_or_else(|| text("Слишком много регионов", "Too many regions"))?;
            }
            E::Clear => c.selection.clear(),
            E::Save => {}
        }
        Ok(summary(&c.selection))
    })
    .map_err(str::to_owned)
}
/// "3x5x4 · 2 regions", or nothing.
fn summary(s: &sel::Selection) -> String {
    let Some((lo, hi)) = s.enclosing() else {
        return String::new();
    };
    let size = [0, 1, 2].map(|i| hi[i] - lo[i] + 1);
    format!(
        "{}x{}x{} · {} {}",
        size[0],
        size[1],
        size[2],
        s.boxes().len(),
        text("рег.", "reg.")
    )
}
/// For the button row: the selection's size, and that a save is under way.
pub(super) fn status() -> String {
    let guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(c) = guard.as_ref() else {
        return String::new();
    };
    if let Some(job) = &c.job {
        let total: usize = job.regions.iter().map(writer::Region::volume).sum();
        let done: usize = job.regions[..job.region]
            .iter()
            .map(writer::Region::volume)
            .sum::<usize>()
            + job.cursor;
        return format!(
            "{} {}%",
            text("Чтение", "Reading"),
            done * 100 / total.max(1)
        );
    }
    if c.writing.is_some() {
        return text("Запись…", "Writing…").to_owned();
    }
    summary(&c.selection)
}
fn save(game: &Game<'_>) -> Result<String, String> {
    let shape = game.world.shape.clone();
    with(game, |c| {
        if c.job.is_some() || c.writing.is_some() {
            return Err(text("Сохранение уже идёт", "A save is already running").to_owned());
        }
        match c.selection.check(MAX_CELLS) {
            Ok(()) => {}
            Err("empty") => {
                return Err(text(
                    "Отметьте углы области кнопками «Угол 1» и «Угол 2»",
                    "Mark the area's corners with the Corner 1 and Corner 2 buttons",
                )
                .to_owned());
            }
            Err("incomplete") => {
                return Err(
                    text("У региона не заданы оба угла", "A region lacks a corner").to_owned(),
                );
            }
            Err(_) => {
                return Err(text(
                    "Область слишком велика (до 4 194 304 блоков)",
                    "The area is too large (up to 4,194,304 blocks)",
                )
                .to_owned());
            }
        }
        let (base, _) = c.selection.enclosing().ok_or("empty")?;
        let mut regions = Vec::new();
        let mut boxes = Vec::new();
        for b in c.selection.boxes() {
            let (lo, hi) = b.bounds().ok_or("incomplete")?;
            if lo[1] < shape.min_y || hi[1] >= shape.min_y + shape.height {
                return Err(text(
                    "Область выходит за высоту мира",
                    "The area leaves the world's height",
                )
                .to_owned());
            }
            let size = [0, 1, 2].map(|i| hi[i] - lo[i] + 1);
            let position = [0, 1, 2].map(|i| lo[i] - base[i]);
            let region = writer::Region::new(&b.name, position, size)
                .ok_or_else(|| text("Пустой регион", "An empty region"))?;
            regions.push(region);
            boxes.push((b.clone(), lo));
        }
        c.job = Some(Job {
            regions,
            boxes,
            region: 0,
            cursor: 0,
            states: HashMap::new(),
            unloaded: 0,
        });
        Ok(text("Читаю блоки области…", "Reading the area's blocks…").to_owned())
    })
}
/// Every tick: the read goes on within its budget; once done the file is written in the
/// background, and its end is told on the action bar.
pub(super) fn tick(game: &mut Game<'_>) {
    let mut done_message: Option<String> = None;
    let mut start_write: Option<(Vec<writer::Region>, usize)> = None;
    {
        let registry = &game.pack.assets.registry;
        let world = &game.world;
        let mut guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(c) = guard.as_mut() else {
            return;
        };
        if let Some(rx) = &c.writing {
            match rx.try_recv() {
                Ok(Ok(name)) => {
                    done_message = Some(format!(
                        "{} {name}",
                        text("Схема сохранена:", "Schematic saved:")
                    ));
                    c.writing = None;
                }
                Ok(Err(e)) => {
                    done_message = Some(format!(
                        "{}: {e}",
                        text("Не удалось сохранить", "Could not save")
                    ));
                    c.writing = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => c.writing = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(job) = c.job.as_mut() {
            let started = Instant::now();
            let mut n = 0u32;
            'scan: while job.region < job.regions.len() {
                let (b, lo) = &job.boxes[job.region];
                let _ = b;
                let size = job.regions[job.region].size;
                let total = job.regions[job.region].volume();
                while job.cursor < total {
                    let i = job.cursor;
                    job.cursor += 1;
                    let (sx, sz) = (size[0] as usize, size[2] as usize);
                    let (x, z, y) = (i % sx, i / sx % sz, i / (sx * sz));
                    let at = [lo[0] + x as i32, lo[1] + y as i32, lo[2] + z as i32];
                    if !world.chunks.contains_key(&(at[0] >> 4, at[2] >> 4)) {
                        job.unloaded += 1;
                        continue;
                    }
                    let id = world.block(at[0], at[1], at[2]);
                    if let Some(state) = job.states.get(&id.0).cloned().or_else(|| {
                        let s = registry.state(id)?;
                        let state = writer::State {
                            name: s.name.clone(),
                            properties: s
                                .properties
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect(),
                        };
                        job.states.insert(id.0, state.clone());
                        Some(state)
                    }) && state != writer::State::air()
                        && !matches!(
                            state.name.as_str(),
                            "minecraft:cave_air" | "minecraft:void_air"
                        )
                    {
                        job.regions[job.region].set(x as i32, y as i32, z as i32, &state);
                    }
                    n += 1;
                    if n % 256 == 0 && started.elapsed() > BUDGET {
                        break 'scan;
                    }
                }
                job.region += 1;
                job.cursor = 0;
            }
            if job.region >= job.regions.len() {
                let job = c.job.take().expect("checked above");
                start_write = Some((job.regions, job.unloaded));
            }
        }
    }
    if let Some((regions, unloaded)) = start_write {
        let folder = workshop::folder(game);
        let (tx, rx) = mpsc::sync_channel(1);
        let data_version = horizon_world::DATA_VERSION;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let author = "DraimHorizon".to_owned();
        let handle = std::thread::Builder::new()
            .name("horizon-schematic-save".into())
            .spawn(move || {
                horizon_threads::idle_current();
                let result = (|| {
                    let name = format!("selection-{}.litematic", places::file_stamp(secs));
                    let meta = writer::Metadata {
                        name: name.clone(),
                        author,
                        description: String::new(),
                        time_ms: secs as i64 * 1000,
                        data_version,
                    };
                    let nbt = writer::write(&meta, &regions);
                    let bytes = horizon_world::compression::Compression::Gzip
                        .encode(&nbt)
                        .map_err(|e| e.to_string())?;
                    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                    std::fs::write(folder.join(&name), bytes).map_err(|e| e.to_string())?;
                    Ok(name)
                })();
                let _ = tx.send(result);
            });
        let mut guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
        if let (Some(c), Ok(_)) = (guard.as_mut(), handle) {
            c.writing = Some(rx);
        }
        if unloaded > 0 {
            done_message = Some(format!(
                "{} ({unloaded} {})",
                text(
                    "Часть области не загружена, там воздух",
                    "Part of the area is not loaded, saved as air"
                ),
                text("блоков", "blocks")
            ));
        }
    }
    if let Some(message) = done_message {
        game.native_client_action_bar(Text::literal(message), false);
    }
}
/// The boxes as outlines: the one being edited brighter, and the corners marked.
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    let guard = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(c) = guard.as_ref() else {
        return;
    };
    let scope = crate::game::claim_borders_navigation_view::scope(game).unwrap_or_default();
    if c.scope != scope {
        return;
    }
    let current = c.selection.current().map(|b| b.name.clone());
    for b in c.selection.boxes() {
        let on = current.as_deref() == Some(b.name.as_str());
        if let Some((lo, hi)) = b.bounds() {
            g.cuboid(
                lo.map(f64::from),
                hi.map(|v| f64::from(v) + 1.0),
                if on { 0xff60_e0ff } else { 0xaa40_a0d0 },
                if on { 2.0 } else { 1.0 },
            );
        }
        for (corner, color) in [(b.pos1, 0xffff_6060u32), (b.pos2, 0xff60_60ff)] {
            if let Some(p) = corner {
                g.cuboid(
                    p.map(|v| f64::from(v) + 0.3),
                    p.map(|v| f64::from(v) + 0.7),
                    color,
                    2.0,
                );
            }
        }
    }
}
