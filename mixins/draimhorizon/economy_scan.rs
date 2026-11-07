// A scanner may click navigation only, never an item, buy/sell button or bid.
use claim_borders_relay_client::translation::Language;
use std::collections::VecDeque;
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
#[derive(Default)]
pub struct Scanner {
    pub active: bool,
    pub kind: Option<Kind>,
    pub pages: u32,
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
                    Some(b.slot)
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
            self.pages += 1;
            self.last = now;
            self.waiting = Some((page.fingerprint, now));
            Step::Click(slot)
        } else {
            self.pages += 1;
            self.stop();
            Step::Finished
        }
    }
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
        assert!(matches!(s.next(&p, 1000, 500, 20), Step::Finished));
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
}
