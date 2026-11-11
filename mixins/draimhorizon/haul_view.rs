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
    /// A shulker box from the inventory is being put down beside the player.
    Place,
    /// The filled shulker box is being broken (it drops with what it holds).
    Break,
    /// Waiting for the dropped shulker box to be picked up (walking to it if needed).
    Pickup,
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
    /// How long the walk under way may take (set when it starts, from its length).
    limit: Duration,
    since: Instant,
    last: Instant,
    tries: u8,
    moved: u16,
    stuck: u8,
    sig: usize,
    /// The trip uses a shulker box from the inventory instead of the storage container.
    shulker: bool,
    /// Shulker boxes carried when the box was put down (one more means it is picked up).
    boxes: u32,
}
static TRIP: Mutex<Option<Trip>> = Mutex::new(None);
/// The shulker box item put down by the trip under way.
static BOX_ITEM: Mutex<Option<String>> = Mutex::new(None);
/// Shulker boxes in the inventory with room left: (slot, item), the emptiest first.
fn boxes_with_room(game: &Game<'_>) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, usize, String)> = (0..36)
        .filter_map(|slot| {
            let stack = if slot < 9 {
                game.stack_in(slot)
            } else {
                game.inventory.get(slot).and_then(Option::as_ref)
            }?;
            let filled = stack.container().len();
            (crate::game::claim_borders_shulker_peek::is_shulker(&stack.item) && filled < 27)
                .then(|| (filled, slot, stack.item.to_string()))
        })
        .collect();
    found.sort();
    found
        .into_iter()
        .map(|(_, slot, item)| (slot, item))
        .collect()
}
fn box_count(game: &Game<'_>) -> u32 {
    inventory(game)
        .into_iter()
        .flatten()
        .filter(|(name, _)| crate::game::claim_borders_shulker_peek::is_shulker(name))
        .map(|(_, n)| n)
        .sum()
}
/// Where to put a shulker box down: beside the player's feet, on firm ground, with room above
/// for its lid and a clear line from the eye.
fn box_spot(game: &Game<'_>) -> Option<nav::Pos> {
    let p = game.native_player()?;
    let feet = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    );
    let cell = |at: nav::Pos| placement::block(game, at).cell;
    [
        [1, 0],
        [-1, 0],
        [0, 1],
        [0, -1],
        [1, 1],
        [-1, -1],
        [1, -1],
        [-1, 1],
    ]
    .into_iter()
    .map(|[dx, dz]| [feet[0] + dx, feet[1], feet[2] + dz])
    .find(|at| {
        cell(*at) == nav::Cell::Air
            && cell([at[0], at[1] + 1, at[2]]) == nav::Cell::Air
            && cell([at[0], at[1] - 1, at[2]]) == nav::Cell::Solid
            && in_sight(game, feet, *at)
    })
}
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
    let Some(p) = game.native_player() else {
        return false;
    };
    // A shulker box carried with room in it is the nearest storage: put down, filled, broken
    // and picked up again, and mining goes on where it is.
    if let Some((_, item)) = boxes_with_room(game).into_iter().next()
        && let Some(spot) = box_spot(game)
    {
        *BOX_ITEM.lock().unwrap_or_else(|e| e.into_inner()) = Some(item);
        *TRIP.lock().unwrap_or_else(|e| e.into_inner()) = Some(Trip {
            step: Step::Place,
            chest: spot,
            origin: nav::block([p.x, p.y, p.z]),
            ore,
            walking: false,
            limit: Duration::from_secs(30),
            since: Instant::now(),
            last: Instant::now(),
            tries: 0,
            moved: 0,
            stuck: 0,
            sig: 0,
            shulker: true,
            boxes: box_count(game),
        });
        return true;
    }
    let Some(chest) = workshop::storage(game) else {
        return false;
    };
    *TRIP.lock().unwrap_or_else(|e| e.into_inner()) = Some(Trip {
        shulker: false,
        boxes: 0,
        step: Step::Walk,
        chest,
        origin: nav::block([p.x, p.y, p.z]),
        ore,
        walking: false,
        limit: Duration::from_secs(90),
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
                    || workshop::on_container(game, at)
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
    crate::game::claim_borders_shulker_peek::is_shulker(item)
        || game.res.items().items.get(item).is_some_and(|info| {
            info.max_damage > 0 || info.food.is_some() || item == "minecraft:totem_of_undying"
        })
}
/// How long a walk to `goal` may take: ninety seconds, or the way at two blocks a second, so a
/// storage 800 blocks away still has its time.
fn walk_time(game: &Game<'_>, goal: nav::Pos) -> Duration {
    let from = game
        .native_player()
        .map_or(nav::center(goal), |p| [p.x, p.y, p.z]);
    let way = (nav::center(goal)[0] - from[0]).hypot(nav::center(goal)[2] - from[2]);
    Duration::from_secs(90.max((way / 2.0) as u64 + 30))
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
                t.limit = walk_time(game, spot);
                save(t);
            } else if t.since.elapsed() > t.limit {
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
        Step::Place => {
            let item = BOX_ITEM.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let Some(item) = item else {
                end(game, false, 30);
                return;
            };
            if game.native_block(t.chest) == item {
                // It stands: open it as any container.
                t.step = Step::Open;
                t.since = Instant::now();
                save(t);
                placement::open_container(t.chest);
            } else if !t.walking {
                let Ok(state) = crate::game::claim_borders_schematic::BlockState::parse(&item)
                else {
                    end(game, false, 60);
                    return;
                };
                let request = crate::game::claim_borders_schematic::PlacementRequest {
                    target: t.chest,
                    item: item.clone(),
                    state: state.clone(),
                    allowed: std::collections::BTreeMap::from([(t.chest, state)]),
                };
                if placement::place_external(game, request).is_ok() {
                    t.walking = true;
                    t.since = Instant::now();
                    save(t);
                } else {
                    end(game, false, 30);
                }
            } else if matches!(placement::external_result(), Some(Err(_)))
                || t.since.elapsed() > Duration::from_secs(5)
            {
                // Refused or nothing came of it: the storage container, if any, next time.
                end(game, false, 30);
            }
        }
        Step::Break => {
            if game.native_block(t.chest).contains("shulker_box") {
                // The player holding the camera holds the time limit back too.
                if walk::mouse_has_camera() {
                    t.since = Instant::now();
                }
                if t.since.elapsed() > Duration::from_secs(15) {
                    end(game, false, 60);
                    return;
                }
                // Turn to the box as a person would, then hold the attack with the right tool
                // (not while the player has the camera).
                if !walk::mouse_has_camera()
                    && let Some(eye) = game.eye_position()
                {
                    let middle = nav::center(t.chest);
                    let middle = [middle[0], middle[1] + 0.5, middle[2]];
                    let (yaw, pitch) = (nav::yaw_to(eye, middle), nav::pitch_to(eye, middle));
                    game.yaw = nav::turn_toward(game.yaw, yaw, 30.0);
                    game.pitch = nav::turn_toward(game.pitch, pitch, 24.0);
                    let aimed = nav::angle_between(game.yaw, yaw).abs() < 4.0
                        && (game.pitch - pitch).abs() < 4.0
                        && game.pick().is_some_and(|h| h.pos == t.chest);
                    if aimed && walk::tool_for(game, t.chest) {
                        walk::press_for_click(game, false);
                    }
                }
            } else {
                t.step = Step::Pickup;
                t.since = Instant::now();
                t.walking = false;
                save(t);
            }
        }
        Step::Pickup => {
            if box_count(game) > t.boxes.saturating_sub(1) {
                end(game, true, 5);
            } else if t.since.elapsed() > Duration::from_secs(12) {
                end(game, false, 60);
            } else if !t.walking && t.since.elapsed() > Duration::from_millis(1500) {
                // Not picked up from where it stands: walk onto the spot it dropped at.
                if go(game, t.chest).is_ok() {
                    t.walking = true;
                    save(t);
                }
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
                t.limit = walk_time(game, t.origin);
                save(t);
            } else if walk::workshop_walk_result().is_some() || t.since.elapsed() > t.limit {
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
    let sig = plan.iter().sum::<usize>() + plan.len() * 100;
    let stuck = if sig == t.sig { t.stuck + 1 } else { 0 };
    if plan.is_empty() || (t.shulker && stuck >= 3) {
        // Unloaded (or the box is full): close the window; walk back, or break the box.
        if game.screen == Screen::Inventory {
            game.close_container();
            game.screen = Screen::Playing;
            game.capture(true);
        }
        t.step = if t.shulker { Step::Break } else { Step::Back };
        t.walking = false;
        t.since = Instant::now();
        save(t);
        return;
    }
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
        Some(Step::Place) => text("Шалкер: ставлю", "Shulker: putting it down"),
        Some(Step::Break) => text("Шалкер: забираю", "Shulker: taking it back"),
        Some(Step::Pickup) => text("Шалкер: подбираю", "Shulker: picking it up"),
        None => "",
    }
}
