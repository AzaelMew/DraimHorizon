use crate::game::claim_borders_ui_view as ui;
// DraimHorizons economy workspace; server transactions remain explicit local actions.
use crate::game::{
    Game, claim_borders as claims, claim_borders_economy as model,
    claim_borders_economy_client as data, claim_borders_economy_scan as scan,
    claim_borders_economy_ui::{Panel, Sort},
    claim_borders_menu as edit,
};
use horizon_client::hud::HudBatch;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Clone)]
pub enum Action {
    Setting(&'static str, i64),
    Defaults,
    Refresh(Option<String>),
    Worth(String),
    Watch(String),
    OpenMarket,
    OpenAuction(Option<String>),
    Scan(scan::Kind),
    Stop,
    PrepareSell,
    ConfirmSell,
    CancelSell,
    Close,
    Copy(String),
    Paste(u64),
}
#[derive(Clone)]
enum Hit {
    Tab(u8),
    SortMenu,
    SortPick(Sort),
    Close,
    Search,
    Target,
    Row(String, Option<u64>),
    Page(i32),
    Group(u8),
    Adjust(&'static str, i64),
    Run(Action),
}
struct State {
    open: bool,
    tab: u8,
    sort: [Sort; 6],
    sort_menu: bool,
    page: usize,
    group: u8,
    settings_page: usize,
    item: Option<String>,
    listing: Option<u64>,
    search: edit::Menu,
    target: edit::Menu,
    focus: u8,
    token: u64,
    hits: Vec<([f32; 4], Hit)>,
    viewport: [f32; 2],
    queue: VecDeque<Action>,
    notice: String,
    suppress: Option<(char, Instant)>,
}
fn editor() -> edit::Menu {
    edit::Menu {
        focus: Some(edit::Field::Name),
        name: String::new(),
        ..edit::Menu::default()
    }
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            tab: 0,
            sort: [
                Sort::Name,
                Sort::Name,
                Sort::Unit,
                Sort::Profit,
                Sort::Unit,
                Sort::Name,
            ],
            sort_menu: false,
            page: 0,
            group: 0,
            settings_page: 0,
            item: None,
            listing: None,
            search: editor(),
            target: editor(),
            focus: 0,
            token: 0,
            hits: Vec::new(),
            viewport: [0.0; 2],
            queue: VecDeque::new(),
            notice: String::new(),
            suppress: None,
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
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn open() {
    state(|s| {
        s.open = true;
        s.queue.push_back(Action::Refresh(s.item.clone()));
    });
}
pub(super) fn close() {
    state(|s| {
        s.open = false;
        s.hits.clear();
        s.focus = 0;
    });
}
pub(super) fn visible() -> bool {
    state(|s| s.open)
}
pub(super) fn notice(text: String) {
    state(|s| s.notice = text);
}
pub(super) fn actions() -> Vec<Action> {
    state(|s| s.queue.drain(..).collect())
}
pub(super) fn paste(token: u64, text: &str) {
    state(|s| {
        if s.token != token || !s.open {
            return;
        }
        match s.focus {
            1 => s.search.paste(text),
            2 => s.target.paste(text),
            _ => {}
        }
    });
}
fn add(s: &mut State, action: Action) {
    if s.queue.len() < 32 {
        s.queue.push_back(action);
    }
}
fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
pub(super) fn label(
    game: &Game<'_>,
    batch: &mut HudBatch,
    text: &str,
    at: [f32; 2],
    width: f32,
    color: u32,
) {
    let mut shown = String::new();
    for c in text.chars() {
        shown.push(c);
        if game.native_text_width(&shown) > width - 7.0 {
            shown.pop();
            shown.push('…');
            break;
        }
    }
    ui::text(game, batch, &shown, at, color, false);
}
fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    rect: [f32; 4],
    text: &str,
    hit: Hit,
    selected: bool,
    mouse: [f32; 2],
) {
    ui::button(game,batch,rect,text,selected,mouse);
    s.hits.push((rect, hit));
}
fn age(at: u64, now: u64) -> String {
    if at == 0 {
        "нет данных".into()
    } else {
        format!("{} с назад", now.saturating_sub(at) / 1000)
    }
}
fn item_label(id: &str) -> String {
    id.strip_prefix("minecraft:")
        .unwrap_or(id)
        .replace('_', " ")
}
fn value(v: Option<model::Coins>) -> String {
    v.map_or("—".into(), model::fmt)
}
fn line(
    game: &Game<'_>,
    batch: &mut HudBatch,
    x: f32,
    y: &mut f32,
    width: f32,
    text: &str,
    color: u32,
) {
    label(game, batch, text, [x, *y], width, color);
    *y += 12.0;
}
fn spark(game: &Game<'_>, batch: &mut HudBatch, history: &[(u64, model::Coins)], rect: [f32; 4]) {
    ui::fill(game, batch, rect, 0xff101a25);
    if history.len() < 2 {
        return;
    }
    let lo = history.iter().map(|(_, p)| *p).min().unwrap_or(0);
    let hi = history.iter().map(|(_, p)| *p).max().unwrap_or(1);
    let span = (hi - lo).max(1) as f32;
    let points: Vec<_> = history
        .iter()
        .enumerate()
        .map(|(i, (_, p))| {
            [
                rect[0] + i as f32 / (history.len() - 1) as f32 * rect[2],
                rect[1] + rect[3] - 3.0 - (*p - lo) as f32 / span * (rect[3] - 6.0),
            ]
        })
        .collect();
    for p in points.windows(2) {
        batch.fill_quad(
            &game.gui.atlas,
            [
                [p[0][0], p[0][1] - 0.6],
                [p[1][0], p[1][1] - 0.6],
                [p[1][0], p[1][1] + 0.6],
                [p[0][0], p[0][1] + 0.6],
            ],
            0xff86ceb8,
        );
    }
}
pub(super) fn render(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    let panel = Panel::new([batch.width, batch.height]);
    let [ox, oy, w, h] = panel.rect;
    let local = crate::game::native_hooks::NativeScreenView {
        id: screen.id,
        menu: screen.menu,
        title: screen.title,
        rect: [0.0, 0.0, w, h],
        slots: screen.slots,
        mouse: panel.local(screen.mouse),
        partial_tick: screen.partial_tick,
    };
    let screen = &local;
    let start = batch.vertices.len();
    let now = data::now();
    state(|s| {
        s.hits.clear();
        s.viewport = [batch.width, batch.height];
        ui::fill(game, batch, [-2.0, -2.0, w + 4.0, h + 4.0], 0xaa071017);
        ui::fill(game, batch, screen.rect, 0xf0182430);
        ui::fill(game, batch, [0.0, 0.0, w, 2.0], 0xff6bbdb1);
        label(
            game,
            batch,
            "DRAIMHORIZONS / ЭКОНОМИКА",
            [8.0, 10.0],
            w - 42.0,
            0xe4edf6,
        );
        button(
            game,
            batch,
            s,
            [w - 28.0, 5.0, 20.0, 18.0],
            "X",
            Hit::Close,
            false,
            screen.mouse,
        );
        let tabs = ["Обзор", "Цены", "AH", "Профит", "План", "Опции"];
        let tw = (w - 16.0) / tabs.len() as f32;
        for (i, text) in tabs.iter().enumerate() {
            button(
                game,
                batch,
                s,
                [8.0 + i as f32 * tw, 30.0, tw - 2.0, 18.0],
                text,
                Hit::Tab(i as u8),
                s.tab == i as u8,
                screen.mouse,
            );
        }
        data::read(|book, p, status, own| {
            if s.tab == 5 {
                let groups = ["Модули", "HUD", "Сбор", "Расчёт", "Сборы", "Sidebar", "Tab"];
                let gw = (w - 16.0) / groups.len() as f32;
                for (i, g) in groups.iter().enumerate() {
                    button(
                        game,
                        batch,
                        s,
                        [8.0 + i as f32 * gw, 56.0, gw - 2.0, 18.0],
                        g,
                        Hit::Group(i as u8),
                        s.group == i as u8,
                        screen.mouse,
                    );
                }
                let settings: Vec<_> = model::SETTINGS
                    .iter()
                    .filter(|v| v.group == s.group)
                    .collect();
                let per = (((h - 130.0) / 24.0).floor() as usize).clamp(2, 12);
                let pages = settings.len().div_ceil(per).max(1);
                s.settings_page = s.settings_page.min(pages - 1);
                for (i, setting) in settings
                    .iter()
                    .skip(s.settings_page * per)
                    .take(per)
                    .enumerate()
                {
                    let y = 82.0 + i as f32 * 24.0;
                    let number = p.get(setting.key);
                    let text = if setting.max == 1 {
                        format!(
                            "{}: {}",
                            setting.label,
                            if number != 0 { "вкл." } else { "выкл." }
                        )
                    } else {
                        format!("{}: {number}", setting.label)
                    };
                    label(game, batch, &text, [12.0, y + 5.0], w - 88.0, 0xd3dee8);
                    button(
                        game,
                        batch,
                        s,
                        [w - 68.0, y, 26.0, 18.0],
                        "−",
                        Hit::Adjust(setting.key, -1),
                        false,
                        screen.mouse,
                    );
                    button(
                        game,
                        batch,
                        s,
                        [w - 38.0, y, 26.0, 18.0],
                        "+",
                        Hit::Adjust(setting.key, 1),
                        false,
                        screen.mouse,
                    );
                }
                button(
                    game,
                    batch,
                    s,
                    [8.0, h - 48.0, 35.0, 18.0],
                    "←",
                    Hit::Page(-1),
                    false,
                    screen.mouse,
                );
                label(
                    game,
                    batch,
                    &format!("{} / {pages}", s.settings_page + 1),
                    [50.0, h - 43.0],
                    90.0,
                    0xd1dce7,
                );
                button(
                    game,
                    batch,
                    s,
                    [112.0, h - 48.0, 35.0, 18.0],
                    "→",
                    Hit::Page(1),
                    false,
                    screen.mouse,
                );
                button(
                    game,
                    batch,
                    s,
                    [w - 160.0, h - 48.0, 150.0, 18.0],
                    "Сброс опций экономики",
                    Hit::Run(Action::Defaults),
                    false,
                    screen.mouse,
                );
            } else if s.tab == 0 {
                let width = w - 24.0;
                let x = 12.0;
                let mut y = 66.0;
                line(game, batch, x, &mut y, width, "Рынок и аукцион", 0xe1eaf1);
                line(
                    game,
                    batch,
                    x,
                    &mut y,
                    width,
                    &format!(
                        "Цены: {} товаров · Аукцион: {} лотов",
                        book.quotes
                            .values()
                            .filter(|q| q.sell.is_some() || q.buy.is_some())
                            .count(),
                        book.listings.len()
                    ),
                    0x94b5ca,
                );
                line(
                    game,
                    batch,
                    x,
                    &mut y,
                    width,
                    &format!(
                        "Лимит продаж: осталось {} из {} монет",
                        value(book.coin_left.map(|(v, _)| v)),
                        value(book.coin_cap)
                    ),
                    0xe7ce95,
                );
                line(
                    game,
                    batch,
                    x,
                    &mut y,
                    width,
                    "Это максимум выручки от продаж серверу до полуночи UTC.",
                    0x8fa9bb,
                );
                line(
                    game,
                    batch,
                    x,
                    &mut y,
                    width,
                    &format!(
                        "Лимит каждого товара: до {} шт. в сутки",
                        book.item_cap.map_or("?".into(), |v| v.to_string())
                    ),
                    0x94b5ca,
                );
                y += 8.0;
                let deals = book.opportunities(p, now, own);
                let best = deals
                    .iter()
                    .filter(|o| o.full_profit > 0)
                    .max_by_key(|o| o.full_profit);
                if let Some(o) = best {
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        &format!(
                            "Лучшая оценка AH → Market: {} · {} монет за лот",
                            o.listing.label,
                            model::delta(o.full_profit)
                        ),
                        0x8edbb8,
                    );
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        &format!(
                            "По лимитам сейчас можно продать {} из {} шт. · подробнее в Профит",
                            o.quantity, o.listing.count
                        ),
                        0x94b5ca,
                    );
                } else {
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        "Откройте AH и Market или нажмите «Обновить цены».",
                        0x94b5ca,
                    );
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        "В Профит видны расчёты и причины, мешающие продаже.",
                        0x94b5ca,
                    );
                }
                if status.sale.is_none() {
                    let bw = (w - 22.0) / 4.0;
                    for (i, (text, action)) in [
                        (
                            "Обновить",
                            Action::Refresh(status.hand.as_ref().map(|h| h.item.clone())),
                        ),
                        ("Market", Action::OpenMarket),
                        ("Аукцион", Action::OpenAuction(None)),
                        ("Проверить руку", Action::PrepareSell),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        button(
                            game,
                            batch,
                            s,
                            [8.0 + i as f32 * (bw + 2.0), h - 48.0, bw, 18.0],
                            text,
                            Hit::Run(action),
                            false,
                            screen.mouse,
                        );
                    }
                }
                if let Some(hand) = &status.sale {
                    let text = data::sale_estimate(book, p, hand, now).map_or(
                        "Нет свежей цены/лимита для подтверждения".into(),
                        |(n, net)| {
                            format!(
                                "{} ×{}: оценка +{} · повторное нажатие продаёт",
                                item_label(&hand.item),
                                n,
                                model::fmt(net)
                            )
                        },
                    );
                    label(game, batch, &text, [8.0, h - 66.0], w - 16.0, 0xf0c18b);
                    button(
                        game,
                        batch,
                        s,
                        [8.0, h - 48.0, 110.0, 18.0],
                        "Отмена",
                        Hit::Run(Action::CancelSell),
                        false,
                        screen.mouse,
                    );
                    button(
                        game,
                        batch,
                        s,
                        [w - 178.0, h - 48.0, 170.0, 18.0],
                        "ПОДТВЕРДИТЬ /sell hand",
                        Hit::Run(Action::ConfirmSell),
                        false,
                        screen.mouse,
                    );
                }
            } else {
                let lw = ((w - 24.0) * 0.44).max(110.0);
                let rx = lw + 20.0;
                let rw = w - rx - 8.0;
                let search = if s.search.name.is_empty() {
                    "Поиск товара / ника".into()
                } else {
                    format!("{}{}", s.search.name, if s.focus == 1 { "│" } else { "" })
                };
                button(
                    game,
                    batch,
                    s,
                    [8.0, 56.0, (lw - 86.0).max(30.0), 18.0],
                    &search,
                    Hit::Search,
                    s.focus == 1,
                    screen.mouse,
                );
                button(
                    game,
                    batch,
                    s,
                    [8.0 + lw - 82.0, 56.0, 82.0, 18.0],
                    s.sort[s.tab as usize].label(),
                    Hit::SortMenu,
                    s.sort_menu,
                    screen.mouse,
                );
                let filter = s.search.name.to_ascii_lowercase();
                let per = (((h - 154.0) / 20.0).floor() as usize).clamp(2, 16);
                let opportunities = if s.tab == 3 {
                    book.opportunities(p, now, own)
                } else {
                    Vec::new()
                };
                let opportunity_by_id: std::collections::BTreeMap<_, _> =
                    opportunities.iter().map(|o| (o.listing.id, o)).collect();
                let mut rows: Vec<(String, Option<u64>, String, String, bool)> = if s.tab == 1 {
                    book.quotes
                        .iter()
                        .filter(|(key, _)| {
                            item_label(key).contains(&filter) || key.contains(&filter)
                        })
                        .map(|(key, q)| {
                            (
                                key.clone(),
                                None,
                                item_label(key),
                                format!("Сдать {} · Купить {}", value(q.sell), value(q.buy)),
                                false,
                            )
                        })
                        .collect()
                } else if s.tab == 3 {
                    s.sort[s.tab as usize]
                        .listings(book, p, now, own, true)
                        .into_iter()
                        .filter(|l| {
                            item_label(&l.item).contains(&filter)
                                || l.seller.to_ascii_lowercase().contains(&filter)
                        })
                        .map(|l| {
                            let o = opportunity_by_id.get(&l.id).copied();
                            (
                                l.item.clone(),
                                Some(l.id),
                                format!("{} ×{}", l.label, l.count),
                                o.map_or_else(
                                    || {
                                        if book.unsupported.contains(&l.item)
                                            || book.quotes.get(&l.item).is_some_and(|q| {
                                                q.buy.is_some() && q.sell.is_none()
                                            })
                                        {
                                            "Market не покупает".into()
                                        } else {
                                            "Нет цены Market · ожидаем ответ".into()
                                        }
                                    },
                                    |o| {
                                        format!(
                                            "{} · {:.1}% · сегодня {} шт.{}",
                                            model::fmt(o.full_profit),
                                            if l.cost > 0 {
                                                o.full_profit as f64 / l.cost as f64 * 100.0
                                            } else {
                                                0.0
                                            },
                                            o.quantity,
                                            if o.ready { "" } else { " · ?" }
                                        )
                                    },
                                ),
                                o.is_some_and(|o| o.full_profit > 0),
                            )
                        })
                        .collect()
                } else {
                    s.sort[s.tab as usize]
                        .listings(book, p, now, own, false)
                        .into_iter()
                        .filter(|l| {
                            item_label(&l.item).contains(&filter)
                                || l.seller.to_ascii_lowercase().contains(&filter)
                        })
                        .map(|l| {
                            (
                                l.item.clone(),
                                Some(l.id),
                                format!("{} ×{}", l.label, l.count),
                                format!(
                                    "{:.2}/шт. · {} за лот · {}{}",
                                    l.cost as f64 / 100.0 / l.count.max(1) as f64,
                                    model::fmt(l.cost),
                                    l.seller,
                                    if l.bid { " · торги" } else { "" }
                                ),
                                false,
                            )
                        })
                        .collect()
                };
                if s.tab == 1 {
                    rows.sort_by(|a, b| {
                        let qa = book.quotes.get(&a.0);
                        let qb = book.quotes.get(&b.0);
                        let order = match s.sort[1] {
                            Sort::Unit | Sort::Total => qa
                                .and_then(|q| q.sell)
                                .unwrap_or(i64::MAX)
                                .cmp(&qb.and_then(|q| q.sell).unwrap_or(i64::MAX)),
                            Sort::Recent => qb.map(|q| q.at).cmp(&qa.map(|q| q.at)),
                            _ => a.2.cmp(&b.2),
                        };
                        order.then(a.0.cmp(&b.0))
                    });
                }
                let pages = rows.len().div_ceil(per).max(1);
                s.page = s.page.min(pages - 1);
                for (i, (item, id, title, subtitle, ready)) in
                    rows.iter().skip(s.page * per).take(per).enumerate()
                {
                    let y = 80.0 + i as f32 * 20.0;
                    let chosen = if id.is_some() {
                        s.listing == *id
                    } else {
                        s.item.as_ref() == Some(item)
                    };
                    ui::fill(game,
                        batch,
                        [8.0, y, lw, 19.0],
                        if chosen { 0xff354e62 } else { 0xff1c2a39 },
                    );
                    label(game, batch, title, [12.0, y + 2.0], lw - 8.0, 0xe0e8ef);
                    label(
                        game,
                        batch,
                        subtitle,
                        [12.0, y + 11.0],
                        lw - 8.0,
                        if *ready { 0x86d4ae } else { 0xc2b48e },
                    );
                    s.hits
                        .push(([8.0, y, lw, 19.0], Hit::Row(item.clone(), *id)));
                }
                if rows.is_empty() {
                    label(
                        game,
                        batch,
                        "Откройте AH / Market для сбора данных",
                        [10.0, 84.0],
                        lw - 4.0,
                        0x8196aa,
                    );
                }
                button(
                    game,
                    batch,
                    s,
                    [8.0, h - 72.0, 28.0, 18.0],
                    "←",
                    Hit::Page(-1),
                    false,
                    screen.mouse,
                );
                label(
                    game,
                    batch,
                    &format!("{} / {pages}", s.page + 1),
                    [42.0, h - 67.0],
                    lw - 76.0,
                    0xc6d3df,
                );
                button(
                    game,
                    batch,
                    s,
                    [8.0 + lw - 28.0, h - 72.0, 28.0, 18.0],
                    "→",
                    Hit::Page(1),
                    false,
                    screen.mouse,
                );
                let selected = s
                    .item
                    .clone()
                    .or_else(|| status.hand.as_ref().map(|h| h.item.clone()));
                let detail_start = batch.vertices.len();
                let detail_hits = s.hits.len();
                let mut y = 60.0;
                if let Some(item) = selected.as_ref() {
                    line(game, batch, rx, &mut y, rw, &item_label(item), 0xe4d29e);
                    if let Some(q) = book.quotes.get(item) {
                        line(
                            game,
                            batch,
                            rx,
                            &mut y,
                            rw,
                            &format!("Market покупает: {} / шт.", value(q.sell)),
                            0x91d1b3,
                        );
                        if s.tab == 1 {
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!("Market продаёт: {} / шт.", value(q.buy)),
                                0xcbd7e2,
                            );
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!("{} · {}", q.source, age(q.at, now)),
                                0x8ea3b7,
                            );
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!(
                                    "Осталось предметов: {}",
                                    q.left.map_or("?".into(), |(n, _)| n.to_string())
                                ),
                                0xcbd7e2,
                            );
                        }
                        if s.tab == 1 {
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!(
                                    "1ч {} · 24ч {}",
                                    q.hour.as_deref().unwrap_or("?"),
                                    q.day.as_deref().unwrap_or("?")
                                ),
                                0x8ea3b7,
                            );
                            let height = (h - y - 82.0).clamp(12.0, 65.0);
                            spark(game, batch, &q.history, [rx, y + 3.0, rw, height]);
                        }
                    }
                    if let Some(l) = s.listing.and_then(|id| book.listings.get(&id)) {
                        line(
                            game,
                            batch,
                            rx,
                            &mut y,
                            rw,
                            &format!("{}: {} / {} шт.", l.seller, model::fmt(l.cost), l.count),
                            0xd0dbe5,
                        );
                        if s.tab == 3 {
                            if let Some(o) = opportunity_by_id.get(&l.id).copied() {
                                line(
                                    game,
                                    batch,
                                    rx,
                                    &mut y,
                                    rw,
                                    &format!(
                                        "За весь лот: {} монет (оценка)",
                                        model::delta(o.full_profit)
                                    ),
                                    0x8edbb8,
                                );
                                line(
                                    game,
                                    batch,
                                    rx,
                                    &mut y,
                                    rw,
                                    &format!(
                                        "Сейчас продать {} шт. · выручка {}",
                                        o.quantity,
                                        model::fmt(o.gross)
                                    ),
                                    0xcbd7e2,
                                );
                                line(
                                    game,
                                    batch,
                                    rx,
                                    &mut y,
                                    rw,
                                    &format!("Выручка по лимитам: {} монет", model::fmt(o.gross)),
                                    0xcbd7e2,
                                );
                                line(
                                    game,
                                    batch,
                                    rx,
                                    &mut y,
                                    rw,
                                    &if o.quantity == 0 {
                                        "Сейчас продажа недоступна: лимиты / ограничения".into()
                                    } else {
                                        format!("Прибыль сейчас: {} монет", model::delta(o.profit))
                                    },
                                    if o.ready { 0x91d1b3 } else { 0xe2ba84 },
                                );
                                for note in o
                                    .notes
                                    .iter()
                                    .take(((h - y - 82.0) / 11.0).max(0.0) as usize)
                                {
                                    line(game, batch, rx, &mut y, rw, note, 0xc5a078);
                                }
                            }
                        } else if s.tab == 4 {
                            let target = if s.target.name.is_empty() {
                                "Цена будущей продажи AH".into()
                            } else {
                                format!("{} │", s.target.name)
                            };
                            button(
                                game,
                                batch,
                                s,
                                [rx, y + 2.0, rw, 18.0],
                                &target,
                                Hit::Target,
                                s.focus == 2,
                                screen.mouse,
                            );
                            y += 26.0;
                            if let Some(target) = model::money(&s.target.name) {
                                if let Some((fee, profit)) = book.resale(l.cost, target, p) {
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        &format!("Комиссия размещения {}", model::fmt(fee)),
                                        0xcbd7e2,
                                    );
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        &format!("Условная прибыль {}", model::fmt(profit)),
                                        0xe4c18b,
                                    );
                                } else {
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        "Уточните комиссии в Опции → Сборы",
                                        0xe4c18b,
                                    );
                                }
                            }
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                "Цена объявления не гарантирует продажу",
                                0x879bad,
                            );
                        } else {
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!(
                                    "{} · {}",
                                    if l.bid {
                                        "Аукционная ставка"
                                    } else {
                                        "Фиксированная цена"
                                    },
                                    age(l.seen, now)
                                ),
                                0x8ea3b7,
                            );
                        }
                    }
                    batch.clip_since(detail_start, rx, 55.0, w - 8.0, h - 79.0);
                    for i in (detail_hits..s.hits.len()).rev() {
                        let r = s.hits[i].0;
                        if r[1] < 55.0 || r[1] + r[3] > h - 79.0 {
                            s.hits.remove(i);
                        }
                    }
                    let item = item.clone();
                    let bw = (rw - 4.0) / 2.0;
                    button(
                        game,
                        batch,
                        s,
                        [rx, h - 72.0, bw, 18.0],
                        "Цена /worth",
                        Hit::Run(Action::Worth(item.clone())),
                        false,
                        screen.mouse,
                    );
                    button(
                        game,
                        batch,
                        s,
                        [rx + bw + 4.0, h - 72.0, bw, 18.0],
                        if book.watch.contains(&item) {
                            "Не следить"
                        } else {
                            "Следить"
                        },
                        Hit::Run(Action::Watch(item.clone())),
                        book.watch.contains(&item),
                        screen.mouse,
                    );
                }
                let bw = (w - 22.0) / 4.0;
                let selected = s.item.clone();
                for (i, (text, action)) in [
                    ("Market", Action::OpenMarket),
                    ("AH: найти", Action::OpenAuction(selected)),
                    ("Обойти market", Action::Scan(scan::Kind::Market)),
                    ("Обойти AH", Action::Scan(scan::Kind::Auction)),
                ]
                .into_iter()
                .enumerate()
                {
                    button(
                        game,
                        batch,
                        s,
                        [8.0 + i as f32 * (bw + 2.0), h - 48.0, bw, 18.0],
                        text,
                        Hit::Run(action),
                        false,
                        screen.mouse,
                    );
                }
            }
            let coverage = format!(
                "{} · AH: {} стр. ({})",
                status.message,
                book.observed_pages,
                if book.complete_scan {
                    "обход завершён"
                } else {
                    "частичный снимок"
                }
            );
            let text = if !s.notice.is_empty() {
                &s.notice
            } else {
                &coverage
            };
            label(
                game,
                batch,
                text,
                [8.0, h - 20.0],
                w - if status.scanning { 100.0 } else { 16.0 },
                0x9bafc2,
            );
            if status.scanning {
                button(
                    game,
                    batch,
                    s,
                    [w - 88.0, h - 24.0, 80.0, 18.0],
                    "Стоп обход",
                    Hit::Run(Action::Stop),
                    false,
                    screen.mouse,
                );
            }
            if s.sort_menu && (1..=4).contains(&s.tab) {
                let lw = ((w - 24.0) * 0.44).max(110.0);
                let x = (8.0 + lw - 124.0).max(8.0);
                for (i, sort) in [
                    Sort::Unit,
                    Sort::Total,
                    Sort::Profit,
                    Sort::Roi,
                    Sort::Count,
                    Sort::Name,
                    Sort::Recent,
                ]
                .into_iter()
                .enumerate()
                {
                    button(
                        game,
                        batch,
                        s,
                        [x, 77.0 + i as f32 * 19.0, 124.0, 18.0],
                        sort.label(),
                        Hit::SortPick(sort),
                        s.sort[s.tab as usize] == sort,
                        screen.mouse,
                    );
                }
            }
        });
    });
    let variant=state(|s|format!("{}:{}",s.tab,s.group));
    ui::finish(game,batch,start,"economy",&variant);
    batch.clip_since(start, 0.0, 0.0, w, h);
    for vertex in &mut batch.vertices[start..] {
        vertex.pos[0] += ox;
        vertex.pos[1] += oy;
    }
    true
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button_: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !visible() || game.native_mod_screen() != Some(claims::ID) {
        return false;
    }
    *canceled = true;
    if !pressed || button_ != 0 {
        return true;
    }
    let panel = Panel::new([screen.rect[2], screen.rect[3]]);
    let mouse = panel.local(screen.mouse);
    state(|s| {
        if s.viewport != [screen.rect[2], screen.rect[3]] {
            return;
        }
        let hit = s
            .hits
            .iter()
            .rev()
            .find(|(r, _)| contains(*r, mouse))
            .map(|(_, h)| h.clone());
        s.notice.clear();
        if !matches!(hit, Some(Hit::SortMenu)) {
            s.sort_menu = false;
        }
        match hit {
            Some(Hit::SortMenu) => s.sort_menu = !s.sort_menu,
            Some(Hit::SortPick(sort)) => {
                s.sort[s.tab as usize] = sort;
                s.page = 0;
            }
            Some(Hit::Tab(t)) => {
                s.tab = t;
                s.page = 0;
                s.focus = 0;
                s.token = s.token.wrapping_add(1);
            }
            Some(Hit::Close) => {
                s.open = false;
                add(s, Action::Close);
            }
            Some(Hit::Search) => {
                s.focus = 1;
                s.token = s.token.wrapping_add(1);
            }
            Some(Hit::Target) => {
                s.focus = 2;
                s.token = s.token.wrapping_add(1);
            }
            Some(Hit::Row(item, id)) => {
                s.item = Some(item);
                s.listing = id;
                s.focus = 0;
                s.token = s.token.wrapping_add(1);
            }
            Some(Hit::Page(dir)) => {
                if s.tab == 5 {
                    s.settings_page = s.settings_page.saturating_add_signed(dir as isize);
                } else {
                    s.page = s.page.saturating_add_signed(dir as isize);
                }
            }
            Some(Hit::Group(g)) => {
                s.group = g;
                s.settings_page = 0;
            }
            Some(Hit::Adjust(key, dir)) => add(s, Action::Setting(key, dir)),
            Some(Hit::Run(a)) => add(s, a),
            None => s.focus = 0,
        }
    });
    true
}
pub(super) fn key(game: &Game<'_>, key: &str, mods: u32, canceled: &mut bool) -> bool {
    if !visible() {
        return false;
    }
    *canceled = true;
    if key == "key.keyboard.escape" {
        close();
        state(|s| add(s, Action::Close));
        return true;
    }
    state(|s| {
        if s.focus == 0 {
            return;
        }
        if mods & (2 | 8) != 0
            && ["key.keyboard.v", "key.keyboard.c", "key.keyboard.x"].contains(&key)
        {
            let c = key.chars().last().unwrap_or('v');
            s.suppress = Some((c, Instant::now()));
            if c == 'v' {
                add(s, Action::Paste(s.token));
            } else {
                let editor = if s.focus == 1 {
                    &mut s.search
                } else {
                    &mut s.target
                };
                let text = editor.selected_text();
                if c == 'x' {
                    editor.delete_selection();
                }
                if !text.is_empty() {
                    add(s, Action::Copy(text));
                }
            }
            return;
        }
        let editor = if s.focus == 1 {
            &mut s.search
        } else {
            &mut s.target
        };
        editor.key(key, mods);
        s.page = 0;
    });
    let _ = game;
    true
}
pub(super) fn character(game: &Game<'_>, c: char, canceled: &mut bool) -> bool {
    if !visible() {
        return false;
    }
    *canceled = true;
    if game.modifiers.control_key() || game.modifiers.super_key() {
        return true;
    }
    state(|s| {
        if let Some((old, at)) = s.suppress.take()
            && old.eq_ignore_ascii_case(&c)
            && at.elapsed().as_millis() < 100
        {
            return;
        }
        if s.focus == 1 {
            s.search.character(c);
        } else if s.focus == 2 && (c.is_ascii_digit() || c == '.' || c == ',') {
            s.target.character(c);
        }
        s.page = 0;
    });
    true
}
pub(super) fn scroll(delta: f64, canceled: &mut bool) -> bool {
    if !visible() {
        return false;
    }
    *canceled = true;
    state(|s| {
        let d = if delta > 0.0 { -1 } else { 1 };
        if s.tab == 5 {
            s.settings_page = s.settings_page.saturating_add_signed(d);
        } else {
            s.page = s.page.saturating_add_signed(d);
        }
    });
    true
}
pub(super) fn hide_sidebar(game: &Game<'_>) -> bool {
    game.native_on(claims::ID)
        && data::read(|_, p, _, _| {
            p.flag("custom_sidebar") || (p.flag("enabled") && p.flag("hide_sidebar"))
        })
}
pub(super) fn hud(
    game: &Game<'_>,
    batch: &mut HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    if !game.native_on(claims::ID)
        || game.ui.hide_gui
        || game.native_player().is_none()
        || !matches!(
            game.screen,
            crate::game::Screen::Playing | crate::game::Screen::Chat
        )
    {
        return;
    }
    data::read(|book, p, status, _| {
        if !p.flag("enabled")
            || !p.flag("hud")
            || (p.flag("custom_sidebar") && !status.sidebar.is_empty())
        {
            return;
        }
        let mut lines = Vec::new();
        if p.flag("hud_balance") {
            lines.push(format!("Баланс: {}", value(book.balance.map(|(n, _)| n))));
        }
        if p.flag("hud_limits") {
            lines.push(format!(
                "Лимит продаж: осталось {} монет",
                value(book.coin_left.map(|(n, _)| n))
            ));
        }
        if p.flag("hud_hand")
            && let Some(h) = &status.hand
        {
            let q = book.quotes.get(&h.item);
            lines.push(format!(
                "{} ×{} · S {}",
                item_label(&h.item),
                h.count,
                value(q.and_then(|q| q.sell))
            ));
        }
        if lines.is_empty() {
            return;
        }
        let scale = p.get("scale") as f32 / 100.0;
        let width = (p.get("width") as f32 * scale).min(batch.width - 8.0);
        let height = (lines.len() as f32 * 11.0 + 8.0) * scale;
        let corner = crate::game::claim_borders_map_view::corner(p.get("corner") as u8);
        let (x, y) = corners.place(corner, width, height);
        let x = (x + p.get("offset_x") as f32).clamp(2.0, (batch.width - width - 2.0).max(2.0));
        let y = (y + p.get("offset_y") as f32).clamp(2.0, (batch.height - height - 28.0).max(2.0));
        let start = batch.vertices.len();
        ui::fill(game,
            batch,
            [x / scale, y / scale, width / scale, height / scale],
            ((p.get("opacity") as u32 * 255 / 100) << 24) | 0x101b27,
        );
        for (i, text) in lines.iter().enumerate() {
            label(
                game,
                batch,
                text,
                [x / scale + 5.0, y / scale + 4.0 + i as f32 * 11.0],
                width / scale - 10.0,
                if i == 0 { 0xe4cf99 } else { 0xb7c9d9 },
            );
        }
        batch.scale_since(start, 0.0, 0.0, scale, scale);
    });
}

// A compact read-only note above the official server container.
pub(super) fn container_overlay(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    if !game.native_on(claims::ID) || screen.id != "inventory" || game.ui.hide_gui {
        return;
    }
    let Some(title) = screen.title.map(|t| t.plain().to_ascii_lowercase()) else {
        return;
    };
    if !["market", "auction house"].contains(&title.as_str()) {
        return;
    }
    let token = format!(
        "{:?}|{}",
        game.remote_menu(),
        game.draimhorizons_menu_stamp()
    );
    let slot = game
        .slot_at(batch, screen.mouse[0], screen.mouse[1])
        .and_then(|s| s.checked_sub(300));
    data::read(|book, p, status, own| {
        if !p.flag("enabled") || !p.flag("overlay") || status.menu_token != token {
            return;
        }
        let now = data::now();
        let mut lines = vec![
            (
                if title == "market" {
                    "MARKET".into()
                } else {
                    "АУКЦИОН".into()
                },
                0x9edccb,
            ),
            (
                format!(
                    "Лимит: {} / {} монет",
                    value(book.coin_left.map(|(v, _)| v)),
                    value(book.coin_cap)
                ),
                0xd9c68f,
            ),
        ];
        if let Some((_, item, lot)) =
            slot.and_then(|slot| status.menu_slots.iter().find(|(s, _, _)| *s == slot))
        {
            if let Some(l) = lot.and_then(|id| book.listings.get(&id)) {
                lines.push((format!("{} ×{}", l.label, l.count), 0xd7e3ec));
                lines.push((format!("Цена лота: {} монет", model::fmt(l.cost)), 0xa8bdcf));
                if let Some(o) = book.opportunity(l, p, now, own) {
                    lines.push((
                        format!("Оценка AH → Market: {}", model::delta(o.full_profit)),
                        if o.full_profit > 0 {
                            0x94d5ae
                        } else {
                            0xdfa68d
                        },
                    ));
                    lines.push((format!("По лимитам сейчас: {} шт.", o.quantity), 0xa8bdcf));
                    if !o.ready {
                        lines.push(("Проверьте цену и ограничения".into(), 0xd7bd87));
                    }
                } else {
                    lines.push((
                        if l.bid {
                            "Аукционные торги: цена не фиксирована".into()
                        } else {
                            "Нет цены Market для расчёта".into()
                        },
                        0xd7bd87,
                    ));
                }
            } else if let Some(q) = book.quotes.get(item) {
                lines.push((item_label(item), 0xd7e3ec));
                lines.push((format!("Купить: {} / шт.", value(q.buy)), 0xa8bdcf));
                lines.push((
                    if q.sell.is_none() && q.buy.is_some() {
                        "Market не принимает на продажу".into()
                    } else {
                        format!("Продать: {} / шт.", value(q.sell))
                    },
                    0xa8bdcf,
                ));
            }
        } else {
            lines.push(("Наведите на товар или лот".into(), 0x8fa9bb));
        }
        let height = lines.len() as f32 * 12.0 + 14.0;
        let Some([x, y, w, h]) = crate::game::claim_borders_economy_ui::beside(
            screen.rect,
            [batch.width, batch.height],
            height,
        ) else {
            return;
        };
        let start = batch.vertices.len();
        ui::fill(game, batch, [x, y, w, h], 0xc00c1721);
        ui::fill(game, batch, [x, y, 2.0, h], 0xbb72cbb5);
        for (i, (text, color)) in lines.iter().enumerate() {
            label(
                game,
                batch,
                text,
                [x + 8.0, y + 7.0 + i as f32 * 12.0],
                w - 16.0,
                *color,
            );
        }
        batch.clip_since(start, x, y, x + w, y + h);
    });
}
