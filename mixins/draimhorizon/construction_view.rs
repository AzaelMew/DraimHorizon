// Native construction driver: ordinary digging and block use, confirmed by server acknowledgements.
use crate::game::claim_borders_schematic as schematic;
use crate::game::{
    Game, claim_borders as claims, claim_borders_construction as model,
    claim_borders_draining as drain, claim_borders_navigation as nav,
    claim_borders_navigation_view as navigation,
};
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct External {
    request: schematic::PlacementRequest,
    active: bool,
    result: Option<Result<usize, schematic::PlaceError>>,
    confirm: Vec<(nav::Pos, schematic::BlockState)>,
}
struct State {
    external: Option<External>,
    job: Option<model::Job>,
    scope: String,
    placement: Option<model::Placement>,
    auto_slot: Option<usize>,
    original_slot: Option<usize>,
    aim: Option<[f32; 2]>,
    started: Instant,
    action_ms: u64,
    fetches: u8,
    fetch_ms: u64,
    diag: &'static str,
}
impl Default for State {
    fn default() -> Self {
        Self {
            external: None,
            job: None,
            scope: String::new(),
            placement: None,
            auto_slot: None,
            original_slot: None,
            aim: None,
            started: Instant::now(),
            action_ms: 0,
            fetches: 0,
            fetch_ms: 0,
            diag: "",
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
fn language() -> claim_borders_relay_client::translation::Language {
    crate::game::claim_borders_chat_view::current_language()
}
pub(super) fn block(game: &Game<'_>, at: nav::Pos) -> model::Block {
    let Some(chunk) = game
        .world
        .chunks
        .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
    else {
        return model::Block::default();
    };
    if at[1] < chunk.min_y() || at[1] >= chunk.max_y() {
        return model::Block::default();
    }
    let facts = game.res.facts().get(game.world.block(at[0], at[1], at[2]));
    let cell = nav::classify(
        &facts.name,
        facts.full_collision,
        facts.fluid_level > 0 || facts.waterlogged,
    );
    model::Block {
        cell,
        diggable: cell == nav::Cell::Solid && facts.hardness >= 0.0 && !facts.block_entity,
        falling: crate::game::claim_borders_mining::falling(&facts.name),
    }
}
pub(super) fn reset() {
    state(|s| {
        s.job = None;
        if !s.placement.as_ref().is_some_and(|p| p.sequence.is_some()) {
            s.placement = None;
            s.external = None;
        }
        s.aim = None;
    });
}
pub(super) fn world_reset() {
    state(|s| *s = State::default());
}
pub(super) fn exists() -> bool {
    state(|s| s.job.is_some())
}
/// (running, resumable) for the action keys; see `workshop_view::control`.
pub(super) fn control() -> Option<(bool, bool)> {
    state(|s| {
        s.job.as_ref().map(|j| {
            (
                j.active,
                !j.preview && !j.active && j.phase != model::Phase::Done,
            )
        })
    })
}
pub(super) fn pause() {
    state(|s| {
        let unsent = s.placement.as_ref().is_some_and(|p| p.sequence.is_none());
        if let Some(j) = s.job.as_mut()
            && j.active
        {
            j.stop(model::Stop::Manual);
        }
        s.aim = None;
        if let Some(external) = s.external.as_mut() {
            external.active = false;
            if unsent && external.result.is_none() {
                external.result = Some(Err(schematic::PlaceError::Manual));
            }
        }
    });
}
fn stop(reason: model::Stop) {
    state(|s| {
        if let Some(j) = s.job.as_mut() {
            j.stop(reason);
        }
        s.aim = None;
        if let Some(external) = s.external.as_mut() {
            external.active = false;
            external.result = Some(Err(match reason {
                model::Stop::Material => schematic::PlaceError::Material,
                model::Stop::Manual => schematic::PlaceError::Manual,
                model::Stop::Timeout => schematic::PlaceError::Timeout,
                _ => schematic::PlaceError::Unreachable,
            }));
        }
    });
}
/// Where a block to place can be taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Hotbar(usize),
    Offhand,
}
/// Whether the selected hand would place a block of its own before the off hand gets a turn.
fn main_hand_places(game: &Game<'_>) -> bool {
    game.held().is_some_and(|stack| {
        game.res
            .items()
            .get(&stack.item)
            .is_some_and(|item| item.block.is_some())
    })
}
fn source_for(game: &Game<'_>, wanted: &dyn Fn(&str) -> bool) -> Option<Source> {
    // The cheapest matching stack goes first, so plentiful stone is spent before valuables.
    (0..9)
        .filter(|slot| {
            game.stack_in(*slot)
                .is_some_and(|stack| stack.count > 0 && wanted(&stack.item))
        })
        .min_by_key(|slot| {
            game.stack_in(*slot)
                .map_or(1, |stack| model::rank(&stack.item))
        })
        .map(Source::Hotbar)
        .or_else(|| {
            // Vanilla offers the off hand the click once the main hand passed it, so a
            // pickaxe in the main hand and blocks in the off hand place normally.
            (!main_hand_places(game)
                && game
                    .offhand
                    .as_ref()
                    .is_some_and(|stack| stack.count > 0 && wanted(&stack.item)))
            .then_some(Source::Offhand)
        })
}
fn inventory_slot(game: &Game<'_>, wanted: &dyn Fn(&str) -> bool) -> Option<usize> {
    (9..36)
        .filter(|slot| {
            game.inventory
                .get(*slot)
                .and_then(Option::as_ref)
                .is_some_and(|stack| stack.count > 0 && wanted(&stack.item))
        })
        .min_by_key(|slot| {
            game.inventory
                .get(*slot)
                .and_then(Option::as_ref)
                .map_or(1, |stack| model::rank(&stack.item))
        })
}
/// Whether a matching stack exists in the hotbar, the off hand or the main inventory.
pub(super) fn has_item(game: &Game<'_>, wanted: &dyn Fn(&str) -> bool) -> bool {
    source_for(game, wanted).is_some() || inventory_slot(game, wanted).is_some()
}
enum Fetch {
    Waiting,
    Missing,
}
/// Moves one matching stack from the main inventory into the hotbar with a single swap click,
/// the click the inventory screen sends for a number key. The selected slot is never touched,
/// the click is rate limited and bounded per task, and the server's answer is awaited.
fn fetch(game: &mut Game<'_>, wanted: &dyn Fn(&str) -> bool) -> Fetch {
    use horizon_client::menu::Input;
    if game.remote.is_none() || !game.native_playing() {
        return Fetch::Missing;
    }
    let Some(from) = inventory_slot(game, wanted) else {
        return Fetch::Missing;
    };
    let target = (0..9)
        .filter(|slot| *slot != game.selected)
        .find(|slot| game.stack_in(*slot).is_none())
        .or_else(|| (0..9).rev().find(|slot| *slot != game.selected));
    let Some(target) = target else {
        return Fetch::Missing;
    };
    let ready = state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        if s.fetches >= 24 {
            return None;
        }
        if now.saturating_sub(s.fetch_ms) < 500 && s.fetch_ms != 0 {
            return Some(false);
        }
        s.fetch_ms = now.max(1);
        s.fetches += 1;
        Some(true)
    });
    match ready {
        None => Fetch::Missing,
        Some(false) => Fetch::Waiting,
        Some(true) => {
            if game.remote_menu_ops(&[(Some(from), target as i32, Input::Swap)]) {
                Fetch::Waiting
            } else {
                Fetch::Missing
            }
        }
    }
}
/// Whether the block an item places is a plain, solid cube: no container, no fluid, no
/// shape that is not a full block, nothing unbreakable. The name filter alone cannot know.
fn solid_cube(game: &Game<'_>, name: &str) -> bool {
    let Some(id) = game.pack.assets.registry.default_state(name) else {
        return false;
    };
    let facts = game.res.facts().get(id);
    facts.full_collision
        && !facts.block_entity
        && facts.fluid_level == 0
        && !facts.waterlogged
        && facts.hardness >= 0.0
}
/// Every distinct item the player carries that may be spent on a floor, a bridge or, with
/// `fill`, on filling water. Taken from the hotbar, the off hand and the main inventory.
pub(super) fn building_items(game: &Game<'_>, fill: bool) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let stacks = (0..9)
        .filter_map(|slot| game.stack_in(slot))
        .chain(game.offhand.as_ref())
        .chain((9..36).filter_map(|slot| game.inventory.get(slot).and_then(Option::as_ref)));
    for stack in stacks {
        let name: &str = &stack.item;
        let allowed = if fill {
            model::fill_material(name)
        } else {
            model::material(name)
        };
        if stack.count > 0 && allowed && !names.iter().any(|n| n == name) && solid_cube(game, name)
        {
            names.push(name.to_owned());
        }
    }
    names
}
/// What to fill water with, and whether it is a falling block. Sand and gravel come first:
/// dropped into the surface cell they sink and stack from the bottom, whatever the depth.
/// Anything else is the cheapest building block, which only stays where it is placed.
pub(super) fn drain_item(game: &Game<'_>) -> Option<(String, bool)> {
    let items = building_items(game, true);
    items
        .iter()
        .find(|name| model::gravity(name))
        .map(|name| (name.clone(), true))
        .or_else(|| {
            items
                .into_iter()
                .min_by_key(|name| model::rank(name))
                .map(|name| (name, false))
        })
}
fn placing(s: &State) -> bool {
    s.external
        .as_ref()
        .is_some_and(|e| e.active && e.result.is_none())
        || s.job
            .as_ref()
            .is_some_and(|j| j.active && j.phase == model::Phase::Placing)
}
/// Why the last placement found no usable face, for the status line.
pub(super) fn diagnostic() -> String {
    let l = language();
    let why = state(|s| s.diag);
    match why {
        "" => String::new(),
        "support" => l.text("нет опоры рядом", "no support block nearby").into(),
        "aim" => l
            .text(
                "луч прицела упирается в соседний блок",
                "the aim ray hits another block first",
            )
            .into(),
        "entity" => l
            .text("на пути моб или предмет", "an entity is in the way")
            .into(),
        "interactive" => l
            .text(
                "опора — интерактивный блок",
                "the support block is interactive",
            )
            .into(),
        "refused" => l
            .text(
                "клиент отказывается ставить сюда",
                "the client refuses to place here",
            )
            .into(),
        _ => l
            .text("ставится не тот блок", "a different block would be placed")
            .into(),
    }
}
pub(super) fn placement_pending() -> bool {
    state(|s| s.placement.as_ref().is_some_and(|p| p.sequence.is_some()))
}
pub(super) fn external_settled() -> bool {
    state(|s| s.auto_slot.is_none())
}
pub(super) fn external_result() -> Option<Result<usize, schematic::PlaceError>> {
    state(|s| s.external.as_ref().and_then(|e| e.result))
}
pub(super) fn external_resume() -> bool {
    state(|s| {
        if let Some(e) = s.external.as_mut()
            && s.placement.as_ref().is_some_and(|p| p.sequence.is_some())
        {
            e.active = true;
            e.result = None;
            if let Some(p) = s.placement.as_mut() {
                p.started_ms = s.started.elapsed().as_millis() as u64;
            }
            true
        } else {
            false
        }
    })
}
pub(super) fn place_external(
    game: &Game<'_>,
    request: schematic::PlacementRequest,
) -> Result<(), String> {
    if placement_pending() {
        return Err("waiting for placement acknowledgement".into());
    }
    let scope = navigation::scope(game).ok_or("no active world")?;
    state(|s| {
        s.scope = scope;
        s.job = None;
        s.placement = Some(model::Placement {
            target: request.target,
            anchor: None,
            material: request.state.name.clone(),
            sequence: None,
            started_ms: s.started.elapsed().as_millis() as u64,
            edge: false,
        });
        s.external = Some(External {
            request,
            active: true,
            result: None,
            confirm: Vec::new(),
        });
        s.aim = None;
    });
    Ok(())
}
pub(super) fn start(game: &Game<'_>, config: model::Config, preview: bool) -> Result<(), String> {
    let l = language();
    if navigation::construction_dig_pending() {
        return Err(l
            .text(
                "Дождитесь подтверждения разрушения сервером",
                "Wait for the server digging acknowledgement",
            )
            .into());
    }
    if state(|s| {
        navigation::scope(game).as_deref() == Some(s.scope.as_str())
            && s.placement.as_ref().is_some_and(|p| p.sequence.is_some())
    }) {
        return Err(l
            .text(
                "Дождитесь подтверждения установки сервером",
                "Wait for the server placement acknowledgement",
            )
            .into());
    }
    if !game.native_on(claims::ID)
        || game
            .native_setting(claims::SETTINGS_ID, "experiments_accepted")
            .as_deref()
            != Some("true")
    {
        return Err(l
            .text(
                "Подтвердите экспериментальные функции",
                "Accept experimental features first",
            )
            .into());
    }
    let player = game.native_player().ok_or_else(|| {
        l.text("Сначала войдите в мир", "Join a world first")
            .to_owned()
    })?;
    if game.remote.is_none() || !game.player.as_ref().is_some_and(|p| p.ground.on_ground) {
        return Err(l
            .text(
                "Встаньте на твёрдый блок на сервере",
                "Stand on solid ground on a server",
            )
            .into());
    }
    let origin = nav::block([player.x, player.y, player.z]);
    let job = model::Job::new(config, origin, preview).ok_or_else(|| {
        l.text("Недопустимые размеры прохода", "Invalid passage dimensions")
            .to_owned()
    })?;
    if block(game, [origin[0], origin[1] - 1, origin[2]]).cell != nav::Cell::Solid {
        return Err(l
            .text("Нужна твёрдая опора", "Solid support is required")
            .into());
    }
    if !preview && config.kind == model::Kind::Tunnel && !navigation::construction_tool(game) {
        return Err(l
            .text("Возьмите исправную кирку", "Hold a usable pickaxe")
            .into());
    }
    if !preview
        && (config.fill_floor || config.kind == model::Kind::Bridge)
        && building_items(game, false).is_empty()
    {
        return Err(l
            .text(
                "Добавьте строительные блоки в инвентарь (камень, земля, доски…)",
                "Put building blocks in your inventory (stone, dirt, planks…)",
            )
            .into());
    }
    navigation::clear_construction_driver();
    state(|s| {
        s.scope = navigation::scope(game).unwrap();
        s.job = Some(job);
        s.placement = None;
        s.aim = None;
        s.action_ms = s.started.elapsed().as_millis() as u64;
    });
    Ok(())
}
pub(super) fn resume(game: &Game<'_>) -> Result<(), String> {
    let valid =
        state(|s| s.job.is_some() && navigation::scope(game).as_deref() == Some(s.scope.as_str()));
    if !valid {
        return Err(language()
            .text(
                "Мир изменился; задайте новую задачу",
                "World changed; start a new task",
            )
            .into());
    }
    let pending_dig = navigation::construction_dig_pending();
    if pending_dig {
        navigation::construction_resume_dig();
    } else {
        navigation::clear_construction_driver();
    }
    state(|s| {
        let pending = s.placement.as_ref().is_some_and(|p| p.sequence.is_some());
        if let Some(j) = s.job.as_mut() {
            j.resume();
            if pending_dig {
                j.phase = model::Phase::Digging;
            } else if pending {
                j.phase = model::Phase::Placing;
            }
        }
        if !pending {
            s.placement = None;
        }
        s.aim = None;
        s.action_ms = s.started.elapsed().as_millis() as u64;
    });
    Ok(())
}
pub(super) fn tick(game: &Game<'_>) {
    let timeout = state(|s| {
        s.external
            .as_ref()
            .is_some_and(|e| e.active && e.result.is_none())
            && s.placement
                .as_ref()
                .is_some_and(|p| s.started.elapsed().as_millis() as u64 - p.started_ms > 5000)
    });
    if timeout {
        stop(model::Stop::Timeout);
    }
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    let current = navigation::scope(game);
    if state(|s| {
        (s.job.is_some() || s.external.is_some()) && current.as_deref() != Some(s.scope.as_str())
    }) {
        world_reset();
        return;
    }
    if !game.native_playing() {
        pause();
        return;
    }
    let dig = navigation::construction_dig_result();
    let action = state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        let Some(j) = s.job.as_mut().filter(|j| j.active) else {
            return None;
        };
        if !matches!(
            j.phase,
            model::Phase::Ready | model::Phase::Done | model::Phase::Stopped(_)
        ) && now.saturating_sub(s.action_ms)
            > if j.phase == model::Phase::Digging {
                35_000
            } else {
                10_000
            }
        {
            j.stop(model::Stop::Timeout);
            return None;
        }
        if j.phase == model::Phase::Digging {
            match dig {
                Some(true) => {
                    j.broken += 1;
                    j.phase = model::Phase::Ready;
                }
                Some(false) => j.stop(model::Stop::Blocked),
                None => {}
            }
        }
        if j.phase != model::Phase::Ready || s.auto_slot.is_some() {
            return None;
        }
        s.action_ms = now;
        let action = j.next(|at| block(game, at));
        match action {
            model::Action::Break(_) => j.phase = model::Phase::Digging,
            model::Action::Place(target) => {
                j.phase = model::Phase::Placing;
                s.placement = Some(model::Placement {
                    target,
                    anchor: None,
                    material: String::new(),
                    sequence: None,
                    started_ms: now,
                    edge: false,
                });
                s.aim = None;
            }
            model::Action::Walk(_) => j.phase = model::Phase::Walking,
            model::Action::Done => {
                j.active = false;
                j.phase = model::Phase::Done;
            }
            model::Action::Stop(reason) => j.stop(reason),
        }
        Some(action)
    });
    if let Some(model::Action::Break(at)) = action
        && navigation::construction_dig(game, at).is_err()
    {
        stop(model::Stop::Tool);
    }
}
pub(super) fn movement(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) -> bool {
    if !game.native_on(claims::ID)
        || state(|s| {
            s.job.is_some() && navigation::scope(game).as_deref() != Some(s.scope.as_str())
        })
    {
        reset();
        return false;
    }
    if !state(|s| s.job.as_ref().is_some_and(|j| j.active)) {
        return false;
    }
    if *forward != 0.0 || *strafe != 0.0 || *jump || *sneak || !game.native_playing() {
        pause();
        navigation::pause();
        return false;
    }
    let Some(p) = game.native_player() else {
        pause();
        return false;
    };
    let position = [p.x, p.y, p.z];
    if game.player.as_ref().is_none_or(|p| {
        !p.ground.on_ground
            || p.flying
            || p.fall_flying
            || p.is_passenger
            || p.fluids.was_touching_water
            || p.is_in_lava()
            || p.health.health() <= 0.0
    }) {
        pause();
        navigation::pause();
        return false;
    }
    state(|s| {
        let Some(j) = s.job.as_mut().filter(|j| j.active) else {
            return false;
        };
        let mut input = nav::Input::default();
        let station = j.station(j.completed);
        match j.phase {
            model::Phase::Align => {
                if nav::distance(position, nav::center(station)) > 1.5
                    || block(game, [station[0], station[1] - 1, station[2]]).cell
                        != nav::Cell::Solid
                {
                    j.stop(model::Stop::Floor);
                } else if nav::distance(position, nav::center(station)) < 0.1 {
                    j.phase = model::Phase::Ready;
                } else {
                    input = model::steer(position, nav::center(station), p.yaw);
                }
            }
            model::Phase::Walking => {
                let goal = j.station(j.completed + 1);
                let mut read = |at| block(game, at).cell;
                if !nav::standable(goal, &mut read) {
                    j.stop(model::Stop::Blocked);
                } else if nav::distance(position, nav::center(goal)) < 0.12 {
                    j.completed += 1;
                    j.phase = model::Phase::Ready;
                    // Keep walking when the next section is already open: no pause per block.
                    if let model::Action::Walk(next) = j.next(|at| block(game, at)) {
                        j.phase = model::Phase::Walking;
                        input = model::steer(position, nav::center(next), p.yaw);
                    }
                } else {
                    input = model::steer(position, nav::center(goal), p.yaw);
                }
            }
            model::Phase::Placing
                if s.placement
                    .as_ref()
                    .is_some_and(|p| p.edge && p.sequence.is_none()) =>
            {
                if !j.edge_safe(position)
                    || block(game, [station[0], station[1] - 1, station[2]]).cell
                        != nav::Cell::Solid
                {
                    j.stop(model::Stop::Floor);
                } else {
                    input = model::steer(position, j.edge(), p.yaw);
                }
            }
            _ => {}
        }
        *forward = input.forward;
        *strafe = input.strafe;
        *jump = false;
        // Only the edge of the bridge needs a sneak. Walking onto a block the server has
        // already confirmed is done at full speed instead of the sneaking crawl.
        *sneak = j.phase == model::Phase::Placing;
        true
    })
}
fn aim(game: &mut Game<'_>, point: [f64; 3]) {
    if let Some(eye) = game.eye_position() {
        let [x, y, z] = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
        game.yaw = (-x).atan2(z).to_degrees() as f32;
        game.pitch = (-y).atan2(x.hypot(z)).to_degrees() as f32;
    }
}
pub(super) fn input(game: &mut Game<'_>) -> bool {
    let current = navigation::scope(game);
    state(|s| {
        if current.as_deref() != Some(s.scope.as_str()) {
            *s = State::default();
            return;
        }
        if let Some(auto) = s.auto_slot {
            if game.selected != auto {
                if let Some(j) = s.job.as_mut() {
                    j.stop(model::Stop::Manual);
                }
                s.original_slot = None;
                s.auto_slot = None;
                if let Some(e) = s.external.as_mut() {
                    e.active = false;
                    e.result = Some(Err(schematic::PlaceError::Manual));
                }
            } else if !placing(s) {
                if let Some(original) = s.original_slot.take() {
                    game.selected = original;
                }
                s.auto_slot = None;
            }
        }
    });
    if !game.native_on(claims::ID) {
        reset();
        return false;
    }
    let data = state(|s| {
        if !placing(s) {
            None
        } else {
            s.placement.as_ref().map(|p| {
                (
                    s.job.clone(),
                    p.clone(),
                    s.aim,
                    s.external.as_ref().map(|e| e.request.clone()),
                )
            })
        }
    });
    let Some((job, placement, last_aim, external)) = data else {
        return false;
    };
    if !game.native_playing() {
        pause();
        return false;
    }
    if let Some([yaw, pitch]) = last_aim
        && (((game.yaw - yaw + 180.0).rem_euclid(360.0) - 180.0).abs() > 0.5
            || (game.pitch - pitch).abs() > 0.5)
    {
        pause();
        return false;
    }
    if placement.sequence.is_some() {
        return false;
    }
    let wanted_item = external.as_ref().map(|e| e.item.clone());
    // Resolved once: the closure must not borrow the game while a swap click needs it mutably.
    let usable = if wanted_item.is_none() {
        building_items(game, false)
    } else {
        Vec::new()
    };
    let wanted = |name: &str| match wanted_item.as_deref() {
        Some(item) => name == item,
        None => usable.iter().any(|n| n == name),
    };
    let Some(source) = source_for(game, &wanted) else {
        // Nothing usable in the hotbar or the off hand: bring a stack over from the inventory
        // and place on a following tick, once the swap has settled.
        match fetch(game, &wanted) {
            Fetch::Waiting => {}
            Fetch::Missing => stop(model::Stop::Material),
        }
        return false;
    };
    let stack = match source {
        Source::Hotbar(slot) => {
            state(|s| {
                if s.auto_slot.is_none() {
                    s.original_slot = Some(game.selected);
                }
                s.auto_slot = Some(slot);
            });
            game.selected = slot;
            game.stack_in(slot).cloned()
        }
        // The main hand keeps its tool; the off hand's block is used when it passes.
        Source::Offhand => game.offhand.clone(),
    };
    let Some(stack) = stack else {
        stop(model::Stop::Material);
        return false;
    };
    let free = block(game, placement.target).cell == nav::Cell::Air
        || (external.is_some() && drain::replaceable(&game.native_block(placement.target)));
    if !free {
        stop(model::Stop::Blocked);
        return false;
    }
    let mut why = "support";
    for delta in [
        [0, -1, 0],
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
        [0, 1, 0],
    ] {
        let anchor = [
            placement.target[0] + delta[0],
            placement.target[1] + delta[1],
            placement.target[2] + delta[2],
        ];
        let b = block(game, anchor);
        let facts = game
            .res
            .facts()
            .get(game.world.block(anchor[0], anchor[1], anchor[2]));
        if (b.cell != nav::Cell::Solid && external.is_none())
            || matches!(b.cell, nav::Cell::Hazard | nav::Cell::Unknown)
            || facts.block_entity
            || game.native_block(anchor) == "minecraft:air"
        {
            continue;
        }
        let face = delta.map(|n| -n);
        aim(
            game,
            [
                f64::from(anchor[0]) + 0.5 + f64::from(face[0]) * 0.501,
                f64::from(anchor[1]) + 0.5 + f64::from(face[1]) * 0.501,
                f64::from(anchor[2]) + 0.5 + f64::from(face[2]) * 0.501,
            ],
        );
        if game.entity_targeted() {
            why = "entity";
            continue;
        }
        let Some(hit) = game.pick().filter(|h| h.pos == anchor && h.face == face) else {
            why = "aim";
            continue;
        };
        // Do not activate a workstation or other interactive support instead of placing.
        if game.block_takes_click(&hit, Some(&stack)) {
            why = "interactive";
            continue;
        }
        let Some((at, changes)) = game.remote_can_place(&hit, &stack) else {
            why = "refused";
            continue;
        };
        if [at.x, at.y, at.z] != placement.target
            || changes.is_empty()
            || (external.is_none() && changes.len() != 1)
        {
            why = "target";
            continue;
        }
        let placed = game.res.facts().get(changes[0].1);
        let mut confirm = Vec::new();
        if let Some(e) = external.as_ref() {
            if changes.len() > 4 {
                why = "target";
                continue;
            }
            let mut valid = true;
            for (p, id) in &changes {
                let pos = [p.x, p.y, p.z];
                let Some(wanted) = e.allowed.get(&pos) else {
                    valid = false;
                    break;
                };
                let Some(actual) = game.pack.assets.registry.state(*id) else {
                    valid = false;
                    break;
                };
                if !wanted.matches(&actual.name, |key| {
                    actual.properties.get(key).map(|v| v.to_string())
                }) {
                    valid = false;
                    break;
                }
                confirm.push((pos, wanted.clone()));
            }
            if !valid {
                why = "target";
                continue;
            }
        } else if !placed.full_collision || placed.name.as_ref() != stack.item.as_ref() {
            why = "target";
            continue;
        }
        state(|s| {
            if let Some(p) = s.placement.as_mut() {
                p.anchor = Some(anchor);
                p.material = external
                    .as_ref()
                    .map_or_else(|| stack.item.to_string(), |e| e.state.name.clone());
            }
            if let Some(e) = s.external.as_mut() {
                e.confirm = confirm;
            }
            s.aim = Some([game.yaw, game.pitch]);
        });
        return true;
    }
    let Some(p) = game.native_player() else {
        pause();
        return false;
    };
    let position = [p.x, p.y, p.z];
    let Some(job) = job else {
        state(|s| s.diag = why);
        stop(model::Stop::Blocked);
        return false;
    };
    if nav::distance(position, job.edge()) < 0.08 {
        state(|s| s.diag = why);
        stop(model::Stop::Blocked);
        return false;
    }
    state(|s| {
        if let Some(p) = s.placement.as_mut() {
            p.edge = true;
        }
        s.aim = Some([game.yaw, game.pitch]);
    });
    false
}
pub(super) fn sent(packet: &horizon_protocol::packets::play::Serverbound) {
    if let horizon_protocol::packets::play::Serverbound::UseItemOn(p) = packet {
        state(|s| {
            if placing(s)
                && let Some(place) = s.placement.as_mut()
                && place.anchor == Some(p.pos)
                && place.sequence.is_none()
            {
                place.sequence = Some(p.sequence);
                place.started_ms = s.started.elapsed().as_millis() as u64;
            }
        });
    }
}
pub(super) fn ack(game: &Game<'_>, sequence: i32) {
    state(|s| {
        if navigation::scope(game).as_deref() != Some(s.scope.as_str()) {
            return;
        }
        let Some(p) = s.placement.as_ref() else {
            return;
        };
        if let Some(e) = s.external.as_mut() {
            if p.sequence.is_some_and(|seq| sequence >= seq) {
                let ok = !e.confirm.is_empty()
                    && e.confirm.iter().all(|(at, w)| {
                        w.matches(&game.native_block(*at), |key| {
                            game.native_block_property(*at, key)
                        })
                    });
                e.result = Some(if ok {
                    Ok(e.confirm.len())
                } else {
                    Err(schematic::PlaceError::Refused)
                });
                e.active = false;
                s.placement = None;
                s.aim = None;
            }
            return;
        }
        let result = p.acknowledge(
            sequence,
            &game.native_block(p.target),
            block(game, p.target).cell == nav::Cell::Solid,
        );
        if let Some(ok) = result {
            if let Some(j) = s.job.as_mut() {
                if ok {
                    j.placed += 1;
                    j.phase = model::Phase::Ready;
                } else {
                    j.stop(model::Stop::Refused);
                }
            }
            s.placement = None;
            s.aim = None;
        }
    });
}
/// How the bridge or tunnel job started by a route is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Working,
    Done,
    Paused,
    Failed,
}
pub(super) fn outcome() -> Option<Outcome> {
    state(|s| {
        s.job.as_ref().map(|j| match j.phase {
            model::Phase::Done => Outcome::Done,
            model::Phase::Stopped(model::Stop::Manual) => Outcome::Paused,
            model::Phase::Stopped(_) => Outcome::Failed,
            _ if j.active => Outcome::Working,
            _ => Outcome::Paused,
        })
    })
}
/// Whether blocks usable for a floor are in the hotbar, the off hand or the inventory.
pub(super) fn has_building_blocks(game: &Game<'_>) -> bool {
    !building_items(game, false).is_empty()
}
pub(super) fn snapshot() -> Option<(String, bool)> {
    let l = language();
    state(|s| {
        s.job.as_ref().map(|j| {
            let kind = if j.config.kind == model::Kind::Tunnel {
                l.text("Тоннель", "Tunnel")
            } else {
                l.text("Проход", "Bridge")
            };
            let phase = if j.preview {
                l.text("Просмотр", "Preview")
            } else if !j.active && !matches!(j.phase, model::Phase::Done | model::Phase::Stopped(_))
            {
                l.text("Пауза", "Paused")
            } else {
                match j.phase {
                    model::Phase::Done => l.text("Готово", "Done"),
                    model::Phase::Stopped(model::Stop::Refused) => {
                        l.text("Сервер отказал", "Server refused")
                    }
                    model::Phase::Stopped(model::Stop::Material) => l.text(
                        "Нет блоков (хотбар, 2-я рука, инвентарь)",
                        "No building blocks (hotbar, off hand, inventory)",
                    ),
                    model::Phase::Stopped(model::Stop::Hazard) => {
                        l.text("Жидкость или опасный блок", "Fluid or hazardous block")
                    }
                    model::Phase::Stopped(model::Stop::Unknown) => {
                        l.text("Чанк не загружен", "Chunk not loaded")
                    }
                    model::Phase::Stopped(model::Stop::Falling) => {
                        l.text("Риск обвала", "Falling blocks above")
                    }
                    model::Phase::Stopped(model::Stop::Tool) => {
                        l.text("Нужна исправная кирка", "A usable pickaxe is required")
                    }
                    model::Phase::Stopped(model::Stop::Timeout) => l.text("Тайм-аут", "Timed out"),
                    model::Phase::Stopped(_) => {
                        l.text("Пауза · проверьте проход", "Paused · check the passage")
                    }
                    model::Phase::Digging => l.text("Разрушение", "Digging"),
                    model::Phase::Placing => l.text("Установка пола", "Placing floor"),
                    _ => l.text("Движение", "Moving"),
                }
            };
            (
                format!("{kind} · {phase} · {}/{}", j.completed, j.config.length),
                !j.active,
            )
        })
    })
}
pub(super) fn render(game: &Game<'_>, g: &mut horizon_client::gizmos::Gizmos) {
    if !game.native_on(claims::ID) || game.ui.hide_gui {
        return;
    }
    let job = state(|s| {
        if navigation::scope(game).as_deref() == Some(s.scope.as_str()) {
            s.job.clone()
        } else {
            None
        }
    });
    if let Some(j) = job {
        let a = nav::center(j.station(j.completed));
        let b = nav::center(j.station(j.config.length));
        g.line(
            [a[0], a[1] + 0.1, a[2]],
            [b[0], b[1] + 0.1, b[2]],
            0xff99bbff,
            2.0,
        );
    }
}
