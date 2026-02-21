use std::path::PathBuf;

use bevy::prelude::*;

use crate::level::GamePhase;
use crate::net::PlayerName;

const MAX_NAME_LEN: usize = 16;

#[derive(Component)]
struct NameEntryUi;

#[derive(Resource, Default)]
struct NameBuffer(String);

pub struct UsernamePlugin;

impl Plugin for UsernamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NameBuffer>()
            .add_systems(OnEnter(GamePhase::NameEntry), check_saved_name)
            .add_systems(OnExit(GamePhase::NameEntry), despawn_name_entry_ui)
            .add_systems(
                Update,
                name_entry_input.run_if(in_state(GamePhase::NameEntry)),
            );
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".freefall").join("config.json")
}

fn load_saved_name() -> Option<String> {
    let path = config_path();
    let data = std::fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
    parsed.get("name")?.as_str().map(|s| s.to_string())
}

fn save_name(name: &str) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let data = serde_json::json!({ "name": name });
    let _ = std::fs::write(path, data.to_string());
}

fn check_saved_name(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut name_buf: ResMut<NameBuffer>,
) {
    if let Some(name) = load_saved_name() {
        if !name.is_empty() {
            commands.insert_resource(PlayerName(name));
            next_state.set(GamePhase::TitleScreen);
            return;
        }
    }
    // No saved name — show the entry UI
    name_buf.0.clear();
    spawn_name_entry_ui(&mut commands, "");
}

fn spawn_name_entry_ui(commands: &mut Commands, current: &str) {
    commands
        .spawn((
            NameEntryUi,
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
                Text::new("FREEFALL"),
                TextFont {
                    font_size: 60.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Enter your name:"),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.7, 1.0)),
            ));

            let display = if current.is_empty() {
                "_".to_string()
            } else {
                format!("{current}_")
            };
            parent.spawn((
                Text::new(display),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("A-Z, 0-9, Backspace  |  Enter to confirm"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
}

fn despawn_name_entry_ui(mut commands: Commands, query: Query<Entity, With<NameEntryUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn name_entry_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut name_buf: ResMut<NameBuffer>,
    mut next_state: ResMut<NextState<GamePhase>>,
    existing: Query<Entity, With<NameEntryUi>>,
) {
    let mut changed = false;

    // Backspace
    if keys.just_pressed(KeyCode::Backspace) && !name_buf.0.is_empty() {
        name_buf.0.pop();
        changed = true;
    }

    // Letter/number input
    for key in keys.get_just_pressed() {
        if name_buf.0.len() >= MAX_NAME_LEN {
            break;
        }
        let ch = match key {
            KeyCode::KeyA => Some('A'),
            KeyCode::KeyB => Some('B'),
            KeyCode::KeyC => Some('C'),
            KeyCode::KeyD => Some('D'),
            KeyCode::KeyE => Some('E'),
            KeyCode::KeyF => Some('F'),
            KeyCode::KeyG => Some('G'),
            KeyCode::KeyH => Some('H'),
            KeyCode::KeyI => Some('I'),
            KeyCode::KeyJ => Some('J'),
            KeyCode::KeyK => Some('K'),
            KeyCode::KeyL => Some('L'),
            KeyCode::KeyM => Some('M'),
            KeyCode::KeyN => Some('N'),
            KeyCode::KeyO => Some('O'),
            KeyCode::KeyP => Some('P'),
            KeyCode::KeyQ => Some('Q'),
            KeyCode::KeyR => Some('R'),
            KeyCode::KeyS => Some('S'),
            KeyCode::KeyT => Some('T'),
            KeyCode::KeyU => Some('U'),
            KeyCode::KeyV => Some('V'),
            KeyCode::KeyW => Some('W'),
            KeyCode::KeyX => Some('X'),
            KeyCode::KeyY => Some('Y'),
            KeyCode::KeyZ => Some('Z'),
            KeyCode::Digit0 => Some('0'),
            KeyCode::Digit1 => Some('1'),
            KeyCode::Digit2 => Some('2'),
            KeyCode::Digit3 => Some('3'),
            KeyCode::Digit4 => Some('4'),
            KeyCode::Digit5 => Some('5'),
            KeyCode::Digit6 => Some('6'),
            KeyCode::Digit7 => Some('7'),
            KeyCode::Digit8 => Some('8'),
            KeyCode::Digit9 => Some('9'),
            _ => None,
        };
        if let Some(c) = ch {
            name_buf.0.push(c);
            changed = true;
        }
    }

    // Confirm
    if keys.just_pressed(KeyCode::Enter) && !name_buf.0.is_empty() {
        let name = name_buf.0.clone();
        save_name(&name);
        commands.insert_resource(PlayerName(name));
        next_state.set(GamePhase::TitleScreen);
        return;
    }

    // Rebuild UI on change
    if changed {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        spawn_name_entry_ui(&mut commands, &name_buf.0);
    }
}
