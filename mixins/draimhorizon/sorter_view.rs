// The storage sorter in the game. The player marks the storage zone (`/dsort pos1`, `pos2`), one
// or more input containers and an optional overflow container (aimed at). Started, it walks to an
// input, takes what has a place, carries it round the containers meant for it (item frames
// first, then what each was seen holding, then the overflow) and comes back until the input
// holds nothing more it can place; "watch" looks at the inputs again every half minute. The
// item frames around the player and the containers of the zone are indexed as it goes (a few
// thousand blocks a tick) and kept per world, so a large storage does not have to be opened
// again on every round. Only what was taken out of an input is carried off: the player's own
// items stay in the inventory.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_clickthrough as frames,
    claim_borders_construction_view as placement, claim_borders_haul_view as haul,
    claim_borders_menu as menu, claim_borders_navigation as nav,
    claim_borders_navigation_view as walk, claim_borders_sorter as sorter,
};
use horizon_client::gizmos::Gizmos;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const INDEX_SETTING: &str = "sorter_index";
const PLACES_SETTING: &str = "sorter_places";
const FILL_SETTING: &str = "sorter_fill_empty";
/// Blocks of the zone looked at per tick while indexing.
const SCAN_PER_TICK: i64 = 1024;
/// Frames are read this close to the player only (well inside the entity tracking range), so a
/// container whose frame is out of range does not lose its label.
const FRAME_RANGE: f64 = 24.0;
/// How often "watch" looks at the inputs again.
const WATCH_EVERY: Duration = Duration::from_secs(30);
/// A click round every this often (the server may run slowly).
const CLICK_EVERY: Duration = Duration::from_millis(160);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Once,
    Watch,
    Survey,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Goal {
    /// Taking out of the input with this index (and putting back what has nowhere to go).
    Input(usize),
    /// Putting into the target what the plan meant for it.
    Put,
    /// Opening the target only to see what it holds.
    Look,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Wait(Instant),
    Walk,
    Open,
    Work,
}
struct Run {
    mode: Mode,
    scope: String,
    goal: Goal,
    step: Step,
    target: sorter::Pos,
    /// The input the current round takes from.
    input: usize,
    /// The containers still to visit this round, in order.
    queue: Vec<sorter::Pos>,
    /// What goes into which container.
    assign: BTreeMap<sorter::Pos, BTreeSet<String>>,
    /// The player's own counts, never carried off.
    keep: BTreeMap<String, u32>,
    /// Containers given up on for this run.
    skip: BTreeSet<sorter::Pos>,
    walking: bool,
    since: Instant,
    limit: Duration,
    tries: u8,
    last: Instant,
    stuck: u8,
    sig: usize,
}
struct State {
    loaded: bool,
    chests: Vec<sorter::Chest>,
    places: BTreeMap<String, sorter::Places>,
    fill_empty: bool,
    pos1: Option<(String, sorter::Pos)>,
    pos2: Option<(String, sorter::Pos)>,
    run: Option<Run>,
    /// The run is out of `run` while this tick's step works on it.
    busy: bool,
    stop: bool,
    /// The index changed since it was last written.
    dirty: bool,
    saved: Option<Instant>,
    /// The world and block of the zone the indexing is at.
    scan: Option<(String, i64)>,
    frames_at: Option<Instant>,
    /// Stacks moved since the mod started.
    moved: u32,
    message: String,
}
static STATE: Mutex<State> = Mutex::new(State {
    loaded: false,
    chests: Vec::new(),
    places: BTreeMap::new(),
    fill_empty: false,
    pos1: None,
    pos2: None,
    run: None,
    busy: false,
    stop: false,
    dirty: false,
    saved: None,
    scan: None,
    frames_at: None,
    moved: 0,
    message: String::new(),
});
fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn with<T>(game: &Game<'_>, f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.loaded {
        s.loaded = true;
        let read = |key| game.native_setting(claims::SETTINGS_ID, key);
        s.chests = read(INDEX_SETTING)
            .map(|v| sorter::decode_index(&v))
            .unwrap_or_default();
        s.places = read(PLACES_SETTING)
            .map(|v| sorter::decode_places(&v))
            .unwrap_or_default();
        s.fill_empty = read(FILL_SETTING).as_deref() == Some("true");
    }
    f(&mut s)
}
fn save_places(game: &Game<'_>, s: &State) {
    game.native_set_setting(
        claims::SETTINGS_ID,
        PLACES_SETTING,
        &sorter::encode_places(&s.places),
    );
}
/// The index is written at most every ten seconds while it changes, and when a run ends.
fn save_index(game: &Game<'_>, s: &mut State, now: bool) {
    if !s.dirty
        || (!now
            && s.saved
                .is_some_and(|t| t.elapsed() < Duration::from_secs(10)))
    {
        return;
    }
    s.dirty = false;
    s.saved = Some(Instant::now());
    game.native_set_setting(
        claims::SETTINGS_ID,
        INDEX_SETTING,
        &sorter::encode_index(&s.chests),
    );
}

/// The position a container is known by: a double chest by its smaller half.
fn canonical(game: &Game<'_>, at: sorter::Pos) -> sorter::Pos {
    if !sorter::joins(&game.native_block(at)) {
        return at;
    }
    let other = sorter::partner(
        at,
        game.native_block_property(at, "type").as_deref(),
        game.native_block_property(at, "facing").as_deref(),
    );
    sorter::canonical(at, other)
}
/// The container under the crosshair, by its known position.
fn aimed(game: &Game<'_>) -> Result<sorter::Pos, String> {
    let miss = || {
        text(
            "Наведите прицел на сундук, бочку или шалкер",
            "Aim at a chest, barrel or shulker box",
        )
        .to_owned()
    };
    let hit = game.pick().ok_or_else(miss)?;
    if !sorter::is_container(&game.native_block(hit.pos)) {
        return Err(miss());
    }
    Ok(canonical(game, hit.pos))
}
fn standing(game: &Game<'_>) -> Option<sorter::Pos> {
    let p = game.native_player()?;
    let at = [p.x, p.y, p.z];
    Some(nav::standing_block(at, &game.native_block(nav::block(at))))
}
fn player_block(game: &Game<'_>) -> sorter::Pos {
    game.native_player()
        .map_or([0; 3], |p| nav::block([p.x, p.y, p.z]))
}
fn close_window(game: &mut Game<'_>) {
    if game.screen == Screen::Inventory {
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    placement::cancel_container();
}
fn window_open(game: &Game<'_>) -> bool {
    game.screen == Screen::Inventory
        && game
            .remote_menu_type()
            .is_some_and(crate::game::claim_borders_farm_storage::is_container_menu)
}
/// The open window's own slots (not the player's 36).
fn window_slots(game: &mut Game<'_>) -> Vec<sorter::Stack> {
    let mut out = Vec::new();
    game.menu_session(|menu, _env, _to_menu| {
        let n = menu.slot_count();
        for i in 0..n.saturating_sub(36) {
            out.push(
                menu.slot(i)
                    .filter(|st| st.count > 0)
                    .map(|st| (st.item.to_string(), st.count)),
            );
        }
        Some(false)
    });
    out
}

// ---- Indexing: item frames and the containers of the zone ----

/// Frames around the player: the container each hangs on and the item it shows.
fn read_frames(game: &mut Game<'_>, scope: &str) {
    let Some(p) = game.native_player() else {
        return;
    };
    let eye = [p.x, p.y, p.z];
    let zone = with(game, |s| s.places.get(scope).and_then(|pl| pl.zone));
    let mut seen: BTreeMap<sorter::Pos, BTreeSet<String>> = BTreeMap::new();
    let found: Vec<(sorter::Pos, String)> = game
        .item_frames
        .iter()
        .filter_map(|f| {
            let item = f.item.as_ref()?.item.to_string();
            let support = frames::supporting_frame(f.pos, f.step())?;
            Some((support, item))
        })
        .collect();
    for (support, item) in found {
        if nav::distance(nav::center(support), eye) > FRAME_RANGE
            || zone.is_some_and(|z| !z.contains(support))
            || !sorter::is_container(&game.native_block(support))
        {
            continue;
        }
        let at = canonical(game, support);
        let labels = seen.entry(at).or_default();
        if labels.len() < sorter::MAX_ITEMS {
            labels.insert(item);
        }
    }
    with(game, |s| {
        let near = |at: sorter::Pos| nav::distance(nav::center(at), eye) <= FRAME_RANGE - 2.0;
        if sorter::apply_labels(&mut s.chests, scope, &seen, &near) {
            s.dirty = true;
        }
    });
}
/// A slice of the zone a tick: containers found are added, ones broken since are dropped.
fn scan_zone(game: &Game<'_>, scope: &str) {
    let Some(zone) = with(game, |s| s.places.get(scope).and_then(|p| p.zone)) else {
        return;
    };
    let start = with(game, |s| match &s.scan {
        Some((w, i)) if w == scope => *i,
        _ => 0,
    });
    let mut found = Vec::new();
    let mut gone = Vec::new();
    let mut i = start;
    for _ in 0..SCAN_PER_TICK {
        let Some(at) = zone.nth(i) else {
            i = 0;
            break;
        };
        i += 1;
        let name = game.native_block(at);
        if sorter::is_container(&name) {
            found.push(canonical(game, at));
        } else if !name.ends_with("air") {
            // A block is there (the chunk is loaded) and it is not a container any more.
            gone.push(at);
        }
    }
    with(game, |s| {
        s.scan = Some((scope.to_owned(), i));
        for at in found {
            if !s.chests.iter().any(|c| c.scope == scope && c.pos == at) {
                sorter::entry(&mut s.chests, scope, at);
                s.dirty = true;
            }
        }
        let before = s.chests.len();
        s.chests
            .retain(|c| !(c.scope == scope && gone.contains(&c.pos)));
        s.dirty |= s.chests.len() != before;
    });
}

/// Whether the player is in the zone or near it (within 32 blocks), or near an input when no
/// zone is set.
fn near_storage(game: &Game<'_>, scope: &str) -> bool {
    let here = player_block(game);
    with(game, |s| {
        if s.run.is_some() || s.busy {
            return true;
        }
        let Some(p) = s.places.get(scope) else {
            return false;
        };
        let close = |lo: sorter::Pos, hi: sorter::Pos| {
            (0..3).all(|i| lo[i] - 32 <= here[i] && here[i] <= hi[i] + 32)
        };
        match p.zone {
            Some(z) => close(z.lo, z.hi),
            None => p.inputs.iter().any(|at| close(*at, *at)),
        }
    })
}

// ---- The run ----

fn new_run(mode: Mode, scope: String, keep: BTreeMap<String, u32>) -> Run {
    Run {
        mode,
        scope,
        goal: Goal::Input(0),
        step: Step::Walk,
        target: [0; 3],
        input: 0,
        queue: Vec::new(),
        assign: BTreeMap::new(),
        keep,
        skip: BTreeSet::new(),
        walking: false,
        since: Instant::now(),
        limit: Duration::from_secs(90),
        tries: 0,
        last: Instant::now(),
        stuck: 0,
        sig: 0,
    }
}
fn start(game: &Game<'_>, mode: Mode) -> Result<String, String> {
    let scope = walk::world_key(game)
        .ok_or_else(|| text("Сначала войдите в мир.", "Join a world first.").to_owned())?;
    if with(game, |s| s.run.is_some() || s.busy) {
        return Err(text(
            "Сортировщик уже работает.",
            "The sorter is already running.",
        )
        .into());
    }
    if walk::task_running() {
        return Err(text(
            "Сначала остановите текущую задачу.",
            "Stop the running task first.",
        )
        .into());
    }
    let keep = sorter::baseline(&haul::inventory(game));
    with(game, |s| {
        let places = s.places.get(&scope).cloned().unwrap_or_default();
        let mut run = new_run(mode, scope.clone(), keep);
        if mode == Mode::Survey {
            let here = player_block(game);
            let todo: Vec<sorter::Pos> = s
                .chests
                .iter()
                .filter(|c| c.scope == scope && !c.seen)
                .map(|c| c.pos)
                .collect();
            run.queue = sorter::route(here, &todo);
            if run.queue.is_empty() {
                return Err(text(
                    "Неосмотренных сундуков нет. Задайте зону и подождите индексации.",
                    "No unseen containers. Set the zone and let it index.",
                )
                .into());
            }
            run.goal = Goal::Look;
            run.target = run.queue.remove(0);
        } else {
            let Some(first) = places.inputs.first() else {
                return Err(text(
                    "Сначала задайте входной сундук.",
                    "Set an input container first.",
                )
                .into());
            };
            run.target = *first;
        }
        s.run = Some(run);
        s.stop = false;
        s.message.clear();
        Ok(match mode {
            Mode::Once => text("Сортировка: разбираю вход.", "Sorter: emptying the input."),
            Mode::Watch => text(
                "Сортировка: слежу за входом.",
                "Sorter: watching the input.",
            ),
            Mode::Survey => text(
                "Сортировка: осматриваю склад.",
                "Sorter: looking through the storage.",
            ),
        }
        .to_owned())
    })
}
/// Ends the run: the window is closed and the walk stopped.
fn end(game: &mut Game<'_>, message: Option<&str>) {
    let had = with(game, |s| {
        let had = s.run.take().is_some() || s.busy;
        s.busy = false;
        had
    });
    if !had {
        return;
    }
    close_window(game);
    crate::game::claim_borders_baritone_bridge::walk_cancel();
    with(game, |s| {
        s.stop = false;
        if let Some(m) = message {
            s.message = m.to_owned();
        }
        save_index(game, s, true);
    });
    if let Some(m) = message {
        game.native_chat(&format!("[DraimHorizon] {m}"));
    }
}
/// Asks the run to stop (from the Stop key, a pause, the menu); the next tick ends it.
pub(super) fn stop_request() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.run.is_some() || s.busy {
        s.stop = true;
    }
}
pub(super) fn active() -> bool {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.run.is_some() || s.busy
}
/// Puts the run back after this tick's step (unless it was ended meanwhile).
fn save_run(game: &Game<'_>, run: Run) {
    with(game, |s| {
        if s.busy {
            s.busy = false;
            s.run = Some(run);
        }
    });
}
/// Whether the container can be clicked from where the player stands.
fn within_reach(game: &Game<'_>, target: sorter::Pos) -> bool {
    let (Some(eye), Some(feet)) = (game.eye_position(), game.native_player()) else {
        return false;
    };
    let middle = nav::center(target);
    nav::distance(eye, [middle[0], middle[1] + 0.5, middle[2]]) <= 4.0
        && haul::in_sight(game, nav::block([feet.x, feet.y, feet.z]), target)
}
/// The next container to go to after the current one, or back to the input.
fn next_target(game: &Game<'_>, mut run: Run) -> Option<Run> {
    run.walking = false;
    run.tries = 0;
    run.stuck = 0;
    run.sig = 0;
    run.step = Step::Walk;
    if !run.queue.is_empty() {
        run.target = run.queue.remove(0);
        if run.mode != Mode::Survey {
            run.goal = Goal::Put;
        }
        return Some(run);
    }
    if run.mode == Mode::Survey {
        return None;
    }
    // Back to the input the round came from: it may hold more.
    run.target = *inputs(game, &run.scope).get(run.input)?;
    run.goal = Goal::Input(run.input);
    Some(run)
}
fn inputs(game: &Game<'_>, scope: &str) -> Vec<sorter::Pos> {
    with(game, |s| {
        s.places
            .get(scope)
            .map(|p| p.inputs.clone())
            .unwrap_or_default()
    })
}
/// Where each carried item goes from here; items with nowhere to go are left out.
fn plan_round(game: &Game<'_>, run: &mut Run) {
    let inv = haul::inventory(game);
    let here = player_block(game);
    let carried = sorter::carried(&inv, &run.keep);
    let fill = with(game, |s| s.fill_empty);
    let assign: BTreeMap<sorter::Pos, BTreeSet<String>> = with(game, |s| {
        let chests: Vec<sorter::Chest> = s
            .chests
            .iter()
            .filter(|c| c.scope == run.scope)
            .cloned()
            .collect();
        let places = s.places.get(&run.scope).cloned().unwrap_or_default();
        let rules = sorter::Rules {
            chests: &chests,
            places: &places,
            fill_empty: fill,
            skip: &run.skip,
        };
        let mut out: BTreeMap<sorter::Pos, BTreeSet<String>> = BTreeMap::new();
        for item in carried {
            if let Some(at) = rules.choose(&item, here) {
                out.entry(at).or_default().insert(item);
            }
        }
        out
    });
    run.queue = sorter::route(here, &assign.keys().copied().collect::<Vec<_>>());
    run.assign = assign;
}

/// Drives the run and the indexing each tick.
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.native_player().is_none() {
        end(game, None);
        return;
    }
    let Some(scope) = walk::world_key(game) else {
        return;
    };
    // The index grows only while the player is at the storage (or a run is under way).
    let near = near_storage(game, &scope);
    if near {
        scan_zone(game, &scope);
    }
    let due = with(game, |s| {
        let due = near
            && s.frames_at
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(1));
        if due {
            s.frames_at = Some(Instant::now());
        }
        due
    });
    if due {
        read_frames(game, &scope);
        with(game, |s| save_index(game, s, false));
    }
    let stop = with(game, |s| s.stop);
    if stop {
        end(
            game,
            Some(text("Сортировка остановлена.", "Sorter stopped.")),
        );
        return;
    }
    let Some(mut run) = with(game, |s| {
        let run = s.run.take();
        s.busy = run.is_some();
        run
    }) else {
        return;
    };
    let step = run.step;
    if run.scope != scope {
        end(
            game,
            Some(text("Сортировка: другой мир.", "Sorter: another world.")),
        );
        return;
    }
    match step {
        Step::Wait(until) => {
            if Instant::now() >= until {
                run.step = Step::Walk;
                run.goal = Goal::Input(0);
                run.input = 0;
                run.skip.clear();
                match inputs(game, &run.scope).first() {
                    Some(at) => run.target = *at,
                    None => {
                        end(
                            game,
                            Some(text("Входной сундук не задан.", "No input container set.")),
                        );
                        return;
                    }
                }
            }
            save_run(game, run);
        }
        Step::Walk => walk_step(game, run),
        Step::Open => {
            if window_open(game) {
                run.step = Step::Work;
                run.last = Instant::now() - CLICK_EVERY;
                save_run(game, run);
            } else if placement::container_blocked()
                || run.since.elapsed() > Duration::from_millis(2500)
            {
                placement::cancel_container();
                run.tries += 1;
                run.step = Step::Walk;
                run.walking = false;
                if run.tries >= 4 {
                    give_up(game, run);
                } else {
                    save_run(game, run);
                }
            } else {
                save_run(game, run);
            }
        }
        Step::Work => {
            if !window_open(game) {
                // Closed by the server or the player: try this container again.
                run.step = Step::Walk;
                run.walking = false;
                run.tries += 1;
                if run.tries >= 4 {
                    give_up(game, run);
                } else {
                    save_run(game, run);
                }
            } else if run.last.elapsed() >= CLICK_EVERY {
                work(game, run);
            } else {
                save_run(game, run);
            }
        }
    }
}
fn walk_step(game: &mut Game<'_>, mut run: Run) {
    if !run.walking {
        if within_reach(game, run.target) {
            run.step = Step::Open;
            run.since = Instant::now();
            placement::open_container(run.target);
            save_run(game, run);
            return;
        }
        let Some(p) = game.native_player() else {
            save_run(game, run);
            return;
        };
        let spots = haul::spots(game, run.target, [p.x, p.y, p.z]);
        let Some(spot) = spots
            .get(usize::from(run.tries) % spots.len().max(1))
            .copied()
        else {
            give_up(game, run);
            return;
        };
        if walk::workshop_walk_for(game, spot, false, false, "sort").is_err() {
            give_up(game, run);
            return;
        }
        run.walking = true;
        run.since = Instant::now();
        run.limit = haul::walk_time(game, spot);
        save_run(game, run);
    } else if run.since.elapsed() > run.limit {
        run.walking = false;
        run.tries += 1;
        if run.tries >= 4 {
            give_up(game, run);
        } else {
            save_run(game, run);
        }
    } else {
        match walk::workshop_walk_result() {
            Some(true) => {
                run.step = Step::Open;
                run.since = Instant::now();
                placement::open_container(run.target);
                save_run(game, run);
            }
            Some(false) => {
                run.walking = false;
                run.tries += 1;
                if run.tries >= 4 {
                    give_up(game, run);
                } else {
                    save_run(game, run);
                }
            }
            None => save_run(game, run),
        }
    }
}
/// The target could not be reached or opened: an input ends the run, a container is skipped
/// and what was meant for it is planned again.
fn give_up(game: &mut Game<'_>, mut run: Run) {
    close_window(game);
    match run.goal {
        Goal::Input(_) => end(
            game,
            Some(text(
                "Сортировка: не могу открыть входной сундук.",
                "Sorter: cannot open the input container.",
            )),
        ),
        Goal::Put => {
            run.skip.insert(run.target);
            plan_round(game, &mut run);
            match next_target(game, run) {
                Some(run) => save_run(game, run),
                None => end(game, None),
            }
        }
        Goal::Look => match next_target(game, run) {
            Some(run) => save_run(game, run),
            None => end(
                game,
                Some(text("Осмотр склада закончен.", "Storage survey done.")),
            ),
        },
    }
}
/// One round of clicks in the open window.
fn work(game: &mut Game<'_>, mut run: Run) {
    use horizon_client::menu::Input;
    let slots = window_slots(game);
    let inv = haul::inventory(game);
    if !matches!(run.goal, Goal::Input(_)) {
        with(game, |s| {
            sorter::observe(&mut s.chests, &run.scope, run.target, &slots);
            s.dirty = true;
        });
    }
    let here = player_block(game);
    match run.goal {
        Goal::Look => {
            close_window(game);
            match next_target(game, run) {
                Some(run) => save_run(game, run),
                None => end(
                    game,
                    Some(text("Осмотр склада закончен.", "Storage survey done.")),
                ),
            }
        }
        Goal::Put => {
            let items = run.assign.get(&run.target).cloned().unwrap_or_default();
            let plan = sorter::put_slots(&inv, &items, &run.keep);
            let sig = plan.iter().sum::<usize>() + plan.len() * 100;
            let stuck = if sig == run.sig { run.stuck + 1 } else { 0 };
            if plan.is_empty() || stuck >= 3 {
                close_window(game);
                if !plan.is_empty() {
                    // Nothing moves: it is full. Plan what is left again without it.
                    with(game, |s| {
                        sorter::entry(&mut s.chests, &run.scope, run.target).full = true;
                        s.dirty = true;
                    });
                    run.skip.insert(run.target);
                    plan_round(game, &mut run);
                } else {
                    run.assign.remove(&run.target);
                }
                match next_target(game, run) {
                    Some(run) => save_run(game, run),
                    None => end(game, None),
                }
                return;
            }
            let ops: Vec<_> = plan
                .iter()
                .take(4)
                .map(|g| (Some(*g), 0, Input::QuickMove))
                .collect();
            if !game.menu_ops(&ops) {
                run.stuck = stuck + 1;
            } else {
                with(game, |s| s.moved = s.moved.saturating_add(ops.len() as u32));
                run.stuck = stuck;
            }
            run.sig = sig;
            run.last = Instant::now();
            save_run(game, run);
        }
        Goal::Input(index) => {
            let (has_place, back) = with(game, |s| {
                let chests: Vec<sorter::Chest> = s
                    .chests
                    .iter()
                    .filter(|c| c.scope == run.scope)
                    .cloned()
                    .collect();
                let places = s.places.get(&run.scope).cloned().unwrap_or_default();
                let rules = sorter::Rules {
                    chests: &chests,
                    places: &places,
                    fill_empty: s.fill_empty,
                    skip: &run.skip,
                };
                let mut known: BTreeMap<String, bool> = BTreeMap::new();
                for (name, _) in slots.iter().flatten().chain(inv.iter().flatten()) {
                    if !known.contains_key(name) {
                        known.insert(name.clone(), rules.choose(name, here).is_some());
                    }
                }
                // Carried items with nowhere to go any more go back into the input.
                let homeless: BTreeSet<String> = sorter::carried(&inv, &run.keep)
                    .into_iter()
                    .filter(|n| !known.get(n).copied().unwrap_or(false))
                    .collect();
                (known, homeless)
            });
            let putback = sorter::put_slots(&inv, &back, &run.keep);
            let take = if putback.is_empty() {
                sorter::take_plan(&slots, sorter::free_slots(&inv), &|n| {
                    has_place.get(n).copied().unwrap_or(false)
                })
            } else {
                Vec::new()
            };
            let sig =
                putback.iter().chain(&take).sum::<usize>() + (putback.len() + take.len()) * 100;
            let stuck = if sig == run.sig { run.stuck + 1 } else { 0 };
            if (putback.is_empty() && take.is_empty()) || stuck >= 3 {
                close_window(game);
                if !putback.is_empty() {
                    end(
                        game,
                        Some(text(
                            "Сортировка: вход полон, а вещам некуда деться.",
                            "Sorter: the input is full and the items have nowhere to go.",
                        )),
                    );
                    return;
                }
                run.input = index;
                plan_round(game, &mut run);
                if run.queue.is_empty()
                    && sorter::free_slots(&inv) == 0
                    && !slots.iter().all(Option::is_none)
                {
                    end(
                        game,
                        Some(text(
                            "Сортировка: освободите слот в инвентаре.",
                            "Sorter: free an inventory slot.",
                        )),
                    );
                    return;
                }
                if run.queue.is_empty() {
                    // Nothing carried: on to the next input, or the round is over.
                    let count = inputs(game, &run.scope).len();
                    if index + 1 < count {
                        run.goal = Goal::Input(index + 1);
                        run.input = index + 1;
                        run.target = inputs(game, &run.scope)[index + 1];
                        run.step = Step::Walk;
                        run.walking = false;
                        run.tries = 0;
                        run.stuck = 0;
                        run.sig = 0;
                        save_run(game, run);
                    } else if run.mode == Mode::Watch {
                        run.step = Step::Wait(Instant::now() + WATCH_EVERY);
                        save_run(game, run);
                    } else {
                        end(game, Some(text("Сортировка закончена.", "Sorting done.")));
                    }
                    return;
                }
                match next_target(game, run) {
                    Some(run) => save_run(game, run),
                    None => end(game, None),
                }
                return;
            }
            let ops: Vec<_> = putback
                .iter()
                .map(|g| (Some(*g), 0, Input::QuickMove))
                .chain(take.iter().map(|i| (Some(300 + *i), 0, Input::QuickMove)))
                .take(4)
                .collect();
            if game.menu_ops(&ops) {
                with(game, |s| s.moved = s.moved.saturating_add(ops.len() as u32));
            }
            run.stuck = stuck;
            run.sig = sig;
            run.last = Instant::now();
            save_run(game, run);
        }
    }
}

// ---- Places, the menu and the commands ----

fn edit_places<T>(game: &Game<'_>, f: impl FnOnce(&mut sorter::Places) -> T) -> Result<T, String> {
    let scope = walk::world_key(game)
        .ok_or_else(|| text("Сначала войдите в мир.", "Join a world first.").to_owned())?;
    Ok(with(game, |s| {
        let out = f(s.places.entry(scope).or_default());
        s.places.retain(|_, p| *p != sorter::Places::default());
        save_places(game, s);
        out
    }))
}
fn fmt(at: sorter::Pos) -> String {
    format!("{} {} {}", at[0], at[1], at[2])
}
fn toggle_input(game: &Game<'_>) -> Result<String, String> {
    let at = aimed(game)?;
    edit_places(game, |p| {
        if let Some(i) = p.inputs.iter().position(|x| *x == at) {
            p.inputs.remove(i);
            Ok(format!(
                "{}: {}",
                text("Вход убран", "Input removed"),
                fmt(at)
            ))
        } else if p.inputs.len() >= sorter::MAX_INPUTS {
            Err(format!(
                "{} {}",
                text("Входов не больше", "At most inputs:"),
                sorter::MAX_INPUTS
            ))
        } else {
            if p.overflow == Some(at) {
                p.overflow = None;
            }
            p.inputs.push(at);
            Ok(format!("{}: {}", text("Вход", "Input"), fmt(at)))
        }
    })?
}
fn toggle_overflow(game: &Game<'_>) -> Result<String, String> {
    match aimed(game) {
        Ok(at) => edit_places(game, |p| {
            if p.overflow == Some(at) {
                p.overflow = None;
                format!(
                    "{}: {}",
                    text("«Прочее»", "Overflow"),
                    text("сброшен", "cleared")
                )
            } else {
                p.inputs.retain(|x| *x != at);
                p.overflow = Some(at);
                format!("{}: {}", text("«Прочее»", "Overflow"), fmt(at))
            }
        }),
        Err(e) => {
            let had = edit_places(game, |p| p.overflow.take().is_some())?;
            if had {
                Ok(format!(
                    "{}: {}",
                    text("«Прочее»", "Overflow"),
                    text("сброшен", "cleared")
                ))
            } else {
                Err(e)
            }
        }
    }
}
fn mark(game: &Game<'_>, second: bool) -> Result<String, String> {
    let scope = walk::world_key(game)
        .ok_or_else(|| text("Сначала войдите в мир.", "Join a world first.").to_owned())?;
    let at = standing(game)
        .ok_or_else(|| text("Сначала войдите в мир.", "Join a world first.").to_owned())?;
    let corners = with(game, |s| {
        let slot = if second { &mut s.pos2 } else { &mut s.pos1 };
        *slot = Some((scope.clone(), at));
        match (&s.pos1, &s.pos2) {
            (Some((a, p)), Some((b, q))) if *a == scope && *b == scope => Some((*p, *q)),
            _ => None,
        }
    });
    let point = if second {
        text("Точка 2", "Point 2")
    } else {
        text("Точка 1", "Point 1")
    };
    let Some((a, b)) = corners else {
        return Ok(format!("{point}: {}", fmt(at)));
    };
    let zone = sorter::Zone::between(a, b);
    if let Some(why) = zone.too_large() {
        return Err(if why == "side" {
            format!(
                "{} {}",
                text(
                    "Зона слишком большая: сторона не больше",
                    "The zone is too large: a side of at most"
                ),
                sorter::MAX_SIDE
            )
        } else {
            format!(
                "{} {}",
                text(
                    "Зона слишком высокая: не больше",
                    "The zone is too tall: at most"
                ),
                sorter::MAX_HEIGHT
            )
        });
    }
    edit_places(game, |p| p.zone = Some(zone))?;
    with(game, |s| {
        s.pos1 = None;
        s.pos2 = None;
        s.scan = None;
    });
    Ok(format!(
        "{}: {}×{}×{}",
        text("Зона склада", "Storage zone"),
        zone.hi[0] - zone.lo[0] + 1,
        zone.hi[2] - zone.lo[2] + 1,
        zone.hi[1] - zone.lo[1] + 1
    ))
}
fn clear(game: &Game<'_>) -> Result<String, String> {
    let scope = walk::world_key(game)
        .ok_or_else(|| text("Сначала войдите в мир.", "Join a world first.").to_owned())?;
    with(game, |s| {
        s.places.remove(&scope);
        s.chests.retain(|c| c.scope != scope);
        s.dirty = true;
        s.pos1 = None;
        s.pos2 = None;
        save_places(game, s);
        save_index(game, s, true);
    });
    Ok(text("Склад этого мира забыт.", "This world's storage forgotten.").into())
}
fn status_line(game: &Game<'_>) -> String {
    let info = snapshot(game);
    format!(
        "{}: {} · {}: {} · {}: {} · {}: {}",
        text("Сундуков", "Containers"),
        info.chests,
        text("с рамкой", "framed"),
        info.labeled,
        text("осмотрено", "seen"),
        info.seen,
        text("перенесено", "moved"),
        info.moved
    )
}
/// The menu's buttons.
pub(super) fn act(
    game: &mut Game<'_>,
    control: menu::SorterControl,
) -> Result<Vec<String>, String> {
    use menu::SorterControl as C;
    let line = match control {
        C::Input => toggle_input(game)?,
        C::Overflow => toggle_overflow(game)?,
        C::Pos1 => mark(game, false)?,
        C::Pos2 => mark(game, true)?,
        C::Sort => start(game, Mode::Once)?,
        C::Watch => start(game, Mode::Watch)?,
        C::Survey => start(game, Mode::Survey)?,
        C::Stop => {
            stop_request();
            text("Сортировка: стоп.", "Sorter: stop.").to_owned()
        }
        C::FillEmpty => {
            let on = with(game, |s| {
                s.fill_empty = !s.fill_empty;
                s.fill_empty
            });
            game.native_set_setting(
                claims::SETTINGS_ID,
                FILL_SETTING,
                if on { "true" } else { "false" },
            );
            format!(
                "{}: {}",
                text("Занимать пустые", "Fill empty ones"),
                if on {
                    text("вкл", "on")
                } else {
                    text("выкл", "off")
                }
            )
        }
        C::Clear => clear(game)?,
    };
    Ok(vec![line])
}
/// What the menu shows.
pub(super) fn snapshot(game: &Game<'_>) -> menu::SorterInfo {
    let scope = walk::world_key(game).unwrap_or_default();
    with(game, |s| {
        let places = s.places.get(&scope).cloned().unwrap_or_default();
        let mine = s.chests.iter().filter(|c| c.scope == scope);
        menu::SorterInfo {
            inputs: places.inputs.len(),
            overflow: places.overflow,
            zone: places.zone.map(|z| {
                format!(
                    "{}×{}×{}",
                    z.hi[0] - z.lo[0] + 1,
                    z.hi[2] - z.lo[2] + 1,
                    z.hi[1] - z.lo[1] + 1
                )
            }),
            chests: mine.clone().count(),
            labeled: mine.clone().filter(|c| !c.labels.is_empty()).count(),
            seen: mine.filter(|c| c.seen).count(),
            fill_empty: s.fill_empty,
            running: s.run.as_ref().map(|r| match r.mode {
                Mode::Once => 0,
                Mode::Watch => 1,
                Mode::Survey => 2,
            }),
            moved: s.moved,
            status: status_of(s),
        }
    })
}
fn status_of(s: &State) -> String {
    let Some(run) = s.run.as_ref() else {
        return s.message.clone();
    };
    match (run.step, run.goal) {
        (Step::Wait(_), _) => text(
            "Сортировка: жду новых вещей",
            "Sorter: waiting for new items",
        ),
        (_, Goal::Input(_)) => text("Сортировка: входной сундук", "Sorter: input container"),
        (_, Goal::Put) => text("Сортировка: раскладываю", "Sorter: putting items away"),
        (_, Goal::Look) => text("Сортировка: осматриваю", "Sorter: looking inside"),
    }
    .to_owned()
}
/// The status line while a run is under way.
pub(super) fn status() -> String {
    status_of(&STATE.lock().unwrap_or_else(|e| e.into_inner()))
}
fn usage() -> &'static str {
    text(
        "Сортировщик: /dsort pos1, pos2 (зона), input (прицел, вкл/выкл), overflow (прицел), start, watch, survey, stop, empty (занимать пустые), status, clear",
        "Sorter: /dsort pos1, pos2 (zone), input (aim, toggles), overflow (aim), start, watch, survey, stop, empty (fill empty ones), status, clear",
    )
}
/// `/dsort ...` (or `/sort` with one of these words): handled here and not sent.
pub(super) fn command(game: &mut Game<'_>, message: &str) -> bool {
    use menu::SorterControl as C;
    let message = message.trim();
    let mut words = message.split_whitespace();
    let Some(name) = words.next().map(str::to_ascii_lowercase) else {
        return false;
    };
    let own = name == "/dsort";
    if (name != "/sort" && !own) || !game.native_on(claims::ID) {
        return false;
    }
    let sub = words.next().map(str::to_ascii_lowercase);
    let control = match sub.as_deref() {
        Some("pos1") => Some(C::Pos1),
        Some("pos2") => Some(C::Pos2),
        Some("input" | "in") => Some(C::Input),
        Some("overflow" | "other") => Some(C::Overflow),
        Some("start" | "once") => Some(C::Sort),
        Some("watch") => Some(C::Watch),
        Some("survey" | "scan") => Some(C::Survey),
        Some("stop") => Some(C::Stop),
        Some("empty") => Some(C::FillEmpty),
        Some("clear") => Some(C::Clear),
        _ => None,
    };
    let reply = match (control, sub.as_deref()) {
        (Some(c), _) => match act(game, c) {
            Ok(lines) => lines.join(" "),
            Err(e) => e,
        },
        (None, Some("status")) => status_line(game),
        // A server's own /sort goes through untouched.
        _ if !own => return false,
        _ => usage().to_owned(),
    };
    game.ui.chat.add_recent(message);
    game.native_chat(&format!("[DraimHorizon] {reply}"));
    true
}
/// The zone (blue), the marks (yellow), inputs (green), the overflow (orange) and the containers
/// of the current round (white).
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let Some(scope) = walk::world_key(game) else {
        return;
    };
    let (places, marks, round) = with(game, |s| {
        let here = |m: &Option<(String, sorter::Pos)>| {
            m.as_ref().filter(|(w, _)| *w == scope).map(|(_, at)| *at)
        };
        let round: Vec<sorter::Pos> = s
            .run
            .as_ref()
            .map(|r| r.assign.keys().copied().collect())
            .unwrap_or_default();
        (
            s.places.get(&scope).cloned().unwrap_or_default(),
            [here(&s.pos1), here(&s.pos2)],
            round,
        )
    });
    let block = |g: &mut Gizmos, at: sorter::Pos, color: u32| {
        let lo = at.map(|v| f64::from(v) + 0.05);
        g.cuboid(lo, lo.map(|v| v + 0.9), color, 2.0);
    };
    if let Some(z) = places.zone {
        g.cuboid(
            z.lo.map(f64::from),
            z.hi.map(|v| f64::from(v) + 1.0),
            0x995aa9ff,
            1.5,
        );
    }
    for at in marks.into_iter().flatten() {
        block(g, at, 0xffffd23c);
    }
    for at in places.inputs {
        block(g, at, 0xff55dd66);
    }
    if let Some(at) = places.overflow {
        block(g, at, 0xffff9a3c);
    }
    for at in round {
        block(g, at, 0xccffffff);
    }
}
