mod components;
mod resources;
mod systems;
mod tcp_network;

use std::time::Duration;

use bevy::{ecs::schedule::IntoScheduleConfigs, prelude::*, time::common_conditions::on_timer};
use tokio::sync::mpsc;

use crate::{
    resources::GameSettings,
    systems::{handle_to_game_rx, new_player, print_position},
    tcp_network::{ToGameRx, run_tcp_server},
};
// TODO: Read from config
const IP_PORT: &str = "127.0.0.1:5942";
const NUMBER_OF_PLAYERS: u32 = 1;
const TURN_DURATION_MS: u64 = 1000;

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
        .insert_resource(ToGameRx { to_game_rx })
        .insert_resource(GameSettings {
            number_of_players: NUMBER_OF_PLAYERS,
            turn_duration_ms: TURN_DURATION_MS,
        })
        .add_systems(
            Update,
            handle_to_game_rx.run_if(on_timer(Duration::from_millis(TURN_DURATION_MS))),
        )
        .run();
}
