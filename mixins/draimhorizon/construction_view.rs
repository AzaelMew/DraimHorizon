// Native construction driver: ordinary digging and block use, confirmed by server acknowledgements.
use crate::game::claim_borders_schematic as schematic;
use crate::game::{
    Game, claim_borders as claims, claim_borders_construction as model,
    claim_borders_draining as drain, claim_borders_navigation as nav,
    claim_borders_navigation_view as navigation,
};
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
struct External {
    request: schematic::PlacementRequest,
    active: bool,
    result: Option<Result<usize, schematic::PlaceError>>,
    confirm: Vec<(nav::Pos, schematic::BlockState)>,
    /// When the server's acknowledgement arrived before the block change did: the block is
    /// checked again for a moment before the placement counts as refused.
    verify_since: Option<u64>,
}
struct State {
    /// A container to open with one use click (the farm's storage), and how many ticks no
    /// point of it was in the line of sight.
    interact: Option<nav::Pos>,
    interact_miss: u32,
    /// An item to hold and use on a block (bone meal on a crop), and how that went: `Some(true)`
    /// once the use click was made, `Some(false)` when the item is missing or the block cannot
    /// be hit.
    use_item: Option<(String, nav::Pos)>,
    use_result: Option<bool>,
    /// Since when a stopped job has been waiting to try again (ms on this state's clock).
    idle_since: Option<u64>,
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
            interact: None,
            interact_miss: 0,
            use_item: None,
            use_result: None,
            idle_since: None,
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
    // For passages that dig and build a half block is something solid to stand on or break.
    let cell = nav::classify_shape(
        &facts.name,
        facts.full_collision,
        facts.fluid_level > 0 || facts.waterlogged,
        &facts.outline,
        nav::Cell::Solid,
    );
    model::Block {
        cell,
        diggable: matches!(cell, nav::Cell::Solid | nav::Cell::Blocked)
            && facts.hardness >= 0.0
            && !facts.block_entity,
        falling: crate::game::claim_borders_mining::falling(&facts.name),
    }
}
/// Gravity blocks seen where they are, with when each was first seen and last looked at.
fn seen<T>(f: impl FnOnce(&mut std::collections::HashMap<nav::Pos, (Instant, Instant)>) -> T) -> T {
    static SEEN: OnceLock<Mutex<std::collections::HashMap<nav::Pos, (Instant, Instant)>>> =
        OnceLock::new();
    f(&mut SEEN
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
/// Whether a sand or gravel block has stayed where it is for a while (1.5 s): a block that was
/// just dropped into the water is there for a moment and then sinks, one that has been there
/// has something under it. The count starts anew when the cell was seen empty (`forget`) or not
/// looked at for 20 s; a job that scans only every second or two still counts.
fn settled(at: nav::Pos) -> bool {
    let now = Instant::now();
    seen(|seen| {
        if seen.len() > 4096 {
            seen.clear();
        }
        let entry = seen.entry(at).or_insert((now, now));
        if now.duration_since(entry.1) > Duration::from_secs(20) {
            *entry = (now, now);
        } else {
            entry.1 = now;
        }
        now.duration_since(entry.0) >= Duration::from_millis(1500)
    })
}
/// Whether the block at `at` is something to stand or hang on: solid, and if it can fall (sand,
/// gravel) either resting on something solid that cannot, or having stayed put for a while. A
/// sand block just dropped into the water is solid for a moment and then sinks; whoever stands
/// on it sinks with it.
/// The cell is empty (water, air): whatever lands there next starts its settling count anew.
pub(super) fn forget(at: nav::Pos) {
    seen(|seen| {
        seen.remove(&at);
    });
}
pub(super) fn stable(game: &Game<'_>, at: nav::Pos) -> bool {
    let origin = at;
    let mut at = at;
    for _ in 0..24 {
        if block(game, at).cell != nav::Cell::Solid {
            break;
        }
        if !model::gravity(&game.native_block(at)) {
            return true;
        }
        at[1] -= 1;
    }
    if block(game, origin).cell != nav::Cell::Solid {
        forget(origin);
        return false;
    }
    settled(origin)
}
pub(super) fn reset() {
    state(|s| {
        s.interact = None;
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
    // Not into the slot of a Fortune tool the farm harvests with: the two swaps would trade
    // places at every crop.
    let target = (0..9)
        .filter(|slot| *slot != game.selected)
        .find(|slot| game.stack_in(*slot).is_none())
        .or_else(|| {
            (0..9).rev().find(|slot| {
                *slot != game.selected
                    && !crate::game::claim_borders_navigation_view::fortune_in(game, *slot)
            })
        })
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
/// What to fill water with: sand, red sand or gravel only (never an ordinary block, which would
/// be left standing in the water). Dropped into the surface cell it sinks and stacks from the
/// bottom, whatever the depth. The flag says it is a falling block, which it always is.
pub(super) fn drain_item(game: &Game<'_>) -> Option<(String, bool)> {
    building_items(game, true)
        .into_iter()
        .find(|name| {
            matches!(
                name.strip_prefix("minecraft:").unwrap_or(name),
                "sand" | "red_sand" | "gravel"
            )
        })
        .map(|name| (name, true))
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
            verify_since: None,
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
    state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        let Some(e) = s.external.as_mut() else {
            return;
        };
        let Some(since) = e.verify_since else {
            return;
        };
        if e.result.is_some() {
            return;
        }
        let ok = !e.confirm.is_empty()
            && e.confirm.iter().all(|(at, w)| {
                w.matches(&game.native_block(*at), |key| {
                    game.native_block_property(*at, key)
                })
            });
        if ok || now.saturating_sub(since) > 900 {
            e.result = Some(if ok {
                Ok(e.confirm.len())
            } else {
                Err(schematic::PlaceError::Refused)
            });
            e.active = false;
            e.verify_since = None;
            s.placement = None;
            s.aim = None;
        }
    });
    let current = navigation::scope(game);
    if state(|s| {
        (s.job.is_some() || s.external.is_some()) && current.as_deref() != Some(s.scope.as_str())
    }) {
        world_reset();
        return;
    }
    if !navigation::in_world(game) {
        pause();
        return;
    }
    // A job that stopped for any reason but the player's own pause stands idle for a few
    // seconds and tries again: a tool turns up, a chunk loads, the world changes.
    let retry = state(|s| {
        let now = s.started.elapsed().as_millis() as u64;
        let waiting = s.job.as_ref().is_some_and(|j| {
            !j.active
                && !j.preview
                && matches!(j.phase, model::Phase::Stopped(r) if r != model::Stop::Manual)
        });
        if !waiting {
            s.idle_since = None;
            return false;
        }
        now.saturating_sub(*s.idle_since.get_or_insert(now)) > 3000
    });
    if retry {
        state(|s| s.idle_since = None);
        let _ = resume(game);
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
    if swim_back(game, forward, strafe, jump, sneak) {
        return true;
    }
    if pier_walk(game, forward, strafe, jump, sneak) {
        return true;
    }
    if edge_drive(game, forward, strafe, jump, sneak) {
        return true;
    }
    if !state(|s| s.job.as_ref().is_some_and(|j| j.active)) {
        return false;
    }
    if ((*forward != 0.0 || *strafe != 0.0 || *jump || *sneak) && !navigation::input_grace())
        || !navigation::in_world(game)
    {
        pause();
        navigation::pause();
        return false;
    }
    let Some(p) = game.native_player() else {
        pause();
        return false;
    };
    let position = [p.x, p.y, p.z];
    // In the water the walker swims the player out (`navigation_view::movement`); the job
    // waits on dry ground instead of being paused.
    if game
        .player
        .as_ref()
        .is_some_and(|p| p.fluids.was_touching_water && !p.is_in_lava())
    {
        return false;
    }
    // In the air for a moment (a shove from a hit, a step off an edge, a glide): the job waits
    // for the ground, it is not paused. Only being dead, riding or lava pause it.
    if game
        .player
        .as_ref()
        .is_some_and(|p| !p.ground.on_ground || p.fall_flying)
    {
        return false;
    }
    if game
        .player
        .as_ref()
        .is_none_or(|p| p.flying || p.is_passenger || p.is_in_lava() || p.health.health() <= 0.0)
    {
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
                    input = steer(position, nav::center(station), p.yaw);
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
                        input = steer(position, nav::center(next), p.yaw);
                    }
                } else {
                    input = steer(position, nav::center(goal), p.yaw);
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
                    input = steer(position, j.edge(), p.yaw);
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
/// The nearest dry spot to stand on around `position`: open air at the feet and head over a
/// solid block, within a few blocks.
/// The nearest dry place to stand within eight blocks (three up or down) of `position`.
pub(super) fn dry_spot(game: &Game<'_>, position: [f64; 3]) -> Option<nav::Pos> {
    let here = nav::block(position);
    let mut best: Option<(i32, nav::Pos)> = None;
    for dy in -3..=3 {
        for dz in -8..=8 {
            for dx in -8..=8 {
                let at = [here[0] + dx, here[1] + dy, here[2] + dz];
                let open = |y: i32| block(game, [at[0], y, at[2]]).cell == nav::Cell::Air;
                if open(at[1]) && open(at[1] + 1) && stable(game, [at[0], at[1] - 1, at[2]]) {
                    let d = dx * dx + dz * dz + dy * dy * 4;
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, at));
                    }
                }
            }
        }
    }
    best.map(|(_, at)| at)
}
/// Walks the route along the blocks a drain pattern has laid, sneaking so that a one-wide line
/// is never walked off.
fn pier_walk(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) -> bool {
    if !game.native_on(claims::ID)
        || !navigation::in_world(game)
        || !crate::game::claim_borders_workshop_view::draining()
        || game
            .player
            .as_ref()
            .is_none_or(|b| b.fluids.was_touching_water || b.flying || !b.ground.on_ground)
    {
        return false;
    }
    let Some(p) = game.native_player() else {
        return false;
    };
    let position = [p.x, p.y, p.z];
    let Some(goal) = crate::game::claim_borders_workshop_view::pier_goal(position) else {
        return false;
    };
    let input = steer(position, goal, p.yaw);
    *forward = input.forward;
    *strafe = input.strafe;
    *jump = false;
    *sneak = true;
    true
}
/// A drain job that fell into the water swims back to the nearest dry ground and goes on from
/// there, instead of pausing.
fn swim_back(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) -> bool {
    if !game.native_on(claims::ID)
        || !navigation::in_world(game)
        // Even a drain job that was paused gets its player out of the water.
        || !crate::game::claim_borders_workshop_view::drain_task()
        || game
            .player
            .as_ref()
            .is_none_or(|b| !b.fluids.was_touching_water || b.flying)
    {
        return false;
    }
    let Some(p) = game.native_player() else {
        return false;
    };
    let position = [p.x, p.y, p.z];
    // The laid blocks are behind now: any route along them is stale.
    crate::game::claim_borders_workshop_view::clear_pier();
    let Some(spot) = dry_spot(game, position) else {
        return false;
    };
    let input = steer(position, nav::center(spot), p.yaw);
    *forward = input.forward;
    *strafe = input.strafe;
    // Swimming up and out: jump held keeps the player at the surface and over the edge.
    *jump = true;
    *sneak = false;
    true
}
/// How long a drain placement may spend reaching the edge before the next spot is tried.
const EDGE_MS: u64 = 3500;
/// The point to hang over and the shore block, for a water cell that no inland face reaches.
fn edge_goal(game: &Game<'_>, target: nav::Pos, player: [f64; 3]) -> Option<([f64; 3], nav::Pos)> {
    drain::edge_stand(
        target,
        player,
        |at| stable(game, at),
        |at| block(game, at).cell == nav::Cell::Air,
    )
}
/// Walks to the shore edge and sneaks over it while a drain placement waits for a face.
fn edge_drive(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) -> bool {
    let Some(target) = state(|s| {
        let p = s.placement.as_ref()?;
        let e = s.external.as_ref()?;
        (e.active && e.result.is_none() && p.edge && p.sequence.is_none()).then_some(p.target)
    }) else {
        return false;
    };
    if ((*forward != 0.0 || *strafe != 0.0 || *jump || *sneak) && !navigation::input_grace())
        || !navigation::in_world(game)
    {
        pause();
        navigation::pause();
        return false;
    }
    let Some(p) = game.native_player() else {
        pause();
        return false;
    };
    if game
        .player
        .as_ref()
        .is_none_or(|b| !b.ground.on_ground || b.fluids.was_touching_water || b.flying)
    {
        // Slipping off the edge into the water is not the player taking over: no job is paused
        // for it (the walker swims the player out), nor for a moment in the air.
        let wet_or_air = game
            .player
            .as_ref()
            .is_some_and(|b| b.fluids.was_touching_water || !b.ground.on_ground);
        if !wet_or_air && !crate::game::claim_borders_workshop_view::draining() {
            pause();
            navigation::pause();
        }
        return false;
    }
    let position = [p.x, p.y, p.z];
    let Some((edge, shore)) = edge_goal(game, target, position) else {
        return false;
    };
    let input = steer(position, edge, p.yaw);
    *forward = input.forward;
    *strafe = input.strafe;
    *jump = false;
    // Sneaking keeps the player from walking off; only needed near the water.
    *sneak = nav::distance(position, nav::center(shore)) < 1.6;
    true
}
/// The keys toward `target`, with the head turned toward it as the walker does.
fn steer(position: [f64; 3], target: [f64; 3], yaw: f32) -> nav::Input {
    navigation::look_toward(position, target);
    model::steer(position, target, yaw)
}
fn aim(game: &mut Game<'_>, point: [f64; 3]) {
    note_aim();
    if let Some(eye) = game.eye_position() {
        let [x, y, z] = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
        game.yaw = (-x).atan2(z).to_degrees() as f32;
        game.pitch = (-y).atan2(x.hypot(z)).to_degrees() as f32;
    }
}
/// Whether a block is being aimed at for a click (a door, a chest, a crop to feed): the walker
/// then leaves the head alone.
pub(super) fn aiming() -> bool {
    state(|s| {
        s.interact.is_some()
            || s.use_item.is_some()
            || s.external.as_ref().is_some_and(|e| e.active)
            || s.placement.is_some()
    })
}
/// When a block was last aimed at by a task (placing, opening, using).
static LAST_AIM: Mutex<Option<Instant>> = Mutex::new(None);
/// Whether a task aimed at a block a moment ago (the camera moved, but not by the mouse).
pub(super) fn aimed_recently() -> bool {
    LAST_AIM
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < Duration::from_millis(400))
}
fn note_aim() {
    *LAST_AIM.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}
/// Asks for one use click on the container at `pos`: the next tick aims at it and clicks.
pub(super) fn open_container(pos: nav::Pos) {
    state(|s| {
        s.interact = Some(pos);
        s.interact_miss = 0;
    });
}
/// Whether the container to open has been out of sight from here for a while (crops, a wall or
/// a corner in the way): the job should try another spot.
pub(super) fn container_blocked() -> bool {
    state(|s| s.interact.is_some() && s.interact_miss >= 10)
}
pub(super) fn cancel_container() {
    state(|s| s.interact = None);
}
/// The click that opens a requested container: aim at the middle of the block and click once
/// the ray really meets it. `None` when nothing was requested.
fn interact_input(game: &mut Game<'_>) -> Option<bool> {
    let pos = state(|s| s.interact)?;
    if !game.native_playing() || navigation::mouse_has_camera() {
        return Some(false);
    }
    // The head turns to the block over a few ticks first, as a person's does; the exact aim at
    // its middle and sides follows once it points at it.
    if let Some(eye) = game.eye_position() {
        let middle = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        let (yaw, pitch) = (nav::yaw_to(eye, middle), nav::pitch_to(eye, middle));
        if nav::angle_between(game.yaw, yaw).abs() > 6.0 || (game.pitch - pitch).abs() > 6.0 {
            note_aim();
            game.yaw = nav::turn_toward(game.yaw, yaw, 30.0);
            game.pitch = nav::turn_toward(game.pitch, pitch, 24.0);
            return None;
        }
    }
    // Where the block really is first (a door's panel stands at one side of its cell, a gate's
    // bar across its middle), then the middle, the top and the four sides: a chest behind a bed
    // of crops has a clear line only to some of them.
    let shape = {
        let facts = game
            .res
            .facts()
            .get(game.world.block(pos[0], pos[1], pos[2]));
        let boxes = &facts.outline;
        if boxes.is_empty() {
            [0.5, 0.5, 0.5]
        } else {
            let n = boxes.len() as f64;
            let mut c = [0.0; 3];
            for b in boxes.iter() {
                for i in 0..3 {
                    c[i] += (b[i] + b[i + 3]) / 2.0 / n;
                }
            }
            c
        }
    };
    for [x, y, z] in [
        shape,
        [0.5, 0.5, 0.5],
        [0.5, 0.9, 0.5],
        [0.15, 0.5, 0.5],
        [0.85, 0.5, 0.5],
        [0.5, 0.5, 0.15],
        [0.5, 0.5, 0.85],
        [0.5, 0.2, 0.5],
    ] {
        aim(
            game,
            [
                f64::from(pos[0]) + x,
                f64::from(pos[1]) + y,
                f64::from(pos[2]) + z,
            ],
        );
        if game.pick().is_some_and(|h| h.pos == pos) {
            state(|s| s.interact = None);
            return Some(true);
        }
    }
    state(|s| s.interact_miss += 1);
    None
}
/// Asks for `item` to be held and used once on the block at `pos` (bone meal on a crop): the
/// next ticks put it in hand, aim at the block and click; `use_result` says how it went.
pub(super) fn use_item_on(item: &str, pos: nav::Pos) {
    state(|s| {
        s.use_item = Some((item.to_owned(), pos));
        s.use_result = None;
        s.interact = None;
        s.interact_miss = 0;
    });
}
/// Whether a `use_item_on` is still under way (the item going to the hand, or the aim).
pub(super) fn use_busy() -> bool {
    state(|s| s.use_item.is_some())
}
/// The outcome of the last `use_item_on`, once, or `None` while it is still under way.
pub(super) fn use_result() -> Option<bool> {
    state(|s| s.use_result.take())
}
pub(super) fn cancel_use() {
    state(|s| {
        s.use_item = None;
        s.use_result = None;
        s.interact = None;
    });
}
/// Drives `use_item_on`: the item into the hotbar and the hand (the slot change has to reach
/// the server before the click), then the click once the ray really meets the block.
fn use_input(game: &mut Game<'_>) -> Option<bool> {
    let (item, pos) = state(|s| s.use_item.clone())?;
    if !game.native_playing() {
        return Some(false);
    }
    let wanted = |name: &str| name == item;
    match source_for(game, &wanted) {
        Some(Source::Hotbar(slot)) => {
            if game.selected != slot {
                game.selected = slot;
                return Some(false);
            }
        }
        _ => {
            // Not in the hotbar: one stack is brought over from the inventory, or it is missing.
            if matches!(fetch(game, &wanted), Fetch::Missing) {
                state(|s| {
                    s.use_item = None;
                    s.use_result = Some(false);
                });
            }
            return Some(false);
        }
    }
    state(|s| {
        if s.interact.is_none() {
            s.interact = Some(pos);
            s.interact_miss = 0;
        }
    });
    match interact_input(game) {
        Some(true) => {
            state(|s| {
                s.use_item = None;
                s.use_result = Some(true);
            });
            Some(true)
        }
        Some(false) => Some(false),
        None => {
            if state(|s| s.interact_miss >= 10) {
                // The block cannot be hit from here: the caller tries another spot or skips it.
                state(|s| {
                    s.use_item = None;
                    s.interact = None;
                    s.use_result = Some(false);
                });
            }
            Some(false)
        }
    }
}
pub(super) fn input(game: &mut Game<'_>) -> bool {
    if let Some(click) = use_input(game) {
        return click;
    }
    if let Some(click) = interact_input(game) {
        return click;
    }
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
    let Some((job, placement, _, external)) = data else {
        return false;
    };
    if !navigation::in_world(game) {
        pause();
        return false;
    }
    // A screen is open (the inventory, the chat, a menu): no click is made until it is closed,
    // the job just waits.
    if !game.native_playing() {
        return false;
    }
    // The player moved the mouse: the camera is theirs for a moment. The placement waits (its
    // time limits held back) and aims again afterwards; it is never paused for a turn of the
    // camera, which another task (the human look, a door, the storage) may have made.
    if navigation::mouse_has_camera() {
        state(|s| {
            let now = s.started.elapsed().as_millis() as u64;
            if let Some(p) = s.placement.as_mut() {
                p.started_ms = now;
            }
        });
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
    // The material is at hand: the swaps that brought it worked. The limit counts only swaps
    // in a row that bring nothing; it counted every swap of the whole job, and after 24 a farm
    // said the material was missing with seeds in the inventory.
    state(|s| s.fetches = 0);
    let stack = match source {
        Source::Hotbar(slot) => {
            state(|s| {
                if s.auto_slot.is_none() {
                    s.original_slot = Some(game.selected);
                }
                s.auto_slot = Some(slot);
            });
            let changed = game.selected != slot;
            game.selected = slot;
            if changed {
                // The slot change has to reach the server before the click, or the click is
                // made with the old item in hand and the server refuses it.
                return false;
            }
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
    // The head comes round to the block over a few ticks, as a person's does; the exact aim at
    // a face of its neighbour follows once it points there (a few degrees off at most).
    if let Some(eye) = game.eye_position() {
        let middle = nav::center(placement.target);
        let middle = [middle[0], middle[1] + 0.5, middle[2]];
        let (yaw, pitch) = (nav::yaw_to(eye, middle), nav::pitch_to(eye, middle));
        // Only a big turn is made gradually: near the block the exact aim at a face takes over
        // (going back to the middle every tick made the head spin).
        if nav::angle_between(game.yaw, yaw).abs() > 35.0 || (game.pitch - pitch).abs() > 35.0 {
            note_aim();
            game.yaw = nav::turn_toward(game.yaw, yaw, 30.0);
            game.pitch = nav::turn_toward(game.pitch, pitch, 24.0);
            let turned = [game.yaw, game.pitch];
            state(|s| s.aim = Some(turned));
            return false;
        }
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
            || matches!(
                b.cell,
                nav::Cell::Hazard | nav::Cell::Unknown | nav::Cell::Water
            )
            || facts.block_entity
            || game.native_block(anchor) == "minecraft:air"
        {
            continue;
        }
        let face = delta.map(|n| -n);
        // Farmland and paths are 15/16 tall. Aiming above that misses their top face, the
        // ray goes on past it and lands on the next crop or block, so seeds were never sown.
        let top = if face == [0, 1, 0]
            && matches!(
                game.native_block(anchor).as_str(),
                "minecraft:farmland" | "minecraft:dirt_path"
            ) {
            0.9375 - 0.5 - 0.01
        } else {
            f64::from(face[1]) * 0.501
        };
        let base = [
            f64::from(anchor[0]) + 0.5 + f64::from(face[0]) * 0.501,
            f64::from(anchor[1]) + 0.5 + top,
            f64::from(anchor[2]) + 0.5 + f64::from(face[2]) * 0.501,
        ];
        // The middle of the face first; when something blocks that line (a corner of the
        // passage, a plant), other points of the same face are tried before giving it up.
        let axis = face.iter().position(|n| *n != 0).unwrap_or(1);
        let tangent: Vec<usize> = (0..3).filter(|a| *a != axis).collect();
        let mut by_entity = false;
        let mut found = None;
        for [u, v] in [
            [0.0, 0.0],
            [0.3, 0.0],
            [-0.3, 0.0],
            [0.0, 0.3],
            [0.0, -0.3],
            [0.3, 0.3],
            [-0.3, -0.3],
        ] {
            let mut point = base;
            point[tangent[0]] += u;
            point[tangent[1]] += v;
            aim(game, point);
            if game.entity_targeted() {
                by_entity = true;
                continue;
            }
            let eye = game.eye_position();
            if let Some(h) = game.pick().filter(|h| {
                h.pos == anchor
                    && h.face == face
                    && (external.is_none()
                        || eye.is_none_or(|e| nav::distance(h.point, e) <= drain::CLICK_REACH))
            }) {
                found = Some(h);
                break;
            }
        }
        let Some(hit) = found else {
            why = if by_entity { "entity" } else { "aim" };
            continue;
        };
        // Do not activate a workstation or other interactive support instead of placing.
        if game.block_takes_click(&hit, Some(&stack)) {
            why = "interactive";
            continue;
        }
        let dry = game.remote_can_place(&hit, &stack);
        // Seeds are items that plant a crop rather than block items; when the dry run cannot
        // model that, sow anyway and let the server's answer confirm the crop.
        let crop = external.as_ref().filter(|e| {
            e.allowed.len() == 1
                && crate::game::claim_borders_farming::Crop::named(&e.state.name).is_some()
                && face == [0, 1, 0]
                && game.native_block(anchor) == "minecraft:farmland"
        });
        if dry.is_none()
            && let Some(e) = crop
        {
            let confirm = vec![(placement.target, e.state.clone())];
            state(|s| {
                if let Some(p) = s.placement.as_mut() {
                    p.anchor = Some(anchor);
                    p.material = e.state.name.clone();
                }
                if let Some(e) = s.external.as_mut() {
                    e.confirm = confirm;
                }
                s.aim = Some([game.yaw, game.pitch]);
            });
            return true;
        }
        let Some((at, changes)) = dry else {
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
        // Filling water: the walls facing the water cannot be hit from inland, so hang over
        // the edge of the shore block, sneaking, until the face is in the ray. Bounded.
        let waited = state(|s| {
            (s.started.elapsed().as_millis() as u64).saturating_sub(placement.started_ms)
        });
        if external.is_some()
            && drain::replaceable(&game.native_block(placement.target))
            && waited < EDGE_MS
            && edge_goal(game, placement.target, position).is_some()
        {
            state(|s| {
                if let Some(p) = s.placement.as_mut() {
                    p.edge = true;
                }
                s.aim = Some([game.yaw, game.pitch]);
            });
            return false;
        }
        state(|s| s.diag = why);
        stop(model::Stop::Blocked);
        return false;
    };
    if nav::distance(position, job.edge()) < 0.08 {
        // At the edge with no face to click: a moment's grace first, since a dropped item
        // settling or the server's block update often clears it, before stopping the job.
        let waited =
            state(|s| s.started.elapsed().as_millis() as u64).saturating_sub(placement.started_ms);
        if waited < 1500 {
            return false;
        }
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
                if ok {
                    e.result = Some(Ok(e.confirm.len()));
                    e.active = false;
                    s.placement = None;
                    s.aim = None;
                } else if e.verify_since.is_none() {
                    // The block update may still be on its way: look again for a moment.
                    e.verify_since = Some(s.started.elapsed().as_millis() as u64);
                }
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
                    model::Phase::Stopped(model::Stop::Protected) => l.text(
                        "Блок нельзя сломать (бедрок, сундук…)",
                        "Cannot break this block (bedrock, chest…)",
                    ),
                    model::Phase::Stopped(model::Stop::Floor) => {
                        l.text("Нет пола под проходом", "No floor under the passage")
                    }
                    model::Phase::Stopped(model::Stop::Blocked) => l.text(
                        "Нет грани для установки блока",
                        "No face to place a block on",
                    ),
                    model::Phase::Stopped(_) => l.text("Пауза", "Paused"),
                    model::Phase::Digging => l.text("Разрушение", "Digging"),
                    model::Phase::Placing => l.text("Установка пола", "Placing floor"),
                    _ => l.text("Движение", "Moving"),
                }
            };
            let idle = matches!(j.phase, model::Phase::Stopped(r) if r != model::Stop::Manual);
            (
                if idle {
                    format!(
                        "{kind} · {} · {phase} · {}/{}",
                        l.text("Простой", "Idle"),
                        j.completed,
                        j.config.length
                    )
                } else {
                    format!("{kind} · {phase} · {}/{}", j.completed, j.config.length)
                },
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
