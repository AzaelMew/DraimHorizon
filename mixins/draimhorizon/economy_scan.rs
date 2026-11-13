// A scanner may click navigation only, never an item, buy/sell button or bid.
use claim_borders_relay_client::translation::Language;
use std::collections::{BTreeSet, VecDeque};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Market,
    Auction,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub slot: usize,
    pub item: String,
    pub name: String,
    pub lore: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Page {
    pub family: Kind,
    pub fingerprint: u64,
    pub buttons: Vec<Button>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Category,
    Goods,
    Listing,
    Preview,
    Back,
    Previous,
    Next,
    Close,
    Balance,
    News,
    Help,
    Mine,
    Mail,
    ClaimAll,
    Claim,
    Buy(u32),
    Sell(u32),
    SellAll,
    Chart(u8),
    Bid,
    Inspect,
}
impl Role {
    pub fn service(self) -> bool {
        !matches!(
            self,
            Self::Category
                | Self::Goods
                | Self::Listing
                | Self::Preview
                | Self::Claim
                | Self::Inspect
        )
    }
    pub fn transaction(self) -> bool {
        matches!(
            self,
            Self::Buy(_) | Self::Sell(_) | Self::SellAll | Self::Bid | Self::Claim | Self::ClaimAll
        )
    }
    pub fn navigation(self) -> bool {
        matches!(
            self,
            Self::Category
                | Self::Goods
                | Self::Back
                | Self::Previous
                | Self::Next
                | Self::Close
                | Self::Mine
                | Self::Mail
                | Self::Chart(_)
        )
    }
}
pub fn listing_text(b: &Button) -> Option<String> {
    // Encoded stacks carry the server's listing facts in their final lore lines.
    // A custom item name must not override those facts.
    if let Some(at) = b.lore.iter().rposition(|l| l.starts_with('#')) {
        let first = &b.lore[at];
        if first.split('·').count() == 4 {
            return Some(first.clone());
        }
        if let Some(lines) = b.lore.get(at..at + 4) {
            let text = lines.join(" · ");
            if text.len() <= 4096 {
                return Some(text);
            }
        }
    }
    if b.name.starts_with('#') && b.name.split('·').count() == 4 {
        Some(b.name.clone())
    } else {
        None
    }
}
pub fn market_detail(page: &Page) -> bool {
    page.family == Kind::Market
        && page.buttons.iter().any(|b| {
            [39, 41].contains(&b.slot)
                && matches!(b.item.as_str(), "minecraft:map" | "minecraft:filled_map")
                && b.name.starts_with("Price chart")
        })
}
pub fn role(page: &Page, b: &Button) -> Role {
    use Role::*;
    let name = b.name.as_str();
    if b.slot == 53 && b.item == "minecraft:barrier" && matches!(name, "Close" | "Закрыть") {
        return Close;
    }
    if b.slot == 48
        && b.item == "minecraft:gold_ingot"
        && (name.starts_with("Balance:") || name.starts_with("Баланс:"))
    {
        return Balance;
    }
    if b.item == "minecraft:arrow" {
        if b.slot == 45 && matches!(name, "Back" | "Назад") {
            return Back;
        }
        if b.slot == 47 && matches!(name, "Previous page" | "Предыдущая страница")
        {
            return Previous;
        }
        if b.slot == 51 && matches!(name, "Next page" | "Следующая страница") {
            return Next;
        }
    }
    match page.family {
        Kind::Market => {
            if b.slot == 46
                && b.item == "minecraft:paper"
                && (name.contains("Movers over 24") || name.contains("Динамика"))
            {
                return News;
            }
            if market_detail(page) {
                if b.slot == 13 {
                    return Preview;
                }
                for (slot, n) in [(19, 1), (20, 16), (21, 64)] {
                    if b.slot == slot
                        && b.item == "minecraft:lime_concrete"
                        && name.starts_with(&format!("Buy {n} for "))
                    {
                        return Buy(n);
                    }
                }
                for (slot, n) in [(23, 1), (24, 16), (25, 64)] {
                    if b.slot == slot
                        && b.item == "minecraft:red_concrete"
                        && name.starts_with(&format!("Sell {n} for "))
                    {
                        return Sell(n);
                    }
                }
                if b.slot == 31 && b.item == "minecraft:hopper" && name.starts_with("Sell all ") {
                    return SellAll;
                }
                if [39, 41].contains(&b.slot)
                    && matches!(b.item.as_str(), "minecraft:map" | "minecraft:filled_map")
                    && name.starts_with("Price chart")
                {
                    return Chart(if b.slot == 39 { 1 } else { 7 });
                }
            }
            if b.slot < 45
                && b.lore.iter().any(|l| {
                    l.strip_suffix(" items")
                        .is_some_and(|n| n.parse::<u32>().is_ok())
                })
            {
                return Category;
            }
            if b.slot < 45
                && b.lore
                    .iter()
                    .any(|l| l.starts_with("Price: ") || l.starts_with("Цена: "))
            {
                return Goods;
            }
        }
        Kind::Auction => {
            if b.slot == 24 && b.item == "minecraft:red_stained_glass_pane" && name == "Back" {
                return Back;
            }
            if b.slot == 46 && b.item == "minecraft:chest" && name.starts_with("Your listings") {
                return Mine;
            }
            if b.slot == 50 && b.item == "minecraft:barrel" && name.starts_with("Your mail") {
                return Mail;
            }
            if b.slot == 49 && b.item == "minecraft:book" {
                return Help;
            }
            if b.slot == 49 && b.item == "minecraft:hopper" && name == "Claim everything" {
                return ClaimAll;
            }
            if b.slot == 20 && b.item == "minecraft:lime_concrete" {
                if name.starts_with("Buy ") {
                    return Buy(0);
                }
                if name.starts_with("Bid ") {
                    return Bid;
                }
            }
            if listing_text(b).is_some() {
                return if b.slot == 22
                    && page
                        .buttons
                        .iter()
                        .any(|b| b.slot == 20 && b.item == "minecraft:lime_concrete")
                {
                    Preview
                } else {
                    Listing
                };
            }
            if b.slot < 45
                && (name.starts_with("Claim ") || b.lore.iter().any(|l| l.starts_with("Claim ")))
            {
                return Claim;
            }
            if b.slot < 45 && name.ends_with(" listings)") {
                return Category;
            }
        }
    }
    Inspect
}
// A confirmation expires, and cannot be replayed after a page, slot or price update.
pub fn confirmed_button(
    page: &Page,
    window: &str,
    expected_window: &str,
    button: &Button,
    prepared_at: u64,
    now: u64,
) -> bool {
    now >= prepared_at
        && now - prepared_at <= 10_000
        && window == expected_window
        && page.buttons.iter().any(|b| b == button)
}
#[derive(Clone, Debug)]
pub enum Step {
    Wait,
    Click(usize),
    Finished,
    Stopped(Stop),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    OtherMenu,
    TimeLimit,
    PageNotChanged,
    PageLimit,
    CategoriesChanged,
}
impl Stop {
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::OtherMenu => language.text("Открыто другое меню", "Another menu opened"),
            Self::TimeLimit => language.text(
                "Сканирование ограничено 10 минутами",
                "Scan limited to 10 minutes",
            ),
            Self::PageNotChanged => language.text(
                "Сервер не переключил страницу",
                "Server did not change the page",
            ),
            Self::PageLimit => language.text("Достигнут предел страниц", "Page limit reached"),
            Self::CategoriesChanged => {
                language.text("Список категорий изменился", "Category list changed")
            }
        }
    }
}
/// How long the server's window may be missing before the catalogue session is over. The engine
/// leaves the container screen only on an explicit ContainerClose, and a server swapping one
/// window for the next sends the close and the open back to back; this only has to outlast the
/// gap between two network polls, and a longer absence is a real close. A wrong value costs
/// convenience, not data: ending the session never prunes the saved lots.
pub const WINDOW_GRACE_MS: u64 = 1500;
/// How long the server's window has been missing: the one owner of the grace rule, shared by
/// the catalogue walk and the catalogue session.
#[derive(Default)]
pub struct Absence(u64);
impl Absence {
    /// Called every frame with whether the window is missing. True once it has been missing
    /// for longer than the grace period.
    pub fn gone(&mut self, away: bool, now: u64) -> bool {
        if !away {
            self.0 = 0;
            return false;
        }
        if self.0 == 0 {
            self.0 = now.max(1);
        }
        now.saturating_sub(self.0) > WINDOW_GRACE_MS
    }
}
/// One opening of the auction catalogue. Every new opening walks the auction again, however
/// recently the last walk finished; frames of one opening, a walk already under way and a
/// window swapped for a moment never start a second walk.
#[derive(Default)]
pub struct Session {
    shown: bool,
    away: Absence,
}
impl Session {
    pub fn shown(&self) -> bool {
        self.shown
    }
    /// Called every frame with whether the server's window is open. True when the session ended.
    pub fn window(&mut self, open: bool, now: u64) -> bool {
        if self.away.gone(!open, now) && self.shown {
            self.shown = false;
            return true;
        }
        false
    }
    /// The auction's home page is on screen: whether a walk should start now.
    pub fn home(&mut self, walking: bool, refresh: bool) -> bool {
        let opening = !std::mem::replace(&mut self.shown, true);
        !walking && (opening || refresh)
    }
    /// The catalogue stepped aside for a server page (My listings, Mail).
    pub fn leave(&mut self) {
        self.shown = false;
    }
}
#[derive(Default)]
pub struct Scanner {
    pub active: bool,
    pub kind: Option<Kind>,
    pub pages: u32,
    /// The walk reached the end of the whole auction; limits, timeouts and stops leave it false.
    pub complete: bool,
    /// Lots the server counted on the home page ("All listings (264 listings)").
    expected: Option<u32>,
    /// Lot numbers seen on the pages of this walk.
    lots: BTreeSet<u64>,
    listing_pages: u32,
    last_listing: u64,
    /// A page with nothing left to click, waiting to be seen unchanged once more.
    candidate: Option<Vec<Button>>,
    blank_since: Option<u64>,
    categories: VecDeque<(usize, String)>,
    home_seen: bool,
    in_category: bool,
    waiting: Option<(u64, u64)>,
    last: u64,
    started: u64,
}
impl Scanner {
    pub fn start(&mut self, kind: Kind, now: u64) {
        *self = Self {
            active: true,
            kind: Some(kind),
            started: now,
            ..Self::default()
        };
    }
    pub fn stop(&mut self) {
        self.active = false;
        self.waiting = None;
    }
    pub fn next(&mut self, page: &Page, now: u64, delay: u64, limit: u32) -> Step {
        if !self.active {
            return Step::Wait;
        }
        if self.kind != Some(page.family) {
            self.stop();
            return Step::Stopped(Stop::OtherMenu);
        }
        if now.saturating_sub(self.started) > 600000 {
            self.stop();
            return Step::Stopped(Stop::TimeLimit);
        }
        if let Some((old, at)) = self.waiting {
            if old == page.fingerprint {
                if now.saturating_sub(at) > 8000 {
                    self.stop();
                    return Step::Stopped(Stop::PageNotChanged);
                }
                return Step::Wait;
            }
            self.waiting = None;
        }
        if now.saturating_sub(self.last) < delay {
            return Step::Wait;
        }
        if self.pages >= limit {
            self.stop();
            return Step::Stopped(Stop::PageLimit);
        }
        if page.family == Kind::Auction {
            // OpenScreen installs an empty window and the slots follow in later packets, so a
            // blank page is a window not filled yet, never the end of the auction.
            if page.buttons.is_empty() {
                let since = *self.blank_since.get_or_insert(now);
                if now.saturating_sub(since) > 8000 {
                    self.stop();
                    return Step::Stopped(Stop::PageNotChanged);
                }
                return Step::Wait;
            }
            self.blank_since = None;
            for b in page.buttons.iter().filter(|b| b.slot < 45) {
                if role(page, b) == Role::Listing
                    && let Some(id) = listing_number(b)
                    && self.lots.len() < 20_000
                {
                    self.lots.insert(id);
                }
            }
            if page.buttons.iter().any(|b| role(page, b) == Role::Listing)
                && self.last_listing != page.fingerprint
            {
                self.last_listing = page.fingerprint;
                self.listing_pages += 1;
            }
        }
        let arrow = |slot: usize, name: &str| {
            page.buttons.iter().any(|b| {
                b.slot == slot
                    && b.item == "minecraft:arrow"
                    && (b.name == name
                        || matches!(
                            (name, b.name.as_str()),
                            ("Back", "Назад") | ("Next page", "Следующая страница")
                        ))
            })
        };
        let click = match page.family {
            Kind::Auction => {
                if let Some(b) = page.buttons.iter().find(|b| {
                    b.slot == 10
                        && b.item == "minecraft:nether_star"
                        && (b.name.starts_with("All listings (")
                            || b.name.starts_with("Все лоты ("))
                }) {
                    if self.expected.is_none() {
                        self.expected = Some(count_in(&b.name).unwrap_or(u32::MAX));
                    }
                    // The server's own count of zero is the evidence of an empty auction.
                    (self.expected != Some(0)).then_some(b.slot)
                } else if arrow(51, "Next page") {
                    Some(51)
                } else {
                    None
                }
            }
            Kind::Market => {
                let home = !arrow(45, "Back")
                    && page.buttons.iter().any(|b| {
                        b.slot < 45
                            && b.lore.iter().any(|s| {
                                s.strip_suffix(" items")
                                    .or_else(|| s.strip_suffix(" предметов"))
                                    .is_some_and(|n| n.parse::<u32>().is_ok())
                            })
                    });
                if home {
                    if !self.home_seen {
                        self.home_seen = true;
                        self.categories.extend(
                            page.buttons
                                .iter()
                                .filter(|b| {
                                    b.slot < 45
                                        && b.lore.iter().any(|s| {
                                            s.strip_suffix(" items")
                                                .is_some_and(|n| n.parse::<u32>().is_ok())
                                        })
                                })
                                .map(|b| (b.slot, b.name.clone())),
                        );
                    }
                    if self.in_category {
                        self.categories.pop_front();
                        self.in_category = false;
                    }
                    match self.categories.front() {
                        Some((slot, name))
                            if page
                                .buttons
                                .iter()
                                .any(|b| b.slot == *slot && b.name == *name) =>
                        {
                            self.in_category = true;
                            Some(*slot)
                        }
                        Some(_) => {
                            self.stop();
                            return Step::Stopped(Stop::CategoriesChanged);
                        }
                        None => None,
                    }
                } else if arrow(51, "Next page") {
                    Some(51)
                } else if self.home_seen && arrow(45, "Back") {
                    Some(45)
                } else {
                    None
                }
            }
        };
        if let Some(slot) = click {
            self.candidate = None;
            self.pages += 1;
            self.last = now;
            self.waiting = Some((page.fingerprint, now));
            Step::Click(slot)
        } else {
            // Packets of one page change can reach the client across two polls, so a page
            // with nothing to click counts as the end only when it is seen unchanged on a
            // later frame too.
            if page.family == Kind::Auction && self.candidate.as_ref() != Some(&page.buttons) {
                self.candidate = Some(page.buttons.clone());
                return Step::Wait;
            }
            self.pages += 1;
            self.complete = self.reached_the_end(page);
            self.stop();
            Step::Finished
        }
    }
    /// Whether the last page seen proves every lot was walked. The server's count on the home
    /// page and the lots actually seen must agree, the last page must be a listing page with
    /// its previous-page button (a page not filled in yet has neither), and a walk that
    /// cannot show this stays partial: the saved lots are kept, nothing is pruned.
    fn reached_the_end(&self, page: &Page) -> bool {
        if page.family != Kind::Auction {
            return false;
        }
        match self.expected {
            Some(0) => self.lots.is_empty(),
            Some(n) if n != u32::MAX => {
                self.pages >= 2
                    // A little slack on big auctions: the count may include lots the pager never shows.
                    && self.lots.len() + n as usize / 50 >= n as usize
                    && page.buttons.iter().any(|b| role(page, b) == Role::Listing)
                    && (self.listing_pages < 2
                        || page.buttons.iter().any(|b| role(page, b) == Role::Previous))
            }
            _ => false,
        }
    }
}
/// The lot number of a listing button ("#1165 64 Netherrack · …").
fn listing_number(b: &Button) -> Option<u64> {
    listing_text(b)?
        .strip_prefix('#')?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}
/// The number inside the first parenthesis of "All listings (264 listings)".
fn count_in(name: &str) -> Option<u32> {
    let rest = name.split_once(" (")?.1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let after = rest[digits.len()..].chars().next();
    (!digits.is_empty() && matches!(after, Some(' ' | ')')))
        .then(|| digits.parse().ok())
        .flatten()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_stop_reasons_have_no_russian_text() {
        for stop in [
            Stop::OtherMenu,
            Stop::TimeLimit,
            Stop::PageNotChanged,
            Stop::PageLimit,
            Stop::CategoriesChanged,
        ] {
            let text = stop.label_in(Language::English);
            assert!(!text.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)));
        }
    }
    #[test]
    fn menu_roles_keep_services_out_of_goods_and_preserve_trade_quantities() {
        let mut p = Page {
            family: Kind::Market,
            fingerprint: 1,
            buttons: vec![Button {
                slot: 39,
                item: "minecraft:map".into(),
                name: "Price chart, 24 hours".into(),
                lore: vec![],
            }],
        };
        let b = |slot, item: &str, name: &str| Button {
            slot,
            item: item.into(),
            name: name.into(),
            lore: vec![],
        };
        assert_eq!(
            role(&p, &b(48, "minecraft:gold_ingot", "Balance: 3,935 coins")),
            Role::Balance
        );
        assert_eq!(
            role(&p, &b(20, "minecraft:lime_concrete", "Buy 16 for 33 coins")),
            Role::Buy(16)
        );
        assert_eq!(
            role(&p, &b(24, "minecraft:red_concrete", "Sell 16 for 32 coins")),
            Role::Sell(16)
        );
        assert_eq!(
            role(&p, &b(31, "minecraft:hopper", "Sell all your Wheat")),
            Role::SellAll
        );
        assert_eq!(
            role(&p, &b(19, "minecraft:stone", "Buy 1 for 2")),
            Role::Inspect
        );
        p.family = Kind::Auction;
        assert_eq!(
            role(&p, &b(24, "minecraft:red_stained_glass_pane", "Back")),
            Role::Back
        );
        assert_eq!(
            role(&p, &b(49, "minecraft:hopper", "Claim everything")),
            Role::ClaimAll
        );
        assert!(!Role::ClaimAll.navigation());
        assert!(Role::ClaimAll.transaction());
        let custom = Button {
            slot: 0,
            item: "minecraft:netherrack".into(),
            name: "Незерак".into(),
            lore: vec![
                "#1165 64 Netherrack".into(),
                "1 coins (0.02 each)".into(),
                "rdblclient".into(),
                "1d 22h left".into(),
            ],
        };
        assert_eq!(
            listing_text(&custom).unwrap(),
            "#1165 64 Netherrack · 1 coins (0.02 each) · rdblclient · 1d 22h left"
        );
        assert_eq!(role(&p, &custom), Role::Listing);
        let mut unnamed = custom.clone();
        unnamed.name.clear();
        assert_eq!(role(&p, &unnamed), Role::Listing);
        assert_eq!(listing_text(&unnamed), listing_text(&custom));
        let mut forged = custom.clone();
        forged.name = "#9 1 Dirt · 99 coins (99 each) · Wrong · 1d left".into();
        assert_eq!(listing_text(&forged), listing_text(&custom));
        let legacy = Button {
            name: listing_text(&custom).unwrap(),
            lore: Vec::new(),
            ..custom.clone()
        };
        assert_eq!(listing_text(&legacy), listing_text(&custom));
        assert_eq!(
            role(&p, &b(11, "minecraft:chest", "Other (91 listings)")),
            Role::Category
        );
    }
    #[test]
    fn confirmation_expires_and_rejects_changed_prices_and_windows() {
        let button = Button {
            slot: 20,
            item: "minecraft:lime_concrete".into(),
            name: "Buy 16 for 50 coins".into(),
            lore: vec!["Exact server total".into()],
        };
        let mut page = Page {
            family: Kind::Market,
            fingerprint: 1,
            buttons: vec![button.clone()],
        };
        assert!(confirmed_button(
            &page, "window-a", "window-a", &button, 100, 101
        ));
        assert!(!confirmed_button(
            &page, "window-b", "window-a", &button, 100, 101
        ));
        assert!(!confirmed_button(
            &page, "window-a", "window-a", &button, 100, 10101
        ));
        page.buttons[0].name = "Buy 16 for 60 coins".into();
        assert!(!confirmed_button(
            &page, "window-a", "window-a", &button, 100, 101
        ));
        page.buttons[0] = button.clone();
        page.buttons[0].lore.push("Limit changed".into());
        assert!(!confirmed_button(
            &page, "window-a", "window-a", &button, 100, 101
        ));
    }
    #[test]
    fn auction_scanner_never_clicks_buy_or_confirm_controls() {
        let mut s = Scanner::default();
        s.start(Kind::Auction, 0);
        let p = Page {
            family: Kind::Auction,
            fingerprint: 1,
            buttons: vec![Button {
                slot: 20,
                item: "minecraft:lime_concrete".into(),
                name: "Buy 64 for 100 coins".into(),
                lore: vec![],
            }],
        };
        // A page with nothing to click ends the walk only once seen unchanged again.
        assert!(matches!(s.next(&p, 1000, 500, 20), Step::Wait));
        assert!(matches!(s.next(&p, 1300, 500, 20), Step::Finished));
    }
    #[test]
    fn navigation_waits_for_a_different_page_and_times_out() {
        let mut s = Scanner::default();
        s.start(Kind::Auction, 0);
        let p = Page {
            family: Kind::Auction,
            fingerprint: 1,
            buttons: vec![Button {
                slot: 51,
                item: "minecraft:arrow".into(),
                name: "Next page".into(),
                lore: vec![],
            }],
        };
        assert!(matches!(s.next(&p, 1000, 500, 20), Step::Click(51)));
        assert!(matches!(s.next(&p, 2000, 500, 20), Step::Wait));
        assert!(matches!(s.next(&p, 10001, 500, 20), Step::Stopped(_)));
    }
    fn lot(slot: usize, id: u64) -> Button {
        Button {
            slot,
            item: "minecraft:diamond".into(),
            name: String::new(),
            lore: vec![
                format!("#{id} 4 Diamond"),
                "40 coins (10.00 each)".into(),
                "Alex_1".into(),
                "1d 2h left".into(),
            ],
        }
    }
    fn arrow(slot: usize, name: &str) -> Button {
        Button {
            slot,
            item: "minecraft:arrow".into(),
            name: name.into(),
            lore: vec![],
        }
    }
    fn home(count: &str) -> Page {
        Page {
            family: Kind::Auction,
            fingerprint: 1,
            buttons: vec![Button {
                slot: 10,
                item: "minecraft:nether_star".into(),
                name: format!("All listings ({count})"),
                lore: vec![],
            }],
        }
    }
    /// A listing page: lots from `first`, a previous arrow after page one, a next arrow unless last.
    fn listing_page(first: u64, lots: u64, previous: bool, next: bool) -> Page {
        let mut buttons: Vec<_> = (0..lots).map(|i| lot(i as usize, first + i)).collect();
        if previous {
            buttons.push(arrow(47, "Previous page"));
        }
        if next {
            buttons.push(arrow(51, "Next page"));
        }
        Page {
            family: Kind::Auction,
            fingerprint: 100 + first,
            buttons,
        }
    }
    fn blank() -> Page {
        Page {
            family: Kind::Auction,
            fingerprint: 7,
            buttons: vec![],
        }
    }
    /// Feeds pages the way the client does, a later frame each time; returns the last step.
    fn run(s: &mut Scanner, pages: &[Page]) -> Step {
        let mut now = 1000;
        let mut step = Step::Wait;
        for page in pages {
            for _ in 0..4 {
                now += 1000;
                step = s.next(page, now, 0, 50);
                if !matches!(step, Step::Wait) {
                    break;
                }
            }
        }
        step
    }
    fn walk() -> Scanner {
        let mut s = Scanner::default();
        s.start(Kind::Auction, 0);
        s
    }
    #[test]
    fn a_walk_to_a_settled_last_page_with_every_counted_lot_is_complete() {
        let mut s = walk();
        let step = run(
            &mut s,
            &[
                home("5 listings"),
                listing_page(1, 3, false, true),
                listing_page(4, 2, true, false),
            ],
        );
        assert!(matches!(step, Step::Finished));
        assert!(s.complete);
    }
    #[test]
    fn one_page_auction_is_complete_by_count_alone() {
        let mut s = walk();
        let step = run(
            &mut s,
            &[home("2 listings"), listing_page(1, 2, false, false)],
        );
        assert!(matches!(step, Step::Finished));
        assert!(s.complete);
    }
    #[test]
    fn a_blank_page_is_never_the_end_of_the_auction() {
        // Right after the server opens the window its slots are still empty.
        let mut s = walk();
        let step = run(
            &mut s,
            &[home("5 listings"), listing_page(1, 3, false, true), blank()],
        );
        assert!(matches!(step, Step::Wait));
        assert!(!s.complete && s.active);
        // It never fills in: the walk stops, not complete.
        assert!(matches!(
            s.next(&blank(), 20_000, 0, 50),
            Step::Stopped(Stop::PageNotChanged)
        ));
        assert!(!s.complete);
    }
    #[test]
    fn a_page_that_fills_in_between_frames_is_walked_on() {
        let mut s = walk();
        let step = run(
            &mut s,
            &[home("5 listings"), listing_page(1, 3, false, true)],
        );
        assert!(matches!(step, Step::Click(51)));
        // Next page arrives without its arrow first (packets split across polls) ...
        let partial = listing_page(4, 2, false, false);
        assert!(matches!(s.next(&partial, 9000, 0, 50), Step::Wait));
        // ... and complete on the next frame: still a candidate, not the end.
        let whole = listing_page(4, 2, true, false);
        assert!(matches!(s.next(&whole, 10_000, 0, 50), Step::Wait));
        assert!(matches!(s.next(&whole, 11_000, 0, 50), Step::Finished));
        assert!(s.complete);
    }
    #[test]
    fn a_partial_tail_page_without_the_previous_button_is_not_complete() {
        // Stable for two frames, lacks the previous-page button and half its lots.
        let mut s = walk();
        let step = run(
            &mut s,
            &[
                home("5 listings"),
                listing_page(1, 3, false, true),
                listing_page(4, 1, false, false),
            ],
        );
        assert!(matches!(step, Step::Finished));
        assert!(!s.complete, "{:?}", s.lots);
    }
    #[test]
    fn a_walk_that_saw_fewer_lots_than_the_server_counted_is_not_complete() {
        let mut s = walk();
        let step = run(
            &mut s,
            &[
                home("9 listings"),
                listing_page(1, 3, false, true),
                listing_page(4, 2, true, false),
            ],
        );
        assert!(matches!(step, Step::Finished));
        assert!(!s.complete);
    }
    #[test]
    fn a_home_page_without_a_readable_count_proves_nothing() {
        let mut s = walk();
        let step = run(&mut s, &[home("many"), listing_page(1, 2, false, false)]);
        assert!(matches!(step, Step::Finished));
        assert!(!s.complete);
    }
    #[test]
    fn an_empty_auction_is_complete_only_on_the_servers_zero_count() {
        let mut s = walk();
        assert!(matches!(run(&mut s, &[home("0 listings")]), Step::Finished));
        assert!(s.complete);
        // A blank or half-loaded home page is not that evidence.
        let mut t = walk();
        let step = run(&mut t, &[blank()]);
        assert!(matches!(step, Step::Wait));
        assert!(!t.complete);
        // Zero counted but lots on the page: the count was stale, keep the data.
        let mut u = walk();
        run(&mut u, &[home("0 listings")]);
        let mut v = walk();
        v.expected = Some(0);
        v.lots.insert(5);
        assert!(!v.reached_the_end(&home("0 listings")));
    }
    #[test]
    fn limits_timeouts_and_stops_are_not_complete() {
        let mut limited = walk();
        assert!(matches!(
            limited.next(&home("5 listings"), 1000, 0, 1),
            Step::Click(10)
        ));
        assert!(matches!(
            limited.next(&listing_page(1, 3, false, true), 2000, 0, 1),
            Step::Stopped(Stop::PageLimit)
        ));
        assert!(!limited.complete);
        let mut slow = walk();
        assert!(matches!(
            slow.next(&listing_page(1, 3, false, true), 700_000, 0, 50),
            Step::Stopped(Stop::TimeLimit)
        ));
        assert!(!slow.complete);
        // Closing or disconnecting stops the scanner from outside.
        let mut interrupted = walk();
        run(
            &mut interrupted,
            &[home("5 listings"), listing_page(1, 3, false, true)],
        );
        interrupted.stop();
        assert!(!interrupted.complete);
        let mut restarted = Scanner {
            complete: true,
            ..Scanner::default()
        };
        restarted.start(Kind::Auction, 0);
        assert!(!restarted.complete);
    }
    #[test]
    fn count_is_read_from_the_server_label_only() {
        assert_eq!(count_in("All listings (264 listings)"), Some(264));
        assert_eq!(count_in("Все лоты (12 лотов)"), Some(12));
        assert_eq!(count_in("All listings (0 listings)"), Some(0));
        assert_eq!(count_in("All listings (many)"), None);
        assert_eq!(count_in("All listings"), None);
    }
    #[test]
    fn every_new_opening_walks_again_even_right_after_a_finished_walk() {
        let mut session = Session::default();
        // First opening: the home page starts one walk, further frames of it do not.
        assert!(session.home(false, false));
        assert!(!session.home(true, false));
        assert!(
            !session.home(false, false),
            "the same opening never starts twice"
        );
        // The window closes and stays closed: the session is over.
        assert!(!session.window(false, 10_000));
        assert!(session.window(false, 10_000 + WINDOW_GRACE_MS + 1));
        // Reopened three seconds after the walk finished: a fresh walk.
        assert!(!session.window(true, 13_000));
        assert!(session.home(false, false));
    }
    #[test]
    fn a_walk_in_progress_blocks_a_new_opening_from_starting_a_second_one() {
        let mut session = Session::default();
        assert!(session.home(false, false));
        session.window(false, 1);
        assert!(session.window(false, 5000));
        assert!(
            !session.home(true, false),
            "the first walk is still running"
        );
        assert!(
            !session.home(false, false),
            "and the opening is already taken"
        );
    }
    #[test]
    fn a_window_swapped_for_a_moment_keeps_the_session() {
        let mut session = Session::default();
        assert!(session.home(false, false));
        assert!(!session.window(false, 5000));
        assert!(!session.window(false, 5000 + WINDOW_GRACE_MS));
        assert!(!session.window(true, 5000 + WINDOW_GRACE_MS + 1));
        assert!(!session.home(false, false), "same session: no second walk");
    }
    #[test]
    fn refresh_walks_again_inside_a_session_and_leaving_ends_it() {
        let mut session = Session::default();
        assert!(session.home(false, false));
        assert!(session.home(false, true));
        assert!(!session.home(true, true), "never while one is running");
        session.leave();
        assert!(!session.shown());
        assert!(session.home(false, false));
    }
}
