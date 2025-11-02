use std::collections::{hash_map::Entry, HashMap};

use tokio::sync::mpsc::Sender;

use crate::{game::MsgToPlayer, harvest::Harvest, pos::Pos, seed::Seed};

pub struct Player {
    pub player_name: String,
    pub to_player_tx: Sender<String>,
    pub pos: Pos,
    pub harvests: HashMap<Harvest, u32>,
    pub seeds: HashMap<Seed, u32>,
    pub points: u32,
    pub next_swapshroom_pair_id: Option<u32>,
    pub connected: bool,
}

impl Player {
    pub fn new(player_name: String, to_player_tx: Sender<String>, pos: Pos) -> Self {
        Self {
            player_name,
            to_player_tx,
            pos,
            harvests: HashMap::from([
                (Harvest::Grains, 999),
                (Harvest::Berry, 999),
                (Harvest::Wood, 999),
                (Harvest::Sugar, 999),
                (Harvest::PumpkinSeed, 999),
                (Harvest::CactusMeat, 999),
                (Harvest::Power, 999),
            ]),
            seeds: HashMap::new(),
            points: 0,
            next_swapshroom_pair_id: None,
            connected: true,
        }
    }

    pub fn harvest(&mut self, harvest: Harvest, volume: u32, points: u32) -> MsgToPlayer {
        match self.harvests.entry(harvest.clone()) {
            Entry::Occupied(occupied_entry) => {
                *occupied_entry.into_mut() += volume;
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(volume);
            }
        }
        self.points += points * volume;
        MsgToPlayer::Harvested { harvest, volume }
    }
}
