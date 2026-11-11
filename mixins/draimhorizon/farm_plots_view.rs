// Farm plots in the game: `/farm pos1` and `/farm pos2` mark two corners where the player stands
// (the box between them is drawn in yellow), `/farm save [name]` keeps it as a plot of this world,
// `/farm clear` drops the marks and `/farm list` lists the plots of this world. Plots are kept in
// the settings, listed on the Farm tab (farm it or not, show or hide, delete) and drawn while
// shown. The farm works the enabled plots of the world it is in (`workshop_view`), so it finds its
// field again after a reconnect.
use crate::game::{
    Game, claim_borders as claims, claim_borders_farm_plots as plots,
    claim_borders_navigation as nav, claim_borders_navigation_view as walk,
};
use horizon_client::gizmos::Gizmos;
use std::sync::Mutex;
const SETTING: &str = "farm_plots";
struct State {
    loaded: bool,
    list: Vec<plots::Plot>,
    /// The corners marked so far, each with the world it was marked in.
    pos1: Option<(String, plots::Pos)>,
    pos2: Option<(String, plots::Pos)>,
}
static STATE: Mutex<State> = Mutex::new(State {
    loaded: false,
    list: Vec::new(),
    pos1: None,
    pos2: None,
});
fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
fn with<T>(game: &Game<'_>, f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.loaded {
        s.loaded = true;
        s.list = game
            .native_setting(claims::SETTINGS_ID, SETTING)
            .map(|v| plots::decode(&v))
            .unwrap_or_default();
    }
    f(&mut s)
}
fn save(game: &Game<'_>, list: &[plots::Plot]) {
    game.native_set_setting(claims::SETTINGS_ID, SETTING, &plots::encode(list));
}
/// The enabled plots of the world the player is in: what the farm works.
pub(super) fn enabled(game: &Game<'_>) -> Vec<plots::Bounds> {
    let Some(scope) = walk::world_key(game) else {
        return Vec::new();
    };
    with(game, |s| plots::enabled_in(&s.list, &scope).collect())
}
/// The plots of this world for the Farm tab: index, label, farmed, shown.
pub(super) fn rows(game: &Game<'_>) -> Vec<(usize, String, bool, bool)> {
    let Some(scope) = walk::world_key(game) else {
        return Vec::new();
    };
    with(game, |s| {
        s.list
            .iter()
            .enumerate()
            .filter(|(_, p)| p.scope == scope)
            .map(|(i, p)| {
                let b = p.bounds;
                let label = format!(
                    "{} · {}×{} · {} {} {}",
                    p.name,
                    b.hi[0] - b.lo[0] + 1,
                    b.hi[2] - b.lo[2] + 1,
                    b.lo[0],
                    b.lo[1] + 1,
                    b.lo[2]
                );
                (i, label, p.enabled, p.visible)
            })
            .collect()
    })
}
/// Changes the plot at `index` of this world and keeps the list.
fn edit(game: &Game<'_>, index: usize, f: impl FnOnce(&mut Vec<plots::Plot>, usize)) {
    let Some(scope) = walk::world_key(game) else {
        return;
    };
    with(game, |s| {
        if s.list.get(index).is_some_and(|p| p.scope == scope) {
            f(&mut s.list, index);
            save(game, &s.list);
        }
    });
}
/// Farm the plot or not. Turning one on is refused when the world's enabled plots would be
/// too large together (the farm reads every cell of them twice a second).
pub(super) fn toggle_farm(game: &Game<'_>, index: usize) -> Result<(), String> {
    let mut refused = false;
    edit(game, index, |list, i| {
        if !list[i].enabled {
            let scope = list[i].scope.clone();
            let total = plots::enabled_cells(list, &scope) + list[i].bounds.cells();
            if total > plots::MAX_CELLS {
                refused = true;
                return;
            }
        }
        list[i].enabled = !list[i].enabled;
    });
    if refused {
        return Err(too_many_cells());
    }
    Ok(())
}
pub(super) fn toggle_visible(game: &Game<'_>, index: usize) {
    edit(game, index, |list, i| list[i].visible = !list[i].visible);
}
pub(super) fn remove(game: &Game<'_>, index: usize) {
    edit(game, index, |list, i| {
        list.remove(i);
    });
}
fn too_many_cells() -> String {
    format!(
        "{} {}",
        text(
            "Включённые участки этого мира вместе больше",
            "The enabled plots of this world together exceed"
        ),
        plots::MAX_CELLS
    ) + text(
        " клеток (ширина × длина × уровни); выключите другой участок.",
        " cells (width × depth × levels); turn another plot off.",
    )
}
/// The crop cell the player stands in: where a crop grows (one above farmland), or the air above
/// the block underfoot on a path between the beds.
fn standing(game: &Game<'_>) -> Option<plots::Pos> {
    let p = game.native_player()?;
    let position = [p.x, p.y, p.z];
    Some(nav::standing_block(
        position,
        &game.native_block(nav::block(position)),
    ))
}
/// `/farm pos1`, `/farm pos2`, `/farm save [name]`, `/farm clear`, `/farm list`: handled here and
/// not sent.
/// Anything else after `/farm` goes to the server (it may have a command of that name).
pub(super) fn command(game: &mut Game<'_>, message: &str) -> bool {
    let message = message.trim();
    let mut words = message.split_whitespace();
    if !words
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case("/farm"))
    {
        return false;
    }
    let Some(sub) = words.next().map(str::to_ascii_lowercase) else {
        return false;
    };
    if !matches!(sub.as_str(), "pos1" | "pos2" | "save" | "clear" | "list")
        || !game.native_on(claims::ID)
    {
        return false;
    }
    // Kept in the chat history like any sent line (the arrow keys bring it back).
    game.ui.chat.add_recent(message);
    let reply = run(game, &sub, &words.collect::<Vec<_>>().join(" "));
    game.native_chat(&format!("[DraimHorizon] {reply}"));
    true
}
fn run(game: &Game<'_>, sub: &str, rest: &str) -> String {
    let Some(scope) = walk::world_key(game) else {
        return text("Сначала войдите в мир.", "Join a world first.").into();
    };
    match sub {
        "pos1" | "pos2" => {
            let Some(at) = standing(game) else {
                return text("Сначала войдите в мир.", "Join a world first.").into();
            };
            with(game, |s| {
                let slot = if sub == "pos1" {
                    &mut s.pos1
                } else {
                    &mut s.pos2
                };
                *slot = Some((scope.clone(), at));
            });
            let [x, y, z] = at;
            let point = if sub == "pos1" {
                text("Точка 1", "Point 1")
            } else {
                text("Точка 2", "Point 2")
            };
            let both = with(game, |s| s.pos1.is_some() && s.pos2.is_some());
            format!(
                "{point}: {x} {y} {z}.{}",
                if both {
                    text(
                        " Сохраните: /farm save [имя]",
                        " Save it: /farm save [name]",
                    )
                } else {
                    ""
                }
            )
        }
        "list" => {
            let rows = rows(game);
            if rows.is_empty() {
                return text(
                    "Участков в этом мире нет. Отметьте: /farm pos1, /farm pos2, /farm save",
                    "No plots in this world. Mark one: /farm pos1, /farm pos2, /farm save",
                )
                .into();
            }
            let lines: Vec<String> = rows
                .iter()
                .enumerate()
                .map(|(n, (_, label, farmed, _))| {
                    format!(
                        "{}. {label} ({})",
                        n + 1,
                        if *farmed {
                            text("ферма вкл", "farm on")
                        } else {
                            text("ферма выкл", "farm off")
                        }
                    )
                })
                .collect();
            format!("{} {}", text("Участки:", "Plots:"), lines.join("; "))
        }
        "clear" => {
            with(game, |s| {
                s.pos1 = None;
                s.pos2 = None;
            });
            text("Отметки сброшены.", "Marks cleared.").into()
        }
        _ => with(game, |s| {
            let (Some((w1, a)), Some((w2, b))) = (&s.pos1, &s.pos2) else {
                return text(
                    "Сначала отметьте две точки: /farm pos1 и /farm pos2.",
                    "Mark two points first: /farm pos1 and /farm pos2.",
                )
                .into();
            };
            if w1 != &scope || w2 != &scope {
                return text(
                    "Точки отмечены в другом мире; отметьте их здесь заново.",
                    "The points were marked in another world; mark them here again.",
                )
                .into();
            }
            let bounds = plots::Bounds::between(*a, *b);
            if let Some(why) = bounds.too_large() {
                return if why == "side" {
                    format!(
                        "{} {}.",
                        text(
                            "Участок слишком большой: сторона не больше",
                            "The plot is too large: at most a side of"
                        ),
                        plots::MAX_SIDE
                    )
                } else {
                    format!(
                        "{} {}.",
                        text(
                            "Участок слишком высокий: не больше уровней",
                            "The plot is too tall: at most levels of"
                        ),
                        plots::MAX_HEIGHT
                    )
                };
            }
            if s.list.len() >= plots::MAX {
                return text(
                    "Участков слишком много; удалите ненужные в меню.",
                    "Too many plots; delete some in the menu.",
                )
                .into();
            }
            let name = rest.trim();
            let name = if name.is_empty() {
                plots::next_name(&s.list, &scope, text("Поле", "Field"))
            } else {
                name.chars().filter(|c| !c.is_control()).take(32).collect()
            };
            let fits = plots::enabled_cells(&s.list, &scope) + bounds.cells() <= plots::MAX_CELLS;
            s.list.push(plots::Plot {
                scope: scope.clone(),
                name: name.clone(),
                bounds,
                enabled: fits,
                visible: true,
            });
            s.pos1 = None;
            s.pos2 = None;
            save(game, &s.list);
            let size = format!(
                "{}×{}",
                bounds.hi[0] - bounds.lo[0] + 1,
                bounds.hi[2] - bounds.lo[2] + 1
            );
            if fits {
                format!(
                    "{} «{name}» ({size}). {}",
                    text("Участок сохранён", "Plot saved"),
                    text(
                        "Ферма работает на включённых участках этого мира.",
                        "The farm works the enabled plots of this world."
                    )
                )
            } else {
                format!(
                    "{} «{name}» ({size}), {} {}",
                    text("Участок сохранён", "Plot saved"),
                    text("но выключен:", "but off:"),
                    too_many_cells()
                )
            }
        }),
    }
}
/// The marks (yellow: the corners and the box between them) and the shown plots of this world
/// (green when farmed, grey when not).
pub(super) fn render(game: &Game<'_>, g: &mut Gizmos) {
    if game.title.is_some() || game.ui.hide_gui {
        return;
    }
    let Some(scope) = walk::world_key(game) else {
        return;
    };
    let (marks, shown) = with(game, |s| {
        let here = |m: &Option<(String, plots::Pos)>| {
            m.as_ref().filter(|(w, _)| *w == scope).map(|(_, at)| *at)
        };
        let shown: Vec<(plots::Bounds, bool)> = s
            .list
            .iter()
            .filter(|p| p.scope == scope && p.visible)
            .map(|p| (p.bounds, p.enabled))
            .collect();
        ((here(&s.pos1), here(&s.pos2)), shown)
    });
    // A plot's cells from its lowest to its highest marked level (not the block of unevenness
    // looked at above and below).
    let cells = |b: plots::Bounds| {
        (
            [
                f64::from(b.lo[0]),
                f64::from(b.lo[1] + 1),
                f64::from(b.lo[2]),
            ],
            [
                f64::from(b.hi[0] + 1),
                f64::from(b.hi[1]),
                f64::from(b.hi[2] + 1),
            ],
        )
    };
    for (bounds, farmed) in shown {
        let (lo, hi) = cells(bounds);
        g.cuboid(lo, hi, if farmed { 0xcc55dd66 } else { 0x99aaaaaa }, 1.5);
    }
    const YELLOW: u32 = 0xffffd23c;
    for at in [marks.0, marks.1].into_iter().flatten() {
        let lo = at.map(|v| f64::from(v) + 0.1);
        g.cuboid(lo, lo.map(|v| v + 0.8), YELLOW, 2.0);
    }
    if let (Some(a), Some(b)) = marks {
        let (lo, hi) = cells(plots::Bounds::between(a, b));
        g.cuboid(lo, hi, YELLOW, 2.0);
    }
}
