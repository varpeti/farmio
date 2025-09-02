use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Deserialize)]
pub enum Action {
    #[serde(skip_deserializing)]
    __Connect__ {
        player_name: String,
        response_tx: Sender<Response>,
    },
}

#[derive(Debug, Deserialize)]
pub struct PlayerAction {
    pub player_uuid: String,
    pub action: Action,
}

#[derive(Debug, Serialize)]
pub enum Response {
    ConnectionSuccess,
    ConnectionDenied,
}

#[derive(Debug)]
pub struct Game {
    map_size: u32,
    //map: Vec<Vec<Cell>>,
    player_count: u32,
    //players: HashMap<String, Player>,
    action_rx: Receiver<PlayerAction>,
    turn_duration: Duration,
}

impl Game {
    pub fn new(
        action_rx: Receiver<PlayerAction>,
        map_size: u32,
        number_of_players: u32,
        turn_duration: Duration,
    ) -> Game {
        Self {
            map_size,
            player_count: number_of_players,
            action_rx,
            turn_duration,
        }
    }
    pub async fn run(&mut self) {
        while let Some(player_action) = self.action_rx.recv().await {
            self.handle_player_action(player_action).await;
        }
    }

    async fn handle_player_action(&mut self, player_action: PlayerAction) {
        println!("Got PlayerAction: `{:?}`", player_action);
        match player_action.action {
            Action::__Connect__ {
                player_name,
                response_tx,
            } => {
                let response = Response::ConnectionSuccess;
                self.send_response(response, response_tx, player_name).await;
            }
        }
    }

    async fn send_response(
        &mut self,
        response: Response,
        response_tx: Sender<Response>,
        player_name: String,
    ) {
        if let Err(err) = response_tx.send(response).await {
            eprintln!(
                "Error when sending back Response to Player `{}`: `{}`",
                player_name, err
            );
        }
    }
}
