use claim_borders_relay_client as websocket;
// Horizon-only adapters. Pure models are compiled independently by Cargo tests.
use crate::game::{
    Game, claim_borders as claims, claim_borders_menu as menu,
    claim_borders_menu_view as menu_view, claim_borders_overlay as overlay,
    claim_borders_party as party, claim_borders_relay_settings as relay_settings,
    claim_borders_visualization as visualization, claim_borders_waypoints as waypoints,
    native_hooks::NativeMod,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use claim_borders_relay_client::translation::Language;

pub(super) fn native_mod(language: Language) -> NativeMod {
    NativeMod {
        id: claims::ID.into(),
        title: format!("{} {}", claims::NAME, claims::VERSION),
        description: claims::description_in(language).into(),
        keys: vec![
            [
                "ore_experiment".into(),
                language
                    .text("Эксперименты: подсветка руд", "Experiments: ore highlight")
                    .into(),
                crate::game::claim_borders_experimental::DEFAULT_KEY.into(),
            ],
            [
                "trades".into(),
                language
                    .text("DraimHorizons: Торги", "DraimHorizons: Trades")
                    .into(),
                "key.keyboard.b".into(),
            ],
            [
                "waypoint".into(),
                language
                    .text(
                        "DraimHorizon: метка / координаты",
                        "DraimHorizon: waypoint / coordinates",
                    )
                    .into(),
                "key.keyboard.x".into(),
            ],
            [
                "map".into(),
                language
                    .text("Seed Map · три измерения", "Seed Map · three dimensions")
                    .into(),
                "key.keyboard.m".into(),
            ],
            [
                "menu".into(),
                language
                    .text("DraimHorizons: открыть меню", "DraimHorizons: open menu")
                    .into(),
                "key.keyboard.j".into(),
            ],
            [
                "zoom".into(),
                language
                    .text("DraimHorizons: зум камеры", "DraimHorizons: camera zoom")
                    .into(),
                crate::game::claim_borders_camera_zoom::DEFAULT_KEY.into(),
            ],
        ]
        .into_iter()
        .chain(
            crate::game::claim_borders_navigation::Hotkey::ALL.map(|action| {
                [
                    action.id().into(),
                    action.label(language).into(),
                    action.preset().into(),
                ]
            }),
        )
        .collect(),
    }
}

fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}
/// Localized `format!` for text created outside a `Game` context (Russian / English).
macro_rules! tr {
    ($russian:literal, $english:literal $(, $arg:expr)* $(,)?) => {
        match language() {
            Language::Russian => format!($russian $(, $arg)*),
            Language::English => format!($english $(, $arg)*),
        }
    };
}

/// Relay status kept language-neutral so switching language re-renders it.
#[derive(Clone)]
enum NetStatus {
    Off,
    Connecting,
    Connected,
    Lost(String),
    Text(String),
}
impl NetStatus {
    fn text(&self) -> String {
        match self {
            Self::Off => tr!("Relay отключён", "Relay disconnected"),
            Self::Connecting => tr!(
                "Подключение к WebSocket relay…",
                "Connecting to WebSocket relay…"
            ),
            Self::Connected => tr!("WebSocket подключён · E2E", "WebSocket connected · E2E"),
            Self::Lost(reason) => tr!(
                "Нет связи с relay: {reason}",
                "No relay connection: {reason}"
            ),
            Self::Text(text) => text.clone(),
        }
    }
}
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq)]
enum Relay {
    Off,
    WebSocket,
}

struct Client {
    reconnect_initialized: bool,
    scope: String,
    own: u128,
    root: PathBuf,
    waypoints: Option<waypoints::WaypointStore>,
    party: Option<party::Party>,
    relay: Relay,
    started: Instant,
    last_send: u64,
    last_status: BTreeMap<u128, u64>,
    last_snapshots: BTreeMap<u128, party::Snapshot>,
    status_cursor: usize,
    last_hello: BTreeMap<u128, u64>,
    last_claims: BTreeMap<u128, u64>,
    claim_revision: u64,
    own_claims: Option<Vec<claims::Claim>>,
    controls: VecDeque<party::Outbound>,
    pages: VecDeque<party::Outbound>,
    network: Option<websocket::Handle>,
    network_settings: Option<relay_settings::Settings>,
    pins: Option<relay_settings::PinStore>,
    connected: bool,
    own_code: String,
    auto_relay: bool,
    manual_off: bool,
    last_auto: u64,
    avatars: BTreeMap<u128, String>,
    network_status: NetStatus,
    peers: Vec<websocket::Peer>,
    pending_codes: BTreeMap<u128, String>,
    cached_territories: Vec<claims::Claim>,
    cached_at: u64,
    tab_cache: Vec<(u128, String)>,
    tab_at: u64,
    share_warning: bool,
    visual: visualization::Settings,
    visual_writable: bool,
    menu_feedback: String,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            reconnect_initialized: false,
            scope: String::new(),
            own: 0,
            root: PathBuf::new(),
            waypoints: None,
            party: None,
            relay: Relay::Off,
            started: Instant::now(),
            last_send: 0,
            last_status: BTreeMap::new(),
            last_snapshots: BTreeMap::new(),
            status_cursor: 0,
            last_hello: BTreeMap::new(),
            last_claims: BTreeMap::new(),
            claim_revision: 1,
            own_claims: None,
            controls: VecDeque::new(),
            pages: VecDeque::new(),
            network: None,
            network_settings: None,
            pins: None,
            connected: false,
            own_code: String::new(),
            auto_relay: true,
            manual_off: false,
            last_auto: 0,
            avatars: BTreeMap::new(),
            network_status: NetStatus::Off,
            peers: Vec::new(),
            pending_codes: BTreeMap::new(),
            cached_territories: Vec::new(),
            cached_at: 0,
            tab_cache: Vec::new(),
            tab_at: 0,
            share_warning: false,
            visual: visualization::Settings::default(),
            visual_writable: true,
            menu_feedback: String::new(),
        }
    }
}

fn state<T>(f: impl FnOnce(&mut Client) -> T) -> T {
    static CLIENT: OnceLock<Mutex<Client>> = OnceLock::new();
    let mut client = CLIENT
        .get_or_init(|| Mutex::new(Client::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    f(&mut client)
}

pub(super) fn scope(game: &Game<'_>) -> Option<(String, u128)> {
    game.native_player()?;
    if let Some(remote) = &game.remote {
        let own = remote.profile();
        Some((
            format!("{}#{own:032x}", remote.session.address.to_ascii_lowercase()),
            own,
        ))
    } else if !game.world_root.as_os_str().is_empty() {
        Some((format!("world:{}", game.world_root.to_string_lossy()), 0))
    } else {
        None
    }
}

fn party_path(client: &Client) -> PathBuf {
    // File name has no server-controlled path components; the file also verifies scope.
    let hash = client.scope.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    client
        .root
        .join(format!("party-websocket-{hash:016x}.json"))
}

fn ensure(game: &mut Game<'_>) -> bool {
    let Some((scope, own)) = scope(game) else {
        return false;
    };
    let unchanged = state(|c| c.scope == scope);
    if unchanged {
        return true;
    }
    let root = match Game::native_config_dir() {
        Ok(p) => p.join("claim_borders"),
        Err(error) => {
            game.native_chat(&format!("[Horizon] Config error: {error}"));
            return false;
        }
    };
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut client = Client {
        scope,
        own,
        root,
        auto_relay: game
            .native_setting(claims::SETTINGS_ID, "relay_autoconnect")
            .as_deref()
            != Some("false"),
        ..Client::default()
    };
    let mut errors = Vec::new();
    match waypoints::WaypointStore::load(&client.root.join("waypoints.json")) {
        Ok(store) => client.waypoints = Some(store),
        Err(error) => errors.push(format!("Waypoint storage disabled: {error}")),
    }
    match visualization::Settings::load(&client.root.join("visualization.json")) {
        Ok(settings) => client.visual = settings,
        Err(error) => {
            client.visual_writable = false;
            errors.push(format!(
                "Visualization uses defaults; saved file needs repair: {error}"
            ));
        }
    }
    match relay_settings::Settings::load(&client.root.join("relay.json")) {
        Ok(settings) => client.network_settings = Some(settings),
        Err(error) => errors.push(format!("Relay configuration disabled: {error}")),
    }
    match relay_settings::PinStore::load(&client.root.join("relay-contacts.json")) {
        Ok(pins) => client.pins = Some(pins),
        Err(error) => errors.push(format!("Relay contacts disabled: {error}")),
    }
    if game.remote.is_some() {
        match party::Party::load(&party_path(&client), &client.scope, seed) {
            Ok(p) => client.party = Some(p),
            Err(error) => errors.push(format!("Party storage disabled: {error}")),
        }
    }
    state(|c| *c = client);
    for error in errors {
        game.native_chat(&format!("[Horizon] {error}"));
    }
    true
}

fn now(c: &Client) -> u64 {
    c.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn online(game: &mut Game<'_>) -> Vec<(u128, String)> {
    game.remote_tab_players()
        .map(|(players, _, _)| players.into_iter().map(|p| (p.uuid, p.name)).collect())
        .unwrap_or_default()
}

fn cached_online(game: &mut Game<'_>) -> Vec<(u128, String)> {
    if let Some(tab) = state(|c| {
        (!c.tab_cache.is_empty() && now(c).saturating_sub(c.tab_at) < 1000)
            .then(|| c.tab_cache.clone())
    }) {
        return tab;
    }
    let tab = online(game);
    state(|c| {
        c.tab_at = now(c);
        c.tab_cache = tab.clone();
    });
    tab
}

pub(super) fn reset() {
    menu_view::reset();
    state(|c| *c = Client::default());
}

pub(super) fn frame(game: &mut Game<'_>) {
    if game.native_on(claims::ID) && !state(|c| c.reconnect_initialized) {
        state(|c| c.reconnect_initialized = true);
        if game
            .native_setting(claims::SETTINGS_ID, "auto_reconnect_initialized")
            .as_deref()
            != Some("true")
        {
            game.settings.extras.auto_reconnect = true;
            game.settings.save(&Game::options_path());
            game.native_set_setting(claims::SETTINGS_ID, "auto_reconnect_initialized", "true");
        }
    }
    crate::game::claim_borders_chat_view::frame(game);
    crate::game::claim_borders_ore_view::frame(game);
    crate::game::claim_borders_auto_trust_view::frame(game);
    crate::game::claim_borders_economy_client::frame(game);
    crate::game::claim_borders_outgoing_view::frame(game);
    crate::game::claim_borders_teleport_view::frame(game);
    crate::game::claim_borders_claim_memory::frame(game);
    crate::game::claim_borders_claim_observations_view::frame(game);
    service_menu(game);
    crate::game::claim_borders_waypoint_view::frame(game);
    crate::game::claim_borders_inventory_search_view::frame(game);
    crate::game::claim_borders_inventory_sort_view::frame(game);
    if !game.native_on(claims::ID) {
        crate::game::claim_borders_map_view::reset();
        return;
    }
    if game.native_on(claims::ID) && game.native_player().is_some() && ensure(game) {
        if game.remote.is_some() {
            let auto = state(|c| {
                c.auto_relay
                    && !c.manual_off
                    && c.network.is_none()
                    && c.network_settings
                        .as_ref()
                        .is_some_and(|s| !s.url.is_empty())
                    && (c.last_auto == 0 || now(c).saturating_sub(c.last_auto) >= 3000)
            });
            if auto {
                state(|c| c.last_auto = now(c).max(1));
                if let Err(error) = start_network(game) {
                    state(|c| c.network_status = NetStatus::Text(error));
                }
            }
            poll_network(game);
        }
        prepare_avatars(game);
        let source = map_snapshot(game);
        crate::game::claim_borders_map_view::sync(game, source);
    }
    for (dimension, position) in crate::game::claim_borders_map_view::take_points() {
        if let Err(e) = add_map_waypoint(game, &dimension, position) {
            game.native_chat(&e);
        }
    }
    if crate::game::claim_borders_map_view::take_close() {
        game.native_mod_screen_close(claims::ID);
    }
}
/// The tab-list name of an online player.
pub(super) fn online_name(game: &mut Game<'_>, uuid: u128) -> Option<String> {
    cached_online(game)
        .into_iter()
        .find(|(id, _)| *id == uuid)
        .map(|(_, name)| name)
}
pub(super) fn entity_position(game: &Game<'_>, uuid: u128) -> Option<[f64; 3]> {
    let entity = game.remote.as_ref()?.player_entity(uuid)?;
    let mob = game.mobs.iter().find(|m| m.id == entity && !m.removed)?;
    let pos = mob.previous.lerp(mob.position, game.clock.fraction());
    Some([pos.x, pos.y, pos.z])
}
pub(super) fn avatar(uuid: u128) -> String {
    state(|c| c.avatars.get(&uuid).cloned())
        .unwrap_or_else(|| horizon_client::tab_list::default_skin(uuid).into())
}
fn prepare_avatars(game: &mut Game<'_>) {
    let (own, important): (u128, BTreeSet<_>) = state(|c| {
        let mut ids: BTreeSet<_> = c
            .party
            .as_ref()
            .map(|p| p.friends().into_iter().map(|f| f.uuid).collect())
            .unwrap_or_default();
        ids.insert(c.own);
        ids.extend(c.peers.iter().map(|p| p.uuid));
        (c.own, ids)
    });
    let mut players = game
        .remote_tab_players()
        .map(|(p, _, _)| p)
        .unwrap_or_default();
    let tab = players.iter().map(|p| (p.uuid, p.name.clone())).collect();
    players.sort_by_key(|p| (p.uuid != own, !important.contains(&p.uuid), p.uuid));
    let mut avatars = BTreeMap::new();
    for p in players
        .into_iter()
        .filter(|p| {
            important.contains(&p.uuid)
                || game
                    .remote
                    .as_ref()
                    .and_then(|r| r.player_entity(p.uuid))
                    .is_some()
        })
        .take(128)
        .collect::<Vec<_>>()
    {
        let skin = game.face_skin(p.uuid, &p.skin);
        avatars.insert(p.uuid, skin);
    }
    state(|c| {
        c.avatars = avatars;
        c.tab_cache = tab;
        c.tab_at = now(c);
    });
}
pub(super) fn map_snapshot(game: &Game<'_>) -> crate::game::claim_borders_map::Snapshot {
    use crate::game::claim_borders_map as map;
    let Some(player) = game.native_player() else {
        return map::Snapshot::default();
    };
    let dimension = if player.dimension.contains(':') {
        player.dimension.to_owned()
    } else {
        format!("minecraft:{}", player.dimension)
    };
    let own = claims::with_state(|s| s.shared_claims())
        .or_else(crate::game::claim_borders_claim_memory::own);
    let known = own.is_some();
    state(|c| {
        let mut cells: Vec<_> = own
            .unwrap_or_default()
            .into_iter()
            .map(|claim| map::Cell {
                dimension: claim.dimension,
                x: claim.x,
                z: claim.z,
                owner: tr!("Вы", "You"),
                reporter: None,
                own: true,
                foreign: false,
            })
            .collect();
        let mut marks = Vec::new();
        if let Some(store) = &c.waypoints {
            for w in store.all(&c.scope) {
                marks.push(map::Mark {
                    player: None,
                    friend: false,
                    skin: String::new(),
                    dimension: w.dimension.clone(),
                    position: [w.position[0], w.position[2]],
                    label: w.name.clone(),
                    color: w.color,
                });
            }
        }
        let online: Vec<_> = c
            .peers
            .iter()
            .filter(|peer| {
                c.tab_cache
                    .iter()
                    .any(|(uuid, name)| *uuid == peer.uuid && *name == peer.name)
                    && c.pins.as_ref().and_then(|p| p.get(&c.scope, peer.uuid))
                        == Some(peer.code.as_str())
            })
            .map(|peer| peer.uuid)
            .collect();
        if let Some(p) = &c.party {
            for friend in p.online(&online, now(c)) {
                if !crate::game::claim_borders_claim_memory::known(friend.uuid) {
                    cells.extend(
                        friend
                            .claims
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|c| map::Cell {
                                dimension: c.dimension,
                                x: c.x,
                                z: c.z,
                                owner: friend.name.clone(),
                                reporter: None,
                                own: false,
                                foreign: false,
                            }),
                    );
                }
                let native = entity_position(game, friend.uuid);
                marks.push(map::Mark {
                    player: Some(friend.uuid),
                    friend: true,
                    skin: c.avatars.get(&friend.uuid).cloned().unwrap_or_else(|| {
                        horizon_client::tab_list::default_skin(friend.uuid).into()
                    }),
                    dimension: if native.is_some() {
                        dimension.clone()
                    } else {
                        friend.snapshot.dimension.clone()
                    },
                    position: native.map_or(
                        [friend.snapshot.position[0], friend.snapshot.position[2]],
                        |p| [p[0], p[2]],
                    ),
                    label: friend.name.clone(),
                    color: c.visual.friend_color,
                });
            }
        }
        let friend_names: std::collections::BTreeSet<_> = c
            .party
            .as_ref()
            .map(|p| {
                p.friends()
                    .into_iter()
                    .map(|f| f.name.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        for (owner, land) in crate::game::claim_borders_claim_memory::others() {
            cells.extend(land.into_iter().map(|claim| map::Cell {
                dimension: claim.dimension,
                x: claim.x,
                z: claim.z,
                owner: owner.clone(),
                own: false,
                foreign: !friend_names.contains(&owner.to_ascii_lowercase()),
                reporter: None,
            }));
        }
        let known_cells: std::collections::BTreeSet<_> = cells
            .iter()
            .map(|c| (c.dimension.clone(), c.x, c.z))
            .collect();
        for r in crate::game::claim_borders_claim_observations_view::records() {
            if !known_cells.contains(&(r.claim.dimension.clone(), r.claim.x, r.claim.z)) {
                cells.push(map::Cell {
                    dimension: r.claim.dimension,
                    x: r.claim.x,
                    z: r.claim.z,
                    owner: r.owner.unwrap_or_default(),
                    reporter: Some(r.reporter),
                    own: false,
                    foreign: true,
                });
            }
        }
        for (uuid, name) in &c.tab_cache {
            if *uuid == c.own || marks.iter().any(|m| m.player == Some(*uuid)) {
                continue;
            }
            if let Some(pos) = entity_position(game, *uuid) {
                marks.push(map::Mark {
                    player: Some(*uuid),
                    friend: false,
                    skin: c
                        .avatars
                        .get(uuid)
                        .cloned()
                        .unwrap_or_else(|| horizon_client::tab_list::default_skin(*uuid).into()),
                    dimension: dimension.clone(),
                    position: [pos[0], pos[2]],
                    label: name.clone(),
                    color: 0xe9c66b,
                });
            }
        }
        map::Snapshot {
            mobs: {
                let mut mobs: Vec<_> = game
                    .mobs
                    .iter()
                    .filter(|m| !m.removed && m.health > 0.0)
                    .filter_map(|m| {
                        let hostile = map::mob_group(&m.kind.id, &m.kind.category)?;
                        let pos = m.previous.lerp(m.position, game.clock.fraction());
                        Some(map::MobMark {
                            entity: m.id,
                            type_id: m.kind.id.clone(),
                            position: [pos.x, pos.y, pos.z],
                            hostile,
                            label: m
                                .kind
                                .id
                                .strip_prefix("minecraft:")
                                .unwrap_or(&m.kind.id)
                                .replace('_', " "),
                        })
                    })
                    .collect();
                mobs.sort_by(|a, b| {
                    let d = |m: &map::MobMark| {
                        (m.position[0] - player.x).powi(2) + (m.position[2] - player.z).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                });
                mobs.truncate(512);
                mobs
            },
            scope: c.scope.clone(),
            dimension,
            player: [player.x, player.y, player.z],
            player_uuid: c.own,
            yaw: game.yaw,
            player_skin: c
                .avatars
                .get(&c.own)
                .cloned()
                .unwrap_or_else(|| horizon_client::tab_list::default_skin(c.own).into()),
            cells,
            marks,
            own_known: known,
            own_color: c.visual.own_color,
            friend_color: c.visual.friend_color,
        }
    })
}
pub(super) fn open_map(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || !ensure(game) {
        return;
    }
    open_menu(game);
    let source = map_snapshot(game);
    crate::game::claim_borders_map_view::open(source, None, false);
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        state(|c| {
            c.relay = Relay::Off;
            c.controls.clear();
            c.pages.clear();
            stop_network(c);
            c.cached_territories.clear();
            c.last_status.clear();
            c.last_snapshots.clear();
            c.last_claims.clear();
            c.last_hello.clear();
            if let Some(p) = &mut c.party {
                p.disconnect();
            }
        });
        return;
    }
    if !ensure(game) {
        return;
    }
    service_menu(game);
    if game.remote.is_none() {
        return;
    }
    poll_network(game);
    // Sample at the game tick rate; transport work stays on its background thread.
    if !state(|c| {
        c.relay == Relay::WebSocket && c.connected && now(c).saturating_sub(c.last_send) >= 50
    }) {
        return;
    }
    let tab = active_peers(game);
    let Some(player) = game.native_player() else {
        return;
    };
    let Some(body) = game.player.as_mut() else {
        return;
    };
    let snapshot = party::Snapshot {
        dimension: player.dimension.into(),
        position: [player.x, player.y, player.z],
        health: f64::from(body.health.health()),
        max_health: body.attribute("minecraft:max_health").unwrap_or(20.0),
    };
    let own_claims = claims::with_state(|s| s.shared_claims());
    let warn = state(|c| {
        let exceeds = own_claims.as_ref().is_some_and(|claims| claims.len() > 64);
        if exceeds && !c.share_warning {
            c.share_warning = true;
            true
        } else {
            false
        }
    });
    if warn {
        game.native_chat("[Party] Sharing supports at most 64 claims. Your full private list is retained locally; friend territory sharing is cleared for this update.");
    }
    for _ in 0..16 {
        let outgoing = state(|c| {
            let time = now(c);
            c.last_send = time;
            if c.own_claims != own_claims {
                c.own_claims = own_claims.clone();
                if c.own_claims
                    .as_ref()
                    .is_none_or(|claims| claims.len() <= 64)
                {
                    c.share_warning = false;
                }
                c.claim_revision = c.claim_revision.saturating_add(1);
                c.pages.clear();
                c.last_claims.clear();
            }
            let p = c.party.as_mut()?;
            let mut friends: Vec<_> = p.friends().iter().map(|f| f.uuid).collect();
            let count = friends.len();
            if count > 0 {
                friends.rotate_left(c.status_cursor % count);
            }
            for uuid in friends
                .iter()
                .copied()
                .filter(|id| tab.iter().any(|(u, _)| u == id))
            {
                if !p.peer_ready(uuid)
                    && (!c.last_hello.contains_key(&uuid)
                        || time.saturating_sub(*c.last_hello.get(&uuid).unwrap_or(&0)) >= 10_000)
                {
                    if let Ok(frames) = p.start_session(uuid, time) {
                        if c.controls.len() + frames.len() <= 32 {
                            c.controls.extend(frames);
                        }
                    }
                    c.last_hello.insert(uuid, time);
                }
                if p.peer_ready(uuid) && c.last_claims.get(&uuid) != Some(&c.claim_revision) {
                    let shared: Vec<_> = c
                        .own_claims
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .map(|claim| party::Claim {
                            dimension: claim.dimension.clone(),
                            x: claim.x,
                            z: claim.z,
                        })
                        .collect();
                    // Never present a truncated set as the friend's complete territory.
                    // More than 64 claims shares an empty set, with one local warning.
                    let bounded = if shared.len() > 64 {
                        &[][..]
                    } else {
                        shared.as_slice()
                    };
                    if let Ok(frames) = p.claims_for(uuid, c.claim_revision, bounded) {
                        if c.pages.len() + frames.len() <= 256 {
                            c.pages.extend(frames);
                            c.last_claims.insert(uuid, c.claim_revision);
                        }
                    }
                }
            }
            // Keep an offline removal queued until the player returns during this connection.
            let waiting = c.controls.len();
            for _ in 0..waiting {
                let Some(frame) = c.controls.pop_front() else {
                    break;
                };
                if tab.iter().any(|(id, _)| *id == frame.uuid) {
                    return Some((frame, c.relay, time));
                }
                if frame.body.starts_with("[DHP1] [\"d\",") {
                    c.controls.push_back(frame);
                }
            }
            for uuid in friends
                .iter()
                .copied()
                .filter(|id| tab.iter().any(|(u, _)| u == id))
            {
                if p.peer_ready(uuid)
                    && (!c.last_status.contains_key(&uuid)
                        || (time.saturating_sub(*c.last_status.get(&uuid).unwrap_or(&0)) >= 50
                            && (c.last_snapshots.get(&uuid) != Some(&snapshot)
                                || time.saturating_sub(*c.last_status.get(&uuid).unwrap_or(&0))
                                    >= 1000)))
                {
                    if let Ok(frame) = p.snapshot_for(uuid, snapshot.clone()) {
                        c.last_status.insert(uuid, time);
                        c.last_snapshots.insert(uuid, snapshot.clone());
                        c.status_cursor = c.status_cursor.wrapping_add(1);
                        return Some((frame, c.relay, time));
                    }
                }
            }
            while let Some(frame) = c.pages.pop_front() {
                if tab.iter().any(|(id, _)| *id == frame.uuid) && p.peer_ready(frame.uuid) {
                    return Some((frame, c.relay, time));
                }
            }
            None
        });
        let Some((frame, _, _)) = outgoing else {
            return;
        };
        state(|c| {
            let code = c
                .pins
                .as_ref()
                .and_then(|p| p.get(&c.scope, frame.uuid))
                .map(str::to_owned)
                .or_else(|| c.pending_codes.get(&frame.uuid).cloned());
            let result = match (&c.network, code) {
                (Some(handle), Some(code)) => handle.send(&code, &frame.body),
                _ => Err("Relay contact unavailable.".into()),
            };
            if let Err(error) = result {
                // A full worker queue must not discard a claims page or consent response.
                if frame.body.starts_with("[DHP1] [\"c\",") {
                    c.pages.push_front(frame);
                } else if frame.body.starts_with("[DHP1] [\"s\",") {
                    c.last_status.remove(&frame.uuid);
                } else if c.controls.len() < 32 {
                    c.controls.push_front(frame);
                }
                c.network_status = NetStatus::Text(error);
            }
        });
    }
}

pub(super) fn visuals(
    game: &mut Game<'_>,
) -> (
    Vec<overlay::FriendRow>,
    Vec<overlay::Marker>,
    Vec<claims::Claim>,
) {
    if !ensure(game) {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let Some(player) = game.native_player() else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    let tab = if state(|c| c.relay != Relay::Off && c.connected) {
        active_peers(game)
    } else {
        Vec::new()
    };
    let ids: Vec<_> = tab.iter().map(|(id, _)| *id).collect();
    let position = [player.x, player.y, player.z];
    state(|c| {
        let mut rows = Vec::new();
        let mut markers = Vec::new();
        let mut territories = Vec::new();
        if let Some(store) = &c.waypoints {
            for w in store.list(&c.scope, player.dimension) {
                if let Some(distance) = w.distance_from(position) {
                    markers.push(overlay::Marker {
                        player: None,
                        label: w.name.clone(),
                        position: w.position,
                        color: w.color,
                        alpha: w.opacity_from(position).unwrap_or(0.0),
                        distance,
                    });
                }
            }
        }
        if c.relay != Relay::Off
            && let Some(p) = &c.party
        {
            let time = now(c);
            for friend in p.online(&ids, time) {
                if friend.uuid == c.own {
                    continue;
                }
                if !crate::game::claim_borders_claim_memory::known(friend.uuid) {
                    territories.extend(friend.claims.clone().unwrap_or_default().into_iter().map(
                        |c| claims::Claim {
                            dimension: c.dimension,
                            x: c.x,
                            z: c.z,
                        },
                    ));
                }
                rows.push(overlay::FriendRow {
                    name: friend.name.clone(),
                    health_percent: Some(friend.health_percent.round().clamp(0.0, 100.0) as u8),
                });
                if friend.snapshot.dimension == player.dimension {
                    if let Some(distance) = waypoints::distance(friend.snapshot.position, position)
                    {
                        let mut anchor = friend.snapshot.position;
                        anchor[1] += 2.3;
                        markers.push(overlay::Marker {
                            player: Some(friend.uuid),
                            label: friend.name.clone(),
                            position: anchor,
                            color: 0x55bbff,
                            alpha: waypoints::opacity(distance),
                            distance,
                        });
                    }
                }
            }
        }

        c.cached_at = now(c);
        c.cached_territories = territories.clone();
        (rows, markers, territories)
    })
}

pub(super) fn command(game: &mut Game<'_>, command: &str) -> bool {
    let words: Vec<_> = command.split_whitespace().collect();
    if matches!(words.as_slice(), ["horizonmenu"] | ["claimborders", "menu"]) {
        open_menu(game);
        return true;
    }
    if !matches!(
        words.first().copied(),
        Some("wp" | "waypoint" | "party" | "friend")
    ) {
        return false;
    }
    if !game.native_on(claims::ID) {
        return false;
    }
    if !ensure(game) {
        game.native_chat("[Horizon] Join a world first.");
        return true;
    }
    let result = if matches!(words[0], "wp" | "waypoint") {
        waypoint_command(game, &words[1..])
    } else {
        party_command(game, &words[1..])
    };
    match result {
        Ok(lines) => {
            for line in lines {
                game.native_chat(&line);
            }
        }
        Err(error) => game.native_chat(&format!("[Horizon] {error}")),
    }
    true
}

fn waypoint_command(game: &Game<'_>, args: &[&str]) -> Result<Vec<String>, String> {
    let player = game.native_player().ok_or("Join a world first.")?;
    state(|c| {
        let store = c
            .waypoints
            .as_ref()
            .ok_or("Waypoint storage unavailable; check the startup error.")?;
        // Mutate a candidate and save before replacing live state (failed writes retain the old data).
        let mut next = store.clone();
        let action = args.first().copied().unwrap_or("help");
        let mut lines = Vec::new();
        match action {
            "add" => {
                let mut start = 1;
                let mut color = 0xffcc55;
                if args.get(start).is_some_and(|s| s.starts_with('#')) {
                    color = waypoints::parse_color(args[start]).map_err(|e| e.to_string())?;
                    start += 1;
                }
                let name = args.get(start..).unwrap_or(&[]).join(" ");
                let id = next
                    .add(
                        &c.scope,
                        player.dimension,
                        &name,
                        [player.x, player.y, player.z],
                        color,
                    )
                    .map_err(|e| e.to_string())?;
                lines.push(format!("[Waypoints] Added #{id}: {name}"));
            }
            "set" if args.len() >= 6 => {
                let parse = |s: &str| {
                    s.parse::<f64>()
                        .map_err(|_| "Invalid coordinate.".to_owned())
                };
                let pos = [parse(args[1])?, parse(args[2])?, parse(args[3])?];
                let color = waypoints::parse_color(args[4]).map_err(|e| e.to_string())?;
                let name = args[5..].join(" ");
                let id = next
                    .add(&c.scope, player.dimension, &name, pos, color)
                    .map_err(|e| e.to_string())?;
                lines.push(format!("[Waypoints] Added #{id}: {name}"));
            }
            "rename" if args.len() >= 3 => {
                let id = args[1].parse().map_err(|_| "Invalid waypoint id.")?;
                next.rename(&c.scope, id, &args[2..].join(" "))
                    .map_err(|e| e.to_string())?;
                lines.push(format!("[Waypoints] Renamed #{id}."));
            }
            "color" if args.len() == 3 => {
                let id = args[1].parse().map_err(|_| "Invalid waypoint id.")?;
                next.recolor(
                    &c.scope,
                    id,
                    waypoints::parse_color(args[2]).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                lines.push(format!("[Waypoints] Recolored #{id}."));
            }
            "remove" | "delete" if args.len() == 2 => {
                let id = args[1].parse().map_err(|_| "Invalid waypoint id.")?;
                next.remove(&c.scope, id).map_err(|e| e.to_string())?;
                lines.push(format!("[Waypoints] Removed #{id}."));
            }
            "list" if args.len() == 1 => {
                lines.push("[Waypoints] Saved markers on this server:".into());
                lines.extend(
                    store.all(&c.scope).iter().map(|w| {
                        format!("#{} {} [{}] #{:06X}", w.id, w.name, w.dimension, w.color)
                    }),
                );
                return Ok(lines);
            }
            _ => {
                return Ok(vec![
                    "[Waypoints] /wp add [#RRGGBB] name | set x y z #RRGGBB name".into(),
                    "[Waypoints] /wp rename id name | color id #RRGGBB | remove id | list".into(),
                ]);
            }
        }
        next.save(&c.root.join("waypoints.json"))
            .map_err(|e| e.to_string())?;
        c.waypoints = Some(next);
        Ok(lines)
    })
}

fn party_command(game: &mut Game<'_>, args: &[&str]) -> Result<Vec<String>, String> {
    if game.remote.is_none() {
        return Err("Party requires a multiplayer server.".into());
    }
    let action = args.first().copied().unwrap_or("help");
    if action == "relay" && args.len() == 2 {
        return match args[1] {
            "on" | "connect" => {
                start_network(game)?;
                crate::game::claim_borders_chat_view::refresh_translators();
                Ok(vec![tr!(
                    "[Party] Подключение к WebSocket relay.",
                    "[Party] Connecting to WebSocket relay."
                )])
            }
            "off" => {
                state(|c| {
                    stop_network(c);
                    c.manual_off = true;
                });
                crate::game::claim_borders_chat_view::refresh_translators();
                Ok(vec![tr!(
                    "[Party] Relay отключён.",
                    "[Party] Relay disconnected."
                )])
            }
            _ => Err(tr!(
                "/party relay on|off. URL настраивается в меню M → Party → Relay.",
                "/party relay on|off. The URL is set in the M menu → Party → Relay."
            )),
        };
    }
    if action == "sync" && args.len() == 1 {
        claims::with_state(|s| s.begin_refresh());
        game.send_unattended_command("claims");
        state(|c| {
            c.last_claims.clear();
            c.pages.clear();
        });
        return Ok(vec![tr!(
            "[Party] Обновляются ваши приваты с сервера; обмен с друзьями идёт только через relay.",
            "[Party] Refreshing your claims from the server; sharing with friends goes through the relay only."
        )]);
    }
    let tab = cached_online(game);
    state(|c| {
        let time = now(c);
        let p = c.party.as_mut().ok_or("Party storage unavailable.")?;
        if action == "list" && args.len() == 1 {
            let mut lines = vec![format!(
                "[Party] {}. Ваш код: {}",
                c.network_status.text(),
                c.own_code
            )];
            lines.extend(p.friends().iter().map(|f| format!("[Party] {}", f.name)));
            lines.extend(p.pending(time).iter().map(|f| {
                let name = &f.name;
                tr!(
                    "[Party] Приглашение {name}. Примите его в M → Пати → Приглашения.",
                    "[Party] Invitation {name}. Accept it in M → Friends → Invitations."
                )
            }));
            return Ok(lines);
        }
        if matches!(action, "invite" | "add" | "accept" | "remove" | "delete") && args.len() == 2 {
            if c.controls.len() >= 32 {
                return Err(tr!(
                    "Очередь занята. Повторите действие чуть позже.",
                    "The queue is full. Try again shortly."
                ));
            }
            let removing = matches!(action, "remove" | "delete");
            let (uuid, name, code) = if removing {
                let f = p
                    .friends()
                    .into_iter()
                    .find(|f| f.name.eq_ignore_ascii_case(args[1]))
                    .ok_or(tr!("Друг не найден.", "Friend not found."))?;
                let code = c
                    .pins
                    .as_ref()
                    .and_then(|s| s.get(&c.scope, f.uuid))
                    .unwrap_or("")
                    .to_owned();
                (f.uuid, f.name, code)
            } else {
                if !c.connected {
                    return Err(tr!(
                        "Сначала подключитесь к relay в меню.",
                        "Connect to the relay in the menu first."
                    ));
                }
                let code = args[1].trim().to_ascii_lowercase();
                let matches: Vec<_> = c
                    .peers
                    .iter()
                    .filter(|peer| peer.code == code || peer.name.eq_ignore_ascii_case(&code))
                    .collect();
                if matches.len() != 1 {
                    return Err(tr!(
                        "Выбери одного доступного игрока из списка пользователей мода.",
                        "Pick exactly one available player from the mod users list."
                    ));
                }
                let peer = matches[0];
                if c.peers.iter().filter(|p| p.uuid == peer.uuid).count() != 1 {
                    return Err(tr!(
                        "Для игрока обнаружено несколько ключей. Дождитесь переподключения.",
                        "Several keys were found for this player. Wait for them to reconnect."
                    ));
                }
                let code = peer.code.clone();
                if !tab
                    .iter()
                    .any(|(id, name)| *id == peer.uuid && name.eq_ignore_ascii_case(&peer.name))
                {
                    return Err(tr!(
                        "Друг не найден в онлайн-списке игрового сервера.",
                        "Friend not found in the game server's online list."
                    ));
                }
                (peer.uuid, peer.name.clone(), code)
            };
            if uuid == c.own {
                return Err(tr!(
                    "Нельзя пригласить себя.",
                    "You cannot invite yourself."
                ));
            }
            let mut candidate = p.clone();
            let mut pins = c
                .pins
                .as_ref()
                .ok_or(tr!(
                    "Хранилище доверенных ключей недоступно.",
                    "Trusted key storage is unavailable."
                ))?
                .clone();
            let frames = match action {
                "invite" | "add" => {
                    if pins.get(&c.scope, uuid).is_some_and(|old| old != code) {
                        return Err(tr!(
                            "Ключ игрока изменился. Удалите прежнюю дружбу и подтвердите новое приглашение.",
                            "The player's key changed. Remove the old friendship and confirm the new invitation."
                        ));
                    }
                    candidate.invite(uuid, &name, time)?
                }
                "accept" => {
                    if c.pending_codes.get(&uuid) != Some(&code) {
                        return Err(tr!(
                            "Приглашение не соответствует выбранному игроку.",
                            "The invitation does not match the selected player."
                        ));
                    }
                    let frames = candidate.accept(uuid, time)?;
                    pins.pin(&c.scope, uuid, &code)?;
                    frames
                }
                _ => {
                    let frames = candidate.remove(uuid)?;
                    pins.remove(&c.scope, uuid);
                    frames
                }
            };
            pins.save(&c.root.join("relay-contacts.json"))?;
            candidate.save(&party_path(c))?;
            c.pins = Some(pins);
            c.party = Some(candidate);
            c.pages.retain(|f| f.uuid != uuid);
            c.controls.retain(|f| f.uuid != uuid);
            c.last_claims.remove(&uuid);
            c.last_status.remove(&uuid);
            c.cached_territories.clear();
            // Retain the exact selected destination until its queued ACK/removal has been delivered.
            if !code.is_empty() {
                c.pending_codes.insert(uuid, code);
            }
            c.controls.extend(frames);
            return Ok(vec![format!("[Party] {action}: {name}")]);
        }
        Ok(vec![tr!(
            "[Party] /party relay on|off | invite НИК | accept НИК | remove НИК | list | sync",
            "[Party] /party relay on|off | invite NAME | accept NAME | remove NAME | list | sync"
        )])
    })
}

pub(super) fn stranger_territories() -> Vec<claims::Claim> {
    let names: std::collections::BTreeSet<_> = friend_ids()
        .into_iter()
        .map(|(_, n)| n.to_ascii_lowercase())
        .collect();
    let mut land: Vec<_> = crate::game::claim_borders_claim_memory::others()
        .into_iter()
        .filter(|(name, _)| !names.contains(&name.to_ascii_lowercase()))
        .flat_map(|(_, land)| land)
        .collect();
    land.extend(
        crate::game::claim_borders_claim_observations_view::records()
            .into_iter()
            .map(|r| r.claim),
    );
    land.sort();
    land.dedup();
    land
}

pub(super) fn territories() -> Vec<claims::Claim> {
    let names: std::collections::BTreeSet<_> = friend_ids()
        .into_iter()
        .map(|(_, n)| n.to_ascii_lowercase())
        .collect();
    let mut land: Vec<_> = crate::game::claim_borders_claim_memory::others()
        .into_iter()
        .filter(|(name, _)| names.contains(&name.to_ascii_lowercase()))
        .flat_map(|(_, land)| land)
        .collect();
    land.extend(state(|c| {
        if now(c).saturating_sub(c.cached_at) <= 1000 && c.relay != Relay::Off {
            c.cached_territories.clone()
        } else {
            Vec::new()
        }
    }));
    land.sort();
    land.dedup();
    land
}

pub(super) fn visual_settings() -> visualization::Settings {
    state(|c| c.visual.clone())
}

fn menu_snapshot(game: &mut Game<'_>) -> menu::ViewModel {
    let chat = crate::game::claim_borders_chat_view::settings(game);
    let language = chat.language;
    let priority = crate::game::claim_borders_chat_view::priority_settings(game);
    let map_preferences = crate::game::claim_borders_map_view::settings(game);
    let map_source = map_snapshot(game);
    let claim_rows = crate::game::claim_borders_map::sorted_cells(
        &map_source,
        crate::game::claim_borders_map::Preferences {
            own: true,
            friends: true,
            ..map_preferences
        },
    )
    .into_iter()
    .map(|c| menu::ClaimRow {
        label: c.row_label(map_source.player, &map_source.dimension, language),
        owner: c.owner_label(language),
        foreign: c.foreign,
        dimension: c.dimension,
        x: c.x,
        z: c.z,
    })
    .collect();
    let tab = if game.remote.is_some() {
        cached_online(game)
    } else {
        Vec::new()
    };
    let ids: Vec<_> = tab.iter().map(|(id, _)| *id).collect();
    let borders_enabled = claims::with_state(|s| s.enabled);
    let (navigation_status, _) = crate::game::claim_borders_navigation_view::snapshot(game);
    let hotkeys = crate::game::claim_borders_navigation::Hotkey::ALL.map(|action| {
        let key = game.native_mod_key(claims::ID, action.id(), action.preset());
        // "key.keyboard.left.shift" -> "LEFT SHIFT"; an unassigned key shows nothing.
        match key.strip_prefix("key.keyboard.") {
            Some("unknown") | None => String::new(),
            Some(name) => name.replace('.', " ").to_uppercase(),
        }
    });
    let navigation_players = tab
        .iter()
        .filter(|(uuid, _)| entity_position(game, *uuid).is_some())
        .cloned()
        .collect();
    let dimension = game.native_player().map(|p| p.dimension);
    state(|c| {
        let time = now(c);
        let waypoints = c
            .waypoints
            .as_ref()
            .map(|store| {
                store
                    .all(&c.scope)
                    .iter()
                    .map(|w| menu::WaypointRow {
                        id: w.id,
                        name: w.name.clone(),
                        color: w.color,
                        detail: format!(
                            "{}: {:.0} {:.0} {:.0}",
                            w.dimension, w.position[0], w.position[1], w.position[2]
                        ),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut rows = Vec::new();
        if let Some(p) = &mut c.party {
            let live = p.online(&ids, time);
            for friend in p.friends() {
                let code = c
                    .pins
                    .as_ref()
                    .and_then(|p| p.get(&c.scope, friend.uuid))
                    .unwrap_or("")
                    .to_owned();
                let present = c.connected
                    && c.peers
                        .iter()
                        .any(|peer| peer.uuid == friend.uuid && peer.code == code)
                    && ids.contains(&friend.uuid);
                let changed = c
                    .peers
                    .iter()
                    .any(|peer| peer.uuid == friend.uuid && peer.code != code);
                let detail = if changed {
                    language
                        .text(
                            "Ключ изменился — добавьте заново",
                            "Key changed. Add this friend again.",
                        )
                        .into()
                } else if live.iter().any(|s| s.uuid == friend.uuid) {
                    language.text("Друг · в сети", "Friend · online").into()
                } else if present {
                    language
                        .text("Друг · синхронизация…", "Friend · syncing…")
                        .into()
                } else {
                    language
                        .text("Друг · не в сети мода", "Friend · offline in the mod")
                        .into()
                };
                rows.push(menu::PartyRow {
                    uuid: friend.uuid,
                    code,
                    name: friend.name,
                    skin: c.avatars.get(&friend.uuid).cloned().unwrap_or_else(|| {
                        horizon_client::tab_list::default_skin(friend.uuid).into()
                    }),
                    detail,
                    pending: false,
                    friend: true,
                    online: present,
                    server_online: false,
                    blocked: changed,
                    local_only: false,
                    outgoing: false,
                });
            }
            for f in p.pending(time) {
                let code = c.pending_codes.get(&f.uuid).cloned().unwrap_or_default();
                rows.push(menu::PartyRow {
                    uuid: f.uuid,
                    skin: c
                        .avatars
                        .get(&f.uuid)
                        .cloned()
                        .unwrap_or_else(|| horizon_client::tab_list::default_skin(f.uuid).into()),
                    code: code.clone(),
                    name: f.name,
                    detail: language
                        .text(
                            "Хочет добавить вас в друзья",
                            "Wants to add you as a friend",
                        )
                        .into(),
                    pending: true,
                    friend: false,
                    online: c
                        .peers
                        .iter()
                        .any(|peer| peer.uuid == f.uuid && peer.code == code),
                    server_online: false,
                    blocked: false,
                    local_only: false,
                    outgoing: false,
                });
            }
        }
        for peer in &c.peers {
            if peer.uuid == c.own
                || rows.iter().any(|row| row.uuid == peer.uuid)
                || !tab
                    .iter()
                    .any(|(id, name)| *id == peer.uuid && name.eq_ignore_ascii_case(&peer.name))
            {
                continue;
            }
            let blocked = c.peers.iter().filter(|p| p.uuid == peer.uuid).count() != 1;
            rows.push(menu::PartyRow {
                uuid: peer.uuid,
                code: peer.code.clone(),
                name: peer.name.clone(),
                detail: if blocked {
                    language.text("Неоднозначная личность", "Ambiguous identity")
                } else {
                    language.text("Пользователь мода · доступен", "Mod user · available")
                }
                .into(),
                pending: false,
                friend: false,
                online: true,
                server_online: false,
                blocked,
                local_only: false,
                outgoing: c
                    .party
                    .as_mut()
                    .is_some_and(|p| p.awaiting(peer.uuid, time)),
                skin: c
                    .avatars
                    .get(&peer.uuid)
                    .cloned()
                    .unwrap_or_else(|| horizon_client::tab_list::default_skin(peer.uuid).into()),
            });
        }
        menu::append_important_rows(&mut rows, &priority, &tab, language);
        for row in &mut rows {
            row.server_online = tab
                .iter()
                .any(|(id, name)| *id == row.uuid && name.eq_ignore_ascii_case(&row.name));
            if row.outgoing {
                row.detail = language
                    .text("Приглашение отправлено", "Invitation sent")
                    .into();
            }
        }
        rows.sort_by(|a, b| {
            b.pending
                .cmp(&a.pending)
                .then(b.online.cmp(&a.online))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then(a.code.cmp(&b.code))
        });
        let settings = &c.visual;
        menu::ViewModel {
            navigation_players,
            schematic_files: crate::game::claim_borders_workshop_view::filenames(),
            navigation_status,
            hotkeys,
            navigation_waypoints: c
                .waypoints
                .as_ref()
                .map(|store| {
                    store
                        .all(&c.scope)
                        .iter()
                        .filter(|w| Some(w.dimension.as_str()) == dimension)
                        .map(|w| menu::WaypointRow {
                            id: w.id,
                            name: w.name.clone(),
                            detail: String::new(),
                            color: w.color,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            auto_reconnect: game.settings.extras.auto_reconnect,
            experiments_accepted: game
                .native_setting(claims::SETTINGS_ID, "experiments_accepted")
                .as_deref()
                == Some("true"),
            language,
            waypoints,
            party: rows,
            relay: c.network_status.text(),
            relay_url: c
                .network_settings
                .as_ref()
                .map(|s| s.url.clone())
                .unwrap_or_default(),
            relay_connected: c.connected,
            relay_auto: c.auto_relay,
            auto_trust: crate::game::claim_borders_auto_trust_view::enabled(game),
            relay_token_present: c
                .network_settings
                .as_ref()
                .is_some_and(|s| s.access_token.is_some()),
            claims: menu::ClaimsSettings {
                enabled: borders_enabled,
                own_visible: settings.own_visible,
                friends_visible: settings.friends_visible,
                through_walls: settings.through_walls,
                height: match settings.span {
                    visualization::Span::Near16 => 16.0,
                    visualization::Span::Near32 => 32.0,
                    visualization::Span::Near64 => 64.0,
                    visualization::Span::FullHeight => 384.0,
                },
                distance: f32::from(settings.distance),
                own_color: settings.own_color,
                friend_color: settings.friend_color,
                opacity: settings.opacity,
                line_width: settings.line_width,
            },
            status: c.menu_feedback.clone(),
            chat_modern: chat.modern,
            chat_translate: chat.translate,
            deepl_configured: chat.deepl_configured,
            chat_outgoing: crate::game::claim_borders_outgoing_view::enabled(game),
            chat_preferences: chat.preferences,
            priority,
            experimental: crate::game::claim_borders_ore_view::settings(game),
            experimental_status: crate::game::claim_borders_ore_view::status(),
            map_preferences,
            claim_rows,
            economy_preferences: crate::game::claim_borders_economy_client::read(|_, p, _, _| {
                p.clone()
            }),
        }
    })
}

pub(super) fn open_trades(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || !ensure(game) {
        return;
    }
    if game.native_mod_screen_open(claims::ID) {
        menu_view::reset();
        crate::game::claim_borders_economy_view::open();
    }
}

pub(super) fn open_menu(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || !ensure(game) {
        return;
    }
    if game.native_mod_screen_open(claims::ID) {
        menu_view::open();
        menu_view::sync(menu_snapshot(game));
    }
}

pub(super) fn open_menu_from_options(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.title.is_some() || game.native_player().is_none() {
        return;
    }
    // Leave Options via an explicit local settings button; preserve all option edits.
    game.settings.save(&Game::options_path());
    let previous = game.screen;
    game.screen = crate::game::Screen::Playing;
    open_menu(game);
    if game.native_mod_screen() != Some(claims::ID) {
        game.screen = previous;
    }
}

fn execute_menu(game: &mut Game<'_>, action: menu::Action) -> Result<Vec<String>, String> {
    use menu::Action;
    match action {
        Action::RefreshSchematics => {
            crate::game::claim_borders_workshop_view::refresh(game)?;
            Ok(Vec::new())
        }
        Action::OpenSchematicFolder => {
            let dir = crate::game::claim_borders_workshop_view::folder(game);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Horizon's own "open folder" (resource packs): Explorer, Finder or xdg-open.
            crate::game::packs::open_folder(&dir);
            {
                use horizon_client::clipboard::Clipboard;
                game.clipboard.set(&dir.display().to_string());
            }
            // A list already being read is fine; the next click reads it again.
            let _ = crate::game::claim_borders_workshop_view::refresh(game);
            Ok(vec![tr!(
                "Папка схем открыта (путь скопирован). Положите .schem / .litematic и выберите файл.",
                "Schematic folder opened (path copied). Add .schem / .litematic files and choose one."
            )])
        }
        Action::SchematicInfo(folder) => {
            use horizon_client::clipboard::Clipboard;
            let text = if folder {
                crate::game::claim_borders_workshop_view::folder(game)
                    .display()
                    .to_string()
            } else {
                crate::game::claim_borders_workshop_view::materials()
            };
            game.clipboard.set(&text);
            Ok(Vec::new())
        }
        Action::LoadSchematic {
            filename,
            origin,
            rotation,
            running,
        } => {
            if let Some(result) = crate::game::claim_borders_navigation_view::build_baritone(
                game, &filename, origin, rotation, running,
            ) {
                result?;
                game.native_mod_screen_close(claims::ID);
                return Ok(Vec::new());
            }
            crate::game::claim_borders_workshop_view::load(
                game, filename, origin, rotation, running,
            )?;
            if running {
                game.native_mod_screen_close(claims::ID);
            }
            Ok(Vec::new())
        }
        Action::BaritoneKey(key) => {
            if crate::game::claim_borders_navigation_view::run(game, key)? {
                game.native_mod_screen_close(claims::ID);
            }
            Ok(Vec::new())
        }
        Action::Navigate { goal, running } => {
            if let Some(result) =
                crate::game::claim_borders_navigation_view::start_baritone(game, goal, running)
            {
                result?;
                if running {
                    game.native_mod_screen_close(claims::ID);
                }
                return Ok(Vec::new());
            }
            crate::game::claim_borders_navigation_view::start(game, goal, running)?;
            if running {
                game.native_mod_screen_close(claims::ID);
            }
            Ok(Vec::new())
        }
        Action::SetDeepLKey(key) => {
            crate::game::claim_borders_chat_view::set_deepl_key(game, key.as_deref())?;
            let language = crate::game::claim_borders_chat_view::language(game);
            Ok(vec![
                if key.is_some() {
                    language.text("Ключ DeepL обновлён.", "DeepL key updated.")
                } else {
                    language.text("Ключ DeepL удалён.", "DeepL key removed.")
                }
                .into(),
            ])
        }
        Action::SetLanguage(language) => {
            crate::game::claim_borders_chat_view::set_language(game, language);
            if let Some(row) = game
                .settings
                .mods
                .installed
                .iter_mut()
                .find(|row| row.id == claims::ID)
                && let Some(page) = &mut row.native
            {
                let info = native_mod(language);
                row.description = info.description;
                page.keys = info.keys;
            }
            // Transient feedback from earlier actions is already formatted in the old language.
            state(|c| c.menu_feedback.clear());
            Ok(Vec::new())
        }
        Action::AddWaypoint { name, color } => {
            waypoint_command(game, &["add", &format!("#{color:06X}"), &name])
        }
        Action::RenameWaypoint { id, name } => {
            waypoint_command(game, &["rename", &id.to_string(), &name])
        }
        Action::RecolorWaypoint { id, color } => {
            waypoint_command(game, &["color", &id.to_string(), &format!("#{color:06X}")])
        }
        Action::DeleteWaypoint { id } => waypoint_command(game, &["remove", &id.to_string()]),
        Action::Decline { uuid } => state(|c| {
            let time = now(c);
            if c.controls.len() >= 32 {
                return Err(tr!("Очередь занята", "The queue is full"));
            }
            let frames = c
                .party
                .as_mut()
                .ok_or(tr!("Пати недоступна", "Friends are unavailable"))?
                .decline(uuid, time)?;
            c.controls.extend(frames);
            Ok(Vec::new())
        }),
        Action::PlayerRequest { kind, uuid, name } => {
            let own = game
                .remote
                .as_ref()
                .ok_or(tr!(
                    "Запрос доступен только на сервере",
                    "Requests are only available on a server"
                ))?
                .profile();
            let command = kind.resolve(uuid, &name, own, &online(game), language())?;
            if !game.chat_abilities().send_commands {
                return Err(tr!(
                    "Отправка команд отключена в настройках чата",
                    "Sending commands is disabled in chat settings"
                ));
            }
            game.native_send_chat(&command).map_err(|e| {
                tr!(
                    "Не удалось отправить запрос: {e}",
                    "Failed to send the request: {e}"
                )
            })?;
            game.native_mod_screen_close(claims::ID);
            menu_view::reset();
            let what = match (kind, language()) {
                (menu::PlayerRequest::Teleport, Language::Russian) => "Запрос телепортации",
                (menu::PlayerRequest::Trade, Language::Russian) => "Запрос обмена",
                (menu::PlayerRequest::Teleport, Language::English) => "Teleport request",
                (menu::PlayerRequest::Trade, Language::English) => "Trade request",
            };
            game.native_chat(&tr!(
                "[Друзья] {what} для {name} отправлен.",
                "[Friends] {what} sent to {name}."
            ));
            Ok(Vec::new())
        }
        Action::Invite { code } => party_command(game, &["invite", &code]),
        Action::Accept { code } => party_command(game, &["accept", &code]),
        Action::RemoveFriend { name } => party_command(game, &["remove", &name]),
        Action::RelayOff => party_command(game, &["relay", "off"]),
        Action::ToggleAutoRelay => {
            let on = state(|c| {
                c.auto_relay = !c.auto_relay;
                if c.auto_relay {
                    c.manual_off = false;
                    c.last_auto = 0;
                }
                c.auto_relay
            });
            game.native_set_setting(
                claims::SETTINGS_ID,
                "relay_autoconnect",
                if on { "true" } else { "false" },
            );
            if on {
                crate::game::claim_borders_chat_view::refresh_translators();
            }
            Ok(Vec::new())
        }
        Action::RelayConnect => party_command(game, &["relay", "on"]),
        Action::ConfigureRelay { url, access_token } => {
            configure_network(url, access_token, false)?;
            Ok(vec![tr!(
                "Настройки relay сохранены. Нажмите «Подключить».",
                "Relay settings saved. Press Connect."
            )])
        }
        Action::ClearRelayToken => {
            configure_network(String::new(), None, true)?;
            Ok(vec![tr!("Токен relay удалён.", "Relay token removed.")])
        }
        Action::OpenMap => {
            let source = map_snapshot(game);
            crate::game::claim_borders_map_view::open(source, None, true);
            Ok(Vec::new())
        }
        Action::FocusClaim { dimension, x, z } => {
            let source = map_snapshot(game);
            crate::game::claim_borders_map_view::open(source, Some((dimension, x, z)), true);
            Ok(Vec::new())
        }
        Action::SetMap(preferences) => {
            crate::game::claim_borders_map_view::configure(game, preferences);
            Ok(Vec::new())
        }
        Action::ToggleReconnect => {
            game.settings.extras.auto_reconnect = !game.settings.extras.auto_reconnect;
            game.settings.save(&Game::options_path());
            Ok(Vec::new())
        }
        Action::AcceptExperiments => {
            game.native_set_setting(claims::SETTINGS_ID, "experiments_accepted", "true");
            Ok(Vec::new())
        }
        Action::SetExperimental(prefs) => {
            crate::game::claim_borders_ore_view::configure(game, prefs);
            Ok(Vec::new())
        }
        Action::SetPriority(prefs) => {
            crate::game::claim_borders_chat_view::priority_configure(game, prefs);
            Ok(Vec::new())
        }
        Action::SetChat(preferences) => {
            crate::game::claim_borders_chat_view::configure(game, preferences);
            Ok(Vec::new())
        }
        Action::Appearance => {
            crate::game::claim_borders_ui_view::open_settings(game);
            Ok(Vec::new())
        }
        Action::EconomySetting(key, dir) => {
            crate::game::claim_borders_economy_client::configure(game, key, dir);
            Ok(Vec::new())
        }
        Action::OpenEconomy => {
            crate::game::claim_borders_economy_view::open();
            Ok(Vec::new())
        }
        Action::ToggleAutoTrust => {
            let on = !crate::game::claim_borders_auto_trust_view::enabled(game);
            game.native_set_setting(
                claims::SETTINGS_ID,
                "auto_trust",
                if on { "true" } else { "false" },
            );
            Ok(vec![tr!(
                "Авто /trust для подтверждённых друзей: {}",
                "Auto /trust for confirmed friends: {}",
                if on {
                    language().text("вкл.", "on")
                } else {
                    language().text("выкл.", "off")
                }
            )])
        }
        Action::ClearClaimCache => {
            crate::game::claim_borders_claim_memory::clear_others();
            crate::game::claim_borders_claim_observations_view::clear();
            Ok(vec![tr!(
                "Кеш приватов игроков сброшен; онлайн-игроки передадут свои данные при переподключении.",
                "Player claims cache cleared; online players will resend their data when they reconnect."
            )])
        }
        Action::ToggleOutgoing => {
            let on = !crate::game::claim_borders_outgoing_view::enabled(game);
            game.native_set_setting(
                claims::SETTINGS_ID,
                "chat_outgoing_en",
                if on { "true" } else { "false" },
            );
            Ok(Vec::new())
        }
        Action::ToggleChat => {
            crate::game::claim_borders_chat_view::toggle(game, false);
            Ok(Vec::new())
        }
        Action::ToggleTranslation => {
            crate::game::claim_borders_chat_view::toggle(game, true);
            Ok(Vec::new())
        }
        Action::CopyText(text) => {
            use horizon_client::clipboard::Clipboard;
            game.clipboard.set(&text);
            Ok(Vec::new())
        }
        Action::PasteFromClipboard => {
            use horizon_client::clipboard::Clipboard;
            let text = game.clipboard.get();
            menu_view::paste(&text);
            Ok(Vec::new())
        }
        Action::RefreshClaims => {
            if game.remote.is_none() {
                return Err(tr!(
                    "Обновление приватных территорий доступно на сервере.",
                    "Claims can only be refreshed on a server."
                ));
            }
            claims::with_state(|s| s.begin_refresh());
            game.send_unattended_command("claims");
            state(|c| {
                c.last_claims.clear();
                c.pages.clear();
            });
            Ok(vec![tr!(
                "Запрошены ваши приваты с сервера; после ответа они передаются друзьям.",
                "Your claims were requested from the server; they are shared with friends after the reply."
            )])
        }
        Action::SetClaims(settings) => {
            state(|c| {
                if !c.visual_writable {
                    return Err(tr!(
                        "Файл настроек повреждён. Исправьте visualization.json перед сохранением.",
                        "The settings file is corrupted. Fix visualization.json before saving."
                    ));
                }
                let mut next = c.visual.clone();
                next.own_visible = settings.own_visible;
                next.friends_visible = settings.friends_visible;
                next.through_walls = settings.through_walls;
                next.own_color = settings.own_color;
                next.friend_color = settings.friend_color;
                next.set("width", &settings.line_width.to_string())?;
                next.set("opacity", &settings.opacity.to_string())?;
                next.distance = match settings.distance {
                    64.0 => 64,
                    160.0 => 160,
                    256.0 => 256,
                    512.0 => 512,
                    _ => return Err(tr!("Некорректная дальность.", "Invalid distance.")),
                };
                next.span = match settings.height {
                    16.0 => visualization::Span::Near16,
                    32.0 => visualization::Span::Near32,
                    64.0 => visualization::Span::Near64,
                    384.0 => visualization::Span::FullHeight,
                    _ => return Err(tr!("Некорректная высота.", "Invalid height.")),
                };
                next.save(&c.root.join("visualization.json"))?;
                c.visual = next;
                Ok(())
            })?;
            claims::with_state(|s| s.enabled = settings.enabled);
            Ok(vec![tr!(
                "Настройки границ сохранены.",
                "Border settings saved."
            )])
        }
        Action::Close => {
            game.native_mod_screen_close(claims::ID);
            menu_view::reset();
            Ok(Vec::new())
        }
    }
}

pub(super) fn service_menu(game: &mut Game<'_>) {
    if game.native_mod_screen() != Some(claims::ID) || !game.native_on(claims::ID) {
        // Closing with Escape discards queued actions, including signature authorization.
        menu_view::drain_actions();
        if game.native_mod_screen() == Some(claims::ID) {
            game.native_mod_screen_close(claims::ID);
        }
        return;
    }
    if !ensure(game) {
        return;
    }
    for action in menu_view::drain_actions() {
        if game.native_mod_screen() != Some(claims::ID) {
            break;
        }
        let feedback = match execute_menu(game, action) {
            Ok(lines) => lines.join(" "),
            Err(error) => error,
        };
        if !feedback.is_empty() {
            state(|c| c.menu_feedback = feedback);
        }
    }
    if game.native_mod_screen() == Some(claims::ID) {
        menu_view::sync(menu_snapshot(game));
    }
}

fn stop_network(c: &mut Client) {
    c.network = None;
    c.relay = Relay::Off;
    c.connected = false;
    c.network_status = NetStatus::Off;
    c.peers.clear();
    c.pending_codes.clear();
    c.controls.clear();
    c.pages.clear();
    c.cached_territories.clear();
    c.last_claims.clear();
    c.last_status.clear();
    c.last_snapshots.clear();
    c.last_hello.clear();
    if let Some(p) = &mut c.party {
        p.disconnect();
    }
}

fn configure_network(url: String, token: Option<String>, clear_token: bool) -> Result<(), String> {
    state(|c| {
        let mut settings = c
            .network_settings
            .as_ref()
            .ok_or(tr!(
                "Файл relay.json недоступен; исправьте его перед настройкой.",
                "relay.json is unavailable; fix it before configuring."
            ))?
            .clone();
        if clear_token {
            settings.access_token = None;
        } else {
            settings.url = url.trim().to_owned();
            if let Some(token) = token {
                settings.access_token = (!token.is_empty()).then_some(token);
            }
        }
        settings.save(&c.root.join("relay.json"))?;
        stop_network(c);
        c.network_settings = Some(settings);
        Ok::<_, String>(())
    })?;
    crate::game::claim_borders_chat_view::refresh_translators();
    Ok(())
}

pub(super) fn translation_relay() -> Option<claim_borders_relay_client::translation::SharedDeepL> {
    state(|c| {
        if c.manual_off {
            return None;
        }
        let settings = c.network_settings.as_ref()?;
        claim_borders_relay_client::translation::SharedDeepL::new(
            &settings.url,
            settings.access_token.clone()?,
        )
    })
}

fn start_network(game: &mut Game<'_>) -> Result<(), String> {
    let tab = cached_online(game);
    let remote = game.remote.as_ref().ok_or(tr!(
        "Relay работает после входа на игровой сервер.",
        "The relay works after joining a game server."
    ))?;
    let uuid = remote.profile();
    let name = tab
        .iter()
        .find(|(id, _)| *id == uuid)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| remote.session.name.clone());
    let realm = websocket::realm_id(&remote.session.address);
    state(|c| {
        if c.network.is_some() {
            return Err(tr!(
                "Relay уже подключается или подключён. Для смены адреса отключите его.",
                "The relay is already connecting or connected. Disconnect it to change the address."
            ));
        }
        if c.pins.is_none() || c.party.is_none() {
            return Err(tr!(
                "Хранилище пати или проверенных ключей недоступно.",
                "Friends or trusted key storage is unavailable."
            ));
        }
        let settings = c.network_settings.as_ref().ok_or(tr!(
            "Настройки relay недоступны.",
            "Relay settings are unavailable."
        ))?;
        if settings.url.is_empty() {
            return Err(tr!(
                "Укажите URL своего relay в M → Party → Настроить relay.",
                "Set your relay URL in M → Party → Configure relay."
            ));
        }
        let handle = websocket::Handle::spawn(websocket::Config {
            url: settings.url.clone(),
            realm,
            uuid,
            name,
            identity_path: c.root.join(format!("relay-identity-{uuid:032x}.json")),
            access_token: settings.access_token.clone(),
        })?;
        c.manual_off = false;
        c.network = Some(handle);
        c.relay = Relay::WebSocket;
        c.connected = false;
        c.network_status = NetStatus::Connecting;
        Ok(())
    })
}

fn active_peers(game: &mut Game<'_>) -> Vec<(u128, String)> {
    let tab = cached_online(game);
    state(|c| {
        if !c.connected {
            return Vec::new();
        }
        c.peers
            .iter()
            .filter(|peer| {
                tab.iter()
                    .any(|(uuid, name)| *uuid == peer.uuid && name.eq_ignore_ascii_case(&peer.name))
                    && (c
                        .pins
                        .as_ref()
                        .and_then(|pins| pins.get(&c.scope, peer.uuid))
                        == Some(peer.code.as_str())
                        || c.pending_codes.get(&peer.uuid) == Some(&peer.code))
            })
            .map(|p| (p.uuid, p.name.clone()))
            .collect()
    })
}

fn frame_kind(body: &str) -> Option<String> {
    let value =
        serde_json::from_str::<serde_json::Value>(body.strip_prefix(party::PREFIX)?).ok()?;
    value.as_array()?.first()?.as_str().map(str::to_owned)
}

fn receive_network(peer: websocket::Peer, body: String, tab: &[(u128, String)]) -> Option<String> {
    if !tab
        .iter()
        .any(|(uuid, name)| *uuid == peer.uuid && name.eq_ignore_ascii_case(&peer.name))
    {
        return None;
    }
    if body.starts_with("[DHO1] ") {
        if state(|c| c.peers.iter().filter(|p| p.uuid == peer.uuid).count() == 1) {
            crate::game::claim_borders_claim_observations_view::receive(&peer, &body);
        }
        return None;
    }
    if body.starts_with("[DHC1] ") {
        if state(|c| c.peers.iter().filter(|p| p.uuid == peer.uuid).count() == 1) {
            crate::game::claim_borders_claim_memory::receive(&peer, &body, tab);
        }
        return None;
    }
    let kind = frame_kind(&body)?;
    state(|c| {
        let time = now(c);
        if !c.connected || peer.uuid == c.own {
            return None;
        }
        let mut candidate = c.party.as_ref()?.clone();
        let pinned = c.pins.as_ref()?.get(&c.scope, peer.uuid);
        let pending = candidate.awaiting(peer.uuid, time);
        let allowed = relay_settings::permits_frame(
            pinned,
            c.pending_codes.get(&peer.uuid).map(String::as_str),
            pending,
            &kind,
            &peer.code,
        );
        if !allowed {
            return None;
        }
        let before = candidate.friends();
        let pending_before = candidate.pending(time);
        let frames = candidate.receive(peer.uuid, &peer.name, &body, time).ok()?;
        if c.controls.len() + frames.len() > 32 {
            return None;
        }
        let after = candidate.friends();
        let changed = before != after;
        let invited = !pending_before.iter().any(|f| f.uuid == peer.uuid)
            && candidate.pending(time).iter().any(|f| f.uuid == peer.uuid);
        if changed {
            let mut pins = c.pins.as_ref()?.clone();
            let saved = if after.iter().any(|f| f.uuid == peer.uuid) {
                pins.pin(&c.scope, peer.uuid, &peer.code)
            } else {
                pins.remove(&c.scope, peer.uuid);
                Ok(())
            };
            if let Err(error) = saved
                .and_then(|_| pins.save(&c.root.join("relay-contacts.json")))
                .and_then(|_| candidate.save(&party_path(c)))
            {
                return Some(error);
            }
            c.pins = Some(pins);
        }
        c.party = Some(candidate);
        if kind == "i" {
            c.pending_codes.insert(peer.uuid, peer.code.clone());
        }
        if changed || matches!(kind.as_str(), "h" | "r" | "a") {
            c.last_claims.remove(&peer.uuid);
            c.last_status.remove(&peer.uuid);
            c.pages.retain(|f| f.uuid != peer.uuid);
            c.cached_territories.clear();
        }
        c.controls.extend(frames);
        if kind == "n" {
            Some(tr!(
                "{} отклонил приглашение",
                "{} declined the invitation",
                peer.name
            ))
        } else if invited {
            Some(tr!(
                "Приглашение от {}. Откройте M → Пати → Приглашения.",
                "Invitation from {}. Open M → Friends → Invitations.",
                peer.name
            ))
        } else if changed {
            Some(tr!(
                "Список друзей обновлён: {}",
                "Friends list updated: {}",
                peer.name
            ))
        } else {
            None
        }
    })
}

fn poll_network(game: &mut Game<'_>) {
    let events = state(|c| {
        let mut events = Vec::new();
        if let Some(handle) = &c.network {
            for _ in 0..64 {
                if let Some(event) = handle.try_event() {
                    events.push(event);
                } else {
                    break;
                }
            }
        }
        events
    });
    if events.is_empty() {
        return;
    }
    let tab = cached_online(game);
    for event in events {
        match event {
            websocket::Event::Connected { code } => {
                crate::game::claim_borders_claim_memory::network_reset();
                crate::game::claim_borders_claim_observations_view::network_reset();
                state(|c| {
                    c.connected = true;
                    c.own_code = code;
                    c.network_status = NetStatus::Connected;
                    c.controls.clear();
                    c.pages.clear();
                    c.last_hello.clear();
                    c.last_status.clear();
                    c.last_snapshots.clear();
                    c.last_claims.clear();
                    if let Some(p) = &mut c.party {
                        p.disconnect();
                    }
                });
            }
            websocket::Event::Peers(peers) => state(|c| {
                if let Some(p) = &mut c.party {
                    for friend in p.friends() {
                        let present = c
                            .pins
                            .as_ref()
                            .and_then(|pins| pins.get(&c.scope, friend.uuid))
                            .is_some_and(|code| {
                                peers
                                    .iter()
                                    .any(|peer| peer.uuid == friend.uuid && peer.code == code)
                            });
                        if !present {
                            p.forget_session(friend.uuid);
                            c.last_hello.remove(&friend.uuid);
                            c.last_status.remove(&friend.uuid);
                            c.last_claims.remove(&friend.uuid);
                        }
                    }
                }
                c.peers = peers;
                c.cached_territories.clear();
            }),
            websocket::Event::Message { peer, body } => {
                if let Some(notice) = receive_network(peer, body, &tab) {
                    state(|c| c.menu_feedback = notice.clone());
                    game.native_chat(&format!("[Party] {notice}"));
                }
            }
            websocket::Event::Disconnected { reason } => state(|c| {
                c.connected = false;
                c.network_status = NetStatus::Lost(reason);
                c.peers.clear();
                c.controls.clear();
                c.pages.clear();
                c.pending_codes.clear();
                c.last_claims.clear();
                c.last_status.clear();
                c.last_snapshots.clear();
                c.last_hello.clear();
                c.cached_territories.clear();
                if let Some(p) = &mut c.party {
                    p.disconnect();
                }
            }),
            websocket::Event::Notice { message } => {
                state(|c| c.network_status = NetStatus::Text(message))
            }
        }
    }
}

pub(super) fn claim_peers(game: &mut Game<'_>) -> Vec<websocket::Peer> {
    let tab = cached_online(game);
    state(|c| {
        if !c.connected {
            Vec::new()
        } else {
            c.peers
                .iter()
                .filter(|p| {
                    p.uuid != c.own
                        && c.peers.iter().filter(|q| q.uuid == p.uuid).count() == 1
                        && tab
                            .iter()
                            .any(|(id, name)| *id == p.uuid && name.eq_ignore_ascii_case(&p.name))
                })
                .cloned()
                .collect()
        }
    })
}
pub(super) fn claim_identity() -> String {
    state(|c| c.own_code.clone())
}
pub(super) fn claim_send(code: &str, body: &str) -> bool {
    state(|c| {
        c.network
            .as_ref()
            .is_some_and(|h| h.send(code, body).is_ok())
    })
}

pub(super) fn friend_ids() -> Vec<(u128, String)> {
    state(|c| {
        c.party
            .as_ref()
            .map(|p| p.friends().into_iter().map(|f| (f.uuid, f.name)).collect())
            .unwrap_or_default()
    })
}

pub(super) fn add_map_waypoint(
    game: &mut Game<'_>,
    dimension: &str,
    position: [f64; 3],
) -> Result<(), String> {
    if !ensure(game) {
        return Err(tr!("Мир ещё не готов", "The world is not ready yet"));
    }
    let name = tr!(
        "Метка {:.0} {:.0}",
        "Waypoint {:.0} {:.0}",
        position[0],
        position[2]
    );
    state(|c| {
        let mut store = c
            .waypoints
            .as_ref()
            .ok_or(tr!(
                "Хранилище меток недоступно",
                "Waypoint storage is unavailable"
            ))?
            .clone();
        store
            .add(&c.scope, dimension, &name, position, 0xffcc66)
            .map_err(|e| e.to_string())?;
        store
            .save(&c.root.join("waypoints.json"))
            .map_err(|e| e.to_string())?;
        c.waypoints = Some(store);
        Ok(())
    })
}
pub(super) fn quick_waypoint(game: &mut Game<'_>) {
    if !ensure(game) {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let dim = p.dimension.to_owned();
    let pos = [p.x, p.y, p.z];
    if game.native_mod_screen_open(claims::ID) {
        crate::game::claim_borders_waypoint_view::open_create(dim, pos, true);
    }
}

pub(super) fn own_waypoints() -> Vec<waypoints::Waypoint> {
    state(|c| {
        c.waypoints
            .as_ref()
            .map(|s| s.all(&c.scope).into_iter().cloned().collect())
            .unwrap_or_default()
    })
}
pub(super) fn save_own_waypoint(
    id: Option<u64>,
    dim: &str,
    pos: [f64; 3],
    name: &str,
    color: u32,
    kind: waypoints::MarkerKind,
) -> Result<u64, String> {
    state(|c| {
        let mut next = c
            .waypoints
            .as_ref()
            .ok_or(tr!(
                "Хранилище меток недоступно",
                "Waypoint storage is unavailable"
            ))?
            .clone();
        let id = match id {
            Some(id) => {
                next.update(&c.scope, id, name, color, kind)
                    .map_err(|e| e.to_string())?;
                id
            }
            None => {
                let id = next
                    .add(&c.scope, dim, name, pos, color)
                    .map_err(|e| e.to_string())?;
                next.set_kind(&c.scope, id, kind)
                    .map_err(|e| e.to_string())?;
                id
            }
        };
        next.save(&c.root.join("waypoints.json"))
            .map_err(|e| e.to_string())?;
        c.waypoints = Some(next);
        Ok(id)
    })
}
pub(super) fn delete_own_waypoint(id: u64) -> Result<(), String> {
    state(|c| {
        let mut next = c
            .waypoints
            .as_ref()
            .ok_or(tr!(
                "Хранилище меток недоступно",
                "Waypoint storage is unavailable"
            ))?
            .clone();
        next.remove(&c.scope, id).map_err(|e| e.to_string())?;
        next.save(&c.root.join("waypoints.json"))
            .map_err(|e| e.to_string())?;
        c.waypoints = Some(next);
        Ok(())
    })
}
pub(super) fn waypoint_kind(dimension: &str, position: [f64; 2]) -> waypoints::MarkerKind {
    own_waypoints()
        .iter()
        .find(|w| {
            w.dimension == dimension
                && (w.position[0] - position[0]).abs() < 0.01
                && (w.position[2] - position[1]).abs() < 0.01
        })
        .map_or(waypoints::MarkerKind::Pin, |w| w.kind)
}
