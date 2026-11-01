// Horizon drawing/input adapter. Mutating Game operations are drained on the client tick.
use crate::game::native_hooks::NativeScreenView;
use crate::game::{Game, claim_borders as claims, claim_borders_menu as model};
use horizon_client::hud::HudBatch;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct State {
    menu: model::Menu,
    model: model::ViewModel,
    actions: VecDeque<model::Action>,
    // Release is consumed only if this screen consumed the matching press.
    pressed: Vec<i32>,
    height: f32,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    let mut state = STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    f(&mut state)
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn open() {
    state(|s| {
        s.menu = model::Menu::default();
        s.actions.clear();
        s.pressed.clear();
    });
}
pub(super) fn sync(model: model::ViewModel) {
    state(|s| {
        s.menu.reconcile(&s.model, &model, s.height.max(240.0));
        s.model = model;
    });
}
pub(super) fn drain_actions() -> Vec<model::Action> {
    state(|s| s.actions.drain(..).collect())
}
fn owned(game: &Game<'_>) -> bool {
    game.native_on(claims::ID) && game.native_mod_screen() == Some(claims::ID)
}
fn size(screen: &NativeScreenView<'_>) -> [f32; 2] {
    [screen.rect[2], screen.rect[3]]
}
fn enqueue(s: &mut State, action: model::Action) {
    // Bound repeated physical input while the client is busy. Preserve action order.
    if s.actions.len() < 32 {
        s.actions.push_back(action);
    }
}
fn clipped(game: &Game<'_>, text: &str, width: f32) -> String {
    if game.native_text_width(text) <= width {
        return text.to_owned();
    }
    let mut result = String::new();
    for c in text.chars() {
        result.push(c);
        if game.native_text_width(&result) + game.native_text_width("…") > width {
            result.pop();
            break;
        }
    }
    result.push('…');
    result
}
fn clipped_tail(game: &Game<'_>, text: &str, width: f32) -> String {
    if game.native_text_width(text) <= width {
        return text.to_owned();
    }
    for (offset, _) in text.char_indices().skip(1) {
        let tail = format!("…{}", &text[offset..]);
        if game.native_text_width(&tail) <= width {
            return tail;
        }
    }
    "…".into()
}
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !owned(game) {
        return;
    }
    game.native_fill(batch, screen.rect, 0xdf101820);
    state(|s| {
        s.height = screen.rect[3];
        s.menu.clamp_page(&s.model, s.height);
        let widgets = s.menu.layout(&s.model, screen.rect[2], screen.rect[3]);
        for widget in widgets {
            let [x, y, w, h] = widget.rect;
            let hover = widget.control.is_some()
                && screen.mouse[0] >= x
                && screen.mouse[0] < x + w
                && screen.mouse[1] >= y
                && screen.mouse[1] < y + h;
            if widget.control.is_some() {
                let color = if widget.selected {
                    0xf0306882
                } else if hover {
                    0xf0445668
                } else {
                    0xe0293544
                };
                game.native_fill(batch, widget.rect, color);
                game.native_fill(
                    batch,
                    [x, y + h - 1.0, w, 1.0],
                    if widget.selected {
                        0xff6dd5ed
                    } else {
                        0xff536071
                    },
                );
            }
            let color = if widget.muted {
                0x8995a5
            } else if widget.selected {
                0xffffff
            } else {
                0xe6edf5
            };
            let text =
                if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                    clipped_tail(game, &widget.text, (w - 8.0).max(0.0))
                } else {
                    clipped(game, &widget.text, (w - 8.0).max(0.0))
                };
            game.native_text(batch, &text, [x + 4.0, y + (h - 8.0) * 0.5], color, true);
            if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                let cursor = (x + 4.0 + game.native_text_width(&text)).min(x + w - 3.0);
                game.native_fill(batch, [cursor, y + 4.0, 1.0, h - 8.0], 0xffeeeeee);
            }
        }
    });
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) {
    if !owned(game) {
        return;
    }
    state(|s| {
        if pressed {
            *canceled = true;
            if !s.pressed.contains(&button) {
                s.pressed.push(button);
            }
            if button == 0
                && let Some(action) = s.menu.click(&s.model, size(screen), screen.mouse)
            {
                enqueue(s, action);
            }
        } else if let Some(index) = s.pressed.iter().position(|b| *b == button) {
            s.pressed.swap_remove(index);
            *canceled = true;
        }
    });
}
pub(super) fn scroll(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !owned(game) {
        return;
    }
    *canceled = true;
    state(|s| s.menu.scroll(&s.model, screen.rect[3], scroll[1]));
}
pub(super) fn key(game: &Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    if !owned(game) {
        return;
    }
    state(|s| {
        if s.menu.key(key, modifiers) {
            *canceled = true;
        }
    });
}
pub(super) fn character(game: &Game<'_>, c: char, canceled: &mut bool) {
    if !owned(game) {
        return;
    }
    *canceled = true;
    state(|s| s.menu.character(c));
}
