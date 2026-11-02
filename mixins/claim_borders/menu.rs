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
    pub code: String,
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
    pub relay_url: String,
    pub relay_code: String,
    pub relay_token_present: bool,
    pub claims: ClaimsSettings,
    pub status: String,
}
#[derive(Clone, PartialEq)]
pub enum Action {
    AddWaypoint {
        name: String,
        color: u32,
    },
    RenameWaypoint {
        id: u64,
        name: String,
    },
    RecolorWaypoint {
        id: u64,
        color: u32,
    },
    DeleteWaypoint {
        id: u64,
    },
    Invite {
        code: String,
    },
    Accept {
        code: String,
    },
    RemoveFriend {
        name: String,
    },
    RefreshClaims,
    RelayOff,
    RelayConnect,
    ConfigureRelay {
        url: String,
        access_token: Option<String>,
    },
    ClearRelayToken,
    CopyOwnCode,
    PasteFromClipboard,
    SetClaims(ClaimsSettings),
    Close,
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigureRelay { url, access_token } => f
                .debug_struct("ConfigureRelay")
                .field("url", url)
                .field("access_token", &access_token.as_ref().map(|_| "[REDACTED]"))
                .finish(),
            Self::AddWaypoint { name, color } => f
                .debug_struct("AddWaypoint")
                .field("name", name)
                .field("color", color)
                .finish(),
            Self::RenameWaypoint { id, name } => f
                .debug_struct("RenameWaypoint")
                .field("id", id)
                .field("name", name)
                .finish(),
            Self::RecolorWaypoint { id, color } => f
                .debug_struct("RecolorWaypoint")
                .field("id", id)
                .field("color", color)
                .finish(),
            Self::DeleteWaypoint { id } => {
                f.debug_struct("DeleteWaypoint").field("id", id).finish()
            }
            Self::Invite { code } => f.debug_struct("Invite").field("code", code).finish(),
            Self::Accept { code } => f.debug_struct("Accept").field("code", code).finish(),
            Self::RemoveFriend { name } => {
                f.debug_struct("RemoveFriend").field("name", name).finish()
            }
            Self::SetClaims(settings) => f.debug_tuple("SetClaims").field(settings).finish(),
            Self::RefreshClaims => f.write_str("RefreshClaims"),
            Self::RelayOff => f.write_str("RelayOff"),
            Self::RelayConnect => f.write_str("RelayConnect"),
            Self::ClearRelayToken => f.write_str("ClearRelayToken"),
            Self::CopyOwnCode => f.write_str("CopyOwnCode"),
            Self::PasteFromClipboard => f.write_str("PasteFromClipboard"),
            Self::Close => f.write_str("Close"),
        }
    }
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
    PeerCode,
    RelayUrl,
    RelayToken,
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
    RelaySetup,
    RelaySave,
    RelayConnect,
    RelayBack,
    ClearRelayToken,
    CopyOwnCode,
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
#[derive(Clone)]
pub struct Menu {
    pub tab: Tab,
    pub page: usize,
    pub selected: Option<usize>,
    pub focus: Option<Field>,
    pub name: String,
    pub color: String,
    pub peer_code: String,
    pub relay_setup: bool,
    pub relay_url: String,
    pub access_token: String,
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
            peer_code: String::new(),
            relay_setup: false,
            relay_url: String::new(),
            access_token: String::new(),
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
                next.party
                    .iter()
                    .position(|row| row.code == old.code && row.pending == old.pending)
            }),
            Tab::Claims => None,
        });
        self.clamp_page(next, height);
    }
    pub fn clamp_page(&mut self, model: &ViewModel, height: f32) {
        let pages = self.count(model).div_ceil(self.page_size(height)).max(1);
        self.page = self.page.min(pages - 1);
    }
    pub fn rows_per_page(height: f32) -> usize {
        (((height - 170.0) / 24.0).floor() as usize).clamp(2, 12)
    }
    fn page_size(&self, height: f32) -> usize {
        if self.tab == Tab::Party {
            (((height - 216.0) / 24.0).floor() as usize).clamp(1, 12)
        } else {
            Self::rows_per_page(height)
        }
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
        if self.tab == Tab::Party {
            return self.party_layout(model, width, height, out);
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
                    "Нет меток. Введите имя и добавьте текущую позицию.",
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
    fn party_layout(
        &self,
        model: &ViewModel,
        width: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        let w = (width - 16.0).clamp(220.0, 590.0);
        let x = (width - w) * 0.5;
        if self.relay_setup {
            out.push(widget(
                [x, 54.0, w, 12.0],
                "Адрес вашего relay (ws:// или wss://)",
                None,
                false,
            ));
            out.push(widget(
                [x, 68.0, w, 20.0],
                &self.relay_url,
                Some(Control::Field(Field::RelayUrl)),
                self.focus == Some(Field::RelayUrl),
            ));
            out.push(widget(
                [x, 94.0, w, 12.0],
                if model.relay_token_present {
                    "Токен сохранён; пустое поле сохраняет его"
                } else {
                    "Токен доступа (если нужен)"
                },
                None,
                false,
            ));
            out.push(widget(
                [x, 108.0, w, 20.0],
                "*".repeat(self.access_token.chars().count()),
                Some(Control::Field(Field::RelayToken)),
                self.focus == Some(Field::RelayToken),
            ));
            for (i, (control, label)) in [
                (Control::RelaySave, "Сохранить"),
                (Control::ClearRelayToken, "Удалить токен"),
                (Control::RelayConnect, "Подключить"),
                (Control::RelayOff, "Отключить"),
                (Control::RelayBack, "Назад"),
            ]
            .into_iter()
            .enumerate()
            {
                let row = i / 2;
                let col = i % 2;
                out.push(widget(
                    [
                        x + col as f32 * w / 2.0,
                        134.0 + row as f32 * 26.0,
                        w / 2.0 - 3.0,
                        22.0,
                    ],
                    label,
                    Some(control),
                    false,
                ));
            }
            out.push(widget(
                [x, 212.0, w, 10.0],
                "Сначала сохраните адрес, затем подключите.",
                None,
                false,
            ));
        } else {
            out.push(widget(
                [x, 54.0, w, 14.0],
                if model.relay_code.is_empty() {
                    "Ваш код: подключите relay".into()
                } else {
                    format!("Ваш код: {}", model.relay_code)
                },
                None,
                false,
            ));
            out.push(widget([x, 70.0, w, 12.0], &model.relay, None, false));
            out.push(widget(
                [x, 86.0, w / 2.0 - 3.0, 20.0],
                "Настроить relay",
                Some(Control::RelaySetup),
                false,
            ));
            out.push(widget(
                [x + w / 2.0, 86.0, w / 2.0, 20.0],
                "Копировать код",
                Some(Control::CopyOwnCode),
                false,
            ));
            let per = self.page_size(height);
            for (slot, index) in
                (self.page * per..((self.page + 1) * per).min(model.party.len())).enumerate()
            {
                let row = &model.party[index];
                out.push(widget(
                    [x, 110.0 + slot as f32 * 24.0, w, 22.0],
                    format!(
                        "{} · {}{} · {}",
                        row.code,
                        if row.pending { "[+] " } else { "" },
                        row.name,
                        row.detail
                    ),
                    Some(Control::Row(index)),
                    self.selected == Some(index),
                ));
            }
            if model.party.is_empty() {
                out.push(widget(
                    [x, 112.0, w, 16.0],
                    "Нет друзей. Получите код друга лично.",
                    None,
                    false,
                ));
            }
            let b = 110.0 + per as f32 * 24.0;
            out.push(widget(
                [x, b, 28.0, 18.0],
                "<",
                Some(Control::Previous),
                false,
            ));
            out.push(widget(
                [x + 32.0, b, w - 64.0, 18.0],
                format!(
                    "{} / {}",
                    self.page + 1,
                    model.party.len().div_ceil(per).max(1)
                ),
                None,
                false,
            ));
            out.push(widget(
                [x + w - 28.0, b, 28.0, 18.0],
                ">",
                Some(Control::Next),
                false,
            ));
            out.push(widget(
                [x, b + 22.0, w, 20.0],
                format!("Код друга: {}", self.peer_code),
                Some(Control::Field(Field::PeerCode)),
                self.focus == Some(Field::PeerCode),
            ));
            for (i, (control, label)) in [
                (Control::Invite, "Пригласить"),
                (Control::Accept, "Принять"),
                (Control::Remove, "Убрать друга"),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x + i as f32 * w / 3.0, b + 46.0, w / 3.0 - 3.0, 20.0],
                    label,
                    Some(control),
                    false,
                ));
            }
            out.push(widget(
                [x, b + 70.0, w, 12.0],
                "Имена не проверены; сверяйте коды вне игры.",
                None,
                false,
            ));
        }
        out.push(widget(
            [x, height - 17.0, w, 12.0],
            if self.notice.is_empty() {
                &model.status
            } else {
                &self.notice
            },
            None,
            false,
        ));
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
        let code = self.peer_code.trim().to_ascii_lowercase();
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
                self.relay_setup = false;
                self.access_token.clear();
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
                    // The displayed incoming code is untrusted; verification requires manual entry.
                    Tab::Party => {}
                    _ => {}
                };
                None
            }
            Control::Previous => {
                self.page = self.page.saturating_sub(1);
                None
            }
            Control::Next => {
                let pages = self.count(model).div_ceil(self.page_size(height)).max(1);
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
            Control::Invite | Control::Accept => {
                if code.len() != 24 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
                    self.notice = "Введите 24 hex-символа кода, полученного от друга.".into();
                    return None;
                }
                Some(if control == Control::Invite {
                    Action::Invite { code }
                } else {
                    Action::Accept { code }
                })
            }
            Control::Remove => {
                let Some(row) = self
                    .selected
                    .and_then(|i| model.party.get(i))
                    .filter(|row| !row.pending)
                else {
                    self.notice = "Выберите друга в списке.".into();
                    return None;
                };
                Some(Action::RemoveFriend {
                    name: row.name.clone(),
                })
            }
            Control::Refresh => Some(Action::RefreshClaims),
            Control::RelayOff => Some(Action::RelayOff),
            Control::RelayConnect => Some(Action::RelayConnect),
            Control::RelaySetup => {
                self.relay_setup = true;
                self.relay_url = model.relay_url.clone();
                self.access_token.clear();
                self.focus = None;
                None
            }
            Control::RelayBack => {
                self.relay_setup = false;
                self.access_token.clear();
                self.focus = None;
                None
            }
            Control::RelaySave => {
                let url = self.relay_url.trim().to_owned();
                if url.is_empty() {
                    self.notice = "Введите адрес ws:// или wss://.".into();
                    return None;
                }
                let access_token = if self.access_token.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.access_token))
                };
                Some(Action::ConfigureRelay { url, access_token })
            }
            Control::ClearRelayToken => {
                self.access_token.clear();
                Some(Action::ClearRelayToken)
            }
            Control::CopyOwnCode => (!model.relay_code.is_empty()).then_some(Action::CopyOwnCode),
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
        if !self.relay_setup && delta.is_finite() && delta != 0.0 {
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
    fn focused_text(&mut self) -> Option<&mut String> {
        Some(match self.focus? {
            Field::Name => &mut self.name,
            Field::Color => &mut self.color,
            Field::PeerCode => &mut self.peer_code,
            Field::RelayUrl => &mut self.relay_url,
            Field::RelayToken => &mut self.access_token,
        })
    }
    pub fn paste(&mut self, text: &str) {
        // The same limits and character policy apply to typing and pasted input.
        for c in text.chars() {
            self.character(c);
        }
    }
    pub fn character(&mut self, c: char) {
        if c.is_control() || c == '\u{00a7}' {
            return;
        }
        let Some(field) = self.focus else {
            return;
        };
        let limit = match field {
            Field::Name => 48,
            Field::Color => 7,
            Field::PeerCode => 24,
            Field::RelayUrl => 2048,
            Field::RelayToken => 1024,
        };
        if (field == Field::Color && !(c.is_ascii_hexdigit() || c == '#'))
            || (field == Field::PeerCode && !c.is_ascii_hexdigit())
        {
            return;
        }
        if let Some(text) = self.focused_text()
            && text.chars().count() < limit
        {
            text.push(c);
        }
    }
    /// False only for Escape when the native screen should close itself.
    pub fn key(&mut self, key: &str, modifiers: u32) -> bool {
        if key == "key.keyboard.escape" {
            self.access_token.clear();
            if self.relay_setup {
                self.relay_setup = false;
                self.focus = None;
                return true;
            }
            return false;
        }
        if key == "key.keyboard.tab" {
            self.focus = Some(if self.relay_setup {
                if self.focus == Some(Field::RelayUrl) {
                    Field::RelayToken
                } else {
                    Field::RelayUrl
                }
            } else if self.tab == Tab::Party {
                Field::PeerCode
            } else if self.focus == Some(Field::Name) {
                Field::Color
            } else {
                Field::Name
            });
        }
        if let Some(text) = self.focused_text() {
            if key == "key.keyboard.backspace" {
                text.pop();
            }
            if key == "key.keyboard.a" && modifiers & (2 | 8) != 0 {
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
    fn controls_fit_minimum_gui() {
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
    }
    #[test]
    fn relay_setup_fits_minimum_gui_and_masks_token() {
        let model = ViewModel {
            relay_token_present: true,
            ..ViewModel::default()
        };
        let menu = Menu {
            tab: Tab::Party,
            relay_setup: true,
            access_token: "never-display-this-secret".into(),
            ..Menu::default()
        };
        let widgets = menu.layout(&model, 320.0, 240.0);
        for w in &widgets {
            assert!(
                w.rect[0] >= 0.0
                    && w.rect[1] >= 0.0
                    && w.rect[0] + w.rect[2] <= 320.0
                    && w.rect[1] + w.rect[3] <= 240.0
            );
            assert!(!w.text.contains("never-display"));
        }
        let token = widgets
            .iter()
            .find(|w| w.control == Some(Control::Field(Field::RelayToken)))
            .unwrap();
        assert_eq!(token.text, "*".repeat(menu.access_token.len()));
        assert!(!format!("{model:?}").contains("never-display"));
    }
    #[test]
    fn row_selection_never_fills_verified_code_or_sends_an_invite() {
        let model = ViewModel {
            party: vec![PartyRow {
                name: "Friend".into(),
                code: "abcdef0123456789abcdef01".into(),
                detail: String::new(),
                pending: true,
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        assert_eq!(menu.activate(&model, 240.0, Control::Row(0)), None);
        assert!(menu.peer_code.is_empty());
        assert_eq!(menu.activate(&model, 240.0, Control::Accept), None);
        menu.peer_code = "1234567890abcdef12345678".into();
        assert_eq!(menu.activate(&model, 240.0, Control::Row(0)), None);
        assert_eq!(menu.peer_code, "1234567890abcdef12345678");
        assert_eq!(
            menu.activate(&model, 240.0, Control::Accept),
            Some(Action::Accept {
                code: menu.peer_code.clone()
            })
        );
    }
    #[test]
    fn remove_uses_selected_friend_and_never_peer_code_input() {
        let model = ViewModel {
            party: vec![PartyRow {
                name: "SavedFriend".into(),
                code: "a".repeat(24),
                detail: String::new(),
                pending: false,
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            peer_code: "b".repeat(24),
            selected: Some(0),
            ..Menu::default()
        };
        assert_eq!(
            menu.activate(&model, 240.0, Control::Remove),
            Some(Action::RemoveFriend {
                name: "SavedFriend".into()
            })
        );
    }
    #[test]
    fn relay_save_and_navigation_never_connect_automatically() {
        let old = ViewModel {
            relay_url: "wss://relay.example/ws".into(),
            ..ViewModel::default()
        };
        let next = ViewModel {
            relay_url: "wss://other.example/ws".into(),
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        assert_eq!(menu.activate(&old, 240.0, Control::RelaySetup), None);
        assert_eq!(menu.relay_url, old.relay_url);
        menu.relay_url = "wss://edited.example/ws".into();
        menu.reconcile(&old, &next, 240.0);
        assert_eq!(menu.relay_url, "wss://edited.example/ws");
        assert_eq!(
            menu.activate(&next, 240.0, Control::RelaySave),
            Some(Action::ConfigureRelay {
                url: menu.relay_url.clone(),
                access_token: None
            })
        );
        menu.access_token = "secret".into();
        assert!(
            matches!(menu.activate(&next,240.0,Control::RelaySave),Some(Action::ConfigureRelay{access_token:Some(token),..}) if token=="secret")
        );
        assert!(menu.access_token.is_empty());
        menu.access_token = "unsaved-secret".into();
        assert_eq!(
            menu.activate(&next, 240.0, Control::Tab(Tab::Waypoints)),
            None
        );
        assert!(menu.access_token.is_empty());
        assert_eq!(
            menu.activate(&next, 240.0, Control::RelayConnect),
            Some(Action::RelayConnect)
        );
    }
    #[test]
    fn peer_code_is_hex_bounded_and_keyboard_never_submits() {
        let model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Party,
            focus: Some(Field::PeerCode),
            ..Menu::default()
        };
        for c in "XYZabcdef0123456789abcdef0123456789".chars() {
            menu.character(c);
        }
        assert_eq!(menu.peer_code, "abcdef0123456789abcdef01");
        assert!(menu.key("key.keyboard.enter", 0));
        assert!(menu.key("key.keyboard.m", 0));
        assert_eq!(
            menu.activate(&model, 240.0, Control::Invite),
            Some(Action::Invite {
                code: menu.peer_code.clone()
            })
        );
        menu.peer_code = "a".repeat(23);
        assert_eq!(menu.activate(&model, 240.0, Control::Invite), None);
    }
    #[test]
    fn pasted_code_is_bounded_and_token_debug_is_redacted() {
        let mut menu = Menu {
            focus: Some(Field::PeerCode),
            ..Menu::default()
        };
        menu.paste("ABCDEF0123456789abcdef01NOT_HEX_extra");
        assert_eq!(menu.peer_code, "ABCDEF0123456789abcdef01");
        menu.focus = Some(Field::RelayToken);
        menu.paste(&"s".repeat(2000));
        assert_eq!(menu.access_token.len(), 1024);
        let action = Action::ConfigureRelay {
            url: "wss://relay.example".into(),
            access_token: Some("neverlogsecret".into()),
        };
        let debug = format!("{action:?}");
        assert!(!debug.contains("neverlogsecret"));
        assert!(debug.contains("[REDACTED]"));
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
