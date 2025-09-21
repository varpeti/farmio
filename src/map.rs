use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
};

use rand::{rngs::SmallRng, seq::SliceRandom};

use crate::{
    cell::Cell,
    dawing::Drawer,
    direction::Direction,
    ground::Ground,
    plant::{Bush, Cactus, Cane, Plant, Pumpkin, Sunflower, Swapshroom, Tree, Wallbush, Wheat},
    pos::Pos,
};

#[derive(Clone)]
pub struct Map {
    map: Vec<Vec<Cell>>,
}

impl Map {
    const GTP_TILLED_BUSH: u8 = 5;
    const GTP_SAND_EMPTY: u8 = 20;
    const GTP_SAND_CANE: u8 = 5;
    const GTP_WATER: u8 = 10;

    pub fn generate_map(map_size: usize, rng: &mut SmallRng) -> Map {
        let a = map_size * map_size;
        let tilled_bush: usize = (a * Map::GTP_TILLED_BUSH as usize) / 100;
        let sand_empty: usize = (a * Map::GTP_SAND_EMPTY as usize) / 100 + tilled_bush;
        let sand_cane: usize = (a * Map::GTP_SAND_CANE as usize) / 100 + sand_empty;
        let water: usize = (a * Map::GTP_WATER as usize) / 100 + sand_cane;
        let pumpkin: usize = 1 + water;
        let cactus: usize = 1 + pumpkin;

        let mut flat_map = Vec::with_capacity(a);
        for i in 0..a {
            let cell = if tilled_bush > i {
                Cell {
                    ground: Ground::Tiled,
                    plant: Plant::Bush(Bush {
                        growth: Bush::GROWTH_TO_WOOD + Bush::GROWTH_PER_BERRIES * Bush::MAX_BERRIES,
                        berries: Bush::MAX_BERRIES,
                    }),
                }
            } else if sand_empty > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::None,
                }
            } else if sand_cane > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::Cane(Cane {
                        growth: Cane::GROWTH_TO_SUGAR,
                    }),
                }
            } else if water > i {
                Cell {
                    ground: Ground::Water,
                    plant: Plant::None,
                }
            } else if pumpkin > i {
                Cell {
                    ground: Ground::Tiled,
                    plant: Plant::Pumpkin(Pumpkin {
                        growth: Pumpkin::GROWTH_TO_PUMPKINSEED,
                        current_size: 1,
                        max_size: 1,
                    }),
                }
            } else if cactus > i {
                Cell {
                    ground: Ground::Sand,
                    plant: Plant::Cactus(Cactus {
                        growth: Cactus::GROWTH_PER_CACTUSMEAT * Cactus::MAX_CACTUSMEAT,
                        size: Cactus::MAX_CACTUSMEAT,
                    }),
                }
            } else {
                Cell {
                    ground: Ground::Dirt,
                    plant: Plant::Wheat(Wheat {
                        growth: Wheat::GROWTH_TO_GRAINS,
                    }),
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

    pub fn get_highest_sunflower_rank(&self) -> u8 {
        let mut max_rank = 0;
        for line in self.map.iter() {
            for cell in line.iter() {
                if let Plant::Sunflower(Sunflower { growth: _, rank }) = cell.plant {
                    max_rank = max_rank.max(rank);
                }
            }
        }
        max_rank
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

    pub fn update_map(&mut self, active_swapshrooms: &mut HashMap<u32, (Pos, Pos)>) {
        let map_clone = self.clone();
        let mut grown_inactive_swapshrooms = HashMap::<u32, Vec<Pos>>::new();

        for (y, line) in self.map.iter_mut().enumerate() {
            for (x, cell) in line.iter_mut().enumerate() {
                let pos = Pos {
                    x: x as i32,
                    y: y as i32,
                };
                let neighbours = map_clone.get_neighbours(&pos);

                let mut growt_rate = 1u8;
                // Water
                for n_cell in neighbours.iter() {
                    if let Ground::Water = n_cell.ground {
                        growt_rate = 2;
                        break;
                    }
                }

                match &mut cell.plant {
                    Plant::None => {
                        if let Ground::Dirt = cell.ground {
                            cell.plant = Plant::Wheat(Wheat { growth: 0 });
                        }
                    }
                    Plant::Wheat(wheat) => {
                        if wheat.growth < Wheat::GROWTH_TO_GRAINS {
                            wheat.growth += growt_rate;
                        }
                    }
                    Plant::Bush(bush) => {
                        if bush.growth < Bush::GROWTH_TO_WOOD {
                            bush.growth += growt_rate;
                        } else if bush.growth
                            < Bush::GROWTH_TO_WOOD + Bush::GROWTH_PER_BERRIES * Bush::MAX_BERRIES
                        {
                            bush.growth += growt_rate;
                            bush.berries +=
                                (bush.growth - Bush::GROWTH_TO_WOOD) / Bush::GROWTH_PER_BERRIES;
                        }
                    }
                    Plant::Tree(tree) => {
                        // Stop growth if a neighbouring cell has a Tree
                        for n_cell in neighbours.iter() {
                            if let Plant::Tree(_) = n_cell.plant {
                                continue;
                            }
                        }
                        if tree.growth < Tree::GROWTH_TO_WOOD {
                            tree.growth += growt_rate;
                        }
                    }
                    Plant::Cane(cane) => {
                        if cane.growth < Cane::GROWTH_TO_SUGAR {
                            cane.growth += growt_rate;
                        }
                    }
                    Plant::Pumpkin(pumpkin) => {
                        let mut next_max_size = 1;
                        for n_cell in neighbours {
                            if let Plant::Pumpkin(pumpkin_) = n_cell.plant {
                                if pumpkin_.growth >= Pumpkin::GROWTH_TO_PUMPKINSEED {
                                    next_max_size += 1;
                                }
                            }
                        }
                        pumpkin.max_size = next_max_size;

                        match (Pumpkin::GROWTH_TO_PUMPKINSEED * pumpkin.max_size)
                            .cmp(&pumpkin.growth)
                        {
                            Ordering::Less => {
                                pumpkin.growth -= growt_rate;
                                pumpkin.current_size =
                                    pumpkin.growth / Pumpkin::GROWTH_TO_PUMPKINSEED;
                            }
                            Ordering::Equal => (),
                            Ordering::Greater => {
                                pumpkin.growth += growt_rate;
                                pumpkin.current_size =
                                    pumpkin.growth / Pumpkin::GROWTH_TO_PUMPKINSEED;
                            }
                        }
                    }
                    Plant::Cactus(cactus) => {
                        if cactus.growth < Cactus::GROWTH_PER_CACTUSMEAT * Cactus::MAX_CACTUSMEAT {
                            cactus.growth += growt_rate;
                            cactus.size = cactus.growth / Cactus::GROWTH_PER_CACTUSMEAT;
                        }
                    }
                    Plant::Wallbush(wallbush) => {
                        if wallbush.growth < Wallbush::GROWTH_TO_BE_READY {
                            wallbush.growth += growt_rate;
                        }
                    }
                    Plant::Swapshroom(swapshroom) => {
                        if swapshroom.growth < Swapshroom::GROWTH_TO_BE_READY {
                            swapshroom.growth += growt_rate;
                        } else if !swapshroom.active {
                            match grown_inactive_swapshrooms.entry(swapshroom.pair_id) {
                                Entry::Occupied(occupied_entry) => {
                                    occupied_entry.into_mut().push(pos);
                                }
                                Entry::Vacant(vacant_entry) => {
                                    vacant_entry.insert(vec![pos]);
                                }
                            }
                        }
                    }
                    Plant::Sunflower(sunflower) => {
                        if sunflower.growth < Sunflower::GROWTH_TO_POWER {
                            sunflower.growth += growt_rate;
                        }
                    }
                }
            }
        }
        for (pair_id, maybe_pair) in grown_inactive_swapshrooms {
            if maybe_pair.len() == 2 {
                let mut ok = true;
                for i in 0..2 {
                    let mut cell = self.get_cell(&maybe_pair[i]).clone();
                    if let Plant::Swapshroom(swapshroom) = &mut cell.plant {
                        swapshroom.active = true;
                        self.set_cell(&maybe_pair[i], cell);
                    } else {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    active_swapshrooms
                        .insert(pair_id, (maybe_pair[0].clone(), maybe_pair[1].clone()));
                } else {
                    eprintln!("Invalid state: Cell is not a Swapshroom: pair_id: `{}`, maybe_pair: `{:?}` ", pair_id, maybe_pair);
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
