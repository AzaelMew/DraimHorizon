// Picks a bot task up again after the server restarts and the client reconnects. The last
// task that was started is remembered; if it was still running when the connection went, it is
// started again once the player is back on the same server with the world loaded around them.
use crate::game::{
    Game, claim_borders as claims, claim_borders_navigation::Goal,
    claim_borders_navigation_view as walk,
};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
/// How long after the connection went a task is still picked up (a restart takes minutes).
const WINDOW: Duration = Duration::from_secs(15 * 60);
/// Time in the world, with chunks loaded, before the task starts again.
const SETTLE: Duration = Duration::from_secs(6);
/// Tries before the task is given up on.
const TRIES: u8 = 6;
struct Saved {
    goal: Goal,
    server: String,
    /// When the connection went with this task running; `None` while it is just remembered.
    lost: Option<Instant>,
    /// The last tick the task was seen running in the world: a lost connection pauses it a
    /// moment before the reset hook comes, so "running a few seconds ago" counts.
    seen_running: Option<Instant>,
    /// When the player was last seen in the world again, and the tries made.
    back: Option<Instant>,
    last_try: Option<Instant>,
    tries: u8,
}
static SAVED: Mutex<Option<Saved>> = Mutex::new(None);
/// Set while a lost task is being started again, so that it keeps where it was working.
static RESUMING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// True while a task is started again after a reconnect (not by the player).
pub(super) fn resuming() -> bool {
    RESUMING.load(std::sync::atomic::Ordering::Relaxed)
}
fn server(game: &Game<'_>) -> Option<String> {
    game.remote.as_ref().map(|r| r.session.address.to_string())
}
/// "Resume after a reconnect" is on unless the player switched it off.
pub(super) fn enabled(game: &Game<'_>) -> bool {
    game.native_setting(claims::SETTINGS_ID, "auto_resume")
        .as_deref()
        != Some("false")
}
/// A task was started by the player: remember it (a preview is not worth resuming).
pub(super) fn note_start(game: &Game<'_>, goal: Goal, running: bool) {
    let mut saved = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    *saved = match (running, server(game)) {
        (true, Some(server)) => Some(Saved {
            goal,
            server,
            lost: None,
            seen_running: Some(Instant::now()),
            back: None,
            last_try: None,
            tries: 0,
        }),
        _ => None,
    };
}
/// A schematic build is kept in the settings (server, file, origin, rotation) until it is
/// finished or stopped, so that it goes on after a server restart and after the game itself is
/// started again: the file is loaded once more and the build carries on (placed blocks are
/// skipped).
const BUILD_KEY: &str = "schematic_job";
pub(super) fn note_build(game: &Game<'_>, filename: &str, origin: [i32; 3], rotation: u8) {
    if let Some(server) = server(game) {
        let [x, y, z] = origin;
        game.native_set_setting(
            claims::SETTINGS_ID,
            BUILD_KEY,
            &format!("{server}\t{filename}\t{x}\t{y}\t{z}\t{rotation}"),
        );
    }
}
/// The build was finished or stopped by the player: it does not come back.
pub(super) fn forget_build(game: &Game<'_>) {
    game.native_set_setting(claims::SETTINGS_ID, BUILD_KEY, "");
}
/// The kept build: server, file, origin, rotation.
fn kept_build(game: &Game<'_>) -> Option<(String, String, [i32; 3], u8)> {
    let value = game.native_setting(claims::SETTINGS_ID, BUILD_KEY)?;
    let parts: Vec<&str> = value.split('\t').collect();
    let [server, file, x, y, z, rotation] = parts[..] else {
        return None;
    };
    Some((
        server.to_owned(),
        file.to_owned(),
        [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
        rotation.parse().ok()?,
    ))
}
/// Picking up a kept build: since when the player is back in the world, the last try, how many.
struct BuildTry {
    back: Option<Instant>,
    last: Option<Instant>,
    tries: u8,
    done: bool,
}
static BUILD: Mutex<BuildTry> = Mutex::new(BuildTry {
    back: None,
    last: None,
    tries: 0,
    done: false,
});
/// Every tick: a kept build on this server is loaded and started again once the world around
/// the player has loaded (once per visit, a few tries).
fn build_frame(game: &Game<'_>) {
    let mut b = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    if b.done || !enabled(game) {
        return;
    }
    let Some(p) = game.native_player().filter(|_| game.native_playing()) else {
        b.back = None;
        return;
    };
    let Some((kept_server, file, origin, rotation)) = kept_build(game) else {
        b.done = true;
        return;
    };
    if server(game).as_deref() != Some(kept_server.as_str()) || !game.native_on(claims::ID) {
        b.back = None;
        return;
    }
    let back = *b.back.get_or_insert_with(Instant::now);
    let chunk = (
        (p.x.floor() as i32).div_euclid(16),
        (p.z.floor() as i32).div_euclid(16),
    );
    if back.elapsed() < SETTLE
        || !game.world.chunks.contains_key(&chunk)
        || b.last
            .is_some_and(|t| t.elapsed() < Duration::from_secs(10))
    {
        return;
    }
    if walk::task_running() || crate::game::claim_borders_workshop_view::exists() {
        b.done = true;
        return;
    }
    b.last = Some(Instant::now());
    b.tries += 1;
    let started =
        crate::game::claim_borders_workshop_view::load(game, file, origin, rotation, true).is_ok();
    if started || b.tries >= 3 {
        b.done = true;
    }
}
/// The player stopped the task: it is not to come back.
pub(super) fn forget() {
    *SAVED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
/// The connection is going: if a task is running, mark it to be resumed. Called before the
/// state of the tasks is cleared.
pub(super) fn disconnected() {
    // A kept build is looked at again on the next visit.
    *BUILD.lock().unwrap_or_else(|e| e.into_inner()) = BuildTry {
        back: None,
        last: None,
        tries: 0,
        done: false,
    };
    let running = walk::task_running();
    let mut saved = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = saved.as_mut() {
        let running = running
            || s.seen_running
                .is_some_and(|at| at.elapsed() < Duration::from_secs(5));
        if running && s.lost.is_none() {
            s.lost = Some(Instant::now());
            s.back = None;
            s.last_try = None;
            s.tries = 0;
        } else if !running && s.lost.is_none() {
            // It had finished or been paused: nothing to bring back.
            *saved = None;
        }
    }
}
/// Every tick: starts a marked task again when the player is back and the world has loaded.
pub(super) fn frame(game: &mut Game<'_>) {
    build_frame(game);
    let mut guard = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    let Some(saved) = guard.as_mut() else {
        return;
    };
    let Some(lost) = saved.lost else {
        if walk::task_running() && game.native_player().is_some() {
            saved.seen_running = Some(Instant::now());
        }
        return;
    };
    if lost.elapsed() > WINDOW || !enabled(game) {
        *guard = None;
        return;
    }
    let here = server(game);
    let Some(p) = game.native_player().filter(|_| game.native_playing()) else {
        saved.back = None;
        return;
    };
    // Another server: the task waits for its own (until the window passes).
    if here.as_deref() != Some(saved.server.as_str()) || !game.native_on(claims::ID) {
        saved.back = None;
        return;
    }
    let back = *saved.back.get_or_insert_with(Instant::now);
    let chunk = (
        (p.x.floor() as i32).div_euclid(16),
        (p.z.floor() as i32).div_euclid(16),
    );
    if back.elapsed() < SETTLE
        || !game.world.chunks.contains_key(&chunk)
        || saved
            .last_try
            .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
    {
        return;
    }
    if walk::task_running() {
        *guard = None;
        return;
    }
    saved.last_try = Some(Instant::now());
    saved.tries += 1;
    let goal = saved.goal;
    let tries = saved.tries;
    drop(guard);
    RESUMING.store(true, std::sync::atomic::Ordering::Relaxed);
    let started = walk::start(game, goal, true);
    RESUMING.store(false, std::sync::atomic::Ordering::Relaxed);
    match started {
        Ok(()) => {
            // Started again: it is the remembered task once more.
            if let Some(s) = SAVED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                s.lost = None;
            }
        }
        Err(_) if tries >= TRIES => forget(),
        Err(_) => {}
    }
}
