//! The claims-list hiding seam: what the chat hook and the chat-capture hook do for one server
//! line, on the real parser and the real window state, plus the wiring of those hooks.
use claim_borders::{claim_borders as claims, claim_replies as replies};
use std::sync::Mutex;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
const LIST: &str = "Your chunks (4 of 4): overworld at x -1904 z 48; overworld at x -1904 z 64; overworld at x -1920 z 48; overworld at x -1936 z 48";
const NONE: &str = "Your chunks (0 of 10): none, /claim where you stand";

fn fresh() -> std::sync::MutexGuard<'static, ()> {
    let guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    replies::leave();
    guard
}
/// One shown server message: the chat hook, then the capture hook. True when it is hidden.
fn shown(text: &str, quiet: bool) -> bool {
    replies::on_server_line(text);
    replies::on_shown_line(text, quiet)
}
/// What the map state does with a line, independent of whether it is hidden.
fn parsed(text: &str) -> Vec<claims::Claim> {
    let mut state = claims::State::default();
    state.begin_refresh();
    state.receive_chat(text);
    state.shared_claims().unwrap_or_default()
}

#[test]
fn a_typed_claims_list_is_visible_and_still_updates_the_map() {
    let _g = fresh();
    replies::manual();
    assert!(!shown(LIST, true));
    assert_eq!(parsed(LIST).len(), 4);
    // Also with no window at all (a typed command while nothing is pending).
    replies::leave();
    assert!(!shown(LIST, true));
    assert!(!shown(NONE, true));
    assert!(parsed(NONE).is_empty());
}

#[test]
fn an_automatic_refresh_hides_its_own_list_while_the_map_still_updates() {
    let _g = fresh();
    assert!(replies::arm());
    assert!(shown(LIST, true));
    assert_eq!(parsed(LIST).len(), 4, "hidden does not mean ignored");
    replies::leave();
    replies::arm();
    assert!(shown(NONE, true), "zero claims is a valid list too");
    assert!(parsed(NONE).is_empty());
}

#[test]
fn formatted_text_is_recognized_and_malformed_or_partial_lists_fail_open() {
    let _g = fresh();
    replies::arm();
    assert!(shown(
        &format!("\u{a7}7Your chunks (4 of 4): \u{a7}f{}", &LIST[27..]),
        true
    ));
    for bad in [
        "Your chunks (4 of 4): overworld at x -1904 z 48",
        "Your chunks (4 of 4): overworld at x -1904 z",
        "Your chunks (x of y):",
        "Your chunks (1 of 4): none, /claim where you stand",
        "Your chunks (0 of 10): something else",
        "<Steve> Your chunks (4 of 4): overworld at x 0 z 0",
    ] {
        replies::leave();
        replies::arm();
        assert!(!shown(bad, true), "{bad}");
    }
}

#[test]
fn unrelated_messages_spend_the_two_message_budget_and_repeats_are_visible() {
    let _g = fresh();
    replies::arm();
    assert!(!shown("<Steve> hi", true));
    assert!(shown(LIST, true), "second message of the window");
    assert!(!shown(LIST, true), "a repeated identical list is shown");
    replies::leave();
    replies::arm();
    assert!(!shown("<Steve> hi", true));
    assert!(!shown("<Alex> yo", true));
    assert!(!shown(LIST, true), "the window is spent before the list");
}

#[test]
fn a_typed_claims_cancels_an_open_window_and_automatic_sends_yield() {
    let _g = fresh();
    assert!(replies::arm());
    replies::on_server_line(LIST); // an automatic reply recognized, not yet shown
    replies::manual();
    assert!(
        !replies::on_shown_line(LIST, true),
        "cancelled before it is shown"
    );
    assert!(
        !replies::arm(),
        "no automatic rearm right after a typed command"
    );
    assert!(!shown(LIST, true));
}

#[test]
fn quiet_off_or_a_reset_scope_hides_nothing() {
    let _g = fresh();
    replies::arm();
    assert!(!shown(LIST, false), "quiet off");
    replies::arm();
    replies::leave(); // reconnect, scope change or the mod switched off
    assert!(!shown(LIST, true));
}

fn source(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/mixins/draimhorizon/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn the_hooks_are_wired_to_origin_not_to_a_blanket_silencer() {
    let chat = source("chat.rs");
    assert!(chat.contains("claim_replies::on_server_line"));
    assert!(
        !chat.contains("economy_client::silence"),
        "the chat hook must not silence every parsed claims list"
    );
    let capture = source("chat_capture_hook.rs");
    assert!(capture.contains("claim_replies::on_shown_line"));
    let command = source("command.rs");
    let typed = command.find("name == \"claims\"").unwrap();
    let manual = command[typed..].find("claim_replies::manual()").unwrap();
    let refresh = command[typed..].find("begin_refresh").unwrap();
    assert!(
        manual < refresh,
        "a typed /claims cancels hiding before it dispatches"
    );
    // The background refresh is the only thing that arms, through the shared policy, and no
    // path wipes a typed command's protection before sending.
    let memory = source("claim_memory.rs");
    assert!(memory.contains("claim_replies::refresh("));
    assert!(!memory.contains("claim_replies::arm()"));
    assert!(!memory.contains("claim_replies::clear"));
    for file in ["client.rs", "command.rs", "chat.rs"] {
        assert!(!source(file).contains("claim_replies::arm()"), "{file}");
    }
    // The connection lifecycle: the reset hook, the per-frame no-connection/mod-off path and
    // the scope bind all reach the claims window.
    assert!(source("reset.rs").contains("claim_replies::leave()"));
    assert!(memory.contains("claim_replies::leave()"));
    assert!(memory.contains("claim_replies::bind("));
}

#[test]
fn production_calls_in_order_a_typed_claims_before_the_first_frame_defers_the_refresh() {
    let _g = fresh();
    // command.rs, then claim_memory's first frame, exactly as production calls them.
    replies::manual();
    replies::bind("play.example.org#0000000000000000000000000000000a");
    let mut due = true;
    let mut sent = false;
    replies::refresh(&mut due, || {
        sent = true;
        true
    });
    assert!(!sent && due, "the refresh waits and is still owed");
    assert!(
        !replies::arm(),
        "and nothing rearms behind the typed command"
    );
    assert!(!shown(LIST, true), "the typed command's list shows");
}

#[test]
fn production_calls_in_order_a_reconnect_with_cached_land_starts_clean() {
    let _g = fresh();
    replies::bind("play.example.org#a");
    replies::manual();
    replies::on_server_line(LIST); // recognized, never shown
    replies::leave(); // reset hook: disconnect
    replies::bind("play.example.org#a"); // reconnect, land comes from the cache: no cold refresh
    let mut due = false;
    replies::refresh(&mut due, || panic!("nothing is owed"));
    assert!(!replies::on_shown_line(LIST, true), "no stale recognition");
    assert!(replies::arm(), "no stale hold either");
}
