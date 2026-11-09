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
    collections::BTreeSet,
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
    Page(i32),
    Search,
    Standard,
}
#[derive(Default)]
struct State {
    icons: Vec<(usize, horizon_client::items::ItemStack)>,
    token: String,
    query: TextInput,
    watch_edit: Option<(String, TextInput)>,
    focus: bool,
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
    let page = data::server_page(game)?;
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
        state(|s| s.icons = icons);
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
            }
        });
    }
    if let Some(p) = action {
        let ok = data::now().saturating_sub(p.at) <= 10_000
            && data::server_click(game, &p.token, &p.button, p.at);
        state(|s| {
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
    if game.screen != Screen::Inventory {
        state(|s| {
            s.pending = None;
            s.focus = false;
            s.hits.clear();
        });
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
        Category => local_category(&b.name),
        Inspect => b.name.clone(),
    }
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
                if russian() {
                    format!("{} монет", s.replace(" coins", ""))
                } else {
                    format!("{} coins", s.replace(" coins", ""))
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
            Hit::Page(n) if *n < 0 => t(
                "Предыдущие карточки этой страницы",
                "Previous cards on this page",
            )
            .into(),
            Hit::Page(_) => t(
                "Следующие карточки этой страницы",
                "Next cards on this page",
            )
            .into(),
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
    if crate::game::claim_borders_ui_style::contains(r, mouse) {
        s.hover = Some(b.slot);
    }
    s.hits.push((r, Hit::Server(b.clone())));
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
    let w = (batch.width - 16.0).clamp(1.0, 620.0);
    let h = (batch.height - 16.0).clamp(1.0, 360.0);
    let x = (batch.width - w) * 0.5;
    let y = (batch.height - h) * 0.5;
    let start = batch.vertices.len();
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x80111b27);
    state(|s| {
        if s.token != token {
            s.token = token.clone();
            s.pending = None;
            s.modal_scroll = 0;
            s.page = 0;
            s.notice.clear();
            s.query = TextInput::default();
            s.focus = false;
            s.watch_edit = None;
        }
        s.hits.clear();
        s.hover = None;
        s.control_tip = None;
        ui::fill(game, batch, [x, y, w, h], 0xee182636);
        ui::icon(
            game,
            batch,
            ui::Icon::Market,
            [x + 10.0, y + 10.0, 13.0, 13.0],
            ui::preferences(game).accent.color(),
        );
        let title = if page.family == scan::Kind::Market {
            if scan::market_detail(&page) {
                t("MARKET · Товар", "MARKET · Item")
            } else {
                t("MARKET · Рынок", "MARKET · Market")
            }
        } else {
            t("AH · Аукцион", "AH · Auction")
        };
        ui::text(game, batch, title, [x + 30.0, y + 12.0], 0xe3ebf3, false);
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
        let services: Vec<_> = page
            .buttons
            .iter()
            .filter(|b| {
                matches!(
                    scan::role(&page, b),
                    Back | News | Help | Mine | Mail | ClaimAll
                )
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
                for (i, l) in lore.into_iter().take(lines).enumerate() {
                    ui::text(
                        game,
                        batch,
                        &ui::clip(
                            game,
                            &crate::game::claim_borders_economy_view::market_lore(game, l),
                            w - 24.0,
                        ),
                        [x + 12.0, y + 92.0 + i as f32 * 11.0],
                        0xb6c7d9,
                        false,
                    );
                }
                let top = if lines == 0 {
                    y + 86.0
                } else {
                    y + 96.0 + lines as f32 * 11.0
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
            let query = [x + 8.0, y + 58.0, w - 16.0, 18.0];
            if !confirming {
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
            let cols = if w >= 560.0 {
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
            // These controls change actual server pages and do not disappear under a search filter.
            let bottom = y + h - 46.0;
            if let Some(b) = page
                .buttons
                .iter()
                .find(|b| scan::role(&page, b) == Previous)
            {
                control(
                    game,
                    batch,
                    s,
                    [x + 8.0, bottom, 112.0, 18.0],
                    t("Пред. страница", "Prev. page"),
                    Hit::Server(b.clone()),
                    screen.mouse,
                    ui::Icon::Back,
                );
            }
            if let Some(b) = page.buttons.iter().find(|b| scan::role(&page, b) == Next) {
                control(
                    game,
                    batch,
                    s,
                    [x + w - 120.0, bottom, 112.0, 18.0],
                    t("След. страница", "Next page"),
                    Hit::Server(b.clone()),
                    screen.mouse,
                    ui::Icon::Next,
                );
            }
            let center = x + w * 0.5;
            control(
                game,
                batch,
                s,
                [center - 48.0, bottom, 18.0, 18.0],
                "",
                Hit::Page(-1),
                screen.mouse,
                ui::Icon::Back,
            );
            ui::text(
                game,
                batch,
                &format!("{}/{}", s.page + 1, pages),
                [center - 23.0, bottom + 5.0],
                0x9aacc0,
                false,
            );
            control(
                game,
                batch,
                s,
                [center + 27.0, bottom, 18.0, 18.0],
                "",
                Hit::Page(1),
                screen.mouse,
                ui::Icon::Next,
            );
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
            let mw = (w - 16.0).min(440.0);
            let mh = (h - 16.0).min(270.0);
            let mx = x + (w - mw) * 0.5;
            let my = y + (h - mh) * 0.5;
            game.native_fill(batch, [mx, my, mw, mh], 0xff1c2b3c);
            ui::text(
                game,
                batch,
                if transaction {
                    t("Подтверждение действия", "Confirm action")
                } else {
                    t("Информация", "Information")
                },
                [mx + 10.0, my + 10.0],
                0xf0d68a,
                false,
            );
            ui::text(
                game,
                batch,
                &ui::clip(game, &label(game, &page, &p.button), mw - 20.0),
                [mx + 10.0, my + 27.0],
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
            let max = (((mh - 100.0) / 11.0).floor() as usize).max(1);
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
                    [mx + 10.0, my + 45.0 + i as f32 * 11.0],
                    0xb6c7d9,
                    false,
                );
            }
            let cr = [mx + 10.0, my + mh - 28.0, (mw - 26.0) * 0.5, 20.0];
            let ok = [mx + mw * 0.5 + 3.0, my + mh - 28.0, (mw - 26.0) * 0.5, 20.0];
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
                    t("Подтвердить", "Confirm"),
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
    true
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
                    s.action = Some(p)
                } else {
                    s.modal_scroll = 0;
                    s.pending = Some(p)
                }
            }
            Some(Hit::Confirm) => {
                s.action = s.pending.take();
            }
            Some(Hit::Cancel) => s.pending = None,
            Some(Hit::Page(n)) => {
                s.page = if n < 0 {
                    s.page.saturating_sub(1)
                } else {
                    s.page + 1
                }
            }
            Some(Hit::Search) => s.focus = true,
            Some(Hit::Standard) => return true,
            _ => {}
        }
        false
    });
    if standard {
        toggle_view(game, page.family);
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
        if state(|s| s.pending.take().is_some() || std::mem::take(&mut s.focus)) {
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
        } else {
            s.page = if amount > 0.0 {
                s.page.saturating_sub(1)
            } else {
                s.page + 1
            };
        }
    });
    true
}
