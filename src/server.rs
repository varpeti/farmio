use std::sync::Arc;

use dashmap::DashMap;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
};
use tokio_util::codec::{Framed, LinesCodec};

use crate::handle_connection::{handle_connection, PlayerAction};

pub type FramedConnection = Framed<TcpStream, LinesCodec>;
pub type Games = Arc<DashMap<String, Sender<PlayerAction>>>;

pub async fn start_server(ip_port: &str) {
    let listener = TcpListener::bind(&ip_port)
        .await
        .unwrap_or_else(|_| panic!("Unable to bind to address: {}", ip_port));
    println!("Listening on {}", ip_port);

    let games: Games = Arc::new(DashMap::new());

    while let Ok((socket, _addr)) = listener.accept().await {
        let framed = Framed::new(socket, LinesCodec::new());
        let games = games.clone();
        tokio::spawn(async move { handle_connection(framed, games).await });
    }
    unreachable!()
}
