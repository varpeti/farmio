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
    cell::Cell,
    direction::Direction,
    drawer::Drawer,
    ground::Ground,
    handle_connection::PlayerAction,
    harvest::Harvest,
    map::Map,
    plant::{Bush, Cactus, Cane, Plant, Pumpkin, Sunflower, Swapshroom, Tree, Wallbush, Wheat},
    player::Player,
    pos::Pos,
    seed::Seed,
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
    // Admin //
    Connected {
        game_settings: GameSettings,
        players_connected: u32,
    },
    AlreadyConnected,
    Reconnected,
    WaitingOtherPlayersToJoin,
    GameIsFull,
    GameStarted,
    // Idle //
    Idled,
    // Move //
    Moved,
    BlockedBy(BlockedBy),
    // Harvest //
    Harvested {
        harvest: Harvest,
        volume: u32,
    },
    NoHarvest,
    // Plant //
    Planted,
    NotEnoughSeed,
    WrongGroundType,
    CannotPlantOver,
    // Trade //
    Traded,
    NotEnoughHarvest,
    InvalidTrade,
    // Till //
    Tilled,
    //WrongGroundType,
    // Forced Move //
    Swapped, // When a palyer receive it they should read again the TCP buffer,
             // because it was sent in the previous round as an extra message,
             // (in case of single thread player)
}

#[derive(Debug, Serialize)]
pub enum BlockedBy {
    AnotherPlayer,
    WallBush,
    Swapshroom,
}

#[derive(Debug, Serialize)]
pub struct MsgToPlayerWithGameContent {
    result: MsgToPlayer,
    cell: Cell,
    harvests: HashMap<Harvest, u32>,
    seeds: HashMap<Seed, u32>,
    points: u32,
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
    drawer: Drawer,
    active_swapshrooms: HashMap<u32, (Pos, Pos)>,
}

impl Game {
    pub async fn new(
        game_name: String,
        to_game_rx: Receiver<PlayerAction>,
        game_settings: GameSettings,
    ) -> Self {
        // TODO: Check if all players could fit in the map
        let mut rng = rand::rngs::SmallRng::seed_from_u64(game_settings.seed);
        let players = HashMap::new();
        let mut drawer = Drawer::new(game_name.clone()).await;
        let map = Map::generate_map(
            game_settings.map_size as usize,
            &mut rng,
            game_settings.number_of_players,
        );
        map.print_map_with_players(&mut drawer, &HashMap::new())
            .await;
        let swapshrooms = HashMap::new();
        Self {
            game_name,
            to_game_rx,
            game_settings,
            turns: 0,
            rng,
            players,
            map,
            drawer,
            active_swapshrooms: swapshrooms,
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
                        let stones = self.map.get_stones();
                        let mut free_spots = stones.difference(&player_positions);

                        match free_spots.next() {
                            Some(pos) => {
                                let player = vacant_entry.insert(Player::new(
                                    player_name,
                                    to_player_tx,
                                    pos.to_owned(),
                                ));
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
                            None => {
                                println!(
                                    "{} No free spots left in the map for Player `{}`",
                                    p, player_name
                                );
                                send_msg_to_player(&mut to_player_tx, MsgToPlayer::GameIsFull)
                                    .await;
                            }
                        }
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
            let player_actions = self.collect_player_actions(turn_duration).await;
            self.process_player_actions(player_actions).await;
            self.map.update_map(&mut self.active_swapshrooms);
            self.map
                .print_map_with_players(
                    &mut self.drawer,
                    &self
                        .players
                        .values()
                        .map(|p| (p.pos.clone(), p.player_name.clone()))
                        .collect(),
                )
                .await;
            self.turns += 1;
        }
        // TODO: End the game if a player reaches a certain score
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
        let mut swap_players = Vec::<(Pos, Pos)>::new();

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
                Action::Harvest => {
                    action_harvest(
                        &mut self.map,
                        player,
                        &mut self.active_swapshrooms,
                        &mut swap_players,
                    )
                    .await
                }
                Action::Plant { seed } => {
                    action_plant(&mut self.map, player, seed, &mut self.rng).await
                }
                Action::Trade { seed, volume } => {
                    action_trade(&mut self.map, player, seed, volume).await
                }
                Action::Till => action_till(&mut self.map, player).await,
                Action::__Connect__ {
                    player_name: _,
                    to_player_tx: _,
                } => unreachable!(),
            }
        }
        action_move_execution(
            &mut self.map,
            &mut self.players,
            next_positions,
            moving_players,
            swap_players,
            &self.active_swapshrooms,
        )
        .await;
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
    map: &mut Map,
    players: &mut HashMap<Uuid, Player>,
    mut next_positions: HashMap<Pos, Vec<Uuid>>,
    moving_players: Vec<Uuid>,
    swap_players: Vec<(Pos, Pos)>,
    active_swapshrooms: &HashMap<u32, (Pos, Pos)>,
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

    let wallbushes = map.get_wallbushes();
    let mut active_swapshroom_positions = HashSet::<Pos>::new();
    for (_, (p1, p2)) in active_swapshrooms.iter() {
        active_swapshroom_positions.insert(p1.to_owned());
        active_swapshroom_positions.insert(p2.to_owned());
    }

    for (pos, uuids) in next_positions {
        // Wants to move to a WallBush
        if wallbushes.contains(&pos) {
            for uuid in uuids.iter() {
                let player = players.get_mut(uuid).unwrap();
                msg_to_player_with_game_content(
                    map,
                    player,
                    MsgToPlayer::BlockedBy(BlockedBy::WallBush),
                )
                .await;
            }
            let mut cell = map.get_cell(&pos).to_owned();
            if let Plant::Wallbush(wallbush) = &mut cell.plant {
                wallbush.health = wallbush.health.saturating_sub(1);
                if wallbush.health == 0 {
                    cell.plant = Plant::None
                }
            }
            map.set_cell(&pos, cell);
            continue;
        }

        if uuids.len() == 1 {
            // No player wants to occupie the same position

            let player = players.get_mut(&uuids[0]).unwrap();
            // If player standing still, do not send notification
            if player.pos == pos {
                continue;
            }
            // Wants to move from an active Swapshrooms
            if active_swapshroom_positions.contains(&player.pos) {
                msg_to_player_with_game_content(
                    map,
                    player,
                    MsgToPlayer::BlockedBy(BlockedBy::Swapshroom),
                )
                .await;
                continue;
            }
            // Can move
            player.pos = pos;
            msg_to_player_with_game_content(map, player, MsgToPlayer::Moved).await;
        } else {
            // AnotherPlayer occupies or tried to occupie the same spot
            for uuid in uuids {
                let player = players.get_mut(&uuid).unwrap();
                // If player standing still, do not send notification
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

    // Swap occures after movements
    for (_, player) in players.iter_mut() {
        for (p1, p2) in swap_players.iter() {
            if player.pos == *p1 {
                player.pos = p2.clone();
                send_msg_to_player(&mut player.to_player_tx, MsgToPlayer::Swapped).await;
            } else if player.pos == *p2 {
                player.pos = p1.clone();
                send_msg_to_player(&mut player.to_player_tx, MsgToPlayer::Swapped).await;
            }
        }
    }
}

async fn action_harvest(
    map: &mut Map,
    player: &mut Player,
    active_swapshrooms: &mut HashMap<u32, (Pos, Pos)>,
    swap_players: &mut Vec<(Pos, Pos)>,
) {
    let mut cell = map.get_cell(&player.pos).clone();
    let msg_to_player = match cell.plant.clone() {
        Plant::None => MsgToPlayer::NoHarvest,
        Plant::Wheat(wheat) => {
            if wheat.growth == Wheat::GROWTH_TO_GRAINS {
                cell.plant = Plant::None;
                player.harvest(
                    Harvest::Grains,
                    Wheat::GRAINS_YIELD as u32,
                    Wheat::POINTS_PER_GRAINS as u32,
                )
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Bush(bush) => {
            if bush.berries > 0 {
                cell.plant = Plant::Bush(Bush {
                    growth: Bush::GROWTH_TO_WOOD,
                    berries: 0,
                });
                player.harvest(
                    Harvest::Berry,
                    bush.berries as u32,
                    Bush::POINTS_PER_BERRIES,
                )
            } else if bush.growth >= Bush::GROWTH_TO_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, Bush::WOOD_YIELD, Bush::POINTS_PER_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Tree(tree) => {
            if tree.growth == Tree::GROWTH_TO_WOOD {
                cell.plant = Plant::None;
                player.harvest(Harvest::Wood, Tree::WOOD_YIELD, Tree::POINTS_PER_WOOD)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Cane(cane) => {
            if cane.growth == Cane::GROWTH_TO_SUGAR {
                cell.plant = Plant::None;
                player.harvest(Harvest::Sugar, Cane::SUGAR_YIELD, Cane::POINTS_PER_SUGAR)
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Pumpkin(pumpkin) => {
            if pumpkin.growth >= Pumpkin::GROWTH_TO_PUMPKINSEED {
                cell.plant = Plant::None;
                player.harvest(
                    Harvest::PumpkinSeed,
                    pumpkin.pumpkinseed_yield(),
                    Pumpkin::POINTS_PER_PUMPKINSEED,
                )
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Cactus(cactus) => {
            if cactus.growth >= Cactus::GROWTH_PER_CACTUSMEAT {
                cell.plant = Plant::None;
                player.harvest(
                    Harvest::CactusMeat,
                    cactus.size as u32,
                    Cactus::POINTS_PER_CACTUSMEAT,
                )
            } else {
                cell.plant = Plant::None;
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Wallbush(_) => MsgToPlayer::NoHarvest,

        Plant::Swapshroom(swapshroom) => {
            if swapshroom.active {
                match active_swapshrooms.entry(swapshroom.pair_id) {
                    Entry::Occupied(occupied_entry) => {
                        let (p1, p2) = occupied_entry.remove();
                        let mut c1 = map.get_cell(&p1).clone();
                        let mut c2 = map.get_cell(&p2).clone();
                        c1.plant = Plant::None;
                        c2.plant = Plant::None;
                        map.set_cell(&p1, c2);
                        map.set_cell(&p2, c1);
                        swap_players.push((p1, p2));
                        return; // Message sent by the swapping action
                    }
                    Entry::Vacant(_vacant_entry) => {
                        eprintln!("Active Swapshroom but not in active_swapshrooms?");
                        MsgToPlayer::NoHarvest
                    }
                }
            } else {
                // It remains
                MsgToPlayer::NoHarvest
            }
        }
        Plant::Sunflower(sunflower) => {
            if sunflower.growth == Sunflower::GROWTH_TO_POWER {
                let max_rank = map.get_highest_sunflower_rank();
                if sunflower.rank == max_rank {
                    cell.plant = Plant::None;
                    player.harvest(
                        Harvest::Power,
                        Sunflower::POWER_YIELD as u32,
                        Sunflower::POINTS_PER_POWER,
                    )
                } else {
                    cell.plant = Plant::None;
                    player.points = player.points.saturating_sub(
                        Sunflower::POINTS_PER_POWER * (Sunflower::POWER_YIELD as u32),
                    );
                    MsgToPlayer::NoHarvest
                }
            } else {
                // It remains
                MsgToPlayer::NoHarvest
            }
        }
    };

    map.set_cell(&player.pos, cell);
    msg_to_player_with_game_content(map, player, msg_to_player).await;
}

async fn action_plant(map: &mut Map, player: &mut Player, seed: Seed, rng: &mut SmallRng) {
    if let Some(volume) = player.seeds.get_mut(&seed) {
        if *volume == 0 {
            return msg_to_player_with_game_content(map, player, MsgToPlayer::NotEnoughSeed).await;
        }
        *volume -= 1;
        let mut cell = map.get_cell(&player.pos).to_owned();

        if let Plant::Wallbush(_) | Plant::Swapshroom(_) = cell.plant {
            return msg_to_player_with_game_content(map, player, MsgToPlayer::CannotPlantOver)
                .await;
        }

        let plant = match (seed, cell.clone().ground) {
            (Seed::Wheat, Ground::Dirt | Ground::Tiled) => Plant::Wheat(Wheat { growth: 0 }),
            (Seed::Bush, Ground::Tiled) => Plant::Bush(Bush {
                growth: 0,
                berries: 0,
            }),
            (Seed::Tree, Ground::Dirt) => Plant::Tree(Tree { growth: 0 }),
            (Seed::Cane, Ground::Sand) => Plant::Cane(Cane { growth: 0 }),
            (Seed::Pumpkin, Ground::Tiled) => Plant::Pumpkin(Pumpkin {
                growth: 0,
                current_size: 1,
                max_size: 1,
            }),
            (Seed::Cactus, Ground::Sand) => Plant::Cactus(Cactus { growth: 0, size: 0 }),
            (Seed::Wallbush, Ground::Tiled) => Plant::Wallbush(Wallbush {
                growth: 0,
                health: Wallbush::MAX_HEALTH,
            }),
            (Seed::Swapshroom, _) => {
                let pair_id = match player.next_swapshroom_pair_id {
                    Some(pair_id) => {
                        player.next_swapshroom_pair_id = None;
                        pair_id
                    }
                    None => {
                        let pair_id = rng.random_range(u32::MIN..u32::MAX);
                        player.next_swapshroom_pair_id = Some(pair_id);
                        pair_id
                    }
                };
                Plant::Swapshroom(Swapshroom {
                    growth: 0,
                    pair_id,
                    active: false,
                })
            }
            (Seed::Sunflower, Ground::Stone) => {
                let stones = map.get_stones();
                for pos in stones {
                    let mut cell = map.get_cell(&pos).to_owned();

                    if let Plant::Swapshroom(_) = cell.plant {
                        // Swapshroom is immune to Sunflower spawning
                    } else {
                        // Overwriting the existing ones, maybe skip if already has Sunflower?
                        // By overwriteing a player gets "Notified" by inspecting the rank again
                        // By not overwriteing a player can cause another player to fail (if lucky)
                        cell.plant = Plant::Sunflower(Sunflower {
                            growth: 0,
                            rank: rng.random_range(u8::MIN..u8::MAX),
                        });
                        map.set_cell(&pos, cell);
                    }
                }
                return msg_to_player_with_game_content(map, player, MsgToPlayer::Planted).await;
            }
            (_seed, _ground) => {
                return msg_to_player_with_game_content(map, player, MsgToPlayer::WrongGroundType)
                    .await;
            }
        };
        cell.plant = plant;
        map.set_cell(&player.pos, cell);
        return msg_to_player_with_game_content(map, player, MsgToPlayer::Planted).await;
    }
    msg_to_player_with_game_content(map, player, MsgToPlayer::NotEnoughSeed).await;
}

async fn action_trade(map: &mut Map, player: &mut Player, seed: Seed, volume: u32) {
    if volume == 0 {
        return msg_to_player_with_game_content(map, player, MsgToPlayer::InvalidTrade).await;
    }

    let trade = match seed {
        Seed::Wheat => {
            return msg_to_player_with_game_content(map, player, MsgToPlayer::InvalidTrade).await
        }
        Seed::Bush => vec![(Harvest::Grains, Seed::TRADE_GRAINS_FOR_BUSH)],
        Seed::Tree => vec![(Harvest::Wood, Seed::TRADE_WOOD_FOR_TREE)],
        Seed::Cane => vec![(Harvest::Grains, Seed::TRADE_GRAINS_FOR_CANE)],
        Seed::Pumpkin => vec![
            (Harvest::Berry, Seed::TRADE_BERRIES_FOR_PUMPKIN),
            (Harvest::Wood, Seed::TRADE_WOOD_FOR_PUMPKIN),
        ],
        Seed::Cactus => vec![
            (Harvest::Sugar, Seed::TRADE_SUGAR_FOR_CACTUS),
            (Harvest::Wood, Seed::TRADE_WOOD_FOR_CACTUS),
        ],
        Seed::Wallbush => vec![(Harvest::PumpkinSeed, Seed::TRADE_PUMKINSEED_FOR_WALLBUSH)],
        Seed::Swapshroom => vec![(Harvest::CactusMeat, Seed::TRADE_CACTUSMEAT_FOR_SWAPSHROOM)],
        Seed::Sunflower => vec![
            (Harvest::PumpkinSeed, Seed::TRADE_PUMKINSEED_FOR_SUNFLOWER),
            (Harvest::CactusMeat, Seed::TRADE_CACTUSMEAT_FOR_SUNFLOWER),
        ],
    };
    action_trade_helper(map, player, volume, seed, trade).await;
}

async fn action_trade_helper(
    map: &mut Map,
    player: &mut Player,
    volume: u32,
    seed: Seed,
    trade: Vec<(Harvest, u32)>,
) {
    let mut ok = true;
    for (harvest, cost) in trade.iter() {
        match player.harvests.get(harvest) {
            Some(available_harvest_volume) => {
                if *available_harvest_volume < *cost * volume {
                    ok = false;
                }
            }
            None => ok = false,
        }
    }
    if ok {
        for (harvest, cost) in trade.iter() {
            if let Some(available_harvest_volume) = player.harvests.get_mut(harvest) {
                *available_harvest_volume -= *cost * volume;
            }
        }
        match player.seeds.entry(seed.clone()) {
            Entry::Occupied(occupied_entry) => {
                *occupied_entry.into_mut() += volume;
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(volume);
            }
        }
        msg_to_player_with_game_content(map, player, MsgToPlayer::Traded).await;
    } else {
        msg_to_player_with_game_content(map, player, MsgToPlayer::NotEnoughHarvest).await;
    }
}

async fn action_till(map: &mut Map, player: &mut Player) {
    let cell = map.get_cell(&player.pos).to_owned();
    match (cell.ground, cell.plant) {
        (Ground::Dirt, Plant::Swapshroom(swapshroom)) => map.set_cell(
            &player.pos,
            Cell {
                ground: Ground::Tiled,
                plant: Plant::Swapshroom(swapshroom),
            },
        ),
        (Ground::Tiled, Plant::Swapshroom(swapshroom)) => {
            map.set_cell(
                &player.pos,
                Cell {
                    ground: Ground::Dirt,
                    plant: Plant::Swapshroom(swapshroom),
                },
            );
        }
        (Ground::Dirt, _) => map.set_cell(
            &player.pos,
            Cell {
                ground: Ground::Tiled,
                plant: Plant::None,
            },
        ),
        (Ground::Tiled, _) => {
            map.set_cell(
                &player.pos,
                Cell {
                    ground: Ground::Dirt,
                    plant: Plant::None,
                },
            );
        }
        _ => {
            return msg_to_player_with_game_content(map, player, MsgToPlayer::WrongGroundType)
                .await;
        }
    }
    msg_to_player_with_game_content(map, player, MsgToPlayer::Tilled).await;
}

async fn msg_to_player_with_game_content(map: &Map, player: &mut Player, result: MsgToPlayer) {
    let msg = MsgToPlayerWithGameContent {
        result,
        cell: map.get_cell(&player.pos).to_owned(),
        harvests: player.harvests.clone(),
        seeds: player.seeds.clone(),
        points: player.points,
    };
    send_msg_to_player(&mut player.to_player_tx, msg).await;
}
