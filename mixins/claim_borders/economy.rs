// Observed economy data and conservative estimates. No trading or game access.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub type Coins = i64; // hundredths of a coin; calculations never use binary floats.
pub const MAX: Coins = 100_000_000_000_000;
pub fn money(raw: &str) -> Option<Coins> {
    let s = raw.trim().trim_start_matches('+').replace(',', "");
    if s.is_empty() || s.len() > 20 {
        return None;
    }
    let (whole, fraction) = s.split_once('.').unwrap_or((&s, ""));
    if whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let w = whole.parse::<i64>().ok()?.checked_mul(100)?;
    let f = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse::<i64>().ok()?,
    };
    w.checked_add(f).filter(|v| *v <= MAX)
}
pub fn amount(text: &str) -> Option<Coins> {
    money(text.split_whitespace().next()?)
}
pub fn fmt(n: Coins) -> String {
    let sign = if n < 0 { "−" } else { "" };
    let a = n.unsigned_abs();
    if a.is_multiple_of(100) {
        format!("{sign}{}", a / 100)
    } else {
        format!("{sign}{}.{:02}", a / 100, a % 100)
    }
}
pub fn id(text: &str) -> Option<String> {
    let s = text.trim().to_ascii_lowercase().replace(' ', "_");
    let s = if s.contains(':') {
        s
    } else {
        format!("minecraft:{s}")
    };
    (s.len() <= 128
        && s.split_once(':')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_:/.-".contains(&c)))
    .then_some(s)
}
fn number(s: &str) -> Option<u32> {
    s.trim().replace(',', "").parse::<u32>().ok()
}
fn product(price: Coins, n: u32) -> Option<Coins> {
    price.checked_mul(i64::from(n)).filter(|v| *v <= MAX)
}
fn charge(amount: Coins, bp: u32, whole: bool) -> Option<Coins> {
    let unit = if whole { 100 } else { 1 };
    let v = i128::from(amount) * i128::from(bp);
    i64::try_from((v + i128::from(unit) * 5000) / (i128::from(unit) * 10000) * i128::from(unit))
        .ok()
}
#[derive(Clone, Copy)]
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub group: u8,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
}
pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "sidebar_tps",
        label: "TPS в sidebar",
        group: 5,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_tps",
        label: "TPS в Tab",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_opacity",
        label: "Фон Tab, %",
        group: 6,
        min: 10,
        max: 90,
        step: 5,
        default: 42,
    },
    Setting {
        key: "custom_sidebar",
        label: "Свой sidebar",
        group: 5,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "sidebar_width",
        label: "Ширина sidebar",
        group: 5,
        min: 140,
        max: 320,
        step: 20,
        default: 200,
    },
    Setting {
        key: "sidebar_scale",
        label: "Масштаб sidebar, %",
        group: 5,
        min: 65,
        max: 125,
        step: 5,
        default: 90,
    },
    Setting {
        key: "sidebar_opacity",
        label: "Фон sidebar, %",
        group: 5,
        min: 0,
        max: 100,
        step: 10,
        default: 42,
    },
    Setting {
        key: "sidebar_corner",
        label: "Угол sidebar: ЛВ ПВ ЛН ПН",
        group: 5,
        min: 0,
        max: 3,
        step: 1,
        default: 1,
    },
    Setting {
        key: "sidebar_limits",
        label: "Лимиты в sidebar",
        group: 5,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "custom_tab",
        label: "Свой Tab",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_columns",
        label: "Колонки Tab",
        group: 6,
        min: 1,
        max: 3,
        step: 1,
        default: 3,
    },
    Setting {
        key: "tab_ping",
        label: "Пинг игроков",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_friends",
        label: "Друзья первыми",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_distance",
        label: "Расстояние до видимых друзей",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "enabled",
        label: "Модуль экономики",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hud",
        label: "HUD экономики",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "overlay",
        label: "Подсказки в AH / market",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hide_sidebar",
        label: "Скрывать серверный sidebar",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "hud_balance",
        label: "HUD: баланс",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hud_limits",
        label: "HUD: остаток лимита",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hud_hand",
        label: "HUD: цена в руке",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "corner",
        label: "Угол HUD (0–3)",
        group: 1,
        min: 0,
        max: 3,
        step: 1,
        default: 2,
    },
    Setting {
        key: "width",
        label: "Ширина HUD",
        group: 1,
        min: 140,
        max: 360,
        step: 20,
        default: 220,
    },
    Setting {
        key: "scale",
        label: "Масштаб HUD, %",
        group: 1,
        min: 65,
        max: 150,
        step: 5,
        default: 85,
    },
    Setting {
        key: "opacity",
        label: "Непрозрачность HUD, %",
        group: 1,
        min: 0,
        max: 100,
        step: 10,
        default: 70,
    },
    Setting {
        key: "offset_x",
        label: "Сдвиг HUD X",
        group: 1,
        min: -300,
        max: 300,
        step: 10,
        default: 0,
    },
    Setting {
        key: "offset_y",
        label: "Сдвиг HUD Y",
        group: 1,
        min: -300,
        max: 300,
        step: 10,
        default: 0,
    },
    Setting {
        key: "quiet",
        label: "Тихие служебные ответы",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "auto_poll",
        label: "Обновлять наблюдаемые цены",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "held",
        label: "Проверять предмет в руке",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "interval",
        label: "Интервал запросов, с",
        group: 2,
        min: 10,
        max: 300,
        step: 10,
        default: 30,
    },
    Setting {
        key: "quote_ttl",
        label: "Свежесть цен / лимитов, с",
        group: 2,
        min: 30,
        max: 600,
        step: 30,
        default: 120,
    },
    Setting {
        key: "ah_ttl",
        label: "Свежесть лотов AH, с",
        group: 2,
        min: 15,
        max: 180,
        step: 15,
        default: 60,
    },
    Setting {
        key: "scan_delay",
        label: "Пауза сканера, мс",
        group: 2,
        min: 500,
        max: 3000,
        step: 250,
        default: 1000,
    },
    Setting {
        key: "pages",
        label: "Предел страниц сканера",
        group: 2,
        min: 1,
        max: 200,
        step: 5,
        default: 100,
    },
    Setting {
        key: "min_profit",
        label: "Минимум прибыли, монет",
        group: 3,
        min: 0,
        max: 10000,
        step: 5,
        default: 5,
    },
    Setting {
        key: "min_roi",
        label: "Минимум ROI, %",
        group: 3,
        min: 0,
        max: 100,
        step: 1,
        default: 5,
    },
    Setting {
        key: "slippage",
        label: "Запас на движение цены, %",
        group: 3,
        min: 0,
        max: 30,
        step: 1,
        default: 3,
    },
    Setting {
        key: "cash_reserve",
        label: "Оставлять монет на балансе",
        group: 3,
        min: 0,
        max: 100000,
        step: 100,
        default: 0,
    },
    Setting {
        key: "strict_history",
        label: "Помечать неполную историю покупок",
        group: 3,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "whole_fee",
        label: "Округлять комиссию до монеты",
        group: 4,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "override",
        label: "Ручная комиссия размещения AH",
        group: 4,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "listing_fee",
        label: "Ручная комиссия объявления, %",
        group: 4,
        min: 0,
        max: 50,
        step: 1,
        default: 1,
    },
    Setting {
        key: "listing_min",
        label: "Минимум комиссии AH, монет",
        group: 4,
        min: 0,
        max: 100,
        step: 1,
        default: 1,
    },
];
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preferences {
    values: BTreeMap<String, i64>,
}
impl Preferences {
    pub fn get(&self, key: &str) -> i64 {
        self.values.get(key).copied().unwrap_or_else(|| {
            SETTINGS
                .iter()
                .find(|s| s.key == key)
                .map_or(0, |s| s.default)
        })
    }
    pub fn flag(&self, key: &str) -> bool {
        self.get(key) != 0
    }
    pub fn adjust(&mut self, key: &str, dir: i64) {
        if let Some(s) = SETTINGS.iter().find(|s| s.key == key) {
            let n = if s.max == 1 {
                1 - self.get(key)
            } else {
                self.get(key)
                    .saturating_add(dir * s.step)
                    .clamp(s.min, s.max)
            };
            self.values.insert(key.into(), n);
        }
    }
    pub fn encode(&self) -> String {
        json!({"schema":1,"values":self.values}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 8192 {
            return Err("economy settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid economy settings")?;
        if v["schema"] != 1 {
            return Err("unknown economy settings".into());
        }
        let mut p = Self::default();
        for s in SETTINGS {
            if let Some(n) = v["values"][s.key].as_i64() {
                p.values.insert(s.key.into(), n.clamp(s.min, s.max));
            }
        }
        Ok(p)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Quote {
    pub buy: Option<Coins>,
    pub sell: Option<Coins>,
    pub at: u64,
    pub source: String,
    pub hour: Option<String>,
    pub day: Option<String>,
    pub volume: Option<u32>,
    pub left: Option<(u32, u64)>,
    pub history: Vec<(u64, Coins)>,
    pub blocked: u32,
    pub blocked_until: u64,
}
#[derive(Clone, Debug)]
pub struct Listing {
    pub id: u64,
    pub item: String,
    pub label: String,
    pub count: u32,
    pub cost: Coins,
    pub seller: String,
    pub bid: bool,
    pub seen: u64,
    pub expires: u64,
    pub generation: u64,
}
#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub fee_bp: Option<u32>,
    pub fee_min: Option<Coins>,
}
#[derive(Clone, Debug)]
pub struct Trade {
    pub at: u64,
    pub item: String,
    pub count: u32,
    pub gross: Coins,
    pub net: Coins,
    pub buying: bool,
}
#[derive(Default)]
pub struct Book {
    pub quotes: BTreeMap<String, Quote>,
    pub unsupported: BTreeSet<String>,
    pub listings: BTreeMap<u64, Listing>,
    pub rules: Rules,
    pub balance: Option<(Coins, u64)>,
    pub coin_left: Option<(Coins, u64)>,
    pub coin_cap: Option<Coins>,
    pub item_cap: Option<u32>,
    pub watch: BTreeSet<String>,
    pub trades: Vec<Trade>,
    pub inventory: BTreeMap<String, u32>,
    pub day: u64,
    pub generation: u64,
    pub complete_scan: bool,
    pub observed_pages: u32,
}
#[derive(Clone, Debug)]
pub struct Opportunity {
    pub listing: Listing,
    pub quantity: u32,
    pub gross: Coins,
    pub full_profit: Coins,
    pub profit: Coins,
    pub notes: Vec<String>,
    pub ready: bool,
}
fn fresh(at: u64, now: u64, ttl: i64) -> bool {
    now >= at && now - at <= ttl as u64 * 1000
}
fn quote_line(text: &str) -> Option<(Option<Coins>, Option<Coins>)> {
    let text = text.trim().trim_end_matches('.');
    if let Some(s) = text
        .strip_suffix(" each, to buy or sell")
        .or_else(|| text.strip_suffix(" each to buy or sell"))
    {
        let p = amount(s)?;
        return Some((Some(p), Some(p)));
    }
    if let Some(s) = text.strip_suffix(" each, to buy (the market doesn't buy it)") {
        return Some((amount(s), None));
    }
    if let Some(s) = text.strip_suffix(" each, to sell (the market doesn't sell it)") {
        return Some((None, amount(s)));
    }
    let mut buy = None;
    let mut sell = None;
    if let Some((_, s)) = text.split_once("the market pays ") {
        sell = amount(s);
    }
    if let Some((_, s)) = text.split_once("sells it for ") {
        buy = amount(s);
    }
    if let Some(s) = text.strip_prefix("Buy ").filter(|s| !s.contains(" for ")) {
        buy = amount(s);
    }
    if let Some((_, s)) = text.split_once(" · Sell ") {
        sell = amount(s);
    }
    if let Some(s) = text.strip_prefix("Sell ").filter(|s| !s.contains(" for ")) {
        sell = amount(s);
    }
    (buy.is_some() || sell.is_some()).then_some((buy, sell))
}
pub fn listing(label: &str, item: &str, now: u64, generation: u64) -> Option<Listing> {
    let parts: Vec<_> = label.split('·').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    let head = parts[0].strip_prefix('#')?;
    let mut h = head.splitn(3, ' ');
    let listing_id = h.next()?.parse().ok()?;
    let count = number(h.next()?).filter(|n| *n > 0 && *n <= 1_000_000)?;
    let title = h.next()?.to_owned();
    let bid = parts[1].starts_with("auction,");
    let cost = if bid {
        let s = parts[1].strip_prefix("auction, ")?;
        amount(s.strip_prefix("bid ").or_else(|| s.strip_prefix("from "))?)?
    } else {
        amount(parts[1])?
    };
    let seller = parts[2];
    if seller.is_empty()
        || seller.len() > 16
        || !seller
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }
    let expires = duration(parts[3].strip_suffix(" left")?)?.checked_add(now)?;
    Some(Listing {
        id: listing_id,
        item: id(item)?,
        label: title,
        count,
        cost,
        seller: seller.into(),
        bid,
        seen: now,
        expires,
        generation,
    })
}
fn duration(raw: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut digits = String::new();
    for c in raw.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if c.is_whitespace() {
            continue;
        } else {
            let n = digits.parse::<u64>().ok()?;
            digits.clear();
            let scale = match c {
                'd' => 86400000,
                'h' => 3600000,
                'm' => 60000,
                's' => 1000,
                _ => return None,
            };
            total = total.checked_add(n.checked_mul(scale)?)?;
        }
    }
    (digits.is_empty() && total > 0 && total <= 30 * 86400000).then_some(total)
}
impl Book {
    pub fn roll_day(&mut self, now: u64) {
        let day = now / 86400000;
        if self.day != day {
            self.day = day;
            self.coin_left = None;
            for q in self.quotes.values_mut() {
                q.left = None;
            }
        }
    }
    pub fn observe(
        &mut self,
        item: &str,
        buy: Option<Coins>,
        sell: Option<Coins>,
        now: u64,
        source: &str,
    ) {
        if self.quotes.len() >= 1024 && !self.quotes.contains_key(item) {
            return;
        }
        self.unsupported.remove(item);
        let q = self.quotes.entry(item.into()).or_default();
        q.buy = buy;
        q.sell = sell;
        q.at = now;
        q.source = source.into();
        if let Some(p) = sell.or(buy)
            && q.history
                .last()
                .is_none_or(|(t, v)| *v != p || now.saturating_sub(*t) >= 60000)
        {
            q.history.push((now, p));
            if q.history.len() > 120 {
                q.history.remove(0);
            }
        }
    }
    pub fn market_slot(&mut self, item: &str, lines: &[String], now: u64) {
        let (mut buy, mut sell, mut quoted) = (None, None, false);
        for line in lines {
            if let Some((b, s)) = quote_line(line) {
                buy = b.or(buy);
                sell = s.or(sell);
                quoted = true;
            }
            if let Some(s) = line.strip_prefix("You may still sell ")
                && let Some(n) = s.strip_suffix(" today").and_then(number)
            {
                self.quotes.entry(item.into()).or_default().left = Some((n, now));
            }
            if let Some(value) = line.strip_prefix("1 h ") {
                self.quotes.entry(item.into()).or_default().hour = Some(value.into());
            }
            if let Some(value) = line.strip_prefix("24 h ") {
                self.quotes.entry(item.into()).or_default().day = Some(value.into());
            }

            self.server(line, now);
        }
        if quoted {
            self.observe(item, buy, sell, now, "GUI market");
        }
    }
    pub fn server(&mut self, text: &str, now: u64) -> bool {
        if text.len() > 4096 {
            return false;
        }
        self.roll_day(now);
        let text = text.trim();
        if let Some(item) = unsupported_item(text) {
            if self.unsupported.len() < 1024 {
                self.unsupported.insert(item.clone());
            }
            self.watch.remove(&item);
            let q = self.quotes.entry(item).or_default();
            q.buy = None;
            q.sell = None;
            return true;
        }
        if let Some((name, body)) = text.split_once(": ")
            && let Some((buy, sell)) = quote_line(body)
            && let Some(item) = id(name)
        {
            self.observe(&item, buy, sell, now, "/worth");
            return true;
        }
        if let Some(s) = text.strip_prefix("Selling today: ") {
            if let Some((left, rest)) = s.split_once(" left of your ") {
                if let Some(n) = amount(left) {
                    self.coin_left = Some((n, now));
                }
                self.coin_cap = amount(rest);
            }
            if let Some((_, rest)) = s.split_once("at most ") {
                self.item_cap = rest
                    .split_once(" of each item a day")
                    .and_then(|(n, _)| number(n));
            }
            return true;
        }
        if let Some(rest) = text.strip_prefix("You may sell ")
            && let Some((n, rest)) = rest.split_once(" more ")
            && let Some(item) = rest.strip_suffix(" today.").and_then(id)
            && let Some(n) = number(n)
        {
            self.quotes.entry(item).or_default().left = Some((n, now));
            return true;
        }
        if let Some(s) = text
            .strip_prefix("Balance: ")
            .or_else(|| text.strip_prefix("Your balance: "))
            && let Some(b) = amount(s)
        {
            self.balance = Some((b, now));
            return true;
        }
        if let Some((kind, rest)) = text.split_once(' ')
            && ["Sold", "Bought"].contains(&kind)
            && !rest.contains(" from ")
            && let Some((count, rest)) = rest.split_once(' ')
            && let Some((item, rest)) = rest.split_once(" for ")
            && let (Some(n), Some(item), Some(gross)) = (number(count), id(item), amount(rest))
        {
            let buying = kind == "Bought";
            let net = if buying {
                gross
                    .checked_add(
                        rest.split_once(" + ")
                            .and_then(|(_, s)| amount(s))
                            .unwrap_or(0),
                    )
                    .unwrap_or(gross)
            } else {
                rest.rsplit_once(": +")
                    .and_then(|(_, s)| amount(s))
                    .unwrap_or(gross)
            };
            if !buying {
                if let Some((left, at)) = &mut self.coin_left {
                    *left = left.saturating_sub(gross);
                    *at = now;
                }
                if let Some((left, at)) = &mut self.quotes.entry(item.clone()).or_default().left {
                    *left = left.saturating_sub(n);
                    *at = now;
                }
            }
            self.trades.push(Trade {
                at: now,
                item: item.clone(),
                count: n,
                gross,
                net,
                buying,
            });
            if self.trades.len() > 256 {
                self.trades.remove(0);
            }
            let q = self.quotes.entry(item).or_default();
            q.at = 0;
            if buying {
                q.blocked = q.blocked.saturating_add(n);
                q.blocked_until = now + 24 * 3600000;
            }
            return true;
        }
        if let Some(rest) = text.strip_prefix("You bought ")
            && let Some((n, rest)) = rest.split_once(' ')
            && let Some((item, rest)) = rest.split_once(" from the market in the last ")
            && let Some((hours, _)) = rest.split_once(" h:")
            && let (Some(n), Some(item), Some(h)) = (number(n), id(item), number(hours))
        {
            let q = self.quotes.entry(item).or_default();
            q.blocked = n;
            q.blocked_until = now + u64::from(h.min(168)) * 3600000;
            return true;
        }
        if text.starts_with("Moves: ")
            && let Some((_, item)) = text.rsplit_once("Chart: /market ")
            && let Some(item) = id(item)
        {
            let q = self.quotes.entry(item).or_default();
            if let Some((moves, rest)) = text
                .trim_start_matches("Moves: ")
                .split_once("; traded in 24 h: ")
            {
                let v: Vec<_> = moves.split(',').collect();
                q.hour = v.first().map(|s| s.trim().into());
                q.day = v.get(1).map(|s| s.trim().into());
                q.volume = rest.split_once('.').and_then(|(n, _)| number(n));
            }
            return true;
        }
        false
    }
    pub fn full_lot_profit(&self, l: &Listing, p: &Preferences) -> Option<Coins> {
        if l.bid {
            return None;
        }
        let price = self.quotes.get(&l.item)?.sell.filter(|p| *p > 0)?;
        let price =
            i64::try_from(i128::from(price) * i128::from(100 - p.get("slippage")) / 100).ok()?;
        product(price, l.count)?.checked_sub(l.cost)
    }
    pub fn opportunity(
        &self,
        l: &Listing,
        p: &Preferences,
        now: u64,
        own: &str,
    ) -> Option<Opportunity> {
        if l.bid
            || l.seller.eq_ignore_ascii_case(own)
            || l.expires <= now
            || l.generation != self.generation
        {
            return None;
        }
        let q = self.quotes.get(&l.item)?;
        let price = q.sell.filter(|v| *v > 0)?;
        let mut notes = Vec::new();
        if !fresh(q.at, now, p.get("quote_ttl")) {
            notes.push("Устарела цена".into());
        }
        if !fresh(l.seen, now, p.get("ah_ttl")) {
            notes.push("Лот нужно перепроверить".into());
        }
        let mut n = l.count;
        if let Some((left, at)) = self.coin_left {
            if left == 0 {
                notes.push("Дневной лимит монет исчерпан".into());
            }
            n = n.min(u32::try_from(left / price).unwrap_or(u32::MAX));
            if !fresh(at, now, p.get("quote_ttl")) {
                notes.push("Обнови денежный лимит".into());
            }
        } else {
            notes.push("Неизвестен денежный лимит".into());
        }
        if let Some((left, at)) = q.left {
            if left == 0 {
                notes.push("Дневной лимит предмета исчерпан".into());
            }
            n = n.min(left);
            if !fresh(at, now, p.get("quote_ttl")) {
                notes.push("Обнови лимит предмета".into());
            }
        } else {
            notes.push("Неизвестен остаток по предмету".into());
        }
        let back = if q.blocked_until > now { q.blocked } else { 0 };
        let have = self.inventory.get(&l.item).copied().unwrap_or(0);
        n = n.min(have.saturating_add(l.count).saturating_sub(back));
        if back > 0 {
            notes.push(format!("Ограничение обратной продажи: {back} шт."));
        }
        if p.flag("strict_history") {
            notes.push("История покупок market может быть неполной".into());
        }
        if n < l.count {
            notes.push(format!("Продажа {n}/{}; остаток не оценён", l.count));
        }
        let conservative =
            i64::try_from(i128::from(price) * i128::from(100 - p.get("slippage")) / 100).ok()?;
        let gross = product(conservative, n)?;
        let net = gross;
        let full_profit = self.full_lot_profit(l, p)?;
        let profit = net.checked_sub(l.cost)?;
        let roi = if l.cost > 0 {
            profit as f64 / l.cost as f64 * 100.0
        } else {
            0.0
        };
        if self
            .balance
            .is_some_and(|(_, at)| !fresh(at, now, p.get("quote_ttl")))
        {
            notes.push("Баланс нужно обновить".into());
        }
        let budget = self
            .balance
            .map(|(v, _)| v.saturating_sub(p.get("cash_reserve") * 100));
        match budget {
            None => notes.push("Баланс неизвестен".into()),
            Some(n) if n < l.cost => notes.push("Недостаточно свободных монет".into()),
            _ => {}
        }

        let ready = notes.is_empty()
            && n > 0
            && profit >= p.get("min_profit") * 100
            && roi >= p.get("min_roi") as f64;
        Some(Opportunity {
            listing: l.clone(),
            quantity: n,
            gross,
            full_profit,
            profit,
            notes,
            ready,
        })
    }
    pub fn opportunities(&self, p: &Preferences, now: u64, own: &str) -> Vec<Opportunity> {
        let mut rows: Vec<_> = self
            .listings
            .values()
            .filter_map(|l| self.opportunity(l, p, now, own))
            .collect();
        rows.sort_by(|a, b| {
            b.ready
                .cmp(&a.ready)
                .then(b.profit.cmp(&a.profit))
                .then(a.listing.id.cmp(&b.listing.id))
        });
        rows
    }
    pub fn hand_sale(
        &self,
        item: &str,
        count: u32,
        p: &Preferences,
        now: u64,
    ) -> Option<(u32, Coins)> {
        let q = self.quotes.get(item)?;
        let price = q.sell.filter(|v| *v > 0)?;
        let (coins, ct) = self.coin_left?;
        let (left, lt) = q.left?;
        if !fresh(q.at, now, p.get("quote_ttl"))
            || !fresh(ct, now, p.get("quote_ttl"))
            || !fresh(lt, now, p.get("quote_ttl"))
        {
            return None;
        }
        let blocked = if q.blocked_until > now { q.blocked } else { 0 };
        let inventory = self.inventory.get(item).copied().unwrap_or(0);
        let quantity = count
            .min(left)
            .min(u32::try_from(coins / price).unwrap_or(u32::MAX))
            .min(inventory.min(count).saturating_sub(blocked));
        if quantity == 0 {
            return None;
        }
        Some((quantity, product(price, quantity)?))
    }
    pub fn resale(&self, cost: Coins, target: Coins, p: &Preferences) -> Option<(Coins, Coins)> {
        let (fee, min) = if p.flag("override") {
            (
                (p.get("listing_fee") * 100) as u32,
                p.get("listing_min") * 100,
            )
        } else {
            (self.rules.fee_bp?, self.rules.fee_min?)
        };
        let fee = charge(target, fee, p.flag("whole_fee"))?.max(min);
        Some((fee, target.checked_sub(fee)?.checked_sub(cost)?))
    }
    pub fn sidebar(&mut self, rows: &[(String, String)], now: u64) {
        for (label, value) in rows {
            let label = label.trim().to_ascii_lowercase();
            if (label == "balance" || label == "balance:" || label == "баланс")
                && let Some(v) = amount(value)
            {
                self.balance = Some((v, now));
            }
        }
    }
    pub fn encode(&self, scope: &str) -> String {
        json!({"schema":1,"scope":scope,"day":self.day,"quotes":self.quotes.iter().map(|(id,q)|json!([id,q.buy,q.sell,q.at,q.history,q.blocked,q.blocked_until])).collect::<Vec<_>>(),"watch":self.watch,"rules":[null,null,self.rules.fee_bp,self.rules.fee_min],"trades":self.trades.iter().map(|t|json!([t.at,t.item,t.count,t.gross,t.net,t.buying])).collect::<Vec<_>>()}).to_string()
    }
    pub fn decode(raw: &str, scope: &str) -> Result<Self, String> {
        if raw.len() > 4 * 1024 * 1024 {
            return Err("economy cache too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid economy cache")?;
        if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
            return Err("economy cache server/schema mismatch".into());
        }
        let mut book = Self {
            day: v["day"].as_u64().unwrap_or(0),
            ..Self::default()
        };
        let rows = v["quotes"]
            .as_array()
            .filter(|v| v.len() <= 1024)
            .ok_or("invalid quotes")?;
        for row in rows {
            let key = row[0].as_str().and_then(id).ok_or("invalid item id")?;
            let m = |v: &Value| v.as_i64().filter(|n| *n >= 0 && *n <= MAX);
            let mut q = Quote {
                buy: m(&row[1]),
                sell: m(&row[2]),
                at: row[3].as_u64().unwrap_or(0),
                source: "сохранено".into(),
                blocked: row[5].as_u64().unwrap_or(0).min(u64::from(u32::MAX)) as u32,
                blocked_until: row[6].as_u64().unwrap_or(0),
                ..Quote::default()
            };
            if let Some(h) = row[4].as_array() {
                for r in h.iter().take(120) {
                    if let (Some(t), Some(p)) = (r[0].as_u64(), m(&r[1])) {
                        q.history.push((t, p));
                    }
                }
            }
            book.quotes.insert(key, q);
        }
        if let Some(w) = v["watch"].as_array() {
            book.watch
                .extend(w.iter().take(64).filter_map(|v| v.as_str().and_then(id)));
        }
        book.rules.fee_bp = v["rules"][2]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n <= 10000);
        book.rules.fee_min = v["rules"][3].as_i64().filter(|n| *n >= 0 && *n <= MAX);
        let m = |v: &Value| v.as_i64().filter(|n| *n >= 0 && *n <= MAX);
        if let Some(trades) = v["trades"].as_array() {
            for t in trades.iter().take(256) {
                if let (Some(at), Some(item), Some(n), Some(gross), Some(net), Some(buying)) = (
                    t[0].as_u64(),
                    t[1].as_str().and_then(id),
                    t[2].as_u64().and_then(|n| u32::try_from(n).ok()),
                    m(&t[3]),
                    m(&t[4]),
                    t[5].as_bool(),
                ) {
                    book.trades.push(Trade {
                        at,
                        item,
                        count: n,
                        gross,
                        net,
                        buying,
                    });
                }
            }
        }
        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: u64 = 100_000;
    fn sample() -> (Book, Preferences, Listing) {
        let mut book = Book::default();
        assert!(book.server("amethyst_block: 4.00 coins each to buy or sell.", NOW));
        assert!(book.server("Selling today: 2,000 coins left of your 2,000 coins a day, at most 640 of each item a day.",NOW));
        assert!(book.server("You may sell 640 more amethyst_block today.", NOW));
        book.balance = Some((100_000, NOW));
        let mut p = Preferences::default();
        p.adjust("strict_history", 1);
        p.adjust("slippage", -1);
        p.adjust("slippage", -1);
        p.adjust("slippage", -1);
        let lot = listing(
            "#248 22 Amethyst Block · 70 coins (3.18 each) · winmastt · 1d 16h left",
            "amethyst_block",
            NOW,
            0,
        )
        .unwrap();
        (book, p, lot)
    }
    #[test]
    fn decimal_quotes_and_total_lot_cost_are_exact() {
        assert_eq!(money("1.99"), Some(199));
        assert_eq!(money("2,000.50"), Some(200050));
        assert_eq!(money("1.999"), None);
        let (book, p, l) = sample();
        assert_eq!(l.cost, 7000);
        assert_eq!(book.quotes[&l.item].sell, Some(400));
        let o = book.opportunity(&l, &p, NOW, "me").unwrap();
        assert_eq!((o.gross, o.profit), (8800, 1800));
        assert!(o.ready);
    }
    #[test]
    fn abolished_taxes_ignore_old_rates_and_bill() {
        let (mut book, p, l) = sample();

        let o = book.opportunity(&l, &p, NOW, "me").unwrap();
        assert_eq!(o.profit, 1800);
        assert!(o.ready);
        assert!(book.resale(7000, 10000, &p).is_none()); // A listing fee is unknown, not inferred from old VAT.
        book.rules.fee_bp = Some(100);
        book.rules.fee_min = Some(100);
        assert_eq!(book.resale(7000, 10000, &p), Some((100, 2900)));
    }
    #[test]
    fn partial_sale_still_pays_for_entire_lot_and_caps_use_gross() {
        let (mut book, p, l) = sample();
        book.coin_left = Some((4000, NOW));
        let o = book.opportunity(&l, &p, NOW, "me").unwrap();
        assert_eq!((o.quantity, o.profit), (10, -3000));
        assert!(!o.ready);
        book.coin_left = Some((0, NOW));
        let o = book.opportunity(&l, &p, NOW, "me").unwrap();
        assert_eq!((o.quantity, o.profit), (0, -7000));
        assert!(!o.ready);
    }
    #[test]
    fn bids_own_expired_and_uncertain_data_are_not_buy_signals() {
        let (mut book, p, mut l) = sample();
        l.bid = true;
        assert!(book.opportunity(&l, &p, NOW, "me").is_none());
        l.bid = false;
        assert!(book.opportunity(&l, &p, NOW, "WINMASTT").is_none());
        assert!(book.opportunity(&l, &p, l.expires, "me").is_none());
        book.coin_left = None;
        assert!(!book.opportunity(&l, &p, NOW, "me").unwrap().ready);
        book.coin_left = Some((200000, NOW));
        assert!(!book.opportunity(&l, &p, NOW + 3600000, "me").unwrap().ready);
        assert!(
            !book
                .opportunity(&l, &Preferences::default(), NOW, "me")
                .unwrap()
                .ready
        );
    }
    #[test]
    fn hand_sale_respects_recent_market_purchases_and_fresh_limits() {
        let (mut book, p, l) = sample();
        book.inventory.insert(l.item.clone(), 30);
        let q = book.quotes.get_mut(&l.item).unwrap();
        q.blocked = 20;
        q.blocked_until = NOW + 10000;
        assert_eq!(book.hand_sale(&l.item, 22, &p, NOW), Some((2, 800)));
        book.coin_left = Some((0, NOW));
        assert!(book.hand_sale(&l.item, 22, &p, NOW).is_none());
    }
    #[test]
    fn cached_prices_do_not_restore_wallet_limits_or_auction_listings() {
        let (mut book, p, l) = sample();
        book.listings.insert(l.id, l.clone());
        book.watch.insert(l.item.clone());
        let raw = book.encode("server/account");
        let restored = Book::decode(&raw, "server/account").unwrap();
        assert_eq!(restored.quotes[&l.item].sell, Some(400));
        assert!(restored.balance.is_none());
        assert!(restored.coin_left.is_none());
        assert!(restored.quotes[&l.item].left.is_none());
        assert!(restored.listings.is_empty());
        assert!(Book::decode(&raw, "another/account").is_err());
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
    }
    #[test]
    fn transaction_buttons_are_never_interpreted_as_unit_prices() {
        assert!(quote_line("Buy 64 for 128 coins").is_none());
        assert!(quote_line("Sell 64 for 128 coins").is_none());
    }
}

pub fn unsupported_item(text: &str) -> Option<String> {
    text.strip_prefix("The market doesn't trade ")
        .and_then(|s| s.strip_suffix('.'))
        .and_then(id)
}
pub fn quote_item(text: &str) -> Option<String> {
    let (name, body) = text.split_once(": ")?;
    quote_line(body)?;
    id(name)
}

#[cfg(test)]
mod live_server_tests {
    use super::*;
    #[test]
    fn live_server_comma_decimal_and_one_way_prices_are_parsed() {
        let mut b = Book::default();
        let now = 100000;
        assert!(b.server("leather: 3.01 coins each, to buy or sell.", now));
        assert_eq!(
            (
                b.quotes["minecraft:leather"].buy,
                b.quotes["minecraft:leather"].sell
            ),
            (Some(301), Some(301))
        );
        assert!(b.server(
            "gravel: 2 coins each, to buy (the market doesn't buy it).",
            now
        ));
        assert_eq!(
            (
                b.quotes["minecraft:gravel"].buy,
                b.quotes["minecraft:gravel"].sell
            ),
            (Some(200), None)
        );
        b.watch.insert("minecraft:brown_wool".into());
        assert!(b.server("The market doesn't trade brown wool.", now));
        assert!(b.unsupported.contains("minecraft:brown_wool"));
        assert!(!b.watch.contains("minecraft:brown_wool"));
        b.market_slot(
            "minecraft:carrot",
            &["1.99 coins each, to buy or sell.".into()],
            now,
        );
        assert_eq!(b.quotes["minecraft:carrot"].sell, Some(199));
    }
    #[test]
    fn full_lot_profit_remains_visible_when_daily_allowance_is_exhausted() {
        let now = 100000;
        let mut b = Book::default();
        b.server("leather: 3.01 coins each, to buy or sell.", now);
        b.server("Selling today: 0 coins left of your 2,000 coins a day, at most 640 of each item a day.",now);
        let l = listing(
            "#7 64 Leather · 100 coins (1.56 each) · Alex · 1d left",
            "leather",
            now,
            0,
        )
        .unwrap();
        let o = b
            .opportunity(&l, &Preferences::default(), now, "me")
            .unwrap();
        assert!(o.full_profit > 0);
        assert_eq!(o.quantity, 0);
        assert_eq!(o.profit, -10000);
        assert!(!o.ready);
    }
    #[test]
    fn obsolete_tax_preferences_and_cache_have_no_effect() {
        let p = Preferences::decode(
            r#"{"schema":1,"values":{"taxes_enabled":1,"market_tax":50,"vat":50}}"#,
        )
        .unwrap();
        assert!(!p.encode().contains("tax"));
        let b=Book::decode(r#"{"schema":1,"scope":"s","quotes":[],"rules":[200,500,100,100],"tax_lines":["old bill"]}"#,"s").unwrap();
        assert_eq!(b.resale(7000, 10000, &p), Some((100, 2900)));
        assert!(!b.encode("s").contains("tax"));
    }
}

pub fn delta(n: Coins) -> String {
    if n > 0 {
        format!("+{}", fmt(n))
    } else {
        fmt(n)
    }
}
