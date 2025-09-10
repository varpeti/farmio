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
    Move {
        direction: Direction,
    },
    Harvest,
    Plant {
        seed: Seed,
    },
    Trade {
        seed: Seed,
        volume: u32,
    },
    Till,
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
    // TODO: with current state
    Idled,
    Moved,
    BlockedBy(BlockedBy),
    Harvested,
    NoHarvest,
}

#[derive(Debug, Serialize)]
pub enum BlockedBy {
    AnotherPlayer,
}

#[derive(Debug, Serialize)]
pub struct MsgToPlayerWithGameContent {
    result: MsgToPlayer,
    cell: Cell,
    harvest: HashMap<Harvest, u32>,
    seed: HashMap<Seed, u32>,
    score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    number_of_players: u32,
    turn_duration_ms: u32,
    map_size: u32,
}

pub struct Game {
    game_name: String,
    to_game_rx: Receiver<PlayerAction>,
    game_settings: GameSettings,
    turns: u32,
    players: HashMap<Uuid, Player>,
    map: Map,
}

impl Game {
    pub fn new(
        game_name: String,
        to_game_rx: Receiver<PlayerAction>,
        game_settings: GameSettings,
    ) -> Self {
        let players = HashMap::new();
        let map = generate_map(game_settings.map_size as usize);
        Self {
            game_name,
            to_game_rx,
            game_settings,
            turns: 0,
            players,
            map,
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
                        let player = vacant_entry.insert(Player::new(
                            player_name,
                            to_player_tx,
                            self.game_settings.map_size as i32,
                        ));
                        self.map[player.pos.y as usize][player.pos.x as usize] = Cell {
                            ground: Ground::Stone,
                            plant: Plant::None,
                        };
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
        // TODO: clean up this enourmous function, break it into smaller ones
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
                        Entry::Occupied(occupied_entry) => {
                            let player = occupied_entry.into_mut();
                            println!("{} Player `{}` Reconnected", p, player_name);
                            player.to_player_tx = to_player_tx;
                            send_msg_to_player(&mut player.to_player_tx, MsgToPlayer::Reconnected)
                                .await;
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

            let mut next_positions = HashMap::<Pos, Vec<Uuid>>::new();

            for (player_uuid, action) in player_actions {
                let player = match self.players.get_mut(&player_uuid) {
                    Some(player) => player,
                    None => {
                        eprintln!("{} Player `{}` tried to do Action `{:?}`, but they are not Connected to the Game!", p, player_uuid, action);
                        continue;
                    }
                };

                // TODO: Make each action a separat function or something
                match action {
                    Action::Idle => {
                        msg_to_player_with_game_content(&self.map, player, MsgToPlayer::Idled)
                            .await;
                    }
                    Action::Move { direction } => {
                        // Collect Players desired movement
                        let next_pos = player
                            .pos
                            .get_next_pos_on_map(direction, self.game_settings.map_size as i32);
                        match next_positions.entry(next_pos) {
                            Entry::Occupied(occupied_entry) => {
                                occupied_entry.into_mut().push(player_uuid);
                            }
                            Entry::Vacant(vacant_entry) => {
                                vacant_entry.insert(vec![player_uuid]);
                            }
                        }
                    }
                    Action::Harvest => {
                        let pos = player.pos.clone();
                        let mut cell = self.map[pos.y as usize][pos.x as usize].clone();
                        let msg_to_player =
                            if let Some((harvest, volume, points)) = harvest(&cell.plant) {
                                match player.harvest.entry(harvest) {
                                    Entry::Occupied(occupied_entry) => {
                                        *occupied_entry.into_mut() += volume;
                                    }
                                    Entry::Vacant(vacant_entry) => {
                                        vacant_entry.insert(volume);
                                    }
                                }
                                player.score += points;

                                MsgToPlayer::Harvested
                            } else {
                                MsgToPlayer::NoHarvest
                            };

                        cell.plant = if let Ground::Dirt = cell.ground {
                            Plant::Wheat { growth: 0 }
                        } else {
                            Plant::None
                        };
                        self.map[pos.y as usize][pos.x as usize] = cell;
                        msg_to_player_with_game_content(&self.map, player, msg_to_player).await;
                    }
                    Action::Plant { seed } => todo!(),
                    Action::Trade { seed, volume } => todo!(),
                    Action::Till => todo!(),
                    Action::__Connect__ {
                        player_name: _,
                        to_player_tx: _,
                    } => unreachable!(),
                }
            }

            // Move Players if possible
            for (pos, uuids) in next_positions {
                if uuids.len() == 1 {
                    let player = self.players.get_mut(&uuids[0]).unwrap();
                    player.pos = pos;
                    msg_to_player_with_game_content(&self.map, player, MsgToPlayer::Moved).await;
                    // TODO: Remove
                    println!(
                        "TODO REMOVE {} Player `{}` moved to pos: {:?}",
                        p, player.player_name, player.pos
                    );
                } else {
                    for uuid in uuids {
                        let player = self.players.get_mut(&uuid).unwrap();
                        msg_to_player_with_game_content(
                            &self.map,
                            player,
                            MsgToPlayer::BlockedBy(BlockedBy::AnotherPlayer),
                        )
                        .await;
                    }
                }
            }

            println!("{} Next turn!", self.p());
        }
    }
}

struct Player {
    player_name: String,
    to_player_tx: Sender<String>,
    pos: Pos,
    harvest: HashMap<Harvest, u32>,
    seeds: HashMap<Seed, u32>,
    score: u32,
}

impl Player {
    fn new(player_name: String, to_player_tx: Sender<String>, map_size: i32) -> Self {
        let mut rng = rand::rng();
        Self {
            player_name,
            to_player_tx,
            pos: Pos {
                x: rng.random_range(0..map_size),
                y: rng.random_range(0..map_size),
            },
            harvest: HashMap::new(),
            seeds: HashMap::new(),
            score: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    fn to_pos(&self) -> Pos {
        match self {
            Direction::Up => Pos { x: 0, y: -1 },
            Direction::Right => Pos { x: 1, y: 0 },
            Direction::Down => Pos { x: 0, y: 1 },
            Direction::Left => Pos { x: -1, y: 0 },
        }
    }
}

type Map = Vec<Vec<Cell>>;

#[derive(Debug, Clone, Serialize)]
struct Cell {
    ground: Ground,
    plant: Plant,
}

fn get_cell(map: &Map, pos: &Pos) -> Cell {
    if let Some(line) = map.get(pos.y as usize) {
        if let Some(cell) = line.get(pos.x as usize) {
            return cell.to_owned();
        }
    }
    eprintln!(
        "Error: Pos `{:?}` is not present in the Map `{map_size}x{map_size}`",
        pos,
        map_size = map.len(),
    );
    Cell {
        ground: Ground::Error,
        plant: Plant::None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Ground {
    Dirt,
    Tiled,
    Sand,
    Water,
    Stone,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Plant {
    None,
    Wheat {
        growth: u8,
    },
    Bush {
        growth: u8,
        berries: u8,
        berries_growth: u8,
    },
    Tree {
        growth: u8,
    },
    Cane {
        growth: u8,
    },
    Pupkin {
        growth: u8,
        size: u32,
    },
    Cactus {
        growth: u8,
        size: u8,
        next_growth: u8,
    },
    Wallbush {
        growth: u8,
        health: u8,
    },
    Swapshroom {
        growth: u8,
        pair_id: Uuid,
    },
    Sunflower {
        growth: u8,
        rank: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Seed {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Harvest {
    Grains,
    Berry,
    Wood,
    Sugar,
    PumpkinSeeds,
    CactusMeat,
    Power,
}

// TODO: we need to &mut access the map and players here
fn harvest(plant: &Plant) -> Option<(Harvest, u32, u32)> {
    match plant {
        Plant::None => None,
        Plant::Wheat { growth } => {
            if *growth != 255u8 {
                return None;
            }
            Some((Harvest::Grains, 1, 1 * 1))
        }
        Plant::Bush {
            growth,
            berries,
            berries_growth: _,
        } => {
            if *growth != 255u8 {
                return None;
            }

            if *berries > 0u8 {
                Some((Harvest::Berry, *berries as u32, *berries as u32))
            } else {
                Some((Harvest::Wood, 1, 1 * 1))
            }
        }
        Plant::Tree { growth } => {
            if *growth != 255u8 {
                return None;
            }
            Some((Harvest::Wood, 16, 16 * 1))
        }
        Plant::Cane { growth } => {
            if *growth != 255u8 {
                return None;
            }
            Some((Harvest::Sugar, 3, 3 * 2))
        }
        Plant::Pupkin { growth, size } => todo!(),
        Plant::Cactus {
            growth,
            size,
            next_growth: _,
        } => todo!(),
        Plant::Wallbush { growth, health } => todo!(),
        Plant::Swapshroom { growth, pair_id } => todo!(),
        Plant::Sunflower { growth, rank } => todo!(),
    }
}

fn generate_map(map_size: usize) -> Map {
    let mut rng = rand::rng();
    let mut map = Map::with_capacity(map_size);
    for _y in 0..map_size {
        let mut line = Vec::with_capacity(map_size);
        for _x in 0..map_size {
            let cell = random_ground(&mut rng);
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
            plant: Plant::Wheat { growth: 255 },
        },
        70..75 => Cell {
            ground: Ground::Tiled,
            plant: Plant::Bush {
                growth: 255,
                berries: 4,
                berries_growth: 0,
            },
        },
        75..90 => Cell {
            ground: Ground::Sand,
            plant: Plant::None,
        },
        90..95 => Cell {
            ground: Ground::Sand,
            plant: Plant::Cane { growth: 255 },
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Pos {
    x: i32,
    y: i32,
}

impl Pos {
    // The map is a doughnut 🍩
    fn get_next_pos_on_map(&self, direction: Direction, map_size: i32) -> Self {
        let dp = direction.to_pos();
        Self {
            x: (self.x + dp.x).rem_euclid(map_size),
            y: (self.y + dp.y).rem_euclid(map_size),
        }
    }
}

async fn msg_to_player_with_game_content(map: &Map, player: &mut Player, result: MsgToPlayer) {
    let msg = MsgToPlayerWithGameContent {
        result,
        cell: get_cell(map, &player.pos),
        harvest: player.harvest.clone(),
        seed: player.seeds.clone(),
        score: player.score,
    };
    send_msg_to_player(&mut player.to_player_tx, msg).await;
}
