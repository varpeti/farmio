mod game;
mod game_lobby;
mod main_lobby;

use std::time::Duration;

use crate::game::{ActionResult, GameServer, PlayerAction};

use serde_json::to_string;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};

use futures::{SinkExt, StreamExt};
use tokio_util::codec::{Framed, LinesCodec};

#[derive(Debug)]
pub struct ClientHandler {
    player_id: usize,
    action_tx: mpsc::Sender<PlayerAction>,
}

impl ClientHandler {
    pub fn new(player_id: usize, action_tx: mpsc::Sender<PlayerAction>) -> Self {
        Self {
            player_id,
            action_tx,
        }
    }

    pub async fn handle_connection(&self, socket: TcpStream) {
        let mut framed = Framed::new(socket, LinesCodec::new());

        println!("Player `{}` connecting...", self.player_id);

        while let Some(frame_result) = framed.next().await {
            match frame_result {
                Ok(msg) => {
                    match serde_json::from_str(msg.as_str()) {
                        Ok(action) => {
                            let (response_tx, response_rx) = oneshot::channel();
                            let player_action = PlayerAction {
                                player_id: self.player_id,
                                action,
                                response_tx,
                            };

                            // Send action to the GameServer
                            if self.action_tx.send(player_action).await.is_err() {
                                eprintln!(
                                    "Connection Error with the Server (Player `{}`)",
                                    self.player_id
                                );
                                break;
                            }

                            // Wait for response and send back to client
                            // That means, if multiple action is requested,
                            // they are buffered and executed one by one each turn.
                            if let Ok(result) = response_rx.await {
                                match to_string(&result) {
                                    Ok(msg) => {
                                        if framed.send(msg).await.is_err() {
                                            eprintln!("Unable to send results, maybe Client Disconnected (Player `{}`)", self.player_id);
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!(
                                    "Error when serializing ActionResult (Player `{}`): `{}`",
                                    self.player_id, err
                                );
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(err) => match to_string(&ActionResult::InvalidAction) {
                            Ok(msg) => {
                                if framed.send(msg).await.is_err() {
                                    eprintln!("Unable to send results (InvalidAction: Error: {}) , maybe Client Disconnected (Player `{}`)", err, self.player_id);
                                }
                            }
                            Err(err) => {
                                eprintln!(
                                    "Unable to parse ActionResult::InvalidAction; Error: {}",
                                    err
                                );
                            }
                        },
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Connection Error with Player `{}`: `{}`",
                        self.player_id, err
                    );
                    break;
                }
            }
        }
        println!("Player `{}` disconnected", self.player_id);
    }
}

#[tokio::main]
async fn main() {
    let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(4096);

    let mut game_server = GameServer::new(
        action_rx,
        16,
        2,
        Duration::from_millis(50000),
        Duration::from_secs(60 * 5),
    );
    tokio::spawn(async move { game_server.run().await });

    let server_ip_port = "127.0.0.1:5942";
    let listener = TcpListener::bind(server_ip_port)
        .await
        .unwrap_or_else(|_| panic!("Unable to bind to address: `{}`", server_ip_port));
    println!("Game server listening on `{}`", server_ip_port);

    let mut next_player_id: usize = 0;

    while let Ok((socket, addr)) = listener.accept().await {
        let player_id = next_player_id;
        next_player_id += 1;

        println!(
            "New connection from: `{}`, player_id: `{}`",
            addr, player_id
        );

        let client_handler = ClientHandler::new(player_id, action_tx.clone());

        tokio::spawn(async move { client_handler.handle_connection(socket).await });
    }
}
