// DraimHorizon's side of the Baritone port (MIT). The LGPL code in `baritone/` is only called from
// here: walks handed over by the farm and mining, the settings and the status the Baritone tab
// shows. Everything runs on the game thread; the callers that only hold `&Game` leave a request
// that the next tick (`tick`, from the mod's tick hook) carries out.
use super::Game;
use super::baritone::{self, with};
use super::baritone_goals::{Goal, GoalBlock, rc};
use super::baritone_pathing::Process;
use super::claim_borders as claims;
use super::claim_borders_baritone_walk as walk;
use std::cell::RefCell;

type Pos = [i32; 3];
/// Settings a walk changes for its time, as (name, value to put back).
type Saved = Vec<(&'static str, String)>;

struct Request {
    /// Ticks the port was busy when the request tried to start.
    tries: u32,
    owner: &'static str,
    goal: Pos,
    overrides: &'static [(&'static str, &'static str)],
    timeout: u32,
}
struct Active {
    goal: Pos,
    ticks: u32,
    timeout: u32,
    saved: Saved,
    /// The values the walk put in place of `saved`, to put in again after a save.
    overrides: &'static [(&'static str, &'static str)],
}
/// What the Baritone tab shows: the process in control and how long it expects to take.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub process: Option<String>,
    pub segment: Option<f64>,
    pub goal: Option<f64>,
    /// `#pause` holds the port.
    pub paused: bool,
    /// The value of each of `walk::SETTINGS` (a flag is 0 or 1); empty until the port has run.
    pub values: Vec<i32>,
}
/// A mining task started from the Baritone tab, watched for a full inventory.
struct MineTask {
    names: Vec<String>,
    phase: walk::MinePhase,
    idle: u32,
    age: u32,
}
struct State {
    mine: Option<MineTask>,
    /// Who asked for the walk under way ("farm", "mining").
    owner: &'static str,
    request: Option<Request>,
    active: Option<Active>,
    cancel: bool,
    status: walk::Walk,
    snapshot: Snapshot,
    /// Ticks since the snapshot was read; large to read it at once.
    since_snapshot: u32,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        mine: None,
        owner: "",
        request: None,
        active: None,
        cancel: false,
        status: walk::Walk::Idle,
        snapshot: Snapshot { process: None, segment: None, goal: None, paused: false, values: Vec::new() },
        since_snapshot: u32::MAX,
    }) };
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Whether the farm's and mining's walking is the port's: it is whenever the port is on.
pub fn engine(game: &Game<'_>) -> bool {
    baritone::enabled(game)
}

/// Starts a walk to `goal` for the farm or mining. It begins on the next tick; `walk_status`
/// says how it goes. `distance` (blocks, from where the player stands) bounds how long it may take.
pub fn walk_to(
    game: &Game<'_>,
    owner: &'static str,
    goal: Pos,
    overrides: &'static [(&'static str, &'static str)],
    distance: f64,
) -> Result<(), String> {
    if !baritone::enabled(game) {
        return Err("Baritone is off".into());
    }
    state(|s| {
        s.owner = owner;
        s.request = Some(Request {
            tries: 0,
            owner,
            goal,
            overrides,
            timeout: walk::timeout_ticks(distance),
        });
        s.cancel = false;
        s.status = walk::Walk::Walking;
    });
    Ok(())
}
pub fn walk_status() -> walk::Walk {
    state(|s| s.status)
}
/// Whether `owner`'s walk is under way (or waiting for the next tick).
pub fn walking_for(owner: &str) -> bool {
    state(|s| s.owner == owner && s.status == walk::Walk::Walking)
}
/// How `owner`'s walk ended, once: `Some(true)` arrived, `Some(false)` failed; `None` while it
/// goes on, when it is another's, or when the result was already taken.
pub fn take_result(owner: &str) -> Option<bool> {
    state(|s| {
        if s.owner != owner {
            return None;
        }
        let result = match s.status {
            walk::Walk::Arrived => Some(true),
            walk::Walk::Failed => Some(false),
            _ => None,
        };
        if result.is_some() {
            s.status = walk::Walk::Idle;
        }
        result
    })
}
/// Whether a walk handed over by `walk_to` is under way (or waiting for the next tick).
pub fn walking() -> bool {
    state(|s| s.status == walk::Walk::Walking)
}
/// Drops the walk: the port is told on the next tick and the settings the walk changed return.
pub fn walk_cancel() {
    state(|s| {
        if s.request.take().is_some() || s.active.is_some() {
            s.cancel = true;
        }
        s.status = walk::Walk::Idle;
    });
}

fn restore(b: &mut baritone::Baritone, _saved: Saved) {
    // The player's values (their latest choice, kept by the port while the walk held its own).
    for (name, value) in baritone::persist_release() {
        let _ = b.core.settings.set(name, &value);
    }
    b.core.refresh();
}

/// Every tick, from the mod's tick hook: carries out a request or a cancel, watches the walk and
/// keeps the snapshot of the tab fresh.
pub fn tick(game: &mut Game<'_>) {
    super::claim_borders_baritone_profile::time("bridge tick", || tick_inner(game));
    // With the profiler on, what the last second of the port's work cost goes to `profile.txt`, and
    // the second with the slowest frame so far to `profile-peak.txt`.
    if super::claim_borders_baritone_profile::enabled()
        && let Some(report) =
            super::claim_borders_baritone_profile::take_report(std::time::Instant::now())
    {
        thread_local! {
            static WORST: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
        }
        let dir = baritone::data_dir(game);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("profile.txt"), &report.text);
        if report.worst_frame_ms > WORST.with(std::cell::Cell::get) {
            WORST.with(|w| w.set(report.worst_frame_ms));
            let _ = std::fs::write(dir.join("profile-peak.txt"), &report.text);
        }
    }
}
fn tick_inner(game: &mut Game<'_>) {
    if !baritone::enabled(game) {
        // The settings a walk changed go back even though the port is off now.
        let left = state(|s| {
            let active = s.active.take();
            if s.request.take().is_some() || active.is_some() {
                s.status = walk::Walk::Failed;
            }
            s.cancel = false;
            s.snapshot = Snapshot::default();
            s.since_snapshot = u32::MAX;
            active.map(|a| a.saved)
        });
        if let Some(saved) = left {
            with(game, |b, _| restore(b, saved));
        }
        // The port is off: the mining watch and its unloading end with it.
        mine_cancel(game);
        return;
    }
    mine_tick(game);
    let (request, cancel) = state(|s| (s.request.take(), std::mem::take(&mut s.cancel)));
    if cancel {
        let saved = state(|s| s.active.take().map(|a| a.saved));
        with(game, |b, game| {
            b.force_cancel(game);
            if let Some(saved) = saved {
                restore(b, saved);
            }
        });
    }
    if let Some(request) = request {
        let done = with(game, |b, _| {
            // A walk that is replaced hands its settings back first.
            if let Some(old) = state(|s| s.active.take().map(|a| a.saved)) {
                restore(b, old);
            }
            let mut saved = Saved::new();
            for (name, value) in request.overrides {
                let now = if b.core.settings.flag(name) {
                    "true"
                } else {
                    "false"
                };
                if b.core.settings.set(name, value).is_ok() {
                    saved.push((*name, now.to_owned()));
                }
            }
            b.core.refresh();
            baritone::persist_hold(saved.clone());
            b.procs.goal.set_goal_and_path(rc(GoalBlock(request.goal)));
            saved
        });
        state(|s| match done {
            Some(saved) => {
                s.active = Some(Active {
                    goal: request.goal,
                    ticks: 0,
                    timeout: request.timeout,
                    saved,
                    overrides: request.overrides,
                });
                s.status = walk::Walk::Walking;
            }
            // The port was busy this tick: ask again on the next, a few times; then the walk
            // failed (the old movement must not stay out of the way for good).
            None if request.tries < 40 => {
                s.request = Some(Request {
                    tries: request.tries + 1,
                    ..request
                });
            }
            None => s.status = walk::Walk::Failed,
        });
    }
    if let Some((goal, ticks, timeout)) = state(|s| {
        s.active.as_mut().map(|a| {
            a.ticks += 1;
            (a.goal, a.ticks, a.timeout)
        })
    }) {
        let seen = with(game, |b, game| {
            let at_goal = b
                .core
                .player_feet(game)
                .is_some_and(|feet| GoalBlock(goal).is_in_goal(feet));
            (b.procs.goal.is_active(), at_goal)
        });
        if let Some((active, at_goal)) = seen {
            let now = walk::decide(true, active, at_goal, ticks, timeout);
            if now != walk::Walk::Walking {
                let saved = state(|s| {
                    s.status = now;
                    s.active.take().map(|a| a.saved)
                });
                if let Some(saved) = saved {
                    with(game, |b, game| {
                        // A walk that timed out is cancelled so the port does not go on alone.
                        if now == walk::Walk::Failed {
                            b.force_cancel(game);
                        }
                        restore(b, saved);
                    });
                }
            }
        }
    }
    let due = state(|s| {
        s.since_snapshot = s.since_snapshot.saturating_add(1);
        s.since_snapshot >= 10
    });
    if due {
        let read = with(game, |b, game| {
            let process = b.describe().map(|(_, _, _, text)| text);
            let segment = b.core.ticks_remaining_in_segment();
            let goal = b.core.estimated_ticks_to_goal(game);
            let values = walk::SETTINGS
                .iter()
                .map(|s| match s.kind {
                    walk::Kind::Flag => i32::from(b.core.settings.flag(s.name)),
                    walk::Kind::Int { .. } => b.core.settings.int(s.name),
                })
                .collect();
            Snapshot {
                process,
                segment,
                goal,
                paused: b.procs.pause.paused,
                values,
            }
        });
        if let Some(read) = read {
            state(|s| {
                s.snapshot = read;
                s.since_snapshot = 0;
            });
        }
    }
}

fn say(game: &mut Game<'_>, ru: &str, en: &str) {
    let text = crate::game::claim_borders_chat_view::current_language().text(ru, en);
    game.native_chat(text);
}

/// Starts mining the chosen blocks with the port (`mine a b c`) and watches the inventory: when
/// there is no room left the port is stopped, the bot unloads (a shulker box carried is put down,
/// filled, broken and picked up, or the set container is visited, `haul_view`) and the port mines
/// again. `names` are block ids; they also become the "own block" target, which tells the haul
/// what to keep one stack of.
pub fn mine_start(game: &mut Game<'_>, names: Vec<String>) -> Result<(), String> {
    use super::claim_borders_mine_picker as picker;
    if !baritone::enabled(game) {
        return Err("Baritone is off".into());
    }
    let line = picker::mine_line(&names).ok_or("Choose what to mine")?;
    super::claim_borders_mining::set_custom(names.clone());
    state(|s| {
        s.mine = Some(MineTask {
            names,
            phase: walk::MinePhase::Mining,
            idle: 0,
            age: 0,
        });
        s.since_snapshot = u32::MAX;
    });
    baritone::run_line(game, &line);
    Ok(())
}
/// The task is over: nothing watches the inventory any more.
pub fn mine_stop() {
    state(|s| s.mine = None);
}
/// The mining task was cancelled (the tab's Cancel, the Stop key, `#cancel`, the port switched
/// off): the watch ends, and so does the unloading that belongs to it, so that nothing is clicked,
/// placed, broken or walked to afterwards and nothing mines again. A trip of another job (the old
/// mining, the farm) is not touched.
pub fn mine_cancel(game: &mut Game<'_>) {
    if state(|s| s.mine.take().is_some()) {
        super::claim_borders_haul_view::abort(game);
        walk_cancel();
    }
}
pub fn mine_active() -> bool {
    state(|s| s.mine.is_some())
}
fn mine_tick(game: &mut Game<'_>) {
    use super::claim_borders_haul_view as haul;
    let Some((phase, idle, age, names)) = state(|s| {
        s.mine
            .as_ref()
            .map(|m| (m.phase, m.idle, m.age, m.names.clone()))
    }) else {
        return;
    };
    let full = super::claim_borders_navigation_view::inventory_full_for(
        game,
        super::claim_borders_mining::CUSTOM,
    );
    let busy = state(|s| s.snapshot.process.is_some());
    let (next, step, idle) = walk::mine_step(phase, full, haul::active(), busy, idle, age);
    state(|s| {
        if let Some(m) = s.mine.as_mut() {
            m.phase = next;
            m.idle = idle;
            m.age = age.saturating_add(1);
        }
    });
    match step {
        walk::MineStep::Wait => {}
        walk::MineStep::StartHaul => {
            with(game, |b, game| b.force_cancel(game));
            if !haul::begin(game, super::claim_borders_mining::CUSTOM) {
                mine_stop();
                say(
                    game,
                    "Инвентарь полон: задайте контейнер (Добыча → Склад) или носите шалкер.",
                    "The inventory is full: set a container (Mining → Storage) or carry a shulker box.",
                );
            }
        }
        walk::MineStep::Resume => {
            if let Some(line) = super::claim_borders_mine_picker::mine_line(&names) {
                baritone::run_line(game, &line);
                state(|s| s.since_snapshot = u32::MAX);
            }
        }
        walk::MineStep::Finished => {
            mine_stop();
            say(game, "Добыча завершена.", "Mining finished.");
        }
    }
}

/// The names of the items carried (hotbar and inventory), for choosing the block of a floor.
fn carried(game: &Game<'_>) -> Vec<String> {
    (0..36)
        .filter_map(|slot| {
            if slot < 9 {
                game.stack_in(slot)
            } else {
                game.inventory.get(slot).and_then(Option::as_ref)
            }
        })
        .map(|stack| stack.item.to_string())
        .collect()
}
/// Hands a schematic to the port's builder: it plans the walking, breaking and placing itself.
fn build(
    game: &mut Game<'_>,
    name: &str,
    origin: [i32; 3],
    schematic: std::rc::Rc<dyn super::baritone_schematic::Schematic>,
) -> Result<(), String> {
    if !baritone::enabled(game) {
        return Err("Baritone is off".into());
    }
    with(game, |b, game| {
        let baritone::Baritone { core, procs, .. } = b;
        procs
            .build
            .build(core, game, name.to_owned(), schematic, origin);
    })
    .ok_or_else(|| "Baritone is busy".to_owned())
}
fn feet(game: &mut Game<'_>) -> Result<[i32; 3], String> {
    with(game, |b, game| b.core.player_feet(game))
        .flatten()
        .ok_or_else(|| "Join a world first".to_owned())
}
/// A tunnel the way the player looks, with a floor when asked and a block for it is carried.
pub fn tunnel(
    game: &mut Game<'_>,
    length: u16,
    width: u8,
    height: u8,
    floor: bool,
) -> Result<(), String> {
    use super::claim_borders_baritone_shapes as shapes;
    let at = feet(game)?;
    let direction = super::claim_borders_construction::Direction::from_yaw(game.yaw);
    let items = carried(game);
    let block = if floor {
        shapes::floor_block(items.iter().map(String::as_str))
    } else {
        None
    };
    if floor && block.is_none() {
        say(
            game,
            "Нет блоков для пола: тоннель без пола.",
            "No blocks for a floor: a tunnel without one.",
        );
    }
    let (origin, schematic) = shapes::tunnel(
        at,
        direction,
        i32::from(length),
        i32::from(width),
        i32::from(height),
        block.as_deref(),
    );
    build(game, "tunnel", origin, schematic)
}
/// A bridge (a floor) the way the player looks, of a block they carry.
pub fn bridge(game: &mut Game<'_>, length: u16, width: u8) -> Result<(), String> {
    use super::claim_borders_baritone_shapes as shapes;
    let at = feet(game)?;
    let direction = super::claim_borders_construction::Direction::from_yaw(game.yaw);
    let items = carried(game);
    let Some(block) = shapes::floor_block(items.iter().map(String::as_str)) else {
        return Err(crate::game::claim_borders_chat_view::current_language()
            .text(
                "Нет блоков для моста: возьмите булыжник, камень или землю",
                "No blocks for a bridge: carry cobblestone, stone or dirt",
            )
            .to_owned());
    };
    let (origin, schematic) =
        shapes::bridge(at, direction, i32::from(length), i32::from(width), &block);
    build(game, "bridge", origin, schematic)
}
/// A pond filled in the pattern chosen, with sand, red sand or gravel carried.
pub fn drain(
    game: &mut Game<'_>,
    config: super::claim_borders_draining::Config,
) -> Result<(), String> {
    use super::claim_borders_baritone_shapes as shapes;
    let at = feet(game)?;
    let items = carried(game);
    let Some(block) = shapes::drain_block(items.iter().map(String::as_str)) else {
        return Err(crate::game::claim_borders_chat_view::current_language()
            .text(
                "Нет песка или гравия для осушения",
                "No sand or gravel to drain with",
            )
            .to_owned());
    };
    let (origin, schematic) = shapes::drain(at, config, &block);
    build(game, "drain", origin, schematic)
}

/// Whether `#pause` holds the port right now (not the half second old snapshot).
pub fn paused(game: &mut Game<'_>) -> bool {
    with(game, |b, _| b.procs.pause.paused).unwrap_or(false)
}

/// What the tab shows, as last read (at most half a second old).
pub fn snapshot() -> Snapshot {
    state(|s| s.snapshot.clone())
}

/// Sets entry `index` of `walk::SETTINGS` (a flag takes 0 or 1) and keeps it for the next game.
pub fn set_setting(game: &mut Game<'_>, index: usize, value: i32) -> Result<(), String> {
    let setting = walk::SETTINGS.get(index).ok_or("Unknown setting")?;
    let text = match setting.kind {
        walk::Kind::Flag => (value != 0).to_string(),
        walk::Kind::Int { min, max, .. } => value.clamp(min, max).to_string(),
    };
    with(game, |b, game| {
        // The port keeps the player's value for a setting a walk holds (and writes that to the
        // file); the walk's own values stay in force in memory.
        let result = b.core.set_setting(game, setting.name, &text).map(|_| ());
        if let Some(overrides) = state(|s| s.active.as_ref().map(|a| a.overrides)) {
            for (name, value) in overrides {
                let _ = b.core.settings.set(name, value);
            }
            b.core.refresh();
        }
        result
    })
    .unwrap_or_else(|| Err("Baritone is busy".into()))?;
    state(|s| s.since_snapshot = u32::MAX);
    Ok(())
}
