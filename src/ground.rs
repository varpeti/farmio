use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum Ground {
    Dirt,
    Tiled,
    Sand,
    Water,
    Stone,
}
