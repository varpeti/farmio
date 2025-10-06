use serde::Serialize;

use crate::{
    ground::Ground,
    plant::{Cactus, Cane, Plant, Tree, Wheat},
};

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
        };

        let (foreground, subcells) = match &self.plant {
            Plant::None => (0, [' '; 8]),
            Plant::Wheat(wheat) => {
                let g = to_chars3(wheat.growth);
                let m = to_chars3(Wheat::GROWTH_TO_GRAINS);
                (184, ['W', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Bush(bush) => {
                let g = to_chars3(bush.growth);
                let b = to_chars3(bush.berries);
                (76, ['B', g[0], g[1], g[2], '°', b[0], b[1], b[2]])
            }
            Plant::Tree(tree) => {
                let g = to_chars3(tree.growth);
                let m = to_chars3(Tree::GROWTH_TO_WOOD);
                (70, ['T', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Cane(cane) => {
                let g = to_chars3(cane.growth);
                let m = to_chars3(Cane::GROWTH_TO_SUGAR);
                (0, ['C', g[0], g[1], g[2], '/', m[0], m[1], m[2]])
            }
            Plant::Pumpkin(pumpkin) => {
                let g = to_chars3(pumpkin.growth);
                (
                    172,
                    [
                        'P',
                        g[0],
                        g[1],
                        g[2],
                        '+',
                        to_char(pumpkin.current_size),
                        '/',
                        to_char(pumpkin.max_size),
                    ],
                )
            }
            Plant::Cactus(cactus) => {
                let g = to_chars3(cactus.growth);
                (
                    22,
                    [
                        'I',
                        g[0],
                        g[1],
                        g[2],
                        '+',
                        to_char(cactus.size),
                        '/',
                        to_char(Cactus::MAX_CACTUSMEAT),
                    ],
                )
            }

            Plant::Wallbush(wallbush) => {
                let g = to_chars3(wallbush.growth);
                let h = to_chars3(wallbush.health);
                (0, ['#', g[0], g[1], g[2], '#', h[0], h[1], h[2]])
            }
            Plant::Swapshroom(swapshroom) => {
                let g = to_char(swapshroom.growth);
                let c = format!("{:07}", swapshroom.pair_id)
                    .chars()
                    .take(7)
                    .collect::<Vec<char>>();
                if swapshroom.active {
                    (53, ['*', c[0], c[1], c[2], c[3], c[4], c[5], c[6]])
                } else {
                    (53, [g, c[0], c[1], c[2], c[3], c[4], c[5], c[6]])
                }
            }
            Plant::Sunflower(sunflower) => {
                let g = to_chars3(sunflower.growth);
                let r = to_chars3(sunflower.rank);
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
