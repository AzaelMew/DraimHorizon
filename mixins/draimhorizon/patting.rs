// Pure model of the pat gesture: key edges, cooldown, timing and poses, the peer event and who
// may pat whom. Native adapters (pat_view.rs, pat_pose.rs) supply time, identities and positions;
// nothing here sends, renders or touches the game. The gesture is cosmetic: it never attacks,
// uses an item or sends chat, and the recipient's client decides alone whether to show it.
use claim_borders_relay_client::translation::Language;
use serde_json::{Value, json};
use std::collections::VecDeque;

/// Relay body prefix. Older clients ignore every prefix they do not know.
pub const PREFIX: &str = "[DHT1] ";
const SCHEMA: u64 = 1;
/// The default Pat key: unbound in vanilla and in this mod.
pub const DEFAULT_KEY: &str = "key.keyboard.k";
/// Reach to the aimed player's hit box, in blocks.
pub const RANGE: f64 = 3.0;
/// Two pats.
pub const DURATION_MS: u64 = 800;
/// Just longer than the gesture, so a new press never cuts the last one short.
pub const COOLDOWN_MS: u64 = 1000;
/// The patter may be this far from the recipient when the event arrives: reach plus movement.
pub const ACCEPT_DISTANCE: f64 = 6.0;
/// Accepted clock difference between the patter's and the recipient's wall clocks.
pub const EVENT_TTL_MS: u64 = 30_000;
pub const MAX_BODY: usize = 160;
/// Remote arms animated at once.
pub const MAX_REMOTE: usize = 8;
/// Events waiting for the next game frame.
pub const MAX_INBOX: usize = 16;
const MAX_SEEN: usize = 64;
/// One accepted pat per sender this often (the sender's own cooldown is longer).
const SENDER_GAP_MS: u64 = 700;
/// At most this many accepted pats from everyone within the window.
const BURST: usize = 8;
const BURST_WINDOW_MS: u64 = 4000;

/// Who may pat this player. Friends only is the default; an unknown saved value keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    Friends,
    Everyone,
    Off,
}
impl Policy {
    pub fn key(self) -> &'static str {
        match self {
            Self::Friends => "friends",
            Self::Everyone => "everyone",
            Self::Off => "off",
        }
    }
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("everyone") => Self::Everyone,
            Some("off") => Self::Off,
            _ => Self::Friends,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Friends => Self::Everyone,
            Self::Everyone => Self::Off,
            Self::Off => Self::Friends,
        }
    }
    pub fn label(self, language: Language) -> &'static str {
        match self {
            Self::Friends => language.text("только друзья", "friends only"),
            Self::Everyone => language.text("все", "everyone"),
            Self::Off => language.text("никто", "nobody"),
        }
    }
}

/// How the relay sender relates to this player, decided by the adapter from pinned identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    /// An accepted friend whose relay identity matches the pinned one.
    Friend,
    /// Not a friend, but its relay identity is the only one for a UUID and name on the tab list.
    Stranger,
    /// A pinned UUID under another identity, a duplicated UUID or an unlisted profile.
    Untrusted,
}

/// Trust in a relay sender. `listed`: its UUID and name are on the server's player list;
/// `unique`: no other relay identity claims that UUID; `pinned`: the identity code saved for
/// that UUID when the friendship was accepted.
pub fn trust(friend: bool, pinned: Option<&str>, code: &str, listed: bool, unique: bool) -> Trust {
    if !listed || !unique {
        return Trust::Untrusted;
    }
    match pinned {
        Some(pin) if pin != code => Trust::Untrusted,
        Some(_) if friend => Trust::Friend,
        // A friend with no pinned identity cannot be told apart from a stranger using its UUID.
        None if friend => Trust::Untrusted,
        _ => Trust::Stranger,
    }
}

/// A held key repeats `pressed`; only its first press counts.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyEdge {
    held: bool,
}
impl KeyEdge {
    pub fn update(&mut self, pressed: bool) -> bool {
        let first = pressed && !self.held;
        self.held = pressed;
        first
    }
}

/// The animation at one moment: how far the arm is raised (0..1) and how far a tap brings the
/// hand down (0..raise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub raise: f32,
    pub tap: f32,
}

/// The pose `elapsed_ms` after the start; none once the two pats are over.
pub fn sample(elapsed_ms: u64) -> Option<Pose> {
    if elapsed_ms >= DURATION_MS {
        return None;
    }
    let t = elapsed_ms as f32 / DURATION_MS as f32;
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    // Up over the first sixth, down over the last sixth.
    let raise = smooth(t * 6.0) * smooth((1.0 - t) * 6.0);
    // Two downstrokes, at a quarter and three quarters of the way.
    let tap = 0.5 - 0.5 * (t * 2.0 * std::f32::consts::TAU).cos();
    Some(Pose {
        raise,
        tap: tap * raise,
    })
}

/// First-person hand offsets for `native_render_hand_pose`: x, y, z in blocks, pitch, yaw and
/// roll in degrees, scale.
pub fn hand_offsets(pose: Pose) -> [f32; 7] {
    let Pose { raise, tap } = pose;
    // Turned about the eye rather than moved, so the arm stays attached at the screen's edge:
    // yaw brings the hand towards the crosshair, pitch lifts it over the other player's head and
    // each tap lowers it again.
    [
        0.0,
        0.26 * raise - 0.08 * tap,
        -0.04 * raise,
        26.0 * raise - 14.0 * tap,
        18.0 * raise,
        0.0,
        1.0,
    ]
}

/// The patting arm of a third-person model: blend weight and its x, y, z rotations in radians
/// (a right arm; the adapter mirrors y and z for a left one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmPose {
    pub weight: f32,
    pub rotation: [f32; 3],
}
pub fn arm_pose(pose: Pose) -> ArmPose {
    ArmPose {
        weight: pose.raise,
        // The arm cannot reach a player two blocks away, so it is raised well above level for the
        // hand to come down on top of the head rather than at the cheek; turned inwards.
        rotation: [-2.5 + 0.35 * (pose.tap / pose.raise.max(0.01)), -0.55, 0.0],
    }
}

/// The local gesture: one start per press, then a cooldown.
#[derive(Clone, Copy, Debug, Default)]
pub struct Patter {
    last: Option<u64>,
    started: Option<u64>,
}
impl Patter {
    pub fn try_start(&mut self, now: u64) -> bool {
        if self
            .last
            .is_some_and(|last| now.saturating_sub(last) < COOLDOWN_MS)
        {
            return false;
        }
        self.last = Some(now);
        self.started = Some(now);
        true
    }
    pub fn pose(&mut self, now: u64) -> Option<Pose> {
        let pose = sample(now.saturating_sub(self.started?));
        if pose.is_none() {
            self.started = None;
        }
        pose
    }
    /// Ends the animation at once (screen change, death, another world); keeps the cooldown.
    pub fn stop(&mut self) {
        self.started = None;
    }
}

/// Why the Pat key does nothing right now.
#[derive(Clone, Copy, Debug, Default)]
pub struct Conditions {
    pub enabled: bool,
    /// No screen, chat or sign editor is open.
    pub playing: bool,
    pub connected: bool,
    pub alive: bool,
    pub spectator: bool,
    /// An item is in use (eating, a bow, a shield).
    pub using_item: bool,
}
pub fn can_pat(c: &Conditions) -> bool {
    c.enabled && c.playing && c.connected && c.alive && !c.spectator && !c.using_item
}

/// The player under the crosshair, as the adapter resolved it.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub uuid: u128,
    /// Along the view to its hit box.
    pub distance: f64,
    /// No block is hit first.
    pub before_block: bool,
    /// The UUID is on the server's player list.
    pub listed: bool,
}
pub fn valid_target(own: u128, c: &Candidate) -> bool {
    c.uuid != 0
        && c.uuid != own
        && c.listed
        && c.before_block
        && c.distance.is_finite()
        && (0.0..=RANGE).contains(&c.distance)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub target: u128,
    /// Fresh per pat, never zero.
    pub id: u64,
    /// The patter's wall clock, milliseconds since the Unix epoch.
    pub sent_at: u64,
    pub dimension: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Malformed,
    /// A newer schema or another frame under this prefix.
    Unsupported,
    WrongTarget,
    SelfPat,
    Untrusted,
    /// The policy or the notification settings refuse it.
    Consent,
    Duplicate,
    Expired,
    Burst,
    /// Another dimension or a stale world.
    Scope,
    /// The patter is not loaded or not near.
    Distance,
}

fn valid_dimension(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    value.len() <= 64
        && !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        && path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-/".contains(&c))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub fn encode(event: &Event) -> Result<String, String> {
    if event.id == 0 || event.target == 0 || !valid_dimension(&event.dimension) {
        return Err("Invalid pat event.".into());
    }
    let body = format!(
        "{PREFIX}{}",
        json!([
            SCHEMA,
            format!("{:032x}", event.target),
            event.id,
            event.sent_at,
            event.dimension
        ])
    );
    if body.len() > MAX_BODY {
        return Err("Pat event is too large.".into());
    }
    Ok(body)
}

pub fn decode(body: &str) -> Result<Event, Reject> {
    if body.len() > MAX_BODY || !body.is_ascii() {
        return Err(Reject::Malformed);
    }
    let raw = body.strip_prefix(PREFIX).ok_or(Reject::Malformed)?;
    let value: Value = serde_json::from_str(raw).map_err(|_| Reject::Malformed)?;
    let a = value.as_array().ok_or(Reject::Malformed)?;
    match a.first().and_then(Value::as_u64) {
        Some(SCHEMA) => {}
        Some(_) => return Err(Reject::Unsupported),
        None => return Err(Reject::Malformed),
    }
    if a.len() != 5 {
        return Err(Reject::Malformed);
    }
    let target = a[1]
        .as_str()
        .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .and_then(|s| u128::from_str_radix(s, 16).ok())
        .filter(|t| *t != 0)
        .ok_or(Reject::Malformed)?;
    let id = a[2].as_u64().filter(|n| *n > 0).ok_or(Reject::Malformed)?;
    let sent_at = a[3].as_u64().ok_or(Reject::Malformed)?;
    let dimension = a[4]
        .as_str()
        .filter(|d| valid_dimension(d))
        .ok_or(Reject::Malformed)?;
    Ok(Event {
        target,
        id,
        sent_at,
        dimension: dimension.into(),
    })
}

/// SplitMix64 over the adapter's seed: event ids only need to be distinct, not secret.
pub fn next_id(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (value ^ (value >> 31)).max(1)
}

/// A relay message waiting for the game frame, stamped with the world it arrived in.
#[derive(Clone, Debug)]
pub struct Inbound {
    pub sender: u128,
    /// The authenticated tab-list name, never the body's.
    pub name: String,
    pub trust: Trust,
    pub body: String,
    pub generation: u64,
}

/// Bounded queue between the relay poll and the frame that applies events.
#[derive(Clone, Debug, Default)]
pub struct Inbox {
    queue: VecDeque<Inbound>,
}
impl Inbox {
    pub fn push(&mut self, inbound: Inbound) -> bool {
        if self.queue.len() >= MAX_INBOX {
            return false;
        }
        self.queue.push_back(inbound);
        true
    }
    /// Events of the current world only; older ones are dropped.
    pub fn drain(&mut self, generation: u64) -> Vec<Inbound> {
        self.queue
            .drain(..)
            .filter(|e| e.generation == generation)
            .collect()
    }
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// What the recipient knows when it applies an event.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    pub own: u128,
    pub policy: Policy,
    /// Monotonic milliseconds of the adapter.
    pub now: u64,
    /// Wall clock, milliseconds since the Unix epoch.
    pub wall: u64,
    pub dimension: &'a str,
    pub own_position: Option<[f64; 3]>,
    /// The sender's loaded player entity.
    pub sender_position: Option<[f64; 3]>,
}

/// Accepted pats and the remote arms they animate, all bounded.
#[derive(Clone, Debug, Default)]
pub struct Receiver {
    seen: VecDeque<(u128, u64)>,
    last: VecDeque<(u128, u64)>,
    recent: VecDeque<u64>,
    remote: Vec<(u128, u64)>,
}
impl Receiver {
    pub fn accept(&mut self, ctx: &Context<'_>, inbound: &Inbound) -> Result<(), Reject> {
        if inbound.sender == ctx.own {
            return Err(Reject::SelfPat);
        }
        if inbound.trust == Trust::Untrusted {
            return Err(Reject::Untrusted);
        }
        match (ctx.policy, inbound.trust) {
            (Policy::Off, _) | (Policy::Friends, Trust::Stranger) => return Err(Reject::Consent),
            _ => {}
        }
        if !valid_name(&inbound.name) {
            return Err(Reject::Malformed);
        }
        let event = decode(&inbound.body)?;
        if event.target != ctx.own {
            return Err(Reject::WrongTarget);
        }
        if event.dimension != ctx.dimension {
            return Err(Reject::Scope);
        }
        if event.sent_at.saturating_sub(ctx.wall) > EVENT_TTL_MS
            || ctx.wall.saturating_sub(event.sent_at) > EVENT_TTL_MS
        {
            return Err(Reject::Expired);
        }
        if self.seen.contains(&(inbound.sender, event.id)) {
            return Err(Reject::Duplicate);
        }
        let near = match (ctx.own_position, ctx.sender_position) {
            (Some(own), Some(sender)) => {
                let d = (0..3).map(|i| (own[i] - sender[i]).powi(2)).sum::<f64>();
                d.is_finite() && d.sqrt() <= ACCEPT_DISTANCE
            }
            _ => false,
        };
        if !near {
            return Err(Reject::Distance);
        }
        self.recent
            .retain(|at| ctx.now.saturating_sub(*at) < BURST_WINDOW_MS);
        let too_soon = self.last.iter().any(|(sender, at)| {
            *sender == inbound.sender && ctx.now.saturating_sub(*at) < SENDER_GAP_MS
        });
        if too_soon || self.recent.len() >= BURST {
            return Err(Reject::Burst);
        }
        if self.seen.len() >= MAX_SEEN {
            self.seen.pop_front();
        }
        self.seen.push_back((inbound.sender, event.id));
        self.last.retain(|(sender, _)| *sender != inbound.sender);
        if self.last.len() >= MAX_SEEN {
            self.last.pop_front();
        }
        self.last.push_back((inbound.sender, ctx.now));
        self.recent.push_back(ctx.now);
        self.remote.retain(|(sender, _)| *sender != inbound.sender);
        if self.remote.len() >= MAX_REMOTE {
            self.remote.remove(0);
        }
        self.remote.push((inbound.sender, ctx.now));
        Ok(())
    }
    /// Remote arms still animating, oldest first; finished ones are forgotten.
    pub fn active(&mut self, now: u64) -> Vec<(u128, Pose)> {
        self.remote
            .retain(|(_, start)| sample(now.saturating_sub(*start)).is_some());
        self.remote
            .iter()
            .filter_map(|(sender, start)| Some((*sender, sample(now.saturating_sub(*start))?)))
            .collect()
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The recipient's notification; `name` is the authenticated profile name, shown as plain text.
pub fn notice(language: Language, name: &str) -> String {
    match language {
        Language::Russian => format!("Вас погладил(а) {name}"),
        Language::English => format!("{name} patted you"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u128 = 0xa;
    const B: u128 = 0xb;
    const C: u128 = 0xc;

    fn event(target: u128, id: u64, sent_at: u64) -> String {
        encode(&Event {
            target,
            id,
            sent_at,
            dimension: "minecraft:overworld".into(),
        })
        .unwrap()
    }
    fn inbound(sender: u128, trust: Trust, body: String) -> Inbound {
        Inbound {
            sender,
            name: "Patter".into(),
            trust,
            body,
            generation: 1,
        }
    }
    fn ctx(policy: Policy, now: u64) -> Context<'static> {
        Context {
            own: B,
            policy,
            now,
            wall: 1_000_000 + now,
            dimension: "minecraft:overworld",
            own_position: Some([0.0, 64.0, 0.0]),
            sender_position: Some([2.0, 64.0, 0.0]),
        }
    }

    #[test]
    fn spoofed_or_ambiguous_identities_are_untrusted() {
        assert_eq!(trust(true, Some("c0de"), "c0de", true, true), Trust::Friend);
        assert_eq!(
            trust(true, Some("c0de"), "f00d", true, true),
            Trust::Untrusted,
            "another relay identity claiming a friend's UUID"
        );
        assert_eq!(trust(false, None, "f00d", true, true), Trust::Stranger);
        assert_eq!(
            trust(false, Some("c0de"), "f00d", true, true),
            Trust::Untrusted
        );
        assert_eq!(trust(true, None, "c0de", true, true), Trust::Untrusted);
        assert_eq!(
            trust(true, Some("c0de"), "c0de", false, true),
            Trust::Untrusted
        );
        assert_eq!(
            trust(true, Some("c0de"), "c0de", true, false),
            Trust::Untrusted
        );
        assert_eq!(trust(false, None, "f00d", false, true), Trust::Untrusted);
    }

    #[test]
    fn a_held_key_pats_once_per_press() {
        let mut key = KeyEdge::default();
        assert!(key.update(true));
        assert!(!key.update(true), "a repeat of the held key");
        assert!(!key.update(true));
        assert!(!key.update(false));
        assert!(key.update(true), "a new press");
    }

    #[test]
    fn the_cooldown_allows_one_gesture_a_second() {
        let mut patter = Patter::default();
        assert!(patter.try_start(10_000));
        assert!(!patter.try_start(10_500));
        assert!(!patter.try_start(10_999));
        assert!(patter.try_start(11_000));
        // Every pat the cooldown lets through reaches the recipient, and never cuts the last
        // one short.
        let mut patter = Patter::default();
        let mut r = Receiver::default();
        let mut now = 0;
        for id in 1..=6 {
            while !patter.try_start(now) {
                now += 10;
            }
            assert!(
                now == 0 || now % COOLDOWN_MS == 0,
                "a press is refused only while cooling down"
            );
            let body = event(B, id, 1_000_000 + now);
            assert_eq!(
                r.accept(&ctx(Policy::Friends, now), &inbound(A, Trust::Friend, body)),
                Ok(()),
                "pat {id} at {now} ms"
            );
        }
    }

    #[test]
    fn two_pats_last_about_eight_tenths_of_a_second() {
        assert!(sample(0).is_some_and(|p| p.raise == 0.0 && p.tap == 0.0));
        let taps: Vec<f32> = (0..DURATION_MS)
            .step_by(10)
            .map(|t| sample(t).unwrap().tap)
            .collect();
        // Two downstrokes: the tap rises to a peak twice.
        let peaks = taps
            .windows(3)
            .filter(|w| w[1] > w[0] && w[1] >= w[2] && w[1] > 0.5)
            .count();
        assert_eq!(peaks, 2, "{taps:?}");
        assert!(sample(DURATION_MS / 2).unwrap().raise > 0.9);
        assert_eq!(sample(DURATION_MS), None);
        assert_eq!(sample(u64::MAX), None);
    }

    #[test]
    fn poses_stay_finite_and_bounded() {
        for t in 0..DURATION_MS {
            let p = sample(t).unwrap();
            assert!((0.0..=1.0).contains(&p.raise) && (0.0..=p.raise).contains(&p.tap));
            for v in hand_offsets(p) {
                assert!(v.is_finite());
            }
            let [x, y, z, pitch, yaw, roll, scale] = hand_offsets(p);
            assert!(x.abs() <= 0.5 && y.abs() <= 0.5 && z.abs() <= 0.5);
            assert!(pitch.abs() <= 90.0 && yaw.abs() <= 90.0 && roll.abs() <= 90.0);
            assert_eq!(scale, 1.0);
            let arm = arm_pose(p);
            assert!((0.0..=1.0).contains(&arm.weight));
            assert!(arm.rotation.iter().all(|r| r.is_finite() && r.abs() <= 3.2));
        }
        // At rest the hand is exactly where vanilla puts it.
        let rest = Pose {
            raise: 0.0,
            tap: 0.0,
        };
        assert_eq!(hand_offsets(rest), [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(arm_pose(rest).weight, 0.0);
    }

    #[test]
    fn the_raised_arm_points_forward_and_taps_down() {
        let up = arm_pose(Pose {
            raise: 1.0,
            tap: 0.0,
        });
        let down = arm_pose(Pose {
            raise: 1.0,
            tap: 1.0,
        });
        // Model arms raise forward with negative x rotation; a tap lowers it a little.
        assert!(up.rotation[0] < -1.6 && up.rotation[0] > -2.6);
        assert!(down.rotation[0] > up.rotation[0]);
        assert_eq!(up.weight, 1.0);
    }

    #[test]
    fn the_animation_expires_and_stop_restores_the_pose() {
        let mut patter = Patter::default();
        assert!(patter.try_start(100));
        assert!(patter.pose(100).is_some());
        assert!(patter.pose(100 + DURATION_MS / 2).is_some());
        assert_eq!(patter.pose(100 + DURATION_MS), None);
        assert_eq!(patter.pose(100 + DURATION_MS / 2), None, "it stays over");
        assert!(patter.try_start(5000));
        patter.stop();
        assert_eq!(patter.pose(5001), None);
        assert!(!patter.try_start(5002), "stopping keeps the cooldown");
    }

    #[test]
    fn menus_death_spectating_and_item_use_block_the_gesture() {
        let ready = Conditions {
            enabled: true,
            playing: true,
            connected: true,
            alive: true,
            spectator: false,
            using_item: false,
        };
        assert!(can_pat(&ready));
        for blocked in [
            Conditions {
                enabled: false,
                ..ready
            },
            Conditions {
                playing: false,
                ..ready
            },
            Conditions {
                connected: false,
                ..ready
            },
            Conditions {
                alive: false,
                ..ready
            },
            Conditions {
                spectator: true,
                ..ready
            },
            Conditions {
                using_item: true,
                ..ready
            },
        ] {
            assert!(!can_pat(&blocked), "{blocked:?}");
        }
    }

    #[test]
    fn targets_must_be_listed_other_players_in_reach_and_in_sight() {
        let ok = Candidate {
            uuid: B,
            distance: 2.0,
            before_block: true,
            listed: true,
        };
        assert!(valid_target(A, &ok));
        assert!(valid_target(
            A,
            &Candidate {
                distance: RANGE,
                ..ok
            }
        ));
        assert!(!valid_target(
            A,
            &Candidate {
                distance: RANGE + 0.01,
                ..ok
            }
        ));
        assert!(!valid_target(
            A,
            &Candidate {
                distance: f64::NAN,
                ..ok
            }
        ));
        assert!(!valid_target(
            A,
            &Candidate {
                distance: -1.0,
                ..ok
            }
        ));
        assert!(!valid_target(A, &Candidate { uuid: A, ..ok }), "self");
        assert!(!valid_target(A, &Candidate { uuid: 0, ..ok }));
        assert!(!valid_target(
            A,
            &Candidate {
                before_block: false,
                ..ok
            }
        ));
        assert!(!valid_target(
            A,
            &Candidate {
                listed: false,
                ..ok
            }
        ));
    }

    #[test]
    fn events_round_trip_and_reject_malformed_bodies() {
        let e = Event {
            target: 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
            id: 42,
            sent_at: 1_700_000_000_000,
            dimension: "minecraft:the_nether".into(),
        };
        let body = encode(&e).unwrap();
        assert!(body.starts_with(PREFIX) && body.len() <= MAX_BODY && body.is_ascii());
        assert_eq!(decode(&body), Ok(e.clone()));
        assert!(encode(&Event { id: 0, ..e.clone() }).is_err());
        assert!(
            encode(&Event {
                dimension: "Bad Dim".into(),
                ..e.clone()
            })
            .is_err()
        );
        let target = format!("{:032x}", e.target);
        for bad in [
            String::new(),
            "[DHT1] ".into(),
            "[DHT1] {}".into(),
            "[DHT1] []".into(),
            format!("[DHT1] [1,\"{target}\",42,1]"),
            format!("[DHT1] [1,\"{target}\",0,1,\"minecraft:overworld\"]"),
            format!("[DHT1] [1,\"{target}\",-4,1,\"minecraft:overworld\"]"),
            format!("[DHT1] [1,\"{target}\",42,1,\"minecraft:overworld\",7]"),
            "[DHT1] [1,\"xyz\",42,1,\"minecraft:overworld\"]".into(),
            format!("[DHT1] [1,\"{target}\",42,1,\"NOT VALID\"]"),
            format!("[DHP1] [1,\"{target}\",42,1,\"minecraft:overworld\"]"),
            format!("[DHT1] [1,\"{target}\",42,1,\"{}\"]", "a".repeat(200)),
        ] {
            assert_eq!(decode(&bad), Err(Reject::Malformed), "{bad}");
        }
        assert_eq!(
            decode(&format!(
                "[DHT1] [2,\"{target}\",42,1,\"minecraft:overworld\"]"
            )),
            Err(Reject::Unsupported)
        );
    }

    #[test]
    fn a_friend_pat_starts_one_remote_arm() {
        let mut r = Receiver::default();
        let body = event(B, 1, 1_000_100);
        assert_eq!(
            r.accept(&ctx(Policy::Friends, 100), &inbound(A, Trust::Friend, body)),
            Ok(())
        );
        let active = r.active(150);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].0, A);
        assert!(r.active(100 + DURATION_MS).is_empty());
    }

    #[test]
    fn consent_follows_the_recipients_policy() {
        for (policy, trust, accepted) in [
            (Policy::Friends, Trust::Friend, true),
            (Policy::Friends, Trust::Stranger, false),
            (Policy::Everyone, Trust::Friend, true),
            (Policy::Everyone, Trust::Stranger, true),
            (Policy::Everyone, Trust::Untrusted, false),
            (Policy::Friends, Trust::Untrusted, false),
            (Policy::Off, Trust::Friend, false),
            (Policy::Off, Trust::Stranger, false),
        ] {
            let mut r = Receiver::default();
            let got = r.accept(
                &ctx(policy, 100),
                &inbound(A, trust, event(B, 1, 1_000_100)),
            );
            assert_eq!(got.is_ok(), accepted, "{policy:?} {trust:?} {got:?}");
        }
        let mut r = Receiver::default();
        assert_eq!(
            r.accept(
                &ctx(Policy::Everyone, 100),
                &inbound(A, Trust::Untrusted, event(B, 1, 1_000_100))
            ),
            Err(Reject::Untrusted),
            "a spoofed or unpinned identity never counts as a stranger"
        );
    }

    #[test]
    fn only_events_for_this_player_from_another_player_count() {
        let mut r = Receiver::default();
        let c = ctx(Policy::Everyone, 100);
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(C, 1, 1_000_100))),
            Err(Reject::WrongTarget)
        );
        assert_eq!(
            r.accept(&c, &inbound(B, Trust::Friend, event(B, 2, 1_000_100))),
            Err(Reject::SelfPat)
        );
        let mut named = inbound(A, Trust::Friend, event(B, 3, 1_000_100));
        named.name = "<b>evil</b>".into();
        assert_eq!(r.accept(&c, &named), Err(Reject::Malformed));
        let mut huge = inbound(A, Trust::Friend, event(B, 4, 1_000_100));
        huge.body.push_str(&" ".repeat(MAX_BODY));
        assert_eq!(r.accept(&c, &huge), Err(Reject::Malformed));
        assert!(r.active(150).is_empty());
    }

    #[test]
    fn replayed_and_expired_events_are_refused() {
        let mut r = Receiver::default();
        let c = ctx(Policy::Friends, 100);
        let body = event(B, 7, 1_000_100);
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, body.clone())),
            Ok(())
        );
        let later = ctx(Policy::Friends, 5000);
        assert_eq!(
            r.accept(&later, &inbound(A, Trust::Friend, body)),
            Err(Reject::Duplicate)
        );
        let stale = 1_000_100 - EVENT_TTL_MS - 1;
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 8, stale))),
            Err(Reject::Expired)
        );
        let future = 1_000_100 + EVENT_TTL_MS + 1;
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 9, future))),
            Err(Reject::Expired)
        );
    }

    #[test]
    fn bursts_are_limited_per_sender_and_overall() {
        let mut r = Receiver::default();
        let mut id = 0;
        let mut send = |r: &mut Receiver, sender: u128, now: u64| {
            id += 1;
            r.accept(
                &ctx(Policy::Everyone, now),
                &inbound(sender, Trust::Stranger, event(B, id, 1_000_000 + now)),
            )
        };
        assert_eq!(send(&mut r, A, 0), Ok(()));
        assert_eq!(send(&mut r, A, SENDER_GAP_MS - 1), Err(Reject::Burst));
        assert_eq!(send(&mut r, A, SENDER_GAP_MS), Ok(()));
        // Others fill the shared window; the next one waits for the window to move on.
        for i in 0..BURST as u64 - 2 {
            assert_eq!(
                send(&mut r, 0x100 + u128::from(i), SENDER_GAP_MS + 1 + i),
                Ok(())
            );
        }
        assert_eq!(send(&mut r, 0xe, SENDER_GAP_MS + 100), Err(Reject::Burst));
        assert_eq!(send(&mut r, 0xe, BURST_WINDOW_MS + 10), Ok(()));
    }

    #[test]
    fn other_dimensions_and_distant_or_unloaded_patters_are_refused() {
        let mut r = Receiver::default();
        let mut c = ctx(Policy::Friends, 100);
        c.dimension = "minecraft:the_nether";
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 1, 1_000_100))),
            Err(Reject::Scope)
        );
        let mut c = ctx(Policy::Friends, 100);
        c.sender_position = None;
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 2, 1_000_100))),
            Err(Reject::Distance)
        );
        c.sender_position = Some([ACCEPT_DISTANCE + 0.5, 64.0, 0.0]);
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 3, 1_000_100))),
            Err(Reject::Distance)
        );
        c.sender_position = Some([ACCEPT_DISTANCE, 64.0, 0.0]);
        assert_eq!(
            r.accept(&c, &inbound(A, Trust::Friend, event(B, 4, 1_000_100))),
            Ok(())
        );
    }

    #[test]
    fn remote_arms_are_independent_bounded_and_reset() {
        let mut r = Receiver::default();
        for i in 0..(MAX_REMOTE as u128 + 3) {
            let sender = 0x100 + i;
            let now = i as u64;
            let c = Context {
                now,
                ..ctx(Policy::Everyone, now)
            };
            // Different senders arrive faster than the global burst allows only in this
            // test's bookkeeping: space them out of the burst window.
            let c = Context {
                now: now * BURST_WINDOW_MS,
                wall: 1_000_000 + now * BURST_WINDOW_MS,
                ..c
            };
            let body = event(B, 1, 1_000_000 + now * BURST_WINDOW_MS);
            assert_eq!(
                r.accept(&c, &inbound(sender, Trust::Stranger, body)),
                Ok(())
            );
        }
        assert!(r.remote.len() <= MAX_REMOTE);
        let mut r = Receiver::default();
        r.accept(
            &ctx(Policy::Everyone, 100),
            &inbound(A, Trust::Stranger, event(B, 1, 1_000_100)),
        )
        .unwrap();
        r.accept(
            &ctx(Policy::Everyone, 300),
            &inbound(C, Trust::Stranger, event(B, 1, 1_000_300)),
        )
        .unwrap();
        let active = r.active(400);
        assert_eq!(active.iter().map(|a| a.0).collect::<Vec<_>>(), [A, C]);
        assert_ne!(active[0].1, active[1].1, "each arm keeps its own start");
        assert_eq!(
            r.active(100 + DURATION_MS).len(),
            1,
            "A finished, C still pats"
        );
        r.reset();
        assert!(r.active(400).is_empty());
    }

    #[test]
    fn the_inbox_is_bounded_and_drops_events_of_an_older_world() {
        let mut inbox = Inbox::default();
        for i in 0..MAX_INBOX + 4 {
            let mut e = inbound(A, Trust::Friend, event(B, i as u64 + 1, 1));
            e.generation = if i == 0 { 1 } else { 2 };
            let accepted = inbox.push(e);
            assert_eq!(accepted, i < MAX_INBOX, "{i}");
        }
        let current = inbox.drain(2);
        assert_eq!(
            current.len(),
            MAX_INBOX - 1,
            "the first one belongs to world 1"
        );
        assert!(inbox.drain(2).is_empty());
    }

    #[test]
    fn policies_persist_by_key_and_keep_the_default_for_unknown_values() {
        for p in [Policy::Friends, Policy::Everyone, Policy::Off] {
            assert_eq!(Policy::parse(Some(p.key())), p);
        }
        assert_eq!(Policy::parse(None), Policy::Friends);
        assert_eq!(Policy::parse(Some("garbage")), Policy::Friends);
        assert_eq!(Policy::Friends.next(), Policy::Everyone);
        assert_eq!(Policy::Everyone.next(), Policy::Off);
        assert_eq!(Policy::Off.next(), Policy::Friends);
    }

    #[test]
    fn notices_keep_the_name_as_plain_text_in_both_languages() {
        assert_eq!(
            notice(Language::English, "Azael_Nyaa"),
            "Azael_Nyaa patted you"
        );
        assert!(notice(Language::Russian, "Azael_Nyaa").contains("Azael_Nyaa"));
    }
}
