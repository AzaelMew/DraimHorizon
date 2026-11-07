// Native construction driver: ordinary digging and block use, confirmed by server acknowledgements.
use crate::game::{
    Game, claim_borders as claims, claim_borders_construction as model,
    claim_borders_navigation as nav, claim_borders_navigation_view as navigation,
};
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct State {
    job: Option<model::Job>,
    scope: String,
    placement: Option<model::Placement>,
    auto_slot: Option<usize>,
    original_slot: Option<usize>,
    aim: Option<[f32; 2]>,
    started: Instant,
    action_ms: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            job: None,
            scope: String::new(),
            placement: None,
            auto_slot: None,
            original_slot: None,
            aim: None,
            started: Instant::now(),
            action_ms: 0,
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
pub(super) fn pause() {
    state(|s| {
        if let Some(j) = s.job.as_mut()
            && j.active
        {
            j.stop(model::Stop::Manual);
        }
        s.aim = None;
    });
}
fn stop(reason: model::Stop) {
    state(|s| {
        if let Some(j) = s.job.as_mut() {
            j.stop(reason);
        }
        s.aim = None;
    });
}
fn material_slot(game: &Game<'_>) -> Option<usize> {
    (0..9).find(|slot| {
        game.stack_in(*slot)
            .is_some_and(|stack| stack.count > 0 && model::material(&stack.item))
    })
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
        && material_slot(game).is_none()
    {
        return Err(l
            .text(
                "Добавьте камень, булыжник или землю в хотбар",
                "Put stone, cobblestone or dirt in the hotbar",
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
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    let current = navigation::scope(game);
    if state(|s| s.job.is_some() && current.as_deref() != Some(s.scope.as_str())) {
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
        *sneak = j.config.kind == model::Kind::Bridge || j.phase == model::Phase::Placing;
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
            s.original_slot = None;
            s.auto_slot = None;
            return;
        }
        if let Some(auto) = s.auto_slot {
            if game.selected != auto {
                if let Some(j) = s.job.as_mut() {
                    j.stop(model::Stop::Manual);
                }
                s.original_slot = None;
                s.auto_slot = None;
            } else if !s
                .job
                .as_ref()
                .is_some_and(|j| j.active && j.phase == model::Phase::Placing)
            {
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
        s.job
            .as_ref()
            .filter(|j| j.active && j.phase == model::Phase::Placing)
            .zip(s.placement.as_ref())
            .map(|(j, p)| (j.clone(), p.clone(), s.aim))
    });
    let Some((job, placement, last_aim)) = data else {
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
    let Some(slot) = material_slot(game) else {
        stop(model::Stop::Material);
        return false;
    };
    state(|s| {
        if s.auto_slot.is_none() {
            s.original_slot = Some(game.selected);
        }
        s.auto_slot = Some(slot);
    });
    game.selected = slot;
    let Some(stack) = game.stack_in(slot).cloned() else {
        stop(model::Stop::Material);
        return false;
    };
    if block(game, placement.target).cell != nav::Cell::Air {
        stop(model::Stop::Blocked);
        return false;
    }
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
        if b.cell != nav::Cell::Solid || facts.block_entity {
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
            continue;
        }
        let Some(hit) = game.pick().filter(|h| h.pos == anchor && h.face == face) else {
            continue;
        };
        // Do not activate a workstation or other interactive support instead of placing.
        if game.block_takes_click(&hit, Some(&stack)) {
            continue;
        }
        let Some((at, changes)) = game.remote_can_place(&hit, &stack) else {
            continue;
        };
        if [at.x, at.y, at.z] != placement.target || changes.len() != 1 {
            continue;
        }
        let placed = game.res.facts().get(changes[0].1);
        if !placed.full_collision || placed.name.as_ref() != stack.item.as_ref() {
            continue;
        }
        state(|s| {
            if let Some(p) = s.placement.as_mut() {
                p.anchor = Some(anchor);
                p.material = stack.item.to_string();
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
    if nav::distance(position, job.edge()) < 0.08 {
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
            if s.job
                .as_ref()
                .is_some_and(|j| j.active && j.phase == model::Phase::Placing)
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
                    model::Phase::Stopped(model::Stop::Material) => {
                        l.text("Нет блоков в хотбаре", "No building blocks in hotbar")
                    }
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
