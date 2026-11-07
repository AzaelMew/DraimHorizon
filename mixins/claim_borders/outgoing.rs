// Translation changes text only. The native adapter sends it as signed chat, never a command.
use claim_borders_relay_client::translation::Language;
use std::collections::VecDeque;
pub const WAIT_MS: u64 = 15_000;
#[derive(Clone, Debug)]
pub struct Message {
    pub id: u64,
    pub original: String,
    pub result: Option<Option<String>>,
    pub at: u64,
}
#[derive(Default)]
pub struct Queue {
    pub messages: VecDeque<Message>,
    next: u64,
}
impl Queue {
    pub fn push(&mut self, text: &str, now: u64, translate: bool) -> Result<u64, &'static str> {
        self.push_in(text, now, translate, Language::Russian)
    }
    pub fn push_in(
        &mut self,
        text: &str,
        now: u64,
        translate: bool,
        language: Language,
    ) -> Result<u64, &'static str> {
        if self.messages.len() >= 8 {
            return Err(language.text(
                "Очередь перевода заполнена. Сообщение осталось в истории ввода.",
                "Translation queue is full. The message stays in the input history.",
            ));
        }
        self.next = self.next.wrapping_add(1).max(1);
        let id = self.next;
        self.messages.push_back(Message {
            id,
            original: text.into(),
            result: (!translate).then(|| Some(text.into())),
            at: now,
        });
        Ok(id)
    }
    pub fn complete(&mut self, id: u64, result: Option<String>) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            m.result = Some(result.filter(|s| {
                s.encode_utf16().count() <= 1024
                    && s.chars().any(|c| c > ' ' && c != '§' && c != '\u{007f}')
            }));
        }
    }
    pub fn pop(&mut self, now: u64) -> Option<(String, bool)> {
        let m = self.messages.front()?;
        if m.result.is_none() && now.saturating_sub(m.at) < WAIT_MS {
            return None;
        }
        let m = self.messages.pop_front()?;
        let translated = m.result.flatten().filter(|s| !s.trim().is_empty());
        let failed = translated.is_none();
        Some((translated.unwrap_or(m.original), failed))
    }
}
pub fn split(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut units = 0;
    for word in text.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| *c >= ' ' && *c != '\u{007f}' && *c != '§')
            .collect();
        let word = clean.as_str();
        let count = word.encode_utf16().count();
        if count == 0 {
            continue;
        }
        if !current.is_empty() && units + 1 + count > 256 {
            lines.push(std::mem::take(&mut current));
            units = 0;
        }
        if count <= 256 {
            if !current.is_empty() {
                current.push(' ');
                units += 1;
            }
            current.push_str(word);
            units += count;
        } else {
            for ch in word.chars() {
                let n = ch.len_utf16();
                if units + n > 256 {
                    lines.push(std::mem::take(&mut current));
                    units = 0;
                }
                current.push(ch);
                units += n;
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_queue_error_is_not_cyrillic() {
        let mut q = Queue::default();
        for _ in 0..8 {
            q.push_in("a", 0, true, Language::English).unwrap();
        }
        let e = q.push_in("a", 0, true, Language::English).unwrap_err();
        assert!(!e.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё')));
    }
    #[test]
    fn preserves_order_and_times_out_without_losing_original() {
        let mut q = Queue::default();
        let a = q.push("Привет", 0, true).unwrap();
        q.push("ok", 1, false).unwrap();
        assert!(q.pop(10).is_none());
        q.complete(a, Some("Hello".into()));
        assert_eq!(q.pop(10), Some(("Hello".into(), false)));
        assert_eq!(q.pop(10), Some(("ok".into(), false)));
        q.push("Ещё", 20, true).unwrap();
        assert_eq!(q.pop(15020), Some(("Ещё".into(), true)));
    }
    #[test]
    fn splits_at_packet_utf16_limit_and_keeps_all_characters() {
        let text = "🙂".repeat(400);
        let parts = split(&text);
        assert!(parts.iter().all(|s| s.encode_utf16().count() <= 256));
        assert_eq!(parts.concat(), text);
        assert_eq!(split("/command"), ["/command"]);
    }
}
