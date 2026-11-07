use crate::game::claim_borders_ui_view as ui;
// Screen controls and verified normal clicks. No direct slot writes, quick-moves or hotbar swaps.
use crate::game::{
    Game, Screen, claim_borders as claims,
    claim_borders_inventory_preferences::{self as prefs, Policy, Preferences},
    claim_borders_inventory_sort as model,
    native_hooks::NativeScreenView,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::{
    hud::HudBatch,
    items::ItemStack,
    menu::{Input, Kind},
};
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
fn t(russian: &'static str, english: &'static str) -> &'static str {
    language().text(russian, english)
}
#[derive(Clone)]
struct Area {
    key: String,
    label: &'static str,
    slots: Vec<usize>,
    columns: usize,
}
#[derive(Clone)]
struct Layout {
    context: String,
    virtual_key: String,
    storage: Option<Area>,
    player: Area,
    trusted: bool,
}
#[derive(Clone, Copy)]
enum Button {
    Run(bool, model::Operation),
    Edit,
    Area,
    Visibility,
    Enabled,
    Mode,
    Columns,
    Offset(i16, i16),
    Clear,
    Default,
    SaveDefault,
    Undo,
    Cancel,
    Done,
    Frames,
    Signs,
    Virtual,
    Collapse,
}
struct Hit {
    rect: [f32; 4],
    button: Button,
    tip: String,
}
struct Run {
    context: String,
    area: Area,
    other: Option<Area>,
    operation: model::Operation,
    policy: Policy,
    other_policy: Policy,
    expected: Option<Vec<Option<model::Stack>>>,
    types: Option<BTreeSet<String>>,
    last: Instant,
    started: Instant,
    steps: usize,
}
struct Editor {
    before: Preferences,
    storage: bool,
    collapsed: bool,
}
#[derive(Default)]
struct State {
    loaded: bool,
    writable: bool,
    preferences: Preferences,
    error: Option<String>,
    run: Option<Run>,
    request: Option<(Layout, bool, model::Operation)>,
    editor: Option<Editor>,
    hits: Vec<Hit>,
    layout: Option<Layout>,
    pressed: BTreeSet<i32>,
    trusted: Option<(i32, String)>,
    entity_use: Option<(String, Instant)>,
    block_use: Option<([i32; 3], Instant)>,
    generation: u64,
    status: Option<(String, Instant)>,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()))
}
fn load_settings(game: &Game<'_>) {
    state(|s| {
        if s.loaded {
            return;
        }
        match std::fs::read_to_string(game.native_data_dir(claims::ID).join("inventory_tools")) {
            Ok(raw) => match Preferences::decode(&raw) {
                Ok(p) => {
                    s.preferences = p;
                    s.writable = true;
                }
                Err(e) => {
                    s.error = Some(if language() == Language::Russian {
                        format!("Настройки повреждены: {e}")
                    } else {
                        format!("Settings are corrupted: {e}")
                    })
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => s.writable = true,
            Err(e) => {
                s.error = Some(if language() == Language::Russian {
                    format!("Не удалось прочитать настройки: {e}")
                } else {
                    format!("Could not read settings: {e}")
                })
            }
        }
        s.loaded = true;
    });
}
pub(super) fn interactions(game: &Game<'_>) -> [bool; 2] {
    load_settings(game);
    state(|s| [s.preferences.frames, s.preferences.signs])
}
/// The remote OpenScreen hook supplies physical provenance before use_target is consumed.
pub(crate) fn entity_use(menu: &str) {
    state(|s| s.entity_use = Some((menu.into(), Instant::now())));
}
pub(crate) fn block_use(pos: [i32; 3]) {
    state(|s| s.block_use = Some((pos, Instant::now())));
}
pub(crate) fn opened(id: i32, menu: &str, physical: Option<[i32; 3]>) {
    state(|s| {
        s.generation = s.generation.wrapping_add(1);
        let physical = s.block_use.take().is_some_and(|(pos, at)| {
            Some(pos) == physical && at.elapsed() < Duration::from_secs(2)
        });
        let entity = s
            .entity_use
            .take()
            .is_some_and(|(m, at)| m == menu && at.elapsed() < Duration::from_secs(2));
        s.trusted = (physical || entity).then(|| (id, menu.to_owned()));
        if let Some(e) = s.editor.take() {
            s.preferences = e.before;
        }
        s.run = None;
        s.request = None;
        s.hits.clear();
        s.layout = None;
    });
}
pub(super) fn reset() {
    state(|s| {
        // Unsaved editor changes do not survive closing or changing a screen.
        if let Some(e) = s.editor.take() {
            s.preferences = e.before;
        }
        s.run = None;
        s.request = None;
        s.hits.clear();
        s.layout = None;
        s.pressed.clear();
        s.trusted = None;
        s.entity_use = None;
        s.block_use = None;
    });
}
fn stop(s: &mut State) {
    s.run = None;
    s.request = None;
}
fn context(game: &Game<'_>, generation: u64) -> String {
    format!(
        "{:?}|{:?}|{}|{}",
        game.remote_menu(),
        game.container,
        game.craft_size,
        generation
    )
}
fn layout(game: &Game<'_>, screen: &NativeScreenView<'_>) -> Option<Layout> {
    if !game.native_on(claims::ID)
        || screen.id != "inventory"
        || game.native_mod_screen().is_some()
        || game.screen != Screen::Inventory
        || screen
            .title
            .is_some_and(|t| prefs::economic_title(&t.plain()))
    {
        return None;
    }
    let menu = game.remote_menu();
    let title = screen.title.map(|t| t.plain()).unwrap_or_default();
    let server = game
        .remote
        .as_ref()
        .map(|r| r.session.address.to_ascii_lowercase())
        .unwrap_or_default();
    let virtual_key = format!("{}|{}|{}", server, menu.1, title);
    if virtual_key.len() > 480 {
        return None;
    }
    let available = game.slots();
    if !(9..36).all(|i| available.contains(&i)) {
        return None;
    }
    let (generation, trusted, opted) = state(|s| {
        (
            s.generation,
            game.remote.is_none() || s.trusted.as_ref() == Some(&menu),
            s.preferences.virtual_menus.contains(&virtual_key),
        )
    });
    let player = Area {
        key: "player".into(),
        label: t("Инвентарь", "Inventory"),
        slots: (9..36).collect(),
        columns: 9,
    };
    let kind = screen.menu.unwrap_or("local");
    let storage_slots: Vec<_> = available
        .iter()
        .copied()
        .filter(|i| (300..354).contains(i) || (710..725).contains(i))
        .collect();
    let plain = if game.remote.is_some() {
        kind.starts_with("minecraft:generic_9x")
            || matches!(
                kind,
                "minecraft:shulker_box" | "minecraft:generic_3x3" | "minecraft:hopper"
            )
            || kind.starts_with("horizon:mount")
    } else {
        matches!(
            game.container_kind(),
            Some(horizon_client::containers::BlockEntity::Chest(_))
        ) || game.mount_active()
    };
    let trusted = trusted || kind.starts_with("horizon:mount");
    let storage = (plain && !storage_slots.is_empty() && storage_slots.len() <= 54).then(|| Area {
        key: format!(
            "storage/{}:{}",
            if trusted { kind } else { &virtual_key },
            storage_slots.len()
        ),
        label: if trusted || opted {
            t("Хранилище", "Storage")
        } else {
            t("Меню сервера", "Server menu")
        },
        slots: storage_slots.clone(),
        columns: if kind == "minecraft:hopper" || game.remote.is_none() && storage_slots.len() == 5
        {
            5
        } else if kind == "minecraft:generic_3x3"
            || game.remote.is_none() && storage_slots.len() == 9
        {
            3
        } else if kind.starts_with("horizon:mount") {
            ((available.len().saturating_sub(38)) / 3).max(1)
        } else {
            9
        },
    });
    Some(Layout {
        context: context(game, generation),
        virtual_key,
        storage,
        player,
        trusted: trusted || opted,
    })
}
fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
fn slot_rect(game: &Game<'_>, screen: &NativeScreenView<'_>, slot: usize) -> [f32; 4] {
    let (x, y) = game.slot_position(slot);
    [screen.rect[0] + x, screen.rect[1] + y, 16.0, 16.0]
}
fn selected(l: &Layout, storage: bool) -> &Area {
    if storage {
        l.storage.as_ref().unwrap_or(&l.player)
    } else {
        &l.player
    }
}
fn order(area: &Area, p: &Policy) -> Vec<usize> {
    let indices: Vec<_> = (0..area.slots.len())
        .filter(|i| !p.ignored.contains(i))
        .collect();
    model::traversal(&indices, area.columns, p.columns)
        .iter()
        .map(|i| area.slots[*i])
        .collect()
}
fn clipped(game: &Game<'_>, text: &str, width: f32) -> String {
    if game.native_text_width(text) <= width {
        return text.into();
    }
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if game.native_text_width(&out) + game.native_text_width("…") > width {
            out.pop();
            break;
        }
    }
    out.push('…');
    out
}
fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    s: &mut State,
    rect: [f32; 4],
    text: &str,
    action: Button,
    tip: String,
    active: bool,
) {
    ui::button(game, batch, rect, text, active, screen.mouse);
    s.hits.push(Hit {
        rect,
        button: action,
        tip,
    });
}
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let render_start = batch.vertices.len();
    load_settings(game);
    let Some(l) = layout(game, screen) else {
        state(|s| {
            s.hits.clear();
            s.layout = None;
        });
        return;
    };
    state(|s| {
        s.hits.clear();
        if s.layout
            .as_ref()
            .is_some_and(|old| old.context != l.context)
        {
            stop(s);
            if let Some(e) = s.editor.take() {
                s.preferences = e.before;
            }
        }
        s.layout = Some(l.clone());
        let editing = s.editor.is_some();
        for storage in [true, false] {
            if storage && l.storage.is_none() {
                continue;
            }
            let area = selected(&l, storage);
            let p = s.preferences.policy(&area.key);
            if p.visible || editing {
                let first = slot_rect(game, screen, area.slots[0]);
                // Use the panel edge: hopper/dispenser grids are narrower than the player inventory.
                let x = screen.rect[0] + screen.rect[2] + 3.0 + p.offset[0] as f32;
                let x = x.clamp(0.0, (batch.width - 16.0).max(0.0));
                let base_y =
                    (first[1] + p.offset[1] as f32).clamp(0.0, (batch.height - 64.0).max(0.0));
                for (i, op) in [
                    model::Operation::Sort,
                    model::Operation::Fill,
                    model::Operation::Restock,
                    model::Operation::Transfer,
                ]
                .iter()
                .copied()
                .enumerate()
                {
                    if editing {
                        button(
                            game,
                            batch,
                            screen,
                            s,
                            [x, base_y + i as f32 * 16.0, 15.0, 15.0],
                            op.symbol(),
                            Button::Area,
                            t(
                                "Предпросмотр кнопок. Tab скрывает панель редактора.",
                                "Button preview. Tab hides the editor panel.",
                            )
                            .into(),
                            false,
                        );
                    } else if p.enabled
                        && (l.storage.is_none() || l.trusted)
                        && (op == model::Operation::Sort || l.storage.is_some() && l.trusted)
                    {
                        let tip = if op == model::Operation::Sort {
                            format!(
                                "{}: {} ({})",
                                area.label,
                                op.label_in(language()),
                                p.mode.label_in(language())
                            )
                        } else {
                            if language() == Language::Russian {
                                format!(
                                    "В {}: {}. Хотбар исключён.",
                                    area.label,
                                    op.label_in(language())
                                )
                            } else {
                                format!(
                                    "In {}: {}. Hotbar excluded.",
                                    area.label,
                                    op.label_in(language())
                                )
                            }
                        };
                        let active = s
                            .run
                            .as_ref()
                            .is_some_and(|r| r.area.key == area.key && r.operation == op);
                        button(
                            game,
                            batch,
                            screen,
                            s,
                            [x, base_y + i as f32 * 16.0, 15.0, 15.0],
                            op.symbol(),
                            Button::Run(storage, op),
                            tip,
                            active,
                        );
                    }
                }
            }
            if editing {
                for (i, slot) in area.slots.iter().enumerate() {
                    if p.ignored.contains(&i) {
                        let r = slot_rect(game, screen, *slot);
                        ui::fill(game, batch, r, 0x88551a25);
                        ui::text(game, batch, "×", [r[0] + 5.0, r[1] + 4.0], 0xff7979, true);
                    }
                }
            }
        }
        if !editing {
            button(
                game,
                batch,
                screen,
                s,
                [
                    screen.rect[0] + screen.rect[2] - 15.0,
                    (screen.rect[1] - 13.0).max(2.0),
                    15.0,
                    12.0,
                ],
                "E",
                Button::Edit,
                t("Редактор сортировки [Ctrl+O]: слоты, кнопки, порядок, нажатие сквозь рамки/таблички.", "Sort editor [Ctrl+O]: slots, buttons, order, click-through frames/signs.")
                    .into(),
                false,
            );
            if let Some((text, at)) = &s.status
                && at.elapsed() < Duration::from_secs(5)
            {
                ui::text(
                    game,
                    batch,
                    &clipped(game, text, batch.width - 12.0),
                    [
                        6.0,
                        (screen.rect[1] + screen.rect[3] + 4.0).min(batch.height - 10.0),
                    ],
                    0xc6e5d4,
                    true,
                );
            }
        } else {
            let collapsed = s.editor.as_ref().unwrap().collapsed;
            let pw = 146.0f32.min(batch.width - 8.0);
            let px = if screen.rect[0] >= pw + 8.0 {
                screen.rect[0] - pw - 6.0
            } else {
                4.0
            };
            let py = 4.0;
            button(game,batch,screen,s,[px,py,pw,15.0],if collapsed {t("Настройки [Tab]", "Settings [Tab]")} else {t("Редактор [Tab]", "Editor [Tab]")},Button::Collapse,
                t("Tab: свернуть панель, чтобы выбрать любой слот. ЛКМ по слоту: исключить / вернуть.", "Tab: collapse the panel to pick any slot. LMB on a slot: exclude / restore.").into(),false);
            if !collapsed {
                let max_rows = ((batch.height - 24.0) / 16.0).floor().max(1.0) as usize;
                let storage = s.editor.as_ref().unwrap().storage;
                let area = selected(&l, storage);
                let p = s.preferences.policy(&area.key);
                let on = |v| {
                    if v {
                        t("вкл", "on")
                    } else {
                        t("выкл", "off")
                    }
                };
                let rows:Vec<(String,Button,String)>=vec![
                    (if language()==Language::Russian{format!("Область: {}",area.label)}else{format!("Area: {}",area.label)},Button::Area,t("Выбрать хранилище или инвентарь.", "Choose storage or inventory.").into()),
                    (if language()==Language::Russian{format!("Кнопки: {}",on(p.visible))}else{format!("Buttons: {}",on(p.visible))},Button::Visibility,t("Видимость S/F/R/T. Средняя кнопка работает и без них.", "Visibility of S/F/R/T. The middle button works without them.").into()),
                    (if language()==Language::Russian{format!("Сортировка: {}",on(p.enabled))}else{format!("Sorting: {}",on(p.enabled))},Button::Enabled,t("Разрешить операции для этой области.", "Allow operations for this area.").into()),
                    (p.mode.label_in(language()).into(),Button::Mode,t("ID → имя в языке игры → количество по убыванию.", "ID → name in the game language → quantity descending.").into()),
                    (if language()==Language::Russian{format!("Порядок: {}",if p.columns{"столбцы"}else{"строки"})}else{format!("Order: {}",if p.columns{"columns"}else{"rows"})},Button::Columns,t("Направление заполнения свободных слотов.", "Fill direction for free slots.").into()),
                    (t("← кнопки / → ПКМ", "← buttons / → RMB").into(),Button::Offset(-2,0),t("ЛКМ сдвигает влево, ПКМ вправо.", "LMB moves left, RMB moves right.").into()),
                    (t("↑ кнопки / ↓ ПКМ", "↑ buttons / ↓ RMB").into(),Button::Offset(0,-2),t("ЛКМ сдвигает вверх, ПКМ вниз.", "LMB moves up, RMB moves down.").into()),
                    (if language()==Language::Russian{format!("Снять исключения ({})",p.ignored.len())}else{format!("Clear exclusions ({})",p.ignored.len())},Button::Clear,t("Вернуть все слоты выбранной области. Хотбар всегда защищён.", "Restore all slots of the selected area. The hotbar is always protected.").into()),
                    (t("Сбросить область", "Reset area").into(),Button::Default,t("Вернуть стандартную раскладку выбранной области.", "Restore the default layout of the selected area.").into()),
                    (t("Сохранить как шаблон", "Save as template").into(),Button::SaveDefault,t("Настройки кнопок и порядка для новых контейнеров. Исключения не копируются.", "Button and order settings for new containers. Exclusions are not copied.").into()),
                    (if language()==Language::Russian{format!("Сквозь рамки: {}",on(s.preferences.frames))}else{format!("Through frames: {}",on(s.preferences.frames))},Button::Frames,t("Открывать хранилище под обычной / светящейся рамкой. Shift сохраняет обычное действие.", "Open storage behind a normal / glow item frame. Shift keeps the normal action.").into()),
                    (if language()==Language::Russian{format!("Сквозь таблички: {}",on(s.preferences.signs))}else{format!("Through signs: {}",on(s.preferences.signs))},Button::Signs,t("Открывать хранилище, на котором закреплена табличка. Shift для редактирования.", "Open storage that has a sign attached. Shift to edit the sign.").into()),
                    (if language()==Language::Russian{format!("Это меню: {}",if l.trusted{"разрешено"}else{"защищено"})}else{format!("This menu: {}",if l.trusted{"allowed"}else{"protected"})},Button::Virtual,t("Для виртуального меню сервера разрешай клики только если это хранилище. Магазины и обмены защищены.", "For a virtual server menu, allow clicks only if it is storage. Shops and trades are protected.").into()),
                    (t("Отменить изменения", "Undo changes").into(),Button::Undo,t("Вернуть все настройки на момент открытия редактора.", "Restore all settings to when the editor was opened.").into()),
                    (t("Отмена [Esc]", "Cancel [Esc]").into(),Button::Cancel,t("Закрыть без сохранения.", "Close without saving.").into()),
                    (t("Готово", "Done").into(),Button::Done,t("Сохранить настройки и закрыть редактор.", "Save settings and close the editor.").into()),
                ];
                // Split into two compact columns on short screens, without unreachable buttons.
                let columns = rows.len().div_ceil(max_rows);
                let rw = if columns > 1 { pw / columns as f32 } else { pw };
                for (i, (label, action, tip)) in rows.into_iter().enumerate() {
                    let col = i / max_rows;
                    let row = i % max_rows;
                    button(
                        game,
                        batch,
                        screen,
                        s,
                        [
                            px + col as f32 * rw,
                            py + 16.0 + row as f32 * 16.0,
                            rw - 1.0,
                            15.0,
                        ],
                        &label,
                        action,
                        tip,
                        false,
                    );
                }
                let hint = s.error.as_deref().unwrap_or(t(
                    "ЛКМ по слоту: исключить. Хотбар защищён.",
                    "LMB on a slot: exclude. The hotbar is protected.",
                ));
                ui::text(
                    game,
                    batch,
                    &clipped(game, hint, batch.width - 8.0),
                    [4.0, batch.height - 9.0],
                    0xffcc91,
                    true,
                );
            }
        }
        if let Some(hit) = s.hits.iter().rev().find(|h| contains(h.rect, screen.mouse)) {
            // Own tooltip stays on-screen; wrapped lines fit smaller GUI scales.
            let max = (batch.width - 16.0).clamp(70.0, 300.0);
            let mut lines = Vec::new();
            let mut line = String::new();
            for word in hit.tip.split_whitespace() {
                let next = if line.is_empty() {
                    word.into()
                } else {
                    format!("{line} {word}")
                };
                if game.native_text_width(&next) > max && !line.is_empty() {
                    lines.push(line);
                    line = word.into();
                } else {
                    line = next;
                }
            }
            if !line.is_empty() {
                lines.push(line);
            }
            let w = lines
                .iter()
                .map(|s| game.native_text_width(s))
                .fold(0.0, f32::max)
                + 8.0;
            let h = lines.len() as f32 * 10.0 + 6.0;
            let x = (screen.mouse[0] + 10.0).min(batch.width - w - 2.0).max(2.0);
            let y = (screen.mouse[1] + 12.0)
                .min(batch.height - h - 2.0)
                .max(2.0);
            ui::fill(game, batch, [x, y, w, h], 0xf0121720);
            for (i, line) in lines.iter().enumerate() {
                ui::text(
                    game,
                    batch,
                    line,
                    [x + 4.0, y + 3.0 + i as f32 * 10.0],
                    0xe5ebf2,
                    false,
                );
            }
        }
    });
    let variant = state(|s| {
        if s.editor.is_some() {
            "editor"
        } else {
            "controls"
        }
    });
    ui::finish(game, batch, render_start, "inventory-tools", variant);
}
fn edit(s: &mut State) {
    stop(s);
    s.editor = Some(Editor {
        before: s.preferences.clone(),
        storage: s.layout.as_ref().is_some_and(|l| l.storage.is_some()),
        collapsed: false,
    });
}
fn operate(game: &Game<'_>, s: &mut State, l: &Layout, action: Button, right: bool) {
    if let Button::Run(storage, op) = action {
        if s.run.is_some() || s.request.is_some() {
            stop(s);
        } else {
            s.request = Some((l.clone(), storage, op));
        }
        return;
    }
    if matches!(action, Button::Edit) {
        edit(s);
        return;
    }
    let Some(editor) = s.editor.as_ref() else {
        return;
    };
    let (editor_storage, collapsed, before) =
        (editor.storage, editor.collapsed, editor.before.clone());
    let area = selected(l, editor_storage);
    let mut p = s.preferences.policy(&area.key);
    let mut policy = false;
    match action {
        Button::Area => {
            if l.storage.is_some() {
                s.editor.as_mut().unwrap().storage = !editor_storage;
            }
        }
        Button::Visibility => {
            p.visible = !p.visible;
            policy = true;
        }
        Button::Enabled => {
            p.enabled = !p.enabled;
            policy = true;
        }
        Button::Mode => {
            p.mode = p.mode.next();
            policy = true;
        }
        Button::Columns => {
            p.columns = !p.columns;
            policy = true;
        }
        Button::Offset(x, y) => {
            let sign = if right { -1 } else { 1 };
            p.offset = [
                (p.offset[0] + x * sign).clamp(-200, 200),
                (p.offset[1] + y * sign).clamp(-200, 200),
            ];
            policy = true;
        }
        Button::Clear => {
            p.ignored.clear();
            policy = true;
        }
        Button::Default => {
            p = Policy::default();
            policy = true;
        }
        Button::SaveDefault => {
            s.preferences.defaults = p.clone();
            s.preferences.defaults.ignored.clear();
        }
        Button::Frames => s.preferences.frames = !s.preferences.frames,
        Button::Signs => s.preferences.signs = !s.preferences.signs,
        Button::Virtual => {
            if !l.trusted {
                s.preferences.virtual_menus.insert(l.virtual_key.clone());
            } else {
                s.preferences.virtual_menus.remove(&l.virtual_key);
            }
        }
        Button::Undo => s.preferences = before.clone(),
        Button::Cancel => {
            s.preferences = before.clone();
            s.editor = None;
        }
        Button::Done => {
            if s.writable {
                let dir = game.native_data_dir(claims::ID);
                let saved = std::fs::create_dir_all(&dir).and_then(|()| {
                    let temp = dir.join("inventory_tools.tmp");
                    std::fs::write(&temp, s.preferences.encode())?;
                    std::fs::rename(temp, dir.join("inventory_tools"))
                });
                match saved {
                    Ok(()) => {
                        s.editor = None;
                        s.error = None;
                    }
                    Err(e) => {
                        s.error = Some(if language() == Language::Russian {
                            format!("Не удалось сохранить: {e}")
                        } else {
                            format!("Could not save: {e}")
                        })
                    }
                }
            } else {
                s.error = Some(
                    t(
                        "Настройки не сохранены: проверь файл inventory_tools",
                        "Settings not saved: check the inventory_tools file",
                    )
                    .into(),
                );
            }
        }
        Button::Collapse => s.editor.as_mut().unwrap().collapsed = !collapsed,
        _ => {}
    }
    if policy
        && (s.preferences.policies.len() < 256 || s.preferences.policies.contains_key(&area.key))
    {
        s.preferences.policies.insert(area.key.clone(), p);
    }
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if *canceled {
        return;
    }
    load_settings(game);
    let Some(l) = layout(game, screen) else {
        return;
    };
    state(|s| {
        if s.layout.as_ref().is_none_or(|old| old.context != l.context) {
            s.hits.clear();
            return;
        }
        if let Some(hit) = s.hits.iter().rev().find(|h| contains(h.rect, screen.mouse)) {
            let action = hit.button;
            if button == 0 || button == 1 {
                operate(game, s, &l, action, button == 1);
                s.pressed.insert(button);
                *canceled = true;
            }
            return;
        }
        if s.editor.is_some() {
            *canceled = true;
            s.pressed.insert(button);
            if button == 0 {
                for storage in [true, false] {
                    if storage && l.storage.is_none() {
                        continue;
                    }
                    let area = selected(&l, storage);
                    if let Some(index) = area
                        .slots
                        .iter()
                        .position(|slot| contains(slot_rect(game, screen, *slot), screen.mouse))
                    {
                        let mut p = s.preferences.policy(&area.key);
                        if !p.ignored.remove(&index) {
                            p.ignored.insert(index);
                        }
                        s.preferences.policies.insert(area.key.clone(), p);
                        s.editor.as_mut().unwrap().storage = storage;
                        break;
                    }
                }
            }
            return;
        }
        if button == 2 {
            for storage in [true, false] {
                if storage && l.storage.is_none() || l.storage.is_some() && !l.trusted {
                    continue;
                }
                let area = selected(&l, storage);
                let p = s.preferences.policy(&area.key);
                if p.enabled
                    && area
                        .slots
                        .iter()
                        .any(|slot| contains(slot_rect(game, screen, *slot), screen.mouse))
                {
                    let mut p = p;
                    if game.modifiers.shift_key() {
                        p.mode = model::Mode::Quantity;
                    } else if game.modifiers.control_key() {
                        p.mode = model::Mode::Name;
                    } else if game.modifiers.alt_key() {
                        p.mode = model::Mode::Id;
                    }
                    // Temporary modifier order belongs to this run, not persisted policy.
                    operate(
                        game,
                        s,
                        &l,
                        Button::Run(storage, model::Operation::Sort),
                        false,
                    );
                    if s.request.is_some() {
                        // The frame captures the temporary mode below.
                        s.run = None;
                        let area = selected(&l, storage).clone();
                        s.run = Some(new_run(
                            s,
                            &l,
                            storage,
                            model::Operation::Sort,
                            Some(p),
                            area,
                        ));
                        s.request = None;
                    }
                    s.pressed.insert(button);
                    *canceled = true;
                    return;
                }
            }
        }
        stop(s);
    });
}
pub(super) fn release(button: i32, canceled: &mut bool) {
    state(|s| {
        if s.pressed.remove(&button) {
            *canceled = true;
        }
    });
}
pub(super) fn key(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if !game.native_on(claims::ID)
        || game.screen != Screen::Inventory
        || game.native_mod_screen().is_some()
    {
        return false;
    }
    state(|s| {
        if s.editor.is_some() {
            *canceled = true;
            match key {
                "key.keyboard.escape" => {
                    if let Some(e) = s.editor.take() {
                        s.preferences = e.before;
                    }
                }
                "key.keyboard.tab" => {
                    let e = s.editor.as_mut().unwrap();
                    e.collapsed = !e.collapsed;
                }
                "key.keyboard.enter" | "key.keyboard.o" => {
                    if let Some(l) = s.layout.clone() {
                        operate(game, s, &l, Button::Done, false);
                    }
                }
                _ => {}
            }
            return true;
        }
        if key == "key.keyboard.escape" {
            stop(s);
        }
        let open_editor = key == "key.keyboard.o" && game.modifiers.control_key();
        if open_editor && s.layout.is_some() {
            edit(s);
            *canceled = true;
            return true;
        }
        false
    })
}
pub(super) fn editing() -> bool {
    state(|s| s.editor.is_some())
}
fn new_run(
    s: &State,
    l: &Layout,
    storage: bool,
    operation: model::Operation,
    override_policy: Option<Policy>,
    area: Area,
) -> Run {
    let other = if storage {
        Some(l.player.clone())
    } else {
        l.storage.clone()
    };
    Run {
        context: l.context.clone(),
        policy: override_policy.unwrap_or_else(|| s.preferences.policy(&area.key)),
        other_policy: other
            .as_ref()
            .map(|a| s.preferences.policy(&a.key))
            .unwrap_or_default(),
        area,
        other,
        operation,
        expected: None,
        types: None,
        last: Instant::now() - Duration::from_secs(1),
        started: Instant::now(),
        steps: 0,
    }
}
fn describe(stack: &ItemStack, env: &horizon_client::menu::Env<'_>) -> model::Stack {
    let name = horizon_client::item_tooltip::name_line(stack, &|key| env.names.display_get(key));
    model::Stack {
        key: format!("{}|{}|{:?}", stack.item, stack.damage, stack.data()),
        id: stack.item.to_string(),
        name: horizon_client::text::Text::legacy(&name).plain(),
        count: stack.count,
        max: env
            .items
            .get(&stack.item)
            .map_or(64, |i| i.max_stack)
            .max(1),
    }
}
fn finish(message: &str) {
    state(|s| {
        stop(s);
        s.status = Some((message.into(), Instant::now()));
    });
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID)
        || game.screen != Screen::Inventory
        || game.native_mod_screen().is_some()
    {
        state(|s| {
            stop(s);
            if let Some(e) = s.editor.take() {
                s.preferences = e.before;
            }
        });
        return;
    }
    let run = state(|s| {
        if let Some((l, storage, op)) = s.request.take() {
            let area = selected(&l, storage).clone();
            s.run = Some(new_run(s, &l, storage, op, None, area));
        }
        if s.run
            .as_ref()
            .is_some_and(|r| r.last.elapsed() >= Duration::from_millis(150))
        {
            s.run.take()
        } else {
            None
        }
    });
    let Some(mut r) = run else {
        return;
    };
    if r.context != state(|s| context(game, s.generation))
        || r.started.elapsed() > Duration::from_secs(90)
        || r.steps >= 512
    {
        finish(t(
            "Сортировка остановлена: окно изменилось или истекло время.",
            "Sorting stopped: the window changed or timed out.",
        ));
        return;
    }
    if !r.policy.enabled || r.operation != model::Operation::Sort && !r.other_policy.enabled {
        finish(t(
            "Операции для этой области выключены.",
            "Operations for this area are disabled.",
        ));
        return;
    }
    let mut operations = Vec::new();
    let mut after = None;
    let mut pending = false;
    let mut invalid = false;
    let mut types = r.types.clone();
    let shulker_window = game.remote_menu_type() == Some("minecraft:shulker_box")
        || game.container_block_is_shulker();
    let valid = game.menu_session(|menu, env, to_menu| {
        if menu.carried.is_some() {
            pending = true;
            return Some(false);
        }
        let before: Vec<_> = (0..menu.slot_count())
            .map(|i| menu.slot(i).map(|s| describe(s, env)))
            .collect();
        if r.expected.as_ref().is_some_and(|e| *e != before) {
            invalid = true;
            return Some(false);
        }
        let dest_game = order(&r.area, &r.policy);
        let dest: Option<Vec<_>> = dest_game.iter().map(|i| to_menu(*i)).collect();
        let Some(dest) = dest else {
            return None;
        };
        let mut mapping: Vec<(usize, usize)> =
            dest_game.into_iter().zip(dest.iter().copied()).collect();
        let plan = if r.operation == model::Operation::Sort {
            model::next_sort(&before, &dest, r.policy.mode)
        } else {
            let Some(other) = &r.other else {
                return None;
            };
            let src_game = order(other, &r.other_policy);
            let src: Option<Vec<_>> = src_game.iter().map(|i| to_menu(*i)).collect();
            let Some(src) = src else {
                return None;
            };
            mapping.extend(src_game.into_iter().zip(src.iter().copied()));
            let allowed_types = types.get_or_insert_with(|| {
                dest.iter()
                    .filter_map(|i| before[*i].as_ref().map(|s| s.key.clone()))
                    .collect()
            });
            let shulker = shulker_window || menu.kind() == Kind::Shulker;
            model::next_transfer(
                &before,
                &src,
                &dest,
                r.operation,
                allowed_types,
                |from, to| {
                    // Shulker boxes cannot contain shulker boxes; skip them rather than abort the rest.
                    !(shulker
                        && to < 27
                        && before[from]
                            .as_ref()
                            .is_some_and(|s| s.id.ends_with("shulker_box")))
                },
            )
        };
        let plan = match plan {
            Ok(p) => p,
            Err(_) => {
                invalid = true;
                return Some(false);
            }
        };
        let Some(plan) = plan else {
            return Some(false);
        };
        // The only clicks ever emitted map back to explicit main-inventory / storage slots.
        for i in &plan.clicks {
            let Some((g, _)) = mapping.iter().find(|(_, m)| m == i) else {
                invalid = true;
                return Some(false);
            };
            if !model::safe_game_slot(*g) {
                invalid = true;
                return Some(false);
            }
            operations.push((Some(*g), 0, Input::Pickup));
        }
        let dropped = menu.dropped.len();
        for i in &plan.clicks {
            if menu.click(env, *i as i32, 0, Input::Pickup).is_err() {
                invalid = true;
                return Some(false);
            }
        }
        let observed: Vec<_> = (0..menu.slot_count())
            .map(|i| menu.slot(i).map(|s| describe(s, env)))
            .collect();
        if menu.carried.is_some() || menu.dropped.len() != dropped || observed != plan.after {
            invalid = true;
            return Some(false);
        }
        after = Some(plan.after);
        Some(false)
    });
    if pending {
        if r.last.elapsed() > Duration::from_secs(2) {
            finish(t(
                "Освободи курсор. Предметы не выбрасываются.",
                "Free your cursor. Items are not dropped.",
            ));
        } else {
            state(|s| s.run = Some(r));
        }
        return;
    }
    if !valid || invalid {
        finish(t(
            "Операция остановлена: содержимое изменилось или меню ограничивает клики.",
            "Operation stopped: contents changed or the menu restricts clicks.",
        ));
        return;
    }
    if operations.is_empty() {
        finish(t("Готово", "Done"));
        return;
    }
    if game.menu_ops(&operations) {
        r.expected = after;
        r.types = types;
        r.last = Instant::now();
        r.steps += 1;
        state(|s| s.run = Some(r));
    } else {
        finish(t(
            "Операция остановлена: окно недоступно.",
            "Operation stopped: window unavailable.",
        ));
    }
}
