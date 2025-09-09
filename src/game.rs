use rand::{rngs::ThreadRng, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    time::Duration,
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::timeout,
};
use uuid::Uuid;

use crate::{handle_connection::PlayerAction, send_to_player::send_msg_to_player};

#[derive(Debug, Deserialize)]
pub enum Action {
    Idle,
    #[serde(skip_deserializing)]
    __Connect__ {
        player_name: String,
        to_player_tx: Sender<String>,
    },
    // TODO: __Disconnect__
}

#[derive(Debug, Serialize)]
pub enum MsgToPlayer {
    Connected {
        game_settings: GameSettings,
        players_connected: u32,
    },
    AlreadyConnected,
    Reconnected,
    WaitingOtherPlayersToJoin,
    GameIsFull,
    GameStarted,
    Idle, // TODO: with current state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    number_of_players: u32,
    turn_duration_ms: u32,
}

pub struct Game {
    game_name: String,
    to_game_rx: Receiver<PlayerAction>,
    game_settings: GameSettings,
    turns: u32,
    players: HashMap<Uuid, Player>,
}

impl Game {
    pub fn new(
        game_name: String,
        to_game_rx: Receiver<PlayerAction>,
        game_settings: GameSettings,
    ) -> Self {
        Self {
            game_name,
            to_game_rx,
            game_settings,
            turns: 0,
            players: HashMap::new(),
        }
    }

    fn p(&self) -> String {
        format!(
            "Game `{}` ({}/{}) #{}",
            self.game_name,
            self.players.len(),
            self.game_settings.number_of_players,
            self.turns,
        )
    }

    pub async fn run(&mut self) {
        self.wait_for_connections().await;
        self.game_loop().await;
        // TODO: Kill the game if all players are disconnecting
    }

    async fn wait_for_connections(&mut self) {
        while let Some(player_action) = self.to_game_rx.recv().await {
            let p = self.p();
            if let Action::__Connect__ {
                player_name,
                mut to_player_tx,
            } = player_action.action
            {
                let players_connected = self.players.len() as u32;
                match self.players.entry(player_action.player_uuid) {
                    Entry::Occupied(_occupied_entry) => {
                        eprintln!("{} Player `{}` Already Connected", p, player_name,);
                        send_msg_to_player(&mut to_player_tx, MsgToPlayer::AlreadyConnected).await;
                    }
                    Entry::Vacant(vacant_entry) => {
                        let player = vacant_entry.insert(Player::new(player_name, to_player_tx));
                        println!("{} Player `{}` Connected", p, player.player_name);
                        send_msg_to_player(
                            &mut player.to_player_tx,
                            MsgToPlayer::Connected {
                                game_settings: self.game_settings.clone(),
                                players_connected: players_connected + 1,
                            },
                        )
                        .await;
                    }
                }
                if self.players.len() as u32 == self.game_settings.number_of_players {
                    break;
                }
            } else if let Some(player) = self.players.get_mut(&player_action.player_uuid) {
                println!(
                    "Player `{}` sent non __Connect__ Action `{:?}` in wait_for_connections phase in Game `{}`",
                    player_action.player_uuid, player_action.action , self.game_name
                );
                send_msg_to_player(
                    &mut player.to_player_tx,
                    MsgToPlayer::WaitingOtherPlayersToJoin,
                )
                .await;
            } else {
                eprintln!("Player `{}` is not connected and sent non __Connect__ Action `{:?}` in wait_for_connections pahse in Game `{}` ", 
                    player_action.player_uuid, player_action.action, self.game_name);
            }
        }

        // Everyone is Connected, Notify the Players
        println!(
            "{} Everyone (`{}`/`{}`) is Connected to the Game `{}`, the Game Starts!",
            self.p(),
            self.game_settings.number_of_players,
            self.players.len(),
            self.game_name
        );
        for (_player_uuid, player) in self.players.iter_mut() {
            send_msg_to_player(&mut player.to_player_tx, MsgToPlayer::GameStarted).await;
        }
    }

    async fn game_loop(&mut self) {
        let turn_duration = Duration::from_millis(self.game_settings.turn_duration_ms as u64);
        loop {
            self.turns += 1;
            let p = self.p();

            // Collect the Player Actions for the turn
            let mut player_actions = HashMap::<Uuid, Action>::new();
            while let Ok(Some(player_action)) = timeout(turn_duration, self.to_game_rx.recv()).await
            {
                if let Action::__Connect__ {
                    player_name,
                    mut to_player_tx,
                } = player_action.action
                {
                    match self.players.entry(player_action.player_uuid) {
                        Entry::Occupied(mut occupied_entry) => {
                            println!("{} Player `{}` Reconnected", p, player_name);
                            occupied_entry.insert(Player::new(player_name, to_player_tx.clone()));
                            send_msg_to_player(&mut to_player_tx, MsgToPlayer::Reconnected).await;
                        }
                        Entry::Vacant(_vacant_entry) => {
                            // This is unreachable, because the Player guard on top of the game_loop
                            eprintln!(
                            "{} Player `{}` tried to connect to the game, but it is already full!",
                            p, player_name
                        );
                            send_msg_to_player(&mut to_player_tx, MsgToPlayer::GameIsFull).await;
                        }
                    }
                    continue; // Connect Action is not counted towards this turn
                }
                // Players can overwrite their own action
                player_actions.insert(player_action.player_uuid, player_action.action);

                // If all player did an action we can fastforward to the processing of the turn
                if player_actions.len() == self.players.len() {
                    break;
                }
            }

            // Process Player Actions for the turn
            for (player_uuid, action) in player_actions {
                let player = match self.players.get_mut(&player_uuid) {
                    Some(player) => player,
                    None => {
                        eprintln!("{} Player `{}` tried to do Action `{:?}`, but they are not Connected to the Game!", p, player_uuid, action);
                        continue;
                    }
                };

                match action {
                    Action::Idle => {
                        send_msg_to_player(&mut player.to_player_tx, MsgToPlayer::Idle).await
                    }
                    Action::__Connect__ {
                        player_name: _,
                        to_player_tx: _,
                    } => unreachable!(),
                }
            }

            println!("{} Next turn!", self.p());
        }
    }
}

struct Player {
    player_name: String,
    to_player_tx: Sender<String>,
}

impl Player {
    fn new(player_name: String, to_player_tx: Sender<String>) -> Self {
        Self {
            player_name,
            to_player_tx,
        }
    }
}

//
// #[derive(Debug, Deserialize)]
// pub struct PlayerAction {
//     pub player_uuid: String,
//     pub action: Action,
// }
//
// #[derive(Debug, Serialize)]
// pub enum Response {
//     Idle,
//     Connected,
//     GameStarted,
//     Error(Error),
// }
//
// #[derive(Debug, Serialize)]
// pub enum Error {
//     ServerIsFull,
//     WaitForOtherPlayers,
// }
//
// #[derive(Debug, Deserialize)]
// pub struct GameSettings {
//     map_size: u32,
//     player_count: u32,
//     turn_duration: Duration,
// }
//
// #[derive(Debug)]
// pub struct Game<'a> {
//     //map: Map,
//     //players: Players,
//     to_game_rx: &'a mut Receiver<String>,
//     game_settings: GameSettings,
// }
//
// impl<'a> Game<'a> {
//     pub fn new(to_game_rx: &'a mut Receiver<String>, game_settings: GameSettings) -> Game {
//         let mut rng = rand::rng();
//         Self {
//             //map: generate_map(map_size as usize, &mut rng),
//             //players: Players::new(),
//             to_game_rx,
//             game_settings,
//         }
//     }
// }

//     pub async fn run(&mut self) {
//         // Wait for everyone to join
//         while let Some(player_action) = self.to_game_rx.recv().await {
//             self.wait_for_join(player_action).await;
//             if self.player_count as usize <= self.players.len() {
//                 for (_, player) in self.players.iter() {
//                     send_response(
//                         Response::GameStarted,
//                         player.response_tx.clone(),
//                         player.player_name.clone(),
//                     )
//                     .await;
//                 }
//
//                 break;
//             }
//         }
//
//         // Game Loop
//         while let Some(player_action) = self.to_game_rx.recv().await {
//             self.handle_player_action(player_action).await;
//         }
//     }
//
//     async fn wait_for_join(&mut self, player_action: PlayerAction) {
//         if let Action::__Connect__ {
//             player_name,
//             response_tx,
//         } = player_action.action
//         {
//             self.connect_player(player_action.player_uuid, player_name, response_tx)
//                 .await;
//         } else if let Some(player) = self.players.get(&player_action.player_uuid) {
//             eprintln!(
//                 "Player `{:?}` tried to `{:?}` in wait_for_join phase",
//                 player.player_name, player_action.action
//             );
//             send_response(
//                 Response::Error(Error::WaitForOtherPlayers),
//                 player.response_tx.clone(),
//                 player.player_name.clone(),
//             )
//             .await;
//         } else {
//             eprintln!(
//                 "Unconnected Player `{}` tried to `{:?}` in wait_for_join phase",
//                 player_action.player_uuid, player_action.action
//             );
//         };
//     }
//
//     async fn handle_player_action(&mut self, player_action: PlayerAction) {
//         let player = self.players.get_mut(&player_action.player_uuid);
//         let response = match (player_action.action, &player) {
//             (
//                 Action::__Connect__ {
//                     player_name,
//                     response_tx,
//                 },
//                 _,
//             ) => {
//                 self.connect_player(player_action.player_uuid.clone(), player_name, response_tx)
//                     .await;
//                 return; // connect_player sends a response, we maybe don't have a new player
//             }
//             (Action::Idle, _) => Response::Idle,
//         };
//
//         if let Some(player) = player {
//             send_response(
//                 response,
//                 player.response_tx.clone(),
//                 player.player_name.clone(),
//             )
//             .await;
//         }
//     }
//
//     async fn connect_player(
//         &mut self,
//         player_uuid: String,
//         player_name: String,
//         response_tx: Sender<Response>,
//     ) {
//         let response = match (
//             self.players.len() >= self.player_count as usize,
//             self.players.contains_key(&player_uuid),
//         ) {
//             (_, true) => {
//                 eprintln!("Player `{}` reconnected!", player_name);
//                 Response::Connected
//             }
//             (true, false) => {
//                 eprintln!("Player `{}` tried to connect but it is full!", player_name);
//                 Response::Error(Error::ServerIsFull)
//             }
//             (false, false) => {
//                 self.players.insert(
//                     player_uuid,
//                     Player::new_with_random_position(
//                         response_tx.clone(),
//                         self.map_size as i32,
//                         player_name.clone(),
//                     ),
//                 );
//                 eprintln!("Player `{}` connected!", player_name);
//                 Response::Connected
//             }
//         };
//         send_response(response, response_tx, player_name).await;
//     }
// }
//
// async fn send_response(response: Response, response_tx: Sender<Response>, player_name: String) {
//     if let Err(err) = response_tx.send(response).await {
//         eprintln!(
//             "Error when sending back Response to Player `{}`: `{}`",
//             player_name, err
//         );
//     }
// }
//
// type Map = Vec<Vec<Cell>>;
//
// #[derive(Debug, Serialize)]
// struct Cell {
//     ground: Ground,
//     plant: Plant,
// }
//
// #[derive(Debug, Serialize, Deserialize)]
// enum Ground {
//     Dirt,
//     Tiled,
//     Sand,
//     Water,
//     Stone,
// }
//
// #[derive(Debug, Serialize, Deserialize)]
// enum Plant {
//     None,
//     Wheat,
//     Bush,
//     Tree,
//     Cane,
//     Pupkin,
//     Cactus,
//     Wallbush,
//     Swapshroom,
//     Sunflower,
// }
//
// #[derive(Debug, Serialize)]
// enum Resource {
//     Grains,
//     Berry,
//     Wood,
//     Sugar,
//     PumpkinSeeds,
//     CactusMeat,
//     Power,
// }
//
// fn generate_map(map_size: usize, rng: &mut ThreadRng) -> Map {
//     let mut map = Map::with_capacity(map_size);
//     for _y in 0..map_size {
//         let mut line = Vec::with_capacity(map_size);
//         for _x in 0..map_size {
//             let cell = random_ground(rng);
//             line.push(cell);
//         }
//         map.push(line);
//     }
//     map
// }
//
// fn random_ground(rng: &mut ThreadRng) -> Cell {
//     let cell = match rng.random_range(0..99) {
//         0..70 => Cell {
//             ground: Ground::Dirt,
//             plant: Plant::Wheat,
//         },
//         70..75 => Cell {
//             ground: Ground::Dirt,
//             plant: Plant::Bush,
//         },
//         75..90 => Cell {
//             ground: Ground::Sand,
//             plant: Plant::None,
//         },
//         90..95 => Cell {
//             ground: Ground::Sand,
//             plant: Plant::Cane,
//         },
//         95..99 => Cell {
//             ground: Ground::Water,
//             plant: Plant::None,
//         },
//         num => {
//             eprintln!("random_ground unreachable generated! `{}`", num);
//             Cell {
//                 ground: Ground::Dirt,
//                 plant: Plant::None,
//             }
//         }
//     };
//     cell
// }
//
// type Players = HashMap<String, Player>;
//
// #[derive(Debug)]
// struct Player {
//     response_tx: Sender<Response>,
//     player_name: String,
//     pos: Pos,
//     resources: Vec<Resource>,
//     plants: Vec<Plant>,
//     score: u32,
// }
//
// impl Player {
//     fn new_with_random_position(
//         response_tx: Sender<Response>,
//         map_size: i32,
//         player_name: String,
//     ) -> Self {
//         let mut rng = rand::rng();
//         Player {
//             response_tx,
//             player_name,
//             pos: Pos {
//                 x: rng.random_range(0..map_size),
//                 y: rng.random_range(0..map_size),
//             },
//             resources: Vec::new(),
//             plants: Vec::new(),
//             score: 0,
//         }
//     }
// }
//
// #[derive(Debug)]
// struct Pos {
//     x: i32,
//     y: i32,
// }
