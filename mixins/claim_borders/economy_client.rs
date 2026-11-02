// Read-only collection plus explicitly confirmed local actions on the game thread.
use crate::game::{
    Game, Screen, claim_borders as claims, claim_borders_client as client,
    claim_borders_economy as model, claim_borders_economy_persistence as disk,
    claim_borders_economy_scan as scan, claim_borders_economy_view as view,
};
use horizon_client::{items::ItemStack, menu::Input};
use horizon_world::nbt::Tag;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug)]
pub struct Hand {
    pub item: String,
    pub count: u32,
    pub plain: bool,
    pub signature: String,
}
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub message: String,
    pub hand: Option<Hand>,
    pub sidebar: Vec<(String, String)>,
    pub scanning: bool,
    pub pages: u32,
    pub sale: Option<Hand>,
    pub menu_token: String,
    pub menu_slots: Vec<(usize, String, Option<u64>)>,
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
    queue: VecDeque<String>,
    issuing: bool,
    query_at: u64,
    quiet_query: bool,
    quiet: VecDeque<(String, u64)>,
    scanner: scan::Scanner,
    menu_token: String,
    nav_last: u64,
    return_scan: bool,
    seen_pages: std::collections::BTreeSet<u64>,
    ah_pages: std::collections::BTreeSet<u64>,
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
            queue: VecDeque::new(),
            issuing: false,
            query_at: 0,
            quiet_query: false,
            quiet: VecDeque::new(),
            scanner: scan::Scanner::default(),
            menu_token: String::new(),
            nav_last: 0,
            return_scan: false,
            seen_pages: std::collections::BTreeSet::new(),
            ah_pages: std::collections::BTreeSet::new(),
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
pub(super) fn reset() {
    flush();
    state(|s| *s = State::default());
    view::reset();
}
pub(super) fn flush() {
    state(|s| {
        if s.writable
            && s.dirty
            && let Some(w) = &s.worker
        {
            let _ = w.save(s.path.clone(), s.scope.clone(), s.book.encode(&s.scope));
        }
        if let Some(w) = &s.worker {
            w.finish();
        }
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
            && s.quiet_query
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
            s.status.message = "Сканирование остановлено командой игрока".into();
        }
        if command.starts_with("worth") || command == "taxes" || command == "bal" {
            s.quiet_query = false;
        }
        if command.starts_with("sell ") || command.starts_with("buy ") {
            s.status.sale = None;
        }
    });
}
pub(super) fn cancel_scan() {
    state(|s| {
        if s.scanner.active {
            s.scanner.stop();
            s.return_scan = false;
            s.status.scanning = false;
            s.status.message = "Сканирование остановлено вводом игрока".into();
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
    if s.queue.len() < 32 && !s.queue.contains(&command) {
        s.queue.push_back(command);
    }
}
fn request(game: &mut Game<'_>, command: String, quiet: bool) {
    let now = now();
    state(|s| {
        s.issuing = true;
        s.last_command = now;
        s.query_at = now;
        s.quiet_query = quiet;
    });
    game.send_unattended_command(&command);
    state(|s| s.issuing = false);
}
fn configure(game: &Game<'_>, key: &str, dir: i64) {
    let raw = state(|s| {
        s.pref.adjust(key, dir);
        s.pref.encode()
    });
    game.native_set_setting(claims::ID, "economy", &raw);
}
fn actions(game: &mut Game<'_>) {
    use view::Action;
    for action in view::actions() {
        match action {
            Action::Setting(key, dir) => configure(game, key, dir),
            Action::Defaults => {
                state(|s| s.pref = model::Preferences::default());
                game.native_set_setting(
                    claims::ID,
                    "economy",
                    &model::Preferences::default().encode(),
                );
            }
            Action::Refresh(item) => state(|s| {
                if s.pref.flag("taxes_enabled") {
                    queue(s, "taxes".into());
                }
                queue(s, "bal".into());
                if let Some(item) = item.and_then(|i| model::id(&i)) {
                    queue(s, format!("worth {item}"));
                }
            }),
            Action::Worth(item) => {
                if let Some(item) = model::id(&item) {
                    state(|s| {
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
                    view::notice("Сначала включите модуль экономики".into());
                    continue;
                }
                let now = now();
                state(|s| {
                    s.scanner.start(kind, now);
                    s.status.scanning = true;
                    s.status.pages = 0;
                    s.status.message =
                        "Сканер: только навигация, ручной ввод остановит обход".into();
                    s.return_scan = true;
                    s.seen_pages.clear();
                    if kind == scan::Kind::Auction {
                        s.book.generation = s.book.generation.wrapping_add(1);
                        s.book.complete_scan = false;
                        s.ah_pages.clear();
                        s.book.observed_pages = 0;
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
            Action::Stop => cancel_scan(),
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
                        "Проверьте предмет, количество и лимиты; отправка только после подтверждения".into()
                    } else {
                        "Market принимает только обычный предмет без данных и повреждений".into()
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
                        "Предмет, цена или лимиты изменились: обновите данные и повторите проверку"
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
fn menu(game: &mut Game<'_>) -> Option<(scan::Page, String)> {
    if game.screen != Screen::Inventory {
        return None;
    }
    let title = game.remote_menu_title()?.plain();
    let family = match title.to_ascii_lowercase().as_str() {
        "market" => scan::Kind::Market,
        "auction house" => scan::Kind::Auction,
        _ => return None,
    };
    let token = format!(
        "{:?}|{}",
        game.remote_menu(),
        game.draimhorizons_menu_stamp()
    );
    let mut buttons = Vec::new();
    let valid = game.menu_session(|menu, _, _| {
        if !matches!(menu.kind(), horizon_client::menu::Kind::Chest(6)) || menu.carried.is_some() {
            return None;
        }
        for slot in 0..54 {
            if let Some(stack) = menu.slot(slot) {
                buttons.push(scan::Button {
                    slot,
                    item: stack.item.to_string(),
                    name: claims::strip_formatting(
                        &stack
                            .custom_name()
                            .or_else(|| stack.item_name())
                            .unwrap_or_default(),
                    ),
                    lore: lore(stack),
                });
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
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return;
    }
    let Some((scope, _)) = client::scope(game) else {
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
        let loaded = disk::load(&path, &scope)
            .and_then(|raw| raw.map(|r| model::Book::decode(&r, &scope)).transpose());
        let pref = game
            .native_setting(claims::ID, "economy")
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
                    s.status.message = format!("Кеш экономики: {e}");
                }
            }
        });
    }
    actions(game);
    let due = state(|s| {
        if now.saturating_sub(s.last_frame) < 200 {
            return false;
        }
        s.last_frame = now;
        true
    });
    if !due {
        return;
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
            && s.status.hand.as_ref().map(|h| &h.item) != held.as_ref().map(|h| &h.item)
            && let Some(h) = &held
        {
            queue(s, format!("worth {}", h.item));
        }
        s.status.hand = held;
    });
    if !read(|_, p, _, _| p.flag("enabled")) {
        state(|s| {
            s.queue.clear();
            s.scanner.stop();
            s.status.scanning = false;
        });
        return;
    }
    let page = menu(game);
    let mut click = None;
    let mut finish = false;
    state(|s| {
        if let Some((page, token)) = &page {
            s.status.menu_token = token.clone();
            s.status.menu_slots = page
                .buttons
                .iter()
                .map(|b| {
                    (
                        b.slot,
                        b.item.clone(),
                        model::listing(&b.name, &b.item, now, s.book.generation).map(|l| l.id),
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
                        s.book.market_slot(&b.item, &b.lore, now);
                        if b.slot == 48 {
                            s.book.server(&b.name, now);
                        }
                    } else if b.slot < 45
                        && let Some(l) = model::listing(&b.name, &b.item, now, s.book.generation)
                    {
                        if s.book.listings.len() < 5000 || s.book.listings.contains_key(&l.id) {
                            if s.book.watch.len() < 32 {
                                s.book.watch.insert(l.item.clone());
                            }
                            s.book.listings.insert(l.id, l);
                        }
                    } else if page.family == scan::Kind::Auction && b.slot == 48 {
                        s.book.server(&b.name, now);
                    }
                }
                s.dirty = true;
            }
            if s.scanner.active {
                match s.scanner.next(
                    page,
                    now,
                    s.pref.get("scan_delay") as u64,
                    s.pref.get("pages") as u32,
                ) {
                    scan::Step::Click(slot) => {
                        click = Some(slot);
                        s.nav_last = now;
                    }
                    scan::Step::Finished => {
                        s.status.message =
                            "Обход завершён: данные — снимок просмотренных страниц".into();
                        s.status.scanning = false;
                        if page.family == scan::Kind::Auction {
                            let generation = s.book.generation;
                            s.book.listings.retain(|_, l| l.generation == generation);
                            s.book.complete_scan = true;
                        }
                        finish = s.return_scan;
                        s.return_scan = false;
                    }
                    scan::Step::Stopped(why) => {
                        s.status.message = why;
                        s.status.scanning = false;
                        s.return_scan = false;
                    }
                    scan::Step::Wait => {}
                }
            }
        } else if s.scanner.active && now.saturating_sub(s.last_command.max(s.nav_last)) > 10000 {
            s.scanner.stop();
            s.status.scanning = false;
            s.return_scan = false;
            s.status.message = "Нет распознанного меню market / AH; обход остановлен".into();
        }
        if let Some(w) = &s.worker {
            while let Ok(error) = w.errors.try_recv() {
                s.status.message = format!("Кеш экономики: {error}");
                s.writable = false;
            }
        }
        if s.dirty
            && s.writable
            && now.saturating_sub(s.last_save) > 5000
            && let Some(w) = &s.worker
            && w.save(s.path.clone(), s.scope.clone(), s.book.encode(&s.scope))
        {
            s.dirty = false;
            s.last_save = now;
        }
    });
    if let Some(slot) = click {
        let _ = game.menu_ops(&[(Some(300 + slot), 0, Input::Pickup)]);
    }
    if finish {
        client::open_menu(game);
        view::open();
    }
    let read_command = state(|s| {
        if now.saturating_sub(s.last_command) < 2000 || s.scanner.active {
            return None;
        }
        if let Some(c) = s.queue.pop_front() {
            return Some(c);
        }
        if s.pref.flag("auto_poll") {
            if let Some(item) = s
                .book
                .watch
                .iter()
                .find(|item| {
                    s.last_poll.get(*item).is_none_or(|t| {
                        now.saturating_sub(*t) >= s.pref.get("interval") as u64 * 1000
                    })
                })
                .cloned()
            {
                s.last_poll.insert(item.clone(), now);
                return Some(format!("worth {item}"));
            }
        }
        None
    });
    if let Some(command) = read_command {
        let quiet = read(|_, p, _, _| p.flag("quiet"));
        request(game, command, quiet);
    }
}
