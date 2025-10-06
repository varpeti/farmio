use bevy::{asset::uuid::Uuid, prelude::*};
use tokio::sync::mpsc::Sender;

#[derive(Component)]
pub struct CName {
    pub name: String,
}

#[derive(Component)]
pub struct CUuid {
    pub uuid: Uuid,
}

#[derive(Component)]
pub struct CToPlayerTx {
    pub to_player_tx: Sender<String>,
}
