use serde::Serialize;

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
