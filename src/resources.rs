use bevy::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Resource)]
pub struct RGameSettings {
    pub number_of_players: u32,
    pub turn_duration_ms: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, States)]
pub enum GameState {
    #[default]
    WaitForConnections,
    CollectPlayerActions,
    HandlePlayerActions,
    MovePlayers,
    //UpdateMap, TODO:
}

#[derive(Resource)]
pub struct RTurnDurationTimer {
    pub timer: Timer,
}

impl RTurnDurationTimer {
    pub fn new(turn_duration_ms: u64) -> Self {
        Self {
            timer: Timer::from_seconds(turn_duration_ms as f32 / 1000.0, TimerMode::Repeating),
        }
    }
}

#[derive(Resource, Default)]
pub struct RTurnActionReceived {
    pub num: u32,
}
