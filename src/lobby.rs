use serde::Deserialize;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    com::Games,
    game::{Game, GameSettings},
};

#[derive(Debug, Deserialize)]
enum LobbyAction {
    NewGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
        game_settings: GameSettings,
    },
    JoinGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
    },
}

pub async fn lobby(
    msg: &str,
    to_game_rx: &mut Receiver<String>,
    to_player_tx: Sender<String>,
    games: Games,
) -> bool {
    match serde_json::from_str::<LobbyAction>(msg) {
        Ok(lobby_action) => match lobby_action {
            LobbyAction::NewGame {
                player_name,
                player_uuid,
                game_name,
                game_settings,
            } => {
                let game = Game::new(to_game_rx, game_settings);
                true
            }
            LobbyAction::JoinGame {
                player_name,
                player_uuid,
                game_name,
            } => {
                // TODO:
                todo!()
            }
        },
        Err(err) => {
            eprintln!("Invalid LobbyAction `{}`: `{}`", msg, err);
            false
        }
    }
}
