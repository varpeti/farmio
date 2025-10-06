use bevy::prelude::*;

#[derive(Resource)]
pub struct GameSettings {
    pub number_of_players: u32,
    pub turn_duration_ms: u64,
}
