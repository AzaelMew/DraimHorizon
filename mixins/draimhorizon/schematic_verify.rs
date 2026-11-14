// The schematic verifier of the Schematics tab (MIT): the selected preview is compared with the
// world cell by cell, a few milliseconds a tick, with Litematica's classification
// (`litematica/verifier.rs`, LGPL). The mismatches are drawn as boxes in the colours of the
// original, nearest first, and counted on the preview card. While it is on, it looks again every
// few seconds, so it follows a build.
use crate::game::{
    Game, claim_borders_litematica_verifier as ver, claim_borders_schematic_ghosts as ghosts,
    claim_borders_schematic_places as places, claim_borders_schematic_view as view,
};
use horizon_client::gizmos::Gizmos;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// Time the scan may take per tick.
const BUDGET: Duration = Duration::from_millis(3);
/// How long a finished scan stands before the next one.
const AGAIN: Duration = Duration::from_secs(4);
/// Boxes drawn at most (the original's `verifierErrorHilightMaxPositions`).
const MAX_BOXES: usize = 400;
/// A scan reads no more cells than this.
const MAX_CELLS: u64 = 16_000_000;

struct Run {
    placement: places::Placement,
    shape: Arc<view::Shape>,
    /// The game's state for each distinct state of the file, turned as placed.
    ids: Vec<Option<u32>>,
    air: u32,
    lo: places::Pos,
    size: [u64; 3],
    cursor: u64,
    verifier: ver::Verifier,
    finished: Option<Instant>,
    /// Which type is drawn: `None` is every type.
    kind: Option<ver::Mismatch>,
    /// The nearest positions to draw, looked up again when the scan ends or the player moves.
    boxes: Vec<(ver::Mismatch, places::Pos)>,
    boxes_at: Option<(Instant, places::Pos)>,
    /// The last finished scan's numbers, kept while the next one runs.
    last: Option<Totals>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Totals {
    pub counts: [usize; 5],
    pub correct: usize,
    pub wanted: usize,
}
static RUN: Mutex<Option<Run>> = Mutex::new(None);

fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
struct Blocks<'a>(&'a Game<'a>);
impl Blocks<'_> {
    fn name(&self, s: u32) -> &str {
        &self
            .0
            .res
            .facts()
            .get(horizon_world::registry::BlockStateId(s))
            .name
    }
}
impl ver::States for Blocks<'_> {
    fn is_air(&self, s: u32) -> bool {
        matches!(
            self.name(s),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
    fn same_block(&self, a: u32, b: u32) -> bool {
        self.name(a) == self.name(b)
    }
    fn is_liquid(&self, s: u32) -> bool {
        matches!(self.name(s), "minecraft:water" | "minecraft:lava")
    }
}

/// Whether the verifier is on.
pub(super) fn active() -> bool {
    RUN.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}
/// Switches the verifier on for the selected preview, or off.
pub(super) fn toggle(game: &Game<'_>) -> Result<(), String> {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.take().is_some() {
        return Ok(());
    }
    let pl = view::active(game).ok_or_else(|| {
        text(
            "Сначала поставьте или выберите предпоказ",
            "Place or select a preview first",
        )
        .to_owned()
    })?;
    let shape = view::shape(game, &pl.file)
        .ok_or_else(|| text("Файл ещё читается", "The file is still being read").to_owned())?;
    *guard = Some(begin(game, pl, shape)?);
    Ok(())
}
fn begin(
    game: &Game<'_>,
    placement: places::Placement,
    shape: Arc<view::Shape>,
) -> Result<Run, String> {
    let (lo, hi) = places::bounds(shape.min, shape.max, placement.origin, placement.rotation);
    let size = [0, 1, 2].map(|i| (i64::from(hi[i]) - i64::from(lo[i]) + 1) as u64);
    if size.iter().product::<u64>() > MAX_CELLS {
        return Err(text("Схема слишком велика для проверки", "Too large to verify").into());
    }
    let ids = shape
        .states
        .iter()
        .map(|s| ghosts::resolve(game, &s.transformed(placement.rotation)).map(|id| id.0))
        .collect();
    let air = game
        .pack
        .assets
        .registry
        .default_state("minecraft:air")
        .ok_or("no air")?
        .0;
    Ok(Run {
        placement,
        shape,
        ids,
        air,
        lo,
        size,
        cursor: 0,
        verifier: ver::Verifier::new(ver::Options {
            ignore_fluids: true,
            different_blocks: false,
        }),
        finished: None,
        kind: None,
        boxes: Vec::new(),
        boxes_at: None,
        last: None,
    })
}
fn totals(v: &ver::Verifier) -> Totals {
    Totals {
        counts: ver::Mismatch::ALL.map(|k| v.count(k)),
        correct: v.correct,
        wanted: v.schematic_blocks,
    }
}
/// Every tick: the scan goes on within its budget, and starts again a while after it ended.
pub(super) fn tick(game: &Game<'_>) {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(run) = guard.as_mut() else {
        return;
    };
    // The preview was moved, turned, hidden or removed: it is looked at again.
    let current = view::active(game).filter(|p| p.visible);
    if current.as_ref() != Some(&run.placement) {
        match current {
            Some(pl) => {
                let Some(shape) = view::shape(game, &pl.file) else {
                    *guard = None;
                    return;
                };
                match begin(game, pl, shape) {
                    Ok(mut next) => {
                        next.kind = run.kind;
                        *run = next;
                    }
                    Err(_) => *guard = None,
                }
            }
            None => *guard = None,
        }
        return;
    }
    let total = run.size[0] * run.size[1] * run.size[2];
    if let Some(done) = run.finished {
        if done.elapsed() < AGAIN {
            return;
        }
        run.cursor = 0;
        run.finished = None;
        run.verifier = ver::Verifier::new(run.verifier.options);
    }
    let started = Instant::now();
    let blocks = Blocks(game);
    let mut n = 0u32;
    while run.cursor < total {
        let i = run.cursor;
        run.cursor += 1;
        let (sx, sz) = (run.size[0], run.size[2]);
        let at = [
            run.lo[0] + (i % sx) as i32,
            run.lo[1] + (i / (sx * sz)) as i32,
            run.lo[2] + (i / sx % sz) as i32,
        ];
        // Cells of chunks that are not loaded are not judged.
        if !game.world.chunks.contains_key(&(at[0] >> 4, at[2] >> 4)) {
            continue;
        }
        let rel = places::unplace(at, run.placement.origin, run.placement.rotation);
        let wanted = match run.shape.cells.get(&rel) {
            Some(i) => match run.ids.get(*i as usize).copied().flatten() {
                Some(id) => id,
                None => continue,
            },
            None => run.air,
        };
        let found = game.world.block(at[0], at[1], at[2]).0;
        run.verifier.check(at, wanted, found, &blocks);
        n += 1;
        if n % 256 == 0 && started.elapsed() > BUDGET {
            break;
        }
    }
    if run.cursor >= total {
        run.finished = Some(Instant::now());
        run.last = Some(totals(&run.verifier));
        run.boxes_at = None;
    }
}
/// Which type is drawn now, changed one step at a time: every type, then each in turn.
pub(super) fn cycle_kind() {
    if let Some(run) = RUN.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let all = ver::Mismatch::ALL;
        run.kind = match run.kind {
            None => Some(all[0]),
            Some(k) => all
                .iter()
                .position(|a| *a == k)
                .and_then(|i| all.get(i + 1))
                .copied(),
        };
        run.boxes_at = None;
    }
}
pub(super) fn kind_name(kind: Option<ver::Mismatch>) -> &'static str {
    match kind {
        None => text("Все", "All"),
        Some(ver::Mismatch::Missing) => text("Нет блока", "Missing"),
        Some(ver::Mismatch::Extra) => text("Лишние", "Extra"),
        Some(ver::Mismatch::WrongBlock) => text("Не тот блок", "Wrong block"),
        Some(ver::Mismatch::WrongState) => text("Не то состояние", "Wrong state"),
        Some(ver::Mismatch::DiffBlock) => text("Другой блок", "Different block"),
    }
}
/// The type being drawn, for the button.
pub(super) fn kind() -> Option<ver::Mismatch> {
    RUN.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.kind)
}
/// Numbers for the card: the progress of a scan under way, and the last finished scan.
pub(super) fn summary() -> Option<(u32, Option<Totals>)> {
    let guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let run = guard.as_ref()?;
    let total = run.size[0] * run.size[1] * run.size[2];
    let percent = if run.finished.is_some() {
        100
    } else {
        (run.cursor * 100 / total.max(1)) as u32
    };
    Some((percent, run.last.clone()))
}
/// The mismatches drawn as boxes in the colours of the original, nearest to the player first.
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(run) = guard.as_mut() else {
        return;
    };
    let Some(me) = game
        .native_player()
        .map(|p| [p.x, p.y, p.z].map(|v| v.floor() as i32))
    else {
        return;
    };
    let stale = run.boxes_at.is_none_or(|(at, from)| {
        at.elapsed() > Duration::from_secs(1) && (0..3).any(|i| (from[i] - me[i]).abs() > 2)
            || at.elapsed() > Duration::from_secs(3)
    });
    if stale {
        let kinds: Vec<ver::Mismatch> = run.kind.map_or(ver::Mismatch::ALL.to_vec(), |k| vec![k]);
        run.boxes = run.verifier.closest(&kinds, &[], me, MAX_BOXES);
        run.boxes_at = Some((Instant::now(), me));
    }
    for (kind, at) in &run.boxes {
        g.cuboid(
            at.map(f64::from),
            at.map(|v| f64::from(v) + 1.0),
            0xff00_0000 | kind.color(),
            1.5,
        );
    }
}
/// The report for the clipboard: the numbers, then every (wanted, found) pair by count.
pub(super) fn report(game: &Game<'_>) -> Option<String> {
    let guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let run = guard.as_ref()?;
    let blocks = Blocks(game);
    let name = |s: u32| blocks.name(s).trim_start_matches("minecraft:").to_owned();
    let mut out = format!(
        "{}: {} · {}: {} · {}: {}\n",
        text("Верно", "Correct"),
        run.verifier.correct,
        text("Ошибок", "Mismatches"),
        run.verifier.total(),
        text("Блоков в схеме", "Blocks in the schematic"),
        run.verifier.schematic_blocks
    );
    for m in run
        .verifier
        .overview(run.kind, ver::Sort::Count, false, &name)
    {
        out.push_str(&format!(
            "{}: {} -> {} x{}\n",
            kind_name(Some(m.kind)),
            name(m.expected),
            name(m.found),
            m.count
        ));
    }
    Some(out)
}
