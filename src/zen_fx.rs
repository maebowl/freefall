//! Zen-mode "juice": real-world height milestones, a personal-best line, and a
//! NEW BEST celebration. Heights use the HUD unit — 1 m = 16 px = one tile — so
//! a milestone at `h` meters sits at world-y `spawn.y + h * 16`. The camera
//! follows the player, so markers scroll into view as you climb.

use bevy::prelude::*;

use crate::level::{GameMode, GamePhase, LevelEntity, SpawnPoint};
use crate::player::Player;
use crate::sfx::SfxEvent;
use crate::ui::ZenLeaderboard;

const TILE: f32 = 16.0;

/// Marker lines are drawn far wider than the 640 px play area so they always
/// span the screen no matter where the camera sits horizontally.
const LEVEL_CENTER_X: f32 = 320.0;
const MARKER_W: f32 = 2400.0;

/// Real-world height references, ascending. Edit freely — this is the single
/// source for milestone heights and labels. (meters, label)
const MILESTONES: &[(f32, &str)] = &[
    (5.0, "Giraffe"),
    (12.0, "Sparrow's flight"),
    (30.0, "Christ the Redeemer"),
    (50.0, "Niagara Falls"),
    (93.0, "Statue of Liberty"),
    (116.0, "Hyperion, the tallest tree"),
    (147.0, "Great Pyramid of Giza"),
    (300.0, "Eiffel Tower"),
    (443.0, "Empire State Building"),
    (553.0, "CN Tower"),
    (828.0, "Burj Khalifa"),
    (1500.0, "Skydivers open their chutes"),
    (2000.0, "Base of the clouds"),
    (8849.0, "Summit of Mt. Everest"),
];

/// Tags a milestone line or its label (also carries `LevelEntity` for cleanup).
#[derive(Component)]
struct ZenMarker;

/// A transient on-screen banner that fades out and despawns (NEW BEST).
#[derive(Component)]
struct FadingText {
    timer: Timer,
}

#[derive(Resource, Default)]
struct ZenFxState {
    /// Markers spawned for the current run.
    spawned: bool,
    /// Index of the next milestone not yet crossed this run.
    next_milestone: usize,
    /// Personal best (meters) from before this run started.
    pb: Option<f32>,
    pb_beaten: bool,
}

pub struct ZenFxPlugin;

impl Plugin for ZenFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ZenFxState>()
            .add_systems(OnEnter(GamePhase::Generating), reset_zen_fx)
            .add_systems(
                Update,
                (spawn_zen_markers, zen_crossing_feedback).run_if(in_state(GamePhase::Playing)),
            )
            .add_systems(Update, animate_fading_text);
    }
}

/// Reset per-run state at the start of each generation. Markers from the
/// previous run are despawned by `cleanup_level` (they carry `LevelEntity`).
fn reset_zen_fx(
    game_mode: Res<GameMode>,
    leaderboard: Res<ZenLeaderboard>,
    mut state: ResMut<ZenFxState>,
) {
    state.spawned = false;
    state.next_milestone = 0;
    state.pb_beaten = false;
    state.pb = if *game_mode == GameMode::Zen {
        leaderboard.heights.first().copied()
    } else {
        None
    };
}

/// Spawn all milestone lines + labels and the personal-best line once per run,
/// after `generate_level` has set the spawn point.
fn spawn_zen_markers(
    mut commands: Commands,
    game_mode: Res<GameMode>,
    spawn_point: Res<SpawnPoint>,
    mut state: ResMut<ZenFxState>,
) {
    if *game_mode != GameMode::Zen || state.spawned {
        return;
    }
    state.spawned = true;

    let base_y = spawn_point.0.y;
    for (h, label) in MILESTONES {
        spawn_marker(&mut commands, base_y + h * TILE, label, *h as u32, false);
    }
    if let Some(pb) = state.pb {
        if pb >= 1.0 {
            spawn_marker(&mut commands, base_y + pb * TILE, "YOUR BEST", pb as u32, true);
        }
    }
}

/// A horizontal world-space line with a label sitting just above it.
fn spawn_marker(commands: &mut Commands, y: f32, label: &str, meters: u32, is_pb: bool) {
    let (line_color, text_color, thickness) = if is_pb {
        (
            Color::srgba(1.0, 0.84, 0.0, 0.85),
            Color::srgb(1.0, 0.86, 0.2),
            3.0,
        )
    } else {
        (
            Color::srgba(0.65, 0.82, 1.0, 0.30),
            Color::srgba(0.8, 0.9, 1.0, 0.9),
            1.5,
        )
    };

    commands.spawn((
        ZenMarker,
        LevelEntity,
        Sprite::from_color(line_color, Vec2::new(MARKER_W, thickness)),
        Transform::from_xyz(LEVEL_CENTER_X, y, -1.0),
    ));

    commands.spawn((
        ZenMarker,
        LevelEntity,
        Text2d::new(format!("{label}  ·  {meters}m")),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(text_color),
        // 32px font scaled to ~8 world units (~16 screen px at the 0.5 zoom),
        // sitting just above the line.
        Transform::from_xyz(LEVEL_CENTER_X, y + 9.0, -1.0).with_scale(Vec3::splat(0.25)),
    ));
}

/// Fire the personal-best celebration as the player climbs past their record;
/// milestones get a quiet tick (no on-screen popup) as they're crossed.
fn zen_crossing_feedback(
    mut commands: Commands,
    game_mode: Res<GameMode>,
    spawn_point: Res<SpawnPoint>,
    player_query: Query<&Transform, With<Player>>,
    mut state: ResMut<ZenFxState>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if *game_mode != GameMode::Zen {
        return;
    }
    let Some(tf) = player_query.iter().next() else {
        return;
    };
    let height = (tf.translation.y - spawn_point.0.y).max(0.0) / TILE;

    while state.next_milestone < MILESTONES.len() && height >= MILESTONES[state.next_milestone].0 {
        sfx.write(SfxEvent::MenuTick);
        state.next_milestone += 1;
    }

    if let Some(pb) = state.pb {
        if !state.pb_beaten && pb >= 1.0 && height >= pb {
            state.pb_beaten = true;
            spawn_banner(&mut commands, "NEW BEST!".to_string(), Color::srgb(1.0, 0.86, 0.2), 52.0);
            sfx.write(SfxEvent::MenuDing);
        }
    }
}

/// A centered, fading on-screen banner (kept only for the personal best).
fn spawn_banner(commands: &mut Commands, text: String, color: Color, font_size: f32) {
    commands.spawn((
        FadingText {
            timer: Timer::from_seconds(2.5, TimerMode::Once),
        },
        Text::new(text),
        TextLayout::new_with_justify(Justify::Center),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(color),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(16.0),
            width: Val::Percent(100.0),
            ..default()
        },
    ));
}

/// Fade banners out over the tail of their lifetime, then despawn.
fn animate_fading_text(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadingText, &mut TextColor)>,
) {
    for (entity, mut fading, mut color) in &mut query {
        fading.timer.tick(time.delta());
        // Hold full opacity, then fade across the final 45% of the lifetime.
        let alpha = (fading.timer.fraction_remaining() / 0.45).min(1.0);
        color.0.set_alpha(alpha);
        if fading.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
