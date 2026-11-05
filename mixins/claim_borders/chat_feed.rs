// Compact chat model, independent of game rendering and networking.
use super::claim_borders_chat_preferences::{Preferences, RepeatMode};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Message,
    Presence,
    Death,
    System,
}
/// Only recognized server-origin auction receipts are promoted to the important tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuctionNotice {
    Listed,
    Sold,
}
impl AuctionNotice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Listed => "Аукцион · лот выставлен",
            Self::Sold => "Аукцион · ваш лот продан",
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
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub auction: Option<AuctionNotice>,
    pub kind: Kind,
    pub name: String,
    pub original: String,
    pub translated: Option<String>,
    pub at: u64,
    pub created_at: u64,
    pub joined: Option<bool>,
    pub repetitions: u32,
    pub show_original: bool,
    comparison: (Kind, String, String),
    raw: String,
}
impl Entry {
    pub fn body(&self, translated: bool) -> &str {
        if translated && !self.show_original {
            self.translated.as_deref().unwrap_or(&self.original)
        } else {
            &self.original
        }
    }
}
/// A compact event keeps its full original in Entry for the hover card.
pub fn event_summary(entry: &Entry, translated: bool) -> String {
    let body = entry.body(translated);
    if let Some(notice) = entry.auction {
        return format!("{} · {body}", notice.label());
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
        "Утонул"
    } else if lower.contains("lava") || lower.contains("лав") {
        "Лава"
    } else if lower.contains("fell")
        || lower.contains("ground")
        || lower.contains("упал")
        || lower.contains("разбил")
    {
        "Падение"
    } else if lower.contains("burn")
        || lower.contains("flame")
        || lower.contains("fire")
        || lower.contains("сгор")
    {
        "Огонь"
    } else if lower.contains("explod")
        || lower.contains("blown")
        || lower.contains("blew up")
        || lower.contains("взорва")
    {
        "Взрыв"
    } else {
        "Погиб"
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
                body.trim_start_matches([' ', ':']).into(),
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
        self.push_decoded(line, now, preferences, None, None)
    }
    pub fn push_component(
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
            semantic_event(component, line),
            auction_notice(line),
        )
    }
    fn push_decoded(
        &mut self,
        line: &str,
        now: u64,
        preferences: Preferences,
        event: Option<(Kind, String, String, Option<bool>)>,
        auction: Option<AuctionNotice>,
    ) -> Option<u64> {
        if line.trim().is_empty() {
            return None;
        }
        let bounded: String = line.chars().take(2048).collect();
        let (kind, name, body, joined) = event.unwrap_or_else(|| classify(&bounded));
        if kind == Kind::Presence
            && let Some(old) = self
                .entries
                .iter_mut()
                .rev()
                .find(|e| e.kind == kind && e.name == name)
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
                    e.kind == kind
                        && e.auction == auction
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
            auction,
            kind,
            name,
            original: body,
            translated: None,
            at: now,
            created_at: now,
            joined,
            repetitions: 1,
            show_original: false,
            comparison,
            raw: bounded,
        });
        if self.entries.len() > 200 {
            // Keep recent auction receipts while ordinary chat scrolls past them.
            let index = self
                .entries
                .iter()
                .position(|e| e.auction.is_none() || now.saturating_sub(e.at) >= 180_000)
                .unwrap_or(0);
            self.entries.remove(index);
        }
        Some(self.next)
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
        self.scroll = 0;
    }
    pub fn translate(&mut self, id: u64, text: String) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.translated = Some(text.chars().take(2048).collect());
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
        for i in 1..500 {
            f.push_with(&format!("<Alex> message {i}"), i, p);
        }
        assert_eq!(f.entries.len(), 200);
        assert!(
            f.entries
                .iter()
                .any(|e| e.auction == Some(AuctionNotice::Sold))
        );
        f.push_with("<Alex> later", 180_001, p);
        assert!(!f.entries.iter().any(|e| e.auction.is_some()));
    }

    #[test]
    fn compact_events_remain_one_line_and_preserve_the_original() {
        let mut feed = Feed::default();
        feed.push("Alex was slain by Steve using a sword", 0);
        let e = &feed.entries[0];
        assert_eq!(event_summary(e, false), "Alex ← Steve");
        let shown = single_line(&event_summary(e, false), 8.0, |s| s.chars().count() as f32);
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
    fn unicode_wrap_and_delayed_translation_after_reset() {
        assert_eq!(
            wrap("Привет", 3.0, |s| s.chars().count() as f32).concat(),
            "Привет"
        );
        let mut feed = Feed::default();
        let old = feed.push("<Steve> hello", 0).unwrap();
        feed.clear();
        let new = feed.push("<Steve> new", 1).unwrap();
        feed.translate(old, "старое".into());
        assert!(feed.entries[0].translated.is_none());
        feed.translate(new, "новое".into());
        assert_eq!(feed.entries[0].body(true), "новое");
    }
    #[test]
    fn recent_spam_compacts_without_merging_authors_or_distinct_messages() {
        let mut f = Feed::default();
        let p = Preferences::default();
        let id = f.push_with("<Alice> привет", 0, p).unwrap();
        f.translate(id, "hello".into());
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
        f.push_with("Steve joined for the first time: welcome!", 1, p);
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].kind, Kind::Presence);
        f.push_component(
            "Steve покинул игру",
            &serde_json::json!({"translate":"multiplayer.player.left","with":["Steve"]}),
            2,
            p,
        );
        f.push_component("Steve was burned to a crisp whilst fighting Zombie",&serde_json::json!({"translate":"death.attack.onFire.player","with":["Steve","Zombie"]}),3,p);
        assert_eq!(f.entries[1].kind, Kind::Presence);
        assert_eq!(f.entries[2].kind, Kind::Death);
        assert_eq!(classify("Steve покинул игру").0, Kind::Presence);
        assert_eq!(classify("Steve был убит зомби").0, Kind::Death);
    }
}
