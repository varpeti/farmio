use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

use crate::game::{Action, Game, PlayerAction, Response};

pub type Games = Arc<DashMap<String, Sender<PlayerAction>>>;
type Com = Framed<TcpStream, LinesCodec>;

#[derive(Debug, Deserialize)]
pub enum LobbyAction {
    NewGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
        player_count: u32,
        map_size: u32,
        turn_duration_ms: u64,
    },
    JoinGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
    },
}

pub async fn new_lobby(ip_port: &str) {
    let listener = TcpListener::bind(ip_port)
        .await
        .unwrap_or_else(|_| panic!("Unable to bind to address: `{}`", ip_port));
    println!("Listening on `{}`", ip_port);

    let games = Arc::new(DashMap::new());

    while let Ok((socket, addr)) = listener.accept().await {
        let games = games.clone();
        tokio::spawn(async move { handle_connection(socket, addr, games).await });
    }

    unreachable!()
}

async fn handle_connection(socket: TcpStream, addr: SocketAddr, games: Games) {
    println!("Addr `{:?}` connecting...", addr);

    let mut framed = Com::new(socket, LinesCodec::new());

    match framed.next().await {
        Some(Ok(msg)) => handle_message(&mut framed, addr, games, msg).await,
        Some(Err(err)) => {
            eprintln!(
                "Error with Addr `{}` in handle_connection/frmed.next() error: `{}`",
                addr, err
            );
        }
        None => eprintln!(
            "Error with Addr `{}` in handle_connection/framed.next() returned None",
            addr
        ),
    }

    println!("Addr `{:?}` disconnecting...", addr);
}

async fn handle_message(framed: &mut Com, addr: SocketAddr, games: Games, msg: String) {
    match serde_json::from_str::<LobbyAction>(&msg) {
        Ok(lobby_action) => handle_lobby_action(framed, games, lobby_action).await,
        Err(err) => {
            eprintln!("Invalid message from Addr `{}`; The Error: `{}`", addr, err);
            // TODO: Send msg to Addr
        }
    }
}

async fn handle_lobby_action(framed: &mut Com, games: Games, lobby_action: LobbyAction) {
    let (player_name, player_uuid, game_name) = match lobby_action {
        LobbyAction::NewGame {
            player_name,
            player_uuid,
            game_name,
            player_count,
            map_size,
            turn_duration_ms,
        } => {
            let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(1024);
            let mut game = Game::new(
                action_rx,
                map_size,
                player_count,
                Duration::from_millis(turn_duration_ms),
            );

            tokio::spawn(async move { game.run().await });
            games.insert(game_name.clone(), action_tx);
            println!(
                "New Game by Player `{}`: game_name: {}, player_count: {}, map_size: {}, turn_duration_ms: {}",
                player_name, game_name, player_count, map_size, turn_duration_ms
            );
            (player_name, player_uuid, game_name)
        }
        LobbyAction::JoinGame {
            player_name,
            player_uuid,
            game_name,
        } => (player_name, player_uuid, game_name),
    };

    match games.get(&game_name).map(|e| e.clone()) {
        Some(action_tx) => {
            connect_player_to_the_game(framed, player_name, player_uuid, game_name, action_tx).await
        }
        None => {
            eprintln!(
                "Game `{}` not found. Requester: Player `{}`",
                game_name, player_name,
            );
            let _ = send_msg_to_player(
                framed,
                &player_name,
                &format!("Error: The Game `{}` not found!", game_name),
            )
            .await;
        }
    }
}

async fn connect_player_to_the_game(
    framed: &mut Com,
    player_name: String,
    player_uuid: String,
    game_name: String,
    action_tx: Sender<PlayerAction>,
) {
    let (response_tx, mut response_rx) = mpsc::channel::<Response>(1024);

    let player_action = PlayerAction {
        player_uuid: player_uuid.clone(),
        action: Action::__Connect__ {
            player_name: player_name.clone(),
            response_tx,
        },
    };

    if send_player_msg_to_game(
        framed,
        &player_name,
        &game_name,
        action_tx.clone(),
        player_action,
    )
    .await
    .is_err()
    {
        eprintln!("SHOULD NOT REACH THIS NOW");
        return;
    }

    match response_rx.recv().await {
        Some(response) => {
            let _ = send_msg_to_player(framed, &player_name, &response).await;
            if let Response::ConnectionSuccess = &response {
                player_game_loop(
                    framed,
                    &player_name,
                    player_uuid,
                    &game_name,
                    action_tx,
                    response_rx,
                )
                .await;
            }
        }
        None => {
            eprintln!(
                "response_rx is closed for Game: `{}`; Player: `{}`",
                game_name, player_name
            );
            let _ = send_msg_to_player(
                framed,
                &player_name,
                &"Error: Response RX is closed for the Game".to_string(),
            )
            .await;
        }
    }
}

async fn player_game_loop(
    framed: &mut Com,
    player_name: &str,
    player_uuid: String,
    game_name: &str,
    action_tx: Sender<PlayerAction>,
    mut response_rx: Receiver<Response>,
) {
    while let Some(frame_result) = framed.next().await {
        match frame_result {
            Ok(msg) => {
                let action: Action = match serde_json::from_str(&msg) {
                    Ok(action) => action,
                    Err(_err) => {
                        let _ = send_msg_to_player(framed, player_name, &"InvalidAction").await;
                        continue;
                    }
                };

                let player_uuid = player_uuid.clone();
                let player_action = PlayerAction {
                    player_uuid,
                    action,
                };

                let _ = send_player_msg_to_game(
                    framed,
                    player_name,
                    game_name,
                    action_tx.clone(),
                    player_action,
                )
                .await;

                match response_rx.recv().await {
                    Some(response) => {
                        let _ = send_msg_to_player(framed, player_name, &response).await;
                    }
                    None => {
                        eprintln!(
                            "player_game_loop(): response_rx is closed for Game: `{}`; Player: `{}`",
                            game_name, player_name
                        );
                        let _ = send_msg_to_player(
                            framed,
                            player_name,
                            &"Error: Response RX is closed for the Game (@player_game_loop)"
                                .to_string(),
                        )
                        .await;
                    }
                }
            }
            Err(err) => {
                eprintln!(
                    "Connection Error with Player `{:?}`: `{}`",
                    player_name, err
                );
                break;
            }
        }
    }
}

async fn send_player_msg_to_game(
    framed: &mut Com,
    player_name: &str,
    game_name: &str,
    action_tx: Sender<PlayerAction>,
    player_action: PlayerAction,
) -> Result<(), ()> {
    if let Err(err) = action_tx.send(player_action).await {
        eprintln!(
            "Unable to send the Player `{}`'s action to the Game `{}`: `{}`",
            player_name, game_name, err
        );
        let _ = send_msg_to_player(
            framed,
            player_name,
            &format!(
                "Error: Unable to send the Action to the Game `{}`",
                game_name
            ),
        )
        .await;
        return Err(());
    }
    Ok(())
}

async fn send_msg_to_player<Msg: std::fmt::Debug + Serialize>(
    framed: &mut Com,
    player_name: &str,
    msg: &Msg,
) -> Result<(), ()> {
    match serde_json::to_string(msg) {
        Ok(msg_str) => {
            if let Err(err) = framed.send(&msg_str).await {
                eprintln!(
                    "Unable to send msg `{:?}` to Player `{}`: `{}`",
                    &msg, player_name, err
                );
                return Err(());
            }
        }
        Err(err) => {
            eprintln!(
                "Unable to serialize msg `{:?}` for Player `{}`: `{}`",
                msg, player_name, err
            );
            return Err(());
        }
    }

    Ok(())
}
