// Uses the normal predicted menu engine and ContainerClick packets, never slot writes.
use crate::game::{Game, claim_borders as claims, claim_borders_inventory_sort as model};
use horizon_client::{
    items::ItemStack,
    menu::{Input, Kind},
};
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
struct State {
    requested: bool,
    active: bool,
    context: String,
    expected: Option<Vec<Option<model::Stack>>>,
    last: Instant,
    started: Instant,
    steps: usize,
}
impl Default for State {
    fn default() -> Self {
        Self {
            requested: false,
            active: false,
            context: String::new(),
            expected: None,
            last: Instant::now() - Duration::from_secs(1),
            started: Instant::now(),
            steps: 0,
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if !game.native_on(claims::ID) {
        return;
    }
    if button != 2 {
        reset();
        return;
    }
    let Some(rows) = screen
        .menu
        .and_then(|name| name.strip_prefix("minecraft:generic_9x"))
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| (1..=6).contains(n))
    else {
        return;
    };
    let [x, y, _, _] = screen.rect;
    if screen.mouse[0] < x + 7.0
        || screen.mouse[0] >= x + 169.0
        || screen.mouse[1] < y + 17.0
        || screen.mouse[1] >= y + 18.0 + rows as f32 * 18.0
    {
        return;
    }
    *canceled = true;
    state(|s| {
        if s.active {
            s.active = false;
            s.requested = false;
        } else {
            s.requested = true;
        }
    });
}
fn describe(stack: &ItemStack, env: &horizon_client::menu::Env<'_>) -> model::Stack {
    model::Stack {
        key: format!("{}|{}|{:?}", stack.item, stack.damage, stack.data()),
        count: stack.count,
        max: env
            .items
            .get(&stack.item)
            .map_or(64, |i| i.max_stack)
            .max(1),
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    let context = format!("{:?}|{:?}", game.remote_menu(), game.container);
    let run = state(|s| {
        if s.requested {
            *s = State {
                active: true,
                context: context.clone(),
                ..State::default()
            };
        }
        if !s.active {
            return false;
        }
        if s.context != context || s.started.elapsed() > Duration::from_secs(30) || s.steps > 256 {
            s.active = false;
            return false;
        }
        s.last.elapsed() >= Duration::from_millis(100)
    });
    if !run {
        return;
    }
    let expected = state(|s| s.expected.clone());
    let mut operations = Vec::new();
    let mut after = None;
    let mut pending = false;
    let mut invalid = false;
    let mut empty_cursor = true;
    let valid = game.menu_session(|menu, env, _| {
        let Kind::Chest(rows) = menu.kind() else {
            return None;
        };
        if !(1..=6).contains(&rows) {
            return None;
        }
        let n = rows * 9;
        if menu.carried.is_some() {
            empty_cursor = false;
            pending = true;
            return Some(false);
        }
        let before: Vec<_> = (0..n)
            .map(|i| menu.slot(i).map(|s| describe(s, env)))
            .collect();
        if expected.as_ref().is_some_and(|wanted| *wanted != before) {
            pending = true;
            return Some(false);
        }
        let plan = match model::next(&before) {
            Ok(plan) => plan,
            Err(_) => {
                invalid = true;
                return Some(false);
            }
        };
        let Some(plan) = plan else {
            return Some(false);
        };
        let tail: Vec<_> = (n..menu.slot_count())
            .map(|i| menu.slot(i).cloned())
            .collect();
        let dropped = menu.dropped.len();
        for slot in &plan.clicks {
            if menu.click(env, *slot as i32, 0, Input::Pickup).is_err() {
                invalid = true;
                return Some(false);
            }
        }
        let observed: Vec<_> = (0..n)
            .map(|i| menu.slot(i).map(|s| describe(s, env)))
            .collect();
        if menu.carried.is_some()
            || menu.dropped.len() != dropped
            || observed != plan.after
            || (n..menu.slot_count())
                .map(|i| menu.slot(i).cloned())
                .collect::<Vec<_>>()
                != tail
        {
            invalid = true;
            return Some(false);
        }
        operations = plan
            .clicks
            .iter()
            .map(|i| (Some(300 + *i), 0, Input::Pickup))
            .collect();
        after = Some(plan.after);
        Some(false)
    });
    if pending {
        if state(|s| s.last.elapsed() > Duration::from_secs(2)) {
            reset();
            if !empty_cursor {
                game.native_chat("[Сортировка] Освободи курсор. Предметы не выбрасываются.");
            } else {
                game.native_chat("[Сортировка] Сундук изменился, сортировка остановлена.");
            }
        }
        return;
    }
    if !valid || invalid {
        reset();
        if invalid {
            game.native_chat(
                "[Сортировка] Эти предметы нельзя безопасно переставить обычными кликами.",
            );
        }
        return;
    }
    if operations.is_empty() {
        reset();
        return;
    }
    if game.menu_ops(&operations) {
        state(|s| {
            s.expected = after;
            s.last = Instant::now();
            s.steps += 1;
        });
    } else {
        reset();
    }
}
