// Hide the reply to the mod's own background `/claims` refresh, and only that.
//
// The same bounded policy as the routine economy replies (economy_requests): when an automatic
// `/claims` was actually sent, the next `BUDGET` incoming chat messages are looked at, and a
// message is hidden only if it is a complete, valid claims list; every message spends the
// budget and then the window closes. A `/claims` the player types cancels the window at once, so
// its list is shown. The window lives here, not in the Economy module, so it does not depend on
// the Economy module being enabled. Map and claim state are updated by the parser either way.
use super::claim_borders_economy_requests::{BUDGET, EXPIRY_MS, MANUAL_HOLD_MS, MANUAL_WAIT_MS};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Default)]
pub struct Replies {
    /// (opened at, messages left)
    window: Option<(u64, u8)>,
    hold_until: u64,
    /// A typed command is still waiting for its reply until this time ...
    awaiting_until: u64,
    /// ... or until this many recognized replies have been shown: its own, plus one for each
    /// automatic request already in flight (whose reply looks exactly the same).
    awaiting_replies: u8,
    /// More typed commands than the count can hold: wait until the deadline whatever is shown.
    awaiting_overflow: bool,
    /// The text of the delivery the parser just accepted as a claims list; taken once.
    recognized: Option<String>,
    /// The connection scope this state belongs to; `None` until the first frame binds it.
    session: Option<String>,
}
impl Replies {
    /// An empty state, usable in a `static`.
    pub const fn new() -> Self {
        Self {
            window: None,
            hold_until: 0,
            awaiting_until: 0,
            awaiting_replies: 0,
            awaiting_overflow: false,
            recognized: None,
            session: None,
        }
    }
    fn live(&self, now: u64) -> bool {
        self.window
            .is_some_and(|(at, left)| left > 0 && now.saturating_sub(at) < EXPIRY_MS)
    }
    /// Whether a new automatic send should wait: a window is open or a typed command was just run.
    pub fn busy(&self, now: u64) -> bool {
        self.live(now)
            || now < self.hold_until
            || (now < self.awaiting_until && (self.awaiting_replies > 0 || self.awaiting_overflow))
    }
    /// An automatic `/claims` was actually sent. Never overwrites an open window, and never arms
    /// right after a typed command.
    pub fn arm(&mut self, now: u64) -> bool {
        if self.busy(now) {
            return false;
        }
        self.window = Some((now, BUDGET));
        true
    }
    /// The player typed `/claims`: cancel the window and any pending recognition now. This
    /// does not need the connection to be bound yet: a command typed before the first frame
    /// of a connection keeps its protection when that frame binds the scope.
    pub fn manual(&mut self, now: u64) {
        // A reply already in flight for an automatic request arrives looking just like this
        // command's own, so one more recognized reply is waited for; several typed commands
        // add up, up to a small bound.
        let in_flight = u8::from(self.live(now));
        let waiting = now < self.awaiting_until;
        if !waiting {
            self.awaiting_replies = 0;
            self.awaiting_overflow = false;
        }
        let total = self.awaiting_replies.saturating_add(1 + in_flight);
        // Past the bound the count cannot say how many replies are owed: hold until the deadline.
        self.awaiting_overflow |= total > 4;
        self.awaiting_replies = total.min(4);
        self.window = None;
        self.recognized = None;
        self.hold_until = now.saturating_add(MANUAL_HOLD_MS);
        self.awaiting_until = now.saturating_add(MANUAL_WAIT_MS);
    }
    /// The connection ended or the mod was switched off: nothing carries over.
    pub fn leave(&mut self) {
        *self = Self::default();
    }
    /// A frame inside a connection. The first bind adopts the scope and keeps what a typed
    /// command already set; a different scope (another server or account) starts clean.
    pub fn bind(&mut self, scope: &str) {
        match &self.session {
            Some(current) if current == scope => {}
            Some(_) => {
                self.leave();
                self.session = Some(scope.to_owned());
            }
            None => self.session = Some(scope.to_owned()),
        }
    }
    /// Remember that the delivery about to be shown is recognized output of this command family.
    pub fn recognize(&mut self, text: &str) {
        self.recognized = Some(text.to_owned());
    }
    /// The chat hook, for one server line: remember it when it is a complete valid claims list.
    pub fn observe(&mut self, text: &str) {
        use super::claim_borders as claims;
        let text = claims::strip_formatting(text);
        if claims::parse_claims(text.trim()).ok().flatten().is_some() {
            self.recognized = Some(text);
        }
    }
    /// One shown incoming message. Advances the window and says whether this message is the
    /// automatic refresh's own list.
    pub fn deliver(&mut self, text: &str, quiet: bool, now: u64) -> bool {
        let text = super::claim_borders::strip_formatting(text);
        let recognized = self.recognized.take().is_some_and(|line| line == text);
        if recognized
            && now < self.awaiting_until
            && (self.awaiting_replies > 0 || self.awaiting_overflow)
        {
            // A reply to a typed command is being observed now: its remaining lines get their
            // grace from now. Ordinary automatic lines never extend anything.
            self.hold_until = self.hold_until.max(now.saturating_add(MANUAL_HOLD_MS));
            self.awaiting_replies = self.awaiting_replies.saturating_sub(1);
        }
        if !self.live(now) {
            self.window = None;
            return false;
        }
        let Some((at, left)) = self.window else {
            return false;
        };
        self.window = (left > 1).then_some((at, left - 1));
        quiet && recognized
    }
    /// The background refresh, once per connection: while a typed command protects the moment
    /// the send waits (and stays due); otherwise it is attempted exactly once, and hiding is
    /// armed only if `send` says it really went out.
    pub fn refresh(&mut self, due: &mut bool, now: u64, send: impl FnOnce() -> bool) {
        if !*due || self.busy(now) {
            return;
        }
        *due = false;
        if send() {
            self.arm(now);
        }
    }
}
static STATE: Mutex<Replies> = Mutex::new(Replies::new());
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn with<T>(f: impl FnOnce(&mut Replies) -> T) -> T {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()))
}
pub fn manual() {
    with(|r| r.manual(now()))
}
pub fn leave() {
    with(Replies::leave)
}
pub fn bind(scope: &str) {
    with(|r| r.bind(scope))
}
pub fn arm() -> bool {
    with(|r| r.arm(now()))
}
/// See [`Replies::refresh`].
pub fn refresh(due: &mut bool, send: impl FnOnce() -> bool) {
    with(|r| r.refresh(due, now(), send))
}
/// The chat hook, once per server line.
pub fn on_server_line(text: &str) {
    with(|r| r.observe(text))
}
/// The capture hook, once per shown incoming message: true when it is the automatic refresh's
/// own list and should be hidden.
pub fn on_shown_line(text: &str, quiet: bool) -> bool {
    with(|r| r.deliver(text, quiet, now()))
}
#[cfg(test)]
mod tests {
    use super::*;
    /// A real reply the parser accepts, as the server prints it.
    const LIST: &str = "Your chunks (2 of 10): overworld at x 0 z 0; overworld at x 16 z 0";
    /// A list cut off mid-entry: the parser rejects it, so it can never be recognized.
    const CUT: &str = "Your chunks (2 of 10): overworld at x 0 z 0; overworld at x 16 z";
    fn shown(r: &mut Replies, text: &str, quiet: bool, now: u64) -> bool {
        r.observe(text);
        r.deliver(text, quiet, now)
    }
    #[test]
    fn the_fixtures_are_what_the_parser_says_they_are() {
        use super::super::claim_borders as claims;
        assert!(claims::parse_claims(LIST).unwrap().is_some());
        assert!(claims::parse_claims(CUT).is_err());
    }
    #[test]
    fn an_automatic_refresh_hides_only_its_own_list_within_two_messages() {
        let mut r = Replies::default();
        assert!(r.arm(100));
        assert!(shown(&mut r, LIST, true, 101));
        // The second message of the window is judged on its own recognition, as any other.
        assert!(shown(&mut r, LIST, true, 102));
        assert!(!shown(&mut r, LIST, true, 103), "closed after two messages");
    }
    #[test]
    fn unrelated_messages_spend_the_budget_and_stay_visible() {
        let mut r = Replies::default();
        r.arm(100);
        assert!(!shown(&mut r, "<Steve> hi", true, 101));
        assert!(!shown(&mut r, "<Alex> yo", true, 102));
        assert!(!shown(&mut r, LIST, true, 103), "the window is spent");
    }
    #[test]
    fn a_typed_claims_command_cancels_the_window_and_is_never_hidden() {
        let mut r = Replies::default();
        r.arm(100);
        r.observe(LIST);
        r.manual(101);
        assert!(!r.deliver(LIST, true, 102));
        assert!(!r.arm(102), "no rearm inside the hold");
        assert!(r.busy(102));
        // The typed command's reply is still awaited, so automatic sends keep waiting past the
        // hold, but only up to a bound.
        assert!(r.busy(101 + MANUAL_HOLD_MS));
        assert!(!r.busy(101 + MANUAL_WAIT_MS));
        let mut r = Replies::default();
        assert!(!shown(&mut r, LIST, true, 100), "no window at all");
    }
    #[test]
    fn a_list_already_in_flight_for_the_automatic_refresh_does_not_release_the_typed_commands_wait()
    {
        let mut r = Replies::default();
        assert!(r.arm(100));
        r.manual(101); // typed while the automatic list may still be coming
        assert!(
            !shown(&mut r, LIST, true, 150),
            "the first list may be the automatic one"
        );
        assert!(r.busy(101 + MANUAL_HOLD_MS + 500));
        assert!(!shown(&mut r, LIST, true, 160));
        assert!(
            !r.busy(101 + MANUAL_HOLD_MS + 500),
            "the second list is the typed one's"
        );
        // Typed twice with nothing in flight: two lists are waited for, never more than a bound.
        let mut r = Replies::default();
        r.manual(100);
        r.manual(110);
        shown(&mut r, LIST, true, 200);
        assert!(r.busy(100 + MANUAL_HOLD_MS + 500));
        shown(&mut r, LIST, true, 210);
        assert!(!r.busy(100 + MANUAL_HOLD_MS + 500));
        let mut r = Replies::default();
        for i in 0..50 {
            r.manual(100 + i);
        }
        assert!(r.busy(100 + MANUAL_HOLD_MS + 500));
        assert!(!r.busy(100 + MANUAL_WAIT_MS + 60));
    }
    #[test]
    fn a_seen_reply_ends_the_wait_and_a_slow_one_is_not_overtaken_by_an_automatic_refresh() {
        let mut r = Replies::default();
        r.manual(1_000);
        // The reply is slower than the 3 s hold; an automatic refresh must still not arm.
        assert!(!r.arm(5_000), "typed reply still awaited");
        // It arrives and is shown at 5.5 s: the wait ends, but the grace for the rest of its
        // output starts now, not at the 1 s the command was typed.
        assert!(!shown(&mut r, LIST, true, 5_500));
        assert!(r.busy(5_600), "grace from the observed reply");
        assert!(!r.arm(5_600));
        assert!(r.busy(5_500 + MANUAL_HOLD_MS - 1));
        assert!(
            r.arm(5_500 + MANUAL_HOLD_MS),
            "only after 8.5 s may automatic resume"
        );
        assert!(
            shown(&mut r, LIST, true, 5_500 + MANUAL_HOLD_MS + 100),
            "now an automatic reply may hide"
        );
    }
    #[test]
    fn ordinary_automatic_lines_never_extend_the_hold() {
        let mut r = Replies::default();
        assert!(r.arm(100));
        assert!(shown(&mut r, LIST, true, 150));
        assert!(!shown(&mut r, "<Steve> hi", true, 151)); // the window is spent
        assert!(!r.busy(152), "an automatic reply starts no grace");
        // Nor does a list shown when nothing typed is awaited.
        let mut r = Replies::default();
        assert!(!shown(&mut r, LIST, true, 100));
        assert!(!r.busy(101));
    }
    #[test]
    fn more_typed_commands_than_the_count_holds_wait_until_the_deadline() {
        let mut r = Replies::default();
        for i in 0..5 {
            r.manual(1_000 + i);
        }
        // Five replies are owed but only four fit: the first four cannot release the wait.
        for i in 0..4 {
            shown(&mut r, LIST, true, 2_000 + i);
        }
        assert!(r.busy(2_000 + MANUAL_HOLD_MS + 500));
        assert!(r.busy(1_004 + MANUAL_WAIT_MS - 1));
        assert!(!r.busy(1_004 + MANUAL_WAIT_MS));
        // The overflow resets with the session.
        let mut r = Replies::default();
        for i in 0..6 {
            r.manual(1_000 + i);
        }
        r.leave();
        assert!(!r.busy(1_100));
        // And a command typed after the deadline starts clean.
        let mut r = Replies::default();
        for i in 0..6 {
            r.manual(1_000 + i);
        }
        r.manual(1_005 + MANUAL_WAIT_MS + 10);
        shown(&mut r, LIST, true, 1_005 + MANUAL_WAIT_MS + 20);
        assert!(!r.busy(1_005 + MANUAL_WAIT_MS + 20 + MANUAL_HOLD_MS));
    }
    #[test]
    fn nothing_hides_when_unrecognized_quiet_off_expired_or_left() {
        let mut r = Replies::default();
        r.arm(100);
        assert!(
            !shown(&mut r, CUT, true, 101),
            "a cut-off list is never a list"
        );
        let mut r = Replies::default();
        r.arm(100);
        assert!(!shown(&mut r, LIST, false, 101), "quiet off");
        let mut r = Replies::default();
        r.arm(100);
        assert!(!shown(&mut r, LIST, true, 100 + EXPIRY_MS), "expired");
        let mut r = Replies::default();
        r.arm(100);
        r.observe(LIST);
        r.leave();
        assert!(!r.deliver(LIST, true, 101), "connection ended");
    }
    #[test]
    fn an_open_window_is_never_overwritten() {
        let mut r = Replies::default();
        assert!(r.arm(100));
        assert!(!r.arm(101));
        assert!(r.busy(101));
    }
    #[test]
    fn a_typed_command_before_the_first_frame_keeps_its_protection_and_defers_the_refresh() {
        let mut r = Replies::default();
        r.manual(1_000);
        r.bind("server#me"); // the first frame of the connection
        let mut due = true;
        let mut sent = false;
        r.refresh(&mut due, 1_100, || {
            sent = true;
            true
        });
        assert!(!sent && due, "deferred, and still owed");
        assert!(!r.arm(1_100));
        // The list the typed command asked for is shown, and nothing is armed behind it.
        assert!(!shown(&mut r, LIST, true, 1_200));
        // The list was observed at 1.2 s, so the grace runs from then; once it is over the
        // refresh goes out once and arms.
        r.refresh(&mut due, 1_200 + MANUAL_HOLD_MS - 1, || {
            sent = true;
            true
        });
        assert!(!sent && due, "still inside the grace");
        r.refresh(&mut due, 1_200 + MANUAL_HOLD_MS, || {
            sent = true;
            true
        });
        assert!(sent && !due);
        assert!(shown(&mut r, LIST, true, 1_200 + MANUAL_HOLD_MS + 1));
    }
    #[test]
    fn a_refresh_that_did_not_go_out_arms_nothing_and_is_not_retried() {
        let mut r = Replies::default();
        let mut due = true;
        r.refresh(&mut due, 100, || false);
        assert!(!due && !r.busy(100));
        assert!(!shown(&mut r, LIST, true, 101));
        let mut calls = 0;
        r.refresh(&mut due, 102, || {
            calls += 1;
            true
        });
        assert_eq!(calls, 0, "one attempt per connection");
    }
    #[test]
    fn nothing_survives_a_reconnect_even_to_the_same_server_with_cached_land() {
        let mut r = Replies::default();
        r.bind("server#me");
        r.arm(100);
        r.observe(LIST); // recognized, but the connection ends before it is shown
        r.manual(101);
        r.leave(); // the reset hook: disconnect, reconnect or mod off
        r.bind("server#me");
        assert!(!r.busy(102), "no hold carried over");
        assert!(!r.deliver(LIST, true, 102), "no recognition carried over");
        assert!(r.arm(103), "a fresh connection can arm again");
    }
    #[test]
    fn switching_server_or_account_without_a_reset_starts_clean_but_the_same_scope_does_not() {
        let mut r = Replies::default();
        r.bind("a#me");
        r.arm(100);
        r.manual(101);
        r.bind("a#me");
        assert!(r.busy(102), "same connection, same protection");
        r.observe(LIST);
        r.bind("b#me");
        assert!(!r.busy(102));
        assert!(!r.deliver(LIST, true, 102));
        r.arm(103);
        r.bind("b#other");
        assert!(!r.busy(104), "another account is another connection");
    }
    #[test]
    fn disabling_and_reenabling_the_mod_starts_clean() {
        let mut r = Replies::default();
        r.bind("a#me");
        r.arm(100);
        r.observe(LIST);
        r.leave(); // every frame while the mod is off or there is no connection
        r.leave();
        r.bind("a#me");
        assert!(!r.deliver(LIST, true, 101));
        assert!(!r.busy(101));
    }
    #[test]
    fn a_list_that_is_never_shown_leaves_no_recognition_for_a_later_session() {
        let mut r = Replies::default();
        r.bind("a#me");
        r.arm(100);
        r.observe(LIST); // chat hook ran, the capture hook never did (chat hidden)
        r.leave();
        r.bind("b#me");
        r.arm(200);
        assert!(
            !r.deliver(LIST, true, 201),
            "recognition belonged to the old connection"
        );
    }
}
