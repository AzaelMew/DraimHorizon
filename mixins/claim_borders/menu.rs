// Pure management screen state. Only explicit local button presses produce actions.
#[derive(Clone, Debug)]
pub struct WaypointRow {
    pub id: u64,
    pub name: String,
    pub detail: String,
    pub color: u32,
}
#[derive(Clone, Debug)]
pub struct PartyRow {
    pub name: String,
    pub detail: String,
    pub pending: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClaimsSettings {
    pub enabled: bool,
    pub own_visible: bool,
    pub friends_visible: bool,
    pub through_walls: bool,
    pub height: f32,
    pub distance: f32,
    pub own_color: u32,
    pub friend_color: u32,
    pub opacity: f32,
}
impl Default for ClaimsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            own_visible: true,
            friends_visible: true,
            through_walls: true,
            height: 32.0,
            distance: 160.0,
            own_color: 0x55ff88,
            friend_color: 0x55bbff,
            opacity: 0.45,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ViewModel {
    pub waypoints: Vec<WaypointRow>,
    pub party: Vec<PartyRow>,
    pub relay: String,
    pub claims: ClaimsSettings,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    AddWaypoint { name: String, color: u32 },
    RenameWaypoint { id: u64, name: String },
    RecolorWaypoint { id: u64, color: u32 },
    DeleteWaypoint { id: u64 },
    Invite { name: String },
    Accept { name: String },
    RemoveFriend { name: String },
    RefreshClaims,
    RelayOff,
    RelayUnsigned,
    AuthorizeSignedRelay,
    SetClaims(ClaimsSettings),
    Close,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tab {
    Waypoints,
    Party,
    Claims,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    Name,
    Color,
    Player,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    Tab(Tab),
    Close,
    Row(usize),
    Previous,
    Next,
    Field(Field),
    Add,
    Rename,
    Color,
    Delete,
    Invite,
    Accept,
    Remove,
    Refresh,
    RelayOff,
    RelayUnsigned,
    RelaySigned,
    ConfirmSigned,
    Cancel,
    BordersEnabled,
    OwnVisible,
    FriendsVisible,
    ThroughWalls,
    Height,
    Distance,
    OwnColor,
    FriendColor,
    Opacity,
}
#[derive(Clone, Debug)]
pub struct Widget {
    pub rect: [f32; 4],
    pub text: String,
    pub control: Option<Control>,
    pub selected: bool,
    pub muted: bool,
}
#[derive(Clone, Debug)]
pub struct Menu {
    pub tab: Tab,
    pub page: usize,
    pub selected: Option<usize>,
    pub focus: Option<Field>,
    pub name: String,
    pub color: String,
    pub player: String,
    pub confirm_signed: bool,
    pub notice: String,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            tab: Tab::Waypoints,
            page: 0,
            selected: None,
            focus: None,
            name: String::new(),
            color: "#FFCC55".into(),
            player: String::new(),
            confirm_signed: false,
            notice: String::new(),
        }
    }
}
fn widget(
    rect: [f32; 4],
    text: impl Into<String>,
    control: Option<Control>,
    selected: bool,
) -> Widget {
    Widget {
        rect,
        text: text.into(),
        control,
        selected,
        muted: false,
    }
}
fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
impl Menu {
    /// Keep a selection attached to its identity while snapshots reorder or remove rows.
    pub fn reconcile(&mut self, previous: &ViewModel, next: &ViewModel, height: f32) {
        self.selected = self.selected.and_then(|index| match self.tab {
            Tab::Waypoints => previous
                .waypoints
                .get(index)
                .and_then(|old| next.waypoints.iter().position(|row| row.id == old.id)),
            Tab::Party => previous.party.get(index).and_then(|old| {
                next.party.iter().position(|row| {
                    row.name.eq_ignore_ascii_case(&old.name) && row.pending == old.pending
                })
            }),
            Tab::Claims => None,
        });
        self.clamp_page(next, height);
    }
    pub fn clamp_page(&mut self, model: &ViewModel, height: f32) {
        let pages = self
            .count(model)
            .div_ceil(Self::rows_per_page(height))
            .max(1);
        self.page = self.page.min(pages - 1);
    }
    pub fn rows_per_page(height: f32) -> usize {
        (((height - 170.0) / 24.0).floor() as usize).clamp(2, 12)
    }
    fn count(&self, model: &ViewModel) -> usize {
        match self.tab {
            Tab::Waypoints => model.waypoints.len(),
            Tab::Party => model.party.len(),
            Tab::Claims => 0,
        }
    }
    pub fn layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        let w = (width - 16.0).clamp(220.0, 590.0);
        let x = (width - w) * 0.5;
        let y = 8.0;
        let mut out = vec![
            widget(
                [x, y, w - 30.0, 18.0],
                "Horizon · Метки / Пати / Клеймы",
                None,
                false,
            ),
            widget(
                [x + w - 26.0, y, 26.0, 18.0],
                "X",
                Some(Control::Close),
                false,
            ),
        ];
        for (i, (tab, label)) in [
            (Tab::Waypoints, "Метки"),
            (Tab::Party, "Пати"),
            (Tab::Claims, "Клеймы"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 3.0, y + 22.0, w / 3.0 - 3.0, 20.0],
                label,
                Some(Control::Tab(tab)),
                self.tab == tab,
            ));
        }
        if self.confirm_signed {
            out.retain(|w| !matches!(w.control, Some(Control::Tab(_))));
            for (i, line) in [
                "Подписанный обмен для этого подключения",
                "Мод автоматически отправит через /msg:",
                "ваши координаты, здоровье и клеймы друзьям.",
                "Сервер видит сообщения и ваши подписи.",
                "Включайте только на сервере, которому доверяете.",
                "После переподключения обмен выключен.",
            ]
            .iter()
            .enumerate()
            {
                out.push(widget(
                    [x + 4.0, y + 52.0 + i as f32 * 16.0, w - 8.0, 15.0],
                    *line,
                    None,
                    false,
                ));
            }
            out.push(widget(
                [x, y + 158.0, w * 0.65 - 3.0, 22.0],
                "Разрешить подписи и обмен",
                Some(Control::ConfirmSigned),
                true,
            ));
            out.push(widget(
                [x + w * 0.65, y + 158.0, w * 0.35, 22.0],
                "Отмена",
                Some(Control::Cancel),
                false,
            ));
            return out;
        }
        if self.tab == Tab::Claims {
            let s = &model.claims;
            let controls = [
                (
                    Control::BordersEnabled,
                    format!("Все границы: {}", on(s.enabled)),
                ),
                (Control::OwnVisible, format!("Свои: {}", on(s.own_visible))),
                (
                    Control::FriendsVisible,
                    format!("Друзья: {}", on(s.friends_visible)),
                ),
                (
                    Control::ThroughWalls,
                    format!("Сквозь стены: {}", on(s.through_walls)),
                ),
                (
                    Control::Height,
                    if s.height >= 384.0 {
                        "Высота: весь мир".into()
                    } else {
                        format!("Высота: ±{:.0}", s.height)
                    },
                ),
                (Control::Distance, format!("Дальность: {:.0}", s.distance)),
                (
                    Control::Opacity,
                    format!("Непрозрачность: {:.0}%", s.opacity * 100.0),
                ),
                (Control::OwnColor, format!("Свои: #{:06X}", s.own_color)),
                (
                    Control::FriendColor,
                    format!("Друзья: #{:06X}", s.friend_color),
                ),
                (Control::Refresh, "Обновить /claims".into()),
            ];
            for (i, (control, label)) in controls.into_iter().enumerate() {
                out.push(widget(
                    [
                        x + (i % 2) as f32 * w / 2.0,
                        y + 50.0 + (i / 2) as f32 * 26.0,
                        w / 2.0 - 3.0,
                        22.0,
                    ],
                    label,
                    Some(control),
                    false,
                ));
            }
            out.push(widget(
                [x, y + 184.0, w, 14.0],
                "Нажмите параметр для следующего значения.",
                None,
                false,
            ));
        } else {
            let per = Self::rows_per_page(height);
            let start = self.page * per;
            let end = (start + per).min(self.count(model));
            for (slot, index) in (start..end).enumerate() {
                let label = match self.tab {
                    Tab::Waypoints => {
                        let r = &model.waypoints[index];
                        format!("#{} {} · {}", r.id, r.name, r.detail)
                    }
                    _ => {
                        let r = &model.party[index];
                        format!(
                            "{}{} · {}",
                            if r.pending {
                                "[Приглашение] "
                            } else {
                                ""
                            },
                            r.name,
                            r.detail
                        )
                    }
                };
                out.push(widget(
                    [x, y + 48.0 + slot as f32 * 24.0, w, 22.0],
                    label,
                    Some(Control::Row(index)),
                    self.selected == Some(index),
                ));
            }
            if self.count(model) == 0 {
                out.push(widget(
                    [x + 4.0, y + 52.0, w - 8.0, 16.0],
                    if self.tab == Tab::Waypoints {
                        "Нет меток. Введите имя и добавьте текущую позицию."
                    } else {
                        "Нет друзей и приглашений. Введите имя игрока."
                    },
                    None,
                    false,
                ));
            }
            let b = y + 48.0 + per as f32 * 24.0;
            let pages = self.count(model).div_ceil(per).max(1);
            out.push(widget(
                [x, b, 28.0, 18.0],
                "<",
                Some(Control::Previous),
                false,
            ));
            out.push(widget(
                [x + 32.0, b, w - 64.0, 18.0],
                format!("Страница {} / {}", self.page + 1, pages),
                None,
                false,
            ));
            out.push(widget(
                [x + w - 28.0, b, 28.0, 18.0],
                ">",
                Some(Control::Next),
                false,
            ));
            if self.tab == Tab::Waypoints {
                out.push(widget(
                    [x, b + 22.0, w * 0.65 - 3.0, 20.0],
                    format!("Имя: {}", self.name),
                    Some(Control::Field(Field::Name)),
                    self.focus == Some(Field::Name),
                ));
                out.push(widget(
                    [x + w * 0.65, b + 22.0, w * 0.35, 20.0],
                    format!("Цвет: {}", self.color),
                    Some(Control::Field(Field::Color)),
                    self.focus == Some(Field::Color),
                ));
                for (i, (c, t)) in [
                    (Control::Add, "+ Здесь"),
                    (Control::Rename, "Имя"),
                    (Control::Color, "Цвет"),
                    (Control::Delete, "Удалить"),
                ]
                .into_iter()
                .enumerate()
                {
                    out.push(widget(
                        [x + i as f32 * w / 4.0, b + 46.0, w / 4.0 - 3.0, 20.0],
                        t,
                        Some(c),
                        false,
                    ));
                }
            } else {
                out.push(widget(
                    [x, b + 22.0, w * 0.5 - 3.0, 20.0],
                    format!("Игрок: {}", self.player),
                    Some(Control::Field(Field::Player)),
                    self.focus == Some(Field::Player),
                ));
                for (i, (c, t)) in [
                    (Control::Invite, "Позвать"),
                    (Control::Accept, "Принять"),
                    (Control::Remove, "Убрать"),
                ]
                .into_iter()
                .enumerate()
                {
                    out.push(widget(
                        [
                            x + w * 0.5 + i as f32 * w / 6.0,
                            b + 22.0,
                            w / 6.0 - 3.0,
                            20.0,
                        ],
                        t,
                        Some(c),
                        false,
                    ));
                }
                for (i, (c, t)) in [
                    (Control::RelayOff, "Выкл."),
                    (Control::RelayUnsigned, "Без подписи"),
                    (Control::RelaySigned, "С подписью…"),
                    (Control::Refresh, "/claims"),
                ]
                .into_iter()
                .enumerate()
                {
                    out.push(widget(
                        [x + i as f32 * w / 4.0, b + 46.0, w / 4.0 - 3.0, 20.0],
                        t,
                        Some(c),
                        false,
                    ));
                }
            }
        }
        let status = if !self.notice.is_empty() {
            self.notice.clone()
        } else if self.tab == Tab::Party {
            format!("Обмен: {} · {}", model.relay, model.status)
        } else {
            model.status.clone()
        };
        out.push(widget([x, height - 19.0, w, 14.0], status, None, false));
        out
    }
    pub fn click(&mut self, model: &ViewModel, size: [f32; 2], mouse: [f32; 2]) -> Option<Action> {
        let hit = self
            .layout(model, size[0], size[1])
            .into_iter()
            .rev()
            .find(|w| w.control.is_some() && contains(w.rect, mouse));
        let Some(control) = hit.and_then(|w| w.control) else {
            self.focus = None;
            return None;
        };
        self.activate(model, size[1], control)
    }
    fn activate(&mut self, model: &ViewModel, height: f32, control: Control) -> Option<Action> {
        self.notice.clear();
        let id = self
            .selected
            .and_then(|i| model.waypoints.get(i))
            .map(|r| r.id);
        let name = self.name.trim().to_owned();
        let player = self.player.trim().to_owned();
        let color = u32::from_str_radix(self.color.trim().trim_start_matches('#'), 16)
            .ok()
            .filter(|_| self.color.trim().trim_start_matches('#').len() == 6);
        match control {
            Control::Close => Some(Action::Close),
            Control::Tab(t) => {
                self.tab = t;
                self.page = 0;
                self.selected = None;
                self.focus = None;
                None
            }
            Control::Field(f) => {
                self.focus = Some(f);
                None
            }
            Control::Row(i) => {
                self.selected = Some(i);
                self.focus = None;
                match self.tab {
                    Tab::Waypoints => {
                        if let Some(r) = model.waypoints.get(i) {
                            self.name = r.name.clone();
                            self.color = format!("#{:06X}", r.color);
                        }
                    }
                    Tab::Party => {
                        if let Some(r) = model.party.get(i) {
                            self.player = r.name.clone();
                        }
                    }
                    _ => {}
                };
                None
            }
            Control::Previous => {
                self.page = self.page.saturating_sub(1);
                None
            }
            Control::Next => {
                let pages = self
                    .count(model)
                    .div_ceil(Self::rows_per_page(height))
                    .max(1);
                self.page = (self.page + 1).min(pages - 1);
                None
            }
            Control::Add | Control::Rename | Control::Color | Control::Delete => {
                if matches!(control, Control::Add | Control::Rename) && name.is_empty() {
                    self.notice = "Введите имя метки.".into();
                    return None;
                }
                if matches!(control, Control::Add | Control::Color) && color.is_none() {
                    self.notice = "Цвет: #RRGGBB, шесть цифр.".into();
                    return None;
                }
                if control != Control::Add && id.is_none() {
                    self.notice = "Сначала выберите метку.".into();
                    return None;
                }
                Some(match control {
                    Control::Add => Action::AddWaypoint {
                        name,
                        color: color?,
                    },
                    Control::Rename => Action::RenameWaypoint { id: id?, name },
                    Control::Color => Action::RecolorWaypoint {
                        id: id?,
                        color: color?,
                    },
                    _ => {
                        self.selected = None;
                        Action::DeleteWaypoint { id: id? }
                    }
                })
            }
            Control::Invite | Control::Accept | Control::Remove => {
                if player.is_empty() {
                    self.notice = "Введите имя игрока или выберите строку.".into();
                    return None;
                }
                Some(match control {
                    Control::Invite => Action::Invite { name: player },
                    Control::Accept => Action::Accept { name: player },
                    _ => Action::RemoveFriend { name: player },
                })
            }
            Control::Refresh => Some(Action::RefreshClaims),
            Control::RelayOff => Some(Action::RelayOff),
            Control::RelayUnsigned => Some(Action::RelayUnsigned),
            Control::RelaySigned => {
                self.confirm_signed = true;
                self.focus = None;
                None
            }
            Control::ConfirmSigned => {
                if self.confirm_signed {
                    self.confirm_signed = false;
                    Some(Action::AuthorizeSignedRelay)
                } else {
                    None
                }
            }
            Control::Cancel => {
                self.confirm_signed = false;
                None
            }
            _ => {
                let mut s = model.claims.clone();
                match control {
                    Control::BordersEnabled => s.enabled = !s.enabled,
                    Control::OwnVisible => s.own_visible = !s.own_visible,
                    Control::FriendsVisible => s.friends_visible = !s.friends_visible,
                    Control::ThroughWalls => s.through_walls = !s.through_walls,
                    Control::Height => s.height = cycle(s.height, &[16.0, 32.0, 64.0, 384.0]),
                    Control::Distance => {
                        s.distance = cycle(s.distance, &[64.0, 160.0, 256.0, 512.0])
                    }
                    Control::Opacity => s.opacity = cycle(s.opacity, &[0.25, 0.45, 0.7, 1.0]),
                    Control::OwnColor => s.own_color = cycle_color(s.own_color),
                    Control::FriendColor => s.friend_color = cycle_color(s.friend_color),
                    _ => return None,
                };
                Some(Action::SetClaims(s))
            }
        }
    }
    pub fn scroll(&mut self, model: &ViewModel, height: f32, delta: f64) {
        if !self.confirm_signed && delta.is_finite() && delta != 0.0 {
            self.activate(
                model,
                height,
                if delta > 0.0 {
                    Control::Previous
                } else {
                    Control::Next
                },
            );
        }
    }
    pub fn character(&mut self, c: char) {
        if self.confirm_signed || c.is_control() || c == '\u{00a7}' {
            return;
        }
        let Some(field) = self.focus else {
            return;
        };
        let text = match field {
            Field::Name => &mut self.name,
            Field::Color => &mut self.color,
            Field::Player => &mut self.player,
        };
        let limit = match field {
            Field::Name => 48,
            Field::Color => 7,
            Field::Player => 16,
        };
        if text.chars().count() < limit
            && (field != Field::Color || c.is_ascii_hexdigit() || c == '#')
            && (field != Field::Player || c.is_ascii_alphanumeric() || c == '_')
        {
            text.push(c);
        }
    }
    /// False only for Escape when the native screen should close itself.
    pub fn key(&mut self, key: &str, modifiers: u32) -> bool {
        if key == "key.keyboard.escape" {
            if self.confirm_signed {
                self.confirm_signed = false;
                return true;
            }
            return false;
        }
        if self.confirm_signed {
            return true;
        }
        if key == "key.keyboard.tab" {
            self.focus = Some(match self.focus {
                Some(Field::Name) => Field::Color,
                Some(Field::Color) => Field::Name,
                _ => {
                    if self.tab == Tab::Party {
                        Field::Player
                    } else {
                        Field::Name
                    }
                }
            });
        }
        if let Some(field) = self.focus {
            let text = match field {
                Field::Name => &mut self.name,
                Field::Color => &mut self.color,
                Field::Player => &mut self.player,
            };
            if key == "key.keyboard.backspace" {
                text.pop();
            }
            if key == "key.keyboard.a" && modifiers & 2 != 0 {
                text.clear();
            }
        }
        true
    }
}
fn on(value: bool) -> &'static str {
    if value { "вкл." } else { "выкл." }
}
fn cycle(value: f32, values: &[f32]) -> f32 {
    values
        .iter()
        .copied()
        .find(|v| *v > value + 0.001)
        .unwrap_or(values[0])
}
fn cycle_color(value: u32) -> u32 {
    let colors = [0x55ff88, 0x55bbff, 0xffcc55, 0xff6688, 0xcc88ff, 0xffffff];
    let i = colors
        .iter()
        .position(|v| *v == value)
        .unwrap_or(colors.len() - 1);
    colors[(i + 1) % colors.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_keep_waypoint_identity_and_preserve_edits() {
        let row = |id| WaypointRow {
            id,
            name: format!("Point {id}"),
            detail: String::new(),
            color: 0,
        };
        let old = ViewModel {
            waypoints: vec![row(1), row(2)],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            selected: Some(1),
            name: "Edited name".into(),
            page: 9,
            ..Menu::default()
        };
        let next = ViewModel {
            waypoints: vec![row(2)],
            ..ViewModel::default()
        };
        menu.reconcile(&old, &next, 240.0);
        assert_eq!(menu.selected, Some(0));
        assert_eq!(menu.page, 0);
        assert_eq!(menu.name, "Edited name");
        menu.reconcile(&next, &ViewModel::default(), 240.0);
        assert_eq!(menu.selected, None);
    }
    #[test]
    fn disabled_master_preserves_independent_visibility_when_adjusting_opacity() {
        let mut model = ViewModel::default();
        model.claims.enabled = false;
        let mut menu = Menu {
            tab: Tab::Claims,
            ..Menu::default()
        };
        let Some(Action::SetClaims(updated)) = menu.activate(&model, 240.0, Control::Opacity)
        else {
            panic!("opacity must produce a settings action");
        };
        assert!(!updated.enabled);
        assert!(updated.own_visible);
        assert!(updated.friends_visible);
        assert_ne!(updated.opacity, model.claims.opacity);
        let Some(Action::SetClaims(enabled)) =
            menu.activate(&model, 240.0, Control::BordersEnabled)
        else {
            panic!("master must produce a settings action");
        };
        assert!(enabled.enabled);
        assert_eq!(enabled.own_visible, model.claims.own_visible);
        assert_eq!(enabled.friends_visible, model.claims.friends_visible);
    }
    #[test]
    fn controls_fit_minimum_gui_and_modal_has_no_tabs() {
        for tab in [Tab::Waypoints, Tab::Party, Tab::Claims] {
            let menu = Menu {
                tab,
                ..Menu::default()
            };
            for widget in menu.layout(&ViewModel::default(), 320.0, 240.0) {
                assert!(widget.rect[0] >= 0.0 && widget.rect[1] >= 0.0);
                assert!(widget.rect[0] + widget.rect[2] <= 320.0);
                assert!(widget.rect[1] + widget.rect[3] <= 240.0);
            }
        }
        let menu = Menu {
            confirm_signed: true,
            ..Menu::default()
        };
        assert!(
            !menu
                .layout(&ViewModel::default(), 320.0, 240.0)
                .iter()
                .any(|w| matches!(w.control, Some(Control::Tab(_))))
        );
    }
    #[test]
    fn signing_requires_visible_confirm_button() {
        let mut m = Menu::default();
        let v = ViewModel::default();
        assert_eq!(m.activate(&v, 300.0, Control::ConfirmSigned), None);
        assert_eq!(m.activate(&v, 300.0, Control::RelaySigned), None);
        assert!(m.confirm_signed);
        assert_eq!(
            m.activate(&v, 300.0, Control::ConfirmSigned),
            Some(Action::AuthorizeSignedRelay)
        );
        assert!(!m.confirm_signed);
    }
    #[test]
    fn unicode_backspace_is_safe_and_bounded() {
        let mut m = Menu {
            focus: Some(Field::Name),
            ..Menu::default()
        };
        for _ in 0..100 {
            m.character('я');
        }
        assert_eq!(m.name.chars().count(), 48);
        m.key("key.keyboard.backspace", 0);
        assert_eq!(m.name.chars().count(), 47);
        m.key("key.keyboard.a", 2);
        assert!(m.name.is_empty());
    }
    #[test]
    fn empty_selection_and_invalid_color_cannot_mutate() {
        let mut m = Menu {
            name: "Дом".into(),
            color: "#xyz".into(),
            ..Menu::default()
        };
        let v = ViewModel::default();
        assert_eq!(m.activate(&v, 300.0, Control::Add), None);
        assert_eq!(m.activate(&v, 300.0, Control::Delete), None);
    }
    #[test]
    fn modal_blocks_underlying_controls() {
        let m = Menu {
            confirm_signed: true,
            ..Menu::default()
        };
        assert!(
            !m.layout(&ViewModel::default(), 450.0, 300.0)
                .iter()
                .any(|w| matches!(w.control, Some(Control::Add | Control::RelayUnsigned)))
        );
    }
    #[test]
    fn screen_geometry_is_clickable_in_gui_pixels() {
        let mut m = Menu::default();
        let v = ViewModel::default();
        let rect = m
            .layout(&v, 400.0, 280.0)
            .iter()
            .find(|w| w.control == Some(Control::Tab(Tab::Party)))
            .unwrap()
            .rect;
        m.click(&v, [400.0, 280.0], [rect[0] + 1.0, rect[1] + 1.0]);
        assert_eq!(m.tab, Tab::Party);
    }
}
