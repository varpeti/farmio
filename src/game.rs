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
    Harvested {
        harvest: Harvest,
        volume: u32,
    },
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
        // TODO: Check if all players could fit in the map
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
                let player_positions = self
                    .players
                    .values()
                    .map(|player| player.pos.clone())
                    .collect();
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
                            player_positions,
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
            let player_actions = self.collect_player_actions(turn_duration).await;
            self.process_player_actions(player_actions).await;
            println!("{} Next turn!", self.p());
        }
    }

    async fn collect_player_actions(&mut self, turn_duration: Duration) -> HashMap<Uuid, Action> {
        let p = self.p();
        let mut player_actions = HashMap::<Uuid, Action>::new();
        while let Ok(Some(player_action)) = timeout(turn_duration, self.to_game_rx.recv()).await {
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
        player_actions
    }

    async fn process_player_actions(&mut self, player_actions: HashMap<Uuid, Action>) {
        let p = self.p();
        let mut next_positions = HashMap::<Pos, Vec<Uuid>>::new();
        let mut moving = false;

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
                    msg_to_player_with_game_content(&self.map, player, MsgToPlayer::Idled).await
                }
                Action::Move { direction } => {
                    action_move_collection(
                        &self.map,
                        player,
                        player_uuid,
                        Some(direction),
                        &mut next_positions,
                    );
                    moving = true;
                }
                Action::Harvest => action_harvest(&mut self.map, player).await,
                Action::Plant { seed } => todo!(),
                Action::Trade { seed, volume } => todo!(),
                Action::Till => todo!(),
                Action::__Connect__ {
                    player_name: _,
                    to_player_tx: _,
                } => unreachable!(),
            }
            if !moving {
                action_move_collection(&self.map, player, player_uuid, None, &mut next_positions);
            }
        }
        action_move_execution(&self.map, &mut self.players, next_positions).await;

        update_map(&mut self.map);
    }
}

fn update_map(map: &mut Map) {
    for line in map.iter_mut() {
        for cell in line.iter_mut() {
            // TODO: Water
            match &mut cell.plant {
                Plant::None => {
                    if let Ground::Dirt = cell.ground {
                        cell.plant = Plant::Wheat { growth: 0 };
                    }
                }
                Plant::Wheat { growth } => {
                    if *growth < G_WHEAT_GRAINS {
                        *growth += 1;
                    }
                }
                Plant::Bush { growth, berries } => {
                    if *growth < G_BUSH_WOOD {
                        *growth += 1;
                    } else if *growth < G_BUSH_WOOD + MAX_BUSH_BERRIES * G_BUSH_BERRIES {
                        *growth += 1;
                        if (*growth - G_BUSH_WOOD) % G_BUSH_BERRIES == 0 {
                            *berries += 1;
                        }
                    }
                }
                Plant::Tree { growth } => {
                    // TODO: Stop growth if a neighbour is Tree
                    if *growth < G_TREE_WOOD {
                        *growth += 1;
                    }
                }
                Plant::Cane { growth } => {
                    if *growth < G_CANE_SUGAR {
                        *growth += 1;
                    }
                }
                Plant::Pumpkin { growth, size } => {
                    if *growth < G_PUMPKIN_PUMPKINSEEDS {
                        *growth += 1;
                    }
                    // TODO: Check square pumpkin formulation, to update size
                }
                Plant::Cactus { growth, size } => {
                    if *growth < G_CACTUS_CACTUSMEAT * MAX_CACTUS_CACTUSMEAT {
                        *growth += 1;
                        if *growth % G_CACTUS_CACTUSMEAT == 0 {
                            *size += 1;
                        }
                    }
                }
                Plant::Wallbush { growth, health: _ } => {
                    if *growth < G_WALLBUSH {
                        *growth += 1;
                    }
                }
                Plant::Swapshroom {
                    pair_id: _,
                    active: _,
                } => {
                    // TODO: activate Swapshroom if pair is placed
                }
                Plant::Sunflower { growth, rank: _ } => {
                    if *growth < G_SUNFLOWER_POWER {
                        *growth += 1;
                    }
                }
            }
        }
    }
}

fn action_move_collection(
    map: &Map,
    player: &mut Player,
    player_uuid: Uuid,
    direction: Option<Direction>,
    next_positions: &mut HashMap<Pos, Vec<Uuid>>,
) {
    let next_pos = player.pos.get_next_pos_on_map(direction, map.len() as i32);
    match next_positions.entry(next_pos) {
        Entry::Occupied(occupied_entry) => {
            occupied_entry.into_mut().push(player_uuid);
        }
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(vec![player_uuid]);
        }
    }
}

async fn action_move_execution(
    map: &Map,
    players: &mut HashMap<Uuid, Player>,
    next_positions: HashMap<Pos, Vec<Uuid>>,
) {
    for (pos, uuids) in next_positions {
        if uuids.len() == 1 {
            let player = players.get_mut(&uuids[0]).unwrap();
            if player.pos == pos {
                continue;
            }
            player.pos = pos;
            msg_to_player_with_game_content(map, player, MsgToPlayer::Moved).await;
        } else {
            for uuid in uuids {
                let player = players.get_mut(&uuid).unwrap();
                if player.pos == pos {
                    continue;
                }
                msg_to_player_with_game_content(
                    map,
                    player,
                    MsgToPlayer::BlockedBy(BlockedBy::AnotherPlayer),
                )
                .await;
            }
        }
    }
}

async fn action_harvest(map: &mut Map, player: &mut Player) {
    let pos = player.pos.clone();
    let mut cell = map[pos.y as usize][pos.x as usize].clone();

    let msg_to_player = match cell.plant {
        Plant::None => MsgToPlayer::NoHarvest,
        Plant::Wheat { growth } => {
            if growth == G_WHEAT_GRAINS {
                cell.plant = Plant::None;
                player.harvest(Harvest::Grains, V_WHEAT_GRAINS)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Bush { growth, berries } => {
            if berries > 0 {
                cell.plant = Plant::Bush {
                    growth: G_BUSH_WOOD,
                    berries: 0,
                };
                player.harvest(Harvest::Berry, berries as u32)
            } else if growth >= G_BUSH_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, V_BUSH_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Tree { growth } => {
            if growth == G_TREE_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, V_TREE_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Cane { growth } => {
            if growth == G_CANE_SUGAR {
                cell.plant = Plant::None;
                player.harvest(Harvest::Sugar, V_CANE_SUGAR)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Pumpkin { growth, size } => {
            todo!()
        }
        Plant::Cactus { growth, size } => {
            if growth >= G_CACTUS_CACTUSMEAT {
                cell.plant = Plant::None;
                player.harvest(Harvest::CactusMeat, size as u32)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Wallbush {
            growth: _,
            health: _,
        } => MsgToPlayer::NoHarvest,

        Plant::Swapshroom { pair_id, active } => {
            if active {
                todo!();
            } else {
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Sunflower { growth, rank } => {
            if growth == G_SUNFLOWER_POWER {
                todo!()
            }
            todo!()
        }
    };

    map[pos.y as usize][pos.x as usize] = cell;
    msg_to_player_with_game_content(map, player, msg_to_player).await;
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
    fn new(
        player_name: String,
        to_player_tx: Sender<String>,
        map_size: i32,
        player_positions: Vec<Pos>,
    ) -> Self {
        let mut rng = rand::rng();

        let mut x;
        let mut y;
        loop {
            (x, y) = (rng.random_range(0..map_size), rng.random_range(0..map_size));
            let mut ok = true;
            for pos in player_positions.iter() {
                if pos.x == x && pos.y == y {
                    ok = false;
                }
            }
            if ok {
                break;
            }
        }

        Self {
            player_name,
            to_player_tx,
            pos: Pos { x, y },
            harvest: HashMap::new(),
            seeds: HashMap::new(),
            score: 0,
        }
    }

    fn harvest(&mut self, harvest: Harvest, volume: u32) -> MsgToPlayer {
        match self.harvest.entry(harvest.clone()) {
            Entry::Occupied(occupied_entry) => {
                *occupied_entry.into_mut() += volume;
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(volume);
            }
        }
        MsgToPlayer::Harvested { harvest, volume }
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
    Wheat { growth: u8 },
    Bush { growth: u8, berries: u8 },
    Tree { growth: u8 },
    Cane { growth: u8 },
    Pumpkin { growth: u8, size: u32 },
    Cactus { growth: u8, size: u8 },
    Wallbush { growth: u8, health: u8 },
    Swapshroom { pair_id: Uuid, active: bool },
    Sunflower { growth: u8, rank: u8 },
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

const G_WHEAT_GRAINS: u8 = 8;
const V_WHEAT_GRAINS: u32 = 1;
const P_WHEAT_GRAINS: u32 = 1; // (1*1)/8 = 0.125

const G_BUSH_WOOD: u8 = 10;
const V_BUSH_WOOD: u32 = 1;
const P_BUSH_WOOD: u32 = 1; // (1*1)/10 = 0.100

const G_BUSH_BERRIES: u8 = 3;
const P_BUSH_BERRIES: u32 = 2; // (2*1)/3 = 0.667
const MAX_BUSH_BERRIES: u8 = 4;

const G_TREE_WOOD: u8 = 16;
const V_TREE_WOOD: u32 = 16;
const P_TREE_WOOD: u32 = 1; // (1*16)/10 = 1.000

const G_CANE_SUGAR: u8 = 9;
const V_CANE_SUGAR: u32 = 3;
const P_CANE_SUGAR: u32 = 2; // (2*3)/9 = 0.667

const G_PUMPKIN_PUMPKINSEEDS: u8 = 4;
fn v_pumpkin_pumpkinseeds(x: u32) -> u32 {
    x * x * x
}
const P_PUMPKIN_PUMPKINSEEDS: u32 = 3;
// (3*(1*1*1))/(8+(1*1-1)*2) = 0.375
// (3*(2*2*2))/(8+(2*2-1)*2) = 1.714
// (3*(3*3*3))/(8+(3*3-1)*2) = 3.375
// (3*(4*4*4))/(8+(4*4-1)*2) = 5.053
// (3*(5*5*5))/(8+(5*5-1)*2) = 6.696
// (3*(6*6*6))/(8+(6*6-1)*2) = 8.308
// (3*(7*7*7))/(8+(7*7-1)*2) = 9.894
// (3*(8*8*8))/(8+(8*8-1)*2) = 11.463

const G_CACTUS_CACTUSMEAT: u8 = 5;
const P_CACTUS_CACTUSMEAT: u32 = 10; // (10*1)/5 = 2.000
const MAX_CACTUS_CACTUSMEAT: u8 = 3;

const G_WALLBUSH: u8 = 7;

const G_SUNFLOWER_POWER: u8 = 4;
const V_SUNFLOWER_POWER: u8 = 1;
const P_SUNFLOWER_POWER: u32 = 1024; // (1024*1)/4 = 256 (solo play)

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
            plant: Plant::Wheat {
                growth: G_WHEAT_GRAINS,
            },
        },
        70..75 => Cell {
            ground: Ground::Tiled,
            plant: Plant::Bush {
                growth: G_BUSH_WOOD + 4 * G_BUSH_BERRIES,
                berries: 4,
            },
        },
        75..90 => Cell {
            ground: Ground::Sand,
            plant: Plant::None,
        },
        90..95 => Cell {
            ground: Ground::Sand,
            plant: Plant::Cane {
                growth: G_CANE_SUGAR,
            },
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
    fn get_next_pos_on_map(&self, direction: Option<Direction>, map_size: i32) -> Self {
        match direction {
            Some(direction) => {
                // The map is a doughnut 🍩
                let dp = direction.to_pos();
                Self {
                    x: (self.x + dp.x).rem_euclid(map_size),
                    y: (self.y + dp.y).rem_euclid(map_size),
                }
            }
            None => self.clone(),
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
