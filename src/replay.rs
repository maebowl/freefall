use avian2d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ldtk::{self, CurrentLevel};
use crate::level::{build_level, GameMode, GamePhase, LevelEntity, SpawnPoint};
use crate::player::{apply_movement, detect_ground_and_walls, GhostPlayer, MergedInput, Player, PlayerState};

const OFFSCREEN: f32 = -99999.0;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct FrameInput {
    pub move_x: f32,
    pub move_y: f32,
    pub sprint: bool,
    pub jump_pressed: bool,
    pub jump_released: bool,
    pub dash_pressed: bool,
    /// Seconds the player was paused at this point (0.0 = not paused).
    #[serde(default)]
    pub pause_secs: f32,
}

#[derive(Resource, Default)]
pub struct ReplayRecorder {
    pub frames: Vec<FrameInput>,
    pub seed: u64,
    /// Wall-clock time when the player last paused (for measuring duration).
    pub pause_start: Option<f64>,
    /// Whether the player paused at any point during this run.
    pub had_pause: bool,
}

#[derive(Resource, Default)]
pub struct ReplayData {
    pub frames: Vec<FrameInput>,
    pub seed: u64,
    pub frame_index: usize,
}

#[derive(Component)]
pub struct ReplayHudUi;

#[derive(Component)]
pub struct ReplayPauseFlash;

/// Counts down how long the "PAUSED" overlay should stay visible during replay.
#[derive(Resource, Default)]
struct PauseFlashTimer(f32);

pub struct ReplayPlugin;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReplayRecorder>()
            .init_resource::<ReplayData>()
            .init_resource::<PauseFlashTimer>()
            .add_systems(
                OnEnter(GamePhase::Replaying),
                (setup_replay, spawn_replay_hud),
            )
            .add_systems(
                OnExit(GamePhase::Replaying),
                (cleanup_replay, despawn_replay_hud),
            )
            // Ghost physics in FixedUpdate (matches recording rate)
            .add_systems(
                FixedUpdate,
                (ghost_ground_detection, ghost_movement)
                    .chain()
                    .run_if(in_state(GamePhase::Replaying)),
            )
            // Exit check and pause flash in Update
            .add_systems(
                Update,
                (check_replay_exit, update_pause_flash)
                    .run_if(in_state(GamePhase::Replaying)),
            );
    }
}

fn setup_replay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    replay_data: Res<ReplayData>,
    mut spawn_point: ResMut<SpawnPoint>,
    mut players: Query<(&mut Visibility, &mut Transform, &mut LinearVelocity), With<Player>>,
    existing_levels: Query<Entity, With<LevelEntity>>,
    game_mode: Res<GameMode>,
    current_level: Res<CurrentLevel>,
) {
    // Despawn existing level before building replay level
    for entity in &existing_levels {
        commands.entity(entity).despawn();
    }

    // Rebuild the correct level type
    let sp = match *game_mode {
        GameMode::Levels => ldtk::build_ldtk_level(&mut commands, &asset_server, current_level.name()),
        GameMode::Zen => build_level(&mut commands, replay_data.seed),
    };
    spawn_point.0 = sp;

    // Hide real player and move offscreen during replay
    for (mut vis, mut tf, mut vel) in &mut players {
        *vis = Visibility::Hidden;
        tf.translation = Vec3::new(OFFSCREEN, OFFSCREEN, 0.0);
        vel.0 = Vec2::ZERO;
    }

    // Spawn ghost player at spawn point
    commands.spawn((
        GhostPlayer,
        PlayerState::default(),
        RigidBody::Dynamic,
        Collider::rectangle(14.0, 14.0),
        LinearVelocity::default(),
        GravityScale(1.0),
        Friction::new(0.0),
        LockedAxes::ROTATION_LOCKED,
        Sprite {
            image: asset_server.load("placeholder-sprite.png"),
            color: Color::srgba(1.0, 1.0, 1.0, 0.6),
            ..default()
        },
        Transform::from_translation(sp.extend(0.0)),
    ));

    info!(
        "Starting replay ({} frames, seed {})",
        replay_data.frames.len(),
        replay_data.seed
    );
}

fn cleanup_replay(
    mut commands: Commands,
    ghost_query: Query<Entity, With<GhostPlayer>>,
    level_query: Query<Entity, With<LevelEntity>>,
    mut players: Query<&mut Visibility, With<Player>>,
) {
    for entity in &ghost_query {
        commands.entity(entity).despawn();
    }
    for entity in &level_query {
        commands.entity(entity).despawn();
    }
    // Show real player again
    for mut vis in &mut players {
        *vis = Visibility::Inherited;
    }
}

fn ghost_ground_detection(
    spatial_query: SpatialQuery,
    mut query: Query<(Entity, &Transform, &mut PlayerState), With<GhostPlayer>>,
    time: Res<Time>,
) {
    for (entity, transform, mut state) in &mut query {
        detect_ground_and_walls(entity, transform, &mut state, &spatial_query, &time);
    }
}

fn ghost_movement(
    mut commands: Commands,
    mut ghosts: Query<
        (&mut LinearVelocity, &mut GravityScale, &mut PlayerState),
        With<GhostPlayer>,
    >,
    mut replay_data: ResMut<ReplayData>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut flash_timer: ResMut<PauseFlashTimer>,
    existing_flash: Query<Entity, With<ReplayPauseFlash>>,
) {
    let Ok((mut velocity, mut gravity_scale, mut state)) = ghosts.single_mut() else {
        return;
    };

    // If we're currently showing the pause overlay, don't advance
    if flash_timer.0 > 0.0 {
        return;
    }

    let idx = replay_data.frame_index;
    if idx >= replay_data.frames.len() {
        next_state.set(GamePhase::Generating);
        return;
    }

    let frame = &replay_data.frames[idx];

    // Pause frame — show overlay for the recorded duration, then advance
    if frame.pause_secs > 0.0 {
        flash_timer.0 = frame.pause_secs;
        if existing_flash.is_empty() {
            spawn_pause_flash(&mut commands);
        }
        replay_data.frame_index += 1;
        return;
    }

    let input = MergedInput {
        move_x: frame.move_x,
        move_y: frame.move_y,
        sprint: frame.sprint,
        jump_pressed: frame.jump_pressed,
        jump_released: frame.jump_released,
        dash_pressed: frame.dash_pressed,
    };

    apply_movement(
        &input,
        &mut velocity,
        &mut gravity_scale,
        &mut state,
        time.delta_secs(),
    );

    replay_data.frame_index += 1;
}

fn check_replay_exit(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_state: ResMut<NextState<GamePhase>>,
) {
    let gp_exit = gamepads
        .iter()
        .next()
        .is_some_and(|g| g.just_pressed(GamepadButton::East) || g.just_pressed(GamepadButton::Start));

    if keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::KeyB)
        || gp_exit
    {
        next_state.set(GamePhase::Generating);
    }
}

fn spawn_replay_hud(mut commands: Commands) {
    commands
        .spawn((
            ReplayHudUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("REPLAY  (Press Escape to exit)"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.4, 0.4)),
            ));
        });
}

fn despawn_replay_hud(
    mut commands: Commands,
    hud_query: Query<Entity, With<ReplayHudUi>>,
    flash_query: Query<Entity, With<ReplayPauseFlash>>,
) {
    for entity in &hud_query {
        commands.entity(entity).despawn();
    }
    for entity in &flash_query {
        commands.entity(entity).despawn();
    }
}

fn spawn_pause_flash(commands: &mut Commands) {
    commands
        .spawn((
            ReplayPauseFlash,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

fn update_pause_flash(
    mut commands: Commands,
    mut timer: ResMut<PauseFlashTimer>,
    time: Res<Time>,
    query: Query<Entity, With<ReplayPauseFlash>>,
) {
    if timer.0 > 0.0 {
        timer.0 -= time.delta_secs();
        if timer.0 <= 0.0 {
            for entity in &query {
                commands.entity(entity).despawn();
            }
        }
    }
}
