//! Smoothly follows the player (or the replay ghost) with a frame-rate-
//! independent camera. Registered by `CameraPlugin`.

use bevy::prelude::*;

use crate::player::{GhostPlayer, Player};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_follow);
    }
}

fn camera_follow(
    player_query: Query<&Transform, (With<Player>, Without<GhostPlayer>, Without<Camera2d>)>,
    ghost_query: Query<&Transform, (With<GhostPlayer>, Without<Player>, Without<Camera2d>)>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>, Without<GhostPlayer>)>,
    time: Res<Time>,
) {
    // Follow ghost if one exists, otherwise follow player
    let target_tf = if let Ok(ghost_tf) = ghost_query.single() {
        ghost_tf
    } else if let Ok(player_tf) = player_query.single() {
        player_tf
    } else {
        return;
    };

    let Ok(mut camera_tf) = camera_query.single_mut() else {
        return;
    };

    let target = target_tf.translation.truncate();
    let current = camera_tf.translation.truncate();

    // Exponential smoothing for frame-rate independent lerp
    let t = 1.0 - (-5.0 * time.delta_secs()).exp();
    let new_pos = current.lerp(target, t);

    camera_tf.translation.x = new_pos.x;
    camera_tf.translation.y = new_pos.y;
}
