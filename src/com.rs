use std::sync::Arc;

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

use crate::{game::Game, lobby::lobby};

type FramedConnection = Framed<TcpStream, LinesCodec>;
type TcpSender = futures::stream::SplitSink<FramedConnection, String>;
type TcpReceiver = futures::stream::SplitStream<FramedConnection>;
pub type Games = Arc<DashMap<String, Game>>;

pub async fn com(ip_port: &str) {
    let listener = TcpListener::bind(&ip_port)
        .await
        .unwrap_or_else(|_| panic!("Unable to bind to address: {}", ip_port));
    println!("Listening on {}", ip_port);

    let games: Games = Arc::new(DashMap::new());

    while let Ok((socket, _addr)) = listener.accept().await {
        let framed = FramedConnection::new(socket, LinesCodec::new());
        let (tcp_sender, tcp_receiver): (TcpSender, TcpReceiver) = framed.split();
        let (to_game_tx, to_game_rx): (Sender<String>, Receiver<String>) = mpsc::channel(1024);
        let (to_player_tx, to_player_rx): (Sender<String>, Receiver<String>) = mpsc::channel(1024);
        let games = games.clone();
        tokio::spawn(async move {
            handle_tcp_receive(
                (tcp_receiver, to_game_tx),
                (to_game_rx, to_player_tx),
                games,
            )
            .await
        });
        tokio::spawn(async move { handle_tcp_send(to_player_rx, tcp_sender).await });
    }
    unreachable!()
}

async fn handle_tcp_receive(
    (mut tcp_receiver, to_game_tx): (TcpReceiver, Sender<String>),
    (mut to_game_rx, to_player_tx): (Receiver<String>, Sender<String>),
    games: Games,
) {
    let mut connection_state = ConnectionState::Lobby;
    while let Some(Ok(msg)) = tcp_receiver.next().await {
        match connection_state {
            ConnectionState::Lobby => {
                if lobby(&msg, &mut to_game_rx, to_player_tx.clone(), games).await {
                    connection_state = ConnectionState::Game;
                } else {
                    connection_state = ConnectionState::Lobby;
                }
            }
            ConnectionState::Game => {
                // game(&msg, to_game_rx, to_player_tx).await;
                connection_state = ConnectionState::Lobby;
            }
        }
    }
}

async fn handle_tcp_send(mut to_player_rx: Receiver<String>, mut tcp_sender: TcpSender) {
    while let Some(msg) = to_player_rx.recv().await {
        if let Err(err) = tcp_sender.send(msg.clone()).await {
            eprintln!("Unable to send Msg `{}` to Player: `{}`", msg, err);
        }
    }
}

enum ConnectionState {
    Lobby,
    Game,
}
