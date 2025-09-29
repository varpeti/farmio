use bevy::{asset::uuid::Uuid, prelude::*};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

const IP_PORT: &str = "127.0.0.1:5942";

///////////
// Entities
///////////

/////////////
// Components
/////////////

#[derive(Component)]
struct Player;

#[derive(Component, Debug)]
struct Position {
    x: u32,
    y: u32,
}

#[derive(Component)]
struct Name(String);

#[derive(Component)]
struct Id(Uuid);

//////////
// Bundles
//////////

#[derive(Bundle)]
struct PlayerBundle {
    marker: Player,
    name: Name,
    id: Id,
    position: Position,
}

//////////
// Systems
//////////

fn print_position(query: Query<(&Position, &Name), With<Player>>) {
    for (position, name) in &query {
        //println!("{}: {:?}", name.0, position);
    }
}

fn new_player(mut commands: Commands) {
    commands.spawn(PlayerBundle {
        marker: Player,
        name: Name("P001".to_string()),
        id: Id(Uuid::new_v4()),
        position: Position { x: 0, y: 0 },
    });
}

fn main() {
    let (player_action_tx, player_action_rx) = mpsc::channel(1024);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Unable to create Tokio Runtime!");
        rt.block_on(async {
            run_tcp_server(player_action_tx, IP_PORT).await;
        })
    });

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(PlayerActionResource(player_action_rx))
        .add_systems(Startup, new_player)
        .add_systems(Update, print_position)
        .run();
}

//////////////
// TCP Network
//////////////

pub async fn run_tcp_server(player_action_tx: mpsc::Sender<MsgToGame>, ip_port: &str) {
    let listener = TcpListener::bind(ip_port)
        .await
        .unwrap_or_else(|err| panic!("Unable to bind to `{}`: `{}`", ip_port, err));

    while let Ok((socket, addr)) = listener.accept().await {
        info!("New TCP connection: `{}`", addr);
        let framed = Framed::new(socket, LinesCodec::new());
        let player_action_tx_clone = player_action_tx.clone();
        tokio::spawn(async move { handle_connection(framed, player_action_tx_clone).await });
    }
    panic!("The run_tcp_server exited from listener loop!");
}

pub async fn handle_connection(
    framed: Framed<TcpStream, LinesCodec>,
    player_action_tx: Sender<MsgToGame>,
) {
    let (tcp_tx, tcp_rx) = framed.split();
    let (to_player_tx, to_player_rx) = mpsc::channel::<MsgToPlayer>(1024);

    tokio::spawn(async move { send_to_player(to_player_rx, tcp_tx).await });
    tokio::spawn(async move { send_to_game(to_player_tx, tcp_rx, player_action_tx).await });
}

pub async fn send_to_player<ToPlayer: Serialize + std::fmt::Debug>(
    mut to_player_rx: Receiver<ToPlayer>,
    mut tcp_tx: futures::stream::SplitSink<Framed<TcpStream, LinesCodec>, String>,
) {
    while let Some(msg) = to_player_rx.recv().await {
        match serde_json::to_string(&msg) {
            Err(err) => {
                error!("Unable to serialize Msg `{:?}` to Player: `{}`", msg, err)
            }
            Ok(msg) => {
                if let Err(err) = tcp_tx.send(msg).await {
                    error!("Unable to send Msg to Player! (send_to_player): `{}`", err);
                }
            }
        }
    }
    info!("send_to_player exited")
}

pub async fn send_to_game(
    to_player_tx: Sender<MsgToPlayer>,
    mut tcp_rx: futures::stream::SplitStream<Framed<TcpStream, LinesCodec>>,
    to_game_tx: Sender<MsgToGame>,
) {
    let mut _s_player_name: Option<String> = None;
    let mut s_player_uuid: Option<Uuid> = None;

    while let Some(Ok(msg)) = tcp_rx.next().await {
        match serde_json::from_str::<MsgToServer>(&msg) {
            Ok(msg) => {
                let msg = match msg {
                    MsgToServer::Connect {
                        player_name,
                        player_uuid,
                    } => {
                        s_player_uuid = Some(player_uuid);
                        _s_player_name = Some(player_name.clone());

                        MsgToGame {
                            msg: MsgToServer::Connect {
                                player_name,
                                player_uuid,
                            },
                            player_uuid,
                            to_player_tx: Some(to_player_tx.clone()),
                        }
                    }
                    _ => match s_player_uuid {
                        Some(player_uuid) => MsgToGame {
                            msg,
                            player_uuid,
                            to_player_tx: None,
                        },

                        None => {
                            error!("Unconnected!");
                            todo!()
                        }
                    },
                };
                if let Err(err) = to_game_tx.send(msg.clone()).await {
                    error!("Unable to send Msg `{:?}` to game: `{}` ", msg, err);
                    todo!()
                }
            }
            Err(err) => {
                error!("Invalid message! `{}`", err);
                todo!()
            }
        }
    }
    info!("send_to_game exited")
}

#[derive(Debug, Clone, Deserialize)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

#[derive(Debug, Clone, Deserialize)]
pub enum MsgToServer {
    Connect {
        player_name: String,
        player_uuid: Uuid,
    },
    Move {
        direction: Direction,
    },
}

#[derive(Debug, Clone)]
pub struct MsgToGame {
    pub msg: MsgToServer,
    pub player_uuid: Uuid,
    pub to_player_tx: Option<Sender<MsgToPlayer>>,
}

#[derive(Resource)]
pub struct PlayerActionResource(mpsc::Receiver<MsgToGame>);

#[derive(Debug, Serialize)]
pub enum MsgToPlayer {
    Ok(String),
    Err(String),
}
