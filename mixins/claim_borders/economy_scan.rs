// A scanner may click navigation only, never an item, buy/sell button or bid.
use std::collections::VecDeque;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Market,
    Auction,
}
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub enum Step {
    Wait,
    Click(usize),
    Finished,
    Stopped(String),
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
            return Step::Stopped("Открыто другое меню".into());
        }
        if now.saturating_sub(self.started) > 600000 {
            self.stop();
            return Step::Stopped("Сканирование ограничено 10 минутами".into());
        }
        if let Some((old, at)) = self.waiting {
            if old == page.fingerprint {
                if now.saturating_sub(at) > 8000 {
                    self.stop();
                    return Step::Stopped("Сервер не переключил страницу".into());
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
            return Step::Stopped("Достигнут предел страниц".into());
        }
        let arrow = |slot: usize, name: &str| {
            page.buttons
                .iter()
                .any(|b| b.slot == slot && b.item == "minecraft:arrow" && (b.name==name||matches!((name,b.name.as_str()),("Back","Назад")|("Next page","Следующая страница"))))
        };
        let click = match page.family {
            Kind::Auction => {
                if let Some(b) = page.buttons.iter().find(|b| {
                    b.slot == 10
                        && b.item == "minecraft:nether_star"
                        && (b.name.starts_with("All listings (")||b.name.starts_with("Все лоты ("))
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
                                s.strip_suffix(" items").or_else(||s.strip_suffix(" предметов"))
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
                            return Step::Stopped("Список категорий изменился".into());
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
