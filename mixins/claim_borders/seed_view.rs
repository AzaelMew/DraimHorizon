// Fullscreen seed atlas with dimension tabs, searchable icon filters and native item markers.
use crate::game::{
    Game, claim_borders as claims, claim_borders_map as map, claim_borders_map_view as terrain,
    claim_borders_seed_map as model, claim_borders_seed_worker as worker,
    claim_borders_ui_view as ui,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::{hud::HudBatch, items::ItemStack};
use std::sync::{Mutex, OnceLock};
struct State {
    active: bool,
    loaded: bool,
    preferences: model::Preferences,
    views: [map::View; 3],
    selected: usize,
    drag: Option<([f32; 2], [f64; 2])>,
    rect: [f32; 4],
    size: [f32; 2],
    scope: String,
    worker: Option<worker::Handle>,
    requested: Option<worker::Query>,
    image: Option<worker::Image>,
    error: Option<String>,
    filters: bool,
    group: Option<model::Group>,
    search: String,
    search_focus: bool,
    offset: usize,
    biome_tab: bool,
    right_click: Option<(std::time::Instant, usize, [f32; 2])>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            active: false,
            loaded: false,
            preferences: model::Preferences::default(),
            views: [map::View {
                center: [0.0; 2],
                blocks_per_pixel: 16.0,
            }; 3],
            selected: 0,
            drag: None,
            rect: [0.0; 4],
            size: [0.0; 2],
            scope: String::new(),
            worker: None,
            requested: None,
            image: None,
            error: None,
            filters: false,
            group: None,
            search: String::new(),
            search_focus: false,
            offset: 0,
            biome_tab: false,
            right_click: None,
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
pub(super) fn active() -> bool {
    state(|s| s.active)
}
pub(super) fn reset() {
    state(|s| {
        if let Some(w) = &s.worker {
            w.cancel();
        }
        s.active = false;
        s.image = None;
        s.requested = None;
        s.drag = None;
        s.right_click = None;
        s.scope.clear();
        s.search_focus = false;
    });
}
/// Opening M always prepares the seed atlas in the background, with explored terrain selected.
pub(super) fn open(source: &map::Snapshot, _focus: &Option<(String, i32, i32)>) {
    state(|s| {
        s.active = false;
        s.drag = None;
        s.right_click = None;
        s.search_focus = false;
        let nether = source.dimension == model::DIMENSIONS[1];
        s.views = std::array::from_fn(|i| {
            let factor = if nether && i == 0 {
                8.0
            } else if !nether && i == 1 {
                0.125
            } else {
                1.0
            };
            let mut v = map::View {
                center: [source.player[0] * factor, source.player[2] * factor],
                blocks_per_pixel: 16.0,
            };
            v.constrain();
            v
        });
        s.selected = model::DIMENSIONS
            .iter()
            .position(|d| *d == source.dimension)
            .unwrap_or(0);
    });
}
pub(super) fn show() {
    state(|s| {
        s.active = true;
        s.requested = None;
        s.drag = None;
    });
}
fn text(game: &Game<'_>, batch: &mut HudBatch, value: &str, pos: [f32; 2], width: f32, color: u32) {
    terrain::label(game, batch, value, pos, width, color);
}
fn save(game: &Game<'_>, s: &State) {
    game.native_set_setting(claims::ID, "seed_map", &s.preferences.encode());
}
fn filter_rect(size: [f32; 2]) -> [f32; 4] {
    model::filter_rect(size)
}
fn search_rect(s: &State) -> [f32; 4] {
    let r = filter_rect(s.size);
    [r[0] + 8.0, r[1] + 30.0, r[2] - 16.0, 20.0]
}
#[derive(Clone, Copy)]
enum Control {
    Explored,
    World(usize),
    Zoom(f32),
    Center,
    Filters,
    Settings,
    Close,
    Claims,
}
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
fn controls(size: [f32; 2]) -> Vec<(Control, [f32; 4], &'static str)> {
    let lang = language();
    let mut out = vec![
        (
            Control::Explored,
            [6.0, 4.0, 118.0, 22.0],
            lang.text("Исследованная", "Explored"),
        ),
        (
            Control::Close,
            [size[0] - 28.0, 4.0, 22.0, 22.0],
            lang.text("Закрыть", "Close"),
        ),
        (Control::Zoom(0.5), [8.0, 42.0, 24.0, 24.0], "+"),
        (Control::Zoom(2.0), [8.0, 70.0, 24.0, 24.0], "−"),
        (
            Control::Center,
            [8.0, 106.0, 24.0, 24.0],
            lang.text("К себе", "Center"),
        ),
        (
            Control::Filters,
            [8.0, 134.0, 24.0, 24.0],
            lang.text("Фильтры", "Filters"),
        ),
        (Control::Claims, [8.0, 162.0, 24.0, 24.0], "Claims"),
        (
            Control::Settings,
            [8.0, 190.0, 24.0, 24.0],
            lang.text("Настройки", "Settings"),
        ),
    ];
    for (i, label) in [
        lang.text("Обычный мир", "Overworld"),
        lang.text("Незер", "Nether"),
        lang.text("Край", "End"),
    ]
    .into_iter()
    .enumerate()
    {
        out.push((Control::World(i), model::dimension_rect(size, i), label));
    }
    let mut index = 0;
    for (_, r, _) in &mut out {
        if r[0] == 8.0 {
            *r = model::tool_rect(size, index);
            index += 1;
        }
    }
    out
}
fn item_icon(game: &Game<'_>, batch: &mut HudBatch, id: &str, at: [f32; 2]) {
    if game.res.items().get(id).is_some() {
        game.draw_item(batch, &ItemStack::new(id, 1), at[0], at[1]);
    } else {
        ui::icon(
            game,
            batch,
            ui::Icon::Map,
            [at[0], at[1], 16.0, 16.0],
            0xe6e9ed,
        );
    }
}
fn biome_label(game: &Game<'_>, id: &str) -> String {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    game.translate(&format!("biome.minecraft.{path}"), &path.replace('_', " "))
}
fn biome_indices(game: &Game<'_>, s: &State) -> Vec<String> {
    let q = s.search.to_lowercase();
    s.image
        .as_ref()
        .map(|im| {
            im.catalog
                .iter()
                .filter(|id| {
                    model::biome_dimension(id) == s.selected
                        && (q.is_empty()
                            || id.contains(&q)
                            || biome_label(game, id).to_lowercase().contains(&q))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
fn chip_rect(s: &State, n: usize) -> [f32; 4] {
    model::filter_chip_rect(s.size, n)
}
fn chip_rows(s: &State) -> usize {
    model::filter_rows(s.size)
}
fn bulk_rect(s: &State) -> [f32; 4] {
    let r = filter_rect(s.size);
    [r[0] + r[2] - 68.0, r[1] + r[3] - 24.0, 60.0, 18.0]
}
fn filters(game: &Game<'_>, batch: &mut HudBatch, s: &State, mouse: [f32; 2]) {
    let r = filter_rect(s.size);
    let lang = language();
    let mut tooltip = None;
    ui::rounded(game, batch, r, 6.0, 0xf019222b);
    ui::button(
        game,
        batch,
        [r[0] + 8.0, r[1] + 6.0, (r[2] - 44.0) / 2.0, 19.0],
        lang.text("Структуры", "Structures"),
        !s.biome_tab,
        mouse,
    );
    ui::button(
        game,
        batch,
        [
            r[0] + 12.0 + (r[2] - 44.0) / 2.0,
            r[1] + 6.0,
            (r[2] - 44.0) / 2.0,
            19.0,
        ],
        lang.text("Биомы", "Biomes"),
        s.biome_tab,
        mouse,
    );
    terrain::tool(
        game,
        batch,
        [r[0] + r[2] - 26.0, r[1] + 6.0, 18.0, 19.0],
        lang.text("Закрыть фильтры", "Close filters"),
        ui::Icon::Close,
        false,
        mouse,
    );
    let search = search_rect(s);
    ui::button_base(
        game,
        batch,
        search,
        lang.text("Поиск фильтров", "Search filters"),
        s.search_focus,
        mouse,
    );
    ui::icon(
        game,
        batch,
        ui::Icon::Search,
        [search[0] + 4.0, search[1] + 4.0, 12.0, 12.0],
        0x99aebf,
    );
    text(
        game,
        batch,
        if s.search.is_empty() {
            lang.text("Поиск…", "Search…")
        } else {
            &s.search
        },
        [search[0] + 20.0, search[1] + 6.0],
        search[2] - 24.0,
        0xc7d7df,
    );
    if s.biome_tab {
        ui::button(
            game,
            batch,
            [r[0] + 8.0, r[1] + 56.0, 80.0, 22.0],
            lang.text("Все биомы", "All biomes"),
            s.preferences.biome.is_none(),
            mouse,
        );
        ui::button(
            game,
            batch,
            [r[0] + 92.0, r[1] + 56.0, r[2] - 100.0, 22.0],
            lang.text("Цвета биомов", "Biome colors"),
            s.preferences.biomes,
            mouse,
        );
        for (n, id) in biome_indices(game, s)
            .into_iter()
            .skip(s.offset * 4)
            .take(chip_rows(s) * 4)
            .enumerate()
        {
            let c = chip_rect(s, n);
            let title = biome_label(game, &id);
            ui::button_base(
                game,
                batch,
                c,
                &title,
                s.preferences.biome.as_ref() == Some(&id),
                mouse,
            );
            item_icon(
                game,
                batch,
                "minecraft:grass_block",
                [c[0] + c[2] / 2.0 - 8.0, c[1] + 4.0],
            );
            text(
                game,
                batch,
                &title,
                [c[0] + 3.0, c[1] + 27.0],
                c[2] - 6.0,
                model::biome_color(&id),
            );
            if model::contains(c, mouse) {
                tooltip = Some(title);
            }
        }
    } else {
        let groups: Vec<_> = std::iter::once(None)
            .chain(
                model::GROUPS
                    .into_iter()
                    .filter(|g| g.dimension() == s.selected)
                    .map(Some),
            )
            .collect();
        let w = (r[2] - 16.0) / groups.len() as f32;
        for (i, g) in groups.into_iter().enumerate() {
            ui::button(
                game,
                batch,
                [r[0] + 8.0 + i as f32 * w, r[1] + 56.0, w - 2.0, 22.0],
                g.map_or(lang.text("Все", "All"), |g| g.label_in(lang)),
                s.group == g,
                mouse,
            );
        }
        for (n, i) in model::filter_indices(s.selected, s.group, &s.search)
            .into_iter()
            .skip(s.offset * 4)
            .take(chip_rows(s) * 4)
            .enumerate()
        {
            let c = chip_rect(s, n);
            let def = &model::STRUCTURES[i];
            let on = s.preferences.shows(def.id);
            ui::button_base(game, batch, c, def.label_in(lang), on, mouse);
            item_icon(game, batch, def.item, [c[0] + c[2] / 2.0 - 8.0, c[1] + 4.0]);
            text(
                game,
                batch,
                def.label_in(lang),
                [c[0] + 3.0, c[1] + 27.0],
                c[2] - 6.0,
                if on { def.color } else { 0x687887 },
            );
            if model::contains(c, mouse) {
                tooltip = Some(format!(
                    "{} · {}",
                    def.label_in(lang),
                    if on {
                        lang.text("показано", "shown")
                    } else {
                        lang.text("скрыто", "hidden")
                    }
                ));
            }
        }
    }
    text(
        game,
        batch,
        lang.text("Колесо — прокрутка", "Wheel — scroll"),
        [r[0] + 8.0, r[1] + r[3] - 13.0],
        r[2] - 84.0,
        0x7f94a4,
    );
    if !s.biome_tab {
        let indices = model::filter_indices(s.selected, s.group, &s.search);
        let all = indices
            .iter()
            .all(|&i| s.preferences.shows(model::STRUCTURES[i].id));
        ui::button(
            game,
            batch,
            bulk_rect(s),
            if all {
                lang.text("Скрыть", "Hide")
            } else {
                lang.text("Показать", "Show")
            },
            false,
            mouse,
        );
    }
    if let Some(value) = tooltip {
        ui::tooltip(game, batch, &value, mouse);
    }
}
pub(super) fn render(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) -> bool {
    if !active() {
        return false;
    }
    let Some(source) = terrain::seed_source() else {
        return false;
    };
    let p = terrain::settings(game);
    state(|s| {
        if !s.loaded {
            if let Some(stored) = game
                .native_setting(claims::ID, "seed_map")
                .and_then(|v| model::Preferences::decode(&v).ok())
            {
                s.preferences = stored;
                s.selected = s.preferences.dimension;
            } else {
                s.preferences.dimension = s.selected;
            }
            s.loaded = true;
        }
        s.size = [screen.rect[2], screen.rect[3]];
        s.rect = model::map_rect(s.size);
        let r = s.rect;
        let aspect = r[2] / r[3];
        if s.scope != source.scope {
            if let Some(w) = &s.worker {
                w.cancel();
            }
            s.image = None;
            s.requested = None;
            s.scope = source.scope.clone();
            s.error = None;
        }
        if let Some((mouse, center)) = s.drag {
            let scale = f64::from(128.0 * s.views[s.selected].blocks_per_pixel / r[3]);
            s.views[s.selected].center = [
                center[0] - f64::from(screen.mouse[0] - mouse[0]) * scale,
                center[1] - f64::from(screen.mouse[1] - mouse[1]) * scale,
            ];
            s.views[s.selected].constrain();
        }
        if s.worker.is_none() && s.error.is_none() {
            match worker::Handle::spawn(
                game.options.resources.join("worldgen"),
                game.pack.assets.registry,
            ) {
                Ok(w) => s.worker = Some(w),
                Err(e) => s.error = Some(e),
            }
        }
        let view = s.views[s.selected];
        let dim = model::DIMENSIONS[s.selected];
        let query = worker::Query {
            scope: source.scope.clone(),
            view,
            aspect,
            dimension: s.selected,
            preferences: s.preferences.clone(),
        };
        if s.requested.as_ref() != Some(&query) {
            if let Some(w) = &s.worker {
                w.query(query.clone());
            }
            s.requested = Some(query.clone());
        }
        if let Some(image) = s.worker.as_ref().and_then(worker::Handle::take) {
            if image.query == query {
                s.error = image.error.clone();
                if let Some(panel) = &image.panel {
                    if let Some(uv) = game
                        .gui
                        .atlas
                        .sprites
                        .get("claim_borders/seed_map")
                        .copied()
                    {
                        terrain::seed_patch(game, uv, panel.pixels.clone());
                    }
                }
                s.image = Some(image);
            }
        }
        let start = batch.vertices.len();
        ui::fill(game, batch, screen.rect, 0xff131b24);
        ui::fill(game, batch, r, 0xff1b2531);
        let mut info = if let Some(e) = &s.error {
            format!("Seed Map: {e}")
        } else if s.image.as_ref().is_none_or(|im| im.query != query) {
            language()
                .text(
                    "Расчёт… · прогноз vanilla Java 26.3",
                    "Calculating… · vanilla Java 26.3 forecast",
                )
                .into()
        } else {
            format!(
                "{} {} · {} Y64 · Java 26.3",
                language().text("Сид", "Seed"),
                model::SEED,
                language().text("Биомы", "Biomes")
            )
        };
        let covered = s.filters && model::contains(filter_rect(s.size), screen.mouse);
        if let Some(im) = s.image.as_ref().filter(|im| {
            im.query.dimension == s.selected && im.query.preferences.same_biomes(&s.preferences)
        }) {
            if let Some(panel) = &im.panel {
                if let Some(uv) = game
                    .gui
                    .atlas
                    .sprites
                    .get("claim_borders/seed_map")
                    .copied()
                {
                    let corners = [[0.0, 0.0], [128.0, 128.0]].map(|xy| {
                        let world = im.query.view.world_aspect(xy, 128.0, im.query.aspect);
                        let at = view.pixel_aspect(world, 128.0, aspect);
                        [r[0] + at[0] / 128.0 * r[2], r[1] + at[1] / 128.0 * r[3]]
                    });
                    let first = batch.vertices.len();
                    batch.rect_uv(
                        corners[0][0],
                        corners[0][1],
                        corners[1][0] - corners[0][0],
                        corners[1][1] - corners[0][1],
                        uv,
                        u32::MAX,
                    );
                    batch.clip_since(first, r[0], r[1], r[0] + r[2], r[1] + r[3]);
                }
                if s.preferences.claims {
                    draw_claims(game, batch, &source, dim, view, r, p);
                }
                let mut clusters = std::collections::BTreeMap::<
                    (i32, i32),
                    (usize, [f32; 2], &worker::Mark),
                >::new();
                for mark in panel.marks.iter().filter(|m| s.preferences.shows(&m.id)) {
                    let at = view.pixel_aspect(mark.position, 128.0, aspect);
                    let at = [r[0] + at[0] / 128.0 * r[2], r[1] + at[1] / 128.0 * r[3]];
                    if model::contains(r, at) {
                        let key = ((at[0] / 22.0).floor() as i32, (at[1] / 22.0).floor() as i32);
                        clusters
                            .entry(key)
                            .and_modify(|v| v.0 += 1)
                            .or_insert((1, at, mark));
                    }
                }
                let first = batch.vertices.len();
                for (_, (count, at, mark)) in clusters {
                    let Some(i) = model::structure_index(&mark.id) else {
                        continue;
                    };
                    let def = &model::STRUCTURES[i];
                    ui::rounded(
                        game,
                        batch,
                        [at[0] - 11.0, at[1] - 11.0, 22.0, 22.0],
                        8.0,
                        0xdd000000 | def.color,
                    );
                    ui::rounded(
                        game,
                        batch,
                        [at[0] - 10.0, at[1] - 10.0, 20.0, 20.0],
                        7.0,
                        0xee101923,
                    );
                    item_icon(game, batch, def.item, [at[0] - 8.0, at[1] - 8.0]);
                    if count > 1 {
                        text(
                            game,
                            batch,
                            &count.to_string(),
                            [at[0] + 6.0, at[1] - 12.0],
                            22.0,
                            0xffe6a6,
                        );
                    }
                    if !covered
                        && (screen.mouse[0] - at[0]).abs() < 11.0
                        && (screen.mouse[1] - at[1]).abs() < 11.0
                    {
                        info = format!(
                            "{} · X {:.0} Z {:.0}{}",
                            def.label_in(language()),
                            mark.position[0],
                            mark.position[1],
                            if count > 1 {
                                format!(
                                    " · {} {}: {}",
                                    language().text("ещё", "more"),
                                    count - 1,
                                    language().text("приблизьте", "zoom in")
                                )
                            } else {
                                String::new()
                            }
                        );
                    }
                }
                batch.clip_since(first, r[0], r[1], r[0] + r[2], r[1] + r[3]);
                if !covered
                    && model::contains(r, screen.mouse)
                    && im.query.view == view
                    && im.query.aspect == aspect
                {
                    let at = model::world(view, r, screen.mouse);
                    let x = (((screen.mouse[0] - r[0]) / r[2] * worker::SIDE as f32) as usize)
                        .min(worker::SIDE - 1);
                    let y = (((screen.mouse[1] - r[1]) / r[3] * worker::SIDE as f32) as usize)
                        .min(worker::SIDE - 1);
                    if !panel.marks.iter().any(|m| {
                        let xy = view.pixel_aspect(m.position, 128.0, aspect);
                        ((xy[0] / 128.0 * r[2] + r[0]) - screen.mouse[0]).abs() < 11.0
                            && ((xy[1] / 128.0 * r[3] + r[1]) - screen.mouse[1]).abs() < 11.0
                    }) {
                        info = format!(
                            "{} · X {:.0} Z {:.0} · {}",
                            map::dimension_label_in(dim, language()),
                            at[0].floor(),
                            at[1].floor(),
                            biome_label(game, &panel.biomes[y * worker::SIDE + x])
                        );
                    }
                }
                if panel.limited {
                    text(
                        game,
                        batch,
                        language().text(
                            "Для полного поиска структур приблизьте карту",
                            "Zoom in for a full structure search",
                        ),
                        [44.0, r[1] + 7.0],
                        r[2] - 60.0,
                        0xf1c77c,
                    );
                }
            }
        } else {
            text(
                game,
                batch,
                language().text("Подготовка карты сида…", "Preparing seed map…"),
                [44.0, r[1] + 8.0],
                r[2] - 60.0,
                0x9cb0bf,
            );
        }
        terrain::map_marks(game, batch, &source, dim, view, r, p, true, aspect);
        terrain::map_frame(game, batch, r, view, aspect, false);
        ui::fill(game, batch, [0.0, 0.0, s.size[0], r[1]], 0xf0151e27);
        ui::fill(
            game,
            batch,
            [0.0, s.size[1] - 32.0, s.size[0], 32.0],
            0xee151e27,
        );
        ui::button(
            game,
            batch,
            [128.0, 4.0, 80.0, 22.0],
            "Seed Map",
            true,
            screen.mouse,
        );
        for (control, rect, label) in controls(s.size) {
            match control {
                Control::World(i) => {
                    ui::button(game, batch, rect, label, s.selected == i, screen.mouse)
                }
                Control::Explored => ui::button(game, batch, rect, label, false, screen.mouse),
                _ => {
                    let icon = match control {
                        Control::Zoom(v) if v < 1.0 => ui::Icon::Plus,
                        Control::Zoom(_) => ui::Icon::Minus,
                        Control::Center => ui::Icon::Home,
                        Control::Filters => ui::Icon::Layers,
                        Control::Claims => ui::Icon::Shield,
                        Control::Settings => ui::Icon::Settings,
                        _ => ui::Icon::Close,
                    };
                    terrain::tool(
                        game,
                        batch,
                        rect,
                        label,
                        icon,
                        matches!(control, Control::Filters) && s.filters
                            || matches!(control, Control::Claims) && s.preferences.claims,
                        screen.mouse,
                    );
                }
            }
        }
        text(
            game,
            batch,
            &info,
            [8.0, s.size[1] - 25.0],
            s.size[0] - 16.0,
            0xb9cbd6,
        );
        text(
            game,
            batch,
            language().text(
                "ЛКМ: двигать · колесо: масштаб · 2×ПКМ: метка Y64 · F: фильтры",
                "LMB: pan · wheel: zoom · 2×RMB: Y64 waypoint · F: filters",
            ),
            [8.0, s.size[1] - 12.0],
            s.size[0] - 16.0,
            0x758d9f,
        );
        if s.filters {
            filters(game, batch, s, screen.mouse);
        }
        ui::finish(game, batch, start, "seed-map", dim);
    });
    true
}
fn draw_claims(
    game: &Game<'_>,
    batch: &mut HudBatch,
    source: &map::Snapshot,
    dim: &str,
    view: map::View,
    r: [f32; 4],
    p: map::Preferences,
) {
    let start = batch.vertices.len();
    for cell in source
        .cells
        .iter()
        .filter(|c| c.dimension == dim && ((c.own && p.own) || (!c.own && p.friends)))
    {
        let at = view.pixel_aspect([f64::from(cell.x), f64::from(cell.z)], 128.0, r[2] / r[3]);
        let at = [r[0] + at[0] / 128.0 * r[2], r[1] + at[1] / 128.0 * r[3]];
        let w = 16.0 / f64::from(view.blocks_per_pixel) / 128.0 * f64::from(r[3]);
        let w = w as f32;
        if at[0] + w < r[0] || at[1] + w < r[1] || at[0] > r[0] + r[2] || at[1] > r[1] + r[3] {
            continue;
        }
        let rgb = if cell.foreign {
            0xe45b65
        } else if cell.own {
            source.own_color
        } else {
            source.friend_color
        };
        ui::fill(
            game,
            batch,
            [at[0], at[1], w, w],
            ((p.fill * 255.0) as u32) << 24 | rgb,
        );
        for line in [
            [at[0], at[1], w, 1.0],
            [at[0], at[1] + w - 1.0, w, 1.0],
            [at[0], at[1], 1.0, w],
            [at[0] + w - 1.0, at[1], 1.0, w],
        ] {
            ui::fill(game, batch, line, 0xff000000 | rgb);
        }
    }
    batch.clip_since(start, r[0], r[1], r[0] + r[2], r[1] + r[3]);
}
fn act(game: &Game<'_>, control: Control) {
    match control {
        Control::Explored => state(|s| {
            s.active = false;
            s.drag = None;
            s.search_focus = false;
            if let Some(w) = &s.worker {
                w.cancel();
            }
        }),
        Control::Close => terrain::seed_close(game),
        Control::Settings => terrain::seed_settings(game),
        Control::Center => {
            if let Some(source) = terrain::seed_source() {
                let selected = state(|s| s.selected);
                open(&source, &None);
                state(|s| {
                    s.active = true;
                    s.selected = selected;
                });
            }
        }
        Control::Zoom(factor) => {
            state(|s| s.views[s.selected].zoom_at([64.0, 64.0], 128.0, factor))
        }
        Control::World(i) => state(|s| {
            s.selected = i;
            s.preferences.dimension = i;
            if s.preferences
                .biome
                .as_ref()
                .is_some_and(|b| model::biome_dimension(b) != i)
            {
                s.preferences.biome = None;
            }
            s.drag = None;
            s.group = None;
            s.offset = 0;
            s.right_click = None;
            save(game, s);
        }),
        Control::Filters => state(|s| {
            s.filters = !s.filters;
            s.search_focus = false;
            s.drag = None;
        }),
        Control::Claims => state(|s| {
            s.preferences.claims = !s.preferences.claims;
            save(game, s);
        }),
    }
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    pressed: bool,
) -> bool {
    if !active() {
        return false;
    }
    if !pressed {
        state(|s| s.drag = None);
        return true;
    }
    let mut control = None;
    let mut point = None;
    state(|s| {
        if s.filters && model::contains(filter_rect(s.size), screen.mouse) {
            if button != 0 {
                return;
            }
            let r = filter_rect(s.size);
            if !s.biome_tab && model::contains(bulk_rect(s), screen.mouse) {
                let indices = model::filter_indices(s.selected, s.group, &s.search);
                let all = indices
                    .iter()
                    .all(|&i| s.preferences.shows(model::STRUCTURES[i].id));
                for i in indices {
                    if all {
                        s.preferences.structures &= !(1u64 << i);
                    } else {
                        s.preferences.structures |= 1u64 << i;
                    }
                }
                save(game, s);
                return;
            }
            s.search_focus = model::contains(search_rect(s), screen.mouse);
            if s.search_focus {
                return;
            }
            if screen.mouse[1] < r[1] + 29.0 {
                if screen.mouse[0] >= r[0] + r[2] - 26.0 {
                    s.filters = false;
                    return;
                }
                s.biome_tab = screen.mouse[0] >= r[0] + 12.0 + (r[2] - 44.0) / 2.0;
                s.offset = 0;
                return;
            }
            if model::contains([r[0] + 8.0, r[1] + 56.0, r[2] - 16.0, 22.0], screen.mouse) {
                if s.biome_tab {
                    if screen.mouse[0] < r[0] + 90.0 {
                        s.preferences.biome = None;
                    } else {
                        s.preferences.biomes = !s.preferences.biomes;
                    }
                    save(game, s);
                } else {
                    let groups: Vec<_> = std::iter::once(None)
                        .chain(
                            model::GROUPS
                                .into_iter()
                                .filter(|g| g.dimension() == s.selected)
                                .map(Some),
                        )
                        .collect();
                    let i = ((screen.mouse[0] - r[0] - 8.0) / ((r[2] - 16.0) / groups.len() as f32))
                        as usize;
                    s.group = groups.get(i).copied().flatten();
                    s.offset = 0;
                }
                return;
            }
            if s.biome_tab {
                for (n, id) in biome_indices(game, s)
                    .into_iter()
                    .skip(s.offset * 4)
                    .take(chip_rows(s) * 4)
                    .enumerate()
                {
                    if model::contains(chip_rect(s, n), screen.mouse) {
                        s.preferences.biome = if s.preferences.biome.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id)
                        };
                        save(game, s);
                        break;
                    }
                }
            } else {
                for (n, i) in model::filter_indices(s.selected, s.group, &s.search)
                    .into_iter()
                    .skip(s.offset * 4)
                    .take(chip_rows(s) * 4)
                    .enumerate()
                {
                    if model::contains(chip_rect(s, n), screen.mouse) {
                        s.preferences.toggle(i);
                        save(game, s);
                        break;
                    }
                }
            }
            return;
        }
        s.search_focus = false;
        if button == 0 {
            if let Some((c, _, _)) = controls(s.size)
                .into_iter()
                .find(|(_, r, _)| model::contains(*r, screen.mouse))
            {
                control = Some(c);
                return;
            }
        }
        if !model::contains(s.rect, screen.mouse) {
            return;
        }
        if button == 0 {
            s.drag = Some((screen.mouse, s.views[s.selected].center));
        }
        if button == 1 {
            let double = s.right_click.as_ref().is_some_and(|(t, i, p)| {
                t.elapsed().as_millis() <= 400
                    && *i == s.selected
                    && (p[0] - screen.mouse[0]).hypot(p[1] - screen.mouse[1]) < 5.0
            });
            s.right_click = Some((std::time::Instant::now(), s.selected, screen.mouse));
            if double
                && s.image
                    .as_ref()
                    .is_some_and(|im| im.query.dimension == s.selected && im.panel.is_some())
            {
                let at = model::world(s.views[s.selected], s.rect, screen.mouse);
                point = Some((
                    model::DIMENSIONS[s.selected].to_string(),
                    [at[0], 64.0, at[1]],
                ));
                s.right_click = None;
            }
        }
    });
    if let Some(c) = control {
        act(game, c);
    }
    if let Some((dim, at)) = point {
        terrain::seed_point(dim, at);
    }
    true
}
pub(super) fn scroll(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    amount: f64,
) -> bool {
    if !active() {
        return false;
    }
    state(|s| {
        if amount == 0.0 {
            return;
        }
        if s.filters && model::contains(filter_rect(s.size), screen.mouse) {
            let count = if s.biome_tab {
                biome_indices(game, s).len()
            } else {
                model::filter_indices(s.selected, s.group, &s.search).len()
            };
            let max = count.div_ceil(4).saturating_sub(chip_rows(s));
            s.offset = if amount > 0.0 {
                s.offset.saturating_sub(1)
            } else {
                (s.offset + 1).min(max)
            };
            return;
        }
        if model::contains(s.rect, screen.mouse) {
            let view = &mut s.views[s.selected];
            let before = model::world(*view, s.rect, screen.mouse);
            view.blocks_per_pixel = (view.blocks_per_pixel * if amount > 0.0 { 0.5 } else { 2.0 })
                .clamp(0.25, 524288.0);
            let after = model::world(*view, s.rect, screen.mouse);
            for n in 0..2 {
                view.center[n] += before[n] - after[n];
            }
            view.constrain();
        }
    });
    true
}
pub(super) fn character(c: char) -> bool {
    state(|s| {
        if !s.active || !s.search_focus {
            return false;
        }
        if !c.is_control() && s.search.chars().count() < 64 {
            s.search.push(c);
            s.offset = 0;
        }
        true
    })
}
pub(super) fn key(game: &Game<'_>, key: &str) -> bool {
    if !active() {
        return false;
    }
    if state(|s| {
        if s.search_focus {
            match key {
                "key.keyboard.escape" | "key.keyboard.enter" => s.search_focus = false,
                "key.keyboard.backspace" => {
                    s.search.pop();
                    s.offset = 0;
                }
                _ => {}
            }
            true
        } else {
            false
        }
    }) {
        return true;
    }
    if key == game.native_mod_key(claims::ID, "map", "key.keyboard.m") {
        terrain::seed_close(game);
    } else if key == "key.keyboard.escape" {
        if state(|s| {
            if s.filters {
                s.filters = false;
                true
            } else {
                false
            }
        }) {
            return true;
        }
        terrain::seed_close(game);
    } else if key == game.native_mod_key(claims::ID, "menu", "key.keyboard.j") {
        terrain::seed_settings(game);
    } else {
        match key {
            "key.keyboard.equal" => act(game, Control::Zoom(0.5)),
            "key.keyboard.minus" => act(game, Control::Zoom(2.0)),
            "key.keyboard.home" => act(game, Control::Center),
            "key.keyboard.f" => act(game, Control::Filters),
            _ => {}
        }
    }
    true
}
