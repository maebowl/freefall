use avian2d::prelude::{Physics, PhysicsTime};
use bevy::prelude::*;

use crate::ldtk::{CurrentLevel, LEVEL_ORDER};
use crate::level::{GameMode, GamePhase, LevelEntity, SpawnPoint, ZenRun};
use crate::net::{NetStatus, OnlineLeaderboard, PendingReplayFetch, PendingSubmission, PlayerName, ReplayFetchStatus};
use crate::player::Player;
use crate::replay::{FrameInput, ReplayData};
use crate::sfx::SfxEvent;
use crate::username::{self, ForceNameEntry};

// --- Resources ---

#[derive(Resource)]
pub struct SpeedrunTimer {
    pub elapsed: f32,
    pub running: bool,
    pub final_time: Option<f32>,
}

impl Default for SpeedrunTimer {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            running: false,
            final_time: None,
        }
    }
}

#[derive(Clone)]
pub struct LeaderboardEntry {
    pub time: f32,
    pub seed: u64,
    pub inputs: Vec<FrameInput>,
}

#[derive(Resource, Default)]
pub struct Leaderboard {
    pub entries: Vec<LeaderboardEntry>,
    pub all_times: Vec<f32>,
}

impl Leaderboard {
    pub fn add_entry(&mut self, time: f32, seed: u64, inputs: Vec<FrameInput>) {
        // Track all times for placement calculation
        let insert_pos = self.all_times.partition_point(|&t| t < time);
        self.all_times.insert(insert_pos, time);

        // Only store replay data for top 5
        self.entries.push(LeaderboardEntry {
            time,
            seed,
            inputs,
        });
        self.entries.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        self.entries.truncate(5);
    }
}

#[derive(Resource, Default)]
struct LeaderboardVisible {
    visible: bool,
    cached_time: Option<f32>,
}

#[derive(Resource, Default)]
pub struct LastRunTime(pub Option<f32>);

#[derive(Resource, Default)]
struct LeaderboardSelection(usize);

#[derive(Resource, Default)]
struct LevelSelectSelection(usize);

#[derive(Resource, Default)]
struct PauseSelection(usize);

#[derive(Resource, Default)]
struct TitleSelection(usize);

// --- Components ---

#[derive(Component)]
struct TitleScreenUi;

#[derive(Component)]
struct LevelSelectUi;

#[derive(Component)]
struct HudUi;

#[derive(Component)]
struct TimerText;

#[derive(Component)]
struct LeaderboardUi;

#[derive(Component)]
struct LeaderboardRow(usize);

#[derive(Component)]
struct PauseUi;

#[derive(Component)]
struct LevelCompleteUi;

#[derive(Component)]
struct RainbowText;

#[derive(Resource, Default)]
struct LevelCompleteSelection(usize);

#[derive(Resource, Default)]
struct ScoreNamePrompt {
    active: bool,
    buffer: String,
}

const MAX_NAME_LEN: usize = 16;

#[derive(Resource, Default)]
pub struct DeferredSubmission(pub Option<crate::net::SubmissionData>);

#[derive(Resource)]
pub struct ZenLeaderboard {
    pub heights: Vec<f32>,
}

impl Default for ZenLeaderboard {
    fn default() -> Self {
        Self { heights: load_zen_heights() }
    }
}

impl ZenLeaderboard {
    pub fn add_height(&mut self, height: f32) {
        self.heights.push(height);
        self.heights.sort_by(|a, b| b.partial_cmp(a).unwrap());
        self.heights.truncate(5);
        save_zen_heights(&self.heights);
    }
}

fn zen_leaderboard_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".freefall").join("zen_leaderboard.json")
}

fn load_zen_heights() -> Vec<f32> {
    let path = zen_leaderboard_path();
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_zen_heights(heights: &[f32]) {
    let path = zen_leaderboard_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, serde_json::to_string(heights).unwrap_or_default());
}

// --- On-screen keyboard ---

pub const KB_GRID: &[&[&str]] = &[
    &["A","B","C","D","E","F","G","H","I","J"],
    &["K","L","M","N","O","P","Q","R","S","T"],
    &["U","V","W","X","Y","Z","0","1","2","3"],
    &["4","5","6","7","8","9","DEL","OK"],
];

#[derive(Resource, Default)]
pub struct KeyboardCursor {
    pub row: usize,
    pub col: usize,
}

pub enum KeyboardAction {
    None,
    Type(char),
    Delete,
    Confirm,
}

pub fn keyboard_gamepad_input(gamepad: &Gamepad, cursor: &mut KeyboardCursor) -> (KeyboardAction, bool) {
    let mut moved = false;

    if gamepad.just_pressed(GamepadButton::DPadUp) && cursor.row > 0 {
        cursor.row -= 1;
        cursor.col = cursor.col.min(KB_GRID[cursor.row].len() - 1);
        moved = true;
    }
    if gamepad.just_pressed(GamepadButton::DPadDown) && cursor.row < KB_GRID.len() - 1 {
        cursor.row += 1;
        cursor.col = cursor.col.min(KB_GRID[cursor.row].len() - 1);
        moved = true;
    }
    if gamepad.just_pressed(GamepadButton::DPadLeft) && cursor.col > 0 {
        cursor.col -= 1;
        moved = true;
    }
    if gamepad.just_pressed(GamepadButton::DPadRight) && cursor.col < KB_GRID[cursor.row].len() - 1 {
        cursor.col += 1;
        moved = true;
    }

    if gamepad.just_pressed(GamepadButton::South) {
        let key = KB_GRID[cursor.row][cursor.col];
        let action = match key {
            "DEL" => KeyboardAction::Delete,
            "OK" => KeyboardAction::Confirm,
            s => s.chars().next().map(KeyboardAction::Type).unwrap_or(KeyboardAction::None),
        };
        return (action, moved);
    }
    if gamepad.just_pressed(GamepadButton::East) {
        return (KeyboardAction::Delete, moved);
    }
    if gamepad.just_pressed(GamepadButton::Start) {
        return (KeyboardAction::Confirm, moved);
    }

    (KeyboardAction::None, moved)
}

pub fn spawn_keyboard_grid(parent: &mut ChildSpawnerCommands, cursor: &KeyboardCursor) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .with_children(|kb: &mut ChildSpawnerCommands| {
            for (r, row) in KB_GRID.iter().enumerate() {
                kb.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row_node: &mut ChildSpawnerCommands| {
                    for (c, key) in row.iter().enumerate() {
                        let selected = r == cursor.row && c == cursor.col;
                        let (bg, fg) = if selected {
                            (Color::srgb(1.0, 1.0, 0.3), Color::srgb(0.05, 0.05, 0.1))
                        } else {
                            match *key {
                                "DEL" => (Color::srgba(0.4, 0.15, 0.15, 0.8), Color::srgb(0.9, 0.5, 0.5)),
                                "OK" => (Color::srgba(0.15, 0.4, 0.15, 0.8), Color::srgb(0.5, 0.9, 0.5)),
                                _ => (Color::srgba(0.2, 0.2, 0.3, 0.8), Color::srgb(0.8, 0.8, 0.8)),
                            }
                        };
                        let min_w = if *key == "DEL" || *key == "OK" {
                            Val::Px(48.0)
                        } else {
                            Val::Px(28.0)
                        };
                        row_node
                            .spawn((
                                Node {
                                    min_width: min_w,
                                    min_height: Val::Px(28.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::horizontal(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(bg),
                            ))
                            .with_children(|cell: &mut ChildSpawnerCommands| {
                                cell.spawn((
                                    Text::new(*key),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(fg),
                                ));
                            });
                    }
                });
            }
        });
}

// --- Plugin ---

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpeedrunTimer>()
            .init_resource::<Leaderboard>()
            .init_resource::<LeaderboardVisible>()
            .init_resource::<LeaderboardSelection>()
            .init_resource::<LevelSelectSelection>()
            .init_resource::<PauseSelection>()
            .init_resource::<TitleSelection>()
            .init_resource::<LevelCompleteSelection>()
            .init_resource::<LastRunTime>()
            .init_resource::<ScoreNamePrompt>()
            .init_resource::<DeferredSubmission>()
            .init_resource::<KeyboardCursor>()
            .init_resource::<ZenLeaderboard>()
            // Title screen
            .add_systems(OnEnter(GamePhase::TitleScreen), (spawn_title_screen, despawn_marked::<HudUi>, despawn_marked::<LeaderboardUi>, clear_leaderboard_visible))
            .add_systems(OnExit(GamePhase::TitleScreen), despawn_marked::<TitleScreenUi>)
            .add_systems(
                Update,
                title_screen_input.run_if(in_state(GamePhase::TitleScreen)),
            )
            // Level select
            .add_systems(OnEnter(GamePhase::LevelSelect), (spawn_level_select, reset_local_leaderboard))
            .add_systems(OnExit(GamePhase::LevelSelect), despawn_marked::<LevelSelectUi>)
            .add_systems(
                Update,
                level_select_input.run_if(in_state(GamePhase::LevelSelect)),
            )
            // HUD + timer
            .add_systems(OnEnter(GamePhase::Playing), spawn_hud_if_missing)
            .add_systems(OnEnter(GamePhase::Generating), reset_timer)
            .add_systems(
                Update,
                (start_timer_on_input, update_timer_display, check_pause)
                    .run_if(in_state(GamePhase::Playing)),
            )
            // Tick in FixedUpdate so the timer measures deterministic simulation
            // time (exact 1/64 s steps, framerate-independent) rather than
            // wall-clock frame time — this is what replays reproduce.
            .add_systems(
                FixedUpdate,
                tick_timer.run_if(in_state(GamePhase::Playing)),
            )
            .add_systems(Update, animate_rainbow)
            // Level complete
            .add_systems(OnEnter(GamePhase::LevelComplete), (spawn_level_complete, freeze_timer_display))
            .add_systems(OnExit(GamePhase::LevelComplete), despawn_marked::<LevelCompleteUi>)
            .add_systems(
                Update,
                level_complete_input.run_if(in_state(GamePhase::LevelComplete)),
            )
            // Pause menu
            .add_systems(OnEnter(GamePhase::Paused), (spawn_pause_menu, pause_physics))
            .add_systems(OnExit(GamePhase::Paused), (despawn_marked::<PauseUi>, despawn_marked::<LeaderboardUi>, clear_leaderboard_visible, record_pause_duration, unpause_physics))
            .add_systems(
                Update,
                pause_menu_input.run_if(in_state(GamePhase::Paused)),
            );
    }
}

// --- Generic despawn ---

fn despawn_marked<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn clear_leaderboard_visible(mut visible: ResMut<LeaderboardVisible>) {
    visible.visible = false;
}

fn reset_local_leaderboard(mut leaderboard: ResMut<Leaderboard>, mut last_run: ResMut<LastRunTime>) {
    leaderboard.entries.clear();
    leaderboard.all_times.clear();
    last_run.0 = None;
}

// --- Title screen ---

const TITLE_OPTIONS: &[&str] = &["Levels", "Zen Mode", "Change Name", "Quit"];

fn spawn_title_screen(mut commands: Commands, mut title_sel: ResMut<TitleSelection>) {
    title_sel.0 = 0;
    rebuild_title_screen(&mut commands, title_sel.0);
}

fn rebuild_title_screen(commands: &mut Commands, selected: usize) {
    commands
        .spawn((
            TitleScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.95)),
        ))
        .with_children(|parent| {
            parent.spawn((
                crate::font::TitleText,
                Text::new("FREEFALL"),
                TextFont {
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Menu options
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    row_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(16.0)),
                    ..default()
                })
                .with_children(|menu| {
                    for (i, label) in TITLE_OPTIONS.iter().enumerate() {
                        let is_selected = i == selected;
                        let color = if is_selected {
                            Color::srgb(1.0, 1.0, 0.3)
                        } else {
                            Color::srgb(0.8, 0.8, 0.8)
                        };
                        let prefix = if is_selected { "> " } else { "  " };
                        menu.spawn((
                            Text::new(format!("{}{}", prefix, label)),
                            TextFont {
                                font_size: 28.0,
                                ..default()
                            },
                            TextColor(color),
                        ));
                    }
                });
        });
}

fn title_screen_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut game_mode: ResMut<GameMode>,
    mut current_level: ResMut<CurrentLevel>,
    mut level_sel: ResMut<LevelSelectSelection>,
    mut title_sel: ResMut<TitleSelection>,
    mut commands: Commands,
    existing: Query<Entity, With<TitleScreenUi>>,
    mut exit: MessageWriter<AppExit>,
    mut force_name: ResMut<ForceNameEntry>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    let gamepad = gamepads.iter().next();
    let max_idx = TITLE_OPTIONS.len().saturating_sub(1);

    let gp_up = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadUp));
    let gp_down = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadDown));

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || gp_up;
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || gp_down;

    let mut changed = false;
    if up && title_sel.0 > 0 {
        title_sel.0 -= 1;
        changed = true;
    }
    if down && title_sel.0 < max_idx {
        title_sel.0 += 1;
        changed = true;
    }

    if changed {
        sfx.write(SfxEvent::MenuTick);
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        rebuild_title_screen(&mut commands, title_sel.0);
    }

    let gp_confirm = gamepad.is_some_and(|g| {
        g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::Start)
    });

    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
        sfx.write(SfxEvent::MenuDing);
        match title_sel.0 {
            0 => {
                // Levels
                *game_mode = GameMode::Levels;
                level_sel.0 = 0;
                next_state.set(GamePhase::LevelSelect);
            }
            1 => {
                // Zen Mode
                *game_mode = GameMode::Zen;
                current_level.0 = 0;
                next_state.set(GamePhase::Generating);
            }
            2 => {
                // Change Name
                force_name.0 = true;
                next_state.set(GamePhase::NameEntry);
            }
            3 => {
                // Quit
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }

    // Escape quits from title screen
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

// --- Level select ---

fn spawn_level_select(
    mut commands: Commands,
    mut selection: ResMut<LevelSelectSelection>,
) {
    selection.0 = 0;
    rebuild_level_select(&mut commands, selection.0);
}

fn level_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut selection: ResMut<LevelSelectSelection>,
    mut current_level: ResMut<CurrentLevel>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut commands: Commands,
    existing: Query<Entity, With<LevelSelectUi>>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    let gamepad = gamepads.iter().next();
    let max_idx = LEVEL_ORDER.len().saturating_sub(1);

    let gp_up = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadUp));
    let gp_down = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadDown));

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || gp_up;
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || gp_down;

    let mut changed = false;
    if up && selection.0 > 0 {
        selection.0 -= 1;
        changed = true;
    }
    if down && selection.0 < max_idx {
        selection.0 += 1;
        changed = true;
    }

    if changed {
        sfx.write(SfxEvent::MenuTick);
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        rebuild_level_select(&mut commands, selection.0);
    }

    let gp_confirm = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
        sfx.write(SfxEvent::MenuDing);
        current_level.0 = selection.0;
        next_state.set(GamePhase::Generating);
    }

    let gp_back = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East));
    if keys.just_pressed(KeyCode::Escape) || gp_back {
        next_state.set(GamePhase::TitleScreen);
    }
}

fn rebuild_level_select(commands: &mut Commands, selected: usize) {
    commands
        .spawn((
            LevelSelectUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.95)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Start,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.15, 0.95)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("SELECT LEVEL"),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.7, 1.0)),
                    ));

                    for (i, name) in LEVEL_ORDER.iter().enumerate() {
                        let is_selected = i == selected;
                        let color = if is_selected {
                            Color::srgb(1.0, 1.0, 0.3)
                        } else {
                            Color::srgb(0.8, 0.8, 0.8)
                        };
                        let prefix = if is_selected { "> " } else { "  " };
                        let display = name.replace('_', " ");
                        panel.spawn((
                            Text::new(format!("{}{}", prefix, display)),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(color),
                        ));
                    }

                    panel.spawn((
                        Text::new("Up/Down: Select  |  Space/A: Play  |  Esc: Back"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        Node {
                            margin: UiRect::top(Val::Px(16.0)),
                            ..default()
                        },
                    ));
                });
        });
}

// --- Pause menu ---

fn check_pause(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut recorder: ResMut<crate::replay::ReplayRecorder>,
    time: Res<Time>,
) {
    let gp_pause = gamepads
        .iter()
        .next()
        .is_some_and(|g| g.just_pressed(GamepadButton::Start));
    if keys.just_pressed(KeyCode::Escape) || gp_pause {
        recorder.pause_start = Some(time.elapsed_secs_f64());
        next_state.set(GamePhase::Paused);
    }
}

/// Freeze the physics simulation while the pause menu is open so the player
/// (e.g. mid-dash, with gravity disabled) doesn't keep drifting.
fn pause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.pause();
}

fn unpause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.unpause();
}

fn record_pause_duration(
    mut recorder: ResMut<crate::replay::ReplayRecorder>,
    time: Res<Time>,
) {
    if let Some(start) = recorder.pause_start.take() {
        let duration = (time.elapsed_secs_f64() - start) as f32;
        if duration > 0.0 {
            recorder.frames.push(crate::replay::FrameInput {
                pause_secs: duration,
                ..default()
            });
        }
    }
}

fn spawn_pause_menu(
    mut commands: Commands,
    mut pause_sel: ResMut<PauseSelection>,
    game_mode: Res<GameMode>,
    mut leaderboard_visible: ResMut<LeaderboardVisible>,
    last_run: Res<LastRunTime>,
) {
    pause_sel.0 = 0;
    leaderboard_visible.visible = false;
    leaderboard_visible.cached_time = last_run.0;
    rebuild_pause_menu_spawn(&mut commands, pause_sel.0, &game_mode);
}

fn pause_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut pause_sel: ResMut<PauseSelection>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut commands: Commands,
    existing_pause: Query<Entity, With<PauseUi>>,
    existing_lb: Query<Entity, With<LeaderboardUi>>,
    cleanup_query: Query<Entity, Or<(With<LevelEntity>, With<Player>)>>,
    game_mode: Res<GameMode>,
    mut leaderboard_visible: ResMut<LeaderboardVisible>,
    mut lb_selection: ResMut<LeaderboardSelection>,
    local_leaderboard: Res<Leaderboard>,
    online_leaderboard: Res<OnlineLeaderboard>,
    (mut replay_data, mut pending_replay): (ResMut<ReplayData>, ResMut<PendingReplayFetch>),
    replay_status: Res<ReplayFetchStatus>,
    (mut zen_leaderboard, zen_run, mut sfx): (ResMut<ZenLeaderboard>, Res<ZenRun>, MessageWriter<SfxEvent>),
) {
    let gamepad = gamepads.iter().next();

    let gp_up = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadUp));
    let gp_down = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadDown));

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || gp_up;
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || gp_down;

    // If leaderboard is open, handle leaderboard navigation
    if leaderboard_visible.visible {
        // In zen mode, leaderboard is view-only (no selection/replay)
        if *game_mode == GameMode::Zen {
            let gp_back = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East));
            if keys.just_pressed(KeyCode::Escape) || gp_back {
                leaderboard_visible.visible = false;
                for entity in &existing_lb {
                    commands.entity(entity).despawn();
                }
            }
            return;
        }

        let cached_time = leaderboard_visible.cached_time;
        let use_online = !online_leaderboard.entries.is_empty();
        let entry_count = if use_online {
            online_leaderboard.entries.len()
        } else {
            local_leaderboard.entries.len()
        };

        if entry_count > 0 {
            let max_idx = entry_count.saturating_sub(1);
            let mut changed = false;
            if up && lb_selection.0 > 0 {
                lb_selection.0 -= 1;
                changed = true;
            }
            if down && lb_selection.0 < max_idx {
                lb_selection.0 += 1;
                changed = true;
            }
            if changed {
                sfx.write(SfxEvent::MenuTick);
                for entity in &existing_lb {
                    commands.entity(entity).despawn();
                }
                spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, lb_selection.0, &replay_status, cached_time, &game_mode, &zen_leaderboard);
            }

            // Confirm — start replay
            let gp_confirm = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
            if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
                sfx.write(SfxEvent::MenuDing);
                if !replay_status.loading {
                    if use_online {
                        pending_replay.0 = Some(lb_selection.0);
                        for entity in &existing_lb {
                            commands.entity(entity).despawn();
                        }
                        spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, lb_selection.0, &replay_status, cached_time, &game_mode, &zen_leaderboard);
                    } else {
                        let entry = &local_leaderboard.entries[lb_selection.0];
                        replay_data.frames = entry.inputs.clone();
                        replay_data.seed = entry.seed;
                        replay_data.frame_index = 0;
                        next_state.set(GamePhase::Replaying);
                    }
                }
                return;
            }
        }

        // Close leaderboard (back to pause menu)
        let gp_back = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East));
        if keys.just_pressed(KeyCode::Escape) || gp_back {
            leaderboard_visible.visible = false;
            for entity in &existing_lb {
                commands.entity(entity).despawn();
            }
        }
        return;
    }

    // Normal pause menu navigation
    let max_idx = pause_menu_options(&game_mode).len().saturating_sub(1);

    if up && pause_sel.0 > 0 {
        pause_sel.0 -= 1;
        sfx.write(SfxEvent::MenuTick);
        rebuild_pause_menu(&mut commands, &existing_pause, pause_sel.0, &game_mode);
    }
    if down && pause_sel.0 < max_idx {
        pause_sel.0 += 1;
        sfx.write(SfxEvent::MenuTick);
        rebuild_pause_menu(&mut commands, &existing_pause, pause_sel.0, &game_mode);
    }

    // Resume on Escape
    let gp_back = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East) || g.just_pressed(GamepadButton::Start));
    if keys.just_pressed(KeyCode::Escape) || gp_back {
        next_state.set(GamePhase::Playing);
        return;
    }

    let gp_confirm = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
        sfx.write(SfxEvent::MenuDing);
        let options = pause_menu_options(&game_mode);
        let label = options[pause_sel.0];
        match label {
            "Resume" => {
                next_state.set(GamePhase::Playing);
            }
            "Leaderboard" => {
                leaderboard_visible.visible = true;
                lb_selection.0 = 0;
                spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, lb_selection.0, &replay_status, leaderboard_visible.cached_time, &game_mode, &zen_leaderboard);
            }
            "Title Screen" => {
                if *game_mode == GameMode::Zen && zen_run.max_height > 0.0 {
                    zen_leaderboard.add_height(zen_run.max_height);
                }
                for entity in &cleanup_query {
                    commands.entity(entity).despawn();
                }
                next_state.set(GamePhase::TitleScreen);
            }
            _ => {}
        }
    }
}

fn pause_menu_options(game_mode: &GameMode) -> Vec<&'static str> {
    match game_mode {
        GameMode::Levels => vec!["Resume", "Leaderboard", "Title Screen"],
        GameMode::Zen => vec!["Resume", "Leaderboard", "Title Screen"],
    }
}

fn rebuild_pause_menu(
    commands: &mut Commands,
    existing: &Query<Entity, With<PauseUi>>,
    selected: usize,
    game_mode: &GameMode,
) {
    for entity in existing {
        commands.entity(entity).despawn();
    }
    rebuild_pause_menu_spawn(commands, selected, game_mode);
}

fn rebuild_pause_menu_spawn(commands: &mut Commands, selected: usize, game_mode: &GameMode) {
    let options = pause_menu_options(game_mode);

    commands
        .spawn((
            PauseUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Start,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.15, 0.95)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.7, 1.0)),
                    ));

                    for (i, label) in options.iter().enumerate() {
                        let is_selected = i == selected;
                        let color = if is_selected {
                            Color::srgb(1.0, 1.0, 0.3)
                        } else {
                            Color::srgb(0.8, 0.8, 0.8)
                        };
                        let prefix = if is_selected { "> " } else { "  " };
                        panel.spawn((
                            Text::new(format!("{}{}", prefix, label)),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(color),
                        ));
                    }
                });
        });
}

// --- Level complete ---

fn spawn_level_complete(
    mut commands: Commands,
    mut sel: ResMut<LevelCompleteSelection>,
    timer: Res<SpeedrunTimer>,
    game_mode: Res<GameMode>,
    current_level: Res<CurrentLevel>,
    online_leaderboard: Res<OnlineLeaderboard>,
    player_name: Option<Res<PlayerName>>,
    mut name_prompt: ResMut<ScoreNamePrompt>,
    mut deferred: ResMut<DeferredSubmission>,
    mut pending: ResMut<PendingSubmission>,
    mut kb_cursor: ResMut<KeyboardCursor>,
) {
    let has_next = *game_mode == GameMode::Levels && current_level.0 + 1 < LEVEL_ORDER.len();
    // Default to "Restart"
    sel.0 = if has_next { 1 } else { 0 };

    let is_wr = timer.final_time.is_some_and(|t| {
        if online_leaderboard.entries.is_empty() {
            true
        } else {
            t < online_leaderboard.entries[0].time
        }
    });

    let is_top5 = *game_mode == GameMode::Levels && timer.final_time.is_some_and(|t| {
        online_leaderboard.entries.len() < 5
            || t < online_leaderboard.entries.last().map(|e| e.time).unwrap_or(f32::MAX)
    });

    if is_top5 {
        let current_name = player_name.map(|n| n.0.clone()).unwrap_or_default();
        name_prompt.active = true;
        name_prompt.buffer = current_name;
        kb_cursor.row = 3;
        kb_cursor.col = 7;
    } else {
        name_prompt.active = false;
        name_prompt.buffer.clear();
        // Not top 5 — submit immediately
        if let Some(data) = deferred.0.take() {
            pending.0 = Some(data);
        }
    }

    rebuild_level_complete_spawn(&mut commands, sel.0, timer.final_time, has_next, is_wr, &name_prompt, &kb_cursor);
}

fn level_complete_options(has_next: bool) -> Vec<&'static str> {
    if has_next {
        vec!["Next Level", "Restart", "Title Screen"]
    } else {
        vec!["Restart", "Title Screen"]
    }
}

fn rebuild_level_complete_spawn(
    commands: &mut Commands,
    selected: usize,
    final_time: Option<f32>,
    has_next: bool,
    is_wr: bool,
    name_prompt: &ScoreNamePrompt,
    kb_cursor: &KeyboardCursor,
) {
    let time = final_time.unwrap_or(0.0);
    let minutes = (time / 60.0) as u32;
    let seconds = time % 60.0;

    commands
        .spawn((
            LevelCompleteUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.08, 0.15, 0.95)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("LEVEL COMPLETE"),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.2, 0.9, 0.3)),
                    ));

                    if is_wr {
                        panel.spawn((
                            RainbowText,
                            Text::new("WORLD RECORD"),
                            TextFont {
                                font_size: 32.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    }

                    panel.spawn((
                        Text::new(format!("{:02}:{:06.3}", minutes, seconds)),
                        TextFont {
                            font_size: 40.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    if name_prompt.active {
                        panel.spawn((
                            Text::new("TOP 5! Enter name:"),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.8, 0.3)),
                        ));

                        let display = if name_prompt.buffer.is_empty() {
                            "_".to_string()
                        } else {
                            format!("{}_", name_prompt.buffer)
                        };
                        panel.spawn((
                            Text::new(display),
                            TextFont {
                                font_size: 28.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));

                        spawn_keyboard_grid(panel, kb_cursor);
                    } else {
                        let options = level_complete_options(has_next);

                        panel.spawn(Node {
                            height: Val::Px(8.0),
                            ..default()
                        });

                        for (i, label) in options.iter().enumerate() {
                            let is_selected = i == selected;
                            let color = if is_selected {
                                Color::srgb(1.0, 1.0, 0.3)
                            } else {
                                Color::srgb(0.8, 0.8, 0.8)
                            };
                            let prefix = if is_selected { "> " } else { "  " };
                            panel.spawn((
                                Text::new(format!("{}{}", prefix, label)),
                                TextFont {
                                    font_size: 24.0,
                                    ..default()
                                },
                                TextColor(color),
                            ));
                        }
                    }
                });
        });
}

fn level_complete_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut sel: ResMut<LevelCompleteSelection>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut commands: Commands,
    existing: Query<Entity, With<LevelCompleteUi>>,
    cleanup_query: Query<Entity, Or<(With<LevelEntity>, With<Player>)>>,
    game_mode: Res<GameMode>,
    mut current_level: ResMut<CurrentLevel>,
    timer: Res<SpeedrunTimer>,
    online_leaderboard: Res<OnlineLeaderboard>,
    mut name_prompt: ResMut<ScoreNamePrompt>,
    mut deferred: ResMut<DeferredSubmission>,
    (mut pending, mut sfx): (ResMut<PendingSubmission>, MessageWriter<SfxEvent>),
    mut kb_cursor: ResMut<KeyboardCursor>,
) {
    let gamepad = gamepads.iter().next();
    let has_next = *game_mode == GameMode::Levels && current_level.0 + 1 < LEVEL_ORDER.len();
    let is_wr = timer.final_time.is_some_and(|t| {
        if online_leaderboard.entries.is_empty() {
            true
        } else {
            t < online_leaderboard.entries[0].time
        }
    });

    // Name entry mode
    if name_prompt.active {
        let mut changed = false;

        if keys.just_pressed(KeyCode::Backspace) && !name_prompt.buffer.is_empty() {
            name_prompt.buffer.pop();
            changed = true;
        }

        for key in keys.get_just_pressed() {
            if name_prompt.buffer.len() >= MAX_NAME_LEN {
                break;
            }
            let ch = match key {
                KeyCode::KeyA => Some('A'), KeyCode::KeyB => Some('B'),
                KeyCode::KeyC => Some('C'), KeyCode::KeyD => Some('D'),
                KeyCode::KeyE => Some('E'), KeyCode::KeyF => Some('F'),
                KeyCode::KeyG => Some('G'), KeyCode::KeyH => Some('H'),
                KeyCode::KeyI => Some('I'), KeyCode::KeyJ => Some('J'),
                KeyCode::KeyK => Some('K'), KeyCode::KeyL => Some('L'),
                KeyCode::KeyM => Some('M'), KeyCode::KeyN => Some('N'),
                KeyCode::KeyO => Some('O'), KeyCode::KeyP => Some('P'),
                KeyCode::KeyQ => Some('Q'), KeyCode::KeyR => Some('R'),
                KeyCode::KeyS => Some('S'), KeyCode::KeyT => Some('T'),
                KeyCode::KeyU => Some('U'), KeyCode::KeyV => Some('V'),
                KeyCode::KeyW => Some('W'), KeyCode::KeyX => Some('X'),
                KeyCode::KeyY => Some('Y'), KeyCode::KeyZ => Some('Z'),
                KeyCode::Digit0 => Some('0'), KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'), KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'), KeyCode::Digit5 => Some('5'),
                KeyCode::Digit6 => Some('6'), KeyCode::Digit7 => Some('7'),
                KeyCode::Digit8 => Some('8'), KeyCode::Digit9 => Some('9'),
                _ => None,
            };
            if let Some(c) = ch {
                name_prompt.buffer.push(c);
                changed = true;
            }
        }

        // Confirm name
        if keys.just_pressed(KeyCode::Enter) && !name_prompt.buffer.is_empty() {
            let name = name_prompt.buffer.clone();
            username::save_name(&name);
            commands.insert_resource(PlayerName(name));
            // Submit the deferred score
            if let Some(data) = deferred.0.take() {
                pending.0 = Some(data);
            }
            name_prompt.active = false;
            // Rebuild UI to show menu options
            for entity in &existing {
                commands.entity(entity).despawn();
            }
            rebuild_level_complete_spawn(&mut commands, sel.0, timer.final_time, has_next, is_wr, &name_prompt, &kb_cursor);
            return;
        }

        // Gamepad input via on-screen keyboard
        if let Some(gp) = gamepad {
            let (action, moved) = keyboard_gamepad_input(gp, &mut kb_cursor);
            match action {
                KeyboardAction::Type(c) => {
                    if name_prompt.buffer.len() < MAX_NAME_LEN {
                        name_prompt.buffer.push(c);
                        changed = true;
                    }
                }
                KeyboardAction::Delete => {
                    if !name_prompt.buffer.is_empty() {
                        name_prompt.buffer.pop();
                        changed = true;
                    }
                }
                KeyboardAction::Confirm => {
                    if !name_prompt.buffer.is_empty() {
                        let name = name_prompt.buffer.clone();
                        username::save_name(&name);
                        commands.insert_resource(PlayerName(name));
                        if let Some(data) = deferred.0.take() {
                            pending.0 = Some(data);
                        }
                        name_prompt.active = false;
                        for entity in &existing {
                            commands.entity(entity).despawn();
                        }
                        rebuild_level_complete_spawn(&mut commands, sel.0, timer.final_time, has_next, is_wr, &name_prompt, &kb_cursor);
                        return;
                    }
                }
                KeyboardAction::None => {}
            }
            if moved { changed = true; }
        }

        if changed {
            for entity in &existing {
                commands.entity(entity).despawn();
            }
            rebuild_level_complete_spawn(&mut commands, sel.0, timer.final_time, has_next, is_wr, &name_prompt, &kb_cursor);
        }
        return;
    }

    // Normal menu navigation
    let options = level_complete_options(has_next);
    let max_idx = options.len().saturating_sub(1);

    let gp_up = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadUp));
    let gp_down = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::DPadDown));

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || gp_up;
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || gp_down;

    let mut changed = false;
    if up && sel.0 > 0 {
        sel.0 -= 1;
        changed = true;
    }
    if down && sel.0 < max_idx {
        sel.0 += 1;
        changed = true;
    }

    if changed {
        sfx.write(SfxEvent::MenuTick);
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        rebuild_level_complete_spawn(&mut commands, sel.0, timer.final_time, has_next, is_wr, &name_prompt, &kb_cursor);
    }

    let gp_confirm = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
        sfx.write(SfxEvent::MenuDing);
        let label = options[sel.0];
        match label {
            "Next Level" => {
                current_level.advance();
                next_state.set(GamePhase::Transitioning);
            }
            "Restart" => {
                next_state.set(GamePhase::Transitioning);
            }
            "Title Screen" => {
                for entity in &cleanup_query {
                    commands.entity(entity).despawn();
                }
                next_state.set(GamePhase::TitleScreen);
            }
            _ => {}
        }
    }
}

// --- Timer ---

fn reset_timer(mut timer: ResMut<SpeedrunTimer>) {
    timer.elapsed = 0.0;
    timer.running = false;
    timer.final_time = None;
}

fn start_timer_on_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut timer: ResMut<SpeedrunTimer>,
) {
    if timer.running || timer.final_time.is_some() {
        return;
    }

    let gamepad = gamepads.iter().next();
    let stick = gamepad.map(|g| g.left_stick()).unwrap_or(Vec2::ZERO);
    let has_stick = stick.x.abs() > 0.1 || stick.y.abs() > 0.1;
    let has_gp_button = gamepad.is_some_and(|g| {
        g.just_pressed(GamepadButton::South)
            || g.just_pressed(GamepadButton::LeftTrigger2)
    });

    let has_key = keys.any_pressed([
        KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::ArrowUp, KeyCode::ArrowDown,
        KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyW, KeyCode::KeyS,
        KeyCode::Space, KeyCode::ShiftLeft,
    ]);

    if has_stick || has_gp_button || has_key {
        timer.running = true;
    }
}

fn tick_timer(mut timer: ResMut<SpeedrunTimer>, time: Res<Time>) {
    if timer.running {
        timer.elapsed += time.delta_secs();
    }
}

fn update_timer_display(
    timer: Res<SpeedrunTimer>,
    mut query: Query<&mut Text, With<TimerText>>,
    game_mode: Res<GameMode>,
    mut zen_run: ResMut<ZenRun>,
    spawn_point: Res<SpawnPoint>,
    player_query: Query<&Transform, With<Player>>,
) {
    if *game_mode == GameMode::Zen {
        let player_height = player_query
            .iter()
            .next()
            .map(|tf| (tf.translation.y - spawn_point.0.y).max(0.0))
            .unwrap_or(0.0);
        let current_height = player_height / 16.0;
        zen_run.max_height = zen_run.max_height.max(current_height);
        let text = format!("{}m", current_height as u32);
        for mut t in &mut query {
            **t = text.clone();
        }
    } else {
        let elapsed = timer.final_time.unwrap_or(timer.elapsed);
        let minutes = (elapsed / 60.0) as u32;
        let seconds = elapsed % 60.0;
        let text = format!("{:02}:{:06.3}", minutes, seconds);
        for mut t in &mut query {
            **t = text.clone();
        }
    }
}

/// On level complete, pin the corner timer to the authoritative `final_time`
/// so it always matches the completion menu. (The live display stops updating
/// when we leave Playing, which could otherwise leave it a frame stale.)
fn freeze_timer_display(
    timer: Res<SpeedrunTimer>,
    mut query: Query<&mut Text, With<TimerText>>,
) {
    let Some(elapsed) = timer.final_time else {
        return;
    };
    let minutes = (elapsed / 60.0) as u32;
    let seconds = elapsed % 60.0;
    let text = format!("{:02}:{:06.3}", minutes, seconds);
    for mut t in &mut query {
        **t = text.clone();
    }
}

// --- HUD ---

fn spawn_hud_if_missing(
    commands: Commands,
    existing: Query<Entity, With<HudUi>>,
) {
    if existing.is_empty() {
        spawn_hud(commands);
    }
}

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                TimerText,
                Text::new("00:00.000"),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

// --- Rainbow ---

fn animate_rainbow(
    time: Res<Time>,
    mut query: Query<&mut TextColor, With<RainbowText>>,
) {
    let t = time.elapsed_secs() * 2.0;
    let r = (t.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let g = ((t + 2.094).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let b = ((t + 4.189).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    for mut color in &mut query {
        color.0 = Color::srgb(r, g, b);
    }
}

// --- Leaderboard ---

fn spawn_leaderboard(
    commands: &mut Commands,
    local_leaderboard: &Leaderboard,
    online_leaderboard: &OnlineLeaderboard,
    selected: usize,
    replay_status: &ReplayFetchStatus,
    last_time: Option<f32>,
    game_mode: &GameMode,
    zen_leaderboard: &ZenLeaderboard,
) {
    if *game_mode == GameMode::Zen {
        spawn_zen_leaderboard(commands, zen_leaderboard);
        return;
    }

    let use_online = !online_leaderboard.entries.is_empty();

    // Compute placement for last run among all personal runs
    let placement = last_time.map(|t| {
        let pos = local_leaderboard.all_times.partition_point(|&lt| lt < t);
        pos + 1
    });

    commands
        .spawn((
            LeaderboardUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexStart,
                    column_gap: Val::Px(24.0),
                    ..default()
                })
                .with_children(|row| {
                    // Left panel: leaderboard entries
                    row.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Start,
                            padding: UiRect::all(Val::Px(24.0)),
                            row_gap: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.9)),
                    ))
                    .with_children(|panel| {
                        let header = match &online_leaderboard.status {
                            NetStatus::Fetching => "LEADERBOARD  [Fetching...]",
                            NetStatus::Error(_) => "LEADERBOARD  [Offline]",
                            _ => "LEADERBOARD",
                        };
                        panel.spawn((
                            Text::new(header),
                            TextFont {
                                font_size: 32.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.4, 0.7, 1.0)),
                        ));

                        if use_online {
                            for (i, entry) in online_leaderboard.entries.iter().enumerate() {
                                let minutes = (entry.time / 60.0) as u32;
                                let seconds = entry.time % 60.0;
                                let is_selected = i == selected;
                                let color = if is_selected {
                                    Color::srgb(1.0, 1.0, 0.3)
                                } else if i == 0 {
                                    Color::srgb(0.2, 0.9, 0.3)
                                } else {
                                    Color::srgb(0.8, 0.8, 0.8)
                                };
                                let prefix = if is_selected { "> " } else { "  " };
                                panel.spawn((
                                    LeaderboardRow(i),
                                    Text::new(format!(
                                        "{}#{}  {:02}:{:06.3}  {}",
                                        prefix,
                                        i + 1,
                                        minutes,
                                        seconds,
                                        entry.name,
                                    )),
                                    TextFont {
                                        font_size: 20.0,
                                        ..default()
                                    },
                                    TextColor(color),
                                ));
                            }
                        } else if !local_leaderboard.entries.is_empty() {
                            for (i, entry) in local_leaderboard.entries.iter().enumerate() {
                                let minutes = (entry.time / 60.0) as u32;
                                let seconds = entry.time % 60.0;
                                let is_selected = i == selected;
                                let color = if is_selected {
                                    Color::srgb(1.0, 1.0, 0.3)
                                } else if i == 0 {
                                    Color::srgb(0.2, 0.9, 0.3)
                                } else {
                                    Color::srgb(0.8, 0.8, 0.8)
                                };
                                let prefix = if is_selected { "> " } else { "  " };
                                panel.spawn((
                                    LeaderboardRow(i),
                                    Text::new(format!(
                                        "{}#{}  {:02}:{:06.3}",
                                        prefix,
                                        i + 1,
                                        minutes,
                                        seconds,
                                    )),
                                    TextFont {
                                        font_size: 20.0,
                                        ..default()
                                    },
                                    TextColor(color),
                                ));
                            }
                        } else {
                            panel.spawn((
                                Text::new("No times recorded yet"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                            ));
                        }

                        if replay_status.loading {
                            panel.spawn((
                                Text::new("Loading replay..."),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.8, 0.3)),
                                Node {
                                    margin: UiRect::top(Val::Px(8.0)),
                                    ..default()
                                },
                            ));
                        }

                        panel.spawn((
                            Text::new("Up/Down: Select  |  A/Space: Replay  |  Esc/B: Back"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.5, 0.5, 0.5)),
                            Node {
                                margin: UiRect::top(Val::Px(16.0)),
                                ..default()
                            },
                        ));
                    });

                    // Right panel: last run info
                    if let Some(time) = last_time {
                        let minutes = (time / 60.0) as u32;
                        let seconds = time % 60.0;
                        let place = placement.unwrap_or(0);
                        let suffix = match place {
                            1 => "st",
                            2 => "nd",
                            3 => "rd",
                            _ => "th",
                        };

                        row.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(24.0)),
                                row_gap: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.9)),
                        ))
                        .with_children(|panel| {
                            panel.spawn((
                                Text::new("YOUR RUN"),
                                TextFont {
                                    font_size: 24.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.4, 0.7, 1.0)),
                            ));

                            panel.spawn((
                                Text::new(format!("{:02}:{:06.3}", minutes, seconds)),
                                TextFont {
                                    font_size: 32.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));

                            panel.spawn((
                                Text::new(format!("{}{}", place, suffix)),
                                TextFont {
                                    font_size: 28.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.8, 0.3)),
                            ));
                        });
                    }
                });
        });
}

fn spawn_zen_leaderboard(
    commands: &mut Commands,
    zen_leaderboard: &ZenLeaderboard,
) {
    commands
        .spawn((
            LeaderboardUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Start,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.9)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("BEST HEIGHTS"),
                        TextFont {
                            font_size: 32.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.7, 1.0)),
                    ));

                    if zen_leaderboard.heights.is_empty() {
                        panel.spawn((
                            Text::new("No heights recorded yet"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        ));
                    } else {
                        for (i, height) in zen_leaderboard.heights.iter().enumerate() {
                            let color = if i == 0 {
                                Color::srgb(0.2, 0.9, 0.3)
                            } else {
                                Color::srgb(0.8, 0.8, 0.8)
                            };
                            panel.spawn((
                                Text::new(format!("  #{}  {}m", i + 1, *height as u32)),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(color),
                            ));
                        }
                    }

                    panel.spawn((
                        Text::new("Esc/B: Back"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        Node {
                            margin: UiRect::top(Val::Px(16.0)),
                            ..default()
                        },
                    ));
                });
        });
}
