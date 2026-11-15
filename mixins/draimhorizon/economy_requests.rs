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
/// A typed command whose reply has not been seen yet also keeps routine sends back, up to this
/// long, so a slow reply is not overtaken by an automatic query of the same family.
pub const MANUAL_WAIT_MS: u64 = 10_000;
struct Window {
    command: String,
    at: u64,
    left: u8,
}
/// A typed command whose own answer has not been seen yet.
struct Awaiting {
    /// The normalized command: `bal` (also for `balance`), `worth` or `worth <item>`.
    command: String,
    until: u64,
    /// Matching answers to let pass first: one when an automatic query of the same command was
    /// already in flight, since its answer looks exactly like this one's.
    skip: u8,
}
/// Typed commands waited for at once; past this the oldest entry becomes [`UNANSWERABLE`].
const MAX_AWAITING: usize = 4;
/// A wait no answer matches: it lasts until its deadline.
const UNANSWERABLE: &str = "?";
#[derive(Default)]
pub struct Replies {
    window: Option<Window>,
    hold_until: u64,
    awaiting: Vec<Awaiting>,
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
/// `worth <item>` with the item in the form the answers name it, so that `Diamond`,
/// `diamond` and `minecraft:diamond` are one command.
fn canon(command: &str) -> String {
    match command.strip_prefix("worth ") {
        Some(item) => format!("worth {}", model::id(item).unwrap_or_else(|| item.into())),
        None => command.into(),
    }
}
/// A typed routine-family command in the form its answers are matched against.
fn normalize(command: &str) -> String {
    let mut words = command.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("bal" | "balance"), None, _) => "bal".into(),
        (Some("worth"), None, _) => "worth".into(),
        (Some("worth"), Some(item), None) => canon(&format!("worth {item}")),
        // Anything else (extra words) matches no answer and is simply waited out.
        _ => "worth ?".into(),
    }
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
        self.live(now) || now < self.hold_until || self.awaiting.iter().any(|a| now < a.until)
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
            let normalized = normalize(command);
            // An automatic query of the same command already in flight will be answered with
            // text that looks exactly like this command's answer.
            let skip = u8::from(
                self.live(now)
                    && self
                        .window
                        .as_ref()
                        .is_some_and(|w| canon(&w.command) == normalized),
            );
            self.window = None;
            self.hold_until = now.saturating_add(MANUAL_HOLD_MS);
            let until = now.saturating_add(MANUAL_WAIT_MS);
            self.awaiting.retain(|a| now < a.until);
            if self.awaiting.len() >= MAX_AWAITING {
                // The list is full and this command cannot get an entry of its own. Turn the
                // oldest entry into one nothing can answer, so it holds until its deadline and
                // covers the commands that did not fit; it can no longer be released early by
                // the oldest command's own answer.
                self.awaiting[0].command = UNANSWERABLE.into();
                self.awaiting[0].skip = 0;
                self.awaiting[0].until = until;
            } else {
                self.awaiting.push(Awaiting {
                    command: normalized,
                    until,
                    skip,
                });
            }
        }
    }
    pub fn clear(&mut self) {
        self.window = None;
        self.hold_until = 0;
        self.awaiting.clear();
    }
    /// One incoming chat message, at the point it is about to be shown. Advances the budget and
    /// says whether this message is recognized output of the watched command. `recognized` is
    /// whether the economy book parsed it for this very delivery.
    pub fn deliver(&mut self, text: &str, recognized: bool, now: u64, quiet: bool) -> bool {
        if recognized {
            // Only an answer to a typed command ends its wait, one entry per line: an unrelated
            // quote, balance, Sold or Bought line does not, and a first line is not taken as proof
            // that a multi-line answer is complete (the 3-second hold still covers the rest).
            if let Some(i) = self
                .awaiting
                .iter()
                .position(|a| now < a.until && answers_strict(&a.command, text))
            {
                // The reply is being observed now, so the grace for its remaining lines starts
                // now, not at the time the command was typed.
                self.hold_until = self.hold_until.max(now.saturating_add(MANUAL_HOLD_MS));
                if self.awaiting[i].skip > 0 {
                    self.awaiting[i].skip -= 1;
                } else {
                    self.awaiting.remove(i);
                }
            }
        }
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
/// Like [`answers`], but a `worth <item>` is only answered by lines that name that item: the
/// generic "Selling today" line also belongs to any other `worth` in flight.
fn answers_strict(command: &str, text: &str) -> bool {
    if command.starts_with("worth ") && text.starts_with("Selling today: ") {
        return false;
    }
    !text.starts_with("Sold ") && !text.starts_with("Bought ") && answers(command, text)
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
        for (typed, answer) in [
            ("worth leather", QUOTE),
            ("worth", TODAY),
            ("bal", "Balance: 50 coins"),
            ("balance", "Balance: 50 coins"),
        ] {
            let mut r = Replies::default();
            r.arm("worth gravel", 100, true);
            r.manual(typed, 101);
            assert!(!r.deliver(answer, true, 102, true), "{typed}");
            assert!(r.busy(102));
            // Its own answer was shown, so only the 3-second hold remains.
            // The grace for the rest of its output runs from when the answer was seen (102).
            assert!(r.busy(101 + MANUAL_HOLD_MS), "{typed}");
            assert!(!r.busy(102 + MANUAL_HOLD_MS), "the hold ends: {typed}");
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
    const LATE: u64 = 101 + MANUAL_HOLD_MS + 500;
    #[test]
    fn unrelated_economy_output_never_releases_a_typed_commands_protection() {
        for unrelated in [
            "Balance: 50 coins",
            QUOTE,
            "Sold 1 leather for 3 coins.",
            "Bought 1 leather for 3 coins.",
            TODAY,
            "You may sell 640 more leather today.",
        ] {
            let mut r = Replies::default();
            r.manual("worth diamond", 100);
            assert!(!r.deliver(unrelated, true, 150, true));
            assert!(r.busy(LATE), "still waiting after the hold: {unrelated}");
            assert!(r.busy(100 + MANUAL_WAIT_MS - 1));
            assert!(!r.busy(100 + MANUAL_WAIT_MS), "bounded");
        }
    }
    #[test]
    fn a_slow_answer_starts_its_grace_when_it_is_observed() {
        let mut r = Replies::default();
        r.manual("worth leather", 1_000);
        assert!(!r.deliver(QUOTE, true, 5_500, true));
        assert!(
            r.busy(5_600),
            "the rest of a multi-line answer may still be coming"
        );
        assert!(r.busy(5_500 + MANUAL_HOLD_MS - 1));
        assert!(
            !r.busy(5_500 + MANUAL_HOLD_MS),
            "only now may automatic queries resume"
        );
        // The same for an answer that is skipped because an automatic look-alike was in flight.
        let mut r = Replies::default();
        assert!(r.arm("worth leather", 900, true));
        r.manual("worth leather", 1_000);
        r.deliver(QUOTE, true, 5_500, true);
        assert!(r.busy(5_600));
        assert!(r.busy(5_500 + MANUAL_HOLD_MS - 1));
        // An unrelated line observed late grants nothing.
        let mut r = Replies::default();
        r.manual("worth leather", 1_000);
        r.deliver("Balance: 5 coins", true, 5_500, true);
        assert!(r.busy(5_600), "still awaiting its own answer");
    }
    #[test]
    fn more_typed_commands_than_fit_hold_until_the_deadline_whatever_is_answered() {
        let mut r = Replies::default();
        for (i, c) in ["bal", "worth a", "worth b", "worth c", "worth d"]
            .iter()
            .enumerate()
        {
            r.manual(c, 1_000 + i as u64);
        }
        // The first four commands are answered; the fifth had no entry of its own.
        r.deliver("Balance: 1 coins", true, 2_000, true);
        for item in ["a", "b", "c"] {
            r.deliver(
                &format!("{item}: 1 coins each, to buy or sell."),
                true,
                2_010,
                true,
            );
        }
        assert!(
            r.busy(2_010 + MANUAL_HOLD_MS + 500),
            "the omitted command is still covered"
        );
        assert!(r.busy(1_004 + MANUAL_WAIT_MS - 1));
        assert!(!r.busy(1_004 + MANUAL_WAIT_MS));
        // The list stays strictly bounded however many are typed.
        let mut r = Replies::default();
        for i in 0..100 {
            r.manual("worth diamond", 1_000 + i);
        }
        assert!(r.awaiting.len() <= MAX_AWAITING);
    }
    #[test]
    fn its_own_answer_ends_the_wait_and_aliases_are_normalized() {
        let mut r = Replies::default();
        r.manual("worth Diamond", 100);
        assert!(!r.deliver("diamond: 100 coins each, to buy or sell.", true, 200, true));
        assert!(!r.busy(LATE), "the matching quote ended the wait");
        // `balance` is waited for as `bal`.
        let mut r = Replies::default();
        r.manual("balance", 100);
        assert!(!r.deliver(QUOTE, true, 150, true));
        assert!(r.busy(LATE));
        assert!(!r.deliver("Balance: 9 coins", true, 160, true));
        assert!(!r.busy(LATE));
        // A rejection of the very item answers it too.
        let mut r = Replies::default();
        r.manual("worth brown_wool", 100);
        assert!(!r.deliver("The market doesn't trade brown wool.", true, 150, true));
        assert!(!r.busy(LATE));
    }
    #[test]
    fn overlapping_typed_commands_keep_the_protection_until_each_is_answered() {
        let mut r = Replies::default();
        r.manual("bal", 100);
        r.manual("worth diamond", 110);
        // The first answer releases only the command it answers.
        r.deliver("Balance: 9 coins", true, 200, true);
        assert!(r.busy(LATE), "worth diamond is still awaited");
        r.deliver("diamond: 100 coins each, to buy or sell.", true, 210, true);
        assert!(!r.busy(LATE + 100));
        // The same command twice needs two answers.
        let mut r = Replies::default();
        r.manual("bal", 100);
        r.manual("bal", 120);
        r.deliver("Balance: 9 coins", true, 200, true);
        assert!(r.busy(LATE));
        r.deliver("Balance: 9 coins", true, 210, true);
        assert!(!r.busy(LATE + 100));
        // More typed commands than the bound keep every protection and do not grow it.
        let mut r = Replies::default();
        for i in 0..10 {
            r.manual("worth diamond", 100 + i);
        }
        assert!(r.busy(LATE));
        assert!(!r.busy(100 + MANUAL_WAIT_MS + 20));
    }
    #[test]
    fn an_answer_already_in_flight_for_an_automatic_query_does_not_count_as_the_typed_ones() {
        let mut r = Replies::default();
        assert!(r.arm("worth diamond", 100, true));
        r.manual("worth diamond", 101); // typed while the automatic reply may still be coming
        assert!(!r.deliver("diamond: 100 coins each, to buy or sell.", true, 150, true));
        assert!(r.busy(LATE), "that first answer may be the automatic one's");
        assert!(!r.deliver("diamond: 100 coins each, to buy or sell.", true, 160, true));
        assert!(!r.busy(LATE));
        // A different item's automatic query does not delay the release.
        let mut r = Replies::default();
        assert!(r.arm("worth leather", 100, true));
        r.manual("worth diamond", 101);
        r.deliver("diamond: 100 coins each, to buy or sell.", true, 150, true);
        assert!(!r.busy(LATE));
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
