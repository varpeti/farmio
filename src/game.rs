use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Serialize, Clone)]
pub struct Cell {
    pub ground: Ground,
    pub plant: Plant,
}

/*
* Dirt + Nothin => Wheat 8 tick => 1 Wheat
* (Tilled, Dirt) + Berry => Bush 8 tick, +1 Fruit for each 2 tick max 4 => Berries (num of fruits) on 1st harvest, 1 Wood
* Sand + Cane => Cane 8 tick => Cane
*/

#[derive(Debug, Serialize, Clone)]
pub enum Ground {
    Dirt,
    Tilled,
    Sand,
    Stone,
    Water,
}

#[derive(Debug, Serialize, Clone)]
pub enum Plant {
    None,
    Wheat,
    Bush { fruits: usize },
    Cane,
    Tree,
    Cactus { size: usize },
    Pumpkin,
    WallBush { life: usize },
    SwapperShroom { pair_id: usize },
    Mandrake { life: usize },
    SunFlower { power: usize },
}

#[derive(Debug, Serialize, Clone)]
pub enum Resource {
    Wheat,
    Berry,
    Sapling,
    Wood,
    Cane,
    Cactus,
    Seed,
    Power,
    Pumpkin,
}

#[derive(Debug)]
pub struct Position {
    pub x: isize,
    pub y: isize,
}

impl Position {
    fn new(x: isize, y: isize) -> Position {
        Self { x, y }
    }

    fn move_by_direction(&mut self, direction: Direction, map_size: usize) {
        // The map is like a torus, it wraps around
        // rem_euclid is the mathematical modulus
        let v = direction.value();
        self.x = (self.x + v.x).rem_euclid(map_size as isize);
        self.y = (self.y + v.y).rem_euclid(map_size as isize);
    }
}

#[derive(Debug, Deserialize)]
pub enum Direction {
    Up,
    Rigth,
    Down,
    Left,
}

impl Direction {
    fn value(&self) -> Position {
        match *self {
            Direction::Up => Position { x: 0, y: -1 },
            Direction::Rigth => Position { x: 1, y: 0 },
            Direction::Down => Position { x: 0, y: 1 },
            Direction::Left => Position { x: -1, y: 0 },
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum Action {
    Move { to: Direction },
    Harvest,
    Idle,
}

type PlayerId = usize;

#[derive(Debug)]
pub struct PlayerAction {
    pub player_name: String,
    pub secret_name: String,
    pub action: Action,
    pub response_tx: oneshot::Sender<Response>,
}

#[derive(Debug, Serialize)]
pub struct Response {
    action_result: ActionResult,
    cell: Cell,
    inventory: Inventory,
}

impl Response {
    fn new(action_result: ActionResult, cell: Cell, inventory: Inventory) -> Response {
        Response {
            action_result,
            cell,
            inventory,
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ActionResult {
    SuccessMove,
    MoveBlockedBy(BlockedBy),
    SuccessHarvers,
    FailedHarvest,
    SuccessIdle,
    InvalidAction,
    Connected {
        players_connected: usize,
        player_count: usize,
        map_size: usize,
        turn_duration_ms: u64,
    },
    GameStarting,
}

#[derive(Debug, Serialize)]
pub enum BlockedBy {
    Player,
    WallBush,
    Mandrake,
    SwapperShroom,
}

type Inventory = HashMap<Resource, usize>;

type SecretName = String;

#[derive(Debug)]
pub struct Player {
    pub player_name: String,
    pub position: Position,
    pub inventory: Inventory,
}

impl Player {
    fn new_with_random_position(player_name: String, map_size: usize) -> Player {
        let mut rng = rand::rng();
        let x = rng.random_range(0..map_size) as isize;
        let y = rng.random_range(0..map_size) as isize;
        Player {
            player_name,
            position: Position { x, y },
            inventory: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Game {
    map_size: usize,
    map: Vec<Vec<Cell>>,
    player_count: usize,
    players: HashMap<SecretName, Player>,
    action_rx: mpsc::Receiver<PlayerAction>,
    turn_duration: Duration,
}

impl Game {
    pub fn new(
        action_rx: mpsc::Receiver<PlayerAction>,
        map_size: usize,
        number_of_players: usize,
        turn_duration: Duration,
    ) -> Game {
        let mut map = Vec::with_capacity(map_size);
        for _y in 0..map_size {
            let mut line = Vec::with_capacity(map_size);
            for _x in 0..map_size {
                line.push(Cell {
                    ground: Ground::Dirt,
                    plant: Plant::None,
                });
            }
            map.push(line);
        }

        Self {
            map_size,
            map,
            player_count: number_of_players,
            players: HashMap::new(),
            action_rx,
            turn_duration,
        }
    }

    pub async fn run(&mut self) {
        self.wait_for_connections().await;
        println!("Game started!");
        loop {
            self.process_turn().await;
        }
    }

    async fn wait_for_connections(&mut self) {
        let mut players_connected = 0;
        while players_connected < self.player_count {
            match self.action_rx.recv().await {
                Some(player_action) => {
                    let player = self
                        .players
                        .entry(player_action.secret_name)
                        .or_insert_with(|| {
                            let player = Player::new_with_random_position(
                                player_action.secret_name,
                                self.map_size,
                            );
                            println!("Player {} connected!", player_action.player_name);
                            players_connected += 1;
                            player
                        });

                    ActionResult::Connected {
                        players_connected,
                        player_count: self.player_count,
                        map_size: self.map_size,
                        turn_duration_ms: self.turn_duration.as_millis() as u64,
                    };

                    if player_action
                        .response_tx
                        .send(Response {
                            action_result,
                            cell: Cell {
                                ground: Ground::Stone,
                                plant: Plant::None,
                            },
                            inventory: HashMap::new(),
                        })
                        .is_err()
                    {
                        eprintln!("Unable to send back the result to the client handler (wait_for_connections). (Player: {})", player_action.player_id);
                    };
                }
                None => {
                    eprintln!("The action_rx channel closed (Durring connection) ?");
                    break;
                }
            }
        }
    }

    async fn process_turn(&mut self) {
        let mut pending_actions = HashMap::<PlayerId, PlayerAction>::new();

        let deadline = tokio::time::Instant::now() + self.turn_duration;

        let mut players_left_to_take_action = self.player_count;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(
                deadline - tokio::time::Instant::now(),
                self.action_rx.recv(),
            )
            .await
            {
                Ok(Some(player_action)) => {
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        pending_actions.entry(player_action.player_id)
                    {
                        e.insert(player_action);
                        players_left_to_take_action -= 1;
                        if players_left_to_take_action == 0 {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    eprintln!("The action_rx channel closed?");
                    break;
                }
                Err(_) => break, // Timeout
            }
        }

        if !pending_actions.is_empty() {
            self.resolve_actions(pending_actions).await;
        }
    }

    async fn resolve_actions(&mut self, pending_actions: HashMap<PlayerId, PlayerAction>) {
        for (player_id, player_action) in pending_actions {
            // Get Player, or connect them if they missed the connection duration.
            let player = self.players.entry(player_id).or_insert_with(|| {
                println!("Player {} Connected during game", player_id);
                Player::new_with_random_position(self.map_size)
            });

            // Take action
            let action_result = match player_action.action {
                Action::Move { to } => {
                    println!("TODO: Move {:?}", to);
                    // TODO: Check if they can move there
                    player.position.move_by_direction(to, self.map_size);
                    ActionResult::SuccessMove
                }
                Action::Harvest => {
                    println!("TODO: Harvest");
                    ActionResult::SuccessHarvers
                }
                Action::Idle => {
                    println!("TODO: Idle");
                    ActionResult::SuccessIdle
                }
            };

            // Modify environment

            let cell = self.map[player.position.x as usize][player.position.y as usize].to_owned();

            // Send response
            let response = Response {
                action_result,
                cell,
                inventory: player.inventory.to_owned(),
            };
            if player_action.response_tx.send(response).is_err() {
                eprintln!(
                    "Unable to send back the result to the client handler. (Player: {})",
                    player_action.player_id
                )
            }
        }
    }
}
