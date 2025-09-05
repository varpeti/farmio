use std::{collections::HashMap, time::Duration};

use rand::{rngs::ThreadRng, Rng};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug, Deserialize)]
pub enum Action {
    Idle,
    #[serde(skip_deserializing)]
    __Connect__ {
        player_name: String,
        response_tx: Sender<Response>,
    },
}

#[derive(Debug, Deserialize)]
pub struct PlayerAction {
    pub player_uuid: String,
    pub action: Action,
}

#[derive(Debug, Serialize)]
pub enum Response {
    Idle,
    Connected,
    GameStarted,
    Error(Error),
}

#[derive(Debug, Serialize)]
pub enum Error {
    ServerIsFull,
    WaitForOtherPlayers,
}

#[derive(Debug)]
pub struct Game {
    map_size: u32,
    map: Map,
    player_count: u32,
    players: Players,
    action_rx: Receiver<PlayerAction>,
    turn_duration: Duration,
}

impl Game {
    pub fn new(
        action_rx: Receiver<PlayerAction>,
        map_size: u32,
        number_of_players: u32,
        turn_duration: Duration,
    ) -> Game {
        let mut rng = rand::rng();
        Self {
            map_size,
            map: generate_map(map_size as usize, &mut rng),
            player_count: number_of_players,
            players: Players::new(),
            action_rx,
            turn_duration,
        }
    }
    pub async fn run(&mut self) {
        // Wait for everyone to join
        while let Some(player_action) = self.action_rx.recv().await {
            self.wait_for_join(player_action).await;
            if self.player_count as usize <= self.players.len() {
                for (_, player) in self.players.iter() {
                    send_response(
                        Response::GameStarted,
                        player.response_tx.clone(),
                        player.player_name.clone(),
                    )
                    .await;
                }

                break;
            }
        }

        // Game Loop
        while let Some(player_action) = self.action_rx.recv().await {
            self.handle_player_action(player_action).await;
        }
    }

    async fn wait_for_join(&mut self, player_action: PlayerAction) {
        if let Action::__Connect__ {
            player_name,
            response_tx,
        } = player_action.action
        {
            self.connect_player(player_action.player_uuid, player_name, response_tx)
                .await;
        } else if let Some(player) = self.players.get(&player_action.player_uuid) {
            eprintln!(
                "Player `{:?}` tried to `{:?}` in wait_for_join phase",
                player.player_name, player_action.action
            );
            send_response(
                Response::Error(Error::WaitForOtherPlayers),
                player.response_tx.clone(),
                player.player_name.clone(),
            )
            .await;
        } else {
            eprintln!(
                "Unconnected Player `{}` tried to `{:?}` in wait_for_join phase",
                player_action.player_uuid, player_action.action
            );
        };
    }

    async fn handle_player_action(&mut self, player_action: PlayerAction) {
        let player = self.players.get_mut(&player_action.player_uuid);
        let response = match (player_action.action, &player) {
            (
                Action::__Connect__ {
                    player_name,
                    response_tx,
                },
                _,
            ) => {
                self.connect_player(player_action.player_uuid.clone(), player_name, response_tx)
                    .await;
                return; // connect_player sends a response, we maybe don't have a new player
            }
            (Action::Idle, _) => Response::Idle,
        };

        if let Some(player) = player {
            send_response(
                response,
                player.response_tx.clone(),
                player.player_name.clone(),
            )
            .await;
        }
    }

    async fn connect_player(
        &mut self,
        player_uuid: String,
        player_name: String,
        response_tx: Sender<Response>,
    ) {
        let response = match (
            self.players.len() >= self.player_count as usize,
            self.players.contains_key(&player_uuid),
        ) {
            (_, true) => {
                eprintln!("Player `{}` reconnected!", player_name);
                Response::Connected
            }
            (true, false) => {
                eprintln!("Player `{}` tried to connect but it is full!", player_name);
                Response::Error(Error::ServerIsFull)
            }
            (false, false) => {
                self.players.insert(
                    player_uuid,
                    Player::new_with_random_position(
                        response_tx.clone(),
                        self.map_size as i32,
                        player_name.clone(),
                    ),
                );
                eprintln!("Player `{}` connected!", player_name);
                Response::Connected
            }
        };
        send_response(response, response_tx, player_name).await;
    }
}

async fn send_response(response: Response, response_tx: Sender<Response>, player_name: String) {
    if let Err(err) = response_tx.send(response).await {
        eprintln!(
            "Error when sending back Response to Player `{}`: `{}`",
            player_name, err
        );
    }
}

type Map = Vec<Vec<Cell>>;

#[derive(Debug, Serialize)]
struct Cell {
    ground: Ground,
    plant: Plant,
}

#[derive(Debug, Serialize, Deserialize)]
enum Ground {
    Dirt,
    Tiled,
    Sand,
    Water,
    Stone,
}

#[derive(Debug, Serialize, Deserialize)]
enum Plant {
    None,
    Wheat,
    Bush,
    Tree,
    Cane,
    Pupkin,
    Cactus,
    Wallbush,
    Swapshroom,
    Sunflower,
}

#[derive(Debug, Serialize)]
enum Resource {
    Grains,
    Berry,
    Wood,
    Sugar,
    PumpkinSeeds,
    CactusMeat,
    Power,
}

fn generate_map(map_size: usize, rng: &mut ThreadRng) -> Map {
    let mut map = Map::with_capacity(map_size);
    for _y in 0..map_size {
        let mut line = Vec::with_capacity(map_size);
        for _x in 0..map_size {
            let cell = random_ground(rng);
            line.push(cell);
        }
        map.push(line);
    }
    map
}

fn random_ground(rng: &mut ThreadRng) -> Cell {
    let cell = match rng.random_range(0..99) {
        0..70 => Cell {
            ground: Ground::Dirt,
            plant: Plant::Wheat,
        },
        70..75 => Cell {
            ground: Ground::Dirt,
            plant: Plant::Bush,
        },
        75..90 => Cell {
            ground: Ground::Sand,
            plant: Plant::None,
        },
        90..95 => Cell {
            ground: Ground::Sand,
            plant: Plant::Cane,
        },
        95..99 => Cell {
            ground: Ground::Water,
            plant: Plant::None,
        },
        num => {
            eprintln!("random_ground unreachable generated! `{}`", num);
            Cell {
                ground: Ground::Dirt,
                plant: Plant::None,
            }
        }
    };
    cell
}

type Players = HashMap<String, Player>;

#[derive(Debug)]
struct Player {
    response_tx: Sender<Response>,
    player_name: String,
    pos: Pos,
    resources: Vec<Resource>,
    plants: Vec<Plant>,
    score: u32,
}

impl Player {
    fn new_with_random_position(
        response_tx: Sender<Response>,
        map_size: i32,
        player_name: String,
    ) -> Self {
        let mut rng = rand::rng();
        Player {
            response_tx,
            player_name,
            pos: Pos {
                x: rng.random_range(0..map_size),
                y: rng.random_range(0..map_size),
            },
            resources: Vec::new(),
            plants: Vec::new(),
            score: 0,
        }
    }
}

#[derive(Debug)]
struct Pos {
    x: i32,
    y: i32,
}
