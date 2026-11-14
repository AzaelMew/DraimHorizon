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
    /// The claims list opens first; the map preferences follow it.
    #[default]
    List,
    Map,
    Borders,
    Colours,
    Storage,
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
    /// The contents of a shulker box in its tooltip (on by default).
    pub shulker_peek: bool,
    /// Eating on its own (on by default).
    pub auto_eat: bool,
    /// Explicit item ids allowed for Auto Eat; empty means nothing may be eaten.
    pub auto_eat_foods: Vec<String>,
    /// Safe foods from the live item registry: id, display name, hunger restored.
    pub auto_eat_options: Vec<(String, String, u32)>,
    /// Who may pat this player, and whether a pat shows a notification.
    pub pat_policy: super::claim_borders_patting::Policy,
    pub pat_notify: bool,
    pub protect_glow_berries: bool,
    pub show_invisible_entities: bool,
    /// The helpers of the Helpers tab, by `Helper::index` (all off by default).
    pub helpers: [bool; 6],
    /// The auto-clicker presses the right button (the left one when false).
    pub click_right: bool,
    /// The auto-clicker's pace: an index into `autoclick::RATES`.
    pub click_rate: u8,
    /// The farm's storage container, when one is set.
    pub farm_storage: [Option<[i32; 3]>; 3],
    /// The safe point the farm waits at, when one is set.
    pub farm_safe: Option<[i32; 3]>,
    /// The composter found by the running farm's field, if any.
    pub farm_composter: Option<[i32; 3]>,
    /// The blocks of the "own block" mining target.
    pub mining_custom: Vec<String>,
    /// The schematic previews placed in this world: (index, label, shown, selected).
    pub schematic_places: Vec<(usize, String, bool, bool)>,
    /// Friends' shared previews of this world: label (file, friend, place) and shown here.
    pub schematic_friend_places: Vec<(String, bool)>,
    /// This player's choice of preview layers to draw, and the chosen layer.
    pub schematic_layers: (super::claim_borders_schematic_places::LayerMode, i32),
    /// The marked farm plots of this world: index, label (name and size), farmed, shown.
    pub farm_plots: Vec<(usize, String, bool, bool)>,
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
/// What a schematic editor button does to the selected preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchematicEdit {
    /// Move it by blocks along the world axes.
    Move([i32; 3]),
    /// Turn it a quarter clockwise.
    Rotate,
    /// Bring its origin to where the player stands.
    Here,
    /// Show or hide it.
    Toggle,
    /// Forget it.
    Remove,
    /// Build it (as the Start key does).
    Build,
    /// Turn the camera to it.
    Look,
}
#[derive(Clone, PartialEq)]
pub enum Action {
    SetAutoEat(bool),
    SetAutoEatFood(String, bool),
    SetPatPolicy(super::claim_borders_patting::Policy),
    SetPatNotify(bool),
    SetProtectGlowBerries(bool),
    SetShowInvisibleEntities(bool),
    SetHelper(Helper, bool),
    /// The auto-clicker's button: true for the right one.
    SetAutoClickButton(bool),
    /// The auto-clicker's pace, an index into `autoclick::RATES`.
    SetAutoClickRate(u8),
    SetShulkerPeek(bool),
    /// Take the container under the crosshair as the farm's storage, or clear it.
    FarmStorage(super::claim_borders_farm_storage::Role),
    /// The safe point of the farm: set to where the player stands, or cleared.
    FarmSafe,
    /// The block under the crosshair goes into the "own block" mining target, or out of it.
    MiningCustom,
    /// The schematic editor acting on the selected preview.
    SchematicEdit(SchematicEdit),
    /// Places `file` as a preview at `origin` (where the player stands when none), turned.
    SchematicPreview {
        file: String,
        origin: Option<[i32; 3]>,
        rotation: u8,
    },
    /// Selects a placed schematic preview (the one Start builds and the keys move).
    SchematicPlace(usize),
    /// Shows or hides a placed schematic preview.
    SchematicPlaceShow(usize),
    /// Forgets a placed schematic preview.
    SchematicPlaceRemove(usize),
    /// Shows or hides a friend's shared preview on this screen.
    SchematicFriendShow(usize),
    /// Turns the farm on a marked plot on or off.
    FarmPlotFarm(usize),
    /// Shows or hides a marked plot.
    FarmPlotShow(usize),
    /// Forgets a marked plot.
    FarmPlotRemove(usize),
    RefreshSchematics,
    SchematicInfo(bool),
    /// Keeps this player's choice of preview layers (mode and layer).
    SchematicLayers(super::claim_borders_schematic_places::LayerMode, i32),
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
            Self::FarmStorage(role) => f.debug_tuple("FarmStorage").field(role).finish(),
            Self::FarmSafe => f.write_str("FarmSafe"),
            Self::MiningCustom => f.write_str("MiningCustom"),
            Self::SchematicPlace(i) => f.debug_tuple("SchematicPlace").field(i).finish(),
            Self::SchematicEdit(e) => f.debug_tuple("SchematicEdit").field(e).finish(),
            Self::SchematicPreview { file, .. } => {
                f.debug_tuple("SchematicPreview").field(file).finish()
            }
            Self::SchematicPlaceShow(i) => f.debug_tuple("SchematicPlaceShow").field(i).finish(),
            Self::SchematicPlaceRemove(i) => {
                f.debug_tuple("SchematicPlaceRemove").field(i).finish()
            }
            Self::SchematicFriendShow(i) => f.debug_tuple("SchematicFriendShow").field(i).finish(),
            Self::FarmPlotFarm(i) => f.debug_tuple("FarmPlotFarm").field(i).finish(),
            Self::FarmPlotShow(i) => f.debug_tuple("FarmPlotShow").field(i).finish(),
            Self::FarmPlotRemove(i) => f.debug_tuple("FarmPlotRemove").field(i).finish(),
            Self::SetAutoEat(on) => f.debug_tuple("SetAutoEat").field(on).finish(),
            Self::SetAutoEatFood(item, on) => f
                .debug_tuple("SetAutoEatFood")
                .field(item)
                .field(on)
                .finish(),
            Self::SetPatPolicy(p) => f.debug_tuple("SetPatPolicy").field(p).finish(),
            Self::SetPatNotify(on) => f.debug_tuple("SetPatNotify").field(on).finish(),
            Self::SetProtectGlowBerries(on) => {
                f.debug_tuple("SetProtectGlowBerries").field(on).finish()
            }
            Self::SetShowInvisibleEntities(on) => {
                f.debug_tuple("SetShowInvisibleEntities").field(on).finish()
            }
            Self::SetAutoClickButton(right) => {
                f.debug_tuple("SetAutoClickButton").field(right).finish()
            }
            Self::SetAutoClickRate(rate) => f.debug_tuple("SetAutoClickRate").field(rate).finish(),
            Self::SetHelper(helper, on) => {
                f.debug_tuple("SetHelper").field(helper).field(on).finish()
            }
            Self::SetShulkerPeek(on) => f.debug_tuple("SetShulkerPeek").field(on).finish(),
            Self::OpenSchematicFolder => f.write_str("OpenSchematicFolder"),
            Self::RefreshSchematics => f.write_str("RefreshSchematics"),
            Self::SchematicInfo(folder) => f.debug_tuple("SchematicInfo").field(folder).finish(),
            Self::SchematicLayers(mode, layer) => f
                .debug_tuple("SchematicLayers")
                .field(mode)
                .field(layer)
                .finish(),
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
    Trading,
    Settings,
}
impl Tab {
    pub fn label(self, language: Language) -> &'static str {
        match self {
            Self::Waypoints => language.text("Метки", "Waypoints"),
            Self::Party => language.text("Друзья", "Friends"),
            Self::Claims => language.text("Карта и приваты", "Map and claims"),
            Self::Chat => language.text("Чат", "Chat"),
            Self::Experimental => language.text("Эксперименты", "Experiments"),
            Self::Trading => language.text("Торговля", "Trading"),
            Self::Settings => language.text("Настройки", "Settings"),
        }
    }
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
/// Height reserved above the claims list for the Map and claims section selector.
const CLAIMS_SELECTOR: f32 = 24.0;
/// Width of the tab rail on the left of the J menu; the layout and the drawing share it.
pub const RAIL_WIDTH: f32 = 128.0;
/// A labelled rail must leave enough room for the editors, not just fit beside them.
pub const RAIL_BREAKPOINT: f32 = 520.0;
pub fn rail_width(panel_width: f32, panel_height: f32) -> f32 {
    if panel_width >= RAIL_BREAKPOINT && panel_height >= 244.0 {
        RAIL_WIDTH
    } else {
        0.0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ExperimentalSection {
    #[default]
    Ores,
    Baritone,
    /// Switches for the helpers that work on their own: eating, totem, refill, tools.
    Helpers,
}
/// A helper that works while the player plays by hand (eating is separate: `Control::AutoEat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helper {
    /// A totem from the inventory goes to the off hand when it is empty.
    Totem,
    /// A used-up stack in the hotbar is replaced from the inventory in the same slot.
    Fill,
    /// The right tool is taken for the block being broken.
    Tools,
    /// A mouse button is pressed again and again at a chosen pace.
    AutoClick,
    /// A bot task picks itself up after the server restarts and the client reconnects (on by
    /// default, unlike the others).
    Resume,
    /// A walking bot turns its head toward where it goes, as a person does (on by default);
    /// moving the mouse lends the camera back for a while without stopping the task.
    HumanLook,
}
impl Helper {
    pub const ALL: [Helper; 6] = [
        Helper::Totem,
        Helper::Fill,
        Helper::Tools,
        Helper::AutoClick,
        Helper::Resume,
        Helper::HumanLook,
    ];
    pub fn index(self) -> usize {
        match self {
            Self::Totem => 0,
            Self::Fill => 1,
            Self::Tools => 2,
            Self::AutoClick => 3,
            Self::Resume => 4,
            Self::HumanLook => 5,
        }
    }
    /// The settings key the choice is kept under.
    pub fn key(self) -> &'static str {
        match self {
            Self::Totem => "auto_totem",
            Self::Fill => "auto_fill",
            Self::Tools => "auto_tools",
            Self::AutoClick => "auto_click",
            Self::Resume => "auto_resume",
            Self::HumanLook => "human_look",
        }
    }
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
    NavigationMenu,
    /// Absolute page for an editor that would otherwise shrink its controls to fit.
    SettingsContentPage(usize),
    /// Sets (or, aiming elsewhere, clears) the farm's storage container.
    FarmStorage(super::claim_borders_farm_storage::Role),
    /// Takes the cell the player stands on as the farm's safe point (or clears it).
    FarmSafe,
    /// Bone meal on the crops that are still growing, on or off.
    FarmBonemeal,
    /// Sells the harvest with `/sell hand` once the harvest container is full, on or off.
    FarmAutoSell,
    /// Cycles the field between flat and several levels (`farming::LEVELS`).
    FarmLevels,
    /// Switches eating on its own.
    AutoEat,
    AutoEatFoods,
    AutoEatFood(usize),
    AutoEatFoodPage(usize),
    /// Switches one of the helpers of the Helpers tab.
    Helper(Helper),
    /// Cycles the auto-clicker's button: left or right.
    AutoClickButton,
    /// Cycles the auto-clicker's pace.
    AutoClickRate,
    /// Switches the contents preview in shulker box tooltips.
    ToggleShulkerPeek,
    /// Cycles who may pat this player.
    PatPolicy,
    /// Switches the notification of a received pat.
    PatNotify,
    ProtectGlowBerries,
    ShowInvisibleEntities,
    /// Cycles what Drain builds: a fill, lines or a grid.
    DrainPattern,
    /// Cycles the distance between the lines of a pattern.
    DrainSpacing,
    /// An icon tile that picks the crop to farm.
    FarmCropPick(super::claim_borders_farming::Crop),
    FarmRadius,
    FarmReplant,
    /// Turns the farm on a marked plot on or off, shows or hides it, forgets it.
    FarmPlotFarm(usize),
    FarmPlotShow(usize),
    FarmPlotRemove(usize),
    DrainRadius,
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
    /// Adds the block under the crosshair to the "own block" mining target (or takes it out;
    /// aimed at nothing, clears the list).
    MiningCustom,
    /// The schematic editor's buttons (on the selected preview).
    SchematicEdit(SchematicEdit),
    /// Places the chosen file as a preview (at XYZ, or where the player stands).
    SchematicPreviewHere,
    /// Which layers of the previews are drawn (all, one, up to one), and the layer ±1.
    SchematicLayerMode,
    SchematicLayer(i8),
    /// Pages through the previews of this world (own and friends').
    SchematicPlacePage(i8),
    /// Pages through the marked farm plots.
    FarmPlotPage(i8),
    /// A placed schematic preview: select it, show or hide it, forget it.
    SchematicPlace(usize),
    SchematicPlaceShow(usize),
    SchematicPlaceRemove(usize),
    SchematicFriendShow(usize),
    MiningPage,
    NavigationPlayer,
    NavigationWaypoint,
    NavigationPlayerChoice(usize),
    NavigationWaypointChoice(usize),
    NavigationChoicesPage(i8),
    SchematicChoice(usize),
    ExperimentalTab(ExperimentalSection),
    HelperPage(usize),
    ToggleReconnect,
    SettingsTab(u8),
    TradingGroup(u8),
    ConfirmExperiments,
    DeclineExperiments,
    SaveDeepLKey,
    RemoveDeepLKey,
    Language,
    SettingsSection(i8),
    SettingsPage(i8),
    EconomySetting(&'static str, i64),
    EconomyChoice(&'static str),
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
    ExperimentalRange(u8),
    ExperimentalOreToggle(u8),
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
    /// A non-editable numeric readout, visually distinct from action buttons and text inputs.
    pub readout: bool,
    /// A labelled row background behind a stepper's readout and buttons.
    pub row: bool,
}
#[derive(Clone)]
pub struct Menu {
    /// Selected crops, one bit each (`Crop::bit`).
    pub farm_crops: u8,
    pub farm_radius: u8,
    pub farm_replant: bool,
    pub farm_bonemeal: bool,
    pub farm_autosell: bool,
    pub farm_levels: u8,
    pub drain_radius: u8,
    pub drain_pattern: super::claim_borders_draining::Pattern,
    pub drain_spacing: u8,
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
    pub navigation_player: Option<u128>,
    pub navigation_x: String,
    pub navigation_y: String,
    pub navigation_z: String,
    pub navigation_waypoint: Option<u64>,
    pub navigation_choices: Option<bool>, // false: waypoints, true: players
    pub navigation_choices_page: usize,
    /// The schematic file list is open under the file button.
    /// The page of the schematics folder shown on the tab.
    pub schematic_catalog_page: usize,
    pub schematic_place_page: usize,
    pub farm_plot_page: usize,
    pub experiments_prompt: bool,
    pub experimental_section: ExperimentalSection,
    pub helper_page: usize,
    pub auto_eat_food_picker: bool,
    pub auto_eat_food_page: usize,
    deepl_key: String,
    pub language: Language,
    pub tab: Tab,
    pub navigation_menu: bool,
    pub settings_section: u8,
    pub trading_group: u8,
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
            farm_crops: super::claim_borders_farming::Crop::Wheat.bit(),
            farm_radius: 6,
            farm_replant: true,
            farm_bonemeal: true,
            farm_autosell: false,
            farm_levels: 0,
            drain_radius: 6,
            drain_pattern: super::claim_borders_draining::Pattern::Fill,
            drain_spacing: 4,
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
            schematic_catalog_page: 0,
            schematic_place_page: 0,
            farm_plot_page: 0,
            navigation_player: None,
            baritone_mode: BaritoneMode::Navigation,
            mining_targets: 1,
            mining_page: 0,
            construct_direction: super::claim_borders_construction::Direction::North,
            construct_length: 16,
            construct_width: 1,
            construct_height: 2,
            construct_floor: true,
            experimental_section: ExperimentalSection::default(),
            helper_page: 0,
            auto_eat_food_picker: false,
            auto_eat_food_page: 0,
            deepl_key: String::new(),
            language: Language::default(),
            tab: Tab::Waypoints,
            navigation_menu: false,
            settings_section: 0,
            trading_group: 0,
            settings_page: 0,
            claim_owner: String::new(),
            claim_filter: 0,
            party_filter: PartyFilter::Users,
            chat_section: ChatSection::Appearance,
            claims_section: ClaimsSection::List,
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
/// Where the Schematics tab's panel starts, and the room kept below it (status and key hint).
const SCHEMATIC_TOP: f32 = 100.0;
const SCHEMATIC_BOTTOM: f32 = 48.0;
/// The files the Schematics tab's list shows at once, below its header row.
fn schematic_list_rows(top: f32, limit: f32) -> usize {
    ((limit - (top + 18.0)) / 16.0).floor().max(0.0) as usize
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
        readout: false,
        row: false,
    }
}
/// Pairs of `Name − value` / `Name + value` buttons become one labelled stepper row:
/// the name, a readout of the value and compact − and + buttons that keep their controls.
fn merge_steppers(widgets: Vec<Widget>) -> Vec<Widget> {
    let mut out = Vec::with_capacity(widgets.len());
    let mut items = widgets.into_iter().peekable();
    while let Some(a) = items.next() {
        let pair = items.peek().and_then(|b| {
            let (name, value) = a.text.split_once(" − ")?;
            let (other, same) = b.text.split_once(" + ")?;
            (a.control.is_some()
                && b.control.is_some()
                && !name.is_empty()
                && name == other
                && value == same
                && a.rect[1] == b.rect[1]
                && a.rect[3] == b.rect[3])
                .then(|| (name.to_owned(), value.to_owned()))
        });
        let Some((name, value)) = pair else {
            out.push(a);
            continue;
        };
        let b = items.next().expect("peeked");
        let [x, y, _, h] = a.rect;
        let right = b.rect[0] + b.rect[2];
        let button = h.clamp(18.0, 22.0);
        let readout_w = 76.0_f32.min((right - x) * 0.35);
        let mut row = widget([x, y, right - x, h], name, None, false);
        row.row = true;
        out.push(row);
        let mut readout = widget(
            [
                right - 2.0 * button - 6.0 - readout_w,
                y + 2.0,
                readout_w,
                (h - 4.0).max(1.0),
            ],
            value,
            None,
            false,
        );
        readout.readout = true;
        out.push(readout);
        out.push(widget(
            [right - 2.0 * button - 3.0, y, button, h],
            "−",
            a.control,
            false,
        ));
        out.push(widget(
            [right - button, y, button, h],
            "+",
            b.control,
            false,
        ));
    }
    out
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
        self.farm_crops = from.farm_crops;
        self.farm_radius = from.farm_radius;
        self.farm_replant = from.farm_replant;
        self.farm_bonemeal = from.farm_bonemeal;
        self.farm_autosell = from.farm_autosell;
        self.farm_levels = from.farm_levels;
        self.drain_radius = from.drain_radius;
        self.drain_pattern = from.drain_pattern;
        self.drain_spacing = from.drain_spacing;
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
                        // Mining has no block limit: start and stop with the action keys.
                        limit: 0,
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
                        width: self.construct_width,
                        height: if bridge { 2 } else { self.construct_height },
                        length: self.construct_length,
                        fill_floor: bridge || self.construct_floor,
                    }),
                    running,
                })
            }
            BaritoneMode::Farm => Ok(Action::Navigate {
                goal: Goal::Farm(super::claim_borders_farming::Config {
                    crops: self.farm_crops,
                    radius: self.farm_radius,
                    // No limit: the farm works until it is stopped.
                    limit: 0,
                    replant: self.farm_replant,
                    bonemeal: self.farm_bonemeal,
                    autosell: self.farm_autosell,
                    levels: self.farm_levels,
                }),
                running,
            }),
            BaritoneMode::Drain => Ok(Action::Navigate {
                goal: Goal::Drain(super::claim_borders_draining::Config {
                    radius: self.drain_radius,
                    pattern: self.drain_pattern,
                    spacing: self.drain_spacing,
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
            Tab::Claims | Tab::Chat | Tab::Experimental | Tab::Trading | Tab::Settings => None,
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
    /// Claims per page of the list, from the panel height: rendering and Next share it.
    pub fn claims_per_page(height: f32) -> usize {
        (((height - CLAIMS_SELECTOR - 150.0) / 20.0).floor() as usize).clamp(1, 12)
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
            Tab::Claims | Tab::Chat | Tab::Experimental | Tab::Trading | Tab::Settings => 0,
        }
    }
    /// The preferences page on screen: Settings shows its own category, while Map and claims,
    /// Chat and Trading show their preferences in their own tabs.
    pub fn section(&self) -> u8 {
        match self.tab {
            Tab::Claims => 2,
            Tab::Chat => 3,
            Tab::Trading => 4,
            _ => self.settings_section,
        }
    }
    pub fn layout(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        merge_steppers(self.layout_widgets(model, width, height))
    }
    fn layout_widgets(&self, model: &ViewModel, width: f32, height: f32) -> Vec<Widget> {
        if self.experiments_prompt {
            return self.experiments_dialog(width, height);
        }
        let panel_w = (width - 16.0).clamp(220.0, 720.0);
        let panel_h = (height - 16.0).clamp(200.0, 400.0);
        let sidebar = rail_width(panel_w, panel_h);
        let nav_step = ((panel_h - 104.0) / 6.0).min(28.0);
        let x = (width - panel_w) * 0.5;
        let y = (height - panel_h) * 0.5;
        let mut widgets = self.legacy_layout(model, panel_w - sidebar, panel_h);
        for widget in &mut widgets {
            if sidebar > 0.0
                && self.tab == Tab::Experimental
                && self.experimental_section == ExperimentalSection::Baritone
                && widget.control.is_none()
                && widget.rect[1] == panel_h - 24.0
            {
                // Settings and language moved into the rail, so the key hint may use the full row.
                widget.rect[2] += 100.0;
            }
            widget.rect[0] += x + sidebar;
            widget.rect[1] += y;

            if sidebar > 0.0 && widget.control == Some(Control::Language) {
                widget.rect = [x + 6.0, y + panel_h - 28.0, sidebar - 12.0, 22.0];
            }
            if sidebar > 0.0
                && let Some(Control::Tab(tab)) = widget.control
            {
                let i = match tab {
                    Tab::Waypoints => 0,
                    Tab::Claims => 1,
                    Tab::Party => 2,
                    Tab::Chat => 3,
                    Tab::Trading => 4,
                    Tab::Experimental => 5,
                    Tab::Settings => 6,
                };
                widget.rect = [
                    x + 6.0,
                    if tab == Tab::Settings {
                        y + panel_h - 54.0
                    } else {
                        y + 44.0 + i as f32 * nav_step
                    },
                    sidebar - 12.0,
                    if tab == Tab::Settings {
                        22.0
                    } else {
                        nav_step - 4.0
                    },
                ];
            }
        }
        if sidebar == 0.0 {
            let navigation = |control: Option<Control>| {
                matches!(control,
                Some(Control::Tab(tab)) if tab != Tab::Settings)
            };
            if self.navigation_menu {
                let mut index = 0;
                widgets.retain_mut(|widget| {
                    if navigation(widget.control) {
                        widget.rect = [
                            x + 8.0 + (index % 2) as f32 * (panel_w - 16.0) / 2.0,
                            y + 56.0 + (index / 2) as f32 * 30.0,
                            (panel_w - 20.0) / 2.0,
                            26.0,
                        ];
                        index += 1;
                        return true;
                    }
                    widget.text == "DraimHorizon"
                        || matches!(
                            widget.control,
                            Some(Control::Close | Control::Language | Control::Tab(Tab::Settings))
                        )
                });
            } else {
                widgets.retain(|widget| !navigation(widget.control));
            }
            widgets.push(widget(
                [x + 8.0, y + 30.0, panel_w - 16.0, 20.0],
                format!(
                    "{}: {}",
                    self.language.text("Раздел", "Section"),
                    self.tab.label(self.language)
                ),
                Some(Control::NavigationMenu),
                self.navigation_menu,
            ));
            if self.navigation_menu {
                return widgets;
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
        widgets
    }
    /// The Schematics tab, in two columns. Left: the files of the schematics folder as a list
    /// (the mouse wheel scrolls it; a click chooses one), with Refresh and Open folder above it.
    /// Right: the preview — where the chosen file goes and its turn, place it, build it, the
    /// editor that moves and turns the selected preview, show or hide, delete, copy its
    /// materials — and the previews placed in this world below.
    fn schematic_panel(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        top: f32,
        limit: f32,
        out: &mut Vec<Widget>,
    ) {
        let row = 16.0;
        let gap = 6.0;
        let left_w = ((w - gap) * 0.46).floor();
        let right = x + left_w + gap;
        let right_w = w - left_w - gap;
        let active = model.schematic_places.iter().find(|p| p.3);
        let l = self.language;
        // Left: header with the count and the two folder buttons, then the list.
        let button = ((left_w - 4.0) * 0.3).min(110.0);
        if top + row <= limit {
            let mut title = widget(
                [x, top, left_w - 2.0 * (button + 3.0), row - 2.0],
                localized!(l, "Файлы: {}", "Files: {}", model.schematic_files.len()),
                None,
                false,
            );
            title.muted = true;
            out.push(title);
            out.push(widget(
                [x + left_w - 2.0 * button - 3.0, top, button, row - 2.0],
                l.text("Обновить", "Refresh"),
                Some(Control::SchematicRefresh),
                false,
            ));
            out.push(widget(
                [x + left_w - button, top, button, row - 2.0],
                l.text("Открыть папку", "Open folder"),
                Some(Control::SchematicFolder),
                false,
            ));
        }
        let rows = schematic_list_rows(top, limit);
        let list_top = top + row + 2.0;
        if model.schematic_files.is_empty() {
            if rows > 0 {
                let mut empty = widget(
                    [x, list_top, left_w, row * 2.0],
                    l.text(
                        "Папка пуста: положите .litematic / .schem / .nbt и нажмите «Обновить»",
                        "The folder is empty: add .litematic / .schem / .nbt files and press Refresh",
                    ),
                    None,
                    false,
                );
                empty.muted = true;
                out.push(empty);
            }
        } else if rows > 0 {
            let start = self
                .schematic_catalog_page
                .min(model.schematic_files.len().saturating_sub(rows));
            for (i, (index, name)) in model
                .schematic_files
                .iter()
                .enumerate()
                .skip(start)
                .take(rows)
                .enumerate()
            {
                out.push(widget(
                    [x, list_top + i as f32 * row, left_w, row - 2.0],
                    name.clone(),
                    Some(Control::SchematicChoice(index)),
                    self.schematic_file.as_deref() == Some(name.as_str()),
                ));
            }
        }
        // Right: the preview.
        let mut y = top;
        let full = |y: f32| [right, y, right_w, row - 2.0];
        let cells = |n: usize, i: usize, y: f32| {
            let cell = (right_w - 3.0 * (n as f32 - 1.0)) / n as f32;
            [right + i as f32 * (cell + 3.0), y, cell, row - 2.0]
        };
        if y + row <= limit {
            let mut title = widget(
                full(y),
                match (&self.schematic_file, active) {
                    (_, Some(a)) => localized!(l, "Предпоказ: {}", "Preview: {}", a.1),
                    (Some(file), None) => localized!(l, "Файл: {}", "File: {}", file),
                    (None, None) => l
                        .text("Выберите схему слева", "Choose a schematic on the left")
                        .to_owned(),
                },
                None,
                false,
            );
            title.muted = true;
            out.push(title);
            y += row;
        }
        if y + row <= limit {
            out.push(widget(
                cells(2, 0, y),
                format!("XYZ: {}", self.schematic_origin),
                Some(Control::Field(Field::SchematicOrigin)),
                false,
            ));
            out.push(widget(
                cells(2, 1, y),
                localized!(l, "Поворот: {}°", "Turn: {}°", self.schematic_rotation * 90),
                Some(Control::SchematicRotation),
                false,
            ));
            y += row;
        }
        if y + row <= limit {
            out.push(widget(
                cells(2, 0, y),
                l.text("Поставить предпоказ", "Place preview"),
                Some(Control::SchematicPreviewHere),
                false,
            ));
            out.push(widget(
                cells(2, 1, y),
                l.text("Строить", "Build"),
                Some(Control::SchematicEdit(SchematicEdit::Build)),
                false,
            ));
            y += row + 3.0;
        }
        // The editor of the selected preview: a block along each axis, then turn, here, look.
        if y + 2.0 * row <= limit {
            let moves = [
                ("X −", [-1, 0, 0]),
                ("Y −", [0, -1, 0]),
                ("Z −", [0, 0, -1]),
                ("X +", [1, 0, 0]),
                ("Y +", [0, 1, 0]),
                ("Z +", [0, 0, 1]),
            ];
            for (i, (label, step)) in moves.into_iter().enumerate() {
                out.push(widget(
                    cells(3, i % 3, y + (i / 3) as f32 * row),
                    label,
                    Some(Control::SchematicEdit(SchematicEdit::Move(step))),
                    false,
                ));
            }
            y += 2.0 * row;
        }
        if y + row <= limit {
            let edits = [
                (l.text("Повернуть", "Turn"), SchematicEdit::Rotate),
                (l.text("Ко мне", "To me"), SchematicEdit::Here),
                (l.text("Вид", "View"), SchematicEdit::Look),
            ];
            for (i, (label, edit)) in edits.into_iter().enumerate() {
                out.push(widget(
                    cells(3, i, y),
                    label,
                    Some(Control::SchematicEdit(edit)),
                    false,
                ));
            }
            y += row;
        }
        if y + row <= limit {
            out.push(widget(
                cells(3, 0, y),
                if active.is_some_and(|a| a.2) {
                    l.text("Скрыть", "Hide")
                } else {
                    l.text("Показать", "Show")
                },
                Some(Control::SchematicEdit(SchematicEdit::Toggle)),
                active.is_some_and(|a| a.2),
            ));
            out.push(widget(
                cells(3, 1, y),
                l.text("Удалить", "Delete"),
                Some(Control::SchematicEdit(SchematicEdit::Remove)),
                false,
            ));
            out.push(widget(
                cells(3, 2, y),
                l.text("Материалы", "Materials"),
                Some(Control::SchematicMaterials),
                false,
            ));
            y += row;
        }
        // Layers: this player's own view of every preview (all, one layer, up to a layer).
        if y + row <= limit {
            use super::claim_borders_schematic_places::LayerMode;
            let (mode, layer) = model.schematic_layers;
            let half = (right_w - 3.0) / 2.0;
            out.push(widget(
                [right, y, half, row - 2.0],
                match mode {
                    LayerMode::All => l.text("Слои: все", "Layers: all").to_owned(),
                    LayerMode::Single => localized!(l, "Слой: {}", "Layer: {}", layer + 1),
                    LayerMode::Below => localized!(l, "Слои: 1–{}", "Layers: 1–{}", layer + 1),
                },
                Some(Control::SchematicLayerMode),
                mode != LayerMode::All,
            ));
            if mode != LayerMode::All {
                let step = (half - 3.0) / 2.0;
                out.push(widget(
                    [right + half + 3.0, y, step, row - 2.0],
                    l.text("Слой −", "Layer −"),
                    Some(Control::SchematicLayer(-1)),
                    false,
                ));
                out.push(widget(
                    [right + half + 6.0 + step, y, step, row - 2.0],
                    l.text("Слой +", "Layer +"),
                    Some(Control::SchematicLayer(1)),
                    false,
                ));
            }
            y += row + 3.0;
        }
        // The previews of this world: the player's own, then friends' (show or hide only), a
        // page at a time with the pager on the last line when they do not fit.
        if y + 12.0 <= limit {
            let mut title = widget(
                [right, y, right_w, 11.0],
                if model.schematic_places.is_empty() && model.schematic_friend_places.is_empty() {
                    l.text("Предпоказов в этом мире нет", "No previews in this world")
                } else {
                    l.text("Предпоказы в этом мире:", "Previews in this world:")
                },
                None,
                false,
            );
            title.muted = true;
            out.push(title);
            y += 13.0;
        }
        let rows = ((limit - y) / row).floor().max(0.0) as usize;
        if rows == 0 {
            return;
        }
        let half = right_w;
        let button = 44.0_f32.min(half * 0.24);
        let own = model.schematic_places.len();
        let total = own + model.schematic_friend_places.len();
        let pager = total > rows && rows >= 2;
        let per_page = if pager { rows - 1 } else { rows };
        let pages = total.div_ceil(per_page.max(1)).max(1);
        let page = self.schematic_place_page.min(pages - 1);
        if pager {
            let py = y + per_page as f32 * row;
            out.push(widget(
                [right, py, half / 2.0 - 2.0, row - 2.0],
                format!("← {}/{}", page + 1, pages),
                Some(Control::SchematicPlacePage(-1)),
                false,
            ));
            out.push(widget(
                [right + half / 2.0 + 2.0, py, half / 2.0 - 2.0, row - 2.0],
                format!("{}/{} →", page + 1, pages),
                Some(Control::SchematicPlacePage(1)),
                false,
            ));
        }
        for (slot, entry) in (page * per_page..total).take(per_page).enumerate() {
            let py = y + slot as f32 * row;
            if let Some((index, label, shown, selected)) = model.schematic_places.get(entry) {
                out.push(widget(
                    [right, py, half - 2.0 * (button + 3.0), row - 2.0],
                    label.clone(),
                    Some(Control::SchematicPlace(*index)),
                    *selected,
                ));
                out.push(widget(
                    [right + half - 2.0 * button - 3.0, py, button, row - 2.0],
                    if *shown {
                        l.text("Скрыть", "Hide")
                    } else {
                        l.text("Показ.", "Show")
                    },
                    Some(Control::SchematicPlaceShow(*index)),
                    *shown,
                ));
                out.push(widget(
                    [right + half - button, py, button, row - 2.0],
                    l.text("Удал.", "Del."),
                    Some(Control::SchematicPlaceRemove(*index)),
                    false,
                ));
            } else if let Some((label, shown)) = model.schematic_friend_places.get(entry - own) {
                out.push(widget(
                    [right, py, half - button - 3.0, row - 2.0],
                    label.clone(),
                    None,
                    false,
                ));
                out.push(widget(
                    [right + half - button, py, button, row - 2.0],
                    if *shown {
                        l.text("Скрыть", "Hide")
                    } else {
                        l.text("Показ.", "Show")
                    },
                    Some(Control::SchematicFriendShow(entry - own)),
                    *shown,
                ));
            }
        }
    }
    /// The Farm tab's marked plots of this world: how to mark one, then a line each with
    /// buttons to farm it or not, show or hide it, forget it.
    fn farm_plot_list(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        top: f32,
        limit: f32,
        out: &mut Vec<Widget>,
    ) {
        let row = 15.0;
        if top + 12.0 > limit {
            return;
        }
        out.push(widget(
            [x, top, w, 11.0],
            if model.farm_plots.is_empty() {
                self.language.text(
                    "Участков нет: /dfarm pos1, /dfarm pos2, /dfarm save (без них — радиус вокруг)",
                    "No plots: /dfarm pos1, /dfarm pos2, /dfarm save (without any, the radius around)",
                )
            } else {
                self.language.text(
                    "Участки (/dfarm pos1, /dfarm pos2, /dfarm save):",
                    "Plots (/dfarm pos1, /dfarm pos2, /dfarm save):",
                )
            },
            None,
            false,
        ));
        let y = top + 13.0;
        let rows = ((limit - y) / row).floor().max(0.0) as usize;
        if rows == 0 {
            return;
        }
        // One page of plots, the pager on the last line when there are more.
        let pager = model.farm_plots.len() > rows && rows >= 2;
        let per_page = if pager { rows - 1 } else { rows };
        let pages = model.farm_plots.len().div_ceil(per_page).max(1);
        let page = self.farm_plot_page.min(pages - 1);
        if pager {
            let py = y + per_page as f32 * row;
            out.push(widget(
                [x, py, w / 2.0 - 2.0, row - 2.0],
                format!("← {}/{}", page + 1, pages),
                Some(Control::FarmPlotPage(-1)),
                false,
            ));
            out.push(widget(
                [x + w / 2.0 + 2.0, py, w / 2.0 - 2.0, row - 2.0],
                format!("{}/{} →", page + 1, pages),
                Some(Control::FarmPlotPage(1)),
                false,
            ));
        }
        let button = 52.0_f32.min(w * 0.16);
        for (i, (index, label, farmed, shown)) in model
            .farm_plots
            .iter()
            .skip(page * per_page)
            .take(per_page)
            .enumerate()
        {
            let py = y + i as f32 * row;
            out.push(widget(
                [x, py, w - 3.0 * (button + 3.0), row - 2.0],
                label.clone(),
                None,
                false,
            ));
            out.push(widget(
                [x + w - 3.0 * button - 6.0, py, button, row - 2.0],
                if *farmed {
                    self.language.text("Ферма: вкл", "Farm: on")
                } else {
                    self.language.text("Ферма: выкл", "Farm: off")
                },
                Some(Control::FarmPlotFarm(*index)),
                *farmed,
            ));
            out.push(widget(
                [x + w - 2.0 * button - 3.0, py, button, row - 2.0],
                if *shown {
                    self.language.text("Скрыть", "Hide")
                } else {
                    self.language.text("Показ.", "Show")
                },
                Some(Control::FarmPlotShow(*index)),
                *shown,
            ));
            out.push(widget(
                [x + w - button, py, button, row - 2.0],
                self.language.text("Удал.", "Del."),
                Some(Control::FarmPlotRemove(*index)),
                false,
            ));
        }
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
                self.language.text("Друзья", "Friends"),
            ),
            (Control::Tab(Tab::Claims), Tab::Claims.label(self.language)),
            (Control::Tab(Tab::Chat), self.language.text("Чат", "Chat")),
            (
                Control::Tab(Tab::Experimental),
                self.language.text("Эксперименты", "Experiments"),
            ),
            (
                Control::Tab(Tab::Trading),
                self.language.text("Торговля", "Trading"),
            ),
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
        if matches!(
            self.tab,
            Tab::Settings | Tab::Trading | Tab::Chat | Tab::Claims
        ) {
            return self.settings_layout(model, x, w, height, out);
        }
        if self.tab == Tab::Experimental {
            return self.experimental_layout(model, x, w, height, out);
        }
        if self.tab == Tab::Party {
            return self.party_layout(model, width, height, out);
        }
        {
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
            [x, 80.0, w * 0.4 - 3.0, 18.0],
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
            [x + w * 0.4, 80.0, w * 0.25 - 3.0, 18.0],
            if model.map_preferences.all_dimensions {
                self.language.text("Все миры", "All worlds")
            } else {
                self.language.text("Этот мир", "This world")
            },
            Some(Control::MapDimensions),
            false,
        ));
        out.push(widget(
            [x + w * 0.65, 80.0, w * 0.35, 18.0],
            self.language.text("Обновить /claims", "Refresh /claims"),
            Some(Control::Refresh),
            false,
        ));
        let indices = self.claim_indices(model);
        if indices.is_empty() {
            let mut hint = widget(
                [x + 4.0, 108.0, w - 8.0, 14.0],
                if model.claim_rows.is_empty() {
                    self.language.text(
                        "Приватов пока нет. На сервере нажмите «Обновить /claims».",
                        "No claims yet. On a server, press Refresh /claims.",
                    )
                } else {
                    self.language.text(
                        "Нет приватов под этот фильтр.",
                        "No claims match this filter.",
                    )
                },
                None,
                false,
            );
            hint.muted = true;
            out.push(hint);
        }
        let per = Self::claims_per_page(height + CLAIMS_SELECTOR);
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
        // Map and chat preferences live in their own tabs; Settings keeps what is mod-wide.
        let titles = match self.tab {
            Tab::Trading => vec![
                (0, self.language.text("Основное", "General")),
                (1, self.language.text("Панель", "Overlay")),
                (2, self.language.text("Обновления", "Updates")),
                (3, self.language.text("Прибыль", "Profit")),
                (4, self.language.text("Комиссии", "Fees")),
            ],
            Tab::Settings => vec![
                (0, self.language.text("Общее", "General")),
                (1, self.language.text("Связь", "Network")),
                (5, self.language.text("Список игроков", "Player list")),
                (6, self.language.text("Боковая панель", "Sidebar")),
            ],
            _ => Vec::new(),
        };
        // One row of categories when the labels fit, otherwise wrap into rows of three.
        let columns = if titles.len() as f32 * 104.0 <= w {
            titles.len().max(1)
        } else {
            3
        };
        let rows = titles.len().div_ceil(columns);
        let body_top = if rows == 0 {
            56.0
        } else {
            56.0 + rows as f32 * 22.0 + 4.0
        };
        for (i, (section, title)) in titles.into_iter().enumerate() {
            out.push(widget(
                [
                    x + (i % columns) as f32 * w / columns as f32,
                    56.0 + (i / columns) as f32 * 22.0,
                    w / columns as f32 - 3.0,
                    20.0,
                ],
                title,
                Some(if self.tab == Tab::Trading {
                    Control::TradingGroup(section)
                } else {
                    Control::SettingsTab(section)
                }),
                if self.tab == Tab::Trading {
                    self.trading_group == section
                } else {
                    self.settings_section == section
                },
            ));
        }
        let section = self.section();
        if self.tab == Tab::Claims && self.claims_section == ClaimsSection::List {
            // The claims list keeps its own filters and paging under the section selector.
            out.extend(
                self.claims_layout(model, x, w, 400.0, Vec::new())
                    .into_iter()
                    .filter(|w| matches!(w.control, Some(Control::ClaimsSection(_)))),
            );
            let list = self.claim_list_layout(model, x, w, height - CLAIMS_SELECTOR, Vec::new());
            out.extend(list.into_iter().map(|mut widget| {
                widget.rect[1] += CLAIMS_SELECTOR;
                widget
            }));
            return out;
        }
        let mut inner = match section {
            0 => {
                let rows = [
                    (
                        self.language
                            .text("Оформление интерфейса", "Interface appearance")
                            .to_owned(),
                        Control::Appearance,
                        false,
                        self.language.text(
                            "Цвет, прозрачность, эффекты и контраст",
                            "Accent, opacity, motion and contrast",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Авто-реконнект: {}",
                            "Auto reconnect: {}",
                            self.on(model.auto_reconnect)
                        ),
                        Control::ToggleReconnect,
                        model.auto_reconnect,
                        self.language.text(
                            "Вернуться на сервер после потери соединения",
                            "Rejoin the server after a lost connection",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Содержимое шалкеров: {}",
                            "Shulker preview: {}",
                            self.on(model.shulker_peek)
                        ),
                        Control::ToggleShulkerPeek,
                        model.shulker_peek,
                        self.language.text(
                            "Показывать предметы шалкера в подсказке",
                            "Show the items inside a shulker in its tooltip",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Кто может гладить: {}",
                            "Who can pat you: {}",
                            model.pat_policy.label(self.language)
                        ),
                        Control::PatPolicy,
                        model.pat_policy != super::claim_borders_patting::Policy::Off,
                        self.language.text(
                            "Анимация у игроков с DraimHorizon через relay",
                            "Animation from DraimHorizon players over the relay",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Уведомление о поглаживании: {}",
                            "Pat notification: {}",
                            self.on(model.pat_notify)
                        ),
                        Control::PatNotify,
                        model.pat_notify,
                        self.language.text(
                            "Строка над хотбаром, когда вас погладили",
                            "A line above the hotbar when someone pats you",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Защита светящихся ягод: {}",
                            "Protect glow berry vines: {}",
                            self.on(model.protect_glow_berries)
                        ),
                        Control::ProtectGlowBerries,
                        model.protect_glow_berries,
                        self.language.text(
                            "Не ломать лианы и не собирать ягоды; приседайте для обхода",
                            "Prevent breaking vines and harvesting berries; sneak to bypass",
                        ),
                    ),
                    (
                        localized!(
                            self.language,
                            "Показывать невидимых: {}",
                            "Show invisible entities: {}",
                            self.on(model.show_invisible_entities)
                        ),
                        Control::ShowInvisibleEntities,
                        model.show_invisible_entities,
                        self.language.text(
                            "Требует согласия на эксперименты; на серверах возможен бан",
                            "Requires experimental consent; servers may ban this feature",
                        ),
                    ),
                ];
                // Hints show under the switches when every row and its hint fit.
                let detailed = height >= 54.0 + rows.len() as f32 * 52.0;
                let step = if detailed { 52.0 } else { 24.0 };
                let mut v = Vec::new();
                for (index, (label, control, selected, hint)) in rows.into_iter().enumerate() {
                    let y = 54.0 + index as f32 * step;
                    v.push(widget([x, y, w, 22.0], label, Some(control), selected));
                    if detailed || control == Control::ShowInvisibleEntities {
                        let mut hint =
                            widget([x + 6.0, y + 26.0, w - 12.0, 14.0], hint, None, false);
                        hint.muted = true;
                        v.push(hint);
                    }
                }
                v
            }
            1 => {
                let mut copy = self.clone();
                copy.relay_setup = true;
                let mut v = copy.party_layout(model, w + 16.0, height - 30.0, Vec::new());
                for widget in &mut v {
                    widget.rect[0] += x - 8.0;
                }
                // Back returns from the Friends tab's relay page; Settings has its own tabs,
                // so the auto-connect switch takes the whole row.
                v.retain(|v| v.rect[1] < 212.0 && v.control != Some(Control::RelayBack));
                for widget in &mut v {
                    if widget.control == Some(Control::ToggleAutoRelay) {
                        widget.rect[0] = x;
                        widget.rect[2] = w;
                    }
                }
                v.push(widget(
                    [x, 212.0, w, 18.0],
                    localized!(
                        self.language,
                        "Автодоверие: {}",
                        "Auto trust: {}",
                        self.on(model.auto_trust)
                    ),
                    Some(Control::ToggleAutoTrust),
                    model.auto_trust,
                ));
                v
            }
            2 => self.claims_layout(model, x, w, 400.0, Vec::new()),
            3 => self.chat_layout(model, x, w, 400.0, Vec::new()),
            section => {
                let settings: Vec<_> = super::claim_borders_economy::SETTINGS
                    .iter()
                    .filter(|setting| match section {
                        4 if self.tab == Tab::Trading => setting.group == self.trading_group,
                        4 => setting.group <= 4,
                        5 => setting.group == 6,
                        _ => setting.group == 5,
                    })
                    .collect();
                let reserve = if self.tab == Tab::Trading { 82.0 } else { 56.0 };
                let per = (((height - body_top - reserve) / 22.0).floor() as usize).max(1);
                let pages = settings.len().div_ceil(per).max(1);
                let page = self.settings_page.min(pages - 1);
                for (i, setting) in settings.into_iter().skip(page * per).take(per).enumerate() {
                    let y = body_top + i as f32 * 22.0;
                    let label = self.language.text(setting.label, setting.label_en);
                    let value = model.economy_preferences.get(setting.key);
                    let shown = setting.value_label(value, self.language);
                    if setting.is_toggle() || setting.is_choice() {
                        out.push(widget(
                            [x, y, w, 20.0],
                            format!("{label}: {shown}"),
                            Some(if setting.is_toggle() {
                                Control::EconomySetting(setting.key, 1)
                            } else {
                                Control::EconomyChoice(setting.key)
                            }),
                            setting.is_toggle() && value != 0,
                        ));
                        continue;
                    }
                    let mut row = widget([x, y, w, 20.0], label, None, false);
                    row.row = true;
                    out.push(row);
                    let mut readout =
                        widget([x + w - 134.0, y + 2.0, 84.0, 16.0], shown, None, false);
                    readout.readout = true;
                    out.push(readout);
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
                if self.tab == Tab::Trading {
                    out.push(widget(
                        [x, height - 74.0, w, 22.0],
                        self.language
                            .text("Открыть торговый кабинет", "Open trading workspace"),
                        Some(Control::Economy),
                        false,
                    ));
                }
                if pages > 1 {
                    out.push(widget(
                        [x, height - 48.0, w / 2.0 - 3.0, 18.0],
                        localized!(
                            self.language,
                            "Назад · {} / {}",
                            "Back · {} / {}",
                            page + 1,
                            pages
                        ),
                        Some(Control::SettingsPage(-1)),
                        false,
                    ));
                    out.push(widget(
                        [x + w / 2.0, height - 48.0, w / 2.0, 18.0],
                        self.language.text("Далее", "Next"),
                        Some(Control::SettingsPage(1)),
                        false,
                    ));
                }
                return out;
            }
        };
        // Keep nested section selectors visible, and paginate rows instead of making text and
        // hit targets microscopic on small GUI sizes. Fields and actions stay in their rows.
        let mut selectors = Vec::new();
        inner.retain(|widget| {
            if matches!(
                widget.control,
                Some(Control::ChatSection(_) | Control::ClaimsSection(_))
            ) {
                selectors.push(widget.clone());
                false
            } else {
                true
            }
        });
        let content_top = body_top + if selectors.is_empty() { 0.0 } else { 24.0 };
        for mut widget in selectors {
            widget.rect[1] = body_top;
            out.push(widget);
        }
        inner.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
        let available = (height - content_top - 52.0).max(22.0);
        let mut pages: Vec<Vec<Widget>> = vec![Vec::new()];
        let mut start = inner.first().map_or(0.0, |widget| widget.rect[1]);
        let mut row = 0;
        while row < inner.len() {
            let y = inner[row].rect[1];
            let end = (row + 1..inner.len())
                .find(|&i| inner[i].rect[1] != y)
                .unwrap_or(inner.len());
            let bottom = inner[row..end]
                .iter()
                .map(|widget| y + widget.rect[3])
                .fold(y, f32::max);
            if bottom - start > available && !pages.last().unwrap().is_empty() {
                pages.push(Vec::new());
                start = y;
            }
            for widget in &inner[row..end] {
                let mut widget = widget.clone();
                widget.rect[1] = content_top + y - start;
                pages.last_mut().unwrap().push(widget);
            }
            row = end;
        }
        let page = self
            .focus
            .and_then(|field| {
                pages.iter().position(|page| {
                    page.iter()
                        .any(|widget| widget.control == Some(Control::Field(field)))
                })
            })
            .unwrap_or_else(|| self.settings_page.min(pages.len() - 1));
        out.extend(pages[page].iter().cloned());
        if pages.len() > 1 {
            out.push(widget(
                [x, height - 48.0, w / 2.0 - 3.0, 20.0],
                localized!(
                    self.language,
                    "Назад · {} / {}",
                    "Back · {} / {}",
                    page + 1,
                    pages.len()
                ),
                Some(Control::SettingsContentPage(page.saturating_sub(1))),
                false,
            ));
            out.push(widget(
                [x + w / 2.0, height - 48.0, w / 2.0, 20.0],
                self.language.text("Далее", "Next"),
                Some(Control::SettingsContentPage(
                    (page + 1).min(pages.len() - 1),
                )),
                false,
            ));
        }
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
            (ClaimsSection::List, self.language.text("Приваты", "Claims")),
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
                    self.language.text("Открыть карту", "Open map").into(),
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
            // Drawn by `claim_list_layout` under the same selector.
            ClaimsSection::List => return out,
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
        let tab_width = (w - 8.0) / 3.0;
        for (index, section, label) in [
            (
                0,
                ExperimentalSection::Ores,
                self.language.text("Руды", "Ores"),
            ),
            (1, ExperimentalSection::Baritone, "Baritone"),
            (
                2,
                ExperimentalSection::Helpers,
                self.language.text("Вспомогательные", "Helpers"),
            ),
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
                            localized!(self.language, "Радиус: {}", "Radius: {}", self.farm_radius),
                            Control::FarmRadius,
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
                        (
                            localized!(
                                self.language,
                                "Костная мука: {}",
                                "Bone meal: {}",
                                if self.farm_bonemeal {
                                    self.language.text("Вкл", "On")
                                } else {
                                    self.language.text("Выкл", "Off")
                                }
                            ),
                            Control::FarmBonemeal,
                        ),
                        (
                            localized!(
                                self.language,
                                "Авто-продажа: {}",
                                "Auto sell: {}",
                                if self.farm_autosell {
                                    self.language.text("Вкл", "On")
                                } else {
                                    self.language.text("Выкл", "Off")
                                }
                            ),
                            Control::FarmAutoSell,
                        ),
                        (
                            if self.farm_levels == 0 {
                                self.language
                                    .text("Уровни: плоская", "Levels: flat")
                                    .to_owned()
                            } else {
                                localized!(
                                    self.language,
                                    "Уровни: ±{}",
                                    "Levels: ±{}",
                                    self.farm_levels
                                )
                            },
                            Control::FarmLevels,
                        ),
                    ]
                } else {
                    // The Schematics tab lays itself out (`schematic_panel`).
                    Vec::new()
                };
                // The crop is picked from item icons (name as the tooltip), not by cycling.
                let mut below = 0.0;
                if farm {
                    use super::claim_borders_farming::Crop;
                    let tile = 22.0;
                    for (i, crop) in Crop::ALL.into_iter().enumerate() {
                        let name = match crop {
                            Crop::Wheat => self.language.text("Пшеница", "Wheat"),
                            Crop::Carrots => self.language.text("Морковь", "Carrots"),
                            Crop::Potatoes => self.language.text("Картофель", "Potatoes"),
                            Crop::Beetroot => self.language.text("Свёкла", "Beetroot"),
                        };
                        out.push(widget(
                            [x + i as f32 * (tile + 2.0), 100.0, tile, tile],
                            name,
                            Some(Control::FarmCropPick(crop)),
                            self.farm_crops & crop.bit() != 0,
                        ));
                    }
                    // The three containers share the row with the crops: harvest, seeds, bone
                    // meal. Aim at a chest, barrel or shulker box and press; press with the
                    // crosshair elsewhere to clear. A highlighted button has its container set.
                    use super::claim_borders_farm_storage::Role;
                    let from = x + Crop::ALL.len() as f32 * (tile + 2.0) + 4.0;
                    let width = ((x + w - from - 6.0) / 4.0).max(18.0);
                    for (i, (role, ru, en)) in [
                        (Role::Harvest, "Урожай", "Harvest"),
                        (Role::Seeds, "Семена", "Seeds"),
                        (Role::Meal, "Мука", "Meal"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        out.push(widget(
                            [from + i as f32 * (width + 2.0), 100.0, width, tile],
                            self.language.text(ru, en),
                            Some(Control::FarmStorage(role)),
                            model.farm_storage[role.index()].is_some(),
                        ));
                    }
                    // The fourth button: the safe point to wait at. Stand there and press.
                    out.push(widget(
                        [from + 3.0 * (width + 2.0), 100.0, width, tile],
                        self.language.text("Точка", "Spot"),
                        Some(Control::FarmSafe),
                        model.farm_safe.is_some(),
                    ));
                    below = tile + 2.0;
                }
                let count = rows.len();
                for (i, (label, c)) in rows.into_iter().enumerate() {
                    out.push(widget(
                        [
                            x + (i % 2) as f32 * (tab_width + 4.0),
                            100.0 + below + (i / 2) as f32 * step,
                            tab_width,
                            h,
                        ],
                        label,
                        Some(c),
                        false,
                    ));
                }
                if !farm {
                    self.schematic_panel(
                        model,
                        x,
                        w,
                        SCHEMATIC_TOP,
                        height - SCHEMATIC_BOTTOM,
                        &mut out,
                    );
                }
                if farm {
                    // What the buttons are bound to, with the coordinates and the colour of the
                    // frame drawn around it in the world, as many lines as there is room for.
                    let at = |pos: Option<[i32; 3]>, none: &str| {
                        pos.map_or(none.to_owned(), |[x, y, z]| format!("{x} {y} {z}"))
                    };
                    let unset = self.language.text("не задан", "not set");
                    let entries = [
                        localized!(
                            self.language,
                            "Урожай (оранж.): {}",
                            "Harvest (orange): {}",
                            at(model.farm_storage[0], unset)
                        ),
                        localized!(
                            self.language,
                            "Семена (зелён.): {}",
                            "Seeds (green): {}",
                            at(model.farm_storage[1], unset)
                        ),
                        localized!(
                            self.language,
                            "Мука (бел.): {}",
                            "Meal (white): {}",
                            at(model.farm_storage[2], unset)
                        ),
                        localized!(
                            self.language,
                            "Точка (син.): {}",
                            "Spot (blue): {}",
                            at(model.farm_safe, unset)
                        ),
                        localized!(
                            self.language,
                            "Компостер (корич.): {}",
                            "Composter (brown): {}",
                            at(
                                model.farm_composter,
                                self.language.text("рядом нет", "none nearby")
                            )
                        ),
                    ];
                    let top = 100.0 + below + count.div_ceil(2) as f32 * step + 2.0;
                    let half = (w - 4.0) / 2.0;
                    let mut next = top;
                    // The plots come first: room for the header and up to three of them (with
                    // the pager when there are more) is kept before the coordinate lines.
                    let plots = model.farm_plots.len();
                    let keep = if plots == 0 {
                        0.0
                    } else {
                        15.0 + 15.0 * plots.min(3) as f32 + if plots > 3 { 15.0 } else { 0.0 }
                    };
                    for (i, entry) in entries.into_iter().enumerate() {
                        let y = top + (i / 2) as f32 * 12.0;
                        if y + 11.0 > height - 50.0 - keep {
                            break;
                        }
                        out.push(widget(
                            [x + (i % 2) as f32 * (half + 4.0), y, half, 11.0],
                            entry,
                            None,
                            false,
                        ));
                        next = y + 12.0;
                    }
                    self.farm_plot_list(model, x, w, next + 2.0, height - 50.0, &mut out);
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
                    // A bridge has a length and a width, like a tunnel; its height is fixed.
                    out.push(widget(
                        [x, 100.0 + step, w, h],
                        localized!(
                            self.language,
                            "Ширина: {}",
                            "Width: {}",
                            self.construct_width
                        ),
                        Some(Control::ConstructWidth),
                        false,
                    ));
                    out.push(widget(
                        [x, 100.0 + 2.0 * step, w, h],
                        self.language.text(
                            "Блоки: хотбар, вторая рука или инвентарь",
                            "Blocks: hotbar, off hand or inventory",
                        ),
                        None,
                        false,
                    ));
                }
            } else if self.baritone_mode == BaritoneMode::Drain {
                use super::claim_borders_draining::Pattern;
                out.push(widget(
                    [x, 100.0, tab_width, h],
                    localized!(self.language, "Радиус: {}", "Radius: {}", self.drain_radius),
                    Some(Control::DrainRadius),
                    false,
                ));
                out.push(widget(
                    [x + tab_width + 4.0, 100.0, tab_width, h],
                    match self.drain_pattern {
                        Pattern::Fill => self.language.text("Узор: заливка", "Pattern: fill"),
                        Pattern::Lines => self.language.text("Узор: линии", "Pattern: lines"),
                        Pattern::Grid => self.language.text("Узор: сетка", "Pattern: grid"),
                    },
                    Some(Control::DrainPattern),
                    self.drain_pattern != Pattern::Fill,
                ));
                if self.drain_pattern != Pattern::Fill {
                    out.push(widget(
                        [x, 100.0 + step, tab_width, h],
                        localized!(self.language, "Шаг: {}", "Spacing: {}", self.drain_spacing),
                        Some(Control::DrainSpacing),
                        false,
                    ));
                }
                out.push(widget(
                    [x, 100.0 + 2.0 * step, w, 14.0],
                    match self.drain_pattern {
                        Pattern::Fill => self.language.text(
                            "Засыпает воду песком и гравием, столбик за столбиком",
                            "Fills water with sand and gravel, one column after another",
                        ),
                        Pattern::Lines => self.language.text(
                            "Линии из песка и гравия; идёт по уже уложенному",
                            "Lines of sand and gravel; walks out along what is laid",
                        ),
                        Pattern::Grid => self.language.text(
                            "Сетка из песка и гравия: вода делится на квадраты",
                            "A grid of sand and gravel: the water is cut into squares",
                        ),
                    },
                    None,
                    false,
                ));
            } else if self.baritone_mode == BaritoneMode::Mining {
                // A chip per target, toggled like the structure filters on the map.
                use super::claim_borders_mining as mining;
                let top = 100.0 + step;
                // Icon tiles: the name is the tooltip. Square, so many fit in a row.
                let tile = 22.0;
                let cols = (((w + 2.0) / (tile + 2.0)).floor() as usize).max(1);
                let rows = (((height - 66.0 - top) / (tile + 2.0)).floor() as usize).max(1);
                let per_page = rows * cols;
                let pages = mining::COUNT.div_ceil(per_page);
                let page = self.mining_page % pages;
                out.push(widget(
                    [x, 100.0, w, h],
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
                            x + (slot % cols) as f32 * (tile + 2.0),
                            top + (slot / cols) as f32 * (tile + 2.0),
                            tile,
                            tile,
                        ],
                        self.language.text(ru, en),
                        Some(Control::MiningTarget(index as u8)),
                        self.mining_targets & (1 << index) != 0,
                    ));
                }
                // Any block of the world: aim at it and press (again to take it out).
                let half = (w - 4.0) / 2.0;
                let names: Vec<&str> = model
                    .mining_custom
                    .iter()
                    .map(|n| n.strip_prefix("minecraft:").unwrap_or(n))
                    .collect();
                out.push(widget(
                    [x + half + 4.0, height - 64.0, half, 18.0],
                    if names.is_empty() {
                        self.language
                            .text("Свой блок: прицел на блок", "Own block: aim at a block")
                            .to_owned()
                    } else {
                        localized!(
                            self.language,
                            "Свой блок: {}",
                            "Own block: {}",
                            names.join(", ")
                        )
                    },
                    Some(Control::MiningCustom),
                    !names.is_empty(),
                ));
                // The storage the miner unloads into when the inventory is full (the farm's).
                out.push(widget(
                    [x, height - 64.0, half, 18.0],
                    match model.farm_storage[0] {
                        Some([sx, sy, sz]) => localized!(
                            self.language,
                            "Склад: {} {} {}",
                            "Storage: {} {} {}",
                            sx,
                            sy,
                            sz
                        ),
                        None => self
                            .language
                            .text(
                                "Склад: не задан (прицел на сундук)",
                                "Storage: not set (aim at a chest)",
                            )
                            .into(),
                    },
                    Some(Control::FarmStorage(
                        super::claim_borders_farm_storage::Role::Harvest,
                    )),
                    model.farm_storage[0].is_some(),
                ));
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
            // The status line, and the switch for eating on its own beside it.
            let eat_w = 92.0_f32.min(w * 0.4);
            let extra_footer = matches!(
                self.baritone_mode,
                BaritoneMode::Mining | BaritoneMode::Schematic
            );
            let status_y = height - if extra_footer { 44.0 } else { 48.0 };
            let status_h = if extra_footer { 18.0 } else { 14.0 };
            out.push(widget(
                [x, status_y, w - eat_w - 4.0, status_h],
                &model.navigation_status,
                None,
                false,
            ));
            out.push(widget(
                [x + w - eat_w, status_y, eat_w, status_h],
                localized!(
                    self.language,
                    "Еда: {}",
                    "Eat: {}",
                    if model.auto_eat {
                        self.language.text("вкл", "on")
                    } else {
                        self.language.text("выкл", "off")
                    }
                ),
                Some(Control::AutoEat),
                model.auto_eat,
            ));
            let mut hint = widget(
                [x, height - 24.0, (w - 100.0).max(1.0), 18.0],
                self.hotkey_hint(model),
                None,
                false,
            );
            hint.muted = true;
            out.push(hint);
            return out;
        }
        if self.experimental_section == ExperimentalSection::Helpers {
            return self.helpers_layout(model, x, w, height, out);
        }
        let p = model.experimental;
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
        ]
        .into_iter()
        .enumerate()
        {
            let step = ((height - 132.0) / 5.0).clamp(16.0, 28.0);
            let y = 82.0 + index as f32 * step;
            out.push(widget(
                [x, y, w, (step - 2.0).min(22.0)],
                label,
                Some(control),
                on,
            ));
        }
        // One icon tile per ore, toggled like the seed map filters; the name is the tooltip.
        let step = ((height - 132.0) / 5.0).clamp(16.0, 28.0);
        let range_y = 82.0 + 2.0 * step;
        let range_w = (w - 8.0) / 3.0;
        for (radius, label) in [
            self.language.text("1 чанк", "1 chunk"),
            "3x3",
            self.language.text("5x5 (тяжело)", "5x5 (heavy)"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(widget(
                [
                    x + radius as f32 * (range_w + 4.0),
                    range_y,
                    range_w,
                    (step - 2.0).min(22.0),
                ],
                label,
                Some(Control::ExperimentalRange(radius as u8)),
                p.prediction_radius == radius as u8,
            ));
        }
        let top = 82.0 + 3.0 * step;
        let count = experimental::ORES.len();
        let tile = ((w + 2.0) / count as f32 - 2.0)
            .min(height - 52.0 - top)
            .clamp(16.0, 26.0);
        for (kind, ore) in experimental::ORES.iter().enumerate() {
            out.push(widget(
                [x + kind as f32 * (tile + 2.0), top, tile, tile],
                self.language.text(ore.label, ore.label_en),
                Some(Control::ExperimentalOreToggle(kind as u8)),
                p.includes(kind),
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
    fn auto_eat_food_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        out.push(widget(
            [x, 84.0, w, 20.0],
            localized!(
                self.language,
                "Назад · Разрешённая еда: {}",
                "Back · Allowed foods: {}",
                model.auto_eat_foods.len()
            ),
            Some(Control::AutoEatFoods),
            false,
        ));
        out.push(widget(
            [x, 108.0, w, 14.0],
            self.language.text(
                "Выберите еду; пустой список запрещает всё",
                "Select foods; an empty list allows nothing",
            ),
            None,
            false,
        ));
        // Same square item tiles and spacing as the mining target filter.
        let tile = 22.0;
        let cols = (((w + 2.0) / (tile + 2.0)).floor() as usize).max(1);
        let rows = (((height - 66.0 - 128.0) / (tile + 2.0)).floor() as usize).max(1);
        let per = rows * cols;
        let pages = model.auto_eat_options.len().div_ceil(per).max(1);
        let page = self.auto_eat_food_page.min(pages - 1);
        let half = (w - 4.0) * 0.5;
        for (i, (item, name, nutrition)) in model
            .auto_eat_options
            .iter()
            .enumerate()
            .skip(page * per)
            .take(per)
        {
            let at = i - page * per;
            out.push(widget(
                [
                    x + (at % cols) as f32 * (tile + 2.0),
                    128.0 + (at / cols) as f32 * (tile + 2.0),
                    tile,
                    tile,
                ],
                format!("{name} (+{nutrition})"),
                Some(Control::AutoEatFood(i)),
                model.auto_eat_foods.contains(item),
            ));
        }
        for (column, target, label) in [
            (
                0,
                page.saturating_sub(1),
                localized!(
                    self.language,
                    "Назад · {} / {}",
                    "Previous · {} / {}",
                    page + 1,
                    pages
                ),
            ),
            (
                1,
                (page + 1).min(pages - 1),
                self.language.text("Далее", "Next").into(),
            ),
        ] {
            out.push(widget(
                [x + column as f32 * (half + 4.0), height - 48.0, half, 20.0],
                label,
                Some(Control::AutoEatFoodPage(target)),
                false,
            ));
        }
        out
    }
    /// The Helpers tab: a switch per helper, with a line under it saying what it does.
    fn helpers_layout(
        &self,
        model: &ViewModel,
        x: f32,
        w: f32,
        height: f32,
        mut out: Vec<Widget>,
    ) -> Vec<Widget> {
        if self.auto_eat_food_picker {
            return self.auto_eat_food_layout(model, x, w, height, out);
        }
        let state = |on: bool| {
            if on {
                self.language.text("вкл", "on")
            } else {
                self.language.text("выкл", "off")
            }
        };
        let rows = [
            (
                localized!(
                    self.language,
                    "Авто-еда: {}",
                    "Auto Eat: {}",
                    state(model.auto_eat)
                ),
                Control::AutoEat,
                model.auto_eat,
                self.language.text(
                    "Только выбранная еда, без потери очков голода",
                    "Only selected foods, without wasting hunger points",
                ),
            ),
            (
                localized!(
                    self.language,
                    "Авто-тотем: {}",
                    "Auto Totem: {}",
                    state(model.helpers[Helper::Totem.index()])
                ),
                Control::Helper(Helper::Totem),
                model.helpers[Helper::Totem.index()],
                self.language.text(
                    "Пустая вторая рука: тотем берётся из инвентаря",
                    "Empty off hand: a totem is taken from the inventory",
                ),
            ),
            (
                localized!(
                    self.language,
                    "Авто-пополнение: {}",
                    "Auto Fill: {}",
                    state(model.helpers[Helper::Fill.index()])
                ),
                Control::Helper(Helper::Fill),
                model.helpers[Helper::Fill.index()],
                self.language.text(
                    "Стак закончился: тот же предмет встаёт в тот же слот",
                    "A stack ran out: the same item goes into the same slot",
                ),
            ),
            (
                localized!(
                    self.language,
                    "Авто-инструменты: {}",
                    "Auto Tools: {}",
                    state(model.helpers[Helper::Tools.index()])
                ),
                Control::Helper(Helper::Tools),
                model.helpers[Helper::Tools.index()],
                self.language.text(
                    "Для ломаемого блока берётся нужный инструмент",
                    "The right tool is picked for the block being broken",
                ),
            ),
        ];
        let mut rows: Vec<_> = rows.into_iter().collect();
        rows.insert(
            1,
            (
                localized!(
                    self.language,
                    "Разрешённая еда: {}",
                    "Allowed foods: {}",
                    model.auto_eat_foods.len()
                ),
                Control::AutoEatFoods,
                !model.auto_eat_foods.is_empty(),
                self.language.text(
                    "Выберите, что может есть авто-еда",
                    "Choose which foods Auto Eat may consume",
                ),
            ),
        );
        let rows = rows.into_iter().chain([
            (
                localized!(
                    self.language,
                    "Авто-возобновление: {}",
                    "Auto Resume: {}",
                    state(model.helpers[Helper::Resume.index()])
                ),
                Control::Helper(Helper::Resume),
                model.helpers[Helper::Resume.index()],
                self.language.text(
                    "После рестарта сервера задача бота продолжается сама",
                    "After a server restart the bot's task carries on by itself",
                ),
            ),
            (
                localized!(
                    self.language,
                    "Человеческий взгляд: {}",
                    "Human Look: {}",
                    state(model.helpers[Helper::HumanLook.index()])
                ),
                Control::Helper(Helper::HumanLook),
                model.helpers[Helper::HumanLook.index()],
                self.language.text(
                    "Бот смотрит, куда идёт; мышь забирает камеру, задача не прерывается",
                    "The bot looks where it walks; moving the mouse takes the camera back",
                ),
            ),
            (
                localized!(
                    self.language,
                    "Авто-клик: {}",
                    "Auto Click: {}",
                    state(model.helpers[Helper::AutoClick.index()])
                ),
                Control::Helper(Helper::AutoClick),
                model.helpers[Helper::AutoClick.index()],
                self.language.text(
                    "Кликает выбранной кнопкой в заданном темпе: бьёт мобов и ломает блоки",
                    "Clicks the chosen button at the chosen pace: hits mobs and breaks blocks",
                ),
            ),
        ]);
        let rows: Vec<_> = rows.collect();
        let click_options = rows.len();
        let note = height >= 340.0;
        let step = if note { 34.0 } else { 24.0 };
        let per = (((height - 136.0) / step).floor() as usize).max(1);
        let pages = (click_options + 1).div_ceil(per);
        let page = self.helper_page.min(pages - 1);
        for (index, (label, control, on, hint)) in
            rows.into_iter().enumerate().skip(page * per).take(per)
        {
            let y = 84.0 + (index - page * per) as f32 * step;
            out.push(widget([x, y, w, 20.0], label, Some(control), on));
            if note {
                let mut hint = widget([x + 4.0, y + 21.0, w - 8.0, 11.0], hint, None, false);
                hint.muted = true;
                out.push(hint);
            }
        }
        if page * per <= click_options && click_options < (page + 1) * per {
            let y = 84.0 + (click_options - page * per) as f32 * step;
            let half = (w - 4.0) * 0.5;
            let row_h = 20.0;
            let russian = self.language == Language::Russian;
            out.push(widget(
                [x, y, half, row_h],
                localized!(
                    self.language,
                    "Кнопка: {}",
                    "Button: {}",
                    if model.click_right {
                        self.language.text("ПКМ", "right")
                    } else {
                        self.language.text("ЛКМ", "left")
                    }
                ),
                Some(Control::AutoClickButton),
                false,
            ));
            out.push(widget(
                [x + half + 4.0, y, half, row_h],
                localized!(
                    self.language,
                    "Частота: {}",
                    "Rate: {}",
                    super::claim_borders_autoclick::label(model.click_rate, russian)
                ),
                Some(Control::AutoClickRate),
                false,
            ));
        }
        if pages > 1 {
            for (column, target, text) in [
                (
                    0,
                    page.saturating_sub(1),
                    localized!(
                        self.language,
                        "Назад · {} / {}",
                        "Back · {} / {}",
                        page + 1,
                        pages
                    ),
                ),
                (
                    1,
                    (page + 1).min(pages - 1),
                    self.language.text("Далее", "Next").into(),
                ),
            ] {
                out.push(widget(
                    [
                        x + column as f32 * (w + 4.0) * 0.5,
                        height - 48.0,
                        (w - 4.0) * 0.5,
                        20.0,
                    ],
                    text,
                    Some(Control::HelperPage(target)),
                    false,
                ));
            }
        }

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
            return None;
        };
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
            Control::FarmCropPick(crop) => {
                // Crops toggle like the mining targets; one always stays selected.
                let next = self.farm_crops ^ crop.bit();
                if next & super::claim_borders_farming::Crop::MASK != 0 {
                    self.farm_crops = next;
                }
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
            Control::FarmReplant => {
                self.farm_replant = !self.farm_replant;
                None
            }
            Control::FarmStorage(role) => Some(Action::FarmStorage(role)),
            Control::FarmSafe => Some(Action::FarmSafe),
            Control::FarmBonemeal => {
                self.farm_bonemeal = !self.farm_bonemeal;
                None
            }
            Control::FarmAutoSell => {
                self.farm_autosell = !self.farm_autosell;
                None
            }
            Control::FarmLevels => {
                use super::claim_borders_farming::LEVELS;
                let at = LEVELS
                    .iter()
                    .position(|l| *l == self.farm_levels)
                    .unwrap_or(0);
                self.farm_levels = LEVELS[(at + 1) % LEVELS.len()];
                None
            }
            Control::AutoEat => Some(Action::SetAutoEat(!model.auto_eat)),
            Control::AutoEatFoods => {
                self.auto_eat_food_picker = !self.auto_eat_food_picker;
                self.auto_eat_food_page = 0;
                None
            }
            Control::AutoEatFoodPage(page) => {
                self.auto_eat_food_page = page;
                None
            }
            Control::AutoEatFood(index) => model.auto_eat_options.get(index).map(|(item, _, _)| {
                Action::SetAutoEatFood(item.clone(), !model.auto_eat_foods.contains(item))
            }),
            Control::Helper(helper) => {
                Some(Action::SetHelper(helper, !model.helpers[helper.index()]))
            }
            Control::AutoClickButton => Some(Action::SetAutoClickButton(!model.click_right)),
            Control::AutoClickRate => Some(Action::SetAutoClickRate(
                super::claim_borders_autoclick::next_rate(model.click_rate),
            )),
            Control::ToggleShulkerPeek => Some(Action::SetShulkerPeek(!model.shulker_peek)),
            Control::PatPolicy => Some(Action::SetPatPolicy(model.pat_policy.next())),
            Control::PatNotify => Some(Action::SetPatNotify(!model.pat_notify)),
            Control::ProtectGlowBerries => {
                Some(Action::SetProtectGlowBerries(!model.protect_glow_berries))
            }
            Control::ShowInvisibleEntities
                if !model.show_invisible_entities && !model.experiments_accepted =>
            {
                self.experiments_prompt = true;
                self.focus = None;
                None
            }
            Control::ShowInvisibleEntities => Some(Action::SetShowInvisibleEntities(
                !model.show_invisible_entities,
            )),
            Control::DrainRadius => {
                self.drain_radius = match self.drain_radius {
                    3 => 6,
                    6 => 9,
                    9 => 12,
                    _ => 3,
                };
                None
            }
            Control::DrainPattern => {
                self.drain_pattern = self.drain_pattern.next();
                None
            }
            Control::DrainSpacing => {
                self.drain_spacing = match self.drain_spacing {
                    3 => 4,
                    4 => 5,
                    5 => 6,
                    6 => 8,
                    _ => 3,
                };
                None
            }
            Control::SchematicChoice(index) => {
                if let Some(name) = model.schematic_files.get(index) {
                    self.schematic_file = Some(name.clone());
                }
                None
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
                self.focus = None;
                // The schematics folder is read again whenever its tab is opened.
                (mode == BaritoneMode::Schematic).then_some(Action::RefreshSchematics)
            }
            Control::MiningTarget(index) => {
                if usize::from(index) < super::claim_borders_mining::COUNT {
                    self.mining_targets ^= 1 << index;
                }
                None
            }
            Control::SchematicEdit(edit) => Some(Action::SchematicEdit(edit)),
            Control::SchematicPreviewHere => {
                let file = self.schematic_file.clone()?;
                let values = self.schematic_origin.split_whitespace().collect::<Vec<_>>();
                let origin = <[&str; 3]>::try_from(values.as_slice())
                    .ok()
                    .and_then(|v| super::claim_borders_waypoints::parse_coordinates(v).ok())
                    .map(|p| p.map(|v| v.floor() as i32));
                Some(Action::SchematicPreview {
                    file,
                    origin,
                    rotation: self.schematic_rotation,
                })
            }
            Control::FarmPlotPage(direction) => {
                self.farm_plot_page =
                    (self.farm_plot_page as isize + direction as isize).max(0) as usize;
                None
            }
            Control::SchematicPlacePage(direction) => {
                self.schematic_place_page =
                    (self.schematic_place_page as isize + direction as isize).max(0) as usize;
                None
            }
            Control::SchematicLayerMode => {
                let (mode, layer) = model.schematic_layers;
                Some(Action::SchematicLayers(mode.next(), layer))
            }
            Control::SchematicLayer(direction) => {
                let (mode, layer) = model.schematic_layers;
                let layer = (layer + i32::from(direction))
                    .clamp(0, super::claim_borders_schematic_places::MAX_LAYER);
                Some(Action::SchematicLayers(mode, layer))
            }
            Control::SchematicPlace(i) => Some(Action::SchematicPlace(i)),
            Control::SchematicPlaceShow(i) => Some(Action::SchematicPlaceShow(i)),
            Control::SchematicPlaceRemove(i) => Some(Action::SchematicPlaceRemove(i)),
            Control::SchematicFriendShow(i) => Some(Action::SchematicFriendShow(i)),
            Control::FarmPlotFarm(i) => Some(Action::FarmPlotFarm(i)),
            Control::FarmPlotShow(i) => Some(Action::FarmPlotShow(i)),
            Control::FarmPlotRemove(i) => Some(Action::FarmPlotRemove(i)),
            Control::MiningCustom => {
                // Picking a block selects the target too.
                self.mining_targets |= 1 << super::claim_borders_mining::CUSTOM;
                Some(Action::MiningCustom)
            }
            Control::MiningPage => {
                self.mining_page = self.mining_page.wrapping_add(1);
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
                self.helper_page = 0;
                self.focus = None;
                None
            }
            Control::HelperPage(page) => {
                self.helper_page = page;
                self.focus = None;
                None
            }
            Control::Language => Some(Action::SetLanguage(self.language.other())),
            Control::NavigationMenu => {
                self.navigation_menu = !self.navigation_menu;
                self.focus = None;
                None
            }
            Control::Appearance => Some(Action::Appearance),
            Control::EconomySetting(key, dir) => Some(Action::EconomySetting(key, dir)),
            Control::EconomyChoice(key) => {
                let setting = super::claim_borders_economy::SETTINGS
                    .iter()
                    .find(|setting| setting.key == key && setting.is_choice())?;
                let current = model.economy_preferences.get(key);
                let direction = if current >= setting.max {
                    (setting.min - current) / setting.step
                } else {
                    1
                };
                Some(Action::EconomySetting(key, direction))
            }
            Control::ToggleReconnect => Some(Action::ToggleReconnect),
            Control::TradingGroup(group) => {
                self.trading_group = group.min(4);
                self.settings_page = 0;
                self.focus = None;
                None
            }
            Control::SettingsTab(4) => {
                self.tab = Tab::Trading;
                self.settings_page = 0;
                self.focus = None;
                None
            }
            Control::SettingsTab(2) => {
                self.tab = Tab::Claims;
                self.claims_section = ClaimsSection::Map;
                self.settings_page = 0;
                self.focus = None;
                None
            }
            Control::SettingsTab(3) => {
                self.tab = Tab::Chat;
                self.settings_page = 0;
                self.focus = None;
                None
            }
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
                let sections = [0, 1, 5, 6];
                let index = sections
                    .iter()
                    .position(|s| *s == self.settings_section)
                    .unwrap_or(0);
                self.settings_section =
                    sections[(index as i8 + dir).rem_euclid(sections.len() as i8) as usize];
                self.settings_page = 0;
                self.focus = None;
                if self.settings_section == 1 {
                    self.relay_url = model.relay_url.clone();
                }
                None
            }
            Control::SettingsContentPage(page) => {
                self.settings_page = page;
                self.focus = None;
                None
            }
            Control::SettingsPage(dir) => {
                let reserve = if self.tab == Tab::Trading {
                    186.0
                } else {
                    160.0
                };
                let per = (((height - reserve) / 22.0).floor() as usize).max(1);
                let count = super::claim_borders_economy::SETTINGS
                    .iter()
                    .filter(|s| match self.settings_section {
                        _ if self.tab == Tab::Trading => s.group == self.trading_group,
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
                self.settings_page = 0;
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
                let per = Self::claims_per_page(height);
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
                self.settings_page = 0;
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
            | Control::ExperimentalRange(_)
            | Control::ExperimentalOreToggle(_) => {
                let mut p = model.experimental;
                match control {
                    Control::ExperimentalAllowed => p.allowed = !p.allowed,
                    Control::ExperimentalSource => p.prediction = !p.prediction,
                    Control::ExperimentalRange(radius) => p.prediction_radius = radius.min(2),
                    Control::ExperimentalOreToggle(kind)
                        if usize::from(kind) < experimental::ORES.len() =>
                    {
                        p.ores ^= 1 << kind;
                    }
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
                self.navigation_menu = false;
                self.experiments_prompt = true;
                self.focus = None;
                None
            }
            Control::Tab(t) => {
                self.navigation_menu = false;
                self.deepl_key.clear();
                self.tab = t;
                if t == Tab::Settings && matches!(self.settings_section, 2..=4) {
                    self.settings_section = 0;
                }
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
        // The Schematics tab: the wheel scrolls the file list a row at a time.
        if self.tab == Tab::Experimental
            && self.experimental_section == ExperimentalSection::Baritone
            && self.baritone_mode == BaritoneMode::Schematic
        {
            let rows = schematic_list_rows(SCHEMATIC_TOP, height - SCHEMATIC_BOTTOM);
            let last = model.schematic_files.len().saturating_sub(rows.max(1));
            self.schematic_catalog_page = if delta > 0.0 {
                self.schematic_catalog_page.min(last).saturating_sub(1)
            } else {
                (self.schematic_catalog_page + 1).min(last)
            };
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
            if self.navigation_menu {
                self.navigation_menu = false;
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
            if self.section() == 3 && self.chat_section == ChatSection::DeepL {
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
                } else if self.relay_setup || self.section() == 1 {
                    if self.focus == Some(Field::RelayUrl) {
                        Field::RelayToken
                    } else {
                        Field::RelayUrl
                    }
                } else if self.section() == 2 && self.claims_section == ClaimsSection::Colours {
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
    fn economy_switches_toggle_once_in_both_directions_and_keep_saved_preferences() {
        for language in [Language::Russian, Language::English] {
            for enabled in [false, true] {
                let values: serde_json::Map<String, serde_json::Value> =
                    super::super::claim_borders_economy::SETTINGS
                        .iter()
                        .filter(|setting| setting.is_toggle())
                        .map(|setting| (setting.key.into(), serde_json::json!(i64::from(enabled))))
                        .collect();
                let mut model = ViewModel {
                    language,
                    economy_preferences: super::super::claim_borders_economy::Preferences::decode(
                        &serde_json::json!({"schema":1,"values":values}).to_string(),
                    )
                    .unwrap(),
                    ..ViewModel::default()
                };
                for section in 4..7 {
                    for page in 0..32 {
                        let mut menu = Menu {
                            language,
                            tab: Tab::Settings,
                            settings_section: section,
                            settings_page: page,
                            ..Menu::default()
                        };
                        let widgets = menu.layout(&model, 800.0, 500.0);
                        for setting in super::super::claim_borders_economy::SETTINGS
                            .iter()
                            .filter(|s| s.is_toggle())
                        {
                            let Some(button) = widgets.iter().find(|w| {
                                w.control == Some(Control::EconomySetting(setting.key, 1))
                            }) else {
                                continue;
                            };
                            assert_eq!(
                                crate::ui_style::switch_label(&button.text).unwrap().1,
                                enabled
                            );
                            assert_eq!(button.selected, enabled);
                            assert!(!widgets.iter().any(
                                |w| w.control == Some(Control::EconomySetting(setting.key, -1))
                            ));
                            let r = button.rect;
                            let Some(Action::EconomySetting(key, dir)) = menu.click(
                                &model,
                                [800.0, 500.0],
                                [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5],
                            ) else {
                                panic!("switch must produce one configuration action")
                            };
                            let previous = model.economy_preferences.clone();
                            model.economy_preferences.adjust(key, dir);
                            assert_eq!(model.economy_preferences.flag(key), !enabled);
                            let saved = model.economy_preferences.encode();
                            assert_eq!(
                                super::super::claim_borders_economy::Preferences::decode(&saved)
                                    .unwrap(),
                                model.economy_preferences
                            );
                            model.economy_preferences = previous;
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn corner_choices_use_names_and_wrap_without_affecting_numeric_settings() {
        for key in ["corner", "sidebar_corner"] {
            let mut model = ViewModel::default();
            let original = model.economy_preferences.get(key);
            let width = model.economy_preferences.get("width");
            let mut menu = Menu::default();
            for _ in 0..4 {
                let current = model.economy_preferences.get(key);
                let Some(Action::EconomySetting(changed, direction)) =
                    menu.activate(&model, 400.0, Control::EconomyChoice(key))
                else {
                    panic!("corner choice must configure its existing key")
                };
                assert_eq!(changed, key);
                model.economy_preferences.adjust(changed, direction);
                assert_eq!(model.economy_preferences.get(key), (current + 1) % 4);
                assert_eq!(model.economy_preferences.get("width"), width);
            }
            assert_eq!(model.economy_preferences.get(key), original);
        }
        assert!(
            Menu::default()
                .activate(
                    &ViewModel::default(),
                    400.0,
                    Control::EconomyChoice("width")
                )
                .is_none()
        );
    }
    #[test]
    fn numeric_readouts_are_separate_from_labels_and_do_not_act_like_inputs() {
        let model = ViewModel::default();
        for language in [Language::Russian, Language::English] {
            for size in [[240.0, 216.0], [800.0, 500.0]] {
                for section in 4..7 {
                    for page in 0..32 {
                        let menu = Menu {
                            language,
                            tab: Tab::Settings,
                            settings_section: section,
                            settings_page: page,
                            ..Menu::default()
                        };
                        let widgets = menu.layout(&model, size[0], size[1]);
                        for readout in widgets.iter().filter(|widget| widget.readout) {
                            assert!(readout.control.is_none());
                            assert!(readout.rect[0] + readout.rect[2] <= size[0]);
                            assert!(readout.rect[1] + readout.rect[3] <= size[1]);
                            let stepper = widgets
                                .iter()
                                .find(|widget| {
                                    // The readout is inset within its row.
                                    widget.rect[1] <= readout.rect[1]
                                        && readout.rect[1] < widget.rect[1] + widget.rect[3]
                                        && matches!(
                                            widget.control,
                                            Some(Control::EconomySetting(_, -1))
                                        )
                                })
                                .unwrap();
                            let Some(Control::EconomySetting(key, _)) = stepper.control else {
                                unreachable!()
                            };
                            let setting = super::super::claim_borders_economy::SETTINGS
                                .iter()
                                .find(|s| s.key == key)
                                .unwrap();
                            assert!(!setting.is_toggle() && !setting.is_choice());
                            assert_eq!(
                                readout.text,
                                setting.value_label(model.economy_preferences.get(key), language)
                            );
                            assert!(readout.rect[0] + readout.rect[2] <= stepper.rect[0]);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn every_trading_player_and_hud_setting_can_be_adjusted_after_paging() {
        let model = ViewModel::default();
        for size in [[240.0, 216.0], [640.0, 360.0], [800.0, 500.0]] {
            for section in 4..7 {
                let mut menu = Menu {
                    tab: Tab::Settings,
                    settings_section: section,
                    ..Menu::default()
                };
                let mut found = std::collections::BTreeSet::new();
                loop {
                    let widgets = menu.layout(&model, size[0], size[1]);
                    for widget in &widgets {
                        if let Some(Control::EconomySetting(key, direction)) = widget.control {
                            found.insert((key, direction));
                            assert_eq!(widget.rect[3], 20.0);
                        }
                        if let Some(Control::EconomyChoice(key)) = widget.control {
                            found.insert((key, 0));
                        }
                    }
                    // A single page has no paging controls.
                    let Some(next) = widgets
                        .iter()
                        .find(|widget| widget.control == Some(Control::SettingsPage(1)))
                        .map(|widget| widget.rect)
                    else {
                        break;
                    };
                    let previous = menu.settings_page;
                    menu.click(
                        &model,
                        size,
                        [next[0] + next[2] / 2.0, next[1] + next[3] / 2.0],
                    );
                    if menu.settings_page == previous {
                        break;
                    }
                    assert!(menu.settings_page < 64);
                }
                for setting in super::super::claim_borders_economy::SETTINGS
                    .iter()
                    .filter(|s| match section {
                        4 => s.group <= 4,
                        5 => s.group == 6,
                        _ => s.group == 5,
                    })
                {
                    if setting.is_choice() {
                        assert!(
                            found.contains(&(setting.key, 0)),
                            "missing choice for {}",
                            setting.key
                        );
                        continue;
                    }
                    assert!(
                        found.contains(&(setting.key, -1)) != setting.is_toggle(),
                        "missing decrease for {}",
                        setting.key
                    );
                    assert!(
                        found.contains(&(setting.key, 1)),
                        "missing increase for {}",
                        setting.key
                    );
                }
            }
        }
    }
    #[test]
    fn nested_settings_toggles_are_localized_in_both_languages() {
        for chat_section in [
            ChatSection::Appearance,
            ChatSection::Notifications,
            ChatSection::Spam,
            ChatSection::Priority,
        ] {
            for claims_section in [
                ClaimsSection::Map,
                ClaimsSection::Borders,
                ClaimsSection::Colours,
                ClaimsSection::Storage,
            ] {
                for section in [2, 3] {
                    for page in 0..32 {
                        let ru = Menu {
                            tab: Tab::Settings,
                            settings_section: section,
                            settings_page: page,
                            chat_section,
                            claims_section,
                            ..Menu::default()
                        };
                        let en = Menu {
                            language: Language::English,
                            ..ru.clone()
                        };
                        let english = en.layout(&ViewModel::default(), 800.0, 500.0);
                        for widget in ru.layout(&ViewModel::default(), 800.0, 500.0) {
                            if let Some((name, on)) = crate::ui_style::switch_label(&widget.text) {
                                assert!(
                                    name.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)),
                                    "{name}"
                                );
                                let counterpart = english
                                    .iter()
                                    .find(|w| w.control == widget.control)
                                    .unwrap();
                                let (name, state) =
                                    crate::ui_style::switch_label(&counterpart.text).unwrap();
                                assert!(name.is_ascii(), "{name}");
                                assert_eq!(on, state);
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn keyboard_focus_reveals_a_field_on_a_different_settings_page() {
        let model = ViewModel::default();
        for (section, chat_section, fields) in [
            (
                1,
                ChatSection::Appearance,
                vec![Field::RelayUrl, Field::RelayToken],
            ),
            (3, ChatSection::DeepL, vec![Field::DeepLKey]),
            (
                2,
                ChatSection::Appearance,
                vec![Field::OwnClaimColor, Field::FriendClaimColor],
            ),
        ] {
            let mut menu = Menu {
                tab: Tab::Settings,
                settings_section: section,
                chat_section,
                claims_section: ClaimsSection::Colours,
                settings_page: 30,
                ..Menu::default()
            };
            for field in fields {
                menu.key("key.keyboard.tab", 0);
                assert_eq!(menu.focus, Some(field));
                assert!(
                    menu.layout(&model, 240.0, 216.0)
                        .iter()
                        .any(|w| w.control == Some(Control::Field(field)) && w.selected)
                );
            }
        }
    }
    #[test]
    fn redesigned_settings_keep_every_editor_control_readable_and_reachable() {
        let model = ViewModel::default();
        for language in [Language::Russian, Language::English] {
            for size in [
                [240.0, 216.0],
                [320.0, 240.0],
                [536.0, 216.0],
                [800.0, 500.0],
            ] {
                for section in 0..4 {
                    for chat_section in [
                        ChatSection::Appearance,
                        ChatSection::Notifications,
                        ChatSection::Spam,
                        ChatSection::DeepL,
                        ChatSection::Priority,
                    ] {
                        for claims_section in [
                            ClaimsSection::Map,
                            ClaimsSection::Borders,
                            ClaimsSection::Colours,
                            ClaimsSection::Storage,
                        ] {
                            // Map and chat preferences are reached from their own tabs.
                            let mut menu = Menu {
                                language,
                                tab: match section {
                                    2 => Tab::Claims,
                                    3 => Tab::Chat,
                                    _ => Tab::Settings,
                                },
                                settings_section: section,
                                chat_section,
                                claims_section,
                                ..Menu::default()
                            };
                            let mut found = Vec::new();
                            loop {
                                let widgets = menu.layout(&model, size[0], size[1]);
                                let controls: Vec<_> =
                                    widgets.iter().filter(|w| w.control.is_some()).collect();
                                for (i, a) in controls.iter().enumerate() {
                                    assert!(
                                        a.rect[3] >= 18.0,
                                        "unreadable control: {:?}",
                                        a.control
                                    );
                                    assert!(
                                        a.rect[0] >= 0.0
                                            && a.rect[1] >= 0.0
                                            && a.rect[0] + a.rect[2] <= size[0]
                                            && a.rect[1] + a.rect[3] <= size[1]
                                    );
                                    for b in &controls[i + 1..] {
                                        assert!(
                                            a.rect[0] + a.rect[2] <= b.rect[0]
                                                || b.rect[0] + b.rect[2] <= a.rect[0]
                                                || a.rect[1] + a.rect[3] <= b.rect[1]
                                                || b.rect[1] + b.rect[3] <= a.rect[1],
                                            "overlap at {size:?}, {section}, {chat_section:?}, {claims_section:?}: {:?} and {:?}",
                                            a.control,
                                            b.control
                                        );
                                    }
                                    found.push(a.control.unwrap());
                                }
                                let next = widgets
                                    .iter()
                                    .filter_map(|w| match w.control {
                                        Some(Control::SettingsContentPage(page))
                                            if page > menu.settings_page =>
                                        {
                                            Some(page)
                                        }
                                        _ => None,
                                    })
                                    .max();
                                let Some(next) = next else { break };
                                assert!(next < 64);
                                menu.activate(
                                    &model,
                                    size[1] - 16.0,
                                    Control::SettingsContentPage(next),
                                );
                            }
                            let raw = match section {
                                0 => vec![
                                    Control::Appearance,
                                    Control::ToggleReconnect,
                                    Control::ToggleShulkerPeek,
                                    Control::PatPolicy,
                                    Control::PatNotify,
                                    Control::ProtectGlowBerries,
                                    Control::ShowInvisibleEntities,
                                ],
                                1 => {
                                    menu.relay_setup = true;
                                    let mut controls: Vec<_> = menu
                                        .party_layout(&model, 480.0, 400.0, Vec::new())
                                        .into_iter()
                                        .filter(|w| w.rect[1] < 212.0)
                                        .filter_map(|w| w.control)
                                        .filter(|c| *c != Control::RelayBack)
                                        .collect();
                                    controls.push(Control::ToggleAutoTrust);
                                    controls
                                }
                                2 => menu
                                    .claims_layout(&model, 0.0, 480.0, 400.0, Vec::new())
                                    .into_iter()
                                    .filter_map(|w| w.control)
                                    .collect(),
                                _ => menu
                                    .chat_layout(&model, 0.0, 480.0, 400.0, Vec::new())
                                    .into_iter()
                                    .filter_map(|w| w.control)
                                    .collect(),
                            };
                            for control in raw {
                                assert!(found.contains(&control), "lost control: {control:?}");
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn glow_berry_protection_switches_both_ways_and_defaults_off() {
        let mut menu = Menu::default();
        let mut model = ViewModel::default();
        assert!(!model.protect_glow_berries);
        assert!(matches!(
            menu.activate(&model, 400.0, Control::ProtectGlowBerries),
            Some(Action::SetProtectGlowBerries(true))
        ));
        model.protect_glow_berries = true;
        assert!(matches!(
            menu.activate(&model, 400.0, Control::ProtectGlowBerries),
            Some(Action::SetProtectGlowBerries(false))
        ));
    }
    #[test]
    fn invisible_entities_switches_both_ways_and_defaults_off() {
        for language in [Language::English, Language::Russian] {
            let mut menu = Menu {
                language,
                ..Menu::default()
            };
            let mut model = ViewModel::default();
            assert!(!model.show_invisible_entities);
            assert_eq!(
                menu.activate(&model, 400.0, Control::ShowInvisibleEntities),
                None
            );
            assert!(menu.experiments_prompt);
            menu.experiments_prompt = false;
            model.experiments_accepted = true;
            assert_eq!(
                menu.activate(&model, 400.0, Control::ShowInvisibleEntities),
                Some(Action::SetShowInvisibleEntities(true))
            );
            model.show_invisible_entities = true;
            assert_eq!(
                menu.activate(&model, 400.0, Control::ShowInvisibleEntities),
                Some(Action::SetShowInvisibleEntities(false))
            );
        }
    }
    #[test]
    fn toggle_captions_have_matching_russian_and_english_controls() {
        let model = ViewModel::default();
        assert_eq!(Menu::default().language, Language::Russian);
        let mut count = 0;
        for section in 0..7 {
            for page in 0..32 {
                for tab in [Tab::Settings, Tab::Experimental, Tab::Chat] {
                    for experimental_section in [
                        ExperimentalSection::Ores,
                        ExperimentalSection::Baritone,
                        ExperimentalSection::Helpers,
                    ] {
                        for baritone_mode in [
                            BaritoneMode::Navigation,
                            BaritoneMode::Mining,
                            BaritoneMode::Tunnel,
                            BaritoneMode::Bridge,
                            BaritoneMode::Farm,
                            BaritoneMode::Drain,
                            BaritoneMode::Schematic,
                        ] {
                            let ru = Menu {
                                tab,
                                settings_section: section,
                                settings_page: page,
                                experimental_section,
                                baritone_mode,
                                language: Language::Russian,
                                ..Menu::default()
                            };
                            let en = Menu {
                                language: Language::English,
                                ..ru.clone()
                            };
                            let english = en.layout(&model, 800.0, 500.0);
                            for widget in ru.layout(&model, 800.0, 500.0) {
                                let Some((russian, state)) =
                                    crate::ui_style::switch_label(&widget.text)
                                else {
                                    continue;
                                };
                                let counterpart = english
                                    .iter()
                                    .find(|w| w.control == widget.control)
                                    .unwrap();
                                let (name, english_state) =
                                    crate::ui_style::switch_label(&counterpart.text).unwrap();
                                assert_eq!(state, english_state);
                                assert!(
                                    russian
                                        .chars()
                                        .any(|c| ('\u{400}'..='\u{4ff}').contains(&c)),
                                    "untranslated: {russian}"
                                );
                                assert!(name.is_ascii(), "untranslated: {name}");
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(count > 100);
    }
    fn reveal_control(
        menu: &mut Menu,
        model: &ViewModel,
        size: [f32; 2],
        control: Control,
    ) -> Widget {
        menu.settings_page = 0;
        for _ in 0..64 {
            let widgets = menu.layout(model, size[0], size[1]);
            if let Some(widget) = widgets
                .iter()
                .find(|widget| widget.control == Some(control))
            {
                return widget.clone();
            }
            let next = widgets
                .iter()
                .filter_map(|widget| match widget.control {
                    Some(Control::SettingsContentPage(page)) if page > menu.settings_page => {
                        Some(page)
                    }
                    _ => None,
                })
                .max()
                .expect("requested control must remain reachable through the editor pages");
            menu.activate(model, size[1] - 16.0, Control::SettingsContentPage(next));
        }
        panic!("settings pagination must be bounded");
    }
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
            let [x, y, w, h] = reveal_control(menu, &model, size, control).rect;
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
            if (size[0] - 16.0).min(720.0) >= RAIL_BREAKPOINT {
                assert_eq!(settings.rect[0], x);
                assert!(settings.rect[1] + settings.rect[3] <= y);
                assert!(
                    settings.rect[2] >= 100.0,
                    "settings must have a visible label"
                );
            } else {
                assert_eq!(settings.rect[1], y);
                assert!(settings.rect[0] + settings.rect[2] <= x);
            }
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
        for height in [200.0, 216.0, 240.0, 256.0, 280.0, 300.0, 500.0] {
            let rows = menu.layout(&model, 360.0, height);
            let controls: Vec<_> = rows
                .iter()
                .filter(|r| {
                    matches!(
                        r.control,
                        Some(
                            Control::ExperimentalAllowed
                                | Control::ExperimentalSource
                                | Control::ExperimentalRange(_)
                                | Control::ExperimentalOreToggle(_)
                        )
                    )
                })
                .collect();
            assert_eq!(controls.len(), 5 + experimental::ORES.len());
            for (i, a) in controls.iter().enumerate() {
                for b in &controls[i + 1..] {
                    assert!(
                        a.rect[1] + a.rect[3] <= b.rect[1] || a.rect[0] + a.rect[2] <= b.rect[0],
                        "{:?} overlaps {:?}",
                        a.rect,
                        b.rect
                    );
                }
            }
            let hint = rows.iter().find(|r| r.text.starts_with("O —")).unwrap();
            for tile in controls.iter().skip(2) {
                assert!(tile.rect[2] >= 16.0 && tile.rect[0] + tile.rect[2] <= 360.0);
                assert!(tile.rect[1] + tile.rect[3] <= hint.rect[1]);
            }
        }
    }

    #[test]
    fn ore_tiles_toggle_their_own_bit_and_ignore_unknown_ores() {
        let mut menu = Menu {
            tab: Tab::Experimental,
            ..Menu::default()
        };
        let model = ViewModel::default();
        let before = model.experimental.ores;
        for kind in 0..experimental::ORES.len() as u8 {
            let Some(Action::SetExperimental(p)) =
                menu.activate(&model, 360.0, Control::ExperimentalOreToggle(kind))
            else {
                panic!("tile {kind} produced no preference change");
            };
            assert_eq!(p.ores, before ^ (1 << kind));
        }
        let Some(Action::SetExperimental(p)) =
            menu.activate(&model, 360.0, Control::ExperimentalOreToggle(200))
        else {
            panic!("unknown tile should leave preferences as they are");
        };
        assert_eq!(p.ores, before);
    }

    #[test]
    fn seed_prediction_range_buttons_save_each_choice_in_both_languages() {
        for language in [Language::Russian, Language::English] {
            let mut menu = Menu {
                tab: Tab::Experimental,
                language,
                ..Menu::default()
            };
            let mut model = ViewModel::default();
            model.experimental.prediction = true;
            for radius in [1, 2, 0] {
                let rows = menu.layout(&model, 360.0, 200.0);
                let row = rows
                    .iter()
                    .find(|row| row.control == Some(Control::ExperimentalRange(radius)))
                    .unwrap();
                let mouse = [
                    row.rect[0] + row.rect[2] * 0.5,
                    row.rect[1] + row.rect[3] * 0.5,
                ];
                let Some(Action::SetExperimental(p)) = menu.click(&model, [360.0, 200.0], mouse)
                else {
                    panic!("range click did not save preferences")
                };
                assert_eq!(p.prediction_radius, radius);
                assert_eq!(p.ores, model.experimental.ores);
                assert_eq!(p.allowed, model.experimental.allowed);
                assert!(p.prediction);
                model.experimental = p;
                let rows = menu.layout(&model, 360.0, 200.0);
                assert!(rows.iter().any(|row| row.control
                    == Some(Control::ExperimentalRange(radius))
                    && row.selected));
            }
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
            for section in [
                ExperimentalSection::Baritone,
                ExperimentalSection::Ores,
                ExperimentalSection::Helpers,
            ] {
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
                        limit: 0
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
        let selector = m
            .layout(&v, 400.0, 280.0)
            .into_iter()
            .find(|w| w.control == Some(Control::NavigationMenu))
            .unwrap()
            .rect;
        m.click(&v, [400.0, 280.0], [selector[0] + 1.0, selector[1] + 1.0]);
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
            let button = reveal_control(
                &mut menu,
                &model,
                [320.0, 240.0],
                Control::ChatCompactMessages,
            );
            assert!(button.text.starts_with(caption));
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
        for section in [0, 1, 5, 6] {
            menu.tab = Tab::Settings;
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
    fn auto_eat_filter_uses_mining_sized_tiles_and_all_foods_are_reachable() {
        let mut model = ViewModel {
            auto_eat_options: (0..45)
                .map(|i| (format!("minecraft:food_{i}"), format!("Food {i}"), 4))
                .collect(),
            ..Default::default()
        };
        for language in [Language::English, Language::Russian] {
            for (width, height) in [(360.0, 200.0), (640.0, 400.0), (720.0, 300.0)] {
                let mut menu = Menu {
                    tab: Tab::Experimental,
                    experimental_section: ExperimentalSection::Helpers,
                    language,
                    ..Default::default()
                };
                assert!(
                    menu.helpers_layout(&model, 100.0, width - 110.0, height, Vec::new())
                        .iter()
                        .any(|w| w.control == Some(Control::AutoEatFoods))
                );
                menu.activate(&model, height, Control::AutoEatFoods);
                let mut seen = std::collections::BTreeSet::new();
                for page in 0..45 {
                    menu.activate(&model, height, Control::AutoEatFoodPage(page));
                    let widgets =
                        menu.helpers_layout(&model, 100.0, width - 110.0, height, Vec::new());
                    for w in widgets
                        .iter()
                        .filter(|w| matches!(w.control, Some(Control::AutoEatFood(_))))
                    {
                        let Some(Control::AutoEatFood(index)) = w.control else {
                            unreachable!()
                        };
                        seen.insert(index);
                        assert_eq!([w.rect[2], w.rect[3]], [22.0, 22.0]);
                        assert!(w.rect[0] >= 100.0 && w.rect[0] + w.rect[2] <= width);
                        assert!(w.rect[1] >= 128.0 && w.rect[1] + w.rect[3] <= height - 48.0);
                        assert_eq!(
                            w.selected,
                            model
                                .auto_eat_foods
                                .contains(&model.auto_eat_options[index].0)
                        );
                    }
                }
                assert_eq!(seen.len(), model.auto_eat_options.len());
                assert_eq!(
                    menu.activate(&model, height, Control::AutoEatFood(0)),
                    Some(Action::SetAutoEatFood("minecraft:food_0".into(), true))
                );
                model.auto_eat_foods = vec!["minecraft:food_0".into()];
                assert_eq!(
                    menu.activate(&model, height, Control::AutoEatFood(0)),
                    Some(Action::SetAutoEatFood("minecraft:food_0".into(), false))
                );
                model.auto_eat_foods.clear();
                assert_eq!(
                    menu.activate(&model, height, Control::AutoEatFood(999)),
                    None
                );
                menu.activate(&model, height, Control::AutoEatFoods);
                assert!(!menu.auto_eat_food_picker);
            }
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
        menu.activate(&model, 200.0, Control::DrainPattern);
        assert_eq!(
            menu.drain_pattern,
            super::super::claim_borders_draining::Pattern::Lines
        );
        menu.activate(&model, 200.0, Control::DrainSpacing);
        assert_eq!(menu.drain_spacing, 5);
        menu.drain_pattern = super::super::claim_borders_draining::Pattern::Fill;
        menu.drain_spacing = 4;
        for _ in 0..3 {
            menu.activate(&model, 200.0, Control::DrainRadius);
        }
        assert_eq!(menu.drain_radius, 6);
        menu.baritone_mode = BaritoneMode::Schematic;
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
                                Control::FarmCropPick(_)
                                    | Control::FarmStorage(_)
                                    | Control::FarmSafe
                                    | Control::FarmBonemeal
                                    | Control::FarmAutoSell
                                    | Control::FarmLevels
                                    | Control::FarmRadius
                                    | Control::FarmPlotFarm(_)
                                    | Control::FarmPlotShow(_)
                                    | Control::FarmPlotRemove(_)
                                    | Control::FarmReplant
                                    | Control::DrainRadius
                                    | Control::DrainPattern
                                    | Control::DrainSpacing
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
