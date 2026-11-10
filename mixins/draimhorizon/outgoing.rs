// Translation changes text only. The native adapter sends it as signed chat, never a command.
use claim_borders_relay_client::translation::{Language, SourcePolicy, TargetLanguage};
use std::collections::VecDeque;
pub const WAIT_MS: u64 = 15_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationTargets {
    pub incoming: TargetLanguage,
    pub outgoing: TargetLanguage,
    pub input: Option<TargetLanguage>,
    /// Language received chat is translated from; None lets the provider detect it.
    pub chat_source: Option<TargetLanguage>,
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
            chat_source: None,
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
            source: if outgoing {
                self.input
            } else {
                self.chat_source
            },
            common_only: self.common_only,
        }
    }
    pub fn encode(self) -> String {
        serde_json::json!({"schema":1,"input":self.input.map(TargetLanguage::code),"output":self.outgoing.code(),"received":self.incoming.code(),"chat_source":self.chat_source.map(TargetLanguage::code),"common_only":self.common_only}).to_string()
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
            chat_source: if v["chat_source"].is_null() {
                None
            } else {
                Some(lang("chat_source")?)
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
    ChatSource,
}
impl TranslationField {
    pub fn hint(self, ui: Language) -> &'static str {
        match self {
            Self::Input => ui.text(
                "Язык текста, который вы пишете. Можно определять автоматически.",
                "The language you type in. Can be detected automatically.",
            ),
            Self::Output => ui.text(
                "На этот язык переводятся ваши отправляемые сообщения.",
                "Your outgoing messages are translated into this language.",
            ),
            Self::Received => ui.text(
                "На этот язык переводятся входящие сообщения.",
                "Incoming messages are translated into this language.",
            ),
            Self::ChatSource => ui.text(
                "Исходный язык входящих сообщений. Обычно лучше выбрать автоопределение.",
                "The original language of incoming messages. Auto detect is usually best.",
            ),
        }
    }
    /// Choices that may be left on provider auto-detection.
    pub fn allows_auto(self) -> bool {
        matches!(self, Self::Input | Self::ChatSource)
    }
    pub fn next(self) -> Self {
        match self {
            Self::Input => Self::Output,
            Self::Output => Self::Received,
            Self::Received => Self::ChatSource,
            Self::ChatSource => Self::Input,
        }
    }
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
/// Text drawn around the controls: the panel, group headings and the arrows between the
/// language a message is written in and the language it is shown in.
pub struct TranslationChrome {
    pub panel: [f32; 4],
    pub compact: bool,
    pub headings: Vec<([f32; 2], String)>,
    pub arrows: Vec<[f32; 4]>,
}
/// Shared geometry for layout, drawing and hit-testing.
struct Plan {
    panel: [f32; 4],
    compact: bool,
    fields: [[f32; 4]; 4],
    groups: [[f32; 2]; 3],
    arrows: [[f32; 4]; 2],
    grid: [f32; 2],
    width: f32,
    columns: usize,
    rows: usize,
    common: [f32; 4],
    pager: Option<f32>,
    footer: f32,
}
const FIELDS: [TranslationField; 4] = [
    TranslationField::Input,
    TranslationField::Output,
    TranslationField::ChatSource,
    TranslationField::Received,
];
impl TranslationScreen {
    pub fn new(draft: TranslationTargets) -> Self {
        Self {
            draft,
            field: TranslationField::Input,
            page: 0,
        }
    }
    fn choices(&self) -> usize {
        TargetLanguage::COMMON.len() + usize::from(self.field.allows_auto())
    }
    fn plan(&self, size: [f32; 2]) -> Plan {
        let width = (size[0] - 16.0).clamp(1.0, 520.0);
        let x = (size[0] - width) * 0.5;
        let compact = size[1] < 300.0;
        let wide = width >= 400.0;
        let columns = if width >= 360.0 { 3 } else { 2 };
        let inner = width - 24.0;
        let header = if compact { 24.0 } else { 44.0 };
        // Field cards: your messages (written in -> sent in), then chat (from -> shown in).
        let card_h = if compact { 26.0 } else { 32.0 };
        let heading = if compact { 0.0 } else { 14.0 };
        let groups_h = if wide || compact && width >= 300.0 {
            heading + card_h
        } else {
            2.0 * (heading + card_h) + 6.0
        };
        let grid_heading = if compact { 0.0 } else { 16.0 };
        let fixed = header + groups_h + 10.0 + grid_heading + 26.0 + 30.0 + 10.0;
        let needed = self.choices().div_ceil(columns);
        let room =
            |extra: f32| ((size[1] - 16.0 - fixed - extra).max(22.0) / 22.0).floor() as usize;
        let (rows, paged) = if room(0.0) >= needed {
            (needed, false)
        } else {
            (room(26.0).max(1), true)
        };
        let height = fixed + rows as f32 * 22.0 + if paged { 26.0 } else { 0.0 };
        let y = ((size[1] - height) * 0.5).max(4.0);
        let left = x + 12.0;
        let mut top = y + header;
        let mut fields = [[0.0; 4]; 4];
        let mut groups = [[0.0; 2]; 3];
        let mut arrows = [[0.0; 4]; 2];
        let gap = 18.0;
        if wide || compact && width >= 300.0 {
            // Two groups side by side, each "from -> to".
            let group_w = (inner - 12.0) * 0.5;
            let card_w = (group_w - gap) * 0.5;
            for g in 0..2 {
                let gx = left + g as f32 * (group_w + 12.0);
                groups[g] = [gx, top];
                for c in 0..2 {
                    fields[g * 2 + c] = [
                        gx + c as f32 * (card_w + gap),
                        top + heading,
                        card_w,
                        card_h,
                    ];
                }
                arrows[g] = [
                    gx + card_w + 4.0,
                    top + heading + (card_h - 10.0) * 0.5,
                    10.0,
                    10.0,
                ];
            }
            top += heading + card_h;
        } else {
            let card_w = (inner - gap) * 0.5;
            for g in 0..2 {
                groups[g] = [left, top];
                for c in 0..2 {
                    fields[g * 2 + c] = [
                        left + c as f32 * (card_w + gap),
                        top + heading,
                        card_w,
                        card_h,
                    ];
                }
                arrows[g] = [
                    left + card_w + 4.0,
                    top + heading + (card_h - 10.0) * 0.5,
                    10.0,
                    10.0,
                ];
                top += heading + card_h + 6.0;
            }
            top -= 6.0;
        }
        top += 10.0;
        groups[2] = [left, top];
        top += grid_heading;
        let grid = [left, top];
        top += rows as f32 * 22.0 + 4.0;
        let pager = paged.then(|| {
            let at = top;
            top += 26.0;
            at
        });
        let common = [left, top, inner, 20.0];
        top += 26.0;
        Plan {
            panel: [x, y, width, height],
            compact,
            fields,
            groups,
            arrows,
            grid,
            width: inner,
            columns,
            rows,
            common,
            pager,
            footer: top,
        }
    }
    fn capacity(&self, size: [f32; 2]) -> usize {
        let plan = self.plan(size);
        plan.rows * plan.columns
    }
    pub fn pages(&self, size: [f32; 2]) -> usize {
        self.choices().div_ceil(self.capacity(size)).max(1)
    }
    pub fn apply(&mut self, control: TranslationControl, size: [f32; 2]) {
        match control {
            TranslationControl::Field(field) => {
                self.field = field;
                self.page = 0;
            }
            TranslationControl::Choose(lang) => match self.field {
                TranslationField::Input => self.draft.input = lang,
                TranslationField::ChatSource => self.draft.chat_source = lang,
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
    fn field_label(field: TranslationField, ui: Language) -> &'static str {
        match field {
            TranslationField::Input => ui.text("Пишу на", "I write in"),
            TranslationField::Output => ui.text("Отправлять на", "Send as"),
            TranslationField::ChatSource => ui.text("Чат на", "Chat is in"),
            TranslationField::Received => ui.text("Показывать на", "Show as"),
        }
    }
    pub fn chrome(&self, size: [f32; 2], ui: Language) -> TranslationChrome {
        let plan = self.plan(size);
        let mut headings = Vec::new();
        if !plan.compact {
            headings.push((
                plan.groups[0],
                ui.text("Мои сообщения", "My messages").to_owned(),
            ));
            headings.push((
                plan.groups[1],
                ui.text("Сообщения чата", "Chat messages").to_owned(),
            ));
            headings.push((
                plan.groups[2],
                match ui {
                    Language::Russian => format!(
                        "Выберите язык: {}",
                        Self::field_label(self.field, ui).to_lowercase()
                    ),
                    Language::English => {
                        format!("Choose a language: {}", Self::field_label(self.field, ui))
                    }
                },
            ));
        }
        TranslationChrome {
            panel: plan.panel,
            compact: plan.compact,
            headings,
            arrows: plan.arrows.to_vec(),
        }
    }
    pub fn layout(&self, size: [f32; 2], ui: Language) -> Vec<TranslationWidget> {
        let plan = self.plan(size);
        let mut widgets = Vec::new();
        let mut add = |rect, text: String, selected, control| {
            widgets.push(TranslationWidget {
                rect,
                text,
                selected,
                control,
            })
        };
        for (field, rect) in FIELDS.into_iter().zip(plan.fields) {
            let language = match field {
                TranslationField::Input => self.draft.input,
                TranslationField::Output => Some(self.draft.outgoing),
                TranslationField::Received => Some(self.draft.incoming),
                TranslationField::ChatSource => self.draft.chat_source,
            };
            add(
                rect,
                format!(
                    "{}\n{}",
                    Self::field_label(field, ui),
                    language.map_or(ui.text("Авто", "Auto detect"), |l| l.name(ui))
                ),
                self.field == field,
                TranslationControl::Field(field),
            );
        }
        let languages: Vec<_> = std::iter::once(None)
            .filter(|_| self.field.allows_auto())
            .chain(TargetLanguage::COMMON.into_iter().map(Some))
            .collect();
        let selected = match self.field {
            TranslationField::Input => self.draft.input,
            TranslationField::Output => Some(self.draft.outgoing),
            TranslationField::Received => Some(self.draft.incoming),
            TranslationField::ChatSource => self.draft.chat_source,
        };
        let capacity = plan.rows * plan.columns;
        let pages = languages.len().div_ceil(capacity).max(1);
        let page = self.page.min(pages - 1);
        let column_w = (plan.width - 4.0 * (plan.columns - 1) as f32) / plan.columns as f32;
        for (i, lang) in languages
            .into_iter()
            .skip(page * capacity)
            .take(capacity)
            .enumerate()
        {
            add(
                [
                    plan.grid[0] + (i % plan.columns) as f32 * (column_w + 4.0),
                    plan.grid[1] + (i / plan.columns) as f32 * 22.0,
                    column_w,
                    20.0,
                ],
                lang.map_or(ui.text("Автоопределение", "Auto detect"), |l| l.name(ui))
                    .into(),
                selected == lang,
                TranslationControl::Choose(lang),
            );
        }
        let [left, _, inner, _] = plan.common;
        if let Some(y) = plan.pager {
            add(
                [left, y, 30.0, 20.0],
                "<".into(),
                false,
                TranslationControl::Previous,
            );
            add(
                [left + inner - 30.0, y, 30.0, 20.0],
                ">".into(),
                false,
                TranslationControl::Next,
            );
        }
        add(
            plan.common,
            format!(
                "{}: {}",
                ui.text(
                    "Переводить чат только с основных языков",
                    "Translate chat only from common languages"
                ),
                if self.draft.common_only {
                    ui.text("вкл.", "on")
                } else {
                    ui.text("выкл.", "off")
                }
            ),
            false,
            TranslationControl::CommonOnly,
        );
        let half = (inner - 6.0) * 0.5;
        add(
            [left, plan.footer, half, 22.0],
            ui.text("Отмена", "Cancel").into(),
            false,
            TranslationControl::Cancel,
        );
        add(
            [left + half + 6.0, plan.footer, half, 22.0],
            ui.text("Сохранить", "Save").into(),
            false,
            TranslationControl::Save,
        );
        widgets
    }
    /// The page indicator between the paging arrows, when the list does not fit.
    pub fn page_label(&self, size: [f32; 2]) -> Option<([f32; 4], String)> {
        let plan = self.plan(size);
        let y = plan.pager?;
        let pages = self.pages(size);
        Some((
            [plan.common[0] + 34.0, y, plan.common[2] - 68.0, 20.0],
            format!("{} / {}", self.page.min(pages - 1) + 1, pages),
        ))
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
