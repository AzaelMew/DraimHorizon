// Native adapter for the helpers that work while the player plays by hand: a totem for the off
// hand, a used-up hotbar stack replaced from the inventory, the right tool for the block hit.
// Each one has its own switch (Experiments > Helpers) and is off until it is turned on.
use crate::game::{
    Game, claim_borders as claims, claim_borders_autoclick as pace, claim_borders_menu::Helper,
    claim_borders_navigation_view as walk,
};
use horizon_client::menu::Input;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
const TOTEM: &str = "minecraft:totem_of_undying";
/// The off hand's slot in the player's menu, as a swap click names it.
const OFFHAND: i32 = 40;
/// Pause between two swaps, so the server has answered the last one.
const SWAP_GAP: Duration = Duration::from_millis(350);
/// How long a hotbar slot that ran out is refilled for before it is given up on.
const REFILL_FOR: Duration = Duration::from_secs(3);
/// What each hotbar slot held when last seen (item and count), and the slots that just ran out.
struct Watch {
    seen: [Option<(String, u32)>; 9],
    ran_out: [Option<(String, Instant)>; 9],
    last_swap: Option<Instant>,
}
static WATCH: Mutex<Watch> = Mutex::new(Watch {
    seen: [const { None }; 9],
    ran_out: [const { None }; 9],
    last_swap: None,
});
fn on(game: &Game<'_>, helper: Helper) -> bool {
    game.native_setting(claims::SETTINGS_ID, helper.key())
        .as_deref()
        == Some("true")
}
/// The first inventory slot (from `from`) holding `item`.
fn find(game: &Game<'_>, item: &str, from: usize) -> Option<usize> {
    (from..36).find(|slot| {
        let stack = if *slot < 9 {
            game.stack_in(*slot)
        } else {
            game.inventory.get(*slot).and_then(Option::as_ref)
        };
        stack.is_some_and(|s| s.count > 0 && s.item.to_string() == item)
    })
}
/// Runs every tick before the game handles input.
pub(super) fn tick(game: &mut Game<'_>) {
    let mut watch = WATCH.lock().unwrap_or_else(|e| e.into_inner());
    // Behind a screen the player moves things by hand: nothing is done, and what was seen is
    // forgotten so that a stack moved out on purpose is not put back.
    if !game.native_on(claims::ID) || !game.native_playing() || game.player.is_none() {
        watch.seen = [const { None }; 9];
        watch.ran_out = [const { None }; 9];
        return;
    }
    let ready = watch.last_swap.is_none_or(|t| t.elapsed() >= SWAP_GAP);
    let mut swapped = false;
    if on(game, Helper::Totem) && ready && game.offhand.is_none() {
        if let Some(slot) = find(game, TOTEM, 0) {
            swapped = game.remote_menu_ops(&[(Some(slot), OFFHAND, Input::Swap)]);
        }
    }
    if on(game, Helper::Fill) {
        for hotbar in 0..9 {
            let now = game
                .stack_in(hotbar)
                .filter(|s| s.count > 0)
                .map(|s| (s.item.to_string(), s.count));
            match now {
                Some(held) => {
                    watch.ran_out[hotbar] = None;
                    watch.seen[hotbar] = Some(held);
                }
                None => {
                    // The last item of a stack was used: the same item replaces it.
                    if let Some((item, 1)) = watch.seen[hotbar].take() {
                        watch.ran_out[hotbar] = Some((item, Instant::now()));
                    }
                    let Some((item, since)) = watch.ran_out[hotbar].clone() else {
                        continue;
                    };
                    if since.elapsed() > REFILL_FOR {
                        watch.ran_out[hotbar] = None;
                    } else if ready && !swapped {
                        if let Some(slot) = find(game, &item, 9) {
                            swapped =
                                game.remote_menu_ops(&[(Some(slot), hotbar as i32, Input::Swap)]);
                        }
                    }
                }
            }
        }
    } else {
        watch.seen = [const { None }; 9];
        watch.ran_out = [const { None }; 9];
    }
    if swapped {
        watch.last_swap = Some(Instant::now());
    }
    drop(watch);
    if on(game, Helper::Tools) {
        crate::game::claim_borders_navigation_view::auto_tool(game);
    }
}
/// When the auto-clicker's clock started, its pace, and whether its key was down last tick.
struct Click {
    clicker: pace::Clicker,
    key_was_down: bool,
}
static CLICK: Mutex<Click> = Mutex::new(Click {
    clicker: pace::Clicker::new(),
    key_was_down: false,
});
fn clock() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
/// The auto-clicker's pace as chosen (an index into `autoclick::RATES`).
pub(super) fn click_rate(game: &Game<'_>) -> u8 {
    game.native_setting(claims::SETTINGS_ID, "auto_click_rate")
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|i| usize::from(*i) < pace::RATES.len())
        .unwrap_or(pace::DEFAULT_RATE)
}
/// The auto-clicker: while it is on and the player is playing by hand (no screen, no task
/// running), the chosen mouse button is pressed in pulses at the chosen pace, on top of whatever
/// the player holds. Called last, after the tasks have read the buttons, so that its presses
/// are not taken for the player. A key (unbound until the player binds one) switches it.
pub(super) fn autoclick(game: &mut Game<'_>) {
    let playing = game.native_on(claims::ID)
        && game.native_playing()
        && !game.screen_over_playing()
        && game.player.is_some();
    let mut click = CLICK.lock().unwrap_or_else(|e| e.into_inner());
    if playing {
        let key = game.native_mod_key(claims::ID, "autoclick", "key.keyboard.unknown");
        let down = horizon_client::keybinds::key_code(&key)
            .is_some_and(|code| game.physical_keys.contains(&code));
        if down && !click.key_was_down {
            let now = !on(game, Helper::AutoClick);
            game.native_set_setting(
                claims::SETTINGS_ID,
                Helper::AutoClick.key(),
                if now { "true" } else { "false" },
            );
        }
        click.key_was_down = down;
    }
    if !(playing && on(game, Helper::AutoClick)) || walk::task_running() {
        click.clicker.reset();
        return;
    }
    let right = game
        .native_setting(claims::SETTINGS_ID, "auto_click_button")
        .as_deref()
        == Some("right");
    let rate = pace::rate(click_rate(game));
    if click.clicker.down(clock(), rate) {
        drop(click);
        walk::press_for_click(game, right);
    }
}
