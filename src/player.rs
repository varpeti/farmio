use bevy::{asset::uuid::Uuid, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::{
    components::{CName, CToPlayerTx, CUuid},
    tcp_network::try_msg_to_player,
};

#[derive(Component)]
pub struct MPlayer;

#[derive(Bundle)]
pub struct BPlayer {
    marker: MPlayer,
    c_name: CName,
    c_uuid: CUuid,
    c_to_player_tx: CToPlayerTx,
    c_action: CAction,
    //pub position: Position,
}

impl BPlayer {
    pub fn new(name: String, uuid: Uuid, to_player_tx: Sender<String>) -> Self {
        Self {
            marker: MPlayer {},
            c_name: CName { name },
            c_uuid: CUuid { uuid },
            c_to_player_tx: CToPlayerTx { to_player_tx },
            c_action: CAction::NoActionReceived,
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum PlayerAction {
    Connect { name: String, uuid: Uuid },
    Disconnect,
    Idle,
    Move { direction: Direction },
}

#[derive(Debug, Component, Serialize)]
pub enum CAction {
    NoActionReceived, // Implicit Idle
    Idling,           // Explicit Idle
    Moving { direction: Direction },
}

#[derive(Debug, Serialize)]
pub enum ActionResult {
    Idled,
    Moved { direction: Direction },
    // Blocked { by: BlockedBy }, TODO:
}

#[derive(Debug, Serialize)]
pub enum Notifications {
    GameStarted,
    //Swapped TODO:
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}
pub fn player_action_idle(to_player_tx: &mut Sender<String>) {
    try_msg_to_player(to_player_tx, ActionResult::Idled)
}

pub fn player_action_move(to_player_tx: &mut Sender<String>, direction: Direction) {
    try_msg_to_player(to_player_tx, ActionResult::Moved { direction })
}
