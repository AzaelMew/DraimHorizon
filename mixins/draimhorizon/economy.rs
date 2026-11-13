// Observed economy data and conservative estimates. No trading or game access.
use claim_borders_relay_client::translation::Language;
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
// SMP abbreviates large scoreboard balances, while /bal supplies the exact value.
fn sidebar_amount(text: &str) -> Option<Coins> {
    let value = text.split_whitespace().next()?;
    let (digits, multiplier) = match value.as_bytes().last()? {
        b'k' | b'K' => (&value[..value.len() - 1], 1_000),
        b'm' | b'M' => (&value[..value.len() - 1], 1_000_000),
        b'b' | b'B' => (&value[..value.len() - 1], 1_000_000_000),
        _ => return money(value),
    };
    money(digits)?.checked_mul(multiplier).filter(|v| *v <= MAX)
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
    pub label_en: &'static str,
    pub group: u8,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
}
impl Setting {
    pub fn is_toggle(&self) -> bool {
        self.min == 0 && self.max == 1 && self.step == 1
    }
    pub fn is_choice(&self) -> bool {
        matches!(self.key, "corner" | "sidebar_corner")
    }
    pub fn value_label(&self, value: i64, language: Language) -> String {
        if self.is_toggle() {
            return if value != 0 {
                language.text("вкл.", "on")
            } else {
                language.text("выкл.", "off")
            }
            .into();
        }
        if self.is_choice() {
            return [
                language.text("Сверху слева", "Top left"),
                language.text("Сверху справа", "Top right"),
                language.text("Снизу слева", "Bottom left"),
                language.text("Снизу справа", "Bottom right"),
            ][value.clamp(0, 3) as usize]
                .into();
        }
        let unit = match self.key {
            "tab_opacity" | "sidebar_scale" | "sidebar_opacity" | "scale" | "opacity"
            | "min_roi" | "slippage" | "listing_fee" => "%",
            "interval" | "quote_ttl" | "ah_ttl" => language.text("с", "s"),
            "scan_delay" => language.text("мс", "ms"),
            "sidebar_width" | "width" | "offset_x" | "offset_y" => language.text("пкс", "px"),
            "min_profit" | "cash_reserve" | "listing_min" => language.text("мон.", "coins"),
            _ => "",
        };
        if unit == "%" {
            format!("{value}%")
        } else if unit.is_empty() {
            value.to_string()
        } else {
            format!("{value} {unit}")
        }
    }
}
pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "custom_auction",
        label: "Оформление аукциона (F8)",
        label_en: "Custom auction screen (F8)",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "confirm_buy",
        label: "Спрашивать перед покупкой на аукционе",
        label_en: "Ask before buying at the auction",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "trade_large",
        label: "Крупные окна аукциона и рынка",
        label_en: "Large auction and market windows",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "custom_market",
        label: "Оформление рынка (F8)",
        label_en: "Custom market screen (F8)",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "sidebar_tps",
        label: "Частота тиков сервера в боковой панели",
        label_en: "Server tick rate in sidebar",
        group: 5,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_tps",
        label: "Частота тиков сервера в списке игроков",
        label_en: "Server tick rate in player list",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_opacity",
        label: "Непрозрачность списка игроков",
        label_en: "Player list opacity",
        group: 6,
        min: 10,
        max: 90,
        step: 5,
        default: 42,
    },
    Setting {
        key: "custom_sidebar",
        label: "Оформление боковой панели",
        label_en: "Custom sidebar",
        group: 5,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "sidebar_width",
        label: "Ширина боковой панели",
        label_en: "Sidebar width",
        group: 5,
        min: 140,
        max: 320,
        step: 20,
        default: 200,
    },
    Setting {
        key: "sidebar_scale",
        label: "Масштаб боковой панели",
        label_en: "Sidebar scale",
        group: 5,
        min: 65,
        max: 125,
        step: 5,
        default: 90,
    },
    Setting {
        key: "sidebar_opacity",
        label: "Непрозрачность боковой панели",
        label_en: "Sidebar opacity",
        group: 5,
        min: 0,
        max: 100,
        step: 10,
        default: 42,
    },
    Setting {
        key: "sidebar_corner",
        label: "Положение боковой панели",
        label_en: "Sidebar position",
        group: 5,
        min: 0,
        max: 3,
        step: 1,
        default: 1,
    },
    Setting {
        key: "custom_tab",
        label: "Оформление списка игроков",
        label_en: "Custom player list",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_columns",
        label: "Число колонок",
        label_en: "Column count",
        group: 6,
        min: 1,
        max: 3,
        step: 1,
        default: 3,
    },
    Setting {
        key: "tab_ping",
        label: "Пинг игроков",
        label_en: "Player ping",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_friends",
        label: "Друзья первыми",
        label_en: "Friends first",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "tab_distance",
        label: "Расстояние до видимых друзей",
        label_en: "Distance to visible friends",
        group: 6,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "enabled",
        label: "Модуль экономики",
        label_en: "Economy module",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hud",
        label: "Экономическая панель на экране",
        label_en: "Economy overlay",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "overlay",
        label: "Подсказки на аукционе и рынке",
        label_en: "Auction and market hints",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hide_sidebar",
        label: "Скрывать боковую панель сервера",
        label_en: "Hide server sidebar",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "hud_balance",
        label: "Баланс на экономической панели",
        label_en: "Balance in economy overlay",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "hud_hand",
        label: "Цена предмета в руке на панели",
        label_en: "Held item price in economy overlay",
        group: 0,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "corner",
        label: "Положение экономической панели",
        label_en: "Economy overlay position",
        group: 1,
        min: 0,
        max: 3,
        step: 1,
        default: 2,
    },
    Setting {
        key: "width",
        label: "Ширина экономической панели",
        label_en: "Economy overlay width",
        group: 1,
        min: 140,
        max: 360,
        step: 20,
        default: 220,
    },
    Setting {
        key: "scale",
        label: "Масштаб экономической панели",
        label_en: "Economy overlay scale",
        group: 1,
        min: 65,
        max: 150,
        step: 5,
        default: 85,
    },
    Setting {
        key: "opacity",
        label: "Непрозрачность экономической панели",
        label_en: "Economy overlay opacity",
        group: 1,
        min: 0,
        max: 100,
        step: 10,
        default: 70,
    },
    Setting {
        key: "offset_x",
        label: "Сдвиг панели по горизонтали",
        label_en: "Overlay horizontal offset",
        group: 1,
        min: -300,
        max: 300,
        step: 10,
        default: 0,
    },
    Setting {
        key: "offset_y",
        label: "Сдвиг панели по вертикали",
        label_en: "Overlay vertical offset",
        group: 1,
        min: -300,
        max: 300,
        step: 10,
        default: 0,
    },
    Setting {
        key: "quiet",
        label: "Скрывать служебные ответы команд",
        label_en: "Hide routine command replies",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "auto_poll",
        label: "Обновлять наблюдаемые цены",
        label_en: "Refresh watched prices",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "held",
        label: "Проверять предмет в руке",
        label_en: "Check held item",
        group: 2,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "interval",
        label: "Интервал запросов",
        label_en: "Request interval",
        group: 2,
        min: 10,
        max: 300,
        step: 10,
        default: 30,
    },
    Setting {
        key: "quote_ttl",
        label: "Срок актуальности цен",
        label_en: "Price validity period",
        group: 2,
        min: 30,
        max: 600,
        step: 30,
        default: 120,
    },
    Setting {
        key: "ah_ttl",
        label: "Срок актуальности лотов аукциона",
        label_en: "Auction listing validity period",
        group: 2,
        min: 15,
        max: 180,
        step: 15,
        default: 60,
    },
    Setting {
        key: "scan_delay",
        label: "Пауза между страницами сканера",
        label_en: "Delay between scanned pages",
        group: 2,
        min: 500,
        max: 3000,
        step: 250,
        default: 1000,
    },
    Setting {
        key: "pages",
        label: "Предел страниц сканера",
        label_en: "Scanner page limit",
        group: 2,
        min: 1,
        max: 200,
        step: 5,
        default: 100,
    },
    Setting {
        key: "min_profit",
        label: "Минимальная прибыль",
        label_en: "Minimum profit",
        group: 3,
        min: 0,
        max: 10000,
        step: 5,
        default: 5,
    },
    Setting {
        key: "min_roi",
        label: "Минимальная доходность",
        label_en: "Minimum return on investment",
        group: 3,
        min: 0,
        max: 100,
        step: 1,
        default: 5,
    },
    Setting {
        key: "slippage",
        label: "Запас на изменение цены",
        label_en: "Price movement buffer",
        group: 3,
        min: 0,
        max: 30,
        step: 1,
        default: 3,
    },
    Setting {
        key: "cash_reserve",
        label: "Оставлять монет на балансе",
        label_en: "Keep coins in balance",
        group: 3,
        min: 0,
        max: 100000,
        step: 100,
        default: 0,
    },
    Setting {
        key: "strict_history",
        label: "Помечать неполную историю покупок",
        label_en: "Flag incomplete purchase history",
        group: 3,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "whole_fee",
        label: "Округлять комиссию до монеты",
        label_en: "Round fees to a whole coin",
        group: 4,
        min: 0,
        max: 1,
        step: 1,
        default: 1,
    },
    Setting {
        key: "override",
        label: "Ручная комиссия размещения AH",
        label_en: "Override AH listing fee",
        group: 4,
        min: 0,
        max: 1,
        step: 1,
        default: 0,
    },
    Setting {
        key: "listing_fee",
        label: "Комиссия размещения лота",
        label_en: "Listing fee rate",
        group: 4,
        min: 0,
        max: 50,
        step: 1,
        default: 1,
    },
    Setting {
        key: "listing_min",
        label: "Минимальная комиссия аукциона",
        label_en: "Minimum auction fee",
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
impl Listing {
    /// A click buys at once only when the question is switched off, the lot has a fixed price
    /// and the known balance covers it; otherwise the confirmation explains the purchase.
    pub fn buys_without_asking(&self, ask: bool, balance: Option<Coins>) -> bool {
        !ask && !self.bid && balance.is_none_or(|v| v >= self.cost)
    }
    /// The same lot with the same terms (what a purchase confirmation promised).
    pub fn same_offer(&self, o: &Self) -> bool {
        (
            self.id,
            &self.item,
            self.count,
            self.cost,
            &self.seller,
            self.bid,
        ) == (o.id, &o.item, o.count, o.cost, &o.seller, o.bid)
    }
    pub fn within_unit_price(&self, cap: Coins) -> bool {
        !self.bid
            && self.count > 0
            && cap > 0
            && i128::from(self.cost) <= i128::from(cap) * i128::from(self.count)
    }
}
const MAX_LISTINGS: usize = 5000;
const MAX_ROWS: usize = 20_000;
const STACK_TEXT_MAX: usize = 4096;
const STACK_BYTES_MAX: usize = 2 * 1024 * 1024;
const FILE_BUDGET: usize = 3_800_000;
/// How long after being seen a lot still counts as current enough to buy without asking.
const FRESH_MS: u64 = 120_000;
const MAX_LOT_LIFE: u64 = 400 * 86_400_000;
/// How far a lot of the catalogue can be trusted right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LotState {
    Gone,
    Expired,
    /// Restored from disk or seen before the current walk: it may be sold or changed.
    Saved,
    /// Seen by the current walk.
    Fresh,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyRefusal {
    Gone,
    Expired,
    Changed,
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
    sidebar_balance: Option<String>,
    pub coin_left: Option<(Coins, u64)>,
    pub coin_cap: Option<Coins>,
    pub item_cap: Option<u32>,
    pub watch: BTreeSet<String>,
    pub auction_watch: BTreeMap<String, Coins>,
    pub trades: Vec<Trade>,
    pub inventory: BTreeMap<String, u32>,
    pub day: u64,
    pub generation: u64,
    pub complete_scan: bool,
    pub observed_pages: u32,
    /// When the last complete walk of the whole auction finished (0: none saved).
    pub ah_complete_at: u64,
    /// Item stack text (SNBT) of lots, for exact titles and details after a restart.
    pub stacks: BTreeMap<u64, String>,
    pub movers: Vec<String>,
    pub movers_at: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    StalePrice,
    RecheckLot,
    ResaleLimit(u32),
    HistoryIncomplete,
    PartialSale(u32, u32),
    BalanceStale,
    BalanceUnknown,
    NotEnoughCoins,
}
impl Note {
    pub fn text(self, language: Language) -> String {
        match self {
            Self::StalePrice => language.text("Устарела цена", "Price is stale").into(),
            Self::RecheckLot => language
                .text("Лот нужно перепроверить", "Lot needs rechecking")
                .into(),
            Self::ResaleLimit(n) => match language {
                Language::Russian => format!("Ограничение обратной продажи: {n} шт."),
                Language::English => format!("Resale limit: {n} pcs"),
            },
            Self::HistoryIncomplete => language
                .text(
                    "История покупок market может быть неполной",
                    "Market purchase history may be incomplete",
                )
                .into(),
            Self::PartialSale(n, total) => match language {
                Language::Russian => format!("Продажа {n}/{total}; остаток не оценён"),
                Language::English => format!("Selling {n}/{total}; remainder not estimated"),
            },
            Self::BalanceStale => language
                .text("Баланс нужно обновить", "Balance needs refreshing")
                .into(),
            Self::BalanceUnknown => language.text("Баланс неизвестен", "Balance unknown").into(),
            Self::NotEnoughCoins => language
                .text("Недостаточно свободных монет", "Not enough free coins")
                .into(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Opportunity {
    pub listing: Listing,
    pub quantity: u32,
    pub gross: Coins,
    pub full_profit: Coins,
    pub profit: Coins,
    pub notes: Vec<Note>,
    pub ready: bool,
}
fn fresh(at: u64, now: u64, ttl: i64) -> bool {
    now >= at && now - at <= ttl as u64 * 1000
}
fn quote_line(text: &str) -> Option<(Option<Coins>, Option<Coins>)> {
    let text = text.trim().trim_end_matches('.');
    let text = text
        .strip_prefix("Price: ")
        .or_else(|| text.strip_prefix("Цена: "))
        .unwrap_or(text);
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
    if let Some(s) = text.strip_suffix(" each (the market doesn't buy it)") {
        return Some((amount(s), None));
    }
    if let Some(s) = text.strip_suffix(" each (the market doesn't sell it)") {
        return Some((None, amount(s)));
    }
    if let Some(s) = text.strip_suffix(" за шт., купить или продать") {
        let p = amount(s)?;
        return Some((Some(p), Some(p)));
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
fn valid_seller(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
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
    if !valid_seller(seller) {
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
    pub fn set_auction_watch(&mut self, item: &str, cap: Coins) -> bool {
        let Some(item) = id(item) else { return false };
        if cap == 0 {
            self.auction_watch.remove(&item);
        } else if (1..=MAX).contains(&cap)
            && (self.auction_watch.len() < 64 || self.auction_watch.contains_key(&item))
        {
            self.auction_watch.insert(item, cap);
        } else {
            return false;
        }
        true
    }
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
        if item == "minecraft:paper"
            && lines
                .iter()
                .any(|l| l.contains("Movers over 24") || l.contains("Динамика за 24"))
        {
            self.movers = lines
                .iter()
                .filter(|l| {
                    l.contains('▲')
                        || l.contains('▼')
                        || l.contains("No move")
                        || l.contains("Без изменений")
                })
                .take(10)
                .map(|l| l.chars().take(120).collect())
                .collect();
            self.movers_at = now;
            return;
        }
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
            if let Some(value) = line
                .strip_prefix("Traded in 24 h: ")
                .or_else(|| line.strip_prefix("Продано за 24 ч: "))
            {
                self.quotes.entry(item.into()).or_default().volume = number(value);
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
            notes.push(Note::StalePrice);
        }
        if !fresh(l.seen, now, p.get("ah_ttl")) {
            notes.push(Note::RecheckLot);
        }
        let mut n = l.count;
        let back = if q.blocked_until > now { q.blocked } else { 0 };
        let have = self.inventory.get(&l.item).copied().unwrap_or(0);
        n = n.min(have.saturating_add(l.count).saturating_sub(back));
        if back > 0 {
            notes.push(Note::ResaleLimit(back));
        }
        if p.flag("strict_history") {
            notes.push(Note::HistoryIncomplete);
        }
        if n < l.count {
            notes.push(Note::PartialSale(n, l.count));
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
            notes.push(Note::BalanceStale);
        }
        let budget = self
            .balance
            .map(|(v, _)| v.saturating_sub(p.get("cash_reserve") * 100));
        match budget {
            None => notes.push(Note::BalanceUnknown),
            Some(n) if n < l.cost => notes.push(Note::NotEnoughCoins),
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
        if !fresh(q.at, now, p.get("quote_ttl")) {
            return None;
        }
        let blocked = if q.blocked_until > now { q.blocked } else { 0 };
        let inventory = self.inventory.get(item).copied().unwrap_or(0);
        let quantity = count.min(inventory.min(count).saturating_sub(blocked));
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
            let label = label.trim().trim_end_matches(':').to_ascii_lowercase();
            if (label == "balance" || label == "balance:" || label == "баланс")
                && let Some(v) = sidebar_amount(value)
            {
                // An unchanged rounded sidebar must not overwrite an exact /bal reply.
                if self.sidebar_balance.as_deref() != Some(value.as_str()) {
                    self.balance = Some((v, now));
                    self.sidebar_balance = Some(value.clone());
                }
            }
        }
    }
    /// Saved auction lots ride in the same file as the prices: the schema stays 1 so older
    /// builds still read it, and a file without them loads as before.
    pub fn encode_at(&self, scope: &str, now: u64) -> String {
        let mut v = json!({"schema":1,"scope":scope,"day":self.day,"movers":self.movers,"movers_at":self.movers_at,"quotes":self.quotes.iter().map(|(id,q)|json!([id,q.buy,q.sell,q.at,q.history,q.blocked,q.blocked_until])).collect::<Vec<_>>(),"watch":self.watch,"auction_watch":self.auction_watch,"rules":[null,null,self.rules.fee_bp,self.rules.fee_min],"trades":self.trades.iter().map(|t|json!([t.at,t.item,t.count,t.gross,t.net,t.buying])).collect::<Vec<_>>()});
        // Newest first, so trimming to the persistence limit keeps the freshest lots.
        let mut lots: Vec<&Listing> = self.listings.values().filter(|l| l.expires > now).collect();
        lots.sort_by_key(|l| std::cmp::Reverse((l.seen, l.id)));
        v["ah_complete_at"] = json!(self.ah_complete_at);
        let mut with_stacks = true;
        loop {
            v["listings"] = lots
                .iter()
                .map(|l| {
                    json!([
                        l.id, l.item, l.label, l.count, l.cost, l.seller, l.bid, l.seen, l.expires
                    ])
                })
                .collect();
            v["stacks"] = if with_stacks {
                lots.iter()
                    .filter_map(|l| self.stacks.get(&l.id).map(|text| json!([l.id, text])))
                    .collect()
            } else {
                json!([])
            };
            let raw = v.to_string();
            // The file must stay inside the persistence limit, or saving would stop altogether:
            // shed the stacks first, then the oldest quarter of the lots, until it fits.
            if raw.len() <= FILE_BUDGET || lots.is_empty() {
                return raw;
            }
            if with_stacks {
                with_stacks = false;
            } else {
                lots.truncate(lots.len() * 3 / 4);
            }
        }
    }
    pub fn decode_at(raw: &str, scope: &str, now: u64) -> Result<Self, String> {
        if raw.len() > 4 * 1024 * 1024 {
            return Err("economy cache too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|_| "invalid economy cache")?;
        if v["schema"] != 1 || v["scope"].as_str() != Some(scope) {
            return Err("economy cache server/schema mismatch".into());
        }
        let mut book = Self {
            day: v["day"].as_u64().unwrap_or(0),
            movers: v["movers"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .take(10)
                        .filter_map(Value::as_str)
                        .map(|s| s.chars().take(120).collect())
                        .collect()
                })
                .unwrap_or_default(),
            movers_at: v["movers_at"].as_u64().unwrap_or(0),
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
                source: "saved".into(),
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
        if let Some(rows) = v["auction_watch"].as_object() {
            for (item, cap) in rows.iter().take(64) {
                if let Some(cap) = cap.as_i64().filter(|cap| *cap > 0) {
                    book.set_auction_watch(item, cap);
                }
            }
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
        // Saved lots are an unverified snapshot: generation 0 is older than the current one.
        book.generation = 1;
        book.ah_complete_at = v["ah_complete_at"].as_u64().unwrap_or(0).min(now);
        if let Some(rows) = v["listings"].as_array() {
            for row in rows.iter().take(MAX_ROWS) {
                if book.listings.len() >= MAX_LISTINGS {
                    break;
                }
                if let Some(l) = restore_listing(row, now) {
                    book.listings.insert(l.id, l);
                }
            }
        }
        if let Some(rows) = v["stacks"].as_array() {
            for row in rows.iter().take(MAX_ROWS) {
                if let (Some(lot), Some(text)) = (row[0].as_u64(), row[1].as_str()) {
                    book.remember_stack(lot, text.into());
                }
            }
        }
        Ok(book)
    }
    /// Starts a new walk of the auction: lots seen from now on belong to it.
    pub fn begin_walk(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.complete_scan = false;
        self.observed_pages = 0;
    }
    /// Merges one observed lot by its number. False when the cache is full of other lots.
    pub fn observe_lot(&mut self, l: Listing) -> bool {
        if self.listings.len() >= MAX_LISTINGS && !self.listings.contains_key(&l.id) {
            // Pruning needs proof of a complete walk, so a full cache makes room by giving up
            // the oldest saved lot the current walk has not seen, never one it has.
            let generation = self.generation;
            let oldest = self
                .listings
                .values()
                .filter(|o| o.generation != generation)
                .min_by_key(|o| (o.seen, o.id))
                .map(|o| o.id);
            match oldest {
                Some(old) => {
                    self.forget_lot(old);
                }
                None => return false,
            }
        }
        self.listings.insert(l.id, l);
        true
    }
    pub fn remember_stack(&mut self, lot: u64, text: String) -> bool {
        if !self.listings.contains_key(&lot) || text.len() > STACK_TEXT_MAX {
            return false;
        }
        let others: usize = self
            .stacks
            .iter()
            .filter(|(id, _)| **id != lot)
            .map(|(_, t)| t.len())
            .sum();
        if others + text.len() > STACK_BYTES_MAX {
            return false;
        }
        self.stacks.insert(lot, text);
        true
    }
    pub fn forget_lot(&mut self, lot: u64) -> bool {
        self.stacks.remove(&lot);
        self.listings.remove(&lot).is_some()
    }
    pub fn drop_expired(&mut self, now: u64) {
        self.listings.retain(|_, l| l.expires > now);
        let listings = &self.listings;
        self.stacks.retain(|id, _| listings.contains_key(id));
    }
    /// Ends a walk. Only one that reached the end of the whole auction may drop the lots it did
    /// not see; a limit, timeout, disconnect or close leaves the saved lots as they were.
    pub fn finish_scan(&mut self, complete: bool, now: u64) {
        if !complete {
            return;
        }
        let generation = self.generation;
        self.listings.retain(|_, l| l.generation == generation);
        let listings = &self.listings;
        self.stacks.retain(|id, _| listings.contains_key(id));
        self.complete_scan = true;
        self.ah_complete_at = now;
    }
    pub fn lot_state(&self, lot: u64, now: u64) -> LotState {
        match self.listings.get(&lot) {
            None => LotState::Gone,
            Some(l) if l.expires <= now => LotState::Expired,
            Some(l) if l.generation == self.generation => LotState::Fresh,
            Some(_) => LotState::Saved,
        }
    }
    /// The lot the player saw must still be there, unexpired and unchanged.
    pub fn buy_ready(&self, shown: &Listing, now: u64) -> Result<(), BuyRefusal> {
        match self.listings.get(&shown.id) {
            None => Err(BuyRefusal::Gone),
            Some(l) if l.expires <= now => Err(BuyRefusal::Expired),
            Some(l) if !l.same_offer(shown) => Err(BuyRefusal::Changed),
            Some(_) => Ok(()),
        }
    }
    /// A click may skip the question only for a lot the current walk has seen.
    pub fn direct_buy_allowed(
        &self,
        shown: &Listing,
        ask: bool,
        balance: Option<Coins>,
        now: u64,
    ) -> bool {
        self.buy_ready(shown, now).is_ok()
            && self.lot_state(shown.id, now) == LotState::Fresh
            && self
                .listings
                .get(&shown.id)
                .is_some_and(|l| now.saturating_sub(l.seen) <= FRESH_MS)
            && shown.buys_without_asking(ask, balance)
    }
}
/// Keeps the order the player is looking at: lots already listed keep their places, lots that
/// left are dropped and new ones go last, so an update never moves a lot under the pointer.
pub fn stable_order(previous: &[u64], sorted: &[u64]) -> Vec<u64> {
    let wanted: BTreeSet<u64> = sorted.iter().copied().collect();
    let mut seen = BTreeSet::new();
    let mut out: Vec<u64> = previous
        .iter()
        .copied()
        .filter(|id| wanted.contains(id) && seen.insert(*id))
        .collect();
    out.extend(sorted.iter().copied().filter(|id| seen.insert(*id)));
    out
}
fn restore_listing(row: &Value, now: u64) -> Option<Listing> {
    let row = row.as_array().filter(|r| r.len() == 9)?;
    let label = row[2].as_str()?;
    let seller = row[5].as_str().filter(|s| valid_seller(s))?;
    let expires = row[8]
        .as_u64()
        .filter(|e| *e > now && *e <= now + MAX_LOT_LIFE)?;
    (label.chars().count() <= 80 && !label.chars().any(char::is_control)).then_some(())?;
    Some(Listing {
        id: row[0].as_u64()?,
        item: id(row[1].as_str()?)?,
        label: label.into(),
        count: u32::try_from(row[3].as_u64()?)
            .ok()
            .filter(|n| (1..=1_000_000).contains(n))?,
        cost: row[4].as_i64().filter(|n| (0..=MAX).contains(n))?,
        seller: seller.into(),
        bid: row[6].as_bool()?,
        seen: row[7].as_u64()?.min(now),
        expires,
        generation: 0,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn auction_purchases_ask_first_unless_switched_off() {
        use super::*;
        assert!(Preferences::default().flag("confirm_buy"));
        let lot = |bid| Listing {
            id: 8716,
            item: "minecraft:black_banner".into(),
            label: "Black Banner".into(),
            count: 16,
            cost: 4_000,
            seller: "Alex".into(),
            bid,
            seen: 0,
            expires: 1,
            generation: 0,
        };
        assert!(!lot(false).buys_without_asking(true, Some(10_000)));
        assert!(lot(false).buys_without_asking(false, Some(10_000)));
        assert!(lot(false).buys_without_asking(false, None));
        assert!(!lot(false).buys_without_asking(false, Some(3_999)));
        assert!(!lot(true).buys_without_asking(false, Some(10_000)));
    }
    #[test]
    fn setting_values_have_localized_states_units_and_corner_names() {
        let setting = |key| SETTINGS.iter().find(|setting| setting.key == key).unwrap();
        for s in SETTINGS {
            assert_eq!(
                s.is_toggle(),
                s.max == 1,
                "boolean metadata changed for {}",
                s.key
            );
            assert!(!s.label.is_empty() && !s.label_en.is_empty());
        }
        assert_eq!(
            setting("custom_tab").value_label(0, Language::Russian),
            "выкл."
        );
        assert_eq!(
            setting("custom_tab").value_label(1, Language::English),
            "on"
        );
        assert_eq!(
            setting("tab_opacity").value_label(42, Language::English),
            "42%"
        );
        assert_eq!(
            setting("scan_delay").value_label(1000, Language::Russian),
            "1000 мс"
        );
        assert_eq!(
            setting("interval").value_label(30, Language::English),
            "30 s"
        );
        assert_eq!(
            setting("width").value_label(220, Language::Russian),
            "220 пкс"
        );
        assert_eq!(
            setting("min_profit").value_label(5, Language::English),
            "5 coins"
        );
        assert_eq!(
            setting("tab_columns").value_label(3, Language::English),
            "3"
        );
        for (value, ru, en) in [
            (0, "Сверху слева", "Top left"),
            (1, "Сверху справа", "Top right"),
            (2, "Снизу слева", "Bottom left"),
            (3, "Снизу справа", "Bottom right"),
        ] {
            for key in ["corner", "sidebar_corner"] {
                assert_eq!(setting(key).value_label(value, Language::Russian), ru);
                assert_eq!(setting(key).value_label(value, Language::English), en);
            }
        }
    }
    use super::*;
    #[test]
    fn english_notes_and_settings_have_no_russian_text() {
        let cyr = |t: &str| t.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c));
        for note in [
            Note::StalePrice,
            Note::RecheckLot,
            Note::ResaleLimit(3),
            Note::HistoryIncomplete,
            Note::PartialSale(1, 2),
            Note::BalanceStale,
            Note::BalanceUnknown,
            Note::NotEnoughCoins,
        ] {
            assert!(!cyr(&note.text(Language::English)));
        }
        assert!(SETTINGS.iter().all(|s| !cyr(s.label_en)));
    }
    #[test]
    fn auction_favorites_persist_and_compare_exact_unit_prices() {
        let mut book = Book::default();
        assert!(book.set_auction_watch("minecraft:diamond", 250));
        let mut listing =
            listing("#42 4 Diamond · 10 coins · Alex · 1h left", "diamond", 0, 0).unwrap();
        assert!(listing.within_unit_price(250));
        assert!(!listing.within_unit_price(249));
        listing.bid = true;
        assert!(!listing.within_unit_price(250));
        let mut restored = Book::decode_at(&book.encode_at("server-a", 0), "server-a", 0).unwrap();
        assert_eq!(restored.auction_watch["minecraft:diamond"], 250);
        assert!(Book::decode_at(&book.encode_at("server-a", 0), "server-b", 0).is_err());
        assert!(restored.set_auction_watch("diamond", 0));
        assert!(restored.auction_watch.is_empty());
    }
    #[test]
    fn standard_auction_view_survives_reload_and_keeps_market_independent() {
        let mut p = Preferences::decode(r#"{"schema":1,"values":{}}"#).unwrap();
        assert!(p.flag("custom_auction") && p.flag("custom_market"));
        p.adjust("custom_auction", 1);
        let mut reloaded = Preferences::decode(&p.encode()).unwrap();
        assert!(!reloaded.flag("custom_auction"));
        assert!(reloaded.flag("custom_market"));
        reloaded.adjust("custom_auction", 1);
        assert!(
            Preferences::decode(&reloaded.encode())
                .unwrap()
                .flag("custom_auction")
        );
    }

    #[test]
    fn actual_market_lore_prices_limits_volume_and_movers() {
        let mut b = Book::default();
        b.market_slot(
            "minecraft:leather",
            &[
                "Price: 3.01 each, to buy or sell".into(),
                "You may still sell 7 today".into(),
                "24 h ▲ 4.1%".into(),
                "Traded in 24 h: 123".into(),
            ],
            100,
        );
        let q = &b.quotes["minecraft:leather"];
        assert_eq!(q.buy, Some(301));
        assert_eq!(q.sell, Some(301));
        assert_eq!(q.left, Some((7, 100)));
        assert_eq!(q.volume, Some(123));
        b.market_slot(
            "minecraft:gravel",
            &["Price: 2 each (the market doesn't buy it)".into()],
            100,
        );
        assert_eq!(b.quotes["minecraft:gravel"].sell, None);
        b.market_slot(
            "minecraft:paper",
            &["Movers over 24 hours".into(), "Pumpkin ▲ 11.8%".into()],
            100,
        );
        assert_eq!(b.movers, ["Pumpkin ▲ 11.8%"]);
        assert!(!b.quotes.contains_key("minecraft:paper"));
    }
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
    fn removed_daily_caps_do_not_reduce_lots_or_block_hand_sales() {
        let (mut book, p, l) = sample();
        book.coin_left = Some((0, NOW));
        book.quotes.get_mut(&l.item).unwrap().left = Some((0, NOW));
        let o = book.opportunity(&l, &p, NOW, "me").unwrap();
        assert_eq!((o.quantity, o.profit), (22, 1800));
        assert!(o.ready);
        book.coin_left = None;
        book.quotes.get_mut(&l.item).unwrap().left = None;
        assert!(book.opportunity(&l, &p, NOW, "me").unwrap().ready);
        book.inventory.insert(l.item.clone(), 22);
        assert_eq!(book.hand_sale(&l.item, 22, &p, NOW), Some((22, 8800)));
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
        assert!(book.opportunity(&l, &p, NOW, "me").unwrap().ready);
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
    fn hand_sale_respects_recent_market_purchases_and_fresh_prices() {
        let (mut book, p, l) = sample();
        book.inventory.insert(l.item.clone(), 30);
        let q = book.quotes.get_mut(&l.item).unwrap();
        q.blocked = 20;
        q.blocked_until = NOW + 10000;
        assert_eq!(book.hand_sale(&l.item, 22, &p, NOW), Some((2, 800)));
        book.coin_left = Some((0, NOW));
        assert_eq!(book.hand_sale(&l.item, 22, &p, NOW), Some((2, 800)));
        assert!(book.hand_sale(&l.item, 22, &p, NOW + 3_600_000).is_none());
    }
    #[test]
    fn cached_prices_do_not_restore_wallet_limits_and_saved_lots_are_not_trade_signals() {
        let (mut book, p, l) = sample();
        book.listings.insert(l.id, l.clone());
        book.watch.insert(l.item.clone());
        let raw = book.encode_at("server/account", NOW);
        let restored = Book::decode_at(&raw, "server/account", NOW).unwrap();
        assert_eq!(restored.quotes[&l.item].sell, Some(400));
        assert!(restored.balance.is_none());
        assert!(restored.coin_left.is_none());
        assert!(restored.quotes[&l.item].left.is_none());
        // The lot comes back as saved data: shown, but never an opportunity before a new walk.
        assert_eq!(restored.lot_state(l.id, NOW), LotState::Saved);
        assert!(restored.opportunity(&l, &p, NOW, "me").is_none());
        assert!(Book::decode_at(&raw, "another/account", NOW).is_err());
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
    fn obsolete_daily_allowance_never_blocks_profit_estimates() {
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
        assert_eq!(o.quantity, 64);
        assert!(o.profit > 0);
        assert!(!o.ready);
    }
    #[test]
    fn obsolete_tax_preferences_and_cache_have_no_effect() {
        let p = Preferences::decode(
            r#"{"schema":1,"values":{"taxes_enabled":1,"market_tax":50,"vat":50}}"#,
        )
        .unwrap();
        assert!(!p.encode().contains("tax"));
        let b=Book::decode_at(r#"{"schema":1,"scope":"s","quotes":[],"rules":[200,500,100,100],"tax_lines":["old bill"]}"#,"s",0).unwrap();
        assert_eq!(b.resale(7000, 10000, &p), Some((100, 2900)));
        assert!(!b.encode_at("s", 0).contains("tax"));
    }
}

pub fn delta(n: Coins) -> String {
    if n > 0 {
        format!("+{}", fmt(n))
    } else {
        fmt(n)
    }
}

#[cfg(test)]
mod scoreboard_regressions {
    use super::*;
    #[test]
    fn reads_abbreviated_and_colon_balance_and_server_limits() {
        let mut book = Book::default();
        book.sidebar(&[("Balance:".into(), "125.4k".into())], 10);
        assert_eq!(book.balance, Some((12_540_000, 10)));
        book.server("Balance: 125,432 coins", 11);
        book.sidebar(&[("Balance:".into(), "125.4k".into())], 12);
        assert_eq!(book.balance, Some((12_543_200, 11)));
        assert!(book.server("Selling today: 12,500 coins left of your 25,000 coins; at most 1,000 of each item a day.", 11));
        assert_eq!(book.coin_left, Some((1_250_000, 11)));
        assert_eq!(book.coin_cap, Some(2_500_000));
        assert_eq!(sidebar_amount("invalid"), None);
    }
}
#[cfg(test)]
mod cache_tests {
    use super::*;
    const NOW: u64 = 1_000_000;
    const SCOPE: &str = "play.example.org#00000000000000000000000000000001";
    fn lot(id: u64, cost: Coins, expires_in: u64) -> Listing {
        Listing {
            id,
            item: "minecraft:diamond".into(),
            label: format!("Diamond {id}"),
            count: 4,
            cost,
            seller: "Alex_1".into(),
            bid: false,
            seen: NOW,
            expires: NOW + expires_in,
            generation: 0,
        }
    }
    fn book_with(ids: &[u64]) -> Book {
        let mut book = Book::default();
        for id in ids {
            assert!(book.observe_lot(lot(*id, 1000 + *id as Coins, 3_600_000)));
        }
        book
    }
    fn restored(book: &Book, now: u64) -> Book {
        Book::decode_at(&book.encode_at(SCOPE, now), SCOPE, now).unwrap()
    }
    #[test]
    fn listings_and_stack_text_survive_a_save_and_reload() {
        let mut book = book_with(&[1, 2, 3]);
        assert!(book.remember_stack(2, r#"{id:"minecraft:diamond",count:4}"#.into()));
        book.ah_complete_at = NOW - 500;
        let back = restored(&book, NOW + 10);
        assert_eq!(back.listings.len(), 3);
        let l = &back.listings[&2];
        assert_eq!((l.cost, l.count, l.seller.as_str()), (1002, 4, "Alex_1"));
        assert_eq!((l.seen, l.expires), (NOW, NOW + 3_600_000));
        assert_eq!(
            back.stacks[&2],
            r#"{id:"minecraft:diamond",count:4}"#.to_string()
        );
        assert_eq!(back.ah_complete_at, NOW - 500);
        // Everything restored is saved data: none of it counts as seen by the current scan.
        assert!(
            back.listings
                .values()
                .all(|l| l.generation != back.generation)
        );
        assert_eq!(back.lot_state(2, NOW + 10), LotState::Saved);
    }
    #[test]
    fn old_cache_without_listings_still_loads_and_other_data_is_kept() {
        let mut book = book_with(&[1]);
        book.watch.insert("minecraft:diamond".into());
        let raw = book.encode_at(SCOPE, NOW);
        let mut v: Value = serde_json::from_str(&raw).unwrap();
        v.as_object_mut().unwrap().remove("listings");
        v.as_object_mut().unwrap().remove("stacks");
        let old = Book::decode_at(&v.to_string(), SCOPE, NOW).unwrap();
        assert!(old.listings.is_empty());
        assert!(old.watch.contains("minecraft:diamond"));
        assert_eq!(v["schema"], 1, "the schema stays readable by older builds");
    }
    #[test]
    fn corrupt_or_hostile_rows_are_dropped_without_losing_the_rest() {
        let book = book_with(&[1, 2]);
        let mut v: Value = serde_json::from_str(&book.encode_at(SCOPE, NOW)).unwrap();
        let rows = v["listings"].as_array_mut().unwrap();
        rows.push(json!([
            9,
            "not an item!",
            "x",
            1,
            1,
            "Alex",
            false,
            NOW,
            NOW + 1000
        ]));
        rows.push(json!([
            10,
            "minecraft:dirt",
            "x",
            0,
            5,
            "Alex",
            false,
            NOW,
            NOW + 1000
        ]));
        rows.push(json!([
            11,
            "minecraft:dirt",
            "x",
            1,
            -5,
            "Alex",
            false,
            NOW,
            NOW + 1000
        ]));
        rows.push(json!([
            12,
            "minecraft:dirt",
            "x",
            1,
            5,
            "bad seller!",
            false,
            NOW,
            NOW + 1000
        ]));
        rows.push(json!([
            13,
            "minecraft:dirt",
            "x",
            1,
            5,
            "Alex",
            false,
            NOW,
            u64::MAX
        ]));
        rows.push(json!("garbage"));
        rows.push(json!([
            14,
            "minecraft:dirt",
            "x",
            1,
            5,
            "Alex",
            false,
            NOW,
            NOW - 1
        ]));
        v["stacks"] = json!([
            [1, "x".repeat(100_000)],
            [99, "{}"],
            [2, "{id:\"ok\"}"],
            "junk"
        ]);
        let back = Book::decode_at(&v.to_string(), SCOPE, NOW).unwrap();
        assert_eq!(
            back.listings.keys().copied().collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(back.stacks.len(), 1);
        assert!(back.stacks.contains_key(&2));
        let mut broken: Value = serde_json::from_str(&book.encode_at(SCOPE, NOW)).unwrap();
        broken["listings"] = json!("nope");
        assert!(
            Book::decode_at(&broken.to_string(), SCOPE, NOW)
                .unwrap()
                .listings
                .is_empty()
        );
        assert!(Book::decode_at("{not json", SCOPE, NOW).is_err());
    }
    #[test]
    fn restored_cache_is_bounded() {
        let mut v: Value = serde_json::from_str(&Book::default().encode_at(SCOPE, NOW)).unwrap();
        v["listings"] = (0..6000u64)
            .map(|id| {
                json!([
                    id,
                    "minecraft:dirt",
                    "Dirt",
                    1,
                    5,
                    "Alex",
                    false,
                    NOW,
                    NOW + 60_000
                ])
            })
            .collect();
        let back = Book::decode_at(&v.to_string(), SCOPE, NOW).unwrap();
        assert_eq!(back.listings.len(), 5000);
    }
    #[test]
    fn a_cache_belongs_to_one_server_and_account() {
        let raw = book_with(&[1]).encode_at(SCOPE, NOW);
        assert!(
            Book::decode_at(
                &raw,
                "play.example.org#00000000000000000000000000000002",
                NOW
            )
            .is_err()
        );
        assert!(
            Book::decode_at(
                &raw,
                "other.example.org#00000000000000000000000000000001",
                NOW
            )
            .is_err()
        );
        assert!(Book::decode_at(&raw, SCOPE, NOW).is_ok());
    }
    #[test]
    fn expired_lots_are_neither_restored_listed_nor_buyable() {
        let mut book = book_with(&[1]);
        book.observe_lot(lot(2, 5000, 1000));
        let later = NOW + 2000;
        assert_eq!(book.lot_state(2, later), LotState::Expired);
        assert_eq!(
            book.buy_ready(&lot(2, 5000, 1000), later),
            Err(BuyRefusal::Expired)
        );
        assert!(!restored(&book, later).listings.contains_key(&2));
        book.remember_stack(2, "{}".into());
        book.drop_expired(later);
        assert!(!book.listings.contains_key(&2) && !book.stacks.contains_key(&2));
        assert!(book.listings.contains_key(&1));
    }
    #[test]
    fn a_partial_scan_never_prunes_unseen_cached_lots() {
        let saved = book_with(&[1, 2, 3]);
        let mut book = restored(&saved, NOW);
        book.begin_walk();
        assert!(book.observe_lot(Listing {
            generation: book.generation,
            ..lot(2, 1002, 3_600_000)
        }));
        // Page limit, timeout, disconnect or close: the scan did not finish.
        book.finish_scan(false, NOW + 5);
        assert_eq!(book.listings.len(), 3);
        assert_eq!(book.ah_complete_at, 0);
        assert!(!book.complete_scan);
        // What a closed session saves still holds every lot.
        assert_eq!(restored(&book, NOW + 5).listings.len(), 3);
    }
    #[test]
    fn only_a_complete_scan_evicts_lots_missing_from_it() {
        let mut book = restored(&book_with(&[1, 2, 3]), NOW);
        book.remember_stack(3, "{}".into());
        book.begin_walk();
        for id in [1, 2] {
            assert!(book.observe_lot(Listing {
                generation: book.generation,
                ..lot(id, 1000, 3_600_000)
            }));
        }
        book.finish_scan(true, NOW + 9);
        assert_eq!(
            book.listings.keys().copied().collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(!book.stacks.contains_key(&3));
        assert!(book.complete_scan);
        assert_eq!(book.ah_complete_at, NOW + 9);
        assert_eq!(book.lot_state(1, NOW + 9), LotState::Fresh);
    }
    #[test]
    fn observing_merges_by_lot_id_and_keeps_known_stack_data() {
        let mut book = book_with(&[7]);
        book.remember_stack(7, "{id:\"minecraft:diamond\"}".into());
        book.begin_walk();
        assert!(book.observe_lot(Listing {
            generation: book.generation,
            cost: 900,
            ..lot(7, 0, 3_600_000)
        }));
        assert_eq!(book.listings.len(), 1);
        assert_eq!(book.listings[&7].cost, 900);
        assert_eq!(book.lot_state(7, NOW), LotState::Fresh);
        assert!(book.stacks.contains_key(&7));
        assert!(
            !book.remember_stack(8, "{}".into()),
            "a stack needs its lot"
        );
        assert!(
            !book.remember_stack(7, "x".repeat(5000)),
            "oversized stack text is refused"
        );
    }
    #[test]
    fn saved_data_stays_inside_the_cache_file_limit() {
        let mut book = Book::default();
        for id in 0..5000u64 {
            book.observe_lot(lot(id, 1000, 3_600_000));
            book.remember_stack(id, "a".repeat(4000));
        }
        let raw = book.encode_at(SCOPE, NOW);
        assert!(raw.len() < 3_800_000, "encoded {} bytes", raw.len());
        let back = Book::decode_at(&raw, SCOPE, NOW).unwrap();
        assert_eq!(
            back.listings.len(),
            5000,
            "lots are kept even when stack text is trimmed"
        );
    }
    #[test]
    fn a_purchase_needs_the_same_offer_the_player_saw() {
        let mut book = book_with(&[5]);
        let shown = book.listings[&5].clone();
        assert_eq!(book.buy_ready(&shown, NOW), Ok(()));
        book.observe_lot(Listing {
            cost: shown.cost + 1,
            ..shown.clone()
        });
        assert_eq!(book.buy_ready(&shown, NOW), Err(BuyRefusal::Changed));
        let mut other_seller = shown.clone();
        other_seller.seller = "Mallory".into();
        book.observe_lot(other_seller);
        assert_eq!(book.buy_ready(&shown, NOW), Err(BuyRefusal::Changed));
        book.forget_lot(5);
        assert_eq!(book.buy_ready(&shown, NOW), Err(BuyRefusal::Gone));
        assert_eq!(book.lot_state(5, NOW), LotState::Gone);
    }
    #[test]
    fn saved_lots_are_never_bought_without_the_question() {
        let mut book = restored(&book_with(&[5]), NOW);
        let saved = book.listings[&5].clone();
        assert_eq!(book.lot_state(5, NOW), LotState::Saved);
        assert!(!book.direct_buy_allowed(&saved, false, None, NOW));
        book.begin_walk();
        book.observe_lot(Listing {
            generation: book.generation,
            ..saved.clone()
        });
        let seen = book.listings[&5].clone();
        assert!(book.direct_buy_allowed(&seen, false, None, NOW));
        assert!(!book.direct_buy_allowed(&seen, true, None, NOW));
        assert!(!book.direct_buy_allowed(&seen, false, Some(1), NOW));
        assert!(!book.direct_buy_allowed(&seen, false, None, seen.expires));
    }
    #[test]
    fn browsing_order_stays_put_while_new_lots_arrive() {
        // Sorted by price the cheap newcomer would jump to the front.
        assert_eq!(
            stable_order(&[10, 20, 30], &[5, 10, 20, 30]),
            vec![10, 20, 30, 5]
        );
        // Gone lots leave; the rest keep their places.
        assert_eq!(stable_order(&[10, 20, 30], &[30, 10]), vec![10, 30]);
        // A fresh session (changed sort or search) takes the sorted order.
        assert_eq!(stable_order(&[], &[3, 1, 2]), vec![3, 1, 2]);
        assert_eq!(stable_order(&[1, 1, 2], &[1, 2]), vec![1, 2]);
    }
    #[test]
    fn a_full_cache_makes_room_by_dropping_the_oldest_unseen_saved_lot() {
        let mut book = Book::default();
        for id in 0..5000u64 {
            let mut l = lot(id, 100, 3_600_000);
            l.seen = NOW + id;
            book.observe_lot(l);
        }
        let mut book = restored(&book, NOW);
        book.begin_walk();
        let g = book.generation;
        assert!(book.observe_lot(Listing {
            generation: g,
            ..lot(9000, 5, 3_600_000)
        }));
        assert_eq!(book.listings.len(), 5000);
        assert!(
            !book.listings.contains_key(&0),
            "the oldest saved lot made room"
        );
        assert!(book.listings.contains_key(&1));
        // Lots the walk has seen are never evicted by later ones.
        for id in 0..5000u64 {
            book.listings.entry(id).and_modify(|l| l.generation = g);
        }
        assert!(!book.observe_lot(Listing {
            generation: g,
            ..lot(9001, 5, 3_600_000)
        }));
    }
}
