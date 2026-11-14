// Shared by the standalone test library and the injected Horizon client module.
use claim_borders_relay_client::translation::Language;
use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const ID: &str = "draimhorizon";
/// Key under which settings and data were saved before the mod id became `draimhorizon`.
pub const SETTINGS_ID: &str = "claim_borders";
pub const NAME: &str = "DraimHorizon";
pub const VERSION: &str = "0.40.3";
pub const DESCRIPTION: &str = "M: друзья, карта, чат. B: торги. J: карта, X: метка. /horizonmenu";
pub fn description_in(language: Language) -> &'static str {
    language.text(
        DESCRIPTION,
        "M: friends, map, chat. B: trades. J: map, X: waypoint. /horizonmenu",
    )
}
const MAX_CLAIMS: usize = 4096;
const MAX_MESSAGE_BYTES: usize = 262_144;
const WORLD_LIMIT: i32 = 30_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(test)]
const DRAW_DISTANCE: f64 = 160.0;

#[cfg(test)]
type Point = [f64; 3];
#[cfg(test)]
pub type Segment = (Point, Point);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Claim {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ParseError {
    TooLarge,
    Header,
    Entry,
    Coordinate,
    Count,
}

// Coordinates in Horizon SMP's response are block origins (chunk * 16).
// Parse the complete unwrapped server component, never individual HUD rows.
pub fn parse_claims(message: &str) -> Result<Option<Vec<Claim>>, ParseError> {
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(ParseError::TooLarge);
    }
    let text = strip_formatting(message);
    let Some(rest) = text.trim().strip_prefix("Your chunks (") else {
        return Ok(None);
    };
    let (counts, body) = rest.split_once("):").ok_or(ParseError::Header)?;
    let (held, limit) = counts.split_once(" of ").ok_or(ParseError::Header)?;
    let held: usize = held.parse().map_err(|_| ParseError::Header)?;
    let _limit: usize = limit.parse().map_err(|_| ParseError::Header)?;
    if held > MAX_CLAIMS {
        return Err(ParseError::Count);
    }
    if held == 0 {
        return if body.trim() == "none, /claim where you stand" {
            Ok(Some(Vec::new()))
        } else {
            Err(ParseError::Count)
        };
    }
    let mut claims = BTreeSet::new();
    for entry in body.split(';') {
        if claims.len() >= held {
            return Err(ParseError::Count);
        }
        let words: Vec<_> = entry.split_whitespace().collect();
        if words.len() != 6 || words[1] != "at" || words[2] != "x" || words[4] != "z" {
            return Err(ParseError::Entry);
        }
        let dimension = normalize_dimension(words[0]).ok_or(ParseError::Entry)?;
        let x = parse_origin(words[3])?;
        let z = parse_origin(words[5])?;
        if !claims.insert(Claim { dimension, x, z }) {
            return Err(ParseError::Count);
        }
    }
    if claims.len() != held {
        return Err(ParseError::Count);
    }
    Ok(Some(claims.into_iter().collect()))
}

pub fn strip_formatting(text: &str) -> String {
    let mut chars = text.chars();
    let mut plain = String::with_capacity(text.len());
    while let Some(c) = chars.next() {
        if c == '\u{00a7}' {
            let _ = chars.next();
        } else {
            plain.push(c);
        }
    }
    plain
}

fn normalize_dimension(value: &str) -> Option<String> {
    let dimension = if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    };
    let (namespace, path) = dimension.split_once(':')?;
    if namespace.is_empty()
        || path.is_empty()
        || !namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        || !path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-/".contains(&c))
    {
        return None;
    }
    Some(dimension)
}

fn parse_origin(value: &str) -> Result<i32, ParseError> {
    let n: i32 = value.parse().map_err(|_| ParseError::Coordinate)?;
    if n.rem_euclid(16) != 0 || !(-WORLD_LIMIT..=WORLD_LIMIT - 16).contains(&n) {
        return Err(ParseError::Coordinate);
    }
    Ok(n)
}

#[derive(Default)]
pub struct State {
    pub enabled: bool,
    claims: Vec<Claim>,
    pending: Option<Instant>,
    claims_known: bool,
}

impl State {
    pub fn clear(&mut self) {
        self.claims.clear();
        self.pending = None;
        self.claims_known = false;
    }

    pub fn restore(&mut self, claims: Vec<Claim>) {
        self.claims = claims;
        self.claims_known = true;
        self.pending = None;
    }

    pub fn begin_refresh(&mut self) {
        // Never keep old ownership visible if the server rejects the refresh.
        self.clear();
        self.pending = Some(Instant::now());
    }

    pub fn receive_chat(&mut self, text: &str) -> bool {
        let Some(requested) = self.pending else {
            return false;
        };
        if requested.elapsed() > REQUEST_TIMEOUT {
            self.clear();
            return false;
        }
        match parse_claims(text) {
            Ok(Some(claims)) => {
                self.claims = claims;
                self.claims_known = true;
                self.pending = None;
                true
            }
            // Unrelated chat, malformed/truncated lists, and trusted-player
            // names must never create a partial or invented claimed region.
            Ok(None) | Err(_) => false,
        }
    }

    pub fn shared_claims(&self) -> Option<Vec<Claim>> {
        self.claims_known.then(|| self.claims.clone())
    }

    pub fn count(&self) -> usize {
        self.claims.len()
    }

    #[cfg(test)]
    pub fn boundaries(&self, dimension: &str, player: Point) -> Vec<Segment> {
        if !self.enabled || !player.iter().all(|n| n.is_finite()) {
            return Vec::new();
        }
        let Some(dimension) = normalize_dimension(dimension) else {
            return Vec::new();
        };
        let chunks: BTreeSet<_> = self
            .claims
            .iter()
            .filter(|c| c.dimension == dimension)
            .map(|c| (c.x, c.z))
            .collect();
        let bottom = player[1].floor() + 0.05;
        let top = bottom + 4.0;
        let mut lines = Vec::new();
        let mut corners = BTreeSet::new();
        for &(x, z) in &chunks {
            let nearest_x = player[0].clamp(f64::from(x), f64::from(x + 16));
            let nearest_z = player[2].clamp(f64::from(z), f64::from(z + 16));
            if (player[0] - nearest_x).hypot(player[2] - nearest_z) > DRAW_DISTANCE {
                continue;
            }
            let edges = [
                ((x - 16, z), (x, z), (x, z + 16)),
                ((x + 16, z), (x + 16, z), (x + 16, z + 16)),
                ((x, z - 16), (x, z), (x + 16, z)),
                ((x, z + 16), (x, z + 16), (x + 16, z + 16)),
            ];
            for (neighbor, a, b) in edges {
                if chunks.contains(&neighbor) {
                    continue;
                }
                for y in [bottom, top] {
                    lines.push((
                        [f64::from(a.0), y, f64::from(a.1)],
                        [f64::from(b.0), y, f64::from(b.1)],
                    ));
                }
                corners.insert(a);
                corners.insert(b);
            }
        }
        for (x, z) in corners {
            lines.push((
                [f64::from(x), bottom, f64::from(z)],
                [f64::from(x), top, f64::from(z)],
            ));
        }
        lines
    }
}

pub fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    let mut state = STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    f(&mut state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_description_is_not_cyrillic() {
        let text = description_in(Language::English);
        assert!(!text.chars().any(|c| matches!(c, 'А'..='я' | 'Ё' | 'ё')));
    }
    const SCREENSHOT: &str = "Your chunks (4 of 4): overworld at x -1904 z 48; overworld at x -1904 z 64; overworld at x -1920 z 48; overworld at x -1936 z 48";

    fn loaded(text: &str) -> State {
        let mut state = State {
            enabled: true,
            ..State::default()
        };
        state.begin_refresh();
        assert!(state.receive_chat(text));
        state
    }

    #[test]
    fn claim_borders_are_opt_in() {
        assert!(!State::default().enabled);
    }

    #[test]
    fn only_verified_server_claims_can_be_shared() {
        let mut state = State::default();
        assert!(state.shared_claims().is_none());
        state.begin_refresh();
        assert!(!state.receive_chat("Your chunks (4 of 4): overworld at x -1904 z 48"));
        assert!(state.shared_claims().is_none());
        assert!(state.receive_chat(SCREENSHOT));
        assert_eq!(state.shared_claims().unwrap().len(), 4);
        state.clear();
        assert!(state.shared_claims().is_none());
    }

    #[test]
    fn screenshot_coordinates_are_block_origins() {
        let claims = parse_claims(SCREENSHOT).unwrap().unwrap();
        assert_eq!(claims.len(), 4);
        assert!(claims.contains(&Claim {
            dimension: "minecraft:overworld".into(),
            x: -1904,
            z: 48
        }));
        assert!(!claims.iter().any(|c| c.x == -1904 * 16));
    }

    #[test]
    fn accepts_formatting_namespaced_dimensions_and_whitespace() {
        let claims = parse_claims(
            "§aYour chunks (2 of 4):\u{00a0}the_nether at x -16 z 0;\nmy_mod:moon at x 0 z 32§r",
        )
        .unwrap()
        .unwrap();
        assert_eq!(claims[0].dimension, "minecraft:the_nether");
        assert_eq!(claims[1].dimension, "my_mod:moon");
    }

    #[test]
    fn lowered_server_quota_does_not_delete_existing_claims() {
        assert_eq!(
            parse_claims(&SCREENSHOT.replace("4 of 4", "4 of 2"))
                .unwrap()
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn empty_claims_clear_previous_ownership() {
        let mut state = loaded(SCREENSHOT);
        state.begin_refresh();
        assert!(state.receive_chat("Your chunks (0 of 4): none, /claim where you stand"));
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn rejects_partial_duplicates_overflow_and_unaligned_origins() {
        for message in [
            "Your chunks (2 of 4): overworld at x 0 z 0",
            "Your chunks (2 of 4): overworld at x 0 z 0; overworld at x 0 z 0",
            "Your chunks (1 of 4): overworld at x 2147483648 z 0",
            "Your chunks (1 of 4): overworld at x -1 z 0",
            "Your chunks (1 of 4): overworld at x 30000000 z 0",
            "Your chunks (1 of 4): overworld at x 0 z 0 trailing",
            "Your chunks (1 of 4): evil:bad:dimension at x 0 z 0",
        ] {
            assert!(parse_claims(message).is_err(), "{message}");
        }
    }

    #[test]
    fn ignores_unrelated_chat_and_unsolicited_or_duplicate_responses() {
        let mut state = State::default();
        assert!(!state.receive_chat(SCREENSHOT));
        state.begin_refresh();
        assert!(!state.receive_chat("Trusted: NewDiDos, Amazingidoshka, SukaDolosys"));
        assert!(!state.receive_chat("<Player> Your chunks (1 of 4): overworld at x 0 z 0"));
        assert!(state.receive_chat(SCREENSHOT));
        assert!(!state.receive_chat("Your chunks (0 of 4): none, /claim where you stand"));
        assert_eq!(state.count(), 4);
    }

    #[test]
    fn bounded_input_and_timeout() {
        assert_eq!(
            parse_claims(&"x".repeat(MAX_MESSAGE_BYTES + 1)),
            Err(ParseError::TooLarge)
        );
        assert!(parse_claims("Your chunks (4097 of 9999): none").is_err());
        let mut state = State {
            pending: Some(Instant::now() - REQUEST_TIMEOUT - Duration::from_secs(1)),
            ..State::default()
        };
        assert!(!state.receive_chat(SCREENSHOT));
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn one_chunk_is_a_sixteen_block_fence() {
        let state = loaded("Your chunks (1 of 4): overworld at x -16 z 0");
        let lines = state.boundaries("minecraft:overworld", [-8.0, 65.9, 8.0]);
        assert_eq!(lines.len(), 12);
        assert!(
            lines
                .iter()
                .flat_map(|(a, b)| [a, b])
                .all(|p| (-16.0..=0.0).contains(&p[0]) && (0.0..=16.0).contains(&p[2]))
        );
        assert!(lines.iter().any(|(a, b)| a[0] == -16.0 && b[0] == 0.0));
        assert!(
            lines
                .iter()
                .flat_map(|(a, b)| [a, b])
                .all(|p| [65.05, 69.05].contains(&p[1]))
        );
    }

    #[test]
    fn adjacent_claims_have_no_internal_wall() {
        let state = loaded("Your chunks (2 of 4): overworld at x 0 z 0; overworld at x 16 z 0");
        let lines = state.boundaries("overworld", [8.0, 64.0, 8.0]);
        assert!(
            !lines
                .iter()
                .any(|(a, b)| a[0] == 16.0 && b[0] == 16.0 && a[2] != b[2])
        );
        assert_eq!(lines.len(), 18);
    }

    #[test]
    fn dimensions_distance_visibility_and_disconnect_are_isolated() {
        let mut state = loaded(SCREENSHOT);
        assert!(
            state
                .boundaries("the_nether", [-1900.0, 64.0, 48.0])
                .is_empty()
        );
        assert!(state.boundaries("overworld", [0.0, 64.0, 0.0]).is_empty());
        assert!(
            state
                .boundaries("overworld", [f64::NAN, 64.0, 0.0])
                .is_empty()
        );
        assert!(
            !state
                .boundaries("overworld", [-1900.0, 64.0, 48.0])
                .is_empty()
        );
        state.enabled = false;
        assert!(
            state
                .boundaries("overworld", [-1900.0, 64.0, 48.0])
                .is_empty()
        );
        state.clear();
        state.enabled = true;
        assert!(
            state
                .boundaries("overworld", [-1900.0, 64.0, 48.0])
                .is_empty()
        );
    }
}
