use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use uuid::Uuid;

use crate::{
    game::{Action, Game, GameSettings},
    send_to_player::{send_msg_to_player, send_to_player},
    server::{FramedConnection, Games},
};

pub type TcpSender = futures::stream::SplitSink<FramedConnection, String>;
type TcpReceiver = futures::stream::SplitStream<FramedConnection>;

pub async fn handle_connection(framed: FramedConnection, games: Games) {
    println!("Player connecting...");

    // State
    let mut s_player_name: Option<String> = None;
    let mut s_player_uuid: Option<Uuid> = None;
    let mut s_game_name: Option<String> = None;
    let mut s_state = SState::Lobby;

    // Com
    let (tcp_tx, mut tcp_rx) = framed.split();
    let (mut to_player_tx, to_player_rx) = mpsc::channel::<String>(1024);
    let mut s_to_game_tx: Option<Sender<PlayerAction>> = None;

    tokio::spawn(async move { send_to_player(to_player_rx, tcp_tx).await });

    while let Some(Ok(msg)) = tcp_rx.next().await {
        match s_state {
            SState::Lobby => {
                match serde_json::from_str::<LobbyMsg>(&msg) {
                    Ok(lobby_msg) => {
                        match lobby_msg {
                            LobbyMsg::NewGame {
                                player_name,
                                player_uuid,
                                game_name,
                                game_settings,
                            } => {
                                // State
                                s_player_name = Some(player_name.clone());
                                s_player_uuid = Some(player_uuid);
                                s_game_name = Some(game_name.clone());

                                if games.contains_key(&game_name) {
                                    eprintln!(
                                            "Player `{}` tried to create Game `{}` witch already exists!",
                                            player_name, game_name
                                        );
                                    send_msg_to_player(
                                        &mut to_player_tx,
                                        LobbyToPlayer::GameAlreadyExists,
                                    )
                                    .await;
                                    continue;
                                }

                                // Com
                                let (to_game_tx, to_game_rx) = mpsc::channel::<PlayerAction>(1024);
                                s_to_game_tx = Some(to_game_tx.clone());

                                let mut game = Game::new(game_name.clone(), to_game_rx);
                                games.insert(game_name.clone(), to_game_tx.clone());

                                tokio::spawn(async move { game.run().await });

                                println!(
                                    "New Game by Player `{}`: `{}({:?})`",
                                    &player_name, &game_name, &game_settings
                                );
                                send_msg_to_player(&mut to_player_tx, LobbyToPlayer::GameCreated)
                                    .await;

                                // Connect
                                send_msg_to_game(
                                    to_game_tx,
                                    Action::__Connect__ {
                                        player_name,
                                        to_player_tx: to_player_tx.clone(),
                                    },
                                    &s_player_uuid,
                                    &s_player_name,
                                    &s_game_name,
                                )
                                .await;

                                // TODO: Either commonicate from Game to Lobby that we can advance into
                                // SState::Game or allways try to parse LobbyMsg...
                            }
                            LobbyMsg::JoinGame {
                                player_name,
                                player_uuid,
                                game_name,
                            } => {
                                // State
                                s_player_name = Some(player_name.clone());
                                s_player_uuid = Some(player_uuid);
                                s_game_name = Some(game_name.clone());

                                // Connect Player to the Game
                                if let Some(to_game_tx) =
                                    games.get(&game_name).map(|e| e.to_owned())
                                {
                                    // Com
                                    s_to_game_tx = Some(to_game_tx.clone());

                                    // Connect
                                    send_msg_to_game(
                                        to_game_tx,
                                        Action::__Connect__ {
                                            player_name,
                                            to_player_tx: to_player_tx.clone(),
                                        },
                                        &s_player_uuid,
                                        &s_player_name,
                                        &s_game_name,
                                    )
                                    .await;
                                } else {
                                    eprintln!(
                                        "Player `{}` tried to join nonexistent Game `{}` ",
                                        player_name, game_name
                                    );
                                    send_msg_to_player(
                                        &mut to_player_tx,
                                        LobbyToPlayer::GameNotExists,
                                    )
                                    .await;
                                };
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Invalid LobbyMsg from Player `{:?}`: `{}`",
                            s_player_uuid, err
                        );
                        send_msg_to_player(
                            &mut to_player_tx,
                            LobbyToPlayer::InvalidLobbyMsg(err.to_string()),
                        )
                        .await;
                    }
                }
            }
            SState::Game => {
                if let Ok(game_msg) = serde_json::from_str::<Action>(&msg) {
                    // TODO: Send Msg to Game
                    continue;
                }
            }
        }
    }

    println!("Player `{:?}` disconnecting...", s_player_name);
}

enum SState {
    Lobby,
    Game,
}

#[derive(Debug, Deserialize)]
enum LobbyMsg {
    NewGame {
        player_name: String,
        player_uuid: Uuid,
        game_name: String,
        game_settings: GameSettings,
    },
    JoinGame {
        player_name: String,
        player_uuid: Uuid,
        game_name: String,
    },
}

#[derive(Debug, Serialize)]
enum LobbyToPlayer {
    GameCreated,
    GameAlreadyExists,
    GameNotExists,
    InvalidLobbyMsg(String),
}

#[derive(Debug)]
pub struct PlayerAction {
    pub player_uuid: Uuid,
    pub action: Action,
}

async fn send_msg_to_game(
    to_game_tx: Sender<PlayerAction>,
    action: Action,
    s_player_uuid: &Option<Uuid>,
    s_player_name: &Option<String>,
    s_game_name: &Option<String>,
) {
    if let Some(player_uuid) = s_player_uuid {
        let player_action = PlayerAction {
            action,
            player_uuid: player_uuid.to_owned(),
        };
        if let Err(err) = to_game_tx.send(player_action).await {
            eprintln!(
                "Unable to send Action of Player `{:?}` to Game `{:?}`: `{}`",
                s_player_name, s_game_name, err
            );
            // TODO: Notify Player
        }
    } else {
        eprintln!("No Player UUID was given!");
    }
}
