// Farm and schematic jobs share the normal movement, attack and placement drivers.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_construction_view as placement,
    claim_borders_draining as drain, claim_borders_economy as econ,
    claim_borders_economy_client as econ_data, claim_borders_farm_plots as plots,
    claim_borders_farm_storage as store, claim_borders_farming as farm,
    claim_borders_navigation as nav, claim_borders_navigation_view as driver,
    claim_borders_schematic as schema, claim_borders_schematic_files as files,
};
use horizon_client::gizmos::Gizmos;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Mutex, OnceLock, mpsc::Receiver},
    time::{Duration, Instant},
};
fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn reason(error: schema::PlaceError) -> &'static str {
    match error {
        schema::PlaceError::Material => text(
            "Нет материала (хотбар, 2-я рука, инвентарь)",
            "Material missing (hotbar, off hand, inventory)",
        ),
        schema::PlaceError::Refused => text("Сервер отказал", "Server refused"),
        schema::PlaceError::Manual => text("Ручное вмешательство", "Manual input"),
        schema::PlaceError::Timeout => text("Нет ответа сервера", "Server acknowledgement timeout"),
        schema::PlaceError::Unsupported => text("Блок не поддерживается", "Unsupported block"),
        schema::PlaceError::Unreachable => text("Нет доступной грани", "No reachable face"),
    }
}
enum Task {
    Farm {
        config: farm::Config,
        /// The fields worked: the enabled plots the player marked in this world, or the square
        /// around the start when there are none.
        areas: Vec<plots::Bounds>,
        /// The areas are the marked plots (looked up again as the player turns them on or off).
        marked: bool,
        harvested: u16,
    },
    Drain {
        config: drain::Config,
        origin: nav::Pos,
        filled: u16,
    },
    Build {
        document: schema::Blueprint,
        cursor: usize,
    },
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Ready,
    Walking,
    /// Walking along the blocks already laid, to the spot a pattern cell is placed from.
    Pier,
    Digging,
    Placing,
    /// Bone meal is being used on a crop: the click is under way, then the result is looked at.
    Growing,
    /// A hoe is being used on trampled soil of the field, to make it farmland again.
    Tilling,
    Done,
}
/// A dropped item the farm is collecting.
struct Loot {
    id: u32,
    since: Instant,
    arrived: Option<Instant>,
    walking: bool,
}
struct Pier {
    path: Vec<nav::Pos>,
    next: usize,
    since: Instant,
}
struct Pending {
    rx: Receiver<Result<files::Response, String>>,
    run: bool,
}
#[derive(Clone, Copy, PartialEq)]
enum Purpose {
    /// Put the harvest into the harvest container.
    Deposit,
    /// Put the spare seeds (and half of the carrots and potatoes) into the seeds container.
    DepositSeeds,
    /// Take seeds of this crop out of the seeds container.
    Seeds(farm::Crop),
    /// Take bone meal (or bone blocks, to be crafted into meal) out of its container.
    Meal,
    /// Craft hay bales from wheat at a crafting table nearby.
    Craft,
    /// Put spare seeds into the composter by the field, and take out the bone meal it makes.
    Compost,
    /// Take seeds out of the seeds container to compost them (the meal ran out).
    CompostSeeds,
    /// Bake bread from wheat at a crafting table nearby: the farmer is hungry and has no food.
    Bread,
}
#[derive(Clone, Copy, PartialEq)]
enum Step {
    /// Walking to the container.
    Walk,
    /// A use click is asked for; waiting for its window to open.
    Open,
    /// Shift-clicking stacks across.
    Transfer,
    /// Closing the window.
    Close,
    /// Using items on a block without a window (the composter).
    Use,
}
/// A trip to the farm's storage container, driven between two harvest targets.
#[derive(Clone, Copy)]
struct Store {
    purpose: Purpose,
    step: Step,
    since: Instant,
    last: Instant,
    tries: u8,
    /// Go to another spot before trying again (the chest was not in sight from this one).
    force_move: bool,
    moved: u16,
    stuck: u8,
    sig: usize,
}
struct State {
    scope: String,
    task: Option<Task>,
    pending: Option<Pending>,
    files: Vec<String>,
    active: bool,
    preview: bool,
    phase: Phase,
    target: Option<nav::Pos>,
    plant: bool,
    attempt: usize,
    message: String,
    last: Instant,
    // How long the next cell scan waits, so a dropped block can sink and the water can settle.
    wait: Duration,
    // The wait after a successful placement of the current kind of block.
    settle: Duration,
    // Consecutive scans that found nothing to fill; the task ends after a few.
    idle: u8,
    // Progress watchdog: the count of things done (cells, crops, blocks) and since when it has
    // not changed; a job stuck for too long is moved on and says so.
    progress: (u32, Instant),
    // Things done so far: a harvest, a placement. Feeds the watchdog.
    done: u32,
    // The column being filled: the bot stays on it until its surface cell is solid, then moves
    // on to the next one instead of wandering between columns.
    column: Option<(i32, i32)>,
    // The route along laid blocks being walked (waypoints, how many are done, since when).
    pier: Option<Pier>,
    // Cells that failed or were just filled, ignored until the instant passes.
    skip: Vec<(nav::Pos, Instant)>,
    // The crop of the current target (the one harvested there, or the one being sown), and
    // whether empty farmland is waiting for seeds that are not at hand.
    crop: Option<farm::Crop>,
    unplanted: bool,
    // The job is up and waiting (ripening crops, light or seeds): shown as idle, not paused.
    waiting: bool,
    // Walking back to the beds, and since when.
    home: Option<Instant>,
    // The dropped item being walked to, and the ones given up on.
    loot: Option<Loot>,
    ignored_items: Vec<u32>,
    // The trip to the storage container in progress, the earliest next trip, and whether the
    // farm ran out of ripe crops (a good moment to unload the harvest).
    store: Option<Store>,
    store_after: Instant,
    idle_deposit: bool,
    // When the harvest was last taken to the containers (a few stacks go after a while).
    last_unload: Instant,
    // The harvest container is full and the harvest is to be sold (`/sell hand`).
    sell_due: bool,
    // Until when selling is off after `/sell hand` did not work (the harvest goes to the chest).
    sell_blocked: Instant,
    // Harvest sold since the job started (counted from the hand going down after each
    // `/sell hand`), and when it started (unix ms, to match the server's sale messages).
    sold: u32,
    sold_since: u64,
    // Failed placements in a row, and cells given up on, for the end message.
    errors: u8,
    missed: u16,
    // Whether the block being dropped falls (sand, gravel) and needs time to sink.
    falling: bool,
    // Crops that are still growing were seen (bone meal would help), the current target is one
    // to be fed bone meal, the meal at hand before the click, and when the click was made.
    growing: bool,
    grow: bool,
    grow_sent: bool,
    grow_meal: u32,
    grow_clicks: u8,
    // Every farmland block of the field seen so far, and whether the target is one of them
    // trampled to dirt, to be tilled again with a hoe.
    field: HashSet<nav::Pos>,
    till: bool,
    grow_at: Instant,
    // Waiting in a safe spot: the walk to it and since when, whether it is there, and since
    // when the farm has had nothing to do.
    park: Option<(nav::Pos, Instant)>,
    parked: bool,
    idle_since: Option<Instant>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: String::new(),
            task: None,
            pending: None,
            files: Vec::new(),
            active: false,
            preview: false,
            phase: Phase::Ready,
            target: None,
            plant: false,
            attempt: 0,
            message: String::new(),
            last: Instant::now(),
            wait: Duration::from_millis(100),
            settle: Duration::from_millis(100),
            idle: 0,
            skip: Vec::new(),
            home: None,
            progress: (0, Instant::now()),
            done: 0,
            loot: None,
            ignored_items: Vec::new(),
            column: None,
            pier: None,
            crop: None,
            unplanted: false,
            waiting: false,
            store: None,
            store_after: Instant::now(),
            idle_deposit: false,
            last_unload: Instant::now(),
            sell_due: false,
            sell_blocked: Instant::now(),
            sold: 0,
            sold_since: econ_data::now(),
            errors: 0,
            missed: 0,
            falling: false,
            growing: false,
            grow: false,
            grow_sent: false,
            grow_meal: 0,
            grow_clicks: 0,
            field: HashSet::new(),
            till: false,
            grow_at: Instant::now(),
            park: None,
            parked: false,
            idle_since: None,
        }
    }
}
fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn exists() -> bool {
    state(|s| s.task.is_some() || s.pending.is_some())
}
/// (running, resumable) for the action keys. A paused or stopped task can resume;
/// a finished or previewed one starts over from the tab's settings.
pub(super) fn control() -> Option<(bool, bool)> {
    state(|s| {
        if s.pending.is_some() {
            return Some((true, false));
        }
        s.task
            .as_ref()
            .map(|_| (s.active, !s.active && !s.preview && s.phase != Phase::Done))
    })
}
pub(super) fn pause() {
    state(|s| {
        s.active = false;
        if let Some(p) = s.pending.as_mut() {
            p.run = false;
        }
    });
}
pub(super) fn manual_movement(f: f32, s: f32, j: bool, c: bool) {
    if (f != 0.0 || s != 0.0 || j || c) && !driver::input_grace() {
        pause();
    }
}
pub(super) fn folder(game: &Game<'_>) -> PathBuf {
    game.native_data_dir(claims::SETTINGS_ID).join("schematics")
}
pub(super) fn filenames() -> Vec<String> {
    state(|s| s.files.clone())
}
/// The files of the schematics folder, read again every few seconds while they are asked for
/// (the menu is open): a list that was read in another world (a server, then single player)
/// was dropped with that world's state, and the tab said the folder was empty until Refresh.
pub(super) fn filenames_read(game: &Game<'_>) -> Vec<String> {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_none_or(|t| t.elapsed() > Duration::from_secs(4)) && refresh(game).is_ok() {
        *last = Some(Instant::now());
    }
    filenames()
}
fn guard(game: &Game<'_>) -> Result<String, String> {
    if !game.native_on(claims::ID)
        || game
            .native_setting(claims::SETTINGS_ID, "experiments_accepted")
            .as_deref()
            != Some("true")
    {
        return Err(text(
            "Подтвердите экспериментальные функции",
            "Accept experimental features",
        )
        .into());
    }
    if game.remote.is_none() {
        return Err(text("Войдите на сервер", "Join a server first").into());
    }
    if driver::workshop_pending() || placement::placement_pending() {
        return Err(text(
            "Дождитесь ответа сервера",
            "Wait for the server acknowledgement",
        )
        .into());
    }
    driver::scope(game).ok_or_else(|| "Join a world first".into())
}
pub(super) fn refresh(game: &Game<'_>) -> Result<(), String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let rx = files::spawn(folder(game), None, [0; 3], 0)?;
    state(|s| {
        if s.scope != scope {
            *s = State::default();
        }
        s.scope = scope;
        s.pending = Some(Pending { rx, run: false });
    });
    Ok(())
}
pub(super) fn load(
    game: &Game<'_>,
    filename: String,
    origin: nav::Pos,
    rotation: u8,
    run: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    if run {
        // Kept until it is finished or stopped: it goes on after a restart.
        crate::game::claim_borders_resume::note_build(game, &filename, origin, rotation);
    }
    let rx = files::spawn(folder(game), Some(filename), origin, rotation)?;
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        let names = std::mem::take(&mut s.files);
        *s = State::default();
        s.files = names;
        s.scope = scope;
        s.pending = Some(Pending { rx, run });
        s.message = text("Загрузка", "Loading").into();
    });
    Ok(())
}
pub(super) fn start_farm(
    game: &Game<'_>,
    config: farm::Config,
    preview: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    if !config.valid() {
        return Err("Invalid farming bounds".into());
    }
    let p = game.native_player().ok_or("Join a world first")?;
    // The marked plots of this world, wherever the player stands (after a reconnect too);
    // without any, the square around the player.
    let marked = crate::game::claim_borders_farm_plots_view::enabled(game);
    let (areas, marked) = if marked.is_empty() {
        let world = driver::world_key(game);
        let square = |s: &Option<(String, u8, plots::Bounds)>| {
            s.as_ref()
                .filter(|(w, r, _)| Some(w) == world.as_ref() && *r == config.radius)
                .map(|(_, _, b)| *b)
        };
        // A farm picked up again after a reconnect keeps the square it was started on, not
        // the one around wherever the bot was when the server went down.
        let kept = crate::game::claim_borders_resume::resuming()
            .then(|| square(&LAST_SQUARE.lock().unwrap_or_else(|e| e.into_inner())))
            .flatten();
        let area = kept.unwrap_or_else(|| {
            let origin = nav::standing_block(
                [p.x, p.y, p.z],
                &game.native_block(nav::block([p.x, p.y, p.z])),
            );
            plots::Bounds::around(origin, i32::from(config.radius), config.reach_y())
        });
        if let Some(world) = world {
            *LAST_SQUARE.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((world, config.radius, area));
        }
        (vec![area], false)
    } else {
        (marked, true)
    };
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        *s = State::default();
        s.scope = scope;
        s.task = Some(Task::Farm {
            config,
            areas,
            marked,
            harvested: 0,
        });
        s.preview = preview;
        s.active = !preview;
    });
    Ok(())
}
/// The square of the last farm started without marked plots: its world, radius and bounds.
static LAST_SQUARE: std::sync::Mutex<Option<(String, u8, plots::Bounds)>> =
    std::sync::Mutex::new(None);
pub(super) fn start_drain(
    game: &Game<'_>,
    config: drain::Config,
    preview: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    if !config.valid() {
        return Err("Invalid draining bounds".into());
    }
    if !preview && placement::drain_item(game).is_none() {
        return Err(text(
            "Нужны песок или гравий (хотбар, 2-я рука или инвентарь)",
            "Sand or gravel is required (hotbar, off hand or inventory)",
        )
        .into());
    }
    let p = game.native_player().ok_or("Join a world first")?;
    // The player may be swimming, so the block is not adjusted to a standing position.
    let origin = nav::block([p.x, p.y, p.z]);
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        *s = State::default();
        s.scope = scope;
        s.task = Some(Task::Drain {
            config,
            origin,
            filled: 0,
        });
        s.preview = preview;
        s.active = !preview;
    });
    Ok(())
}
pub(super) fn resume(game: &Game<'_>) -> Result<(), String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    state(|s| {
        if s.scope != scope || s.task.is_none() {
            return Err(text("Задача из другого мира", "Task belongs to another world").into());
        }
        // Never resend an unacknowledged action. Late acknowledgements remain observable.
        if driver::workshop_pending() {
            driver::construction_resume_dig();
        } else if placement::placement_pending() {
            placement::external_resume();
        } else if !(s.phase == Phase::Digging && driver::farm_result() == Some(Ok(())))
            && !(s.phase == Phase::Placing
                && placement::external_result().is_some_and(|r| r.is_ok()))
        {
            s.phase = Phase::Ready;
            driver::clear_construction_driver();
            placement::reset();
        }
        s.active = true;
        s.preview = false;
        s.message.clear();
        // A paused farm picks up from a clean slate: the old target, the idle flag and the
        // trips are stale, and the first look at the beds comes at once (it used to stand
        // "idle" with a blank reason while crops were ripe).
        s.waiting = false;
        s.target = None;
        s.plant = false;
        s.grow = false;
        s.till = false;
        s.attempt = 0;
        s.store = None;
        s.home = None;
        s.park = None;
        s.parked = false;
        s.idle_since = None;
        s.idle_deposit = false;
        s.sell_due = false;
        s.store_after = Instant::now();
        s.last = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        placement::cancel_container();
        placement::cancel_use();
        Ok(())
    })
}
/// The containers the player set, one per role, in the current world.
static STORAGE: Mutex<[Option<(String, nav::Pos)>; 3]> = Mutex::new([None, None, None]);
pub(super) fn storage_of(game: &Game<'_>, role: store::Role) -> Option<nav::Pos> {
    let scope = driver::scope(game)?;
    STORAGE.lock().unwrap_or_else(|e| e.into_inner())[role.index()]
        .as_ref()
        .filter(|(s, _)| *s == scope)
        .map(|(_, at)| *at)
}
/// The harvest container (the one mining unloads into as well).
pub(super) fn storage(game: &Game<'_>) -> Option<nav::Pos> {
    storage_of(game, store::Role::Harvest)
}
/// The three containers: harvest, seeds, bone meal.
pub(super) fn storages(game: &Game<'_>) -> [Option<nav::Pos>; 3] {
    store::Role::ALL.map(|role| storage_of(game, role))
}
/// Takes the container under the crosshair as the one for `role`.
pub(super) fn set_storage(game: &Game<'_>, role: store::Role) -> Result<nav::Pos, String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    let aimed = || {
        text(
            "Наведите прицел на сундук, бочку или шалкер",
            "Aim at a chest, barrel or shulker box",
        )
    };
    let hit = game.pick().ok_or_else(|| aimed().to_owned())?;
    if !store::is_container(&game.native_block(hit.pos)) {
        return Err(aimed().into());
    }
    STORAGE.lock().unwrap_or_else(|e| e.into_inner())[role.index()] = Some((scope, hit.pos));
    Ok(hit.pos)
}
pub(super) fn clear_storage(role: store::Role) {
    STORAGE.lock().unwrap_or_else(|e| e.into_inner())[role.index()] = None;
}
/// The container a trip for `purpose` goes to. Seeds go to the seeds container, or to the
/// harvest one when there is none; bone meal only has its own.
fn purpose_chest(game: &Game<'_>, purpose: Purpose) -> Option<nav::Pos> {
    let [harvest, seeds, meal] = storages(game);
    match purpose {
        Purpose::Deposit => harvest,
        Purpose::DepositSeeds | Purpose::Seeds(_) => seeds.or(harvest),
        Purpose::Meal => meal,
        Purpose::Craft | Purpose::Bread => table_near(game),
        Purpose::CompostSeeds => seeds,
        // Found by the field, not set: see `composter_near`.
        Purpose::Compost => None,
    }
}
/// Containers found empty, not asked again for a while: the seeds of each crop (by its index)
/// and, last, the bone meal. Without this the farm walked to an empty chest every minute.
static EMPTY_SOURCE: Mutex<[Option<Instant>; 6]> = Mutex::new([None; 6]);
const MEAL_SOURCE: usize = 4;
const COMPOST_SOURCE: usize = 5;
fn source_empty(index: usize) -> bool {
    EMPTY_SOURCE.lock().unwrap_or_else(|e| e.into_inner())[index]
        .is_some_and(|until| Instant::now() < until)
}
fn mark_source(index: usize, empty: bool) {
    EMPTY_SOURCE.lock().unwrap_or_else(|e| e.into_inner())[index] =
        empty.then(|| Instant::now() + Duration::from_secs(15 * 60));
}
/// The composter by the field: the nearest one within the field and eight blocks around it,
/// three blocks up or down from its start. Looked for again every 20 seconds at most.
static COMPOSTER: Mutex<Option<(Instant, nav::Pos, Option<nav::Pos>)>> = Mutex::new(None);
fn composter_near(game: &Game<'_>, origin: nav::Pos, radius: i32) -> Option<nav::Pos> {
    let mut cache = COMPOSTER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((when, at, found)) = *cache
        && at == origin
        && when.elapsed() < Duration::from_secs(20)
    {
        return found;
    }
    // By the containers first: that is where a composter belongs (seeds go in and meal comes
    // out next to the chests). Then the field and eight blocks around it.
    let mut best: Option<(i32, nav::Pos)> = None;
    for chest in storages(game).into_iter().flatten() {
        for dx in -4..=4 {
            for dz in -4..=4 {
                for dy in -2..=2 {
                    let at = [chest[0] + dx, chest[1] + dy, chest[2] + dz];
                    let d = dx * dx + dz * dz + dy * dy;
                    if best.is_none_or(|(b, _)| d < b)
                        && game.native_block(at) == "minecraft:composter"
                    {
                        best = Some((d, at));
                    }
                }
            }
        }
    }
    if let Some((_, at)) = best {
        *cache = Some((Instant::now(), origin, Some(at)));
        return Some(at);
    }
    let reach = radius + 8;
    for dx in -reach..=reach {
        for dz in -reach..=reach {
            for dy in -3..=3 {
                let at = [origin[0] + dx, origin[1] + dy, origin[2] + dz];
                let d = dx * dx + dz * dz + dy * dy;
                if best.is_none_or(|(b, _)| d < b) && game.native_block(at) == "minecraft:composter"
                {
                    best = Some((d, at));
                }
            }
        }
    }
    let found = best.map(|(_, at)| at);
    *cache = Some((Instant::now(), origin, found));
    found
}
/// The composter the current farm uses, for the menu.
pub(super) fn farm_composter(game: &Game<'_>) -> Option<nav::Pos> {
    let areas = state(|s| match &s.task {
        Some(Task::Farm { areas, .. }) => Some(areas.clone()),
        _ => None,
    })?;
    farm_composter_in(game, &areas)
}
/// The area of the farm the player is in, or the nearest one.
fn nearest_area(game: &Game<'_>, areas: &[plots::Bounds]) -> Option<plots::Bounds> {
    let p = game.native_player()?;
    let here = [p.x, p.y, p.z];
    areas.iter().copied().min_by(|a, b| {
        let d = |x: &plots::Bounds| {
            if x.near(here, 0.0, 2.0) {
                0.0
            } else {
                nav::distance(nav::center(x.center()), here)
            }
        };
        d(a).total_cmp(&d(b))
    })
}
/// Where to walk to reach a field: a place to stand in its middle column (on a terraced plot
/// the middle level may be inside a step), the middle cell when none is found (not loaded yet).
fn area_home(game: &Game<'_>, area: plots::Bounds) -> nav::Pos {
    let middle = area.center();
    let open = |at: nav::Pos| {
        let name = game.native_block(at);
        // Crops are walked through as the planner walks them.
        name == "minecraft:air" || nav::passable(&name) || farm::Crop::named(&name).is_some()
    };
    (area.lo[1]..=area.hi[1])
        .map(|y| [middle[0], y, middle[2]])
        .find(|at| {
            let below = [at[0], at[1] - 1, at[2]];
            open(*at) && open([at[0], at[1] + 1, at[2]]) && !open(below)
        })
        .unwrap_or(middle)
}
/// The composter by the area of the farm the player is at.
fn farm_composter_in(game: &Game<'_>, areas: &[plots::Bounds]) -> Option<nav::Pos> {
    let area = nearest_area(game, areas)?;
    let (rx, rz) = area.half();
    composter_near(game, area.center(), rx.max(rz))
}
/// A crafting table close to the player (eight blocks around, three up and down), the nearest.
fn table_near(game: &Game<'_>) -> Option<nav::Pos> {
    let p = game.native_player()?;
    let me = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let mut best: Option<(i32, nav::Pos)> = None;
    for dx in -8..=8 {
        for dz in -8..=8 {
            for dy in -3..=3 {
                let at = [me[0] + dx, me[1] + dy, me[2] + dz];
                let d = dx * dx + dz * dz + dy * dy;
                if best.is_none_or(|(b, _)| d < b)
                    && game.native_block(at) == "minecraft:crafting_table"
                {
                    best = Some((d, at));
                }
            }
        }
    }
    best.map(|(_, at)| at)
}
/// Whether a drain job is running (a fall into the water is then recovered from, not a pause).
pub(super) fn draining() -> bool {
    state(|s| s.active && matches!(s.task, Some(Task::Drain { .. })))
}
/// Whether the current task is a drain, running or paused.
pub(super) fn drain_task() -> bool {
    state(|s| matches!(s.task, Some(Task::Drain { .. })))
}
/// Whether the storage window is being used (the movement hook must not pause the job then).
pub(super) fn storing() -> bool {
    state(|s| s.store.is_some_and(|t| t.step != Step::Walk))
}
/// The player's main inventory and hotbar as `farm_storage` plans see them.
fn inventory(game: &Game<'_>) -> Vec<store::Stack> {
    (0..36)
        .map(|g| {
            let stack = if g < 9 {
                game.stack_in(g)
            } else {
                game.inventory.get(g).and_then(Option::as_ref)
            };
            stack
                .filter(|st| st.count > 0)
                .map(|st| (st.item.to_string(), st.count))
        })
        .collect()
}
/// Why a trip to a container is due now, if it is, and which container.
fn store_due(game: &Game<'_>, s: &State) -> Option<(Purpose, nav::Pos)> {
    let Some(Task::Farm { config, areas, .. }) = &s.task else {
        return None;
    };
    if Instant::now() < s.store_after {
        return None;
    }
    let inv = inventory(game);
    let crops: Vec<_> = config.selected().collect();
    // Hungry with nothing to eat: bread from the wheat, at a crafting table nearby.
    if hungry_without_food(game)
        && store::count(&inv, WHEAT) >= 3
        && craft_allowed()
        && let Some(table) = purpose_chest(game, Purpose::Bread)
    {
        craft_started();
        return Some((Purpose::Bread, table));
    }
    // Empty farmland and a selected crop with no seeds on hand: fetch them (not from a
    // container that had none a moment ago).
    if config.replant
        && s.unplanted
        && let Some(crop) = config
            .selected()
            .find(|c| !seed(game, *c) && !source_empty(*c as usize))
        && let Some(chest) = purpose_chest(game, Purpose::Seeds(crop))
    {
        return Some((Purpose::Seeds(crop), chest));
    }
    let meal = store::count(&inv, store::BONE_MEAL);
    let low =
        config.bonemeal && s.growing && meal < 8 && store::count(&inv, store::BONE_BLOCK) == 0;
    // Crops are growing and the meal ran out: fetch meal, or blocks to craft it from. An empty
    // meal container is left alone for a while: the farm just sows and harvests meanwhile.
    if low
        && !source_empty(MEAL_SOURCE)
        && let Some(chest) = purpose_chest(game, Purpose::Meal)
    {
        return Some((Purpose::Meal, chest));
    }
    let composter = if config.bonemeal {
        farm_composter_in(game, areas)
    } else {
        None
    };
    let spare = store::compost_seed(&inv, &crops, store::KEEP_SEEDS).is_some();
    if low && let Some(at) = composter {
        // No meal to fetch: the seeds make it. Spare seeds on hand go into the composter;
        // without any, two stacks are fetched from the seeds container first.
        if spare {
            return Some((Purpose::Compost, at));
        }
        if !source_empty(COMPOST_SOURCE)
            && let Some(chest) = purpose_chest(game, Purpose::CompostSeeds)
        {
            return Some((Purpose::CompostSeeds, chest));
        }
    }
    let [_, seeds_chest, _] = storages(game);
    let split = store::deposit_split(&inv, &crops, store::KEEP_SEEDS, seeds_chest.is_some());
    // Not after every crop: when the inventory fills up, the field is idle, a lot has gathered,
    // or a few stacks have waited a while.
    if !store::unload_due(
        &inv,
        split.harvest.len() + split.seeds.len(),
        s.idle_deposit,
        s.last_unload.elapsed().as_secs(),
    ) {
        return None;
    }
    // Spare seeds make bone meal before they go into the seeds container.
    if spare
        && meal < store::MEAL_STOCK
        && let Some(at) = composter
    {
        return Some((Purpose::Compost, at));
    }
    // With selling on, the harvest is sold where the farm stands (`sell_frame`), not stored;
    // only when the command does not work does it go to the harvest container.
    let selling = config.autosell && Instant::now() >= s.sell_blocked;
    // Wheat to bales first, so that the bales go in with the same unload.
    if !selling
        && store::hay_plan(&inv, &crops).is_some()
        && craft_allowed()
        && let Some(table) = purpose_chest(game, Purpose::Craft)
    {
        craft_started();
        return Some((Purpose::Craft, table));
    }
    if !selling
        && !split.harvest.is_empty()
        && let Some(chest) = purpose_chest(game, Purpose::Deposit)
    {
        return Some((Purpose::Deposit, chest));
    }
    if !split.seeds.is_empty()
        && let Some(chest) = purpose_chest(game, Purpose::DepositSeeds)
    {
        return Some((Purpose::DepositSeeds, chest));
    }
    None
}
const WHEAT: &str = "minecraft:wheat";
/// Whether anything in the inventory can be eaten (and is not one of the foods eating on its
/// own leaves alone).
fn has_food(game: &Game<'_>) -> bool {
    let allowed = crate::game::claim_borders_navigation_view::auto_eat_foods(game);
    (0..36).any(|g| {
        let stack = if g < 9 {
            game.stack_in(g)
        } else {
            game.inventory.get(g).and_then(Option::as_ref)
        };
        stack.filter(|st| st.count > 0).is_some_and(|st| {
            crate::game::claim_borders_auto_eat::allowed(&st.item, &allowed)
                && game
                    .res
                    .items()
                    .get(&st.item)
                    .is_some_and(|item| item.food.is_some_and(|(nutrition, _)| nutrition > 0))
        })
    })
}
/// Restock only food that Auto Eat is enabled and allowed to use.
fn can_auto_bread(game: &Game<'_>) -> bool {
    crate::game::claim_borders_navigation_view::auto_eat_enabled(game)
        && crate::game::claim_borders_auto_eat::allowed(
            "minecraft:bread",
            &crate::game::claim_borders_navigation_view::auto_eat_foods(game),
        )
}
/// Restock at the existing farm threshold, not merely because a larger meal does not fit yet.
fn hungry_without_food(game: &Game<'_>) -> bool {
    can_auto_bread(game)
        && game.player.as_ref().is_some_and(|p| {
            (p.health.food.food as i32) <= crate::game::claim_borders_auto_eat::HUNGRY_AT
        })
        && !has_food(game)
}
/// The first hoe in the inventory, any material.
fn hoe(game: &Game<'_>) -> Option<String> {
    inventory(game)
        .into_iter()
        .flatten()
        .map(|(name, _)| name)
        .find(|name| name.ends_with("_hoe"))
}
/// Crafting is not tried again for a while after a trip to a table: one that cannot be opened
/// must not hold the unload back.
static CRAFT_RETRY: Mutex<Option<Instant>> = Mutex::new(None);
fn craft_allowed() -> bool {
    CRAFT_RETRY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_none_or(|t| Instant::now() >= t)
}
fn craft_started() {
    *CRAFT_RETRY.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(Instant::now() + Duration::from_secs(90));
}
/// On the way to unload: with wheat in the inventory, a crafting table close by turns the trip
/// into a crafting one first (the bales then go in with the unload). Without a table the wheat
/// is put into the container as it is. Looked at twice a second at most.
fn craft_on_the_way(game: &Game<'_>, t: Store) {
    static LOOKED: Mutex<Option<Instant>> = Mutex::new(None);
    if t.purpose != Purpose::Deposit || !craft_allowed() {
        return;
    }
    {
        let mut looked = LOOKED.lock().unwrap_or_else(|e| e.into_inner());
        if looked.is_some_and(|l| l.elapsed() < Duration::from_millis(500)) {
            return;
        }
        *looked = Some(Instant::now());
    }
    let crops: Vec<_> = state(|s| match &s.task {
        Some(Task::Farm { config, .. }) => config.selected().collect(),
        _ => Vec::new(),
    });
    if store::hay_plan(&inventory(game), &crops).is_none() {
        return;
    }
    let Some(table) = table_near(game) else {
        return;
    };
    craft_started();
    state(|s| {
        if let Some(t) = s.store.as_mut() {
            t.purpose = Purpose::Craft;
            t.tries = 0;
            t.force_move = false;
            t.since = Instant::now();
        }
        s.target = Some(table);
    });
}
/// Walks within reach of the storage, then asks for the click that opens it.
fn store_walk(game: &Game<'_>, s: &mut State) {
    let Some(chest) = s.target else {
        s.store = None;
        return;
    };
    if s.store.is_none_or(|t| t.step != Step::Walk) {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let middle = [
        f64::from(chest[0]) + 0.5,
        f64::from(chest[1]) + 0.5,
        f64::from(chest[2]) + 0.5,
    ];
    let force = s.store.is_some_and(|t| t.force_move);
    if force || nav::distance(middle, [p.x, p.y + 1.2, p.z]) > 3.4 {
        if let Some(t) = s.store.as_mut() {
            t.force_move = false;
        }
        approach(game, s, chest);
        return;
    }
    if s.store.is_some_and(|t| t.purpose == Purpose::Compost) {
        // A composter has no window: items are used on it.
        if let Some(t) = s.store.as_mut() {
            t.step = Step::Use;
            t.since = Instant::now();
            t.last = Instant::now();
        }
        return;
    }
    if let Some(t) = s.store.as_mut() {
        t.step = Step::Open;
        t.since = Instant::now();
    }
    placement::open_container(chest);
}
/// Closes the storage window if it is open and ends the trip, with a reason when it failed.
fn end_store(game: &mut Game<'_>, failure: Option<&str>) {
    end_store_for(game, failure, 15);
}
/// As `end_store`, with the pause (in seconds) before the next trip.
/// When the current trip's window began to be used (the cap on a transfer).
static TRANSFER_SINCE: Mutex<Option<Instant>> = Mutex::new(None);
fn end_store_for(game: &mut Game<'_>, failure: Option<&str>, cooldown: u64) {
    *TRANSFER_SINCE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if game.screen == Screen::Inventory {
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    placement::cancel_container();
    state(|s| {
        // A trip for seeds that failed (no way to the container, it would not open) counts as
        // finding none: the farm stops asking for them for a while instead of trying again and
        // again, and harvests and sows what it has meanwhile.
        if failure.is_some()
            && let Some(Purpose::Seeds(crop)) = s.store.as_ref().map(|t| t.purpose)
        {
            mark_source(crop as usize, true);
        }
        s.store = None;
        s.target = None;
        s.phase = Phase::Ready;
        s.last = Instant::now();
        s.wait = Duration::from_millis(250);
        // Not again at once, whatever the trip did.
        s.store_after = Instant::now() + Duration::from_secs(cooldown);
        match failure {
            // The trip ended badly: carry on with the field, say why, ask again later.
            Some(message) => {
                s.message = message.into();
                s.waiting = true;
            }
            None => s.message.clear(),
        }
    });
}
/// Drives the storage window each tick: waits for it to open, moves a few stacks, closes it.
pub(super) fn store_frame(game: &mut Game<'_>) {
    craft_frame(game);
    sell_frame(game);
    let container = game.screen == Screen::Inventory
        && game
            .remote_menu_type()
            .is_some_and(|menu| store::is_container_menu(menu) || menu == "minecraft:crafting");
    // A container window left open with no trip to drive it (it opened after the trip gave up
    // on it, or the trip ended without closing it): the farm stood in front of the chest for
    // hours. It is closed after a few seconds while the farm runs.
    static ORPHAN: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut orphan = ORPHAN.lock().unwrap_or_else(|e| e.into_inner());
        let unattended = container
            && state(|s| {
                s.active
                    && !s.preview
                    && matches!(s.task, Some(Task::Farm { .. }))
                    && s.store
                        .is_none_or(|t| !matches!(t.step, Step::Open | Step::Transfer))
            });
        if !unattended {
            *orphan = None;
        } else if orphan.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(6) {
            *orphan = None;
            drop(orphan);
            game.close_container();
            game.screen = Screen::Playing;
            game.capture(true);
            return;
        }
    }
    let store = state(|s| s.store);
    if store.is_none_or(|t| t.step != Step::Transfer) {
        *TRANSFER_SINCE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    let Some(t) = store else {
        return;
    };
    if !game.native_on(claims::ID) {
        end_store(game, None);
        return;
    }
    let open = game.screen == Screen::Inventory
        && game.remote_menu_type().is_some_and(|menu| {
            store::is_container_menu(menu)
                || (matches!(t.purpose, Purpose::Craft | Purpose::Bread)
                    && menu == "minecraft:crafting")
        });
    match t.step {
        // The window of the container opened after the click was given up on: use it.
        Step::Walk if open => state(|s| {
            if let Some(t) = s.store.as_mut() {
                t.step = Step::Transfer;
                t.last = Instant::now();
            }
        }),
        Step::Walk => craft_on_the_way(game, t),
        Step::Open => {
            if open {
                state(|s| {
                    if let Some(t) = s.store.as_mut() {
                        t.step = Step::Transfer;
                        t.last = Instant::now();
                    }
                });
            } else if placement::container_blocked()
                || t.since.elapsed() > Duration::from_millis(2500)
            {
                if t.tries >= 3 {
                    end_store(
                        game,
                        Some(text(
                            "Не удалось открыть склад: к нему нет свободного пути взгляда",
                            "Could not open the storage: no clear line of sight to it",
                        )),
                    );
                } else {
                    // From here the chest is hidden (crops, a wall) or the click did not open
                    // it: stand somewhere else and try again.
                    placement::cancel_container();
                    state(|s| {
                        if let Some(t) = s.store.as_mut() {
                            t.tries += 1;
                            t.step = Step::Walk;
                            t.force_move = true;
                            t.since = Instant::now();
                        }
                    });
                }
            }
        }
        Step::Transfer => {
            let began = *TRANSFER_SINCE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(Instant::now);
            if !open {
                end_store(game, Some(text("Склад закрылся", "The storage closed")));
            } else if began.elapsed() > Duration::from_secs(40) {
                // Whatever keeps it busy (a stack that never moves, a menu that never answers),
                // the window does not stay open for good.
                end_store(
                    game,
                    Some(text(
                        "Склад не отвечает, попробую позже",
                        "The storage does not answer, trying later",
                    )),
                );
            } else if t.last.elapsed() >= Duration::from_millis(160) {
                transfer(game, t);
            }
        }
        Step::Close => end_store(game, None),
        Step::Use => compost_frame(game, t),
    }
}
/// At the composter: spare seeds go in one click at a time (each fills a layer by chance),
/// a full composter is emptied with a click (the bone meal pops out and is picked up after),
/// and the trip ends when no seeds are spare or the meal stock is full.
fn compost_frame(game: &mut Game<'_>, t: Store) {
    let Some(at) = state(|s| s.target) else {
        end_store(game, None);
        return;
    };
    if placement::use_busy() || t.last.elapsed() < Duration::from_millis(300) {
        return;
    }
    let clicked = placement::use_result();
    if clicked == Some(false) {
        end_store(
            game,
            Some(text(
                "Компостер не виден отсюда",
                "The composter cannot be reached from here",
            )),
        );
        return;
    }
    if game.native_block(at) != "minecraft:composter" {
        end_store(
            game,
            Some(text("Компостер пропал", "The composter is gone")),
        );
        return;
    }
    let crops = state(|s| match &s.task {
        Some(Task::Farm { config, .. }) => config.selected().collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    let level: u8 = game
        .native_block_property(at, "level")
        .and_then(|l| l.parse().ok())
        .unwrap_or(0);
    let inv = inventory(game);
    let spare = store::compost_seed(&inv, &crops, store::KEEP_SEEDS);
    let meal = store::count(&inv, store::BONE_MEAL);
    // Anything in hand empties a full composter; seeds are always about.
    let any = store::COMPOST_SEEDS
        .into_iter()
        .chain(crops.iter().map(|c| c.seed()))
        .chain([store::BONE_MEAL])
        .find(|item| store::count(&inv, item) > 0);
    let item = match level {
        8.. => any,
        // Ready in a moment (the server turns 7 into 8 after a second).
        7 if t.last.elapsed() < Duration::from_millis(2500) => return,
        7 => any,
        _ if meal < store::MEAL_STOCK => spare,
        _ => None,
    };
    let Some(item) = item else {
        // Done: the meal it made is picked up by the loot pass on the way back.
        end_store_for(game, None, 5);
        return;
    };
    let seeds = spare.map_or(0, |seed| store::count(&inv, seed)) as usize;
    let stuck = if level < 7 && seeds == t.sig {
        t.stuck + 1
    } else {
        0
    };
    if stuck >= 5 || t.moved >= 400 {
        end_store(
            game,
            Some(text(
                "Компостер не принимает семена",
                "The composter does not take the seeds",
            )),
        );
        return;
    }
    placement::use_item_on(item, at);
    state(|s| {
        s.message = text("компостирую семена", "composting seeds").into();
        if let Some(t) = s.store.as_mut() {
            t.last = Instant::now();
            t.moved += 1;
            t.stuck = stuck;
            t.sig = seeds;
        }
    });
}
/// One round of shift-clicks between the inventory and the open container.
fn transfer(game: &mut Game<'_>, t: Store) {
    use horizon_client::menu::Input;
    let Some(crops) = state(|s| match &s.task {
        Some(Task::Farm { config, .. }) => Some(config.selected().collect::<Vec<_>>()),
        _ => None,
    }) else {
        end_store(game, None);
        return;
    };
    let inv = inventory(game);
    // The container's own slots are the menu's slots but the 36 of the player's inventory.
    let mut chest: Vec<store::Stack> = Vec::new();
    game.menu_session(|menu, _env, _to_menu| {
        let n = menu.slot_count();
        if n >= 36 {
            for i in 0..n - 36 {
                chest.push(
                    menu.slot(i)
                        .filter(|st| st.count > 0)
                        .map(|st| (st.item.to_string(), st.count)),
                );
            }
        }
        Some(false)
    });
    if chest.is_empty() {
        end_store(
            game,
            Some(text("Окно склада недоступно", "Storage window unavailable")),
        );
        return;
    }
    let seeds_set = storages(game)[1].is_some();
    let (plan, ops): (Vec<usize>, Vec<_>) = match t.purpose {
        Purpose::Deposit | Purpose::DepositSeeds => {
            let split = store::deposit_split(&inv, &crops, store::KEEP_SEEDS, seeds_set);
            let plan = if t.purpose == Purpose::Deposit {
                split.harvest
            } else {
                split.seeds
            };
            let ops = plan
                .iter()
                .take(4)
                .map(|g| (Some(*g), 0, Input::QuickMove))
                .collect();
            (plan, ops)
        }
        Purpose::Seeds(crop) => {
            let plan = store::withdraw(&chest, &inv, crop.seed(), store::TAKE_SEEDS);
            let ops = plan
                .iter()
                .take(2)
                .map(|i| (Some(300 + *i), 0, Input::QuickMove))
                .collect();
            (plan, ops)
        }
        Purpose::Craft => {
            use horizon_client::menu::Input;
            // The wheat stack goes into the 3x3 grid one item per cell, a round of up to four
            // bales at a time; a shift-click on the result crafts them, the rest goes back.
            let plan = store::hay_plan(&inv, &crops)
                .map(|(slot, bales)| {
                    vec![
                        slot,
                        bales as usize,
                        store::count(&inv, "minecraft:wheat") as usize,
                    ]
                })
                .unwrap_or_default();
            let mut ops = Vec::new();
            if let [slot, bales, _] = plan[..] {
                ops.push((Some(slot), 0, Input::Pickup));
                for _ in 0..bales {
                    ops.extend((0..9).map(|cell| (Some(100 + cell), 1, Input::Pickup)));
                }
                ops.push((Some(200), 0, Input::QuickMove));
                ops.push((Some(slot), 0, Input::Pickup));
            }
            (plan, ops)
        }
        Purpose::Meal => {
            let plan = store::meal_withdraw(&chest, &inv);
            let ops = plan
                .iter()
                .take(2)
                .map(|i| (Some(300 + *i), 0, Input::QuickMove))
                .collect();
            (plan, ops)
        }
        Purpose::CompostSeeds => {
            // Two stacks beyond the ones kept for sowing, of whichever seeds the chest has.
            let plan = store::COMPOST_SEEDS
                .into_iter()
                .map(|seed| store::withdraw(&chest, &inv, seed, store::KEEP_SEEDS + 128))
                .find(|plan| !plan.is_empty())
                .unwrap_or_default();
            let ops = plan
                .iter()
                .take(2)
                .map(|i| (Some(300 + *i), 0, Input::QuickMove))
                .collect();
            (plan, ops)
        }
        Purpose::Bread => {
            use horizon_client::menu::Input;
            // Three wheat in the top row of the 3x3 grid make a loaf: the stack goes in one per
            // cell, up to three loaves, a shift-click takes them, the rest goes back. Done once
            // there is food.
            let plan = if has_food(game) || !can_auto_bread(game) {
                Vec::new()
            } else {
                inv.iter()
                    .position(|st| st.as_ref().is_some_and(|(n, c)| n == WHEAT && *c >= 3))
                    .map(|slot| {
                        let n = inv[slot].as_ref().map_or(0, |(_, c)| *c);
                        vec![
                            slot,
                            (n / 3).min(3) as usize,
                            store::count(&inv, WHEAT) as usize,
                        ]
                    })
                    .unwrap_or_default()
            };
            let mut ops = Vec::new();
            if let [slot, loaves, _] = plan[..] {
                ops.push((Some(slot), 0, Input::Pickup));
                for _ in 0..loaves {
                    ops.extend((0..3).map(|cell| (Some(100 + cell), 1, Input::Pickup)));
                }
                ops.push((Some(200), 0, Input::QuickMove));
                ops.push((Some(slot), 0, Input::Pickup));
            }
            (plan, ops)
        }
        // No window to move stacks in: done in `compost_frame`.
        Purpose::Compost => (Vec::new(), Vec::new()),
    };
    let sig = plan.iter().sum::<usize>() + plan.len() * 100;
    let stuck = if sig == t.sig { t.stuck + 1 } else { 0 };
    if plan.is_empty() {
        // Nothing (more) to move. Seeds or meal that are not in the container either are no
        // error: the farm idles and asks again a minute later.
        let none = match t.purpose {
            Purpose::Seeds(crop) => store::count(&inv, crop.seed()) == 0,
            Purpose::Meal => {
                store::count(&inv, store::BONE_MEAL) == 0
                    && store::count(&inv, store::BONE_BLOCK) == 0
            }
            Purpose::CompostSeeds => store::compost_seed(&inv, &crops, store::KEEP_SEEDS).is_none(),
            _ => false,
        };
        // An empty container is not visited again for a while; a full one is fine again.
        match t.purpose {
            Purpose::Seeds(crop) => mark_source(crop as usize, none),
            Purpose::Meal => mark_source(MEAL_SOURCE, none),
            Purpose::CompostSeeds => mark_source(COMPOST_SOURCE, none),
            Purpose::Deposit | Purpose::DepositSeeds => {
                state(|s| s.last_unload = Instant::now());
            }
            _ => {}
        }
        // The harvest is in: the spare seeds are the next trip, soon.
        let more = t.purpose == Purpose::Craft
            || (t.purpose == Purpose::Deposit
                && seeds_set
                && !store::deposit_split(&inv, &crops, store::KEEP_SEEDS, true)
                    .seeds
                    .is_empty());
        end_store_for(
            game,
            None,
            if none {
                60
            } else if more {
                2
            } else {
                15
            },
        );
        if more {
            // The unload trip reset the flag: keep the seeds trip due, or it never happens
            // while there are fewer than three free slots missing.
            state(|s| s.idle_deposit = true);
        }
        if none {
            state(|s| {
                s.message = match t.purpose {
                    // No meal is no reason to stop: sow, harvest and wait as without it.
                    Purpose::Meal | Purpose::CompostSeeds => text(
                        "костная мука закончилась, работаю без неё",
                        "out of bone meal, farming without it",
                    ),
                    _ => text(
                        "в складе нет семян, жду созревания",
                        "no seeds in the storage, waiting for crops to ripen",
                    ),
                }
                .into();
                s.waiting = matches!(t.purpose, Purpose::Seeds(_));
            });
        }
        return;
    }
    if (stuck >= 3 || t.moved >= 200)
        && t.purpose == Purpose::Deposit
        && state(|s| matches!(&s.task, Some(Task::Farm { config, .. }) if config.autosell))
    {
        // The harvest container is full: sell what is left instead of giving up.
        state(|s| s.sell_due = true);
        end_store_for(game, None, 30);
        return;
    }
    if stuck >= 3 || t.moved >= 200 {
        // Nothing moves any more: the container is full or refuses the stacks.
        end_store(
            game,
            Some(text(
                "Склад полон или не принимает предметы",
                "The storage is full or refuses the items",
            )),
        );
        return;
    }
    if !game.menu_ops(&ops) {
        end_store(
            game,
            Some(text("Окно склада недоступно", "Storage window unavailable")),
        );
        return;
    }
    state(|s| {
        if let Some(t) = s.store.as_mut() {
            t.last = Instant::now();
            t.moved += ops.len() as u16;
            t.stuck = stuck;
            t.sig = sig;
        }
    });
}
/// Puts bone meal on the crop at `target`: the click is asked of the placement driver, the
/// `Growing` phase then looks at the result.
fn grow_click(game: &Game<'_>, s: &mut State, target: nav::Pos, crop: farm::Crop) {
    let ripe = game.native_block(target) != crop.block()
        || crop.mature(game.native_block_property(target, "age").as_deref());
    let meal = store::count(&inventory(game), store::BONE_MEAL);
    if ripe || meal == 0 {
        // Already grown, or the meal ran out: the next look at the beds decides.
        s.grow = false;
        s.target = None;
        return;
    }
    placement::use_item_on(store::BONE_MEAL, target);
    s.grow_meal = meal;
    s.grow_clicks = 0;
    s.grow_sent = false;
    s.grow_at = Instant::now();
    s.phase = Phase::Growing;
}
/// Soil a hoe turns into farmland in one click.
fn tillable(block: &str) -> bool {
    matches!(
        block,
        "minecraft:dirt" | "minecraft:grass_block" | "minecraft:dirt_path"
    )
}
/// Uses the hoe on the trampled soil at `target`; the `Tilling` phase looks at the result.
fn till_click(game: &Game<'_>, s: &mut State, target: nav::Pos) {
    let Some(hoe) = hoe(game).filter(|_| tillable(&game.native_block(target))) else {
        // No hoe any more, or it is farmland (or something else) already.
        s.till = false;
        s.target = None;
        return;
    };
    placement::use_item_on(&hoe, target);
    s.grow_sent = false;
    s.grow_at = Instant::now();
    s.phase = Phase::Tilling;
}
fn till_give_up(s: &mut State, target: nav::Pos) {
    placement::cancel_use();
    s.skip
        .push((target, Instant::now() + Duration::from_secs(30)));
    s.till = false;
    s.target = None;
    s.phase = Phase::Ready;
}
/// Leaves this crop alone for a while (no line to it, or the meal does nothing) and goes on.
fn grow_give_up(s: &mut State, target: nav::Pos) {
    placement::cancel_use();
    s.skip
        .push((target, Instant::now() + Duration::from_secs(25)));
    s.grow = false;
    s.target = None;
    s.phase = Phase::Ready;
}
/// The safe point the player set: where an idle farm waits (a nook, behind a wall), in the
/// current world.
static SAFE: Mutex<Option<(String, nav::Pos)>> = Mutex::new(None);
pub(super) fn safe_point(game: &Game<'_>) -> Option<nav::Pos> {
    let scope = driver::scope(game)?;
    SAFE.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .filter(|(s, _)| *s == scope)
        .map(|(_, at)| *at)
}
/// Takes the cell the player stands on as the safe point.
pub(super) fn set_safe_point(game: &Game<'_>) -> Result<nav::Pos, String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    let p = game.native_player().ok_or("Join a world first")?;
    let at = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    );
    *SAFE.lock().unwrap_or_else(|e| e.into_inner()) = Some((scope, at));
    Ok(at)
}
pub(super) fn clear_safe_point() {
    *SAFE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
/// While the farm has nothing to do it waits at the safe point the player set (not in the open
/// between the beds, where a mob or a player could get at it): after a few idle seconds it walks
/// there and stays; the next ripe crop, sowing or feeding sends it back. With no point set it
/// stays where it is.
fn park_step(game: &Game<'_>, s: &mut State) {
    let Some(spot) = safe_point(game) else {
        return;
    };
    if let Some((_, since)) = s.park {
        match driver::workshop_walk_result() {
            Some(_) => {
                s.park = None;
                s.parked = true;
            }
            None if since.elapsed() > Duration::from_secs(180) => {
                s.park = None;
                s.parked = true;
            }
            None => {}
        }
        return;
    }
    if s.parked || s.store.is_some() {
        return;
    }
    let since = *s.idle_since.get_or_insert_with(Instant::now);
    if since.elapsed() < Duration::from_secs(6) {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    if nav::distance(nav::center(spot), [p.x, p.y, p.z]) < 1.2 {
        s.parked = true;
        return;
    }
    // Like every walk: on foot first; with no way on foot, blocks in the way are broken and
    // gaps bridged (when a tool and blocks are at hand).
    if driver::workshop_walk(game, spot, false, false).is_ok() {
        s.park = Some((spot, Instant::now()));
        s.message = text("иду в безопасное место", "walking to the safe spot").into();
    } else {
        s.parked = true;
    }
}
/// Crafts bone meal from bone blocks in the inventory (the 2x2 grid of the player's own menu,
/// no screen needed): one block to the grid at a time is not needed, the whole number goes in
/// and a shift-click on the result crafts them all. Never more than `MEAL_STOCK` meal.
fn craft_frame(game: &mut Game<'_>) {
    use horizon_client::menu::Input;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let wanted = state(|s| match (&s.task, s.active, &s.store) {
        (Some(Task::Farm { config, .. }), true, None) => config.bonemeal,
        _ => false,
    });
    if !wanted
        || !game.native_on(claims::ID)
        || !game.native_playing()
        || game.screen != Screen::Playing
    {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed() < Duration::from_millis(900)) {
        return;
    }
    let Some((slot, blocks)) = store::craft_plan(&inventory(game)) else {
        return;
    };
    // Lift the stack, put one block at a time into the grid, craft, put the rest back.
    let mut ops = vec![(Some(slot), 0, Input::Pickup)];
    ops.extend((0..blocks).map(|_| (Some(100), 1, Input::Pickup)));
    ops.push((Some(200), 0, Input::QuickMove));
    ops.push((Some(slot), 0, Input::Pickup));
    *last = Some(Instant::now());
    game.remote_menu_ops(&ops);
}
/// A stack this big starts a sale while there is still harvesting to do.
const SELL_STACK: u32 = 32;
/// How long selling stays off after the command did not work.
const SELL_PAUSE: Duration = Duration::from_secs(300);
/// Sells the harvest with `/sell hand` as it is gathered (with selling on, the harvest does not
/// go to the container): between two crops, once a stack has grown or the field is idle, each
/// stack of the crops' yield goes to the selected hotbar slot (a swap click), the command sells
/// what is in the hand, and the next stack follows. A stack that does not shrink after three
/// commands stops the selling for a few minutes (no such command here, or nothing is bought);
/// the harvest goes to the container meanwhile.
fn sell_frame(game: &mut Game<'_>) {
    use horizon_client::menu::Input;
    static LAST: Mutex<Option<(Instant, u32, u8)>> = Mutex::new(None);
    // The stack in hand when the last command went: slot, item, count, when.
    static SENT: Mutex<Option<(usize, String, u32, Instant)>> = Mutex::new(None);
    {
        let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((slot, item, count, at)) = sent.as_ref() {
            let now = game
                .stack_in(*slot)
                .filter(|st| st.count > 0 && st.item.as_ref() == item.as_str())
                .map_or(0, |st| st.count);
            if now < *count {
                let gone = count - now;
                state(|s| s.sold = s.sold.saturating_add(gone));
                *sent = None;
            } else if at.elapsed() > Duration::from_secs(2) {
                *sent = None;
            }
        }
    }
    let (wanted, due, crops) = state(|s| match (&s.task, s.active, &s.store) {
        (Some(Task::Farm { config, .. }), true, None)
            if config.autosell
                && (s.sell_due || Instant::now() >= s.sell_blocked)
                // Not in the middle of a dig or a sowing: the hand is in use then.
                && s.phase == Phase::Ready
                && !driver::workshop_pending()
                && !placement::placement_pending() =>
        {
            (
                true,
                s.sell_due || s.waiting,
                config.selected().collect::<Vec<_>>(),
            )
        }
        _ => (false, false, Vec::new()),
    });
    if !wanted
        || !game.native_on(claims::ID)
        || !game.native_playing()
        || game.screen != Screen::Playing
    {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|(t, _, _)| t.elapsed() < Duration::from_millis(900)) {
        return;
    }
    let inv = inventory(game);
    let slots = store::sell_slots(&inv, &crops, store::KEEP_SEEDS);
    // A sale starts once a stack has grown or there is nothing to harvest just now (not one
    // command per carrot); once started, every stack goes.
    let started = last.is_some();
    if !started
        && !due
        && !slots
            .iter()
            .any(|&i| inv[i].as_ref().is_some_and(|(_, n)| *n >= SELL_STACK))
    {
        return;
    }
    let Some(&slot) = slots.first() else {
        // Everything is sold.
        *last = None;
        state(|s| {
            s.sell_due = false;
            s.message.clear();
        });
        return;
    };
    let selected = usize::from(game.selected);
    if slot != selected {
        // Into the hand first (a number-key swap, the selected slot itself stays selected).
        if !game.remote_menu_ops(&[(Some(slot), selected as i32, Input::Swap)]) {
            state(|s| {
                s.sell_due = false;
                s.sell_blocked = Instant::now() + SELL_PAUSE;
            });
        }
        *last = Some((Instant::now(), 0, 0));
        return;
    }
    let here = inv[slot].as_ref().map_or(0, |(_, n)| *n);
    let same = match *last {
        Some((_, count, tries)) if count == here => tries + 1,
        _ => 0,
    };
    let sent = same < 3
        && game.chat_abilities().send_commands
        && game.native_send_chat("/sell hand").is_ok();
    if !sent {
        *last = None;
        state(|s| {
            s.sell_due = false;
            s.sell_blocked = Instant::now() + SELL_PAUSE;
            s.message = text(
                "/sell hand не сработал, урожай идёт в сундук",
                "/sell hand did not work, the harvest goes to the chest",
            )
            .into();
        });
        return;
    }
    *last = Some((Instant::now(), here, same));
    if let Some((item, _)) = inv[slot].as_ref() {
        *SENT.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((slot, item.clone(), here, Instant::now()));
    }
    state(|s| {
        s.message = text(
            "продажа урожая: /sell hand",
            "selling the harvest: /sell hand",
        )
        .into()
    });
}
/// Stands idle instead of stopping: the job stays up, says why and looks again in a few seconds.
/// Only the player's own stop ends a job; a missing block, a route that is not there, a refusal
/// are things that change.
fn idle(s: &mut State, message: impl Into<String>) {
    s.message = message.into();
    s.waiting = true;
    s.target = None;
    s.column = None;
    s.pier = None;
    s.attempt = 0;
    s.phase = Phase::Ready;
    s.last = Instant::now() + Duration::from_secs(3);
}
fn fail(s: &mut State, message: impl Into<String>) {
    s.active = false;
    s.message = message.into();
    if !driver::workshop_pending() {
        driver::clear_construction_driver();
    }
    placement::pause();
}
fn matches(game: &Game<'_>, at: nav::Pos, w: &schema::BlockState) -> bool {
    w.matches(&game.native_block(at), |key| {
        game.native_block_property(at, key)
    })
}
/// Seeds anywhere the placement driver can reach them: hotbar, off hand or inventory.
fn seed(game: &Game<'_>, crop: farm::Crop) -> bool {
    placement::has_item(game, &|name| name == crop.seed())
}
fn desired(game: &Game<'_>, wanted: &schema::BlockState) -> Result<String, String> {
    let registry = &game.pack.assets.registry;
    let id = registry.default_state(&wanted.name).ok_or_else(|| {
        format!(
            "{}: {}",
            text("Неизвестный блок", "Unknown block"),
            wanted.name
        )
    })?;
    let facts = game.res.facts().get(id);
    if facts.block_entity || facts.fluid_level > 0 || facts.waterlogged {
        return Err(format!(
            "{}: {}",
            text("Данные блока не поддерживаются", "Unsupported block data"),
            wanted.name
        ));
    }
    if facts.item.is_empty() || facts.item.as_ref() == "minecraft:air" {
        Err(format!(
            "{}: {}",
            text("Нет предмета для установки", "No placeable item"),
            wanted.name
        ))
    } else {
        Ok(facts.item.to_string())
    }
}
/// Collects what the harvest dropped: walks to the nearest item lying in or around the farm
/// that has not been picked up and waits for the pickup, giving up on an item that cannot be
/// reached or taken. True while it is busy with one (the job waits).
fn loot_step(game: &Game<'_>, s: &mut State) -> bool {
    let areas = match &s.task {
        Some(Task::Farm { areas, .. }) => areas.clone(),
        _ => return false,
    };
    let Some(p) = game.native_player() else {
        return false;
    };
    let here = [p.x, p.y, p.z];
    let at = |d: &horizon_client::entities::ItemEntity| [d.position.x, d.position.y, d.position.z];
    // Besides the field: around the composter and the containers (the bone meal a composter
    // pops out lands beside it, often outside the field).
    let spots: Vec<nav::Pos> = storages(game)
        .into_iter()
        .flatten()
        .chain(farm_composter_in(game, &areas))
        .collect();
    let near = |d: &horizon_client::entities::ItemEntity| {
        let a = at(d);
        let by_spot = spots
            .iter()
            .any(|p| nav::distance(nav::center(*p), a) <= 4.0);
        // Only what lies at the field's levels: a drop on a roof would lead the walker up.
        by_spot || areas.iter().any(|f| f.near(a, 3.0, 1.0))
    };
    let nearest = game
        .dropped
        .iter()
        .filter(|d| d.stack.count > 0 && !s.ignored_items.contains(&d.id) && near(d))
        .min_by(|a, b| nav::distance(at(a), here).total_cmp(&nav::distance(at(b), here)))
        .map(|d| (d.id, at(d)));
    let Some((id, item)) = nearest else {
        s.loot = None;
        s.ignored_items.clear();
        return false;
    };
    let now = Instant::now();
    let close = nav::distance(item, here) <= 1.1;
    if s.loot.as_ref().is_none_or(|l| l.id != id) {
        s.loot = Some(Loot {
            id,
            since: now,
            arrived: close.then_some(now),
            walking: false,
        });
        if !close {
            match driver::workshop_walk(game, nav::block(item), true, false) {
                Ok(()) => {
                    if let Some(l) = s.loot.as_mut() {
                        l.walking = true;
                    }
                }
                Err(_) => {
                    s.ignored_items.push(id);
                    s.loot = None;
                    return false;
                }
            }
        }
        return true;
    }
    let mut give_up = false;
    if let Some(loot) = s.loot.as_mut() {
        if loot.walking {
            match driver::workshop_walk_result() {
                Some(true) => {
                    loot.walking = false;
                    loot.arrived = Some(now);
                }
                Some(false) => give_up = true,
                None => {}
            }
        }
        if close && loot.arrived.is_none() {
            loot.arrived = Some(now);
        }
        // Standing on it, the pickup follows within a moment; a long walk is a lost cause.
        give_up |= loot
            .arrived
            .is_some_and(|t| t.elapsed() > Duration::from_millis(900))
            || loot.since.elapsed() > Duration::from_secs(10);
    }
    if give_up {
        s.ignored_items.push(id);
        s.loot = None;
        return false;
    }
    true
}
/// Takes the farm back to its beds when the player is far from them or the chunks they are in
/// are not loaded (from a distance the field reads as empty, and the job would wait for crops
/// that are not there). True while it is walking there.
fn home_step(game: &Game<'_>, s: &mut State) -> bool {
    let areas = match &s.task {
        Some(Task::Farm { areas, .. }) => areas.clone(),
        _ => return false,
    };
    let (Some(p), Some(area)) = (game.native_player(), nearest_area(game, &areas)) else {
        return false;
    };
    let origin = area_home(game, area);
    // Near in height too: a walker on a roof or a tree over the field is not at the beds (from
    // there no crop can be reached, and the scan would only find unreachable ones).
    let near = area.near([p.x, p.y, p.z], 6.0, 2.0);
    let loaded = game
        .world
        .chunks
        .contains_key(&(origin[0].div_euclid(16), origin[2].div_euclid(16)));
    if near && loaded {
        s.home = None;
        return false;
    }
    let message = text("возвращаюсь к грядкам", "walking back to the beds");
    match s.home {
        None => {
            match driver::workshop_walk(game, origin, false, false) {
                Ok(()) => {
                    s.home = Some(Instant::now());
                    s.message = message.into();
                    s.waiting = false;
                }
                Err(e) => idle(s, e),
            }
            true
        }
        Some(since) => match driver::workshop_walk_result() {
            Some(true) => {
                s.home = None;
                false
            }
            Some(false) => {
                // No way back for now: wait and ask again instead of pausing.
                s.home = None;
                idle(
                    s,
                    text(
                        "далеко от грядок, пути нет, жду и пробую снова",
                        "far from the beds, no route, waiting to try again",
                    ),
                );
                true
            }
            None if since.elapsed() > Duration::from_secs(90) => {
                s.home = None;
                true
            }
            None => true,
        },
    }
}
/// Whether a ray from the eye of someone standing at `feet` reaches the middle of `target`
/// through nothing but air (plants and crops count: they stop the aim ray).
fn clear_line(game: &Game<'_>, feet: nav::Pos, target: nav::Pos) -> bool {
    let eye = [
        f64::from(feet[0]) + 0.5,
        f64::from(feet[1]) + 1.62,
        f64::from(feet[2]) + 0.5,
    ];
    let goal = [
        f64::from(target[0]) + 0.5,
        f64::from(target[1]) + 0.5,
        f64::from(target[2]) + 0.5,
    ];
    let length = nav::distance(eye, goal);
    let steps = (length / 0.1).ceil() as i32;
    (1..steps).all(|i| {
        let t = f64::from(i) / f64::from(steps);
        let at = [
            (eye[0] + (goal[0] - eye[0]) * t).floor() as i32,
            (eye[1] + (goal[1] - eye[1]) * t).floor() as i32,
            (eye[2] + (goal[2] - eye[2]) * t).floor() as i32,
        ];
        at == target
            || at == feet
            || matches!(
                game.native_block(at).as_str(),
                "minecraft:air" | "minecraft:cave_air"
            )
    })
}
/// Leaves a drain cell for a while (no route to it, or it keeps failing) and goes on.
/// How long a cell that just received a falling block is left alone: about the time the block
/// needs to sink out of it. A line is laid at a few blocks a second, as when bridging by hand.
const SINK_MS: u64 = 450;
fn give_up_cell(s: &mut State, target: nav::Pos) {
    s.errors += 1;
    s.missed += 1;
    if s.errors >= 10 {
        // Not a reason to stop: wait a little, then try the cells again.
        s.errors = 0;
        s.waiting = true;
        s.message = text(
            "нет пути к клеткам узора, жду и пробую снова",
            "no route to the cells of the pattern, waiting to try again",
        )
        .into();
        s.last = Instant::now() + Duration::from_secs(3);
    }
    s.skip
        .push((target, Instant::now() + Duration::from_secs(6)));
    s.target = None;
    s.column = None;
    s.attempt = 0;
    s.phase = Phase::Ready;
}
/// How to reach a spot to place `target` from: `Some(None)` when already standing on it,
/// `Some(Some(route))` along solid cells at one height, `None` when there is no such route.
fn pier_plan(
    game: &Game<'_>,
    target: nav::Pos,
    position: [f64; 3],
) -> Option<Option<Vec<nav::Pos>>> {
    let feet = [
        position[0].floor() as i32,
        (position[1] + 0.05).floor() as i32,
        position[2].floor() as i32,
    ];
    let cell = |at: nav::Pos| placement::block(game, at).cell;
    let walkable = |at: nav::Pos| {
        cell(at) == nav::Cell::Air
            && cell([at[0], at[1] + 1, at[2]]) == nav::Cell::Air
            && placement::stable(game, [at[0], at[1] - 1, at[2]])
    };
    let mut best: Option<Vec<nav::Pos>> = None;
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let neighbour = [target[0] + dx, target[1], target[2] + dz];
        let spot = [neighbour[0], neighbour[1] + 1, neighbour[2]];
        if !placement::stable(game, neighbour) || !walkable(spot) {
            continue;
        }
        if feet == spot {
            return Some(None);
        }
        if let Some(path) = drain::flat_path(feet, spot, 2000, walkable)
            && best.as_ref().is_none_or(|b| path.len() < b.len())
        {
            best = Some(path);
        }
    }
    best.map(Some)
}
/// The next waypoint of the route along the laid blocks, for the movement hook; the route ends
/// (and the job goes on) once the last one is reached.
pub(super) fn pier_goal(position: [f64; 3]) -> Option<[f64; 3]> {
    state(|s| {
        let pier = s.pier.as_mut()?;
        while let Some(at) = pier.path.get(pier.next) {
            let c = nav::center(*at);
            if (position[0] - c[0]).hypot(position[2] - c[2]) < 0.3 {
                pier.next += 1;
            } else {
                return Some(c);
            }
        }
        s.pier = None;
        None
    })
}
/// Forgets the route (the player left the blocks: a fall into the water).
pub(super) fn clear_pier() {
    state(|s| s.pier = None);
}
/// Whether the place to stand at `at` is on top of a container (a block with a block entity).
pub(super) fn on_container(game: &Game<'_>, at: nav::Pos) -> bool {
    game.res
        .facts()
        .get(game.world.block(at[0], at[1] - 1, at[2]))
        .block_entity
}
fn approach(game: &Game<'_>, s: &mut State, target: nav::Pos) -> bool {
    let Some(p) = game.native_player() else {
        return false;
    };
    let level = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    )[1];
    let mut candidates = Vec::new();
    for y in [level, target[1], target[1] - 1, target[1] + 1] {
        for dz in -3..=3_i32 {
            for dx in -3..=3_i32 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let at = [target[0] + dx, y, target[2] + dz];
                if nav::distance(nav::center(at), nav::center(target)) > 3.4 {
                    continue;
                }
                if !nav::standable(at, &mut |p| placement::block(game, p).cell) {
                    continue;
                }
                // Not on top of a chest, a barrel or the like: that is no place to work from.
                if on_container(game, at) {
                    continue;
                }
                candidates.push(at);
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    if s.store.is_some() {
        // To open the chest the ray from the eye has to reach it: no spot behind crops or a
        // corner. When none has a clear line, any spot will do.
        let clear: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|at| clear_line(game, *at, target))
            .collect();
        if !clear.is_empty() {
            candidates = clear;
        }
    }
    if matches!(s.task, Some(Task::Farm { .. })) {
        // Seeds go onto the farmland next to the player's feet. Standing right beside the
        // target keeps the aim ray from meeting the neighbouring crops first.
        candidates.sort_by(|a, b| {
            nav::distance(nav::center(*a), nav::center(target))
                .total_cmp(&nav::distance(nav::center(*b), nav::center(target)))
                .then(
                    nav::distance(nav::center(*a), [p.x, p.y, p.z])
                        .total_cmp(&nav::distance(nav::center(*b), [p.x, p.y, p.z])),
                )
        });
    } else {
        candidates.sort_by(|a, b| {
            nav::distance(nav::center(*a), [p.x, p.y, p.z])
                .total_cmp(&nav::distance(nav::center(*b), [p.x, p.y, p.z]))
        });
    }
    candidates.truncate(16);
    let farm = matches!(s.task, Some(Task::Farm { .. }));
    let count = candidates.len();
    // A farm first walks without breaking or placing blocks (it may jump: a raised bed has to
    // be climbed, and jumping up onto farmland is harmless). When no spot can be reached that
    // way, a second pass over the same spots may break and place blocks to get there.
    let pick = if s.attempt < count {
        candidates.get(s.attempt).map(|at| (*at, farm))
    } else if farm && s.attempt < 2 * count {
        candidates.get(s.attempt - count).map(|at| (*at, false))
    } else {
        None
    };
    if let Some((at, flat)) = pick {
        s.attempt += 1;
        match driver::workshop_walk(game, at, flat, false) {
            Ok(()) => {
                s.phase = Phase::Walking;
                true
            }
            Err(e) => {
                idle(s, e);
                false
            }
        }
    } else if farm && s.store.is_some() {
        // No way to the storage: carry on with the crops and ask again in a minute.
        s.store = None;
        s.target = None;
        s.attempt = 0;
        s.phase = Phase::Ready;
        s.store_after = Instant::now() + Duration::from_secs(60);
        s.message = text(
            "нет пути к складу, продолжаю с грядками",
            "no route to the storage, carrying on with the crops",
        )
        .into();
        s.waiting = true;
        false
    } else if farm {
        // No way to this bed for now (a long way round, a gap): wait and look again instead
        // of pausing the whole job.
        s.target = None;
        s.attempt = 0;
        s.phase = Phase::Ready;
        s.last = Instant::now() + Duration::from_secs(4);
        s.message = text(
            "нет пути к грядке, ищу снова",
            "no route to the bed, looking again",
        )
        .into();
        s.waiting = true;
        false
    } else if matches!(s.task, Some(Task::Drain { .. })) && s.errors < 10 {
        // No spot reaches this cell: leave it for a while and fill the others.
        s.errors += 1;
        s.missed += 1;
        s.skip
            .push((target, Instant::now() + Duration::from_secs(6)));
        s.target = None;
        s.column = None;
        s.attempt = 0;
        s.phase = Phase::Ready;
        false
    } else {
        // No spot to place or harvest from can be reached right now: wait and look again.
        let why = placement::diagnostic();
        let message = text(
            "нет доступной грани, жду и ищу снова",
            "no reachable placement face, waiting to look again",
        );
        idle(
            s,
            if why.is_empty() {
                message.to_owned()
            } else {
                format!("{message} ({why})")
            },
        );
        false
    }
}
pub(super) fn tick(game: &Game<'_>) {
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    state(|s| {
        if s.scope.is_empty() {
            return;
        }
        if driver::scope(game).as_deref() != Some(s.scope.as_str()) {
            *s = State::default();
            return;
        }
        let answer = s.pending.as_ref().and_then(|p| p.rx.try_recv().ok());
        if let Some(answer) = answer {
            let pending = s.pending.take().unwrap();
            match answer {
                Ok(files::Response::Files(names)) => s.files = names,
                Ok(files::Response::Loaded(document)) => {
                    let ignored = document.ignored_data;
                    s.task = Some(Task::Build {
                        document,
                        cursor: 0,
                    });
                    s.active = pending.run && game.native_playing();
                    s.preview = !s.active;
                    s.message = format!("{}: {ignored}", text("Пропущено NBT", "NBT skipped"));
                }
                Err(e) => fail(s, e),
            }
        }
        if !s.active || s.task.is_none() {
            return;
        }
        if !driver::in_world(game) {
            // The storage window is open: store_frame drives it, the job just waits.
            if s.store.is_some() && game.screen == Screen::Inventory {
                return;
            }
            s.active = false;
            return;
        }
        // The inventory, the chat or one of the mod's screens is open: no click can be made, so
        // the job waits for it to close instead of pausing. (Walking is not affected.)
        if !game.native_playing() {
            return;
        }
        // A job that does nothing for a long time without saying why (a cell it cannot place, a
        // route it cannot walk, a wait that never ends) is shaken loose: the cell it was on is
        // left for a while and it looks for another.
        if !s.waiting && s.store.is_none() && s.loot.is_none() && s.pier.is_none() {
            let done = s.done;
            if done != s.progress.0 {
                s.progress = (done, Instant::now());
            } else if s.progress.1.elapsed() > Duration::from_secs(20) {
                s.progress.1 = Instant::now();
                if let Some(at) = s.target {
                    s.skip.push((at, Instant::now() + Duration::from_secs(10)));
                }
                s.target = None;
                s.column = None;
                s.pier = None;
                s.attempt = 0;
                s.phase = Phase::Ready;
                s.message = text(
                    "застрял, ищу другую цель",
                    "got stuck, looking for another target",
                )
                .into();
                driver::clear_construction_driver();
                placement::reset();
            }
        }
        // Fallen into the water: the movement hook swims back to dry ground, no new cell until then.
        if matches!(s.task, Some(Task::Drain { .. }))
            && game
                .player
                .as_ref()
                .is_some_and(|p| p.fluids.was_touching_water)
        {
            return;
        }
        match s.phase {
            Phase::Done => {
                s.active = false;
                return;
            }
            Phase::Pier => {
                // Walking along the laid blocks (the movement hook steers); wait for it.
                let stale = s
                    .pier
                    .as_ref()
                    .is_none_or(|p| p.since.elapsed() > Duration::from_secs(30));
                if !stale {
                    return;
                }
                if s.pier.take().is_some()
                    && let Some(target) = s.target
                {
                    give_up_cell(s, target);
                }
                s.phase = Phase::Ready;
            }
            Phase::Walking => match driver::workshop_walk_result() {
                None => return,
                Some(false) => {
                    let target = s.target.unwrap();
                    approach(game, s, target);
                    return;
                }
                Some(true) => s.phase = Phase::Ready,
            },
            Phase::Digging => match driver::farm_result() {
                None => return,
                Some(Err(schema::PlaceError::Unreachable)) => {
                    driver::clear_construction_driver();
                    approach(game, s, s.target.unwrap());
                    return;
                }
                Some(Err(e)) => {
                    // This crop would not come: leave it for a while and go on.
                    let _ = e;
                    if let Some(at) = s.target {
                        s.skip.push((at, Instant::now() + Duration::from_secs(8)));
                    }
                    driver::clear_construction_driver();
                    s.target = None;
                    s.phase = Phase::Ready;
                    return;
                }
                Some(Ok(())) => {
                    s.done = s.done.wrapping_add(1);
                    s.waiting = false;
                    if let Some(Task::Farm {
                        harvested, config, ..
                    }) = s.task.as_mut()
                    {
                        *harvested += 1;
                        s.plant = config.replant;
                    }
                    driver::clear_construction_driver();
                    s.phase = Phase::Ready;
                    if !s.plant {
                        s.target = None;
                    }
                }
            },
            Phase::Placing => {
                let Some(result) = placement::external_result() else {
                    return;
                };
                if !placement::external_settled() {
                    return;
                }
                match result {
                    Ok(_) => {
                        s.done = s.done.wrapping_add(1);
                        s.waiting = false;
                        if let Some(Task::Drain { filled, .. }) = s.task.as_mut() {
                            *filled += 1;
                            s.wait = s.settle;
                            s.errors = 0;
                            // A dropped block is still sinking in this column: aim elsewhere.
                            if s.falling
                                && let Some(at) = s.target
                            {
                                s.skip
                                    .push((at, Instant::now() + Duration::from_millis(SINK_MS)));
                            }
                        }
                        s.target = None;
                        s.plant = false;
                        s.attempt = 0;
                        s.phase = Phase::Ready;
                    }
                    Err(schema::PlaceError::Unreachable) => {
                        let target = s.target.unwrap();
                        approach(game, s, target);
                        return;
                    }
                    Err(e)
                        if matches!(s.task, Some(Task::Drain { .. }))
                            && !matches!(
                                e,
                                schema::PlaceError::Material | schema::PlaceError::Manual
                            )
                            && s.errors < 10 =>
                    {
                        // One stubborn cell must not stop the whole job: leave it for later.
                        s.errors += 1;
                        s.missed += 1;
                        if let Some(at) = s.target {
                            s.skip.push((at, Instant::now() + Duration::from_secs(5)));
                        }
                        s.target = None;
                        s.column = None;
                        s.attempt = 0;
                        s.phase = Phase::Ready;
                        return;
                    }
                    Err(e) => {
                        // A refused or late placement is no reason to stop: leave that cell for
                        // a while (a farm) or wait and place it again (a schematic).
                        let message = format!("{}: {}", text("установка", "placement"), reason(e));
                        if let (Some(Task::Farm { .. }), Some(at)) = (&s.task, s.target) {
                            s.skip.push((at, Instant::now() + Duration::from_secs(8)));
                        }
                        idle(s, message);
                        return;
                    }
                }
            }
            Phase::Growing => {
                let Some(target) = s.target else {
                    s.grow = false;
                    s.phase = Phase::Ready;
                    return;
                };
                if !s.grow_sent {
                    match placement::use_result() {
                        None => {
                            // The bone meal is being put in hand, or the click waits for a
                            // line of sight; not for ever.
                            if s.grow_at.elapsed() > Duration::from_secs(8) {
                                grow_give_up(s, target);
                            }
                        }
                        Some(false) => grow_give_up(s, target),
                        Some(true) => {
                            s.grow_sent = true;
                            s.grow_at = Instant::now();
                        }
                    }
                    return;
                }
                if s.grow_at.elapsed() < Duration::from_millis(450) {
                    return;
                }
                let Some(crop) = s.crop else {
                    grow_give_up(s, target);
                    return;
                };
                let grown = game.native_block(target) != crop.block()
                    || crop.mature(game.native_block_property(target, "age").as_deref());
                let meal = store::count(&inventory(game), store::BONE_MEAL);
                if grown {
                    s.done = s.done.wrapping_add(1);
                    s.grow = false;
                    s.target = None;
                    s.phase = Phase::Ready;
                } else if meal < s.grow_meal && s.grow_clicks < 8 {
                    // The meal was used and the crop is still growing: another dose.
                    s.grow_clicks += 1;
                    s.grow_meal = meal;
                    s.grow_sent = false;
                    s.grow_at = Instant::now();
                    placement::use_item_on(store::BONE_MEAL, target);
                } else {
                    // The click did nothing, or the crop will not take more: leave it a while.
                    grow_give_up(s, target);
                }
            }
            Phase::Tilling => {
                let Some(target) = s.target else {
                    s.till = false;
                    s.phase = Phase::Ready;
                    return;
                };
                if !s.grow_sent {
                    match placement::use_result() {
                        None if s.grow_at.elapsed() > Duration::from_secs(8) => {
                            till_give_up(s, target);
                        }
                        None => {}
                        Some(false) => till_give_up(s, target),
                        Some(true) => {
                            s.grow_sent = true;
                            s.grow_at = Instant::now();
                        }
                    }
                    return;
                }
                if s.grow_at.elapsed() < Duration::from_millis(450) {
                    return;
                }
                if game.native_block(target) == "minecraft:farmland" {
                    s.till = false;
                    s.target = None;
                    s.phase = Phase::Ready;
                } else {
                    till_give_up(s, target);
                }
            }
            Phase::Ready => {}
        }
        if !placement::external_settled() {
            return;
        }
        // Idling after a problem: nothing new until the wait is over.
        if s.last > Instant::now() && s.target.is_none() {
            return;
        }
        // Between two crops: first pick up what the last one dropped.
        if s.target.is_none() && s.store.is_none() && loot_step(game, s) {
            return;
        }
        // Then unload the harvest or fetch seeds when the storage is set.
        if s.target.is_none()
            && s.store.is_none()
            && let Some((purpose, chest)) = store_due(game, s)
        {
            s.store = Some(Store {
                purpose,
                step: Step::Walk,
                since: Instant::now(),
                last: Instant::now(),
                tries: 0,
                force_move: false,
                moved: 0,
                stuck: 0,
                sig: usize::MAX,
            });
            s.target = Some(chest);
            s.plant = false;
            s.attempt = 0;
            s.idle_deposit = false;
        }
        if s.store.is_some() {
            store_walk(game, s);
            return;
        }
        // Far from the beds, or their chunks are not loaded: go back before looking for crops.
        if s.target.is_none() && home_step(game, s) {
            return;
        }
        if s.target.is_none() {
            match s.task.as_mut().unwrap() {
                Task::Farm {
                    config,
                    areas,
                    marked,
                    ..
                } => {
                    // No limit: the farm works until it is stopped.
                    // The field is looked at again a fifth of a second after the last look: the
                    // next crop is chosen right after one is done (it waited up to half a second).
                    if s.last.elapsed() < Duration::from_millis(200) {
                        return;
                    }
                    s.last = Instant::now();
                    // Marked plots: the ones turned on now (the player may change them while
                    // the farm runs).
                    if *marked {
                        *areas = crate::game::claim_borders_farm_plots_view::enabled(game);
                        if areas.is_empty() {
                            idle(
                                s,
                                text(
                                    "нет включённых участков в этом мире",
                                    "no enabled plots in this world",
                                ),
                            );
                            return;
                        }
                    }
                    // Every cell of a plot the player marked is field: plain soil in it is tilled
                    // (a new plot has no farmland yet to go by). The square around the start
                    // tills only soil that was farmland or lies in a bed, not the paths beside.
                    let plot_field = *marked;
                    let areas = areas.clone();
                    let mut seen = std::collections::HashSet::new();
                    let mut targets = Vec::new();
                    // Planted crops that have not ripened yet: bone meal goes on these.
                    let mut growing_at: Vec<(nav::Pos, farm::Crop)> = Vec::new();
                    let mut dark = 0;
                    let mut unplanted = false;
                    let mut trampled: Vec<nav::Pos> = Vec::new();
                    let crops: Vec<_> = config.selected().collect();
                    // What is sown where: the field is shared between the selected crops (a
                    // quarter each for four), each part gets its own crop and only its own.
                    // Without seeds for a part they are fetched; with none in the container
                    // either the part waits for them (another crop no longer stands in: the
                    // field ended up sown with whatever seeds were at hand, not by its parts).
                    let have: Vec<farm::Crop> =
                        crops.iter().copied().filter(|c| seed(game, *c)).collect();
                    let plan = *config;
                    let sow_at = |area: &plots::Bounds, x: i32, z: i32| {
                        let zone = plan
                            .zone_crop_in(
                                [area.lo[0], area.lo[2]],
                                [area.hi[0], area.hi[2]],
                                [x, z],
                            )
                            .filter(|_| plan.replant)?;
                        have.contains(&zone).then_some(zone)
                    };
                    for area in &areas {
                        for z in area.lo[2]..=area.hi[2] {
                            for x in area.lo[0]..=area.hi[0] {
                                // Beds at other heights count too: a block of unevenness on a flat
                                // field, the chosen number of levels on a terraced one, every level
                                // of a marked plot.
                                for y in area.lo[1]..=area.hi[1] {
                                    let at = [x, y, z];
                                    // Overlapping plots: a cell is looked at once.
                                    if areas.len() > 1 && !seen.insert(at) {
                                        continue;
                                    }
                                    let below = [at[0], at[1] - 1, at[2]];
                                    let soil = game.native_block(below);
                                    if soil != "minecraft:farmland" {
                                        // Farmland of this field trampled to dirt (a jump, a
                                        // fall, a mob): tilled again when a hoe is at hand.
                                        // Also one trampled before the job began: plain dirt in
                                        // a bed, next to farmland on two sides or more.
                                        let in_bed = || {
                                            [[1, 0], [-1, 0], [0, 1], [0, -1]]
                                                .iter()
                                                .filter(|[dx, dz]| {
                                                    game.native_block([
                                                        below[0] + dx,
                                                        below[1],
                                                        below[2] + dz,
                                                    ]) == "minecraft:farmland"
                                                })
                                                .count()
                                                >= 2
                                        };
                                        if tillable(&soil)
                                            && game.native_block(at) == "minecraft:air"
                                            && (plot_field
                                                || (soil == "minecraft:dirt"
                                                    && (s.field.contains(&below) || in_bed())))
                                        {
                                            trampled.push(below);
                                        }
                                        continue;
                                    }
                                    if s.field.len() < 8192 {
                                        s.field.insert(below);
                                    }
                                    let name = game.native_block(at);
                                    let ripe = crops.iter().copied().find(|c| {
                                        name == c.block()
                                            && c.mature(
                                                game.native_block_property(at, "age").as_deref(),
                                            )
                                    });
                                    let mature = ripe.is_some();
                                    if ripe.is_none()
                                        && let Some(c) =
                                            crops.iter().copied().find(|c| name == c.block())
                                    {
                                        growing_at.push((at, c));
                                    }
                                    // A crop only survives where the light is 9 or more (sky or
                                    // block light): in a cave the server refuses the seed.
                                    let light = i32::from(game.world.light(at[0], at[1], at[2]));
                                    let lit = (light >> 4).max(light & 15) >= 9;
                                    let sow = sow_at(area, x, z);
                                    let empty = config.replant && name == "minecraft:air";
                                    if empty && !lit {
                                        dark += 1;
                                    }
                                    if empty && lit && sow.is_none() {
                                        unplanted = true;
                                    }
                                    let empty = empty && lit && sow.is_some();
                                    if mature || empty {
                                        targets.push((at, empty, ripe.or(sow)));
                                    }
                                }
                            }
                        }
                    }
                    let p = game.native_player().unwrap();
                    targets.sort_by(|a, b| {
                        nav::distance(nav::center(a.0), [p.x, p.y, p.z])
                            .total_cmp(&nav::distance(nav::center(b.0), [p.x, p.y, p.z]))
                    });
                    s.unplanted = unplanted;
                    s.growing = !growing_at.is_empty();
                    // Bone meal at hand, and the cells it was not just given up on for.
                    let now = Instant::now();
                    s.skip.retain(|(_, until)| *until > now);
                    let meal = if config.bonemeal {
                        store::count(&inventory(game), store::BONE_MEAL)
                    } else {
                        0
                    };
                    growing_at.retain(|(at, _)| !s.skip.iter().any(|(p, _)| p == at));
                    growing_at.sort_by(|a, b| {
                        nav::distance(nav::center(a.0), [p.x, p.y, p.z])
                            .total_cmp(&nav::distance(nav::center(b.0), [p.x, p.y, p.z]))
                    });
                    // Trampled soil first (a quick click, and the bed is sown on the next look);
                    // it waited behind every ripe crop before and was never reached.
                    let till = trampled
                        .iter()
                        .filter(|at| !s.skip.iter().any(|(p, _)| p == *at))
                        .min_by(|a, b| {
                            nav::distance(nav::center(**a), [p.x, p.y, p.z])
                                .total_cmp(&nav::distance(nav::center(**b), [p.x, p.y, p.z]))
                        })
                        .copied()
                        .filter(|_| hoe(game).is_some());
                    if let Some(at) = till {
                        s.target = Some(at);
                        s.plant = false;
                        s.crop = crops.first().copied();
                        s.grow = false;
                        s.till = true;
                        s.waiting = false;
                        s.attempt = 0;
                        s.idle_since = None;
                        s.parked = false;
                        s.park = None;
                    } else if let Some((at, plant, crop)) = targets.first() {
                        s.target = Some(*at);
                        s.plant = *plant;
                        s.crop = *crop;
                        s.grow = false;
                        s.waiting = false;
                        s.attempt = 0;
                        s.idle_since = None;
                        s.parked = false;
                        s.park = None;
                    } else if let Some(at) = trampled
                        .iter()
                        .filter(|at| !s.skip.iter().any(|(p, _)| p == *at))
                        .min_by(|a, b| {
                            nav::distance(nav::center(**a), [p.x, p.y, p.z])
                                .total_cmp(&nav::distance(nav::center(**b), [p.x, p.y, p.z]))
                        })
                        .filter(|_| hoe(game).is_some())
                    {
                        // Trampled soil: a hoe makes it farmland again, sown on the next look.
                        s.target = Some(*at);
                        s.plant = false;
                        s.crop = crops.first().copied();
                        s.grow = false;
                        s.till = true;
                        s.waiting = false;
                        s.attempt = 0;
                        s.idle_since = None;
                        s.parked = false;
                        s.park = None;
                    } else if meal > 0
                        && let Some((at, crop)) = growing_at.first()
                    {
                        // Nothing to harvest or sow: feed the crops that are still growing.
                        s.target = Some(*at);
                        s.plant = false;
                        s.crop = Some(*crop);
                        s.grow = true;
                        s.waiting = false;
                        s.attempt = 0;
                        s.idle_since = None;
                        s.parked = false;
                        s.park = None;
                    } else if unplanted {
                        // Nothing ripe and no seeds anywhere: idle, not paused. The job goes
                        // on by itself when something ripens or seeds turn up.
                        s.message = text(
                            "нет семян, жду созревания",
                            "no seeds, waiting for crops to ripen",
                        )
                        .into();
                        s.waiting = true;
                        s.idle_deposit = true;
                        park_step(game, s);
                        return;
                    } else if dark > 0 {
                        // Waits for a torch: the scan repeats, so the job goes on by itself.
                        s.message = text(
                            "нет света на грядках: для посева нужен источник света (уровень 9+)",
                            "no light on the farmland: crops need a light source (level 9+)",
                        )
                        .into();
                        s.waiting = true;
                        park_step(game, s);
                        return;
                    } else {
                        s.message = text("ожидание созревания", "waiting for ripe crops").into();
                        s.waiting = true;
                        // Nothing to harvest for now: a good time to unload.
                        s.idle_deposit = true;
                        park_step(game, s);
                        return;
                    }
                }
                Task::Drain {
                    config,
                    origin,
                    filled,
                } => {
                    if s.last.elapsed() < s.wait {
                        return;
                    }
                    s.last = Instant::now();
                    s.wait = Duration::from_millis(60);
                    let now = Instant::now();
                    s.skip.retain(|(_, until)| *until > now);
                    let Some(p) = game.native_player() else {
                        return;
                    };
                    if *filled >= 4000 {
                        s.phase = Phase::Done;
                        s.active = false;
                        s.message =
                            text("Достигнут предел операций", "Operation limit reached").into();
                        return;
                    }
                    // A pattern is laid on the surface, one block per cell.
                    let pattern = config.pattern != drain::Pattern::Fill;
                    let falling = placement::drain_item(game).is_some_and(|(_, falling)| falling);
                    let surface = falling || pattern;
                    s.falling = falling;
                    s.settle = Duration::from_millis(if falling { 70 } else { 80 });
                    let held = |at: nav::Pos| s.skip.iter().any(|(p, _)| *p == at);
                    let eye = [p.x, p.y + 1.2, p.z];
                    let r = i32::from(config.radius);
                    let body = p.y.floor() as i32;
                    // Only a block that will stay counts as something to click on or stand on.
                    let solid = |at: nav::Pos| placement::stable(game, at);
                    let mut cells = Vec::new();
                    for z in -r..=r {
                        for x in -r..=r {
                            let (cx, cz) = (origin[0] + x, origin[2] + z);
                            if !drain::in_pattern(*config, *origin, [cx, cz]) {
                                continue;
                            }
                            // The player's own body occupies these cells.
                            let own = |y: i32| {
                                (p.x - (f64::from(cx) + 0.5)).abs() < 0.8
                                    && (p.z - (f64::from(cz) + 0.5)).abs() < 0.8
                                    && (body..=body + 1).contains(&y)
                            };
                            if surface {
                                // Only the surface cell of a column matters: a dropped block
                                // sinks to the bottom and the column fills from below. The cell
                                // needs a solid neighbour to click on.
                                for y in (origin[1] - 4..=origin[1] + 1).rev() {
                                    let name = game.native_block([cx, y, cz]);
                                    if matches!(
                                        name.as_str(),
                                        "minecraft:air" | "minecraft:cave_air"
                                    ) {
                                        continue;
                                    }
                                    if drain::replaceable(&name) {
                                        placement::forget([cx, y, cz]);
                                    }
                                    if drain::replaceable(&name) && !own(y) && !held([cx, y, cz]) {
                                        let anchored = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                                            .iter()
                                            .any(|[dx, dz]| solid([cx + dx, y, cz + dz]))
                                            || solid([cx, y - 1, cz]);
                                        cells.push(drain::Candidate {
                                            at: [cx, y, cz],
                                            supported: anchored,
                                        });
                                    }
                                    break;
                                }
                                continue;
                            }
                            let mut in_water = false;
                            for y in (origin[1] - drain::DEPTH..=origin[1]).rev() {
                                let name = game.native_block([cx, y, cz]);
                                if drain::replaceable(&name) {
                                    in_water = true;
                                    if own(y) || held([cx, y, cz]) {
                                        continue;
                                    }
                                    cells.push(drain::Candidate {
                                        at: [cx, y, cz],
                                        supported: solid([cx, y - 1, cz]),
                                    });
                                } else if in_water
                                    || !matches!(
                                        name.as_str(),
                                        "minecraft:air" | "minecraft:cave_air"
                                    )
                                {
                                    break;
                                }
                            }
                        }
                    }
                    let mut picked = if falling {
                        drain::pick_nearest(&cells, eye)
                    } else {
                        drain::pick(&cells, eye)
                    };
                    if surface {
                        // Finish the column in progress before going anywhere else.
                        if let Some((sx, sz)) = s.column {
                            let top = (origin[1] - 4..=origin[1] + 1).rev().find_map(|y| {
                                let name = game.native_block([sx, y, sz]);
                                if matches!(name.as_str(), "minecraft:air" | "minecraft:cave_air") {
                                    None
                                } else {
                                    Some((y, drain::replaceable(&name)))
                                }
                            });
                            // How long the cell is still held: a block that just sank (under a
                            // second) is waited for, a cell given up on (seconds) is left.
                            let hold = |y: i32| {
                                s.skip
                                    .iter()
                                    .find(|(p, _)| *p == [sx, y, sz])
                                    .map(|(_, until)| until.saturating_duration_since(now))
                            };
                            match top {
                                // Still water on top: this block is sinking, drop the next
                                // one when it has, right here.
                                Some((y, true))
                                    if hold(y)
                                        .is_some_and(|d| d <= Duration::from_millis(SINK_MS)) =>
                                {
                                    s.wait = Duration::from_millis(50);
                                    return;
                                }
                                Some((y, true)) if hold(y).is_none() => {
                                    picked = Some(([sx, y, sz], true))
                                }
                                // Solid on top, or a cell given up on: take the nearest next one.
                                _ => s.column = None,
                            }
                        }
                        if let Some((at, _)) = picked {
                            s.column = Some((at[0], at[2]));
                        }
                    }
                    match picked {
                        Some((at, _)) => {
                            s.target = Some(at);
                            s.attempt = 0;
                            s.idle = 0;
                        }
                        None => {
                            // A block that just sank leaves the cell empty for a moment, so
                            // finish only after several empty scans in a row.
                            s.idle += 1;
                            if s.idle < 4 {
                                s.wait = Duration::from_millis(1000);
                                return;
                            }
                            if !cells.is_empty() {
                                // Water is left but no cell has a firm block beside it to click:
                                // not done. Wait, look again (the player may move, a block may
                                // settle) and keep saying why.
                                s.idle = 0;
                                s.waiting = true;
                                s.wait = Duration::from_millis(2500);
                                s.message = format!(
                                    "{}: {}",
                                    text(
                                        "нет опоры для клика у края, жду (встаньте на блок у воды)",
                                        "no firm block to click on at the edge, waiting (stand on a block by the water)"
                                    ),
                                    cells.len()
                                );
                                return;
                            }
                            s.phase = Phase::Done;
                            s.active = false;
                            s.message = if cells.is_empty() && s.missed > 0 {
                                format!(
                                    "{}: {filled} · {}: {}",
                                    text("Готово, засыпано", "Done, filled"),
                                    text("пропущено", "skipped"),
                                    s.missed
                                )
                            } else if cells.is_empty() {
                                format!("{}: {filled}", text("Готово, засыпано", "Done, filled"))
                            } else if falling {
                                format!(
                                    "{}: {}",
                                    text(
                                        "Нет опоры для клика у края: встаньте на блок у воды",
                                        "Nothing to click on at the edge: stand on a block by the water"
                                    ),
                                    cells.len()
                                )
                            } else {
                                format!(
                                    "{}: {}",
                                    text(
                                        "Дно вне досягаемости или под вами; для глубокой воды нужен песок или гравий",
                                        "Floor out of reach or under you; deep water needs sand or gravel"
                                    ),
                                    cells.len()
                                )
                            };
                            return;
                        }
                    }
                }
                Task::Build { document, cursor } => {
                    for _ in 0..64 {
                        if *cursor == document.blocks.len() {
                            s.phase = Phase::Done;
                            s.active = false;
                            s.message = text("Готово", "Done").into();
                            crate::game::claim_borders_resume::forget_build(game);
                            return;
                        }
                        let (at, wanted) = &document.blocks[*cursor];
                        if matches(game, *at, wanted) {
                            *cursor += 1;
                        } else {
                            s.target = Some(*at);
                            s.attempt = 0;
                            break;
                        }
                    }
                    if s.target.is_none() {
                        return;
                    }
                }
            }
        }
        let target = s.target.unwrap();
        let Some(p) = game.native_player() else {
            return;
        };
        let pattern_drain = matches!(
            &s.task,
            Some(Task::Drain { config, .. }) if config.pattern != drain::Pattern::Fill
        );
        if pattern_drain {
            // A pattern grows along its own blocks: stand on the one next to the cell, which
            // may mean walking out along the line laid so far.
            match pier_plan(game, target, [p.x, p.y, p.z]) {
                Some(None) => {}
                Some(Some(path)) => {
                    s.pier = Some(Pier {
                        path,
                        next: 0,
                        since: Instant::now(),
                    });
                    s.phase = Phase::Pier;
                    return;
                }
                None => {
                    give_up_cell(s, target);
                    return;
                }
            }
        } else if nav::distance(nav::center(target), [p.x, p.y + 1.2, p.z]) > 3.7 {
            approach(game, s, target);
            return;
        }
        let request = match s.task.as_ref().unwrap() {
            Task::Farm { .. } => {
                let Some(crop) = s.crop else {
                    s.target = None;
                    return;
                };
                if s.till {
                    till_click(game, s, target);
                    return;
                }
                if s.grow {
                    grow_click(game, s, target, crop);
                    return;
                }
                // The seeds ran out while walking here: look again (and fetch or idle).
                if s.plant && !seed(game, crop) {
                    s.target = None;
                    return;
                }
                if !s.plant {
                    if game.native_block(target) != crop.block()
                        || !crop.mature(game.native_block_property(target, "age").as_deref())
                    {
                        s.target = None;
                        return;
                    }
                    match driver::farm_dig(game, target, crop) {
                        Ok(()) => s.phase = Phase::Digging,
                        Err(e) => idle(s, e),
                    }
                    return;
                }
                if game.native_block([target[0], target[1] - 1, target[2]]) != "minecraft:farmland"
                {
                    // The bed changed under it: look at the field again.
                    s.target = None;
                    return;
                }
                let wanted = schema::BlockState::parse(crop.block()).unwrap();
                schema::PlacementRequest {
                    target,
                    item: crop.seed().into(),
                    state: wanted.clone(),
                    allowed: BTreeMap::from([(target, wanted)]),
                }
            }
            Task::Drain { .. } => {
                if !drain::replaceable(&game.native_block(target)) {
                    s.target = None;
                    return;
                }
                let Some((item, _)) = placement::drain_item(game) else {
                    idle(
                        s,
                        text(
                            "нужны песок или гравий (хотбар, 2-я рука или инвентарь)",
                            "sand or gravel needed (hotbar, off hand or inventory)",
                        ),
                    );
                    return;
                };
                let Ok(wanted) = schema::BlockState::parse(&item) else {
                    fail(s, text("Блок не поддерживается", "Unsupported block"));
                    return;
                };
                schema::PlacementRequest {
                    target,
                    item,
                    state: wanted.clone(),
                    allowed: BTreeMap::from([(target, wanted)]),
                }
            }
            Task::Build { document, cursor } => {
                let wanted = document.blocks[*cursor].1.clone();
                let item = match desired(game, &wanted) {
                    Ok(i) => i,
                    Err(e) => {
                        idle(s, e);
                        return;
                    }
                };
                let mut allowed = BTreeMap::new();
                for y in -1..=1 {
                    for z in -1..=1 {
                        for x in -1..=1 {
                            let p = [target[0] + x, target[1] + y, target[2] + z];
                            if let Ok(i) = document
                                .blocks
                                .binary_search_by_key(&(p[1], p[2], p[0]), |(p, _)| {
                                    (p[1], p[2], p[0])
                                })
                            {
                                allowed.insert(p, document.blocks[i].1.clone());
                            }
                        }
                    }
                }
                schema::PlacementRequest {
                    target,
                    item,
                    state: wanted,
                    allowed,
                }
            }
        };
        let current = game.native_block(target);
        let free = current == "minecraft:air"
            || current == "minecraft:cave_air"
            || (matches!(s.task, Some(Task::Drain { .. })) && drain::replaceable(&current));
        if !free {
            idle(
                s,
                format!(
                    "{}: {} {} {}",
                    text("Занято", "Occupied"),
                    target[0],
                    target[1],
                    target[2]
                ),
            );
            return;
        }
        driver::clear_construction_driver();
        match placement::place_external(game, request) {
            Ok(()) => s.phase = Phase::Placing,
            Err(e) => idle(s, e),
        }
    });
}
pub(super) fn snapshot() -> Option<(String, bool)> {
    // What the farm sold: the count from the hand, and the money from the server's sale
    // messages for the crops' yield since the job started (when the server sends them).
    let sales = state(|s| match &s.task {
        Some(Task::Farm { config, .. }) => {
            Some((s.sold, s.sold_since, config.selected().collect::<Vec<_>>()))
        }
        _ => None,
    });
    let sold = sales.map(|(sold, since, crops)| {
        let earned: econ::Coins = econ_data::read(|book, _, _, _| {
            book.trades
                .iter()
                .filter(|t| !t.buying && t.at >= since && store::yields(&crops, &t.item))
                .map(|t| t.net)
                .fold(0, econ::Coins::saturating_add)
        });
        match (sold, earned) {
            (0, 0) => String::new(),
            (n, 0) => format!(" · {} {n}", text("продано", "sold")),
            (n, money) => format!(" · {} {n} (+{})", text("продано", "sold"), econ::fmt(money)),
        }
    });
    state(|s| {
        let label = match s.task.as_ref() {
            Some(Task::Farm { harvested, .. }) => {
                format!(
                    "{}: {harvested}{}",
                    text("Ферма", "Farm"),
                    sold.as_deref().unwrap_or("")
                )
            }
            Some(Task::Drain { filled, .. }) => {
                format!("{}: {filled}", text("Осушение", "Drain"))
            }
            Some(Task::Build { document, cursor }) => {
                format!("{}: {cursor}/{}", document.format, document.blocks.len())
            }
            None if s.pending.is_some() || !s.message.is_empty() => {
                text("Схемы", "Schematics").into()
            }
            None => return None,
        };
        Some((
            format!(
                "{label} · {} · {}",
                if s.preview {
                    text("Просмотр", "Preview")
                } else if s.active && s.waiting {
                    text("Простой", "Idle")
                } else if s.active {
                    text("Работа", "Running")
                } else {
                    text("Пауза", "Paused")
                },
                s.message
            ),
            !s.active,
        ))
    })
}
pub(super) fn materials() -> String {
    state(|s| match &s.task {
        Some(Task::Build { document, .. }) => document
            .materials()
            .into_iter()
            .map(|(n, c)| format!("{n}: {c}"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => text("Загрузите схему", "Load a schematic").into(),
    })
}
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    // The schematic previews of this world, whether or not a build runs.
    crate::game::claim_borders_schematic_view::render(game, g);
    state(|s| {
        if driver::scope(game).as_deref() != Some(s.scope.as_str()) {
            return;
        }
        let Some(p) = game.native_player() else {
            return;
        };
        // The working area is an outline on the ground (the two corner cubes drawn before
        // looked like stray holograms); the block being worked on is the one cube.
        // Each field as its outline: (x0, x1, z0, z1, y) with x1 and z1 past the last cell.
        let mut outlines: Vec<(i32, i32, i32, i32, i32)> = Vec::new();
        let blocks = match &s.task {
            Some(Task::Farm { areas, .. }) => {
                // The middle level of each field (the crop level of a flat one).
                outlines.extend(
                    areas
                        .iter()
                        .map(|a| (a.lo[0], a.hi[0] + 1, a.lo[2], a.hi[2] + 1, a.center()[1])),
                );
                s.target.into_iter().collect()
            }
            Some(Task::Drain { origin, config, .. }) => {
                let r = i32::from(config.radius);
                outlines.push((
                    origin[0] - r,
                    origin[0] + r + 1,
                    origin[2] - r,
                    origin[2] + r + 1,
                    origin[1],
                ));
                s.target.into_iter().collect()
            }
            Some(Task::Build { document, cursor }) => document
                .blocks
                .iter()
                .skip(*cursor)
                .take(256)
                .map(|(p, _)| *p)
                .collect(),
            None => Vec::new(),
        };
        for (x0, x1, z0, z1, y) in outlines {
            let (x0, x1, z0, z1) = (f64::from(x0), f64::from(x1), f64::from(z0), f64::from(z1));
            let y = f64::from(y) + 0.05;
            for (a, b) in [
                ([x0, y, z0], [x1, y, z0]),
                ([x1, y, z0], [x1, y, z1]),
                ([x1, y, z1], [x0, y, z1]),
                ([x0, y, z1], [x0, y, z0]),
            ] {
                g.line(a, b, 0xff88ccaa, 1.5);
            }
        }
        if let Some(Task::Farm { areas, config, .. }) = &s.task {
            for area in areas {
                let (lo, hi) = ([area.lo[0], area.lo[2]], [area.hi[0], area.hi[2]]);
                let y = f64::from(area.center()[1]) + 0.05;
                // Where one crop's part of the field meets the next: the zones sown apart.
                let mut cuts_x = Vec::new();
                let mut cuts_z = Vec::new();
                for x in lo[0] + 1..=hi[0] {
                    if config.zone_crop_in(lo, hi, [x - 1, lo[1]])
                        != config.zone_crop_in(lo, hi, [x, lo[1]])
                    {
                        cuts_x.push(x);
                    }
                }
                for z in lo[1] + 1..=hi[1] {
                    if config.zone_crop_in(lo, hi, [lo[0], z - 1])
                        != config.zone_crop_in(lo, hi, [lo[0], z])
                    {
                        cuts_z.push(z);
                    }
                }
                let (x0, x1) = (f64::from(lo[0]), f64::from(hi[0] + 1));
                let (z0, z1) = (f64::from(lo[1]), f64::from(hi[1] + 1));
                for x in cuts_x {
                    g.line(
                        [f64::from(x), y, z0],
                        [f64::from(x), y, z1],
                        0x9988ccaa,
                        1.0,
                    );
                }
                for z in cuts_z {
                    g.line(
                        [x0, y, f64::from(z)],
                        [x1, y, f64::from(z)],
                        0x9988ccaa,
                        1.0,
                    );
                }
            }
            // What the farm is bound to, each in its colour (the menu lists the same with the
            // coordinates): harvest orange, seeds green, bone meal white, the composter brown,
            // the safe spot blue.
            let [harvest, seeds, meal] = storages(game);
            let marks = [
                (harvest, 0xffffa040_u32),
                (seeds, 0xff60e060),
                (meal, 0xfff0f0f0),
                (farm_composter_in(game, areas), 0xffb07840),
                (safe_point(game), 0xff60a0ff),
            ];
            for (at, color) in marks {
                let Some(at) = at else {
                    continue;
                };
                if nav::distance(nav::center(at), [p.x, p.y, p.z]) > 96.0 {
                    continue;
                }
                let min = at.map(|v| f64::from(v) - 0.02);
                let max = at.map(|v| f64::from(v) + 1.02);
                g.cuboid(min, max, color, 2.0);
            }
        }
        for at in blocks {
            let a = nav::center(at);
            if nav::distance(a, [p.x, p.y, p.z]) > 96.0 {
                continue;
            }
            for (u, v) in [(0, 1), (0, 2), (1, 2)] {
                for side in 0..4 {
                    let mut b = at.map(f64::from);
                    b[u] += f64::from(side & 1);
                    b[v] += f64::from((side >> 1) & 1);
                    let mut c = b;
                    c[3 - u - v] += 1.0;
                    g.line(b, c, 0xff88ccaa, 1.5);
                }
            }
        }
    });
}
