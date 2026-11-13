// Exact shortcuts and bounded caches for text that is measured and wrapped every frame.
use std::{borrow::Cow, collections::HashMap, sync::Arc};

/// True when no prefix of `text` can measure wider than `text` itself.
///
/// The client measures a string as its reordered glyph advances, rounded up. Without right-to-left
/// text, bidi controls or legacy `§` formatting nothing is reordered, so a prefix is a prefix of the
/// glyph sequence and widths only grow. Anything else must be measured prefix by prefix.
pub fn prefixes_never_wider(text: &str) -> bool {
    text.chars().all(|c| {
        !matches!(c,
            '§'
            | '\u{0590}'..='\u{08ff}'
            | '\u{200e}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{fb1d}'..='\u{fdff}'
            | '\u{fe70}'..='\u{feff}'
            | '\u{10800}'..='\u{10fff}'
            | '\u{1e800}'..='\u{1efff}')
    })
}

/// `text` cut with an ellipsis once a prefix is wider than `limit` (sidebar labels).
pub fn clip_label<'a>(text: &'a str, limit: f32, measure: impl Fn(&str) -> f32) -> Cow<'a, str> {
    if prefixes_never_wider(text) && measure(text) <= limit {
        return Cow::Borrowed(text);
    }
    let mut shown = String::new();
    for c in text.chars() {
        shown.push(c);
        if measure(&shown) > limit {
            shown.pop();
            shown.push('…');
            break;
        }
    }
    Cow::Owned(shown)
}

/// Clipped sidebar labels, keyed by font identity, width and exact text. Dynamic values (FPS, TPS,
/// balance) are drawn without it, so it only ever holds the few labels a card shows.
type Label = (String, Arc<str>);
#[derive(Default)]
pub struct LabelCache {
    labels: HashMap<(u64, u64, u32), Vec<Label>>,
    count: usize,
}
const MAX_LABELS: usize = 128;
impl LabelCache {
    pub fn clip(
        &mut self,
        font: (u64, u64),
        text: &str,
        limit: f32,
        measure: impl Fn(&str) -> f32,
    ) -> Arc<str> {
        let key = (font.0, font.1, limit.to_bits());
        if let Some((_, shown)) = self
            .labels
            .get(&key)
            .and_then(|b| b.iter().find(|(t, _)| t == text))
        {
            return shown.clone();
        }
        let shown: Arc<str> = clip_label(text, limit, measure).as_ref().into();
        if self.count >= MAX_LABELS {
            self.labels.clear();
            self.count = 0;
        }
        self.labels
            .entry(key)
            .or_default()
            .push((text.to_owned(), shown.clone()));
        self.count += 1;
        shown
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.count
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn growing(text: &str, limit: f32, measure: impl Fn(&str) -> f32) -> String {
        let mut shown = String::new();
        for c in text.chars() {
            shown.push(c);
            if measure(&shown) > limit {
                shown.pop();
                shown.push('…');
                break;
            }
        }
        shown
    }

    #[test]
    fn label_clipping_matches_the_growing_prefix_loop() {
        // Per-script advances rounded up like the client's measurement.
        let measure = |s: &str| {
            s.chars()
                .map(|c| if c.is_ascii() { 5.5 } else { 6.25 })
                .sum::<f32>()
                .ceil()
        };
        for text in [
            "",
            "FPS",
            "Баланс",
            "Playtime · 12h",
            "ClaimBorders-Горизонт",
            "日本語テキスト",
        ] {
            for limit in [0.0, 6.0, 20.0, 41.0, 200.0] {
                assert_eq!(
                    clip_label(text, limit, measure),
                    growing(text, limit, measure)
                );
            }
        }
    }

    #[test]
    fn a_label_that_fits_is_measured_once_and_borrowed() {
        let calls = Cell::new(0);
        let measure = |s: &str| {
            calls.set(calls.get() + 1);
            s.chars().count() as f32
        };
        let shown = clip_label("Online players", 40.0, measure);
        assert!(matches!(shown, Cow::Borrowed("Online players")));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn right_to_left_and_formatted_text_keeps_prefix_measurement() {
        for text in ["שלום עולם", "mix عربى text", "§aGreen", "a\u{202e}b"] {
            assert!(!prefixes_never_wider(text), "{text}");
        }
        // A measurement where the whole is narrower than a prefix, as reordering can produce.
        let measure = |s: &str| {
            if s.chars().count() == 4 {
                2.0
            } else {
                s.chars().count() as f32 * 3.0
            }
        };
        assert_eq!(
            clip_label("שלום", 7.0, measure),
            growing("שלום", 7.0, measure)
        );
        assert_eq!(clip_label("שלום", 7.0, measure), "של…");
    }

    #[test]
    fn clipped_labels_are_reused_until_text_width_or_font_changes() {
        let calls = Cell::new(0);
        let narrow = |s: &str| {
            calls.set(calls.get() + 1);
            s.chars().count() as f32 * 4.0
        };
        let wide = |s: &str| s.chars().count() as f32 * 9.0;
        let mut cache = LabelCache::default();
        let font = (1, 5);
        let first = cache.clip(font, "A long richest player name", 60.0, narrow);
        assert!(first.ends_with('…'));
        let measured = calls.get();
        assert!(Arc::ptr_eq(
            &first,
            &cache.clip(font, "A long richest player name", 60.0, narrow)
        ));
        assert_eq!(calls.get(), measured, "a hit measures nothing");
        // Another font revision (same epoch) with other widths, width and text each re-clip.
        let rebuilt = cache.clip((2, 5), "A long richest player name", 60.0, wide);
        assert_ne!(first, rebuilt);
        assert_ne!(
            first,
            cache.clip(font, "A long richest player name", 160.0, narrow)
        );
        assert_eq!(&*cache.clip(font, "Online", 160.0, narrow), "Online");
        // Many distinct labels cannot grow the cache without bound.
        for i in 0..1_000 {
            cache.clip(font, &format!("name {i}"), 60.0, narrow);
            assert!(cache.len() <= MAX_LABELS);
        }
    }
}
