// Presentation of the live server menu. Transactions are revalidated against the current window.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_economy as economy,
    claim_borders_economy_client as data, claim_borders_economy_scan as scan,
    claim_borders_input::{Edit, TextInput},
    claim_borders_ui_view as ui,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::{clipboard::Clipboard, hud::HudBatch};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Mutex, OnceLock},
};
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
fn t(russian: &'static str, english: &'static str) -> &'static str {
    language().text(russian, english)
}
fn russian() -> bool {
    language() == Language::Russian
}
#[derive(Clone)]
struct Pending {
    token: String,
    button: scan::Button,
    at: u64,
}
#[derive(Clone)]
enum Hit {
    Server(scan::Button),
    Confirm,
    Cancel,
    /// One pager for cards and server pages: it turns the server page at either end.
    Pager(i32),
    Search,
    Standard,
    Sort,
    /// A lot of the catalogue, by its auction number.
    Lot(u64),
    /// A server command for a page the catalogue does not replace.
    Command(&'static str),
    /// Walk the auction again.
    Refresh,
    Buy,
    /// The "ask before buying" switch inside the confirmation.
    AskToggle,
}
/// A server page turn requested by the pager, applied when the server's window changes.
#[derive(Clone, Copy)]
enum Nav {
    /// Forward from a page that had this many card pages.
    Forward(usize),
    Backward,
    /// Anything else (a category, Back, a purchase): numbering starts again.
    Other,
}
#[derive(Default)]
struct State {
    icons: Vec<(usize, horizon_client::items::ItemStack)>,
    token: String,
    query: TextInput,
    watch_edit: Option<(String, TextInput)>,
    focus: bool,
    /// 0 = the server's order, 1 = unit price, 2 = lot price (cheapest first).
    sort: u8,
    page: usize,
    hits: Vec<([f32; 4], Hit)>,
    pending: Option<Pending>,
    action: Option<Pending>,
    pressed: BTreeSet<i32>,
    notice: String,
    copy: Option<String>,
    paste: bool,
    hover: Option<usize>,
    control_tip: Option<String>,
    modal_scroll: usize,
    /// Card pages of the server pages already passed, for a continuous page number.
    passed: Vec<usize>,
    nav: Option<Nav>,
    /// When the pending page turn was asked; an unanswered one expires.
    nav_at: u64,
    /// The pager as last drawn: card pages and the server's own page buttons.
    pager: (usize, Option<scan::Button>, Option<scan::Button>),
    /// The catalogue replaces the auction while it stays open from its home page.
    session: scan::Session,
    catalog_page: usize,
    catalog_pages: usize,
    /// 0 = unit price, 1 = lot price, 2 = ending soonest, 3 = newest.
    catalog_sort: u8,
    /// The lot, as the player saw it, whose purchase waits for confirmation.
    buy: Option<economy::Listing>,
    /// The catalogue's order while it is browsed: lots keep their places as data arrives.
    order: Vec<u64>,
    order_key: (String, u8),
    /// The cards on screen as drawn, so a click acts on what was seen, not on newer data.
    shown: HashMap<u64, economy::Listing>,
    /// A confirmed server command, sent on the next frame.
    command: Option<String>,
    /// Item stacks seen for each lot, for icons and the item tooltip.
    stacks: HashMap<u64, horizon_client::items::ItemStack>,
    hover_lot: Option<u64>,
    /// Refresh asked from this window: walk again once a newer home page arrives.
    refresh: Option<String>,
    /// Close the window on the next frame.
    close: bool,
    /// Flip "ask before buying" on the next frame (it needs the game to save).
    ask_toggle: bool,
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
/// The auction catalogue is on screen, so a click is for it rather than for a scan.
pub(super) fn catalog_active(game: &Game<'_>) -> bool {
    live(game).is_some_and(|(page, _)| page.family == scan::Kind::Auction)
        && state(|s| s.session.shown())
}
pub(super) fn active(game: &Game<'_>) -> bool {
    live(game).is_some()
}
fn view_key(kind: scan::Kind) -> &'static str {
    match kind {
        scan::Kind::Auction => "custom_auction",
        scan::Kind::Market => "custom_market",
    }
}
fn toggle_view(game: &Game<'_>, kind: scan::Kind) {
    // The walk clicks the server's pages; it must not go on under the player's own hands.
    data::cancel_scan();
    data::configure(game, view_key(kind), 1);
    state(|s| {
        s.pending = None;
        s.action = None;
        s.focus = false;
        s.watch_edit = None;
        s.hits.clear();
    });
}
fn live(game: &Game<'_>) -> Option<(scan::Page, String)> {
    // While the catalogue walks the auction a page change is not yet parsed for a frame or
    // two; the walk's last page stands in so the server's own page is never drawn or clicked.
    let page = data::server_page(game).or_else(|| data::catalog_hold(game))?;
    if !data::read(|_, prefs, _, _| prefs.flag(view_key(page.0.family))) {
        None
    } else {
        Some(page)
    }
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        reset();
        return;
    }
    if game.screen == Screen::Inventory {
        let mut icons = Vec::new();
        let _ = game.menu_session(|menu, _, _| {
            if matches!(menu.kind(), horizon_client::menu::Kind::Chest(6)) {
                for slot in 0..54 {
                    if let Some(stack) = menu.slot(slot) {
                        icons.push((slot, stack.clone()));
                    }
                }
            }
            Some(false)
        });
        let lots = data::lot_stacks(game);
        data::remember_stacks(&lots);
        state(|s| {
            if s.stacks.len() > 6000 {
                s.stacks.clear();
            }
            s.stacks.extend(lots);
            s.icons = icons;
        });
    }
    if state(|s| std::mem::take(&mut s.close)) && game.screen == Screen::Inventory {
        data::cancel_scan();
        game.close_container();
        game.screen = Screen::Playing;
        game.capture(true);
    }
    if let Some(command) = state(|s| s.command.take()) {
        game.send_unattended_command(&command);
    }
    let (action, copy, paste) =
        state(|s| (s.action.take(), s.copy.take(), std::mem::take(&mut s.paste)));
    if let Some(copy) = copy {
        game.clipboard.set(&copy);
    }
    if paste {
        let v = game.clipboard.get();
        state(|s| {
            if let Some((_, edit)) = &mut s.watch_edit {
                edit.insert(&v, 20);
            } else {
                s.query.insert(&v, 64);
                s.page = 0;
                s.catalog_page = 0;
            }
        });
    }
    if let Some(p) = action {
        let ok = data::now().saturating_sub(p.at) <= 10_000
            && data::server_click(game, &p.token, &p.button, p.at);
        state(|s| {
            if !ok {
                s.nav = None;
            }
            s.notice = if ok {
                t("Запрос отправлен серверу", "Request sent to the server").into()
            } else {
                t(
                    "Окно, цена или предмет изменились. Выберите действие заново.",
                    "The window, price or item changed. Choose the action again.",
                )
                .into()
            }
        });
    }
    // A server swapping one window for the next must not end the catalogue session under the
    // player; a window that stays closed does, and the next opening is a new session.
    let open = game.screen == Screen::Inventory;
    let now = data::now();
    state(|s| {
        if !open {
            s.pending = None;
            s.focus = false;
            s.hits.clear();
        }
        if s.session.window(open, now) {
            s.buy = None;
        }
    });
}
/// How long ago, in the player's language.
fn ago(ms: u64) -> String {
    let secs = ms / 1000;
    let (n, unit) = match secs {
        0..60 => (secs, t("с", "s")),
        60..3600 => (secs / 60, t("мин", "min")),
        3600..86_400 => (secs / 3600, t("ч", "h")),
        _ => (secs / 86_400, t("д", "d")),
    };
    if russian() {
        format!("{n} {unit} назад")
    } else {
        format!("{n} {unit} ago")
    }
}
/// The auction's start page, where the catalogue walk begins.
fn auction_home(page: &scan::Page) -> bool {
    page.family == scan::Kind::Auction
        && page.buttons.iter().any(|b| {
            b.slot == 10
                && b.item == "minecraft:nether_star"
                && (b.name.starts_with("All listings (") || b.name.starts_with("Все лоты ("))
        })
}
/// Panel size: compact by default, larger on request for big screens.
fn panel(batch: &HudBatch) -> [f32; 4] {
    let (max_w, max_h) = if data::read(|_, p, _, _| p.flag("trade_large")) {
        (900.0, 560.0)
    } else {
        (620.0, 360.0)
    };
    let w = (batch.width - 16.0).clamp(1.0, max_w);
    let h = (batch.height - 16.0).clamp(1.0, max_h);
    [(batch.width - w) * 0.5, (batch.height - h) * 0.5, w, h]
}
fn coins(value: economy::Coins) -> String {
    if russian() {
        format!("{} монет", economy::fmt(value))
    } else if value == 100 {
        "1 coin".into()
    } else {
        format!("{} coins", economy::fmt(value))
    }
}
fn unit(l: &economy::Listing) -> String {
    let value = l.cost as f64 / 100.0 / f64::from(l.count.max(1));
    if russian() {
        format!("{value:.2} / шт.")
    } else {
        format!("{value:.2} each")
    }
}
fn time_left(l: &economy::Listing) -> String {
    let secs = l.expires.saturating_sub(data::now()) / 1000;
    let (d, h, m) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    let (dd, hh, mm) = if russian() {
        ("д", "ч", "м")
    } else {
        ("d", "h", "m")
    };
    if secs == 0 {
        t("истёк", "ended").into()
    } else if d > 0 {
        format!("{d}{dd} {h}{hh}")
    } else if h > 0 {
        format!("{h}{hh} {m}{mm}")
    } else {
        format!("{}{mm}", m.max(1))
    }
}
fn lot_icon(
    game: &Game<'_>,
    s: &State,
    l: &economy::Listing,
) -> Option<horizon_client::items::ItemStack> {
    let mut stack = match s.stacks.get(&l.id) {
        Some(stack) => stack.clone(),
        None if game.res.items().get(&l.item).is_some() => {
            horizon_client::items::ItemStack::new(&l.item, 1)
        }
        None => return None,
    };
    stack.count = 1;
    Some(stack)
}
fn lot_title(game: &Game<'_>, l: &economy::Listing) -> String {
    let name = if l.label.trim().is_empty() {
        crate::game::claim_borders_economy_view::item_label(game, &l.item)
    } else {
        l.label.clone()
    };
    format!("{name} ×{}", l.count)
}
/// The whole auction as one list: every lot found by the walk, searched and sorted locally,
/// with one pager. Buying sends `/ah buy <number>` after a confirmation.
fn render_catalog(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    rect: [f32; 4],
) {
    use crate::game::claim_borders_ui_style as style;
    let [x, y, w, h] = rect;
    let mouse = screen.mouse;
    let accent = ui::preferences(game).accent.color();
    s.hits.clear();
    s.hover = None;
    s.hover_lot = None;
    s.control_tip = None;
    let (mut lots, scanning, pages, finished, balance, caps, generation) =
        data::read(|book, _, status, _| {
            (
                book.listings.values().cloned().collect::<Vec<_>>(),
                status.catalog,
                status.pages,
                status.catalog_at,
                book.balance.map(|(v, _)| v),
                book.auction_watch.clone(),
                book.generation,
            )
        });
    let now = data::now();
    lots.retain(|l| l.expires > now);
    let total = lots.len();
    // Lots the current walk has not seen are saved data: shown, but not proof of anything.
    let saved_seen = lots
        .iter()
        .filter(|l| l.generation != generation)
        .map(|l| l.seen)
        .max();
    let newest_seen = lots.iter().map(|l| l.seen).max();
    lots.retain(|l| {
        crate::game::claim_borders_input::matches(
            &s.query.value,
            &format!("{} {} {} #{}", l.label, l.item, l.seller, l.id),
        )
    });
    let per_unit = |l: &economy::Listing| l.cost as f64 / f64::from(l.count.max(1));
    match s.catalog_sort {
        1 => lots.sort_by_key(|l| (l.cost, l.id)),
        2 => lots.sort_by_key(|l| (l.expires, l.id)),
        3 => lots.sort_by(|a, b| b.id.cmp(&a.id)),
        _ => lots.sort_by(|a, b| per_unit(a).total_cmp(&per_unit(b)).then(a.id.cmp(&b.id))),
    }
    // Sorting applies when the search or the sort changes; between those, lots that arrive
    // join at the end instead of moving the ones the player is reading.
    let key = (s.query.value.clone(), s.catalog_sort);
    if s.order_key != key {
        s.order.clear();
        s.order_key = key;
    }
    let sorted: Vec<u64> = lots.iter().map(|l| l.id).collect();
    s.order = economy::stable_order(&s.order, &sorted);
    let mut by_id: HashMap<u64, economy::Listing> = lots.drain(..).map(|l| (l.id, l)).collect();
    lots.extend(s.order.iter().filter_map(|id| by_id.remove(id)));
    s.shown.clear();
    ui::rounded(
        game,
        batch,
        [x - 3.0, y - 1.0, w + 6.0, h + 8.0],
        10.0,
        0x40000000,
    );
    ui::rounded(game, batch, rect, 8.0, 0xff000000 | style::BORDER);
    ui::rounded(
        game,
        batch,
        [x + 1.0, y + 1.0, w - 2.0, h - 2.0],
        7.0,
        0xf4000000 | style::BACKGROUND,
    );
    // Header: title, how much of the auction is loaded, balance and close.
    ui::icon(
        game,
        batch,
        ui::Icon::Market,
        [x + 10.0, y + 9.0, 12.0, 12.0],
        accent,
    );
    let title = t("Аукцион", "Auction");
    ui::text(game, batch, title, [x + 28.0, y + 11.0], style::TEXT, false);
    let status = if scanning {
        let saved = saved_seen.map(|t| ago(now.saturating_sub(t)));
        match (russian(), saved) {
            (true, Some(age)) => format!(
                "Обновление · стр. {} · {total} лотов · сохранено {age}",
                pages.max(1)
            ),
            (true, None) => format!("Обновление · стр. {} · {total} лотов", pages.max(1)),
            (false, Some(age)) => format!(
                "Refreshing · page {} · {total} lots · saved {age}",
                pages.max(1)
            ),
            (false, None) => format!("Refreshing · page {} · {total} lots", pages.max(1)),
        }
    } else if finished > 0 {
        let age = ago(now.saturating_sub(finished));
        if russian() {
            format!("{total} лотов · обновлено {age}")
        } else {
            format!("{total} lots · updated {age}")
        }
    } else if let Some(seen) = newest_seen {
        let age = ago(now.saturating_sub(seen));
        if russian() {
            format!("{total} лотов · сохранённые данные, видели {age}")
        } else {
            format!("{total} lots · saved data, last seen {age}")
        }
    } else if russian() {
        "Нет сохранённых данных".to_string()
    } else {
        "No saved data".to_string()
    };
    let title_w = game.native_text_width(title);
    let balance_text = balance.map(coins).unwrap_or_default();
    let balance_w = game.native_text_width(&balance_text);
    ui::text(
        game,
        batch,
        &ui::clip(game, &status, (w - title_w - balance_w - 90.0).max(0.0)),
        [x + 36.0 + title_w, y + 11.0],
        if scanning { 0xf0d68a } else { style::MUTED },
        false,
    );
    if !balance_text.is_empty() {
        ui::rounded(
            game,
            batch,
            [x + w - balance_w - 49.0, y + 6.0, balance_w + 14.0, 17.0],
            8.0,
            0x30f0d68a,
        );
        ui::text(
            game,
            batch,
            &balance_text,
            [x + w - balance_w - 42.0, y + 11.0],
            0xf0d68a,
            false,
        );
    }
    let close = [x + w - 28.0, y + 5.0, 21.0, 19.0];
    ui::button_base(game, batch, close, "", false, mouse);
    ui::icon(
        game,
        batch,
        ui::Icon::Close,
        [close[0] + 6.0, close[1] + 5.0, 9.0, 9.0],
        0xdfa8b5,
    );
    s.hits.push((close, Hit::Command("")));
    // Toolbar: search, sort and the pages the catalogue does not replace.
    let bar = y + 30.0;
    let tools: [(Hit, ui::Icon, &str); 3] = [
        (Hit::Refresh, ui::Icon::Refresh, t("Обновить", "Refresh")),
        (
            Hit::Command("ah mine"),
            ui::Icon::Chest,
            t("Мои лоты", "My listings"),
        ),
        (
            Hit::Command("ah mail"),
            ui::Icon::Restock,
            t("Почта", "Mail"),
        ),
    ];
    let labelled = w >= 520.0;
    let tool_w = |label: &str| {
        if labelled {
            game.native_text_width(label) + 26.0
        } else {
            22.0
        }
    };
    let sort_label = match s.catalog_sort {
        1 => t("Цена лота", "Lot price"),
        2 => t("Скоро конец", "Ending soon"),
        3 => t("Новые", "Newest"),
        _ => t("Цена за шт.", "Unit price"),
    };
    let sort_w = game.native_text_width(sort_label) + 30.0;
    let mut right = x + w - 8.0;
    for (hit, icon, label) in tools.into_iter().rev() {
        let bw = tool_w(label);
        right -= bw;
        let r = [right, bar, bw, 18.0];
        right -= 4.0;
        ui::button_base(game, batch, r, label, false, mouse);
        ui::icon(
            game,
            batch,
            icon,
            [r[0] + 6.0, r[1] + 4.0, 10.0, 10.0],
            accent,
        );
        if labelled {
            ui::text(
                game,
                batch,
                label,
                [r[0] + 20.0, r[1] + 5.0],
                style::TEXT,
                false,
            );
        } else if style::contains(r, mouse) {
            s.control_tip = Some(label.into());
        }
        s.hits.push((r, hit));
    }
    right -= sort_w;
    let sort = [right, bar, sort_w, 18.0];
    ui::button_base(game, batch, sort, sort_label, false, mouse);
    ui::icon(
        game,
        batch,
        ui::Icon::Sort,
        [sort[0] + 6.0, sort[1] + 4.0, 10.0, 10.0],
        accent,
    );
    ui::text(
        game,
        batch,
        sort_label,
        [sort[0] + 20.0, sort[1] + 5.0],
        style::TEXT,
        false,
    );
    if style::contains(sort, mouse) {
        s.control_tip = Some(t("Порядок сортировки", "Sort order").into());
    }
    s.hits.push((sort, Hit::Sort));
    let query = [x + 8.0, bar, right - x - 12.0, 18.0];
    let editing = s.focus || s.watch_edit.is_some();
    ui::rounded(
        game,
        batch,
        query,
        4.0,
        if editing { 0xff16303a } else { 0xff0e1620 },
    );
    if editing {
        for edge in [
            [query[0], query[1], query[2], 1.0],
            [query[0], query[1] + query[3] - 1.0, query[2], 1.0],
            [query[0], query[1], 1.0, query[3]],
            [query[0] + query[2] - 1.0, query[1], 1.0, query[3]],
        ] {
            ui::fill(game, batch, edge, 0xff000000 | accent);
        }
    }
    ui::icon(
        game,
        batch,
        ui::Icon::Search,
        [query[0] + 6.0, query[1] + 4.0, 10.0, 10.0],
        if editing { accent } else { 0x8fa9bb },
    );
    let (caption, color) = if let Some((item, edit)) = &s.watch_edit {
        let name = item.rsplit(':').next().unwrap_or(item);
        (
            if russian() {
                format!("{name} · монет/шт. ≤ {}│", edit.value)
            } else {
                format!("{name} · coins/each ≤ {}│", edit.value)
            },
            style::TEXT,
        )
    } else if s.query.value.is_empty() && !s.focus {
        (
            t(
                "Поиск: предмет, продавец или #номер…",
                "Search: item, seller or #number…",
            )
            .to_string(),
            0x7b8fa3,
        )
    } else {
        let cursor = if s.focus { "│" } else { "" };
        (format!("{}{cursor}", s.query.value), style::TEXT)
    };
    ui::text(
        game,
        batch,
        &ui::clip(game, &caption, query[2] - 28.0),
        [query[0] + 22.0, query[1] + 5.0],
        color,
        false,
    );
    s.hits.push((query, Hit::Search));
    // The lots: compact cards in as many columns as fit.
    let top = bar + 24.0;
    let card_h = 27.0;
    let footer = y + h - 24.0;
    let cols = ((w - 16.0) / 196.0).floor().clamp(1.0, 4.0) as usize;
    let rows = (((footer - top - 2.0) / (card_h + 3.0)).floor() as usize).max(1);
    let per = cols * rows;
    let pages_total = lots.len().div_ceil(per).max(1);
    s.catalog_pages = pages_total;
    s.catalog_page = s.catalog_page.min(pages_total - 1);
    let gw = (w - 16.0 + 4.0) / cols as f32;
    for (i, l) in lots.iter().skip(s.catalog_page * per).take(per).enumerate() {
        let saved = l.generation != generation;
        s.shown.insert(l.id, l.clone());
        if !s.stacks.contains_key(&l.id) {
            // The exact stack saved with the lot; a plain stack when there is none.
            let stack = data::saved_stack(l.id)
                .filter(|st| *st.item == *l.item)
                .unwrap_or_else(|| horizon_client::items::ItemStack::new(&l.item, 1));
            s.stacks.insert(l.id, stack);
        }
        let r = [
            x + 8.0 + (i % cols) as f32 * gw,
            top + (i / cols) as f32 * (card_h + 3.0),
            gw - 4.0,
            card_h,
        ];
        let deal = caps
            .get(&l.item)
            .is_some_and(|cap| l.within_unit_price(*cap));
        ui::button_base(game, batch, r, &l.id.to_string(), deal, mouse);
        // The count is in the title, so the icon is drawn without its stack number.
        if let Some(icon) = lot_icon(game, s, l) {
            game.draw_item(batch, &icon, r[0] + 5.0, r[1] + 5.5);
        }
        let price = if l.bid {
            if russian() {
                format!("ставка {}", economy::fmt(l.cost))
            } else {
                format!("bid {}", economy::fmt(l.cost))
            }
        } else {
            unit(l)
        };
        let total = if l.bid { time_left(l) } else { coins(l.cost) };
        let price_w = game.native_text_width(&price);
        let total_w = game.native_text_width(&total);
        let mut name = lot_title(game, l);
        if caps.contains_key(&l.item) {
            name.insert_str(0, "* ");
        }
        ui::text(
            game,
            batch,
            &ui::clip(game, &name, r[2] - 36.0 - price_w),
            [r[0] + 25.0, r[1] + 4.0],
            if deal {
                0x87e4af
            } else if saved {
                0xa9b6c4
            } else {
                style::TEXT
            },
            false,
        );
        ui::text(
            game,
            batch,
            &price,
            [r[0] + r[2] - 6.0 - price_w, r[1] + 4.0],
            if deal {
                0x87e4af
            } else if l.bid {
                0xc8b6ff
            } else {
                0xf0d68a
            },
            false,
        );
        let who = if l.bid {
            l.seller.clone()
        } else {
            format!("{} · {}", l.seller, time_left(l))
        };
        ui::text(
            game,
            batch,
            &ui::clip(game, &who, r[2] - 36.0 - total_w),
            [r[0] + 25.0, r[1] + 15.0],
            style::MUTED,
            false,
        );
        ui::text(
            game,
            batch,
            &total,
            [r[0] + r[2] - 6.0 - total_w, r[1] + 15.0],
            style::MUTED,
            false,
        );
        if style::contains(r, mouse) {
            s.hover_lot = Some(l.id);
        }
        s.hits.push((r, Hit::Lot(l.id)));
    }
    if lots.is_empty() {
        let text = if scanning {
            t("Загружаем лоты аукциона…", "Loading the auction's lots…")
        } else if total == 0 && finished == 0 {
            t(
                "Нет сохранённых лотов · нажмите «Обновить»",
                "No saved lots yet · press Refresh",
            )
        } else if total == 0 {
            t("На аукционе нет лотов", "The auction has no lots")
        } else {
            t("Ничего не найдено", "Nothing matches the search")
        };
        ui::text(
            game,
            batch,
            text,
            [x + 12.0, top + 6.0],
            style::MUTED,
            false,
        );
    }
    // Footer: hints on the left, the one pager in the middle.
    ui::fill(game, batch, [x + 8.0, footer, w - 16.0, 1.0], 0x22ffffff);
    let center = x + w * 0.5;
    let pager_y = footer + 4.0;
    let caption = format!("{} / {}", s.catalog_page + 1, pages_total);
    let caption_w = game.native_text_width(&caption);
    for (dir, r, icon, enabled) in [
        (
            -1,
            [center - caption_w * 0.5 - 30.0, pager_y, 22.0, 16.0],
            ui::Icon::Back,
            s.catalog_page > 0,
        ),
        (
            1,
            [center + caption_w * 0.5 + 8.0, pager_y, 22.0, 16.0],
            ui::Icon::Next,
            s.catalog_page + 1 < pages_total,
        ),
    ] {
        if enabled {
            ui::button_base(game, batch, r, "", false, mouse);
            s.hits.push((r, Hit::Pager(dir)));
        }
        ui::icon(
            game,
            batch,
            icon,
            [r[0] + 6.0, r[1] + 3.0, 10.0, 10.0],
            if enabled { style::TEXT } else { 0x4d5b68 },
        );
    }
    ui::text(
        game,
        batch,
        &caption,
        [center - caption_w * 0.5, pager_y + 4.0],
        0xc9d6e3,
        false,
    );
    let hint = if !s.notice.is_empty() {
        s.notice.clone()
    } else if s.watch_edit.is_some() {
        t(
            "Enter — сохранить · 0 — убрать · Esc — отменить",
            "Enter — save · 0 — remove · Esc — cancel",
        )
        .into()
    } else {
        if data::read(|_, prefs, _, _| prefs.flag("confirm_buy")) {
            t(
                "ЛКМ купить · ПКМ порог цены · F8 окно сервера",
                "LMB buy · RMB price alert · F8 server window",
            )
        } else {
            t(
                "ЛКМ купить сразу · ПКМ порог цены · F8 окно сервера",
                "LMB buys at once · RMB price alert · F8 server window",
            )
        }
        .into()
    };
    ui::text(
        game,
        batch,
        &ui::clip(
            game,
            &hint,
            (center - caption_w * 0.5 - 40.0 - x - 10.0).max(0.0),
        ),
        [x + 10.0, pager_y + 4.0],
        0xb8cdce,
        false,
    );
    if let Some(shown) = s.buy.clone() {
        render_buy(game, batch, s, screen, shown, balance);
    }
}
/// The purchase confirmation: what, from whom, for how much, and the command it sends.
fn render_buy(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    l: economy::Listing,
    balance: Option<economy::Coins>,
) {
    use crate::game::claim_borders_ui_style as style;
    s.hits.clear();
    s.hover = None;
    s.hover_lot = None;
    s.control_tip = None;
    // The dialog shows the lot as it was chosen; newer data never rewrites it silently.
    let now = data::now();
    let (ready, state_now) =
        data::read(|book, _, _, _| (book.buy_ready(&l, now), book.lot_state(l.id, now)));
    let form = style::DialogLayout::new([batch.width, batch.height], [360.0, 224.0]);
    let [mx, _, mw, _] = form.panel;
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x9009111c);
    ui::popup_frame(
        game,
        batch,
        form.panel,
        if l.bid {
            ui::Icon::Info
        } else {
            ui::Icon::Shield
        },
        if l.bid {
            t("Лот с торгами", "Auction lot")
        } else {
            t("Подтвердите покупку", "Confirm purchase")
        },
    );
    let [bx, by, _, _] = form.body;
    if let Some(icon) = lot_icon(game, s, &l) {
        game.draw_item(batch, &icon, bx, by);
    }
    ui::text(
        game,
        batch,
        &ui::clip(game, &lot_title(game, &l), mw - 48.0),
        [bx + 22.0, by + 4.0],
        style::TEXT,
        false,
    );
    let mut lines = vec![if russian() {
        format!("Продавец: {} · осталось {}", l.seller, time_left(&l))
    } else {
        format!("Seller: {} · {} left", l.seller, time_left(&l))
    }];
    if l.bid {
        lines.push(if russian() {
            format!("Текущая ставка: {}", coins(l.cost))
        } else {
            format!("Current bid: {}", coins(l.cost))
        });
        lines.push(
            t(
                "Ставки делаются в виде сервера (F8).",
                "Bids are placed in the server view (F8).",
            )
            .into(),
        );
    } else {
        lines.push(if russian() {
            format!("Цена: {} · {}", coins(l.cost), unit(&l))
        } else {
            format!("Price: {} · {}", coins(l.cost), unit(&l))
        });
        if let Some(balance) = balance {
            lines.push(if balance < l.cost {
                t(
                    "Похоже, монет не хватает.",
                    "You may not have enough coins.",
                )
                .into()
            } else if russian() {
                format!("После покупки: {}", coins(balance - l.cost))
            } else {
                format!("After buying: {}", coins(balance - l.cost))
            });
        }
        lines.push(if russian() {
            format!("Покупка сразу: /ah buy {} · без отмены", l.id)
        } else {
            format!("Buys at once: /ah buy {} · cannot be undone", l.id)
        });
    }
    for (i, line) in lines.iter().enumerate() {
        ui::text(
            game,
            batch,
            &ui::clip(game, line, mw - 24.0),
            [mx + 12.0, by + 26.0 + i as f32 * 12.0],
            if line.contains("enough") || line.contains("хватает") {
                0xe39ca8
            } else {
                0xb6c7d9
            },
            false,
        );
    }
    let warning = match (ready, state_now) {
        (Err(why), _) => Some((refusal(why), 0xe39ca8)),
        (Ok(()), economy::LotState::Saved) => Some((
            if russian() {
                format!(
                    "Сохранённые данные ({}): лот мог быть продан или изменён.",
                    ago(now.saturating_sub(l.seen))
                )
            } else {
                format!(
                    "Saved data ({}): the lot may be sold or changed.",
                    ago(now.saturating_sub(l.seen))
                )
            },
            0xf0d68a,
        )),
        _ => None,
    };
    if let Some((text, color)) = warning {
        ui::text(
            game,
            batch,
            &ui::clip(game, &text, mw - 24.0),
            [mx + 12.0, by + 26.0 + lines.len() as f32 * 12.0],
            color,
            false,
        );
    }
    let cancel = form.footer(0, false);
    let ok = form.footer(1, false);
    if !l.bid {
        let ask = data::read(|_, prefs, _, _| prefs.flag("confirm_buy"));
        let r = form.footer(0, true);
        let r = [r[0], r[1], ok[0] + ok[2] - r[0], r[3]];
        let label = format!(
            "{}: {}",
            t("Спрашивать перед покупкой", "Ask before buying"),
            if ask {
                t("вкл.", "on")
            } else {
                t("выкл.", "off")
            }
        );
        ui::button(game, batch, r, &label, false, screen.mouse);
        if style::contains(r, screen.mouse) {
            s.control_tip = Some(
                t(
                    "Выкл.: щелчок по лоту сразу покупает его. Меняется и в Торговля → Основное.",
                    "Off: clicking a lot buys it at once. Also under Trading → General.",
                )
                .into(),
            );
        }
        s.hits.push((r, Hit::AskToggle));
    }
    ui::button(
        game,
        batch,
        cancel,
        if l.bid {
            t("Понятно", "Got it")
        } else {
            t("Отмена [Esc]", "Cancel [Esc]")
        },
        false,
        screen.mouse,
    );
    s.hits.push((cancel, Hit::Cancel));
    if !l.bid && ready.is_ok() {
        ui::button(game, batch, ok, t("Купить", "Buy"), false, screen.mouse);
        s.hits.push((ok, Hit::Buy));
    }
}

fn label(game: &Game<'_>, page: &scan::Page, b: &scan::Button) -> String {
    use scan::Role::*;
    match scan::role(page, b) {
        Close => t("Закрыть", "Close").into(),
        Back => t("Назад", "Back").into(),
        Previous => t("Предыдущая страница", "Previous page").into(),
        Next => t("Следующая страница", "Next page").into(),
        Balance if !russian() => b.name.clone(),
        Balance => b
            .name
            .replace("Balance:", "Баланс:")
            .replace("coins", "монет"),
        News => t("Динамика · 24 часа", "Trends · 24 hours").into(),
        Help => t("Информация", "Information").into(),
        Mine => t("Мои объявления", "My listings").into(),
        Mail => t("Почта", "Mail").into(),
        ClaimAll => t("Забрать всё", "Claim all").into(),
        Claim => {
            let item = crate::game::claim_borders_economy_view::item_label(game, &b.item);
            if russian() {
                format!("Получить · {item}")
            } else {
                format!("Claim · {item}")
            }
        }
        Buy(n) if n > 0 => {
            if russian() {
                format!("Купить {n}")
            } else {
                format!("Buy {n}")
            }
        }
        Sell(n) => {
            if russian() {
                format!("Продать {n}")
            } else {
                format!("Sell {n}")
            }
        }
        SellAll => t("Продать всё", "Sell all").into(),
        Buy(_) => {
            let price = b
                .name
                .split_once(" for ")
                .map(|(_, v)| v)
                .unwrap_or(&b.name);
            if russian() {
                format!("Купить лот · {}", price.replace("coins", "монет"))
            } else {
                format!("Buy listing · {price}")
            }
        }
        Bid if !russian() => b.name.clone(),
        Bid => b
            .name
            .replacen("Bid ", "Ставка ", 1)
            .replace("coins", "монет"),
        Chart(1) => t("График · 24 часа", "Chart · 24 hours").into(),
        Chart(_) => t("График · 7 дней", "Chart · 7 days").into(),
        Goods | Preview => {
            if page.family == scan::Kind::Auction
                && !b.name.trim().is_empty()
                && !b.name.starts_with('#')
            {
                b.name.clone()
            } else {
                crate::game::claim_borders_economy_view::item_label(game, &b.item)
            }
        }
        Listing => {
            let facts = scan::listing_text(b).unwrap_or_default();
            let head = facts.split('·').next().unwrap_or("").trim();
            let count = head.split_whitespace().nth(1).unwrap_or("");
            let name = if !b.name.trim().is_empty() && !b.name.starts_with('#') {
                b.name.clone()
            } else {
                crate::game::claim_borders_economy_view::item_label(game, &b.item)
            };
            format!("{name} ×{count}")
        }
        // "All listings (264 listings)": the count goes on the card's second line.
        Category => local_category(category_count(&b.name).map_or(&b.name, |(name, _)| name)),
        Inspect => b.name.clone(),
    }
}
/// "Farming (13 listings)" -> ("Farming", "13").
fn category_count(raw: &str) -> Option<(&str, &str)> {
    let (name, rest) = raw.rsplit_once(" (")?;
    let count = rest.strip_suffix(" listings)")?;
    count
        .chars()
        .all(|c| c.is_ascii_digit())
        .then_some((name, count))
}
fn local_category(raw: &str) -> String {
    if !russian() {
        return raw.to_owned();
    }
    let mut s = raw.to_owned();
    for (en, ru) in [
        ("All listings", "Все лоты"),
        ("Farming", "Фермерство"),
        ("Mining", "Ресурсы"),
        ("Wood", "Древесина"),
        ("Mob drops", "Добыча с мобов"),
        ("Building", "Строительство"),
        ("Food", "Еда"),
        ("Tools", "Инструменты"),
        ("Other", "Разное"),
    ] {
        if s.starts_with(en) {
            s = s.replacen(en, ru, 1);
            break;
        }
    }
    s.replace(" listings", " лотов")
}
fn detail(game: &Game<'_>, page: &scan::Page, b: &scan::Button) -> String {
    use scan::Role::*;
    match scan::role(page, b) {
        Buy(_) | Sell(_) => b
            .name
            .split_once(" for ")
            .map(|(_, s)| {
                let value = s.replace(" coins", "").replace(" coin", "");
                if russian() {
                    format!("{value} монет")
                } else if value == "1" {
                    "1 coin".into()
                } else {
                    format!("{value} coins")
                }
            })
            .unwrap_or_default(),
        Listing => {
            let text = scan::listing_text(b).unwrap_or_default();
            let parts: Vec<_> = text.split('·').map(str::trim).collect();
            if parts.len() == 4 && !russian() {
                format!("{} · {} · {}", parts[1], parts[2], parts[3])
            } else if parts.len() == 4 {
                format!(
                    "{} · {} · {}",
                    parts[1].replace("coins", "монет").replace("each", "/ шт."),
                    parts[2],
                    parts[3].replace(" left", " осталось")
                )
            } else {
                t("Лот сервера", "Server listing").into()
            }
        }
        Goods | Preview
            if page.family == scan::Kind::Auction
                && b.lore.first().is_some_and(|l| l.starts_with('#')) =>
        {
            // "#7833 1 Wooden Pickaxe" -> "Lot #7833 · ×1"
            let mut parts = b.lore[0].split_whitespace();
            let id = parts.next().unwrap_or("");
            let count = parts.next().unwrap_or("1");
            if russian() {
                format!("Лот {id} · ×{count}")
            } else {
                format!("Lot {id} · ×{count}")
            }
        }
        Category if let Some((_, count)) = category_count(&b.name) => {
            if russian() {
                format!("{count} лотов")
            } else if count == "1" {
                "1 listing".into()
            } else {
                format!("{count} listings")
            }
        }
        _ => {
            let lore = b
                .lore
                .first()
                .map(|l| crate::game::claim_borders_economy_view::market_lore(game, l))
                .unwrap_or_else(|| match scan::role(page, b) {
                    Category => t("Открыть категорию", "Open category").into(),
                    Claim => t("Возвращённый товар / выигрыш", "Returned goods / won lots").into(),
                    _ => String::new(),
                });
            if russian() {
                lore.replace(" items", " товаров")
            } else {
                lore
            }
        }
    }
}
fn action_icon(role: scan::Role) -> ui::Icon {
    use scan::Role::*;
    match role {
        Close => ui::Icon::Close,
        Back | Previous => ui::Icon::Back,
        Next => ui::Icon::Next,
        Buy(_) => ui::Icon::Plus,
        Sell(_) | SellAll => ui::Icon::Minus,
        Chart(_) | News => ui::Icon::Chart,
        Mine => ui::Icon::Chest,
        Mail | ClaimAll | Claim => ui::Icon::Restock,
        Help => ui::Icon::Info,
        _ => ui::Icon::Market,
    }
}
fn control(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    r: [f32; 4],
    text: &str,
    hit: Hit,
    mouse: [f32; 2],
    icon: ui::Icon,
) {
    ui::button_base(game, batch, r, text, false, mouse);
    let trading = matches!(icon, ui::Icon::Plus | ui::Icon::Minus);
    let inset = if trading && r[2] < 100.0 { 5.0 } else { 19.0 };
    let color = if matches!(icon, ui::Icon::Minus) {
        0xdfa8b5
    } else {
        ui::preferences(game).accent.color()
    };
    if inset > 5.0 {
        ui::icon(
            game,
            batch,
            icon,
            [r[0] + 5.0, r[1] + (r[3] - 10.0) * 0.5, 10.0, 10.0],
            color,
        );
    }
    if trading && r[3] >= 23.0 {
        let (qty, price) = text.split_once(" · ").unwrap_or((text, ""));
        ui::text(
            game,
            batch,
            &ui::clip(game, qty, r[2] - inset - 5.0),
            [r[0] + inset, r[1] + 3.0],
            color,
            false,
        );
        ui::text(
            game,
            batch,
            &ui::clip(game, price, r[2] - inset - 5.0),
            [r[0] + inset, r[1] + 14.0],
            0xe3ebf3,
            false,
        );
    } else {
        ui::text(
            game,
            batch,
            &ui::clip(game, text, r[2] - inset - 5.0),
            [r[0] + inset, r[1] + (r[3] - 8.0) * 0.5],
            0xe3ebf3,
            false,
        );
    }
    if crate::game::claim_borders_ui_style::contains(r, mouse) {
        s.control_tip = Some(match &hit {
            Hit::Pager(n) if *n < 0 => t("Предыдущая страница", "Previous page").into(),
            Hit::Pager(_) => t("Следующая страница", "Next page").into(),
            Hit::Standard => t(
                "F8 — стандартный вид · ПКМ по лоту AH — порог избранного",
                "F8 — standard view · RMB on an AH listing — favorite threshold",
            )
            .into(),
            _ => text.into(),
        });
    }
    s.hits.push((r, hit));
}
fn goods(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    page: &scan::Page,
    b: &scan::Button,
    r: [f32; 4],
    mouse: [f32; 2],
) {
    let listing = fixed_listing(page, b);
    let cap = listing
        .as_ref()
        .and_then(|l| data::read(|book, _, _, _| book.auction_watch.get(&l.item).copied()));
    let deal = listing
        .as_ref()
        .zip(cap)
        .is_some_and(|(l, cap)| l.within_unit_price(cap));
    let mut title = label(game, page, b);
    if cap.is_some() {
        title.insert_str(0, "* ");
    }
    ui::button_base(game, batch, r, &title, deal, mouse);
    if let Some((_, stack)) = s.icons.iter().find(|(slot, _)| *slot == b.slot) {
        game.draw_item(batch, stack, r[0] + 5.0, r[1] + 7.0);
    }
    // A fixed-price lot: the price per unit stands out on the right, the lot price and the
    // seller (with the time left) sit below.
    if let Some(l) = &listing {
        let unit = l.cost as f64 / 100.0 / f64::from(l.count.max(1));
        let unit_text = if russian() {
            format!("{unit:.2} / шт.")
        } else {
            format!("{unit:.2} each")
        };
        let total = if russian() {
            format!("{} монет", economy::fmt(l.cost))
        } else {
            format!("{} coins", economy::fmt(l.cost))
        };
        let time = b
            .lore
            .iter()
            .find(|line| line.contains(" left"))
            .map(|line| line.replace(" left", "").trim().to_owned())
            .map(|v| {
                if russian() {
                    v.replace('d', "д").replace('h', "ч").replace('m', "м")
                } else {
                    v
                }
            });
        let who = match time {
            Some(time) => format!("{} · {time}", l.seller),
            None => l.seller.clone(),
        };
        let unit_w = game.native_text_width(&unit_text);
        let total_w = game.native_text_width(&total);
        ui::text(
            game,
            batch,
            &ui::clip(game, &title, r[2] - 40.0 - unit_w),
            [r[0] + 26.0, r[1] + 5.0],
            if deal { 0x87e4af } else { 0xe3ebf3 },
            false,
        );
        ui::text(
            game,
            batch,
            &unit_text,
            [r[0] + r[2] - 8.0 - unit_w, r[1] + 5.0],
            if deal { 0x87e4af } else { 0xf0d68a },
            false,
        );
        ui::text(
            game,
            batch,
            &ui::clip(game, &who, r[2] - 40.0 - total_w),
            [r[0] + 26.0, r[1] + 18.0],
            0x9aacc0,
            false,
        );
        ui::text(
            game,
            batch,
            &total,
            [r[0] + r[2] - 8.0 - total_w, r[1] + 18.0],
            0x9aacc0,
            false,
        );
        if crate::game::claim_borders_ui_style::contains(r, mouse) {
            s.hover = Some(b.slot);
        }
        s.hits.push((r, Hit::Server(b.clone())));
        return;
    }
    // A market item: its unit price on the right, the day's trend and trade limits below.
    if page.family == scan::Kind::Market
        && scan::role(page, b) == scan::Role::Goods
        && let Some((price, note)) = b.lore.first().and_then(|l| market_price(l))
    {
        let price = if russian() {
            format!("{price} / шт.")
        } else {
            format!("{price} each")
        };
        let trend = b
            .lore
            .iter()
            .find(|l| l.starts_with("24 h"))
            .map(|l| crate::game::claim_borders_economy_view::market_lore(game, l));
        let below = match (trend, note) {
            (Some(trend), Some(note)) => format!("{trend} · {note}"),
            (Some(trend), None) => trend,
            (None, Some(note)) => note.into(),
            (None, None) => String::new(),
        };
        let price_w = game.native_text_width(&price);
        ui::text(
            game,
            batch,
            &ui::clip(game, &title, r[2] - 40.0 - price_w),
            [r[0] + 26.0, r[1] + 5.0],
            0xe3ebf3,
            false,
        );
        ui::text(
            game,
            batch,
            &price,
            [r[0] + r[2] - 8.0 - price_w, r[1] + 5.0],
            0xf0d68a,
            false,
        );
        ui::text(
            game,
            batch,
            &ui::clip(game, &below, r[2] - 34.0),
            [r[0] + 26.0, r[1] + 18.0],
            if below.contains('▲') {
                0x87e4af
            } else if below.contains('▼') {
                0xe39ca8
            } else {
                0x9aacc0
            },
            false,
        );
        if crate::game::claim_borders_ui_style::contains(r, mouse) {
            s.hover = Some(b.slot);
        }
        s.hits.push((r, Hit::Server(b.clone())));
        return;
    }
    ui::text(
        game,
        batch,
        &ui::clip(game, &title, r[2] - 32.0),
        [r[0] + 26.0, r[1] + 5.0],
        if deal { 0x87e4af } else { 0xe3ebf3 },
        false,
    );
    ui::text(
        game,
        batch,
        &ui::clip(game, &detail(game, page, b), r[2] - 32.0),
        [r[0] + 26.0, r[1] + 18.0],
        0x9aacc0,
        false,
    );
    // A category's tooltip repeats its card, so only other entries show the server tooltip.
    if crate::game::claim_borders_ui_style::contains(r, mouse)
        && scan::role(page, b) != scan::Role::Category
    {
        s.hover = Some(b.slot);
    }
    s.hits.push((r, Hit::Server(b.clone())));
}
/// "Price: 1.56 coins each, to buy or sell" -> ("1.56", None); a one-way price gets a note.
fn market_price(line: &str) -> Option<(String, Option<&'static str>)> {
    let rest = line.strip_prefix("Price: ")?;
    let (price, tail) = rest.split_once(" coins each")?;
    let note = if tail.contains("doesn't buy") {
        Some(t("только покупка", "buy only"))
    } else if tail.contains("doesn't sell") {
        Some(t("только продажа", "sell only"))
    } else {
        None
    };
    Some((price.trim().to_owned(), note))
}
fn fixed_listing(page: &scan::Page, b: &scan::Button) -> Option<economy::Listing> {
    if page.family != scan::Kind::Auction || scan::role(page, b) != scan::Role::Listing {
        return None;
    }
    let listing = economy::listing(&scan::listing_text(b)?, &b.item, data::now(), 0)?;
    (!listing.bid).then_some(listing)
}
pub(super) fn render(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) -> bool {
    let Some((page, token)) = live(game) else {
        return false;
    };
    use scan::Role::*;
    let [x, y, w, h] = panel(batch);
    let start = batch.vertices.len();
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x80111b27);
    // The auction's home page starts the catalogue: the walk runs behind it, and its pages
    // are never drawn. Every new opening walks the auction again (see `scan::Session`).
    if auction_home(&page) {
        let running = data::read(|_, _, status, _| status.catalog);
        let start = state(|s| {
            let refresh = s.refresh.as_ref().is_some_and(|old| *old != token);
            if refresh {
                s.refresh = None;
            }
            s.session.home(running, refresh)
        });
        if start {
            data::start_catalog();
        }
    }
    // A server confirmation page (Buy / Bid buttons) is shown as itself.
    let confirming = page
        .buttons
        .iter()
        .any(|b| matches!(scan::role(&page, b), Buy(_) | Bid));
    if page.family == scan::Kind::Auction && !confirming && state(|s| s.session.shown()) {
        state(|s| {
            s.token = token.clone();
            render_catalog(game, batch, s, screen, [x, y, w, h]);
        });
        ui::finish(game, batch, start, "commerce", "catalog");
        tooltip(game, batch, screen);
        return true;
    }
    state(|s| {
        if s.token != token {
            let first = s.token.is_empty();
            s.token = token.clone();
            s.pending = None;
            s.modal_scroll = 0;
            s.notice.clear();
            s.watch_edit = None;
            // A page turn continues the numbering; any other new window starts it again, and
            // a refresh of the same window (no request) keeps the place.
            match if first {
                Some(Nav::Other)
            } else {
                s.nav.take()
            } {
                Some(Nav::Forward(pages)) => {
                    s.passed.push(pages);
                    s.page = 0;
                }
                Some(Nav::Backward) => {
                    s.passed.pop();
                    s.page = usize::MAX;
                }
                Some(Nav::Other) => {
                    s.passed.clear();
                    s.page = 0;
                    s.query = TextInput::default();
                    s.focus = false;
                }
                None => {}
            }
        }
        s.hits.clear();
        s.hover = None;
        s.control_tip = None;
        ui::rounded(
            game,
            batch,
            [x - 3.0, y - 1.0, w + 6.0, h + 8.0],
            10.0,
            0x40000000,
        );
        ui::rounded(
            game,
            batch,
            [x, y, w, h],
            8.0,
            0xff000000 | crate::game::claim_borders_ui_style::BORDER,
        );
        ui::rounded(
            game,
            batch,
            [x + 1.0, y + 1.0, w - 2.0, h - 2.0],
            7.0,
            0xf4000000 | crate::game::claim_borders_ui_style::BACKGROUND,
        );
        ui::icon(
            game,
            batch,
            ui::Icon::Market,
            [x + 10.0, y + 10.0, 13.0, 13.0],
            ui::preferences(game).accent.color(),
        );
        let title = if page.family == scan::Kind::Market {
            if scan::market_detail(&page) {
                t("Товар", "Item details")
            } else {
                t("Рынок", "Market")
            }
        } else {
            t("Аукцион", "Auction")
        };

        let balance = page
            .buttons
            .iter()
            .find(|b| scan::role(&page, b) == Balance)
            .map(|b| label(game, &page, b))
            .unwrap_or_else(|| {
                data::read(|book, _, _, _| {
                    let v = book.balance.map_or("—".into(), |(v, _)| economy::fmt(v));
                    if russian() {
                        format!("Баланс: {v}")
                    } else {
                        format!("Balance: {v}")
                    }
                })
            });
        let bw = (w * 0.45).min(220.0);
        let balance = ui::clip(game, &balance, bw);
        let balance_w = game.native_text_width(&balance);
        ui::text(
            game,
            batch,
            &ui::clip(game, title, (w - balance_w - 86.0).max(0.0)),
            [x + 30.0, y + 12.0],
            0xe3ebf3,
            false,
        );
        ui::rounded(
            game,
            batch,
            [x + w - balance_w - 49.0, y + 7.0, balance_w + 14.0, 17.0],
            8.0,
            0x30f0d68a,
        );
        ui::text(
            game,
            batch,
            &balance,
            [x + w - balance_w - 42.0, y + 12.0],
            0xf0d68a,
            false,
        );
        ui::fill(game, batch, [x + 8.0, y + 28.0, w - 16.0, 1.0], 0x22ffffff);
        if let Some(b) = page.buttons.iter().find(|b| scan::role(&page, b) == Close) {
            control(
                game,
                batch,
                s,
                [x + w - 28.0, y + 6.0, 21.0, 20.0],
                "",
                Hit::Server(b.clone()),
                screen.mouse,
                ui::Icon::Close,
            );
        }
        // One button per kind: some pages carry two Back buttons.
        let mut seen_roles = Vec::new();
        let services: Vec<_> = page
            .buttons
            .iter()
            .filter(|b| {
                matches!(
                    scan::role(&page, b),
                    Back | News | Help | Mine | Mail | ClaimAll
                )
            })
            .filter(|b| {
                let role = scan::role(&page, b);
                if seen_roles.contains(&role) {
                    false
                } else {
                    seen_roles.push(role);
                    true
                }
            })
            .collect();
        let cw = (w - 16.0) / services.len().max(1) as f32;
        for (i, b) in services.iter().enumerate() {
            control(
                game,
                batch,
                s,
                [x + 8.0 + i as f32 * cw, y + 32.0, cw - 3.0, 20.0],
                &label(game, &page, b),
                Hit::Server((*b).clone()),
                screen.mouse,
                action_icon(scan::role(&page, b)),
            );
        }
        if scan::market_detail(&page) {
            if let Some(b) = page
                .buttons
                .iter()
                .find(|b| scan::role(&page, b) == Preview)
            {
                let lines = if h >= 300.0 {
                    5
                } else if h >= 220.0 {
                    2
                } else {
                    0
                };
                goods(
                    game,
                    batch,
                    s,
                    &page,
                    b,
                    [
                        x + 8.0,
                        y + 58.0,
                        w - 16.0,
                        if h >= 220.0 { 30.0 } else { 24.0 },
                    ],
                    screen.mouse,
                );
                let mut lore: Vec<_> = b
                    .lore
                    .iter()
                    .filter(|l| !l.starts_with("Price: "))
                    .collect();
                if h < 300.0 {
                    lore.sort_by_key(|l| !l.starts_with("You may still sell "));
                }
                // The figures of the item as tiles: the colour stripe says up, down or neutral.
                let cols = if w >= 480.0 { 3 } else { 2 };
                let shown = lore.len().min(if lines == 0 { 0 } else { cols * 2 });
                let tile_w = (w - 16.0 - (cols as f32 - 1.0) * 4.0) / cols as f32;
                for (i, l) in lore.into_iter().take(shown).enumerate() {
                    let text = crate::game::claim_borders_economy_view::market_lore(game, l);
                    let tile = [
                        x + 8.0 + (i % cols) as f32 * (tile_w + 4.0),
                        y + 92.0 + (i / cols) as f32 * 24.0,
                        tile_w,
                        21.0,
                    ];
                    let stripe = if text.contains('▲') {
                        0xff87e4af
                    } else if text.contains('▼') {
                        0xffe39ca8
                    } else {
                        0xff5d7690
                    };
                    ui::rounded(game, batch, tile, 4.0, 0xff1c2a39);
                    ui::fill(game, batch, [tile[0], tile[1] + 3.0, 2.0, 15.0], stripe);
                    ui::text(
                        game,
                        batch,
                        &ui::clip(game, &text, tile_w - 14.0),
                        [tile[0] + 8.0, tile[1] + 6.0],
                        0xc9d6e3,
                        false,
                    );
                }
                let top = if shown == 0 {
                    y + 86.0
                } else {
                    y + 96.0 + shown.div_ceil(cols) as f32 * 24.0
                };
                let tw = (w - 16.0) / 3.0;
                let th = if h >= 300.0 {
                    30.0
                } else if h >= 220.0 {
                    23.0
                } else {
                    16.0
                };
                for (side, selling) in [false, true].into_iter().enumerate() {
                    let controls: Vec<_> = page
                        .buttons
                        .iter()
                        .filter(|b| {
                            if selling {
                                matches!(scan::role(&page, b), Sell(_))
                            } else {
                                matches!(scan::role(&page, b), Buy(_))
                            }
                        })
                        .collect();
                    for (i, b) in controls.iter().enumerate() {
                        let caption =
                            format!("{} · {}", label(game, &page, b), detail(game, &page, b));
                        control(
                            game,
                            batch,
                            s,
                            [
                                x + 8.0 + i as f32 * tw,
                                top + side as f32 * (th + 3.0),
                                tw - 4.0,
                                th,
                            ],
                            &caption,
                            Hit::Server((*b).clone()),
                            screen.mouse,
                            action_icon(scan::role(&page, b)),
                        );
                    }
                }
                let extra: Vec<_> = page
                    .buttons
                    .iter()
                    .filter(|b| matches!(scan::role(&page, b), SellAll | Chart(_)))
                    .collect();
                for (i, b) in extra.iter().enumerate() {
                    control(
                        game,
                        batch,
                        s,
                        [
                            x + 8.0 + i as f32 * tw,
                            top + 2.0 * (th + 3.0),
                            tw - 4.0,
                            if h >= 220.0 { 20.0 } else { 16.0 },
                        ],
                        &label(game, &page, b),
                        Hit::Server((*b).clone()),
                        screen.mouse,
                        action_icon(scan::role(&page, b)),
                    );
                }
            }
        } else {
            let confirming = page.family == scan::Kind::Auction
                && page
                    .buttons
                    .iter()
                    .any(|b| matches!(scan::role(&page, b), Buy(_) | Bid));
            let sort_w = 96.0_f32.min(w * 0.25);
            let query = [x + 8.0, y + 58.0, w - 20.0 - sort_w, 18.0];
            if !confirming {
                let sort_rect = [x + w - 8.0 - sort_w, y + 58.0, sort_w, 18.0];
                ui::button(
                    game,
                    batch,
                    sort_rect,
                    match s.sort {
                        1 => t("Цена/шт. ↑", "Unit price ↑"),
                        2 => t("Цена лота ↑", "Lot price ↑"),
                        _ => t("Порядок: сервер", "Order: server"),
                    },
                    s.sort != 0,
                    screen.mouse,
                );
                s.hits.push((sort_rect, Hit::Sort));
                let editing = s.focus || s.watch_edit.is_some();
                let accent = ui::preferences(game).accent.color();
                ui::rounded(
                    game,
                    batch,
                    query,
                    4.0,
                    if editing { 0xff16303a } else { 0xff0e1620 },
                );
                if editing {
                    for edge in [
                        [query[0], query[1], query[2], 1.0],
                        [query[0], query[1] + query[3] - 1.0, query[2], 1.0],
                        [query[0], query[1], 1.0, query[3]],
                        [query[0] + query[2] - 1.0, query[1], 1.0, query[3]],
                    ] {
                        ui::fill(game, batch, edge, 0xff000000 | accent);
                    }
                }
                ui::icon(
                    game,
                    batch,
                    ui::Icon::Search,
                    [query[0] + 6.0, query[1] + 4.0, 10.0, 10.0],
                    if editing { accent } else { 0x8fa9bb },
                );
                let (caption, color) = if let Some((item, edit)) = &s.watch_edit {
                    let name = item.rsplit(':').next().unwrap_or(item);
                    (
                        if russian() {
                            format!("{name} · монет/шт. ≤ {}│", edit.value)
                        } else {
                            format!("{name} · coins/each ≤ {}│", edit.value)
                        },
                        0xe3ebf3,
                    )
                } else if s.query.value.is_empty() && !s.focus {
                    (
                        t("Поиск по названию…", "Search by name…").to_string(),
                        0x7b8fa3,
                    )
                } else {
                    let cursor = if s.focus { "│" } else { "" };
                    (format!("{}{cursor}", s.query.value), 0xe3ebf3)
                };
                ui::text(
                    game,
                    batch,
                    &ui::clip(game, &caption, query[2] - 28.0),
                    [query[0] + 22.0, query[1] + 5.0],
                    color,
                    false,
                );
                s.hits.push((query, Hit::Search));
            }
            let cols = if w >= 820.0 {
                4
            } else if w >= 560.0 {
                3
            } else if w >= 300.0 {
                2
            } else {
                1
            };
            let rows = (((h - 140.0) / 33.0).floor() as usize).max(1);
            let per = cols * rows;
            let entries: Vec<_> = page
                .buttons
                .iter()
                .filter(|b| {
                    (!b.name.trim().is_empty() || scan::role(&page, b) != Inspect)
                        && !scan::role(&page, b).service()
                })
                .filter(|b| {
                    crate::game::claim_borders_input::matches(
                        &s.query.value,
                        &format!("{} {} {}", label(game, &page, b), b.item, b.lore.join(" ")),
                    )
                })
                .collect();
            let mut entries = entries;
            if s.sort != 0 {
                let price = |b: &scan::Button| {
                    fixed_listing(&page, b).map_or(f64::MAX, |l| {
                        if s.sort == 1 {
                            l.cost as f64 / f64::from(l.count.max(1))
                        } else {
                            l.cost as f64
                        }
                    })
                };
                entries.sort_by(|a, b| price(a).total_cmp(&price(b)));
            }
            let pages = entries.len().div_ceil(per).max(1);
            s.page = s.page.min(pages - 1);
            let gw = (w - 16.0) / cols as f32;
            for (i, b) in entries.iter().skip(s.page * per).take(per).enumerate() {
                goods(
                    game,
                    batch,
                    s,
                    &page,
                    b,
                    [
                        x + 8.0 + (i % cols) as f32 * gw,
                        y + if confirming { 58.0 } else { 82.0 } + (i / cols) as f32 * 33.0,
                        gw - 4.0,
                        30.0,
                    ],
                    screen.mouse,
                );
            }
            if entries.is_empty() {
                ui::text(
                    game,
                    batch,
                    t("Нет предметов на этой странице", "No items on this page"),
                    [x + 12.0, y + 88.0],
                    0x9aacc0,
                    false,
                );
            }
            // One pager: it steps through the cards, then turns the server's page at either
            // end, so a long list reads as one numbered sequence.
            let bottom = y + h - 46.0;
            let previous = page
                .buttons
                .iter()
                .find(|b| scan::role(&page, b) == Previous)
                .cloned();
            let next = page
                .buttons
                .iter()
                .find(|b| scan::role(&page, b) == Next)
                .cloned();
            s.pager = (pages, previous.clone(), next.clone());
            if pages > 1 || previous.is_some() || next.is_some() {
                let passed: usize = s.passed.iter().sum();
                let number = passed + s.page + 1;
                let caption = if next.is_none() {
                    format!("{number} / {}", passed + pages)
                } else if russian() {
                    format!("Стр. {number}")
                } else {
                    format!("Page {number}")
                };
                let center = x + w * 0.5;
                let back = s.page > 0 || previous.is_some();
                let forward = s.page + 1 < pages || next.is_some();
                for (rect, enabled, dir, icon, label) in [
                    (
                        [center - 104.0, bottom, 64.0, 18.0],
                        back,
                        -1,
                        ui::Icon::Back,
                        t("Назад", "Back"),
                    ),
                    (
                        [center + 40.0, bottom, 64.0, 18.0],
                        forward,
                        1,
                        ui::Icon::Next,
                        t("Далее", "Next"),
                    ),
                ] {
                    if enabled {
                        control(
                            game,
                            batch,
                            s,
                            rect,
                            label,
                            Hit::Pager(dir),
                            screen.mouse,
                            icon,
                        );
                    } else {
                        ui::rounded(
                            game,
                            batch,
                            rect,
                            4.0,
                            0x60000000 | crate::game::claim_borders_ui_style::SURFACE,
                        );
                        ui::text(
                            game,
                            batch,
                            label,
                            [rect[0] + 19.0, rect[1] + 5.0],
                            0x5d6b78,
                            false,
                        );
                    }
                }
                ui::text(
                    game,
                    batch,
                    &caption,
                    [
                        center - game.native_text_width(&caption) * 0.5,
                        bottom + 5.0,
                    ],
                    0xc9d6e3,
                    false,
                );
            }
            // Server confirmation buttons are controls, never items in the results grid.
            let trades: Vec<_> = page
                .buttons
                .iter()
                .filter(|b| matches!(scan::role(&page, b), Buy(_) | Bid))
                .collect();
            if !trades.is_empty() {
                for (i, b) in trades.iter().enumerate() {
                    control(
                        game,
                        batch,
                        s,
                        [x + 8.0, y + h - 79.0 + i as f32 * 22.0, w - 16.0, 20.0],
                        &label(game, &page, b),
                        Hit::Server((*b).clone()),
                        screen.mouse,
                        action_icon(scan::role(&page, b)),
                    );
                }
            }
        }
        let status = data::read(|_, _, status, _| {
            if status.scanning {
                if russian() {
                    format!(
                        "Синхронизация · {} страниц · нажатие остановит обход",
                        status.pages
                    )
                } else {
                    format!("Syncing · {} pages · click to stop the scan", status.pages)
                }
            } else {
                t(
                    "Цены обновляются по данным сервера",
                    "Prices update from server data",
                )
                .into()
            }
        });
        ui::fill(
            game,
            batch,
            [x + 8.0, y + h - 24.0, w - 16.0, 1.0],
            0x22ffffff,
        );
        let raw_w = (w * 0.3).clamp(96.0, 168.0);
        ui::text(
            game,
            batch,
            &ui::clip(
                game,
                if !s.notice.is_empty() {
                    &s.notice
                } else if s.watch_edit.is_some() {
                    t(
                        "Enter — сохранить · 0 — убрать · Esc — отменить",
                        "Enter — save · 0 — remove · Esc — cancel",
                    )
                } else {
                    &status
                },
                w - raw_w - 24.0,
            ),
            [x + 10.0, y + h - 16.0],
            0xb8cdce,
            false,
        );
        let raw = [x + w - raw_w - 8.0, y + h - 20.0, raw_w, 16.0];
        ui::button(
            game,
            batch,
            raw,
            t("Стандартный вид · F8", "Standard view · F8"),
            false,
            screen.mouse,
        );
        s.hits.push((raw, Hit::Standard));
        if let Some(p) = s.pending.clone() {
            s.hits.clear();
            s.hover = None;
            s.control_tip = None;
            let role = scan::role(&page, &p.button);
            let transaction = role.transaction() || role == Listing;
            let form = crate::game::claim_borders_ui_style::DialogLayout::new(
                [batch.width, batch.height],
                [(w - 16.0).min(440.0), (h - 16.0).min(270.0)],
            );
            let [mx, my, mw, mh] = form.panel;
            game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x9009111c);
            ui::popup_frame(
                game,
                batch,
                form.panel,
                if transaction {
                    ui::Icon::Shield
                } else {
                    ui::Icon::Info
                },
                if transaction {
                    t("Подтверждение действия", "Confirm action")
                } else {
                    t("Информация", "Information")
                },
            );
            ui::text(
                game,
                batch,
                &ui::clip(game, &label(game, &page, &p.button), mw - 20.0),
                [form.body[0], form.body[1]],
                0xe3ebf3,
                false,
            );
            let mut lines = vec![detail(game, &page, &p.button)];
            if transaction {
                if let Some(preview) = page
                    .buttons
                    .iter()
                    .find(|b| scan::role(&page, b) == Preview)
                {
                    lines.push(label(game, &page, preview));
                    lines.push(detail(game, &page, preview));
                }
            }
            if role == Help {
                lines = [
                    (
                        "Выберите лот, затем подтвердите покупку или ставку.",
                        "Choose a listing, then confirm the purchase or bid.",
                    ),
                    (
                        "Мои объявления — ваши выставленные товары.",
                        "My listings — your items for sale.",
                    ),
                    (
                        "Почта — непроданные товары и выигранные лоты.",
                        "Mail — unsold items and won lots.",
                    ),
                    (
                        "Забрать всё — перенести доступные посылки в инвентарь.",
                        "Claim all — move available parcels to your inventory.",
                    ),
                    (
                        "/ah sell <цена> [количество] — выставить предмет из руки.",
                        "/ah sell <price> [amount] — list the item in your hand.",
                    ),
                    (
                        "/ah auction <цена> [часы] — создать торги.",
                        "/ah auction <price> [hours] — start an auction.",
                    ),
                    (
                        "/ah claim — забрать почту.",
                        "/ah claim — collect your mail.",
                    ),
                ]
                .into_iter()
                .map(|(ru, en)| t(ru, en).into())
                .collect();
            } else if role == ClaimAll {
                lines.push(
                    t(
                        "Забрать все доступные посылки в инвентарь.",
                        "Claim all available parcels to your inventory.",
                    )
                    .into(),
                );
            } else if role == SellAll {
                lines.push(
                    t(
                        "Продать весь доступный запас этого товара.",
                        "Sell the entire available stock of this item.",
                    )
                    .into(),
                );
            } else {
                lines.extend(
                    p.button
                        .lore
                        .iter()
                        .map(|l| crate::game::claim_borders_economy_view::market_lore(game, l)),
                );
            }
            lines.dedup();
            let max = (((form.body[3] - 34.0) / 11.0).floor() as usize).max(1);
            let lines: Vec<_> = lines
                .iter()
                .filter(|l| !l.is_empty())
                .flat_map(|l| {
                    crate::game::claim_borders_chat_feed::wrap(l, mw - 20.0, |s| {
                        game.native_text_width(s)
                    })
                })
                .take(100)
                .collect();
            s.modal_scroll = s.modal_scroll.min(lines.len().saturating_sub(max));
            for (i, l) in lines.iter().skip(s.modal_scroll).take(max).enumerate() {
                ui::text(
                    game,
                    batch,
                    &ui::clip(game, l, mw - 20.0),
                    [form.body[0], form.body[1] + 22.0 + i as f32 * 11.0],
                    0xb6c7d9,
                    false,
                );
            }
            let cr = form.footer(0, false);
            let ok = form.footer(1, false);
            ui::button(
                game,
                batch,
                cr,
                if transaction {
                    t("Отмена", "Cancel")
                } else {
                    t("Понятно", "Got it")
                },
                false,
                screen.mouse,
            );
            s.hits.push((cr, Hit::Cancel));
            if transaction {
                ui::button(
                    game,
                    batch,
                    ok,
                    match role {
                        Buy(_) => t("Купить", "Buy"),
                        Sell(_) | SellAll => t("Продать", "Sell"),
                        Bid => t("Сделать ставку", "Place bid"),
                        Claim | ClaimAll => t("Забрать", "Claim"),
                        _ => t("Подтвердить выбор", "Confirm selection"),
                    },
                    false,
                    screen.mouse,
                );
                s.hits.push((ok, Hit::Confirm));
            }
            if transaction || lines.len() > max {
                ui::text(
                    game,
                    batch,
                    &ui::clip(
                        game,
                        if lines.len() > max {
                            t(
                                "Колесо — прокрутка описания",
                                "Wheel — scroll the description",
                            )
                        } else {
                            t("Проверьте количество и сумму", "Check the amount and total")
                        },
                        mw - 20.0,
                    ),
                    [mx + 10.0, my + mh - 43.0],
                    0x9aacc0,
                    false,
                );
            }
        }
    });
    ui::finish(game, batch, start, "commerce", "live");
    tooltip(game, batch, screen);
    true
}
fn tooltip(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    if let Some((id, stack)) = state(|s| s.hover_lot.map(|id| (id, s.stacks.get(&id).cloned()))) {
        let lines = stack
            .map(|stack| game.stack_tooltip(&stack))
            .unwrap_or_default();
        if lines.is_empty()
            && let Some(l) = data::read(|book, _, _, _| book.listings.get(&id).cloned())
        {
            let text = format!(
                "{}\n{} · {}\n#{}",
                lot_title(game, &l),
                l.seller,
                coins(l.cost),
                l.id
            );
            ui::tooltip(game, batch, &text, screen.mouse);
            return;
        }
        game.draw_tooltip_lines(batch, &lines, screen.mouse[0], screen.mouse[1]);
        return;
    }
    let hover = state(|s| {
        s.hover.and_then(|slot| {
            s.icons
                .iter()
                .find(|(i, _)| *i == slot)
                .map(|(_, stack)| stack.clone())
        })
    });
    if let Some(stack) = hover {
        let lines = game.stack_tooltip(&stack);
        game.draw_tooltip_lines(batch, &lines, screen.mouse[0], screen.mouse[1]);
    } else if let Some(tip) = state(|s| s.control_tip.clone()).filter(|s| !s.is_empty()) {
        ui::tooltip(game, batch, &tip, screen.mouse);
    }
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    pressed: bool,
    canceled: &mut bool,
) -> bool {
    if !pressed && state(|s| s.pressed.remove(&button)) {
        *canceled = true;
        return true;
    }
    let Some((page, token)) = live(game) else {
        return false;
    };
    *canceled = true;
    if !pressed {
        return true;
    }
    let standard = state(|s| {
        s.pressed.insert(button);
        if s.token != token || s.watch_edit.is_some() {
            return false;
        }
        let hit = s
            .hits
            .iter()
            .rev()
            .find(|(r, _)| crate::game::claim_borders_ui_style::contains(*r, screen.mouse))
            .map(|(_, h)| h.clone());
        if button == 1
            && s.buy.is_none()
            && let Some(Hit::Lot(id)) = &hit
            && let Some(listing) = data::read(|book, _, _, _| book.listings.get(id).cloned())
        {
            let cap = data::read(|book, _, _, _| book.auction_watch.get(&listing.item).copied());
            s.watch_edit = Some((
                listing.item,
                TextInput::new(cap.map(economy::fmt).unwrap_or_default()),
            ));
            s.focus = false;
            s.notice.clear();
            return false;
        }
        if button == 1
            && s.pending.is_none()
            && let Some(Hit::Server(b)) = &hit
            && page.buttons.contains(b)
            && let Some(listing) = fixed_listing(&page, b)
        {
            let cap = data::read(|book, _, _, _| book.auction_watch.get(&listing.item).copied());
            s.watch_edit = Some((
                listing.item,
                TextInput::new(cap.map(economy::fmt).unwrap_or_default()),
            ));
            s.focus = false;
            s.notice.clear();
            return false;
        }
        if button != 0 {
            return false;
        }
        s.focus = false;
        match hit {
            Some(Hit::Server(b)) => {
                let p = Pending {
                    token: token.clone(),
                    button: b.clone(),
                    at: data::now(),
                };
                let role = scan::role(&page, &b);
                let own_lot = role == scan::Role::Listing
                    && scan::listing_text(&b).is_some_and(|text| {
                        let seller = text.split('·').nth(2).unwrap_or("").trim();
                        data::read(|_, _, _, own| seller.eq_ignore_ascii_case(own))
                    });
                if role.navigation() || (role == scan::Role::Listing && !own_lot) {
                    s.nav = Some(match role {
                        scan::Role::Next => Nav::Forward(s.pager.0),
                        scan::Role::Previous => Nav::Backward,
                        _ => Nav::Other,
                    });
                    s.action = Some(p)
                } else {
                    s.modal_scroll = 0;
                    s.pending = Some(p)
                }
            }
            Some(Hit::Confirm) => {
                s.nav = Some(Nav::Other);
                s.action = s.pending.take();
            }
            Some(Hit::Cancel) => {
                s.pending = None;
                s.buy = None;
            }
            Some(Hit::Lot(id)) => {
                // Without the question a click buys at once, except lots taking bids, saved
                // lots the current walk has not seen, and purchases the known balance cannot
                // cover. It acts on the card as drawn, checked against the data now.
                if let Some(shown) = s.shown.get(&id).cloned() {
                    let now = data::now();
                    let direct = data::read(|book, prefs, _, _| {
                        book.direct_buy_allowed(
                            &shown,
                            prefs.flag("confirm_buy"),
                            book.balance.map(|(v, _)| v),
                            now,
                        )
                    });
                    if direct {
                        buy(s, shown);
                    } else {
                        s.buy = Some(shown);
                        s.notice.clear();
                    }
                }
            }
            Some(Hit::Buy) => {
                if let Some(shown) = s.buy.take() {
                    buy(s, shown);
                }
            }
            Some(Hit::AskToggle) => {
                s.ask_toggle = true;
            }
            Some(Hit::Command("")) => s.close = true,
            Some(Hit::Command(command)) => {
                // My listings and Mail are server pages; the catalogue steps aside for them.
                data::cancel_scan();
                s.session.leave();
                s.command = Some(command.into());
            }
            Some(Hit::Refresh) => {
                // The walk starts from the home page: from there at once, otherwise once the
                // reopened auction arrives.
                data::cancel_scan();
                s.order.clear();
                if auction_home(&page) {
                    data::start_catalog();
                } else {
                    s.refresh = Some(token.clone());
                    s.command = Some("ah".into());
                }
            }
            Some(Hit::Pager(dir)) if s.session.shown() => catalog_step(s, dir),
            Some(Hit::Pager(dir)) => step(s, &token, dir),
            Some(Hit::Search) => s.focus = true,
            Some(Hit::Sort) if s.session.shown() => {
                s.catalog_sort = (s.catalog_sort + 1) % 4;
                s.catalog_page = 0;
            }
            Some(Hit::Sort) => {
                s.sort = (s.sort + 1) % 3;
                s.page = 0;
            }
            Some(Hit::Standard) => return true,
            _ => {}
        }
        false
    });
    if standard {
        toggle_view(game, page.family);
    }
    if state(|s| std::mem::take(&mut s.ask_toggle)) {
        data::configure(game, "confirm_buy", 1);
    }
    true
}
pub(super) fn key(game: &Game<'_>, key: &str, canceled: &mut bool) -> bool {
    if key == "key.keyboard.f8" && game.native_on(claims::ID) {
        if let Some((page, _)) = data::server_page(game) {
            toggle_view(game, page.family);
            *canceled = true;
            return true;
        }
    }
    if live(game).is_none() {
        return false;
    }
    if state(|s| s.watch_edit.is_some()) {
        *canceled = true;
        state(|s| {
            let Some((item, edit)) = s.watch_edit.as_mut() else {
                return;
            };
            match key {
                "key.keyboard.escape" => {
                    s.watch_edit = None;
                    s.notice.clear();
                }
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                    if let Some(cap) = economy::money(&edit.value) {
                        s.notice = if data::auction_watch(item, cap) {
                            s.watch_edit = None;
                            if cap == 0 {
                                t("Удалено из избранного", "Removed from favorites").into()
                            } else {
                                if russian() {
                                    format!("Порог сохранён: {} монет/шт.", economy::fmt(cap))
                                } else {
                                    format!("Threshold saved: {} coins/each", economy::fmt(cap))
                                }
                            }
                        } else {
                            t(
                                "Не удалось сохранить порог · лимит 64 позиции",
                                "Could not save the threshold · limit of 64 entries",
                            )
                            .into()
                        };
                    } else {
                        s.notice =
                            t("Введите цену, например 12.50", "Enter a price, e.g. 12.50").into();
                    }
                }
                _ => match edit.key(
                    key,
                    game.modifiers.control_key(),
                    game.modifiers.shift_key(),
                ) {
                    Edit::Copy(t) => s.copy = Some(t),
                    Edit::Paste => s.paste = true,
                    _ => {}
                },
            }
        });
        return true;
    }
    if key == "key.keyboard.escape" {
        if state(|s| {
            s.pending.take().is_some() || s.buy.take().is_some() || std::mem::take(&mut s.focus)
        }) {
            *canceled = true;
            return true;
        }
        return false;
    }
    if key == "key.keyboard.e" && !state(|s| s.focus) {
        return false;
    }
    *canceled = true;
    state(|s| {
        if game.modifiers.control_key() && key == "key.keyboard.f" {
            s.focus = true;
            return;
        }
        if s.focus {
            match s.query.key(
                key,
                game.modifiers.control_key(),
                game.modifiers.shift_key(),
            ) {
                Edit::Copy(t) => s.copy = Some(t),
                Edit::Paste => s.paste = true,
                _ => {}
            }
            s.page = 0;
            s.catalog_page = 0;
        }
    });
    true
}
pub(super) fn character(game: &Game<'_>, c: char, canceled: &mut bool) -> bool {
    if live(game).is_none() {
        return false;
    }
    *canceled = true;
    if !game.modifiers.control_key() {
        state(|s| {
            if let Some((_, edit)) = &mut s.watch_edit {
                edit.insert(&c.to_string(), 20);
                s.notice.clear();
            } else if s.focus {
                s.query.insert(&c.to_string(), 64);
                s.page = 0;
                s.catalog_page = 0;
            }
        });
    }
    true
}
pub(super) fn scroll(game: &Game<'_>, amount: f64, canceled: &mut bool) -> bool {
    if live(game).is_none() {
        return false;
    }
    *canceled = true;
    state(|s| {
        if s.pending.is_some() {
            s.modal_scroll = if amount > 0.0 {
                s.modal_scroll.saturating_sub(1)
            } else {
                s.modal_scroll.saturating_add(1)
            };
        } else if s.session.shown() {
            catalog_step(s, if amount > 0.0 { -1 } else { 1 });
        } else {
            let token = s.token.clone();
            step(s, &token, if amount > 0.0 { -1 } else { 1 });
        }
    });
    true
}
/// Sends `/ah buy <number>`: the server buys at once, with no window of its own. The lot must
/// still be there, unexpired and unchanged from what the player saw.
fn buy(s: &mut State, shown: economy::Listing) {
    if let Err(why) = data::read(|book, _, _, _| book.buy_ready(&shown, data::now())) {
        s.notice = refusal(why);
        return;
    }
    let id = shown.id;
    // A walk must not read the server's answer page as the end of the list.
    data::cancel_scan();
    s.command = Some(format!("ah buy {id}"));
    data::forget_listing(id);
    s.notice = if russian() {
        format!("Отправлено /ah buy {id} · ответ сервера в чате")
    } else {
        format!("Sent /ah buy {id} · the server replies in chat")
    };
}
fn refusal(why: economy::BuyRefusal) -> String {
    match why {
        economy::BuyRefusal::Gone => t(
            "Лот пропал из каталога · обновите список",
            "The lot is gone from the catalogue · refresh the list",
        ),
        economy::BuyRefusal::Expired => t("Лот истёк", "The lot has expired"),
        economy::BuyRefusal::Changed => t(
            "Лот изменился (цена или продавец) · выберите его заново",
            "The lot changed (price or seller) · choose it again",
        ),
    }
    .into()
}
fn catalog_step(s: &mut State, dir: i32) {
    s.catalog_page = if dir < 0 {
        s.catalog_page.saturating_sub(1)
    } else {
        (s.catalog_page + 1).min(s.catalog_pages.saturating_sub(1))
    };
}
/// One pager step: the next or previous cards, or the server's page beyond them. A server
/// turn waits for its window before another can be asked, so a fast wheel cannot skip pages.
fn step(s: &mut State, token: &str, dir: i32) {
    let (pages, previous, next) = s.pager.clone();
    if dir < 0 && s.page > 0 {
        s.page -= 1;
    } else if dir > 0 && s.page + 1 < pages {
        s.page += 1;
    } else if (s.nav.is_none() || data::now().saturating_sub(s.nav_at) > 3_000)
        && s.action.is_none()
        && let Some(button) = if dir < 0 { previous } else { next }
    {
        s.nav = Some(if dir < 0 {
            Nav::Backward
        } else {
            Nav::Forward(pages)
        });
        s.nav_at = data::now();
        s.action = Some(Pending {
            token: token.to_owned(),
            button,
            at: data::now(),
        });
    }
}
