use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

use crate::game::{Action, Game, PlayerAction};

pub type Games = Arc<DashMap<String, Sender<PlayerAction>>>;
type FramedConnection = Framed<TcpStream, LinesCodec>;
pub type TcpSender = futures::stream::SplitSink<FramedConnection, String>;
type TcpReceiver = futures::stream::SplitStream<FramedConnection>;

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

    let framed = FramedConnection::new(socket, LinesCodec::new());
    let (tcp_sender, mut tcp_receiver) = framed.split();

    match tcp_receiver.next().await {
        Some(Ok(msg)) => handle_message(tcp_sender, tcp_receiver, addr, games, msg).await,
        Some(Err(err)) => {
            eprintln!(
                "Error with Addr `{}` in handle_connection/tcp_receiver.next() error: `{}`",
                addr, err
            );
        }
        None => eprintln!(
            "Error with Addr `{}` in handle_connection/tcp_receiver.next() returned None",
            addr
        ),
    }

    println!("Addr `{:?}` disconnecting...", addr);
}

async fn handle_message(
    mut tcp_sender: TcpSender,
    tcp_receiver: TcpReceiver,
    addr: SocketAddr,
    games: Games,
    msg: String,
) {
    match serde_json::from_str::<LobbyAction>(&msg) {
        Ok(lobby_action) => {
            handle_lobby_action(tcp_sender, tcp_receiver, games, lobby_action).await
        }
        Err(err) => {
            eprintln!("Invalid message from Addr `{}`; The Error: `{}`", addr, err);
            send_msg_to_player(&mut tcp_sender, &addr.to_string(), &"InvalidAction").await;
        }
    }
}

async fn handle_lobby_action(
    mut tcp_sender: TcpSender,
    tcp_receiver: TcpReceiver,
    games: Games,
    lobby_action: LobbyAction,
) {
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
            connect_player_to_the_game_and_game_loop(
                tcp_sender,
                tcp_receiver,
                player_name,
                player_uuid,
                game_name,
                action_tx,
            )
            .await
        }
        None => {
            eprintln!(
                "Game `{}` not found. Requester: Player `{}`",
                game_name, player_name,
            );
            let _ = send_msg_to_player(
                &mut tcp_sender,
                &player_name,
                &format!("Error: The Game `{}` not found!", game_name),
            )
            .await;
        }
    }
}

async fn connect_player_to_the_game_and_game_loop(
    mut tcp_sender: TcpSender,
    tcp_receiver: TcpReceiver,
    player_name: String,
    player_uuid: String,
    game_name: String,
    action_tx: Sender<PlayerAction>,
) {
    // Connect
    let player_action = PlayerAction {
        player_uuid: player_uuid.clone(),
        action: Action::__Connect__ {
            player_name: player_name.clone(),
            tcp_sender: tcp_sender.clone(),
            // TODO: move back to mpsc from TCP
            // but separate thread for mpsc -> TCP
        },
    };

    if send_player_msg_to_game(
        &mut tcp_sender,
        &player_name,
        &game_name,
        action_tx.clone(),
        player_action,
    )
    .await
    .is_err()
    {
        return;
    }

    // Player's game loop
    while let Some(frame_result) = tcp_receiver.next().await {
        match frame_result {
            Ok(msg) => {
                let action: Action = match serde_json::from_str(&msg) {
                    Ok(action) => action,
                    Err(_err) => {
                        let _ = send_msg_to_player(&mut tcp_sender, &player_name, &"InvalidAction")
                            .await;
                        continue;
                    }
                };

                let player_uuid = player_uuid.clone();
                let player_action = PlayerAction {
                    player_uuid,
                    action,
                };

                let _ = send_player_msg_to_game(
                    &mut tcp_sender,
                    &player_name,
                    &game_name,
                    action_tx.clone(),
                    player_action,
                )
                .await;
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
    tcp_sender: &mut TcpSender,
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
            tcp_sender,
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
    tcp_sender: &mut TcpSender,
    player_name: &str,
    msg: &Msg,
) -> Result<(), ()> {
    match serde_json::to_string(msg) {
        Ok(msg_str) => {
            if let Err(err) = tcp_sender.send(msg_str).await {
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
