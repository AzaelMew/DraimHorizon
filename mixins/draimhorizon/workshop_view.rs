// Farm and schematic jobs share the normal movement, attack and placement drivers.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_construction_view as placement,
    claim_borders_draining as drain, claim_borders_farm_storage as store,
    claim_borders_farming as farm, claim_borders_navigation as nav,
    claim_borders_navigation_view as driver, claim_borders_schematic as schema,
    claim_borders_schematic_files as files,
};
use horizon_client::gizmos::Gizmos;
use std::{
    collections::BTreeMap,
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
        origin: nav::Pos,
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
    /// Put the harvest (and spare seeds) into the storage.
    Deposit,
    /// Take seeds of this crop out of it.
    Seeds(farm::Crop),
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
    // The dropped item being walked to, and the ones given up on.
    loot: Option<Loot>,
    ignored_items: Vec<u32>,
    // The trip to the storage container in progress, the earliest next trip, and whether the
    // farm ran out of ripe crops (a good moment to unload the harvest).
    store: Option<Store>,
    store_after: Instant,
    idle_deposit: bool,
    // Failed placements in a row, and cells given up on, for the end message.
    errors: u8,
    missed: u16,
    // Whether the block being dropped falls (sand, gravel) and needs time to sink.
    falling: bool,
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
            errors: 0,
            missed: 0,
            falling: false,
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
    let origin = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    );
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        *s = State::default();
        s.scope = scope;
        s.task = Some(Task::Farm {
            config,
            origin,
            harvested: 0,
        });
        s.preview = preview;
        s.active = !preview;
    });
    Ok(())
}
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
        Ok(())
    })
}
/// The storage container the player set, in the current world.
static STORAGE: Mutex<Option<(String, nav::Pos)>> = Mutex::new(None);
pub(super) fn storage(game: &Game<'_>) -> Option<nav::Pos> {
    let scope = driver::scope(game)?;
    STORAGE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .filter(|(s, _)| *s == scope)
        .map(|(_, at)| *at)
}
/// Takes the container under the crosshair as the farm's storage.
pub(super) fn set_storage(game: &Game<'_>) -> Result<nav::Pos, String> {
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
    *STORAGE.lock().unwrap_or_else(|e| e.into_inner()) = Some((scope, hit.pos));
    Ok(hit.pos)
}
pub(super) fn clear_storage() {
    *STORAGE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
/// Whether a drain job is running (a fall into the water is then recovered from, not a pause).
pub(super) fn draining() -> bool {
    state(|s| s.active && matches!(s.task, Some(Task::Drain { .. })))
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
/// Why a trip to the storage is due now, if it is.
fn store_due(game: &Game<'_>, s: &State) -> Option<Purpose> {
    let Some(Task::Farm { config, .. }) = &s.task else {
        return None;
    };
    if Instant::now() < s.store_after {
        return None;
    }
    // Empty farmland and a selected crop with no seeds on hand: fetch them.
    if config.replant
        && s.unplanted
        && let Some(crop) = config.selected().find(|c| !seed(game, *c))
    {
        return Some(Purpose::Seeds(crop));
    }
    let inv = inventory(game);
    let crops: Vec<_> = config.selected().collect();
    let load = store::deposit(&inv, &crops, store::KEEP_SEEDS);
    if !load.is_empty() && (store::free_slots(&inv) <= 2 || s.idle_deposit) {
        return Some(Purpose::Deposit);
    }
    None
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
fn end_store_for(game: &mut Game<'_>, failure: Option<&str>, cooldown: u64) {
    if game.screen == Screen::Inventory {
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    placement::cancel_container();
    state(|s| {
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
    let Some(t) = state(|s| s.store) else {
        return;
    };
    if !game.native_on(claims::ID) {
        end_store(game, None);
        return;
    }
    let open = game.screen == Screen::Inventory
        && game
            .remote_menu_type()
            .is_some_and(store::is_container_menu);
    match t.step {
        Step::Walk => {}
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
            if !open {
                end_store(game, Some(text("Склад закрылся", "The storage closed")));
            } else if t.last.elapsed() >= Duration::from_millis(160) {
                transfer(game, t);
            }
        }
        Step::Close => end_store(game, None),
    }
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
    let (plan, ops): (Vec<usize>, Vec<_>) = match t.purpose {
        Purpose::Deposit => {
            let plan = store::deposit(&inv, &crops, store::KEEP_SEEDS);
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
    };
    let sig = plan.iter().sum::<usize>() + plan.len() * 100;
    let stuck = if sig == t.sig { t.stuck + 1 } else { 0 };
    if plan.is_empty() {
        // No seeds in the storage either: not an error. The farm idles, waits for the crops
        // to ripen and asks the storage again a minute later.
        let none =
            matches!(t.purpose, Purpose::Seeds(crop) if store::count(&inv, crop.seed()) == 0);
        end_store_for(game, None, if none { 60 } else { 15 });
        if none {
            state(|s| {
                s.message = text(
                    "в складе нет семян, жду созревания",
                    "no seeds in the storage, waiting for crops to ripen",
                )
                .into();
                s.waiting = true;
            });
        }
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
    let (origin, radius) = match &s.task {
        Some(Task::Farm { origin, config, .. }) => (*origin, i32::from(config.radius)),
        _ => return false,
    };
    let Some(p) = game.native_player() else {
        return false;
    };
    let here = [p.x, p.y, p.z];
    let at = |d: &horizon_client::entities::ItemEntity| [d.position.x, d.position.y, d.position.z];
    let near = |d: &horizon_client::entities::ItemEntity| {
        let a = at(d);
        (a[0] - f64::from(origin[0])).abs() <= f64::from(radius + 3)
            && (a[2] - f64::from(origin[2])).abs() <= f64::from(radius + 3)
            && (a[1] - f64::from(origin[1])).abs() <= 4.0
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
        if !game.native_playing() {
            // The storage window is open: store_frame drives it, the job just waits.
            if s.store.is_some() && game.screen == Screen::Inventory {
                return;
            }
            s.active = false;
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
                                    .push((at, Instant::now() + Duration::from_millis(800)));
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
            && let (Some(chest), Some(purpose)) = (storage(game), store_due(game, s))
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
        if s.target.is_none() {
            match s.task.as_mut().unwrap() {
                Task::Farm {
                    config,
                    origin,
                    harvested,
                } => {
                    if config.limit > 0 && *harvested >= config.limit {
                        s.phase = Phase::Done;
                        s.active = false;
                        s.message = text("Готово", "Done").into();
                        return;
                    }
                    if s.last.elapsed() < Duration::from_millis(500) {
                        return;
                    }
                    s.last = Instant::now();
                    let r = i32::from(config.radius);
                    let mut targets = Vec::new();
                    let mut dark = 0;
                    let mut unplanted = false;
                    let crops: Vec<_> = config.selected().collect();
                    // What can be sown now: the first selected crop whose seeds are at hand.
                    let sow = if config.replant {
                        crops.iter().copied().find(|c| seed(game, *c))
                    } else {
                        None
                    };
                    for z in -r..=r {
                        for x in -r..=r {
                            // Beds at other heights count too (a raised bed, a terrace): every
                            // layer from two below the start to two above is looked at.
                            for dy in -2..=2 {
                                let at = [origin[0] + x, origin[1] + dy, origin[2] + z];
                                if game.native_block([at[0], at[1] - 1, at[2]])
                                    != "minecraft:farmland"
                                {
                                    continue;
                                }
                                let name = game.native_block(at);
                                let ripe = crops.iter().copied().find(|c| {
                                    name == c.block()
                                        && c.mature(
                                            game.native_block_property(at, "age").as_deref(),
                                        )
                                });
                                let mature = ripe.is_some();
                                // A crop only survives where the light is 9 or more (sky or
                                // block light): in a cave the server refuses the seed.
                                let light = i32::from(game.world.light(at[0], at[1], at[2]));
                                let lit = (light >> 4).max(light & 15) >= 9;
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
                    let p = game.native_player().unwrap();
                    targets.sort_by(|a, b| {
                        nav::distance(nav::center(a.0), [p.x, p.y, p.z])
                            .total_cmp(&nav::distance(nav::center(b.0), [p.x, p.y, p.z]))
                    });
                    s.unplanted = unplanted;
                    if let Some((at, plant, crop)) = targets.first() {
                        s.target = Some(*at);
                        s.plant = *plant;
                        s.crop = *crop;
                        s.waiting = false;
                        s.attempt = 0;
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
                        return;
                    } else if dark > 0 {
                        // Waits for a torch: the scan repeats, so the job goes on by itself.
                        s.message = text(
                            "нет света на грядках: для посева нужен источник света (уровень 9+)",
                            "no light on the farmland: crops need a light source (level 9+)",
                        )
                        .into();
                        s.waiting = true;
                        return;
                    } else {
                        s.message = text("ожидание созревания", "waiting for ripe crops").into();
                        s.waiting = true;
                        // Nothing to harvest for now: a good time to unload.
                        s.idle_deposit = true;
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
                    s.settle = Duration::from_millis(if falling { 120 } else { 80 });
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
                                        .is_some_and(|d| d <= Duration::from_millis(1000)) =>
                                {
                                    s.wait = Duration::from_millis(120);
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
    state(|s| {
        let label = match s.task.as_ref() {
            Some(Task::Farm {
                harvested, config, ..
            }) => format!(
                "{}: {harvested}/{}",
                text("Ферма", "Farm"),
                if config.limit == 0 {
                    "∞".into()
                } else {
                    config.limit.to_string()
                }
            ),
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
    state(|s| {
        if driver::scope(game).as_deref() != Some(s.scope.as_str()) {
            return;
        }
        let Some(p) = game.native_player() else {
            return;
        };
        // The working area is an outline on the ground (the two corner cubes drawn before
        // looked like stray holograms); the block being worked on is the one cube.
        let mut area = None;
        let blocks = match &s.task {
            Some(Task::Farm { origin, config, .. }) => {
                area = Some((*origin, i32::from(config.radius)));
                s.target.into_iter().collect()
            }
            Some(Task::Drain { origin, config, .. }) => {
                area = Some((*origin, i32::from(config.radius)));
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
        if let Some((origin, r)) = area {
            let (x0, x1) = (f64::from(origin[0] - r), f64::from(origin[0] + r + 1));
            let (z0, z1) = (f64::from(origin[2] - r), f64::from(origin[2] + r + 1));
            let y = f64::from(origin[1]) + 0.05;
            for (a, b) in [
                ([x0, y, z0], [x1, y, z0]),
                ([x1, y, z0], [x1, y, z1]),
                ([x1, y, z1], [x0, y, z1]),
                ([x0, y, z1], [x0, y, z0]),
            ] {
                g.line(a, b, 0xff88ccaa, 1.5);
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
