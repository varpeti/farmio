use bevy::{asset::uuid::Uuid, prelude::*};
use serde::Serialize;
use tokio::sync::mpsc::Sender;

use crate::{
    components::{CName, CToPlayerTx, CUuid},
    player::{
        BPlayer, CAction, MPlayer, Notifications, PlayerAction, player_action_idle,
        player_action_move,
    },
    resources::{GameState, RGameSettings, RTurnDurationTimer},
    tcp_network::{RToGameRx, try_msg_to_player},
};

#[derive(Debug, Serialize)]
enum GameResponse {
    InvalidMsg { err: String },
    Connected { game_settings: RGameSettings },
    Reconnected,
}

fn get_player_by_uuid(
    players: &Query<(&CName, &CUuid, &CToPlayerTx, Entity), With<MPlayer>>,
    player_uuid: Uuid,
) -> Option<(String, Uuid, Sender<String>, Entity)> {
    players
        .iter()
        .find(|(_, c_uuid, _, _)| c_uuid.uuid == player_uuid)
        .map(|(c_name, c_uuid, c_to_player_tx, e_player)| {
            (
                c_name.name.clone(),
                c_uuid.uuid,
                c_to_player_tx.to_player_tx.clone(),
                e_player,
            )
        })
}

pub fn wait_for_connections(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut to_game_rx: ResMut<RToGameRx>,
    players: Query<(&CName, &CUuid, &CToPlayerTx, Entity), With<MPlayer>>,
    game_settings: Res<RGameSettings>,
) {
    if let Ok(to_game_msg) = to_game_rx.to_game_rx.try_recv() {
        match serde_json::from_str::<PlayerAction>(&to_game_msg.msg) {
            Ok(PlayerAction::Connect { name, uuid }) => match to_game_msg.to_player_tx {
                Some(mut to_player_tx) => match get_player_by_uuid(&players, uuid) {
                    Some((_player_name, _player_uuid, _to_player_tx, e_player)) => {
                        commands.entity(e_player).despawn();
                        commands.spawn(BPlayer::new(name.clone(), uuid, to_player_tx.clone()));
                        try_msg_to_player(&mut to_player_tx, GameResponse::Reconnected);
                        info!("Player `{}` Reconnected", name);
                    }
                    None => {
                        commands.spawn(BPlayer::new(name.clone(), uuid, to_player_tx.clone()));
                        try_msg_to_player(
                            &mut to_player_tx,
                            GameResponse::Connected {
                                game_settings: game_settings.to_owned(),
                            },
                        );
                        info!("Player `{}` Connected", name);

                        if players.iter().len() + 1 == game_settings.number_of_players as usize {
                            next_state.set(GameState::CollectPlayerActions);
                            commands.insert_resource(RTurnDurationTimer::new(
                                game_settings.turn_duration_ms,
                            ));
                            for (_c_name, _c_uuid, c_to_player_tx, _e_player) in players {
                                try_msg_to_player(
                                    &mut c_to_player_tx.to_player_tx.clone(),
                                    Notifications::GameStarted,
                                );
                            }
                            info!("All Players Connected: Game Started");
                        }
                    }
                },
                None => {
                    error!("to_player_tx is None");
                }
            },
            Ok(PlayerAction::Disconnect) => {
                for (c_name, c_uuid, _c_to_player_tx, e_player) in players {
                    if c_uuid.uuid == to_game_msg.player_uuid {
                        commands.entity(e_player).despawn();
                        info!("Player `{}` Disconnected", c_name.name);
                        break;
                    }
                }
            }
            Ok(invalid_action) => {
                warn!("Invalid action: `{:?}`", invalid_action);
            }
            Err(err) => {
                warn!("Invalid action: `{}`", err);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn collect_player_actions(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut turn_duration_timer: ResMut<RTurnDurationTimer>,
    game_settings: Res<RGameSettings>,
    mut r_to_game_rx: ResMut<RToGameRx>,
    players: Query<(&CName, &CUuid, &CToPlayerTx, Entity), With<MPlayer>>,
    actions: Query<&CAction, With<MPlayer>>,
) {
    let turn_is_ending = turn_duration_timer.timer.tick(time.delta()).just_finished();

    if let Ok(msg) = r_to_game_rx.to_game_rx.try_recv() {
        match get_player_by_uuid(&players, msg.player_uuid) {
            Some((player_name, _player_uuid, mut to_player_tx, e_player)) => {
                match serde_json::from_str::<PlayerAction>(&msg.msg) {
                    Ok(player_action) => match player_action {
                        PlayerAction::Connect { name, uuid } => {
                            commands.entity(e_player).despawn();
                            commands.spawn(BPlayer::new(name, uuid, to_player_tx.clone()));
                            try_msg_to_player(&mut to_player_tx, GameResponse::Reconnected);
                            info!("Player `{}` Reconnected", player_name);
                        }
                        PlayerAction::Disconnect => {
                            commands.entity(e_player).despawn();
                            info!("Player `{}` Disconnected", player_name);
                        }
                        PlayerAction::Idle => {
                            commands
                                .entity(e_player)
                                .remove::<CAction>()
                                .insert(CAction::Idling);
                            try_msg_to_player(&mut to_player_tx, CAction::Idling);
                        }
                        PlayerAction::Move { direction } => {
                            commands
                                .entity(e_player)
                                .remove::<CAction>()
                                .insert(CAction::Moving { direction });
                            try_msg_to_player(&mut to_player_tx, CAction::Moving { direction });
                        }
                    },
                    Err(err) => {
                        warn!("Invalid msg from Player `{}`: `{}`", err, player_name);
                        try_msg_to_player(
                            &mut to_player_tx,
                            GameResponse::InvalidMsg {
                                err: err.to_string(),
                            },
                        );
                    }
                }
            }
            None => {
                warn!(
                    "Player in not connected, , and tried to do an action: `{:?}`",
                    msg
                )
            }
        }
    }

    let action_received = !actions
        .iter()
        .filter(|c_action| !matches!(&c_action, CAction::NoActionReceived))
        .count(); // FIXME: Its not a normal number...

    if turn_is_ending || action_received == game_settings.number_of_players as usize {
        turn_duration_timer.timer.reset();
        info!(
            "Turn is ending (time is up: `{}`, all actions received: `{}/{}` )",
            turn_is_ending, action_received, game_settings.number_of_players
        );
        next_state.set(GameState::HandlePlayerActions);
    }
}

pub fn handle_player_actions(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    players_with_action: Query<(&CToPlayerTx, Entity, &CAction), With<MPlayer>>,
    players_without_action: Query<Entity, (With<MPlayer>, Without<CAction>)>,
) {
    if players_with_action.iter().len() == 0 {
        next_state.set(GameState::MovePlayers);
        for c_player in players_without_action {
            commands.entity(c_player).insert(CAction::NoActionReceived);
        }
    }

    for (c_to_player_tx, e_player, c_action) in players_with_action {
        let mut to_player_tx = c_to_player_tx.to_player_tx.clone();
        match c_action {
            CAction::Idling | CAction::NoActionReceived => {
                player_action_idle(&mut to_player_tx);
            }
            CAction::Moving { direction } => {
                player_action_move(&mut to_player_tx, *direction);
            }
        }
        commands.entity(e_player).remove::<CAction>();
    }
}

pub fn move_players(mut next_state: ResMut<NextState<GameState>>) {
    error!("Move players should be implemented");
    next_state.set(GameState::CollectPlayerActions);
}

pub fn debug_prints(//state: Res<State<GameState>>,
) {
    //info!("Current state: {:?}", state);
}
