// Compact chat model, independent of game rendering and networking.
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Message,
    Presence,
    Death,
    System,
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
    pub name: String,
    pub original: String,
    pub translated: Option<String>,
    pub at: u64,
    pub joined: Option<bool>,
}
impl Entry {
    pub fn body(&self, translated: bool) -> &str {
        if translated {
            self.translated.as_deref().unwrap_or(&self.original)
        } else {
            &self.original
        }
    }
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
        if tail == "joined the game" || tail.starts_with("joined for the first time") {
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
        if tail == "left the game" {
            return (
                Kind::Presence,
                name.into(),
                "Отключился".into(),
                Some(false),
            );
        }
        if [
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
impl Feed {
    pub fn push(&mut self, line: &str, now: u64) -> Option<u64> {
        if line.trim().is_empty() {
            return None;
        }
        let bounded: String = line.chars().take(2048).collect();
        let (kind, name, body, joined) = classify(&bounded);
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
        self.next = self.next.saturating_add(1);
        self.entries.push_back(Entry {
            id: self.next,
            kind,
            name,
            original: body,
            translated: None,
            at: now,
            joined,
        });
        if self.entries.len() > 200 {
            self.entries.pop_front();
        }
        Some(self.next)
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
}
