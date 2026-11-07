// Compact chat model, independent of game rendering and networking.
use super::claim_borders_chat_preferences::{Preferences, RepeatMode};
use serde_json::Value;
use std::collections::VecDeque;

pub const MAX_ENTRIES: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Message,
    Whisper(Whisper),
    Presence,
    Death,
    System,
}
impl Kind {
    pub fn is_chat(self) -> bool {
        matches!(self, Self::Message | Self::Whisper(_))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whisper {
    Incoming,
    Outgoing,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    #[default]
    All,
    Whisper,
    Party,
    Trade,
}
impl Channel {
    pub const TABS: [Self; 4] = [Self::All, Self::Whisper, Self::Party, Self::Trade];
    pub fn shows(self, entry: &Entry) -> bool {
        self == Self::All || entry.channel == self
    }
    /// Friends is a view of existing chat, not a replacement for DM/trade channels.
    pub fn shows_with_friends(self, entry: &Entry, important: bool) -> bool {
        self.shows(entry) || self == Self::Party && important && entry.kind.is_chat()
    }
}
#[derive(Default)]
pub struct Tabs {
    pub selected: Channel,
    unread: [u16; 4],
}
impl Tabs {
    pub fn record(&mut self, channel: Channel) {
        if channel != Channel::All {
            self.unread[channel as usize] = self.unread[channel as usize].saturating_add(1);
        }
    }
    pub fn read(&mut self) {
        if self.selected == Channel::All {
            self.unread.fill(0);
        } else {
            self.unread[self.selected as usize] = 0;
        }
    }
    pub fn unread(&self, channel: Channel) -> u16 {
        self.unread[channel as usize]
    }
}
impl Whisper {
    pub fn label(self) -> &'static str {
        self.label_in(Language::Russian)
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Incoming => language.text("ЛС от", "DM from"),
            Self::Outgoing => language.text("ЛС для", "DM to"),
        }
    }
}
/// Only recognized server-origin auction receipts are promoted to the important tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuctionNotice {
    Listed,
    Sold,
}
impl AuctionNotice {
    pub fn label(self) -> &'static str {
        self.label_in(Language::Russian)
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Listed => language.text("Аукцион · лот выставлен", "Auction · lot listed"),
            Self::Sold => language.text("Аукцион · ваш лот продан", "Auction · your lot sold"),
        }
    }
    pub fn color(self) -> u32 {
        match self {
            Self::Listed => 0xf1ca75,
            Self::Sold => 0x77e3ac,
        }
    }
}
fn auction_notice(line: &str) -> Option<AuctionNotice> {
    fn listing_id(s: &str) -> bool {
        !s.is_empty() && s.len() <= 20 && s.bytes().all(|c| c.is_ascii_digit())
    }
    fn goods(s: &str) -> bool {
        s.split_once(' ').is_some_and(|(n, item)| {
            n.parse::<u32>().is_ok_and(|n| n > 0) && !item.trim().is_empty()
        })
    }
    fn coins(s: &str) -> bool {
        s.strip_suffix(" coins")
            .and_then(super::claim_borders_economy::money)
            .is_some()
    }
    if line.len() > 2048 {
        return None;
    }
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("Listed #") {
        let (id, rest) = rest.split_once(": ")?;
        let (item, rest) = rest.rsplit_once(" for ")?;
        let (price, rest) = rest.split_once(" (")?;
        let (each, hours) = rest.split_once(" each), ")?;
        return (listing_id(id)
            && goods(item)
            && coins(price)
            && super::claim_borders_economy::money(each).is_some()
            && hours.strip_suffix("h.").is_some_and(listing_id))
        .then_some(AuctionNotice::Listed);
    }
    if let Some(rest) = line.strip_prefix("Auction #") {
        if let Some((id, rest)) = rest.split_once(": ") {
            let (item, rest) = rest.rsplit_once(" from ")?;
            let (price, hours) = rest.split_once(", ends in ")?;
            return (listing_id(id)
                && goods(item)
                && coins(price)
                && hours.strip_suffix("h.").is_some_and(listing_id))
            .then_some(AuctionNotice::Listed);
        }
        let (id, rest) = rest.split_once(" (")?;
        let (item, rest) = rest.rsplit_once(") sold to ")?;
        let (buyer, price) = rest.split_once(" for ")?;
        return (listing_id(id)
            && goods(item)
            && player_name(buyer)
            && price.strip_suffix('.').is_some_and(coins))
        .then_some(AuctionNotice::Sold);
    }
    let (buyer, rest) = line.split_once(" bought your ")?;
    let (item, price) = rest.rsplit_once(" for ")?;
    (player_name(buyer) && goods(item) && price.strip_suffix('.').is_some_and(coins))
        .then_some(AuctionNotice::Sold)
}
fn party_channel(line: &str) -> Channel {
    if line.starts_with("[Party] ") || line.starts_with("[Пати] ") {
        Channel::Party
    } else {
        Channel::All
    }
}
fn trade_notice(line: &str) -> bool {
    line.starts_with("Trade request sent to ")
        || line.starts_with("Trade cancelled: ")
        || line.starts_with("Trade complete")
        || line.contains(" wants to trade with you: /trade accept ")
        || line.ends_with(" refused your trade.")
        || line == "Your trade request is withdrawn."
}
use claim_borders_relay_client::translation::{
    Language, TargetLanguage, detect_source_language, source_language,
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    /// Restored entries are visible only while reading history.
    pub archived: bool,
    pub auction: Option<AuctionNotice>,
    pub kind: Kind,
    pub channel: Channel,
    pub name: String,
    pub original: String,
    pub source_language: Option<String>,
    pub translated: Option<String>,
    pub translation_language: TargetLanguage,
    pub at: u64,
    pub created_at: u64,
    pub joined: Option<bool>,
    pub repetitions: u32,
    pub show_original: bool,
    comparison: (Kind, String, String),
    raw: String,
}
impl Entry {
    pub fn display_body(&self, translated: bool, _language: Language) -> &str {
        if self.kind == Kind::Presence {
            match self.joined {
                Some(false) => "−",
                Some(true) => "+",
                None => &self.original,
            }
        } else {
            self.body(translated)
        }
    }
    pub fn has_translation(&self, language: impl Into<TargetLanguage>) -> bool {
        self.translated.is_some() && self.translation_language == language.into()
    }
    pub fn body(&self, translated: bool) -> &str {
        if translated && !self.show_original {
            self.translated.as_deref().unwrap_or(&self.original)
        } else {
            &self.original
        }
    }
}
/// Keep presence labels independent of translation results and preserve the original for hover/history.
pub fn presence_summary(
    entry: &Entry,
    style: super::claim_borders_chat_preferences::PresenceStyle,
    language: Language,
) -> String {
    use super::claim_borders_chat_preferences::PresenceStyle;
    if style == PresenceStyle::Icons && entry.joined.is_some() {
        return entry.name.clone();
    }
    let body = match entry.joined {
        Some(false) => language.text("Отключился", "Left the server"),
        Some(true) if entry.original == "Впервые на сервере" => {
            language.text("Впервые на сервере", "First time on the server")
        }
        Some(true) => language.text("Подключился", "Joined the server"),
        None => &entry.original,
    };
    format!("{} · {body}", entry.name)
}
/// A compact event keeps its full original in Entry for the hover card.
pub fn event_summary(entry: &Entry, translated: bool, language: Language) -> String {
    let body = entry.display_body(translated, language);
    if let Some(notice) = entry.auction {
        return format!("{} · {body}", notice.label_in(language));
    }
    if entry.kind != Kind::Death {
        return format!("{} · {}", entry.name, body);
    }
    let lower = body.to_lowercase();
    for prefix in [
        "was slain by ",
        "was killed by ",
        "was shot by ",
        "был убит ",
        "была убита ",
        "был застрелен ",
        "была застрелена ",
    ] {
        if lower.starts_with(prefix) {
            let who = body
                .get(prefix.len()..)
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !who.is_empty() {
                return format!("{} ← {}", entry.name, who);
            }
        }
    }
    let reason = if lower.contains("drown") || lower.contains("утону") {
        language.text("Утонул", "Drowned")
    } else if lower.contains("lava") || lower.contains("лав") {
        language.text("Лава", "Lava")
    } else if lower.contains("fell")
        || lower.contains("ground")
        || lower.contains("упал")
        || lower.contains("разбил")
    {
        language.text("Падение", "Fall")
    } else if lower.contains("burn")
        || lower.contains("flame")
        || lower.contains("fire")
        || lower.contains("сгор")
    {
        language.text("Огонь", "Fire")
    } else if lower.contains("explod")
        || lower.contains("blown")
        || lower.contains("blew up")
        || lower.contains("взорва")
    {
        language.text("Взрыв", "Explosion")
    } else {
        language.text("Погиб", "Died")
    };
    format!("{} · {reason}", entry.name)
}
pub fn single_line(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> String {
    let plain = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if measure(&plain) <= width {
        return plain;
    }
    let mut shown = String::new();
    for c in plain.chars() {
        let next = format!("{shown}{c}…");
        if measure(&next) > width {
            break;
        }
        shown.push(c);
    }
    if measure("…") <= width {
        shown.push('…');
    }
    shown
}
#[derive(Default)]
pub struct Feed {
    pub entries: VecDeque<Entry>,
    pub last_whisper_sender: Option<String>,
    next: u64,
    pub scroll: usize,
}
fn player_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn whisper_name(value: &str) -> bool {
    player_name(value.strip_prefix('~').unwrap_or(value))
}
pub fn nickname_suggestions(input: &str, names: &[String]) -> Option<(usize, Vec<String>)> {
    let (command, partial) = input.strip_prefix('/')?.split_once(' ')?;
    let verb = command.strip_prefix("minecraft:").unwrap_or(command);
    if !matches!(
        verb,
        "tp" | "teleport" | "msg" | "tell" | "w" | "trade" | "tpa" | "pay"
    ) {
        return None;
    }
    let fragment = partial.trim_start_matches(' ');
    if !fragment.is_empty() && !player_name(fragment) {
        return None;
    }
    let needle = fragment.to_ascii_lowercase();
    let mut matches: Vec<_> = names
        .iter()
        .filter(|name| player_name(name) && name.to_ascii_lowercase().starts_with(&needle))
        .cloned()
        .collect();
    matches.sort_unstable_by_key(|name| name.to_ascii_lowercase());
    matches.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Some((input.len() - fragment.len(), matches))
}
fn whisper_event(
    line: &str,
    component: &Value,
    server: bool,
) -> Option<(Kind, String, String, Option<bool>)> {
    let (direction, name, body) = match component["translate"].as_str() {
        Some("commands.message.display.incoming") => (
            Whisper::Incoming,
            component_plain(&component["with"][0]),
            component_plain(&component["with"][1]),
        ),
        Some("commands.message.display.outgoing") => (
            Whisper::Outgoing,
            component_plain(&component["with"][0]),
            component_plain(&component["with"][1]),
        ),
        None if server => {
            if let Some(rest) = line.strip_prefix("You whisper to ") {
                let (name, body) = rest.split_once(": ")?;
                (Whisper::Outgoing, name.into(), body.into())
            } else {
                let (name, body) = line.split_once(" whispers to you: ")?;
                (Whisper::Incoming, name.into(), body.into())
            }
        }
        _ => return None,
    };
    (whisper_name(&name) && !body.trim().is_empty()).then(|| {
        (
            Kind::Whisper(direction),
            name,
            body.chars().take(2048).collect(),
            None,
        )
    })
}
pub fn classify(line: &str) -> (Kind, String, String, Option<bool>) {
    let line = line.trim();
    if let Some((name, tail)) = line.split_once(' ')
        && player_name(name)
    {
        if [
            "joined the game",
            "присоединился к игре",
            "присоединилась к игре",
            "вошёл в игру",
            "вошел в игру",
            "зашёл на сервер",
            "зашел на сервер",
            "подключился к серверу",
        ]
        .contains(&tail)
            || tail.starts_with("joined for the first time")
        {
            return (
                Kind::Presence,
                name.into(),
                if tail.starts_with("joined for") {
                    "Впервые на сервере"
                } else {
                    "Подключился"
                }
                .into(),
                Some(true),
            );
        }
        if [
            "left the game",
            "покинул игру",
            "покинула игру",
            "вышел из игры",
            "вышла из игры",
            "отключился",
            "отключилась",
        ]
        .contains(&tail)
        {
            return (
                Kind::Presence,
                name.into(),
                "Отключился".into(),
                Some(false),
            );
        }
        if [
            "was burned",
            "was roasted",
            "was obliterated",
            "was stung",
            "was struck",
            "was squished",
            "was doomed",
            "blew up",
            "fell ",
            "walked into",
            "discovered the floor",
            "left the confines",
            "был убит",
            "была убита",
            "утонул",
            "утонула",
            "разбился",
            "разбилась",
            "сгорел",
            "сгорела",
            "погиб",
            "погибла",
            "умер",
            "умерла",
            "упал",
            "упала",
            "was slain by ",
            "fell from ",
            "drowned",
            "died",
            "burned to death",
            "was blown up by ",
            "hit the ground too hard",
            "suffocated in a wall",
            "went up in flames",
            "starved to death",
            "was shot by ",
            "tried to swim in lava",
            "experienced kinetic energy",
            "was killed by ",
            "froze to death",
            "fell out of the world",
            "was impaled",
            "withered away",
            "was squashed by ",
            "was pricked to death",
            "walked into danger zone",
            "was pummeled by ",
        ]
        .iter()
        .any(|s| tail.starts_with(s))
        {
            return (Kind::Death, name.into(), tail.into(), None);
        }
    }
    for (open, close) in [('<', '>'), ('(', ')')] {
        if let Some(tail) = line.strip_prefix(open)
            && let Some((name, body)) = tail.split_once(close)
            && player_name(name)
        {
            return (
                Kind::Message,
                name.into(),
                // Only an adjacent ": " belongs to the name separator.
                // Colons after the separator's whitespace are message content.
                body.strip_prefix(": ").unwrap_or(body).trim_start().into(),
                None,
            );
        }
    }
    if let Some(tail) = line.strip_prefix("* ")
        && let Some((name, body)) = tail.split_once(' ')
        && player_name(name)
    {
        return (Kind::Message, name.into(), format!("* {body}"), None);
    }
    (Kind::System, String::new(), line.into(), None)
}
fn component_plain(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    if let Some(parts) = value.as_array() {
        return parts.iter().map(component_plain).collect();
    }
    let mut text = value["text"].as_str().unwrap_or("").to_owned();
    if let Some(extra) = value["extra"].as_array() {
        for part in extra {
            text.push_str(&component_plain(part));
        }
    }
    text
}
fn semantic_event(value: &Value, line: &str) -> Option<(Kind, String, String, Option<bool>)> {
    if let Some(parts) = value.as_array() {
        return parts.iter().take(32).find_map(|v| semantic_event(v, line));
    }
    if let Some(key) = value["translate"].as_str() {
        // Player chat bodies cannot impersonate a system event via nested components.
        if key.starts_with("chat.type.") {
            return None;
        }
        let name = component_plain(&value["with"][0]);
        if player_name(&name) {
            if key == "multiplayer.player.joined" || key == "multiplayer.player.joined.renamed" {
                return Some((Kind::Presence, name, "Подключился".into(), Some(true)));
            }
            if key == "multiplayer.player.left" {
                return Some((Kind::Presence, name, "Отключился".into(), Some(false)));
            }
            if key.starts_with("death.") {
                let body = line.strip_prefix(&name).unwrap_or(line).trim().to_owned();
                return Some((Kind::Death, name, body, None));
            }
        }
    }
    if let Some(extra) = value["extra"].as_array() {
        return extra.iter().find_map(|v| semantic_event(v, line));
    }
    None
}
fn comparison_key(raw: &str, ignore: u8) -> (Kind, String, String) {
    let key: String = raw.chars().skip(ignore as usize).collect();
    let (kind, name, body, _) = classify(&key);
    (kind, name, body)
}
impl Feed {
    #[cfg(test)]
    pub fn push(&mut self, line: &str, now: u64) -> Option<u64> {
        self.push_with(
            line,
            now,
            Preferences {
                repeats: RepeatMode::Off,
                ..Preferences::default()
            },
        )
    }
    pub fn push_with(&mut self, line: &str, now: u64, preferences: Preferences) -> Option<u64> {
        self.push_decoded(line, now, preferences, None, None, Channel::All)
    }
    pub fn push_local(&mut self, line: &str, now: u64, preferences: Preferences) -> Option<u64> {
        self.push_decoded(line, now, preferences, None, None, party_channel(line))
    }
    pub fn push_component(
        &mut self,
        line: &str,
        component: &Value,
        now: u64,
        preferences: Preferences,
    ) -> Option<u64> {
        let auction = auction_notice(line);
        let channel = if auction.is_some() || trade_notice(line) {
            Channel::Trade
        } else {
            party_channel(line)
        };
        self.push_decoded(
            line,
            now,
            preferences,
            whisper_event(line, component, true).or_else(|| semantic_event(component, line)),
            auction,
            channel,
        )
    }
    pub fn push_player_component(
        &mut self,
        line: &str,
        component: &Value,
        now: u64,
        preferences: Preferences,
    ) -> Option<u64> {
        self.push_decoded(
            line,
            now,
            preferences,
            whisper_event(line, component, false),
            None,
            Channel::All,
        )
    }
    fn push_decoded(
        &mut self,
        line: &str,
        now: u64,
        preferences: Preferences,
        event: Option<(Kind, String, String, Option<bool>)>,
        auction: Option<AuctionNotice>,
        channel: Channel,
    ) -> Option<u64> {
        if line.trim().is_empty() {
            return None;
        }
        let bounded: String = line.chars().take(2048).collect();
        let (kind, name, body, joined) = event.unwrap_or_else(|| classify(&bounded));
        let channel = if matches!(kind, Kind::Whisper(_)) {
            Channel::Whisper
        } else {
            channel
        };
        if kind == Kind::Whisper(Whisper::Incoming) {
            self.last_whisper_sender = Some(name.clone());
        }
        if kind == Kind::Presence
            && let Some(old) = self
                .entries
                .iter_mut()
                .rev()
                .find(|e| !e.archived && e.kind == kind && e.name == name)
            && old.joined == joined
            && now.saturating_sub(old.at) < 5000
        {
            if body == "Впервые на сервере" {
                old.original = body;
            }
            return None;
        }
        let comparison = comparison_key(&bounded, preferences.ignore_prefix);
        if matches!(kind, Kind::Message | Kind::System) && preferences.repeats != RepeatMode::Off {
            let candidate = self
                .entries
                .iter()
                .enumerate()
                .rev()
                .take(if preferences.repeats == RepeatMode::Consecutive {
                    1
                } else {
                    50
                })
                .find(|(_, e)| {
                    !e.archived
                        && e.kind == kind
                        && e.auction == auction
                        && e.channel == channel
                        && e.name == name
                        && e.comparison == comparison
                        && now >= e.at
                        && now - e.at <= u64::from(preferences.repeat_window) * 1000
                })
                .map(|(i, _)| i);
            if let Some(index) = candidate {
                let mut previous = self.entries.remove(index)?;
                previous.repetitions = previous.repetitions.saturating_add(1);
                previous.at = now;
                // Keep the cached translation and the per-message original toggle.
                self.entries.push_back(previous);
                return self.entries.back().map(|e| e.id);
            }
        }
        self.next = self.next.saturating_add(1);
        self.entries.push_back(Entry {
            id: self.next,
            archived: false,
            auction,
            kind,
            channel,
            name,
            source_language: kind
                .is_chat()
                .then(|| detect_source_language(&body))
                .flatten(),
            original: body,
            translated: None,
            translation_language: TargetLanguage::default(),
            at: now,
            created_at: now,
            joined,
            repetitions: 1,
            show_original: false,
            comparison,
            raw: bounded,
        });
        if self.entries.len() > MAX_ENTRIES {
            // Keep recent auction receipts while ordinary chat scrolls past them.
            let index = self
                .entries
                .iter()
                .position(|e| {
                    e.archived || e.auction.is_none() || now.saturating_sub(e.at) >= 180_000
                })
                .unwrap_or(0);
            self.entries.remove(index);
        }
        Some(self.next)
    }
    /// The archive contains display data only; runtime IDs and reply targets are session-local.
    pub fn archive(&self) -> Value {
        Value::Array(self.entries.iter().map(|e| serde_json::json!({
            "kind": match e.kind { Kind::Message => "message", Kind::Whisper(Whisper::Incoming) => "incoming", Kind::Whisper(Whisper::Outgoing) => "outgoing", Kind::Presence => "presence", Kind::Death => "death", Kind::System => "system" },
            "channel": e.channel as u8,
            "auction": e.auction.map(|a| match a { AuctionNotice::Listed => "listed", AuctionNotice::Sold => "sold" }),
            "name": e.name, "original": e.original, "translated": e.translated,
            "source_language": e.source_language,
            "translation_language": e.translation_language.code(),
            "joined": e.joined, "repetitions": e.repetitions,
            "show_original": e.show_original, "raw": e.raw,
        })).collect())
    }
    /// Merge late disk reads before live messages, without recording unread/reply state.
    pub fn restore(&mut self, value: &Value, preferences: Preferences) -> Result<(), String> {
        let rows = value.as_array().ok_or("Invalid chat entries")?;
        if rows.len() > MAX_ENTRIES {
            return Err("Too many archived chat entries".into());
        }
        let mut restored = VecDeque::new();
        let mut next = self.next;
        for row in rows {
            let string = |key: &str, max: usize| -> Result<String, String> {
                let s = row[key]
                    .as_str()
                    .ok_or_else(|| format!("Invalid chat {key}"))?;
                if s.chars().count() > max {
                    return Err(format!("Oversized chat {key}"));
                }
                Ok(s.to_owned())
            };
            let kind = match row["kind"].as_str() {
                Some("message") => Kind::Message,
                Some("incoming") => Kind::Whisper(Whisper::Incoming),
                Some("outgoing") => Kind::Whisper(Whisper::Outgoing),
                Some("presence") => Kind::Presence,
                Some("death") => Kind::Death,
                Some("system") => Kind::System,
                _ => return Err("Invalid archived chat kind".into()),
            };
            let channel = match row["channel"].as_u64() {
                Some(0) => Channel::All,
                Some(1) => Channel::Whisper,
                Some(2) => Channel::Party,
                Some(3) => Channel::Trade,
                _ => return Err("Invalid chat channel".into()),
            };
            let auction = match row["auction"].as_str() {
                Some("listed") => Some(AuctionNotice::Listed),
                Some("sold") => Some(AuctionNotice::Sold),
                None if row["auction"].is_null() => None,
                _ => return Err("Invalid auction notice".into()),
            };
            let translated = if row["translated"].is_null() {
                None
            } else {
                Some(string("translated", 2048)?)
            };
            let joined = if row["joined"].is_null() {
                None
            } else {
                Some(row["joined"].as_bool().ok_or("Invalid presence")?)
            };
            let raw = string("raw", 2048)?;
            next = next.checked_add(1).ok_or("Chat ID overflow")?;
            restored.push_back(Entry {
                id: next,
                archived: true,
                auction,
                kind,
                channel,
                name: string("name", 256)?,
                source_language: if kind.is_chat() {
                    row["source_language"]
                        .as_str()
                        .and_then(source_language)
                        .or_else(|| row["original"].as_str().and_then(detect_source_language))
                } else {
                    None
                },
                original: string("original", 2048)?,
                translated,
                translation_language: TargetLanguage::from_code(
                    row["translation_language"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default(),
                at: 0,
                created_at: 0,
                joined,
                repetitions: row["repetitions"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0)
                    .ok_or("Invalid repetitions")?,
                show_original: row["show_original"]
                    .as_bool()
                    .ok_or("Invalid original toggle")?,
                comparison: comparison_key(&raw, preferences.ignore_prefix),
                raw,
            });
        }
        self.next = next;
        restored.append(&mut self.entries);
        while restored.len() > MAX_ENTRIES {
            restored.pop_front();
        }
        self.entries = restored;
        Ok(())
    }
    pub fn reconfigure(&mut self, preferences: Preferences) {
        for entry in &mut self.entries {
            entry.comparison = comparison_key(&entry.raw, preferences.ignore_prefix);
        }
    }
    pub fn toggle_original(&mut self, id: u64) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.show_original = !entry.show_original;
            return true;
        }
        false
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.last_whisper_sender = None;
        self.scroll = 0;
    }
    pub fn translate(&mut self, id: u64, text: String, language: impl Into<TargetLanguage>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.translated = Some(text.chars().take(2048).collect());
            entry.translation_language = language.into();
        }
    }
}
pub fn wrap(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut rows = Vec::new();
    let mut line = String::new();
    for c in text.chars().take(2048) {
        if c == '\n' {
            rows.push(std::mem::take(&mut line));
            continue;
        }
        line.push(c);
        if measure(&line) > width.max(12.0) && line.chars().count() > 1 {
            if let Some(boundary) = line.rfind(char::is_whitespace) {
                let head = line[..boundary].trim_end();
                if !head.is_empty() {
                    let remainder = line[boundary..].trim_start().to_owned();
                    rows.push(head.to_owned());
                    line = remainder;
                    continue;
                }
            }
            // A word wider than an empty line has no usable space boundary.
            // Retain the UTF-8-safe fallback instead of clipping or dropping it.
            line.pop();
            rows.push(std::mem::take(&mut line));
            line.push(c);
        }
    }
    if !line.is_empty() || rows.is_empty() {
        rows.push(line);
    }
    rows
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_wrap_moves_a_whole_word_to_the_next_line() {
        let measure = |s: &str| s.chars().count() as f32;
        assert_eq!(
            wrap("hello wonderful world", 16.0, measure),
            ["hello wonderful", "world"]
        );
        assert_eq!(
            wrap("Steve: hello beautiful world", 18.0, measure),
            ["Steve: hello", "beautiful world"]
        );
        assert_eq!(
            wrap("Привет красивый мир", 12.0, measure),
            ["Привет", "красивый мир"]
        );
        assert_eq!(
            wrap("hello\u{2003}wonderful", 12.0, measure),
            ["hello", "wonderful"]
        );
        assert_eq!(wrap("one  two three", 12.0, measure), ["one  two", "three"]);
    }
    #[test]
    fn chat_wrap_preserves_newlines_and_bounds_oversized_words() {
        let measure = |s: &str| s.chars().count() as f32;
        assert_eq!(
            wrap("hello\n\nworld", 12.0, measure),
            ["hello", "", "world"]
        );
        assert_eq!(wrap("", 12.0, measure), [""]);
        let word = "длинноесловобезпробелов";
        let rows = wrap(word, 12.0, measure);
        assert!(rows.iter().all(|r| measure(r) <= 12.0));
        assert_eq!(rows.concat(), word);
    }
    #[test]
    fn chat_wrap_uses_measured_width_instead_of_character_count() {
        let measure = |s: &str| s.chars().map(|c| if c == 'W' { 4.0 } else { 1.0 }).sum();
        assert_eq!(wrap("iii WWW iii", 12.0, measure), ["iii", "WWW", "iii"]);
        assert_eq!(wrap("hello world!", 12.0, measure), ["hello world!"]);
        let text = "word ".repeat(500);
        let rows = wrap(&text, 12.0, measure);
        assert!(rows.iter().all(|row| measure(row) <= 12.0));
        assert!(rows.concat().chars().count() <= 2048);
    }
    #[test]
    fn archived_auction_receipts_cannot_evict_new_live_chat() {
        let p = Preferences::default();
        let mut source = Feed::default();
        source.push_with("<Alex> old", 1, p);
        let mut row = source.archive()[0].clone();
        row["auction"] = serde_json::json!("sold");
        let mut restored = Feed::default();
        restored
            .restore(&Value::Array(vec![row; MAX_ENTRIES]), p)
            .unwrap();
        let live_id = restored.push_with("<Bob> fresh", 10, p).unwrap();
        assert_eq!(restored.entries.len(), MAX_ENTRIES);
        assert_eq!(restored.entries.back().unwrap().id, live_id);
        assert!(!restored.entries.back().unwrap().archived);
    }
    #[test]
    fn archive_distinguishes_english_translations_from_legacy_russian_ones() {
        let preferences = Preferences::default();
        let mut feed = Feed::default();
        let id = feed.push_with("<Alex> hello", 1, preferences).unwrap();
        feed.translate(id, "привет".into(), Language::Russian);
        let id = feed.push_with("<Alex> привет", 2, preferences).unwrap();
        feed.translate(id, "hello".into(), Language::English);
        let mut archive = feed.archive();
        archive[0]
            .as_object_mut()
            .unwrap()
            .remove("translation_language");
        let mut restored = Feed::default();
        restored.restore(&archive, preferences).unwrap();
        assert!(restored.entries[0].has_translation(Language::Russian));
        assert!(!restored.entries[0].has_translation(Language::English));
        assert!(restored.entries[1].has_translation(Language::English));
        assert!(!restored.entries[1].has_translation(Language::Russian));
    }
    #[test]
    fn archive_merge_keeps_live_ids_and_reply_target_without_repeat_coalescing() {
        let p = Preferences::default();
        let mut old = Feed::default();
        let id = old.push_with("<Alex> hello", 1, p).unwrap();
        old.translate(id, "привет".into(), Language::Russian);
        old.toggle_original(id);
        old.push_component("", &serde_json::json!({}), 2, p);
        let incoming = serde_json::json!({"translate": "commands.message.display.incoming", "with": ["OldSender", "secret"]});
        old.push_player_component("old whisper", &incoming, 3, p);
        let mut live = Feed::default();
        let live_id = live.push_with("<Bob> live", 2, p).unwrap();
        live.restore(&old.archive(), p).unwrap();
        assert_eq!(live.entries.len(), 3);
        assert!(live.last_whisper_sender.is_none());
        assert_eq!(live.entries.back().unwrap().id, live_id);
        assert!(!live.entries.back().unwrap().archived);
        assert!(live.entries[0].archived && live.entries[0].show_original);
        assert_eq!(live.entries[0].translated.as_deref(), Some("привет"));
        assert_eq!(live.entries[0].source_language.as_deref(), Some("EN"));
        let fresh = live.push_with("<Alex> hello", 4, p).unwrap();
        assert_ne!(fresh, live.entries[0].id);
        assert_eq!(live.entries.len(), 4);
        assert_eq!(live.entries.back().unwrap().repetitions, 1);
        assert!(!live.entries.back().unwrap().archived);
        let incoming = serde_json::json!({"translate": "commands.message.display.incoming", "with": ["NewSender", "live secret"]});
        live.push_player_component("new whisper", &incoming, 5, p);
        assert_eq!(live.last_whisper_sender.as_deref(), Some("NewSender"));
    }
    #[test]
    fn invalid_archive_does_not_partially_replace_live_feed() {
        let mut feed = Feed::default();
        let p = Preferences::default();
        feed.push_with("<Alex> live", 1, p);
        let mut bad = feed.archive();
        bad.as_array_mut()
            .unwrap()
            .push(serde_json::json!({"kind": "invalid"}));
        assert!(feed.restore(&bad, p).is_err());
        assert_eq!(feed.entries.len(), 1);
        assert!(!feed.entries[0].archived);
        let mut oversized = feed.archive();
        oversized[0]["original"] = serde_json::json!("x".repeat(2049));
        assert!(feed.restore(&oversized, p).is_err());
        assert_eq!(feed.entries[0].original, "live");
        let row = feed.archive()[0].clone();
        assert!(
            feed.restore(&Value::Array(vec![row; MAX_ENTRIES + 1]), p)
                .is_err()
        );
    }
    #[test]
    fn chat_tabs_keep_trusted_sources_and_independent_unread_counts() {
        let mut feed = Feed::default();
        let p = Preferences::default();
        let notice = "Trade request sent to Alex.";
        feed.push_with(notice, 0, p);
        feed.push_component(notice, &serde_json::json!({"text": notice}), 1, p);
        feed.push_local("[Party] Alex joined.", 2, p);
        assert_eq!(feed.entries.len(), 3);
        assert_eq!(feed.entries[0].channel, Channel::All);
        assert!(Channel::Trade.shows(&feed.entries[1]));
        assert!(Channel::Party.shows(&feed.entries[2]));
        let mut tabs = Tabs::default();
        tabs.record(Channel::Trade);
        tabs.record(Channel::Party);
        tabs.record(Channel::Party);
        tabs.selected = Channel::Trade;
        tabs.read();
        assert_eq!(tabs.unread(Channel::Trade), 0);
        assert_eq!(tabs.unread(Channel::Party), 2);
        tabs.selected = Channel::All;
        tabs.read();
        assert_eq!(tabs.unread(Channel::Party), 0);
    }
    #[test]
    fn whispers_keep_the_body_and_only_incoming_updates_reply_target() {
        let mut feed = Feed::default();
        let p = Preferences::default();
        let incoming = serde_json::json!({
            "translate": "commands.message.display.incoming",
            "with": [{"text": "Alex_42", "color": "gray"}, {"text": "hello", "extra": [" there"]}]
        });
        let id = feed
            .push_player_component("локализованное ЛС", &incoming, 1, p)
            .unwrap();
        let entry = &feed.entries[0];
        assert_eq!(entry.kind, Kind::Whisper(Whisper::Incoming));
        assert!(entry.kind.is_chat());
        assert_eq!(entry.original, "hello there");
        feed.translate(id, "привет".into(), Language::Russian);
        assert_eq!(feed.entries[0].body(true), "привет");
        let outgoing = "You whisper to Bob: Steve whispers to you: hi";
        feed.push_component(outgoing, &serde_json::json!({"text": outgoing}), 2, p);
        let entry = &feed.entries[1];
        assert_eq!(entry.kind, Kind::Whisper(Whisper::Outgoing));
        assert_eq!(entry.name, "Bob");
        assert_eq!(feed.last_whisper_sender.as_deref(), Some("Alex_42"));
        let fake = "Eve whispers to you: forged";
        feed.push_with(fake, 3, p);
        feed.push_player_component(fake, &serde_json::json!({"text": fake}), 4, p);
        assert_eq!(feed.last_whisper_sender.as_deref(), Some("Alex_42"));
        let literal = "~Rocky whispers to you: psst";
        feed.push_component(literal, &serde_json::json!({"text": literal}), 5, p);
        assert_eq!(feed.last_whisper_sender.as_deref(), Some("~Rocky"));
        feed.clear();
        assert_eq!(feed.last_whisper_sender, None);
    }

    #[test]
    fn nickname_list_includes_pay_and_preserves_the_amount_argument() {
        let names = ["Bob", "albert", "Alex_42", "not a name", "albert"].map(str::to_owned);
        for command in ["pay", "msg", "tpa", "trade", "minecraft:pay"] {
            let input = format!("/{command} AL");
            let (start, matches) = nickname_suggestions(&input, &names).unwrap();
            assert_eq!(&input[start..], "AL");
            assert_eq!(matches, ["albert", "Alex_42"]);
        }
        assert_eq!(
            nickname_suggestions("/pay ", &names).unwrap().1,
            ["albert", "Alex_42", "Bob"]
        );
        assert_eq!(
            nickname_suggestions("/pay  Bo", &names).unwrap(),
            (6, vec!["Bob".into()])
        );
        for input in [
            "/pay Bob 100",
            "/pay Bob ",
            "/pay @a",
            "/pay",
            "/reply Bo",
            "/msg Bob hi",
        ] {
            assert_eq!(nickname_suggestions(input, &names), None, "{input}");
        }
        assert!(
            nickname_suggestions("/pay no_match", &names)
                .unwrap()
                .1
                .is_empty()
        );
    }
    #[test]
    fn auction_receipts_require_server_origin_and_exact_grammar() {
        let p = Preferences::default();
        let samples = [
            (
                "Listed #12: 64 diamond for 1,000 coins (15.62 each), 48h.",
                AuctionNotice::Listed,
            ),
            (
                "Auction #3: 1 diamond_sword from 100 coins, ends in 2h.",
                AuctionNotice::Listed,
            ),
            (
                "Steve bought your 10 diamond for 1,000 coins.",
                AuctionNotice::Sold,
            ),
            (
                "Auction #1 (3 emerald) sold to Steve for 400 coins.",
                AuctionNotice::Sold,
            ),
        ];
        let mut f = Feed::default();
        for (line, kind) in samples {
            f.push_with(line, 0, p); // local/player text cannot promote a receipt
            assert_eq!(f.entries.back().unwrap().auction, None);
            f.push_component(line, &serde_json::json!({"text":line}), 1, p);
            let entry = f.entries.back().unwrap();
            assert_eq!(entry.auction, Some(kind));
            assert_eq!(entry.original, line);
            assert_eq!(entry.repetitions, 1); // never merge an untrusted copy
        }
        for line in [
            "<Steve> Alex bought your 10 diamond for 1,000 coins.",
            "Steve: Listed #12: 64 diamond for 1,000 coins (15.62 each), 48h.",
            "Steve bought your diamonds for hello coins.",
            "Auction #x (3 emerald) sold to Steve for 400 coins.",
            "Bought 3 emerald from Steve for 400 coins.",
            "Auction #1 (3 emerald) sold to Steve for 400 coins. extra",
        ] {
            assert_eq!(auction_notice(line), None, "{line}");
        }
    }
    #[test]
    fn chat_flood_preserves_recent_auction_receipts_with_bounded_history() {
        let mut f = Feed::default();
        let p = Preferences {
            repeats: RepeatMode::Off,
            ..Preferences::default()
        };
        let line = "Steve bought your 10 diamond for 1,000 coins.";
        f.push_component(line, &serde_json::json!({"text":line}), 0, p);
        for i in 1..1000 {
            f.push_with(&format!("<Alex> message {i}"), i, p);
        }
        assert_eq!(f.entries.len(), MAX_ENTRIES);
        assert!(
            f.entries
                .iter()
                .any(|e| e.auction == Some(AuctionNotice::Sold))
        );
        f.push_with("<Alex> later", 180_001, p);
        assert!(!f.entries.iter().any(|e| e.auction.is_some()));
    }

    #[test]
    fn english_event_labels_are_not_cyrillic() {
        let cyrillic = |s: &str| s.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё'));
        let mut feed = Feed::default();
        feed.push("Steve joined for the first time: welcome!", 1);
        feed.push("Steve left the game", 20_000);
        feed.push("Steve joined the game", 40_000);
        for e in &feed.entries {
            assert!(!cyrillic(&event_summary(e, false, Language::English)));
            assert_eq!(
                e.display_body(false, Language::Russian),
                if e.joined == Some(false) { "−" } else { "+" }
            );
        }
        for line in [
            "Alex drowned",
            "Alex tried to swim in lava",
            "Alex fell from a high place",
            "Alex burned to death",
            "Alex blew up",
            "Alex died",
        ] {
            let mut f = Feed::default();
            f.push(line, 0);
            assert!(!cyrillic(&event_summary(
                &f.entries[0],
                false,
                Language::English
            )));
        }
    }
    #[test]
    fn compact_events_remain_one_line_and_preserve_the_original() {
        let mut feed = Feed::default();
        feed.push("Alex was slain by Steve using a sword", 0);
        let e = &feed.entries[0];
        let summary = event_summary(e, false, Language::Russian);
        assert_eq!(summary, "Alex ← Steve");
        let shown = single_line(&summary, 8.0, |s| s.chars().count() as f32);
        assert!(!shown.contains('\n'));
        assert!(shown.chars().count() <= 8);
        assert!(shown.ends_with('…'));
        assert_eq!(e.original, "was slain by Steve using a sword");
    }
    #[test]
    fn joins_are_deduplicated_but_leaves_and_rejoins_remain() {
        let mut feed = Feed::default();
        feed.push("Steve joined the game", 1);
        feed.push("Steve joined for the first time: welcome!", 2);
        assert_eq!(feed.entries.len(), 1);
        assert_eq!(feed.entries[0].original, "Впервые на сервере");
        assert_eq!(feed.entries[0].display_body(false, Language::English), "+");
        feed.push("Steve left the game", 3);
        feed.push("Steve joined the game", 4);
        assert_eq!(feed.entries.len(), 3);
    }
    #[test]
    fn death_and_chat_are_separate_and_chat_cannot_trigger_join_dedup() {
        assert_eq!(classify("Steve fell from a high place").0, Kind::Death);
        assert_eq!(classify("<Steve> Alex joined the game").0, Kind::Message);
        assert_eq!(classify("(Steve) hello").2, "hello");
    }
    #[test]
    fn chat_bodies_preserve_leading_colons() {
        for header in ["<Steve> ", "(Steve) ", "<Steve>: ", "(Steve): "] {
            for body in [":3", ":", "::", ": hello", ":: привет", "hello"] {
                let line = format!("{header}{body}");
                let mut feed = Feed::default();
                feed.push_player_component(
                    &line,
                    &serde_json::json!({"text": line}),
                    0,
                    Preferences::default(),
                )
                .unwrap();
                let entry = &feed.entries[0];
                assert_eq!(entry.kind, Kind::Message);
                assert_eq!(entry.name, "Steve");
                assert_eq!(entry.original, body, "line={line:?}");
                for language in [Language::English, Language::Russian] {
                    assert_eq!(entry.display_body(false, language), body);
                }
            }
        }
    }
    #[test]
    fn colon_emotes_do_not_merge_with_different_message_bodies() {
        let mut feed = Feed::default();
        let p = Preferences::default();
        let emote = feed.push_with("<Steve> :3", 0, p).unwrap();
        let digit = feed.push_with("<Steve> 3", 1, p).unwrap();
        assert_ne!(emote, digit);
        assert_eq!(feed.entries.len(), 2);
        assert_eq!(feed.push_with("<Steve> :3", 2, p), Some(emote));
    }
    #[test]
    fn unicode_wrap_and_delayed_translation_after_reset() {
        assert_eq!(
            wrap("Привет", 3.0, |s| s.chars().count() as f32).concat(),
            "Привет"
        );
        let mut feed = Feed::default();
        let old = feed.push("<Steve> hello", 0).unwrap();
        feed.clear();
        let new = feed.push("<Steve> new", 1).unwrap();
        feed.translate(old, "старое".into(), Language::Russian);
        assert!(feed.entries[0].translated.is_none());
        feed.translate(new, "новое".into(), Language::Russian);
        assert_eq!(feed.entries[0].body(true), "новое");
    }
    #[test]
    fn recent_spam_compacts_without_merging_authors_or_distinct_messages() {
        let mut f = Feed::default();
        let p = Preferences::default();
        let id = f.push_with("<Alice> привет", 0, p).unwrap();
        f.translate(id, "hello".into(), Language::Russian);
        f.toggle_original(id);
        f.push_with("<Bob> привет", 10, p);
        f.push_with("<Alice> другое", 20, p);
        assert_eq!(f.push_with("<Alice> привет", 30, p), Some(id));
        assert_eq!(f.entries.len(), 3);
        let entry = f.entries.back().unwrap();
        assert_eq!(entry.repetitions, 2);
        assert_eq!(entry.created_at, 0);
        assert!(entry.show_original);
        assert_eq!(entry.body(true), "привет");
        f.toggle_original(id);
        assert_eq!(f.entries.back().unwrap().body(true), "hello");
        f.push_with("<Alice> привет", 20000, p);
        assert_eq!(f.entries.len(), 4);
    }
    #[test]
    fn consecutive_off_and_ignored_timestamp_prefix() {
        let mut f = Feed::default();
        let p = Preferences {
            repeats: RepeatMode::Consecutive,
            ..Preferences::default()
        };
        f.push_with("<Alice> one", 0, p);
        f.push_with("<Bob> two", 1, p);
        f.push_with("<Alice> one", 2, p);
        assert_eq!(f.entries.len(), 3);
        f.push_with("<Alice> one", 3, p);
        assert_eq!(f.entries.len(), 3);
        f.push_with(
            "<Alice> one",
            4,
            Preferences {
                repeats: RepeatMode::Off,
                ..p
            },
        );
        assert_eq!(f.entries.len(), 4);
        f.clear();
        let p = Preferences {
            ignore_prefix: 11,
            ..Preferences::default()
        };
        f.push_with("[12:00:00] <Alice> one", 0, p);
        f.push_with("[12:00:01] <Alice> one", 1, p);
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].repetitions, 2);
        f.push_with("[12:00:02] <Bob> one", 2, p);
        assert_eq!(f.entries.len(), 2);
    }
    #[test]
    fn changing_prefix_setting_reindexes_existing_history() {
        let mut f = Feed::default();
        f.push_with("[12:00:00] <Alice> hello", 0, Preferences::default());
        let p = Preferences {
            ignore_prefix: 11,
            ..Preferences::default()
        };
        f.reconfigure(p);
        f.push_with("[12:00:01] <Alice> hello", 1, p);
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].repetitions, 2);
    }
    #[test]
    fn system_components_are_classified_independent_of_client_language() {
        let p = Preferences::default();
        let mut f = Feed::default();
        f.push_component(
            "Steve присоединился к игре",
            &serde_json::json!({"translate":"multiplayer.player.joined","with":[{"text":"Steve"}]}),
            0,
            p,
        );
        assert_eq!(f.entries[0].kind, Kind::Presence);
        assert_eq!(f.entries[0].display_body(false, Language::English), "+");
        f.push_with("Steve joined for the first time: welcome!", 1, p);
        assert_eq!(f.entries.len(), 1);
        f.push_component(
            "Steve покинул игру",
            &serde_json::json!({"translate":"multiplayer.player.left","with":["Steve"]}),
            2,
            p,
        );
        f.push_component("Steve was burned to a crisp whilst fighting Zombie",&serde_json::json!({"translate":"death.attack.onFire.player","with":["Steve","Zombie"]}),3,p);
        assert_eq!(f.entries[1].kind, Kind::Presence);
        assert_eq!(f.entries[1].display_body(false, Language::English), "−");
        assert_eq!(f.entries[2].kind, Kind::Death);
        assert_eq!(classify("Steve покинул игру").0, Kind::Presence);
        assert_eq!(classify("Steve был убит зомби").0, Kind::Death);
    }
    #[test]
    fn friends_view_includes_existing_important_chat_without_changing_other_tabs() {
        let mut feed = Feed::default();
        feed.push("<Alex> hello", 1);
        let entry = &feed.entries[0];
        let mut priority = super::super::claim_borders_chat_priority::Preferences::default();
        assert!(
            !Channel::Party.shows_with_friends(entry, priority.rule(&entry.name, false).is_some())
        );
        priority.set("aLeX", 0x77d9b0, false).unwrap();
        assert!(
            Channel::Party.shows_with_friends(entry, priority.rule(&entry.name, false).is_some())
        );
        assert!(Channel::All.shows(entry));
        assert_eq!(entry.channel, Channel::All);
        assert!(
            Channel::Party.shows_with_friends(entry, priority.rule(&entry.name, true).is_some())
        );
        feed.push("Steve joined the game", 2);
        assert!(!Channel::Party.shows_with_friends(&feed.entries[1], true));
    }
    #[test]
    fn presence_modes_keep_name_and_original_without_a_stray_symbol_line() {
        use super::super::claim_borders_chat_preferences::PresenceStyle;
        let mut f = Feed::default();
        f.push("Steve joined the game", 1);
        f.push("Steve left the game", 20000);
        assert_eq!(
            presence_summary(&f.entries[0], PresenceStyle::Icons, Language::Russian),
            "Steve"
        );
        assert_eq!(
            presence_summary(&f.entries[0], PresenceStyle::Text, Language::Russian),
            "Steve · Подключился"
        );
        assert_eq!(
            presence_summary(&f.entries[1], PresenceStyle::Text, Language::English),
            "Steve · Left the server"
        );
        assert_eq!(f.entries[0].original, "Подключился");
        assert_eq!(f.entries[1].original, "Отключился");
    }
}
