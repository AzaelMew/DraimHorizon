// Auto-trade with villagers: a shift-click on an offer in the trading window repeats that trade
// for as long as it goes (everything of the price item in the inventory, until the villager's
// stock runs out). Each round is the game's own: the offer is selected (the server moves the
// price into the payment slots) and the result is taken with a shift-click (the server trades
// as often as the payment slots allow). A round in which nothing more was paid ends it, as do
// a closed window, a sold-out offer and a cap on rounds.
use crate::game::{Game, claim_borders as claims};
use horizon_client::menu::Input;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
/// The merchant menu's result slot in the client's own numbering.
const RESULT: usize = 650;
/// Rounds at most (a round trades up to a stack of the price).
const ROUNDS: u16 = 96;
/// Time for the server to move the price in, and to make the trades.
const SELECT_WAIT: Duration = Duration::from_millis(150);
const TAKE_WAIT: Duration = Duration::from_millis(250);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Take,
    Select,
}
struct Run {
    offer: usize,
    item: String,
    step: Step,
    at: Instant,
    rounds: u16,
    /// The price item on hand after the last round, and rounds in a row it did not go down.
    left: u32,
    still: u8,
}
static RUN: Mutex<Option<Run>> = Mutex::new(None);
fn run<T>(f: impl FnOnce(&mut Option<Run>) -> T) -> T {
    f(&mut RUN.lock().unwrap_or_else(|e| e.into_inner()))
}
/// The offer list shown, the focused offer: the open trading window's.
fn window<'a>(game: &'a Game<'_>) -> Option<&'a crate::game::trading::MerchantState> {
    game.station.as_ref().and_then(|s| s.merchant.as_deref())
}
/// How much of `item` the inventory and the payment slots hold.
fn on_hand(game: &Game<'_>, item: &str) -> u32 {
    let count = |s: Option<&horizon_client::items::ItemStack>| {
        s.filter(|s| s.count > 0 && s.item.as_ref() == item)
            .map_or(0, |s| s.count)
    };
    (0..9).map(|g| count(game.stack_in(g))).sum::<u32>()
        + (9..36)
            .map(|g| count(game.inventory.get(g).and_then(Option::as_ref)))
            .sum::<u32>()
}
/// After a click on the offer list: with Shift held, the clicked offer is traded on and on.
pub(super) fn clicked(game: &mut Game<'_>, hit: bool) {
    if !hit || !game.modifiers.shift_key() || !game.native_on(claims::ID) {
        return;
    }
    // A press on the scroller is a hit too, with the focus of an earlier click.
    let Some(m) = window(game).filter(|m| !m.dragging) else {
        return;
    };
    let Some(offer) = m
        .focused
        .map(|row| row + m.scroll)
        .filter(|&i| i < m.session.offers.len())
    else {
        return;
    };
    let item = m.session.offers[offer].base_cost_a.stack.item.to_string();
    let left = on_hand(game, &item);
    // The click itself selected the offer: the result is taken next.
    run(|r| {
        *r = Some(Run {
            offer,
            item,
            step: Step::Take,
            at: Instant::now(),
            rounds: 0,
            left,
            still: 0,
        })
    });
}
/// Selects an offer as a click on it would (the server moves the price in).
fn select(game: &mut Game<'_>, offer: usize) {
    if game.remote.is_some() {
        if let Some(m) = game.station.as_mut().and_then(|s| s.merchant.as_mut()) {
            m.session.selection_hint = offer;
        }
        game.send(horizon_protocol::packets::play::Serverbound::SelectTrade(
            horizon_protocol::packets::play::SelectTrade { item: offer as i32 },
        ));
    } else {
        game.menu_session(|menu, env, _| {
            menu.merchant_select(env, offer);
            Some(true)
        });
    }
}
/// Every tick: the next step of a running auto-trade.
pub(super) fn frame(game: &mut Game<'_>) {
    let Some((offer, item, step, at)) =
        run(|r| r.as_ref().map(|r| (r.offer, r.item.clone(), r.step, r.at)))
    else {
        return;
    };
    // The window closed, another one opened, or the offer is gone or sold out.
    let open = window(game)
        .and_then(|m| m.session.offers.get(offer))
        .map(|o| o.uses < o.max_uses && o.base_cost_a.stack.item.as_ref() == item.as_str());
    if open != Some(true) || !game.native_on(claims::ID) {
        run(|r| *r = None);
        return;
    }
    match step {
        Step::Take if at.elapsed() >= SELECT_WAIT => {
            game.remote_menu_ops(&[(Some(RESULT), 0, Input::QuickMove)]);
            run(|r| {
                if let Some(r) = r.as_mut() {
                    r.step = Step::Select;
                    r.at = Instant::now();
                }
            });
        }
        Step::Select if at.elapsed() >= TAKE_WAIT => {
            let now = on_hand(game, &item);
            let done = run(|r| {
                let Some(r) = r.as_mut() else {
                    return true;
                };
                r.still = if now < r.left { 0 } else { r.still + 1 };
                r.left = now;
                r.rounds += 1;
                // Two rounds in a row with nothing paid: the price ran out, or the villager
                // will not trade any more.
                r.still >= 2 || r.rounds >= ROUNDS || now == 0
            });
            if done {
                run(|r| *r = None);
                return;
            }
            select(game, offer);
            run(|r| {
                if let Some(r) = r.as_mut() {
                    r.step = Step::Take;
                    r.at = Instant::now();
                }
            });
        }
        _ => {}
    }
}
