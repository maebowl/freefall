use std::collections::HashMap;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::net::{PendingSubmission, SubmissionData};
use crate::pieces;
use crate::player::{spawn_player, Player};
use crate::replay::ReplayRecorder;
use crate::ui::{Leaderboard, SpeedrunTimer};
use crate::walls::Wall;

const TILE: f32 = 16.0;
const GRID_W: i32 = 40; // 640px wide

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GamePhase {
    #[default]
    NameEntry,
    TitleScreen,
    Generating,
    Playing,
    Transitioning,
    Replaying,
}

#[derive(Resource)]
pub struct LevelSeed(pub u64);

#[derive(Resource)]
pub struct SpawnPoint(pub Vec2);

#[derive(Component)]
pub struct LevelEntity;

#[derive(Component)]
pub struct Checkpoint;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GamePhase>()
            .insert_resource(LevelSeed(0))
            .insert_resource(SpawnPoint(Vec2::ZERO))
            .add_systems(OnEnter(GamePhase::Generating), generate_level)
            .add_systems(OnEnter(GamePhase::Playing), reposition_player)
            .add_systems(OnEnter(GamePhase::Transitioning), cleanup_level)
            .add_systems(
                Update,
                checkpoint_collision.run_if(in_state(GamePhase::Playing)),
            )
            .add_systems(
                Update,
                regenerate_level.run_if(in_state(GamePhase::Playing)),
            );
    }
}

// --- Tile merging ---

#[derive(Clone, Eq, PartialEq, Debug, Default, Hash)]
struct Plate {
    left: i32,
    right: i32,
}

struct WallRect {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

fn merge_grid_to_rects(grid: &[Vec<bool>]) -> Vec<WallRect> {
    let height = grid.len() as i32;
    let width = if height > 0 { grid[0].len() as i32 } else { return vec![] };

    let mut plate_stack: Vec<Vec<Plate>> = Vec::new();

    for y in 0..height {
        let mut row_plates: Vec<Plate> = Vec::new();
        let mut plate_start = None;

        for x in 0..width + 1 {
            let filled = x < width && grid[y as usize][x as usize];
            match (plate_start, filled) {
                (Some(s), false) => {
                    row_plates.push(Plate {
                        left: s,
                        right: x - 1,
                    });
                    plate_start = None;
                }
                (None, true) => plate_start = Some(x),
                _ => (),
            }
        }

        plate_stack.push(row_plates);
    }

    let mut rect_builder: HashMap<Plate, WallRect> = HashMap::new();
    let mut prev_row: Vec<Plate> = Vec::new();
    let mut wall_rects: Vec<WallRect> = Vec::new();

    plate_stack.push(Vec::new());

    for (y, current_row) in plate_stack.into_iter().enumerate() {
        for prev_plate in &prev_row {
            if !current_row.contains(prev_plate) {
                if let Some(rect) = rect_builder.remove(prev_plate) {
                    wall_rects.push(rect);
                }
            }
        }
        for plate in &current_row {
            rect_builder
                .entry(plate.clone())
                .and_modify(|e| e.top += 1)
                .or_insert(WallRect {
                    bottom: y as i32,
                    top: y as i32,
                    left: plate.left,
                    right: plate.right,
                });
        }
        prev_row = current_row;
    }

    wall_rects
}

// --- Level generation ---

/// Generates the level grid and spawns entities. Used for both normal play and replay.
pub fn build_level(
    commands: &mut Commands,
    seed: u64,
) -> Vec2 {
    let (grid, grid_h, checkpoint_x, checkpoint_plat_y) = pieces::select_and_layout(seed);

    // Merge grid into rects
    let rects = merge_grid_to_rects(&grid);

    // Spawn level root
    let level_entity = commands
        .spawn((
            LevelEntity,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // Spawn wall colliders as children
    commands.entity(level_entity).with_children(|parent| {
        for rect in &rects {
            let w = (rect.right - rect.left + 1) as f32 * TILE;
            let h = (rect.top - rect.bottom + 1) as f32 * TILE;
            let cx = (rect.left + rect.right + 1) as f32 * TILE / 2.0;
            let cy = (rect.bottom + rect.top + 1) as f32 * TILE / 2.0;

            parent.spawn((
                Wall,
                Collider::rectangle(w, h),
                RigidBody::Static,
                Friction::new(0.0),
                Sprite::from_color(
                    Color::srgb(0.55, 0.35, 0.2),
                    Vec2::new(w, h),
                ),
                Transform::from_xyz(cx, cy, 0.0),
            ));
        }

        // Spawn checkpoint (green square with sensor collider)
        let cp_cx = (checkpoint_x as f32 + 2.5) * TILE;
        let cp_cy = (checkpoint_plat_y as f32 + 1.5) * TILE;

        parent.spawn((
            Checkpoint,
            Collider::rectangle(TILE * 2.0, TILE * 2.0),
            Sensor,
            RigidBody::Static,
            Sprite::from_color(
                Color::srgb(0.2, 0.9, 0.3),
                Vec2::new(TILE * 2.0, TILE * 2.0),
            ),
            Transform::from_xyz(cp_cx, cp_cy, 1.0),
        ));
    });

    // Return spawn point
    let sp_x = (GRID_W as f32 / 2.0) * TILE;
    let sp_y = (3.0 + 1.5) * TILE; // FLOOR_H + 1.5

    info!("Generated level ({}x{} grid, seed {})", GRID_W, grid_h, seed);

    Vec2::new(sp_x, sp_y)
}

fn generate_level(
    mut commands: Commands,
    mut spawn_point: ResMut<SpawnPoint>,
    mut next_state: ResMut<NextState<GamePhase>>,
    player_query: Query<Entity, With<Player>>,
    mut level_seed: ResMut<LevelSeed>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    // Generate a new random seed
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    level_seed.0 = seed;

    let sp = build_level(&mut commands, seed);
    spawn_point.0 = sp;

    // Reset recorder for this run
    recorder.frames.clear();
    recorder.seed = seed;

    // Spawn player on first level
    if player_query.is_empty() {
        spawn_player(&mut commands, spawn_point.0);
    }

    next_state.set(GamePhase::Playing);
}

fn reposition_player(
    spawn_point: Res<SpawnPoint>,
    mut players: Query<(&mut Transform, &mut LinearVelocity), With<Player>>,
) {
    for (mut tf, mut vel) in &mut players {
        tf.translation = spawn_point.0.extend(0.0);
        vel.0 = Vec2::ZERO;
    }
}

fn cleanup_level(
    mut commands: Commands,
    level_query: Query<Entity, With<LevelEntity>>,
    mut next_state: ResMut<NextState<GamePhase>>,
) {
    for entity in &level_query {
        commands.entity(entity).despawn();
    }

    next_state.set(GamePhase::Generating);
}

fn regenerate_level(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_state: ResMut<NextState<GamePhase>>,
) {
    let gp_regen = gamepads.iter().next().is_some_and(|g| g.just_pressed(GamepadButton::Select));
    if keys.just_pressed(KeyCode::KeyR) || gp_regen {
        next_state.set(GamePhase::Transitioning);
    }
}

fn checkpoint_collision(
    collisions: Collisions,
    player_query: Query<Entity, With<Player>>,
    checkpoint_query: Query<Entity, With<Checkpoint>>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut timer: ResMut<SpeedrunTimer>,
    mut leaderboard: ResMut<Leaderboard>,
    recorder: Res<ReplayRecorder>,
    mut pending: ResMut<PendingSubmission>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };

    for checkpoint in &checkpoint_query {
        if collisions.contains(player, checkpoint) {
            // Stop timer and record time
            if timer.final_time.is_none() {
                timer.running = false;
                timer.final_time = Some(timer.elapsed);
                leaderboard.add_entry(
                    timer.elapsed,
                    recorder.seed,
                    recorder.frames.clone(),
                );
                // Submit score to online leaderboard
                pending.0 = Some(SubmissionData {
                    time: timer.elapsed,
                    seed: recorder.seed,
                    inputs: recorder.frames.clone(),
                });
            }
            next_state.set(GamePhase::Transitioning);
            return;
        }
    }
}
