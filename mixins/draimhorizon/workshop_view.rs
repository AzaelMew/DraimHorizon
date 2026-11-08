// Farm and schematic jobs share the normal movement, attack and placement drivers.
use crate::game::{
    Game, claim_borders as claims, claim_borders_construction_view as placement,
    claim_borders_draining as drain, claim_borders_farming as farm,
    claim_borders_navigation as nav, claim_borders_navigation_view as driver,
    claim_borders_schematic as schema, claim_borders_schematic_files as files,
};
use horizon_client::gizmos::Gizmos;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Mutex, OnceLock, mpsc::Receiver},
    time::{Duration, Instant},
};
fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn reason(error: schema::PlaceError) -> &'static str {
    match error {
        schema::PlaceError::Material => text(
            "Нет материала (хотбар, 2-я рука, инвентарь)",
            "Material missing (hotbar, off hand, inventory)",
        ),
        schema::PlaceError::Refused => text("Сервер отказал", "Server refused"),
        schema::PlaceError::Manual => text("Ручное вмешательство", "Manual input"),
        schema::PlaceError::Timeout => text("Нет ответа сервера", "Server acknowledgement timeout"),
        schema::PlaceError::Unsupported => text("Блок не поддерживается", "Unsupported block"),
        schema::PlaceError::Unreachable => text("Нет доступной грани", "No reachable face"),
    }
}
enum Task {
    Farm {
        config: farm::Config,
        origin: nav::Pos,
        harvested: u16,
    },
    Drain {
        config: drain::Config,
        origin: nav::Pos,
        filled: u16,
    },
    Build {
        document: schema::Blueprint,
        cursor: usize,
    },
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Ready,
    Walking,
    Digging,
    Placing,
    Done,
}
struct Pending {
    rx: Receiver<Result<files::Response, String>>,
    run: bool,
}
struct State {
    scope: String,
    task: Option<Task>,
    pending: Option<Pending>,
    files: Vec<String>,
    active: bool,
    preview: bool,
    phase: Phase,
    target: Option<nav::Pos>,
    plant: bool,
    attempt: usize,
    message: String,
    last: Instant,
    // How long the next cell scan waits, so a dropped block can sink and the water can settle.
    wait: Duration,
    // The wait after a successful placement of the current kind of block.
    settle: Duration,
    // Consecutive scans that found nothing to fill; the task ends after a few.
    idle: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: String::new(),
            task: None,
            pending: None,
            files: Vec::new(),
            active: false,
            preview: false,
            phase: Phase::Ready,
            target: None,
            plant: false,
            attempt: 0,
            message: String::new(),
            last: Instant::now(),
            wait: Duration::from_millis(400),
            settle: Duration::from_millis(400),
            idle: 0,
        }
    }
}
fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn exists() -> bool {
    state(|s| s.task.is_some() || s.pending.is_some())
}
/// (running, resumable) for the action keys. A paused or stopped task can resume;
/// a finished or previewed one starts over from the tab's settings.
pub(super) fn control() -> Option<(bool, bool)> {
    state(|s| {
        if s.pending.is_some() {
            return Some((true, false));
        }
        s.task
            .as_ref()
            .map(|_| (s.active, !s.active && !s.preview && s.phase != Phase::Done))
    })
}
pub(super) fn pause() {
    state(|s| {
        s.active = false;
        if let Some(p) = s.pending.as_mut() {
            p.run = false;
        }
    });
}
pub(super) fn manual_movement(f: f32, s: f32, j: bool, c: bool) {
    if f != 0.0 || s != 0.0 || j || c {
        pause();
    }
}
pub(super) fn folder(game: &Game<'_>) -> PathBuf {
    game.native_data_dir(claims::SETTINGS_ID).join("schematics")
}
pub(super) fn filenames() -> Vec<String> {
    state(|s| s.files.clone())
}
fn guard(game: &Game<'_>) -> Result<String, String> {
    if !game.native_on(claims::ID)
        || game
            .native_setting(claims::SETTINGS_ID, "experiments_accepted")
            .as_deref()
            != Some("true")
    {
        return Err(text(
            "Подтвердите экспериментальные функции",
            "Accept experimental features",
        )
        .into());
    }
    if game.remote.is_none() {
        return Err(text("Войдите на сервер", "Join a server first").into());
    }
    if driver::workshop_pending() || placement::placement_pending() {
        return Err(text(
            "Дождитесь ответа сервера",
            "Wait for the server acknowledgement",
        )
        .into());
    }
    driver::scope(game).ok_or_else(|| "Join a world first".into())
}
pub(super) fn refresh(game: &Game<'_>) -> Result<(), String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let rx = files::spawn(folder(game), None, [0; 3], 0)?;
    state(|s| {
        if s.scope != scope {
            *s = State::default();
        }
        s.scope = scope;
        s.pending = Some(Pending { rx, run: false });
    });
    Ok(())
}
pub(super) fn load(
    game: &Game<'_>,
    filename: String,
    origin: nav::Pos,
    rotation: u8,
    run: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    let rx = files::spawn(folder(game), Some(filename), origin, rotation)?;
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        let names = std::mem::take(&mut s.files);
        *s = State::default();
        s.files = names;
        s.scope = scope;
        s.pending = Some(Pending { rx, run });
        s.message = text("Загрузка", "Loading").into();
    });
    Ok(())
}
pub(super) fn start_farm(
    game: &Game<'_>,
    config: farm::Config,
    preview: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    if !config.valid() {
        return Err("Invalid farming bounds".into());
    }
    let p = game.native_player().ok_or("Join a world first")?;
    let origin = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    );
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        *s = State::default();
        s.scope = scope;
        s.task = Some(Task::Farm {
            config,
            origin,
            harvested: 0,
        });
        s.preview = preview;
        s.active = !preview;
    });
    Ok(())
}
pub(super) fn start_drain(
    game: &Game<'_>,
    config: drain::Config,
    preview: bool,
) -> Result<(), String> {
    if state(|s| s.pending.is_some()) {
        return Err(text("Загрузка уже выполняется", "Import already running").into());
    }
    let scope = guard(game)?;
    if !config.valid() {
        return Err("Invalid draining bounds".into());
    }
    if !preview && placement::drain_item(game).is_none() {
        return Err(text(
            "Нужны блоки для засыпки (хотбар, 2-я рука или инвентарь)",
            "Blocks to fill with are required (hotbar, off hand or inventory)",
        )
        .into());
    }
    let p = game.native_player().ok_or("Join a world first")?;
    // The player may be swimming, so the block is not adjusted to a standing position.
    let origin = nav::block([p.x, p.y, p.z]);
    driver::clear_construction_driver();
    placement::reset();
    state(|s| {
        *s = State::default();
        s.scope = scope;
        s.task = Some(Task::Drain {
            config,
            origin,
            filled: 0,
        });
        s.preview = preview;
        s.active = !preview;
    });
    Ok(())
}
pub(super) fn resume(game: &Game<'_>) -> Result<(), String> {
    let scope = driver::scope(game).ok_or("Join a world first")?;
    state(|s| {
        if s.scope != scope || s.task.is_none() {
            return Err(text("Задача из другого мира", "Task belongs to another world").into());
        }
        // Never resend an unacknowledged action. Late acknowledgements remain observable.
        if driver::workshop_pending() {
            driver::construction_resume_dig();
        } else if placement::placement_pending() {
            placement::external_resume();
        } else if !(s.phase == Phase::Digging && driver::farm_result() == Some(Ok(())))
            && !(s.phase == Phase::Placing
                && placement::external_result().is_some_and(|r| r.is_ok()))
        {
            s.phase = Phase::Ready;
            driver::clear_construction_driver();
            placement::reset();
        }
        s.active = true;
        s.preview = false;
        s.message.clear();
        Ok(())
    })
}
fn fail(s: &mut State, message: impl Into<String>) {
    s.active = false;
    s.message = message.into();
    if !driver::workshop_pending() {
        driver::clear_construction_driver();
    }
    placement::pause();
}
fn matches(game: &Game<'_>, at: nav::Pos, w: &schema::BlockState) -> bool {
    w.matches(&game.native_block(at), |key| {
        game.native_block_property(at, key)
    })
}
/// Seeds anywhere the placement driver can reach them: hotbar, off hand or inventory.
fn seed(game: &Game<'_>, crop: farm::Crop) -> bool {
    placement::has_item(game, &|name| name == crop.seed())
}
fn desired(game: &Game<'_>, wanted: &schema::BlockState) -> Result<String, String> {
    let registry = &game.pack.assets.registry;
    let id = registry.default_state(&wanted.name).ok_or_else(|| {
        format!(
            "{}: {}",
            text("Неизвестный блок", "Unknown block"),
            wanted.name
        )
    })?;
    let facts = game.res.facts().get(id);
    if facts.block_entity || facts.fluid_level > 0 || facts.waterlogged {
        return Err(format!(
            "{}: {}",
            text("Данные блока не поддерживаются", "Unsupported block data"),
            wanted.name
        ));
    }
    if facts.item.is_empty() || facts.item.as_ref() == "minecraft:air" {
        Err(format!(
            "{}: {}",
            text("Нет предмета для установки", "No placeable item"),
            wanted.name
        ))
    } else {
        Ok(facts.item.to_string())
    }
}
fn approach(game: &Game<'_>, s: &mut State, target: nav::Pos) -> bool {
    let Some(p) = game.native_player() else {
        return false;
    };
    let level = nav::standing_block(
        [p.x, p.y, p.z],
        &game.native_block(nav::block([p.x, p.y, p.z])),
    )[1];
    let mut candidates = Vec::new();
    for y in [level, target[1], target[1] - 1, target[1] + 1] {
        for dz in -3..=3_i32 {
            for dx in -3..=3_i32 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let at = [target[0] + dx, y, target[2] + dz];
                if nav::distance(nav::center(at), nav::center(target)) > 3.4 {
                    continue;
                }
                if !nav::standable(at, &mut |p| placement::block(game, p).cell) {
                    continue;
                }
                candidates.push(at);
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    if matches!(s.task, Some(Task::Farm { .. })) {
        // Seeds go onto the farmland next to the player's feet. Standing right beside the
        // target keeps the aim ray from meeting the neighbouring crops first.
        candidates.sort_by(|a, b| {
            nav::distance(nav::center(*a), nav::center(target))
                .total_cmp(&nav::distance(nav::center(*b), nav::center(target)))
                .then(
                    nav::distance(nav::center(*a), [p.x, p.y, p.z])
                        .total_cmp(&nav::distance(nav::center(*b), [p.x, p.y, p.z])),
                )
        });
    } else {
        candidates.sort_by(|a, b| {
            nav::distance(nav::center(*a), [p.x, p.y, p.z])
                .total_cmp(&nav::distance(nav::center(*b), [p.x, p.y, p.z]))
        });
    }
    candidates.truncate(16);
    if let Some(at) = candidates.get(s.attempt).copied() {
        s.attempt += 1;
        match driver::workshop_walk(game, at, matches!(s.task, Some(Task::Farm { .. }))) {
            Ok(()) => {
                s.phase = Phase::Walking;
                true
            }
            Err(e) => {
                fail(s, e);
                false
            }
        }
    } else {
        let why = placement::diagnostic();
        let message = text(
            "Нет доступной грани; подойдите ближе и продолжите",
            "No reachable placement face",
        );
        fail(
            s,
            if why.is_empty() {
                message.to_owned()
            } else {
                format!("{message} ({why})")
            },
        );
        false
    }
}
pub(super) fn tick(game: &Game<'_>) {
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    state(|s| {
        if s.scope.is_empty() {
            return;
        }
        if driver::scope(game).as_deref() != Some(s.scope.as_str()) {
            *s = State::default();
            return;
        }
        let answer = s.pending.as_ref().and_then(|p| p.rx.try_recv().ok());
        if let Some(answer) = answer {
            let pending = s.pending.take().unwrap();
            match answer {
                Ok(files::Response::Files(names)) => s.files = names,
                Ok(files::Response::Loaded(document)) => {
                    let ignored = document.ignored_data;
                    s.task = Some(Task::Build {
                        document,
                        cursor: 0,
                    });
                    s.active = pending.run && game.native_playing();
                    s.preview = !s.active;
                    s.message = format!("{}: {ignored}", text("Пропущено NBT", "NBT skipped"));
                }
                Err(e) => fail(s, e),
            }
        }
        if !s.active || s.task.is_none() {
            return;
        }
        if !game.native_playing() {
            s.active = false;
            return;
        }
        match s.phase {
            Phase::Done => {
                s.active = false;
                return;
            }
            Phase::Walking => match driver::workshop_walk_result() {
                None => return,
                Some(false) => {
                    let target = s.target.unwrap();
                    approach(game, s, target);
                    return;
                }
                Some(true) => s.phase = Phase::Ready,
            },
            Phase::Digging => match driver::farm_result() {
                None => return,
                Some(Err(schema::PlaceError::Unreachable)) => {
                    driver::clear_construction_driver();
                    approach(game, s, s.target.unwrap());
                    return;
                }
                Some(Err(e)) => {
                    fail(s, format!("{}: {}", text("Сбор", "Harvest"), reason(e)));
                    return;
                }
                Some(Ok(())) => {
                    if let Some(Task::Farm {
                        harvested, config, ..
                    }) = s.task.as_mut()
                    {
                        *harvested += 1;
                        s.plant = config.replant;
                    }
                    driver::clear_construction_driver();
                    s.phase = Phase::Ready;
                    if !s.plant {
                        s.target = None;
                    }
                }
            },
            Phase::Placing => {
                let Some(result) = placement::external_result() else {
                    return;
                };
                if !placement::external_settled() {
                    return;
                }
                match result {
                    Ok(_) => {
                        if let Some(Task::Drain { filled, .. }) = s.task.as_mut() {
                            *filled += 1;
                            s.wait = s.settle;
                        }
                        s.target = None;
                        s.plant = false;
                        s.attempt = 0;
                        s.phase = Phase::Ready;
                    }
                    Err(schema::PlaceError::Unreachable) => {
                        let target = s.target.unwrap();
                        approach(game, s, target);
                        return;
                    }
                    Err(e) => {
                        fail(
                            s,
                            format!("{}: {}", text("Установка", "Placement"), reason(e)),
                        );
                        return;
                    }
                }
            }
            Phase::Ready => {}
        }
        if !placement::external_settled() {
            return;
        }
        if s.target.is_none() {
            match s.task.as_mut().unwrap() {
                Task::Farm {
                    config,
                    origin,
                    harvested,
                } => {
                    if config.limit > 0 && *harvested >= config.limit {
                        s.phase = Phase::Done;
                        s.active = false;
                        s.message = text("Готово", "Done").into();
                        return;
                    }
                    if s.last.elapsed() < Duration::from_millis(500) {
                        return;
                    }
                    s.last = Instant::now();
                    let r = i32::from(config.radius);
                    let mut targets = Vec::new();
                    for z in -r..=r {
                        for x in -r..=r {
                            let at = [origin[0] + x, origin[1], origin[2] + z];
                            if game.native_block([at[0], at[1] - 1, at[2]]) != "minecraft:farmland"
                            {
                                continue;
                            }
                            let name = game.native_block(at);
                            let mature = name == config.crop.block()
                                && config
                                    .crop
                                    .mature(game.native_block_property(at, "age").as_deref());
                            let empty = config.replant && name == "minecraft:air";
                            if mature || empty {
                                targets.push((at, empty));
                            }
                        }
                    }
                    let p = game.native_player().unwrap();
                    targets.sort_by(|a, b| {
                        nav::distance(nav::center(a.0), [p.x, p.y, p.z])
                            .total_cmp(&nav::distance(nav::center(b.0), [p.x, p.y, p.z]))
                    });
                    if let Some((at, plant)) = targets.first() {
                        s.target = Some(*at);
                        s.plant = *plant;
                        s.attempt = 0;
                    } else {
                        s.message = text("Ожидание созревания", "Waiting for ripe crops").into();
                        return;
                    }
                }
                Task::Drain {
                    config,
                    origin,
                    filled,
                } => {
                    if s.last.elapsed() < s.wait {
                        return;
                    }
                    s.last = Instant::now();
                    s.wait = Duration::from_millis(400);
                    let Some(p) = game.native_player() else {
                        return;
                    };
                    if *filled >= 4000 {
                        s.phase = Phase::Done;
                        s.active = false;
                        s.message =
                            text("Достигнут предел операций", "Operation limit reached").into();
                        return;
                    }
                    let falling = placement::drain_item(game).is_some_and(|(_, falling)| falling);
                    s.settle = Duration::from_millis(if falling { 1500 } else { 400 });
                    let eye = [p.x, p.y + 1.2, p.z];
                    let r = i32::from(config.radius);
                    let body = p.y.floor() as i32;
                    let solid = |at: nav::Pos| placement::block(game, at).cell == nav::Cell::Solid;
                    let mut cells = Vec::new();
                    for z in -r..=r {
                        for x in -r..=r {
                            let (cx, cz) = (origin[0] + x, origin[2] + z);
                            // The player's own body occupies these cells.
                            let own = |y: i32| {
                                (p.x - (f64::from(cx) + 0.5)).abs() < 0.8
                                    && (p.z - (f64::from(cz) + 0.5)).abs() < 0.8
                                    && (body..=body + 1).contains(&y)
                            };
                            if falling {
                                // Only the surface cell of a column matters: a dropped block
                                // sinks to the bottom and the column fills from below. The cell
                                // needs a solid neighbour to click on.
                                for y in (origin[1] - 4..=origin[1] + 1).rev() {
                                    let name = game.native_block([cx, y, cz]);
                                    if matches!(
                                        name.as_str(),
                                        "minecraft:air" | "minecraft:cave_air"
                                    ) {
                                        continue;
                                    }
                                    if drain::replaceable(&name) && !own(y) {
                                        let anchored = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                                            .iter()
                                            .any(|[dx, dz]| solid([cx + dx, y, cz + dz]))
                                            || solid([cx, y - 1, cz]);
                                        cells.push(drain::Candidate {
                                            at: [cx, y, cz],
                                            supported: anchored,
                                        });
                                    }
                                    break;
                                }
                                continue;
                            }
                            let mut in_water = false;
                            for y in (origin[1] - drain::DEPTH..=origin[1]).rev() {
                                let name = game.native_block([cx, y, cz]);
                                if drain::replaceable(&name) {
                                    in_water = true;
                                    if own(y) {
                                        continue;
                                    }
                                    cells.push(drain::Candidate {
                                        at: [cx, y, cz],
                                        supported: solid([cx, y - 1, cz]),
                                    });
                                } else if in_water
                                    || !matches!(
                                        name.as_str(),
                                        "minecraft:air" | "minecraft:cave_air"
                                    )
                                {
                                    break;
                                }
                            }
                        }
                    }
                    let picked = if falling {
                        drain::pick_nearest(&cells, eye)
                    } else {
                        drain::pick(&cells, eye)
                    };
                    match picked {
                        Some((at, _)) => {
                            s.target = Some(at);
                            s.attempt = 0;
                            s.idle = 0;
                        }
                        None => {
                            // A block that just sank leaves the cell empty for a moment, so
                            // finish only after several empty scans in a row.
                            s.idle += 1;
                            if s.idle < 4 {
                                s.wait = Duration::from_millis(1000);
                                return;
                            }
                            s.phase = Phase::Done;
                            s.active = false;
                            s.message = if cells.is_empty() {
                                format!("{}: {filled}", text("Готово, засыпано", "Done, filled"))
                            } else if falling {
                                format!(
                                    "{}: {}",
                                    text(
                                        "Нет опоры для клика у края: встаньте на блок у воды",
                                        "Nothing to click on at the edge: stand on a block by the water"
                                    ),
                                    cells.len()
                                )
                            } else {
                                format!(
                                    "{}: {}",
                                    text(
                                        "Дно вне досягаемости или под вами; для глубокой воды нужен песок или гравий",
                                        "Floor out of reach or under you; deep water needs sand or gravel"
                                    ),
                                    cells.len()
                                )
                            };
                            return;
                        }
                    }
                }
                Task::Build { document, cursor } => {
                    for _ in 0..64 {
                        if *cursor == document.blocks.len() {
                            s.phase = Phase::Done;
                            s.active = false;
                            s.message = text("Готово", "Done").into();
                            return;
                        }
                        let (at, wanted) = &document.blocks[*cursor];
                        if matches(game, *at, wanted) {
                            *cursor += 1;
                        } else {
                            s.target = Some(*at);
                            s.attempt = 0;
                            break;
                        }
                    }
                    if s.target.is_none() {
                        return;
                    }
                }
            }
        }
        let target = s.target.unwrap();
        let Some(p) = game.native_player() else {
            return;
        };
        if nav::distance(nav::center(target), [p.x, p.y + 1.2, p.z]) > 3.7 {
            approach(game, s, target);
            return;
        }
        let request = match s.task.as_ref().unwrap() {
            Task::Farm { config, .. } => {
                if config.replant && !seed(game, config.crop) {
                    fail(
                        s,
                        text(
                            "Нужны семена (хотбар, 2-я рука или инвентарь)",
                            "Seeds required (hotbar, off hand or inventory)",
                        ),
                    );
                    return;
                }
                if !s.plant {
                    if game.native_block(target) != config.crop.block()
                        || !config
                            .crop
                            .mature(game.native_block_property(target, "age").as_deref())
                    {
                        s.target = None;
                        return;
                    }
                    match driver::farm_dig(game, target, config.crop) {
                        Ok(()) => s.phase = Phase::Digging,
                        Err(e) => fail(s, e),
                    }
                    return;
                }
                if game.native_block([target[0], target[1] - 1, target[2]]) != "minecraft:farmland"
                {
                    fail(s, text("Грядка изменилась", "Farmland changed"));
                    return;
                }
                let wanted = schema::BlockState::parse(config.crop.block()).unwrap();
                schema::PlacementRequest {
                    target,
                    item: config.crop.seed().into(),
                    state: wanted.clone(),
                    allowed: BTreeMap::from([(target, wanted)]),
                }
            }
            Task::Drain { .. } => {
                if !drain::replaceable(&game.native_block(target)) {
                    s.target = None;
                    return;
                }
                let Some((item, _)) = placement::drain_item(game) else {
                    fail(
                        s,
                        text(
                            "Нужны блоки (хотбар, 2-я рука или инвентарь)",
                            "Blocks required (hotbar, off hand or inventory)",
                        ),
                    );
                    return;
                };
                let Ok(wanted) = schema::BlockState::parse(&item) else {
                    fail(s, text("Блок не поддерживается", "Unsupported block"));
                    return;
                };
                schema::PlacementRequest {
                    target,
                    item,
                    state: wanted.clone(),
                    allowed: BTreeMap::from([(target, wanted)]),
                }
            }
            Task::Build { document, cursor } => {
                let wanted = document.blocks[*cursor].1.clone();
                let item = match desired(game, &wanted) {
                    Ok(i) => i,
                    Err(e) => {
                        fail(s, e);
                        return;
                    }
                };
                let mut allowed = BTreeMap::new();
                for y in -1..=1 {
                    for z in -1..=1 {
                        for x in -1..=1 {
                            let p = [target[0] + x, target[1] + y, target[2] + z];
                            if let Ok(i) = document
                                .blocks
                                .binary_search_by_key(&(p[1], p[2], p[0]), |(p, _)| {
                                    (p[1], p[2], p[0])
                                })
                            {
                                allowed.insert(p, document.blocks[i].1.clone());
                            }
                        }
                    }
                }
                schema::PlacementRequest {
                    target,
                    item,
                    state: wanted,
                    allowed,
                }
            }
        };
        let current = game.native_block(target);
        let free = current == "minecraft:air"
            || current == "minecraft:cave_air"
            || (matches!(s.task, Some(Task::Drain { .. })) && drain::replaceable(&current));
        if !free {
            fail(
                s,
                format!(
                    "{}: {} {} {}",
                    text("Занято", "Occupied"),
                    target[0],
                    target[1],
                    target[2]
                ),
            );
            return;
        }
        driver::clear_construction_driver();
        match placement::place_external(game, request) {
            Ok(()) => s.phase = Phase::Placing,
            Err(e) => fail(s, e),
        }
    });
}
pub(super) fn snapshot() -> Option<(String, bool)> {
    state(|s| {
        let label = match s.task.as_ref() {
            Some(Task::Farm {
                harvested, config, ..
            }) => format!(
                "{}: {harvested}/{}",
                text("Ферма", "Farm"),
                if config.limit == 0 {
                    "∞".into()
                } else {
                    config.limit.to_string()
                }
            ),
            Some(Task::Drain { filled, .. }) => {
                format!("{}: {filled}", text("Осушение", "Drain"))
            }
            Some(Task::Build { document, cursor }) => {
                format!("{}: {cursor}/{}", document.format, document.blocks.len())
            }
            None if s.pending.is_some() || !s.message.is_empty() => {
                text("Схемы", "Schematics").into()
            }
            None => return None,
        };
        Some((
            format!(
                "{label} · {} · {}",
                if s.preview {
                    text("Просмотр", "Preview")
                } else if s.active {
                    text("Работа", "Running")
                } else {
                    text("Пауза", "Paused")
                },
                s.message
            ),
            !s.active,
        ))
    })
}
pub(super) fn materials() -> String {
    state(|s| match &s.task {
        Some(Task::Build { document, .. }) => document
            .materials()
            .into_iter()
            .map(|(n, c)| format!("{n}: {c}"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => text("Загрузите схему", "Load a schematic").into(),
    })
}
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    state(|s| {
        if driver::scope(game).as_deref() != Some(s.scope.as_str()) {
            return;
        }
        let Some(p) = game.native_player() else {
            return;
        };
        let blocks = match &s.task {
            Some(Task::Farm { origin, config, .. }) => {
                let r = i32::from(config.radius);
                vec![
                    [origin[0] - r, origin[1], origin[2] - r],
                    [origin[0] + r, origin[1], origin[2] + r],
                ]
            }
            Some(Task::Drain { origin, config, .. }) => {
                let r = i32::from(config.radius);
                vec![
                    [origin[0] - r, origin[1], origin[2] - r],
                    [origin[0] + r, origin[1], origin[2] + r],
                ]
            }
            Some(Task::Build { document, cursor }) => document
                .blocks
                .iter()
                .skip(*cursor)
                .take(256)
                .map(|(p, _)| *p)
                .collect(),
            None => Vec::new(),
        };
        for at in blocks {
            let a = nav::center(at);
            if nav::distance(a, [p.x, p.y, p.z]) > 96.0 {
                continue;
            }
            for (u, v) in [(0, 1), (0, 2), (1, 2)] {
                for side in 0..4 {
                    let mut b = at.map(f64::from);
                    b[u] += f64::from(side & 1);
                    b[v] += f64::from((side >> 1) & 1);
                    let mut c = b;
                    c[3 - u - v] += 1.0;
                    g.line(b, c, 0xff88ccaa, 1.5);
                }
            }
        }
    });
}
