use std::time::Duration;

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
    ConnectionSuccess,
    ConnectionDenied,
}

#[derive(Debug)]
pub struct Game {
    map_size: u32,
    map: Map,
    player_count: u32,
    //players: HashMap<String, Player>,
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
            action_rx,
            turn_duration,
        }
    }
    pub async fn run(&mut self) {
        while let Some(player_action) = self.action_rx.recv().await {
            self.handle_player_action(player_action).await;
        }
    }

    async fn handle_player_action(&mut self, player_action: PlayerAction) {
        println!("Got PlayerAction: `{:?}`", player_action);
        let response = match player_action.action {
            Action::__Connect__ {
                player_name,
                response_tx,
            } => {
                let response = Response::ConnectionSuccess;
                self.send_response(response, response_tx, player_name).await;
            }
            Action::Idle => {
                let response = Response::Idle;
                println!("TODO: Resoponse {:?}", response);
            }
        };
    }

    async fn send_response(
        &mut self,
        response: Response,
        response_tx: Sender<Response>,
        player_name: String,
    ) {
        if let Err(err) = response_tx.send(response).await {
            eprintln!(
                "Error when sending back Response to Player `{}`: `{}`",
                player_name, err
            );
        }
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
enum Resources {
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
