mod game;
mod lobby;

use crate::lobby::new_lobby;

#[tokio::main]
async fn main() {
    let ip_port = "127.0.0.1:5942";
    new_lobby(ip_port).await;
}

// use std::{net::SocketAddr, sync::Arc, time::Duration};
//
// use crate::{
//     game::{Action, ActionResult, Game, PlayerAction, Response},
//     lobby::new_lobby,
// };
//
// use dashmap::DashMap;
// use serde::{Deserialize, Serialize};
// use tokio::{
//     net::{TcpListener, TcpStream},
//     sync::mpsc::{self, Receiver, Sender},
// };
//
// use futures::{SinkExt, StreamExt};
// use tokio_util::codec::{Framed, LinesCodec};
//
// #[derive(Debug, Deserialize)]
// pub enum MainLobbyReq {
//     NewGame {
//         player_name: String,
//         secret_name: String,
//         game_name: String,
//         player_count: usize,
//         map_size: usize,
//         turn_duration_ms: u64,
//     },
//     JoinGame {
//         player_name: String,
//         secret_name: String,
//         game_name: String,
//     },
// }
//
// #[derive(Debug, Serialize)]
// pub enum MainLobbyRes {
//     Connected,
//     GameStarting,
//     InvalidMsg,
// }
//
// type Games = Arc<DashMap<String, Sender<PlayerAction>>>;
//
// #[tokio::main]
// async fn main() {
//     let ip_port = "127.0.0.1:5942";
//
//     new_lobby(ip_port).await;
//
//     let listener = TcpListener::bind(ip_port)
//         .await
//         .unwrap_or_else(|_| panic!("Unable to bind to address: `{}`", ip_port));
//     println!("Game server listening on `{}`", ip_port);
//
//     let games: Games = Arc::new(DashMap::new());
//
//     while let Ok((socket, addr)) = listener.accept().await {
//         let games = games.clone();
//         tokio::spawn(async move { handle_connection(socket, addr, games).await });
//     }
// }
//
// pub async fn handle_connection(socket: TcpStream, addr: SocketAddr, games: Games) {
//     let mut framed = Framed::new(socket, LinesCodec::new());
//
//     println!("Player `{:?}` connecting...", addr);
//
//     while let Some(frame_result) = framed.next().await {
//         match frame_result {
//             Ok(msg) => match serde_json::from_str::<MainLobbyReq>(msg.as_str()) {
//                 Ok(main_lobby_req) => {
//                     handle_main_lobby_req(&mut framed, addr, main_lobby_req, games.clone()).await
//                 }
//                 Err(err) => {
//                     if framed
//                         .send(serde_json::to_string(&MainLobbyRes::InvalidMsg).unwrap())
//                         .await
//                         .is_err()
//                     {
//                         eprintln!(
//                             "Unable to send results `{:?}` to Player `{:?}`: `{}`. ",
//                             MainLobbyRes::InvalidMsg,
//                             addr,
//                             err
//                         );
//                     }
//                 }
//             },
//             Err(err) => {
//                 eprintln!("Connection Error with Player `{:?}`: `{}`", addr, err);
//                 break;
//             }
//         }
//     }
//     println!("Player `{:?}` disconnected", addr);
// }
//
// pub async fn handle_main_lobby_req(
//     framed: &mut Framed<TcpStream, LinesCodec>,
//     addr: SocketAddr,
//     main_lobby_req: MainLobbyReq,
//     games: Games,
// ) {
//     match main_lobby_req {
//         MainLobbyReq::NewGame {
//             player_name,
//             secret_name,
//             game_name,
//             player_count,
//             map_size,
//             turn_duration_ms,
//         } => {
//             let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(1024);
//             let mut game = Game::new(
//                 action_rx,
//                 map_size,
//                 player_count,
//                 Duration::from_millis(turn_duration_ms),
//             );
//
//             tokio::spawn(async move { game.run().await });
//             games.insert(game_name.clone(), action_tx);
//             println!(
//                 "New Game: game_name: {}, player_count: {}, map_size: {}, turn_duration_ms: {}",
//                 game_name, player_count, map_size, turn_duration_ms
//             );
//
//             connect_player_to_game(player_name, secret_name, game_name, games, framed, addr).await;
//         }
//         MainLobbyReq::JoinGame {
//             player_name,
//             secret_name,
//             game_name,
//         } => connect_player_to_game(player_name, secret_name, game_name, games, framed, addr).await,
//     }
// }
//
// pub async fn connect_player_to_game(
//     player_name: String,
//     secret_name: String,
//     game_name: String,
//     games: Games,
//     framed: &mut Framed<TcpStream, LinesCodec>,
//     addr: SocketAddr,
// ) {
//     let action_tx_option = games.get(&game_name).map(|e| e.clone());
//     match action_tx_option {
//         Some(action_tx) => {
//             let (response_tx, mut response_rx) = mpsc::channel::<Response>(1024);
//
//             let player_action = PlayerAction {
//                 player_name: player_name.clone(),
//                 secret_name: secret_name.clone(),
//                 action: Action::__Connect__ { response_tx },
//             };
//
//             if let Err(err) = action_tx.send(player_action).await {
//                 eprintln!(
//                     "Unable to join the Player `{}|{}` to the Game: `{}`: `{}`",
//                     player_name, addr, game_name, err
//                 );
//                 return;
//             }
//
//             match response_rx.recv().await {
//                 Some(response) => {
//                     if let Err(err) = framed.send(serde_json::to_string(&response).unwrap()).await {
//                         eprintln!(
//                             "Unable to send connection results `{:?}` to Player `{:?}|{}`: `{}`. ",
//                             response, addr, player_name, err
//                         );
//                     }
//                 }
//                 None => {
//                     eprintln!(
//                         "response_rx is closed for Game: `{}`; Player: `{}|{}`",
//                         game_name, addr, player_name
//                     );
//                 }
//             }
//
//             player_gameloop(
//                 player_name,
//                 secret_name,
//                 action_tx,
//                 response_rx,
//                 framed,
//                 addr,
//             )
//             .await;
//         }
//         None => {
//             eprintln!(
//                 "Player `{}|{}` Unable to join Game `{}`, because it is not existing!",
//                 game_name, player_name, addr
//             );
//             if let Err(err) = framed
//                 .send(format!(
//                     "Unable to join Game `{}`, it does not exists!",
//                     game_name
//                 ))
//                 .await
//             {
//                 eprintln!("Unable to send Player back the message: Unable to join the Game `{}`, it does not exists! The Error: `{}`", game_name, err);
//             }
//         }
//     }
// }
//
// pub async fn player_gameloop(
//     player_name: String,
//     secret_name: String,
//     action_tx: Sender<PlayerAction>,
//     mut response_rx: Receiver<Response>,
//     framed: &mut Framed<TcpStream, LinesCodec>,
//     addr: SocketAddr,
// ) {
//     while let Some(frame_result) = framed.next().await {
//         match frame_result {
//             Ok(msg) => match serde_json::from_str::<Action>(msg.as_str()) {
//                 Ok(action) => {
//                     if let Err(err) = action_tx
//                         .send(PlayerAction {
//                             action,
//                             player_name: player_name.clone(),
//                             secret_name: secret_name.clone(),
//                         })
//                         .await
//                     {
//                         eprintln!(
//                             "Unable to send results Action to the Game of Player `{}|{}`; The error: {}",
//                             addr, player_name, err
//                         );
//                     }
//
//                     // Wait for response from the game server, and send it to the client
//                     if let Some(response) = response_rx.recv().await {
//                         if let Err(err) =
//                             framed.send(serde_json::to_string(&response).unwrap()).await
//                         {
//                             eprintln!(
//                                 "Unable to send results `{:?}` to Player `{:?}|{}`: `{}`. ",
//                                 response, addr, player_name, err
//                             );
//                         }
//                     }
//                 }
//                 Err(err) => {
//                     if framed
//                         .send(serde_json::to_string(&ActionResult::InvalidAction).unwrap())
//                         .await
//                         .is_err()
//                     {
//                         eprintln!(
//                             "Unable to send results `{:?}` to Player `{:?}|{}`: `{}`. ",
//                             ActionResult::InvalidAction,
//                             addr,
//                             player_name,
//                             err
//                         );
//                     }
//                 }
//             },
//             Err(err) => {
//                 eprintln!("Connection Error with Player `{:?}`: `{}`", addr, err);
//                 break;
//             }
//         }
//     }
// }
