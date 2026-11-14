// Hide the replies to the mod's own routine checks (`worth`, `bal`), and only those.
//
// The server tags no reply with a request id, so attribution is a deliberately small window: when
// an automatic routine command was actually sent, the next `BUDGET` incoming chat messages are
// looked at, and a message is hidden only if it is output specific to that command. Every
// message advances the budget, hidden or not, and then the window closes; a short expiry is only
// a guard against a window that never receives its messages. A command typed by the player
// cancels the window at once and holds routine sends for a moment, so manual output is shown.
use super::claim_borders_economy as model;
/// Incoming messages watched after an automatic command (the owner's hypothesis; a `worth`
/// answer can be longer, and what is beyond the budget stays visible).
pub const BUDGET: u8 = 2;
/// Secondary guard: a window nobody answered is dropped after this many milliseconds.
pub const EXPIRY_MS: u64 = 4000;
/// After a command the player typed, routine sends wait this long so none overlaps its reply.
pub const MANUAL_HOLD_MS: u64 = 3000;
struct Window {
    command: String,
    at: u64,
    left: u8,
}
#[derive(Default)]
pub struct Replies {
    window: Option<Window>,
    hold_until: u64,
}
/// Whether `command` is one of the read-only routine checks the mod issues itself.
pub fn routine(command: &str) -> bool {
    command == "bal"
        || command == "worth"
        || command
            .strip_prefix("worth ")
            .is_some_and(|item| model::id(item).is_some())
}
/// Whether a typed command is a routine-family command (`balance` is the server's alias).
pub fn manual_family(command: &str) -> bool {
    matches!(
        command.split_whitespace().next(),
        Some("worth" | "bal" | "balance")
    )
}
impl Replies {
    fn live(&self, now: u64) -> bool {
        self.window
            .as_ref()
            .is_some_and(|w| w.left > 0 && now.saturating_sub(w.at) < EXPIRY_MS)
    }
    /// Whether a new routine send must wait: a window is still open, or a typed command was
    /// just issued.
    pub fn busy(&self, now: u64) -> bool {
        self.live(now) || now < self.hold_until
    }
    /// An automatic command was actually dispatched: watch its replies. Nothing is armed when
    /// quiet is off, the command is not a routine one, or another window is still open (an
    /// unresolved window is never overwritten).
    pub fn arm(&mut self, command: &str, now: u64, quiet: bool) -> bool {
        if !quiet || !routine(command) || self.live(now) {
            return false;
        }
        self.window = Some(Window {
            command: command.into(),
            at: now,
            left: BUDGET,
        });
        true
    }
    /// A command the player issued: cancel any window and hold routine sends.
    pub fn manual(&mut self, command: &str, now: u64) {
        if manual_family(command) {
            self.window = None;
            self.hold_until = now.saturating_add(MANUAL_HOLD_MS);
        }
    }
    pub fn clear(&mut self) {
        self.window = None;
        self.hold_until = 0;
    }
    /// One incoming chat message, at the point it is about to be shown. Advances the budget and
    /// says whether this message is recognized output of the watched command. `recognized` is
    /// whether the economy book parsed it for this very delivery.
    pub fn deliver(&mut self, text: &str, recognized: bool, now: u64, quiet: bool) -> bool {
        if !self.live(now) {
            self.window = None;
            return false;
        }
        let Some(window) = self.window.as_mut() else {
            return false;
        };
        window.left -= 1;
        let hide = quiet
            && recognized
            && !text.starts_with("Sold ")
            && !text.starts_with("Bought ")
            && answers(&window.command, text);
        if window.left == 0 {
            self.window = None;
        }
        hide
    }
}
/// Whether `text` is output specific to `command`: its quote, limits or rejection, or the
/// balance. Anything else, or another item, is not.
fn answers(command: &str, text: &str) -> bool {
    let item = model::quote_item(text).or_else(|| model::unsupported_item(text));
    if command == "bal" {
        return text.starts_with("Balance: ") || text.starts_with("Your balance: ");
    }
    let Some(expected) = command.strip_prefix("worth ") else {
        return command == "worth" && text.starts_with("Selling today: ");
    };
    let expected = model::id(expected);
    if let Some(item) = &item {
        return expected.as_ref() == Some(item);
    }
    text.starts_with("Selling today: ")
        || text.starts_with("You may sell ")
            && text
                .strip_suffix(" today.")
                .and_then(|s| s.split_once(" more "))
                .and_then(|(_, s)| model::id(s))
                == expected
        || text.starts_with("Moves: ")
            && text
                .rsplit_once("Chart: /market ")
                .and_then(|(_, s)| model::id(s))
                == expected
}
#[cfg(test)]
mod tests {
    use super::*;
    const QUOTE: &str = "leather: 3.01 coins each, to buy or sell.";
    const TODAY: &str = "Selling today: 50 coins left of your 100 coins";
    #[test]
    fn an_automatic_worth_hides_its_own_quote_and_limits_within_two_messages() {
        let mut r = Replies::default();
        assert!(r.arm("worth leather", 100, true));
        assert!(r.deliver(QUOTE, true, 101, true));
        assert!(r.deliver(TODAY, true, 102, true));
        // The budget is spent: a third line, even a matching one, stays visible.
        assert!(!r.deliver("You may sell 640 more leather today.", true, 103, true));
        assert!(!r.busy(103));
    }
    #[test]
    fn unrelated_messages_stay_visible_and_still_spend_the_budget() {
        let mut r = Replies::default();
        r.arm("worth leather", 100, true);
        assert!(!r.deliver("<Steve> hello", false, 101, true));
        assert!(r.deliver(QUOTE, true, 102, true));
        // Two messages passed: the window closed, so an identical line later is shown.
        assert!(!r.deliver(QUOTE, true, 103, true));
    }
    #[test]
    fn only_output_of_the_issued_command_is_hidden() {
        let mut r = Replies::default();
        r.arm("worth leather", 100, true);
        assert!(!r.deliver("diamond: 100 coins each, to buy or sell.", true, 101, true));
        assert!(!r.deliver("Sold 1 leather for 3 coins.", true, 102, true));
        let mut r = Replies::default();
        r.arm("worth", 100, true);
        assert!(!r.deliver("Balance: 50 coins", true, 101, true));
        let mut r = Replies::default();
        r.arm("bal", 100, true);
        assert!(r.deliver("Balance: 50 coins", true, 101, true));
        let mut r = Replies::default();
        r.arm("worth brown_wool", 100, true);
        assert!(r.deliver("The market doesn't trade brown wool.", true, 101, true));
        let mut r = Replies::default();
        r.arm("bal", 100, true);
        assert!(!r.deliver("Bought 1 leather for 3 coins.", true, 101, true));
    }
    #[test]
    fn an_unrecognized_or_unparsed_line_is_never_hidden() {
        let mut r = Replies::default();
        r.arm("bal", 100, true);
        assert!(!r.deliver("Balance: 50 coins", false, 101, true));
    }
    #[test]
    fn a_typed_command_cancels_the_window_and_holds_routine_sends() {
        for typed in ["worth leather", "worth", "bal", "balance"] {
            let mut r = Replies::default();
            r.arm("worth leather", 100, true);
            r.manual(typed, 101);
            assert!(!r.deliver(QUOTE, true, 102, true), "{typed}");
            assert!(r.busy(102));
            assert!(!r.busy(101 + MANUAL_HOLD_MS), "the hold ends");
            // A queued routine send right after the manual command cannot rearm hiding.
            assert!(r.busy(110));
        }
        // Commands outside the routine family leave an open window alone.
        let mut r = Replies::default();
        r.arm("worth leather", 100, true);
        r.manual("sell leather 1", 101);
        assert!(r.deliver(QUOTE, true, 102, true));
    }
    #[test]
    fn nothing_is_armed_when_quiet_is_off_the_command_is_unknown_or_a_window_is_open() {
        let mut r = Replies::default();
        assert!(!r.arm("worth leather", 100, false));
        assert!(!r.arm("sell leather 1", 100, true));
        assert!(!r.arm("worth not a real item!", 100, true));
        assert!(!r.deliver(QUOTE, true, 101, true));
        assert!(r.arm("bal", 100, true));
        assert!(!r.arm("worth leather", 101, true), "an open window is kept");
        assert!(r.deliver("Balance: 5 coins", true, 102, true));
        // Quiet switched off while a window is open hides nothing.
        let mut r = Replies::default();
        r.arm("bal", 100, true);
        assert!(!r.deliver("Balance: 5 coins", true, 101, false));
    }
    #[test]
    fn a_delayed_reply_after_expiry_is_visible_and_the_window_is_gone() {
        let mut r = Replies::default();
        r.arm("worth leather", 100, true);
        assert!(r.busy(100 + EXPIRY_MS - 1));
        assert!(!r.busy(100 + EXPIRY_MS));
        assert!(!r.deliver(QUOTE, true, 100 + EXPIRY_MS, true));
        // After a clear (reconnect, disable) nothing is hidden either.
        r.arm("worth leather", 9000, true);
        r.clear();
        assert!(!r.deliver(QUOTE, true, 9001, true));
    }
    #[test]
    fn a_worth_without_an_item_only_claims_its_selling_line() {
        let mut r = Replies::default();
        r.arm("worth", 100, true);
        assert!(r.deliver(TODAY, true, 101, true));
    }
}

pub fn next_watch<'a>(
    watch: &'a std::collections::BTreeSet<String>,
    unsupported: &std::collections::BTreeSet<String>,
    polled: &std::collections::BTreeMap<String, u64>,
    now: u64,
    interval: u64,
) -> Option<&'a str> {
    watch
        .iter()
        .filter(|id| {
            !unsupported.contains(*id)
                && polled
                    .get(*id)
                    .is_none_or(|at| now.saturating_sub(*at) >= interval)
        })
        .min_by_key(|id| polled.get(*id).copied().unwrap_or(0))
        .map(String::as_str)
}
#[cfg(test)]
mod scheduling_tests {
    use super::*;
    #[test]
    fn all_sixty_four_goods_are_queried_before_any_is_polled_twice() {
        let watch = (0..64).map(|i| format!("minecraft:item{i:02}")).collect();
        let unsupported = Default::default();
        let mut polled = std::collections::BTreeMap::new();
        for step in 0..64 {
            let now = 100000 + step * 2000;
            let item = next_watch(&watch, &unsupported, &polled, now, 30000)
                .unwrap()
                .to_owned();
            assert!(!polled.contains_key(&item));
            polled.insert(item, now);
        }
        assert_eq!(polled.len(), 64);
    }
}
