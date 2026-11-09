// A trip to the storage container for a mining job whose inventory is full: walk there, shift-click
// everything the miner does not need into it, walk back and go on. The storage is the one the
// farm uses (set by aiming at a chest, a barrel or a shulker box).
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_construction_view as placement,
    claim_borders_farm_storage as store, claim_borders_mining as mining,
    claim_borders_navigation as nav, claim_borders_navigation_view as walk,
    claim_borders_workshop_view as workshop,
};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Walking to a spot within reach of the container.
    Walk,
    /// Waiting for the container window to open.
    Open,
    /// Shift-clicking stacks across.
    Transfer,
    /// Walking back to where the mining stopped.
    Back,
}
#[derive(Clone, Copy)]
struct Trip {
    step: Step,
    chest: nav::Pos,
    /// Where the player stood when the inventory filled up.
    origin: nav::Pos,
    ore: u8,
    /// A walk has been asked for and its result is awaited.
    walking: bool,
    since: Instant,
    last: Instant,
    tries: u8,
    moved: u16,
    stuck: u8,
    sig: usize,
}
static TRIP: Mutex<Option<Trip>> = Mutex::new(None);
/// Not another trip before this instant.
static AFTER: Mutex<Option<Instant>> = Mutex::new(None);
fn text(russian: &'static str, english: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(russian, english)
}
/// Whether a trip is under way (mining then stands still).
pub(super) fn active() -> bool {
    TRIP.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}
/// Starts a trip when the storage is set and the last one was not just now. The first step is
/// taken by `frame`, outside the navigator's lock. True when a trip is under way.
pub(super) fn begin(game: &Game<'_>, ore: u8) -> bool {
    if active() {
        return true;
    }
    if AFTER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|at| Instant::now() < at)
    {
        return false;
    }
    let (Some(chest), Some(p)) = (workshop::storage(game), game.native_player()) else {
        return false;
    };
    *TRIP.lock().unwrap_or_else(|e| e.into_inner()) = Some(Trip {
        step: Step::Walk,
        chest,
        origin: nav::block([p.x, p.y, p.z]),
        ore,
        walking: false,
        since: Instant::now(),
        last: Instant::now(),
        tries: 0,
        moved: 0,
        stuck: 0,
        sig: 0,
    });
    true
}
/// Walks to `goal` with the mining job put aside (a walk starts the navigator over).
fn go(game: &Game<'_>, goal: nav::Pos) -> Result<(), String> {
    walk::haul_stash();
    walk::workshop_walk(game, goal, false, false)
}
fn save(t: Trip) {
    *TRIP.lock().unwrap_or_else(|e| e.into_inner()) = Some(t);
}
/// Ends the trip: the window is closed, the next trip waits `cooldown` seconds and mining
/// goes on (`ok`) or idles with the inventory full.
fn end(game: &mut Game<'_>, ok: bool, cooldown: u64) {
    if game.screen == Screen::Inventory {
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    placement::cancel_container();
    *TRIP.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *AFTER.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(Instant::now() + Duration::from_secs(cooldown));
    walk::haul_ended(ok);
}
/// Whether nothing but air lies between the eye at `feet` and the middle of the container.
fn in_sight(game: &Game<'_>, feet: nav::Pos, chest: nav::Pos) -> bool {
    let eye = [
        f64::from(feet[0]) + 0.5,
        f64::from(feet[1]) + 1.62,
        f64::from(feet[2]) + 0.5,
    ];
    let goal = nav::center(chest);
    let goal = [goal[0], goal[1] + 0.5, goal[2]];
    let steps = (nav::distance(eye, goal) / 0.1).ceil() as i32;
    (1..steps).all(|i| {
        let t = f64::from(i) / f64::from(steps);
        let at = [
            (eye[0] + (goal[0] - eye[0]) * t).floor() as i32,
            (eye[1] + (goal[1] - eye[1]) * t).floor() as i32,
            (eye[2] + (goal[2] - eye[2]) * t).floor() as i32,
        ];
        at == chest
            || at == feet
            || matches!(
                game.native_block(at).as_str(),
                "minecraft:air" | "minecraft:cave_air"
            )
    })
}
/// Spots to stand on within reach of the container, ones with a clear line to it first.
fn spots(game: &Game<'_>, chest: nav::Pos, from: [f64; 3]) -> Vec<nav::Pos> {
    let mut found = Vec::new();
    for y in [chest[1], chest[1] - 1, chest[1] + 1, chest[1] - 2] {
        for dz in -3..=3_i32 {
            for dx in -3..=3_i32 {
                let at = [chest[0] + dx, y, chest[2] + dz];
                if (dx == 0 && dz == 0)
                    || nav::distance(nav::center(at), nav::center(chest)) > 3.4
                    || !nav::standable(at, &mut |p| placement::block(game, p).cell)
                {
                    continue;
                }
                found.push(at);
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    found.sort_by(|a, b| {
        let key = |at: &nav::Pos| {
            (
                !in_sight(game, *at, chest),
                nav::distance(nav::center(*at), from) as i64,
            )
        };
        key(a).cmp(&key(b))
    });
    found
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
/// What the miner keeps: tools and armour (anything that wears), food and a totem.
fn needed(game: &Game<'_>, item: &str) -> bool {
    game.res.items().items.get(item).is_some_and(|info| {
        info.max_damage > 0 || info.food.is_some() || item == "minecraft:totem_of_undying"
    })
}
/// Drives the trip each tick.
pub(super) fn frame(game: &mut Game<'_>) {
    let Some(mut t) = *TRIP.lock().unwrap_or_else(|e| e.into_inner()) else {
        return;
    };
    if !game.native_on(claims::ID) || game.native_player().is_none() {
        end(game, false, 30);
        return;
    }
    match t.step {
        Step::Walk => {
            let Some(p) = game.native_player() else {
                return;
            };
            if !t.walking {
                let spots = spots(game, t.chest, [p.x, p.y, p.z]);
                let Some(spot) = spots
                    .get(usize::from(t.tries) % spots.len().max(1))
                    .copied()
                else {
                    end(game, false, 60);
                    return;
                };
                if go(game, spot).is_err() {
                    end(game, false, 60);
                    return;
                }
                t.walking = true;
                t.since = Instant::now();
                save(t);
            } else if t.since.elapsed() > Duration::from_secs(90) {
                end(game, false, 60);
            } else {
                match walk::workshop_walk_result() {
                    Some(true) => {
                        t.step = Step::Open;
                        t.since = Instant::now();
                        save(t);
                        placement::open_container(t.chest);
                    }
                    Some(false) => {
                        t.tries += 1;
                        t.walking = false;
                        if t.tries >= 4 {
                            end(game, false, 60);
                        } else {
                            save(t);
                        }
                    }
                    None => {}
                }
            }
        }
        Step::Open => {
            let open = game.screen == Screen::Inventory
                && game
                    .remote_menu_type()
                    .is_some_and(store::is_container_menu);
            if open {
                t.step = Step::Transfer;
                t.last = Instant::now();
                save(t);
            } else if placement::container_blocked()
                || t.since.elapsed() > Duration::from_millis(2500)
            {
                placement::cancel_container();
                t.tries += 1;
                if t.tries >= 4 {
                    end(game, false, 60);
                } else {
                    // Hidden from here, or the click did not open it: another spot.
                    t.step = Step::Walk;
                    t.walking = false;
                    save(t);
                }
            }
        }
        Step::Transfer => {
            let open = game.screen == Screen::Inventory
                && game
                    .remote_menu_type()
                    .is_some_and(store::is_container_menu);
            if !open {
                end(game, false, 30);
            } else if t.last.elapsed() >= Duration::from_millis(160) {
                transfer(game, t);
            }
        }
        Step::Back => {
            if !t.walking {
                if go(game, t.origin).is_err() {
                    end(game, true, 5);
                    return;
                }
                t.walking = true;
                t.since = Instant::now();
                save(t);
            } else if walk::workshop_walk_result().is_some()
                || t.since.elapsed() > Duration::from_secs(60)
            {
                // Back (or as near as it gets): the search finds the next block from here.
                end(game, true, 5);
            }
        }
    }
}
/// One round of shift-clicks from the inventory into the open container.
fn transfer(game: &mut Game<'_>, mut t: Trip) {
    use horizon_client::menu::Input;
    let inv = inventory(game);
    let kept = |name: &str| mining::matches(t.ore, name);
    let plan = store::deposit_mined(&inv, &|name| needed(game, name), &kept, 64);
    if plan.is_empty() {
        // Unloaded: close the window and walk back.
        if game.screen == Screen::Inventory {
            game.close_container();
            game.screen = Screen::Playing;
            game.capture(true);
        }
        t.step = Step::Back;
        t.walking = false;
        save(t);
        return;
    }
    let sig = plan.iter().sum::<usize>() + plan.len() * 100;
    let stuck = if sig == t.sig { t.stuck + 1 } else { 0 };
    if stuck >= 3 || t.moved >= 300 {
        // Nothing moves any more: the container is full or refuses the stacks.
        end(game, false, 120);
        return;
    }
    let ops: Vec<_> = plan
        .iter()
        .take(4)
        .map(|g| (Some(*g), 0, Input::QuickMove))
        .collect();
    if !game.menu_ops(&ops) {
        end(game, false, 30);
        return;
    }
    t.last = Instant::now();
    t.moved += ops.len() as u16;
    t.stuck = stuck;
    t.sig = sig;
    save(t);
}
/// The label of the status line while a trip is under way.
pub(super) fn status() -> &'static str {
    match TRIP
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map(|t| t.step)
    {
        Some(Step::Walk) => text("Склад: иду к нему", "Storage: walking there"),
        Some(Step::Open) => text("Склад: открываю", "Storage: opening it"),
        Some(Step::Transfer) => text("Склад: разгрузка", "Storage: unloading"),
        Some(Step::Back) => text("Склад: возвращаюсь", "Storage: walking back"),
        None => "",
    }
}
