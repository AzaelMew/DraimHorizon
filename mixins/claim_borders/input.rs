// Bounded UTF-8 editing and inventory-search geometry, independent of the game.
#[derive(Clone, Debug, Default)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
    pub anchor: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Handled,
    Ignored,
    Copy(String),
    Paste,
}
impl TextInput {
    pub fn new(value: String) -> Self {
        Self {
            cursor: value.len(),
            value,
            anchor: None,
        }
    }
    pub fn selection(&self) -> std::ops::Range<usize> {
        let a = self.anchor.unwrap_or(self.cursor);
        a.min(self.cursor)..a.max(self.cursor)
    }
    pub fn insert(&mut self, text: &str, limit: usize) {
        let range = self.selection();
        let count = self.value.chars().count() - self.value[range.clone()].chars().count();
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take(limit.saturating_sub(count))
            .collect();
        self.value.replace_range(range.clone(), &text);
        self.cursor = range.start + text.len();
        self.anchor = None;
    }
    pub fn key(&mut self, key: &str, ctrl: bool, shift: bool) -> Edit {
        if ctrl {
            match key {
                "key.keyboard.a" => {
                    self.anchor = Some(0);
                    self.cursor = self.value.len();
                    return Edit::Handled;
                }
                "key.keyboard.c" => return Edit::Copy(self.value[self.selection()].into()),
                "key.keyboard.x" => {
                    let text = self.value[self.selection()].to_owned();
                    self.insert("", usize::MAX);
                    return Edit::Copy(text);
                }
                "key.keyboard.v" => return Edit::Paste,
                _ => {}
            }
        }
        let prev = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        let next = self.value[self.cursor..]
            .chars()
            .next()
            .map_or(self.value.len(), |c| self.cursor + c.len_utf8());
        match key {
            "key.keyboard.left" | "key.keyboard.right" | "key.keyboard.home"
            | "key.keyboard.end" => {
                let to = match key {
                    "key.keyboard.left" => prev,
                    "key.keyboard.right" => next,
                    "key.keyboard.home" => 0,
                    _ => self.value.len(),
                };
                if shift {
                    if self.anchor.is_none() {
                        self.anchor = Some(self.cursor);
                    }
                } else {
                    self.anchor = None;
                }
                self.cursor = to;
                Edit::Handled
            }
            "key.keyboard.backspace" | "key.keyboard.delete" => {
                if self.selection().is_empty() {
                    self.anchor = Some(if key.ends_with("backspace") {
                        prev
                    } else {
                        next
                    });
                }
                self.insert("", usize::MAX);
                Edit::Handled
            }
            _ => Edit::Ignored,
        }
    }
}
pub fn matches(query: &str, text: &str) -> bool {
    let text = text.to_lowercase().replace('_', " ");
    query
        .to_lowercase()
        .replace('_', " ")
        .split_whitespace()
        .all(|word| text.contains(word))
}
pub fn search_rect(panel: [f32; 4], size: [f32; 2]) -> Option<[f32; 4]> {
    if !panel.iter().chain(&size).all(|v| v.is_finite()) {
        return None;
    }
    let [x, y, w, h] = panel;
    if y + h + 22.0 <= size[1] {
        return Some([x, y + h + 4., w, 17.]);
    }
    if y >= 22.0 {
        return Some([x, y - 21., w, 17.]);
    }
    let right = size[0] - x - w - 8.;
    if right >= 56.0 {
        return Some([x + w + 4., y + h - 18., right.min(176.), 17.]);
    }
    let left = x - 8.;
    if left >= 56.0 {
        let width = left.min(176.);
        return Some([x - width - 4., y + h - 18., width, 17.]);
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_editing_selection_and_capacity() {
        let mut t = TextInput::new("Дом у реки".into());
        t.key("key.keyboard.home", false, false);
        t.key("key.keyboard.right", false, true);
        t.insert("Л", 48);
        assert_eq!(t.value, "Лом у реки");
        t.key("key.keyboard.a", true, false);
        t.insert("Привет🌍", 6);
        assert_eq!(t.value, "Привет");
        t.key("key.keyboard.backspace", false, false);
        assert_eq!(t.value, "Приве");
        assert!(t.value.is_char_boundary(t.cursor));
    }
    #[test]
    fn multilingual_search() {
        let names = "minecraft:diamond_sword Diamond Sword Алмазный меч";
        for q in ["алмаз меч", "DIAMOND sword", "minecraft:diamond", "алм", ""] {
            assert!(matches(q, names));
        }
        assert!(!matches("iron", names));
        assert!(!matches("алмаз лопата", names));
    }
    #[test]
    fn search_field_stays_outside_slots_and_inside_window() {
        for (panel, size) in [
            ([72., 9., 176., 222.], [320., 240.]),
            ([72., 37., 176., 166.], [320., 240.]),
            ([300., 80., 176., 222.], [854., 480.]),
        ] {
            let r = search_rect(panel, size).unwrap();
            assert!(r[0] >= 0. && r[1] >= 0. && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]);
            assert!(
                r[0] >= panel[0] + panel[2]
                    || r[0] + r[2] <= panel[0]
                    || r[1] >= panel[1] + panel[3]
                    || r[1] + r[3] <= panel[1]
            );
        }
    }
}
