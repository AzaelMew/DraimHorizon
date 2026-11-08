use super::claim_borders_chat_preferences::Preferences;
use super::claim_borders_chat_priority as priority;
pub use claim_borders_relay_client::translation::Language;
use claim_borders_relay_client::translation::{DEEPL_KEY_LIMIT, valid_deepl_key};

macro_rules! localized {
    ($language:expr, $russian:literal, $english:literal $(, $arg:expr)* $(,)?) => {
        match $language {
            Language::Russian => format!($russian $(, $arg)*),
            Language::English => format!($english $(, $arg)*),
        }
    };
}
use super::claim_borders_experimental as experimental;
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
    pub server_online: bool,
    pub blocked: bool,
    pub local_only: bool,
    pub outgoing: bool,
    pub skin: String,
    pub code: String,
    pub name: String,
    pub detail: String,
    pub pending: bool,
}
/// Include manual chat priorities even when the player does not use the relay.
pub fn append_important_rows(
    rows: &mut Vec<PartyRow>,
    prefs: &priority::Preferences,
    online: &[(u128, String)],
    language: Language,
) {
    for name in prefs.rules.keys() {
        if rows.iter().any(|row| row.name.eq_ignore_ascii_case(name)) {
            continue;
        }
        let live = online.iter().find(|(_, n)| n.eq_ignore_ascii_case(name));
        rows.push(PartyRow {
            uuid: live.map_or(0, |(id, _)| *id),
            name: live.map_or_else(|| name.clone(), |(_, n)| n.clone()),
            detail: if live.is_some() {
                language.text("Отметка чата · онлайн", "Chat priority · online")
            } else {
                language.text("Отметка чата · не в сети", "Chat priority · offline")
            }
            .into(),
            online: live.is_some(),
            server_online: live.is_some(),
            blocked: true,
            local_only: true,
            ..PartyRow::default()
        });
    }
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
    pub owner: String,
    pub foreign: bool,
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
    pub navigation_players: Vec<(u128, String)>,
    pub navigation_status: String,
    pub schematic_files: Vec<String>,
    /// Display names of the Baritone action keys, in `Hotkey::ALL` order; empty = unassigned.
    pub hotkeys: [String; 4],
    pub navigation_waypoints: Vec<WaypointRow>,
    pub auto_reconnect: bool,
    pub experiments_accepted: bool,
    pub language: Language,
    pub waypoints: Vec<WaypointRow>,
    pub party: Vec<PartyRow>,
    pub relay: String,
    pub relay_url: String,
    pub relay_token_present: bool,
    pub relay_connected: bool,
    pub relay_auto: bool,
    pub auto_trust: bool,
    pub claims: ClaimsSettings,
    pub status: String,
    pub chat_modern: bool,
    pub chat_translate: bool,
    pub chat_outgoing: bool,
    pub deepl_configured: bool,
    pub priority: priority::Preferences,
    pub experimental: experimental::Preferences,
    pub experimental_status: String,
    pub chat_preferences: Preferences,
    pub map_preferences: map::Preferences,
    pub claim_rows: Vec<ClaimRow>,
    pub economy_preferences: super::claim_borders_economy::Preferences,
}
/// Requests are plain server commands; neither action accepts a request or a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerRequest {
    Teleport,
    Trade,
}
impl PlayerRequest {
    pub fn command(self, name: &str, language: Language) -> Result<String, String> {
        if name.is_empty()
            || name.len() > 16
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(language
                .text("Недопустимый ник игрока", "Invalid player name")
                .into());
        }
        match self {
            Self::Teleport => Ok(format!("/tpa {name}")),
            Self::Trade
                if ["accept", "deny", "cancel", "money"]
                    .iter()
                    .any(|verb| name.eq_ignore_ascii_case(verb)) =>
            {
                Err(language
                    .text(
                        "Ник совпадает с подкомандой /trade; быстрый запрос недоступен",
                        "The name matches a /trade subcommand; quick request is unavailable",
                    )
                    .into())
            }
            Self::Trade => Ok(format!("/trade {name}")),
        }
    }
    /// Resolve the same player again immediately before sending, using the current server roster.
    pub fn resolve(
        self,
        uuid: u128,
        name: &str,
        own: u128,
        online: &[(u128, String)],
        language: Language,
    ) -> Result<String, String> {
        if uuid == own {
            return Err(language
                .text(
                    "Нельзя отправить запрос самому себе",
                    "You cannot send a request to yourself",
                )
                .into());
        }
        let (_, current) = online
            .iter()
            .find(|(id, current)| *id == uuid && current.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                language
                    .text(
                        "Игрок уже вышел с сервера или сменил ник",
                        "The player left the server or changed their name",
                    )
                    .to_owned()
            })?;
        self.command(current, language)
    }
}
#[derive(Clone, PartialEq)]
pub enum Action {
    RefreshSchematics,
    SchematicInfo(bool),
    LoadSchematic {
        filename: String,
        origin: [i32; 3],
        rotation: u8,
        running: bool,
    },
    Navigate {
        goal: super::claim_borders_navigation::Goal,
        running: bool,
    },
    /// An action key pressed while the menu is open: the game itself does not pass
    /// keys to the mod while a screen is shown.
    BaritoneKey(super::claim_borders_navigation::Hotkey),
    /// Create and open the schematic folder in the system file manager, then list it.
    OpenSchematicFolder,
    ToggleReconnect,
    AcceptExperiments,
    SetDeepLKey(Option<String>),
    SetLanguage(Language),
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
    PlayerRequest {
        kind: PlayerRequest,
        uuid: u128,
        name: String,
    },
    RefreshClaims,
    OpenEconomy,
    EconomySetting(&'static str, i64),
    Appearance,
    ToggleAutoTrust,
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
    SetPriority(priority::Preferences),
    SetExperimental(experimental::Preferences),
    PasteFromClipboard,
    SetClaims(ClaimsSettings),
    Close,
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Navigate { goal, running } => f
                .debug_struct("Navigate")
                .field("goal", goal)
                .field("running", running)
                .finish(),
            Self::BaritoneKey(key) => f.debug_tuple("BaritoneKey").field(key).finish(),
            Self::OpenSchematicFolder => f.write_str("OpenSchematicFolder"),
            Self::RefreshSchematics => f.write_str("RefreshSchematics"),
            Self::SchematicInfo(folder) => f.debug_tuple("SchematicInfo").field(folder).finish(),
            Self::LoadSchematic {
                filename,
                origin,
                rotation,
                running,
            } => f
                .debug_tuple("LoadSchematic")
                .field(filename)
                .field(origin)
                .field(rotation)
                .field(running)
                .finish(),
            Self::SetDeepLKey(_) => f.write_str("SetDeepLKey([redacted])"),
            Self::SetLanguage(language) => f.debug_tuple("SetLanguage").field(language).finish(),
            Self::PlayerRequest { kind, uuid, name } => f
                .debug_struct("PlayerRequest")
                .field("kind", kind)
                .field("uuid", uuid)
                .field("name", name)
                .finish(),
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
            Self::ToggleReconnect => f.write_str("ToggleReconnect"),
            Self::AcceptExperiments => f.write_str("AcceptExperiments"),
            Self::SetExperimental(p) => f.debug_tuple("SetExperimental").field(p).finish(),
            Self::SetPriority(p) => f.debug_tuple("SetPriority").field(p).finish(),
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
            Self::EconomySetting(key, dir) => f
                .debug_tuple("EconomySetting")
                .field(key)
                .field(dir)
                .finish(),
            Self::Appearance => f.write_str("Appearance"),
            Self::OpenEconomy => f.write_str("OpenEconomy"),
            Self::ToggleAutoTrust => f.write_str("ToggleAutoTrust"),
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
    Experimental,
    Settings,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    SchematicOrigin,
    NavigationX,
    NavigationY,
    NavigationZ,
    Name,
    Color,
    PeerCode,
    RelayUrl,
    RelayToken,
    DeepLKey,
    OwnClaimColor,
    FriendClaimColor,
    PriorityName,
    PriorityColor,
    ClaimOwner,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PartyFilter {
    Friends,
    #[default]
    Users,
    Invitations,
    Important,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ChatSection {
    #[default]
    Appearance,
    Notifications,
    Spam,
    DeepL,
    Priority,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ExperimentalSection {
    #[default]
    Ores,
    Baritone,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BaritoneMode {
    #[default]
    Navigation,
    Mining,
    Tunnel,
    Bridge,
    Farm,
    Drain,
    Schematic,
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
    FarmCrop,
    FarmRadius,
    FarmLimit,
    FarmReplant,
    DrainRadius,
    SchematicFile,
    SchematicRefresh,
    SchematicRotation,
    SchematicMaterials,
    SchematicFolder,
    ConstructDirection,
    ConstructLength,
    ConstructWidth,
    ConstructHeight,
    ConstructFloor,
    BaritoneMode(BaritoneMode),
    MiningTarget(u8),
    MiningPage,
    MiningLimit,
    NavigationPlayer,
    NavigationWaypoint,
    NavigationPlayerChoice(usize),
    NavigationWaypointChoice(usize),
    NavigationChoicesPage(i8),
    SchematicChoice(usize),
    SchematicChoicesPage(i8),
    SchematicOpenFolder,
    ExperimentalTab(ExperimentalSection),
    ToggleReconnect,
    SettingsTab(u8),
    ConfirmExperiments,
    DeclineExperiments,
    SaveDeepLKey,
    RemoveDeepLKey,
    Language,
    SettingsSection(i8),
    SettingsPage(i8),
    EconomySetting(&'static str, i64),
    Appearance,
    ClaimFilter,
    PartyAccept(usize),
    PartyDecline(usize),
    PlayerRequest(PlayerRequest),
    PriorityNext,
    PriorityRow(usize),
    PrioritySave,
    PriorityRemove,
    PriorityPin,
    PriorityFriends,
    PriorityTray,
    PriorityTime(i8),
    PriorityRows(i8),
    Economy,
    ToggleAutoTrust,
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
    ExperimentalAllowed,
    ExperimentalSource,
    ExperimentalNext,
    ExperimentalOre,
    ChatAdjust(ChatOption, i8),
    ChatPlacement,
    ChatCompactMessages,
    ChatStyle,
    ChatPresence,
    ChatPresenceStyle,
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
    pub farm_crop: super::claim_borders_farming::Crop,
    pub farm_radius: u8,
    pub farm_limit: u16,
    pub farm_replant: bool,
    pub drain_radius: u8,
    pub schematic_file: Option<String>,
    pub schematic_origin: String,
    pub schematic_rotation: u8,
    pub construct_direction: super::claim_borders_construction::Direction,
    pub construct_length: u16,
    pub construct_width: u8,
    pub construct_height: u8,
    pub construct_floor: bool,
    pub baritone_mode: BaritoneMode,
    /// Selected mining targets, one bit per `mining::target` index.
    pub mining_targets: u32,
    pub mining_page: usize,
    pub mining_limit: u16,
    pub navigation_player: Option<u128>,
    pub navigation_x: String,
    pub navigation_y: String,
    pub navigation_z: String,
    pub navigation_waypoint: Option<u64>,
    pub navigation_choices: Option<bool>, // false: waypoints, true: players
    pub navigation_choices_page: usize,
    /// The schematic file list is open under the file button.
    pub schematic_choices: bool,
    pub schematic_choices_page: usize,
    pub experiments_prompt: bool,
    pub experimental_section: ExperimentalSection,
    deepl_key: String,
    pub language: Language,
    pub tab: Tab,
    pub settings_section: u8,
    pub settings_page: usize,
    pub claim_owner: String,
    pub claim_filter: u8,
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
    pub priority_name: String,
    pub priority_color: String,
    pub priority_pinned: bool,
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
            farm_crop: super::claim_borders_farming::Crop::Wheat,
            farm_radius: 6,
            farm_limit: 16,
            farm_replant: true,
            drain_radius: 6,
            schematic_file: None,
            schematic_origin: String::new(),
            schematic_rotation: 0,
            experiments_prompt: false,
            navigation_x: String::new(),
            navigation_y: String::new(),
            navigation_z: String::new(),
            navigation_waypoint: None,
            navigation_choices: None,
            navigation_choices_page: 0,
            schematic_choices: false,
            schematic_choices_page: 0,
            navigation_player: None,
            baritone_mode: BaritoneMode::Navigation,
            mining_targets: 1,
            mining_page: 0,
            mining_limit: 16,
            construct_direction: super::claim_borders_construction::Direction::North,
            construct_length: 16,
            construct_width: 1,
            construct_height: 2,
            construct_floor: true,
            experimental_section: ExperimentalSection::default(),
            deepl_key: String::new(),
            language: Language::default(),
            tab: Tab::Waypoints,
            settings_section: 0,
            settings_page: 0,
            claim_owner: String::new(),
            claim_filter: 0,
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
            priority_name: String::new(),
            priority_color: "#FFCC66".into(),
            priority_pinned: true,
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
    pub fn clear_navigation_coordinates(&mut self) {
        self.navigation_x.clear();
        self.navigation_y.clear();
        self.navigation_z.clear();
    }
    /// Typed X/Y/Z of the navigation tab, parsed as one position. Any empty or
    /// malformed field makes the whole destination invalid.
    pub fn navigation_position(&self) -> Option<[f64; 3]> {
        super::claim_borders_waypoints::parse_coordinates([
            self.navigation_x.trim(),
            self.navigation_y.trim(),
            self.navigation_z.trim(),
        ])
        .ok()
    }
    /// Carry the Baritone settings over to a fresh menu: the action keys run the
    /// last settings shown here, so reopening or resetting the menu must not lose them.
    pub fn keep_baritone(&mut self, from: &Menu) {
        self.farm_crop = from.farm_crop;
        self.farm_radius = from.farm_radius;
        self.farm_limit = from.farm_limit;
        self.farm_replant = from.farm_replant;
        self.drain_radius = from.drain_radius;
        self.schematic_file = from.schematic_file.clone();
        self.schematic_origin = from.schematic_origin.clone();
        self.schematic_rotation = from.schematic_rotation;
        self.construct_direction = from.construct_direction;
        self.construct_length = from.construct_length;
        self.construct_width = from.construct_width;
        self.construct_height = from.construct_height;
        self.construct_floor = from.construct_floor;
        self.baritone_mode = from.baritone_mode;
        self.mining_targets = from.mining_targets;
        self.mining_page = from.mining_page;
        self.mining_limit = from.mining_limit;
        self.navigation_player = from.navigation_player;
        self.navigation_waypoint = from.navigation_waypoint;
        self.navigation_x = from.navigation_x.clone();
        self.navigation_y = from.navigation_y.clone();
        self.navigation_z = from.navigation_z.clone();
    }
    /// Footer line of the Baritone tabs: what each action key is bound to.
    /// Whether the Baritone tab is on screen with no text field taking the keys.
    pub fn baritone_keys_active(&self) -> bool {
        self.tab == Tab::Experimental
            && self.experimental_section == ExperimentalSection::Baritone
            && self.focus.is_none()
            && self.navigation_choices.is_none()
            && !self.schematic_choices
            && !self.experiments_prompt
    }
    fn hotkey_hint(&self, model: &ViewModel) -> String {
        if model.hotkeys.iter().all(String::is_empty) {
            return self
                .language
                .text(
                    "Клавиши не назначены: Параметры → Моды → DraimHorizon",
                    "No keys assigned: Options → Mods → DraimHorizon",
                )
                .into();
        }
        [
            self.language.text("Просмотр", "Preview"),
            self.language.text("Старт", "Start"),
            self.language.text("Пауза", "Pause"),
            self.language.text("Стоп", "Stop"),
        ]
        .into_iter()
        .zip(&model.hotkeys)
        .map(|(name, key)| format!("{name} {}", if key.is_empty() { "—" } else { key }))
        .collect::<Vec<_>>()
        .join(" · ")
    }
    /// The task the Baritone tab on screen describes, built from its current
    /// settings. The action keys call this, so they work with the menu closed;
    /// the consent, world and tool checks stay in the drivers that run it.
    pub fn baritone_request(&self, running: bool) -> Result<Action, String> {
        use super::claim_borders_construction as construction;
        use super::claim_borders_navigation::Goal;
        let language = self.language;
        match self.baritone_mode {
            BaritoneMode::Navigation => {
                let goal = if let Some(id) = self.navigation_player {
                    Goal::Follow(id)
                } else if let Some(id) = self.navigation_waypoint {
                    Goal::Waypoint(id)
                } else {
                    let position = self.navigation_position().ok_or_else(|| {
                        language
                            .text(
                                "Введите X, Y, Z, например: 120 64 -350",
                                "Enter X, Y, Z, for example: 120 64 -350",
                            )
                            .to_owned()
                    })?;
                    Goal::Coordinates(super::claim_borders_navigation::block(position))
                };
                Ok(Action::Navigate { goal, running })
            }
            BaritoneMode::Mining => {
                use super::claim_borders_mining as mining;
                let targets = self.mining_targets & mining::ALL;
                if targets == 0 {
                    return Err(language
                        .text("Выберите, что добывать", "Choose what to mine")
                        .into());
                }
                let ore = if targets.count_ones() == 1 {
                    targets.trailing_zeros() as u8
                } else {
                    mining::ANY
                };
                Ok(Action::Navigate {
                    goal: Goal::Mine {
                        ore,
                        targets,
                        limit: self.mining_limit,
                    },
                    running,
                })
            }
            BaritoneMode::Tunnel | BaritoneMode::Bridge => {
                let bridge = self.baritone_mode == BaritoneMode::Bridge;
                Ok(Action::Navigate {
                    goal: Goal::Construct(construction::Config {
                        kind: if bridge {
                            construction::Kind::Bridge
                        } else {
                            construction::Kind::Tunnel
                        },
                        direction: self.construct_direction,
                        width: if bridge { 1 } else { self.construct_width },
                        height: if bridge { 2 } else { self.construct_height },
                        length: self.construct_length,
                        fill_floor: bridge || self.construct_floor,
                    }),
                    running,
                })
            }
            BaritoneMode::Farm => Ok(Action::Navigate {
                goal: Goal::Farm(super::claim_borders_farming::Config {
                    crop: self.farm_crop,
                    radius: self.farm_radius,
                    limit: self.farm_limit,
                    replant: self.farm_replant,
                }),
                running,
            }),
            BaritoneMode::Drain => Ok(Action::Navigate {
                goal: Goal::Drain(super::claim_borders_draining::Config {
                    radius: self.drain_radius,
                }),
                running,
            }),
            BaritoneMode::Schematic => {
                let filename = self.schematic_file.clone().ok_or_else(|| {
                    language
                        .text("Сначала выберите файл", "Select a file first")
                        .to_owned()
                })?;
                let values = self.schematic_origin.split_whitespace().collect::<Vec<_>>();
                let origin = <[&str; 3]>::try_from(values.as_slice())
                    .ok()
                    .and_then(|v| super::claim_borders_waypoints::parse_coordinates(v).ok())
                    .filter(|p| p.iter().all(|v| v.fract() == 0.0))
                    .map(|p| p.map(|v| v as i32))
                    .ok_or_else(|| language.text("Введите X Y Z", "Enter X Y Z").to_owned())?;
                Ok(Action::LoadSchematic {
                    filename,
                    origin,
                    rotation: self.schematic_rotation,
                    running,
                })
            }
        }
    }
    /// Keep a selection attached to its identity while snapshots reorder or remove rows.
    pub fn reconcile(&mut self, previous: &ViewModel, next: &ViewModel, height: f32) {
        self.language = next.language;
        self.selected = self.selected.and_then(|index| match self.tab {
            Tab::Waypoints => previous
                .waypoints
                .get(index)
                .and_then(|old| next.waypoints.iter().position(|row| row.id == old.id)),
            Tab::Party => previous.party.get(index).and_then(|old| {
                next.party.iter().position(|row| {
                    if old.local_only || row.local_only {
                        old.local_only == row.local_only && row.name.eq_ignore_ascii_case(&old.name)
                    } else {
                        row.code == old.code && row.pending == old.pending
                    }
                })
            }),
            Tab::Claims | Tab::Chat | Tab::Experimental | Tab::Settings => None,
        });
        if self.tab == Tab::Party
            && self
                .selected
                .is_some_and(|i| !self.party_indices(next).contains(&i))
        {
            self.selected = None;
        }
        // Navigation targets are identified by stable IDs, not snapshot indices.
        // A vanished player/waypoint must not leave an invisible stale destination.
        let mut target_missing = false;
        if self
            .navigation_player
            .is_some_and(|id| !next.navigation_players.iter().any(|(uuid, _)| *uuid == id))
        {
            self.navigation_player = None;
            target_missing = true;
        }
        if self
            .navigation_waypoint
            .is_some_and(|id| !next.navigation_waypoints.iter().any(|row| row.id == id))
        {
            self.navigation_waypoint = None;
            target_missing = true;
        }
        if target_missing {
            // Coordinates typed before selecting a target are not an authorized fallback.
            self.clear_navigation_coordinates();
            self.notice = self
                .language
                .text(
                    "Цель недоступна; выберите новую цель",
                    "Target unavailable; select a new destination",
                )
                .into();
        }
        if let Some(players) = self.navigation_choices {
            let count = if players {
                next.navigation_players.len()
            } else {
                next.navigation_waypoints.len()
            };
            if count == 0 {
                self.navigation_choices_page = 0;
            } else {
                self.navigation_choices_page =
                    self.navigation_choices_page.min(count.div_ceil(8) - 1);
            }
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
        if self.tab == Tab::Party && self.party_filter == PartyFilter::Important {
            (((height - 176.0) / 24.0).floor() as usize).max(1)
        } else if self.tab == Tab::Party {
            (((height - 164.0) / 24.0).floor() as usize).clamp(1, 12)
        } else {
            Self::rows_per_page(height)
        }
    }
    fn count(&self, model: &ViewModel) -> usize {
        match self.tab {
            Tab::Waypoints => model.waypoints.len(),
            Tab::Party if self.party_filter == PartyFilter::Important => {
                self.important_names(model).len()
            }
            Tab::Party => self.party_indices(model).len(),
            Tab::Claims | Tab::Chat | Tab::Experimental | Tab::Settings => 0,
        }
    }
    pub fn layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        if self.experiments_prompt {
            return self.experiments_dialog(width, height);
        }
        let panel_w = (width - 16.0).clamp(220.0, 720.0);
        let panel_h = (height - 16.0).clamp(200.0, 400.0);
        let sidebar = if panel_w >= 380.0 { 96.0 } else { 0.0 };
        let nav_step = ((panel_h - 80.0) / 6.0).min(26.0);
        let x = (width - panel_w) * 0.5;
        let y = (height - panel_h) * 0.5;
        let mut widgets = self.legacy_layout(model, panel_w - sidebar, panel_h);
        for widget in &mut widgets {
            widget.rect[0] += x + sidebar;
            widget.rect[1] += y;
            if sidebar > 0.0 && widget.control == Some(Control::Economy) {
                widget.rect = [
                    x + 6.0,
                    y + 44.0 + 5.0 * nav_step,
                    sidebar - 10.0,
                    nav_step - 4.0,
                ];
            }
            if sidebar > 0.0 && widget.control == Some(Control::Language) {
                widget.rect = [x + 63.0, y + panel_h - 28.0, 27.0, 22.0];
            }
            if sidebar > 0.0
                && let Some(Control::Tab(tab)) = widget.control
            {
                let i = match tab {
                    Tab::Waypoints => 0,
                    Tab::Party => 1,
                    Tab::Claims => 2,
                    Tab::Chat => 3,
                    Tab::Experimental => 4,
                    Tab::Settings => 6,
                };
                widget.rect = [
                    x + 6.0,
                    if tab == Tab::Settings {
                        y + panel_h - 28.0
                    } else {
                        y + 44.0 + i as f32 * nav_step
                    },
                    if tab == Tab::Settings {
                        54.0
                    } else {
                        sidebar - 10.0
                    },
                    if tab == Tab::Settings {
                        22.0
                    } else {
                        nav_step - 4.0
                    },
                ];
            }
        }
        if self.tab == Tab::Experimental
            && self.baritone_mode == BaritoneMode::Navigation
            && let Some(players) = self.navigation_choices
        {
            let anchor = widgets.iter().find(|w| {
                w.control
                    == Some(if players {
                        Control::NavigationPlayer
                    } else {
                        Control::NavigationWaypoint
                    })
            });
            if let Some(anchor) = anchor {
                let rect = anchor.rect;
                let entries = if players {
                    model
                        .navigation_players
                        .iter()
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<_>>()
                } else {
                    model
                        .navigation_waypoints
                        .iter()
                        .map(|w| w.name.as_str())
                        .collect::<Vec<_>>()
                };
                let page_count = entries.len().div_ceil(8);
                let page = self
                    .navigation_choices_page
                    .min(page_count.saturating_sub(1));
                let start = page * 8;
                let count = entries.len().saturating_sub(start).min(8);
                let item_h = 20.0;
                let menu_h = (count + usize::from(page_count > 1)).max(1) as f32 * item_h;
                let top = if rect[1] + rect[3] + menu_h <= height - 12.0 {
                    rect[1] + rect[3]
                } else {
                    (rect[1] - menu_h).max(8.0)
                };
                if count == 0 {
                    widgets.push(widget(
                        [rect[0], top, rect[2], item_h],
                        self.language
                            .text("Нет доступных целей", "No available targets"),
                        Some(Control::NavigationChoicesPage(0)),
                        false,
                    ));
                }
                for i in 0..count {
                    widgets.push(widget(
                        [rect[0], top + i as f32 * item_h, rect[2], item_h],
                        entries[start + i],
                        Some(if players {
                            Control::NavigationPlayerChoice(start + i)
                        } else {
                            Control::NavigationWaypointChoice(start + i)
                        }),
                        false,
                    ));
                }
                if page_count > 1 {
                    widgets.push(widget(
                        [rect[0], top + count as f32 * item_h, rect[2] / 2.0, item_h],
                        format!("← {}/{}", page + 1, page_count),
                        Some(Control::NavigationChoicesPage(-1)),
                        false,
                    ));
                    widgets.push(widget(
                        [
                            rect[0] + rect[2] / 2.0,
                            top + count as f32 * item_h,
                            rect[2] / 2.0,
                            item_h,
                        ],
                        format!("{}/{} →", page + 1, page_count),
                        Some(Control::NavigationChoicesPage(1)),
                        false,
                    ));
                }
            }
        }
        if self.tab == Tab::Experimental
            && self.baritone_mode == BaritoneMode::Schematic
            && self.schematic_choices
            && let Some(anchor) = widgets
                .iter()
                .find(|w| w.control == Some(Control::SchematicFile))
        {
            let rect = anchor.rect;
            // The files, then one entry that opens the folder to add more.
            let total = model.schematic_files.len() + 1;
            let page_count = total.div_ceil(8);
            let page = self.schematic_choices_page.min(page_count - 1);
            let start = page * 8;
            let count = (total - start).min(8);
            let item_h = 20.0;
            let menu_h = (count + usize::from(page_count > 1)) as f32 * item_h;
            let top = if rect[1] + rect[3] + menu_h <= height - 12.0 {
                rect[1] + rect[3]
            } else {
                (rect[1] - menu_h).max(8.0)
            };
            for i in 0..count {
                let index = start + i;
                let (label, control) = match model.schematic_files.get(index) {
                    Some(name) => (name.clone(), Control::SchematicChoice(index)),
                    None => (
                        self.language
                            .text("Открыть папку…", "Open folder…")
                            .to_owned(),
                        Control::SchematicOpenFolder,
                    ),
                };
                widgets.push(widget(
                    [rect[0], top + i as f32 * item_h, rect[2], item_h],
                    label,
                    Some(control),
                    self.schematic_file.as_deref()
                        == model.schematic_files.get(index).map(String::as_str),
                ));
            }
            if page_count > 1 {
                let y = top + count as f32 * item_h;
                widgets.push(widget(
                    [rect[0], y, rect[2] / 2.0, item_h],
                    format!("← {}/{}", page + 1, page_count),
                    Some(Control::SchematicChoicesPage(-1)),
                    false,
                ));
                widgets.push(widget(
                    [rect[0] + rect[2] / 2.0, y, rect[2] / 2.0, item_h],
                    format!("{}/{} →", page + 1, page_count),
                    Some(Control::SchematicChoicesPage(1)),
                    false,
                ));
            }
        }
        widgets
    }
    fn experiments_dialog(&self, width: f32, height: f32) -> Vec<Widget> {
        let w = (width - 32.0).clamp(200.0, 460.0);
        let x = (width - w) * 0.5;
        let y = (height - 184.0) * 0.5;
        let lines = match self.language {
            Language::Russian => [
                "Экспериментальные функции",
                "Автор мода не рекомендует",
                "использовать эти функции.",
                "Используйте их только",
                "на свой страх и риск.",
            ],
            Language::English => [
                "Experimental features",
                "The mod author does not",
                "recommend these features.",
                "Use them entirely",
                "at your own risk.",
            ],
        };
        let mut out: Vec<_> = lines
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                widget(
                    [x + 10.0, y + 44.0 + i as f32 * 16.0, w - 20.0, 18.0],
                    text,
                    None,
                    false,
                )
            })
            .collect();
        out.push(widget(
            [x + 10.0, y + 150.0, (w - 30.0) * 0.5, 24.0],
            self.language.text("Подтвердить", "Confirm"),
            Some(Control::ConfirmExperiments),
            false,
        ));
        out.push(widget(
            [
                x + 20.0 + (w - 30.0) * 0.5,
                y + 150.0,
                (w - 30.0) * 0.5,
                24.0,
            ],
            self.language.text("Отклонить", "Decline"),
            Some(Control::DeclineExperiments),
            false,
        ));
        out
    }
    fn legacy_layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        let w = (width - 16.0).clamp(220.0, 590.0);
        let x = (width - w) * 0.5;
        let y = 8.0;
        let mut out = vec![
            widget([x, y, w - 30.0, 18.0], "DraimHorizon", None, false),
            widget(
                [x + w - 26.0, y, 26.0, 18.0],
                "X",
                Some(Control::Close),
                false,
            ),
        ];
        for (i, (control, label)) in [
            (
                Control::Tab(Tab::Waypoints),
                self.language.text("Метки", "Waypoints"),
            ),
            (
                Control::Tab(Tab::Party),
                self.language.text("Пати", "Friends"),
            ),
            (
                Control::Tab(Tab::Claims),
                self.language.text("Клеймы", "Claims"),
            ),
            (Control::Tab(Tab::Chat), self.language.text("Чат", "Chat")),
            (
                Control::Tab(Tab::Experimental),
                self.language.text("Эксперименты", "Experiments"),
            ),
            (Control::Economy, self.language.text("Торги", "Trading")),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 6.0, y + 22.0, w / 6.0 - 3.0, 20.0],
                label,
                Some(control),
                matches!(control,Control::Tab(tab)if self.tab==tab),
            ));
        }
        out.push(widget(
            [x + w - 96.0, height - 24.0, 63.0, 20.0],
            self.language.text("Настройки", "Settings"),
            Some(Control::Tab(Tab::Settings)),
            self.tab == Tab::Settings,
        ));
        out.push(widget(
            [x + w - 30.0, height - 24.0, 30.0, 20.0],
            self.language.text("RU", "EN"),
            Some(Control::Language),
            false,
        ));
        if self.tab == Tab::Settings {
            return self.settings_layout(model, x, w, height, out);
        }
        if self.tab == Tab::Experimental {
            return self.experimental_layout(model, x, w, height, out);
        }
        if self.tab == Tab::Chat {
            for (i, (label, control, selected)) in [
                (
                    localized!(
                        self.language,
                        "Перевод входящих: {}",
                        "Incoming translation: {}",
                        self.on(model.chat_translate)
                    ),
                    Control::ChatTranslate,
                    model.chat_translate,
                ),
                (
                    self.outgoing_label(model.chat_outgoing),
                    Control::ChatOutgoing,
                    model.chat_outgoing,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x, 64.0 + i as f32 * 28.0, w, 24.0],
                    label,
                    Some(control),
                    selected,
                ));
            }
            return out;
        }
        if self.tab == Tab::Party {
            return self.party_layout(model, width, height, out);
        }
        if self.tab == Tab::Claims {
            return self.claim_list_layout(model, x, w, height, out);
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
                                self.language.text("[Приглашение] ", "[Invitation] ")
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
                    self.language.text(
                        "Нет меток. Введите имя и добавьте текущую позицию.",
                        "No waypoints. Enter a name and add your current position.",
                    ),
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
                localized!(
                    self.language,
                    "Страница {} / {}",
                    "Page {} / {}",
                    self.page + 1,
                    pages
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
            if self.tab == Tab::Waypoints {
                out.push(widget(
                    [x, b + 22.0, w * 0.65 - 3.0, 20.0],
                    localized!(self.language, "Имя: {}", "Name: {}", self.name),
                    Some(Control::Field(Field::Name)),
                    self.focus == Some(Field::Name),
                ));
                out.push(widget(
                    [x + w * 0.65, b + 22.0, w * 0.35, 20.0],
                    localized!(self.language, "Цвет: {}", "Color: {}", self.color),
                    Some(Control::Field(Field::Color)),
                    self.focus == Some(Field::Color),
                ));
                for (i, (c, t)) in [
                    (Control::Add, self.language.text("+ Здесь", "+ Here")),
                    (Control::Rename, self.language.text("Имя", "Name")),
                    (Control::Color, self.language.text("Цвет", "Color")),
                    (Control::Delete, self.language.text("Удалить", "Delete")),
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
            localized!(
                self.language,
                "Обмен: {} · {}",
                "Relay: {} · {}",
                model.relay,
                model.status
            )
        } else {
            model.status.clone()
        };
        out.push(widget([x, height - 19.0, w, 14.0], status, None, false));
        out
    }
    fn claim_indices(&self, model: &ViewModel) -> Vec<usize> {
        let query = self.claim_owner.to_lowercase();
        model
            .claim_rows
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.owner.to_lowercase().contains(&query)
                    && match self.claim_filter {
                        1 => !c.foreign,
                        2 => c.foreign,
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn claim_list_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        out.push(widget(
            [x, 56.0, w * 0.55 - 3.0, 20.0],
            localized!(self.language, "Игрок: {}", "Player: {}", self.claim_owner),
            Some(Control::Field(Field::ClaimOwner)),
            self.focus == Some(Field::ClaimOwner),
        ));
        out.push(widget(
            [x + w * 0.55, 56.0, w * 0.45, 20.0],
            [
                self.language.text("Все приваты", "All claims"),
                self.language.text("Свои / дружественные", "Own / friends"),
                self.language.text("Чужие", "Other players"),
            ][self.claim_filter as usize],
            Some(Control::ClaimFilter),
            false,
        ));
        out.push(widget(
            [x, 80.0, w * 0.5 - 3.0, 18.0],
            localized!(
                self.language,
                "Порядок: {}",
                "Order: {}",
                model.map_preferences.sort.label_in(self.language)
            ),
            Some(Control::MapSort),
            false,
        ));
        out.push(widget(
            [x + w * 0.5, 80.0, w * 0.5, 18.0],
            self.language.text("Обновить /claims", "Refresh /claims"),
            Some(Control::Refresh),
            false,
        ));
        let indices = self.claim_indices(model);
        let per = (((height - 150.0) / 20.0).floor() as usize).clamp(1, 12);
        let pages = indices.len().div_ceil(per).max(1);
        let page = self.claims_page.min(pages - 1);
        for (slot, index) in indices
            .iter()
            .copied()
            .skip(page * per)
            .take(per)
            .enumerate()
        {
            out.push(widget(
                [x, 104.0 + slot as f32 * 20.0, w, 18.0],
                &model.claim_rows[index].label,
                Some(Control::ClaimRow(index)),
                false,
            ));
        }
        out.push(widget(
            [x, height - 46.0, 24.0, 18.0],
            "←",
            Some(Control::ClaimsPrevious),
            false,
        ));
        out.push(widget(
            [x + 28.0, height - 46.0, w - 56.0, 18.0],
            localized!(
                self.language,
                "{} / {} · {} чанков",
                "{} / {} · {} chunks",
                page + 1,
                pages,
                indices.len()
            ),
            None,
            false,
        ));
        out.push(widget(
            [x + w - 24.0, height - 46.0, 24.0, 18.0],
            "→",
            Some(Control::ClaimsNext),
            false,
        ));
        out
    }
    fn settings_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        let titles = [
            self.language.text("Основное", "General"),
            self.language.text("Связь", "Network"),
            self.language.text("Карта", "Map"),
            self.language.text("Чат", "Chat"),
            self.language.text("Торги", "Trading"),
            "TAB",
            "HUD",
        ];
        let columns = 7_usize;
        let rows = titles.len().div_ceil(columns);
        let body_top = 56.0 + rows as f32 * 22.0 + 4.0;
        for (i, title) in titles.into_iter().enumerate() {
            out.push(widget(
                [
                    x + (i % columns) as f32 * w / columns as f32,
                    56.0 + (i / columns) as f32 * 22.0,
                    w / columns as f32 - 3.0,
                    20.0,
                ],
                title,
                Some(Control::SettingsTab(i as u8)),
                self.settings_section == i as u8,
            ));
        }
        let mut inner = match self.settings_section {
            0 => {
                out.push(widget(
                    [x, body_top, w, 22.0],
                    self.language
                        .text("Оформление интерфейса", "Interface appearance"),
                    Some(Control::Appearance),
                    false,
                ));
                out.push(widget(
                    [x, body_top + 28.0, w, 22.0],
                    localized!(
                        self.language,
                        "Авто-реконнект: {}",
                        "Auto reconnect: {}",
                        self.on(model.auto_reconnect)
                    ),
                    Some(Control::ToggleReconnect),
                    model.auto_reconnect,
                ));
                return out;
            }
            1 => {
                let mut copy = self.clone();
                copy.relay_setup = true;
                let mut v = copy.party_layout(model, w + 16.0, height - 30.0, Vec::new());
                for widget in &mut v {
                    widget.rect[0] += x - 8.0;
                }
                v.retain(|v| v.rect[1] < 212.0);
                v.push(widget(
                    [x, 212.0, w, 18.0],
                    format!("Auto Trust: {}", self.on(model.auto_trust)),
                    Some(Control::ToggleAutoTrust),
                    model.auto_trust,
                ));
                v
            }
            2 => self.claims_layout(model, x, w, height - 26.0, Vec::new()),
            3 => self.chat_layout(model, x, w, height - 26.0, Vec::new()),
            section => {
                let settings: Vec<_> = super::claim_borders_economy::SETTINGS
                    .iter()
                    .filter(|setting| match section {
                        4 => setting.group <= 4,
                        5 => setting.group == 6,
                        _ => setting.group == 5,
                    })
                    .collect();
                let per = (((height - body_top - 56.0) / 22.0).floor() as usize).max(1);
                let pages = settings.len().div_ceil(per).max(1);
                let page = self.settings_page.min(pages - 1);
                for (i, setting) in settings.into_iter().skip(page * per).take(per).enumerate() {
                    let y = body_top + i as f32 * 22.0;
                    out.push(widget(
                        [x, y, w - 48.0, 20.0],
                        format!(
                            "{}: {}",
                            self.language.text(setting.label, setting.label_en),
                            model.economy_preferences.get(setting.key)
                        ),
                        None,
                        false,
                    ));
                    out.push(widget(
                        [x + w - 46.0, y, 22.0, 20.0],
                        "−",
                        Some(Control::EconomySetting(setting.key, -1)),
                        false,
                    ));
                    out.push(widget(
                        [x + w - 22.0, y, 22.0, 20.0],
                        "+",
                        Some(Control::EconomySetting(setting.key, 1)),
                        false,
                    ));
                }
                out.push(widget(
                    [x, height - 48.0, w / 2.0 - 3.0, 18.0],
                    format!("← {} / {}", page + 1, pages),
                    Some(Control::SettingsPage(-1)),
                    false,
                ));
                out.push(widget(
                    [x + w / 2.0, height - 48.0, w / 2.0, 18.0],
                    "→",
                    Some(Control::SettingsPage(1)),
                    false,
                ));
                return out;
            }
        };
        // Preserve each editor's spacing while keeping its last row above the footer.
        let bottom = inner
            .iter()
            .map(|widget| widget.rect[1] + widget.rect[3])
            .fold(80.0_f32, f32::max);
        let scale = ((height - body_top - 28.0) / (bottom - 54.0).max(1.0)).min(1.0);
        for widget in &mut inner {
            widget.rect[1] = body_top + (widget.rect[1] - 54.0).max(0.0) * scale;
            widget.rect[3] *= scale;
        }
        out.extend(inner);
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
            (ClaimsSection::Map, self.language.text("Карта", "Map")),
            (
                ClaimsSection::Borders,
                self.language.text("Границы", "Borders"),
            ),
            (
                ClaimsSection::Colours,
                self.language.text("Цвета", "Colors"),
            ),
            (
                ClaimsSection::Storage,
                self.language.text("Память", "Storage"),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 4.0, 56.0, w / 4.0 - 3.0, 18.0],
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
                    self.language
                        .text("Открыть карту · J", "Open map · J")
                        .into(),
                    Control::OpenMap,
                    self.language
                        .text("Обновить /claims", "Refresh /claims")
                        .into(),
                    Control::Refresh,
                ),
                pair(
                    localized!(
                        self.language,
                        "Миникарта: {}",
                        "Minimap: {}",
                        self.on(p.minimap)
                    ),
                    Control::MapToggle,
                    localized!(
                        self.language,
                        "Угол: {}",
                        "Corner: {}",
                        [
                            self.language.text("Слева ↑", "Top left"),
                            self.language.text("Справа ↑", "Top right"),
                            self.language.text("Слева ↓", "Bottom left"),
                            self.language.text("Справа ↓", "Bottom right")
                        ][p.corner as usize]
                    ),
                    Control::MapCorner,
                ),
                adjust(
                    self.language.text("Размер", "Size"),
                    p.size.to_string(),
                    Control::MapAdjust(MapOption::Size, -1),
                    Control::MapAdjust(MapOption::Size, 1),
                ),
                adjust(
                    self.language.text("Блоков/пикс.", "Blocks/pixel"),
                    format!("{:.1}", p.zoom),
                    Control::MapAdjust(MapOption::Zoom, -1),
                    Control::MapAdjust(MapOption::Zoom, 1),
                ),
                pair(
                    localized!(
                        self.language,
                        "Рельеф: {}",
                        "Terrain: {}",
                        p.layer.label_in(self.language)
                    ),
                    Control::MapLayer,
                    localized!(
                        self.language,
                        "Метки: {}",
                        "Waypoints: {}",
                        self.on(p.marks)
                    ),
                    Control::MapMarks,
                ),
                pair(
                    localized!(
                        self.language,
                        "Сетка чанков: {}",
                        "Chunk grid: {}",
                        self.on(p.grid)
                    ),
                    Control::MapGrid,
                    localized!(
                        self.language,
                        "Координаты: {}",
                        "Coordinates: {}",
                        self.on(p.coordinates)
                    ),
                    Control::MapCoordinates,
                ),
                pair(
                    localized!(self.language, "Тени: {}", "Shading: {}", self.on(p.shading)),
                    Control::MapShading,
                    localized!(
                        self.language,
                        "Мобы: {}",
                        "Mobs: {}",
                        [
                            self.language.text("выкл.", "off"),
                            self.language.text("все", "all"),
                            self.language.text("опасные", "hostile")
                        ][p.mobs as usize]
                    ),
                    Control::MapMobs,
                ),
                vec![(
                    self.language
                        .text("Сбросить настройки карты", "Reset map settings")
                        .into(),
                    Control::MapReset,
                )],
            ],
            ClaimsSection::Storage => vec![
                vec![(
                    self.language
                        .text("Сбросить кеш приватов игроков", "Clear player claims cache")
                        .into(),
                    Control::ClearClaimCache,
                )],
                vec![(
                    localized!(
                        self.language,
                        "Запись карты: {}",
                        "Record map: {}",
                        self.on(p.remember)
                    ),
                    Control::MapRemember,
                )],
                vec![(
                    localized!(
                        self.language,
                        "Атлас мира: {} / 8",
                        "World atlas: {} / 8",
                        p.profile + 1
                    ),
                    Control::MapProfile,
                )],
            ],
            ClaimsSection::Borders => vec![
                pair(
                    localized!(
                        self.language,
                        "Границы 3D: {}",
                        "3D borders: {}",
                        self.on(v.enabled)
                    ),
                    Control::BordersEnabled,
                    localized!(
                        self.language,
                        "Сквозь блоки: {}",
                        "Through blocks: {}",
                        self.on(v.through_walls)
                    ),
                    Control::ThroughWalls,
                ),
                pair(
                    localized!(
                        self.language,
                        "Свои 3D: {}",
                        "Own 3D: {}",
                        self.on(v.own_visible)
                    ),
                    Control::OwnVisible,
                    localized!(
                        self.language,
                        "Чужие приваты 3D: {}",
                        "Other claims in 3D: {}",
                        self.on(v.friends_visible)
                    ),
                    Control::FriendsVisible,
                ),
                adjust(
                    self.language.text("Дальность", "Distance"),
                    format!("{:.0}", v.distance),
                    Control::BorderAdjust(BorderOption::Distance, -1),
                    Control::BorderAdjust(BorderOption::Distance, 1),
                ),
                adjust(
                    self.language.text("Высота", "Height"),
                    if v.height >= 384.0 {
                        self.language.text("весь мир", "whole world").into()
                    } else {
                        format!("±{:.0}", v.height / 2.0)
                    },
                    Control::BorderAdjust(BorderOption::Height, -1),
                    Control::BorderAdjust(BorderOption::Height, 1),
                ),
                adjust(
                    self.language.text("Толщина", "Thickness"),
                    format!("{:.0}", v.line_width),
                    Control::BorderAdjust(BorderOption::Width, -1),
                    Control::BorderAdjust(BorderOption::Width, 1),
                ),
                adjust(
                    self.language.text("Непрозрачность", "Opacity"),
                    format!("{}%", (v.opacity * 100.0).round() as u8),
                    Control::BorderAdjust(BorderOption::Opacity, -1),
                    Control::BorderAdjust(BorderOption::Opacity, 1),
                ),
                pair(
                    localized!(
                        self.language,
                        "Шаг сетки: {}",
                        "Grid spacing: {}",
                        if p.grid_step == 0 {
                            self.language.text("только контур", "outline only").into()
                        } else {
                            p.grid_step.to_string()
                        }
                    ),
                    Control::BorderGrid,
                    self.language.text("Сбросить 3D", "Reset 3D").into(),
                    Control::BorderReset,
                ),
            ],
            ClaimsSection::Colours => vec![
                pair(
                    localized!(
                        self.language,
                        "Свои на карте: {}",
                        "Own claims on map: {}",
                        self.on(p.own)
                    ),
                    Control::MapOwn,
                    localized!(
                        self.language,
                        "Игроки на карте: {}",
                        "Players on map: {}",
                        self.on(p.friends)
                    ),
                    Control::MapFriends,
                ),
                vec![(
                    localized!(self.language, "Свои: {}", "Own: {}", self.own_claim_color),
                    Control::Field(Field::OwnClaimColor),
                )],
                vec![(
                    localized!(
                        self.language,
                        "Друзья: {}",
                        "Friends: {}",
                        self.friend_claim_color
                    ),
                    Control::Field(Field::FriendClaimColor),
                )],
                vec![(
                    self.language
                        .text("Применить цвета #RRGGBB", "Apply colors #RRGGBB")
                        .into(),
                    Control::ApplyClaimColours,
                )],
                adjust(
                    self.language.text("Заливка карты", "Map fill"),
                    format!("{}%", (p.fill * 100.0).round() as u8),
                    Control::MapAdjust(MapOption::Fill, -1),
                    Control::MapAdjust(MapOption::Fill, 1),
                ),
                adjust(
                    self.language.text("Контур карты", "Map outline"),
                    p.outline.to_string(),
                    Control::MapAdjust(MapOption::Outline, -1),
                    Control::MapAdjust(MapOption::Outline, 1),
                ),
                vec![(
                    localized!(
                        self.language,
                        "Подписи владельцев: {}",
                        "Owner labels: {}",
                        self.on(p.labels)
                    ),
                    Control::MapLabels,
                )],
            ],
            ClaimsSection::List => {
                out.push(widget(
                    [x, 80.0, w * 0.62 - 3.0, 18.0],
                    localized!(
                        self.language,
                        "Сортировка: {}",
                        "Sort: {}",
                        p.sort.label_in(self.language)
                    ),
                    Some(Control::MapSort),
                    false,
                ));
                out.push(widget(
                    [x + w * 0.62, 80.0, w * 0.38 - 3.0, 18.0],
                    if p.all_dimensions {
                        self.language.text("Все миры", "All worlds")
                    } else {
                        self.language.text("Этот мир", "This world")
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
                        self.language.text(
                            "Нет данных. Нажми «Обновить /claims» в «Карта».",
                            "No data. Use “Refresh /claims” under Map.",
                        ),
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
                    localized!(
                        self.language,
                        "{}/{} · {} чанков",
                        "{}/{} · {} chunks",
                        page + 1,
                        pages,
                        model.claim_rows.len()
                    ),
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
                self.language.text(
                    "Сохраняется местность рядом с игроком.",
                    "Records terrain near the player.",
                ),
                self.language.text(
                    "Серверы, аккаунты и измерения разделены.",
                    "Servers, accounts and dimensions are stored separately.",
                ),
                self.language.text(
                    "Другой мир на том же сервере? Выбери атлас.",
                    "Different world on the same server? Choose an atlas.",
                ),
                self.language.text(
                    "Смена атласа сохраняет предыдущую карту.",
                    "Switching atlases keeps the previous map.",
                ),
                self.language.text(
                    "Выключение записи не удаляет файлы.",
                    "Turning off recording keeps existing files.",
                ),
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
            (
                ChatSection::Appearance,
                self.language.text("Вид / позиция", "Style / position"),
            ),
            (
                ChatSection::Notifications,
                self.language.text("Уведомления", "Notifications"),
            ),
            (
                ChatSection::Spam,
                self.language.text("Антиспам", "Anti-spam"),
            ),
            (ChatSection::DeepL, "DeepL"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [x + i as f32 * w / 4.0, 56.0, w / 4.0 - 3.0, 18.0],
                label,
                Some(Control::ChatSection(section)),
                self.chat_section == section,
            ));
        }
        if self.chat_section == ChatSection::DeepL {
            let language = self.language;
            let masked = "*".repeat(self.deepl_key.len());
            let status = if !self.notice.is_empty() {
                &self.notice
            } else if model.deepl_configured {
                language.text("Ключ сохранён", "Key saved")
            } else {
                language.text("Ключ не настроен", "No key configured")
            };
            out.extend([
                widget(
                    [x, 80.0, w, 18.0],
                    language.text(
                        "DeepL используется при сбое Google",
                        "DeepL is used when Google fails",
                    ),
                    None,
                    false,
                ),
                widget(
                    [x, 102.0, w, 20.0],
                    if masked.is_empty() {
                        language
                            .text("Вставь ключ DeepL", "Paste your DeepL key")
                            .into()
                    } else {
                        masked
                    },
                    Some(Control::Field(Field::DeepLKey)),
                    self.focus == Some(Field::DeepLKey),
                ),
                widget(
                    [x, 126.0, w / 2.0 - 3.0, 20.0],
                    language.text("Сохранить", "Save"),
                    Some(Control::SaveDeepLKey),
                    false,
                ),
                widget(
                    [x + w / 2.0, 126.0, w / 2.0, 20.0],
                    language.text("Удалить ключ", "Remove key"),
                    Some(Control::RemoveDeepLKey),
                    false,
                ),
                widget([x, 151.0, w, 16.0], status, None, false),
                widget([x, 169.0, w, 16.0], &model.status, None, false),
            ]);
            return out;
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
            ChatSection::DeepL => unreachable!("DeepL has a separate editor"),
            ChatSection::Appearance => vec![
                vec![
                    (
                        localized!(
                            self.language,
                            "Чат: {}",
                            "Chat: {}",
                            self.on(model.chat_modern)
                        ),
                        Control::ChatModern,
                        model.chat_modern,
                    ),
                    (
                        localized!(
                            self.language,
                            "Перевод входящих: {}",
                            "Incoming translation: {}",
                            self.on(model.chat_translate)
                        ),
                        Control::ChatTranslate,
                        model.chat_translate,
                    ),
                ],
                vec![(
                    self.outgoing_label(model.chat_outgoing),
                    Control::ChatOutgoing,
                    model.chat_outgoing,
                )],
                pair(
                    ChatOption::Width,
                    self.language.text("Ширина", "Width"),
                    p.width.to_string(),
                ),
                pair(
                    ChatOption::Height,
                    self.language.text("Высота", "Height"),
                    p.height.to_string(),
                ),
                pair(
                    ChatOption::Scale,
                    self.language.text("Масштаб", "Scale"),
                    format!("{}%", (p.scale * 100.0).round() as u16),
                ),
                vec![(
                    localized!(
                        self.language,
                        "Компактные сообщения: {}",
                        "Compact messages: {}",
                        self.on(p.compact_messages)
                    ),
                    Control::ChatCompactMessages,
                    p.compact_messages,
                )],
                vec![(
                    localized!(
                        self.language,
                        "Положение: {}",
                        "Position: {}",
                        p.placement.label_in(self.language)
                    ),
                    Control::ChatPlacement,
                    false,
                )],
                pair(
                    ChatOption::OffsetX,
                    self.language.text("Сдвиг X", "X offset"),
                    p.offset_x.to_string(),
                ),
                pair(
                    ChatOption::OffsetY,
                    self.language.text("Сдвиг Y", "Y offset"),
                    p.offset_y.to_string(),
                ),
            ],
            ChatSection::Notifications => vec![
                vec![(
                    localized!(
                        self.language,
                        "Вид: {}",
                        "Style: {}",
                        p.notifications.label_in(self.language)
                    ),
                    Control::ChatStyle,
                    false,
                )],
                vec![(
                    localized!(
                        self.language,
                        "Входы / выходы: {}",
                        "Joins / leaves: {}",
                        self.on(p.presence)
                    ),
                    Control::ChatPresence,
                    p.presence,
                )],
                vec![(
                    localized!(
                        self.language,
                        "Формат входов: {}",
                        "Presence view: {}",
                        p.presence_style.label_in(self.language)
                    ),
                    Control::ChatPresenceStyle,
                    false,
                )],
                vec![(
                    localized!(self.language, "Смерти: {}", "Deaths: {}", self.on(p.deaths)),
                    Control::ChatDeaths,
                    p.deaths,
                )],
                pair(
                    ChatOption::Duration,
                    self.language.text("Время", "Duration"),
                    localized!(self.language, "{} с", "{} s", p.duration),
                ),
                pair(
                    ChatOption::Opacity,
                    self.language.text("Фон", "Background"),
                    format!("{}%", (p.opacity * 100.0).round() as u16),
                ),
                pair(
                    ChatOption::EventWidth,
                    self.language.text("Ширина событий", "Event width"),
                    p.event_width.to_string(),
                ),
            ],
            ChatSection::Priority => vec![
                vec![(
                    localized!(self.language, "Ник: {}", "Name: {}", self.priority_name),
                    Control::Field(Field::PriorityName),
                    self.focus == Some(Field::PriorityName),
                )],
                vec![(
                    localized!(self.language, "Цвет: {}", "Color: {}", self.priority_color),
                    Control::Field(Field::PriorityColor),
                    self.focus == Some(Field::PriorityColor),
                )],
                vec![(
                    localized!(
                        self.language,
                        "Закреплять сверху: {}",
                        "Pin to top: {}",
                        self.on(self.priority_pinned)
                    ),
                    Control::PriorityPin,
                    self.priority_pinned,
                )],
                vec![
                    (
                        self.language.text("Сохранить игрока", "Save player").into(),
                        Control::PrioritySave,
                        false,
                    ),
                    (
                        self.language.text("Удалить правило", "Remove rule").into(),
                        Control::PriorityRemove,
                        false,
                    ),
                    (
                        self.language.text("Следующий", "Next").into(),
                        Control::PriorityNext,
                        false,
                    ),
                ],
                vec![
                    (
                        self.language
                            .text("Друзья: всегда важные", "Friends: always important")
                            .into(),
                        Control::PriorityFriends,
                        model.priority.friends,
                    ),
                    (
                        localized!(
                            self.language,
                            "Верхняя лента: {}",
                            "Pinned feed: {}",
                            self.on(model.priority.tray)
                        ),
                        Control::PriorityTray,
                        model.priority.tray,
                    ),
                ],
                vec![
                    (
                        localized!(
                            self.language,
                            "Время − {}с",
                            "Duration − {}s",
                            model.priority.seconds
                        ),
                        Control::PriorityTime(-1),
                        false,
                    ),
                    (
                        self.language.text("Время +", "Duration +").into(),
                        Control::PriorityTime(1),
                        false,
                    ),
                ],
                vec![
                    (
                        localized!(
                            self.language,
                            "Строк − {}",
                            "Rows − {}",
                            model.priority.rows
                        ),
                        Control::PriorityRows(-1),
                        false,
                    ),
                    (
                        localized!(
                            self.language,
                            "Строк + · правил {}",
                            "Rows + · {} rules",
                            model.priority.rules.len()
                        ),
                        Control::PriorityRows(1),
                        false,
                    ),
                ],
            ],
            ChatSection::Spam => vec![
                vec![(
                    localized!(
                        self.language,
                        "Режим: {}",
                        "Mode: {}",
                        p.repeats.label_in(self.language)
                    ),
                    Control::ChatRepeats,
                    false,
                )],
                pair(
                    ChatOption::RepeatWindow,
                    self.language.text("Интервал", "Interval"),
                    localized!(self.language, "{} с", "{} s", p.repeat_window),
                ),
                vec![(
                    localized!(
                        self.language,
                        "Игнорировать префикс: {} симв.",
                        "Ignore prefix: {} chars",
                        p.ignore_prefix
                    ),
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
                self.language
                    .text("Сбросить настройки чата", "Reset chat settings"),
                Some(Control::ChatReset),
                false,
            ));
            for (i, label) in [
                self.language.text(
                    "Одинаковые сообщения одного автора → ×N",
                    "Repeated messages from one author → ×N",
                ),
                self.language.text(
                    "ЛКМ по сообщению: оригинал / перевод",
                    "Left-click a message to switch original / translation",
                ),
                self.language.text(
                    "Префикс 10 / 11 — для времени [12:34:56]",
                    "Use a 10/11 character prefix for timestamps like [12:34:56]",
                ),
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
    fn experimental_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        let tab_width = (w - 4.0) * 0.5;
        for (index, section, label) in [
            (
                0,
                ExperimentalSection::Ores,
                self.language.text("Руды", "Ores"),
            ),
            (1, ExperimentalSection::Baritone, "Baritone"),
        ] {
            out.push(widget(
                [x + index as f32 * (tab_width + 4.0), 56.0, tab_width, 20.0],
                label,
                Some(Control::ExperimentalTab(section)),
                self.experimental_section == section,
            ));
        }
        if self.experimental_section == ExperimentalSection::Baritone {
            let mode_width = (w - 24.0) / 7.0;
            for (i, mode, label) in [
                (
                    0,
                    BaritoneMode::Navigation,
                    self.language.text("Навигация", "Navigation"),
                ),
                (
                    1,
                    BaritoneMode::Mining,
                    self.language.text("Добыча", "Mining"),
                ),
                (
                    2,
                    BaritoneMode::Tunnel,
                    self.language.text("Тоннель", "Tunnel"),
                ),
                (
                    3,
                    BaritoneMode::Bridge,
                    self.language.text("Мост", "Bridge"),
                ),
                (4, BaritoneMode::Farm, self.language.text("Ферма", "Farm")),
                (5, BaritoneMode::Drain, self.language.text("Вода", "Drain")),
                (
                    6,
                    BaritoneMode::Schematic,
                    self.language.text("Схемы", "Schematics"),
                ),
            ] {
                out.push(widget(
                    [x + i as f32 * (mode_width + 4.0), 80.0, mode_width, 16.0],
                    label,
                    Some(Control::BaritoneMode(mode)),
                    self.baritone_mode == mode,
                ));
            }
            let step = ((height - 148.0) / 4.0).clamp(12.0, 26.0);
            let h = (step - 2.0).min(22.0);
            if matches!(
                self.baritone_mode,
                BaritoneMode::Farm | BaritoneMode::Schematic
            ) {
                let farm = self.baritone_mode == BaritoneMode::Farm;
                let rows: Vec<(String, Control)> = if farm {
                    vec![
                        (
                            match self.farm_crop {
                                super::claim_borders_farming::Crop::Wheat => {
                                    self.language.text("Пшеница", "Wheat")
                                }
                                super::claim_borders_farming::Crop::Carrots => {
                                    self.language.text("Морковь", "Carrots")
                                }
                                super::claim_borders_farming::Crop::Potatoes => {
                                    self.language.text("Картофель", "Potatoes")
                                }
                                super::claim_borders_farming::Crop::Beetroot => {
                                    self.language.text("Свёкла", "Beetroot")
                                }
                            }
                            .into(),
                            Control::FarmCrop,
                        ),
                        (
                            localized!(self.language, "Радиус: {}", "Radius: {}", self.farm_radius),
                            Control::FarmRadius,
                        ),
                        (
                            localized!(
                                self.language,
                                "Лимит: {} (0 = ∞)",
                                "Limit: {} (0 = ∞)",
                                self.farm_limit
                            ),
                            Control::FarmLimit,
                        ),
                        (
                            localized!(
                                self.language,
                                "Пересадка: {}",
                                "Replant: {}",
                                if self.farm_replant {
                                    self.language.text("Вкл", "On")
                                } else {
                                    self.language.text("Выкл", "Off")
                                }
                            ),
                            Control::FarmReplant,
                        ),
                    ]
                } else {
                    vec![
                        (
                            self.schematic_file.clone().unwrap_or_else(|| {
                                self.language.text("Выберите файл", "Select file").into()
                            }),
                            Control::SchematicFile,
                        ),
                        (
                            self.language.text("Обновить", "Refresh").into(),
                            Control::SchematicRefresh,
                        ),
                        (
                            format!("XYZ: {}", self.schematic_origin),
                            Control::Field(Field::SchematicOrigin),
                        ),
                        (
                            format!("{}°", self.schematic_rotation * 90),
                            Control::SchematicRotation,
                        ),
                    ]
                };
                for (i, (label, c)) in rows.into_iter().enumerate() {
                    out.push(widget(
                        [
                            x + (i % 2) as f32 * (tab_width + 4.0),
                            100.0 + (i / 2) as f32 * step,
                            tab_width,
                            h,
                        ],
                        label,
                        Some(c),
                        false,
                    ));
                }
            } else if matches!(
                self.baritone_mode,
                BaritoneMode::Tunnel | BaritoneMode::Bridge
            ) {
                let compact_tunnel = self.baritone_mode == BaritoneMode::Tunnel && height < 320.0;
                out.push(widget(
                    [x, 100.0, if compact_tunnel { tab_width } else { w }, h],
                    localized!(
                        self.language,
                        "Длина: {}",
                        "Length: {}",
                        self.construct_length
                    ),
                    Some(Control::ConstructLength),
                    false,
                ));
                if self.baritone_mode == BaritoneMode::Tunnel {
                    for (i, (label, control, on)) in [
                        (
                            localized!(
                                self.language,
                                "Ширина: {}",
                                "Width: {}",
                                self.construct_width
                            ),
                            Control::ConstructWidth,
                            false,
                        ),
                        (
                            localized!(
                                self.language,
                                "Высота: {}",
                                "Height: {}",
                                self.construct_height
                            ),
                            Control::ConstructHeight,
                            false,
                        ),
                        (
                            localized!(
                                self.language,
                                "Пол: {}",
                                "Floor: {}",
                                if self.construct_floor {
                                    self.language.text("да", "on")
                                } else {
                                    self.language.text("нет", "off")
                                }
                            ),
                            Control::ConstructFloor,
                            self.construct_floor,
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        out.push(widget(
                            if compact_tunnel {
                                [
                                    x + ((i + 1) % 2) as f32 * (tab_width + 4.0),
                                    100.0 + i.div_ceil(2) as f32 * step,
                                    tab_width,
                                    h,
                                ]
                            } else {
                                [x, 100.0 + (i + 1) as f32 * step, w, h]
                            },
                            label,
                            Some(control),
                            on,
                        ));
                    }
                } else {
                    out.push(widget(
                        [x, 100.0 + step, w, h],
                        self.language.text(
                            "Блоки: хотбар, вторая рука или инвентарь",
                            "Blocks: hotbar, off hand or inventory",
                        ),
                        None,
                        false,
                    ));
                }
            } else if self.baritone_mode == BaritoneMode::Drain {
                out.push(widget(
                    [x, 100.0, w, h],
                    localized!(self.language, "Радиус: {}", "Radius: {}", self.drain_radius),
                    Some(Control::DrainRadius),
                    false,
                ));
                out.push(widget(
                    [x, 100.0 + step, w, 14.0],
                    self.language.text(
                        "Засыпает воду снизу вверх блоками из инвентаря (в т. ч. песком)",
                        "Fills water from the bottom up with inventory blocks (sand too)",
                    ),
                    None,
                    false,
                ));
            } else if self.baritone_mode == BaritoneMode::Mining {
                // A chip per target, toggled like the structure filters on the map.
                use super::claim_borders_mining as mining;
                let top = 100.0 + step;
                let cols = 3;
                let chip_w = (w - 8.0) / cols as f32;
                let rows = (((height - 50.0 - top) / 16.0).floor() as usize).max(1);
                let per_page = rows * cols;
                let pages = mining::COUNT.div_ceil(per_page);
                let page = self.mining_page % pages;
                out.push(widget(
                    [x, 100.0, tab_width, h],
                    localized!(self.language, "Блоков: {}", "Blocks: {}", self.mining_limit),
                    Some(Control::MiningLimit),
                    false,
                ));
                out.push(widget(
                    [x + tab_width + 4.0, 100.0, tab_width, h],
                    if pages > 1 {
                        localized!(
                            self.language,
                            "Выбрано: {} · стр. {}/{} →",
                            "Selected: {} · page {}/{} →",
                            (self.mining_targets & mining::ALL).count_ones(),
                            page + 1,
                            pages
                        )
                    } else {
                        localized!(
                            self.language,
                            "Выбрано: {}",
                            "Selected: {}",
                            (self.mining_targets & mining::ALL).count_ones()
                        )
                    },
                    Some(Control::MiningPage),
                    false,
                ));
                for slot in 0..per_page {
                    let index = page * per_page + slot;
                    let Some((ru, en, _)) = mining::target(index as u8) else {
                        break;
                    };
                    out.push(widget(
                        [
                            x + (slot % cols) as f32 * (chip_w + 4.0),
                            top + (slot / cols) as f32 * 16.0,
                            chip_w,
                            14.0,
                        ],
                        self.language.text(ru, en),
                        Some(Control::MiningTarget(index as u8)),
                        self.mining_targets & (1 << index) != 0,
                    ));
                }
            } else {
                let waypoint = self
                    .navigation_waypoint
                    .and_then(|id| model.navigation_waypoints.iter().find(|w| w.id == id));
                let player = self.navigation_player.and_then(|id| {
                    model
                        .navigation_players
                        .iter()
                        .find(|(uuid, _)| *uuid == id)
                });
                let field_width = (w - 8.0) / 3.0;
                for (i, (label, field, value)) in [
                    ("X", Field::NavigationX, &self.navigation_x),
                    ("Y", Field::NavigationY, &self.navigation_y),
                    ("Z", Field::NavigationZ, &self.navigation_z),
                ]
                .into_iter()
                .enumerate()
                {
                    out.push(widget(
                        [x + i as f32 * (field_width + 4.0), 100.0, field_width, h],
                        format!("{label}: {value}"),
                        Some(Control::Field(field)),
                        self.focus == Some(field),
                    ));
                }
                out.push(widget(
                    [x, 100.0 + step, tab_width, h],
                    localized!(
                        self.language,
                        "Метка: {}",
                        "Waypoint: {}",
                        waypoint.map_or(self.language.text("выбрать", "select"), |w| w
                            .name
                            .as_str())
                    ),
                    Some(Control::NavigationWaypoint),
                    waypoint.is_some(),
                ));
                out.push(widget(
                    [x + tab_width + 4.0, 100.0 + step, tab_width, h],
                    localized!(
                        self.language,
                        "Игрок: {}",
                        "Player: {}",
                        player.map_or(
                            self.language.text("выбрать", "select"),
                            |(_, name)| name.as_str()
                        )
                    ),
                    Some(Control::NavigationPlayer),
                    player.is_some(),
                ));
            }
            out.push(widget(
                [x, height - 48.0, w, 14.0],
                &model.navigation_status,
                None,
                false,
            ));
            if self.baritone_mode == BaritoneMode::Schematic {
                out.push(widget(
                    [x, height - 30.0, tab_width, 14.0],
                    self.language
                        .text("Материалы → буфер", "Materials → clipboard"),
                    Some(Control::SchematicMaterials),
                    false,
                ));
                out.push(widget(
                    [x + tab_width + 4.0, height - 30.0, tab_width, 14.0],
                    self.language.text("Открыть папку", "Open folder"),
                    Some(Control::SchematicFolder),
                    false,
                ));
                // The two buttons above use the last line; the keys go one row higher
                // when the panel is tall enough not to touch the controls.
                if height >= 300.0 {
                    out.push(widget(
                        [x, height - 66.0, w, 14.0],
                        self.hotkey_hint(model),
                        None,
                        false,
                    ));
                }
            } else {
                out.push(widget(
                    [x, height - 30.0, w, 14.0],
                    self.hotkey_hint(model),
                    None,
                    false,
                ));
            }
            return out;
        }
        let p = model.experimental;
        let selected = usize::from(p.selected);
        for (index, (label, control, on)) in [
            (
                localized!(
                    self.language,
                    "Разрешить подсветку: {}",
                    "Allow highlighting: {}",
                    if p.allowed {
                        self.language.text("вкл.", "on")
                    } else {
                        self.language.text("выкл.", "off")
                    }
                ),
                Control::ExperimentalAllowed,
                p.allowed,
            ),
            (
                localized!(
                    self.language,
                    "Источник: {}",
                    "Source: {}",
                    if p.prediction {
                        self.language
                            .text("ПРОГНОЗ сида SMP", "SMP seed prediction")
                    } else {
                        self.language.text("загруженные чанки", "loaded chunks")
                    }
                ),
                Control::ExperimentalSource,
                p.prediction,
            ),
            (
                localized!(
                    self.language,
                    "Тип руды: {} →",
                    "Ore type: {} →",
                    self.language.text(
                        experimental::ORES[selected].label,
                        experimental::ORES[selected].label_en
                    )
                ),
                Control::ExperimentalNext,
                false,
            ),
            (
                localized!(
                    self.language,
                    "Показывать этот тип: {}",
                    "Show this type: {}",
                    if p.includes(selected) {
                        self.language.text("да", "yes")
                    } else {
                        self.language.text("нет", "no")
                    }
                ),
                Control::ExperimentalOre,
                p.includes(selected),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let step = ((height - 132.0) / 4.0).clamp(16.0, 28.0);
            let y = 82.0 + index as f32 * step;
            out.push(widget(
                [x, y, w, (step - 2.0).min(22.0)],
                label,
                Some(control),
                on,
            ));
        }

        out.push(widget(
            [x, height - 48.0, w, 14.0],
            self.language.text(
                "O — включить/выключить · после входа всегда выключено",
                "O toggles highlighting · off after joining",
            ),
            None,
            false,
        ));
        out.push(widget(
            [x, height - 30.0, w, 14.0],
            &model.experimental_status,
            None,
            false,
        ));
        out
    }
    fn important_names(&self, model: &ViewModel) -> Vec<String> {
        let mut names: std::collections::BTreeMap<String, String> = model
            .priority
            .rules
            .keys()
            .map(|n| (n.clone(), n.clone()))
            .collect();
        for p in model.party.iter().filter(|p| p.friend) {
            names.insert(p.name.to_ascii_lowercase(), p.name.clone());
        }
        names.into_values().collect()
    }
    fn party_indices(&self, model: &ViewModel) -> Vec<usize> {
        model
            .party
            .iter()
            .enumerate()
            .filter(|(_, p)| match self.party_filter {
                PartyFilter::Friends => {
                    p.friend
                        || model
                            .priority
                            .rules
                            .contains_key(&p.name.to_ascii_lowercase())
                }
                PartyFilter::Users => (p.online || p.pending) && !p.local_only,
                PartyFilter::Invitations => p.pending,
                PartyFilter::Important => false,
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
        if !self.relay_setup {
            out.push(widget(
                [x, height - 32.0, (w - 100.0).max(100.0), 12.0],
                if self.notice.is_empty() {
                    &model.status
                } else {
                    &self.notice
                },
                None,
                false,
            ));
        }
        if self.relay_setup {
            out.push(widget(
                [x, 54.0, w, 12.0],
                self.language.text(
                    "Адрес вашего relay (ws:// или wss://)",
                    "Relay address (ws:// or wss://)",
                ),
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
                    self.language.text(
                        "Токен сохранён; пустое поле сохраняет его",
                        "Token saved. Leave blank to keep it.",
                    )
                } else {
                    self.language
                        .text("Токен доступа (если нужен)", "Access token (if required)")
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
                (Control::RelaySave, self.language.text("Сохранить", "Save")),
                (
                    Control::ClearRelayToken,
                    self.language.text("Удалить токен", "Remove token"),
                ),
                (
                    Control::RelayConnect,
                    self.language.text("Подключить", "Connect"),
                ),
                (
                    Control::RelayOff,
                    self.language.text("Отключить", "Disconnect"),
                ),
                (Control::RelayBack, self.language.text("Назад", "Back")),
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
                localized!(
                    self.language,
                    "Автоподключение: {}",
                    "Auto-connect: {}",
                    self.on(model.relay_auto)
                ),
                Some(Control::ToggleAutoRelay),
                model.relay_auto,
            ));
            out.push(widget(
                [x, 212.0, w, 10.0],
                self.language.text(
                    "После сохранения relay подключается автоматически.",
                    "Relay connects automatically after saving.",
                ),
                None,
                false,
            ));
        } else {
            for (i, (filter, title)) in [
                (
                    PartyFilter::Friends,
                    self.language.text("Друзья", "Friends"),
                ),
                (
                    PartyFilter::Users,
                    self.language.text("С модом", "Mod users"),
                ),
                (
                    PartyFilter::Important,
                    self.language.text("Важные", "Important"),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x + i as f32 * w / 3.0, 56.0, w / 3.0 - 3.0, 20.0],
                    title,
                    Some(Control::PartyFilter(filter)),
                    self.party_filter == filter,
                ));
            }
            if self.party_filter == PartyFilter::Important {
                self.layout_priorities(model, x, w, height, &mut out);
                return out;
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
                let y = 82.0 + slot as f32 * 24.0;
                out.push(widget(
                    [x, y, w - 48.0, 22.0],
                    format!(
                        "{}{} · {}",
                        row.name,
                        if model.priority.rule(&row.name, row.friend).is_some() {
                            self.language.text(" · Важный", " · Important")
                        } else {
                            ""
                        },
                        row.detail
                    ),
                    Some(Control::Row(index)),
                    self.selected == Some(index),
                ));
                if row.pending || (!row.friend && !row.outgoing && !row.blocked && !row.local_only)
                {
                    out.push(widget(
                        [x + w - 46.0, y, 22.0, 22.0],
                        if row.pending {
                            self.language.text("Принять", "Accept")
                        } else {
                            self.language.text("Добавить друга", "Add friend")
                        },
                        Some(Control::PartyAccept(index)),
                        false,
                    ));
                }
                if row.pending || row.friend {
                    out.push(widget(
                        [x + w - 22.0, y, 22.0, 22.0],
                        if row.pending {
                            self.language.text("Отклонить", "Decline")
                        } else {
                            self.language.text("Удалить друга", "Remove friend")
                        },
                        Some(Control::PartyDecline(index)),
                        false,
                    ));
                }
            }
            if indices.is_empty() {
                out.push(widget(
                    [x, 86.0, w, 18.0],
                    self.language.text(
                        "Пока нет игроков в этой группе",
                        "No players in this group yet",
                    ),
                    None,
                    false,
                ));
            }
            let b = 82.0 + per as f32 * 24.0;
            out.push(widget(
                [x, b, 24.0, 18.0],
                "←",
                Some(Control::Previous),
                false,
            ));
            out.push(widget(
                [x + 28.0, b, w - 56.0, 18.0],
                format!("{} / {}", page + 1, pages),
                None,
                false,
            ));
            out.push(widget(
                [x + w - 24.0, b, 24.0, 18.0],
                "→",
                Some(Control::Next),
                false,
            ));
            for (i, (kind, label)) in [
                (
                    PlayerRequest::Teleport,
                    self.language.text("Запрос ТП", "Teleport request"),
                ),
                (PlayerRequest::Trade, self.language.text("Обмен", "Trade")),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(widget(
                    [x + i as f32 * w / 2.0, height - 56.0, w / 2.0 - 3.0, 20.0],
                    label,
                    Some(Control::PlayerRequest(kind)),
                    false,
                ));
            }
        }
        out
    }
    fn layout_priorities(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        out: &mut Vec<Widget>,
    ) {
        let names = self.important_names(model);
        let per = self.page_size(height);
        let pages = names.len().div_ceil(per).max(1);
        let page = self.page.min(pages - 1);
        for (i, name) in names.iter().enumerate().skip(page * per).take(per) {
            let automatic = !model
                .priority
                .rules
                .contains_key(&name.to_ascii_lowercase());
            out.push(widget(
                [x, 82.0 + (i - page * per) as f32 * 24.0, w, 22.0],
                if automatic {
                    localized!(self.language, "{name} · друг", "{name} · friend")
                } else {
                    name.clone()
                },
                Some(Control::PriorityRow(i)),
                self.priority_name.eq_ignore_ascii_case(name),
            ));
        }
        if names.is_empty() {
            out.push(widget(
                [x, 82.0, w, 22.0],
                self.language
                    .text("Введите ник игрока ниже", "Enter a player name below"),
                None,
                false,
            ));
        }
        let y = height - 94.0;
        out.extend([
            widget([x, y, 24.0, 18.0], "←", Some(Control::Previous), false),
            widget(
                [x + 28.0, y, w - 56.0, 18.0],
                format!("{} / {}", page + 1, pages),
                None,
                false,
            ),
            widget(
                [x + w - 24.0, y, 24.0, 18.0],
                "→",
                Some(Control::Next),
                false,
            ),
            widget(
                [x, y + 20.0, w * 0.6 - 3.0, 20.0],
                localized!(self.language, "Ник: {}", "Name: {}", self.priority_name),
                Some(Control::Field(Field::PriorityName)),
                self.focus == Some(Field::PriorityName),
            ),
            widget(
                [x + w * 0.6, y + 20.0, w * 0.4, 20.0],
                self.language.text("Сохранить", "Save"),
                Some(Control::PrioritySave),
                false,
            ),
            widget(
                [x, y + 42.0, w * 0.6 - 3.0, 20.0],
                localized!(self.language, "Цвет: {}", "Color: {}", self.priority_color),
                Some(Control::Field(Field::PriorityColor)),
                self.focus == Some(Field::PriorityColor),
            ),
            widget(
                [x + w * 0.6, y + 42.0, w * 0.4, 20.0],
                self.language.text("Убрать правило", "Remove rule"),
                Some(Control::PriorityRemove),
                false,
            ),
        ]);
    }
    pub fn click(&mut self, model: &ViewModel, size: [f32; 2], mouse: [f32; 2]) -> Option<Action> {
        let hit = self
            .layout(model, size[0], size[1])
            .into_iter()
            .rev()
            .find(|w| w.control.is_some() && contains(w.rect, mouse));
        let Some(control) = hit.and_then(|w| w.control) else {
            self.focus = None;
            self.navigation_choices = None;
            self.schematic_choices = false;
            return None;
        };
        if self.schematic_choices
            && !matches!(
                control,
                Control::SchematicFile
                    | Control::SchematicChoice(_)
                    | Control::SchematicChoicesPage(_)
                    | Control::SchematicOpenFolder
            )
        {
            self.schematic_choices = false;
        }
        if self.navigation_choices.is_some()
            && !matches!(
                control,
                Control::NavigationWaypoint
                    | Control::NavigationPlayer
                    | Control::NavigationWaypointChoice(_)
                    | Control::NavigationPlayerChoice(_)
                    | Control::NavigationChoicesPage(_)
            )
        {
            self.navigation_choices = None;
        }
        self.activate(model, (size[1] - 16.0).clamp(200.0, 400.0), control)
    }
    fn activate(&mut self, model: &ViewModel, height: f32, control: Control) -> Option<Action> {
        if self.experiments_prompt {
            match control {
                Control::ConfirmExperiments => {
                    self.experiments_prompt = false;
                    self.tab = Tab::Experimental;
                    return Some(Action::AcceptExperiments);
                }
                Control::DeclineExperiments | Control::Close => self.experiments_prompt = false,
                _ => {}
            }
            return None;
        }
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
            Control::ConstructDirection => {
                self.construct_direction = self.construct_direction.next();
                None
            }
            Control::ConstructLength => {
                let values = [1, 8, 16, 32, 64];
                let i = values
                    .iter()
                    .position(|v| *v == self.construct_length)
                    .unwrap_or(0);
                self.construct_length = values[(i + 1) % values.len()];
                None
            }
            Control::ConstructWidth => {
                self.construct_width = self.construct_width % 3 + 1;
                None
            }
            Control::ConstructHeight => {
                self.construct_height = if self.construct_height == 2 { 3 } else { 2 };
                None
            }
            Control::ConstructFloor => {
                self.construct_floor = !self.construct_floor;
                None
            }
            Control::FarmCrop => {
                self.farm_crop = self.farm_crop.next();
                None
            }
            Control::FarmRadius => {
                self.farm_radius = match self.farm_radius {
                    3 => 6,
                    6 => 9,
                    9 => 12,
                    _ => 3,
                };
                None
            }
            Control::FarmLimit => {
                self.farm_limit = match self.farm_limit {
                    0 => 16,
                    16 => 32,
                    32 => 64,
                    _ => 0,
                };
                None
            }
            Control::FarmReplant => {
                self.farm_replant = !self.farm_replant;
                None
            }
            Control::DrainRadius => {
                self.drain_radius = match self.drain_radius {
                    3 => 6,
                    6 => 9,
                    9 => 12,
                    _ => 3,
                };
                None
            }
            Control::SchematicFile => {
                self.focus = None;
                if model.schematic_files.is_empty() {
                    // Nothing to choose yet: show the folder so files can be put there.
                    self.schematic_choices = false;
                    return Some(Action::OpenSchematicFolder);
                }
                self.schematic_choices = !self.schematic_choices;
                self.schematic_choices_page = 0;
                // Pick up files added since the list was last read.
                self.schematic_choices.then_some(Action::RefreshSchematics)
            }
            Control::SchematicChoice(index) => {
                if let Some(name) = model.schematic_files.get(index) {
                    self.schematic_file = Some(name.clone());
                }
                self.schematic_choices = false;
                None
            }
            Control::SchematicChoicesPage(direction) => {
                let pages = (model.schematic_files.len() + 1).div_ceil(8);
                self.schematic_choices_page =
                    (self.schematic_choices_page as isize + direction as isize)
                        .rem_euclid(pages as isize) as usize;
                None
            }
            Control::SchematicOpenFolder => {
                self.schematic_choices = false;
                Some(Action::OpenSchematicFolder)
            }
            Control::SchematicRefresh => Some(Action::RefreshSchematics),
            Control::SchematicRotation => {
                self.schematic_rotation = (self.schematic_rotation + 1) % 4;
                None
            }
            Control::SchematicMaterials => Some(Action::SchematicInfo(false)),
            Control::SchematicFolder => Some(Action::OpenSchematicFolder),
            Control::BaritoneMode(mode) => {
                self.baritone_mode = mode;
                self.navigation_choices = None;
                self.schematic_choices = false;
                self.focus = None;
                None
            }
            Control::MiningTarget(index) => {
                if usize::from(index) < super::claim_borders_mining::COUNT {
                    self.mining_targets ^= 1 << index;
                }
                None
            }
            Control::MiningPage => {
                self.mining_page = self.mining_page.wrapping_add(1);
                None
            }
            Control::MiningLimit => {
                let limits = [1, 8, 16, 32, 64];
                let at = limits
                    .iter()
                    .position(|n| *n == self.mining_limit)
                    .unwrap_or(0);
                self.mining_limit = limits[(at + 1) % limits.len()];
                None
            }
            Control::NavigationPlayer => {
                self.navigation_choices = if self.navigation_choices == Some(true) {
                    None
                } else {
                    Some(true)
                };
                self.navigation_choices_page = 0;
                self.focus = None;
                None
            }
            Control::NavigationWaypoint => {
                self.navigation_choices = if self.navigation_choices == Some(false) {
                    None
                } else {
                    Some(false)
                };
                self.navigation_choices_page = 0;
                self.focus = None;
                None
            }
            Control::NavigationChoicesPage(direction) => {
                let count = if self.navigation_choices == Some(true) {
                    model.navigation_players.len()
                } else {
                    model.navigation_waypoints.len()
                };
                let pages = count.div_ceil(8);
                if pages > 0 {
                    self.navigation_choices_page =
                        (self.navigation_choices_page as isize + direction as isize)
                            .rem_euclid(pages as isize) as usize;
                }
                None
            }
            Control::NavigationPlayerChoice(index) => {
                if let Some((uuid, _)) = model.navigation_players.get(index) {
                    self.navigation_player = Some(*uuid);
                    self.navigation_waypoint = None;
                }
                self.navigation_choices = None;
                self.focus = None;
                None
            }
            Control::NavigationWaypointChoice(index) => {
                if let Some(waypoint) = model.navigation_waypoints.get(index) {
                    self.navigation_waypoint = Some(waypoint.id);
                    self.navigation_player = None;
                }
                self.navigation_choices = None;
                self.focus = None;
                None
            }
            Control::ExperimentalTab(section) => {
                self.experimental_section = section;
                self.focus = None;
                None
            }
            Control::Language => Some(Action::SetLanguage(self.language.other())),
            Control::Appearance => Some(Action::Appearance),
            Control::EconomySetting(key, dir) => Some(Action::EconomySetting(key, dir)),
            Control::ToggleReconnect => Some(Action::ToggleReconnect),
            Control::SettingsTab(section) => {
                self.settings_section = section.min(6);
                self.settings_page = 0;
                self.focus = None;
                self.deepl_key.clear();
                if self.settings_section == 1 {
                    self.relay_url = model.relay_url.clone();
                }
                None
            }
            Control::SettingsSection(dir) => {
                self.deepl_key.clear();
                self.settings_section = (self.settings_section as i8 + dir).rem_euclid(7) as u8;
                self.settings_page = 0;
                self.focus = None;
                if self.settings_section == 1 {
                    self.relay_url = model.relay_url.clone();
                }
                None
            }
            Control::SettingsPage(dir) => {
                let per = (((height - 138.0) / 22.0).floor() as usize).max(1);
                let count = super::claim_borders_economy::SETTINGS
                    .iter()
                    .filter(|s| match self.settings_section {
                        4 => s.group <= 4,
                        5 => s.group == 6,
                        _ => s.group == 5,
                    })
                    .count();
                self.settings_page = self
                    .settings_page
                    .saturating_add_signed(dir as isize)
                    .min(count.div_ceil(per).max(1) - 1);
                None
            }
            Control::ClaimFilter => {
                self.claim_filter = (self.claim_filter + 1) % 3;
                self.claims_page = 0;
                None
            }
            Control::PartyAccept(index) => {
                self.selected = Some(index);
                self.activate(
                    model,
                    height,
                    if model.party.get(index).is_some_and(|p| p.pending) {
                        Control::Accept
                    } else {
                        Control::Invite
                    },
                )
            }
            Control::PartyDecline(index) => {
                self.selected = Some(index);
                self.activate(
                    model,
                    height,
                    if model.party.get(index).is_some_and(|p| p.pending) {
                        Control::Decline
                    } else {
                        Control::Remove
                    },
                )
            }
            Control::Economy => Some(Action::OpenEconomy),
            Control::ToggleAutoTrust => Some(Action::ToggleAutoTrust),
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
                let per = (((height - 150.0) / 20.0).floor() as usize).clamp(1, 12);
                self.claims_page = (self.claims_page + 1)
                    .min(self.claim_indices(model).len().div_ceil(per).max(1) - 1);
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
                    self.notice = self
                        .language
                        .text("Цвет должен быть #RRGGBB", "Use the #RRGGBB color format")
                        .into();
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
                self.deepl_key.clear();
                self.focus = None;
                self.chat_section = section;
                None
            }
            Control::SaveDeepLKey => {
                if !valid_deepl_key(&self.deepl_key) {
                    self.notice = self
                        .language
                        .text("Введите ключ DeepL", "Enter a DeepL key")
                        .into();
                    return None;
                }
                self.focus = None;
                Some(Action::SetDeepLKey(Some(std::mem::take(
                    &mut self.deepl_key,
                ))))
            }
            Control::RemoveDeepLKey => {
                self.deepl_key.clear();
                self.focus = None;
                Some(Action::SetDeepLKey(None))
            }
            Control::ChatReset => Some(Action::SetChat(Preferences::default())),
            Control::ChatCompactMessages => {
                let mut p = model.chat_preferences;
                p.compact_messages = !p.compact_messages;
                Some(Action::SetChat(p))
            }
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
            Control::ChatPresenceStyle => {
                let mut p = model.chat_preferences;
                p.presence_style = p.presence_style.next();
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

            Control::ExperimentalAllowed
            | Control::ExperimentalSource
            | Control::ExperimentalNext
            | Control::ExperimentalOre => {
                let mut p = model.experimental;
                match control {
                    Control::ExperimentalAllowed => p.allowed = !p.allowed,
                    Control::ExperimentalSource => p.prediction = !p.prediction,
                    Control::ExperimentalNext => {
                        p.selected = (p.selected + 1) % experimental::ORES.len() as u8
                    }
                    Control::ExperimentalOre => p.ores ^= 1 << p.selected,
                    _ => {}
                }
                Some(Action::SetExperimental(p.normalized()))
            }
            Control::PriorityRow(i) => {
                if let Some(name) = self.important_names(model).get(i) {
                    self.priority_name = name.clone();
                    self.focus = None;
                    if let Some(r) = model.priority.rule(
                        name,
                        model
                            .party
                            .iter()
                            .any(|p| p.friend && p.name.eq_ignore_ascii_case(name)),
                    ) {
                        self.priority_color = format!("#{:06X}", r.color);
                        self.priority_pinned = r.pinned;
                    }
                }
                None
            }
            Control::PriorityNext => {
                let rules = &model.priority.rules;
                let next = rules
                    .range((
                        std::ops::Bound::Excluded(self.priority_name.to_ascii_lowercase()),
                        std::ops::Bound::Unbounded,
                    ))
                    .next()
                    .or_else(|| rules.iter().next());
                if let Some((name, r)) = next {
                    self.priority_name = name.clone();
                    self.priority_color = format!("#{:06X}", r.color);
                    self.priority_pinned = r.pinned;
                }
                None
            }
            Control::PriorityPin => {
                self.priority_pinned = !self.priority_pinned;
                None
            }
            Control::PrioritySave
            | Control::PriorityRemove
            | Control::PriorityFriends
            | Control::PriorityTray
            | Control::PriorityTime(_)
            | Control::PriorityRows(_) => {
                let mut p = model.priority.clone();
                match control {
                    Control::PrioritySave => {
                        let color = u32::from_str_radix(
                            self.priority_color.trim().trim_start_matches('#'),
                            16,
                        )
                        .ok()
                        .filter(|c| *c <= 0xffffff);
                        let Some(color) = color else {
                            self.notice = self
                                .language
                                .text(
                                    "Цвет нужен в формате #RRGGBB",
                                    "Use the #RRGGBB color format",
                                )
                                .into();
                            return None;
                        };
                        if let Err(e) = p.set_in(
                            self.priority_name.trim(),
                            color,
                            self.priority_pinned,
                            self.language,
                        ) {
                            self.notice = e;
                            return None;
                        }
                    }
                    Control::PriorityRemove => {
                        p.rules
                            .remove(&self.priority_name.trim().to_ascii_lowercase());
                    }
                    Control::PriorityFriends => p.friends = true,
                    Control::PriorityTray => p.tray = !p.tray,
                    Control::PriorityTime(dir) => {
                        p.seconds = (p.seconds as i32 + dir as i32 * 10).clamp(10, 180) as u16
                    }
                    Control::PriorityRows(dir) => {
                        p.rows = (p.rows as i32 + dir as i32).clamp(1, 6) as u8
                    }
                    _ => {}
                }
                Some(Action::SetPriority(p))
            }
            Control::ChatOutgoing => Some(Action::ToggleOutgoing),
            Control::ChatModern => Some(Action::ToggleChat),
            Control::ChatTranslate => Some(Action::ToggleTranslation),
            Control::Close => Some(Action::Close),
            Control::ConfirmExperiments | Control::DeclineExperiments => None,
            Control::Tab(Tab::Experimental) if !model.experiments_accepted => {
                self.experiments_prompt = true;
                self.focus = None;
                None
            }
            Control::Tab(t) => {
                self.deepl_key.clear();
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
                    self.notice = self
                        .language
                        .text("Введите имя метки.", "Enter a waypoint name.")
                        .into();
                    return None;
                }
                if matches!(control, Control::Add | Control::Color) && color.is_none() {
                    self.notice = self
                        .language
                        .text(
                            "Цвет: #RRGGBB, шесть цифр.",
                            "Color: #RRGGBB, six hex digits.",
                        )
                        .into();
                    return None;
                }
                if control != Control::Add && id.is_none() {
                    self.notice = self
                        .language
                        .text("Сначала выберите метку.", "Select a waypoint first.")
                        .into();
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
            Control::PlayerRequest(kind) => {
                let Some(row) = self.selected.and_then(|i| model.party.get(i)) else {
                    self.notice = self
                        .language
                        .text("Сначала выберите игрока", "Select a player first")
                        .into();
                    return None;
                };
                if !row.server_online {
                    self.notice = self
                        .language
                        .text(
                            "Игрок не в сети на этом сервере",
                            "Player is offline on this server",
                        )
                        .into();
                    return None;
                }
                if let Err(error) = kind.command(&row.name, self.language) {
                    self.notice = error;
                    return None;
                }
                Some(Action::PlayerRequest {
                    kind,
                    uuid: row.uuid,
                    name: row.name.clone(),
                })
            }
            Control::Invite | Control::Accept => {
                let Some(row) = self.selected.and_then(|i| model.party.get(i)) else {
                    self.notice = self
                        .language
                        .text("Выбери игрока из списка", "Select a player from the list")
                        .into();
                    return None;
                };
                if row.blocked || row.code.is_empty() || !row.online {
                    self.notice = self
                        .language
                        .text("Игрок сейчас недоступен", "Player is currently unavailable")
                        .into();
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
                    self.notice = self
                        .language
                        .text("Выберите друга в списке.", "Select a friend from the list.")
                        .into();
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
                self.settings_section = 0;
                self.relay_setup = false;
                self.access_token.clear();
                self.focus = None;
                None
            }
            Control::RelaySave => {
                let url = self.relay_url.trim().to_owned();
                if url.is_empty() {
                    self.notice = self
                        .language
                        .text(
                            "Введите адрес ws:// или wss://.",
                            "Enter a ws:// or wss:// address.",
                        )
                        .into();
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
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        if self.navigation_choices.is_some() {
            self.activate(
                model,
                height,
                Control::NavigationChoicesPage(if delta > 0.0 { -1 } else { 1 }),
            );
            return;
        }
        if self.schematic_choices {
            self.activate(
                model,
                height,
                Control::SchematicChoicesPage(if delta > 0.0 { -1 } else { 1 }),
            );
            return;
        }
        if !self.relay_setup {
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
            Field::SchematicOrigin => &mut self.schematic_origin,
            Field::NavigationX => {
                self.navigation_waypoint = None;
                self.navigation_player = None;
                &mut self.navigation_x
            }
            Field::NavigationY => {
                self.navigation_waypoint = None;
                self.navigation_player = None;
                &mut self.navigation_y
            }
            Field::NavigationZ => {
                self.navigation_waypoint = None;
                self.navigation_player = None;
                &mut self.navigation_z
            }
            Field::ClaimOwner => &mut self.claim_owner,
            Field::PriorityName => &mut self.priority_name,
            Field::PriorityColor => &mut self.priority_color,
            Field::Name => &mut self.name,
            Field::Color => &mut self.color,
            Field::PeerCode => &mut self.peer_code,
            Field::RelayUrl => &mut self.relay_url,
            Field::RelayToken => &mut self.access_token,
            Field::DeepLKey => &mut self.deepl_key,
            Field::OwnClaimColor => &mut self.own_claim_color,
            Field::FriendClaimColor => &mut self.friend_claim_color,
        })
    }
    pub fn text(&self) -> &str {
        match self.focus {
            Some(Field::SchematicOrigin) => &self.schematic_origin,
            Some(Field::NavigationX) => &self.navigation_x,
            Some(Field::NavigationY) => &self.navigation_y,
            Some(Field::NavigationZ) => &self.navigation_z,
            Some(Field::ClaimOwner) => &self.claim_owner,
            Some(Field::PriorityName) => &self.priority_name,
            Some(Field::PriorityColor) => &self.priority_color,
            Some(Field::Name) => &self.name,
            Some(Field::Color) => &self.color,
            Some(Field::PeerCode) => &self.peer_code,
            Some(Field::RelayUrl) => &self.relay_url,
            Some(Field::RelayToken) => &self.access_token,
            Some(Field::DeepLKey) => &self.deepl_key,
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
        if self.focus == Some(Field::DeepLKey) {
            return String::new();
        }
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
        if matches!(
            self.focus,
            Some(Field::NavigationX | Field::NavigationY | Field::NavigationZ)
        ) {
            let parts: Vec<&str> = text
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|part| !part.is_empty())
                .collect();
            if let [x, y, z] = parts.as_slice() {
                let clean = |value: &str| {
                    value
                        .chars()
                        .filter(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'))
                        .take(12)
                        .collect::<String>()
                };
                self.navigation_waypoint = None;
                self.navigation_player = None;
                self.navigation_x = clean(x);
                self.navigation_y = clean(y);
                self.navigation_z = clean(z);
                self.caret = usize::MAX;
                self.anchor = None;
                return;
            }
        }
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
            Field::SchematicOrigin => 80,
            Field::NavigationX | Field::NavigationY | Field::NavigationZ => 12,
            Field::Name => 48,
            Field::Color | Field::OwnClaimColor | Field::FriendClaimColor => 7,
            Field::PriorityName | Field::ClaimOwner => 16,
            Field::PriorityColor => 7,
            Field::PeerCode => 24,
            Field::RelayUrl => 2048,
            Field::RelayToken => 1024,
            Field::DeepLKey => DEEPL_KEY_LIMIT,
        };
        if (matches!(
            field,
            Field::Color | Field::OwnClaimColor | Field::FriendClaimColor
        ) && !(c.is_ascii_hexdigit() || c == '#'))
            || (field == Field::PriorityName && !(c.is_ascii_alphanumeric() || c == '_'))
            || (field == Field::PriorityColor && !(c.is_ascii_hexdigit() || c == '#'))
            || (field == Field::PeerCode && !c.is_ascii_hexdigit())
            || (field == Field::DeepLKey && !c.is_ascii_graphic())
            || (field == Field::SchematicOrigin
                && !(c.is_ascii_digit() || matches!(c, ' ' | '-' | '+' | '.')))
            || (matches!(
                field,
                Field::NavigationX | Field::NavigationY | Field::NavigationZ
            ) && !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.')))
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
        if self.experiments_prompt {
            if key == "key.keyboard.escape" {
                self.experiments_prompt = false;
            }
            return true;
        }
        if key == "key.keyboard.escape" {
            if self.navigation_choices.take().is_some() {
                return true;
            }
            if self.schematic_choices {
                self.schematic_choices = false;
                return true;
            }
            self.access_token.clear();
            self.deepl_key.clear();
            if self.relay_setup {
                self.relay_setup = false;
                self.focus = None;
                return true;
            }
            return false;
        }
        if key == "key.keyboard.tab" {
            if self.tab == Tab::Experimental
                && self.experimental_section == ExperimentalSection::Baritone
                && !matches!(
                    self.baritone_mode,
                    BaritoneMode::Navigation | BaritoneMode::Schematic
                )
            {
                self.focus = None;
                return true;
            }
            if self.tab == Tab::Settings
                && self.settings_section == 3
                && self.chat_section == ChatSection::DeepL
            {
                self.focus = Some(Field::DeepLKey);
                self.caret = usize::MAX;
                self.anchor = None;
                return true;
            }
            self.focus = Some(
                if self.tab == Tab::Experimental
                    && self.experimental_section == ExperimentalSection::Baritone
                {
                    if self.baritone_mode == BaritoneMode::Schematic {
                        Field::SchematicOrigin
                    } else {
                        match self.focus {
                            Some(Field::NavigationX) => Field::NavigationY,
                            Some(Field::NavigationY) => Field::NavigationZ,
                            _ => Field::NavigationX,
                        }
                    }
                } else if self.relay_setup
                    || (self.tab == Tab::Settings && self.settings_section == 1)
                {
                    if self.focus == Some(Field::RelayUrl) {
                        Field::RelayToken
                    } else {
                        Field::RelayUrl
                    }
                } else if self.tab == Tab::Settings
                    && self.settings_section == 2
                    && self.claims_section == ClaimsSection::Colours
                {
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
                },
            );
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
    fn outgoing_label(&self, enabled: bool) -> String {
        localized!(
            self.language,
            "Перевод исходящих: {}",
            "Outgoing translation: {}",
            self.on(enabled)
        )
    }
    fn on(&self, enabled: bool) -> &'static str {
        match enabled {
            true => self.language.text("вкл.", "on"),
            false => self.language.text("выкл.", "off"),
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn deepl_settings_mask_the_key_and_offer_save_and_remove() {
        let mut menu = Menu {
            tab: Tab::Settings,
            settings_section: 3,
            ..Menu::default()
        };
        let model = ViewModel::default();
        let size = [240.0, 216.0];
        let click = |menu: &mut Menu, control| {
            let [x, y, w, h] = menu
                .layout(&model, size[0], size[1])
                .into_iter()
                .find(|widget| widget.control == Some(control))
                .unwrap()
                .rect;
            menu.click(&model, size, [x + w / 2.0, y + h / 2.0])
        };
        let key = "test-key:fx";
        click(&mut menu, Control::ChatSection(ChatSection::DeepL));
        click(&mut menu, Control::Field(Field::DeepLKey));
        menu.paste(key);
        menu.key("key.keyboard.a", 2);
        assert!(menu.selected_text().is_empty());
        assert!(
            menu.layout(&model, size[0], size[1])
                .iter()
                .all(|widget| !widget.text.contains(key))
        );
        let action = click(&mut menu, Control::SaveDeepLKey).unwrap();
        assert!(!format!("{action:?}").contains(key));
        assert!(matches!(action, Action::SetDeepLKey(Some(saved)) if saved == key));
        assert!(menu.deepl_key.is_empty());
        assert!(matches!(
            click(&mut menu, Control::RemoveDeepLKey),
            Some(Action::SetDeepLKey(None))
        ));
    }
    #[test]
    fn language_button_sits_next_to_settings_and_reverses_translation() {
        let mut model = ViewModel {
            chat_translate: true,
            chat_outgoing: true,
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Chat,
            ..Menu::default()
        };
        for size in [[240.0, 216.0], [800.0, 500.0]] {
            let widgets = menu.layout(&model, size[0], size[1]);
            let [x, y, w, h] = widgets
                .iter()
                .find(|w| w.control == Some(Control::Language))
                .unwrap()
                .rect;
            let settings = widgets
                .iter()
                .find(|w| w.control == Some(Control::Tab(Tab::Settings)))
                .unwrap();
            assert_eq!(settings.rect[1], y);
            assert!(settings.rect[0] + settings.rect[2] <= x);
            assert!(matches!(
                menu.click(&model, size, [x + w / 2.0, y + h / 2.0]),
                Some(Action::SetLanguage(Language::English))
            ));
        }
        let previous = model.clone();
        model.language = Language::English;
        menu.reconcile(&previous, &model, 400.0);
        let widgets = menu.layout(&model, 800.0, 500.0);
        assert!(widgets.iter().any(|w| w.text == "Incoming translation: on"));
        assert!(widgets.iter().any(|w| w.text == "Outgoing translation: on"));
        assert!(widgets.iter().any(|w| w.text == "Settings"));
    }
    #[test]
    fn unified_settings_keep_work_tabs_clean_and_separate_tab_from_trades() {
        let model = ViewModel::default();
        for tab in [Tab::Party, Tab::Chat, Tab::Claims] {
            let menu = Menu {
                tab,
                ..Menu::default()
            };
            let widgets = menu.layout(&model, 800.0, 500.0);
            assert!(!widgets.iter().any(|w| matches!(
                w.control,
                Some(Control::RelaySetup | Control::ToggleAutoTrust | Control::RelayOff)
            )));
            assert!(!widgets.iter().any(|w| w.text == "Приглашения"));
        }
        for section in 0..7 {
            let menu = Menu {
                tab: Tab::Settings,
                settings_section: section,
                ..Menu::default()
            };
            for width in [240.0, 800.0] {
                let widgets = menu.layout(&model, width, 216.0);
                let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
                for (index, a) in controls.iter().enumerate() {
                    for b in &controls[index + 1..] {
                        let overlap = a.rect[0] < b.rect[0] + b.rect[2]
                            && b.rect[0] < a.rect[0] + a.rect[2]
                            && a.rect[1] < b.rect[1] + b.rect[3]
                            && b.rect[1] < a.rect[1] + a.rect[3];
                        assert!(
                            !overlap,
                            "section {section}: {:?} overlaps {:?}",
                            a.control, b.control
                        );
                    }
                }
                for widget in widgets {
                    if let Some(Control::EconomySetting(key, _)) = widget.control {
                        let group = super::super::claim_borders_economy::SETTINGS
                            .iter()
                            .find(|s| s.key == key)
                            .unwrap()
                            .group;
                        assert!(match section {
                            4 => group <= 4,
                            5 => group == 6,
                            6 => group == 5,
                            _ => false,
                        });
                    }
                }
            }
        }
    }
    #[test]
    fn invitations_are_actionable_in_mod_users_and_claim_filters_combine() {
        let mut model = ViewModel {
            party: vec![PartyRow {
                uuid: 42,
                name: "Alex".into(),
                code: "abcdef".into(),
                online: true,
                pending: true,
                ..PartyRow::default()
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        assert_eq!(menu.party_indices(&model), vec![0]);
        assert_eq!(
            menu.activate(&model, 240.0, Control::PartyAccept(0)),
            Some(Action::Accept {
                code: "abcdef".into()
            })
        );
        assert_eq!(
            menu.activate(&model, 240.0, Control::PartyDecline(0)),
            Some(Action::Decline { uuid: 42 })
        );
        model.claim_rows = vec![
            ClaimRow {
                owner: "Alex".into(),
                foreign: false,
                dimension: "minecraft:overworld".into(),
                x: 0,
                z: 0,
                label: "Alex".into(),
            },
            ClaimRow {
                owner: "Other".into(),
                foreign: true,
                dimension: "minecraft:overworld".into(),
                x: 16,
                z: 0,
                label: "Other".into(),
            },
        ];
        menu.claim_owner = "alex".into();
        menu.claim_filter = 1;
        assert_eq!(menu.claim_indices(&model), vec![0]);
        menu.claim_filter = 2;
        assert!(menu.claim_indices(&model).is_empty());
    }
    #[test]
    fn quick_requests_use_server_presence_and_require_an_explicit_button() {
        let mut model = ViewModel {
            party: vec![PartyRow {
                uuid: 42,
                name: "Alex".into(),
                friend: true,
                server_online: true,
                online: false,
                ..PartyRow::default()
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Party,
            party_filter: PartyFilter::Friends,
            ..Menu::default()
        };
        let tp = Control::PlayerRequest(PlayerRequest::Teleport);
        let trade = Control::PlayerRequest(PlayerRequest::Trade);
        assert_eq!(menu.activate(&model, 400.0, tp), None);
        assert_eq!(menu.activate(&model, 400.0, Control::Row(0)), None);
        assert_eq!(
            menu.activate(&model, 400.0, tp),
            Some(Action::PlayerRequest {
                kind: PlayerRequest::Teleport,
                uuid: 42,
                name: "Alex".into()
            })
        );
        // A local chat priority can use server commands without relay trust.
        model.party[0].friend = false;
        model.party[0].local_only = true;
        model.party[0].blocked = true;
        assert!(matches!(
            menu.activate(&model, 400.0, trade),
            Some(Action::PlayerRequest {
                kind: PlayerRequest::Trade,
                ..
            })
        ));
        model.party[0].server_online = false;
        assert_eq!(menu.activate(&model, 400.0, tp), None);
        assert_eq!(menu.activate(&model, 400.0, trade), None);
    }
    #[test]
    fn quick_requests_validate_current_identity_and_never_run_trade_subcommands() {
        let online = vec![(42, "Alex".into())];
        assert_eq!(
            PlayerRequest::Teleport
                .resolve(42, "alex", 1, &online, Language::Russian)
                .unwrap(),
            "/tpa Alex"
        );
        assert_eq!(
            PlayerRequest::Trade
                .resolve(42, "Alex", 1, &online, Language::Russian)
                .unwrap(),
            "/trade Alex"
        );
        assert!(
            PlayerRequest::Trade
                .resolve(99, "Alex", 1, &online, Language::Russian)
                .is_err()
        );
        assert!(
            PlayerRequest::Trade
                .resolve(42, "OldName", 1, &online, Language::Russian)
                .is_err()
        );
        assert!(
            PlayerRequest::Teleport
                .resolve(42, "Alex", 42, &online, Language::Russian)
                .is_err()
        );
        assert!(
            PlayerRequest::Teleport
                .resolve(42, "Alex", 1, &[], Language::Russian)
                .is_err()
        );
        for name in [
            "",
            "Alex Other",
            "@a",
            "Alex\n/op",
            "Привет",
            "12345678901234567",
        ] {
            assert!(
                PlayerRequest::Teleport
                    .command(name, Language::Russian)
                    .is_err()
            );
            assert!(
                PlayerRequest::Trade
                    .command(name, Language::Russian)
                    .is_err()
            );
        }
        for name in ["accept", "deny", "cancel", "money"] {
            assert!(
                PlayerRequest::Trade
                    .command(name, Language::Russian)
                    .is_err()
            );
        }
    }
    #[test]
    fn quick_request_buttons_fit_with_existing_party_controls() {
        let model = ViewModel::default();
        let menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        for width in [240.0, 400.0, 800.0] {
            for height in [216.0, 300.0, 500.0] {
                let rows = menu.layout(&model, width, height);
                let mut footer: Vec<_> = rows
                    .iter()
                    .filter(|w| {
                        matches!(
                            w.control,
                            Some(
                                Control::PlayerRequest(_)
                                    | Control::Invite
                                    | Control::ToggleAutoTrust
                            )
                        )
                    })
                    .collect();
                assert_eq!(footer.len(), 2);
                footer.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
                for pair in footer.windows(2) {
                    assert!(pair[0].rect[0] + pair[0].rect[2] <= pair[1].rect[0]);
                }
            }
        }
    }
    #[test]
    fn important_people_are_visible_in_friends_without_granting_friend_rights() {
        let mut model = ViewModel {
            party: vec![
                PartyRow {
                    name: "Buddy".into(),
                    friend: true,
                    ..PartyRow::default()
                },
                PartyRow {
                    name: "Alex".into(),
                    online: true,
                    ..PartyRow::default()
                },
                PartyRow {
                    name: "Other".into(),
                    online: true,
                    ..PartyRow::default()
                },
            ],
            ..ViewModel::default()
        };
        model.priority.set("ALEX", 0xffaa00, true).unwrap();
        model.priority.set("Offline", 0xffaa00, true).unwrap();
        let menu = Menu {
            tab: Tab::Party,
            party_filter: PartyFilter::Friends,
            ..Menu::default()
        };
        assert_eq!(menu.party_indices(&model), vec![0, 1]);
        assert!(!model.party[1].friend);
        let rows = menu.layout(&model, 800.0, 500.0);
        assert!(
            rows.iter()
                .any(|r| r.text.contains("Alex") && r.text.contains("Важный"))
        );
        assert!(menu.important_names(&model).iter().any(|n| n == "offline"));
        assert!(!model.experimental.allowed);
    }
    #[test]
    fn friends_tab_can_add_and_remove_an_offline_priority_by_nickname() {
        let mut model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Party,
            ..Menu::default()
        };
        let size = [240.0, 216.0];
        let click = |menu: &mut Menu, model: &ViewModel, control| {
            let [x, y, w, h] = menu
                .layout(model, size[0], size[1])
                .into_iter()
                .find(|widget| widget.control == Some(control))
                .unwrap()
                .rect;
            menu.click(model, size, [x + w / 2.0, y + h / 2.0])
        };
        click(
            &mut menu,
            &model,
            Control::PartyFilter(PartyFilter::Important),
        );
        click(&mut menu, &model, Control::Field(Field::PriorityName));
        menu.paste("Offline_Player");
        let Some(Action::SetPriority(prefs)) = click(&mut menu, &model, Control::PrioritySave)
        else {
            panic!("saving an offline priority should update settings");
        };
        assert!(prefs.rules.contains_key("offline_player"));
        model.priority = prefs;
        menu.priority_name.clear();
        click(&mut menu, &model, Control::PriorityRow(0));
        assert_eq!(menu.priority_name, "offline_player");
        let Some(Action::SetPriority(prefs)) = click(&mut menu, &model, Control::PriorityRemove)
        else {
            panic!("removing an offline priority should update settings");
        };
        assert!(prefs.rules.is_empty());
    }
    #[test]
    fn manual_priorities_show_without_relay_and_never_offer_friendship_actions() {
        let mut model = ViewModel::default();
        model.priority.set("Offline", 0xabcdef, true).unwrap();
        model.priority.set("Alex", 0xabcdef, true).unwrap();
        append_important_rows(
            &mut model.party,
            &model.priority,
            &[(42, "Alex".into())],
            Language::Russian,
        );
        assert_eq!(model.party.len(), 2);
        let mut menu = Menu {
            tab: Tab::Party,
            party_filter: PartyFilter::Friends,
            selected: Some(0),
            ..Menu::default()
        };
        assert_eq!(menu.party_indices(&model).len(), 2);
        assert!(model.party.iter().all(|row| row.local_only && !row.friend));
        assert_eq!(menu.activate(&model, 400.0, Control::Invite), None);
        menu.party_filter = PartyFilter::Users;
        assert!(menu.party_indices(&model).is_empty());
        menu.party_filter = PartyFilter::Friends;
        let mut next = model.clone();
        next.party.reverse();
        menu.reconcile(&model, &next, 400.0);
        assert_eq!(menu.selected, Some(1));
    }

    #[test]
    fn experimental_permission_does_not_activate_from_opening_menu() {
        let model = ViewModel::default();
        let menu = Menu {
            tab: Tab::Experimental,
            ..Menu::default()
        };
        let rows = menu.layout(&model, 800.0, 500.0);
        assert!(
            rows.iter()
                .any(|r| r.text.contains("Разрешить подсветку: выкл."))
        );
        assert!(!model.experimental.allowed && !model.experimental.prediction);
    }

    #[test]
    fn experimental_controls_fit_compact_panels() {
        let model = ViewModel::default();
        let menu = Menu {
            tab: Tab::Experimental,
            ..Menu::default()
        };
        for height in [200.0, 216.0, 300.0, 500.0] {
            let rows = menu.layout(&model, 360.0, height);
            let controls: Vec<_> = rows
                .iter()
                .filter(|r| {
                    matches!(
                        r.control,
                        Some(
                            Control::ExperimentalAllowed
                                | Control::ExperimentalSource
                                | Control::ExperimentalNext
                                | Control::ExperimentalOre
                        )
                    )
                })
                .collect();
            assert_eq!(controls.len(), 4);
            for pair in controls.windows(2) {
                assert!(pair[0].rect[1] + pair[0].rect[3] <= pair[1].rect[1]);
            }
            let hint = rows.iter().find(|r| r.text.starts_with("O —")).unwrap();
            let last = controls.last().unwrap();
            assert!(last.rect[1] + last.rect[3] <= hint.rect[1]);
        }
    }

    #[test]
    fn experimental_tabs_preserve_ore_preferences_without_starting_automation() {
        let model = ViewModel::default();
        for language in [Language::Russian, Language::English] {
            let mut menu = Menu {
                tab: Tab::Experimental,
                language,
                ..Menu::default()
            };
            for section in [ExperimentalSection::Baritone, ExperimentalSection::Ores] {
                for height in [200.0, 216.0, 500.0] {
                    let rows = menu.layout(&model, 360.0, height);
                    let tab = rows
                        .iter()
                        .find(|row| row.control == Some(Control::ExperimentalTab(section)))
                        .unwrap();
                    let mouse = [
                        tab.rect[0] + tab.rect[2] * 0.5,
                        tab.rect[1] + tab.rect[3] * 0.5,
                    ];
                    assert!(menu.click(&model, [360.0, height], mouse).is_none());
                    assert_eq!(menu.experimental_section, section);
                    let rows = menu.layout(&model, 360.0, height);
                    assert_eq!(
                        rows.iter()
                            .filter(|row| row.control == Some(Control::ExperimentalAllowed))
                            .count(),
                        usize::from(section == ExperimentalSection::Ores)
                    );
                    assert!(
                        rows.iter()
                            .any(|row| row.control == Some(Control::ExperimentalTab(section))
                                && row.selected)
                    );
                    assert!(!model.experimental.allowed && !model.experimental.prediction);
                }
            }
        }
    }

    #[test]
    fn navigation_validates_coordinates_from_three_fields() {
        use super::super::claim_borders_navigation::Goal;
        let mut menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            navigation_x: "-0.2".into(),
            navigation_y: "64".into(),
            navigation_z: "120".into(),
            ..Menu::default()
        };
        assert!(matches!(
            menu.baritone_request(false),
            Ok(Action::Navigate {
                goal: Goal::Coordinates([-1, 64, 120]),
                running: false
            })
        ));
        for (x, y, z) in [
            ("", "64", "1"),
            ("1", "", "1"),
            ("1", "64", ""),
            ("NaN", "64", "1"),
            ("1", "64", "inf"),
            ("30000001", "64", "0"),
            ("1 2", "64", "0"),
        ] {
            menu.navigation_x = x.into();
            menu.navigation_y = y.into();
            menu.navigation_z = z.into();
            assert!(menu.baritone_request(true).is_err(), "{x} {y} {z}");
        }
    }

    #[test]
    fn pasted_coordinates_fill_all_three_fields() {
        let mut menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            ..Menu::default()
        };
        menu.focus = Some(Field::NavigationX);
        menu.paste("120 64 -350");
        assert_eq!(menu.navigation_x, "120");
        assert_eq!(menu.navigation_y, "64");
        assert_eq!(menu.navigation_z, "-350");
    }

    #[test]
    fn navigation_waypoint_selection_and_manual_edit_are_separate_targets() {
        let model = ViewModel {
            experiments_accepted: true,
            navigation_waypoints: vec![WaypointRow {
                id: 41,
                name: "Home".into(),
                detail: String::new(),
                color: 0,
            }],
            ..ViewModel::default()
        };
        let mut menu = Menu::default();
        assert!(
            menu.activate(&model, 200.0, Control::NavigationWaypoint)
                .is_none()
        );
        assert_eq!(menu.navigation_waypoint, None);
        menu.activate(&model, 200.0, Control::NavigationWaypointChoice(0));
        assert!(matches!(
            menu.baritone_request(true),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Waypoint(41),
                running: true
            })
        ));
        menu.focus = Some(Field::NavigationX);
        menu.character('1');
        assert_eq!(menu.navigation_waypoint, None);
        assert_eq!(menu.navigation_x, "1");
    }

    #[test]
    fn navigation_controls_fit_both_languages_in_compact_panels() {
        for language in [Language::Russian, Language::English] {
            let menu = Menu {
                tab: Tab::Experimental,
                experimental_section: ExperimentalSection::Baritone,
                language,
                ..Menu::default()
            };
            let model = ViewModel {
                navigation_status: "Navigation status".into(),
                ..ViewModel::default()
            };
            for height in [200.0, 216.0, 300.0, 500.0] {
                let rows = menu.layout(&model, 360.0, height);
                let controls: Vec<_> = rows
                    .iter()
                    .filter(|w| {
                        matches!(
                            w.control,
                            Some(
                                Control::Field(
                                    Field::NavigationX | Field::NavigationY | Field::NavigationZ
                                ) | Control::NavigationWaypoint
                                    | Control::NavigationPlayer
                            )
                        )
                    })
                    .collect();
                assert_eq!(controls.len(), 5);
                let status = rows.iter().find(|w| w.text == "Navigation status").unwrap();
                for (i, a) in controls.iter().enumerate() {
                    assert!(a.rect[1] + a.rect[3] <= status.rect[1]);
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
        }
    }

    #[test]
    fn mining_controls_are_explicit_and_fit_compact_panels() {
        for language in [Language::Russian, Language::English] {
            let menu = Menu {
                tab: Tab::Experimental,
                experimental_section: ExperimentalSection::Baritone,
                baritone_mode: BaritoneMode::Mining,
                language,
                ..Menu::default()
            };
            let mut model = ViewModel {
                navigation_status: "Mining status".into(),
                experiments_accepted: true,
                ..ViewModel::default()
            };
            assert!(matches!(
                menu.baritone_request(false),
                Ok(Action::Navigate {
                    goal: super::super::claim_borders_navigation::Goal::Mine {
                        ore: 0,
                        targets: 1,
                        limit: 16
                    },
                    running: false
                })
            ));
            for height in [200.0, 216.0, 300.0, 500.0] {
                let rows = menu.layout(&model, 360.0, height);
                let controls: Vec<_> = rows
                    .iter()
                    .filter(|r| {
                        matches!(
                            r.control,
                            Some(
                                Control::MiningTarget(_)
                                    | Control::MiningPage
                                    | Control::MiningLimit
                                    | Control::BaritoneMode(_)
                            )
                        )
                    })
                    .collect();
                assert!(controls.len() >= 8);
                let status = rows.iter().find(|w| w.text == "Mining status").unwrap();
                for (i, a) in controls.iter().enumerate() {
                    assert!(a.rect[1] + a.rect[3] <= status.rect[1]);
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
            model.experiments_accepted = false;
            assert!(!model.experiments_accepted);
        }
    }

    #[test]
    fn construction_modes_validate_consent_and_compact_geometry() {
        for language in [Language::Russian, Language::English] {
            for mode in [BaritoneMode::Tunnel, BaritoneMode::Bridge] {
                let menu = Menu {
                    tab: Tab::Experimental,
                    experimental_section: ExperimentalSection::Baritone,
                    baritone_mode: mode,
                    language,
                    ..Menu::default()
                };
                let mut model = ViewModel {
                    navigation_status: "Build status".into(),
                    experiments_accepted: true,
                    ..ViewModel::default()
                };
                assert!(
                    menu.construct_floor,
                    "tunnel floor must be enabled by default"
                );
                assert!(matches!(
                    menu.baritone_request(false),
                    Ok(Action::Navigate {
                        goal: super::super::claim_borders_navigation::Goal::Construct(_),
                        running: false
                    })
                ));
                for height in [200., 216., 300., 500.] {
                    let rows = menu.layout(&model, 360., height);
                    let controls: Vec<_> = rows
                        .iter()
                        .filter(|w| {
                            matches!(
                                w.control,
                                Some(
                                    Control::ConstructDirection
                                        | Control::ConstructLength
                                        | Control::ConstructWidth
                                        | Control::ConstructHeight
                                        | Control::ConstructFloor
                                        | Control::BaritoneMode(_)
                                )
                            )
                        })
                        .collect();
                    assert!(controls.len() >= if mode == BaritoneMode::Tunnel { 11 } else { 7 });
                    if mode == BaritoneMode::Tunnel && height < 320.0 {
                        let control_rect = |control| {
                            controls
                                .iter()
                                .find(|w| w.control == Some(control))
                                .unwrap()
                                .rect
                        };
                        let length = control_rect(Control::ConstructLength);
                        let width = control_rect(Control::ConstructWidth);
                        let tunnel_height = control_rect(Control::ConstructHeight);
                        let floor = control_rect(Control::ConstructFloor);
                        assert_eq!(length[1], width[1]);
                        assert_eq!(tunnel_height[1], floor[1]);
                        assert!(length[0] < width[0]);
                        assert!(tunnel_height[0] < floor[0]);
                        assert!(length[1] < tunnel_height[1]);
                    }
                    let status = rows.iter().find(|w| w.text == "Build status").unwrap();
                    for (i, a) in controls.iter().enumerate() {
                        assert!(a.rect[1] + a.rect[3] <= status.rect[1]);
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
                model.experiments_accepted = false;
                assert!(!model.experiments_accepted);
            }
        }
    }

    #[test]
    fn navigation_follow_selection_is_by_uuid_and_coordinate_edits_clear_it() {
        let model = ViewModel {
            experiments_accepted: true,
            navigation_players: vec![(71, "Alice".into()), (82, "Bob".into())],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            navigation_waypoint: Some(7),
            ..Menu::default()
        };
        menu.activate(&model, 200.0, Control::NavigationPlayer);
        assert_eq!(menu.navigation_choices, Some(true));
        menu.activate(&model, 200.0, Control::NavigationPlayerChoice(0));
        assert_eq!(menu.navigation_waypoint, None);
        assert_eq!(menu.navigation_player, Some(71));
        assert!(matches!(
            menu.baritone_request(true),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Follow(71),
                running: true
            })
        ));
        menu.activate(&model, 200.0, Control::NavigationPlayer);
        menu.activate(&model, 200.0, Control::NavigationPlayerChoice(1));
        assert_eq!(menu.navigation_player, Some(82));
        menu.focus = Some(Field::NavigationY);
        menu.character('2');
        assert_eq!(menu.navigation_player, None);
        assert_eq!(menu.navigation_y, "2");
    }

    #[test]
    fn navigation_snapshot_reconciles_removed_targets_and_shrunk_pages() {
        let old = ViewModel {
            navigation_players: (0..18).map(|i| (100 + i, format!("Player{i}"))).collect(),
            ..ViewModel::default()
        };
        let next = ViewModel {
            navigation_players: vec![(101, "Player1".into())],
            ..ViewModel::default()
        };
        let mut menu = Menu {
            navigation_player: Some(117),
            navigation_choices: Some(true),
            navigation_choices_page: 2,
            ..Menu::default()
        };
        menu.reconcile(&old, &next, 200.0);
        assert_eq!(menu.navigation_player, None);
        assert_eq!(menu.navigation_choices_page, 0);
        menu.navigation_player = Some(101);
        menu.reconcile(&next, &next, 200.0);
        assert_eq!(menu.navigation_player, Some(101));
    }

    #[test]
    fn vanished_navigation_target_never_falls_back_to_old_coordinates() {
        for language in [Language::Russian, Language::English] {
            for player in [false, true] {
                let mut menu = Menu {
                    language,
                    navigation_player: player.then_some(117),
                    navigation_waypoint: (!player).then_some(41),
                    navigation_x: "120".into(),
                    navigation_y: "64".into(),
                    navigation_z: "-350".into(),
                    ..Menu::default()
                };
                let next = ViewModel {
                    language,
                    experiments_accepted: true,
                    ..ViewModel::default()
                };
                menu.reconcile(&ViewModel::default(), &next, 200.0);
                assert!(menu.navigation_x.is_empty() && menu.navigation_z.is_empty());
                assert!(!menu.notice.is_empty());
                assert!(menu.baritone_request(true).is_err());
            }
        }
    }

    #[test]
    fn empty_navigation_dropdown_shows_non_actionable_placeholder() {
        let model = ViewModel::default();
        let menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            baritone_mode: BaritoneMode::Navigation,
            navigation_choices: Some(true),
            ..Menu::default()
        };
        let widgets = menu.layout(&model, 800.0, 480.0);
        assert!(widgets.iter().any(|w| {
            w.control == Some(Control::NavigationChoicesPage(0))
                && (w.text.contains("No available targets")
                    || w.text.contains("Нет доступных целей"))
        }));
    }

    #[test]
    fn navigation_dropdown_escape_closes_popup_before_screen() {
        let model = ViewModel::default();
        let mut menu = Menu::default();
        menu.activate(&model, 200.0, Control::NavigationWaypoint);
        assert_eq!(menu.navigation_choices, Some(false));
        assert!(menu.key("key.keyboard.escape", 0));
        assert_eq!(menu.navigation_choices, None);
        assert!(!menu.key("key.keyboard.escape", 0));
    }

    #[test]
    fn navigation_dropdown_mouse_wheel_pages_without_changing_main_menu() {
        let model = ViewModel {
            navigation_waypoints: (0..18)
                .map(|id| WaypointRow {
                    id,
                    name: format!("Waypoint{id}"),
                    detail: String::new(),
                    color: 0,
                })
                .collect(),
            ..ViewModel::default()
        };
        let mut menu = Menu {
            tab: Tab::Experimental,
            baritone_mode: BaritoneMode::Navigation,
            ..Menu::default()
        };
        menu.activate(&model, 200.0, Control::NavigationWaypoint);
        menu.scroll(&model, 200.0, -1.0);
        assert_eq!(menu.navigation_choices_page, 1);
        menu.scroll(&model, 200.0, 1.0);
        assert_eq!(menu.navigation_choices_page, 0);
        menu.scroll(&model, 200.0, f64::NAN);
        assert_eq!(menu.navigation_choices_page, 0);
        menu.activate(&model, 200.0, Control::NavigationChoicesPage(-1));
        assert_eq!(menu.navigation_choices_page, 2);
        menu.activate(&model, 200.0, Control::NavigationWaypointChoice(17));
        assert_eq!(menu.navigation_waypoint, Some(17));
        assert_eq!(menu.navigation_choices, None);
    }

    #[test]
    fn navigation_dropdown_pagination_selects_absolute_player_id() {
        let model = ViewModel {
            navigation_players: (0..19).map(|i| (100 + i, format!("Player{i}"))).collect(),
            ..ViewModel::default()
        };
        let mut menu = Menu::default();
        menu.activate(&model, 200.0, Control::NavigationPlayer);
        menu.activate(&model, 200.0, Control::NavigationChoicesPage(1));
        assert_eq!(menu.navigation_choices_page, 1);
        menu.activate(&model, 200.0, Control::NavigationPlayerChoice(9));
        assert_eq!(menu.navigation_player, Some(109));
        assert_eq!(menu.navigation_choices, None);
        menu.activate(&model, 200.0, Control::NavigationPlayer);
        menu.activate(&model, 200.0, Control::NavigationChoicesPage(-1));
        assert_eq!(menu.navigation_choices_page, 2);
    }

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
        for tab in [
            Tab::Waypoints,
            Tab::Party,
            Tab::Claims,
            Tab::Chat,
            Tab::Experimental,
        ] {
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
                tab: Tab::Settings,
                settings_section: 3,
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
    fn chat_spacing_control_toggles_and_resets_in_both_languages() {
        for (language, caption) in [
            (Language::English, "Compact messages"),
            (Language::Russian, "Компактные сообщения"),
        ] {
            let mut menu = Menu {
                language,
                tab: Tab::Settings,
                settings_section: 3,
                chat_section: ChatSection::Appearance,
                ..Menu::default()
            };
            let mut model = ViewModel {
                language,
                ..ViewModel::default()
            };
            let widgets = menu.layout(&model, 320.0, 240.0);
            let button = widgets
                .iter()
                .find(|w| w.text.starts_with(caption))
                .expect("chat spacing control");
            let Some(Action::SetChat(p)) = menu.activate(&model, 224.0, button.control.unwrap())
            else {
                panic!("spacing must update chat preferences");
            };
            assert!(p.compact_messages);
            assert_eq!(p.scale, model.chat_preferences.scale);
            model.chat_preferences = p;
            let Some(Action::SetChat(p)) = menu.activate(&model, 224.0, button.control.unwrap())
            else {
                panic!();
            };
            assert!(!p.compact_messages);
            let Some(Action::SetChat(p)) = menu.activate(&model, 224.0, Control::ChatReset) else {
                panic!();
            };
            assert!(!p.compact_messages);
        }
    }
    #[test]
    fn claims_sections_are_grouped_fit_small_gui_and_custom_colours_apply() {
        let model = ViewModel {
            claim_rows: (0..20)
                .map(|i| ClaimRow {
                    owner: "Alex".into(),
                    foreign: false,
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
    #[test]
    fn english_layout_has_no_russian_text_on_any_screen() {
        let has_cyrillic = |t: &str| t.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c));
        let party = |i: u128, friend, pending, blocked| PartyRow {
            uuid: i,
            friend,
            online: true,
            server_online: true,
            blocked,
            pending,
            name: format!("Player{i}"),
            code: format!("CODE{i}"),
            ..PartyRow::default()
        };
        let model = ViewModel {
            language: Language::English,
            waypoints: vec![WaypointRow {
                id: 1,
                name: "Home".into(),
                detail: "x 1 y 2 z 3".into(),
                color: 0xffcc55,
            }],
            party: vec![
                party(1, true, false, false),
                party(2, false, false, false),
                party(3, false, true, false),
                party(4, false, false, true),
            ],
            claim_rows: vec![ClaimRow {
                dimension: "overworld".into(),
                x: 0,
                z: 0,
                label: "Base".into(),
                owner: "Alex".into(),
                foreign: false,
            }],
            ..ViewModel::default()
        };
        let mut bad = std::collections::BTreeSet::new();
        let mut check = |menu: &Menu, what: String| {
            for size in [[800.0, 500.0], [240.0, 216.0]] {
                for w in menu.layout(&model, size[0], size[1]) {
                    if has_cyrillic(&w.text) {
                        bad.insert(format!("{what}: {:?}", w.text));
                    }
                }
            }
        };
        let tabs = [
            Tab::Waypoints,
            Tab::Party,
            Tab::Claims,
            Tab::Chat,
            Tab::Experimental,
            Tab::Settings,
        ];
        for tab in tabs {
            for relay_setup in [false, true] {
                let base = Menu {
                    language: Language::English,
                    tab,
                    relay_setup,
                    ..Menu::default()
                };
                check(&base, format!("{tab:?} relay_setup={relay_setup}"));
                for selected in [Some(0), Some(1)] {
                    let menu = Menu {
                        selected,
                        ..base.clone()
                    };
                    check(&menu, format!("{tab:?} selected={selected:?}"));
                }
            }
        }
        for filter in [
            PartyFilter::Friends,
            PartyFilter::Users,
            PartyFilter::Invitations,
            PartyFilter::Important,
        ] {
            let menu = Menu {
                language: Language::English,
                tab: Tab::Party,
                party_filter: filter,
                ..Menu::default()
            };
            check(&menu, format!("party {filter:?}"));
        }
        for claim_filter in 0..3 {
            for section in [
                ClaimsSection::Map,
                ClaimsSection::Borders,
                ClaimsSection::Colours,
                ClaimsSection::Storage,
                ClaimsSection::List,
            ] {
                let menu = Menu {
                    language: Language::English,
                    tab: Tab::Claims,
                    claims_section: section,
                    claim_filter,
                    ..Menu::default()
                };
                check(&menu, format!("claims {section:?} filter {claim_filter}"));
            }
        }
        for section in [
            ChatSection::Appearance,
            ChatSection::Notifications,
            ChatSection::Spam,
            ChatSection::Priority,
        ] {
            let menu = Menu {
                language: Language::English,
                tab: Tab::Settings,
                settings_section: 3,
                chat_section: section,
                ..Menu::default()
            };
            check(&menu, format!("chat {section:?}"));
        }
        for section in 0..7u8 {
            for page in 0..6 {
                let menu = Menu {
                    language: Language::English,
                    tab: Tab::Settings,
                    settings_section: section,
                    settings_page: page,
                    ..Menu::default()
                };
                check(&menu, format!("settings {section}/{page}"));
            }
        }
        for field in [
            Field::Name,
            Field::Color,
            Field::PeerCode,
            Field::RelayUrl,
            Field::RelayToken,
            Field::OwnClaimColor,
            Field::FriendClaimColor,
            Field::PriorityName,
            Field::PriorityColor,
            Field::ClaimOwner,
        ] {
            let menu = Menu {
                language: Language::English,
                focus: Some(field),
                ..Menu::default()
            };
            check(&menu, format!("focus {field:?}"));
        }
        assert!(
            bad.is_empty(),
            "Russian text in English layout:\n{}",
            bad.into_iter().collect::<Vec<_>>().join("\n")
        );
    }
    #[test]
    fn experiments_require_explicit_consent_and_escape_declines() {
        let model = ViewModel::default();
        let mut menu = Menu::default();
        assert_eq!(
            menu.activate(&model, 400.0, Control::Tab(Tab::Experimental)),
            None
        );
        assert!(menu.experiments_prompt);
        assert_eq!(menu.tab, Tab::Waypoints);
        assert_eq!(menu.activate(&model, 400.0, Control::ChatTranslate), None);
        assert!(menu.key("key.keyboard.escape", 0));
        assert!(!menu.experiments_prompt);
        menu.activate(&model, 400.0, Control::Tab(Tab::Experimental));
        assert_eq!(
            menu.activate(&model, 400.0, Control::ConfirmExperiments),
            Some(Action::AcceptExperiments)
        );
        assert_eq!(menu.tab, Tab::Experimental);
        let accepted = ViewModel {
            experiments_accepted: true,
            ..ViewModel::default()
        };
        let mut reopened = Menu::default();
        reopened.activate(&accepted, 400.0, Control::Tab(Tab::Experimental));
        assert!(!reopened.experiments_prompt);
        assert_eq!(reopened.tab, Tab::Experimental);
    }
    #[test]
    fn settings_tabs_jump_to_sections_and_reset_editor_focus() {
        let model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Settings,
            ..Menu::default()
        };
        for section in 0..7 {
            menu.settings_page = 10;
            let rect = menu
                .layout(&model, 320.0, 240.0)
                .into_iter()
                .find(|w| w.control == Some(Control::SettingsTab(section)))
                .unwrap()
                .rect;
            menu.click(
                &model,
                [320.0, 240.0],
                [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0],
            );
            assert_eq!(menu.settings_section, section);
            assert_eq!(menu.settings_page, 0);
            assert!(
                menu.layout(&model, 320.0, 240.0)
                    .iter()
                    .any(|w| w.control == Some(Control::SettingsTab(section)) && w.selected)
            );
        }
    }
    #[test]
    fn presence_format_control_changes_only_the_presence_view() {
        use super::super::claim_borders_chat_preferences::PresenceStyle;
        let mut model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Settings,
            settings_section: 3,
            chat_section: ChatSection::Notifications,
            ..Menu::default()
        };
        for expected in [PresenceStyle::Text, PresenceStyle::Icons] {
            let rect = menu
                .layout(&model, 640.0, 400.0)
                .into_iter()
                .find(|w| w.control == Some(Control::ChatPresenceStyle))
                .unwrap()
                .rect;
            let action = menu.click(
                &model,
                [640.0, 400.0],
                [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0],
            );
            let Some(Action::SetChat(next)) = action else {
                panic!("presence view did not produce a preference update");
            };
            let mut expected_preferences = model.chat_preferences;
            expected_preferences.presence_style = expected;
            assert_eq!(next, expected_preferences);
            model.chat_preferences = next;
        }
    }
    #[test]
    fn mining_targets_toggle_like_filters_and_several_mine_together() {
        use super::super::claim_borders_mining as mining;
        let model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            baritone_mode: BaritoneMode::Mining,
            ..Menu::default()
        };
        menu.activate(&model, 300.0, Control::MiningTarget(0));
        assert!(menu.baritone_request(false).is_err());
        menu.activate(&model, 300.0, Control::MiningTarget(3));
        assert!(matches!(
            menu.baritone_request(false),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Mine {
                    ore: 3,
                    targets: 8,
                    ..
                },
                ..
            })
        ));
        menu.activate(&model, 300.0, Control::MiningTarget(12));
        assert!(matches!(
            menu.baritone_request(false),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Mine {
                    ore: mining::ANY,
                    ..
                },
                ..
            })
        ));
        // Every target is reachable through the pages at the smallest panel.
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..mining::COUNT {
            for w in menu.layout(&model, 360.0, 200.0) {
                if let Some(Control::MiningTarget(i)) = w.control {
                    seen.insert(i);
                }
            }
            menu.activate(&model, 200.0, Control::MiningPage);
        }
        assert_eq!(seen.len(), mining::COUNT);
    }
    #[test]
    fn schematic_button_opens_the_folder_or_a_file_list() {
        let mut model = ViewModel::default();
        let mut menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            baritone_mode: BaritoneMode::Schematic,
            ..Menu::default()
        };
        assert!(matches!(
            menu.activate(&model, 300.0, Control::SchematicFile),
            Some(Action::OpenSchematicFolder)
        ));
        assert!(!menu.schematic_choices);
        model.schematic_files = vec!["a.schem".into(), "b.litematic".into()];
        assert!(matches!(
            menu.activate(&model, 300.0, Control::SchematicFile),
            Some(Action::RefreshSchematics)
        ));
        assert!(menu.schematic_choices);
        let rows = menu.layout(&model, 360.0, 300.0);
        assert!(
            rows.iter()
                .any(|w| w.control == Some(Control::SchematicChoice(1)))
        );
        assert!(
            rows.iter()
                .any(|w| w.control == Some(Control::SchematicOpenFolder))
        );
        assert!(!menu.baritone_keys_active());
        menu.activate(&model, 300.0, Control::SchematicChoice(1));
        assert_eq!(menu.schematic_file.as_deref(), Some("b.litematic"));
        assert!(!menu.schematic_choices);
    }
    #[test]
    fn action_keys_reach_the_baritone_tab_unless_a_field_is_being_edited() {
        let mut menu = Menu::default();
        assert!(!menu.baritone_keys_active());
        menu.tab = Tab::Experimental;
        menu.experimental_section = ExperimentalSection::Baritone;
        assert!(menu.baritone_keys_active());
        menu.focus = Some(Field::NavigationX);
        assert!(!menu.baritone_keys_active());
        menu.focus = None;
        menu.experiments_prompt = true;
        assert!(!menu.baritone_keys_active());
    }
    #[test]
    fn farming_and_schematic_actions_validate_and_fit() {
        let mut model = ViewModel {
            experiments_accepted: true,
            schematic_files: vec!["demo.schem".into(), "demo.litematic".into()],
            ..Default::default()
        };
        let mut menu = Menu {
            tab: Tab::Experimental,
            experimental_section: ExperimentalSection::Baritone,
            ..Default::default()
        };
        menu.baritone_mode = BaritoneMode::Farm;
        assert!(matches!(
            menu.baritone_request(true),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Farm(_),
                running: true
            })
        ));
        menu.baritone_mode = BaritoneMode::Drain;
        assert!(matches!(
            menu.baritone_request(false),
            Ok(Action::Navigate {
                goal: super::super::claim_borders_navigation::Goal::Drain(c),
                running: false
            }) if c.radius == 6
        ));
        menu.activate(&model, 200.0, Control::DrainRadius);
        assert_eq!(menu.drain_radius, 9);
        for _ in 0..3 {
            menu.activate(&model, 200.0, Control::DrainRadius);
        }
        assert_eq!(menu.drain_radius, 6);
        menu.baritone_mode = BaritoneMode::Schematic;
        menu.activate(&model, 200.0, Control::SchematicFile);
        assert!(menu.schematic_choices);
        menu.activate(&model, 200.0, Control::SchematicChoice(0));
        assert_eq!(menu.schematic_file.as_deref(), Some("demo.schem"));
        for value in ["", "1.5 64 0", "1 64", "NaN 64 0", "30000001 64 0"] {
            menu.schematic_origin = value.into();
            assert!(menu.baritone_request(true).is_err(), "{value}");
        }
        menu.schematic_origin = "-12 64 30".into();
        menu.activate(&model, 200.0, Control::SchematicRotation);
        assert!(matches!(
            menu.baritone_request(false),
            Ok(Action::LoadSchematic {
                origin: [-12, 64, 30],
                rotation: 1,
                running: false,
                ..
            })
        ));
        for mode in [
            BaritoneMode::Farm,
            BaritoneMode::Drain,
            BaritoneMode::Schematic,
        ] {
            menu.baritone_mode = mode;
            for height in [200.0, 216.0, 300.0, 500.0] {
                let rows = menu.layout(&model, 360.0, height);
                let controls = rows
                    .iter()
                    .filter(|w| {
                        matches!(
                            w.control,
                            Some(
                                Control::FarmCrop
                                    | Control::FarmRadius
                                    | Control::FarmLimit
                                    | Control::FarmReplant
                                    | Control::DrainRadius
                                    | Control::SchematicFile
                                    | Control::SchematicRefresh
                                    | Control::SchematicRotation
                                    | Control::SchematicMaterials
                                    | Control::SchematicFolder
                                    | Control::BaritoneMode(_)
                                    | Control::Field(Field::SchematicOrigin)
                            )
                        )
                    })
                    .collect::<Vec<_>>();
                for (i, a) in controls.iter().enumerate() {
                    assert!(a.rect[1] + a.rect[3] <= height - 16.0);
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
        }
        menu.baritone_mode = BaritoneMode::Schematic;
        menu.key("key.keyboard.tab", 0);
        assert_eq!(menu.focus, Some(Field::SchematicOrigin));
        model.experiments_accepted = false;
        assert!(!model.experiments_accepted);
    }
}
