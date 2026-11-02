use super::claim_borders_chat_preferences::Preferences;
use super::claim_borders_map as map;
// Pure management screen state. Only explicit local button presses produce actions.
#[derive(Clone, Debug)]
pub struct WaypointRow {
    pub id: u64,
    pub name: String,
    pub detail: String,
    pub color: u32,
}
#[derive(Clone, Debug, Default)]
pub struct PartyRow {
    pub uuid: u128,
    pub friend: bool,
    pub online: bool,
    pub blocked: bool,
    pub outgoing: bool,
    pub skin: String,
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
    pub line_width: f32,
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
            line_width: 2.0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ClaimRow {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
    pub label: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ClaimsSection {
    #[default]
    Map,
    Borders,
    Colours,
    Storage,
    List,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapOption {
    Size,
    Zoom,
    Fill,
    Outline,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderOption {
    Distance,
    Height,
    Opacity,
    Width,
}
#[derive(Clone, Debug, Default)]
pub struct ViewModel {
    pub waypoints: Vec<WaypointRow>,
    pub party: Vec<PartyRow>,
    pub relay: String,
    pub relay_url: String,
    pub relay_token_present: bool,
    pub relay_connected: bool,
    pub relay_auto: bool,
    pub claims: ClaimsSettings,
    pub status: String,
    pub chat_modern: bool,
    pub chat_translate: bool,
    pub chat_outgoing: bool,
    pub chat_preferences: Preferences,
    pub map_preferences: map::Preferences,
    pub claim_rows: Vec<ClaimRow>,
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
    Decline {
        uuid: u128,
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
    ToggleAutoRelay,
    ConfigureRelay {
        url: String,
        access_token: Option<String>,
    },
    ClearRelayToken,
    CopyText(String),
    OpenMap,
    FocusClaim {
        dimension: String,
        x: i32,
        z: i32,
    },
    SetMap(map::Preferences),
    ToggleChat,
    ToggleOutgoing,
    ClearClaimCache,
    ToggleTranslation,
    SetChat(Preferences),
    PasteFromClipboard,
    SetClaims(ClaimsSettings),
    Close,
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decline { uuid } => f.debug_tuple("Decline").field(uuid).finish(),
            Self::ToggleAutoRelay => f.write_str("ToggleAutoRelay"),
            Self::OpenMap => f.write_str("OpenMap"),
            Self::FocusClaim { dimension, x, z } => f
                .debug_tuple("FocusClaim")
                .field(dimension)
                .field(x)
                .field(z)
                .finish(),
            Self::SetMap(p) => f.debug_tuple("SetMap").field(p).finish(),
            Self::SetChat(p) => f.debug_tuple("SetChat").field(p).finish(),
            Self::ClearClaimCache => f.write_str("ClearClaimCache"),
            Self::ToggleOutgoing => f.write_str("ToggleOutgoing"),
            Self::ToggleChat => f.write_str("ToggleChat"),
            Self::ToggleTranslation => f.write_str("ToggleTranslation"),
            Self::CopyText(_) => f.write_str("CopyText"),
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
    Chat,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    Name,
    Color,
    PeerCode,
    RelayUrl,
    RelayToken,
    OwnClaimColor,
    FriendClaimColor,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PartyFilter {
    Friends,
    #[default]
    Users,
    Invitations,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ChatSection {
    #[default]
    Appearance,
    Notifications,
    Spam,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChatOption {
    Width,
    Height,
    Scale,
    OffsetX,
    OffsetY,
    Duration,
    Opacity,
    EventWidth,
    RepeatWindow,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    PartyFilter(PartyFilter),
    ToggleAutoRelay,
    ClaimsSection(ClaimsSection),
    OpenMap,
    ClaimRow(usize),
    ClaimsPrevious,
    ClaimsNext,
    MapToggle,
    MapCorner,
    MapLayer,
    MapMarks,
    MapRemember,
    MapShading,
    MapMobs,
    MapProfile,
    ClearClaimCache,
    MapGrid,
    MapCoordinates,
    MapOwn,
    MapFriends,
    MapLabels,
    MapSort,
    MapDimensions,
    MapReset,
    BorderGrid,
    MapAdjust(MapOption, i8),
    BorderAdjust(BorderOption, i8),
    ApplyClaimColours,
    BorderReset,
    ChatSection(ChatSection),
    ChatAdjust(ChatOption, i8),
    ChatPlacement,
    ChatStyle,
    ChatPresence,
    ChatDeaths,
    ChatRepeats,
    ChatPrefix,
    ChatReset,
    ChatModern,
    ChatTranslate,
    ChatOutgoing,
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
    Decline,
    Accept,
    Remove,
    Refresh,
    RelayOff,
    RelaySetup,
    RelaySave,
    RelayConnect,
    RelayBack,
    ClearRelayToken,
    BordersEnabled,
    OwnVisible,
    FriendsVisible,
    ThroughWalls,
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
    pub party_filter: PartyFilter,
    pub chat_section: ChatSection,
    pub claims_section: ClaimsSection,
    pub claims_page: usize,
    pub own_claim_color: String,
    pub friend_claim_color: String,
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
    pub caret: usize,
    pub anchor: Option<usize>,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            tab: Tab::Waypoints,
            party_filter: PartyFilter::Users,
            chat_section: ChatSection::Appearance,
            claims_section: ClaimsSection::Map,
            claims_page: 0,
            own_claim_color: "#55FF88".into(),
            friend_claim_color: "#55BBFF".into(),
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
            caret: usize::MAX,
            anchor: None,
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
            Tab::Claims | Tab::Chat => None,
        });
        if self.tab == Tab::Party
            && self
                .selected
                .is_some_and(|i| !self.party_indices(next).contains(&i))
        {
            self.selected = None;
        }
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
            (((height - 176.0) / 24.0).floor() as usize).clamp(1, 12)
        } else {
            Self::rows_per_page(height)
        }
    }
    fn count(&self, model: &ViewModel) -> usize {
        match self.tab {
            Tab::Waypoints => model.waypoints.len(),
            Tab::Party => self.party_indices(model).len(),
            Tab::Claims | Tab::Chat => 0,
        }
    }
    pub fn layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        let panel_w = (width - 16.0).clamp(220.0, 640.0);
        let panel_h = (height - 16.0).clamp(200.0, 400.0);
        let sidebar = if panel_w >= 380.0 { 78.0 } else { 0.0 };
        let x = (width - panel_w) * 0.5;
        let y = (height - panel_h) * 0.5;
        let mut widgets = self.legacy_layout(model, panel_w - sidebar, panel_h);
        for widget in &mut widgets {
            widget.rect[0] += x + sidebar;
            widget.rect[1] += y;
            if sidebar > 0.0
                && let Some(Control::Tab(tab)) = widget.control
            {
                let i = match tab {
                    Tab::Waypoints => 0,
                    Tab::Party => 1,
                    Tab::Claims => 2,
                    Tab::Chat => 3,
                };
                widget.rect = [x + 6.0, y + 44.0 + i as f32 * 26.0, sidebar - 10.0, 22.0];
            }
        }
        widgets
    }
    fn legacy_layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        let w = (width - 16.0).clamp(220.0, 590.0);
        let x = (width - w) * 0.5;
        let y = 8.0;
        let mut out = vec![
            widget([x, y, w - 30.0, 18.0], "HORIZON  /  SOCIAL", None, false),
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
            (Tab::Chat, "Чат"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 4.0, y + 22.0, w / 4.0 - 3.0, 20.0],
                label,
                Some(Control::Tab(tab)),
                self.tab == tab,
            ));
        }
        if self.tab == Tab::Chat {
            return self.chat_layout(model, x, w, height, out);
        }
        if self.tab == Tab::Party {
            return self.party_layout(model, width, height, out);
        }
        if self.tab == Tab::Claims {
            return self.claims_layout(model, x, w, height, out);
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
    fn claims_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        for (i, (section, label)) in [
            (ClaimsSection::Map, "Карта"),
            (ClaimsSection::Borders, "Границы"),
            (ClaimsSection::Colours, "Цвета"),
            (ClaimsSection::List, "Приваты"),
            (ClaimsSection::Storage, "Память"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 5.0, 56.0, w / 5.0 - 3.0, 18.0],
                label,
                Some(Control::ClaimsSection(section)),
                self.claims_section == section,
            ));
        }
        let p = model.map_preferences;
        let v = &model.claims;
        let pair = |a: String, ca: Control, b: String, cb: Control| vec![(a, ca), (b, cb)];
        let adjust = |name: &str, value: String, a: Control, b: Control| {
            pair(
                format!("{name} − {value}"),
                a,
                format!("{name} + {value}"),
                b,
            )
        };
        let rows = match self.claims_section {
            ClaimsSection::Map => vec![
                pair(
                    "Открыть карту · J".into(),
                    Control::OpenMap,
                    "Обновить /claims".into(),
                    Control::Refresh,
                ),
                pair(
                    format!("Миникарта: {}", on(p.minimap)),
                    Control::MapToggle,
                    format!(
                        "Угол: {}",
                        ["Слева ↑", "Справа ↑", "Слева ↓", "Справа ↓"][p.corner as usize]
                    ),
                    Control::MapCorner,
                ),
                adjust(
                    "Размер",
                    p.size.to_string(),
                    Control::MapAdjust(MapOption::Size, -1),
                    Control::MapAdjust(MapOption::Size, 1),
                ),
                adjust(
                    "Блоков/пикс.",
                    format!("{:.1}", p.zoom),
                    Control::MapAdjust(MapOption::Zoom, -1),
                    Control::MapAdjust(MapOption::Zoom, 1),
                ),
                pair(
                    format!("Рельеф: {}", p.layer.label()),
                    Control::MapLayer,
                    format!("Метки: {}", on(p.marks)),
                    Control::MapMarks,
                ),
                pair(
                    format!("Сетка чанков: {}", on(p.grid)),
                    Control::MapGrid,
                    format!("Координаты: {}", on(p.coordinates)),
                    Control::MapCoordinates,
                ),
                pair(
                    format!("Тени: {}", on(p.shading)),
                    Control::MapShading,
                    format!("Мобы: {}", ["выкл.", "все", "опасные"][p.mobs as usize]),
                    Control::MapMobs,
                ),
                vec![("Сбросить настройки карты".into(), Control::MapReset)],
            ],
            ClaimsSection::Storage => vec![
                vec![(
                    "Сбросить кеш приватов игроков".into(),
                    Control::ClearClaimCache,
                )],
                vec![(
                    format!("Запись карты: {}", on(p.remember)),
                    Control::MapRemember,
                )],
                vec![(
                    format!("Атлас мира: {} / 8", p.profile + 1),
                    Control::MapProfile,
                )],
            ],
            ClaimsSection::Borders => vec![
                pair(
                    format!("Границы 3D: {}", on(v.enabled)),
                    Control::BordersEnabled,
                    format!("Сквозь блоки: {}", on(v.through_walls)),
                    Control::ThroughWalls,
                ),
                pair(
                    format!("Свои 3D: {}", on(v.own_visible)),
                    Control::OwnVisible,
                    format!("Друзья 3D: {}", on(v.friends_visible)),
                    Control::FriendsVisible,
                ),
                adjust(
                    "Дальность",
                    format!("{:.0}", v.distance),
                    Control::BorderAdjust(BorderOption::Distance, -1),
                    Control::BorderAdjust(BorderOption::Distance, 1),
                ),
                adjust(
                    "Высота",
                    if v.height >= 384.0 {
                        "весь мир".into()
                    } else {
                        format!("±{:.0}", v.height / 2.0)
                    },
                    Control::BorderAdjust(BorderOption::Height, -1),
                    Control::BorderAdjust(BorderOption::Height, 1),
                ),
                adjust(
                    "Толщина",
                    format!("{:.0}", v.line_width),
                    Control::BorderAdjust(BorderOption::Width, -1),
                    Control::BorderAdjust(BorderOption::Width, 1),
                ),
                adjust(
                    "Непрозрачность",
                    format!("{}%", (v.opacity * 100.0).round() as u8),
                    Control::BorderAdjust(BorderOption::Opacity, -1),
                    Control::BorderAdjust(BorderOption::Opacity, 1),
                ),
                pair(
                    format!(
                        "Шаг сетки: {}",
                        if p.grid_step == 0 {
                            "только контур".into()
                        } else {
                            p.grid_step.to_string()
                        }
                    ),
                    Control::BorderGrid,
                    "Сбросить 3D".into(),
                    Control::BorderReset,
                ),
            ],
            ClaimsSection::Colours => vec![
                pair(
                    format!("Свои на карте: {}", on(p.own)),
                    Control::MapOwn,
                    format!("Игроки на карте: {}", on(p.friends)),
                    Control::MapFriends,
                ),
                vec![(
                    format!("Свои: {}", self.own_claim_color),
                    Control::Field(Field::OwnClaimColor),
                )],
                vec![(
                    format!("Друзья: {}", self.friend_claim_color),
                    Control::Field(Field::FriendClaimColor),
                )],
                vec![("Применить цвета #RRGGBB".into(), Control::ApplyClaimColours)],
                adjust(
                    "Заливка карты",
                    format!("{}%", (p.fill * 100.0).round() as u8),
                    Control::MapAdjust(MapOption::Fill, -1),
                    Control::MapAdjust(MapOption::Fill, 1),
                ),
                adjust(
                    "Контур карты",
                    p.outline.to_string(),
                    Control::MapAdjust(MapOption::Outline, -1),
                    Control::MapAdjust(MapOption::Outline, 1),
                ),
                vec![(
                    format!("Подписи владельцев: {}", on(p.labels)),
                    Control::MapLabels,
                )],
            ],
            ClaimsSection::List => {
                out.push(widget(
                    [x, 80.0, w * 0.62 - 3.0, 18.0],
                    format!("Сортировка: {}", p.sort.label()),
                    Some(Control::MapSort),
                    false,
                ));
                out.push(widget(
                    [x + w * 0.62, 80.0, w * 0.38 - 3.0, 18.0],
                    if p.all_dimensions {
                        "Все миры"
                    } else {
                        "Этот мир"
                    },
                    Some(Control::MapDimensions),
                    false,
                ));
                let per = (((height - 140.0) / 20.0).floor() as usize).clamp(2, 10);
                let pages = model.claim_rows.len().div_ceil(per).max(1);
                let page = self.claims_page.min(pages - 1);
                for (slot, index) in
                    (page * per..((page + 1) * per).min(model.claim_rows.len())).enumerate()
                {
                    out.push(widget(
                        [x, 102.0 + slot as f32 * 20.0, w, 18.0],
                        &model.claim_rows[index].label,
                        Some(Control::ClaimRow(index)),
                        false,
                    ));
                }
                if model.claim_rows.is_empty() {
                    out.push(widget(
                        [x, 112.0, w, 18.0],
                        "Нет данных. Нажми «Обновить /claims» в «Карта».",
                        None,
                        false,
                    ));
                }
                out.push(widget(
                    [x, height - 30.0, w * 0.3 - 3.0, 18.0],
                    "←",
                    Some(Control::ClaimsPrevious),
                    false,
                ));
                out.push(widget(
                    [x + w * 0.3, height - 30.0, w * 0.4 - 3.0, 18.0],
                    format!("{}/{} · {} чанков", page + 1, pages, model.claim_rows.len()),
                    None,
                    false,
                ));
                out.push(widget(
                    [x + w * 0.7, height - 30.0, w * 0.3, 18.0],
                    "→",
                    Some(Control::ClaimsNext),
                    false,
                ));
                return out;
            }
        };
        if self.claims_section == ClaimsSection::Storage {
            for (i, text) in [
                "Сохраняется местность рядом с игроком.",
                "Серверы, аккаунты и измерения разделены.",
                "Другой мир на том же сервере? Выбери атлас.",
                "Смена атласа сохраняет предыдущую карту.",
                "Выключение записи не удаляет файлы.",
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x, 150.0 + i as f32 * 12.0, w, 10.0],
                    text,
                    None,
                    false,
                ));
            }
        }
        let step = ((height - 88.0) / rows.len() as f32).clamp(12.0, 20.0);
        for (i, row) in rows.into_iter().enumerate() {
            let n = row.len();
            for (j, (label, control)) in row.into_iter().enumerate() {
                out.push(widget(
                    [
                        x + j as f32 * w / n as f32,
                        80.0 + i as f32 * step,
                        w / n as f32 - 3.0,
                        step - 2.0,
                    ],
                    label,
                    Some(control),
                    self.focus.is_some()
                        && matches!(control,Control::Field(f) if self.focus==Some(f)),
                ));
            }
        }
        out
    }
    fn chat_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        for (i, (section, label)) in [
            (ChatSection::Appearance, "Вид / позиция"),
            (ChatSection::Notifications, "Уведомления"),
            (ChatSection::Spam, "Антиспам"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 3.0, 56.0, w / 3.0 - 3.0, 18.0],
                label,
                Some(Control::ChatSection(section)),
                self.chat_section == section,
            ));
        }
        let p = model.chat_preferences;
        let pair = |field: ChatOption, name: &str, value: String| {
            vec![
                (
                    format!("{name} − {value}"),
                    Control::ChatAdjust(field, -1),
                    false,
                ),
                (
                    format!("{name} + {value}"),
                    Control::ChatAdjust(field, 1),
                    false,
                ),
            ]
        };
        let rows = match self.chat_section {
            ChatSection::Appearance => vec![
                vec![
                    (
                        format!("Чат: {}", on(model.chat_modern)),
                        Control::ChatModern,
                        model.chat_modern,
                    ),
                    (
                        format!("Google→RU: {}", on(model.chat_translate)),
                        Control::ChatTranslate,
                        model.chat_translate,
                    ),
                ],
                vec![(
                    format!("Мои сообщения RU → EN: {}", on(model.chat_outgoing)),
                    Control::ChatOutgoing,
                    model.chat_outgoing,
                )],
                pair(ChatOption::Width, "Ширина", p.width.to_string()),
                pair(ChatOption::Height, "Высота", p.height.to_string()),
                pair(
                    ChatOption::Scale,
                    "Масштаб",
                    format!("{}%", (p.scale * 100.0).round() as u16),
                ),
                vec![(
                    format!("Положение: {}", p.placement.label()),
                    Control::ChatPlacement,
                    false,
                )],
                pair(ChatOption::OffsetX, "Сдвиг X", p.offset_x.to_string()),
                pair(ChatOption::OffsetY, "Сдвиг Y", p.offset_y.to_string()),
            ],
            ChatSection::Notifications => vec![
                vec![(
                    format!("Вид: {}", p.notifications.label()),
                    Control::ChatStyle,
                    false,
                )],
                vec![(
                    format!("Входы / выходы: {}", on(p.presence)),
                    Control::ChatPresence,
                    p.presence,
                )],
                vec![(
                    format!("Смерти: {}", on(p.deaths)),
                    Control::ChatDeaths,
                    p.deaths,
                )],
                pair(ChatOption::Duration, "Время", format!("{} с", p.duration)),
                pair(
                    ChatOption::Opacity,
                    "Фон",
                    format!("{}%", (p.opacity * 100.0).round() as u16),
                ),
                pair(
                    ChatOption::EventWidth,
                    "Ширина событий",
                    p.event_width.to_string(),
                ),
            ],
            ChatSection::Spam => vec![
                vec![(
                    format!("Режим: {}", p.repeats.label()),
                    Control::ChatRepeats,
                    false,
                )],
                pair(
                    ChatOption::RepeatWindow,
                    "Интервал",
                    format!("{} с", p.repeat_window),
                ),
                vec![(
                    format!("Игнорировать префикс: {} симв.", p.ignore_prefix),
                    Control::ChatPrefix,
                    false,
                )],
            ],
        };
        let step = ((height - 88.0) / rows.len() as f32).clamp(12.0, 20.0);
        for (i, row) in rows.into_iter().enumerate() {
            let n = row.len();
            for (j, (label, control, selected)) in row.into_iter().enumerate() {
                out.push(widget(
                    [
                        x + j as f32 * w / n as f32,
                        80.0 + i as f32 * step,
                        w / n as f32 - 3.0,
                        step - 2.0,
                    ],
                    label,
                    Some(control),
                    selected,
                ));
            }
        }
        if self.chat_section == ChatSection::Spam {
            out.push(widget(
                [x, 206.0, w - 3.0, 18.0],
                "Сбросить настройки чата",
                Some(Control::ChatReset),
                false,
            ));
            for (i, label) in [
                "Одинаковые сообщения одного автора → ×N",
                "ЛКМ по сообщению: оригинал / перевод",
                "Префикс 10 / 11 — для времени [12:34:56]",
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x, 146.0 + i as f32 * 20.0, w, 18.0],
                    label,
                    None,
                    false,
                ));
            }
        }
        out
    }
    fn party_indices(&self, model: &ViewModel) -> Vec<usize> {
        model
            .party
            .iter()
            .enumerate()
            .filter(|(_, p)| match self.party_filter {
                PartyFilter::Friends => p.friend,
                PartyFilter::Users => p.online && !p.pending,
                PartyFilter::Invitations => p.pending,
            })
            .map(|(i, _)| i)
            .collect()
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
                [x + w / 2.0, 186.0, w / 2.0 - 3.0, 22.0],
                format!("Автоподключение: {}", on(model.relay_auto)),
                Some(Control::ToggleAutoRelay),
                model.relay_auto,
            ));
            out.push(widget(
                [x, 212.0, w, 10.0],
                "После сохранения relay подключается автоматически.",
                None,
                false,
            ));
        } else {
            out.push(widget([x, 54.0, w, 14.0], &model.relay, None, false));
            out.push(widget(
                [x, 72.0, w / 2.0 - 3.0, 18.0],
                "Настроить relay",
                Some(Control::RelaySetup),
                false,
            ));
            out.push(widget(
                [x + w / 2.0, 72.0, w / 2.0, 18.0],
                if model.relay_connected {
                    "Отключить"
                } else {
                    "Подключить"
                },
                Some(if model.relay_connected {
                    Control::RelayOff
                } else {
                    Control::RelayConnect
                }),
                false,
            ));
            for (i, (filter, title)) in [
                (PartyFilter::Friends, "Друзья"),
                (PartyFilter::Users, "С модом"),
                (PartyFilter::Invitations, "Приглашения"),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x + i as f32 * w / 3.0, 94.0, w / 3.0 - 3.0, 18.0],
                    title,
                    Some(Control::PartyFilter(filter)),
                    self.party_filter == filter,
                ));
            }
            let indices = self.party_indices(model);
            let per = self.page_size(height);
            let pages = indices.len().div_ceil(per).max(1);
            let page = self.page.min(pages - 1);
            for (slot, index) in indices
                .iter()
                .copied()
                .skip(page * per)
                .take(per)
                .enumerate()
            {
                let row = &model.party[index];
                out.push(widget(
                    [x, 118.0 + slot as f32 * 24.0, w, 22.0],
                    format!("{} · {}", row.name, row.detail),
                    Some(Control::Row(index)),
                    self.selected == Some(index),
                ));
            }
            if indices.is_empty() {
                out.push(widget(
                    [x, 120.0, w, 18.0],
                    match self.party_filter {
                        PartyFilter::Friends => "Пока нет друзей. Выбери игрока в «С модом».",
                        PartyFilter::Users => "Здесь появятся игроки вашего relay на этом сервере.",
                        PartyFilter::Invitations => "Нет входящих приглашений.",
                    },
                    None,
                    false,
                ));
            }
            let b = 118.0 + per as f32 * 24.0;
            out.push(widget(
                [x, b, 28.0, 16.0],
                "←",
                Some(Control::Previous),
                false,
            ));
            out.push(widget(
                [x + 32.0, b, w - 64.0, 16.0],
                format!("{} / {}", page + 1, pages),
                None,
                false,
            ));
            out.push(widget(
                [x + w - 28.0, b, 28.0, 16.0],
                "→",
                Some(Control::Next),
                false,
            ));
            let selected = self.selected.and_then(|i| model.party.get(i));
            let (primary, title) = match selected {
                Some(p) if p.pending => (Control::Accept, "Принять приглашение"),
                Some(p) if p.friend => (Control::Remove, "Удалить друга"),
                _ => (Control::Invite, "Пригласить игрока"),
            };
            out.push(widget(
                [x, height - 38.0, w * 0.62 - 3.0, 18.0],
                title,
                Some(primary),
                false,
            ));
            out.push(widget(
                [x + w * 0.62, height - 38.0, w * 0.38, 18.0],
                if selected.is_some_and(|p| p.pending) {
                    "Отклонить"
                } else {
                    "Обновить приваты"
                },
                Some(if selected.is_some_and(|p| p.pending) {
                    Control::Decline
                } else {
                    Control::Refresh
                }),
                false,
            ));
        }
        if !self.relay_setup {
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
        }
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
        self.activate(model, (size[1] - 16.0).clamp(200.0, 400.0), control)
    }
    fn activate(&mut self, model: &ViewModel, height: f32, control: Control) -> Option<Action> {
        self.notice.clear();
        let id = self
            .selected
            .and_then(|i| model.waypoints.get(i))
            .map(|r| r.id);
        let name = self.name.trim().to_owned();
        let color = u32::from_str_radix(self.color.trim().trim_start_matches('#'), 16)
            .ok()
            .filter(|_| self.color.trim().trim_start_matches('#').len() == 6);
        match control {
            Control::Decline => self
                .selected
                .and_then(|i| model.party.get(i))
                .filter(|p| p.pending)
                .map(|p| Action::Decline { uuid: p.uuid }),
            Control::PartyFilter(filter) => {
                self.party_filter = filter;
                self.selected = None;
                self.page = 0;
                self.focus = None;
                None
            }
            Control::ToggleAutoRelay => Some(Action::ToggleAutoRelay),
            Control::ClaimsSection(section) => {
                self.claims_section = section;
                self.focus = None;
                self.claims_page = 0;
                if section == ClaimsSection::Colours {
                    self.own_claim_color = format!("#{:06X}", model.claims.own_color);
                    self.friend_claim_color = format!("#{:06X}", model.claims.friend_color);
                }
                None
            }
            Control::OpenMap => Some(Action::OpenMap),
            Control::ClaimRow(index) => model.claim_rows.get(index).map(|c| Action::FocusClaim {
                dimension: c.dimension.clone(),
                x: c.x,
                z: c.z,
            }),
            Control::ClaimsPrevious => {
                self.claims_page = self.claims_page.saturating_sub(1);
                None
            }
            Control::ClaimsNext => {
                let per = (((height - 140.0) / 20.0).floor() as usize).clamp(2, 10);
                self.claims_page =
                    (self.claims_page + 1).min(model.claim_rows.len().div_ceil(per).max(1) - 1);
                None
            }
            Control::ClearClaimCache => Some(Action::ClearClaimCache),
            Control::MapReset => Some(Action::SetMap(map::Preferences::default())),
            Control::BorderReset => Some(Action::SetClaims(ClaimsSettings::default())),
            Control::ApplyClaimColours => {
                let parse = |s: &str| {
                    s.trim()
                        .strip_prefix('#')
                        .unwrap_or(s.trim())
                        .len()
                        .eq(&6)
                        .then(|| u32::from_str_radix(s.trim().trim_start_matches('#'), 16).ok())
                        .flatten()
                };
                if let (Some(own), Some(friend)) = (
                    parse(&self.own_claim_color),
                    parse(&self.friend_claim_color),
                ) {
                    let mut v = model.claims.clone();
                    v.own_color = own;
                    v.friend_color = friend;
                    Some(Action::SetClaims(v))
                } else {
                    self.notice = "Цвет должен быть #RRGGBB".into();
                    None
                }
            }
            Control::MapToggle
            | Control::MapCorner
            | Control::MapLayer
            | Control::MapRemember
            | Control::MapProfile
            | Control::MapMobs
            | Control::MapShading
            | Control::MapMarks
            | Control::MapGrid
            | Control::MapCoordinates
            | Control::MapOwn
            | Control::MapFriends
            | Control::MapLabels
            | Control::MapSort
            | Control::MapDimensions
            | Control::BorderGrid => {
                let mut p = model.map_preferences;
                match control {
                    Control::MapToggle => p.minimap = !p.minimap,
                    Control::MapCorner => p.corner = (p.corner + 1) % 4,
                    Control::MapLayer => p.layer = p.layer.next(),
                    Control::MapRemember => p.remember = !p.remember,
                    Control::MapProfile => p.profile = (p.profile + 1) % 8,
                    Control::MapMobs => p.mobs = (p.mobs + 1) % 3,
                    Control::MapShading => p.shading = !p.shading,
                    Control::MapMarks => p.marks = !p.marks,
                    Control::MapGrid => p.grid = !p.grid,
                    Control::MapCoordinates => p.coordinates = !p.coordinates,
                    Control::MapOwn => p.own = !p.own,
                    Control::MapFriends => p.friends = !p.friends,
                    Control::MapLabels => p.labels = !p.labels,
                    Control::MapSort => p.sort = p.sort.next(),
                    Control::MapDimensions => p.all_dimensions = !p.all_dimensions,
                    Control::BorderGrid => {
                        p.grid_step = match p.grid_step {
                            8 => 16,
                            16 => 32,
                            32 => 0,
                            _ => 8,
                        }
                    }
                    _ => {}
                }
                Some(Action::SetMap(p))
            }
            Control::MapAdjust(field, d) => {
                let mut p = model.map_preferences;
                match field {
                    MapOption::Size => {
                        p.size = (i32::from(p.size) + i32::from(d) * 16).max(0) as u16
                    }
                    MapOption::Zoom => p.zoom *= if d > 0 { 2.0 } else { 0.5 },
                    MapOption::Fill => p.fill += f32::from(d) * 0.05,
                    MapOption::Outline => {
                        p.outline = (i16::from(p.outline) + i16::from(d)).clamp(1, 3) as u8
                    }
                }
                Some(Action::SetMap(p.normalized()))
            }
            Control::BorderAdjust(field, d) => {
                let mut v = model.claims.clone();
                let choose = |old: f32, values: &[f32]| {
                    let index = values.iter().position(|v| *v == old).unwrap_or(0);
                    values[(index as i32 + i32::from(d)).clamp(0, values.len() as i32 - 1) as usize]
                };
                match field {
                    BorderOption::Distance => {
                        v.distance = choose(v.distance, &[64.0, 160.0, 256.0, 512.0])
                    }
                    BorderOption::Height => v.height = choose(v.height, &[16.0, 32.0, 64.0, 384.0]),
                    BorderOption::Opacity => {
                        v.opacity = (v.opacity + f32::from(d) * 0.05).clamp(0.05, 1.0)
                    }
                    BorderOption::Width => {
                        v.line_width = (v.line_width + f32::from(d)).clamp(1.0, 6.0)
                    }
                }
                Some(Action::SetClaims(v))
            }

            Control::ChatSection(section) => {
                self.chat_section = section;
                None
            }
            Control::ChatReset => Some(Action::SetChat(Preferences::default())),
            Control::ChatPlacement => {
                let mut p = model.chat_preferences;
                p.placement = p.placement.next();
                Some(Action::SetChat(p))
            }
            Control::ChatStyle => {
                let mut p = model.chat_preferences;
                p.notifications = p.notifications.next();
                Some(Action::SetChat(p))
            }
            Control::ChatPresence => {
                let mut p = model.chat_preferences;
                p.presence = !p.presence;
                Some(Action::SetChat(p))
            }
            Control::ChatDeaths => {
                let mut p = model.chat_preferences;
                p.deaths = !p.deaths;
                Some(Action::SetChat(p))
            }
            Control::ChatRepeats => {
                let mut p = model.chat_preferences;
                p.repeats = p.repeats.next();
                Some(Action::SetChat(p))
            }
            Control::ChatPrefix => {
                let mut p = model.chat_preferences;
                p.ignore_prefix = match p.ignore_prefix {
                    0 => 10,
                    10 => 11,
                    _ => 0,
                };
                Some(Action::SetChat(p))
            }
            Control::ChatAdjust(field, delta) => {
                let mut p = model.chat_preferences;
                let d = i32::from(delta);
                let adjust =
                    |v: u16, step: i32| (i32::from(v) + d * step).clamp(0, u16::MAX as i32) as u16;
                match field {
                    ChatOption::Width => p.width = adjust(p.width, 20),
                    ChatOption::Height => p.height = adjust(p.height, 20),
                    ChatOption::Scale => {
                        p.scale = ((p.scale * 100.0 + delta as f32 * 5.0).round()) / 100.0
                    }
                    ChatOption::Opacity => p.opacity += delta as f32 * 0.05,
                    ChatOption::OffsetX => {
                        p.offset_x = p.offset_x.saturating_add(i16::from(delta) * 8)
                    }
                    ChatOption::OffsetY => {
                        p.offset_y = p.offset_y.saturating_add(i16::from(delta) * 8)
                    }
                    ChatOption::Duration => p.duration = adjust(p.duration, 1),
                    ChatOption::EventWidth => p.event_width = adjust(p.event_width, 10),
                    ChatOption::RepeatWindow => p.repeat_window = adjust(p.repeat_window, 2),
                }
                Some(Action::SetChat(p.normalized()))
            }

            Control::ChatOutgoing => Some(Action::ToggleOutgoing),
            Control::ChatModern => Some(Action::ToggleChat),
            Control::ChatTranslate => Some(Action::ToggleTranslation),
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
                self.caret = usize::MAX;
                self.anchor = None;
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
                let Some(row) = self.selected.and_then(|i| model.party.get(i)) else {
                    self.notice = "Выбери игрока из списка".into();
                    return None;
                };
                if row.blocked || row.code.is_empty() || !row.online {
                    self.notice = "Игрок сейчас недоступен".into();
                    return None;
                }
                if control == Control::Accept {
                    if !row.pending {
                        return None;
                    }
                    Some(Action::Accept {
                        code: row.code.clone(),
                    })
                } else {
                    if row.friend || row.pending || row.outgoing {
                        return None;
                    }
                    Some(Action::Invite {
                        code: row.code.clone(),
                    })
                }
            }
            Control::Remove => {
                let Some(row) = self
                    .selected
                    .and_then(|i| model.party.get(i))
                    .filter(|row| row.friend)
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
            _ => {
                let mut s = model.claims.clone();
                match control {
                    Control::BordersEnabled => s.enabled = !s.enabled,
                    Control::OwnVisible => s.own_visible = !s.own_visible,
                    Control::FriendsVisible => s.friends_visible = !s.friends_visible,
                    Control::ThroughWalls => s.through_walls = !s.through_walls,
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
            Field::OwnClaimColor => &mut self.own_claim_color,
            Field::FriendClaimColor => &mut self.friend_claim_color,
        })
    }
    pub fn text(&self) -> &str {
        match self.focus {
            Some(Field::Name) => &self.name,
            Some(Field::Color) => &self.color,
            Some(Field::PeerCode) => &self.peer_code,
            Some(Field::RelayUrl) => &self.relay_url,
            Some(Field::RelayToken) => &self.access_token,
            Some(Field::OwnClaimColor) => &self.own_claim_color,
            Some(Field::FriendClaimColor) => &self.friend_claim_color,
            None => "",
        }
    }
    pub fn cursor(&self) -> usize {
        self.caret.min(self.text().len())
    }
    pub fn selection(&self) -> std::ops::Range<usize> {
        let cursor = self.cursor();
        let anchor = self.anchor.unwrap_or(cursor).min(self.text().len());
        cursor.min(anchor)..cursor.max(anchor)
    }
    pub fn selected_text(&self) -> String {
        self.text()[self.selection()].to_owned()
    }
    pub fn delete_selection(&mut self) {
        let range = self.selection();
        self.caret = range.start;
        self.anchor = None;
        if let Some(text) = self.focused_text() {
            text.replace_range(range, "");
        }
    }
    pub fn paste(&mut self, text: &str) {
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
            Field::Color | Field::OwnClaimColor | Field::FriendClaimColor => 7,
            Field::PeerCode => 24,
            Field::RelayUrl => 2048,
            Field::RelayToken => 1024,
        };
        if (matches!(
            field,
            Field::Color | Field::OwnClaimColor | Field::FriendClaimColor
        ) && !(c.is_ascii_hexdigit() || c == '#'))
            || (field == Field::PeerCode && !c.is_ascii_hexdigit())
        {
            return;
        }
        let range = self.selection();
        let count = self.text().chars().count() - self.text()[range.clone()].chars().count();
        if count >= limit {
            return;
        }
        self.caret = range.start + c.len_utf8();
        self.anchor = None;
        if let Some(text) = self.focused_text() {
            text.replace_range(range, &c.to_string());
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
            } else if self.tab == Tab::Claims && self.claims_section == ClaimsSection::Colours {
                if self.focus == Some(Field::OwnClaimColor) {
                    Field::FriendClaimColor
                } else {
                    Field::OwnClaimColor
                }
            } else if self.tab == Tab::Party {
                self.focus = None;
                return true;
            } else if self.focus == Some(Field::Name) {
                Field::Color
            } else {
                Field::Name
            });
            self.caret = usize::MAX;
            self.anchor = None;
            return true;
        }
        if self.focus.is_none() {
            return true;
        }
        let cursor = self.cursor();
        let range = self.selection();
        let shift = modifiers & 1 != 0;
        let control = modifiers & (2 | 8) != 0;
        if key == "key.keyboard.a" && control {
            self.anchor = Some(0);
            self.caret = self.text().len();
            return true;
        }
        let previous = self.text()[..cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        let next = self.text()[cursor..]
            .chars()
            .next()
            .map_or(cursor, |c| cursor + c.len_utf8());
        let target = match key {
            "key.keyboard.left" => Some(if !shift && !range.is_empty() {
                range.start
            } else if control {
                0
            } else {
                previous
            }),
            "key.keyboard.right" => Some(if !shift && !range.is_empty() {
                range.end
            } else if control {
                self.text().len()
            } else {
                next
            }),
            "key.keyboard.home" => Some(0),
            "key.keyboard.end" => Some(self.text().len()),
            _ => None,
        };
        if let Some(target) = target {
            self.anchor = if shift {
                Some(self.anchor.unwrap_or(cursor))
            } else {
                None
            };
            self.caret = target;
            return true;
        }
        if matches!(key, "key.keyboard.backspace" | "key.keyboard.delete") {
            if !range.is_empty() {
                self.delete_selection();
            } else {
                let deletion = if key.ends_with("backspace") {
                    previous..cursor
                } else {
                    cursor..next
                };
                self.caret = deletion.start;
                self.anchor = None;
                if let Some(text) = self.focused_text() {
                    text.replace_range(deletion, "");
                }
            }
        }
        true
    }
}
fn on(value: bool) -> &'static str {
    if value { "вкл." } else { "выкл." }
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
        let Some(Action::SetClaims(updated)) = menu.activate(
            &model,
            240.0,
            Control::BorderAdjust(BorderOption::Opacity, 1),
        ) else {
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
        for tab in [Tab::Waypoints, Tab::Party, Tab::Claims, Tab::Chat] {
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
    fn selecting_an_invitation_does_not_accept_until_explicit_click() {
        let model = ViewModel {
            party: vec![PartyRow {
                uuid: 42,
                name: "Friend".into(),
                code: "abcdef0123456789abcdef01".into(),
                pending: true,
                online: true,
                ..PartyRow::default()
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            party_filter: PartyFilter::Invitations,
            ..Menu::default()
        };
        assert_eq!(menu.activate(&model, 240.0, Control::Row(0)), None);
        assert!(menu.peer_code.is_empty());
        assert_eq!(
            menu.activate(&model, 240.0, Control::Accept),
            Some(Action::Accept {
                code: model.party[0].code.clone()
            })
        );
        assert_eq!(menu.activate(&model, 240.0, Control::Invite), None);
    }
    #[test]
    fn remove_uses_selected_friend_and_never_peer_code_input() {
        let model = ViewModel {
            party: vec![PartyRow {
                name: "SavedFriend".into(),
                code: "a".repeat(24),
                detail: String::new(),
                pending: false,
                friend: true,
                ..PartyRow::default()
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
    fn users_are_invited_by_selection_without_exposing_codes() {
        let mut model = ViewModel {
            party: vec![PartyRow {
                uuid: 42,
                name: "Friend".into(),
                code: "abcdef0123456789abcdef01".into(),
                online: true,
                ..PartyRow::default()
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        assert_eq!(menu.activate(&model, 240.0, Control::Invite), None);
        menu.activate(&model, 240.0, Control::Row(0));
        assert_eq!(
            menu.activate(&model, 240.0, Control::Invite),
            Some(Action::Invite {
                code: model.party[0].code.clone()
            })
        );
        for w in menu.layout(&model, 320.0, 240.0) {
            assert!(!w.text.contains(&model.party[0].code));
            assert_ne!(w.control, Some(Control::Field(Field::PeerCode)));
        }
        model.party[0].blocked = true;
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
        assert_eq!(m.selection(), 0..m.name.len());
        m.character('й');
        assert_eq!(m.name, "й");
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
    #[test]
    fn edits_in_middle_and_unicode_selection_work() {
        let mut m = Menu {
            name: "ksadk1a2s".into(),
            focus: Some(Field::Name),
            ..Menu::default()
        };
        m.key("key.keyboard.home", 0);
        m.key("key.keyboard.delete", 0);
        m.character('K');
        assert_eq!(m.name, "Ksadk1a2s");
        m.key("key.keyboard.right", 1);
        m.key("key.keyboard.right", 1);
        assert_eq!(m.selected_text(), "sa");
        m.paste("я");
        assert_eq!(m.name, "Kяdk1a2s");
        m.key("key.keyboard.left", 0);
        m.key("key.keyboard.delete", 0);
        assert_eq!(m.name, "Kdk1a2s");
    }
    #[test]
    fn all_chat_settings_pages_fit_small_gui_and_keep_controls_separate() {
        for section in [
            ChatSection::Appearance,
            ChatSection::Notifications,
            ChatSection::Spam,
        ] {
            let menu = Menu {
                tab: Tab::Chat,
                chat_section: section,
                ..Menu::default()
            };
            let widgets = menu.layout(&ViewModel::default(), 320.0, 240.0);
            for widget in &widgets {
                let [x, y, w, h] = widget.rect;
                assert!(x >= 0.0 && y >= 0.0 && x + w <= 320.0 && y + h <= 240.0);
            }
            let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
            for (i, a) in controls.iter().enumerate() {
                for b in controls.iter().skip(i + 1) {
                    assert!(
                        a.rect[0] + a.rect[2] <= b.rect[0]
                            || b.rect[0] + b.rect[2] <= a.rect[0]
                            || a.rect[1] + a.rect[3] <= b.rect[1]
                            || b.rect[1] + b.rect[3] <= a.rect[1],
                        "overlapping controls"
                    );
                }
            }
        }
    }
    #[test]
    fn claims_sections_are_grouped_fit_small_gui_and_custom_colours_apply() {
        let model = ViewModel {
            claim_rows: (0..20)
                .map(|i| ClaimRow {
                    dimension: "minecraft:overworld".into(),
                    x: i * 16,
                    z: 0,
                    label: format!("Чанк {i}"),
                })
                .collect(),
            ..ViewModel::default()
        };
        for section in [
            ClaimsSection::Map,
            ClaimsSection::Borders,
            ClaimsSection::Colours,
            ClaimsSection::Storage,
            ClaimsSection::List,
        ] {
            let menu = Menu {
                tab: Tab::Claims,
                claims_section: section,
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
            }
            let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
            for (i, a) in controls.iter().enumerate() {
                for b in controls.iter().skip(i + 1) {
                    assert!(
                        a.rect[0] + a.rect[2] <= b.rect[0]
                            || b.rect[0] + b.rect[2] <= a.rect[0]
                            || a.rect[1] + a.rect[3] <= b.rect[1]
                            || b.rect[1] + b.rect[3] <= a.rect[1]
                    );
                }
            }
        }
        let mut menu = Menu {
            tab: Tab::Claims,
            claims_section: ClaimsSection::Colours,
            focus: Some(Field::OwnClaimColor),
            own_claim_color: "#123ABC".into(),
            friend_claim_color: "#FEDCBA".into(),
            ..Menu::default()
        };
        let Some(Action::SetClaims(p)) = menu.activate(&model, 224.0, Control::ApplyClaimColours)
        else {
            panic!()
        };
        assert_eq!(p.own_color, 0x123abc);
        assert_eq!(p.friend_color, 0xfedcba);
        menu.key("key.keyboard.home", 0);
        menu.key("key.keyboard.delete", 0);
        menu.character('#');
        assert_eq!(menu.own_claim_color, "#123ABC");
    }
}
