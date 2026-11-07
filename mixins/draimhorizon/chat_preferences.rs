use claim_borders_relay_client::translation::Language;
// Shared chat preferences, geometry and hit testing, independent of the game.
use serde_json::{Value, json};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    #[default]
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}
impl Placement {
    pub fn next(self) -> Self {
        match self {
            Self::BottomLeft => Self::BottomRight,
            Self::BottomRight => Self::TopRight,
            Self::TopRight => Self::TopLeft,
            Self::TopLeft => Self::BottomLeft,
        }
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::BottomLeft => language.text("Слева снизу", "Bottom left"),
            Self::BottomRight => language.text("Справа снизу", "Bottom right"),
            Self::TopLeft => language.text("Слева сверху", "Top left"),
            Self::TopRight => language.text("Справа сверху", "Top right"),
        }
    }
    pub fn top(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }
    fn right(self) -> bool {
        matches!(self, Self::BottomRight | Self::TopRight)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationStyle {
    #[default]
    Cards,
    Compact,
    Inline,
    Off,
}
impl NotificationStyle {
    pub fn next(self) -> Self {
        match self {
            Self::Cards => Self::Compact,
            Self::Compact => Self::Inline,
            Self::Inline => Self::Off,
            Self::Off => Self::Cards,
        }
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Cards => language.text("Карточки рядом", "Side cards"),
            Self::Compact => language.text("Компактные рядом", "Compact cards"),
            Self::Inline => language.text("В ленте чата", "In chat"),
            Self::Off => language.text("Выключены", "Off"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    Consecutive,
    #[default]
    Recent,
}
impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Consecutive,
            Self::Consecutive => Self::Recent,
            Self::Recent => Self::Off,
        }
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Off => language.text("Выключен", "Off"),
            Self::Consecutive => language.text("Повторы подряд", "Consecutive repeats"),
            Self::Recent => language.text("Повторы за интервал", "Recent repeats"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub width: u16,
    pub height: u16,
    pub scale: f32,
    pub compact_messages: bool,
    pub placement: Placement,
    pub offset_x: i16,
    pub offset_y: i16,
    pub notifications: NotificationStyle,
    pub presence: bool,
    pub deaths: bool,
    pub duration: u16,
    pub opacity: f32,
    pub event_width: u16,
    pub repeats: RepeatMode,
    pub repeat_window: u16,
    pub ignore_prefix: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            width: 320,
            height: 180,
            scale: 1.0,
            compact_messages: false,
            placement: Placement::BottomLeft,
            offset_x: 0,
            offset_y: 0,
            notifications: NotificationStyle::Cards,
            presence: true,
            deaths: true,
            duration: 12,
            opacity: 0.86,
            event_width: 170,
            repeats: RepeatMode::Recent,
            repeat_window: 10,
            ignore_prefix: 0,
        }
    }
}
impl Preferences {
    pub fn normalized(mut self) -> Self {
        self.width = self.width.clamp(140, 600);
        self.height = self.height.clamp(80, 360);
        self.scale = if self.scale.is_finite() {
            self.scale.clamp(0.65, 1.5)
        } else {
            1.0
        };
        self.opacity = if self.opacity.is_finite() {
            self.opacity.clamp(0.1, 1.0)
        } else {
            0.86
        };
        self.offset_x = self.offset_x.clamp(-2048, 2048);
        self.offset_y = self.offset_y.clamp(-2048, 2048);
        self.duration = self.duration.clamp(3, 60);
        self.event_width = self.event_width.clamp(110, 280);
        self.repeat_window = self.repeat_window.clamp(2, 120);
        self.ignore_prefix = self.ignore_prefix.min(64);
        self
    }
    pub fn encode(self) -> String {
        let p = self.normalized();
        json!({"schema":1,"width":p.width,"height":p.height,"scale":p.scale,"compact_messages":p.compact_messages,"placement":p.placement as u8,
            "offset_x":p.offset_x,"offset_y":p.offset_y,"notifications":p.notifications as u8,
            "presence":p.presence,"deaths":p.deaths,"duration":p.duration,"opacity":p.opacity,
            "event_width":p.event_width,"repeats":p.repeats as u8,"repeat_window":p.repeat_window,"ignore_prefix":p.ignore_prefix}).to_string()
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > 4096 {
            return Err("chat preferences too large".into());
        }
        let v: Value = serde_json::from_str(text).map_err(|_| "invalid chat preferences")?;
        if v["schema"] != 1 {
            return Err("unsupported chat preferences".into());
        }
        let mut p = Self::default();
        let number = |key: &str, default: u16| {
            v[key]
                .as_u64()
                .unwrap_or(u64::from(default))
                .min(u64::from(u16::MAX)) as u16
        };
        p.width = number("width", p.width);
        p.height = number("height", p.height);
        p.duration = number("duration", p.duration);
        p.event_width = number("event_width", p.event_width);
        p.repeat_window = number("repeat_window", p.repeat_window);
        p.ignore_prefix = number("ignore_prefix", 0).min(64) as u8;
        p.scale = v["scale"].as_f64().unwrap_or(1.0) as f32;
        p.compact_messages = v["compact_messages"].as_bool().unwrap_or(false);
        p.opacity = v["opacity"].as_f64().unwrap_or(0.86) as f32;
        p.offset_x = v["offset_x"].as_i64().unwrap_or(0).clamp(-2048, 2048) as i16;
        p.offset_y = v["offset_y"].as_i64().unwrap_or(0).clamp(-2048, 2048) as i16;
        p.presence = v["presence"].as_bool().unwrap_or(true);
        p.deaths = v["deaths"].as_bool().unwrap_or(true);
        p.placement = match v["placement"].as_u64() {
            Some(1) => Placement::BottomRight,
            Some(2) => Placement::TopLeft,
            Some(3) => Placement::TopRight,
            _ => Placement::BottomLeft,
        };
        p.notifications = match v["notifications"].as_u64() {
            Some(1) => NotificationStyle::Compact,
            Some(2) => NotificationStyle::Inline,
            Some(3) => NotificationStyle::Off,
            _ => NotificationStyle::Cards,
        };
        p.repeats = match v["repeats"].as_u64() {
            Some(0) => RepeatMode::Off,
            Some(1) => RepeatMode::Consecutive,
            _ => RepeatMode::Recent,
        };
        Ok(p.normalized())
    }
}
// Shared by card painting and row fitting so compact hit boxes match the visuals.
#[derive(Clone, Copy, Debug)]
pub struct CardMetrics {
    pub gap: f32,
    pub text_y: f32,
    pub header_y: f32,
    pub body_y: f32,
    pub face_y: f32,
    pub face_size: i32,
    padding: f32,
    minimum: f32,
    summary: bool,
}
impl CardMetrics {
    pub fn new(dense: bool, inline: bool, summary: bool) -> Self {
        Self {
            gap: if dense { 1.0 } else { 4.0 },
            text_y: if dense { 3.0 } else { 6.0 },
            header_y: if dense { 3.0 } else { 4.0 },
            body_y: if dense { 13.0 } else { 15.0 },
            face_y: if dense { 2.0 } else { 5.0 },
            face_size: if dense { 10 } else { 12 },
            padding: match (dense, inline) {
                (true, true) => 4.0,
                (true, false) => 14.0,
                (false, true) => 10.0,
                (false, false) => 20.0,
            },
            minimum: match (dense, inline, summary) {
                (true, true, _) => 14.0,
                (true, false, _) => 24.0,
                (false, _, true) => 20.0,
                (false, true, false) => 22.0,
                (false, false, false) => 30.0,
            },
            summary,
        }
    }
    pub fn height(self, rows: usize) -> f32 {
        if self.summary {
            self.minimum
        } else {
            (self.padding + rows as f32 * 10.0).max(self.minimum)
        }
    }
    pub fn max_rows(self, available: f32) -> usize {
        if available < self.minimum {
            0
        } else if self.summary {
            1
        } else {
            ((available - self.padding) / 10.0).floor() as usize
        }
    }
}
pub fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] < rect[0] + rect[2]
        && point[1] < rect[1] + rect[3]
}
pub fn intersection(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let r = (a[0] + a[2]).min(b[0] + b[2]);
    let bottom = (a[1] + a[3]).min(b[1] + b[3]);
    (r > x && bottom > y).then_some([x, y, r - x, bottom - y])
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub id: u64,
    pub rect: [f32; 4],
}
pub fn hit_at(hits: &[Hit], point: [f32; 2]) -> Option<u64> {
    hits.iter()
        .rev()
        .find(|hit| contains(hit.rect, point))
        .map(|hit| hit.id)
}
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub main: [f32; 4],
    pub events: Option<[f32; 4]>,
    pub bounds: [f32; 4],
    pub scale: f32,
    pub top: bool,
}
impl Layout {
    pub fn new(p: Preferences, viewport: [f32; 2]) -> Self {
        let p = p.normalized();
        let scale = p.scale;
        let vw = viewport[0].max(32.0) / scale;
        let vh = viewport[1].max(64.0) / scale;
        let margin = 8.0 / scale;
        let avail = (vw - 2.0 * margin).max(1.0);
        let main_w = (p.width as f32).min(avail);
        let events = matches!(
            p.notifications,
            NotificationStyle::Cards | NotificationStyle::Compact
        ) && (p.presence || p.deaths);
        let side = events && (main_w + p.event_width as f32 + 6.0 <= avail);
        let event_w = if side { p.event_width as f32 } else { main_w };
        let w = if side { main_w + 6.0 + event_w } else { main_w };
        // Keep the input box clear even when the chat is anchored at the top.
        let h = (p.height as f32).min((vh - 50.0 / scale).max(1.0));
        let x = if p.placement.right() {
            vw - margin - w
        } else {
            margin
        };
        let y = if p.placement.top() {
            margin
        } else {
            vh - 42.0 / scale - h
        };
        let x = (x + p.offset_x as f32 / scale).clamp(margin, (vw - margin - w).max(margin));
        let y = (y + p.offset_y as f32 / scale).clamp(margin, (vh - 34.0 / scale - h).max(margin));
        let mut main = [x, y, main_w, h];
        let event_rect = if !events {
            None
        } else if side {
            Some([x + main_w + 6.0, y, event_w, h])
        } else {
            let event_h = (h * 0.32).max(1.0);
            main[3] = (h - event_h - 4.0).max(1.0);
            if p.placement.top() {
                Some([x, y + main[3] + 4.0, event_w, event_h])
            } else {
                main[1] = y + event_h + 4.0;
                Some([x, y, event_w, event_h])
            }
        };
        Self {
            main,
            events: event_rect,
            bounds: [x * scale, y * scale, w * scale, h * scale],
            scale,
            top: p.placement.top(),
        }
    }
    pub fn screen(self, r: [f32; 4]) -> [f32; 4] {
        r.map(|n| n * self.scale)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_cards_reduce_padding_and_gap_without_changing_line_height() {
        let normal = CardMetrics::new(false, true, false);
        let dense = CardMetrics::new(true, true, false);
        assert_eq!(normal.height(1) + normal.gap, 26.0);
        assert_eq!(dense.height(1) + dense.gap, 15.0);
        for rows in 1..=24 {
            assert_eq!(dense.height(rows + 1) - dense.height(rows), 10.0);
            assert!(dense.text_y + (rows - 1) as f32 * 10.0 + 8.0 <= dense.height(rows));
        }
        assert!(dense.face_y + dense.face_size as f32 <= dense.height(1));
        let system = CardMetrics::new(true, false, false);
        assert_eq!(system.height(1), 24.0);
        assert!(system.body_y + 8.0 <= system.height(1));
        let summary = CardMetrics::new(true, true, true);
        assert_eq!(summary.height(1), 14.0);
        assert_eq!(normal.max_rows(21.0), 0);
        assert_eq!(dense.max_rows(14.0), 1);
        assert_eq!(dense.max_rows(13.0), 0);
    }
    #[test]
    fn old_or_invalid_preferences_keep_normal_message_spacing() {
        for raw in [
            r#"{"schema":1}"#,
            r#"{"schema":1,"compact_messages":"true"}"#,
        ] {
            assert!(!Preferences::decode(raw).unwrap().compact_messages);
        }
        assert!(!Preferences::default().compact_messages);
    }
    #[test]
    fn compact_message_preference_survives_save_and_reload() {
        let p = Preferences::decode(r#"{"schema":1,"compact_messages":true}"#).unwrap();
        let saved: Value = serde_json::from_str(&p.encode()).unwrap();
        assert_eq!(saved["compact_messages"], true);
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
    }
    #[test]
    fn round_trip_and_corrupt_values_are_bounded() {
        let p = Preferences {
            placement: Placement::TopRight,
            notifications: NotificationStyle::Inline,
            offset_x: -32,
            scale: 0.8,
            ..Preferences::default()
        };
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        assert!(Preferences::decode("{}").is_err());
        let p = Preferences::decode(
            r#"{"schema":1,"width":999999,"scale":999,"height":0,"offset_x":-999999}"#,
        )
        .unwrap();
        assert_eq!(p.width, 600);
        assert_eq!(p.scale, 1.5);
        assert_eq!(p.height, 80);
        assert_eq!(p.offset_x, -2048);
    }
    #[test]
    fn corners_scaling_and_hit_geometry_stay_on_screen() {
        for placement in [
            Placement::BottomLeft,
            Placement::BottomRight,
            Placement::TopLeft,
            Placement::TopRight,
        ] {
            for scale in [0.65, 1.0, 1.5] {
                for viewport in [[320.0, 240.0], [800.0, 450.0]] {
                    let p = Preferences {
                        placement,
                        scale,
                        offset_x: 2048,
                        offset_y: -2048,
                        ..Preferences::default()
                    };
                    let l = Layout::new(p, viewport);
                    let b = l.bounds;
                    assert!(
                        b[0] >= 0.0
                            && b[1] >= 0.0
                            && b[0] + b[2] <= viewport[0] + 0.01
                            && b[1] + b[3] <= viewport[1] + 0.01
                    );
                    let main = l.screen(l.main);
                    assert!(contains(b, [main[0] + 1.0, main[1] + 1.0]));
                    assert!(intersection(main, b).is_some());
                }
            }
        }
    }
    #[test]
    fn scaled_card_clicks_hit_only_visible_rectangles() {
        let l = Layout::new(
            Preferences {
                scale: 1.3,
                placement: Placement::TopRight,
                ..Preferences::default()
            },
            [800.0, 450.0],
        );
        let rect = l.screen([l.main[0], l.main[1], l.main[2], 22.0]);
        let hits = [Hit { id: 7, rect }];
        assert_eq!(hit_at(&hits, [rect[0] + 5.0, rect[1] + 5.0]), Some(7));
        assert_eq!(hit_at(&hits, [rect[0] - 1.0, rect[1] + 5.0]), None);
        assert_eq!(hit_at(&hits, [rect[0] + rect[2], rect[1] + 5.0]), None);
    }
}
