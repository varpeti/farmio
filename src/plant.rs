use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Plant {
    None,
    Wheat(Wheat),
    Bush(Bush),
    Tree(Tree),
    Cane(Cane),
    Pumpkin(Pumpkin),
    Cactus(Cactus),
    Wallbush(Wallbush),
    Swapshroom(Swapshroom),
    Sunflower(Sunflower),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wheat {
    pub growth: u8,
}

impl Wheat {
    pub const GROWTH_TO_GRAINS: u8 = 8;
    pub const GRAINS_YIELD: u8 = 1;
    pub const POINTS_PER_GRAINS: u8 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bush {
    pub growth: u8,
    pub berries: u8,
}

impl Bush {
    pub const GROWTH_TO_WOOD: u8 = 10;
    pub const WOOD_YIELD: u32 = 1;
    pub const POINTS_PER_WOOD: u32 = 1;

    pub const GROWTH_PER_BERRIES: u8 = 4;
    pub const MAX_BERRIES: u8 = 4;
    pub const POINTS_PER_BERRIES: u32 = 2;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub growth: u8,
}

impl Tree {
    pub const GROWTH_TO_WOOD: u8 = 16;
    pub const WOOD_YIELD: u32 = 16;
    pub const POINTS_PER_WOOD: u32 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cane {
    pub growth: u8,
}

impl Cane {
    pub const GROWTH_TO_SUGAR: u8 = 10;
    pub const SUGAR_YIELD: u32 = 3;
    pub const POINTS_PER_SUGAR: u32 = 2;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pumpkin {
    pub growth: u8,
    pub current_size: u8,
    pub max_size: u8,
}

impl Pumpkin {
    pub const GROWTH_TO_PUMPKINSEED: u8 = 4;
    pub const POINTS_PER_PUMPKINSEED: u32 = 5;

    pub fn pumpkinseed_yield(&self) -> u32 {
        (self.current_size * self.current_size) as u32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cactus {
    pub growth: u8,
    pub size: u8,
}

impl Cactus {
    pub const GROWTH_PER_CACTUSMEAT: u8 = 6;
    pub const MAX_CACTUSMEAT: u8 = 3;
    pub const POINTS_PER_CACTUSMEAT: u32 = 10;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallbush {
    pub growth: u8,
    pub health: u8,
}

impl Wallbush {
    pub const GROWTH_TO_BE_READY: u8 = 8;
    pub const MAX_HEALTH: u8 = 42;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swapshroom {
    pub growth: u8,
    pub pair_id: u32,
    pub active: bool,
}

impl Swapshroom {
    pub const GROWTH_TO_BE_READY: u8 = 8;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sunflower {
    pub growth: u8,
    pub rank: u8,
}

impl Sunflower {
    pub const GROWTH_TO_POWER: u8 = 30;
    pub const POWER_YIELD: u8 = 1;
    pub const POINTS_PER_POWER: u32 = 1024;
}
