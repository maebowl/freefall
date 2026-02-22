use bevy::prelude::*;

use crate::level::{GameMode, GamePhase};
use crate::net::{NetStatus, OnlineLeaderboard, PendingReplayFetch, ReplayFetchStatus};
use crate::replay::{FrameInput, ReplayData};

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
}

impl Leaderboard {
    pub fn add_entry(&mut self, time: f32, seed: u64, inputs: Vec<FrameInput>) {
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
struct LeaderboardVisible(bool);

#[derive(Resource, Default)]
struct LeaderboardSelection(usize);

// --- Components ---

#[derive(Component)]
struct TitleScreenUi;

#[derive(Component)]
struct HudUi;

#[derive(Component)]
struct TimerText;

#[derive(Component)]
struct LeaderboardUi;

#[derive(Component)]
struct LeaderboardRow(usize);

// --- Plugin ---

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpeedrunTimer>()
            .init_resource::<Leaderboard>()
            .init_resource::<LeaderboardVisible>()
            .init_resource::<LeaderboardSelection>()
            // Title screen
            .add_systems(OnEnter(GamePhase::TitleScreen), spawn_title_screen)
            .add_systems(OnExit(GamePhase::TitleScreen), despawn_marked::<TitleScreenUi>)
            .add_systems(
                Update,
                title_screen_input.run_if(in_state(GamePhase::TitleScreen)),
            )
            // HUD + timer
            .add_systems(OnEnter(GamePhase::Playing), (spawn_hud, reset_timer))
            .add_systems(OnExit(GamePhase::Playing), (despawn_marked::<HudUi>, despawn_marked::<LeaderboardUi>, clear_leaderboard_visible))
            .add_systems(
                Update,
                (tick_timer, update_timer_display, toggle_leaderboard, navigate_leaderboard)
                    .run_if(in_state(GamePhase::Playing)),
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
    visible.0 = false;
}

// --- Title screen ---

fn spawn_title_screen(mut commands: Commands) {
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
            // Title
            parent.spawn((
                Text::new("FREEFALL"),
                TextFont {
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Mode selection
            parent.spawn((
                Text::new("1: Levels       2: Zen Mode"),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.8, 0.3)),
            ));

            // Start prompt
            parent.spawn((
                Text::new("Press Space or A for Levels"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));

            // Controls section
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    row_gap: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(40.0)),
                    ..default()
                })
                .with_children(|section| {
                    // Header
                    section.spawn((
                        Text::new("CONTROLS"),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.7, 1.0)),
                    ));

                    let controls = [
                        "Move          A/D or Left Stick",
                        "Jump          Space or A button",
                        "Dash          E or LT",
                        "Sprint        Shift or RT",
                        "Wall Jump     Jump while on wall",
                        "Regen Level   R or Select",
                        "Leaderboard   L or Menu",
                    ];

                    for line in &controls {
                        section.spawn((
                            Text::new(*line),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.8, 0.8)),
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
) {
    let gp_start = gamepads.iter().next().is_some_and(|g| {
        g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::Start)
    });

    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_start {
        *game_mode = GameMode::Levels;
        next_state.set(GamePhase::Generating);
    } else if keys.just_pressed(KeyCode::Digit2) {
        *game_mode = GameMode::Zen;
        next_state.set(GamePhase::Generating);
    }
}

// --- Timer ---

fn reset_timer(mut timer: ResMut<SpeedrunTimer>) {
    timer.elapsed = 0.0;
    timer.running = true;
    timer.final_time = None;
}

fn tick_timer(mut timer: ResMut<SpeedrunTimer>, time: Res<Time>) {
    if timer.running {
        timer.elapsed += time.delta_secs();
    }
}

fn update_timer_display(
    timer: Res<SpeedrunTimer>,
    mut query: Query<&mut Text, With<TimerText>>,
) {
    let elapsed = timer.final_time.unwrap_or(timer.elapsed);
    let minutes = (elapsed / 60.0) as u32;
    let seconds = elapsed % 60.0;
    let text = format!("{:02}:{:06.3}", minutes, seconds);
    for mut t in &mut query {
        **t = text.clone();
    }
}

// --- HUD ---

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

// --- Leaderboard ---

fn toggle_leaderboard(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut visible: ResMut<LeaderboardVisible>,
    mut commands: Commands,
    existing: Query<Entity, With<LeaderboardUi>>,
    local_leaderboard: Res<Leaderboard>,
    online_leaderboard: Res<OnlineLeaderboard>,
    mut selection: ResMut<LeaderboardSelection>,
    replay_status: Res<ReplayFetchStatus>,
    game_mode: Res<GameMode>,
) {
    // No leaderboard in Zen mode
    if *game_mode == GameMode::Zen {
        return;
    }

    let gp_toggle = gamepads
        .iter()
        .next()
        .is_some_and(|g| g.just_pressed(GamepadButton::Start));
    if keys.just_pressed(KeyCode::KeyL) || gp_toggle {
        visible.0 = !visible.0;
        if visible.0 {
            selection.0 = 0;
            spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, selection.0, &replay_status);
        } else {
            for entity in &existing {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn navigate_leaderboard(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    visible: Res<LeaderboardVisible>,
    local_leaderboard: Res<Leaderboard>,
    online_leaderboard: Res<OnlineLeaderboard>,
    mut selection: ResMut<LeaderboardSelection>,
    mut commands: Commands,
    existing: Query<Entity, With<LeaderboardUi>>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut replay_data: ResMut<ReplayData>,
    mut pending_replay: ResMut<PendingReplayFetch>,
    replay_status: Res<ReplayFetchStatus>,
) {
    if !visible.0 {
        return;
    }

    let use_online = !online_leaderboard.entries.is_empty();
    let entry_count = if use_online {
        online_leaderboard.entries.len()
    } else {
        local_leaderboard.entries.len()
    };

    if entry_count == 0 {
        return;
    }

    let gamepad = gamepads.iter().next();
    let max_idx = entry_count.saturating_sub(1);

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
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, selection.0, &replay_status);
    }

    // Confirm selection — start replay
    let gp_confirm = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || gp_confirm {
        if replay_status.loading {
            return; // Already fetching a replay
        }
        if use_online {
            // Fetch replay from server
            pending_replay.0 = Some(selection.0);
            // Rebuild UI to show loading state
            for entity in &existing {
                commands.entity(entity).despawn();
            }
            spawn_leaderboard(&mut commands, &local_leaderboard, &online_leaderboard, selection.0, &replay_status);
        } else {
            // Use local replay data directly
            let entry = &local_leaderboard.entries[selection.0];
            replay_data.frames = entry.inputs.clone();
            replay_data.seed = entry.seed;
            replay_data.frame_index = 0;
            next_state.set(GamePhase::Replaying);
        }
    }

    // Close with B or Escape
    let gp_close = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East));
    if keys.just_pressed(KeyCode::Escape) || gp_close {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_leaderboard(
    commands: &mut Commands,
    local_leaderboard: &Leaderboard,
    online_leaderboard: &OnlineLeaderboard,
    selected: usize,
    replay_status: &ReplayFetchStatus,
) {
    let use_online = !online_leaderboard.entries.is_empty();

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
                    // Header with status indicator
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
                        // Show online entries with names
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
                        // Fallback to local entries (no names)
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

                    // Loading indicator for replay fetch
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
                        Text::new("Up/Down: Select  |  A/Space: Replay  |  L: Close"),
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
