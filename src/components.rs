use bevy::{asset::uuid::Uuid, prelude::*};
use tokio::sync::mpsc::Sender;

#[derive(Component)]
pub struct Player;

#[derive(Component, Debug)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

#[derive(Component)]
pub struct Name(pub String);

#[derive(Component)]
pub struct Id(pub Uuid);

#[derive(Component)]
pub struct ToPlayerTx(pub Sender<String>);

#[derive(Bundle)]
pub struct PlayerBundle {
    pub marker: Player,
    pub name: Name,
    pub id: Id,
    pub to_player_tx: ToPlayerTx,
    pub position: Position,
}
