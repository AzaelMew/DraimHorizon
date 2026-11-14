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
    /// The value of each of `walk::SETTINGS` (a flag is 0 or 1); empty until the port has run.
    pub values: Vec<i32>,
}
struct State {
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
        owner: "",
        request: None,
        active: None,
        cancel: false,
        status: walk::Walk::Idle,
        snapshot: Snapshot { process: None, segment: None, goal: None, values: Vec::new() },
        since_snapshot: u32::MAX,
    }) };
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Whether the player asked for the port to do the farm's and mining's walking, and the port is on.
pub fn engine(game: &Game<'_>) -> bool {
    baritone::enabled(game)
        && game
            .native_setting(claims::SETTINGS_ID, "baritone_engine")
            .as_deref()
            == Some("true")
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

fn restore(b: &mut baritone::Baritone, saved: Saved) {
    for (name, value) in saved {
        let _ = b.core.settings.set(name, &value);
    }
    b.core.refresh();
}

/// Every tick, from the mod's tick hook: carries out a request or a cancel, watches the walk and
/// keeps the snapshot of the tab fresh.
pub fn tick(game: &mut Game<'_>) {
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
        return;
    }
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
        // A walk's temporary values must not reach the file: the player's choice is kept as the
        // value to restore, the saved values are put in for the write and the walk's come back.
        let held = state(|s| {
            s.active.as_mut().map(|a| {
                for entry in a.saved.iter_mut().filter(|(n, _)| *n == setting.name) {
                    entry.1.clone_from(&text);
                }
                (a.saved.clone(), a.overrides)
            })
        });
        if let Some((saved, _)) = &held {
            for (name, value) in saved {
                let _ = b.core.settings.set(name, value);
            }
        }
        let result = b.core.set_setting(game, setting.name, &text).map(|_| ());
        if let Some((saved, overrides)) = held {
            for (name, value) in overrides
                .iter()
                .filter(|(n, _)| saved.iter().any(|(m, _)| m == n))
            {
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
