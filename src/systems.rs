use bevy::prelude::*;

use crate::{
    components::{Id, Name, Player, PlayerBundle, Position, ToPlayerTx},
    resources::GameSettings,
    tcp_network::ToGameRx,
};

pub fn print_position(query: Query<(&Position, &Name), With<Player>>) {
    for (position, name) in &query {
        //println!("{}: {:?}", name.0, position);
    }
}

pub fn new_player(mut commands: Commands) {
    // commands.spawn(PlayerBundle {
    //     marker: Player,
    //     name: Name("P001".to_string()),
    //     id: Id(Uuid::new_v4()),
    //     position: Position { x: 0, y: 0 },
    // });
}

pub fn handle_to_game_rx(
    game_settings: Res<GameSettings>,
    mut to_game_rx: ResMut<ToGameRx>,
    mut commands: Commands,
    mut players: Query<(&Name, &Id, &ToPlayerTx, &Position), With<Player>>,
) {
    let players = players.iter_mut();
    let connected_players = players.len() as u32;

    info!("Turn!");
}
