use std::{cmp::Ordering, collections::HashMap, fmt::format};

use rand::{rngs::SmallRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dawing::Drawer;

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

    pub const G_PUMPKIN_PUMPKINSEED: u8 = 4;
    pub const P_PUMPKIN_PUMPKINSEED: u32 = 5;
    // V_PUMPKIN_PUMPKINSEED = size*size where size = 1..5
    // (1*1*5)/(1*4) = 1.250
    // (2*2*5)/(2*4) = 2.500
    // (3*3*5)/(3*4) = 3.750
    // (4*4*5)/(4*4) = 5.000
    // (5*5*5)/(5*4) = 6.250

    pub const G_CACTUS_CACTUSMEAT: u8 = 5;
    pub const P_CACTUS_CACTUSMEAT: u32 = 10; // (10*1)/5 = 2.000
    pub const MAX_CACTUS_CACTUSMEAT: u8 = 3;

    pub const G_WALLBUSH: u8 = 7;
    pub const MAX_WALLBUSH_HEALTH: u8 = 42;

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

    // Trade Costs
    pub const T_GRAINS_BUSH: u32 = 4;
    pub const T_GRAINS_CANE: u32 = 2;

    pub const T_WOOD_TREE: u32 = 4;

    pub const T_WOOD_PUMPKIN: u32 = 16;
    pub const T_BERRIES_PUMPKIN: u32 = 8;

    pub const T_WOOD_CACTUS: u32 = 16;
    pub const T_SUGAR_CACTUS: u32 = 9;

    pub const T_PUMKINSEED_WALLBUSH: u32 = 10;
    pub const T_CACTUSMEAT_SWAPSHROOM: u32 = 9;

    pub const T_PUMKINSEED_SUNFLOWER: u32 = 50;
    pub const T_CACTUSMEAT_SUNFLOWER: u32 = 27;
}

#[derive(Clone)]
pub struct Map {
    map: Vec<Vec<Cell>>,
}

impl Map {
    pub fn generate_map(map_size: usize, rng: &mut SmallRng) -> Map {
        let a = map_size * map_size;
        let tilled_bush: usize = (a * GameConsts::GTP_TILLED_BUSH as usize) / 100;
        let sand_empty: usize = (a * GameConsts::GTP_SAND_EMPTY as usize) / 100 + tilled_bush;
        let sand_cane: usize = (a * GameConsts::GTP_SAND_CANE as usize) / 100 + sand_empty;
        let water: usize = (a * GameConsts::GTP_WATER as usize) / 100 + sand_cane;
        let pumpkin: usize = 1 + water;
        let cactus: usize = 1 + pumpkin;

        let mut flat_map = Vec::with_capacity(a);
        for i in 0..a {
            let cell = if tilled_bush > i {
                Cell {
                    ground: Ground::Tiled,
                    plant: Plant::Bush {
                        growth: GameConsts::G_BUSH_WOOD
                            + GameConsts::G_BUSH_BERRIES * GameConsts::MAX_BUSH_BERRIES,
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
            } else if pumpkin > i {
                Cell {
                    ground: Ground::Tiled,
                    plant: Plant::Pumpkin {
                        growth: GameConsts::G_PUMPKIN_PUMPKINSEED,
                        curent_size: 1,
                        max_size: 1,
                    },
                }
            } else if cactus > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::Cactus {
                        growth: GameConsts::G_CACTUS_CACTUSMEAT * GameConsts::MAX_CACTUS_CACTUSMEAT,
                        size: GameConsts::MAX_CACTUS_CACTUSMEAT,
                    },
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

    pub fn get_stones(&self) -> Vec<Pos> {
        let mut stones = Vec::new();
        for (y, line) in self.map.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                if let Ground::Stone = cell.ground {
                    stones.push(Pos {
                        x: x as i32,
                        y: y as i32,
                    });
                }
            }
        }
        stones
    }

    pub fn get_neighbours(&self, pos: &Pos) -> Vec<Cell> {
        let mut neighbours = Vec::with_capacity(4);
        for direction in [
            Direction::Up,
            Direction::Right,
            Direction::Down,
            Direction::Left,
        ] {
            let pos = pos.get_next_pos_on_map(Some(direction), self.map.len() as i32);
            neighbours.push(self.get_cell(&pos).to_owned());
        }
        neighbours
    }

    pub fn update_map(&mut self) {
        let map_clone = self.clone();
        for (y, line) in self.map.iter_mut().enumerate() {
            for (x, cell) in line.iter_mut().enumerate() {
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
                            *berries +=
                                (*growth - GameConsts::G_BUSH_WOOD) / GameConsts::G_BUSH_BERRIES;
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
                    Plant::Pumpkin {
                        growth,
                        curent_size,
                        max_size,
                    } => {
                        let mut next_max_size = 1;
                        for n_cell in map_clone.get_neighbours(&Pos {
                            x: x as i32,
                            y: y as i32,
                        }) {
                            if let Plant::Pumpkin {
                                growth,
                                curent_size: _,
                                max_size: _,
                            } = n_cell.plant
                            {
                                if growth >= GameConsts::G_PUMPKIN_PUMPKINSEED {
                                    next_max_size += 1;
                                }
                            }
                        }
                        *max_size = next_max_size;

                        match (GameConsts::G_PUMPKIN_PUMPKINSEED * next_max_size).cmp(growth) {
                            Ordering::Less => {
                                *growth -= 1;
                                *curent_size = *growth / GameConsts::G_PUMPKIN_PUMPKINSEED;
                            }
                            Ordering::Equal => (),
                            Ordering::Greater => {
                                *growth += 1;
                                *curent_size = *growth / GameConsts::G_PUMPKIN_PUMPKINSEED;
                            }
                        }
                    }
                    Plant::Cactus { growth, size } => {
                        if *growth
                            < GameConsts::G_CACTUS_CACTUSMEAT * GameConsts::MAX_CACTUS_CACTUSMEAT
                        {
                            *growth += 1;
                            *size = *growth / GameConsts::G_CACTUS_CACTUSMEAT;
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

    pub async fn print_map_with_players(
        &self,
        drawer: &mut Drawer,
        players: &HashMap<Pos, String>,
    ) {
        let map_size = self.map.len();
        let mut map = vec![vec![" ".to_string(); map_size * 4]; map_size * 2];
        for (y, line) in self.map.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                let c = cell.to_ansi();
                if let Some(player_name) = players.get(&Pos {
                    x: x as i32,
                    y: y as i32,
                }) {
                    for i in 0..4 {
                        map[y * 2][x * 4 + i] = c[i].clone();
                    }
                    for (i, c) in player_name.chars().take(3).enumerate() {
                        map[y * 2 + 1][x * 4 + i] = c.to_string();
                    }
                } else {
                    for i in 0..4 {
                        map[y * 2][x * 4 + i] = c[i].clone();
                    }
                    for i in 0..4 {
                        map[y * 2 + 1][x * 4 + i] = c[i + 4].clone();
                    }
                }
            }
        }
        drawer.clear().await;
        for line in map {
            for cell in line {
                drawer.write(cell).await;
            }
            drawer.write("\n".to_string()).await;
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
    pub fn to_ansi(&self) -> [String; 8] {
        let background = match self.ground {
            Ground::Dirt => 94,
            Ground::Tiled => 22,
            Ground::Sand => 142,
            Ground::Water => 62,
            Ground::Stone => 249,
            Ground::Error => 13,
        };

        let (foreground, subcells) = match self.plant {
            Plant::None => (0, [' '; 8]),
            Plant::Wheat { growth } => {
                let g = to_chars3(growth);
                let m = to_chars3(GameConsts::G_WHEAT_GRAINS);
                (184, ['W', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Bush { growth, berries } => {
                let g = to_chars3(growth);
                let b = to_chars3(berries);
                (76, ['B', g[0], g[1], g[2], '°', b[0], b[1], b[2]])
            }
            Plant::Tree { growth } => {
                let g = to_chars3(growth);
                let m = to_chars3(GameConsts::G_TREE_WOOD);
                (70, ['T', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Cane { growth } => {
                let g = to_chars3(growth);
                let m = to_chars3(GameConsts::G_CANE_SUGAR);
                (0, ['C', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Pumpkin {
                growth,
                curent_size,
                max_size,
            } => {
                let g = to_chars3(growth);
                (
                    172,
                    [
                        'P',
                        g[0],
                        g[1],
                        g[2],
                        '+',
                        to_char(curent_size),
                        '/',
                        to_char(max_size),
                    ],
                )
            }
            Plant::Cactus { growth, size } => {
                let g = to_chars3(growth);
                (
                    22,
                    [
                        'I',
                        g[0],
                        g[1],
                        g[2],
                        '+',
                        to_char(size),
                        '/',
                        to_char(GameConsts::MAX_CACTUS_CACTUSMEAT),
                    ],
                )
            }

            Plant::Wallbush { growth, health } => {
                let g = to_chars3(growth);
                let h = to_chars3(health);
                (22, ['#', g[0], g[1], g[2], '#', h[0], h[1], h[2]])
            }
            Plant::Swapshroom { pair_id, active } => {
                let c = pair_id.to_string().chars().take(4).collect::<Vec<char>>();
                if active {
                    (53, ['*', c[0], c[1], c[2], c[3], c[4], c[5], c[6]])
                } else {
                    (53, ['o', c[0], c[1], c[2], c[3], c[4], c[5], c[6]])
                }
            }
            Plant::Sunflower { growth, rank } => {
                let g = to_chars3(growth);
                let r = to_chars3(rank);
                (11, ['S', g[0], g[1], g[2], 's', r[0], r[1], r[2]])
            }
        };
        [
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[0]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[1]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[2]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[3]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[4]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[5]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[6]
            ),
            format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}\x1b[0m",
                background, foreground, subcells[7]
            ),
        ]
    }
}

fn to_char(x: u8) -> char {
    let s = format!("{}", x);
    s.chars().next().unwrap_or('0')
}

fn to_chars3(x: u8) -> [char; 3] {
    let s = format!("{:03}", x);
    let mut c = s.chars();
    [
        c.next().unwrap_or('0'),
        c.next().unwrap_or('0'),
        c.next().unwrap_or('0'),
    ]
}

fn to_braille(x: u8, max_x: u8) -> char {
    let n = (x * 8) / max_x;
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
    Wheat {
        growth: u8,
    },
    Bush {
        growth: u8,
        berries: u8,
    },
    Tree {
        growth: u8,
    },
    Cane {
        growth: u8,
    },
    Pumpkin {
        growth: u8,
        curent_size: u8,
        max_size: u8,
    },
    Cactus {
        growth: u8,
        size: u8,
    },
    Wallbush {
        growth: u8,
        health: u8,
    },
    Swapshroom {
        pair_id: Uuid,
        active: bool,
    },
    Sunflower {
        growth: u8,
        rank: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Seed {
    Wheat,
    Bush,
    Tree,
    Cane,
    Pupkin,
    Cactus,
    Wallbush,
    Swapshroom { pair_id: Option<Uuid> },
    Sunflower,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Harvest {
    Grains,
    Berry,
    Wood,
    Sugar,
    PumpkinSeed,
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
