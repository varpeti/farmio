use core::error;

use bevy::{asset::uuid::Uuid, prelude::*};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

pub async fn run_tcp_server(to_game_tx: mpsc::Sender<ToGameMsgWithPlayerUuid>, ip_port: &str) {
    let listener = TcpListener::bind(ip_port)
        .await
        .unwrap_or_else(|err| panic!("Unable to bind to `{}`: `{}`", ip_port, err));

    while let Ok((socket, addr)) = listener.accept().await {
        info!("New TCP connection: `{}`", addr);
        let framed = Framed::new(socket, LinesCodec::new());
        let to_game_tx_clone = to_game_tx.clone();
        tokio::spawn(async move { handle_connection(framed, to_game_tx_clone).await });
    }
    panic!("The run_tcp_server exited from listener loop!");
}

async fn handle_connection(
    framed: Framed<TcpStream, LinesCodec>,
    to_game_tx: Sender<ToGameMsgWithPlayerUuid>,
) {
    let (tcp_tx, tcp_rx) = framed.split();
    let (to_player_tx, to_player_rx) = mpsc::channel::<String>(1024);

    tokio::spawn(async move { send_to_player(to_player_rx, tcp_tx).await });
    tokio::spawn(async move { send_to_game(to_player_tx, tcp_rx, to_game_tx).await });
}

async fn send_to_player(
    mut to_player_rx: Receiver<String>,
    mut tcp_tx: futures::stream::SplitSink<Framed<TcpStream, LinesCodec>, String>,
) {
    while let Some(msg) = to_player_rx.recv().await {
        if let Err(err) = tcp_tx.send(msg).await {
            error!("Unable to send Msg to Player! (send_to_player): `{}`", err);
        }
    }
    info!("send_to_player exited")
}

async fn send_to_game(
    mut to_player_tx: Sender<String>,
    mut tcp_rx: futures::stream::SplitStream<Framed<TcpStream, LinesCodec>>,
    to_game_tx: Sender<ToGameMsgWithPlayerUuid>,
) {
    let mut player_uuid = None;

    while let Some(Ok(msg)) = tcp_rx.next().await {
        if let Ok(LobbyMsg::Connect { name: _, uuid }) = serde_json::from_str::<LobbyMsg>(&msg) {
            if player_uuid.is_some() {
                warn!("Player Already Connected!");
                msg_to_player(&mut to_player_tx, LobbyError::AlreadyConnected).await;
                continue;
            }
            player_uuid = Some(uuid);
            let msg = ToGameMsgWithPlayerUuid::new_with_tx(msg, uuid, to_player_tx.clone());
            if let Err(err) = to_game_tx.send(msg).await {
                error!("Unable to send Msg to Game: `{}`", err);
                msg_to_player(&mut to_player_tx, LobbyError::UnableToSendMsgToGame).await;
            }
            continue;
        }

        match player_uuid {
            Some(player_uuid) => {
                let msg = ToGameMsgWithPlayerUuid::new(msg, player_uuid);
                if let Err(err) = to_game_tx.send(msg).await {
                    error!("Unable to send Msg to Game: `{}`", err);
                    msg_to_player(&mut to_player_tx, LobbyError::UnableToSendMsgToGame).await;
                }
            }
            None => {
                warn!("Player is unconnected, but tried to send: `{}`", msg);
                msg_to_player(&mut to_player_tx, LobbyError::Unconnected).await;
                continue;
            }
        }
    }
    info!("send_to_game exited");

    match player_uuid {
        Some(player_uuid) => {
            let msg = ToGameMsgWithPlayerUuid::new("\"Disconnect\"".to_string(), player_uuid);
            if let Err(err) = to_game_tx.send(msg).await {
                error!("Unable to send Msg to Game: `{}`", err);
            }
        }
        None => {
            // Player was never connected
        }
    }
}

#[derive(Deserialize)]
enum LobbyMsg {
    Connect { name: String, uuid: Uuid },
}

#[derive(Debug, Serialize)]
enum LobbyError {
    Unconnected,
    UnableToSendMsgToGame,
    AlreadyConnected,
}

#[derive(Debug)]
pub struct ToGameMsgWithPlayerUuid {
    pub msg: String,
    pub player_uuid: Uuid,
    pub to_player_tx: Option<Sender<String>>,
}

impl ToGameMsgWithPlayerUuid {
    fn new(msg: String, player_uuid: Uuid) -> Self {
        Self {
            msg,
            player_uuid,
            to_player_tx: None,
        }
    }

    fn new_with_tx(msg: String, player_uuid: Uuid, to_player_tx: Sender<String>) -> Self {
        Self {
            msg,
            player_uuid,
            to_player_tx: Some(to_player_tx),
        }
    }
}

pub async fn msg_to_player<M: Serialize + std::fmt::Debug>(
    to_player_tx: &mut Sender<String>,
    msg: M,
) {
    match serde_json::to_string(&msg) {
        Err(err) => {
            error!("Unable to serialize Msg `{:?}` to Player: `{}`", msg, err)
        }
        Ok(msg) => {
            if let Err(err) = to_player_tx.send(msg).await {
                error!("Unable to send Msg to Player! (msg_to_player): `{}`", err);
            }
        }
    }
}

pub fn try_msg_to_player<M: Serialize + std::fmt::Debug>(
    to_player_tx: &mut Sender<String>,
    msg: M,
) {
    match serde_json::to_string(&msg) {
        Err(err) => {
            error!("Unable to serialize Msg `{:?}` to Player: `{}`", msg, err)
        }
        Ok(msg) => {
            if let Err(err) = to_player_tx.try_send(msg) {
                error!(
                    "Unable to send Msg to Player! (try_msg_to_player): `{}`",
                    err
                );
            }
        }
    }
}

#[derive(Resource)]
pub struct RToGameRx {
    pub to_game_rx: mpsc::Receiver<ToGameMsgWithPlayerUuid>,
}
