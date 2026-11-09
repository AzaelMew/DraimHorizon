use crate::game::claim_borders_ui_view as ui;
// DraimHorizons economy workspace; server transactions remain explicit local actions.
use crate::game::{
    Game, claim_borders as claims, claim_borders_economy as model,
    claim_borders_economy_client as data, claim_borders_economy_scan as scan,
    claim_borders_economy_ui::{Panel, Sort},
    claim_borders_menu as edit,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::hud::HudBatch;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::Instant,
};
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
macro_rules! tr {
    ($russian:literal, $english:literal $(, $arg:expr)* $(,)?) => {
        match language() {
            Language::Russian => format!($russian $(, $arg)*),
            Language::English => format!($english $(, $arg)*),
        }
    };
}
#[derive(Clone)]
pub enum Action {
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
    Run(Action),
}
struct State {
    open: bool,
    tab: u8,
    sort: [Sort; 5],
    sort_menu: bool,
    page: usize,
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
    let mut menu = edit::Menu::default();
    menu.focus = Some(edit::Field::Name);
    menu
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            tab: 0,
            sort: [Sort::Name, Sort::Name, Sort::Unit, Sort::Profit, Sort::Unit],
            sort_menu: false,
            page: 0,
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
    ui::button(game, batch, rect, text, selected, mouse);
    s.hits.push((rect, hit));
}
fn age(at: u64, now: u64) -> String {
    if at == 0 {
        language().text("нет данных", "no data").into()
    } else {
        tr!("{} с назад", "{} s ago", now.saturating_sub(at) / 1000)
    }
}
pub(super) fn item_label(game: &Game<'_>, id: &str) -> String {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let prefix = if game.res.items().get(id).is_some_and(|i| i.block.is_some()) {
        "block"
    } else {
        "item"
    };
    game.translate(
        &format!("{prefix}.{namespace}.{}", path.replace('/', ".")),
        &path.replace('_', " "),
    )
}
pub(super) fn market_lore(game: &Game<'_>, text: &str) -> String {
    if let Some((name, change)) = text.split_once('▲').or_else(|| text.split_once('▼')) {
        if let Some(id) = model::id(name.trim()).filter(|id| game.res.items().get(id).is_some()) {
            let arrow = if text.contains('▲') { '▲' } else { '▼' };
            return format!("{} {arrow}{}", item_label(game, &id), change);
        }
    }
    if language() == Language::English {
        return text.to_string();
    }
    text.replace("Price: ", "Цена: ")
        .replace(" each, to buy or sell", " / шт. · купить / продать")
        .replace(
            " each (the market doesn't buy it)",
            " / шт. · сервер не покупает",
        )
        .replace(
            " each (the market doesn't sell it)",
            " / шт. · сервер не продаёт",
        )
        .replace("Traded in 24 h: ", "Объём за 24 ч: ")
        .replace("You may still sell ", "Можно продать ещё ")
        .replace(" today", " сегодня")
        .replace("Base price: ", "Базовая цена: ")
        .replace("1 h ", "1 ч ")
        .replace("24 h ", "24 ч ")
        .replace("unchanged", "без изменений")
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
            language().text("DRAIMHORIZONS / ЭКОНОМИКА", "DRAIMHORIZONS / ECONOMY"),
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
        let tabs = [
            language().text("Обзор", "Overview"),
            language().text("Цены", "Prices"),
            language().text("Аукцион", "Auction"),
            language().text("Прибыль", "Profit"),
            language().text("План", "Plan"),
        ];
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
            if s.tab == 0 {
                let width = w - 24.0;
                let x = 12.0;
                // Three figures as cards: balance, priced items, lots seen at the auction.
                let priced = book
                    .quotes
                    .values()
                    .filter(|q| q.sell.is_some() || q.buy.is_some())
                    .count();
                let card_w = (width - 8.0) / 3.0;
                for (i, (title, figure, color)) in [
                    (
                        language().text("Баланс", "Balance"),
                        value(book.balance.map(|(v, _)| v)),
                        0xe7ce95,
                    ),
                    (
                        language().text("Товаров с ценой", "Priced items"),
                        priced.to_string(),
                        0x9edccb,
                    ),
                    (
                        language().text("Лотов на аукционе", "Auction lots"),
                        book.listings.len().to_string(),
                        0xb9a7e8,
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let cx = x + i as f32 * (card_w + 4.0);
                    ui::fill(game, batch, [cx, 60.0, card_w, 40.0], 0xff1c2a39);
                    ui::fill(game, batch, [cx, 60.0, 2.0, 40.0], 0xff000000 | color);
                    label(
                        game,
                        batch,
                        title,
                        [cx + 8.0, 66.0],
                        card_w - 12.0,
                        0x8fa9bb,
                    );
                    label(game, batch, &figure, [cx + 8.0, 82.0], card_w - 12.0, color);
                }
                let mut y = 112.0;
                if !book.movers.is_empty() {
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        &tr!(
                            "Динамика за 24 ч · {}",
                            "24 h movers · {}",
                            age(book.movers_at, now)
                        ),
                        0xb9a7e8,
                    );
                    for mover in book.movers.iter().take(if h > 300.0 { 5 } else { 2 }) {
                        line(
                            game,
                            batch,
                            x,
                            &mut y,
                            width,
                            &market_lore(game, mover),
                            if mover.contains('▲') {
                                0x94d5ae
                            } else {
                                0xdfa68d
                            },
                        );
                    }
                }
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
                        &tr!(
                            "Лучшая оценка AH → Market: {} · {} монет за лот",
                            "Best AH → Market estimate: {} · {} coins per lot",
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
                        &tr!(
                            "Сейчас можно продать {} из {} шт. · подробнее в Прибыль",
                            "Can sell {} of {} pcs now · details in Profit",
                            o.quantity,
                            o.listing.count
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
                        language().text(
                            "Цены и лоты обновляются автоматически при открытии.",
                            "Prices and lots refresh automatically when you open them.",
                        ),
                        0x94b5ca,
                    );
                    line(
                        game,
                        batch,
                        x,
                        &mut y,
                        width,
                        language().text(
                            "В Прибыль видны расчёты и причины, мешающие продаже.",
                            "Profit shows the calculations and what blocks a sale.",
                        ),
                        0x94b5ca,
                    );
                }
                if status.sale.is_none() {
                    let bw = (w - 22.0) / 4.0;
                    for (i, (text, action)) in [
                        (
                            language().text("Обновить", "Refresh"),
                            Action::Refresh(status.hand.as_ref().map(|h| h.item.clone())),
                        ),
                        ("Market", Action::OpenMarket),
                        (
                            language().text("Аукцион", "Auction"),
                            Action::OpenAuction(None),
                        ),
                        (
                            language().text("Проверить руку", "Check hand"),
                            Action::PrepareSell,
                        ),
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
                        language()
                            .text(
                                "Нет свежей цены для подтверждения",
                                "No fresh price to confirm",
                            )
                            .into(),
                        |(n, net)| {
                            tr!(
                                "{} ×{}: оценка +{} · повторное нажатие продаёт",
                                "{} ×{}: estimate +{} · press again to sell",
                                item_label(game, &hand.item),
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
                        language().text("Отмена", "Cancel"),
                        Hit::Run(Action::CancelSell),
                        false,
                        screen.mouse,
                    );
                    button(
                        game,
                        batch,
                        s,
                        [w - 178.0, h - 48.0, 170.0, 18.0],
                        language().text("ПОДТВЕРДИТЬ /sell hand", "CONFIRM /sell hand"),
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
                    language()
                        .text("Поиск товара / ника", "Search item / player")
                        .into()
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
                    s.sort[s.tab as usize].label_in(language()),
                    Hit::SortMenu,
                    s.sort_menu,
                    screen.mouse,
                );
                let filter = s.search.name.to_lowercase();
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
                            item_label(game, key).to_lowercase().contains(&filter)
                                || key.contains(&filter)
                        })
                        .map(|(key, q)| {
                            (
                                key.clone(),
                                None,
                                item_label(game, key),
                                tr!(
                                    "Сдать {} · Купить {}",
                                    "Sell {} · Buy {}",
                                    value(q.sell),
                                    value(q.buy)
                                ),
                                false,
                            )
                        })
                        .collect()
                } else if s.tab == 3 {
                    s.sort[s.tab as usize]
                        .listings(book, p, now, own, true)
                        .into_iter()
                        .filter(|l| {
                            item_label(game, &l.item).to_lowercase().contains(&filter)
                                || l.seller.to_lowercase().contains(&filter)
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
                                            language()
                                                .text("Market не покупает", "Market does not buy")
                                                .into()
                                        } else {
                                            language()
                                                .text(
                                                    "Нет цены Market · ожидаем ответ",
                                                    "No Market price · waiting for reply",
                                                )
                                                .into()
                                        }
                                    },
                                    |o| {
                                        tr!(
                                            "{} · {:.1}% · сегодня {} шт.{}",
                                            "{} · {:.1}% · today {} pcs{}",
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
                            item_label(game, &l.item).to_lowercase().contains(&filter)
                                || l.seller.to_lowercase().contains(&filter)
                        })
                        .map(|l| {
                            (
                                l.item.clone(),
                                Some(l.id),
                                format!("{} ×{}", l.label, l.count),
                                tr!(
                                    "{:.2}/шт. · {} за лот · {}{}",
                                    "{:.2}/unit · {} per lot · {}{}",
                                    l.cost as f64 / 100.0 / l.count.max(1) as f64,
                                    model::fmt(l.cost),
                                    l.seller,
                                    if l.bid {
                                        language().text(" · торги", " · bidding")
                                    } else {
                                        ""
                                    }
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
                    ui::fill(
                        game,
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
                        language().text(
                            "Откройте AH / Market для сбора данных",
                            "Open AH / Market to collect data",
                        ),
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
                    line(
                        game,
                        batch,
                        rx,
                        &mut y,
                        rw,
                        &item_label(game, item),
                        0xe4d29e,
                    );
                    if let Some(q) = book.quotes.get(item) {
                        line(
                            game,
                            batch,
                            rx,
                            &mut y,
                            rw,
                            &tr!(
                                "Market покупает: {} / шт.",
                                "Market buys: {} / unit",
                                value(q.sell)
                            ),
                            0x91d1b3,
                        );
                        if s.tab == 1 {
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &tr!(
                                    "Market продаёт: {} / шт.",
                                    "Market sells: {} / unit",
                                    value(q.buy)
                                ),
                                0xcbd7e2,
                            );
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &format!(
                                    "{} · {}",
                                    if q.source == "saved" {
                                        language().text("сохранено", "saved")
                                    } else {
                                        q.source.as_str()
                                    },
                                    age(q.at, now)
                                ),
                                0x8ea3b7,
                            );
                            line(
                                game,
                                batch,
                                rx,
                                &mut y,
                                rw,
                                &tr!(
                                    "Осталось предметов: {}",
                                    "Items left: {}",
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
                                &tr!(
                                    "1ч {} · 24ч {}",
                                    "1h {} · 24h {}",
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
                            &tr!(
                                "{}: {} / {} шт.",
                                "{}: {} / {} pcs",
                                l.seller,
                                model::fmt(l.cost),
                                l.count
                            ),
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
                                    &tr!(
                                        "За весь лот: {} монет (оценка)",
                                        "Whole lot: {} coins (estimate)",
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
                                    &tr!(
                                        "Сейчас продать {} шт. · выручка {}",
                                        "Sell {} pcs now · proceeds {}",
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
                                    &tr!(
                                        "Ожидаемая выручка: {} монет",
                                        "Expected proceeds: {} coins",
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
                                    &if o.quantity == 0 {
                                        language()
                                            .text(
                                                "Сейчас продажа недоступна: проверьте ограничения",
                                                "Selling is unavailable now: check the restrictions",
                                            )
                                            .into()
                                    } else {
                                        tr!(
                                            "Прибыль сейчас: {} монет",
                                            "Profit now: {} coins",
                                            model::delta(o.profit)
                                        )
                                    },
                                    if o.ready { 0x91d1b3 } else { 0xe2ba84 },
                                );
                                for note in o
                                    .notes
                                    .iter()
                                    .take(((h - y - 82.0) / 11.0).max(0.0) as usize)
                                {
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        &note.text(language()),
                                        0xc5a078,
                                    );
                                }
                            }
                        } else if s.tab == 4 {
                            let target = if s.target.name.is_empty() {
                                language()
                                    .text("Цена будущей продажи AH", "Future AH sale price")
                                    .into()
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
                                        &tr!(
                                            "Комиссия размещения {}",
                                            "Listing fee {}",
                                            model::fmt(fee)
                                        ),
                                        0xcbd7e2,
                                    );
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        &tr!(
                                            "Условная прибыль {}",
                                            "Hypothetical profit {}",
                                            model::fmt(profit)
                                        ),
                                        0xe4c18b,
                                    );
                                } else {
                                    line(
                                        game,
                                        batch,
                                        rx,
                                        &mut y,
                                        rw,
                                        language().text(
                                            "Комиссии: Настройки → Торги",
                                            "Fees: Settings → Trading",
                                        ),
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
                                language().text(
                                    "Цена объявления не гарантирует продажу",
                                    "A listing price does not guarantee a sale",
                                ),
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
                                        language().text("Аукционная ставка", "Auction bid")
                                    } else {
                                        language().text("Фиксированная цена", "Fixed price")
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
                        language().text("Цена /worth", "/worth price"),
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
                            language().text("Не следить", "Unwatch")
                        } else {
                            language().text("Следить", "Watch")
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
                    (
                        language().text("AH: найти", "AH: find"),
                        Action::OpenAuction(selected),
                    ),
                    (
                        language().text("Обойти market", "Scan market"),
                        Action::Scan(scan::Kind::Market),
                    ),
                    (
                        language().text("Обойти AH", "Scan AH"),
                        Action::Scan(scan::Kind::Auction),
                    ),
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
            let coverage = tr!(
                "{} · AH: {} стр. ({})",
                "{} · AH: {} pages ({})",
                status.message.text(language()),
                book.observed_pages,
                if book.complete_scan {
                    language().text("обход завершён", "scan complete")
                } else {
                    language().text("частичный снимок", "partial snapshot")
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
                    language().text("Стоп обход", "Stop scan"),
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
                        sort.label_in(language()),
                        Hit::SortPick(sort),
                        s.sort[s.tab as usize] == sort,
                        screen.mouse,
                    );
                }
            }
        });
    });
    let variant = state(|s| s.tab.to_string());
    ui::finish(game, batch, start, "economy", &variant);
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
                s.page = s.page.saturating_add_signed(dir as isize);
            }
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
        s.page = s.page.saturating_add_signed(d);
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
            lines.push(tr!(
                "Баланс: {}",
                "Balance: {}",
                value(book.balance.map(|(n, _)| n))
            ));
        }
        if p.flag("hud_hand")
            && let Some(h) = &status.hand
        {
            let q = book.quotes.get(&h.item);
            lines.push(format!(
                "{} ×{} · S {}",
                item_label(game, &h.item),
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
        ui::fill(
            game,
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
    let Some(title) = screen.title.map(|t| t.plain().to_lowercase()) else {
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
        let mut lines = vec![(
            if title == "market" {
                "MARKET".into()
            } else {
                language().text("АУКЦИОН", "AUCTION").into()
            },
            0x9edccb,
        )];
        if let Some((_, item, lot)) =
            slot.and_then(|slot| status.menu_slots.iter().find(|(s, _, _)| *s == slot))
        {
            if let Some(l) = lot.and_then(|id| book.listings.get(&id)) {
                lines.push((format!("{} ×{}", l.label, l.count), 0xd7e3ec));
                lines.push((
                    tr!(
                        "Цена лота: {} монет",
                        "Lot price: {} coins",
                        model::fmt(l.cost)
                    ),
                    0xa8bdcf,
                ));
                if let Some(o) = book.opportunity(l, p, now, own) {
                    lines.push((
                        tr!(
                            "Оценка AH → Market: {}",
                            "AH → Market estimate: {}",
                            model::delta(o.full_profit)
                        ),
                        if o.full_profit > 0 {
                            0x94d5ae
                        } else {
                            0xdfa68d
                        },
                    ));
                    lines.push((
                        tr!(
                            "Доступно к продаже: {} шт.",
                            "Available to sell: {} pcs",
                            o.quantity
                        ),
                        0xa8bdcf,
                    ));
                    if !o.ready {
                        lines.push((
                            language()
                                .text(
                                    "Проверьте цену и ограничения",
                                    "Check the price and restrictions",
                                )
                                .into(),
                            0xd7bd87,
                        ));
                    }
                } else {
                    lines.push((
                        if l.bid {
                            language()
                                .text(
                                    "Аукционные торги: цена не фиксирована",
                                    "Auction bidding: price is not fixed",
                                )
                                .into()
                        } else {
                            language()
                                .text(
                                    "Нет цены Market для расчёта",
                                    "No Market price to calculate with",
                                )
                                .into()
                        },
                        0xd7bd87,
                    ));
                }
            } else if let Some(q) = book.quotes.get(item) {
                lines.push((item_label(game, item), 0xd7e3ec));
                lines.push((
                    tr!("Купить: {} / шт.", "Buy: {} / unit", value(q.buy)),
                    0xa8bdcf,
                ));
                lines.push((
                    if q.sell.is_none() && q.buy.is_some() {
                        language()
                            .text(
                                "Market не принимает на продажу",
                                "Market does not accept this for sale",
                            )
                            .into()
                    } else {
                        tr!("Продать: {} / шт.", "Sell: {} / unit", value(q.sell))
                    },
                    0xa8bdcf,
                ));
            }
        } else {
            lines.push((
                language()
                    .text("Наведите на товар или лот", "Hover over an item or lot")
                    .into(),
                0x8fa9bb,
            ));
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
