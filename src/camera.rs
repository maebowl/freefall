use bevy::prelude::*;

use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_follow);
    }
}

fn camera_follow(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
    time: Res<Time>,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let Ok(mut camera_tf) = camera_query.single_mut() else {
        return;
    };

    let target = player_tf.translation.truncate();
    let current = camera_tf.translation.truncate();

    // Exponential smoothing for frame-rate independent lerp
    let t = 1.0 - (-5.0 * time.delta_secs()).exp();
    let new_pos = current.lerp(target, t);

    camera_tf.translation.x = new_pos.x;
    camera_tf.translation.y = new_pos.y;
}
