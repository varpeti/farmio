mod game;
mod handle_connection;
mod map;
mod send_to_player;
mod server;

use crate::server::start_server;

#[tokio::main]
async fn main() {
    let ip_port = "127.0.0.1:5942";
    start_server(ip_port).await;
}
