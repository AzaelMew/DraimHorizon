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
    suppress_shortcut: Option<(char, std::time::Instant)>,
    paste_fields: VecDeque<model::Field>,
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
        s.paste_fields.clear();
    });
}
pub(super) fn sync(model: model::ViewModel) {
    state(|s| {
        s.menu.reconcile(&s.model, &model, s.height.max(240.0));
        s.model = model;
    });
}
pub(super) fn paste(text: &str) {
    state(|s| {
        // Do not paste a secret into a different field if focus changed before the frame drained.
        if let Some(field) = s.paste_fields.pop_front()
            && s.menu.focus == Some(field)
        {
            s.menu.paste(text);
        }
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
    game.native_fill(batch, screen.rect, 0x9c080b12);
    let pw = (screen.rect[2] - 16.0).clamp(220.0, 640.0);
    let ph = (screen.rect[3] - 16.0).clamp(200.0, 400.0);
    let px = (screen.rect[2] - pw) * 0.5;
    let py = (screen.rect[3] - ph) * 0.5;
    game.native_fill(batch, [px, py, pw, ph], 0xf0181c28);
    game.native_fill(batch, [px, py, pw, 2.0], 0xff8b7bff);
    if pw >= 380.0 {
        game.native_fill(batch, [px, py + 2.0, 78.0, ph - 2.0], 0xff111520);
        game.native_text(batch, "SOCIAL", [px + 10.0, py + 20.0], 0xa99eff, false);
    }
    state(|s| {
        s.height = (screen.rect[3] - 16.0).clamp(200.0, 400.0);
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
                    0xff6050ae
                } else if hover {
                    0xff34364c
                } else {
                    0xff242938
                };
                game.native_fill(batch, widget.rect, color);
                game.native_fill(
                    batch,
                    [x, y + h - 1.0, w, 1.0],
                    if widget.selected {
                        0xffb2a5ff
                    } else {
                        0xff3a4053
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
            let display_text =
                if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                    let cursor = s.menu.cursor();
                    let prefix_len = s.menu.text()[..cursor].chars().count();
                    let shown = if s.menu.focus == Some(model::Field::RelayToken) {
                        "*".repeat(prefix_len)
                    } else {
                        s.menu.text()[..cursor].to_owned()
                    };
                    let label = match s.menu.focus {
                        Some(model::Field::Name) => "Имя: ",
                        Some(model::Field::Color) => "Цвет: ",
                        Some(model::Field::PeerCode) => "Код друга: ",
                        _ => "",
                    };
                    let rest = if s.menu.focus == Some(model::Field::RelayToken) {
                        "*".repeat(s.menu.text()[cursor..].chars().count())
                    } else {
                        s.menu.text()[cursor..].to_owned()
                    };
                    format!("{label}{shown}│{rest}")
                } else {
                    widget.text.clone()
                };
            let text =
                if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                    {
                        let (prefix, suffix) =
                            display_text.split_once('│').unwrap_or((&display_text, ""));
                        let prefix = clipped_tail(game, prefix, (w - 14.0).max(0.0) * 0.65);
                        let remaining = (w - 14.0 - game.native_text_width(&prefix)).max(0.0);
                        format!("{prefix}│{}", clipped(game, suffix, remaining))
                    }
                } else {
                    clipped(game, &display_text, (w - 8.0).max(0.0))
                };
            if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                let selected = s.menu.selection();
                if !selected.is_empty()
                    && let Some(caret) = text.find('│')
                {
                    let cursor = s.menu.cursor();
                    let caret_x = x + 4.0 + game.native_text_width(&text[..caret]);
                    let value = s.menu.text();
                    let measure = |part: &str| {
                        if s.menu.focus == Some(model::Field::RelayToken) {
                            game.native_text_width(&"*".repeat(part.chars().count()))
                        } else {
                            game.native_text_width(part)
                        }
                    };
                    let left = (caret_x - measure(&value[selected.start..cursor])).max(x + 4.0);
                    let right = (caret_x + measure(&value[cursor..selected.end])).min(x + w - 4.0);
                    if right > left {
                        game.native_fill(batch, [left, y + 3.0, right - left, h - 6.0], 0x995f50b8);
                    }
                }
            }
            game.native_text(batch, &text, [x + 4.0, y + (h - 8.0) * 0.5], color, true);
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
    _screen: &NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !owned(game) {
        return;
    }
    *canceled = true;
    state(|s| s.menu.scroll(&s.model, s.height, scroll[1]));
}
pub(super) fn key(game: &Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    if !owned(game) {
        return;
    }
    state(|s| {
        if modifiers & (2 | 8) != 0 && matches!(key, "key.keyboard.c" | "key.keyboard.x") {
            *canceled = true;
            s.suppress_shortcut = Some((
                if key.ends_with(".c") { 'c' } else { 'x' },
                std::time::Instant::now(),
            ));
            let selected = s.menu.selected_text();
            if !selected.is_empty() {
                enqueue(s, model::Action::CopyText(selected));
            }
            if key.ends_with(".x") {
                s.menu.delete_selection();
            }
            return;
        }
        if key == "key.keyboard.v" && modifiers & (2 | 8) != 0 {
            *canceled = true;
            s.suppress_shortcut = Some(('v', std::time::Instant::now()));
            if let Some(field) = s.menu.focus
                && s.actions.len() < 32
            {
                s.paste_fields.push_back(field);
                enqueue(s, model::Action::PasteFromClipboard);
            }
            return;
        }
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
    if (game.modifiers.control_key() && !game.modifiers.alt_key()) || game.modifiers.super_key() {
        state(|s| s.suppress_shortcut = None);
        return;
    }
    state(|s| {
        if let Some((shortcut, at)) = s.suppress_shortcut.take()
            && c.eq_ignore_ascii_case(&shortcut)
            && at.elapsed().as_millis() < 100
        {
            return;
        }
        s.menu.character(c);
    });
}
