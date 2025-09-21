use crate::direction::Direction;

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
