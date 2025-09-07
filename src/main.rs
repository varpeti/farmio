use crate::server::start_server;

mod game;
mod handle_connection;
mod send_to_player;
mod server;

#[tokio::main]
async fn main() {
    let ip_port = "127.0.0.1:5942";
    start_server(ip_port).await;
}
