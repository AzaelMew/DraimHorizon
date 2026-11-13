// Read-only collection plus explicitly confirmed local actions on the game thread.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_client as client,
    claim_borders_economy as model, claim_borders_economy_persistence as disk,
    claim_borders_economy_scan as scan, claim_borders_economy_view as view,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::{items::ItemStack, menu::Input};
use horizon_world::nbt::Tag;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
/// Status line kept language-neutral so switching language re-renders it.
#[derive(Clone, Debug, Default)]
pub enum Message {
    #[default]
    None,
    StoppedByCommand,
    StoppedByInput,
    ConfirmSale,
    PlainItemOnly,
    Cache(String),
    MenuNotOpened,
    ScanFinished,
    ScanStopped(scan::Stop),
    MenuNotRecognized,
    ScanHint,
}
impl Message {
    pub fn text(&self, language: Language) -> String {
        match self {
            Self::None => String::new(),
            Self::Cache(e) => match language {
                Language::Russian => format!("Кеш экономики: {e}"),
                Language::English => format!("Economy cache: {e}"),
            },
            Self::ScanStopped(why) => why.label_in(language).into(),
            other => match other {
                Self::StoppedByCommand => language.text(
                    "Сканирование остановлено командой игрока",
                    "Scan stopped by player command",
                ),
                Self::StoppedByInput => language.text(
                    "Сканирование остановлено вводом игрока",
                    "Scan stopped by player input",
                ),
                Self::ConfirmSale => language.text(
                    "Проверьте предмет, количество и цену; отправка только после подтверждения",
                    "Check the item, quantity and price; nothing is sent until you confirm",
                ),
                Self::PlainItemOnly => language.text(
                    "Market принимает только обычный предмет без данных и повреждений",
                    "Market accepts only a plain, undamaged item with no custom data",
                ),
                Self::MenuNotOpened => language.text(
                    "Сервер не открыл запрошенное меню",
                    "Server did not open the requested menu",
                ),
                Self::ScanFinished => language.text(
                    "Обход завершён: данные — снимок просмотренных страниц",
                    "Scan finished: data is a snapshot of the viewed pages",
                ),
                Self::MenuNotRecognized => language.text(
                    "Нет распознанного меню market / AH; обход остановлен",
                    "No recognised market / AH menu; scan stopped",
                ),
                Self::ScanHint => language.text(
                    "Сканер: только навигация, ручной ввод остановит обход",
                    "Scanner: navigation only; manual input stops the scan",
                ),
                _ => "",
            }
            .into(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Hand {
    pub item: String,
    pub count: u32,
    pub plain: bool,
    pub signature: String,
}
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub message: Message,
    pub hand: Option<Hand>,
    pub sidebar: Vec<(String, String)>,
    pub scanning: bool,
    pub pages: u32,
    pub sale: Option<Hand>,
    pub menu_token: String,
    pub menu_slots: Vec<(usize, String, Option<u64>)>,
    pub page: Option<scan::Page>,
    /// The scan walking the open auction for the catalogue view is running.
    pub catalog: bool,
    /// When the last catalogue scan finished (0: none this session).
    pub catalog_at: u64,
}
struct State {
    scope: String,
    own: String,
    path: PathBuf,
    book: model::Book,
    pref: model::Preferences,
    status: Status,
    worker: Option<disk::Worker>,
    writable: bool,
    dirty: bool,
    last_save: u64,
    last_frame: u64,
    last_command: u64,
    last_poll: BTreeMap<String, u64>,
    last_finance: u64,
    queue: VecDeque<String>,
    issuing: bool,
    query_at: u64,
    replies: crate::game::claim_borders_economy_requests::Replies,
    quiet: VecDeque<(String, u64)>,
    scanner: scan::Scanner,
    menu_token: String,
    nav_last: u64,
    return_scan: bool,
    auto_cycle: bool,
    last_auto: u64,
    opening: Option<(String, u64)>,
    seen_pages: std::collections::BTreeSet<u64>,
    ah_pages: std::collections::BTreeSet<u64>,
    /// The window token last parsed, so a changed window is read at once, not at the next tick.
    probed: String,
    /// The window whose lot stacks were last saved.
    stacked: String,
    /// Lots the budget refused for the window named by `stacked_for`, so they are not encoded again.
    refused: std::collections::BTreeSet<u64>,
    refused_for: String,
    /// When the server window was first seen gone during a catalogue walk.
    away: scan::Absence,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: String::new(),
            own: String::new(),
            path: PathBuf::new(),
            book: model::Book::default(),
            pref: model::Preferences::default(),
            status: Status::default(),
            worker: None,
            writable: true,
            dirty: false,
            last_save: 0,
            last_frame: 0,
            last_command: 0,
            last_poll: BTreeMap::new(),
            last_finance: 0,
            queue: VecDeque::new(),
            issuing: false,
            query_at: 0,
            replies: Default::default(),
            quiet: VecDeque::new(),
            scanner: scan::Scanner::default(),
            menu_token: String::new(),
            nav_last: 0,
            return_scan: false,
            auto_cycle: false,
            last_auto: 0,
            opening: None,
            seen_pages: std::collections::BTreeSet::new(),
            ah_pages: std::collections::BTreeSet::new(),
            probed: String::new(),
            stacked: String::new(),
            refused: std::collections::BTreeSet::new(),
            refused_for: String::new(),
            away: scan::Absence::default(),
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
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub(super) fn read<T>(f: impl FnOnce(&model::Book, &model::Preferences, &Status, &str) -> T) -> T {
    state(|s| f(&s.book, &s.pref, &s.status, &s.own))
}
pub(super) fn auction_watch(item: &str, cap: model::Coins) -> bool {
    state(|s| {
        if s.scope.is_empty() || !s.writable || !s.book.set_auction_watch(item, cap) {
            return false;
        }
        s.dirty = true;
        true
    })
}
pub(super) fn reset() {
    flush();
    state(|s| *s = State::default());
    view::reset();
    crate::game::claim_borders_commerce_view::reset();
}
pub(super) fn flush() {
    state(|s| {
        if s.writable
            && s.dirty
            && let Some(w) = &s.worker
        {
            let _ = w.save(
                s.path.clone(),
                s.scope.clone(),
                s.book.encode_at(&s.scope, now()),
            );
        }
        if let Some(w) = &s.worker {
            w.finish();
        }
    });
}
pub(super) fn silence(text: &str) {
    let text = claims::strip_formatting(text);
    let time = now();
    state(|s| {
        if !s.pref.flag("quiet") {
            return;
        }
        s.quiet.retain(|(_, t)| time.saturating_sub(*t) < 3000);
        if s.quiet.len() >= 48 {
            s.quiet.pop_front();
        }
        s.quiet.push_back((text, time));
    });
}
pub(super) fn quiet(text: &str) -> bool {
    let text = claims::strip_formatting(text);
    let now = now();
    state(|s| {
        s.quiet.retain(|(_, t)| now.saturating_sub(*t) < 3000);
        s.quiet.iter().any(|(line, _)| *line == text)
    })
}
pub(super) fn server(text: &str) {
    let now = now();
    let text = claims::strip_formatting(text);
    state(|s| {
        if s.scope.is_empty() || !s.pref.flag("enabled") {
            return;
        }
        let recognized = s.book.server(&text, now);
        if recognized {
            s.dirty = true;
        }
        if recognized
            && s.pref.flag("quiet")
            && s.replies.quiet(&text, now)
            && now.saturating_sub(s.query_at) < 10000
            && !text.starts_with("Sold ")
            && !text.starts_with("Bought ")
        {
            if s.quiet.len() >= 48 {
                s.quiet.pop_front();
            }
            s.quiet.push_back((text, now));
        }
    });
}
pub(super) fn command(command: &str) {
    state(|s| {
        if s.issuing {
            return;
        }
        if s.scanner.active {
            s.scanner.stop();
            s.return_scan = false;
            s.status.scanning = false;
            s.auto_cycle = false;
            s.opening = None;
            s.status.message = Message::StoppedByCommand;
        }
        s.replies.manual(command);
        if matches!(
            command.split_whitespace().next(),
            Some("worth" | "bal" | "balance")
        ) {
            s.quiet.clear();
        }
        if command.starts_with("sell ") || command.starts_with("buy ") {
            s.status.sale = None;
        }
    });
}
pub(super) fn cancel_scan() {
    state(|s| {
        s.auto_cycle = false;
        s.opening = None;
        if s.scanner.active {
            s.scanner.stop();
            s.return_scan = false;
            s.status.scanning = false;
            s.status.catalog = false;
            s.status.message = Message::StoppedByInput;
        }
    });
}
fn hand(stack: &ItemStack) -> Hand {
    Hand {
        item: stack.item.to_string(),
        count: stack.count,
        plain: stack.damage == 0 && stack.data().is_none(),
        signature: format!("{stack:?}"),
    }
}
fn queue(s: &mut State, command: String) {
    if s.queue.len() < 64 && !s.queue.contains(&command) {
        s.queue.push_back(command);
    }
}
fn request(game: &mut Game<'_>, command: String, quiet: bool) {
    let now = now();
    state(|s| {
        s.issuing = true;
        s.last_command = now;
        s.query_at = now;
        s.replies.begin(&command, now, quiet);
        if command == "bal" {
            s.last_finance = now;
        }
        if let Some(item) = command.strip_prefix("worth ").and_then(model::id) {
            s.last_poll.insert(item, now);
        }
    });
    game.send_unattended_command(&command);
    state(|s| s.issuing = false);
}
pub(super) fn configure(game: &Game<'_>, key: &str, dir: i64) {
    let raw = state(|s| {
        s.pref.adjust(key, dir);
        s.pref.encode()
    });
    game.native_set_setting(claims::SETTINGS_ID, "economy", &raw);
}
fn actions(game: &mut Game<'_>) {
    use view::Action;
    for action in view::actions() {
        match action {
            Action::Refresh(item) => state(|s| {
                queue(s, "bal".into());
                let missing: Vec<_> = s
                    .book
                    .listings
                    .values()
                    .filter(|l| !l.bid && !s.book.unsupported.contains(&l.item))
                    .map(|l| l.item.clone())
                    .collect();
                for id in missing {
                    queue(s, format!("worth {id}"));
                }
                if let Some(item) = item.and_then(|i| model::id(&i)) {
                    queue(s, format!("worth {item}"));
                }
            }),
            Action::Worth(item) => {
                if let Some(item) = model::id(&item) {
                    state(|s| {
                        s.book.unsupported.remove(&item);
                        s.queue.push_front(format!("worth {item}"));
                        s.queue.truncate(32);
                    });
                }
            }
            Action::Watch(item) => {
                if let Some(item) = model::id(&item) {
                    state(|s| {
                        if !s.book.watch.remove(&item) && s.book.watch.len() < 64 {
                            s.book.watch.insert(item);
                        }
                        s.dirty = true;
                    });
                }
            }
            Action::OpenMarket => {
                view::close();
                request(game, "market".into(), false);
            }
            Action::OpenAuction(item) => {
                view::close();
                let command = item
                    .and_then(|i| model::id(&i))
                    .map_or("ah".into(), |i| format!("ah search {i}"));
                request(game, command, false);
            }
            Action::Scan(kind) => {
                let enabled = read(|_, p, _, _| p.flag("enabled"));
                if !enabled {
                    view::notice(
                        language()
                            .text(
                                "Сначала включите модуль экономики",
                                "Enable the economy module first",
                            )
                            .into(),
                    );
                    continue;
                }
                start_scan(game, kind, false);
            }
            Action::Stop => cancel_scan(),
            Action::Close => {
                view::close();
                state(|s| s.queue.clear());
                game.native_mod_screen_close(claims::ID);
                crate::game::claim_borders_menu_view::reset();
            }
            Action::CancelSell => state(|s| s.status.sale = None),
            Action::PrepareSell => {
                let held = game
                    .inventory
                    .get(game.selected)
                    .and_then(Option::as_ref)
                    .map(hand);
                state(|s| {
                    s.status.sale = held.filter(|h| h.plain);
                    s.status.message = if s.status.sale.is_some() {
                        Message::ConfirmSale
                    } else {
                        Message::PlainItemOnly
                    };
                });
            }
            Action::ConfirmSell => {
                let held = game
                    .inventory
                    .get(game.selected)
                    .and_then(Option::as_ref)
                    .map(hand);
                let allowed = state(|s| {
                    let valid = s.pref.flag("enabled")
                        && s.status
                            .sale
                            .as_ref()
                            .zip(held.as_ref())
                            .is_some_and(|(a, b)| a.signature == b.signature)
                        && sale_estimate(&s.book, &s.pref, held.as_ref()?, now()).is_some();
                    s.status.sale = None;
                    Some(valid)
                })
                .unwrap_or(false);
                if allowed {
                    view::close();
                    request(game, "sell hand".into(), false);
                } else {
                    view::notice(
                        language()
                            .text(
                                "Предмет или цена изменились: обновите данные и повторите проверку",
                                "The item or price changed: refresh the data and check again",
                            )
                            .into(),
                    );
                }
            }
            Action::Copy(text) => {
                use horizon_client::clipboard::Clipboard;
                game.clipboard.set(&text);
            }
            Action::Paste(token) => {
                use horizon_client::clipboard::Clipboard;
                let text = game.clipboard.get();
                view::paste(token, &text);
            }
        }
    }
}
pub(super) fn sale_estimate(
    book: &model::Book,
    p: &model::Preferences,
    h: &Hand,
    now: u64,
) -> Option<(u32, model::Coins)> {
    if !h.plain {
        return None;
    }
    book.hand_sale(&h.item, h.count, p, now)
}
fn lore(stack: &ItemStack) -> Vec<String> {
    let Some(Tag::List(parts)) = stack.data().and_then(|d| d.other("minecraft:lore")) else {
        return Vec::new();
    };
    parts
        .iter()
        .take(24)
        .map(|t| horizon_client::component::name_text(t).plain())
        .map(|s| claims::strip_formatting(&s))
        .collect()
}
fn button(slot: usize, stack: &horizon_client::items::ItemStack) -> scan::Button {
    scan::Button {
        slot,
        item: stack.item.to_string(),
        name: claims::strip_formatting(
            &stack
                .custom_name()
                .or_else(|| stack.item_name())
                .unwrap_or_default(),
        ),
        lore: lore(stack),
    }
}
/// The auction lots of the open window with their own stacks: each number is read from the
/// stack it belongs to, so an icon can never be paired with another lot.
pub(super) fn lot_stacks(game: &mut Game<'_>) -> Vec<(u64, horizon_client::items::ItemStack)> {
    let now = now();
    let mut out = Vec::new();
    let _ = game.menu_session(|menu, _, _| {
        if matches!(menu.kind(), horizon_client::menu::Kind::Chest(6)) {
            for slot in 0..45 {
                if let Some(stack) = menu.slot(slot)
                    && let Some(l) = model::listing(
                        &scan::listing_text(&button(slot, stack)).unwrap_or_default(),
                        &stack.item,
                        now,
                        0,
                    )
                {
                    out.push((l.id, stack.clone()));
                }
            }
        }
        Some(false)
    });
    out
}
fn menu(game: &mut Game<'_>) -> Option<(scan::Page, String)> {
    if game.screen != Screen::Inventory {
        return None;
    }
    let title = game.remote_menu_title()?.plain();
    let family = match title.to_ascii_lowercase().as_str() {
        // /shop opens the same menu as /market.
        "market" | "shop" | "рынок" | "маркет" | "магазин" => scan::Kind::Market,
        "auction house" | "аукцион" | "аукционный дом" => scan::Kind::Auction,
        _ => return None,
    };
    let token = window_token(game);
    let mut buttons = Vec::new();
    let valid = game.menu_session(|menu, _, _| {
        if !matches!(menu.kind(), horizon_client::menu::Kind::Chest(6)) || menu.carried.is_some() {
            return None;
        }
        for slot in 0..54 {
            if let Some(stack) = menu.slot(slot) {
                buttons.push(button(slot, stack));
            }
        }
        Some(false)
    });
    if !valid {
        return None;
    }
    let identity = buttons
        .iter()
        .filter(|b| b.slot < 45)
        .map(|b| {
            format!(
                "{}:{}:{}",
                b.slot,
                b.item,
                b.name.split('·').next().unwrap_or(&b.name)
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let fingerprint = crate::game::claim_borders_map_history::hash(identity.as_bytes());
    Some((
        scan::Page {
            family,
            fingerprint,
            buttons,
        },
        token,
    ))
}
/// Identifies the server window as it is now; built once per frame and kept in `State::probed`.
fn window_token(game: &Game<'_>) -> String {
    format!(
        "{:?}|{}",
        game.remote_menu(),
        game.draimhorizons_menu_stamp()
    )
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        suspend();
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
        suspend();
        return;
    };
    let now = now();
    if state(|s| s.scope != scope) {
        flush();
        let Ok(root) = Game::native_config_dir() else {
            return;
        };
        let path = root.join("claim_borders").join(format!(
            "economy-{:016x}.json",
            crate::game::claim_borders_map_history::hash(scope.as_bytes())
        ));
        let loaded = disk::load(&path, &scope).and_then(|raw| {
            raw.map(|r| model::Book::decode_at(&r, &scope, now()))
                .transpose()
        });
        let pref = game
            .native_setting(claims::SETTINGS_ID, "economy")
            .and_then(|r| model::Preferences::decode(&r).ok())
            .unwrap_or_default();
        let own = game.remote.as_ref().unwrap().session.name.clone();
        state(|s| {
            *s = State {
                scope,
                own,
                path,
                pref,
                worker: disk::Worker::spawn().ok(),
                ..State::default()
            };
            match loaded {
                Ok(Some(book)) => s.book = book,
                Ok(None) => {}
                Err(e) => {
                    s.writable = false;
                    s.status.message = Message::Cache(e.to_string());
                }
            }
        });
    }
    crate::game::claim_borders_commerce_view::frame(game);
    actions(game);
    // A new server window is read on the frame it arrives, so the catalogue replaces it
    // before the server's own page is ever drawn.
    let window = (game.screen == Screen::Inventory).then(|| window_token(game));
    let gone = game.screen != Screen::Inventory;
    let due = state(|s| {
        // The window closed under a catalogue walk: stop it, unless the server is only
        // swapping one window for the next.
        if s.away.gone(s.status.catalog && gone, now) {
            abandon(s);
        }
        let changed = window.as_ref().is_some_and(|w| *w != s.probed);
        if now.saturating_sub(s.last_frame) < 200 && !changed {
            return false;
        }
        s.last_frame = now;
        if let Some(w) = window {
            s.probed = w;
        }
        true
    });
    if !due {
        return;
    }
    let ui_visible = view::visible();
    let auto = state(|s| {
        if ui_visible
            && game.native_mod_screen() == Some(claims::ID)
            && s.pref.flag("enabled")
            && s.pref.flag("auto_poll")
            && !s.scanner.active
            && s.status.sale.is_none()
            && (s.last_auto == 0 || now.saturating_sub(s.last_auto) > 120_000)
        {
            s.last_auto = now;
            s.auto_cycle = true;
            true
        } else {
            false
        }
    });
    if auto {
        start_scan(game, scan::Kind::Market, true);
    }

    let sidebar = crate::game::scoreboard::claim_borders_economy_sidebar::read(game);
    let held = game
        .inventory
        .get(game.selected)
        .and_then(Option::as_ref)
        .map(hand);
    let inventory: BTreeMap<String, u32> = game
        .inventory
        .iter()
        .flatten()
        .filter(|s| s.damage == 0 && s.data().is_none())
        .fold(BTreeMap::new(), |mut map, s| {
            let n = map.entry(s.item.to_string()).or_default();
            *n = n.saturating_add(s.count);
            map
        });
    state(|s| {
        s.status.sidebar = sidebar.clone();
        s.book.sidebar(&sidebar, now);
        s.book.inventory = inventory;
        s.book.roll_day(now);
        if s.pref.flag("held")
            && ui_visible
            && s.status.hand.as_ref().map(|h| &h.item) != held.as_ref().map(|h| &h.item)
            && let Some(h) = &held
        {
            if !s.book.unsupported.contains(&h.item) {
                queue(s, format!("worth {}", h.item));
            }
        }
        s.status.hand = held;
    });
    if !read(|_, p, _, _| p.flag("enabled")) {
        state(|s| {
            s.queue.clear();
            s.scanner.stop();
            s.status.scanning = false;
            s.status.catalog = false;
        });
        return;
    }
    let page = menu(game);
    let mut click = None;
    let mut finish = false;
    state(|s| {
        if let Some((page, token)) = &page {
            s.status.page = Some(page.clone());
            s.status.menu_token = token.clone();
            s.status.menu_slots = page
                .buttons
                .iter()
                .map(|b| {
                    (
                        b.slot,
                        b.item.clone(),
                        model::listing(
                            &scan::listing_text(b).unwrap_or_default(),
                            &b.item,
                            now,
                            s.book.generation,
                        )
                        .map(|l| l.id),
                    )
                })
                .collect();
            let full = format!("{token}|{:?}", page.buttons);
            if full != s.menu_token {
                s.menu_token = full;
                if s.seen_pages.len() < 5000 {
                    s.seen_pages.insert(page.fingerprint);
                }
                s.status.pages = s.seen_pages.len() as u32;
                if page.family == scan::Kind::Auction {
                    if s.ah_pages.len() < 5000 {
                        s.ah_pages.insert(page.fingerprint);
                    }
                    s.book.observed_pages = s.ah_pages.len() as u32;
                }
                for b in &page.buttons {
                    if page.family == scan::Kind::Market {
                        let mut lines = b.lore.clone();
                        lines.push(b.name.clone());
                        s.book.market_slot(&b.item, &lines, now);
                        if s.book.watch.len() < 64
                            && s.book.quotes.contains_key(&b.item)
                            && !s.book.unsupported.contains(&b.item)
                        {
                            s.book.watch.insert(b.item.clone());
                        }
                        if b.slot == 48 {
                            s.book.server(&b.name, now);
                        }
                    } else if b.slot < 45
                        && let Some(l) = model::listing(
                            &scan::listing_text(b).unwrap_or_default(),
                            &b.item,
                            now,
                            s.book.generation,
                        )
                    {
                        if s.book.watch.len() < 64 {
                            s.book.watch.insert(l.item.clone());
                        }
                        s.book.observe_lot(l);
                    } else if page.family == scan::Kind::Auction && b.slot == 48 {
                        s.book.server(&b.name, now);
                    }
                }
                s.dirty = true;
            }
            let ready = if let Some((old, at)) = &s.opening {
                if old != token && s.scanner.kind == Some(page.family) {
                    s.opening = None;
                    true
                } else if now.saturating_sub(*at) > 10_000 {
                    s.scanner.stop();
                    s.status.scanning = false;
                    s.auto_cycle = false;
                    s.return_scan = false;
                    s.opening = None;
                    s.status.message = Message::MenuNotOpened;
                    false
                } else {
                    false
                }
            } else {
                true
            };
            if s.scanner.active && ready {
                // The catalogue walk waits for each page to arrive, then only briefly: the
                // player is looking at it. Other scans keep the configured pause.
                let delay = if s.status.catalog {
                    CATALOG_DELAY_MS
                } else {
                    s.pref.get("scan_delay") as u64
                };
                match s.scanner.next(page, now, delay, s.pref.get("pages") as u32) {
                    scan::Step::Click(slot) => {
                        click = Some(slot);
                        s.nav_last = now;
                    }
                    scan::Step::Finished => {
                        s.status.message = Message::ScanFinished;
                        s.status.scanning = false;
                        if s.status.catalog {
                            s.status.catalog = false;
                            s.status.catalog_at = now;
                        }
                        if page.family == scan::Kind::Auction {
                            // Only a walk that reached the last page may drop unseen lots.
                            let complete = s.scanner.complete;
                            s.book.finish_scan(complete, now);
                            s.dirty = true;
                        }
                        finish = s.return_scan;
                        s.return_scan = false;
                    }
                    scan::Step::Stopped(why) => {
                        s.status.message = Message::ScanStopped(why);
                        s.status.scanning = false;
                        s.status.catalog = false;
                        s.return_scan = false;
                    }
                    scan::Step::Wait => {}
                }
            }
        } else if s.scanner.active && now.saturating_sub(s.last_command.max(s.nav_last)) > 10000 {
            s.scanner.stop();
            s.status.scanning = false;
            s.status.catalog = false;
            s.return_scan = false;
            s.status.message = Message::MenuNotRecognized;
        }
        if let Some(w) = &s.worker {
            while let Ok(error) = w.errors.try_recv() {
                s.status.message = Message::Cache(error.to_string());
                s.writable = false;
            }
        }
        if s.dirty && s.writable && now.saturating_sub(s.last_save) > 5000 {
            s.book.drop_expired(now);
            if let Some(w) = &s.worker
                && w.save(
                    s.path.clone(),
                    s.scope.clone(),
                    s.book.encode_at(&s.scope, now),
                )
            {
                s.dirty = false;
                s.last_save = now;
            }
        }
    });
    if let Some(slot) = click {
        let _ = game.menu_ops(&[(Some(300 + slot), 0, Input::Pickup)]);
    }
    if finish {
        let next = state(|s| {
            if s.auto_cycle && s.scanner.kind == Some(scan::Kind::Market) {
                true
            } else {
                s.auto_cycle = false;
                s.last_auto = now;
                false
            }
        });
        if next {
            start_scan(game, scan::Kind::Auction, true);
        } else {
            // The workspace cannot open over the server's window, so the walk closes it first.
            if game.screen == Screen::Inventory {
                game.close_container();
                game.screen = Screen::Playing;
                game.capture(true);
            }
            client::open_trades(game);
        }
    }
    let read_command = state(|s| {
        if now.saturating_sub(s.last_command) < 2000 || s.scanner.active {
            return None;
        }
        // Scoreboard balance updates independently of the trading window.
        if s.pref.flag("auto_poll")
            && s.pref.flag("custom_sidebar")
            && (s.last_finance == 0 || now.saturating_sub(s.last_finance) >= 60_000)
        {
            s.last_finance = now;
            return Some("bal".into());
        }
        if !ui_visible && page.is_none() {
            s.queue.clear();
            return None;
        }
        while let Some(c) = s.queue.pop_front() {
            if c.strip_prefix("worth ")
                .is_some_and(|id| s.book.unsupported.contains(id))
            {
                continue;
            }
            return Some(c);
        }
        if s.pref.flag("auto_poll") && (ui_visible || page.is_some()) {
            if let Some(item) = crate::game::claim_borders_economy_requests::next_watch(
                &s.book.watch,
                &s.book.unsupported,
                &s.last_poll,
                now,
                s.pref.get("interval") as u64 * 1000,
            )
            .map(str::to_owned)
            {
                s.last_poll.insert(item.clone(), now);
                return Some(format!("worth {item}"));
            }
        }
        None
    });
    // Single player runs Horizon's own server, which has no economy commands: an automatic
    // poll there would only raise the game's unknown-command confirmation.
    if let Some(command) = read_command
        && game.integrated.is_none()
    {
        let quiet = read(|_, p, _, _| p.flag("quiet"));
        request(game, command, quiet);
    }
}

/// Pause after each auction page arrives during the catalogue walk.
const CATALOG_DELAY_MS: u64 = 200;
fn abandon(s: &mut State) {
    s.scanner.stop();
    s.status.scanning = false;
    s.status.catalog = false;
    s.return_scan = false;
    s.auto_cycle = false;
    s.opening = None;
    s.away = scan::Absence::default();
    s.status.message = Message::None;
}
/// The module is off, or there is no server session: no walk may go on, and what the walk
/// found is saved. Nothing is pruned, since the walk did not finish.
fn suspend() {
    let running = state(|s| s.scanner.active || s.status.catalog);
    if running {
        state(abandon);
        flush();
    }
}
/// The window is not parsed yet (a new one just arrived) while the catalogue walk is running:
/// the walk's last page stands in, so the server's page is not drawn or clicked meanwhile.
pub(super) fn catalog_hold(game: &Game<'_>) -> Option<(scan::Page, String)> {
    if !game.native_on(claims::ID) || game.screen != Screen::Inventory {
        return None;
    }
    let title = game.remote_menu_title()?.plain().to_ascii_lowercase();
    if !matches!(
        title.as_str(),
        "auction house" | "аукцион" | "аукционный дом"
    ) {
        return None;
    }
    read(|_, p, status, _| {
        (p.flag("enabled") && p.flag("overlay") && status.catalog)
            .then(|| status.page.clone().zip(Some(status.menu_token.clone())))
            .flatten()
            .filter(|(page, _)| page.family == scan::Kind::Auction)
    })
}
/// Keeps the item stack of each lot (as text) so titles, enchantments and icons survive a
/// restart. Only new lots are encoded; the cache is bounded in the book.
pub(super) fn remember_stacks(lots: &[(u64, ItemStack)]) {
    state(|s| {
        // One pass per window: a lot the budget refuses is not encoded again every frame.
        if s.stacked == s.menu_token {
            return;
        }
        if s.refused_for != s.menu_token {
            s.refused.clear();
            s.refused_for = s.menu_token.clone();
        }
        let mut complete = true;
        for (id, stack) in lots {
            if !s.book.listings.contains_key(id) {
                complete = false;
            } else if !s.book.stacks.contains_key(id) && !s.refused.contains(id) {
                let text = horizon_client::item_data::canon_stack(Some(stack));
                if s.book.remember_stack(*id, text) {
                    s.dirty = true;
                } else {
                    s.refused.insert(*id);
                }
            }
        }
        if complete {
            s.stacked = s.menu_token.clone();
        }
    });
}
/// The saved stack of a lot, when no window has shown it yet this session.
pub(super) fn saved_stack(id: u64) -> Option<ItemStack> {
    let text = read(|book, _, _, _| book.stacks.get(&id).cloned())?;
    horizon_client::item_data::parse_stack(&text).ok().flatten()
}
/// Walks the auction that is already open, from its home page through every page of All
/// listings, so the catalogue view can show and search all lots at once. Nothing is bought or
/// clicked besides the server's own page buttons.
pub(super) fn start_catalog() -> bool {
    let now = now();
    state(|s| {
        if !s.pref.flag("enabled") || s.scanner.active || s.scope.is_empty() {
            return false;
        }
        s.opening = None;
        s.auto_cycle = false;
        s.return_scan = false;
        s.scanner.start(scan::Kind::Auction, now);
        s.status.scanning = true;
        s.status.catalog = true;
        s.status.pages = 0;
        s.status.message = Message::ScanHint;
        s.seen_pages.clear();
        s.book.begin_walk();
        s.ah_pages.clear();
        true
    })
}
/// A lot the player just asked to buy leaves the catalogue at once; the next walk confirms it.
pub(super) fn forget_listing(id: u64) {
    state(|s| {
        if s.book.forget_lot(id) {
            s.dirty = true;
        }
    });
}
fn start_scan(game: &mut Game<'_>, kind: scan::Kind, automatic: bool) {
    let now = now();
    let old = window_token(game);
    state(|s| {
        s.opening = Some((old, now));
        s.auto_cycle = automatic;
        s.last_auto = now;
        s.scanner.start(kind, now);
        s.status.scanning = true;
        s.status.pages = 0;
        s.status.message = Message::ScanHint;
        s.return_scan = true;
        s.seen_pages.clear();
        if kind == scan::Kind::Auction {
            s.book.begin_walk();
            s.ah_pages.clear();
        }
    });
    view::close();
    request(
        game,
        if kind == scan::Kind::Auction {
            "ah"
        } else {
            "market"
        }
        .into(),
        false,
    );
}

pub(super) fn server_page(game: &Game<'_>) -> Option<(scan::Page, String)> {
    if !game.native_on(claims::ID) || game.screen != Screen::Inventory {
        return None;
    }
    let token = state(|s| s.probed.clone());
    read(|_, p, status, _| {
        if p.flag("enabled") && p.flag("overlay") && status.menu_token == token {
            status.page.clone().map(|page| (page, token))
        } else {
            None
        }
    })
}
pub(super) fn server_click(
    game: &mut Game<'_>,
    token: &str,
    button: &scan::Button,
    prepared_at: u64,
) -> bool {
    let Some((page, actual)) = menu(game) else {
        return false;
    };
    if !scan::confirmed_button(&page, &actual, token, button, prepared_at, now()) {
        return false;
    }
    let allowed = read(|_, p, _, _| p.flag("enabled"));
    if !allowed {
        return false;
    }
    cancel_scan();
    game.menu_ops(&[(Some(300 + button.slot), 0, Input::Pickup)])
}
