use futures::SinkExt;
use serde::Serialize;
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

pub async fn send_to_player(
    mut to_player_rx: Receiver<String>,
    mut tcp_tx: futures::stream::SplitSink<Framed<TcpStream, LinesCodec>, String>,
) {
    while let Some(msg) = to_player_rx.recv().await {
        if let Err(err) = tcp_tx.send(msg).await {
            eprintln!("Unable to send Msg to Player! (send_to_player): `{}`", err);
        }
    }
}

pub async fn send_msg_to_player<M: Serialize + std::fmt::Debug>(
    to_player_tx: &mut Sender<String>,
    msg_to_player: M,
) {
    match serde_json::to_string(&msg_to_player) {
        Ok(msg) => {
            if let Err(err) = to_player_tx.send(msg).await {
                eprintln!(
                    "Unable to send Msg to Player! (send_msg_to_player): `{}`",
                    err
                );
            }
        }
        Err(err) => {
            eprintln!(
                "Unable to serialize Message `{:?}` to Player: `{}`",
                msg_to_player, err
            );
        }
    }
}
