// Three independent map viewports and a scrollable, persistent layer sidebar.
use crate::game::{
    Game, claim_borders as claims, claim_borders_map as map, claim_borders_map_view as terrain,
    claim_borders_seed_map as model, claim_borders_seed_worker as worker,
    claim_borders_ui_view as ui,
};
use horizon_client::hud::HudBatch;
use std::sync::{Mutex, OnceLock};
struct State {
    active: bool,
    loaded: bool,
    preferences: model::Preferences,
    views: [map::View; 3],
    selected: usize,
    drag: Option<(usize, [f32; 2], [f64; 2])>,
    rects: [[f32; 4]; 3],
    size: [f32; 2],
    offset: usize,
    scope: String,
    worker: Option<worker::Handle>,
    requested: Option<worker::Query>,
    image: Option<worker::Image>,
    error: Option<String>,
    right_click: Option<(std::time::Instant, usize, [f32; 2])>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            active: true,
            loaded: false,
            preferences: model::Preferences::default(),
            views: [map::View {
                center: [0.0; 2],
                blocks_per_pixel: 16.0,
            }; 3],
            selected: 0,
            drag: None,
            rects: [[0.0; 4]; 3],
            size: [0.0; 2],
            offset: 0,
            scope: String::new(),
            worker: None,
            requested: None,
            image: None,
            error: None,
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
        s.image = None;
        s.requested = None;
        s.drag = None;
        s.right_click = None;
        s.scope.clear();
    });
}
pub(super) fn open(source: &map::Snapshot, focus: &Option<(String, i32, i32)>) {
    state(|s| {
        s.active = focus.is_none();
        s.drag = None;
        s.right_click = None;
        let nether = source.dimension == model::DIMENSIONS[1];
        s.views = std::array::from_fn(|i| {
            let factor = if nether && i == 0 {
                8.0
            } else if !nether && i == 1 {
                0.125
            } else {
                1.0
            };
            let mut view = map::View {
                center: [source.player[0] * factor, source.player[2] * factor],
                blocks_per_pixel: 16.0,
            };
            view.constrain();
            view
        });
        s.selected = model::DIMENSIONS
            .iter()
            .position(|d| *d == source.dimension)
            .unwrap_or(0);
    });
}
fn text(
    game: &Game<'_>,
    batch: &mut HudBatch,
    caption: &str,
    pos: [f32; 2],
    width: f32,
    color: u32,
) {
    terrain::label(game, batch, caption, pos, width, color);
}
fn sidebar(size: [f32; 2]) -> [f32; 4] {
    [size[0] - 126.0, 53.0, 120.0, (size[1] - 91.0).max(1.0)]
}
fn controls(size: [f32; 2]) -> Vec<([f32; 4], &'static str)> {
    [
        "−",
        "+",
        "К себе",
        "Исследованная",
        "Настройки J",
        "Закрыть M",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, t)| {
        (
            [
                6.0 + i as f32 * ((size[0] - 12.0) / 6.0),
                size[1] - 23.0,
                (size[0] - 12.0) / 6.0 - 3.0,
                17.0,
            ],
            t,
        )
    })
    .collect()
}
fn rows(s: &State) -> Vec<([f32; 4], usize, String)> {
    let r = sidebar(s.size);
    let count = ((r[3] - 4.0) / 16.0).max(0.0) as usize;
    let mut captions = vec![
        format!(
            "{} Биомы · Y64",
            if s.preferences.biomes { "☑" } else { "☐" }
        ),
        format!(
            "Биом: {}",
            s.preferences
                .biome
                .as_deref()
                .unwrap_or("Все")
                .strip_prefix("minecraft:")
                .unwrap_or(s.preferences.biome.as_deref().unwrap_or("Все"))
        ),
        format!("{} Claims", if s.preferences.claims { "☑" } else { "☐" }),
        if s.preferences.structures == 0 {
            "☐ Все структуры".into()
        } else {
            "☑ Все структуры".into()
        },
    ];
    captions.extend(
        model::STRUCTURES
            .iter()
            .enumerate()
            .map(|(i, (_, name, _))| {
                format!(
                    "{} {name}",
                    if s.preferences.structures & (1 << i) != 0 {
                        "☑"
                    } else {
                        "☐"
                    }
                )
            }),
    );
    captions
        .into_iter()
        .enumerate()
        .skip(s.offset)
        .take(count)
        .enumerate()
        .map(|(row, (id, t))| {
            (
                [r[0] + 2.0, r[1] + 2.0 + row as f32 * 16.0, r[2] - 4.0, 15.0],
                id,
                t,
            )
        })
        .collect()
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
            s.preferences = game
                .native_setting(claims::ID, "seed_map")
                .and_then(|raw| model::Preferences::decode(&raw).ok())
                .unwrap_or_default();
            s.loaded = true;
        }
        s.size = [screen.rect[2], screen.rect[3]];
        s.rects = model::panel_rects(s.size);
        if source.scope != s.scope {
            if let Some(w) = &s.worker {
                w.cancel();
            }
            s.image = None;
            s.requested = None;
            s.scope = source.scope.clone();
            s.error = None;
        }
        if let Some((i, mouse, center)) = s.drag {
            let scale = f64::from(128.0 * s.views[i].blocks_per_pixel / s.rects[i][3]);
            s.views[i].center = [
                center[0] - f64::from(screen.mouse[0] - mouse[0]) * scale,
                center[1] - f64::from(screen.mouse[1] - mouse[1]) * scale,
            ];
            s.views[i].constrain();
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
        let query = worker::Query {
            scope: source.scope.clone(),
            views: s.views,
            aspects: s.rects.map(|r| r[2] / r[3]),
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
                if image.error.is_none() {
                    if let Some(rect) = game
                        .gui
                        .atlas
                        .sprites
                        .get("claim_borders/seed_map")
                        .copied()
                    {
                        let mut pixels = vec![0; 256 * 256 * 4];
                        for (i, panel) in image.panels.iter().enumerate() {
                            for y in 0..worker::SIDE {
                                let dest = (((i / 2) * 128 + y) * 256 + (i % 2) * 128) * 4;
                                pixels[dest..dest + 128 * 4]
                                    .copy_from_slice(&panel.pixels[y * 128 * 4..(y + 1) * 128 * 4]);
                            }
                        }
                        // Patch queue is drained after native rendering on this same game thread.
                        terrain::seed_patch(game, rect, pixels);
                    }
                }
                s.image = Some(image);
            }
        }
        let start = batch.vertices.len();
        ui::fill(game, batch, screen.rect, 0xff101720);
        text(
            game,
            batch,
            "DRAIMHORIZON / SEED MAP",
            [8.0, 8.0],
            s.size[0] * 0.5,
            0xe7edf4,
        );
        text(
            game,
            batch,
            &format!("Java 26.3 · {}", model::SEED),
            [s.size[0] * 0.5, 8.0],
            s.size[0] * 0.5 - 6.0,
            0x93b6c5,
        );
        text(
            game,
            batch,
            "ЛКМ — перемещение · колесо — масштаб · 2×ПКМ — метка (Y64) · фильтры справа ↕",
            [8.0, 23.0],
            s.size[0] - 16.0,
            0x8397aa,
        );
        let mut footer = if s.error.is_some() {
            format!(
                "Seed Map недоступна: {} · доступна исследованная карта",
                s.error.as_deref().unwrap_or_default()
            )
        } else if s.image.as_ref().is_none_or(|im| im.query != query) {
            "Расчёт карты…".into()
        } else {
            "Прогноз vanilla · серверные datapack и старые чанки могут отличаться".into()
        };
        for i in 0..3 {
            let r = s.rects[i];
            let dim = model::DIMENSIONS[i];
            let view = s.views[i];
            let aspect = r[2] / r[3];
            text(
                game,
                batch,
                map::dimension_label(dim),
                [r[0] + 3.0, r[1] - 13.0],
                r[2],
                if s.selected == i { 0x7ddbc1 } else { 0xb6c5d3 },
            );
            ui::fill(game, batch, r, 0xff1b2531);
            let displayed = s
                .image
                .as_ref()
                .filter(|im| im.error.is_none() && im.query.preferences == s.preferences);
            if let Some(image) = displayed {
                if let Some(mut uv) = game
                    .gui
                    .atlas
                    .sprites
                    .get("claim_borders/seed_map")
                    .copied()
                {
                    let du = (uv.u1 - uv.u0) / 2.0;
                    let dv = (uv.v1 - uv.v0) / 2.0;
                    uv.u0 += (i % 2) as f32 * du;
                    uv.u1 = uv.u0 + du;
                    uv.v0 += (i / 2) as f32 * dv;
                    uv.v1 = uv.v0 + dv;
                    let old_view = image.query.views[i];
                    let old_aspect = image.query.aspects[i];
                    let corners = [[0.0, 0.0], [128.0, 128.0]].map(|xy| {
                        let world = old_view.world_aspect(xy, 128.0, old_aspect);
                        let xy = view.pixel_aspect(world, 128.0, aspect);
                        [r[0] + xy[0] / 128.0 * r[2], r[1] + xy[1] / 128.0 * r[3]]
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
                if image.panels[i].limited {
                    text(
                        game,
                        batch,
                        "Структуры: приблизьте",
                        [r[0] + 3.0, r[1] + 14.0],
                        r[2] - 6.0,
                        0xf1c77c,
                    );
                }
                let marks_start = batch.vertices.len();
                for mark in &image.panels[i].marks {
                    let at = view.pixel_aspect(mark.position, 128.0, aspect);
                    let at = [r[0] + at[0] / 128.0 * r[2], r[1] + at[1] / 128.0 * r[3]];
                    if !model::contains(r, at) {
                        continue;
                    }
                    let index = model::structure_index(&mark.id).unwrap_or(0);
                    ui::fill(
                        game,
                        batch,
                        [at[0] - 3.0, at[1] - 3.0, 6.0, 6.0],
                        0xff111923,
                    );
                    ui::fill(
                        game,
                        batch,
                        [at[0] - 2.0, at[1] - 2.0, 4.0, 4.0],
                        0xff000000 | model::STRUCTURES[index].2,
                    );
                    if (screen.mouse[0] - at[0]).abs() < 5.0
                        && (screen.mouse[1] - at[1]).abs() < 5.0
                    {
                        footer = format!(
                            "{} · {} · X {:.0} Z {:.0}",
                            model::STRUCTURES[index].1,
                            map::dimension_label(dim),
                            mark.position[0],
                            mark.position[1]
                        );
                    }
                }
                batch.clip_since(marks_start, r[0], r[1], r[0] + r[2], r[1] + r[3]);
                if model::contains(r, screen.mouse)
                    && image.query.views[i] == view
                    && image.query.aspects[i] == aspect
                {
                    let at = model::world(view, r, screen.mouse);
                    let x = (((screen.mouse[0] - r[0]) / r[2] * 128.0) as usize).min(127);
                    let y = (((screen.mouse[1] - r[1]) / r[3] * 128.0) as usize).min(127);
                    let biome = &image.panels[i].biomes[y * 128 + x];
                    text(
                        game,
                        batch,
                        &format!("X {:.0} Z {:.0}", at[0].floor(), at[1].floor()),
                        [r[0] + 4.0, r[1] + r[3] - 25.0],
                        r[2] - 8.0,
                        0xffffff,
                    );
                    if !image.panels[i].marks.iter().any(|m| {
                        let xy = view.pixel_aspect(m.position, 128.0, aspect);
                        ((xy[0] - (screen.mouse[0] - r[0]) / r[2] * 128.0) * r[2] / 128.0).abs()
                            < 5.0
                            && ((xy[1] - (screen.mouse[1] - r[1]) / r[3] * 128.0) * r[3] / 128.0)
                                .abs()
                                < 5.0
                    }) {
                        footer = format!(
                            "{} · Y64 · {biome} · X {:.0} Z {:.0}",
                            map::dimension_label(dim),
                            at[0].floor(),
                            at[1].floor()
                        );
                    }
                }
            } else {
                text(
                    game,
                    batch,
                    "Расчёт…",
                    [r[0] + 8.0, r[1] + 8.0],
                    r[2] - 16.0,
                    0x8fa6b9,
                );
            }
            if s.preferences.claims {
                draw_claims(game, batch, &source, dim, view, r, p);
            }
            terrain::map_marks(game, batch, &source, dim, view, r, p, true, aspect);
            terrain::map_frame(game, batch, r, view, aspect, false);
        }
        let side = sidebar(s.size);
        ui::fill(game, batch, side, 0xff192330);
        text(
            game,
            batch,
            "СЛОИ / ФИЛЬТРЫ",
            [side[0] + 3.0, side[1] - 13.0],
            side[2],
            0xb6c5d3,
        );
        for (rect, _, caption) in rows(s) {
            ui::button(game, batch, rect, &caption, false, screen.mouse);
        }
        text(
            game,
            batch,
            &footer,
            [8.0, s.size[1] - 35.0],
            s.size[0] - 16.0,
            if s.error.is_some() {
                0xf1b07e
            } else {
                0x93a8ba
            },
        );
        for (rect, caption) in controls(s.size) {
            ui::button(game, batch, rect, caption, false, screen.mouse);
        }
        ui::finish(
            game,
            batch,
            start,
            "seed-map",
            &format!("{}:{}", model::SEED, s.selected),
        );
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
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    pressed: bool,
) -> bool {
    if !active() {
        return false;
    }
    let mut control = None;
    let mut point = None;
    state(|s| {
        if !pressed {
            s.drag = None;
            return;
        }
        if button == 0 {
            if let Some((i, _)) = controls(s.size)
                .iter()
                .enumerate()
                .find(|(_, (r, _))| model::contains(*r, screen.mouse))
            {
                control = Some(i);
                return;
            }
            if let Some((_, id, _)) = rows(s)
                .into_iter()
                .find(|(r, _, _)| model::contains(*r, screen.mouse))
            {
                match id {
                    0 => s.preferences.biomes = !s.preferences.biomes,
                    1 => {
                        let catalog = s
                            .image
                            .as_ref()
                            .map(|im| im.catalog.as_slice())
                            .unwrap_or(&[]);
                        s.preferences.biome = match s
                            .preferences
                            .biome
                            .as_ref()
                            .and_then(|b| catalog.iter().position(|v| v == b))
                        {
                            Some(i) => catalog.get(i + 1).cloned(),
                            None => catalog.first().cloned(),
                        };
                    }
                    2 => s.preferences.claims = !s.preferences.claims,
                    3 => {
                        s.preferences.structures = if s.preferences.structures == 0 {
                            model::Preferences::default().structures
                        } else {
                            0
                        }
                    }
                    _ => s.preferences.structures ^= 1 << (id - 4),
                }
                game.native_set_setting(claims::ID, "seed_map", &s.preferences.encode());
                return;
            }
        }
        if let Some(i) = s
            .rects
            .iter()
            .position(|r| model::contains(*r, screen.mouse))
        {
            s.selected = i;
            if button == 0 {
                s.drag = Some((i, screen.mouse, s.views[i].center));
            }
            if button == 1 {
                let double = s.right_click.as_ref().is_some_and(|(t, old, at)| {
                    t.elapsed().as_millis() <= 400
                        && *old == i
                        && (at[0] - screen.mouse[0]).hypot(at[1] - screen.mouse[1]) < 5.0
                });
                s.right_click = Some((std::time::Instant::now(), i, screen.mouse));
                if double
                    && s.image
                        .as_ref()
                        .is_some_and(|im| im.error.is_none() && im.query.views[i] == s.views[i])
                {
                    let at = model::world(s.views[i], s.rects[i], screen.mouse);
                    point = Some((model::DIMENSIONS[i].to_string(), [at[0], 64.0, at[1]]));
                    s.right_click = None;
                }
            }
        }
    });
    if let Some((dim, at)) = point {
        terrain::seed_point(dim, at);
    }
    if let Some(control) = control {
        match control {
            0 => zoom(2.0),
            1 => zoom(0.5),
            2 => {
                if let Some(source) = terrain::seed_source() {
                    open(&source, &None);
                }
            }
            3 => state(|s| {
                s.active = false;
                s.drag = None;
                if let Some(w) = &s.worker {
                    w.cancel();
                }
            }),
            4 => terrain::seed_settings(game),
            5 => terrain::seed_close(game),
            _ => {}
        }
    }
    true
}
fn zoom(factor: f32) {
    state(|s| {
        s.views[s.selected].zoom_at([64.0, 64.0], 128.0, factor);
    });
}
pub(super) fn scroll(
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    amount: f64,
) -> bool {
    if !active() {
        return false;
    }
    state(|s| {
        if model::contains(sidebar(s.size), screen.mouse) {
            let visible = ((sidebar(s.size)[3] - 4.0) / 16.0).max(0.0) as usize;
            let max = (model::STRUCTURES.len() + 4).saturating_sub(visible);
            s.offset = if amount > 0.0 {
                s.offset.saturating_sub(1)
            } else {
                s.offset.saturating_add(1).min(max)
            };
            return;
        }
        if amount == 0.0 {
            return;
        }
        if let Some(i) = s
            .rects
            .iter()
            .position(|r| model::contains(*r, screen.mouse))
        {
            let before = model::world(s.views[i], s.rects[i], screen.mouse);
            s.views[i].blocks_per_pixel = (s.views[i].blocks_per_pixel
                * if amount > 0.0 { 0.5 } else { 2.0 })
            .clamp(0.25, 524288.0);
            let after = model::world(s.views[i], s.rects[i], screen.mouse);
            for n in 0..2 {
                s.views[i].center[n] += before[n] - after[n];
            }
            s.views[i].constrain();
            s.selected = i;
        }
    });
    true
}
pub(super) fn key(game: &Game<'_>, key: &str) -> bool {
    if !active() {
        return false;
    }
    if key == game.native_mod_key(claims::ID, "map", "key.keyboard.m")
        || key == "key.keyboard.escape"
    {
        terrain::seed_close(game);
    } else if key == game.native_mod_key(claims::ID, "menu", "key.keyboard.j") {
        terrain::seed_settings(game);
    } else {
        match key {
            "key.keyboard.equal" => zoom(0.5),
            "key.keyboard.minus" => zoom(2.0),
            "key.keyboard.home" => {
                if let Some(source) = terrain::seed_source() {
                    open(&source, &None);
                }
            }
            _ => {}
        }
    }
    true
}
