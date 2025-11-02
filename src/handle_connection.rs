use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};
use uuid::Uuid;

use crate::{
    game::{Action, Game, GameSettings},
    send_to_player::{send_msg_to_player, send_to_player},
    server::Games,
};

pub async fn handle_connection(framed: Framed<TcpStream, LinesCodec>, games: Games) {
    println!("Player connecting...");

    // State
    let mut s_player_name: Option<String> = None;
    let mut s_player_uuid: Option<Uuid> = None;
    let mut s_game_name: Option<String> = None;

    // Com
    let (tcp_tx, mut tcp_rx) = framed.split();
    let (mut to_player_tx, to_player_rx) = mpsc::channel::<String>(1024);
    let mut s_to_game_tx: Option<Sender<PlayerAction>> = None;

    tokio::spawn(async move { send_to_player(to_player_rx, tcp_tx).await });

    while let Some(Ok(msg)) = tcp_rx.next().await {
        // Lobby
        if let Ok(lobby_msg) = serde_json::from_str::<LobbyMsg>(&msg) {
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

                    // Check if Game exists
                    if games.contains_key(&game_name) {
                        eprintln!(
                            "Player `{}` tried to create Game `{}` witch already exists!",
                            player_name, game_name
                        );
                        send_msg_to_player(&mut to_player_tx, LobbyToPlayer::GameAlreadyExists)
                            .await;
                        continue;
                    }

                    // Com
                    let (to_game_tx, to_game_rx) = mpsc::channel::<PlayerAction>(1024);
                    s_to_game_tx = Some(to_game_tx.clone());

                    // New Game
                    println!(
                        "New Game by Player `{}`: `{}({:?})`",
                        &player_name, &game_name, &game_settings
                    );
                    let mut game = Game::new(game_name.clone(), to_game_rx, game_settings).await;
                    games.insert(game_name.clone(), to_game_tx.clone());
                    tokio::spawn(async move { game.run().await });
                    send_msg_to_player(&mut to_player_tx, LobbyToPlayer::GameCreated).await;

                    // Connect
                    send_msg_to_game(
                        &mut Some(to_game_tx),
                        Action::__Connect__ {
                            player_name,
                            to_player_tx: to_player_tx.clone(),
                        },
                        &s_player_uuid,
                        &s_player_name,
                        &s_game_name,
                        &mut to_player_tx,
                    )
                    .await;
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
                    if let Some(to_game_tx) = games.get(&game_name).map(|e| e.to_owned()) {
                        // Com
                        s_to_game_tx = Some(to_game_tx.clone());

                        // Connect
                        send_msg_to_game(
                            &mut Some(to_game_tx),
                            Action::__Connect__ {
                                player_name,
                                to_player_tx: to_player_tx.clone(),
                            },
                            &s_player_uuid,
                            &s_player_name,
                            &s_game_name,
                            &mut to_player_tx,
                        )
                        .await;
                    } else {
                        eprintln!(
                            "Player `{}` tried to join nonexistent Game `{}` ",
                            player_name, game_name
                        );
                        send_msg_to_player(&mut to_player_tx, LobbyToPlayer::GameNotExists).await;
                    };
                }
            }
            continue;
        }

        // Action
        if let Ok(action) = serde_json::from_str::<Action>(&msg) {
            send_msg_to_game(
                &mut s_to_game_tx,
                action,
                &s_player_uuid,
                &s_player_name,
                &s_game_name,
                &mut to_player_tx,
            )
            .await;
            continue;
        }

        eprintln!(
            "Invalid Msg `{}` by Player `{:?}` playing Game `{:?}` ",
            msg, s_player_name, s_game_name
        );
        send_msg_to_player(&mut to_player_tx, LobbyToPlayer::InvalidMsg).await;
    }

    println!("Player `{:?}` disconnecting...", s_player_name);
    send_msg_to_game(
        &mut s_to_game_tx,
        Action::__Disconnect__,
        &s_player_uuid,
        &s_player_name,
        &s_game_name,
        &mut to_player_tx,
    )
    .await
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
    NotConnectedToAnyGame,
    UnableToCommunicateWithGame,
    InvalidMsg,
}

#[derive(Debug)]
pub struct PlayerAction {
    pub player_uuid: Uuid,
    pub action: Action,
}

async fn send_msg_to_game(
    s_to_game_tx: &mut Option<Sender<PlayerAction>>,
    action: Action,
    s_player_uuid: &Option<Uuid>,
    s_player_name: &Option<String>,
    s_game_name: &Option<String>,
    to_player_tx: &mut Sender<String>,
) {
    if let (Some(to_game_tx), Some(player_uuid)) = (s_to_game_tx, s_player_uuid) {
        let player_action = PlayerAction {
            action,
            player_uuid: player_uuid.to_owned(),
        };
        if let Err(err) = to_game_tx.send(player_action).await {
            eprintln!(
                "Unable to send Action of Player `{:?}` to Game `{:?}`: `{}`",
                s_player_name, s_game_name, err
            );
            send_msg_to_player(to_player_tx, LobbyToPlayer::UnableToCommunicateWithGame).await;
        }
    } else {
        eprintln!("Player is not connected to any Game!");
        send_msg_to_player(to_player_tx, LobbyToPlayer::NotConnectedToAnyGame).await;
    }
}
