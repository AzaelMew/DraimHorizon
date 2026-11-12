// Native adapter for the local walking navigator; bounded background path planning.
use crate::game::claim_borders_mining as mining;
use crate::game::{
    Game, Resources, Screen, claim_borders as claims, claim_borders_navigation as model,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::{chunk::Chunk, gizmos::Gizmos};
use horizon_world::registry::BlockStateId;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

pub use model::Goal;

/// Native keys are persisted and edited by Horizon's Options > Mods page.
/// The native_key hook excludes repeats and runs independently of our menu.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) -> bool {
    let bindings = model::Hotkey::ALL
        .map(|action| game.native_mod_key(claims::ID, action.id(), action.preset()));
    let eligible = allowed(game)
        && game.native_window()
        && game.focused
        && game.native_playing()
        && !game.screen_over_playing()
        && game.player.is_some()
        && game.sleeping.is_none()
        && game.modifiers.is_empty();
    let conflict = game
        .settings
        .keys
        .mappings_for(key)
        .any(|name| !name.starts_with("key.debug."))
        || game.settings.mods.installed.iter().any(|row| {
            game.native_on(&row.id)
                && row.native.as_ref().is_some_and(|native| {
                    native.keys.iter().any(|[name, _, default]| {
                        !(row.id == claims::ID && model::Hotkey::ALL.iter().any(|a| a.id() == name))
                            && game.native_mod_key(&row.id, name, default) == key
                    })
                })
        });
    let Some(action) = model::hotkey(
        key,
        pressed,
        eligible,
        conflict,
        bindings.each_ref().map(String::as_str),
    ) else {
        return false;
    };
    if let Err(error) = run(game, action) {
        game.native_chat(&error);
    }
    true
}
/// Carry out an action key. Returns whether the menu should close (a task was started).
/// A paused task resumes where it stopped; with nothing to resume, Start begins what the
/// Baritone tab shows. Neither path bypasses the consent, world identity or
/// server-confirmation gates, and a press while a task is running does nothing, so an
/// active dig never loses its ACK state.
pub(super) fn run(game: &mut Game<'_>, action: model::Hotkey) -> Result<bool, String> {
    match action {
        model::Hotkey::Preview => start_from_menu(game, false).map(|()| false),
        model::Hotkey::Start => {
            let (running, resumable) = control();
            if running {
                return Ok(false);
            }
            if resumable {
                resume(game)
            } else {
                start_from_menu(game, true)
            }
            .map(|()| true)
        }
        model::Hotkey::Pause => {
            pause();
            Ok(false)
        }
        model::Hotkey::Stop => {
            // The player ended it: it does not come back after a reconnect.
            crate::game::claim_borders_resume::forget();
            crate::game::claim_borders_resume::forget_build(game);
            reset();
            Ok(false)
        }
    }
}
/// Start or preview the task of the Baritone tab that was last on screen.
fn start_from_menu(game: &mut Game<'_>, running: bool) -> Result<(), String> {
    use crate::game::claim_borders_menu::Action;
    use crate::game::claim_borders_schematic_view as preview;
    // Schematics: Start builds the selected preview of this world as it stands (moved and
    // turned with the editor keys); with none, the file and place chosen in the menu.
    if running
        && crate::game::claim_borders_menu_view::schematic_mode()
        && let Some((file, origin, rotation)) = preview::build_target(game)
    {
        return crate::game::claim_borders_workshop_view::load(game, file, origin, rotation, true);
    }
    match crate::game::claim_borders_menu_view::baritone_request(running)? {
        Action::Navigate { goal, running } => start(game, goal, running),
        Action::LoadSchematic {
            filename,
            origin,
            rotation,
            running,
        } => {
            // The preview is a hologram in the world, kept until it is removed; building
            // starts from it.
            preview::preview(game, filename.clone(), origin, rotation);
            if running {
                crate::game::claim_borders_workshop_view::load(
                    game, filename, origin, rotation, true,
                )
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}
static WAS_FOCUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static REFOCUSED: Mutex<Option<Instant>> = Mutex::new(None);
/// True for a moment after the window got focus back: input in that moment (the click that
/// focused it, keys still held) must not pause a task.
pub(super) fn input_grace() -> bool {
    REFOCUSED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < Duration::from_millis(1500))
}
/// Set while this mod has switched off "pause when the window loses focus".
static FOCUS_PAUSE_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A task keeps running with the window minimised or in the background: the game would open
/// the pause menu after the window loses focus, and every task stops on an open screen. While
/// a task runs, that option is off in memory only (never saved); it is put back when the task
/// ends, or when the mod is switched off.
pub(super) fn keep_running_unfocused(game: &mut Game<'_>) {
    use std::sync::atomic::Ordering::Relaxed;
    // The click that brings the window back to the front is not the player taking over.
    let focused = game.focused;
    if focused && !WAS_FOCUSED.swap(true, Relaxed) {
        *REFOCUSED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    } else if !focused {
        WAS_FOCUSED.store(false, Relaxed);
    }
    let busy = game.native_on(claims::ID) && control().0;
    let off = FOCUS_PAUSE_OFF.load(Relaxed);
    if busy && !off && game.settings.file.bool("pauseOnLostFocus") {
        game.settings.file.set(
            "pauseOnLostFocus",
            horizon_client::options_file::V::Bool(false),
        );
        FOCUS_PAUSE_OFF.store(true, Relaxed);
    } else if !busy && off {
        game.settings.file.set(
            "pauseOnLostFocus",
            horizon_client::options_file::V::Bool(true),
        );
        FOCUS_PAUSE_OFF.store(false, Relaxed);
    }
}
/// In the world, possibly with the inventory, the chat or one of this mod's own screens (the
/// menu, the map) open: tasks go on. Walking carries on as usual; breaking and placing wait
/// until the screen is closed (the game takes no use or attack clicks behind a screen).
pub(super) fn in_world(game: &Game<'_>) -> bool {
    game.title.is_none()
        && game.sign_edit.is_none()
        && game.book.is_none()
        && matches!(
            game.screen,
            Screen::Playing | Screen::Inventory | Screen::Chat | Screen::Mod
        )
}
/// What the action keys should do with the task in progress: (running, resumable).
fn control() -> (bool, bool) {
    if assist_active() {
        return (true, false);
    }
    if let Some(control) = crate::game::claim_borders_workshop_view::control() {
        return control;
    }
    if let Some(control) = crate::game::claim_borders_construction_view::control() {
        return control;
    }
    state(|s| {
        if let Some(job) = s.mining.as_ref() {
            let preview = !job.active
                && matches!(
                    s.nav.status,
                    model::Status::Preview | model::Status::Planning
                );
            return (
                job.active,
                !job.active && !preview && (job.limit == 0 || job.confirmed < job.limit),
            );
        }
        (
            s.nav.running,
            !s.nav.running && matches!(s.nav.status, model::Status::Paused | model::Status::Stuck),
        )
    })
}

struct Query {
    flat: bool,
    /// Stay on one level: no steps up or down, so no jumps (farmland is not trampled).
    level: bool,
    /// Digging is allowed: a usable pickaxe is held and the inventory has room.
    dig: bool,
    /// Missing floor may be bridged: building blocks are on hand.
    place: bool,
    /// Plan a dig/bridge route even when a walking route exists.
    force: bool,
    /// Ores already marked by the ore experiment, tried first when mining.
    hints: Vec<model::Pos>,
    mine: Option<(u8, Vec<model::Pos>)>,
    token: u64,
    start: model::Pos,
    goal: model::Pos,
    radius: u8,
    /// The longest drop the route may step off (`navigation::drop_limit`).
    drop: i32,
    /// The goal is far off or at another height: the wide search (`navigation::WIDE`).
    wide: bool,
    chunks: HashMap<(i32, i32), Arc<Chunk>>,
    resources: Arc<Resources>,
}
struct Answer {
    target: Option<model::Pos>,
    token: u64,
    start: model::Pos,
    path: Option<model::Path>,
    dig_plan: Option<model::DigPath>,
    /// What the search for a mining target saw, for the status line.
    stats: Option<mining::PlanStats>,
}
struct Worker {
    tx: SyncSender<Query>,
    rx: Receiver<Answer>,
    generation: Arc<AtomicU64>,
}
impl Worker {
    fn spawn() -> Result<Self, String> {
        let (tx, requests) = mpsc::sync_channel::<Query>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = generation.clone();
        std::thread::Builder::new()
            .name("horizon-navigation".into())
            .spawn(move || {
                horizon_threads::idle_current();
                while let Ok(query) = requests.recv() {
                    let canceled = || worker_generation.load(Ordering::Relaxed) != query.token;
                    // Block states to what the planners need of them (a fast hasher: this runs
                    // for every block read), and the chunk read last (reads come in runs).
                    let mut table: HashMap<
                        u32,
                        mining::Block,
                        std::hash::BuildHasherDefault<model::PosHasher>,
                    > = HashMap::default();
                    let mut last: Option<((i32, i32), Option<&Arc<Chunk>>)> = None;
                    let mut read = |at: model::Pos| {
                        if query.level
                            && !(query.start[1] - 1..=query.start[1] + 1).contains(&at[1])
                        {
                            return mining::Block::default();
                        }
                        let key = (at[0].div_euclid(16), at[2].div_euclid(16));
                        let chunk = match last {
                            Some((k, chunk)) if k == key => chunk,
                            _ => {
                                let chunk = query.chunks.get(&key);
                                last = Some((key, chunk));
                                chunk
                            }
                        };
                        let Some(chunk) = chunk else {
                            return mining::Block::default();
                        };
                        if at[1] < chunk.min_y() || at[1] >= chunk.max_y() {
                            return mining::Block::default();
                        }
                        let id = chunk.get(at[0].rem_euclid(16), at[1], at[2].rem_euclid(16));
                        *table.entry(id).or_insert_with(|| {
                            let f = query.resources.facts().get(BlockStateId(id));
                            mining::Block {
                                cell: model::classify_shape(
                                    &f.name,
                                    f.full_collision,
                                    f.fluid_level > 0 || f.waterlogged,
                                    &f.outline,
                                    model::Cell::Air,
                                ),
                                ore: query
                                    .mine
                                    .as_ref()
                                    .is_some_and(|(ore, _)| mining::matches(*ore, &f.name)),
                                falling: mining::falling(&f.name),
                                breakable: f.hardness >= 0.0 && !f.block_entity,
                                soft: mining::clearable(&f.name),
                                water: (f.fluid_level > 0 || f.waterlogged)
                                    && !f.name.contains("lava"),
                            }
                        })
                    };
                    // Every planner on this thread goes by the request's drop limit.
                    model::set_drop_limit(query.drop);
                    model::set_reach(if query.wide { model::WIDE } else { model::NEAR });
                    model::set_long_jumps(!query.level);
                    // The player may stand on an edge or in the air: plan from the nearest place
                    // that can be stood on (the answer keeps the real start for its own check).
                    let start = model::nearest_standable(query.start, &mut |at| read(at).cell)
                        .unwrap_or(query.start);
                    let mut dig_plan = None;
                    let mut stats = None;
                    let (target, path) = if let Some((_, excluded)) = query.mine.as_ref() {
                        let mut seen = mining::PlanStats::default();
                        let found = mining::plan_with_stats(
                            start, excluded, &mut read, canceled, &mut seen,
                        );
                        stats = Some(seen);
                        match found {
                            Some((target, path)) => (Some(target), Some(path)),
                            // Nothing is exposed: look for ore inside the rock and plan
                            // the route that digs to it.
                            None if query.dig && !canceled() => {
                                match mining::plan_buried(
                                    start,
                                    excluded,
                                    &query.hints,
                                    query.place,
                                    &mut read,
                                    canceled,
                                ) {
                                    Some((target, route)) => {
                                        dig_plan = Some(route);
                                        (Some(target), None)
                                    }
                                    None => (None, None),
                                }
                            }
                            None => (None, None),
                        }
                    } else {
                        (
                            None,
                            model::search_near_drop(
                                start,
                                query.goal,
                                query.radius,
                                query.drop,
                                |at| read(at).cell,
                                canceled,
                            ),
                        )
                    };
                    // A walking route that does not reach the goal (or a request to avoid a
                    // dead end) is answered with a route that breaks and places blocks.
                    if query.mine.is_none()
                        && (query.dig || query.place)
                        && (query.force || path.as_ref().is_none_or(|walking| !walking.complete))
                        && !canceled()
                    {
                        let read = std::cell::RefCell::new(read);
                        let digging = query.dig;
                        dig_plan = model::search_dig(
                            start,
                            query.goal,
                            false,
                            query.place,
                            |at| (*read.borrow_mut())(at).cell,
                            |at| {
                                let block = (*read.borrow_mut())(at);
                                digging
                                    && block.cell == model::Cell::Solid
                                    && block.breakable
                                    && !block.falling
                                    && !(*read.borrow_mut())([at[0], at[1] + 1, at[2]]).falling
                            },
                            canceled,
                        );
                    }
                    if !canceled() {
                        let _ = results.send(Answer {
                            target,
                            token: query.token,
                            start: query.start,
                            path,
                            dig_plan,
                            stats,
                        });
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, rx, generation })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}
struct State {
    flat: bool,
    level: bool,
    use_restore: Option<bool>,
    attack_restore: Option<bool>,
    mining: Option<mining::Job>,
    /// What the last mining search saw, shown while it finds nothing.
    mine_stats: Option<mining::PlanStats>,
    mine_note: String,
    mine_aim: Option<[f32; 2]>,
    started: Instant,
    nav: model::Navigator,
    scope: String,
    worker: Option<Worker>,
    token: u64,
    pending: bool,
    /// (blocks to break, blocks to place) of a route shown as a preview.
    preview_route: Option<(u16, u16)>,
    /// Ask the planner for a dig/bridge route even if walking could continue.
    force_dig: bool,
    /// A dead end was already answered with a dig/bridge route once.
    stuck_tried: bool,
    last_query: Instant,
}
impl Default for State {
    fn default() -> Self {
        Self {
            flat: false,
            level: false,
            use_restore: None,
            attack_restore: None,
            mining: None,
            mine_stats: None,
            mine_note: String::new(),
            mine_aim: None,
            started: Instant::now(),
            nav: model::Navigator::default(),
            scope: String::new(),
            worker: None,
            token: 0,
            pending: false,
            preview_route: None,
            force_dig: false,
            stuck_tried: false,
            last_query: Instant::now() - Duration::from_secs(2),
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    crate::game::claim_borders_workshop_view::reset();
    crate::game::claim_borders_construction_view::reset();
    drop_assist();
    state(|s| {
        let restore = s.attack_restore;
        let use_restore = s.use_restore;
        *s = State::default();
        s.attack_restore = restore;
        s.use_restore = use_restore;
    });
}
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
pub(super) fn scope(game: &Game<'_>) -> Option<String> {
    let p = game.native_player()?;
    Some(if let Some(remote) = game.remote.as_ref() {
        format!(
            "{}#{:032x}#{}#{}",
            remote.session.address,
            remote.profile(),
            p.dimension,
            game.pack.render.generation
        )
    } else {
        format!(
            "{}#{}#{}",
            game.world_root.to_string_lossy(),
            p.dimension,
            game.pack.render.generation
        )
    })
}
/// The world the player is in, for what is kept across sessions (farm plots, schematic
/// previews): the server (or the save in single player) and the dimension. Unlike `scope`, no
/// account and no resource generation (which counts up at every resource reload and starts again
/// with the game), so it is the same after a reconnect and after a restart.
pub(super) fn world_key(game: &Game<'_>) -> Option<String> {
    let p = game.native_player()?;
    Some(if let Some(remote) = game.remote.as_ref() {
        format!("{}#{}", remote.session.address, p.dimension)
    } else {
        format!("{}#{}", game.world_root.to_string_lossy(), p.dimension)
    })
}
fn allowed(game: &Game<'_>) -> bool {
    game.native_on(claims::ID)
        && game
            .native_setting(claims::SETTINGS_ID, "experiments_accepted")
            .as_deref()
            == Some("true")
}
fn cancel_pending(s: &mut State) {
    s.token = s.token.wrapping_add(1);
    if let Some(w) = s.worker.as_ref() {
        w.generation.store(s.token, Ordering::Relaxed);
    }
    s.pending = false;
    s.preview_route = None;
}
fn pause_state(s: &mut State) {
    release_assist(s);
    s.nav.pause();
    if let Some(job) = s.mining.as_mut() {
        job.active = false;
    }
    s.mine_aim = None;
}
/// Something in the way of the block being mined that is being cleared (see `Job::clear_blocker`).
static BLOCKER: Mutex<Option<model::Pos>> = Mutex::new(None);
fn mining_block(game: &Game<'_>, ore: u8, at: model::Pos) -> mining::Block {
    if !game
        .world
        .chunks
        .contains_key(&(at[0].div_euclid(16), at[2].div_euclid(16)))
    {
        return mining::Block::default();
    }
    let f = game.res.facts().get(game.world.block(at[0], at[1], at[2]));
    mining::Block {
        cell: model::classify_shape(
            &f.name,
            f.full_collision,
            f.fluid_level > 0 || f.waterlogged,
            &f.outline,
            model::Cell::Air,
        ),
        ore: if let Some(crop) = crate::game::claim_borders_farming::Crop::from_code(ore) {
            crop.block() == f.name.as_ref()
                && crop.mature(game.native_block_property(at, "age").as_deref())
        } else if ore == u8::MAX {
            crate::game::claim_borders_construction_view::block(game, at).diggable
        } else {
            mining::matches(ore, &f.name)
                || (mining::clearable(&f.name)
                    && *BLOCKER.lock().unwrap_or_else(|e| e.into_inner()) == Some(at))
        },
        falling: mining::falling(&f.name),
        breakable: f.hardness >= 0.0 && !f.block_entity,
        soft: mining::clearable(&f.name),
        water: (f.fluid_level > 0 || f.waterlogged) && !f.name.contains("lava"),
    }
}
/// The stack in slot `slot`: 0..9 the hotbar, 9..36 the main inventory.
fn slot_stack<'a>(game: &'a Game<'_>, slot: usize) -> Option<&'a horizon_client::items::ItemStack> {
    if slot < 9 {
        game.stack_in(slot)
    } else {
        game.inventory.get(slot).and_then(Option::as_ref)
    }
}
/// Durability a tool keeps in reserve so that it never breaks: about half a percent of its
/// total (10 on a netherite tool, 7 on a diamond one), at least 3 points.
fn tool_reserve(max: u32) -> u32 {
    (max / 200).max(3)
}
/// (total durability, durability left) of a stack; (0, 0) for something that does not wear.
fn tool_wear(game: &Game<'_>, stack: &horizon_client::items::ItemStack) -> (u32, u32) {
    let mut max = game
        .res
        .items()
        .items
        .get(&stack.item)
        .map_or(0, |i| i.max_damage);
    if let Some(horizon_world::nbt::Tag::Int(value)) = stack
        .extra
        .as_deref()
        .and_then(|e| e.other("minecraft:max_damage"))
    {
        max = (*value).max(0) as u32;
    }
    (max, max.saturating_sub(stack.damage))
}
/// Whether `stack` is a pickaxe, axe or shovel that still has durability to spare and, with a
/// block given, is the right kind for it.
fn tool_usable(
    game: &Game<'_>,
    stack: &horizon_client::items::ItemStack,
    block: Option<&str>,
    pickaxe_only: bool,
) -> bool {
    let kinds: &[&str] = if pickaxe_only {
        &["_pickaxe"]
    } else {
        &["_pickaxe", "_axe", "_shovel"]
    };
    if !kinds.iter().any(|kind| stack.item.ends_with(kind)) {
        return false;
    }
    let (max, left) = tool_wear(game, stack);
    if max > 0 && left <= tool_reserve(max) {
        return false;
    }
    block.is_none_or(|name| game.res.items().correct_for_drops(Some(stack), name))
}
/// Whether the block at `at` can be broken without a tool (dirt, sand, leaves): any hand will do.
fn needs_no_tool(game: &Game<'_>, at: model::Pos) -> bool {
    !game
        .res
        .facts()
        .get(game.world.block(at[0], at[1], at[2]))
        .requires_tool
}
/// The slot of the tool to use: among the right ones for the block (any, with no block given)
/// anywhere in the hotbar or inventory, a fresh one (over a fifth of its durability left) comes
/// before a worn one, the held one before the rest.
fn tool_slot(game: &Game<'_>, target: Option<model::Pos>, pickaxe_only: bool) -> Option<usize> {
    let block = target.map(|at| game.native_block(at));
    let fits = |slot: &usize| {
        slot_stack(game, *slot)
            .is_some_and(|stack| tool_usable(game, stack, block.as_deref(), pickaxe_only))
    };
    let fresh = |slot: &usize| {
        slot_stack(game, *slot).is_some_and(|stack| {
            let (max, left) = tool_wear(game, stack);
            max == 0 || left * 5 >= max
        })
    };
    let order = || std::iter::once(game.selected).chain(0..36);
    order()
        .filter(|slot| fits(slot))
        .find(|slot| fresh(slot))
        .or_else(|| order().find(|slot| fits(slot)))
}
fn mining_tool(game: &Game<'_>, target: Option<model::Pos>) -> bool {
    tool_slot(game, target, false).is_some() || target.is_some_and(|at| needs_no_tool(game, at))
}
/// Tunnels and bridges dig stone, which only a pickaxe is meant for.
pub(super) fn construction_tool(game: &Game<'_>) -> bool {
    tool_slot(game, None, true).is_some()
}
enum Tool {
    Ready,
    /// A swap from the inventory is on its way: try again next tick.
    Wait,
    Missing,
}
/// Puts a suitable tool in hand: the selected one if it fits, else a hotbar slot is selected, else
/// a stack from the main inventory is swapped into the hotbar first.
fn ensure_tool(game: &mut Game<'_>, target: Option<model::Pos>) -> Tool {
    let Some(slot) = tool_slot(game, target, false) else {
        // No tool is the right one, but dirt, sand and the like break by hand: carry on.
        return if target.is_some_and(|at| needs_no_tool(game, at)) {
            Tool::Ready
        } else {
            Tool::Missing
        };
    };
    hold_slot(game, slot, &|_| false)
}
/// The slot of the item with the most Fortune (a crop broken with it drops more): the selected
/// one first, then the hotbar, then the inventory. Breaking a crop wears no tool.
pub(super) fn fortune_slot(game: &Game<'_>) -> Option<usize> {
    let level = |slot: usize| {
        slot_stack(game, slot)
            .filter(|s| s.count > 0)
            .map_or(0, |s| s.enchantment_level("minecraft:fortune"))
    };
    std::iter::once(game.selected)
        .chain(0..36)
        .filter(|&slot| level(slot) > 0)
        .max_by_key(|&slot| {
            (
                level(slot),
                std::cmp::Reverse(slot != game.selected),
                std::cmp::Reverse(slot),
            )
        })
}
/// Whether the stack in `slot` carries Fortune (kept in the hotbar by the farm's other swaps).
pub(super) fn fortune_in(game: &Game<'_>, slot: usize) -> bool {
    slot_stack(game, slot).is_some_and(|s| s.enchantment_level("minecraft:fortune") > 0)
}
/// Puts the stack of `slot` in hand: a hotbar slot is selected, a stack of the main inventory
/// is swapped into the hotbar first (into an empty slot, else one `keep` does not protect).
fn hold_slot(game: &mut Game<'_>, slot: usize, keep: &dyn Fn(usize) -> bool) -> Tool {
    use horizon_client::menu::Input;
    static LAST_SWAP: Mutex<Option<Instant>> = Mutex::new(None);
    if slot < 9 {
        if game.selected != slot {
            // The slot change has to reach the server before the first hit: the server works
            // out the break time with the item it believes is in hand, and refuses a block the
            // client broke too fast with a better tool.
            game.selected = slot;
            return Tool::Wait;
        }
        return Tool::Ready;
    }
    let mut last = LAST_SWAP.lock().unwrap_or_else(|e| e.into_inner());
    // One inventory swap every five ticks: enough for the server to answer each, without the
    // job standing still for most of a second at every change of tool.
    if last.is_some_and(|t| t.elapsed() < Duration::from_millis(250)) {
        return Tool::Wait;
    }
    let spare = (0..9)
        .filter(|h| *h != game.selected)
        .find(|h| game.stack_in(*h).is_none())
        .or_else(|| (0..9).rev().find(|h| *h != game.selected && !keep(*h)))
        .or_else(|| (0..9).rev().find(|h| *h != game.selected));
    let Some(spare) = spare else {
        return Tool::Missing;
    };
    *last = Some(Instant::now());
    if game.remote_menu_ops(&[(Some(slot), spare as i32, Input::Swap)]) {
        Tool::Wait
    } else {
        Tool::Missing
    }
}
/// Walks to `goal` for a workshop job. `flat` forbids breaking and placing blocks on the way;
/// `level` also keeps the route on one height (no jumps), which a farm wants among its crops
/// but not on the way to a chest on another level.
/// A walk to a far goal, taken in legs: the planner only sees 32 blocks around its start, so a
/// goal farther than that (a storage 80 or 800 blocks away) is reached by walking to a point on
/// the way, then planning again from there. A leg that has no way (a cliff, a river) is tried
/// again toward another point (turned aside, then shorter) before the walk gives up, and the
/// legs have a budget so that it never goes round in circles.
struct Legs {
    goal: model::Pos,
    flat: bool,
    level: bool,
    /// The last leg is under way: the walk is over when it arrives.
    last: bool,
    /// Which end of the current leg is being tried (`navigation::leg_target`).
    attempt: usize,
    /// Legs left to walk.
    budget: u32,
    /// Points of a remembered route still to pass, in order (`navigation::route_via`).
    via: Vec<model::Pos>,
    /// Where the legs so far ended, to see the walk going back and forth.
    ends: Vec<model::Pos>,
}
static LEGS: Mutex<Option<Legs>> = Mutex::new(None);
/// Routes walked to the end, kept per world: a later walk between the same places follows the
/// one it starts near instead of searching again (a second floor whose way down is far off is
/// found once). The cells of the walk under way are gathered as it goes.
struct Routes {
    scope: String,
    list: Vec<Vec<model::Pos>>,
    trail: Vec<model::Pos>,
    trail_goal: Option<model::Pos>,
}
static ROUTES: Mutex<Routes> = Mutex::new(Routes {
    scope: String::new(),
    list: Vec::new(),
    trail: Vec::new(),
    trail_goal: None,
});
/// The remembered route that serves a walk from `from` to `goal`, as the points to pass.
fn remembered_via(game: &Game<'_>, from: model::Pos, goal: model::Pos) -> Vec<model::Pos> {
    let routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    if scope(game).as_deref() != Some(routes.scope.as_str()) {
        return Vec::new();
    }
    routes
        .list
        .iter()
        .filter_map(|r| model::route_via(r, from, goal, 8))
        .min_by_key(Vec::len)
        .unwrap_or_default()
}
/// Starts gathering the cells of a walk to `goal` (a new walk drops the old trail).
fn begin_trail(game: &Game<'_>, from: model::Pos, goal: model::Pos) {
    let mut routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    let here = scope(game).unwrap_or_default();
    if routes.scope != here {
        routes.scope = here;
        routes.list.clear();
    }
    routes.trail = vec![from];
    routes.trail_goal = Some(goal);
}
/// Adds where the walker stands to the trail, every two blocks.
fn extend_trail(at: model::Pos) {
    let mut routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    if routes.trail_goal.is_none() || routes.trail.len() >= 4000 {
        return;
    }
    let far = routes.trail.last().is_none_or(|last| {
        (last[0] - at[0]).abs().max((last[2] - at[2]).abs()) >= 2 || (last[1] - at[1]).abs() >= 2
    });
    if far {
        routes.trail.push(at);
    }
}
/// The walk arrived: its trail is kept as a route (long ones only, replacing one between the
/// same two places, at most 32 per world).
fn keep_trail() {
    let mut routes = ROUTES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(goal) = routes.trail_goal.take() else {
        return;
    };
    let mut trail = std::mem::take(&mut routes.trail);
    trail.push(goal);
    let (Some(&a), Some(&b)) = (trail.first(), trail.last()) else {
        return;
    };
    let length = (a[0] - b[0]).abs() + (a[2] - b[2]).abs() + (a[1] - b[1]).abs();
    if trail.len() < 6 || length < 16 {
        return;
    }
    let near = |p: model::Pos, q: model::Pos| {
        (p[0] - q[0]).abs().max((p[2] - q[2]).abs()) <= 4 && (p[1] - q[1]).abs() <= 2
    };
    routes.list.retain(|r| {
        let (Some(&x), Some(&y)) = (r.first(), r.last()) else {
            return false;
        };
        !((near(x, a) && near(y, b)) || (near(x, b) && near(y, a)))
    });
    routes.list.push(trail);
    if routes.list.len() > 32 {
        routes.list.remove(0);
    }
}
/// A standable cell near `near` (columns within two blocks, heights from sixteen below to
/// eight above the player), for the end of a leg. Of the places to stand in a column (the
/// ground, a roof, the top of a tree) the one nearest to the height the walk is heading for is
/// taken, so that a walk back to a farm ends at the farm's level, not on a roof above it.
fn leg_cell(game: &Game<'_>, near: [f64; 3], from_y: i32, want_y: i32) -> Option<model::Pos> {
    let column = model::block(near);
    let mut best: Option<(i32, model::Pos)> = None;
    for dz in -2..=2_i32 {
        for dx in -2..=2_i32 {
            for y in from_y - 16..=from_y + 8 {
                let at = [column[0] + dx, y, column[2] + dz];
                if model::standable(at, &mut |p| live_cell(game, p)) {
                    let d = dx * dx + dz * dz + 3 * (y - want_y).abs();
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, at));
                    }
                }
            }
        }
    }
    best.map(|(_, at)| at)
}
/// The next point toward `goal` from `from`, trying the ends of a leg from `first` on: the goal
/// itself when it is within a leg, otherwise the first end that has ground under it. Returns
/// the point, whether it is the last leg and which end was used; `None` when no end is left.
fn leg_point(
    game: &Game<'_>,
    from: [f64; 3],
    goal: model::Pos,
    first: usize,
) -> Option<(model::Pos, bool, usize)> {
    let g = model::center(goal);
    if (g[0] - from[0]).hypot(g[2] - from[2]) <= model::LEG_REACH[0] + 2.0 {
        return Some((goal, true, first));
    }
    let from_y = from[1].floor() as i32;
    // The height to aim for at the end of this leg: on the way from here to the goal's.
    let span = (g[0] - from[0]).hypot(g[2] - from[2]).max(1.0);
    (first..model::LEG_TRIES).find_map(|attempt| {
        let near = model::leg_target(from, g, attempt)?;
        let part = ((near[0] - from[0]).hypot(near[2] - from[2]) / span).min(1.0);
        let want_y = from_y + (f64::from(goal[1] - from_y) * part).round() as i32;
        leg_cell(game, near, from_y, want_y).map(|at| (at, false, attempt))
    })
}
pub(super) fn workshop_walk(
    game: &Game<'_>,
    goal: model::Pos,
    flat: bool,
    level: bool,
) -> Result<(), String> {
    let from = game.native_player().map_or([0.0; 3], |p| [p.x, p.y, p.z]);
    let here = model::block(from);
    let g = model::center(goal);
    let distance = (g[0] - from[0]).hypot(g[2] - from[2]);
    begin_trail(game, here, goal);
    // A way walked before between here and there is followed point by point.
    let via = remembered_via(game, here, goal);
    let (point, last, attempt) = match via.first() {
        Some(first) => (*first, false, 0),
        None => leg_point(game, from, goal, 0).unwrap_or((goal, true, 0)),
    };
    *LEGS.lock().unwrap_or_else(|e| e.into_inner()) = (!last).then_some(Legs {
        goal,
        flat,
        level,
        last: false,
        attempt,
        budget: (distance / 8.0).ceil() as u32 + 40,
        via,
        ends: vec![point],
    });
    walk_to(game, point, flat, level)
}
/// Every tick: the trail of the walk grows, and when a leg of a far walk has arrived, or has no
/// way, the next one starts: the next point of a remembered route, or a new leg that does not
/// lead back to where the walk just came from.
pub(super) fn leg_frame(game: &Game<'_>) {
    let (running, status) = state(|s| (s.nav.running, s.nav.status));
    let Some(p) = game.native_player() else {
        return;
    };
    let from = [p.x, p.y, p.z];
    if running {
        extend_trail(model::block(from));
    }
    // The walk is over (no leg left): its way is remembered, whoever asked for it.
    if status == model::Status::Arrived
        && LEGS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_none_or(|l| l.last)
    {
        keep_trail();
    }
    let Some((goal, flat, level, attempt, budget, via, ends)) = LEGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .filter(|l| !l.last)
        .map(|l| {
            (
                l.goal,
                l.flat,
                l.level,
                l.attempt,
                l.budget,
                l.via.clone(),
                l.ends.clone(),
            )
        })
    else {
        return;
    };
    let arrived = match status {
        model::Status::Arrived => true,
        model::Status::NoPath | model::Status::Stuck => false,
        _ => return,
    };
    let mut via = via;
    if !via.is_empty() {
        if arrived {
            via.remove(0);
        } else {
            // The remembered way is blocked now (a door shut for good, a block placed): plan.
            via.clear();
        }
    }
    let next = if let Some(point) = via.first() {
        Some((*point, false, 0))
    } else if budget == 0 {
        None
    } else {
        // Arrived: on to the next leg, straight at the goal again; no way: another end of the
        // same leg. An end that leads back to where the walk came from is skipped.
        let mut first = if arrived { 0 } else { attempt + 1 };
        loop {
            match leg_point(game, from, goal, first) {
                Some((point, false, used)) if model::ping_pong(&ends, point) => first = used + 1,
                other => break other,
            }
        }
    };
    let Some((point, last, used)) = next else {
        // Out of ends to try or of legs: the walk is over (the status says there is no way).
        *LEGS.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return;
    };
    if let Some(l) = LEGS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        l.last = last;
        l.attempt = used;
        l.budget = budget - u32::from(arrived);
        l.via = via;
        l.ends.push(point);
        if l.ends.len() > 8 {
            l.ends.remove(0);
        }
    }
    if walk_to(game, point, flat, level).is_err() {
        *LEGS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
fn walk_to(game: &Game<'_>, goal: model::Pos, flat: bool, level: bool) -> Result<(), String> {
    clear_construction_driver();
    state(|s| {
        s.scope = scope(game).ok_or("Join a world first")?;
        s.flat = flat;
        s.level = level;
        s.nav.begin(goal, true);
        request(game, s)
    })
}
pub(super) fn workshop_walk_result() -> Option<bool> {
    let more = LEGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|l| !l.last);
    let status = state(|s| s.nav.status);
    match status {
        // A leg arrived or found no way, but the goal is farther: the next one starts on the
        // next tick (`leg_frame` clears the walk when there is nothing left to try).
        model::Status::Arrived | model::Status::NoPath | model::Status::Stuck if more => None,
        model::Status::Arrived => {
            // There: the way it took is remembered for the next walk between these places.
            keep_trail();
            Some(true)
        }
        model::Status::NoPath | model::Status::Stuck | model::Status::Paused => {
            *LEGS.lock().unwrap_or_else(|e| e.into_inner()) = None;
            Some(false)
        }
        _ => None,
    }
}
pub(super) fn farm_dig(
    game: &Game<'_>,
    target: model::Pos,
    crop: crate::game::claim_borders_farming::Crop,
) -> Result<(), String> {
    if !free_slot(game) {
        return Err(language()
            .text("Освободите слот в инвентаре", "Free an inventory slot")
            .into());
    }
    clear_construction_driver();
    state(|s| {
        s.scope = scope(game).ok_or("Join a world first")?;
        // Harvesting never breaks or places blocks on the way; it may climb a raised bed.
        s.flat = true;
        s.level = false;
        s.nav.begin(target, true);
        s.nav.status = model::Status::Mining;
        let mut j = mining::Job::excavation(target);
        j.ore = crop.code();
        j.started_ms = s.started.elapsed().as_millis() as u64;
        s.mining = Some(j);
        Ok(())
    })
}
pub(super) fn farm_result() -> Option<Result<(), crate::game::claim_borders_schematic::PlaceError>>
{
    use crate::game::claim_borders_schematic::PlaceError as E;
    state(|s| {
        match s
            .mining
            .as_ref()
            .filter(|j| crate::game::claim_borders_farming::Crop::from_code(j.ore).is_some())
        {
            Some(j) => match j.phase {
                mining::Phase::Done => Some(Ok(())),
                mining::Phase::Refused => Some(Err(E::Refused)),
                mining::Phase::Timeout => Some(Err(E::Timeout)),
                mining::Phase::Searching | mining::Phase::NoTarget => Some(Err(E::Unreachable)),
                _ if !j.active => Some(Err(E::Manual)),
                _ => None,
            },
            None => Some(Err(E::Manual)),
        }
    })
}
pub(super) fn workshop_pending() -> bool {
    state(|s| s.mining.as_ref().is_some_and(|j| j.awaiting_confirmation()))
}
pub(super) fn clear_construction_driver() {
    state(|s| {
        let attack = s.attack_restore;
        let use_restore = s.use_restore;
        *s = State::default();
        s.attack_restore = attack;
        s.use_restore = use_restore;
    });
}
pub(super) fn construction_dig(game: &Game<'_>, target: model::Pos) -> Result<(), ()> {
    // A full inventory does not stop a route's dig: the block's drop just stays on the ground.
    if !mining_tool(game, Some(target)) {
        return Err(());
    }
    let Some(p) = game.native_player() else {
        return Err(());
    };
    state(|s| {
        cancel_pending(s);
        s.scope = scope(game).unwrap();
        s.nav.begin(model::block([p.x, p.y, p.z]), true);
        s.nav.status = model::Status::Mining;
        let mut job = mining::Job::excavation(target);
        job.started_ms = s.started.elapsed().as_millis() as u64;
        s.mining = Some(job);
        s.mine_aim = None;
    });
    Ok(())
}
pub(super) fn construction_dig_result() -> Option<bool> {
    state(|s| match s.mining.as_ref().filter(|j| j.ore == u8::MAX) {
        Some(j) if j.phase == mining::Phase::Done => Some(true),
        Some(j) if j.active => None,
        _ => Some(false),
    })
}
pub(super) fn construction_dig_pending() -> bool {
    state(|s| {
        s.mining
            .as_ref()
            .is_some_and(|j| j.ore == u8::MAX && j.awaiting_confirmation())
    })
}
pub(super) fn construction_resume_dig() {
    state(|s| {
        if let Some(j) = s.mining.as_mut() {
            j.resume();
            j.started_ms = s.started.elapsed().as_millis() as u64;
        }
        s.nav.running = true;
        s.nav.status = model::Status::Mining;
    });
}
fn free_slot(game: &Game<'_>) -> bool {
    game.inventory.iter().take(36).any(Option::is_none)
}
/// Whether what a mining job breaks can still be picked up: a free slot, or a stack of the same
/// kind that is not full (sand laid one by one across the inventory still has room). The
/// inventory is full only when every slot is taken and every stack of the kind is at its limit.
fn room_for_mining(game: &Game<'_>, ore: u8) -> bool {
    free_slot(game)
        || (0..36).any(|slot| {
            slot_stack(game, slot).is_some_and(|stack| {
                let limit = game
                    .res
                    .items()
                    .get(&stack.item)
                    .map_or(64, |item| item.max_stack);
                stack.count < limit && mining::matches(ore, &stack.item)
            })
        })
}
fn start_mining(game: &Game<'_>, ore: u8, limit: u16, running: bool) -> Result<(), String> {
    let language = language();
    if !allowed(game) {
        return Err(language
            .text(
                "Сначала подтвердите экспериментальные функции",
                "Accept experimental features first",
            )
            .into());
    }
    let mut job = mining::Job::new(ore, limit).ok_or_else(|| {
        language
            .text(
                "Недопустимая руда или количество",
                "Invalid ore or block limit",
            )
            .to_owned()
    })?;
    let player = game.native_player().ok_or_else(|| {
        language
            .text("Сначала войдите в мир", "Join a world first")
            .to_owned()
    })?;
    if game.remote.is_none() {
        return Err(language
            .text("Подключитесь к серверу", "Connect to a server first")
            .into());
    }
    if running && !mining_tool(game, None) {
        return Err(language
            .text(
                "Возьмите подходящий инструмент (кирка, топор, лопата) с запасом прочности",
                "Hold a suitable tool (pickaxe, axe, shovel) with durability remaining",
            )
            .into());
    }
    if running && !room_for_mining(game, job.ore) {
        return Err(language
            .text("Освободите слот в инвентаре", "Free an inventory slot")
            .into());
    }
    job.active = running;
    state(|s| {
        cancel_pending(s);
        s.scope = scope(game).unwrap();
        s.mine_aim = None;
        s.mining = Some(job);
        s.nav
            .begin(model::block([player.x, player.y, player.z]), running);
        s.nav.precise = true;
        if let Err(error) = request(game, s) {
            pause_state(s);
            return Err(error);
        }
        Ok(())
    })
}
fn resume_mining(game: &Game<'_>) -> Result<(), String> {
    if !allowed(game) {
        return Err(language()
            .text(
                "Сначала подтвердите экспериментальные функции",
                "Accept experimental features first",
            )
            .into());
    }
    state(|s| {
        if scope(game).as_deref() != Some(s.scope.as_str()) {
            return Err(language()
                .text(
                    "Мир изменился; задайте новую задачу",
                    "World changed; start a new task",
                )
                .into());
        }
        let Some(job) = s.mining.as_mut() else {
            return Ok(());
        };
        if !mining_tool(game, None) || !room_for_mining(game, job.ore) {
            return Err(language()
                .text(
                    "Нужен исправный инструмент и свободный слот",
                    "A usable tool and a free slot are required",
                )
                .into());
        }
        job.resume();
        s.nav.running = job.active;
        s.mine_aim = None;
        if !job.active {
            return Ok(());
        }
        if job.phase == mining::Phase::Confirming {
            job.started_ms = s.started.elapsed().as_millis() as u64;
            s.nav.status = model::Status::Mining;
            return Ok(());
        }
        cancel_pending(s);
        s.nav.replan();
        request(game, s)
    })
}
/// Whether nothing but air lies between the eye and the middle of `target`.
fn in_plain_sight(game: &Game<'_>, ore: u8, eye: [f64; 3], target: model::Pos) -> bool {
    let end = model::center(target);
    let steps = (model::distance(eye, end) / 0.2).ceil() as i32;
    (1..steps).all(|i| {
        let t = f64::from(i) / f64::from(steps);
        let at = [
            (eye[0] + (end[0] - eye[0]) * t).floor() as i32,
            (eye[1] + (end[1] - eye[1]) * t).floor() as i32,
            (eye[2] + (end[2] - eye[2]) * t).floor() as i32,
        ];
        at == target || mining_block(game, ore, at).cell == model::Cell::Air
    })
}
/// The block of the job's kind to dig next from where the player stands: within reach, in plain
/// sight, safe to break, beside the one just broken. `None` when the player has to move.
fn chain_target(game: &Game<'_>, job: &mining::Job, position: [f64; 3]) -> Option<model::Pos> {
    // A job that digs a route (a tunnel, a bridge) or clears the way has its own order.
    if job.ore == u8::MAX
        || job.blocker
        || crate::game::claim_borders_farming::Crop::from_code(job.ore).is_some()
    {
        return None;
    }
    let eye = game.eye_position()?;
    let base = model::block(eye);
    let mut found = Vec::new();
    for dy in -4..=4 {
        for dz in -4..=4 {
            for dx in -4..=4 {
                let at = [base[0] + dx, base[1] + dy, base[2] + dz];
                if job.excluded.contains(&at)
                    || model::distance(model::center(at), eye) > 4.2
                    || !mining_block(game, job.ore, at).ore
                {
                    continue;
                }
                if mining::safe_target(at, position, &mut |p| mining_block(game, job.ore, p))
                    && in_plain_sight(game, job.ore, eye, at)
                {
                    found.push(at);
                }
            }
        }
    }
    mining::next_in_reach(job.last, eye, &found)
}
fn mining_tick(game: &Game<'_>, s: &mut State, position: [f64; 3]) {
    let Some(job) = s.mining.as_mut().filter(|j| j.active) else {
        return;
    };
    let now = s.started.elapsed().as_millis() as u64;
    // The inventory is full of the mined kind: unload into the storage (when one is set), then go
    // on from where the trip began. Nothing else is done while a trip runs.
    if crate::game::claim_borders_haul_view::active()
        // No free slot at all also counts: drops of other kinds (sticks, saplings) could not be
        // picked up, and the miner went back and forth trying. Not for a route's dig of one
        // block: no trip to the storage in the middle of a route.
        || (job.ore != u8::MAX
            && (!room_for_mining(game, job.ore) || !free_slot(game))
            && crate::game::claim_borders_haul_view::begin(game, job.ore))
    {
        job.wait = Some(mining::Wait::Storage);
        job.started_ms = now;
        return;
    }
    if job.wait == Some(mining::Wait::Storage) {
        job.wait = None;
    }
    match job.phase {
        mining::Phase::Searching => {
            // Anything of the kind within reach and in sight is dug at once, from where the
            // player stands: no route is needed for it, and none can exist when the player is
            // on a ridge between pits. Walking and digging a way come only when none is in reach.
            if let Some(next) = chain_target(game, job, position) {
                job.select(next);
                job.phase = mining::Phase::Breaking;
                job.started_ms = now;
                s.mine_aim = None;
                s.nav.running = true;
                s.nav.status = model::Status::Mining;
                cancel_pending(s);
                return;
            }
            s.nav.running = true;
            // A search that found nothing is repeated every couple of seconds, not every tick.
            if !s.pending && s.last_query.elapsed() >= Duration::from_millis(1500) {
                s.nav.replan();
            }
        }
        mining::Phase::Approaching => {
            if matches!(s.nav.status, model::Status::NoPath | model::Status::Stuck)
                || job
                    .target
                    .is_none_or(|at| !mining_block(game, job.ore, at).ore)
            {
                job.skip();
                cancel_pending(s);
                s.nav.replan();
            } else if s.nav.status == model::Status::Arrived {
                job.phase = mining::Phase::Breaking;
                job.started_ms = now;
                s.nav.running = true;
                s.nav.status = model::Status::Mining;
                s.mine_aim = None;
            }
        }
        mining::Phase::Breaking => {
            if job
                .target
                .is_some_and(|at| !mining_block(game, job.ore, at).ore)
            {
                job.predicted_missing();
                if job.phase == mining::Phase::Breaking {
                    job.skip();
                    s.nav.replan();
                }
            }
            if job.wait.is_some() || mouse_has_camera() {
                // Waiting for a tool or a free slot, or for the player to let go of the camera,
                // is not a timeout.
                job.started_ms = now;
            } else if now.saturating_sub(job.started_ms) > 30_000 {
                // This block does not break: leave it and look for another.
                job.skip();
                s.nav.replan();
            }
        }
        mining::Phase::Confirming => {
            let waited = now.saturating_sub(job.started_ms);
            // The block has stayed gone for 0.7 s: the server would have put it back by now if
            // it refused, so it is taken as broken without waiting for the answer (which came
            // late on a busy server, and the miner stood for five seconds).
            let gone = job
                .target
                .is_some_and(|t| !mining_block(game, job.ore, t).ore);
            if gone && waited >= 700 {
                let before = job.confirmed;
                job.broken();
                if job.confirmed > before {
                    assist_reset_budget();
                }
                *BLOCKER.lock().unwrap_or_else(|e| e.into_inner()) = None;
                if !job.active {
                    s.nav.running = false;
                } else if job.phase == mining::Phase::Searching {
                    s.nav.replan();
                }
            } else if waited > 2_500 {
                // No answer from the server: count it as a refusal (tried again, then left).
                job.refuse();
                job.started_ms = now;
            }
        }
        mining::Phase::Collecting => {
            // First the next block in reach, with no step and no search: a person digging sand
            // does not stop after each block. The drops are picked up when none is in reach.
            if let Some(next) = chain_target(game, job, position) {
                job.select(next);
                job.phase = mining::Phase::Breaking;
                job.started_ms = now;
                job.collecting = None;
                job.arrived_ms = None;
                s.mine_aim = None;
                s.nav.running = true;
                s.nav.status = model::Status::Mining;
                return;
            }
            // Pick up what the broken block dropped before the next block: walk to each
            // item lying near it, give up on one that cannot be reached or taken.
            let here = position;
            let at_of = |d: &horizon_client::entities::ItemEntity| {
                [d.position.x, d.position.y, d.position.z]
            };
            let origin = job.last.map_or(here, model::center);
            let nearest = game
                .dropped
                .iter()
                .filter(|d| {
                    d.stack.count > 0
                        && !job.ignored.contains(&d.id)
                        && model::distance(at_of(d), origin) <= 9.0
                })
                .min_by(|a, b| {
                    model::distance(at_of(a), here).total_cmp(&model::distance(at_of(b), here))
                })
                .map(|d| (d.id, at_of(d)));
            let mut walk = None;
            let mut search = false;
            match nearest {
                None => {
                    job.phase = mining::Phase::Searching;
                    job.target = None;
                    job.collecting = None;
                    job.arrived_ms = None;
                    job.ignored.clear();
                    search = true;
                }
                Some((id, at)) if job.collecting != Some(id) => {
                    job.collecting = Some(id);
                    job.started_ms = now;
                    job.arrived_ms = None;
                    // Within a step of it the item is picked up where the player stands.
                    if model::distance(at, here) > 1.1 {
                        walk = Some(model::block(at));
                    } else {
                        job.arrived_ms = Some(now);
                    }
                }
                Some((id, _)) => {
                    if s.nav.status == model::Status::Arrived && job.arrived_ms.is_none() {
                        job.arrived_ms = Some(now);
                    }
                    let gave_up =
                        matches!(s.nav.status, model::Status::NoPath | model::Status::Stuck)
                            || now.saturating_sub(job.started_ms) > 8_000
                            || job.arrived_ms.is_some_and(|t| now.saturating_sub(t) > 800);
                    if gave_up {
                        job.ignored.push(id);
                        job.collecting = None;
                        job.arrived_ms = None;
                    }
                }
            }
            if search {
                s.nav.running = true;
                if !s.pending {
                    s.nav.replan();
                }
            }
            if let Some(goal) = walk {
                s.nav.begin(goal, true);
                s.nav.precise = true;
                cancel_pending(s);
                if request(game, s).is_err()
                    && let Some(job) = s.mining.as_mut()
                {
                    if let Some(id) = job.collecting.take() {
                        job.ignored.push(id);
                    }
                }
            }
        }
        _ => {
            s.nav.running = false;
            job.active = false;
        }
    }
    if s.mining.as_ref().is_some_and(|j| !j.active) {
        s.nav.running = false;
    }
    let _ = position;
}
pub(super) fn mining_attack(game: &mut Game<'_>, physical: bool) -> bool {
    // Mouse buttons pressed behind a screen (clicks in the inventory) are not the player taking
    // the wheel, nor are the use presses of a task (a door, a potion).
    let grace = input_grace() || !game.native_playing();
    let right = state(|s| s.use_restore).unwrap_or(game.right);
    if (physical || right) && !grace {
        crate::game::claim_borders_workshop_view::pause();
        crate::game::claim_borders_construction_view::pause();
    }
    let Some(player) = game.native_player() else {
        return physical;
    };
    state(|s| {
        if (physical || right) && !grace {
            cancel_pending(s);
            pause_state(s);
            return physical;
        }
        let Some(job) = s
            .mining
            .as_mut()
            .filter(|j| j.active && j.phase == mining::Phase::Breaking)
        else {
            return physical;
        };
        if !game.native_on(claims::ID)
            || !in_world(game)
            || scope(game).as_deref() != Some(s.scope.as_str())
            || game.player.as_ref().is_none_or(|p| {
                p.flying || p.is_passenger || p.is_in_lava() || p.health.health() <= 0.0
            })
        {
            pause_state(s);
            return physical;
        }
        // In the air (a glide, a shove from a hit): wait, no hit, no pause.
        if game.player.as_ref().is_some_and(|p| {
            gliding_in_air(p) || !p.ground.on_ground && !p.fluids.was_touching_water
        }) && live_cell(game, model::block([player.x, player.y, player.z])) != model::Cell::Climb
        {
            return false;
        }
        // Under water the miner swims for breath first, and swims up to the block before a hit.
        if *SURFACING.lock().unwrap_or_else(|e| e.into_inner()) {
            return false;
        }
        if let (Some(target), Some(eye)) = (job.target, game.eye_position()) {
            let middle = model::center(target);
            if model::distance(eye, [middle[0], middle[1] + 0.5, middle[2]]) > 4.4 {
                return false;
            }
        }
        // A screen is open: no hit is made until it is closed, and the job waits for that.
        if !game.native_playing() {
            return physical;
        }
        let Some(target) = job.target else {
            return false;
        };
        if crate::game::claim_borders_haul_view::active() {
            job.wait = Some(mining::Wait::Storage);
            return false;
        }
        // A crop is broken with the item with the most Fortune, when there is one: it drops
        // more. Without one, whatever is in hand does (a crop needs no tool).
        if crate::game::claim_borders_farming::Crop::from_code(job.ore).is_some()
            && !job.blocker
            && let Some(slot) = fortune_slot(game)
        {
            let seeds = |h: usize| {
                game.stack_in(h).is_some_and(|s| {
                    crate::game::claim_borders_farming::Crop::ALL
                        .iter()
                        .any(|c| c.seed() == s.item.as_ref())
                })
            };
            let seeds: Vec<bool> = (0..9).map(seeds).collect();
            if let Tool::Wait = hold_slot(game, slot, &|h| seeds[h]) {
                return false;
            }
        }
        if crate::game::claim_borders_farming::Crop::from_code(job.ore).is_none() && !job.blocker {
            match ensure_tool(game, Some(target)) {
                Tool::Ready => {}
                Tool::Wait => return false,
                // No tool for this block: wait for one (the player picks one up, repairs one)
                // rather than stopping; the job goes on by itself.
                Tool::Missing => {
                    job.wait = Some(mining::Wait::Tool);
                    return false;
                }
            }
        }
        // A route's dig of one block goes on with a full inventory (its drop stays on the
        // ground); it waited for room for good, standing still.
        if job.ore != u8::MAX && !room_for_mining(game, job.ore) {
            job.wait = Some(mining::Wait::Inventory);
            return false;
        }
        job.wait = None;
        // The player moved the mouse: the camera is theirs for a moment; the miner waits and
        // aims again from wherever it is left (a turn by another task is not the mouse).
        if mouse_has_camera() {
            return physical;
        }
        let position = [player.x, player.y, player.z];
        if !mining::safe_target(target, position, &mut |at| mining_block(game, job.ore, at)) {
            job.skip();
            s.mine_aim = None;
            s.nav.replan();
            return false;
        }
        let Some(eye) = game.eye_position() else {
            pause_state(s);
            return false;
        };
        // A crop is low (carrots and beetroot barely reach half a block): its middle is aimed
        // at low down, and when a neighbouring crop or the farmland takes the ray, other points
        // of it are tried from where the player stands. Giving up at the first miss sent the
        // farm walking round the crop from one spot to the next.
        let crop = crate::game::claim_borders_farming::Crop::from_code(job.ore).is_some();
        let aim = if crop { crop_aim(target) } else { 0 };
        let point = if crop {
            CROP_AIM[usize::from(aim)]
        } else {
            [0.5, 0.5, 0.5]
        };
        let center = [
            f64::from(target[0]) + point[0],
            f64::from(target[1]) + point[1],
            f64::from(target[2]) + point[2],
        ];
        // The head comes round to the block as a person's does, not in one jump; the swing
        // starts once it points at it.
        let (yaw, pitch) = (model::yaw_to(eye, center), model::pitch_to(eye, center));
        if model::angle_between(game.yaw, yaw).abs() > 4.0 || (game.pitch - pitch).abs() > 4.0 {
            game.yaw = model::turn_toward(game.yaw, yaw, 30.0);
            game.pitch = model::turn_toward(game.pitch, pitch, 24.0);
            // This turn is the miner's own, not the player moving the camera (which pauses).
            s.mine_aim = Some([game.yaw, game.pitch]);
            return false;
        }
        game.yaw = yaw;
        game.pitch = pitch;
        let hit = if game.entity_targeted() {
            None
        } else {
            game.pick()
        };
        match hit {
            Some(hit) if hit.pos == target => {}
            // Grass or leaves are in the way: break them first, then come back to the block.
            Some(hit)
                if job.blockers < 6
                    && mining::clearable(&game.native_block(hit.pos))
                    && !job.blocker =>
            {
                *BLOCKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(hit.pos);
                job.clear_blocker(hit.pos);
                s.mine_aim = None;
                return false;
            }
            _ if crop && usize::from(aim) + 1 < CROP_AIM.len() => {
                *CROP_AIM_AT.lock().unwrap_or_else(|e| e.into_inner()) = Some((target, aim + 1));
                s.mine_aim = None;
                return false;
            }
            _ => {
                job.skip();
                s.mine_aim = None;
                s.nav.replan();
                return false;
            }
        }
        s.mine_aim = Some([game.yaw, game.pitch]);
        true
    })
}
/// Points of a crop block aimed at in turn (low down, where every ripe crop has its shape).
const CROP_AIM: [[f64; 3]; 7] = [
    [0.5, 0.25, 0.5],
    [0.5, 0.12, 0.5],
    [0.3, 0.2, 0.3],
    [0.7, 0.2, 0.7],
    [0.3, 0.2, 0.7],
    [0.7, 0.2, 0.3],
    [0.5, 0.4, 0.5],
];
/// The crop being aimed at and which of its points.
static CROP_AIM_AT: Mutex<Option<(model::Pos, u8)>> = Mutex::new(None);
fn crop_aim(target: model::Pos) -> u8 {
    let mut at = CROP_AIM_AT.lock().unwrap_or_else(|e| e.into_inner());
    match *at {
        Some((pos, n)) if pos == target => n,
        _ => {
            *at = Some((target, 0));
            0
        }
    }
}
/// One meal in progress: the slot to go back to, the food bar when it began, and whether the
/// food is already in hand (the slot change needs a tick to reach the server first).
struct Meal {
    original: usize,
    slot: usize,
    level: i32,
    since: Instant,
    armed: bool,
}
static MEAL: Mutex<Option<Meal>> = Mutex::new(None);
/// "Eat on its own" is on unless the player switched it off.
pub(super) fn auto_eat_enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::SETTINGS_ID, "auto_eat")
        .as_deref()
        != Some("false")
}
/// Eats while a task runs and the food bar is low: puts food in hand, holds the use button
/// until the bar rises, then takes the old slot back. True while it holds the button.
fn auto_eat(game: &mut Game<'_>) -> bool {
    use crate::game::claim_borders_auto_eat as eat;
    let level = game.player.as_ref().map(|p| p.health.food.food as i32);
    // Eating is not only for tasks: by hand it waits while the attack or the use button is held
    // (a fight, a block being broken, a bow drawn) and gives way if one is pressed during a meal.
    let running = level.is_some()
        && game.native_on(claims::ID)
        && game.native_playing()
        && auto_eat_enabled(game)
        && (control().0 || !(game.left || game.right));
    let mut meal = MEAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(level) = level else {
        *meal = None;
        return false;
    };
    if let Some(m) = meal.as_mut() {
        let done = level > m.level
            || level >= 20
            || !running
            || m.since.elapsed() > Duration::from_millis(eat::EAT_TIMEOUT_MS);
        if done {
            game.selected = m.original;
            *meal = None;
            return false;
        }
        if !m.armed {
            m.armed = game.selected == m.slot;
            game.selected = m.slot;
            return true;
        }
        let physical = game.right;
        state(|s| s.use_restore = Some(physical));
        game.right = true;
        return true;
    }
    if !running || !eat::hungry(level, true) {
        return false;
    }
    let foods: Vec<eat::Food> = (0..36)
        .filter_map(|slot| {
            let stack = slot_stack(game, slot)?;
            let (nutrition, _) = game.res.items().get(&stack.item)?.food?;
            Some(eat::Food {
                slot,
                item: stack.item.to_string(),
                nutrition: nutrition as i32,
            })
        })
        .collect();
    let Some(food) = eat::choose(&foods, level).cloned() else {
        return false;
    };
    if food.slot >= 9 {
        // Bring it into the hotbar first (one swap, rate limited), eat it next tick.
        static LAST_SWAP: Mutex<Option<Instant>> = Mutex::new(None);
        let mut last = LAST_SWAP.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_none_or(|t| t.elapsed() >= Duration::from_millis(1000)) {
            let spare = (0..9)
                .filter(|h| *h != game.selected)
                .find(|h| game.stack_in(*h).is_none())
                .or_else(|| (0..9).rev().find(|h| *h != game.selected));
            if let Some(spare) = spare {
                *last = Some(Instant::now());
                game.remote_menu_ops(&[(
                    Some(food.slot),
                    spare as i32,
                    horizon_client::menu::Input::Swap,
                )]);
            }
        }
        return false;
    }
    *meal = Some(Meal {
        original: game.selected,
        slot: food.slot,
        level,
        since: Instant::now(),
        armed: false,
    });
    game.selected = food.slot;
    true
}
/// Ticks of water breathing left (Water Breathing or Conduit Power; endless counts as a lot).
pub(super) fn breathing_ticks(game: &Game<'_>) -> i32 {
    ["water_breathing", "conduit_power"]
        .iter()
        .filter_map(|id| game.potions.effects.get(id))
        .map(|e| if e.duration < 0 { i32::MAX } else { e.duration })
        .max()
        .unwrap_or(0)
}
/// A potion of Water Breathing in the inventory (normal or long).
fn breathing_potion(game: &Game<'_>) -> Option<usize> {
    (0..36).find(|slot| {
        slot_stack(game, *slot).is_some_and(|st| {
            &*st.item == "minecraft:potion"
                && matches!(
                    st.potion(),
                    Some("minecraft:water_breathing" | "minecraft:long_water_breathing")
                )
        })
    })
}
/// Breathing is about to run out (or is not there) in the water during a task.
const BREATH_RENEW: i32 = 200;
/// A potion being drunk: the slot to go back to, the potion's slot, since when, and whether it
/// is in hand already.
struct Drink {
    original: usize,
    slot: usize,
    since: Instant,
    armed: bool,
}
static DRINK: Mutex<Option<Drink>> = Mutex::new(None);
/// In the water during a task, with Water Breathing missing or about to end: a potion of it
/// from the inventory is drunk (into the hand, use held until the effect is there), so the bot
/// need not swim up for air. True while it holds the button.
fn auto_drink(game: &mut Game<'_>) -> bool {
    let mut drink = DRINK.lock().unwrap_or_else(|e| e.into_inner());
    if drink.is_none() {
        merge_bottles(game);
    }
    if let Some(d) = drink.as_mut() {
        if breathing_ticks(game) > BREATH_RENEW * 2
            || d.since.elapsed() > Duration::from_millis(4000)
            || !game.native_playing()
        {
            game.selected = d.original;
            *drink = None;
            return false;
        }
        if !d.armed {
            d.armed = game.selected == d.slot;
            game.selected = d.slot;
            return true;
        }
        let physical = game.right;
        state(|s| s.use_restore = Some(physical));
        game.right = true;
        return true;
    }
    let wet = game
        .player
        .as_ref()
        .is_some_and(|b| b.fluids.was_touching_water || b.fluids.was_eye_in_water);
    if !wet || !task_running() || breathing_ticks(game) >= BREATH_RENEW {
        return false;
    }
    let Some(slot) = breathing_potion(game) else {
        return false;
    };
    if slot >= 9 {
        static LAST_SWAP: Mutex<Option<Instant>> = Mutex::new(None);
        let mut last = LAST_SWAP.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_none_or(|t| t.elapsed() >= Duration::from_millis(1000)) {
            let spare = (0..9)
                .filter(|h| *h != game.selected)
                .find(|h| game.stack_in(*h).is_none())
                .or_else(|| (0..9).rev().find(|h| *h != game.selected));
            if let Some(spare) = spare {
                *last = Some(Instant::now());
                game.remote_menu_ops(&[(
                    Some(slot),
                    spare as i32,
                    horizon_client::menu::Input::Swap,
                )]);
            }
        }
        return false;
    }
    *drink = Some(Drink {
        original: game.selected,
        slot,
        since: Instant::now(),
        armed: false,
    });
    game.selected = slot;
    true
}
/// A drunk potion leaves a glass bottle in its slot, one per slot: while a task runs they are
/// put together into one stack (pick one up, put it on another, anything left goes back), a
/// pair a second, so they do not fill the inventory.
fn merge_bottles(game: &mut Game<'_>) {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if !task_running() || !game.native_playing() {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
        return;
    }
    let bottles: Vec<usize> = (0..36)
        .filter(|slot| {
            slot_stack(game, *slot)
                .is_some_and(|st| &*st.item == "minecraft:glass_bottle" && st.count < 64)
        })
        .collect();
    let [from, onto, ..] = bottles[..] else {
        return;
    };
    *last = Some(Instant::now());
    use horizon_client::menu::Input;
    game.remote_menu_ops(&[
        (Some(from), 0, Input::Pickup),
        (Some(onto), 0, Input::Pickup),
        (Some(from), 0, Input::Pickup),
    ]);
}
/// Auto Tools: while the attack button is held by hand and no task runs, the hotbar tool that
/// fits the block under the crosshair is selected (the held one stays if it fits).
pub(super) fn auto_tool(game: &mut Game<'_>) {
    if !game.left || game.right || control().0 || !game.native_playing() {
        return;
    }
    let Some(at) = game.pick().map(|hit| hit.pos) else {
        return;
    };
    if game
        .res
        .facts()
        .get(game.world.block(at[0], at[1], at[2]))
        .hardness
        < 0.0
    {
        return;
    }
    if let Some(slot) = tool_slot(game, Some(at), false).filter(|slot| *slot < 9)
        && game.selected != slot
    {
        game.selected = slot;
    }
}
/// The mining job while a trip to the storage walks: a walk starts the navigator over, which
/// would drop the job, so it waits here and is put back when the trip is over.
static HAUL_JOB: Mutex<Option<mining::Job>> = Mutex::new(None);
/// Puts the right tool for the block at `at` in hand; true once it is there (or none is
/// needed), false while a swap is on its way.
pub(super) fn tool_for(game: &mut Game<'_>, at: model::Pos) -> bool {
    !matches!(ensure_tool(game, Some(at)), Tool::Wait)
}
pub(super) fn haul_stash() {
    state(|s| {
        if let Some(job) = s.mining.take() {
            *HAUL_JOB.lock().unwrap_or_else(|e| e.into_inner()) = Some(job);
        }
    });
}
/// The trip to the storage is over: mining goes on from where it is (`ok`), or idles with the
/// inventory full.
pub(super) fn haul_ended(ok: bool) {
    let stashed = HAUL_JOB.lock().unwrap_or_else(|e| e.into_inner()).take();
    state(|s| {
        if let Some(job) = stashed {
            s.mining = Some(job);
        }
        if let Some(job) = s.mining.as_mut() {
            if ok {
                job.wait = None;
                job.phase = mining::Phase::Searching;
                job.target = None;
                job.collecting = None;
                job.arrived_ms = None;
            } else {
                job.wait = Some(mining::Wait::Inventory);
            }
        }
    });
}
/// Whether a task (walking, mining, a build, the farm) is running: nothing is clicked for the
/// player then.
pub(super) fn task_running() -> bool {
    control().0
}
/// Holds a mouse button for this tick, on top of what the player holds; the player's own state
/// is put back when the input has been handled.
pub(super) fn press_for_click(game: &mut Game<'_>, right: bool) {
    if right {
        let physical = game.right;
        state(|s| {
            s.use_restore.get_or_insert(physical);
        });
        game.right = true;
    } else {
        let physical = game.left;
        state(|s| {
            s.attack_restore.get_or_insert(physical);
        });
        game.left = true;
    }
}
pub(super) fn mining_attack_begin(game: &mut Game<'_>) {
    crate::game::claim_borders_helpers::tick(game);
    leg_frame(game);
    face(game);
    sprint_frame(game);
    crate::game::claim_borders_schematic_view::keys(game);
    crate::game::claim_borders_haul_view::frame(game);
    crate::game::claim_borders_resume::frame(game);
    if auto_eat(game) || auto_drink(game) {
        return;
    }
    // The buttons the player really holds: a task's own presses this tick (breaking a shulker
    // box, a click on a door) are not the player taking over, which pauses mining.
    let physical = state(|s| s.attack_restore).unwrap_or(game.left);
    let attack = match pillar_frame(game) {
        Some(attack) => attack,
        None => mining_attack(game, physical),
    };
    if attack != physical {
        state(|s| s.attack_restore = Some(physical));
        game.left = attack;
    }
    let physical_right = game.right;
    if crate::game::claim_borders_construction_view::input(game) && !physical_right {
        state(|s| s.use_restore = Some(physical_right));
        game.right = true;
    }
    // Last: the auto-clicker's presses must not be read as the player taking the wheel.
    crate::game::claim_borders_helpers::autoclick(game);
}
pub(super) fn mining_attack_end(game: &mut Game<'_>) {
    if let Some(physical) = state(|s| s.attack_restore.take()) {
        game.left = physical;
    }
    if let Some(physical) = state(|s| s.use_restore.take()) {
        game.right = physical;
    }
}
pub(super) fn mining_sent(game: &Game<'_>, packet: &horizon_protocol::packets::play::Serverbound) {
    if let horizon_protocol::packets::play::Serverbound::PlayerAction(p) = packet {
        state(|s| {
            if let Some(job) = s.mining.as_mut() {
                let before = job.phase;
                job.sent(p.pos, p.action, p.sequence);
                if before != job.phase && job.phase == mining::Phase::Confirming {
                    job.started_ms = s.started.elapsed().as_millis() as u64;
                }
            }
        });
    }
    let _ = game;
}
pub(super) fn mining_ack(game: &Game<'_>, sequence: i32) {
    state(|s| {
        if scope(game).as_deref() != Some(s.scope.as_str()) {
            return;
        }
        if let Some(job) = s.mining.as_mut()
            && let Some(target) = job.target
        {
            let before = job.confirmed;
            job.acknowledge(sequence, mining_block(game, job.ore, target).ore);
            // A block that was really mined proves the way works: the budget of dug routes and
            // failures counts only those without a success in between, so a long mining job is
            // not stopped by the total number of routes it needed.
            if job.confirmed > before {
                assist_reset_budget();
            }
            *BLOCKER.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if !job.active {
                s.nav.running = false;
            } else if job.phase == mining::Phase::Searching {
                s.nav.replan();
            }
        }
    });
}
fn request(game: &Game<'_>, s: &mut State) -> Result<(), String> {
    let Some(player) = game.native_player() else {
        return Err(language()
            .text("Сначала войдите в мир", "Join a world first")
            .into());
    };
    let Some(goal) = s.nav.goal else {
        return Ok(());
    };
    // A jump is not a valid standing start for the planner. Wait for landing (in the water or on
    // a ladder there is no landing to wait for: plan from there).
    if s.nav.running
        && game
            .player
            .as_ref()
            .is_some_and(|p| !p.ground.on_ground && !p.fluids.was_touching_water && !p.swimming)
        && live_cell(game, model::block([player.x, player.y, player.z])) != model::Cell::Climb
    {
        s.nav.status = model::Status::Waiting;
        return Ok(());
    }
    if s.worker.is_none() {
        s.worker = Some(Worker::spawn()?);
    }
    cancel_pending(s);
    let center = (
        model::block([player.x, player.y, player.z])[0].div_euclid(16),
        model::block([player.x, player.y, player.z])[2].div_euclid(16),
    );
    let start = model::standing_block(
        [player.x, player.y, player.z],
        &game.native_block(model::block([player.x, player.y, player.z])),
    );
    let wide = model::wide_needed(start, goal);
    // The chunks the search may read: ten around for the wide one (160 blocks), two otherwise.
    let span = if wide { 10 } else { 2 };
    let chunks = (-span..=span)
        .flat_map(|z| (-span..=span).map(move |x| (center.0 + x, center.1 + z)))
        .filter_map(|at| game.world.chunks.get(&at).map(|c| (at, c.clone())))
        .collect();
    let hints = s
        .mining
        .as_ref()
        .filter(|j| {
            j.phase == mining::Phase::Searching
                && j.ore != u8::MAX
                && crate::game::claim_borders_farming::Crop::from_code(j.ore).is_none()
        })
        .map(|j| {
            if j.ore == mining::ANY {
                let mask = mining::selection();
                (0..crate::game::claim_borders_experimental::ORES.len() as u8)
                    .filter(|i| mask & (1 << i) != 0)
                    .flat_map(crate::game::claim_borders_ore_view::marks_for)
                    .collect()
            } else {
                crate::game::claim_borders_ore_view::marks_for(j.ore)
            }
        })
        .unwrap_or_default();
    let query = Query {
        flat: s.flat,
        level: s.level,
        // Breaking a way to the block is allowed with any digging tool in the inventory (the
        // right one is taken per block); a full inventory only leaves drops on the ground.
        dig: !s.flat && tool_slot(game, None, false).is_some(),
        place: !s.flat && crate::game::claim_borders_construction_view::has_building_blocks(game),
        force: s.force_dig,
        hints,
        mine: s
            .mining
            .as_ref()
            .filter(|j| j.phase == mining::Phase::Searching)
            .map(|j| (j.ore, j.excluded.clone())),
        token: s.token,
        start,
        goal,
        wide,
        radius: if s.nav.follow.is_some() { 1 } else { 0 },
        // A long drop (when nothing else leads down) costs a few health points: the shared
        // rule weighs health and the fall protection worn. Eating gets the points back.
        // Elytra glide any drop down (the walker spreads them on the way).
        drop: if elytra_ready(game) {
            model::ELYTRA_DROP
        } else {
            model::drop_limit(
                game.player.as_ref().map_or(0.0, |p| p.health.health()),
                fall_protection(game),
            )
        },
        chunks,
        resources: game.res.clone(),
    };
    // The walker's own check of each step on this thread goes by the same drop limit.
    model::set_drop_limit(query.drop);
    match s.worker.as_ref().unwrap().tx.try_send(query) {
        Ok(()) => {}
        // The planner still holds a query that was cancelled a moment ago: a running task asks
        // again on a following tick (as it waits) instead of pausing; a preview, which nothing
        // asks again for, says so.
        Err(mpsc::TrySendError::Full(_)) if s.nav.running => {
            s.last_query = Instant::now();
            s.nav.status = model::Status::Waiting;
            return Ok(());
        }
        Err(_) => {
            return Err(language()
                .text(
                    "Планировщик занят; повторите запрос",
                    "Planner busy; try again",
                )
                .to_owned());
        }
    }
    s.pending = true;
    s.last_query = Instant::now();
    s.nav.status = model::Status::Planning;
    Ok(())
}
/// The "own block" mining target's blocks, from the settings, when none are set yet (after a
/// restart of the game).
pub(super) fn load_custom_blocks(game: &Game<'_>) {
    if mining::custom().is_empty()
        && let Some(saved) = game.native_setting(claims::SETTINGS_ID, "mining_custom")
    {
        mining::set_custom(
            saved
                .split(',')
                .filter(|b| !b.is_empty())
                .map(str::to_owned)
                .collect(),
        );
    }
}
pub(super) fn start(game: &Game<'_>, goal_request: Goal, running: bool) -> Result<(), String> {
    load_custom_blocks(game);
    let result = start_task(game, goal_request, running);
    if result.is_ok() {
        crate::game::claim_borders_resume::note_start(game, goal_request, running);
    }
    result
}
fn follow_request(goal: Goal) -> bool {
    matches!(goal, Goal::Follow(_))
}
fn start_task(game: &Game<'_>, goal_request: Goal, running: bool) -> Result<(), String> {
    drop_assist();
    assist_reset_budget();
    if let Goal::Farm(config) = goal_request {
        return crate::game::claim_borders_workshop_view::start_farm(game, config, !running);
    }
    if let Goal::Drain(config) = goal_request {
        return crate::game::claim_borders_workshop_view::start_drain(game, config, !running);
    }
    crate::game::claim_borders_workshop_view::reset();
    if let Goal::Construct(mut config) = goal_request {
        config.direction = crate::game::claim_borders_construction::Direction::from_yaw(game.yaw);
        return crate::game::claim_borders_construction_view::start(game, config, !running);
    }
    crate::game::claim_borders_construction_view::reset();
    if let Goal::Mine {
        ore,
        targets,
        limit,
    } = goal_request
    {
        mining::select(targets);
        return start_mining(game, ore, limit, running);
    }
    let language = language();
    if !allowed(game) {
        return Err(language
            .text(
                "Сначала подтвердите экспериментальные функции",
                "Accept experimental features first",
            )
            .into());
    }
    let player = game.native_player().ok_or_else(|| {
        language
            .text("Сначала войдите в мир", "Join a world first")
            .to_owned()
    })?;
    let goal = match goal_request {
        Goal::Farm(_) | Goal::Drain(_) => unreachable!(),
        Goal::Construct(_) => unreachable!(),
        Goal::Mine { .. } => unreachable!(),
        Goal::Coordinates(at) => at,
        Goal::Follow(uuid) => model::block(
            crate::game::claim_borders_client::entity_position(game, uuid)
                .filter(|p| p.iter().all(|v| v.is_finite()))
                .ok_or_else(|| {
                    language
                        .text("Игрок вне зоны видимости", "Player is not loaded nearby")
                        .to_owned()
                })?,
        ),
        Goal::Waypoint(id) => {
            let waypoint = crate::game::claim_borders_client::own_waypoints()
                .into_iter()
                .find(|w| w.id == id && w.dimension == player.dimension)
                .ok_or_else(|| {
                    language
                        .text(
                            "Метка недоступна в этом измерении",
                            "Waypoint unavailable in this dimension",
                        )
                        .to_owned()
                })?;
            model::block(waypoint.position)
        }
    };
    // A goal given as a block (the ground under the feet, a waypoint saved on it) is walked to
    // where one stands at it: in its column, up to three blocks up or down.
    let goal = if follow_request(goal_request) {
        goal
    } else {
        model::standing_goal(goal, &mut |at| live_cell(game, at)).unwrap_or(goal)
    };
    if goal[0].unsigned_abs() > 29_999_984
        || goal[2].unsigned_abs() > 29_999_984
        || goal[1].unsigned_abs() > 2048
    {
        return Err(language
            .text(
                "Координаты вне допустимых границ",
                "Coordinates outside supported bounds",
            )
            .into());
    }
    let scope = scope(game).unwrap();
    let follow = if let Goal::Follow(uuid) = goal_request {
        Some(uuid)
    } else {
        None
    };
    // A walk to a place (not following) is remembered once it arrives, and one walked before
    // between here and there is followed again.
    if follow.is_none() && running {
        state(|s| {
            s.mining = None;
            s.mine_aim = None;
            s.stuck_tried = false;
            s.force_dig = false;
            cancel_pending(s);
            s.nav.follow = None;
        });
        return workshop_walk(game, goal, false, false);
    }
    state(|s| {
        s.mining = None;
        s.mine_aim = None;
        s.stuck_tried = false;
        s.force_dig = false;
        cancel_pending(s);
        s.scope = scope;
        s.nav.begin(goal, running);
        s.nav.follow =
            follow.map(|uuid| model::Follow::new(uuid, s.started.elapsed().as_millis() as u64));
        if running && let Some(follow) = s.nav.follow.as_mut() {
            let target = crate::game::claim_borders_client::entity_position(game, follow.uuid);
            if matches!(
                follow.update(
                    [player.x, player.y, player.z],
                    target,
                    s.started.elapsed().as_millis() as u64
                ),
                model::FollowDecision::Hold(_)
            ) {
                s.nav.status = model::Status::Holding;
                return Ok(());
            }
        }
        if let Err(e) = request(game, s) {
            s.nav.pause();
            return Err(e);
        }
        Ok(())
    })
}
pub(super) fn pause() {
    crate::game::claim_borders_workshop_view::pause();
    crate::game::claim_borders_construction_view::pause();
    state(|s| {
        if s.nav.running || s.mining.as_ref().is_some_and(|j| j.active) {
            cancel_pending(s);
            pause_state(s);
        } else {
            release_assist(s);
        }
    });
}
pub(super) fn resume(game: &Game<'_>) -> Result<(), String> {
    if crate::game::claim_borders_workshop_view::exists() {
        return crate::game::claim_borders_workshop_view::resume(game);
    }
    if crate::game::claim_borders_construction_view::exists() {
        return crate::game::claim_borders_construction_view::resume(game);
    }
    if state(|s| s.mining.is_some()) {
        return resume_mining(game);
    }
    let goal = state(|s| {
        if scope(game).as_deref() == Some(s.scope.as_str()) {
            s.nav
                .follow
                .as_ref()
                .map(|f| Goal::Follow(f.uuid))
                .or_else(|| s.nav.goal.map(Goal::Coordinates))
        } else {
            None
        }
    });
    let goal = goal.ok_or_else(|| {
        language()
            .text("Сначала задайте цель", "Set a destination first")
            .to_owned()
    })?;
    start(game, goal, true)
}
pub(super) fn tick(game: &Game<'_>) {
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    let Some(scope) = scope(game) else {
        reset();
        return;
    };
    let Some(p) = game.native_player() else {
        return;
    };
    let position = [p.x, p.y, p.z];
    state(|s| {
        if s.scope.is_empty() {
            return;
        }
        if s.scope != scope {
            *s = State::default();
            return;
        }
        let answers: Vec<Answer> = s
            .worker
            .as_ref()
            .map(|w| std::iter::from_fn(|| w.rx.try_recv().ok()).collect())
            .unwrap_or_default();
        {
            for answer in answers {
                if answer.token == s.token && s.pending {
                    s.pending = false;
                    s.mine_stats = answer.stats;
                    // Nothing in reach: a tall trunk overhead is climbed to on a column (and a
                    // column climbed is taken down once nothing more is in reach from its top).
                    if answer.target.is_none()
                        && let Some(job) = s.mining.as_ref().filter(|j| {
                            j.active && j.phase == mining::Phase::Searching && j.ore != u8::MAX
                        })
                    {
                        pillar_consider(game, job.ore, position);
                    }
                    // Blocks of the kind are around but none can be mined yet: say why.
                    s.mine_note = answer
                        .stats
                        .filter(|st| st.seen > 0 && answer.target.is_none())
                        .map(|st| {
                            let l = language();
                            format!(
                                "{} · {}: {}/{} · {}: {}/{}/{}",
                                l.text("Поиск руды…", "Finding ore…"),
                                l.text("видно/снаружи", "seen/open"),
                                st.seen,
                                st.candidates,
                                l.text("нельзя/нет опоры/нет пути", "unsafe/no footing/no route"),
                                st.unsafe_,
                                st.no_stand,
                                st.no_path
                            )
                        })
                        .unwrap_or_default();
                    let off = model::distance(position, model::center(answer.start));
                    let wet = game
                        .player
                        .as_ref()
                        .is_some_and(|p| p.fluids.was_touching_water || p.swimming);
                    if off > 64.0 {
                        // Far from where the route was asked for (a teleport): a pause.
                        s.nav.running = false;
                        s.nav.status = model::Status::Paused;
                        if let Some(job) = s.mining.as_mut() {
                            job.active = false;
                        }
                    } else if off > if wet { 3.0 } else { 1.5 } && s.nav.running {
                        // Moved on while it was planned (following a player, after a jump):
                        // plan again from here instead of stopping.
                        s.nav.replan();
                    } else {
                        let walking_complete = answer.path.as_ref().is_some_and(|p| p.complete);
                        let route = answer.dig_plan.filter(|r| !r.steps.is_empty());
                        let running = s.nav.running || s.mining.as_ref().is_some_and(|j| j.active);
                        let mut handled = false;
                        if s.mining
                            .as_ref()
                            .is_some_and(|j| j.phase == mining::Phase::Searching)
                        {
                            if let Some(target) = answer.target {
                                s.mining.as_mut().unwrap().select(target);
                                match route {
                                    // The ore is inside the rock: dig to it, or show the way.
                                    Some(route) if running && assist_budget() => {
                                        let job = s.mining.take();
                                        install_assist(s, target, job, route);
                                        handled = true;
                                    }
                                    Some(route) => {
                                        s.preview_route = Some(route_counts(&route));
                                        s.nav.goal = route.steps.last().map(|step| step.to);
                                        s.nav.accept(Some(model::Path {
                                            nodes: route_nodes(&route),
                                            complete: true,
                                        }));
                                        handled = true;
                                    }
                                    None => {
                                        s.nav.goal = answer
                                            .path
                                            .as_ref()
                                            .and_then(|p| p.nodes.last().copied());
                                    }
                                }
                            } else if let Some(job) = s.mining.as_mut() {
                                // Nothing reachable right now: keep looking (the world loads,
                                // blocks change, the player moves) instead of stopping.
                                job.target = None;
                                s.nav.running = true;
                                s.last_query = Instant::now();
                            }
                        } else if s.mining.is_none()
                            && s.nav.follow.is_none()
                            && !assist_active()
                            && (s.force_dig || !walking_complete)
                            && (answer.path.is_none()
                                || s.force_dig
                                || route.as_ref().is_some_and(|r| r.complete))
                        {
                            // Walking cannot get there (or just got stuck): break and
                            // place blocks instead of giving up.
                            if let Some(route) = route {
                                let goal = s.nav.goal.unwrap_or(route.start);
                                s.force_dig = false;
                                if s.nav.running && assist_budget() {
                                    install_assist(s, goal, None, route);
                                } else {
                                    s.preview_route = Some(route_counts(&route));
                                    s.nav.accept(Some(model::Path {
                                        nodes: route_nodes(&route),
                                        complete: route.complete,
                                    }));
                                }
                                handled = true;
                            }
                        }
                        s.force_dig = false;
                        if !handled {
                            if answer.path.is_none() && s.mining.is_none() && s.nav.running {
                                // Retry from the current position with a bounded recovery
                                // budget. Never turn a failed plan into unbounded movement.
                                s.nav.blocked(position);
                            } else {
                                s.nav.accept(answer.path);
                            }
                        }
                    }
                }
            }
        }
        if s.pending && s.last_query.elapsed() > Duration::from_secs(5) {
            cancel_pending(s);
            if s.mining.is_some() {
                // The planner was slow: ask again, do not stop the job.
                s.last_query = Instant::now();
                s.nav.replan();
            } else if s.nav.running {
                // A delayed worker response is not proof that the destination is unreachable.
                // Retry a bounded number of times without reusing the stale snapshot.
                s.nav.blocked(position);
            } else {
                s.nav.status = model::Status::NoPath;
            }
        }
        if (s.nav.running || s.mining.as_ref().is_some_and(|j| j.active)) && !in_world(game) {
            cancel_pending(s);
            pause_state(s);
        }
        if s.nav.running
            && let Some(follow) = s.nav.follow.as_mut()
        {
            let target = crate::game::claim_borders_client::entity_position(game, follow.uuid);
            let decision = follow.update(position, target, s.started.elapsed().as_millis() as u64);
            match decision {
                model::FollowDecision::Hold(goal) => {
                    if s.nav.status != model::Status::Holding {
                        cancel_pending(s);
                        s.nav.replan();
                    }
                    s.nav.goal = Some(goal);
                    s.nav.status = model::Status::Holding;
                }
                model::FollowDecision::Pursue(goal) => {
                    let changed = s.nav.goal.is_none_or(|old| {
                        model::distance(model::center(old), model::center(goal)) >= 2.0
                    });
                    if !s.pending
                        && (matches!(
                            s.nav.status,
                            model::Status::Holding | model::Status::TargetMissing
                        ) || (changed && s.last_query.elapsed() >= Duration::from_millis(250)))
                    {
                        s.nav.goal = Some(goal);
                        s.nav.replan();
                    }
                }
                model::FollowDecision::Missing => {
                    if s.nav.status != model::Status::TargetMissing {
                        cancel_pending(s);
                        s.nav.replan();
                        s.nav.status = model::Status::TargetMissing;
                    }
                }
                model::FollowDecision::Lost => {
                    cancel_pending(s);
                    s.nav.replan();
                    s.nav.running = false;
                    s.nav.status = model::Status::TargetLost;
                }
            }
        }
        mining_tick(game, s, position);
        // A walk that ended in a dead end is not the end of the task: once, ask for a
        // route that breaks and places blocks from where the player stands.
        if s.nav.status == model::Status::Stuck
            && !s.nav.running
            && s.mining.is_none()
            && s.nav.follow.is_none()
            && s.nav.goal.is_some()
            && !s.flat
            && !s.stuck_tried
            && !assist_active()
            && assist_budget()
        {
            s.stuck_tried = true;
            s.force_dig = true;
            s.nav.running = true;
            cancel_pending(s);
            s.nav.replan();
        }
        if s.nav.running
            && s.nav.status == model::Status::Waiting
            // Plans take milliseconds now (a few tens at most): the next one is asked for two
            // ticks later, not after a third of a second standing still. A retry after a failed
            // plan keeps the old pace (the chunks it needs may still be arriving).
            && s.last_query.elapsed()
                > Duration::from_millis(if s.nav.recovering() { 350 } else { 100 })
            && request(game, s).is_err()
        {
            pause_state(s);
        }
    });
    assist_tick(game);
}

// ---------------------------------------------------------------------------------------
// Routes that break and place blocks.
//
// The planner (`model::search_dig`) answers with steps; this executor carries them out one
// at a time with the ordinary drivers: a server-confirmed dig for every block that stands in
// the way, the bridge job for a missing floor, the walking navigator for the rest. After a
// route ends, or fails, the destination is planned again from wherever the player stands, so
// a stop never leaves the bot guessing. Pausing or any manual input drops the route.
// ---------------------------------------------------------------------------------------
enum Stage {
    /// Look at the next step against the live world.
    Next,
    /// Blocks of the current step still to break; `issued` once the dig was started.
    Dig { rest: Vec<model::Pos>, issued: bool },
    /// A straight run of `count` floor blocks is being built by the bridge job.
    Bridge { count: usize },
    /// Walking the steps `index..end` on the navigator.
    Walk { end: usize },
}
struct Assist {
    scope: String,
    /// Where the whole task should end (navigation), or the ore being approached.
    goal: model::Pos,
    /// The ore job waiting for the route to bring the player next to its target.
    mining: Option<mining::Job>,
    route: model::DigPath,
    index: usize,
    stage: Stage,
    stage_ms: u64,
    started: Instant,
}
enum Flow {
    Keep,
    /// The route ran to its end.
    Done,
    /// The world no longer matches the plan; plan again.
    Failed,
    /// The player took over or paused.
    Abort,
    /// Another world: forget everything.
    Drop,
}
/// Failed routes and routes started since the player last gave the order.
static ASSIST_FAILS: AtomicU64 = AtomicU64::new(0);
static ASSIST_ROUTES: AtomicU64 = AtomicU64::new(0);
const ASSIST_MAX_FAILS: u64 = 3;
const ASSIST_MAX_ROUTES: u64 = 24;
fn assist_slot<T>(f: impl FnOnce(&mut Option<Assist>) -> T) -> T {
    static SLOT: Mutex<Option<Assist>> = Mutex::new(None);
    f(&mut SLOT.lock().unwrap_or_else(|e| e.into_inner()))
}
fn assist_active() -> bool {
    assist_slot(|a| a.is_some())
}
fn assist_budget() -> bool {
    ASSIST_FAILS.load(Ordering::Relaxed) < ASSIST_MAX_FAILS
        && ASSIST_ROUTES.load(Ordering::Relaxed) < ASSIST_MAX_ROUTES
}
fn assist_reset_budget() {
    ASSIST_FAILS.store(0, Ordering::Relaxed);
    ASSIST_ROUTES.store(0, Ordering::Relaxed);
}
fn drop_assist() {
    assist_slot(|a| *a = None);
}
/// Called with the state lock held: hands a waiting ore job back, paused.
fn release_assist(s: &mut State) {
    if let Some(a) = assist_slot(Option::take) {
        s.scope = a.scope.clone();
        if let Some(mut job) = a.mining {
            job.active = false;
            s.mining = Some(job);
        } else {
            // Resuming plans toward the real destination, not the step that was under way.
            s.nav.goal = Some(a.goal);
            s.nav.status = model::Status::Paused;
        }
    }
}
fn install_assist(
    s: &mut State,
    goal: model::Pos,
    mining: Option<mining::Job>,
    route: model::DigPath,
) {
    ASSIST_ROUTES.fetch_add(1, Ordering::Relaxed);
    s.nav.running = true;
    s.nav.status = model::Status::Mining;
    s.nav.goal = route.steps.last().map(|step| step.to);
    s.preview_route = None;
    let scope = s.scope.clone();
    assist_slot(|slot| {
        *slot = Some(Assist {
            scope,
            goal,
            mining,
            route,
            index: 0,
            stage: Stage::Next,
            stage_ms: 0,
            started: Instant::now(),
        });
    });
}
fn route_nodes(route: &model::DigPath) -> Vec<model::Pos> {
    std::iter::once(route.start)
        .chain(route.steps.iter().map(|step| step.to))
        .collect()
}
fn route_counts(route: &model::DigPath) -> (u16, u16) {
    (
        route.steps.iter().map(|step| step.dig.len() as u16).sum(),
        route
            .steps
            .iter()
            .filter(|step| step.place.is_some())
            .count() as u16,
    )
}
fn assist_label() -> Option<String> {
    let l = language();
    assist_slot(|slot| {
        slot.as_ref().map(|a| {
            let what = match a.stage {
                Stage::Next => l.text("Анализ пути", "Analysing the route"),
                Stage::Dig { .. } => l.text("Ломаю блоки", "Breaking blocks"),
                Stage::Bridge { .. } => l.text("Строю мост", "Building a bridge"),
                Stage::Walk { .. } => l.text("Иду по проходу", "Walking the passage"),
            };
            format!("{what} · {}/{}", a.index, a.route.steps.len())
        })
    })
}
/// Start the walking navigator on `nodes`, which begin where the player stands.
fn assist_walk(scope: &str, nodes: Vec<model::Pos>) {
    let Some(&goal) = nodes.last() else {
        return;
    };
    state(|s| {
        cancel_pending(s);
        s.scope = scope.to_owned();
        s.nav.begin(goal, true);
        s.nav.precise = true;
        s.nav.accept(Some(model::Path {
            nodes,
            complete: true,
        }));
    });
}
/// Plan again toward the same destination after a route ended or failed.
fn assist_replan(game: &Game<'_>, a: Assist, failed: bool) {
    if failed {
        ASSIST_FAILS.fetch_add(1, Ordering::Relaxed);
    }
    let give_up = !assist_budget();
    let Assist {
        scope,
        goal,
        mining,
        route,
        ..
    } = a;
    // The dig and bridge drivers keep their own state; clear whatever the route left.
    crate::game::claim_borders_construction_view::reset();
    state(|s| {
        cancel_pending(s);
        s.scope = scope;
        if let Some(mut job) = mining {
            // An ore job resumes: next to its ore it breaks it, otherwise it looks again.
            let near = route.steps.last().map(|step| step.to);
            if failed || give_up {
                job.skip();
                if give_up {
                    job.active = false;
                    job.phase = mining::Phase::NoTarget;
                }
            }
            let ready = !failed && job.phase == mining::Phase::Approaching;
            s.mining = Some(job);
            s.mine_aim = None;
            s.nav.begin(near.unwrap_or(goal), true);
            s.nav.precise = true;
            if ready {
                s.nav.running = false;
                s.nav.status = model::Status::Arrived;
            } else if give_up {
                s.nav.running = false;
                s.nav.status = model::Status::NoPath;
            } else {
                s.nav.replan();
            }
            return;
        }
        s.mining = None;
        s.nav.begin(goal, true);
        if give_up {
            s.nav.running = false;
            s.nav.status = model::Status::NoPath;
        } else if request(game, s).is_err() {
            s.nav.replan();
        }
    });
}
fn assist_tick(game: &Game<'_>) {
    let Some(mut a) = assist_slot(Option::take) else {
        return;
    };
    match assist_step(game, &mut a) {
        Flow::Keep => assist_slot(|slot| {
            if slot.is_none() {
                *slot = Some(a);
            }
        }),
        Flow::Done => assist_replan(game, a, false),
        Flow::Failed => assist_replan(game, a, true),
        Flow::Abort => state(|s| {
            cancel_pending(s);
            s.scope = a.scope.clone();
            if let Some(mut job) = a.mining {
                job.active = false;
                s.mining = Some(job);
            } else {
                s.nav.goal = Some(a.goal);
            }
            s.nav.running = false;
            s.nav.status = model::Status::Paused;
        }),
        Flow::Drop => {}
    }
}
fn assist_step(game: &Game<'_>, a: &mut Assist) -> Flow {
    use crate::game::claim_borders_construction as building;
    use crate::game::claim_borders_construction_view as construction;
    if !game.native_on(claims::ID) || scope(game).as_deref() != Some(a.scope.as_str()) {
        return Flow::Drop;
    }
    if !in_world(game) || !allowed(game) {
        return Flow::Abort;
    }
    let Some(p) = game.native_player() else {
        return Flow::Abort;
    };
    let position = [p.x, p.y, p.z];
    let now = a.started.elapsed().as_millis() as u64;
    // A step that makes no progress for 20 seconds (a block that does not break, a walk that
    // does not get there) is planned again; it used to stand for 45 seconds first.
    if now > 300_000 || now.saturating_sub(a.stage_ms) > 20_000 {
        return Flow::Failed;
    }
    match &mut a.stage {
        Stage::Next => {
            if a.index >= a.route.steps.len() {
                return Flow::Done;
            }
            let expected = if a.index == 0 {
                a.route.start
            } else {
                a.route.steps[a.index - 1].to
            };
            if model::distance(position, model::center(expected)) > 1.3 {
                return Flow::Failed;
            }
            let step = a.route.steps[a.index].clone();
            let rest: Vec<_> = step
                .dig
                .iter()
                .copied()
                .filter(|at| live_cell(game, *at) == model::Cell::Solid)
                .collect();
            a.stage_ms = now;
            if !rest.is_empty() {
                a.stage = Stage::Dig {
                    rest,
                    issued: false,
                };
                return Flow::Keep;
            }
            if let Some(floor) = step.place {
                if live_cell(game, floor) != model::Cell::Air {
                    // Somebody (or the server) already put something there.
                    if live_cell(game, floor) != model::Cell::Solid {
                        return Flow::Failed;
                    }
                } else {
                    // One bridge job builds the whole straight run of floor blocks.
                    let from = expected;
                    let Some(direction) =
                        building::Direction::from_step(step.to[0] - from[0], step.to[2] - from[2])
                    else {
                        return Flow::Failed;
                    };
                    let mut count = 1;
                    while let Some(next) = a.route.steps.get(a.index + count) {
                        let before = a.route.steps[a.index + count - 1].to;
                        if next.place.is_none()
                            || !next.dig.is_empty()
                            || next.to[1] != before[1]
                            || building::Direction::from_step(
                                next.to[0] - before[0],
                                next.to[2] - before[2],
                            ) != Some(direction)
                            || count >= 32
                        {
                            break;
                        }
                        count += 1;
                    }
                    let config = building::Config {
                        kind: building::Kind::Bridge,
                        direction,
                        width: 1,
                        height: 2,
                        length: count as u16,
                        fill_floor: true,
                    };
                    return match construction::start(game, config, false) {
                        Ok(()) => {
                            a.stage = Stage::Bridge { count };
                            Flow::Keep
                        }
                        Err(_) => Flow::Failed,
                    };
                }
            }
            // A run of plain steps is walked in one go.
            let mut end = a.index + 1;
            while let Some(next) = a.route.steps.get(end) {
                if next.place.is_some() || !next.dig.is_empty() {
                    break;
                }
                end += 1;
            }
            let mut nodes = vec![expected];
            nodes.extend(a.route.steps[a.index..end].iter().map(|step| step.to));
            let scope = a.scope.clone();
            assist_walk(&scope, nodes);
            a.stage = Stage::Walk { end };
            Flow::Keep
        }
        Stage::Dig { rest, issued } => {
            while let Some(&at) = rest.first() {
                if live_cell(game, at) == model::Cell::Solid {
                    break;
                }
                rest.remove(0);
                *issued = false;
            }
            let Some(&at) = rest.first() else {
                a.stage = Stage::Next;
                return Flow::Keep;
            };
            if !*issued {
                if construction_dig(game, at).is_err() {
                    return Flow::Failed;
                }
                *issued = true;
                a.stage_ms = now;
                return Flow::Keep;
            }
            match construction_dig_state() {
                DigState::Done => {
                    rest.remove(0);
                    *issued = false;
                    a.stage_ms = now;
                    Flow::Keep
                }
                DigState::Working => Flow::Keep,
                DigState::Paused => Flow::Abort,
                DigState::Failed => Flow::Failed,
            }
        }
        Stage::Bridge { count } => match construction::outcome() {
            Some(construction::Outcome::Done) => {
                construction::reset();
                a.index += *count;
                a.stage = Stage::Next;
                a.stage_ms = now;
                Flow::Keep
            }
            Some(construction::Outcome::Working) => Flow::Keep,
            Some(construction::Outcome::Failed) => Flow::Failed,
            Some(construction::Outcome::Paused) | None => Flow::Abort,
        },
        Stage::Walk { end } => {
            let end = *end;
            match state(|s| s.nav.status) {
                model::Status::Arrived => {
                    a.index = end;
                    a.stage = Stage::Next;
                    a.stage_ms = now;
                    Flow::Keep
                }
                model::Status::Stuck | model::Status::NoPath => Flow::Failed,
                model::Status::Paused | model::Status::Idle => Flow::Abort,
                _ => Flow::Keep,
            }
        }
    }
}
/// How an excavation started by `construction_dig` is going.
pub(super) enum DigState {
    Working,
    Done,
    Paused,
    Failed,
}
pub(super) fn construction_dig_state() -> DigState {
    state(|s| match s.mining.as_ref().filter(|j| j.ore == u8::MAX) {
        Some(j) if j.phase == mining::Phase::Done => DigState::Done,
        Some(j) if j.active => DigState::Working,
        Some(j)
            if matches!(
                j.phase,
                mining::Phase::Refused
                    | mining::Phase::Tool
                    | mining::Phase::Inventory
                    | mining::Phase::Timeout
                    | mining::Phase::NoTarget
            ) =>
        {
            DigState::Failed
        }
        _ => DigState::Paused,
    })
}

fn live_cell(game: &Game<'_>, at: model::Pos) -> model::Cell {
    if !game
        .world
        .chunks
        .contains_key(&(at[0].div_euclid(16), at[2].div_euclid(16)))
    {
        return model::Cell::Unknown;
    }
    let block = game.world.block(at[0], at[1], at[2]);
    let f = game.res.facts().get(block);
    model::classify_shape(
        &f.name,
        f.full_collision,
        f.fluid_level > 0 || f.waterlogged,
        &f.outline,
        model::Cell::Air,
    )
}
/// A column the miner builds under itself to reach a block high over it (the top of a tall
/// tree once its trunk is cut from below), and takes down again afterwards: jump, put a block
/// where the feet were, land on it, until the block is in reach; then dig the column away from
/// the top, one block under the feet at a time, back to the ground.
struct Pillar {
    column: [i32; 2],
    /// The height to stand at: `mining::PILLAR_BELOW` under the block overhead.
    goal: i32,
    /// The blocks put down so far, bottom first (only these are dug on the way down).
    placed: Vec<model::Pos>,
    /// Up while climbing, false while taking the column down.
    up: bool,
    /// A block being put down, since when.
    placing: Option<(model::Pos, Instant)>,
    tries: u8,
    item: String,
    since: Instant,
    /// At the top: the miner works from here as from the ground (walking and mining free).
    top: bool,
}
static PILLAR: Mutex<Option<Pillar>> = Mutex::new(None);
/// The block to build the column from: earth or stone first, then any plain building block,
/// the kind being mined included (what is put down is dug again with the same tool).
fn pillar_item(game: &Game<'_>, _ore: u8) -> Option<(String, u32)> {
    let items = crate::game::claim_borders_construction_view::building_items(game, false);
    let count = |name: &str| -> u32 {
        (0..36)
            .filter_map(|slot| slot_stack(game, slot))
            .filter(|st| &*st.item == name)
            .map(|st| st.count)
            .sum()
    };
    let rank = |name: &str| {
        let n = name.strip_prefix("minecraft:").unwrap_or(name);
        if matches!(
            n,
            "dirt" | "coarse_dirt" | "netherrack" | "cobblestone" | "cobbled_deepslate"
        ) {
            0
        } else {
            1
        }
    };
    items
        .into_iter()
        .min_by_key(|name| (rank(name), std::cmp::Reverse(count(name))))
        .map(|name| {
            let n = count(&name);
            (name, n)
        })
}
/// After a search found nothing to mine: a block of the kind high overhead (a tall trunk) is
/// climbed to on a column of blocks, if there are blocks enough to build it; a column already
/// climbed is taken down once nothing more is in reach from its top.
fn pillar_consider(game: &Game<'_>, ore: u8, position: [f64; 3]) {
    let feet = model::standing_block(position, &game.native_block(model::block(position)));
    let mut pillar = PILLAR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = pillar.as_mut() {
        if p.up && p.placing.is_none() && feet[1] >= p.goal {
            // At the top and nothing more to mine from here: take the column down.
            p.up = false;
            p.since = Instant::now();
        }
        return;
    }
    let Some(log) = mining::overhead(feet, |at| mining_block(game, ore, at)) else {
        return;
    };
    let goal = log[1] - mining::PILLAR_BELOW;
    let needed = (goal - feet[1]).max(0) as u32;
    let Some((item, have)) = pillar_item(game, ore) else {
        return;
    };
    if needed == 0 || have < needed {
        return;
    }
    *pillar = Some(Pillar {
        column: [feet[0], feet[2]],
        goal,
        placed: Vec::new(),
        up: true,
        placing: None,
        tries: 0,
        item,
        since: Instant::now(),
        top: false,
    });
}
/// While a column is in use: the keys to hold (no walking; jump on the way up when no block is
/// being put down). `None` when there is no column.
fn pillar_controls() -> Option<bool> {
    PILLAR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .filter(|p| !(p.up && p.top))
        .map(|p| p.up && p.placing.is_none())
}
/// Drives the column each tick. Returns the attack button to hold while it is in use (held
/// on the way down, to dig the block under the feet), `None` when there is no column.
fn pillar_frame(game: &mut Game<'_>) -> Option<bool> {
    let mut slot = PILLAR.lock().unwrap_or_else(|e| e.into_inner());
    let p = slot.as_mut()?;
    let active = state(|s| s.mining.as_ref().is_some_and(|j| j.active));
    let player = game.native_player();
    let grounded = game.player.as_ref().is_some_and(|b| b.ground.on_ground);
    let (Some(body), true) = (player, active) else {
        // The job stopped or paused: the column stays, nothing is driven.
        *slot = None;
        return None;
    };
    if p.since.elapsed() > Duration::from_secs(120) || p.tries >= 6 {
        *slot = None;
        return None;
    }
    let position = [body.x, body.y, body.z];
    let feet = model::standing_block(position, &game.native_block(model::block(position)));
    if p.up {
        if let Some((at, when)) = p.placing {
            match crate::game::claim_borders_construction_view::external_result() {
                Some(Ok(_)) => {
                    p.placed.push(at);
                    p.placing = None;
                }
                Some(Err(_)) => {
                    p.tries += 1;
                    p.placing = None;
                }
                None if when.elapsed() > Duration::from_secs(2) => {
                    p.tries += 1;
                    p.placing = None;
                }
                None => {}
            }
            return Some(false);
        }
        if feet[1] >= p.goal {
            // Up there: mining goes on from the top; `pillar_consider` turns it round.
            p.top = true;
            return None;
        }
        // In the air over the cell the feet left: put the block there.
        let base = p.placed.last().map_or(feet[1], |b| b[1] + 1);
        let at = [p.column[0], base, p.column[1]];
        if !grounded
            && body.y >= f64::from(base) + 1.0
            && live_cell(game, at) == model::Cell::Air
            && !crate::game::claim_borders_construction_view::placement_pending()
        {
            let Ok(state) = crate::game::claim_borders_schematic::BlockState::parse(&p.item) else {
                *slot = None;
                return None;
            };
            let request = crate::game::claim_borders_schematic::PlacementRequest {
                target: at,
                item: p.item.clone(),
                state: state.clone(),
                allowed: std::collections::BTreeMap::from([(at, state)]),
            };
            if crate::game::claim_borders_construction_view::place_external(game, request).is_ok() {
                p.placing = Some((at, Instant::now()));
            } else {
                p.tries += 1;
            }
        }
        return Some(false);
    }
    // Down: dig the placed block under the feet, looking straight down; done on the ground.
    let below = [feet[0], feet[1] - 1, feet[2]];
    if !p.placed.contains(&below) || live_cell(game, below) != model::Cell::Solid {
        if live_cell(game, below) != model::Cell::Solid {
            p.placed.retain(|b| *b != below);
        }
        if p.placed.is_empty() || !p.placed.iter().any(|b| b[1] < feet[1]) {
            *slot = None;
            return None;
        }
        return Some(false);
    }
    if mouse_has_camera() {
        return Some(false);
    }
    game.pitch = model::turn_toward(game.pitch, 90.0, 24.0);
    // The right tool for the block under the feet (a shovel for earth, a pickaxe for stone).
    drop(slot);
    match ensure_tool(game, Some(below)) {
        Tool::Wait => return Some(false),
        Tool::Ready | Tool::Missing => {}
    }
    Some(game.pitch > 80.0 && grounded)
}
/// Set while gliding: the head is turned faster then (the flight follows the look).
static GLIDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Whether a glide heading `yaw` has six blocks of open air ahead, at the body's height and the
/// block below it.
fn glide_clear(game: &Game<'_>, position: [f64; 3], yaw: f32) -> bool {
    let (sin, cos) = f64::from(yaw).to_radians().sin_cos();
    (1..=6).all(|step| {
        let d = f64::from(step);
        let x = position[0] - sin * d;
        let z = position[2] + cos * d;
        (-1..=1).all(|dy| {
            let at = model::block([x, position[1] + f64::from(dy), z]);
            matches!(
                live_cell(game, at),
                model::Cell::Air | model::Cell::Climb | model::Cell::Water | model::Cell::Unknown
            )
        })
    })
}
/// Gliding through the air on elytra. The glide flag alone is not enough: the server keeps it
/// after a fall into water (vanilla ends a glide on the ground, not in a liquid), and the body
/// swims there, it does not glide.
fn gliding_in_air(body: &horizon_client::motion::PlayerMotion) -> bool {
    body.fall_flying && !body.fluids.was_touching_water && !body.is_in_lava()
}
/// Every other tick while falling with elytra: a fresh jump press spreads them.
static ELYTRA_TAP: Mutex<bool> = Mutex::new(false);
/// Elytra worn, with durability to spare.
pub(super) fn elytra_ready(game: &Game<'_>) -> bool {
    game.armor
        .get(1)
        .and_then(Option::as_ref)
        .is_some_and(|piece| {
            &*piece.item == "minecraft:elytra"
                && game
                    .res
                    .items()
                    .items
                    .get(&*piece.item)
                    .is_some_and(|info| info.max_damage == 0 || info.max_damage > piece.damage + 10)
        })
}
/// The walker wants to run (a jump across a gap): until when, set by the movement hook and
/// carried out with the sprint key on the next input tick.
static SPRINT: Mutex<Option<Instant>> = Mutex::new(None);
/// Whether the sprint key is held by the walker now (to let go of it after).
static SPRINT_HELD: Mutex<bool> = Mutex::new(false);
/// Holds or lets go of the sprint key as the walker asks.
fn sprint_frame(game: &mut Game<'_>) {
    let want = SPRINT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(150));
    let mut held = SPRINT_HELD.lock().unwrap_or_else(|e| e.into_inner());
    let Some(code) = horizon_client::keybinds::key_code(game.settings.keys.get("key.sprint"))
    else {
        return;
    };
    if want && !*held {
        game.keys.press(code, horizon_client::held::Source::Agent);
        *held = true;
    } else if !want && *held {
        game.keys
            .release(&code, horizon_client::held::Source::Agent);
        *held = false;
    }
}
/// Swimming up for air: set when the air runs low under water, cleared when it is full again.
static SURFACING: Mutex<bool> = Mutex::new(false);
/// Where the walker wants to look (yaw, pitch) and since when: set by the movement hook from the
/// route, turned to a little each tick by `face` (the head moves as a person's does).
static FACE: Mutex<Option<(f32, f32, Instant)>> = Mutex::new(None);
/// The last tick the walker was really walking, and how many ticks it has coasted since.
static COAST: Mutex<Option<(Instant, u8)>> = Mutex::new(None);
/// How far the head turns in one tick while walking, at most.
const TURN_STEP: f32 = 22.0;
/// Turns the head toward where the walk goes, a little each tick, with the small sway of a
/// person's view; nothing while a block is being aimed at (a door, a chest, mining) or when the
/// wish is stale.
fn face(game: &mut Game<'_>) {
    use crate::game::claim_borders_construction_view as placement;
    let flying = game.player.as_ref().is_some_and(|b| b.fall_flying);
    if !flying {
        GLIDING.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    let gliding = flying && GLIDING.load(std::sync::atomic::Ordering::Relaxed);
    // The player moved the mouse a moment ago: the camera is theirs, the task goes on.
    if mouse_has_camera() {
        return;
    }
    let wish = *FACE.lock().unwrap_or_else(|e| e.into_inner());
    let Some((yaw, pitch, since)) = wish else {
        return;
    };
    // The miner aims at its block itself: the walk's look must not turn the head then.
    let mining_at_block = state(|s| {
        s.mine_aim.is_some()
            || s.mining
                .as_ref()
                .is_some_and(|j| j.active && j.phase != mining::Phase::Approaching)
    });
    // A glide is steered by the look, so it turns the head with the human look off too.
    if since.elapsed() > Duration::from_millis(250)
        || placement::aiming()
        || placement::aimed_recently()
        || mining_at_block
        || (!gliding && !human_look_enabled(game))
    {
        return;
    }
    if gliding {
        game.yaw = model::turn_toward(game.yaw, yaw, TURN_STEP);
        game.pitch = model::turn_toward(game.pitch, pitch, 12.0);
        return;
    }
    let t = since.elapsed().as_secs_f32() + START.get_or_init(Instant::now).elapsed().as_secs_f32();
    let sway_yaw = 1.2 * (t * 1.7).sin();
    game.yaw = model::turn_toward(game.yaw, yaw + sway_yaw, TURN_STEP);
    // The pitch follows slowly (a few degrees a tick): no nodding.
    game.pitch = model::turn_toward(game.pitch, pitch, 3.0);
}
/// When the player last turned the camera themselves (mouse or controller), from the game's
/// own look input: a turn made by a task is never taken for one.
static PLAYER_LOOK: Mutex<Option<Instant>> = Mutex::new(None);
/// How long the camera stays the player's after the mouse last moved.
const MOUSE_LEND: Duration = Duration::from_secs(3);
/// The look hook: the player turned the camera.
pub(super) fn player_looked() {
    *PLAYER_LOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}
/// Whether the player moved the mouse within the last three seconds: every task leaves the
/// camera alone then (it waits to aim, it never pauses for it) and walks with the keys only.
pub(super) fn mouse_has_camera() -> bool {
    PLAYER_LOOK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < MOUSE_LEND)
}
/// Whether the walker's look turns the head now: the switch is on and the mouse does not have
/// the camera.
fn look_active(game: &Game<'_>) -> bool {
    human_look_enabled(game) && !mouse_has_camera()
}
/// The keys toward `target` for swimming: with the head turning, only ahead and to the sides
/// (the head comes round first); with it still (the switch off, the mouse has the camera), every
/// way, or the swimmer would wait for a turn that never comes.
fn swim_steer(game: &Game<'_>, position: [f64; 3], target: [f64; 3], yaw: f32) -> model::Input {
    if look_active(game) {
        model::human_steer(position, target, yaw)
    } else {
        crate::game::claim_borders_construction::steer(position, target, yaw)
    }
}
/// "Human look" is on unless the player switched it off (Experiments → Helpers).
pub(super) fn human_look_enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::SETTINGS_ID, "human_look")
        .as_deref()
        != Some("false")
}
/// Any task moving on its own (tunnel, bridge, drain, swimming back) asks the head to turn
/// toward the point it heads for, as the walker does.
pub(super) fn look_toward(position: [f64; 3], point: [f64; 3]) {
    let eye = [position[0], position[1] + 1.62, position[2]];
    let ahead = [point[0], point[1] + 0.9, point[2]];
    if (ahead[0] - eye[0]).hypot(ahead[2] - eye[2]) < 0.3 {
        return;
    }
    let yaw = model::yaw_to(eye, ahead);
    let pitch = model::pitch_to(eye, ahead).clamp(-10.0, 35.0);
    *FACE.lock().unwrap_or_else(|e| e.into_inner()) = Some((yaw, pitch, Instant::now()));
}
static START: OnceLock<Instant> = OnceLock::new();
/// Enchantment protection against falls from the armour worn (`navigation::fall_protection`).
fn fall_protection(game: &Game<'_>) -> u32 {
    let levels: Vec<(u32, u32)> = game
        .armor
        .iter()
        .flatten()
        .map(|piece| {
            (
                piece.enchantment_level("minecraft:feather_falling"),
                piece.enchantment_level("minecraft:protection"),
            )
        })
        .collect();
    model::fall_protection(&levels)
}
/// Doors and gates on the way: the one being opened (with when it was last clicked and how many
/// times), and those opened by the walker, to be closed again behind it.
struct Doors {
    opening: Option<(model::Pos, Instant, u8)>,
    opened: Vec<model::Pos>,
}
static DOORS: Mutex<Doors> = Mutex::new(Doors {
    opening: None,
    opened: Vec::new(),
});
/// Whether the door or gate at `at` is one a hand opens, and closed.
fn closed_door(game: &Game<'_>, at: model::Pos) -> bool {
    model::openable(&game.native_block(at))
        && game.native_block_property(at, "open").as_deref() == Some("false")
}
fn unblock_by_door(game: &Game<'_>, position: [f64; 3]) {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let blocked = state(|s| {
        s.nav.goal.is_some() && matches!(s.nav.status, model::Status::NoPath | model::Status::Stuck)
    });
    if !blocked {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
        return;
    }
    let feet = model::standing_block(position, &game.native_block(model::block(position)));
    let door = [[1, 0], [-1, 0], [0, 1], [0, -1]]
        .into_iter()
        .flat_map(|[dx, dz]| (0..=1).map(move |y| [feet[0] + dx, feet[1] + y, feet[2] + dz]))
        .chain([
            [feet[0], feet[1] - 1, feet[2]],
            [feet[0], feet[1] + 1, feet[2]],
        ])
        .find(|at| closed_door(game, *at));
    let Some(door) = door else {
        return;
    };
    *last = Some(Instant::now());
    crate::game::claim_borders_construction_view::open_container(door);
    state(|s| {
        s.nav.running = true;
        s.nav.replan();
    });
}
/// The walk is over or waits: a door being opened that is open now counts as opened (to be
/// closed behind), and those left behind are closed.
fn settle_doors(game: &Game<'_>, position: [f64; 3]) {
    {
        let mut doors = DOORS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, _, _)) = doors.opening
            && !closed_door(game, at)
        {
            doors.opening = None;
            if !doors.opened.contains(&at) {
                doors.opened.push(at);
            }
        }
    }
    close_doors_behind(game, &[], position);
}
/// Closes behind the walker a door, gate or trapdoor it opened, once it is out of the doorway
/// and the next steps do not go through it again; called every tick, also after the walk has
/// ended right behind the door.
fn close_doors_behind(game: &Game<'_>, ahead: &[model::Pos], position: [f64; 3]) {
    let placement = crate::game::claim_borders_construction_view::open_container;
    let mut doors = DOORS.lock().unwrap_or_else(|e| e.into_inner());
    if doors.opening.is_none()
        && let Some(i) = doors.opened.iter().position(|d| {
            let away = model::distance(model::center(*d), position);
            away > 6.0
                || (away >= 1.8
                    && !ahead
                        .iter()
                        .any(|a| a[0] == d[0] && a[2] == d[2] && (a[1] - d[1]).abs() <= 1))
        })
    {
        let door = doors.opened.remove(i);
        let away = model::distance(model::center(door), position);
        if away <= 4.0
            && model::openable(&game.native_block(door))
            && game.native_block_property(door, "open").as_deref() == Some("true")
        {
            placement(door);
        }
    }
}
/// Opens the closed door or gate on the next steps of the route once it is within reach, and
/// closes behind the walker the ones it opened. True while the walker should wait for a door
/// to open (the click is asked of the placement driver, which aims and presses use).
fn door_step(
    game: &Game<'_>,
    path: &[model::Pos],
    next: usize,
    position: [f64; 3],
    stalled: u16,
) -> bool {
    let placement = crate::game::claim_borders_construction_view::open_container;
    let ahead: Vec<model::Pos> = path.iter().skip(next).take(3).copied().collect();
    close_doors_behind(game, &ahead, position);
    let mut doors = DOORS.lock().unwrap_or_else(|e| e.into_inner());
    let wanted = ahead.iter().enumerate().find_map(|(k, n)| {
        // A step straight up or down (a ladder) passes through the cell itself.
        let vertical = (next + k)
            .checked_sub(1)
            .and_then(|i| path.get(i))
            .is_some_and(|prev| prev[0] == n[0] && prev[2] == n[2]);
        (0..=1).map(|y| [n[0], n[1] + y, n[2]]).find(|at| {
            closed_door(game, *at)
                && (!model::hand_trapdoor(&game.native_block(*at))
                    // A trapdoor shut on the floor is walked over; one at head height, shut at
                    // the top of its cell, or in the way of a climb is opened.
                    || at[1] > n[1]
                    || vertical
                    || game.native_block_property(*at, "half").as_deref() == Some("top"))
        })
    });
    // Held up anyway (a door the route did not know was shut, a player who went through and
    // closed it, a trapdoor under the feet over a ladder): whatever closed door, gate or
    // trapdoor is under the feet, at them, or one step toward the next point is opened.
    let wanted = wanted.or_else(|| {
        let feet = model::standing_block(position, &game.native_block(model::block(position)));
        let toward = path.get(next).map_or([0, 0], |n| {
            [(n[0] - feet[0]).signum(), (n[2] - feet[2]).signum()]
        });
        // Right in front, the way the walk goes (each axis of a corner too): always.
        let mut cells = vec![
            [feet[0] + toward[0], feet[1], feet[2] + toward[1]],
            [feet[0] + toward[0], feet[1] + 1, feet[2] + toward[1]],
            [feet[0] + toward[0], feet[1], feet[2]],
            [feet[0], feet[1], feet[2] + toward[1]],
        ];
        // At the feet, the head or under them: once held up for a moment.
        if stalled >= 3 {
            if path.get(next).is_some_and(|n| n[1] < feet[1]) {
                cells.push([feet[0], feet[1] - 1, feet[2]]);
            }
            cells.push([feet[0], feet[1] + 1, feet[2]]);
            cells.push(feet);
        }
        cells.into_iter().find(|at| closed_door(game, *at))
    });
    let Some(door) = wanted else {
        if let Some((at, _, _)) = doors.opening.take()
            && !doors.opened.contains(&at)
        {
            // It opened: remember it, to close it behind.
            doors.opened.push(at);
            doors.opened.truncate(4);
        }
        return false;
    };
    let reach = (model::center(door)[0] - position[0]).hypot(model::center(door)[2] - position[2]);
    if reach > 2.6 {
        return false;
    }
    match doors.opening {
        Some((at, _, tries)) if at == door && mouse_has_camera() => {
            // The player has the camera: no click is aimed, so none is counted as tried.
            doors.opening = Some((door, Instant::now(), tries));
        }
        Some((at, when, tries)) if at == door => {
            if when.elapsed() >= Duration::from_millis(600) {
                if tries >= 5 {
                    // It will not open (locked by a plugin, out of reach): let the stall logic
                    // plan another way.
                    doors.opening = None;
                    return false;
                }
                placement(door);
                doors.opening = Some((door, Instant::now(), tries + 1));
            }
        }
        _ => {
            placement(door);
            doors.opening = Some((door, Instant::now(), 1));
        }
    }
    true
}
/// The direction (horizontal, unit) away from another body (a player, a mob) that touches the
/// walker, if one does: within a body's width and a head's height of it.
fn body_in_the_way(game: &Game<'_>, position: [f64; 3]) -> Option<[f64; 2]> {
    game.mobs
        .iter()
        .filter(|m| !m.removed)
        .filter_map(|m| {
            let (dx, dz) = (position[0] - m.position.x, position[2] - m.position.z);
            let near = dx.hypot(dz);
            ((position[1] - m.position.y).abs() < 1.6 && near < 1.0).then_some((near, dx, dz))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(near, dx, dz)| {
            if near < 0.05 {
                // Exactly on top of each other: any way out will do.
                [1.0, 0.0]
            } else {
                [dx / near, dz / near]
            }
        })
}
pub(super) fn movement(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) {
    crate::game::claim_borders_workshop_view::manual_movement(*forward, *strafe, *jump, *sneak);
    // The storage window is open on purpose: nothing here may pause the trip.
    if crate::game::claim_borders_workshop_view::storing() {
        return;
    }
    if crate::game::claim_borders_construction_view::movement(game, forward, strafe, jump, sneak) {
        return;
    }
    if !game.native_on(claims::ID) || !in_world(game) {
        pause();
        return;
    }
    let Some(body) = game.player.as_ref() else {
        pause();
        return;
    };
    // In the water, whatever the task: stay at the surface (jump held) and swim to the nearest
    // dry ground, looking where it goes; the task goes on from there. Nobody drowns.
    if (body.swimming || body.fluids.was_touching_water)
        && !body.is_in_lava()
        && !body.flying
        && task_running()
        && let Some(p) = game.native_player()
    {
        let position = [p.x, p.y, p.z];
        *sneak = false;
        // Out of breath: straight up to the surface, and wait there until the air is full.
        {
            let mut surfacing = SURFACING.lock().unwrap_or_else(|e| e.into_inner());
            let under = body.fluids.was_eye_in_water;
            // Water Breathing (or a potion of it to drink) means no need to go up at all.
            let breathing = breathing_ticks(game) > 0;
            if under && body.air < mining::AIR_LOW && !breathing && breathing_potion(game).is_none()
            {
                *surfacing = true;
            }
            if *surfacing && (body.air >= mining::AIR_FULL || breathing) {
                *surfacing = false;
            }
            if *surfacing {
                *jump = true;
                *forward = 0.0;
                *strafe = 0.0;
                return;
            }
        }
        // Mining under water: swim to the block (up with jump, down with sneak), hold still once
        // it is in reach; the miner aims and breaks it.
        let below = state(|s| {
            // A route of the start alone (swum to): it has arrived.
            if s.nav.running && s.nav.path.len() <= 1 {
                let _ = s.nav.input(position, p.yaw, false);
            }
            s.mining
                .as_ref()
                .filter(|j| j.active)
                .and_then(|j| j.target.map(|t| (t, j.ore)))
        })
        .filter(|(target, ore)| mining::underwater(*target, |at| mining_block(game, *ore, at)));
        if let Some((target, ore)) = below {
            let eye = [position[0], position[1] + 1.62, position[2]];
            let middle = model::center(target);
            let middle = [middle[0], middle[1] + 0.5, middle[2]];
            *jump = false;
            *forward = 0.0;
            *strafe = 0.0;
            if model::distance(eye, middle) > 3.6 {
                // The nearest water cell beside the block is where to swim.
                let goal = [
                    [0, 1, 0],
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                    [0, -1, 0],
                ]
                .into_iter()
                .map(|d| [target[0] + d[0], target[1] + d[1], target[2] + d[2]])
                .filter(|at| mining_block(game, ore, *at).water)
                .min_by(|a, b| {
                    model::distance(model::center(*a), position)
                        .total_cmp(&model::distance(model::center(*b), position))
                })
                .map_or(middle, model::center);
                let yaw = model::yaw_to(eye, goal);
                let pitch = model::pitch_to(eye, goal);
                *FACE.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some((yaw, pitch.clamp(-60.0, 60.0), Instant::now()));
                let input = swim_steer(game, position, goal, p.yaw);
                *forward = input.forward;
                *strafe = input.strafe;
                if goal[1] > position[1] + 0.3 {
                    *jump = true;
                } else if goal[1] < position[1] - 0.3 {
                    *sneak = true;
                }
            }
            return;
        }
        // A mining job under water looks for its next block from here: stay, do not swim off.
        if state(|s| s.mining.as_ref().is_some_and(|j| j.active)) {
            *jump = false;
            *forward = 0.0;
            *strafe = 0.0;
            return;
        }
        *jump = true;
        // On a route: swim along it (it climbs out where it leaves the water); otherwise to the
        // nearest dry ground.
        let along = state(|s| {
            if s.nav.running && s.nav.status == model::Status::Walking {
                // Steps already reached are passed (the keys are set here, not by it).
                let _ = s.nav.input(position, p.yaw, false);
            }
            (s.nav.running && s.nav.status == model::Status::Walking)
                .then(|| s.nav.path.get(s.nav.next).copied())
                .flatten()
        });
        if let Some(spot) =
            along.or_else(|| crate::game::claim_borders_construction_view::dry_spot(game, position))
        {
            let target = model::center(spot);
            look_toward(position, target);
            let input = swim_steer(game, position, target, p.yaw);
            *forward = input.forward;
            *strafe = input.strafe;
        }
        return;
    }
    // Elytra: a long drop on the route is glided down instead of fallen. Spread while falling
    // (one jump press in the air), then glide toward where the route goes, looking a little
    // down; landing folds them.
    if task_running()
        && !body.ground.on_ground
        && !body.is_in_lava()
        && elytra_ready(game)
        && let Some(p) = game.native_player()
    {
        let position = [p.x, p.y, p.z];
        // Where to come down: the first point of the route below (the landing of the drop), or
        // the goal when the route is being planned again.
        let landing = state(|s| {
            s.nav
                .path
                .iter()
                .skip(s.nav.next)
                .find(|n| f64::from(n[1]) < position[1] - 1.0)
                .or_else(|| s.nav.path.last())
                .copied()
                .or(s.nav.goal)
        });
        if let Some(next) = landing {
            let goal = model::center(next);
            if body.fall_flying {
                // Flight routing: toward the landing, round what is in the way, circling to
                // lose height, levelling out low over it.
                let open = |yaw: f32| glide_clear(game, position, yaw);
                let (yaw, pitch) = model::glide(position, goal, open);
                *FACE.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some((yaw, pitch, Instant::now()));
                GLIDING.store(true, std::sync::atomic::Ordering::Relaxed);
                *forward = 0.0;
                *strafe = 0.0;
                *jump = false;
                return;
            }
            if position[1] - goal[1] > 3.5 {
                let mut tap = ELYTRA_TAP.lock().unwrap_or_else(|e| e.into_inner());
                *tap = !*tap;
                *jump = *tap;
                // Falling to spread the elytra: nothing else this tick. Going on, the walker took
                // this jump press for the player's own and paused the task in mid-air.
                *forward = 0.0;
                *strafe = 0.0;
                *sneak = false;
                return;
            }
        }
    }
    if task_running() && gliding_in_air(body) {
        // Gliding with no route at hand: no keys (the look steers), never a pause; the route is
        // planned again once landed. Not in water (the glide flag outlasts a fall into it, and
        // taking the keys there left the player unable to swim), and never without a task.
        *forward = 0.0;
        *strafe = 0.0;
        return;
    }
    if body.flying
        || body.swimming
        || body.fluids.was_touching_water
        || body.is_in_lava()
        || body.is_passenger
        || body.sleeping
        || body.health.health() <= 0.0
    {
        pause();
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let position = [p.x, p.y, p.z];
    // Doors opened on the way are closed behind also when the walk has just ended.
    if !state(|s| s.nav.running && s.nav.status == model::Status::Walking) {
        settle_doors(game, position);
    }
    // No way found or stuck, with a shut door, gate or trapdoor right beside: open it and plan
    // again (the planner may not have known it can be opened from here).
    unblock_by_door(game, position);
    // A column being built under the miner (or taken down): stand still, jump to climb.
    if let Some(climb) = pillar_controls() {
        if (*forward != 0.0 || *strafe != 0.0 || *sneak) && !input_grace() {
            *PILLAR.lock().unwrap_or_else(|e| e.into_inner()) = None;
            pause();
            return;
        }
        *forward = 0.0;
        *strafe = 0.0;
        *sneak = false;
        *jump = climb && body.ground.on_ground;
        return;
    }
    state(|s| {
        if !s.nav.running {
            return;
        }
        if ((*forward != 0.0 || *strafe != 0.0 || *jump || *sneak) && !input_grace())
            || scope(game).as_deref() != Some(s.scope.as_str())
        {
            cancel_pending(s);
            pause_state(s);
            return;
        }
        s.nav.climbing = live_cell(game, model::block(position)) == model::Cell::Climb;
        // Turning round before walking back only when the head is really turned (human look on
        // and the camera not lent to the mouse); otherwise the walker steps back as before.
        s.nav.human = look_active(game);
        let mut input = s.nav.input(position, p.yaw, body.ground.on_ground);
        // Look where the walk goes: a few blocks ahead, a little down, as a person walks.
        if let Some(look) = s.nav.look_point(position) {
            let eye = [position[0], position[1] + 1.62, position[2]];
            let ahead = [look[0], look[1] + 0.9, look[2]];
            let flat = (ahead[0] - eye[0]).hypot(ahead[2] - eye[2]);
            let yaw = if flat > 0.3 {
                model::yaw_to(eye, ahead)
            } else {
                p.yaw
            };
            // A steady glance down at the way ahead, more on the way down, less on the way up;
            // not the exact angle to each next cell, which made the head nod at every step.
            let rise = (look[1] - position[1]).clamp(-3.0, 3.0) as f32;
            let pitch = (14.0 - rise * 6.0).clamp(0.0, 32.0);
            *FACE.lock().unwrap_or_else(|e| e.into_inner()) = Some((yaw, pitch, Instant::now()));
        }
        // Between two plans (a leg ends, the route is planned again) a person keeps walking for
        // a moment instead of freezing: on firm ground, straight on, only onto a cell that can
        // be walked to, for half a second at most.
        {
            let mut coast = COAST.lock().unwrap_or_else(|e| e.into_inner());
            if s.nav.status == model::Status::Walking && input.forward > 0.0 {
                *coast = Some((Instant::now(), 0));
            } else if matches!(
                s.nav.status,
                model::Status::Waiting | model::Status::Planning
            ) && body.ground.on_ground
                && let Some((when, ticks)) = coast.as_mut()
                && when.elapsed() < Duration::from_millis(600)
                && *ticks < 10
            {
                let yaw = f64::from(p.yaw).to_radians();
                let here =
                    model::standing_block(position, &game.native_block(model::block(position)));
                let step = [(-yaw.sin()).round() as i32, (yaw.cos()).round() as i32];
                let ahead = [here[0] + step[0], here[1], here[2] + step[1]];
                if (step[0] != 0 || step[1] != 0)
                    && model::traversable(here, ahead, &mut |at| live_cell(game, at))
                    && live_cell(game, ahead) == model::Cell::Air
                {
                    *ticks += 1;
                    input = model::Input {
                        forward: 1.0,
                        strafe: 0.0,
                        jump: false,
                        sprint: false,
                    };
                }
            }
        }
        // A closed door or gate on the next steps: open it first, standing still meanwhile.
        if s.nav.status == model::Status::Walking
            && door_step(game, &s.nav.path, s.nav.next, position, s.nav.stalled())
        {
            s.nav.yielded();
            input = model::Input::default();
        }
        if s.nav.status == model::Status::Stuck {
            cancel_pending(s);
            s.nav.blocked(position);
            return;
        }
        // Someone is in the way (a player passing, touching the walker): step away from them
        // instead of pushing into them until the stall counts as stuck, and carry on.
        if s.nav.status == model::Status::Walking
            && s.nav.stalled() >= 4
            && let Some(away) = body_in_the_way(game, position)
        {
            s.nav.yielded();
            let yaw = f64::from(p.yaw).to_radians();
            let forward_away = -away[0] * yaw.sin() + away[1] * yaw.cos();
            let left_away = away[0] * yaw.cos() + away[1] * yaw.sin();
            input.forward = if forward_away.abs() > 0.3 {
                forward_away.signum() as f32
            } else {
                0.0
            };
            input.strafe = if left_away.abs() > 0.3 {
                left_away.signum() as f32
            } else {
                0.0
            };
            input.jump = false;
        }
        if s.nav.status == model::Status::Walking
            && let Some(&next) = s.nav.path.get(s.nav.next)
        {
            // Recheck live blocks immediately before driving; worker snapshots may be stale.
            let from = if s.nav.next > 0 {
                s.nav.path[s.nav.next - 1]
            } else {
                model::standing_block(position, &game.native_block(model::block(position)))
            };
            if !model::traversable(from, next, &mut |at| live_cell(game, at)) {
                cancel_pending(s);
                s.nav.blocked(position);
                return;
            }
        }
        *forward = input.forward;
        *strafe = input.strafe;
        // Farm routes are level: nothing there is ever jumped onto (jumping onto farmland
        // tramples it).
        *jump = input.jump && !s.level;
        if input.sprint {
            *SPRINT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        }
    });
}

pub(super) fn snapshot(game: &Game<'_>) -> (String, bool) {
    if let Some(snapshot) = crate::game::claim_borders_workshop_view::snapshot() {
        return snapshot;
    }
    if let Some(snapshot) = crate::game::claim_borders_construction_view::snapshot() {
        return snapshot;
    }
    if let Some(label) = assist_label() {
        return (label, false);
    }
    let language = language();
    state(|s| {
        if let Some(job) = s.mining.as_ref() {
            let text = if let Some(wait) = job.wait.filter(|_| job.active) {
                match wait {
                    mining::Wait::Tool => language.text(
                        "Простой: нужен подходящий инструмент",
                        "Idle: a suitable tool is needed",
                    ),
                    mining::Wait::Storage => crate::game::claim_borders_haul_view::status(),
                    mining::Wait::Inventory => language.text(
                        "Простой: освободите слот в инвентаре",
                        "Idle: free an inventory slot",
                    ),
                }
            } else if !job.active
                && matches!(
                    job.phase,
                    mining::Phase::Searching
                        | mining::Phase::Approaching
                        | mining::Phase::Breaking
                        | mining::Phase::Confirming
                        | mining::Phase::Collecting
                )
                && !matches!(
                    s.nav.status,
                    model::Status::Preview | model::Status::Planning
                )
            {
                language.text("Добыча на паузе", "Mining paused")
            } else {
                match job.phase {
                    mining::Phase::Searching if !s.mine_note.is_empty() => s.mine_note.as_str(),
                    mining::Phase::Searching => language.text("Поиск руды…", "Finding ore…"),
                    mining::Phase::Approaching if s.nav.status == model::Status::Preview => {
                        language.text("Просмотр добычи", "Mining preview")
                    }
                    mining::Phase::Approaching => language.text("Подход к руде", "Approaching ore"),
                    mining::Phase::Breaking => language.text("Добыча", "Mining"),
                    mining::Phase::Confirming => {
                        language.text("Ожидание сервера", "Waiting for server")
                    }
                    mining::Phase::Collecting => {
                        language.text("Подбор предметов", "Collecting drops")
                    }
                    mining::Phase::Done => language.text("Добыча завершена", "Mining complete"),
                    mining::Phase::NoTarget => {
                        language.text("Доступная руда не найдена", "No reachable ore found")
                    }
                    mining::Phase::Refused => {
                        language.text("Сервер отклонил разрушение", "Server refused the break")
                    }
                    mining::Phase::Tool => language.text(
                        "Нужен подходящий исправный инструмент",
                        "A suitable usable tool is required",
                    ),
                    mining::Phase::Inventory => {
                        language.text("Нужен свободный слот", "A free inventory slot is required")
                    }
                    mining::Phase::Timeout => language.text("Тайм-аут добычи", "Mining timed out"),
                }
            };
            return (
                if job.limit == 0 {
                    format!("{text} · {}", job.confirmed)
                } else {
                    format!("{text} · {}/{}", job.confirmed, job.limit)
                },
                !job.active,
            );
        }
        let label = match s.nav.status {
            model::Status::Mining => language.text("Добыча", "Mining"),
            model::Status::Holding => language.text(
                "Следование · держу дистанцию",
                "Following · holding distance",
            ),
            model::Status::TargetMissing => language.text(
                "Игрок вне видимости · ожидание",
                "Player not visible · waiting",
            ),
            model::Status::TargetLost => language.text(
                "Игрок потерян · следование остановлено",
                "Player lost · following stopped",
            ),
            model::Status::Idle => language.text(
                "Задача не запущена · нажмите клавишу «Старт»",
                "No task running · press the Start key",
            ),
            model::Status::Planning => language.text("Расчёт маршрута…", "Planning route…"),
            model::Status::Preview => language.text("Просмотр маршрута", "Route preview"),
            model::Status::Walking => language.text("Движение", "Walking"),
            model::Status::Paused => language.text("Пауза", "Paused"),
            model::Status::Arrived => language.text("Цель достигнута", "Destination reached"),
            model::Status::NoPath => language.text("Нет проходимого пути", "No walkable route"),
            model::Status::Stuck => {
                language.text("Застрял — движение остановлено", "Stuck — movement stopped")
            }
            model::Status::Waiting => {
                language.text("Ожидание следующего участка", "Waiting for next segment")
            }
        };
        let distance = s
            .nav
            .goal
            .zip(game.native_player())
            .map(|(goal, p)| model::distance([p.x, p.y, p.z], model::center(goal)));
        let action_hint = s
            .preview_route
            .filter(|(digs, places)| digs + places > 0)
            .map(|(digs, places)| match language {
                Language::Russian => format!(" · маршрут: ломать {digs}, ставить {places}"),
                Language::English => format!(" · route: break {digs}, place {places}"),
            })
            .unwrap_or_default();
        (
            format!(
                "{}{}",
                distance.map_or_else(|| label.into(), |d| format!("{label} · {d:.0} m")),
                action_hint
            ),
            !s.nav.running,
        )
    })
}
pub(super) fn render(game: &Game<'_>, gizmos: &mut Gizmos) {
    if !game.native_on(claims::ID) || game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let path = state(|s| {
        if scope(game).as_deref() == Some(s.scope.as_str()) {
            s.nav
                .path
                .iter()
                .skip(s.nav.next.saturating_sub(1))
                .copied()
                .take(256)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    });
    for pair in path.windows(2) {
        let a = model::center(pair[0]);
        let b = model::center(pair[1]);
        if model::distance([p.x, p.y, p.z], a) > 96.0 {
            continue;
        }
        gizmos.line(
            [a[0], a[1] + 0.08, a[2]],
            [b[0], b[1] + 0.08, b[2]],
            0xff66ddbb,
            2.0,
        );
    }
}

pub(super) fn hud(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::claim_borders_ui_view as ui;
    if !game.native_playing()
        || game.ui.hide_gui
        || !(state(|s| s.nav.goal.is_some())
            || crate::game::claim_borders_construction_view::exists()
            || crate::game::claim_borders_workshop_view::exists())
    {
        return;
    }
    let (label, paused) = snapshot(game);
    let label = ui::clip(game, &label, (batch.width - 24.0).max(0.0));
    let width = game.native_text_width(&label) + 16.0;
    let x = (batch.width - width) * 0.5;
    ui::fill(game, batch, [x, 6.0, width, 18.0], 0xcc1d2a3a);
    ui::text(
        game,
        batch,
        &label,
        [x + 8.0, 11.0],
        if paused { 0xe1c69e } else { 0xa2d0b7 },
        false,
    );
}
