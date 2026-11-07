// Native adapter for the local walking navigator; bounded background path planning.
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
    token: u64,
    start: model::Pos,
    goal: model::Pos,
    radius: u8,
    chunks: HashMap<(i32, i32), Arc<Chunk>>,
    resources: Arc<Resources>,
}
struct Answer {
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
                    let path = model::search_near(
                        query.start,
                        query.goal,
                        query.radius,
                        |at| {
                            let Some(chunk) = query
                                .chunks
                                .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
                            else {
                                return model::Cell::Unknown;
                            };
                            if at[1] < chunk.min_y() || at[1] >= chunk.max_y() {
                                return model::Cell::Unknown;
                            }
                            let id = chunk.get(at[0].rem_euclid(16), at[1], at[2].rem_euclid(16));
                            *table.entry(id).or_insert_with(|| {
                                let facts = query.resources.facts().get(BlockStateId(id));
                                model::classify(
                                    &facts.name,
                                    facts.full_collision,
                                    facts.fluid_level > 0 || facts.waterlogged,
                                )
                            })
                        },
                        canceled,
                    );
                    if !canceled() {
                        let _ = results.send(Answer {
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
    state(|s| *s = State::default());
}
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
fn scope(game: &Game<'_>) -> Option<String> {
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
        token: s.token,
        start: model::block([player.x, player.y, player.z]),
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
    state(|s| {
        if s.nav.running {
            cancel_pending(s);
            s.nav.pause();
        }
    });
}
pub(super) fn resume(game: &Game<'_>) -> Result<(), String> {
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
                    } else {
                        s.nav.accept(answer.path);
                    }
                }
            }
        }
        if s.pending && s.last_query.elapsed() > Duration::from_secs(5) {
            cancel_pending(s);
            s.nav.running = false;
            s.nav.status = model::Status::NoPath;
        }
        if s.nav.running && !game.native_playing() {
            cancel_pending(s);
            s.nav.pause();
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
        if s.nav.running
            && s.nav.status == model::Status::Waiting
            && s.last_query.elapsed() > Duration::from_millis(350)
            && request(game, s).is_err()
        {
            s.nav.pause();
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
            s.nav.pause();
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
                model::block(position)
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
    let language = language();
    state(|s| {
        let label = match s.nav.status {
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
    if !game.native_playing() || game.ui.hide_gui || !state(|s| s.nav.goal.is_some()) {
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
