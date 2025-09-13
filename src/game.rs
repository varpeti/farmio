use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    time::Duration,
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::timeout,
};
use uuid::Uuid;

use crate::{
    handle_connection::PlayerAction,
    map::{Cell, Direction, GameConsts, Ground, Harvest, Map, Plant, Pos, Seed},
    send_to_player::send_msg_to_player,
};

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
    seed: u64,
}

pub struct Game {
    game_name: String,
    to_game_rx: Receiver<PlayerAction>,
    game_settings: GameSettings,
    turns: u32,
    rng: SmallRng,
    players: HashMap<Uuid, Player>,
    map: Map,
}

impl Game {
    pub fn new(
        game_name: String,
        to_game_rx: Receiver<PlayerAction>,
        game_settings: GameSettings,
    ) -> Self {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(game_settings.seed);
        let players = HashMap::new();
        let map = Map::generate_map(game_settings.map_size as usize, &mut rng);
        map.print_map_with_players(&HashMap::new());
        // TODO: Check if all players could fit in the map
        Self {
            game_name,
            to_game_rx,
            game_settings,
            turns: 0,
            rng,
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
                            &mut self.rng,
                        ));
                        self.map.set_cell(
                            &player.pos,
                            Cell {
                                ground: Ground::Stone,
                                plant: Plant::None,
                            },
                        );
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
            let player_actions = self.collect_player_actions(turn_duration).await;
            self.process_player_actions(player_actions).await;
            self.map.update_map();
            // TODO: use log instead of prints, or separate the prints form drawing
            println!("{}", self.p());
            self.map.print_map_with_players(
                &self
                    .players
                    .values()
                    .map(|p| (p.pos.clone(), p.player_name.clone()))
                    .collect(),
            );
            self.turns += 1;
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
        let mut moving_players = Vec::<Uuid>::new();

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
                    moving_players.push(player_uuid);
                    action_move_collection(
                        &self.map,
                        player,
                        player_uuid,
                        Some(direction),
                        &mut next_positions,
                    );
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
        }
        action_move_execution(&self.map, &mut self.players, next_positions, moving_players).await;
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
    mut next_positions: HashMap<Pos, Vec<Uuid>>,
    moving_players: Vec<Uuid>,
) {
    for (player_uuid, player) in players.iter_mut() {
        if !moving_players.contains(player_uuid) {
            action_move_collection(
                map,
                player,
                player_uuid.to_owned(),
                None,
                &mut next_positions,
            );
        }
    }
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
    let mut cell = map.get_cell(&player.pos).clone();
    let msg_to_player = match cell.plant {
        Plant::None => MsgToPlayer::NoHarvest,
        Plant::Wheat { growth } => {
            if growth == GameConsts::G_WHEAT_GRAINS {
                cell.plant = Plant::None;
                player.harvest(Harvest::Grains, GameConsts::V_WHEAT_GRAINS)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Bush { growth, berries } => {
            if berries > 0 {
                cell.plant = Plant::Bush {
                    growth: GameConsts::G_BUSH_WOOD,
                    berries: 0,
                };
                player.harvest(Harvest::Berry, berries as u32)
            } else if growth >= GameConsts::G_BUSH_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, GameConsts::V_BUSH_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Tree { growth } => {
            if growth == GameConsts::G_TREE_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, GameConsts::V_TREE_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Cane { growth } => {
            if growth == GameConsts::G_CANE_SUGAR {
                cell.plant = Plant::None;
                player.harvest(Harvest::Sugar, GameConsts::V_CANE_SUGAR)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Pumpkin { growth, size } => {
            todo!()
        }
        Plant::Cactus { growth, size } => {
            if growth >= GameConsts::G_CACTUS_CACTUSMEAT {
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
            if growth == GameConsts::G_SUNFLOWER_POWER {
                todo!()
            }
            todo!()
        }
    };

    map.set_cell(&player.pos, cell);
    msg_to_player_with_game_content(map, player, msg_to_player).await;
}

pub struct Player {
    pub player_name: String,
    pub to_player_tx: Sender<String>,
    pub pos: Pos,
    pub harvest: HashMap<Harvest, u32>,
    pub seeds: HashMap<Seed, u32>,
    pub score: u32,
}

impl Player {
    fn new(
        player_name: String,
        to_player_tx: Sender<String>,
        map_size: i32,
        player_positions: Vec<Pos>,
        rng: &mut SmallRng,
    ) -> Self {
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

async fn msg_to_player_with_game_content(map: &Map, player: &mut Player, result: MsgToPlayer) {
    let msg = MsgToPlayerWithGameContent {
        result,
        cell: map.get_cell(&player.pos).to_owned(),
        harvest: player.harvest.clone(),
        seed: player.seeds.clone(),
        score: player.score,
    };
    send_msg_to_player(&mut player.to_player_tx, msg).await;
}
