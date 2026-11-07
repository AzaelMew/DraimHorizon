// Native adapter for the local walking navigator; bounded background path planning.
use crate::game::claim_borders_mining as mining;
use crate::game::{Game, Resources, claim_borders as claims, claim_borders_navigation as model};
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
struct Query {
    flat: bool,
    mine: Option<(u8, Vec<model::Pos>)>,
    token: u64,
    start: model::Pos,
    goal: model::Pos,
    radius: u8,
    chunks: HashMap<(i32, i32), Arc<Chunk>>,
    resources: Arc<Resources>,
}
struct Answer {
    target: Option<model::Pos>,
    token: u64,
    start: model::Pos,
    path: Option<model::Path>,
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
                    let mut table = HashMap::new();
                    let mut read = |at: model::Pos| {
                        if query.flat && !(query.start[1] - 1..=query.start[1] + 1).contains(&at[1])
                        {
                            return mining::Block::default();
                        }
                        let Some(chunk) = query
                            .chunks
                            .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
                        else {
                            return mining::Block::default();
                        };
                        if at[1] < chunk.min_y() || at[1] >= chunk.max_y() {
                            return mining::Block::default();
                        }
                        let id = chunk.get(at[0].rem_euclid(16), at[1], at[2].rem_euclid(16));
                        *table.entry(id).or_insert_with(|| {
                            let f = query.resources.facts().get(BlockStateId(id));
                            mining::Block {
                                cell: model::classify(
                                    &f.name,
                                    f.full_collision,
                                    f.fluid_level > 0 || f.waterlogged,
                                ),
                                ore: query
                                    .mine
                                    .as_ref()
                                    .is_some_and(|(ore, _)| mining::matches(*ore, &f.name)),
                                falling: mining::falling(&f.name),
                            }
                        })
                    };
                    let (target, path) = if let Some((_, excluded)) = query.mine.as_ref() {
                        match mining::plan(query.start, excluded, &mut read, canceled) {
                            Some((target, path)) => (Some(target), Some(path)),
                            None => (None, None),
                        }
                    } else {
                        (
                            None,
                            model::search_near(
                                query.start,
                                query.goal,
                                query.radius,
                                |at| read(at).cell,
                                canceled,
                            ),
                        )
                    };
                    if !canceled() {
                        let _ = results.send(Answer {
                            target,
                            token: query.token,
                            start: query.start,
                            path,
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
    use_restore: Option<bool>,
    attack_restore: Option<bool>,
    mining: Option<mining::Job>,
    mine_aim: Option<[f32; 2]>,
    started: Instant,
    nav: model::Navigator,
    scope: String,
    worker: Option<Worker>,
    token: u64,
    pending: bool,
    last_query: Instant,
}
impl Default for State {
    fn default() -> Self {
        Self {
            flat: false,
            use_restore: None,
            attack_restore: None,
            mining: None,
            mine_aim: None,
            started: Instant::now(),
            nav: model::Navigator::default(),
            scope: String::new(),
            worker: None,
            token: 0,
            pending: false,
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
}
fn pause_state(s: &mut State) {
    s.nav.pause();
    if let Some(job) = s.mining.as_mut() {
        job.active = false;
    }
    s.mine_aim = None;
}
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
        cell: model::classify(
            &f.name,
            f.full_collision,
            f.fluid_level > 0 || f.waterlogged,
        ),
        ore: if let Some(crop) = crate::game::claim_borders_farming::Crop::from_code(ore) {
            crop.block() == f.name.as_ref()
                && crop.mature(game.native_block_property(at, "age").as_deref())
        } else if ore == u8::MAX {
            crate::game::claim_borders_construction_view::block(game, at).diggable
        } else {
            mining::matches(ore, &f.name)
        },
        falling: mining::falling(&f.name),
    }
}
fn mining_tool(game: &Game<'_>, target: Option<model::Pos>) -> bool {
    let Some(stack) = game.held() else {
        return false;
    };
    if !stack.item.ends_with("_pickaxe") {
        return false;
    }
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
    if max > 0 && max.saturating_sub(stack.damage) <= 10 {
        return false;
    }
    target.is_none_or(|at| {
        game.res
            .items()
            .correct_for_drops(Some(stack), &game.native_block(at))
    })
}
pub(super) fn construction_tool(game: &Game<'_>) -> bool {
    mining_tool(game, None)
}
pub(super) fn workshop_walk(game: &Game<'_>, goal: model::Pos, flat: bool) -> Result<(), String> {
    clear_construction_driver();
    state(|s| {
        s.scope = scope(game).ok_or("Join a world first")?;
        s.flat = flat;
        s.nav.begin(goal, true);
        request(game, s)
    })
}
pub(super) fn workshop_walk_result() -> Option<bool> {
    state(|s| match s.nav.status {
        model::Status::Arrived => Some(true),
        model::Status::NoPath | model::Status::Stuck | model::Status::Paused => Some(false),
        _ => None,
    })
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
    if !mining_tool(game, Some(target)) || !free_slot(game) {
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
                "Возьмите кирку с запасом прочности",
                "Hold a pickaxe with durability remaining",
            )
            .into());
    }
    if running && !free_slot(game) {
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
        if !mining_tool(game, None) || !free_slot(game) {
            return Err(language()
                .text(
                    "Нужна исправная кирка и свободный слот",
                    "A usable pickaxe and a free slot are required",
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
fn mining_tick(game: &Game<'_>, s: &mut State, position: [f64; 3]) {
    let Some(job) = s.mining.as_mut().filter(|j| j.active) else {
        return;
    };
    let now = s.started.elapsed().as_millis() as u64;
    match job.phase {
        mining::Phase::Searching => {
            s.nav.running = true;
            if !s.pending {
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
            if now.saturating_sub(job.started_ms) > 30_000 {
                job.active = false;
                job.phase = mining::Phase::Timeout;
                s.nav.running = false;
            }
        }
        mining::Phase::Confirming => {
            if now.saturating_sub(job.started_ms) > 5_000 {
                job.active = false;
                job.phase = mining::Phase::Timeout;
                s.nav.running = false;
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
    if physical || game.right {
        crate::game::claim_borders_workshop_view::pause();
        crate::game::claim_borders_construction_view::pause();
    }
    let Some(player) = game.native_player() else {
        return physical;
    };
    state(|s| {
        if physical || game.right {
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
            || !game.native_playing()
            || scope(game).as_deref() != Some(s.scope.as_str())
            || game.player.as_ref().is_none_or(|p| {
                p.flying
                    || p.fall_flying
                    || p.is_passenger
                    || p.fluids.was_touching_water
                    || p.is_in_lava()
                    || p.health.health() <= 0.0
            })
        {
            pause_state(s);
            return physical;
        }
        let Some(target) = job.target else {
            return false;
        };
        if crate::game::claim_borders_farming::Crop::from_code(job.ore).is_none()
            && !mining_tool(game, Some(target))
        {
            job.active = false;
            job.phase = mining::Phase::Tool;
            s.nav.running = false;
            return false;
        }
        if !free_slot(game) {
            job.active = false;
            job.phase = mining::Phase::Inventory;
            s.nav.running = false;
            return false;
        }
        if let Some([yaw, pitch]) = s.mine_aim
            && (((game.yaw - yaw + 180.0).rem_euclid(360.0) - 180.0).abs() > 0.5
                || (game.pitch - pitch).abs() > 0.5)
        {
            pause_state(s);
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
        let center = [
            f64::from(target[0]) + 0.5,
            f64::from(target[1]) + 0.5,
            f64::from(target[2]) + 0.5,
        ];
        let [dx, dy, dz] = [center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]];
        game.yaw = (-dx).atan2(dz).to_degrees() as f32;
        game.pitch = (-dy).atan2(dx.hypot(dz)).to_degrees() as f32;
        if game.entity_targeted() || game.pick().is_none_or(|hit| hit.pos != target) {
            job.skip();
            s.mine_aim = None;
            s.nav.replan();
            return false;
        }
        s.mine_aim = Some([game.yaw, game.pitch]);
        true
    })
}
pub(super) fn mining_attack_begin(game: &mut Game<'_>) {
    let physical = game.left;
    let attack = mining_attack(game, physical);
    if attack != physical {
        state(|s| s.attack_restore = Some(physical));
        game.left = attack;
    }
    let physical_right = game.right;
    if crate::game::claim_borders_construction_view::input(game) && !physical_right {
        state(|s| s.use_restore = Some(physical_right));
        game.right = true;
    }
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
            job.acknowledge(sequence, mining_block(game, job.ore, target).ore);
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
    // A jump is not a valid standing start for the planner. Wait for landing.
    if s.nav.running && game.player.as_ref().is_some_and(|p| !p.ground.on_ground) {
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
    let chunks = (-2..=2)
        .flat_map(|z| (-2..=2).map(move |x| (center.0 + x, center.1 + z)))
        .filter_map(|at| game.world.chunks.get(&at).map(|c| (at, c.clone())))
        .collect();
    let query = Query {
        flat: s.flat,
        mine: s
            .mining
            .as_ref()
            .filter(|j| j.phase == mining::Phase::Searching)
            .map(|j| (j.ore, j.excluded.clone())),
        token: s.token,
        start: model::standing_block(
            [player.x, player.y, player.z],
            &game.native_block(model::block([player.x, player.y, player.z])),
        ),
        goal,
        radius: if s.nav.follow.is_some() { 1 } else { 0 },
        chunks,
        resources: game.res.clone(),
    };
    s.worker.as_ref().unwrap().tx.try_send(query).map_err(|_| {
        language()
            .text(
                "Планировщик занят; повторите запрос",
                "Planner busy; try again",
            )
            .to_owned()
    })?;
    s.pending = true;
    s.last_query = Instant::now();
    s.nav.status = model::Status::Planning;
    Ok(())
}
pub(super) fn start(game: &Game<'_>, goal_request: Goal, running: bool) -> Result<(), String> {
    if let Goal::Farm(config) = goal_request {
        return crate::game::claim_borders_workshop_view::start_farm(game, config, !running);
    }
    crate::game::claim_borders_workshop_view::reset();
    if let Goal::Construct(mut config) = goal_request {
        config.direction = crate::game::claim_borders_construction::Direction::from_yaw(game.yaw);
        return crate::game::claim_borders_construction_view::start(game, config, !running);
    }
    crate::game::claim_borders_construction_view::reset();
    if let Goal::Mine { ore, limit } = goal_request {
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
        Goal::Farm(_) => unreachable!(),
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
    state(|s| {
        s.mining = None;
        s.mine_aim = None;
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
        if let Some(w) = s.worker.as_ref() {
            while let Ok(answer) = w.rx.try_recv() {
                if answer.token == s.token && s.pending {
                    s.pending = false;
                    if model::distance(position, model::center(answer.start)) > 1.5 {
                        s.nav.running = false;
                        s.nav.status = model::Status::Paused;
                        if let Some(job) = s.mining.as_mut() {
                            job.active = false;
                        }
                    } else {
                        if let Some(job) = s.mining.as_mut()
                            && job.phase == mining::Phase::Searching
                        {
                            if let Some(target) = answer.target {
                                job.select(target);
                                s.nav.goal =
                                    answer.path.as_ref().and_then(|p| p.nodes.last().copied());
                            } else {
                                job.active = false;
                                job.phase = mining::Phase::NoTarget;
                            }
                        }
                        if answer.path.is_none() && s.mining.is_none() && s.nav.running {
                            // Retry from the current position with a bounded recovery budget.
                            // Never turn a failed plan into unbounded movement or block edits.
                            s.nav.blocked(position);
                        } else {
                            s.nav.accept(answer.path);
                        }
                    }
                }
            }
        }
        if s.pending && s.last_query.elapsed() > Duration::from_secs(5) {
            cancel_pending(s);
            s.nav.running = false;
            s.nav.status = model::Status::NoPath;
            if let Some(job) = s.mining.as_mut() {
                job.active = false;
                job.phase = mining::Phase::Timeout;
            }
        }
        if (s.nav.running || s.mining.as_ref().is_some_and(|j| j.active)) && !game.native_playing()
        {
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
                        ) || (changed && s.last_query.elapsed() >= Duration::from_millis(500)))
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
        if s.nav.running
            && s.nav.status == model::Status::Waiting
            && s.last_query.elapsed() > Duration::from_millis(350)
            && request(game, s).is_err()
        {
            pause_state(s);
        }
    });
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
    model::classify(
        &f.name,
        f.full_collision,
        f.fluid_level > 0 || f.waterlogged,
    )
}
pub(super) fn movement(
    game: &Game<'_>,
    forward: &mut f32,
    strafe: &mut f32,
    jump: &mut bool,
    sneak: &mut bool,
) {
    crate::game::claim_borders_workshop_view::manual_movement(*forward, *strafe, *jump, *sneak);
    if crate::game::claim_borders_construction_view::movement(game, forward, strafe, jump, sneak) {
        return;
    }
    if !game.native_on(claims::ID) || !game.native_playing() {
        pause();
        return;
    }
    let Some(body) = game.player.as_ref() else {
        pause();
        return;
    };
    if body.flying
        || body.fall_flying
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
    state(|s| {
        if !s.nav.running {
            return;
        }
        if *forward != 0.0
            || *strafe != 0.0
            || *jump
            || *sneak
            || scope(game).as_deref() != Some(s.scope.as_str())
        {
            cancel_pending(s);
            pause_state(s);
            return;
        }
        let input = s.nav.input(position, p.yaw, body.ground.on_ground);
        if s.nav.status == model::Status::Stuck {
            cancel_pending(s);
            s.nav.blocked(position);
            return;
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
        *jump = input.jump;
    });
}

pub(super) fn snapshot(game: &Game<'_>) -> (String, bool) {
    if let Some(snapshot) = crate::game::claim_borders_workshop_view::snapshot() {
        return snapshot;
    }
    if let Some(snapshot) = crate::game::claim_borders_construction_view::snapshot() {
        return snapshot;
    }
    let language = language();
    state(|s| {
        if let Some(job) = s.mining.as_ref() {
            let text = if !job.active
                && matches!(
                    job.phase,
                    mining::Phase::Searching
                        | mining::Phase::Approaching
                        | mining::Phase::Breaking
                        | mining::Phase::Confirming
                )
                && !matches!(
                    s.nav.status,
                    model::Status::Preview | model::Status::Planning
                ) {
                language.text("Добыча на паузе", "Mining paused")
            } else {
                match job.phase {
                    mining::Phase::Searching => language.text("Поиск руды…", "Finding ore…"),
                    mining::Phase::Approaching if s.nav.status == model::Status::Preview => {
                        language.text("Просмотр добычи", "Mining preview")
                    }
                    mining::Phase::Approaching => language.text("Подход к руде", "Approaching ore"),
                    mining::Phase::Breaking => language.text("Добыча", "Mining"),
                    mining::Phase::Confirming => {
                        language.text("Ожидание сервера", "Waiting for server")
                    }
                    mining::Phase::Done => language.text("Добыча завершена", "Mining complete"),
                    mining::Phase::NoTarget => {
                        language.text("Доступная руда не найдена", "No reachable exposed ore")
                    }
                    mining::Phase::Refused => {
                        language.text("Сервер отклонил разрушение", "Server refused the break")
                    }
                    mining::Phase::Tool => language.text(
                        "Нужна подходящая исправная кирка",
                        "A suitable usable pickaxe is required",
                    ),
                    mining::Phase::Inventory => {
                        language.text("Нужен свободный слот", "A free inventory slot is required")
                    }
                    mining::Phase::Timeout => language.text("Тайм-аут добычи", "Mining timed out"),
                }
            };
            return (
                format!("{text} · {}/{}", job.confirmed, job.limit),
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
            model::Status::Idle => language.text("Цель не задана", "No destination"),
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
        (
            distance.map_or_else(|| label.into(), |d| format!("{label} · {d:.0} m")),
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
