// Pat gesture adapter: the Pat key, the aimed player, the first-person hand, the third-person
// arms of the model pass and the relay event. Rules and bounds live in patting.rs; this file only
// reads the game and hands results back on the game thread. A pat never attacks, uses an item or
// sends chat: without a relay peer for the target it stays a local animation.
use crate::game::{
    Game, claim_borders as claims, claim_borders_client as client, claim_borders_patting as pat,
};
use claim_borders_relay_client::translation::Language;
use horizon_client::first_person::Hand;
use horizon_client::mobs::anim::claim_borders_pat_pose as pose;
use horizon_client::text::Text;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct State {
    started: Instant,
    patter: pat::Patter,
    receiver: pat::Receiver,
    inbox: pat::Inbox,
    /// Server, account and dimension; a change forgets every pat in flight.
    scope: String,
    generation: u64,
    ids: u64,
    published: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            patter: pat::Patter::default(),
            receiver: pat::Receiver::default(),
            inbox: pat::Inbox::default(),
            scope: String::new(),
            generation: 0,
            ids: wall() ^ u64::from(std::process::id()).rotate_left(32),
            published: false,
        }
    }
}

fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}

fn now(s: &State) -> u64 {
    s.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn wall() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn language() -> Language {
    crate::game::claim_borders_chat_view::current_language()
}

pub(super) fn policy(game: &Game<'_>) -> pat::Policy {
    pat::Policy::parse(
        game.native_setting(claims::SETTINGS_ID, "pat_policy")
            .as_deref(),
    )
}

pub(super) fn notify(game: &Game<'_>) -> bool {
    game.native_setting(claims::SETTINGS_ID, "pat_notify")
        .as_deref()
        != Some("false")
}

fn conditions(game: &Game<'_>) -> pat::Conditions {
    pat::Conditions {
        enabled: game.native_on(claims::ID) && game.native_window(),
        playing: game.native_playing() && game.focused,
        connected: game.remote.is_some(),
        alive: game
            .player
            .as_ref()
            .is_some_and(|p| !p.health.dead_or_dying()),
        spectator: game.spectator(),
        using_item: game.using.is_some(),
    }
}

fn world_scope(game: &Game<'_>) -> Option<String> {
    let (scope, _) = client::scope(game)?;
    game.remote.as_ref()?;
    Some(format!("{scope}|{}", game.dim.id()))
}

/// The other player under the crosshair within reach, before any block.
fn aimed_player(game: &mut Game<'_>) -> Option<u128> {
    let own = game.remote.as_ref()?.profile();
    let (index, distance, _) = game.mob_hit_within(pat::RANGE)?;
    let id = game.mobs.get(index)?.id;
    let tab = client::online_players(game);
    let remote = game.remote.as_ref()?;
    let uuid = tab
        .iter()
        .map(|(uuid, _)| *uuid)
        .find(|uuid| remote.player_entity(*uuid) == Some(id))?;
    pat::valid_target(
        own,
        &pat::Candidate {
            uuid,
            distance,
            before_block: game.before_block(distance),
            listed: true,
        },
    )
    .then_some(uuid)
}

/// The Pat key (`native_key`): one gesture per press while playing.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if key != game.native_mod_key(claims::ID, "pat", pat::DEFAULT_KEY) {
        return;
    }
    if !pat::key_pressed(pressed, &conditions(game)) {
        return;
    }
    let Some(target) = aimed_player(game) else {
        let text = language().text("Рядом нет игрока под прицелом", "No player in reach");
        game.native_client_action_bar(Text::literal(text), false);
        return;
    };
    if !state(|s| {
        let t = now(s);
        s.patter.try_start(t)
    }) {
        return;
    }
    // A target without a compatible relay peer only sees the vanilla model: no chat fallback.
    let Some(code) = client::pat_destination(game, target) else {
        return;
    };
    let dimension = game.dim.id().to_owned();
    let body = state(|s| {
        pat::encode(&pat::Event {
            target,
            id: pat::next_id(&mut s.ids),
            sent_at: wall(),
            dimension,
        })
    });
    if let Ok(body) = body {
        client::claim_send(&code, &body);
    }
}

/// A pat event from the relay (`client::receive_network`), applied on the next frame.
pub(super) fn receive(sender: u128, name: String, body: String, trust: pat::Trust) {
    state(|s| {
        let generation = s.generation;
        s.inbox.push(pat::Inbound {
            sender,
            name,
            trust,
            body,
            generation,
        })
    });
}

fn arm(model: &horizon_client::mobs::Mob, left: Option<bool>, p: pat::Pose) -> pose::Arm {
    let a = pat::arm_pose(p);
    pose::Arm {
        model: pose::Model {
            entity: model.id,
            join: model.join.get(),
        },
        left,
        weight: a.weight,
        rotation: a.rotation,
    }
}

/// Once a frame (`native_frame`): world changes, accepted events, the arms to draw.
pub(super) fn frame(game: &mut Game<'_>) {
    let ready = game.native_on(claims::ID) && game.native_window();
    let scope = if ready { world_scope(game) } else { None };
    let current = scope.clone().unwrap_or_default();
    state(|s| {
        if s.scope != current {
            s.scope = current;
            s.generation = s.generation.wrapping_add(1);
            s.patter.stop();
            s.receiver.reset();
            s.inbox.clear();
        }
    });
    if scope.is_none() {
        if state(|s| std::mem::take(&mut s.published)) {
            pose::publish(&[]);
        }
        return;
    }
    let c = conditions(game);
    if !c.alive || c.spectator {
        state(|s| s.patter.stop());
    }
    let inbound = state(|s| s.inbox.drain(s.generation));
    if !inbound.is_empty() {
        let policy = policy(game);
        let notify = notify(game);
        let own = game.remote.as_ref().map_or(0, |r| r.profile());
        let own_position = game.native_player().map(|p| [p.x, p.y, p.z]);
        let dimension = game.dim.id().to_owned();
        for e in inbound {
            let sender_position = client::entity_position(game, e.sender);
            let accepted = state(|s| {
                let ctx = pat::Context {
                    own,
                    policy,
                    now: now(s),
                    wall: wall(),
                    dimension: &dimension,
                    own_position,
                    sender_position,
                };
                s.receiver.accept(&ctx, &e)
            });
            if accepted.is_ok() && notify {
                let text = pat::notice(language(), &e.name);
                game.native_client_action_bar(Text::literal(&text), false);
            }
        }
    }
    let (local, remote) = state(|s| {
        let t = now(s);
        (s.patter.pose(t), s.receiver.active(t))
    });
    let mut arms = Vec::with_capacity(remote.len() + 1);
    if let (Some(p), Some(model)) = (local, game.player_model.as_ref()) {
        arms.push(arm(model, Some(game.settings.main_hand_left), p));
    }
    if let Some(r) = &game.remote {
        for (uuid, p) in remote {
            if let Some(model) = r
                .player_entity(uuid)
                .and_then(|id| game.mob_index.get(&game.mobs, id))
            {
                arms.push(arm(model, None, p));
            }
        }
    }
    let publish = state(|s| {
        let was = s.published;
        s.published = !arms.is_empty();
        was || s.published
    });
    if publish {
        pose::publish(&arms);
    }
}

/// The first-person hand (`native_render_hand_pose`) while this player pats.
pub(super) fn hand_pose(game: &Game<'_>, hand: Hand, values: [&mut f32; 7]) {
    if hand != Hand::Main || !game.native_on(claims::ID) {
        return;
    }
    let Some(p) = state(|s| {
        let t = now(s);
        s.patter.pose(t)
    }) else {
        return;
    };
    let offsets = pat::hand_offsets(p);
    let left = game.settings.main_hand_left;
    for (i, value) in values.into_iter().enumerate() {
        // A left hand mirrors sideways movement, yaw and roll.
        let offset = if left && matches!(i, 0 | 4 | 5) {
            -offsets[i]
        } else {
            offsets[i]
        };
        if i == 6 {
            *value *= offset;
        } else {
            *value += offset;
        }
    }
}
