use crate::game::claim_borders_ui_view as ui;
// Horizon drawing/input adapter. Mutating Game operations are drained on the client tick.
use crate::game::native_hooks::NativeScreenView;
use crate::game::{Game, claim_borders as claims, claim_borders_menu as model};
use horizon_client::{hud::HudBatch, items::ItemStack};
use model::Language;
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
pub(super) fn claims_settings() {
    state(|s| {
        s.menu.tab = model::Tab::Claims;
        s.menu.settings_page = 0;
        s.menu.navigation_menu = false;
        s.menu.claims_section = model::ClaimsSection::Map;
        s.menu.focus = None;
    });
}
pub(super) fn trading_settings() {
    state(|s| {
        s.menu.tab = model::Tab::Trading;
        s.menu.navigation_menu = false;
        s.menu.settings_page = 0;
        s.menu.focus = None;
    });
}
pub(super) fn experiments_prompt() -> bool {
    state(|s| s.menu.experiments_prompt)
}
pub(super) fn navigation_open() -> bool {
    state(|s| s.menu.navigation_menu)
}
pub(super) fn reset() {
    state(|s| {
        let previous = std::mem::take(&mut s.menu);
        *s = State::default();
        s.menu.keep_baritone(&previous);
    });
}
/// The Baritone task the Experiments tab currently describes, for the action keys.
/// It works with the menu closed: the settings survive `open` and `reset`.
pub(super) fn baritone_request(running: bool) -> Result<model::Action, String> {
    state(|s| s.menu.baritone_request(running))
}
/// Whether the Baritone tab last on screen is Schematics.
pub(super) fn schematic_mode() -> bool {
    state(|s| s.menu.baritone_mode == model::BaritoneMode::Schematic)
}
pub(super) fn open() {
    state(|s| {
        let previous = std::mem::take(&mut s.menu);
        s.menu = model::Menu::default();
        s.menu.keep_baritone(&previous);
        s.actions.clear();
        s.pressed.clear();
        s.paste_fields.clear();
    });
}
pub(super) fn sync(model: model::ViewModel) {
    state(|s| {
        s.menu.reconcile(&s.model, &model, s.height.max(200.0));
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
    if !owned(game) || crate::game::claim_borders_waypoint_view::quick() {
        return;
    }
    let render_start = batch.vertices.len();
    let mut request_tip = None;
    ui::fill(game, batch, screen.rect, 0x9c080b12);
    let pw = (screen.rect[2] - 16.0).clamp(220.0, 720.0);
    let ph = (screen.rect[3] - 16.0).clamp(200.0, 400.0);
    let px = (screen.rect[2] - pw) * 0.5;
    let py = (screen.rect[3] - ph) * 0.5;
    // The window: a soft shadow, a thin border, rounded corners; the tab rail is a plate inset
    // in the window and the content area is the rest.
    ui::rounded(
        game,
        batch,
        [px - 3.0, py - 3.0, pw + 6.0, ph + 6.0],
        10.0,
        0x38000000,
    );
    ui::rounded(
        game,
        batch,
        [px, py, pw, ph],
        8.0,
        0xff000000 | crate::game::claim_borders_ui_style::BORDER,
    );
    ui::rounded(
        game,
        batch,
        [px + 1.0, py + 1.0, pw - 2.0, ph - 2.0],
        7.0,
        0xf0000000 | crate::game::claim_borders_ui_style::BACKGROUND,
    );
    if model::rail_width(pw, ph) > 0.0 {
        let rail = model::RAIL_WIDTH;
        ui::rounded(
            game,
            batch,
            [px + 4.0, py + 4.0, rail - 4.0, ph - 8.0],
            6.0,
            0xff000000 | crate::game::claim_borders_ui_style::SURFACE,
        );
        ui::text(
            game,
            batch,
            "DraimHorizon",
            [px + 10.0, py + 17.0],
            crate::game::claim_borders_ui_style::TEXT,
            false,
        );
        let caption = state(|s| s.menu.language.text("Инструменты клиента", "Client tools"));
        ui::text(
            game,
            batch,
            caption,
            [px + 10.0, py + 30.0],
            crate::game::claim_borders_ui_style::MUTED,
            false,
        );
    }
    let variant = state(|s| {
        format!(
            "{:?}:{:?}:{:?}:{}",
            s.menu.tab, s.menu.chat_section, s.menu.claims_section, s.menu.relay_setup
        )
    });
    state(|s| {
        s.height = (screen.rect[3] - 16.0).clamp(200.0, 400.0);
        s.menu.clamp_page(&s.model, s.height);
        if s.menu.experiments_prompt {
            ui::fill(game, batch, screen.rect, 0xcc080b12);
            let w = (screen.rect[2] - 32.0).clamp(200.0, 460.0);
            let r = [
                (screen.rect[2] - w) * 0.5,
                (screen.rect[3] - 184.0) * 0.5,
                w,
                184.0,
            ];
            ui::rounded(game, batch, r, 7.0, 0xffd58c98);
            ui::rounded(
                game,
                batch,
                [r[0] + 1.5, r[1] + 1.5, w - 3.0, 181.0],
                6.0,
                0xff181c28,
            );
            ui::icon(
                game,
                batch,
                ui::Icon::Warning,
                [r[0] + (w - 22.0) * 0.5, r[1] + 12.0, 22.0, 22.0],
                0xe7a1aa,
            );
        }
        let widgets = s.menu.layout(&s.model, screen.rect[2], screen.rect[3]);
        for widget in widgets {
            if !s.menu.experiments_prompt
                && s.menu.tab == model::Tab::Waypoints
                && widget.text != "DraimHorizon"
                && !matches!(
                    widget.control,
                    Some(
                        model::Control::Tab(_)
                            | model::Control::NavigationMenu
                            | model::Control::Language
                            | model::Control::Economy
                            | model::Control::Close
                    )
                )
            {
                continue;
            }

            let [mut x, y, mut w, h] = widget.rect;
            let hover = screen.mouse[0] >= x
                && screen.mouse[0] < x + w
                && screen.mouse[1] >= y
                && screen.mouse[1] < y + h;
            if hover && widget.control == Some(model::Control::SchematicMaterials) {
                request_tip = Some(
                    s.menu
                        .language
                        .text(
                            "Копирует в буфер обмена список блоков, которые нужны для схемы",
                            "Copies the list of blocks the schematic needs to the clipboard",
                        )
                        .to_owned(),
                );
            }
            if hover && let Some(model::Control::PlayerRequest(kind)) = widget.control {
                request_tip = Some(match s.menu.selected.and_then(|i| s.model.party.get(i)) {
                    None => s
                        .menu
                        .language
                        .text(
                            "Выберите игрока, затем отправьте запрос",
                            "Select a player, then send the request",
                        )
                        .into(),
                    Some(row) if !row.server_online => match s.menu.language {
                        Language::Russian => format!("{} не в сети на этом сервере", row.name),
                        Language::English => format!("{} is offline on this server", row.name),
                    },
                    Some(row) => match kind.command(&row.name, s.menu.language) {
                        Ok(command) => format!(
                            "{command} — {}",
                            match kind {
                                model::PlayerRequest::Teleport => s.menu.language.text(
                                    "запросить телепортацию к игроку",
                                    "request teleport to the player"
                                ),
                                model::PlayerRequest::Trade => s
                                    .menu
                                    .language
                                    .text("предложить обмен игроку", "offer a trade to the player"),
                            }
                        ),
                        Err(error) => error,
                    },
                });
            }
            // The title and, under it, a line about the tab, on the same left edge as the content.
            if widget.text == "DraimHorizon" && model::rail_width(pw, ph) > 0.0 {
                let (ru, en) = match s.menu.tab {
                    model::Tab::Waypoints => (
                        "Сохранённые места и быстрые ориентиры",
                        "Saved places and quick waypoints",
                    ),
                    model::Tab::Party => (
                        "Друзья, приглашения и игроки мода",
                        "Friends, invitations and mod users",
                    ),
                    model::Tab::Claims => (
                        "Список приватов, карта, 3D-границы и память",
                        "Claims list, map, 3D borders and storage",
                    ),
                    model::Tab::Chat => (
                        "Вид, уведомления, антиспам и перевод",
                        "Style, notifications, anti-spam and translation",
                    ),
                    model::Tab::Experimental => (
                        "Автоматизация, помощники и расширенные инструменты",
                        "Automation, helpers and advanced tools",
                    ),
                    model::Tab::Settings => (
                        "Оформление, связь, список игроков и боковая панель",
                        "Appearance, network, player list and sidebar",
                    ),
                    model::Tab::Trading => (
                        "Торговые инструменты и настройки по разделам",
                        "Trading tools and preferences by category",
                    ),
                };
                ui::text(
                    game,
                    batch,
                    &ui::clip(game, s.menu.language.text(ru, en), widget.rect[2]),
                    [widget.rect[0] + 4.0, widget.rect[1] + widget.rect[3] + 1.0],
                    0x9aacc0,
                    false,
                );
            }
            if s.menu.experiments_prompt {
                if let Some(control) = widget.control {
                    let confirm = control == model::Control::ConfirmExperiments;
                    let color = if confirm { 0xa2d0b7 } else { 0xdfa8b5 };
                    ui::rounded(game, batch, widget.rect, 4.0, 0x35000000 | color);
                    let tw = game.native_text_width(&widget.text);
                    let left = x + (w - tw - 17.0) * 0.5;
                    ui::icon(
                        game,
                        batch,
                        if confirm {
                            ui::Icon::Check
                        } else {
                            ui::Icon::Close
                        },
                        [left, y + (h - 11.0) * 0.5, 11.0, 11.0],
                        color,
                    );
                    ui::text(
                        game,
                        batch,
                        &widget.text,
                        [left + 17.0, y + (h - 8.0) * 0.5],
                        color,
                        false,
                    );
                } else {
                    ui::text(
                        game,
                        batch,
                        &widget.text,
                        [
                            x + (w - game.native_text_width(&widget.text)) * 0.5,
                            y + (h - 8.0) * 0.5,
                        ],
                        0xe3ebf3,
                        false,
                    );
                }
                continue;
            }
            if widget.row {
                // A stepper's labelled row: the readout and its buttons are drawn over it.
                ui::rounded(
                    game,
                    batch,
                    widget.rect,
                    4.0,
                    0xff000000 | crate::game::claim_borders_ui_style::BORDER,
                );
                ui::rounded(
                    game,
                    batch,
                    [x + 0.75, y + 0.75, w - 1.5, h - 1.5],
                    3.25,
                    0xff000000
                        | crate::game::claim_borders_ui_style::mix(
                            crate::game::claim_borders_ui_style::SURFACE,
                            crate::game::claim_borders_ui_style::RAISED,
                            0.32,
                        ),
                );
                let name = clipped(game, &widget.text, (w * 0.5).max(0.0));
                ui::text(
                    game,
                    batch,
                    &name,
                    [x + 6.0, y + (h - 8.0) * 0.5],
                    crate::game::claim_borders_ui_style::TEXT,
                    false,
                );
                continue;
            }
            if widget.readout {
                ui::rounded(
                    game,
                    batch,
                    widget.rect,
                    3.0,
                    0xff000000 | crate::game::claim_borders_ui_style::SURFACE,
                );
                let text = ui::clip(game, &widget.text, w - 8.0);
                ui::text(
                    game,
                    batch,
                    &text,
                    [
                        x + (w - game.native_text_width(&text)) * 0.5,
                        y + (h - 8.0) * 0.5,
                    ],
                    crate::game::claim_borders_ui_style::TEXT,
                    false,
                );
                if hover && text != widget.text {
                    request_tip = Some(widget.text.clone());
                }
                continue;
            }
            let field = matches!(widget.control, Some(model::Control::Field(_)));
            // A switch shows its own state, so its row is not drawn as a selected button.
            let switch = widget.control.is_some()
                && !field
                && w >= 120.0
                && crate::game::claim_borders_ui_style::switch_label(&widget.text).is_some();
            if widget.control.is_some() {
                ui::button_base(
                    game,
                    batch,
                    widget.rect,
                    &widget.text,
                    widget.selected && !switch,
                    screen.mouse,
                );
            }
            if field {
                // Text inputs are inset, so they read as editable rather than as buttons.
                ui::rounded(
                    game,
                    batch,
                    [x + 2.0, y + 2.0, w - 4.0, h - 4.0],
                    2.5,
                    0xc0000000 | crate::game::claim_borders_ui_style::BACKGROUND,
                );
            }
            // Arrow captions are paging controls; draw them as Lucide chevrons.
            if widget.control.is_some()
                && let Some(icon) = match widget.text.as_str() {
                    "<" | "←" | "‹" => Some(ui::Icon::Back),
                    ">" | "→" | "›" => Some(ui::Icon::Next),
                    _ => None,
                }
            {
                ui::icon(
                    game,
                    batch,
                    icon,
                    [x + (w - 10.0) * 0.5, y + (h - 10.0) * 0.5, 10.0, 10.0],
                    crate::game::claim_borders_ui_style::TEXT,
                );
                continue;
            }

            if let Some(model::Control::BaritoneMode(mode)) = widget.control
                && w < 56.0
            {
                let icon = match mode {
                    model::BaritoneMode::Navigation => ui::Icon::Map,
                    model::BaritoneMode::Mining => ui::Icon::Search,
                    model::BaritoneMode::Tunnel => ui::Icon::Layers,
                    model::BaritoneMode::Bridge => ui::Icon::Fill,
                    model::BaritoneMode::Farm => ui::Icon::Restock,
                    model::BaritoneMode::Drain => ui::Icon::Grid,
                    model::BaritoneMode::Schematic => ui::Icon::Home,
                };
                ui::icon(
                    game,
                    batch,
                    icon,
                    [x + (w - 12.0) * 0.5, y + (h - 12.0) * 0.5, 12.0, 12.0],
                    ui::preferences(game).accent.color(),
                );
                if hover {
                    ui::tooltip(game, batch, &widget.text, screen.mouse);
                }
                continue;
            }
            if widget.control == Some(model::Control::Language)
                || matches!(
                    widget.control,
                    Some(
                        model::Control::SettingsTab(_)
                            | model::Control::TradingGroup(_)
                            | model::Control::ExperimentalTab(_)
                            | model::Control::BaritoneMode(_)
                            | model::Control::ClaimsSection(_)
                            | model::Control::ChatSection(_)
                            | model::Control::PartyFilter(_)
                    )
                )
            {
                let text = ui::clip(game, &widget.text, w - 6.0);
                if hover && text != widget.text {
                    request_tip = Some(widget.text.clone());
                }
                ui::text(
                    game,
                    batch,
                    &text,
                    [
                        x + (w - game.native_text_width(&text)) * 0.5,
                        y + (h - 8.0) * 0.5,
                    ],
                    0xe3ebf3,
                    false,
                );
                continue;
            }

            if widget.control == Some(model::Control::Close) {
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Close,
                    [x + (w - 10.0) * 0.5, y + (h - 10.0) * 0.5, 10.0, 10.0],
                    0xdfa8b5,
                );

                continue;
            }
            if matches!(
                widget.control,
                Some(model::Control::PartyAccept(_) | model::Control::PartyDecline(_))
            ) {
                ui::icon(
                    game,
                    batch,
                    if matches!(widget.control, Some(model::Control::PartyAccept(_))) {
                        ui::Icon::Check
                    } else {
                        ui::Icon::Close
                    },
                    [x + 5.0, y + 5.0, 12.0, 12.0],
                    if matches!(widget.control, Some(model::Control::PartyAccept(_))) {
                        0x9ce3ba
                    } else {
                        0xe39ca8
                    },
                );
                if hover {
                    ui::tooltip(game, batch, &widget.text, screen.mouse);
                }
                continue;
            }
            if widget.control.is_some() && !matches!(widget.control, Some(model::Control::Field(_)))
            {
                let caption = match widget.text.as_str() {
                    "Пати" => "Друзья",
                    "Клеймы" => "Приваты",
                    s => s,
                };
                if let Some(icon) = crate::game::claim_borders_ui_style::icon_for(&widget.text)
                    && (w >= game.native_text_width(caption) + 22.0
                        || matches!(
                            widget.control,
                            Some(model::Control::Tab(model::Tab::Experimental))
                        ))
                {
                    ui::icon(
                        game,
                        batch,
                        icon,
                        [x + 6.0, y + (h - 10.0) * 0.5, 10.0, 10.0],
                        ui::preferences(game).accent.color(),
                    );
                    x += 17.0;
                    w -= 17.0;
                }
            }
            if s.menu.tab == model::Tab::Party
                && let Some(model::Control::Row(index)) = widget.control
                && let Some(row) = s.model.party.get(index)
            {
                let skin = if row.skin.is_empty() {
                    horizon_client::tab_list::default_skin(row.uuid)
                } else {
                    row.skin.as_str()
                };
                let mut draws = Vec::new();
                horizon_client::tab_list::face(
                    &mut draws,
                    skin,
                    (x + 4.0) as i32,
                    (y + 5.0) as i32,
                    12,
                    true,
                    false,
                    u32::MAX,
                );
                horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
                x += 18.0;
                w -= 18.0;
                if let Some(rule) = s.model.priority.rule(&row.name, row.friend) {
                    ui::icon(
                        game,
                        batch,
                        ui::Icon::Star,
                        [x + 2.0, y + 6.0, 10.0, 10.0],
                        rule.color,
                    );
                    x += 14.0;
                    w -= 14.0;
                }
            }
            if hover
                && matches!(
                    widget.control,
                    Some(model::Control::NavigationWaypoint | model::Control::NavigationPlayer)
                )
            {
                request_tip = Some(match widget.control {
                    Some(model::Control::NavigationWaypoint) => {
                        let mut choices = s
                            .model
                            .navigation_waypoints
                            .iter()
                            .take(8)
                            .map(|waypoint| waypoint.name.as_str())
                            .collect::<Vec<_>>()
                            .join(" • ");
                        if s.model.navigation_waypoints.len() > 8 {
                            choices.push_str(" …");
                        }
                        if choices.is_empty() {
                            widget.text.clone()
                        } else {
                            format!("{}\n{}", widget.text, choices)
                        }
                    }
                    Some(model::Control::NavigationPlayer) => {
                        let mut choices = s
                            .model
                            .navigation_players
                            .iter()
                            .take(8)
                            .map(|(_, name)| name.as_str())
                            .collect::<Vec<_>>()
                            .join(" • ");
                        if s.model.navigation_players.len() > 8 {
                            choices.push_str(" …");
                        }
                        if choices.is_empty() {
                            widget.text.clone()
                        } else {
                            format!("{}\n{}", widget.text, choices)
                        }
                    }
                    _ => widget.text.clone(),
                });
            }
            // Mining targets are item icons; the name is shown as a tooltip.
            if let Some(
                control @ (model::Control::MiningTarget(_)
                | model::Control::FarmCropPick(_)
                | model::Control::AutoEatFood(_)
                | model::Control::ExperimentalOreToggle(_)),
            ) = widget.control
            {
                if hover {
                    request_tip = Some(widget.text.clone());
                }
                let icon = match control {
                    model::Control::MiningTarget(index)
                    | model::Control::ExperimentalOreToggle(index) => {
                        crate::game::claim_borders_mining::icon(index)
                    }
                    model::Control::FarmCropPick(crop) => Some(crop.icon()),
                    model::Control::AutoEatFood(index) => s
                        .model
                        .auto_eat_options
                        .get(index)
                        .map(|(id, _, _)| id.as_str()),
                    _ => None,
                };
                if let Some(item) = icon
                    && game.res.items().get(item).is_some()
                {
                    game.draw_item(
                        batch,
                        &ItemStack::new(item, 1),
                        x + (w - 16.0) * 0.5,
                        y + (h - 16.0) * 0.5,
                    );
                    continue;
                }
            }
            // "Name: on/off" rows: the name on the left and a switch on the right.
            if widget.control.is_some()
                && !matches!(widget.control, Some(model::Control::Field(_)))
                && w >= 120.0
                && let Some((name, on)) =
                    crate::game::claim_borders_ui_style::switch_label(&widget.text)
            {
                let label_width = ui::toggle_end(game, batch, [x, y, w, h], on);
                let name = clipped(game, name, label_width);
                if hover {
                    request_tip = Some(widget.text.clone());
                }
                ui::text(
                    game,
                    batch,
                    &name,
                    [x + 6.0, y + (h - 8.0) * 0.5],
                    if widget.muted { 0x8995a5 } else { 0xe6edf5 },
                    false,
                );
                continue;
            }
            // "Name: value" choices: the name on the left, the value and a chevron on the right.
            if widget.control.is_some()
                && !field
                && w >= 150.0
                && !matches!(
                    widget.control,
                    Some(
                        model::Control::Row(_)
                            | model::Control::ClaimRow(_)
                            | model::Control::Tab(_)
                            | model::Control::NavigationMenu
                    )
                )
                && !widget.text.contains('\n')
                && let Some((name, value)) = widget.text.split_once(": ")
                && !value.trim().is_empty()
            {
                let value = value.trim_end_matches(['→', ' ']);
                let value_w = game.native_text_width(value).min(w * 0.5);
                let value = clipped(game, value, value_w);
                let name = clipped(game, name, (w - value_w - 34.0).max(0.0));
                if hover && (name.ends_with('…') || !widget.text.contains(value.as_str())) {
                    request_tip = Some(widget.text.clone());
                }
                ui::text(
                    game,
                    batch,
                    &name,
                    [x + 6.0, y + (h - 8.0) * 0.5],
                    if widget.muted { 0x8995a5 } else { 0xe6edf5 },
                    false,
                );
                ui::text(
                    game,
                    batch,
                    &value,
                    [
                        x + w - 18.0 - game.native_text_width(&value),
                        y + (h - 8.0) * 0.5,
                    ],
                    ui::preferences(game).accent.color(),
                    false,
                );
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Next,
                    [x + w - 14.0, y + (h - 8.0) * 0.5, 8.0, 8.0],
                    crate::game::claim_borders_ui_style::MUTED,
                );
                continue;
            }
            // An unfocused text input: its label muted, the entered value in full colour.
            if field
                && !widget.selected
                && let Some((name, value)) = widget.text.split_once(": ")
            {
                let name = format!("{name}: ");
                let name_w = game.native_text_width(&name);
                ui::text(
                    game,
                    batch,
                    &name,
                    [x + 6.0, y + (h - 8.0) * 0.5],
                    crate::game::claim_borders_ui_style::MUTED,
                    false,
                );
                ui::text(
                    game,
                    batch,
                    &clipped(game, value, (w - name_w - 12.0).max(0.0)),
                    [x + 6.0 + name_w, y + (h - 8.0) * 0.5],
                    0xe6edf5,
                    false,
                );
                continue;
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
                    let secret = matches!(
                        s.menu.focus,
                        Some(model::Field::RelayToken | model::Field::DeepLKey)
                    );
                    let shown = if secret {
                        "*".repeat(prefix_len)
                    } else {
                        s.menu.text()[..cursor].to_owned()
                    };
                    let (ru, en) = match s.menu.focus {
                        Some(model::Field::SchematicOrigin) => ("XYZ: ", "XYZ: "),
                        Some(model::Field::NavigationX) => ("X: ", "X: "),
                        Some(model::Field::NavigationY) => ("Y: ", "Y: "),
                        Some(model::Field::NavigationZ) => ("Z: ", "Z: "),
                        Some(model::Field::ClaimOwner) => ("Игрок: ", "Player: "),
                        Some(model::Field::Name) => ("Имя: ", "Name: "),
                        Some(model::Field::Color | model::Field::PriorityColor) => {
                            ("Цвет: ", "Color: ")
                        }
                        Some(model::Field::OwnClaimColor) => ("Свои: ", "Own: "),
                        Some(model::Field::FriendClaimColor) => ("Друзья: ", "Friends: "),
                        Some(model::Field::PeerCode) => ("Код друга: ", "Friend code: "),
                        Some(model::Field::PriorityName) => ("Ник: ", "Name: "),
                        Some(model::Field::DeepLKey) => ("Ключ: ", "Key: "),
                        _ => ("", ""),
                    };
                    let label = s.menu.language.text(ru, en);
                    let rest = if secret {
                        "*".repeat(s.menu.text()[cursor..].chars().count())
                    } else {
                        s.menu.text()[cursor..].to_owned()
                    };
                    format!("{label}{shown}│{rest}")
                } else {
                    match widget.text.as_str() {
                        "DraimHorizon" => {
                            let (ru, en) = match s.menu.tab {
                                model::Tab::Waypoints => ("Мои метки", "My waypoints"),
                                model::Tab::Party => ("Друзья и игроки", "Friends and players"),
                                model::Tab::Claims => ("Карта и приваты", "Map and claims"),
                                model::Tab::Chat => ("Чат", "Chat"),
                                model::Tab::Experimental => ("Эксперименты", "Experiments"),
                                model::Tab::Settings => ("Настройки", "Settings"),
                                model::Tab::Trading => ("Торговля", "Trading"),
                            };
                            s.menu.language.text(ru, en).into()
                        }
                        "Пати" => "Друзья".into(),
                        "Клеймы" => "Приваты".into(),
                        _ => widget.text.clone(),
                    }
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
            if hover
                && text != display_text
                && !matches!(widget.control, Some(model::Control::Field(_)))
                && request_tip.is_none()
            {
                request_tip = Some(display_text.clone());
            }
            if widget.selected && matches!(widget.control, Some(model::Control::Field(_))) {
                let selected = s.menu.selection();
                if !selected.is_empty()
                    && let Some(caret) = text.find('│')
                {
                    let cursor = s.menu.cursor();
                    let caret_x = x + 4.0 + game.native_text_width(&text[..caret]);
                    let value = s.menu.text();
                    let measure = |part: &str| {
                        if matches!(
                            s.menu.focus,
                            Some(model::Field::RelayToken | model::Field::DeepLKey)
                        ) {
                            game.native_text_width(&"*".repeat(part.chars().count()))
                        } else {
                            game.native_text_width(part)
                        }
                    };
                    let left = (caret_x - measure(&value[selected.start..cursor])).max(x + 4.0);
                    let right = (caret_x + measure(&value[cursor..selected.end])).min(x + w - 4.0);
                    if right > left {
                        ui::fill(
                            game,
                            batch,
                            [left, y + 3.0, right - left, h - 6.0],
                            0x995f50b8,
                        );
                    }
                }
            }
            ui::text(
                game,
                batch,
                &text,
                [x + 4.0, y + (h - 8.0) * 0.5],
                color,
                true,
            );
        }
    });
    if !state(|s| s.menu.experiments_prompt) {
        crate::game::claim_borders_waypoint_view::render_list(game, batch, screen);
    }
    if let Some(tip) = request_tip {
        ui::tooltip(game, batch, &tip, screen.mouse);
    }
    ui::finish(game, batch, render_start, "main-menu", &variant);
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
        // While a screen is open the game does not pass keys to the mod, so the action keys
        // are also handled here, for the tab on screen, unless a text field is being edited.
        if modifiers == 0 && s.menu.baritone_keys_active() {
            let bindings = crate::game::claim_borders_navigation::Hotkey::ALL
                .map(|action| game.native_mod_key(claims::ID, action.id(), action.preset()));
            if let Some(action) = crate::game::claim_borders_navigation::hotkey(
                key,
                true,
                true,
                false,
                bindings.each_ref().map(String::as_str),
            ) {
                *canceled = true;
                enqueue(s, model::Action::BaritoneKey(action));
                return;
            }
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

pub(super) fn waypoints_tab() -> bool {
    state(|s| s.menu.tab == model::Tab::Waypoints)
}

pub(super) fn open_settings() {
    state(|s| {
        s.menu.tab = model::Tab::Settings;
        s.menu.settings_section = 0;
        s.menu.focus = None;
    });
}
