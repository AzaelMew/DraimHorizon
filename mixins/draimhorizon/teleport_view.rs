use crate::game::claim_borders_ui_view as ui;
// Heads-up requests can be clicked only with the chat cursor open; actions drain on the game thread.
use crate::game::{Game, Screen, claim_borders as claims, claim_borders_teleport as model};
use claim_borders_relay_client::translation::Language;
use horizon_client::hud::HudBatch;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::Instant,
};
struct State {
    requests: model::Requests,
    clock: Instant,
    hits: Vec<([f32; 4], u64, bool)>,
    viewport: [f32; 2],
    commands: VecDeque<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            requests: model::Requests::default(),
            clock: Instant::now(),
            hits: Vec::new(),
            viewport: [0.0; 2],
            commands: VecDeque::new(),
        }
    }
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
pub(super) fn reset() {
    state(|s| *s = State::default());
}
pub(super) fn capture(text: &str) {
    state(|s| {
        let now = s.clock.elapsed().as_millis() as u64;
        s.requests.capture(text, now);
    });
}
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        reset();
        return;
    }
    let commands = state(|s| s.commands.drain(..).collect::<Vec<_>>());
    for command in commands {
        game.send_unattended_command(&command);
    }
}
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(claims::ID)
        || game.title.is_some()
        || game.ui.hide_gui
        || game.remote.is_none()
    {
        state(|s| s.hits.clear());
        return;
    }
    let language = crate::game::claim_borders_chat_view::current_language();
    state(|s| {
        let now = s.clock.elapsed().as_millis() as u64;
        s.requests.prune(now);
        s.hits.clear();
        s.viewport = [batch.width, batch.height];
        let w = (batch.width - 16.0).clamp(220.0, 310.0);
        let x = (batch.width - w) * 0.5;
        for (i, r) in s.requests.entries.iter().take(3).enumerate() {
            let render_start = batch.vertices.len();
            let y = 8.0 + i as f32 * 54.0;
            let remain = 60000u64.saturating_sub(now.saturating_sub(r.at));
            ui::fill(game, batch, [x, y, w, 49.0], 0xeb182331);
            ui::icon(
                game,
                batch,
                if r.trade {
                    ui::Icon::Transfer
                } else {
                    ui::Icon::Pin
                },
                [x + 8.0, y + 5.0, 11.0, 11.0],
                ui::preferences(game).accent.color(),
            );
            ui::text(
                game,
                batch,
                &format!(
                    "{} · {} · {}{}",
                    match (r.trade, language) {
                        (true, Language::Russian) => "Обмен",
                        (false, Language::Russian) => "Телепорт",
                        (true, Language::English) => "Trade",
                        (false, Language::English) => "Teleport",
                    },
                    r.name,
                    remain.div_ceil(1000),
                    language.text("с", "s")
                ),
                [x + 26.0, y + 6.0],
                0xe1e9f5,
                false,
            );
            for (j, accept, label) in [
                (0, true, language.text("Принять", "Accept")),
                (1, false, language.text("Отклонить", "Decline")),
            ] {
                let rect = [
                    x + 8.0 + j as f32 * (w - 16.0) / 2.0,
                    y + 21.0,
                    (w - 20.0) / 2.0,
                    18.0,
                ];
                ui::button(game, batch, rect, label, false, game.native_cursor());
                s.hits.push((rect, r.id, accept));
            }
            ui::fill(game, batch, [x + 8.0, y + 44.0, w - 16.0, 2.0], 0xff2d3949);
            ui::fill(
                game,
                batch,
                [x + 8.0, y + 44.0, (w - 16.0) * remain as f32 / 60000.0, 2.0],
                0xff87b6ea,
            );
            ui::finish(
                game,
                batch,
                render_start,
                &format!("incoming:{}", r.id),
                "request",
            );
        }
    });
}
pub(super) fn mouse(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if !game.native_on(claims::ID) || game.screen != Screen::Chat || button != 0 {
        return;
    }
    state(|s| {
        if s.viewport != [screen.rect[2], screen.rect[3]] {
            return;
        }
        let now = s.clock.elapsed().as_millis() as u64;
        if let Some((_, id, accept)) = s
            .hits
            .iter()
            .find(|(r, _, _)| {
                screen.mouse[0] >= r[0]
                    && screen.mouse[0] < r[0] + r[2]
                    && screen.mouse[1] >= r[1]
                    && screen.mouse[1] < r[1] + r[3]
            })
            .copied()
        {
            *canceled = true;
            if let Some(command) = s.requests.choose(id, accept, now) {
                s.commands.push_back(command);
            }
        }
    });
}

pub(super) fn command(text: &str) {
    let words: Vec<_> = text.split_whitespace().collect();
    if words.len() == 2 && ["tpaccept", "tpaaccept", "tpdeny", "tpadeny"].contains(&words[0]) {
        state(|s| {
            s.requests
                .entries
                .retain(|r| r.trade || !r.name.eq_ignore_ascii_case(words[1]))
        });
    }
}

pub(super) fn trade_command(text: &str) {
    let words: Vec<_> = text.split_whitespace().collect();
    if words.len() == 3 && words[0] == "trade" && ["accept", "deny"].contains(&words[1]) {
        state(|s| {
            s.requests
                .entries
                .retain(|r| !r.trade || !r.name.eq_ignore_ascii_case(words[2]))
        });
    }
}
