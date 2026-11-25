extern crate self as sunrise_client;
use game::native_server::ClientCommandOutcome;

pub mod chat {
    #[derive(Clone, Copy, PartialEq)]
    pub enum ChatSource {
        Client,
        Server,
        Player,
    }
}

pub mod text {
    pub struct Text(pub &'static str);
    impl Text {
        pub fn plain(&self) -> String {
            self.0.into()
        }
    }
}

mod game {
    use std::{cell::RefCell, marker::PhantomData};
    pub struct Game<'a> {
        pub calls: RefCell<Vec<&'static str>>,
        pub ui: Ui,
        pub lifetime: PhantomData<&'a ()>,
    }
    pub struct Ui {
        pub chat: Chat,
    }
    pub struct Chat(pub bool);
    impl Chat {
        pub fn shows(&self, _: crate::chat::ChatSource) -> bool {
            self.0
        }
    }
    pub mod native_server {
        #[derive(Debug, PartialEq)]
        pub enum ClientCommandOutcome {
            Success(i32),
        }
    }
    pub mod claim_borders_command {
        pub fn claim_borders_command(game: &mut super::Game<'_>, command: &str, ran: &mut bool) {
            game.calls.borrow_mut().push("observe");
            if command == "owned" {
                *ran = true;
            }
        }
    }
    pub mod claim_borders_bar_command {
        pub fn baritone_command(game: &mut super::Game<'_>, command: &str, ran: &mut bool) {
            if !*ran {
                game.calls.borrow_mut().push("baritone");
                if command == "bar" {
                    *ran = true;
                }
            }
        }
    }
    pub mod claim_borders_chat_view {
        pub fn capture_local(game: &super::Game<'_>, _: &str) {
            game.calls.borrow_mut().push("local");
        }
    }
    pub mod claim_borders_chat_capture {
        pub fn horizon_chat_capture(
            _: bool,
            game: &mut super::Game<'_>,
            _: &crate::text::Text,
            _: crate::chat::ChatSource,
        ) -> core::ops::ControlFlow<bool> {
            game.calls.borrow_mut().push("server-parser");
            core::ops::ControlFlow::Continue(())
        }
    }
}

fn game(shows: bool) -> game::Game<'static> {
    game::Game {
        calls: Default::default(),
        ui: game::Ui { chat: game::Chat(shows) },
        lifetime: Default::default(),
    }
}

#[test]
fn consumed_commands_stay_local_and_do_not_run_a_second_handler() {
    use core::ops::ControlFlow::Break;
    use game::native_server::ClientCommandOutcome::Success;
    let mut game = game(true);
    assert_eq!(claim_borders_client_command(&mut game, "owned"), Break(Some(Success(1))));
    assert_eq!(*game.calls.borrow(), ["observe"]);
}

#[test]
fn baritone_can_consume_and_unknown_commands_continue_after_observation() {
    use core::ops::ControlFlow::{Break, Continue};
    use game::native_server::ClientCommandOutcome::Success;
    let mut game = game(true);
    assert_eq!(claim_borders_client_command(&mut game, "bar"), Break(Some(Success(1))));
    assert_eq!(*game.calls.borrow(), ["observe", "baritone"]);
    game.calls.borrow_mut().clear();
    assert_eq!(claim_borders_client_command(&mut game, "claims"), Continue(()));
    assert_eq!(*game.calls.borrow(), ["observe", "baritone"]);
}

#[test]
fn local_chat_is_captured_once_and_never_enters_the_server_parser() {
    let mut game = game(true);
    let flow = claim_borders_chat_capture_line(&mut game, &text::Text("notice"), chat::ChatSource::Client);
    assert_eq!(flow, core::ops::ControlFlow::Continue(()));
    assert_eq!(*game.calls.borrow(), ["local"]);
    game.calls.borrow_mut().clear();
    game.ui.chat.0 = false;
    let _ = claim_borders_chat_capture_line(&mut game, &text::Text("hidden"), chat::ChatSource::Client);
    assert!(game.calls.borrow().is_empty());
}

#[test]
fn server_and_player_chat_keep_the_existing_capture_path() {
    for source in [chat::ChatSource::Server, chat::ChatSource::Player] {
        let mut game = game(true);
        let _ = claim_borders_chat_capture_line(&mut game, &text::Text("chat"), source);
        assert_eq!(*game.calls.borrow(), ["server-parser"]);
    }
}
