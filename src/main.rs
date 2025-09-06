mod tcp_handler;

use serde::{Deserialize, Serialize};

use crate::tcp_handler::TcpHandler;

// TODO: Remove these
#[derive(Debug, Deserialize)]
enum PlayerAction {}
#[derive(Debug, Serialize)]
enum Response {}
#[derive(Debug, Deserialize)]
enum GameSettings {}

#[tokio::main]
async fn main() {
    let ip_port = "127.0.0.1:5942";
    TcpHandler::<PlayerAction, Response, GameSettings>::run(ip_port).await;
}
