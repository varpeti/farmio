use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Seed {
    Wheat,
    Bush,
    Tree,
    Cane,
    Pumpkin,
    Cactus,
    Wallbush,
    Swapshroom,
    Sunflower,
}

impl Seed {
    pub const TRADE_GRAINS_FOR_BUSH: u32 = 4;

    pub const TRADE_WOOD_FOR_TREE: u32 = 4;

    pub const TRADE_GRAINS_FOR_CANE: u32 = 2;

    pub const TRADE_WOOD_FOR_PUMPKIN: u32 = 16;
    pub const TRADE_BERRIES_FOR_PUMPKIN: u32 = 8;

    pub const TRADE_WOOD_FOR_CACTUS: u32 = 16;
    pub const TRADE_SUGAR_FOR_CACTUS: u32 = 9;

    pub const TRADE_PUMKINSEED_FOR_WALLBUSH: u32 = 10;

    pub const TRADE_CACTUSMEAT_FOR_SWAPSHROOM: u32 = 9;

    pub const TRADE_PUMKINSEED_FOR_SUNFLOWER: u32 = 50;
    pub const TRADE_CACTUSMEAT_FOR_SUNFLOWER: u32 = 27;
}
