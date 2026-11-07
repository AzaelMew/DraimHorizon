// Translation changes text only. The native adapter sends it as signed chat, never a command.
use claim_borders_relay_client::translation::{Language, SourcePolicy, TargetLanguage};
use std::collections::VecDeque;
pub const WAIT_MS: u64 = 15_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationTargets {
    pub incoming: TargetLanguage,
    pub outgoing: TargetLanguage,
    pub input: Option<TargetLanguage>,
    pub common_only: bool,
}
impl TranslationTargets {
    /// First run after an update (no saved preferences): reproduce the pre-update behaviour.
    /// You wrote in the UI language, sent the other one, and read chat in the UI language.
    /// Input is an explicit language, never Auto, so the button does not start as `?`.
    pub fn load(incoming: Option<&str>, outgoing: Option<&str>, ui: Language) -> Self {
        Self {
            incoming: incoming
                .and_then(TargetLanguage::from_code)
                .unwrap_or(ui.into()),
            outgoing: outgoing
                .and_then(TargetLanguage::from_code)
                .unwrap_or(ui.other().into()),
            input: Some(ui.into()),
            common_only: true,
        }
    }
    pub fn label(self) -> String {
        format!(
            "{}>{}",
            self.input.map_or("?".into(), TargetLanguage::tag),
            self.outgoing.tag()
        )
    }
    pub fn policy(self, outgoing: bool) -> SourcePolicy {
        SourcePolicy {
            source: if outgoing { self.input } else { None },
            common_only: self.common_only,
        }
    }
    pub fn encode(self) -> String {
        serde_json::json!({"schema":1,"input":self.input.map(TargetLanguage::code),"output":self.outgoing.code(),"received":self.incoming.code(),"common_only":self.common_only}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 2048 {
            return Err("Translation settings too large");
        }
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|_| "Invalid translation settings")?;
        if v["schema"] != 1 {
            return Err("Unknown translation settings version");
        }
        let lang = |key: &str| {
            v[key]
                .as_str()
                .and_then(TargetLanguage::from_code)
                .ok_or("Invalid translation language")
        };
        Ok(Self {
            input: if v["input"].is_null() {
                None
            } else {
                Some(lang("input")?)
            },
            outgoing: lang("output")?,
            incoming: lang("received")?,
            common_only: v["common_only"].as_bool().ok_or("Invalid source filter")?,
        })
    }
    pub fn save(self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        match std::fs::read_to_string(path) {
            Ok(raw) => {
                Self::decode(&raw).map_err(str::to_owned)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let dir = path.parent().ok_or("Missing settings directory")?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let pending = path.with_extension(format!("{}.pending", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(self.encode().as_bytes())?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&pending, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }
}
pub fn translation_button(size: [f32; 2]) -> [f32; 4] {
    [2.0, size[1] - 14.0, 39.0, 12.0]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationField {
    Input,
    Output,
    Received,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationControl {
    Field(TranslationField),
    Choose(Option<TargetLanguage>),
    CommonOnly,
    Previous,
    Next,
    Save,
    Cancel,
}
pub struct TranslationWidget {
    pub rect: [f32; 4],
    pub text: String,
    pub selected: bool,
    pub control: TranslationControl,
}
pub struct TranslationScreen {
    pub draft: TranslationTargets,
    pub field: TranslationField,
    pub page: usize,
}
impl TranslationScreen {
    pub fn new(draft: TranslationTargets) -> Self {
        Self {
            draft,
            field: TranslationField::Input,
            page: 0,
        }
    }
    fn capacity(size: [f32; 2]) -> usize {
        let columns = if size[0] < 420.0 { 2 } else { 3 };
        ((size[1] - 132.0).max(22.0) / 22.0).floor() as usize * columns
    }
    pub fn pages(&self, size: [f32; 2]) -> usize {
        (20_usize + usize::from(self.field == TranslationField::Input))
            .div_ceil(Self::capacity(size))
    }
    pub fn apply(&mut self, control: TranslationControl, size: [f32; 2]) {
        match control {
            TranslationControl::Field(field) => {
                self.field = field;
                self.page = 0;
            }
            TranslationControl::Choose(lang) => match self.field {
                TranslationField::Input => self.draft.input = lang,
                TranslationField::Output => {
                    if let Some(lang) = lang {
                        self.draft.outgoing = lang;
                    }
                }
                TranslationField::Received => {
                    if let Some(lang) = lang {
                        self.draft.incoming = lang;
                    }
                }
            },
            TranslationControl::CommonOnly => self.draft.common_only = !self.draft.common_only,
            TranslationControl::Previous => self.page = self.page.saturating_sub(1),
            TranslationControl::Next => self.page = (self.page + 1).min(self.pages(size) - 1),
            _ => {}
        }
    }
    pub fn hit(&self, size: [f32; 2], ui: Language, mouse: [f32; 2]) -> Option<TranslationControl> {
        self.layout(size, ui)
            .into_iter()
            .find(|w| {
                mouse[0] >= w.rect[0]
                    && mouse[1] >= w.rect[1]
                    && mouse[0] < w.rect[0] + w.rect[2]
                    && mouse[1] < w.rect[1] + w.rect[3]
            })
            .map(|w| w.control)
    }
    pub fn layout(&self, size: [f32; 2], ui: Language) -> Vec<TranslationWidget> {
        let width = (size[0] - 16.0).min(620.0);
        let x = (size[0] - width) * 0.5;
        let mut widgets = Vec::new();
        let mut add = |rect, text: String, selected, control| {
            widgets.push(TranslationWidget {
                rect,
                text,
                selected,
                control,
            })
        };
        for (i, field) in [
            TranslationField::Input,
            TranslationField::Output,
            TranslationField::Received,
        ]
        .into_iter()
        .enumerate()
        {
            let (label, language) = match field {
                TranslationField::Input => (ui.text("Ввод", "Input"), self.draft.input),
                TranslationField::Output => {
                    (ui.text("Отправка", "Output"), Some(self.draft.outgoing))
                }
                TranslationField::Received => (
                    ui.text("Входящий чат", "Received chat"),
                    Some(self.draft.incoming),
                ),
            };
            add(
                [
                    x + i as f32 * (width + 4.0) / 3.0,
                    34.0,
                    (width - 8.0) / 3.0,
                    30.0,
                ],
                format!(
                    "{label}\n{}",
                    language.map_or(ui.text("Авто", "Auto detect"), |l| l.name(ui))
                ),
                self.field == field,
                TranslationControl::Field(field),
            );
        }
        add(
            [x, 70.0, width, 18.0],
            ui.text(
                "Автоперевод: только 20 основных языков",
                "Auto-translate: common languages only",
            )
            .into(),
            self.draft.common_only,
            TranslationControl::CommonOnly,
        );
        let columns = if size[0] < 420.0 { 2 } else { 3 };
        let languages: Vec<_> = std::iter::once(None)
            .filter(|_| self.field == TranslationField::Input)
            .chain(TargetLanguage::COMMON.into_iter().map(Some))
            .collect();
        let selected = match self.field {
            TranslationField::Input => self.draft.input,
            TranslationField::Output => Some(self.draft.outgoing),
            TranslationField::Received => Some(self.draft.incoming),
        };
        for (i, lang) in languages
            .into_iter()
            .skip(self.page.min(self.pages(size) - 1) * Self::capacity(size))
            .take(Self::capacity(size))
            .enumerate()
        {
            let colwidth = (width - 4.0 * (columns - 1) as f32) / columns as f32;
            add(
                [
                    x + (i % columns) as f32 * (colwidth + 4.0),
                    96.0 + (i / columns) as f32 * 22.0,
                    colwidth,
                    20.0,
                ],
                lang.map_or(ui.text("Автоопределение", "Auto detect"), |l| l.name(ui))
                    .into(),
                selected == lang,
                TranslationControl::Choose(lang),
            );
        }
        let y = size[1] - 28.0;
        add(
            [x, y, 24.0, 20.0],
            "<".into(),
            false,
            TranslationControl::Previous,
        );
        add(
            [x + 28.0, y, 44.0, 20.0],
            format!(
                "{}/{} >",
                self.page.min(self.pages(size) - 1) + 1,
                self.pages(size)
            ),
            false,
            TranslationControl::Next,
        );
        add(
            [x + width - 166.0, y, 80.0, 20.0],
            ui.text("Отмена", "Cancel").into(),
            false,
            TranslationControl::Cancel,
        );
        add(
            [x + width - 80.0, y, 80.0, 20.0],
            ui.text("Сохранить", "Save").into(),
            true,
            TranslationControl::Save,
        );
        widgets
    }
}
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
    fn translation_controls_have_clean_labels_and_independent_targets() {
        let targets = TranslationTargets::load(None, None, Language::English);
        assert_eq!(targets.incoming, TargetLanguage::English);
        assert_eq!(targets.outgoing, TargetLanguage::Russian);
        assert_eq!(targets.label(), "EN>RU");
        let targets = TranslationTargets::load(Some("ru"), Some("en"), Language::English);
        assert_eq!(targets.label(), "EN>EN");
        assert_eq!(targets.incoming, TargetLanguage::Russian);
        assert_eq!(targets.outgoing, TargetLanguage::English);
        assert_eq!(
            TranslationTargets::load(None, None, Language::Russian).label(),
            "RU>EN"
        );
    }
    #[test]
    fn translation_choices_fit_small_chat_and_share_hit_geometry() {
        for size in [[320.0, 180.0], [854.0, 480.0]] {
            let menu =
                TranslationScreen::new(TranslationTargets::load(None, None, Language::English));
            for widget in menu.layout(size, Language::English) {
                let rect = widget.rect;
                assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
                assert!(rect[0] + rect[2] <= size[0]);
                assert!(rect[1] + rect[3] <= size[1]);
                assert_eq!(
                    menu.hit(size, Language::English, [rect[0] + 1.0, rect[1] + 1.0]),
                    Some(widget.control)
                );
            }
            assert_eq!(
                menu.hit(size, Language::English, [size[0] - 1.0, 1.0]),
                None
            );
        }
    }
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
