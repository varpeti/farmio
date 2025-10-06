mod components;
mod map;
mod player;
mod resources;
mod systems;
mod tcp_network;

use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use tokio::sync::mpsc;

use crate::{
    resources::{GameState, RGameSettings},
    systems::{
        collect_player_actions, debug_prints, handle_player_actions, move_players,
        wait_for_connections,
    },
    tcp_network::{RToGameRx, run_tcp_server},
};
// TODO: Read from config
const IP_PORT: &str = "127.0.0.1:5942";
const NUMBER_OF_PLAYERS: u32 = 1;
const TURN_DURATION_MS: u64 = 10000;

fn setup_camera_system(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn main() {
    let (to_game_tx, to_game_rx) = mpsc::channel(1024);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Unable to create Tokio Runtime!");
        rt.block_on(async {
            run_tcp_server(to_game_tx, IP_PORT).await;
        })
    });

    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_camera_system)
        .insert_resource(RToGameRx { to_game_rx })
        .insert_resource(RGameSettings {
            number_of_players: NUMBER_OF_PLAYERS,
            turn_duration_ms: TURN_DURATION_MS,
        })
        .init_state::<GameState>()
        .add_systems(
            Update,
            wait_for_connections.run_if(in_state(GameState::WaitForConnections)),
        )
        .add_systems(
            Update,
            collect_player_actions.run_if(in_state(GameState::CollectPlayerActions)),
        )
        .add_systems(
            Update,
            handle_player_actions.run_if(in_state(GameState::HandlePlayerActions)),
        )
        .add_systems(
            Update,
            move_players.run_if(in_state(GameState::MovePlayers)),
        )
        .add_systems(
            Update,
            debug_prints.run_if(on_timer(Duration::from_secs(3))),
        )
        .run();
}
