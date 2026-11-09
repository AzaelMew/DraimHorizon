// The pace of the auto-clicker: how often a button is pressed and for how long. Pure: the game
// adapter asks it every frame whether the button should be down at this moment.
/// Clicks per second the player can choose from; the last one holds the button down.
pub const RATES: [u16; 11] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 20];
/// How long one click keeps the button down: a game tick, so that the game sees it.
pub const PRESS_MS: u64 = 50;
/// The default choice: five clicks a second.
pub const DEFAULT_RATE: u8 = 4;
/// The rate for a stored index (a bad one gives the default).
pub fn rate(index: u8) -> u16 {
    RATES
        .get(usize::from(index))
        .copied()
        .unwrap_or(RATES[usize::from(DEFAULT_RATE)])
}
/// The next index in the list, round again after the last.
pub fn next_rate(index: u8) -> u8 {
    ((usize::from(index) + 1) % RATES.len()) as u8
}
/// "5 clicks/s", or "hold" for the fastest.
pub fn label(index: u8, russian: bool) -> String {
    let rate = rate(index);
    if rate >= *RATES.last().unwrap_or(&20) {
        return if russian {
            "удержание"
        } else {
            "hold"
        }
        .into();
    }
    if russian {
        format!("{rate} кл/с")
    } else {
        format!("{rate}/s")
    }
}
/// When to press: pulses of `PRESS_MS` every `1000 / rate` ms; at the top rate the button is
/// simply held.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clicker {
    next: Option<u64>,
    release: u64,
}
impl Clicker {
    pub const fn new() -> Self {
        Self {
            next: None,
            release: 0,
        }
    }
    /// Whether the button is down at `now_ms` (any steady clock) at `rate` clicks per second.
    pub fn down(&mut self, now_ms: u64, rate: u16) -> bool {
        let interval = 1000 / u64::from(rate.max(1));
        if interval <= PRESS_MS {
            return true;
        }
        let next = *self.next.get_or_insert(now_ms);
        if now_ms >= next {
            self.release = now_ms + PRESS_MS;
            self.next = Some(next + interval);
            // A long stall must not be paid back with a burst.
            if self.next.is_some_and(|n| n <= now_ms) {
                self.next = Some(now_ms + interval);
            }
        }
        now_ms < self.release
    }
    /// Forgets the pace (switched off, a screen opened): the next press comes at once.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// How many separate presses a clicker makes in `seconds` at `rate`, polled every `step` ms.
    fn presses(rate: u16, seconds: u64, step: u64) -> u32 {
        let mut clicker = Clicker::default();
        let (mut was_down, mut count) = (false, 0);
        for now in (0..seconds * 1000).step_by(step as usize) {
            let down = clicker.down(now, rate);
            if down && !was_down {
                count += 1;
            }
            was_down = down;
        }
        count
    }
    #[test]
    fn the_press_count_follows_the_rate_whatever_the_frame_rate() {
        for step in [4, 7, 16] {
            assert_eq!(presses(5, 4, step), 20, "5/s polled every {step} ms");
            assert_eq!(presses(1, 5, step), 5);
            assert_eq!(presses(10, 2, step), 20);
        }
    }
    #[test]
    fn the_top_rate_holds_the_button_and_a_stall_is_not_paid_back() {
        let mut clicker = Clicker::default();
        assert!((0..100).all(|now| clicker.down(now * 7, 20)));
        let mut clicker = Clicker::default();
        assert!(clicker.down(0, 2));
        // Ten seconds of nothing: one press now, not twenty.
        assert!(clicker.down(10_000, 2));
        assert!(!clicker.down(10_060, 2));
        assert!(!clicker.down(10_400, 2));
        assert!(clicker.down(10_500, 2));
    }
    #[test]
    fn rates_cycle_and_label_in_both_languages() {
        assert_eq!(rate(4), 5);
        assert_eq!(rate(200), RATES[usize::from(DEFAULT_RATE)]);
        assert_eq!(next_rate(RATES.len() as u8 - 1), 0);
        assert_eq!(label(4, true), "5 кл/с");
        assert_eq!(label(4, false), "5/s");
        assert_eq!(label(RATES.len() as u8 - 1, false), "hold");
        assert_eq!(label(RATES.len() as u8 - 1, true), "удержание");
    }
}
