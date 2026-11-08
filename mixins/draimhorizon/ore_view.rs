// Local opt-in controls and through-wall cages for the experimental ore worker.
use crate::game::{
    Game, claim_borders as claims, claim_borders_experimental as model,
    claim_borders_ore_worker as worker,
};
use claim_borders_relay_client::translation::Language;
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
struct State {
    loaded: bool,
    prefs: model::Preferences,
    active: bool,
    scope: String,
    worker: Option<worker::Handle>,
    requested: Option<worker::Key>,
    marks: Vec<model::Mark>,
    last: Instant,
    status: Status,
}
// Status kept as data so it is localized when drawn, not when produced.
#[derive(Clone)]
enum Status {
    Off,
    Preparing,
    OffAfterWorldChange,
    Computing,
    Reading,
    Result { prediction: bool, blocks: usize },
    Error(String),
}
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
impl Default for State {
    fn default() -> Self {
        Self {
            loaded: false,
            prefs: model::Preferences::default(),
            active: false,
            scope: String::new(),
            worker: None,
            requested: None,
            marks: Vec::new(),
            last: Instant::now() - Duration::from_secs(5),
            status: Status::Off,
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn settings(game: &Game<'_>) -> model::Preferences {
    state(|s| {
        if !s.loaded {
            s.prefs = game
                .native_setting(claims::SETTINGS_ID, "ore_experiment")
                .and_then(|s| model::Preferences::decode(&s).ok())
                .unwrap_or_default();
            s.loaded = true;
        }
        s.prefs
    })
}
pub(super) fn configure(game: &Game<'_>, prefs: model::Preferences) {
    let prefs = prefs.normalized();
    game.native_set_setting(claims::SETTINGS_ID, "ore_experiment", &prefs.encode());
    state(|s| {
        s.prefs = prefs;
        s.loaded = true;
        s.requested = None;
        s.worker = None;
        s.marks.clear();
        if !prefs.allowed {
            s.active = false;
            s.status = Status::Off;
        }
    });
}
/// Ores of one kind that the experiment currently marks in loaded chunks. The mining job
/// tries these first, so the route leads to what the player sees highlighted.
pub(super) fn marks_for(kind: u8) -> Vec<[i32; 3]> {
    state(|s| {
        s.marks
            .iter()
            .filter(|mark| mark.kind == kind && !mark.predicted)
            .map(|mark| mark.position)
            .collect()
    })
}
pub(super) fn status() -> String {
    let status = state(|s| s.status.clone());
    let language = language();
    match status {
        Status::Off => language.text("Подсветка выключена", "Highlight off").into(),
        Status::Preparing => language
            .text("Подготовка подсветки…", "Preparing highlight…")
            .into(),
        Status::OffAfterWorldChange => language
            .text(
                "Подсветка выключена после смены мира",
                "Highlight turned off after world change",
            )
            .into(),
        Status::Computing => language
            .text("Расчёт генерации сида…", "Computing seed generation…")
            .into(),
        Status::Reading => language
            .text("Чтение загруженных чанков…", "Reading loaded chunks…")
            .into(),
        Status::Result { prediction, blocks } => {
            let source = if prediction {
                language.text("Прогноз сида (1 чанк)", "Seed prediction (1 chunk)")
            } else {
                language.text("Загруженные чанки", "Loaded chunks")
            };
            match language {
                Language::Russian => format!("{source}: {blocks} блоков · O — выключить"),
                Language::English => format!("{source}: {blocks} blocks · O — turn off"),
            }
        }
        Status::Error(e) => {
            let e = if e == worker::UNSUPPORTED_DIMENSION {
                language
                    .text(
                        "Прогноз доступен только для обычного мира и Незера",
                        "Prediction works only in the Overworld and the Nether",
                    )
                    .to_owned()
            } else {
                e
            };
            match language {
                Language::Russian => format!("Ошибка эксперимента: {e}"),
                Language::English => format!("Experiment error: {e}"),
            }
        }
    }
}
pub(super) fn reset() {
    state(|s| {
        s.active = false;
        s.worker = None;
        s.requested = None;
        s.marks.clear();
        s.scope.clear();
        s.status = Status::Off;
    });
}
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if key != game.native_mod_key(claims::ID, "ore_experiment", model::DEFAULT_KEY) {
        return;
    }
    static HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let previous = HELD.swap(pressed, std::sync::atomic::Ordering::Relaxed);
    if !pressed || previous || !game.native_playing() || !game.native_on(claims::ID) {
        return;
    }
    if !settings(game).allowed {
        game.native_chat(language().text(
            "[Эксперименты] Сначала разрешите подсветку руд в J → Эксперименты.",
            "[Experiments] First allow ore highlighting in J → Experiments.",
        ));
        return;
    }
    let on = state(|s| {
        s.active = !s.active;
        s.worker = None;
        s.requested = None;
        s.marks.clear();
        s.status = if s.active {
            Status::Preparing
        } else {
            Status::Off
        };
        s.active
    });
    game.native_chat(match (on, language()) {
        (true, Language::Russian) => {
            "[Эксперименты] Подсветка руд включена. Прогноз по сиду не гарантирует наличие блока."
        }
        (true, Language::English) => {
            "[Experiments] Ore highlight on. Seed prediction does not guarantee the block exists."
        }
        (false, Language::Russian) => "[Эксперименты] Подсветка руд выключена.",
        (false, Language::English) => "[Experiments] Ore highlight off.",
    });
}
pub(super) fn frame(game: &Game<'_>) {
    if !game.native_on(claims::ID) || game.native_player().is_none() {
        reset();
        return;
    }
    let prefs = settings(game);
    if !prefs.allowed {
        return;
    }
    let player = game.native_player().unwrap();
    let dim = if player.dimension.contains(':') {
        player.dimension.to_string()
    } else {
        format!("minecraft:{}", player.dimension)
    };
    let scope = game
        .remote
        .as_ref()
        .map(|r| {
            format!(
                "{}#{:032x}#{}",
                r.session.address,
                r.profile(),
                game.pack.render.generation
            )
        })
        .unwrap_or_else(|| {
            format!(
                "world:{}#{}",
                game.world_root.to_string_lossy(),
                game.pack.render.generation
            )
        });
    let center = (
        (player.x.floor() as i32).div_euclid(16),
        (player.z.floor() as i32).div_euclid(16),
    );
    state(|s| {
        if !s.scope.is_empty() && s.scope != scope {
            s.active = false;
            s.worker = None;
            s.requested = None;
            s.marks.clear();
            s.status = Status::OffAfterWorldChange;
        }
        s.scope = scope.clone();
        if !s.active {
            return;
        }
        let key = worker::Key {
            scope,
            dimension: dim,
            center,
            vertical_band: (player.y.floor() as i32).div_euclid(32),
            ores: prefs.ores,
            prediction: prefs.prediction,
        };
        if let Some(result) = s.worker.as_ref().and_then(worker::Handle::take) {
            if s.requested.as_ref() == Some(&result.key) {
                s.status = result.error.map_or_else(
                    || Status::Result {
                        prediction: prefs.prediction,
                        blocks: result.marks.len(),
                    },
                    Status::Error,
                );
                s.marks = result.marks;
            }
        }
        if s.requested.as_ref() == Some(&key)
            && (prefs.prediction || s.last.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        if s.worker.is_none() {
            match worker::Handle::spawn(
                game.options.resources.join("worldgen"),
                game.pack.assets.registry,
            ) {
                Ok(w) => s.worker = Some(w),
                Err(e) => {
                    s.status = Status::Error(e);
                    s.active = false;
                    return;
                }
            }
        }
        let chunks = if prefs.prediction {
            Vec::new()
        } else {
            (-1..=1)
                .flat_map(|z| (-1..=1).map(move |x| (center.0 + x, center.1 + z)))
                .filter_map(|at| game.world.chunks.get(&at).map(|chunk| (at, chunk.clone())))
                .collect()
        };
        if s.requested.as_ref() != Some(&key) {
            s.marks.clear();
        }
        s.last = Instant::now();
        s.requested = Some(key.clone());
        s.status = if prefs.prediction {
            Status::Computing
        } else {
            Status::Reading
        };
        s.worker.as_ref().unwrap().query(worker::Query {
            key,
            player: [player.x, player.y, player.z],
            chunks,
        });
    });
}
pub(super) fn render(game: &Game<'_>, gizmos: &mut horizon_client::gizmos::Gizmos) {
    if !game.native_on(claims::ID) || game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let Some(player) = game.native_player() else {
        return;
    };
    let marks = state(|s| {
        if s.active && s.prefs.allowed {
            s.marks.clone()
        } else {
            Vec::new()
        }
    });
    for mark in marks {
        let at = mark.position;
        let distance = (f64::from(at[0]) + 0.5 - player.x).powi(2)
            + (f64::from(at[1]) + 0.5 - player.y).powi(2)
            + (f64::from(at[2]) + 0.5 - player.z).powi(2);
        if distance > model::RANGE * model::RANGE {
            continue;
        }
        let actual = game
            .world
            .chunks
            .get(&(at[0].div_euclid(16), at[2].div_euclid(16)))
            .and_then(|c| {
                game.pack
                    .assets
                    .registry
                    .state(horizon_world::registry::BlockStateId(
                        c.get(at[0], at[1], at[2]),
                    ))
            });
        if mark.predicted {
            if actual.is_some_and(|s| {
                matches!(
                    s.name.as_str(),
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                )
            }) {
                continue;
            }
        } else if !actual.is_some_and(|s| {
            model::ORES[usize::from(mark.kind)]
                .blocks
                .contains(&s.name.as_str())
        }) {
            continue;
        }
        for (start, end) in model::edges(&mark) {
            gizmos.on_top.push(horizon_client::gizmos::Line {
                start,
                end,
                color: 0xe0000000 | model::ORES[usize::from(mark.kind)].color,
                width: 1.5,
            });
        }
    }
}
