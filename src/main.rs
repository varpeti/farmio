use crate::com::com;

mod com;
mod game;
mod lobby;

#[tokio::main]
async fn main() {
    let ip_port = "127.0.0.1:5942";
    com(ip_port).await;
}
