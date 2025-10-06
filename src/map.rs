use bevy::{asset::uuid::Uuid, platform::collections::HashMap, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Component)]
pub struct RMap {
    forward: Vec<Option<Entity>>,
    backward: HashMap<Entity, usize>,
    size: usize,
}

// impl RMap {
//     pub fn new(size: usize) -> Self {
//         Self {
//             forward: Vec::with_capacity(size * size),
//             backward: HashMap::new(),
//             size,
//         }
//     }
//
//     pub fn get_entity(&self, pos: &CPos) -> Option<Entity> {
//         self.forward.get(pos.y + self.size * pos.x)
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct CPos {
    pub x: usize,
    pub y: usize,
}

#[derive(Component)]
pub struct MCell;

#[derive(Debug, Component)]
pub enum CGround {
    Dirt,
    Tiled,
    Sand,
    Water,
    Stone,
}

// #[derive(Debug, Component)]
// pub enum CPlant {
//     Wheat,
//     Bush,
//     Tree,
//     Cane,
//     Pumpkin,
//     Cactus,
//     Wallbush,
//     Swapshroom,
//     Sunflower,
// }
//
// #[derive(Debug, Component)]
// pub enum CHarvest {
//     Grains,
//     Berries,
//     Wood,
//     Sugar,
//     PumpkinSeed,
//     CactusMeat,
//     Power,
// }
//
// #[derive(Debug, Component)]
// pub struct CGrowth {
//     current_growth: u8,
//     growth_rate: u8,
// }
//
// #[derive(Debug, Component)]
// pub struct CYield {
//     required_growth: u8,
//
// }
