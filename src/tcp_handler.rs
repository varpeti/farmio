use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::codec::{Framed, LinesCodec};

type FramedConnection = Framed<TcpStream, LinesCodec>;
type TcpSender = futures::stream::SplitSink<FramedConnection, String>;
type TcpReceiver = futures::stream::SplitStream<FramedConnection>;
pub type Games<ToGame> = Arc<DashMap<String, Sender<ToGame>>>;

pub struct TcpHandler<ToGame, ToPlayer, GameSettings> {
    ip_port: String,
    _phantom: PhantomData<(ToGame, ToPlayer, GameSettings)>,
}

impl<ToGame, ToPlayer, GameSettings> TcpHandler<ToGame, ToPlayer, GameSettings>
where
    ToPlayer: std::fmt::Debug + Serialize + std::marker::Send + 'static,
    ToGame: std::fmt::Debug + Deserialize<'static> + std::marker::Send + 'static,
{
    pub fn new(ip_port: String) -> Self {
        Self {
            ip_port,
            _phantom: PhantomData,
        }
    }
    pub async fn run(&self) {
        let listener = TcpListener::bind(&self.ip_port)
            .await
            .unwrap_or_else(|_| panic!("Unable to bind to address: `{}`", self.ip_port));
        println!("Listening on `{}`", self.ip_port);

        let games: Games<ToGame> = Arc::new(DashMap::new());

        while let Ok((socket, addr)) = listener.accept().await {
            let framed = FramedConnection::new(socket, LinesCodec::new());
            let (tcp_sender, tcp_receiver): (TcpSender, TcpReceiver) = framed.split();
            let (to_game_tx, to_game_rx): (Sender<ToGame>, Receiver<ToGame>) = mpsc::channel(1024);
            let (to_player_tx, to_player_rx): (Sender<ToPlayer>, Receiver<ToPlayer>) =
                mpsc::channel(1024);
            let games = games.clone();
            tokio::spawn(async move {
                self.handle_tcp_receive(
                    (tcp_receiver, to_game_tx), // tcp_receiver hold them
                    (to_game_rx, to_player_tx), // game hold them
                    addr,
                    games,
                )
                .await
            });
            tokio::spawn(async move { self.handle_tcp_send(to_player_rx, tcp_sender).await });
        }

        unreachable!()
    }

    async fn handle_tcp_send(
        &self,
        mut to_player_rx: Receiver<ToPlayer>,
        mut tcp_sender: TcpSender,
    ) {
        while let Some(to_player) = to_player_rx.recv().await {
            match serde_json::to_string(&to_player) {
                Ok(msg) => {
                    if let Err(err) = tcp_sender.send(msg).await {
                        eprintln!("Failed to Send msg `{:?}` to Player: `{}`", to_player, err);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to Serialize msg `{:?}`: `{}`", to_player, err);
                    // TODO: Notify Player
                }
            }
        }
    }

    async fn handle_tcp_receive(
        &self,
        (mut tcp_receiver, to_game_tx): (TcpReceiver, Sender<ToGame>),
        (to_game_rx, to_player_tx): (Receiver<ToGame>, Sender<ToPlayer>),
        addr: SocketAddr,
        games: Games<ToGame>,
    ) {
        println!("Addr `{:?}` connecting...", addr);

        match tcp_receiver.next().await {
            Some(Ok(msg)) => todo!(),
            Some(Err(err)) => {
                eprintln!(
                    "Error with Addr `{}` in handle_connection/tcp_receiver.next() error: `{}`",
                    addr, err
                );
            }
            None => eprintln!(
                "Error with Addr `{}` in handle_connection/tcp_receiver.next() returned None",
                addr
            ),
        }

        println!("Addr `{:?}` disconnecting...", addr);
    }

    // async fn handle_message<'a>(
    //     tcp_receiver: TcpReceiver,
    //     addr: SocketAddr,
    //     games: Games<ToGame>,
    //     msg : &'a str,
    // ) where GameSettings: Deserialize<'a> {
    //     match serde_json::from_str::<LobbyAction<GameSettings>>(msg) {
    //         Ok(lobby_action) => handle_lobby_action(tcp_receiver, games, lobby_action).await,
    //         Err(err) => {
    //             eprintln!("Invalid message from Addr `{}`; The Error: `{}`", addr, err);
    //             // TODO: Send msg to Addr
    //         }
    //     }
    // }
    //
    // async fn handle_lobby_action<ToGame, GameSettings>(
    //     tcp_receiver: TcpReceiver,
    //     games: Games<ToGame>,
    //     lobby_action: LobbyAction<GameSettings>,
    // ) {
    //     let (player_name, player_uuid, game_name) = match lobby_action {
    //         LobbyAction::NewGame {
    //             player_name,
    //             player_uuid,
    //             game_name,
    //             player_count,
    //             map_size,
    //             turn_duration_ms,
    //         } => {
    //             let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(1024);
    //             let mut game = Game<ToGame>::new(
    //                 action_rx,
    //                 map_size,
    //                 player_count,
    //                 Duration::from_millis(turn_duration_ms),
    //             );
    //
    //             tokio::spawn(async move { game.run().await });
    //             games.insert(game_name.clone(), action_tx);
    //             println!(
    //                  "New Game by Player `{}`: game_name: {}, player_count: {}, map_size: {}, turn_duration_ms: {}",
    //                  player_name, game_name, player_count, map_size, turn_duration_ms
    //              );
    //             (player_name, player_uuid, game_name)
    //         }
    //         LobbyAction::JoinGame {
    //             player_name,
    //             player_uuid,
    //             game_name,
    //         } => (player_name, player_uuid, game_name),
    //     };
    //
    //     match games.get(&game_name).map(|e| e.clone()) {
    //         Some(action_tx) => {
    //             connect_player_to_the_game(tcp_receiver, player_name, player_uuid, game_name, action_tx)
    //                 .await
    //         }
    //         None => {
    //             eprintln!(
    //                 "Game `{}` not found. Requester: Player `{}`",
    //                 game_name, player_name,
    //             );
    //             // TODO: Send TCP msg to player
    //         }
    //     }
    // }
    //
    // async fn connect_player_to_the_game(
    //     tcp_receiver: TcpReceiver,
    //     player_name: String,
    //     player_uuid: String,
    //     game_name: String,
    //     action_tx: Sender<PlayerAction>,
    // ) {
    //     let (response_tx, mut response_rx) = mpsc::channel::<Response>(1024);
    //
    //     let player_action = PlayerAction {
    //         player_uuid: player_uuid.clone(),
    //         action: Action::__Connect__ {
    //             player_name: player_name.clone(),
    //             response_tx,
    //         },
    //     };
    //
    //     if send_player_msg_to_game(action_tx.clone(), player_action, &player_name, &game_name)
    //         .await
    //         .is_err()
    //     {
    //         return;
    //     }
    // }
    //
    // async fn player_game_loop(
    //     framed: &mut FramedConnection,
    //     player_name: &str,
    //     player_uuid: String,
    //     game_name: &str,
    //     action_tx: Sender<PlayerAction>,
    //     mut response_rx: Receiver<Response>,
    // ) {
    //     while let Some(frame_result) = framed.next().await {
    //         match frame_result {
    //             Ok(msg) => {
    //                 let action: Action = match serde_json::from_str(&msg) {
    //                     Ok(action) => action,
    //                     Err(_err) => {
    //                         let _ = send_msg_to_player(framed, player_name, &"InvalidAction").await;
    //                         continue;
    //                     }
    //                 };
    //
    //                 let player_uuid = player_uuid.clone();
    //                 let player_action = PlayerAction {
    //                     player_uuid,
    //                     action,
    //                 };
    //
    //                 let _ = send_player_msg_to_game(
    //                     framed,
    //                     player_name,
    //                     game_name,
    //                     action_tx.clone(),
    //                     player_action,
    //                 )
    //                 .await;
    //
    //                 // TODO: FIXME: Separate response_rx -> TcpSender to a different thread
    //                 // b2c84147-029e-451f-b509-b0fa5b236393
    //                 match response_rx.recv().await {
    //                     Some(response) => {
    //                         let _ = send_msg_to_player(framed, player_name, &response).await;
    //                     }
    //                     None => {
    //                         eprintln!(
    //                             "player_game_loop(): response_rx is closed for Game: `{}`; Player: `{}`",
    //                             game_name, player_name
    //                         );
    //                         let _ = send_msg_to_player(
    //                             framed,
    //                             player_name,
    //                             &"Error: Response RX is closed for the Game (@player_game_loop)"
    //                                 .to_string(),
    //                         )
    //                         .await;
    //                     }
    //                 }
    //             }
    //             Err(err) => {
    //                 eprintln!(
    //                     "Connection Error with Player `{:?}`: `{}`",
    //                     player_name, err
    //                 );
    //                 break;
    //             }
    //         }
    //     }
    // }
    //
    // async fn send_player_msg_to_game(
    //     action_tx: Sender<PlayerAction>,
    //     player_action: PlayerAction,
    //     _player_name: &str,
    //     _game_name: &str,
    // ) -> Result<(), ()> {
    //     if let Err(err) = action_tx.send(player_action).await {
    //         eprintln!(
    //             "Unable to send the Player `{}`'s action to the Game `{}`: `{}`",
    //             _player_name, _game_name, err
    //         );
    //         return Err(());
    //     }
    //     Ok(())
    // }
    //
    // async fn send_msg_to_player<Msg: std::fmt::Debug + Serialize>(
    //     framed: &mut FramedConnection,
    //     player_name: &str,
    //     msg: &Msg,
    // ) -> Result<(), ()> {
    //     match serde_json::to_string(msg) {
    //         Ok(msg_str) => {
    //             if let Err(err) = framed.send(&msg_str).await {
    //                 eprintln!(
    //                     "Unable to send msg `{:?}` to Player `{}`: `{}`",
    //                     &msg, player_name, err
    //                 );
    //                 return Err(());
    //             }
    //         }
    //         Err(err) => {
    //             eprintln!(
    //                 "Unable to serialize msg `{:?}` for Player `{}`: `{}`",
    //                 msg, player_name, err
    //             );
    //             return Err(());
    //         }
    //     }
    //
    //     Ok(())
    // }
}

#[derive(Debug, Deserialize)]
pub enum LobbyAction<GameSettings> {
    NewGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
        game_settings: GameSettings,
    },
    JoinGame {
        player_name: String,
        player_uuid: String,
        game_name: String,
    },
}
