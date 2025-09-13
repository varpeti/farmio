use std::collections::HashMap;

use rand::{rngs::SmallRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct GameConsts;

impl GameConsts {
    // (G)rowth, (V)olume, (P)oints
    pub const G_WHEAT_GRAINS: u8 = 8;
    pub const V_WHEAT_GRAINS: u32 = 1;
    pub const P_WHEAT_GRAINS: u32 = 1; // (1*1)/8 = 0.125

    pub const G_BUSH_WOOD: u8 = 10;
    pub const V_BUSH_WOOD: u32 = 1;
    pub const P_BUSH_WOOD: u32 = 1; // (1*1)/10 = 0.100

    pub const G_BUSH_BERRIES: u8 = 3;
    pub const P_BUSH_BERRIES: u32 = 2; // (2*1)/3 = 0.667
    pub const MAX_BUSH_BERRIES: u8 = 4;

    pub const G_TREE_WOOD: u8 = 16;
    pub const V_TREE_WOOD: u32 = 16;
    pub const P_TREE_WOOD: u32 = 1; // (1*16)/10 = 1.000

    pub const G_CANE_SUGAR: u8 = 9;
    pub const V_CANE_SUGAR: u32 = 3;
    pub const P_CANE_SUGAR: u32 = 2; // (2*3)/9 = 0.667

    pub const G_PUMPKIN_PUMPKINSEEDS: u8 = 4;
    pub fn v_pumpkin_pumpkinseeds(x: u32) -> u32 {
        x * x * x
    }
    pub const P_PUMPKIN_PUMPKINSEEDS: u32 = 3;
    // (3*(1*1*1))/(8+(1*1-1)*2) = 0.375
    // (3*(2*2*2))/(8+(2*2-1)*2) = 1.714
    // (3*(3*3*3))/(8+(3*3-1)*2) = 3.375
    // (3*(4*4*4))/(8+(4*4-1)*2) = 5.053
    // (3*(5*5*5))/(8+(5*5-1)*2) = 6.696
    // (3*(6*6*6))/(8+(6*6-1)*2) = 8.308
    // (3*(7*7*7))/(8+(7*7-1)*2) = 9.894
    // (3*(8*8*8))/(8+(8*8-1)*2) = 11.463

    pub const G_CACTUS_CACTUSMEAT: u8 = 5;
    pub const P_CACTUS_CACTUSMEAT: u32 = 10; // (10*1)/5 = 2.000
    pub const MAX_CACTUS_CACTUSMEAT: u8 = 3;

    pub const G_WALLBUSH: u8 = 7;

    pub const G_SUNFLOWER_POWER: u8 = 4;
    pub const V_SUNFLOWER_POWER: u8 = 1;
    pub const P_SUNFLOWER_POWER: u32 = 1024; // (1024*1)/4 = 256 (solo play)

    // Ground Type Percentages
    const GTP_TILLED_BUSH: u8 = 5;
    const GTP_SAND_EMPTY: u8 = 20;
    const GTP_SAND_CANE: u8 = 5;
    const GTP_WATER: u8 = 10;
    // const GTP_STONE // Game has Stone where Players spawn
    // const P_DIRT_WHEAT // Rest is Dirt with Wheat
}

pub struct Map {
    map: Vec<Vec<Cell>>,
}

impl Map {
    pub fn generate_map(map_size: usize, rng: &mut SmallRng) -> Map {
        let a = map_size * map_size;
        let tilled_bush: usize = (a * GameConsts::GTP_TILLED_BUSH as usize) / 100;
        let sand_empty: usize = (a * GameConsts::GTP_SAND_EMPTY as usize) / 100 + tilled_bush;
        let sand_cane: usize = (a * GameConsts::GTP_TILLED_BUSH as usize) / 100 + sand_empty;
        let water: usize = (a * GameConsts::GTP_WATER as usize) / 100 + sand_cane;
        let mut flat_map = Vec::with_capacity(a);
        for i in 0..a {
            let cell = if tilled_bush > i {
                Cell {
                    ground: Ground::Tiled,
                    plant: Plant::Bush {
                        growth: GameConsts::G_BUSH_BERRIES,
                        berries: GameConsts::MAX_BUSH_BERRIES,
                    },
                }
            } else if sand_empty > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::None,
                }
            } else if sand_cane > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::Cane {
                        growth: GameConsts::G_CANE_SUGAR,
                    },
                }
            } else if water > i {
                Cell {
                    ground: Ground::Water,
                    plant: Plant::None,
                }
            } else {
                Cell {
                    ground: Ground::Dirt,
                    plant: Plant::Wheat {
                        growth: GameConsts::G_WHEAT_GRAINS,
                    },
                }
            };
            flat_map.push(cell);
        }
        // Shuffle
        flat_map.shuffle(rng);

        // Save as n*n map
        let mut map = Vec::with_capacity(map_size);
        let mut i = 0;
        for _y in 0..map_size {
            let mut line = Vec::with_capacity(map_size);
            for _x in 0..map_size {
                line.push(flat_map[i].clone());
                i += 1;
            }
            map.push(line);
        }

        Self { map }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn get_cell(&self, pos: &Pos) -> &Cell {
        if let Some(line) = self.map.get(pos.y as usize) {
            if let Some(cell) = line.get(pos.x as usize) {
                return cell;
            }
        }
        eprintln!("(get_cell) Invalid Position: `{:?}`", pos);
        &self.map[0][0]
    }

    pub fn set_cell(&mut self, pos: &Pos, cell: Cell) {
        if let Some(line) = self.map.get_mut(pos.y as usize) {
            if let Some(old_cell) = line.get_mut(pos.x as usize) {
                *old_cell = cell;
                return;
            }
        }
        eprintln!("(set_cell) Invalid Position: `{:?}`", pos);
    }

    pub fn update_map(&mut self) {
        for line in self.map.iter_mut() {
            for cell in line.iter_mut() {
                // TODO: Water
                match &mut cell.plant {
                    Plant::None => {
                        if let Ground::Dirt = cell.ground {
                            cell.plant = Plant::Wheat { growth: 0 };
                        }
                    }
                    Plant::Wheat { growth } => {
                        if *growth < GameConsts::G_WHEAT_GRAINS {
                            *growth += 1;
                        }
                    }
                    Plant::Bush { growth, berries } => {
                        if *growth < GameConsts::G_BUSH_WOOD {
                            *growth += 1;
                        } else if *growth
                            < GameConsts::G_BUSH_WOOD
                                + GameConsts::MAX_BUSH_BERRIES * GameConsts::G_BUSH_BERRIES
                        {
                            *growth += 1;
                            if (*growth - GameConsts::G_BUSH_WOOD) % GameConsts::G_BUSH_BERRIES == 0
                            {
                                *berries += 1;
                            }
                        }
                    }
                    Plant::Tree { growth } => {
                        // TODO: Stop growth if a neighbour is Tree
                        if *growth < GameConsts::G_TREE_WOOD {
                            *growth += 1;
                        }
                    }
                    Plant::Cane { growth } => {
                        if *growth < GameConsts::G_CANE_SUGAR {
                            *growth += 1;
                        }
                    }
                    Plant::Pumpkin { growth, size } => {
                        if *growth < GameConsts::G_PUMPKIN_PUMPKINSEEDS {
                            *growth += 1;
                        }
                        // TODO: Check square pumpkin formulation, to update size
                    }
                    Plant::Cactus { growth, size } => {
                        if *growth
                            < GameConsts::G_CACTUS_CACTUSMEAT * GameConsts::MAX_CACTUS_CACTUSMEAT
                        {
                            *growth += 1;
                            if *growth % GameConsts::G_CACTUS_CACTUSMEAT == 0 {
                                *size += 1;
                            }
                        }
                    }
                    Plant::Wallbush { growth, health: _ } => {
                        if *growth < GameConsts::G_WALLBUSH {
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
                        if *growth < GameConsts::G_SUNFLOWER_POWER {
                            *growth += 1;
                        }
                    }
                }
            }
        }
    }
    pub fn print_map_with_players(&self, players: &HashMap<Pos, String>) {
        for (y, line) in self.map.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                match players.get(&Pos {
                    x: x as i32,
                    y: y as i32,
                }) {
                    Some(player_name) => {
                        print!("{}", player_name.chars().take(2).collect::<String>())
                    }
                    None => print!("{}", cell.to_ansi()),
                }
            }
            println!();
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

#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub ground: Ground,
    pub plant: Plant,
}

impl Cell {
    pub fn to_ansi(&self) -> String {
        let ground = match self.ground {
            Ground::Dirt => "94",
            Ground::Tiled => "40",
            Ground::Sand => "228",
            Ground::Water => "27",
            Ground::Stone => "253",
            Ground::Error => "200",
        };
        let plant = match self.plant {
            Plant::None => ('0', '⠀'),
            Plant::Wheat { growth } => ('W', growth_to_barlie(growth, GameConsts::G_WHEAT_GRAINS)),
            Plant::Bush { growth, berries } => {
                if berries > 0 {
                    ('B', growth_to_barlie(berries, GameConsts::MAX_BUSH_BERRIES))
                } else {
                    ('b', growth_to_barlie(growth, GameConsts::G_BUSH_WOOD))
                }
            }
            Plant::Tree { growth } => ('T', growth_to_barlie(growth, GameConsts::G_TREE_WOOD)),
            Plant::Cane { growth } => ('C', growth_to_barlie(growth, GameConsts::G_CANE_SUGAR)),
            Plant::Pumpkin { growth, size: _ } => (
                'P',
                growth_to_barlie(growth, GameConsts::G_PUMPKIN_PUMPKINSEEDS),
            ),
            Plant::Cactus { growth, size: _ } => (
                'U',
                growth_to_barlie(
                    growth,
                    GameConsts::MAX_CACTUS_CACTUSMEAT * GameConsts::G_CACTUS_CACTUSMEAT,
                ),
            ),
            Plant::Wallbush { growth, health: _ } => {
                ('-', growth_to_barlie(growth, GameConsts::G_WALLBUSH))
            }
            Plant::Swapshroom { pair_id: _, active } => {
                if active {
                    ('M', '⣿')
                } else {
                    ('m', '⠀')
                }
            }
            Plant::Sunflower { growth, rank: _ } => {
                ('S', growth_to_barlie(growth, GameConsts::G_SUNFLOWER_POWER))
            }
        };
        format!("\x1b[38;5;{}m{}{}\x1b[0m", ground, plant.0, plant.1)
    }
}

fn growth_to_barlie(growth: u8, max_growth: u8) -> char {
    let n = (growth * 8) / max_growth;
    match n {
        0 => '⠀',
        1 => '⡀',
        2 => '⣀',
        3 => '⣄',
        4 => '⣤',
        5 => '⣦',
        6 => '⣶',
        7 => '⣷',
        _ => '⣿',
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ground {
    Dirt,
    Tiled,
    Sand,
    Water,
    Stone,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Plant {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn get_next_pos_on_map(&self, direction: Option<Direction>, map_size: i32) -> Self {
        match direction {
            Some(direction) => {
                // The map is Wrapping around, it's a Torus 🍩
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
