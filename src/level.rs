use std::collections::HashMap;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::net::SubmitScoreEvent;
use crate::player::{spawn_player, Player};
use crate::replay::{ReplayData, ReplayRecorder};
use crate::ui::{Leaderboard, SpeedrunTimer};
use crate::walls::Wall;

const TILE: f32 = 16.0;
const GRID_W: i32 = 40; // 640px wide
const SIDE_WALL: i32 = 2;
const FLOOR_H: i32 = 3;
const CEILING_H: i32 = 2;
const SECTION_COUNT: i32 = 8;

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
pub struct LevelCounter(pub u32);

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
            .insert_resource(LevelCounter(1))
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

// --- Difficulty ---

struct Difficulty {
    min_plat_width: i32,
    max_v_gap: i32,
    max_h_gap: i32,
    dash_gauntlet_chance: f32,
    chimney_chance: f32,
}

fn difficulty(level: u32) -> Difficulty {
    let t = ((level - 1) as f32 / 9.0).min(1.0); // 0..1 over levels 1..10
    Difficulty {
        min_plat_width: (5.0 - 3.0 * t).round() as i32, // 5→2
        max_v_gap: (3.0 + 0.5 * t).round() as i32,      // 3→3-4 tiles
        max_h_gap: (3.0 + 3.0 * t).round() as i32,      // 3→6
        dash_gauntlet_chance: if level >= 3 { 0.15 + 0.1 * t } else { 0.0 },
        chimney_chance: 0.1 + 0.15 * t,
    }
}

// --- Section patterns ---

#[derive(Clone, Copy)]
enum SectionType {
    PlatformStaircase,
    WallCorridor,
    ScatteredPlatforms,
    Chimney,
    DashGauntlet,
}

fn pick_section(rng: &mut impl Rng, diff: &Difficulty) -> SectionType {
    let r: f32 = rng.random();
    if r < diff.dash_gauntlet_chance {
        SectionType::DashGauntlet
    } else if r < diff.dash_gauntlet_chance + diff.chimney_chance {
        SectionType::Chimney
    } else {
        match rng.random_range(0..3) {
            0 => SectionType::PlatformStaircase,
            1 => SectionType::WallCorridor,
            _ => SectionType::ScatteredPlatforms,
        }
    }
}

fn generate_section(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    section_type: SectionType,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    diff: &Difficulty,
) -> i32 {
    let playable_left = SIDE_WALL;
    let playable_right = GRID_W - SIDE_WALL - 1;

    match section_type {
        SectionType::PlatformStaircase => {
            generate_platform_staircase(grid, rng, entry_x, y_start, y_end, diff, playable_left, playable_right)
        }
        SectionType::WallCorridor => {
            generate_wall_corridor(grid, rng, entry_x, y_start, y_end, playable_left, playable_right)
        }
        SectionType::ScatteredPlatforms => {
            generate_scattered_platforms(grid, rng, entry_x, y_start, y_end, diff, playable_left, playable_right)
        }
        SectionType::Chimney => {
            generate_chimney(grid, rng, entry_x, y_start, y_end, playable_left, playable_right)
        }
        SectionType::DashGauntlet => {
            generate_dash_gauntlet(grid, rng, entry_x, y_start, y_end, diff, playable_left, playable_right)
        }
    }
}

fn place_platform(grid: &mut Vec<Vec<bool>>, x: i32, y: i32, width: i32) {
    let height = grid.len() as i32;
    let grid_w = if grid.is_empty() { 0 } else { grid[0].len() as i32 };
    for dx in 0..width {
        let px = x + dx;
        if px >= 0 && px < grid_w && y >= 0 && y < height {
            grid[y as usize][px as usize] = true;
        }
    }
}

fn generate_platform_staircase(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    diff: &Difficulty,
    playable_left: i32,
    playable_right: i32,
) -> i32 {
    let mut cur_x = entry_x;
    let section_height = y_end - y_start;
    // Ensure vertical gaps stay within max_v_gap
    let step_count = (section_height / diff.max_v_gap).max(2);
    let step_h = section_height / step_count;

    for i in 0..step_count {
        let y = y_start + i * step_h;
        let plat_w = rng.random_range(diff.min_plat_width..=5);
        let max_shift = diff.max_h_gap.min(playable_right - plat_w - playable_left).max(0);
        let shift = rng.random_range(-max_shift..=max_shift);
        cur_x = (cur_x + shift).clamp(playable_left, playable_right - plat_w + 1);
        place_platform(grid, cur_x, y, plat_w);
    }

    cur_x
}

fn generate_wall_corridor(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    playable_left: i32,
    playable_right: i32,
) -> i32 {
    // Narrow shaft with walls on both sides, 4-6 tiles wide (within wall-jump range)
    let corridor_w = rng.random_range(4..=6);
    let corridor_x = entry_x.clamp(playable_left, playable_right - corridor_w);

    // Stop walls 3 rows short of the top so the exit isn't blocked
    for y in y_start..(y_end - 3).max(y_start) {
        // Left wall of corridor
        if corridor_x > playable_left {
            for x in playable_left..corridor_x {
                if y >= 0 && (y as usize) < grid.len() {
                    grid[y as usize][x as usize] = true;
                }
            }
        }
        // Right wall of corridor
        let right_start = corridor_x + corridor_w;
        if right_start <= playable_right {
            for x in right_start..=playable_right {
                if y >= 0 && (y as usize) < grid.len() {
                    grid[y as usize][x as usize] = true;
                }
            }
        }
    }

    corridor_x + corridor_w / 2
}

fn generate_scattered_platforms(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    diff: &Difficulty,
    playable_left: i32,
    playable_right: i32,
) -> i32 {
    // Place platforms in a grid pattern with a guaranteed path
    let section_height = y_end - y_start;
    let rows = (section_height / 3).max(2);
    let step_h = section_height / rows;

    let mut path_x = entry_x;

    for i in 0..rows {
        let y = y_start + i * step_h;
        let max_w = diff.min_plat_width.max(4);
        let plat_w = rng.random_range(diff.min_plat_width.min(4)..=max_w);

        // Place guaranteed path platform
        let shift = rng.random_range(-3..=3i32);
        path_x = (path_x + shift).clamp(playable_left, playable_right - plat_w + 1);
        place_platform(grid, path_x, y, plat_w);

        // Place 1-3 extra scattered platforms
        let extras = rng.random_range(1..=3);
        for _ in 0..extras {
            let ex = rng.random_range(playable_left..=playable_right - diff.min_plat_width.min(4) + 1);
            let ew = rng.random_range(diff.min_plat_width.min(4)..=max_w);
            place_platform(grid, ex, y, ew);
        }
    }

    path_x
}

fn generate_chimney(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    playable_left: i32,
    playable_right: i32,
) -> i32 {
    // Two parallel walls for wall-jumping straight up, 3-5 tiles apart
    let gap = rng.random_range(3..=5);
    let wall_thickness = 2;
    let left_wall_x = entry_x.clamp(playable_left, playable_right - gap - wall_thickness * 2);

    for y in y_start..y_end {
        if y >= 0 && (y as usize) < grid.len() {
            // Left wall
            for dx in 0..wall_thickness {
                let x = left_wall_x + dx;
                if x >= playable_left && x <= playable_right {
                    grid[y as usize][x as usize] = true;
                }
            }
            // Right wall
            for dx in 0..wall_thickness {
                let x = left_wall_x + wall_thickness + gap + dx;
                if x >= playable_left && x <= playable_right {
                    grid[y as usize][x as usize] = true;
                }
            }
        }
    }

    // Landing platform at top
    let center = left_wall_x + wall_thickness + gap / 2;
    place_platform(grid, center - 1, y_end - 1, 3);

    center
}

fn generate_dash_gauntlet(
    grid: &mut Vec<Vec<bool>>,
    rng: &mut impl Rng,
    entry_x: i32,
    y_start: i32,
    y_end: i32,
    diff: &Difficulty,
    playable_left: i32,
    playable_right: i32,
) -> i32 {
    // Platforms with gaps requiring dash+jump combos
    let section_height = y_end - y_start;
    let step_count = (section_height / 3).max(2);
    let step_h = section_height / step_count;

    let mut cur_x = entry_x;

    for i in 0..step_count {
        let y = y_start + i * step_h;
        let plat_w = rng.random_range(2..=diff.min_plat_width.max(2));
        // Wider gaps requiring dash
        let gap = rng.random_range(4..=diff.max_h_gap.max(4));
        let dir = if rng.random_bool(0.5) { 1 } else { -1 };
        cur_x = (cur_x + dir * gap).clamp(playable_left, playable_right - plat_w + 1);
        place_platform(grid, cur_x, y, plat_w);
    }

    cur_x
}

// --- Tile merging (from walls.rs plate algorithm) ---

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

    // Stage 1: Create horizontal plates per row
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

    // Stage 2: Merge plates vertically into rectangles
    let mut rect_builder: HashMap<Plate, WallRect> = HashMap::new();
    let mut prev_row: Vec<Plate> = Vec::new();
    let mut wall_rects: Vec<WallRect> = Vec::new();

    // Extra empty row to finalize rects touching the top edge
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
    level: u32,
) -> Vec2 {
    let mut rng = StdRng::seed_from_u64(seed);
    let diff = difficulty(level);

    // Determine grid height: 80 + some randomness
    let grid_h = rng.random_range(80..=100) as i32;
    let grid_w = GRID_W;

    // Initialize empty grid
    let mut grid = vec![vec![false; grid_w as usize]; grid_h as usize];

    // Floor (bottom 3 rows)
    for y in 0..FLOOR_H {
        for x in 0..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Ceiling (top 2 rows)
    for y in (grid_h - CEILING_H)..grid_h {
        for x in 0..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Side walls (2 tiles thick on each side)
    for y in 0..grid_h {
        for x in 0..SIDE_WALL {
            grid[y as usize][x as usize] = true;
        }
        for x in (grid_w - SIDE_WALL)..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Generate sections between floor and ceiling
    let playable_bottom = FLOOR_H;
    let playable_top = grid_h - CEILING_H;
    let total_playable = playable_top - playable_bottom;
    let section_h = total_playable / SECTION_COUNT;

    let mut entry_x = grid_w / 2; // Start in the middle

    // Place a landing platform at the bottom for the player
    place_platform(&mut grid, entry_x - 2, playable_bottom, 5);

    for i in 0..SECTION_COUNT {
        let y_start = playable_bottom + i * section_h;
        let y_end = if i == SECTION_COUNT - 1 {
            playable_top
        } else {
            playable_bottom + (i + 1) * section_h
        };

        let section_type = pick_section(&mut rng, &diff);
        entry_x = generate_section(&mut grid, &mut rng, section_type, entry_x, y_start, y_end, &diff);
    }

    // Place checkpoint platform near the top
    let checkpoint_plat_y = playable_top - 3;
    let checkpoint_x = entry_x.clamp(SIDE_WALL + 1, grid_w - SIDE_WALL - 4);
    place_platform(&mut grid, checkpoint_x, checkpoint_plat_y, 5);

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
    let sp_x = (grid_w as f32 / 2.0) * TILE;
    let sp_y = (playable_bottom as f32 + 1.5) * TILE;

    info!("Generated level {} ({}x{} grid, seed {})", level, grid_w, grid_h, seed);

    Vec2::new(sp_x, sp_y)
}

fn generate_level(
    mut commands: Commands,
    level_counter: Res<LevelCounter>,
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

    let sp = build_level(&mut commands, seed, level_counter.0);
    spawn_point.0 = sp;

    // Reset recorder for this run
    recorder.frames.clear();
    recorder.seed = seed;
    recorder.level = level_counter.0;

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
    mut level_counter: ResMut<LevelCounter>,
    mut next_state: ResMut<NextState<GamePhase>>,
) {
    for entity in &level_query {
        commands.entity(entity).despawn();
    }

    level_counter.0 += 1;
    next_state.set(GamePhase::Generating);
}

fn regenerate_level(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut level_counter: ResMut<LevelCounter>,
) {
    let gp_regen = gamepads.iter().next().is_some_and(|g| g.just_pressed(GamepadButton::Select));
    if keys.just_pressed(KeyCode::KeyR) || gp_regen {
        // Keep same level number — just regenerate
        level_counter.0 = level_counter.0.saturating_sub(1);
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
    mut submit_events: EventWriter<SubmitScoreEvent>,
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
                    recorder.level,
                    recorder.frames.clone(),
                );
                // Submit score to online leaderboard
                submit_events.write(SubmitScoreEvent {
                    time: timer.elapsed,
                    seed: recorder.seed,
                    level: recorder.level,
                    inputs: recorder.frames.clone(),
                });
            }
            next_state.set(GamePhase::Transitioning);
            return;
        }
    }
}
