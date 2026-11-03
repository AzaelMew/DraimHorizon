// Compact waypoint editor and dimension-sorted list. All writes run on the game frame.
use crate::game::{
    Game, claim_borders as claims, claim_borders_client as client,
    claim_borders_input::{Edit, TextInput},
    claim_borders_ui_view as ui, claim_borders_waypoints as model,
    native_hooks::NativeScreenView,
};
use horizon_client::{clipboard::Clipboard, hud::HudBatch};
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};
#[derive(Clone)]
struct Draft {
    id: Option<u64>,
    dimension: String,
    position: [f64; 3],
    name: TextInput,
    color: TextInput,
    kind: model::MarkerKind,
    focus: bool,
}
#[derive(Clone, Copy)]
enum Hit {
    Row(u64),
    Delete(u64),
    World,
    Page(i32),
    Create,
    Name,
    Color,
    Kind,
    Swatch(u32),
    Save,
    Cancel,
}
enum Action {
    Save(Draft),
    Delete(u64),
    Close,
}
#[derive(Default)]
struct State {
    draft: Option<Draft>,
    quick: bool,
    rows: Vec<model::Waypoint>,
    filter: Option<String>,
    page: usize,
    hits: Vec<([f32; 4], Hit)>,
    content: Option<[f32; 4]>,
    pressed: BTreeSet<i32>,
    action: Option<Action>,
    error: String,
    copy: Option<String>,
    paste: bool,
    suppress: Option<std::time::Instant>,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn quick() -> bool {
    state(|s| s.quick)
}
pub(super) fn editing() -> bool {
    state(|s| s.draft.is_some())
}
pub(super) fn open_create(dimension: String, position: [f64; 3], quick: bool) {
    state(|s| {
        s.quick = quick;
        s.suppress = quick.then(std::time::Instant::now);
        s.error.clear();
        s.draft = Some(Draft {
            id: None,
            dimension,
            position,
            name: TextInput::new("Новая метка".into()),
            color: TextInput::new("#FFCC55".into()),
            kind: model::MarkerKind::Pin,
            focus: false,
        });
        if let Some(d) = s.draft.as_mut() {
            d.name.anchor = Some(0);
        }
    });
}
pub(super) fn queue_delete(id: u64) {
    state(|s| s.action = Some(Action::Delete(id)));
}
pub(super) fn icon(kind: model::MarkerKind) -> ui::Icon {
    match kind {
        model::MarkerKind::Pin => ui::Icon::Pin,
        model::MarkerKind::Home => ui::Icon::Home,
        model::MarkerKind::Portal => ui::Icon::Portal,
        model::MarkerKind::Chest => ui::Icon::Chest,
        model::MarkerKind::Star => ui::Icon::Star,
    }
}
fn world(dim: &str) -> String {
    crate::game::claim_borders_map::dimension_label(dim).to_owned()
}
fn world_order(dim: &str) -> u8 {
    match dim {
        "minecraft:overworld" => 0,
        "minecraft:the_nether" => 1,
        "minecraft:the_end" => 2,
        _ => 3,
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        reset();
        return;
    }
    let rows = client::own_waypoints();
    state(|s| s.rows = rows);
    let (action, copy, paste) = state(|s| {
        let p = s.paste;
        s.paste = false;
        (s.action.take(), s.copy.take(), p)
    });
    if let Some(text) = copy {
        game.clipboard.set(&text);
    }
    if paste {
        let text = game.clipboard.get();
        state(|s| {
            if let Some(d) = s.draft.as_mut() {
                if d.focus {
                    d.color.insert(&text, 7);
                } else {
                    d.name.insert(&text, 48);
                }
            }
        });
    }
    if let Some(action) = action {
        let deleting = matches!(&action, Action::Delete(_));
        let result = match action {
            Action::Delete(id) => {
                client::delete_own_waypoint(id).map(|()| "Метка удалена".to_owned())
            }
            Action::Save(d) => model::parse_color(&d.color.value)
                .map_err(|e| e.to_string())
                .and_then(|color| {
                    client::save_own_waypoint(
                        d.id,
                        &d.dimension,
                        d.position,
                        &d.name.value,
                        color,
                        d.kind,
                    )
                })
                .map(|_| "Метка сохранена".into()),
            Action::Close => Ok(String::new()),
        };
        let close = state(|s| match result {
            Ok(text) => {
                s.error = text;
                if !deleting {
                    s.draft = None;
                    let q = s.quick;
                    s.quick = false;
                    q
                } else {
                    false
                }
            }
            Err(e) => {
                s.error = format!("Не удалось сохранить: {e}");
                false
            }
        });
        if close {
            game.native_mod_screen_close(claims::ID);
        }
    }
}
fn list_owned(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(claims::ID)
        && crate::game::claim_borders_menu_view::waypoints_tab()
        && !crate::game::claim_borders_map_view::visible()
        && !crate::game::claim_borders_economy_view::visible()
        && !quick()
}
fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    s: &mut State,
    r: [f32; 4],
    label: &str,
    hit: Hit,
) {
    ui::button(game, batch, r, label, false, screen.mouse);
    s.hits.push((r, hit));
}
pub(super) fn render_list(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !list_owned(game) {
        return;
    }
    let pw = (screen.rect[2] - 16.).clamp(220., 720.);
    let ph = (screen.rect[3] - 16.).clamp(200., 400.);
    let sidebar = if pw >= 380. { 96. } else { 0. };
    let x = (screen.rect[2] - pw) * 0.5 + sidebar + 8.;
    let y = (screen.rect[3] - ph) * 0.5 + 54.;
    let width = pw - sidebar - 16.;
    let h = ph - 62.;
    state(|s| {
        if s.draft.is_none() {
            s.hits.clear();
        }
        s.content = Some([x, y, width, h]);
        let rows: Vec<_> = s
            .rows
            .iter()
            .filter(|w| s.filter.as_ref().is_none_or(|d| w.dimension == *d))
            .cloned()
            .collect();
        let mut rows = rows;
        rows.sort_by(|a, b| {
            (
                world_order(&a.dimension),
                &a.dimension,
                a.name.to_lowercase(),
                a.id,
            )
                .cmp(&(
                    world_order(&b.dimension),
                    &b.dimension,
                    b.name.to_lowercase(),
                    b.id,
                ))
        });
        button(
            game,
            batch,
            screen,
            s,
            [x, y, width * 0.65 - 3., 20.],
            &format!(
                "Мир: {}",
                s.filter.as_deref().map(world).unwrap_or("Все миры".into())
            ),
            Hit::World,
        );
        button(
            game,
            batch,
            screen,
            s,
            [x + width * 0.65, y, width * 0.35, 20.],
            "+ Здесь",
            Hit::Create,
        );
        let per = (((h - 54.) / 34.).floor() as usize).max(1);
        let pages = rows.len().div_ceil(per).max(1);
        s.page = s.page.min(pages - 1);
        for (i, w) in rows.iter().skip(s.page * per).take(per).enumerate() {
            let r = [x, y + 28. + i as f32 * 34., width, 30.];
            ui::button_base(
                game,
                batch,
                r,
                &format!("waypoint:{}", w.id),
                false,
                screen.mouse,
            );
            ui::icon(
                game,
                batch,
                icon(w.kind),
                [r[0] + 7., r[1] + 8., 12., 12.],
                w.color,
            );
            ui::text(
                game,
                batch,
                &ui::clip(game, &w.name, width - 62.),
                [x + 26., r[1] + 4.],
                0xe3ebf3,
                false,
            );
            ui::text(
                game,
                batch,
                &ui::clip(
                    game,
                    &format!(
                        "{} · {:.0} {:.0} {:.0}",
                        world(&w.dimension),
                        w.position[0],
                        w.position[1],
                        w.position[2]
                    ),
                    width - 62.,
                ),
                [x + 26., r[1] + 17.],
                0x9aacc0,
                false,
            );
            s.hits.push(([x, r[1], width - 31., 30.], Hit::Row(w.id)));
            let trash = [x + width - 27., r[1] + 3., 24., 24.];
            ui::button_base(game, batch, trash, "Удалить", false, screen.mouse);
            ui::icon(
                game,
                batch,
                ui::Icon::Trash,
                [trash[0] + 6., trash[1] + 6., 12., 12.],
                0xdfa8b5,
            );
            s.hits.push((trash, Hit::Delete(w.id)));
        }
        if rows.is_empty() {
            ui::text(
                game,
                batch,
                "Нет меток в выбранном мире",
                [x + 6., y + 42.],
                0x9aacc0,
                false,
            );
        }
        let bottom = y + h - 20.;
        button(
            game,
            batch,
            screen,
            s,
            [x, bottom, 26., 18.],
            "<",
            Hit::Page(-1),
        );
        ui::text(
            game,
            batch,
            &format!("{} / {} · {} меток", s.page + 1, pages, rows.len()),
            [x + 34., bottom + 5.],
            0x9aacc0,
            false,
        );
        button(
            game,
            batch,
            screen,
            s,
            [x + width - 26., bottom, 26., 18.],
            ">",
            Hit::Page(1),
        );
    });
}
fn field(game: &Game<'_>, batch: &mut HudBatch, r: [f32; 4], input: &TextInput, focus: bool) {
    ui::button_base(game, batch, r, "waypoint-field", focus, [-1., -1.]);
    let width = r[2] - 12.;
    let mut start = 0;
    if focus {
        while game.native_text_width(&input.value[start..input.cursor]) > width * 0.7 {
            start += input.value[start..].chars().next().unwrap().len_utf8();
        }
    }
    let shown = if focus {
        let prefix = &input.value[start..input.cursor];
        let left = width - game.native_text_width(prefix) - 3.;
        format!(
            "{prefix}│{}",
            ui::clip(game, &input.value[input.cursor..], left)
        )
    } else {
        ui::clip(game, &input.value, width)
    };
    let selected = input.selection();
    if focus && !selected.is_empty() {
        let a = selected.start.max(start);
        let b = selected.end.max(start);
        let left =
            (r[0] + 6. + game.native_text_width(&input.value[start..a])).min(r[0] + r[2] - 5.);
        let right =
            (r[0] + 6. + game.native_text_width(&input.value[start..b])).min(r[0] + r[2] - 5.);
        if right > left {
            ui::rounded(
                game,
                batch,
                [left, r[1] + 3., right - left, r[3] - 6.],
                2.,
                0x665f8091,
            );
        }
    }
    ui::text(game, batch, &shown, [r[0] + 6., r[1] + 6.], 0xe3ebf3, false);
}
pub(super) fn render_popup(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if game.native_mod_screen() != Some(claims::ID) || !editing() {
        return;
    }
    let start = batch.vertices.len();
    let w = (batch.width - 20.).min(320.);
    let h = 210.;
    let x = (batch.width - w) * 0.5;
    let y = (batch.height - h) * 0.5;
    game.native_fill(batch, [0., 0., batch.width, batch.height], 0x55080d16);
    ui::rounded(game, batch, [x, y, w, h], 8., 0xf21d2a3a);
    state(|s| {
        s.hits.clear();
        let Some(d) = s.draft.clone() else {
            return;
        };
        ui::icon(
            game,
            batch,
            icon(d.kind),
            [x + 12., y + 12., 12., 12.],
            ui::preferences(game).accent.color(),
        );
        ui::text(
            game,
            batch,
            if d.id.is_some() {
                "Редактировать метку"
            } else {
                "Новая метка"
            },
            [x + 32., y + 12.],
            0xe3ebf3,
            false,
        );
        let name = [x + 12., y + 34., w - 24., 21.];
        field(game, batch, name, &d.name, !d.focus);
        s.hits.push((name, Hit::Name));
        button(
            game,
            batch,
            screen,
            s,
            [x + 12., y + 62., w - 24., 21.],
            &format!("Вид: {} · изменить", d.kind.label()),
            Hit::Kind,
        );
        ui::text(
            game,
            batch,
            "Цвет метки",
            [x + 12., y + 93.],
            0x9aacc0,
            false,
        );
        for (i, color) in [
            0xffcc55, 0xa4d3c9, 0xa6c9e5, 0xbcb8e3, 0xdfa8b5, 0xa2d0b7, 0xffffff, 0xff8866,
        ]
        .into_iter()
        .enumerate()
        {
            let r = [x + 12. + i as f32 * 23., y + 106., 18., 17.];
            ui::rounded(game, batch, r, 3., 0xff000000 | color);
            s.hits.push((r, Hit::Swatch(color)));
        }
        let color = [x + 12., y + 130., w - 24., 20.];
        field(game, batch, color, &d.color, d.focus);
        s.hits.push((color, Hit::Color));
        ui::text(
            game,
            batch,
            &ui::clip(
                game,
                &format!(
                    "{} · {:.0} {:.0} {:.0}",
                    world(&d.dimension),
                    d.position[0],
                    d.position[1],
                    d.position[2]
                ),
                w - 24.,
            ),
            [x + 12., y + 157.],
            0x9aacc0,
            false,
        );
        if !s.error.is_empty() {
            ui::text(
                game,
                batch,
                &ui::clip(game, &s.error, w - 24.),
                [x + 12., y + 171.],
                0xdfa8b5,
                false,
            );
        }
        button(
            game,
            batch,
            screen,
            s,
            [x + 12., y + h - 27., (w - 30.) * 0.5, 20.],
            "Отмена [Esc]",
            Hit::Cancel,
        );
        button(
            game,
            batch,
            screen,
            s,
            [x + w * 0.5 + 3., y + h - 27., (w - 30.) * 0.5, 20.],
            "Сохранить",
            Hit::Save,
        );
    });
    ui::finish(game, batch, start, "waypoint-editor", "editor");
}
fn choose(game: &Game<'_>, s: &mut State, hit: Hit) {
    match hit {
        Hit::Delete(id) => s.action = Some(Action::Delete(id)),
        Hit::Row(id) => {
            if let Some(w) = s.rows.iter().find(|w| w.id == id) {
                s.error.clear();
                s.draft = Some(Draft {
                    id: Some(id),
                    dimension: w.dimension.clone(),
                    position: w.position,
                    name: TextInput::new(w.name.clone()),
                    color: TextInput::new(format!("#{:06X}", w.color)),
                    kind: w.kind,
                    focus: false,
                });
            }
        }
        Hit::World => {
            let mut worlds: Vec<_> = s.rows.iter().map(|w| w.dimension.clone()).collect();
            worlds.sort_by_key(|d| (world_order(d), d.clone()));
            worlds.dedup();
            s.filter = match &s.filter {
                None => worlds.first().cloned(),
                Some(old) => worlds
                    .iter()
                    .position(|d| d == old)
                    .and_then(|i| worlds.get(i + 1))
                    .cloned(),
            };
            s.page = 0;
        }
        Hit::Page(n) => {
            s.page = if n < 0 {
                s.page.saturating_sub(1)
            } else {
                s.page + 1
            }
        }
        Hit::Create => {
            if let Some(p) = game.native_player() {
                s.error.clear();
                s.draft = Some(Draft {
                    id: None,
                    dimension: p.dimension.into(),
                    position: [p.x, p.y, p.z],
                    name: TextInput::new("Новая метка".into()),
                    color: TextInput::new("#FFCC55".into()),
                    kind: model::MarkerKind::Pin,
                    focus: false,
                });
            }
        }
        Hit::Name => {
            if let Some(d) = s.draft.as_mut() {
                d.focus = false;
            }
        }
        Hit::Color => {
            if let Some(d) = s.draft.as_mut() {
                d.focus = true;
            }
        }
        Hit::Kind => {
            if let Some(d) = s.draft.as_mut() {
                d.kind = d.kind.next();
            }
        }
        Hit::Swatch(color) => {
            if let Some(d) = s.draft.as_mut() {
                d.color = TextInput::new(format!("#{color:06X}"));
            }
        }
        Hit::Save => {
            if let Some(d) = &s.draft {
                if d.name.value.trim().is_empty() {
                    s.error = "Введите название метки".into();
                } else if model::parse_color(&d.color.value).is_err() {
                    s.error = "Цвет должен быть в формате #RRGGBB".into();
                } else {
                    s.action = Some(Action::Save(d.clone()));
                }
            }
        }
        Hit::Cancel => {
            s.draft = None;
            if s.quick {
                s.action = Some(Action::Close);
            }
            s.error.clear();
        }
    }
}
pub(super) fn release(button: i32, canceled: &mut bool) -> bool {
    state(|s| {
        if s.pressed.remove(&button) {
            *canceled = true;
            true
        } else {
            false
        }
    })
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) -> bool {
    let active = editing();
    if !active && !list_owned(game) {
        return false;
    }
    state(|s| {
        let hit = s
            .hits
            .iter()
            .rev()
            .find(|(r, _)| crate::game::claim_borders_ui_style::contains(*r, screen.mouse))
            .map(|(_, h)| *h);
        if active
            || s.content
                .is_some_and(|r| crate::game::claim_borders_ui_style::contains(r, screen.mouse))
        {
            *canceled = true;
            s.pressed.insert(button);
            if button == 0 {
                if let Some(hit) = hit {
                    choose(game, s, hit);
                }
            }
            true
        } else {
            false
        }
    })
}
pub(super) fn key(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if !editing() {
        return false;
    }
    *canceled = true;
    state(|s| match key {
        "key.keyboard.escape" => choose(game, s, Hit::Cancel),
        "key.keyboard.enter" => choose(game, s, Hit::Save),
        "key.keyboard.tab" => {
            if let Some(d) = s.draft.as_mut() {
                d.focus = !d.focus;
            }
        }
        _ => {
            if let Some(d) = s.draft.as_mut() {
                let input = if d.focus { &mut d.color } else { &mut d.name };
                match input.key(
                    key,
                    game.modifiers.control_key(),
                    game.modifiers.shift_key(),
                ) {
                    Edit::Copy(t) => s.copy = Some(t),
                    Edit::Paste => s.paste = true,
                    _ => {}
                }
            }
        }
    });
    true
}
pub(super) fn character(game: &Game<'_>, c: char, canceled: &mut bool) -> bool {
    if !editing() {
        return false;
    }
    *canceled = true;
    if state(|s| {
        s.suppress
            .take()
            .is_some_and(|at| at.elapsed().as_millis() < 150)
            && c.eq_ignore_ascii_case(&'x')
    }) {
        return true;
    }
    if !game.modifiers.control_key() {
        state(|s| {
            if let Some(d) = s.draft.as_mut() {
                if d.focus {
                    d.color.insert(&c.to_string(), 7);
                } else {
                    d.name.insert(&c.to_string(), 48);
                }
            }
        });
    }
    true
}
pub(super) fn scroll(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    amount: f64,
    canceled: &mut bool,
) -> bool {
    if editing() {
        *canceled = true;
        return true;
    }
    if !list_owned(game) {
        return false;
    }
    state(|s| {
        if s.content
            .is_some_and(|r| crate::game::claim_borders_ui_style::contains(r, screen.mouse))
        {
            *canceled = true;
            s.page = if amount > 0. {
                s.page.saturating_sub(1)
            } else {
                s.page + 1
            };
            true
        } else {
            false
        }
    })
}
