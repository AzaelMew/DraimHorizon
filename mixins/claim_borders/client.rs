// Horizon-only adapters. Pure models are compiled independently by Cargo tests.
use crate::game::{
    Game, claim_borders as claims, claim_borders_overlay as overlay, claim_borders_party as party,
    claim_borders_transport as transport, claim_borders_waypoints as waypoints,
};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq)]
enum Relay {
    Off,
    Unsigned,
    Signed,
}

struct Client {
    scope: String,
    own: u128,
    root: PathBuf,
    waypoints: Option<waypoints::WaypointStore>,
    party: Option<party::Party>,
    relay: Relay,
    started: Instant,
    last_send: u64,
    last_status: BTreeMap<u128, u64>,
    last_hello: BTreeMap<u128, u64>,
    last_claims: BTreeMap<u128, u64>,
    claim_revision: u64,
    own_claims: Option<Vec<claims::Claim>>,
    controls: VecDeque<party::Outbound>,
    pages: VecDeque<party::Outbound>,
    echoes: VecDeque<(u128, String, u64)>,
    cached_territories: Vec<claims::Claim>,
    cached_at: u64,
    tab_cache: Vec<(u128, String)>,
    tab_at: u64,
    share_warning: bool,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            scope: String::new(),
            own: 0,
            root: PathBuf::new(),
            waypoints: None,
            party: None,
            relay: Relay::Off,
            started: Instant::now(),
            last_send: 0,
            last_status: BTreeMap::new(),
            last_hello: BTreeMap::new(),
            last_claims: BTreeMap::new(),
            claim_revision: 1,
            own_claims: None,
            controls: VecDeque::new(),
            pages: VecDeque::new(),
            echoes: VecDeque::new(),
            cached_territories: Vec::new(),
            cached_at: 0,
            tab_cache: Vec::new(),
            tab_at: 0,
            share_warning: false,
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

fn scope(game: &Game<'_>) -> Option<(String, u128)> {
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
    client.root.join(format!("party-{hash:016x}.json"))
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
        ..Client::default()
    };
    let mut errors = Vec::new();
    match waypoints::WaypointStore::load(&client.root.join("waypoints.json")) {
        Ok(store) => client.waypoints = Some(store),
        Err(error) => errors.push(format!("Waypoint storage disabled: {error}")),
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
    state(|c| *c = Client::default());
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) {
        state(|c| {
            c.relay = Relay::Off;
            c.controls.clear();
            c.pages.clear();
            c.echoes.clear();
            c.cached_territories.clear();
            c.last_status.clear();
            c.last_claims.clear();
            c.last_hello.clear();
            if let Some(p) = &mut c.party {
                p.disconnect();
            }
        });
        return;
    }
    if game.remote.is_none() || !ensure(game) {
        return;
    }
    // Do not consume queued frames while chat settings prevent sending commands.
    if !game.chat_abilities().allows("/msg") {
        return;
    }
    // Avoid updating the skin atlas on every game tick; only the 1 Hz send budget needs tab data.
    if !state(|c| c.relay != Relay::Off && now(c).saturating_sub(c.last_send) >= 1000) {
        return;
    }
    let tab = online(game);
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
    let outgoing = state(|c| {
        let time = now(c);
        c.last_send = time;
        if c.own_claims != own_claims {
            c.own_claims = own_claims;
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
        let friends: Vec<_> = p.friends().iter().map(|f| f.uuid).collect();
        for uuid in friends
            .iter()
            .copied()
            .filter(|id| tab.iter().any(|(u, _)| u == id))
        {
            if !p.peer_ready(uuid)
                && time.saturating_sub(*c.last_hello.get(&uuid).unwrap_or(&0)) >= 10_000
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
                    }
                }
                c.last_claims.insert(uuid, c.claim_revision);
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
                && time.saturating_sub(*c.last_status.get(&uuid).unwrap_or(&0)) >= 6000
            {
                if let Ok(frame) = p.snapshot_for(uuid, snapshot.clone()) {
                    c.last_status.insert(uuid, time);
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
    let Some((frame, relay, time)) = outgoing else {
        return;
    };
    let Some((_, recipient)) = tab.iter().find(|(id, _)| *id == frame.uuid) else {
        return;
    };
    let command = format!("msg {recipient} {}", frame.body);
    if command.len() > 255 || !frame.body.starts_with(party::PREFIX) {
        return;
    }
    state(|c| {
        while c.echoes.len() >= 32 {
            c.echoes.pop_front();
        }
        c.echoes.push_back((frame.uuid, frame.body.clone(), time));
    });
    // Signed mode requires a deliberately typed /party relay signed command on this connection.
    // Never automatically fall back from an unsigned command to a signature.
    if relay == Relay::Signed {
        game.send_chat(&format!("/{command}"));
    } else {
        game.send_unattended_command(&command);
    }
}

pub(super) fn private_chat(game: &mut Game<'_>, component: &serde_json::Value) -> bool {
    if !game.native_on(claims::ID) || game.remote.is_none() || !ensure(game) {
        return false;
    }
    let tab = online(game);
    if let Some((recipient, _, body)) = transport::outgoing(component, &tab) {
        return state(|c| {
            let time = now(c);
            c.echoes
                .retain(|(_, _, at)| time.saturating_sub(*at) <= 20_000);
            if let Some(index) = c
                .echoes
                .iter()
                .position(|(id, text, _)| *id == recipient && *text == body)
            {
                c.echoes.remove(index);
                true
            } else {
                false
            }
        });
    }
    let Some((sender, name, body)) = transport::incoming(component, &tab) else {
        return false;
    };
    let (handled, notice) = state(|c| {
        // Sharing is connection-scoped and opt-in; no packet is acted on while disabled.
        if c.relay == Relay::Off {
            return (false, None);
        }
        let time = now(c);
        let Some(original) = &c.party else {
            return (false, None);
        };
        // Receiving a valid frame can create a friendship or a reply. Commit
        // neither if storage or the bounded response queue cannot accept it.
        let mut candidate = original.clone();
        let before = candidate.friends();
        let pending_before = candidate.pending(time);
        let frames = match candidate.receive(sender, &name, &body, time) {
            Ok(frames) => frames,
            Err(_) => return (false, None),
        };
        if c.controls.len() + frames.len() > 32 {
            return (false, None);
        }
        let after = candidate.friends();
        let changed = before != after;
        let invited = !pending_before.iter().any(|f| f.uuid == sender)
            && candidate.pending(time).iter().any(|f| f.uuid == sender);
        if changed && let Err(error) = candidate.save(&party_path(c)) {
            return (false, Some(error));
        }
        // A peer can restart without changing the persisted friend list. Its
        // fresh session still needs a new complete claims transfer.
        let handshake = serde_json::from_str::<serde_json::Value>(
            body.strip_prefix(party::PREFIX).unwrap_or(""),
        )
        .ok()
        .and_then(|v| v.as_array()?.first()?.as_str().map(str::to_owned))
        .is_some_and(|kind| matches!(kind.as_str(), "h" | "r" | "a"));
        c.party = Some(candidate);
        if changed || handshake {
            c.last_claims.remove(&sender);
            c.last_status.remove(&sender);
            c.pages.retain(|f| f.uuid != sender);
        }
        if changed {
            c.cached_territories.clear();
        }
        c.controls.extend(frames);
        (
            true,
            if invited {
                Some(format!("{name} invited you. /party accept {name}"))
            } else if changed {
                Some(format!("Friend list updated: {name}."))
            } else {
                None
            },
        )
    });
    if let Some(notice) = notice {
        game.native_chat(&format!("[Party] {notice}"));
    }
    handled
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
    let tab = if state(|c| c.relay != Relay::Off) {
        cached_online(game)
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
                            label: friend.name.clone(),
                            position: anchor,
                            color: 0x55bbff,
                            alpha: waypoints::opacity(distance),
                            distance,
                        });
                    }
                }
                territories.extend(friend.claims.unwrap_or_default().into_iter().map(|claim| {
                    claims::Claim {
                        dimension: claim.dimension,
                        x: claim.x,
                        z: claim.z,
                    }
                }));
            }
        }
        c.cached_at = now(c);
        c.cached_territories = territories.clone();
        (rows, markers, territories)
    })
}

pub(super) fn command(game: &mut Game<'_>, command: &str) -> bool {
    let words: Vec<_> = command.split_whitespace().collect();
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
        party_command(game, &words[1..], false)
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

fn party_command(
    game: &mut Game<'_>,
    args: &[&str],
    allow_signed: bool,
) -> Result<Vec<String>, String> {
    if game.remote.is_none() {
        return Err("Party requires a multiplayer server.".into());
    }
    let tab = online(game);
    let action = args.first().copied().unwrap_or("help");
    if action == "sync" && args.len() == 1 {
        claims::with_state(|s| s.begin_refresh());
        game.send_unattended_command("claims");
        state(|c| {
            c.last_claims.clear();
            c.pages.clear();
        });
        return Ok(vec![
            "[Party] Refreshing your server claims, then sharing them with accepted friends."
                .into(),
        ]);
    }
    state(|c| {
        let time = now(c);
        if action == "relay" && args.len() == 2 {
            if args[1] == "signed" && !allow_signed {
                return Err("To authorize protocol signatures, type /party relay signed in your chat input; clicked commands cannot enable signed mode.".into());
            }
            c.relay = match args[1] {
                "off" => Relay::Off,
                "unsigned" => Relay::Unsigned,
                "signed" => Relay::Signed,
                _ => return Err("Use /party relay off|unsigned|signed".into()),
            };
            c.controls.clear();
            c.pages.clear();
            c.last_status.clear();
            c.last_claims.clear();
            c.last_hello.clear();
            c.cached_territories.clear();
            if let Some(p) = &mut c.party {
                p.disconnect();
            }
            return Ok(vec![if c.relay == Relay::Off {
                "[Party] Sharing stopped for this connection.".into()
            } else {
                format!(
                    "[Party] {} relay enabled for this connection. /msg will automatically share your position, HP and /claims with accepted friends. The server can read this traffic. /party relay off stops it.",
                    if c.relay == Relay::Signed {
                        "SIGNED (you authorize these protocol-message signatures)"
                    } else {
                        "UNSIGNED (secure-chat servers may reject it)"
                    }
                )
            }]);
        }
        let own = c.own;
        let p = c
            .party
            .as_mut()
            .ok_or("Party storage unavailable; check the startup error.")?;
        if action == "list" && args.len() == 1 {
            let mut lines = vec!["[Party] Friends:".into()];
            lines.extend(p.friends().iter().map(|f| format!("[Party] {}", f.name)));
            lines.extend(
                p.pending(time)
                    .iter()
                    .map(|f| format!("[Party] Invitation: {} (/party accept {})", f.name, f.name)),
            );
            return Ok(lines);
        }
        if matches!(action, "invite" | "add" | "accept" | "remove" | "delete") && args.len() == 2 {
            if c.relay == Relay::Off && !matches!(action, "remove" | "delete") {
                return Err("Choose /party relay signed (secure chat) or unsigned (if the server allows it) on both clients first. Both modes send your position, HP and claims after mutual acceptance.".into());
            }
            let uuid = if matches!(action, "remove" | "delete") {
                p.friends()
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(args[1]))
                    .map(|f| f.uuid)
            } else {
                tab.iter()
                    .find(|(_, n)| n.eq_ignore_ascii_case(args[1]))
                    .map(|(id, _)| *id)
            }
            .ok_or("Player not found (invitations need an online player).")?;
            if uuid == own {
                return Err("You cannot invite yourself.".into());
            }
            if c.controls.len() >= 32 {
                return Err("Party queue is busy; try again shortly.".into());
            }
            let mut candidate = p.clone();
            let frames = match action {
                "invite" | "add" => candidate.invite(uuid, args[1], time)?,
                "accept" => candidate.accept(uuid, time)?,
                _ => candidate.remove(uuid)?,
            };
            if c.controls.len() + frames.len() > 32 {
                return Err("Party queue is busy; try again shortly.".into());
            }
            candidate.save(&party_path(c))?;
            c.party = Some(candidate);
            c.pages.retain(|f| f.uuid != uuid);
            c.controls.retain(|f| f.uuid != uuid);
            c.last_claims.remove(&uuid);
            c.last_status.remove(&uuid);
            c.cached_territories.clear();
            c.controls.extend(frames);
            return Ok(vec![format!("[Party] {action}: {}", args[1])]);
        }
        Ok(vec!["[Party] Both clients: /party relay signed (or unsigned if allowed). Sharing is OFF after reconnect.".into(),
            "[Party] /party invite name | accept name | remove name | list | sync | relay off".into()])
    })
}

pub(super) fn territories() -> Vec<claims::Claim> {
    state(|c| {
        if now(c).saturating_sub(c.cached_at) <= 1000 && c.relay != Relay::Off {
            c.cached_territories.clone()
        } else {
            Vec::new()
        }
    })
}

pub(super) fn typed_relay(game: &mut Game<'_>, message: &str) -> bool {
    let words: Vec<_> = message.split_whitespace().collect();
    if !matches!(words.as_slice(), ["/party" | "/friend", "relay", "signed"])
        || !game.native_on(claims::ID)
    {
        return false;
    }
    if !ensure(game) {
        game.native_chat("[Party] Join a server first.");
        return true;
    }
    match party_command(game, &["relay", "signed"], true) {
        Ok(lines) => {
            for line in lines {
                game.native_chat(&line);
            }
        }
        Err(error) => game.native_chat(&format!("[Party] {error}")),
    }
    true
}
