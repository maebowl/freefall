//! Tutorial prompts: a `TutorialText` LDtk entity that shows a hint near the
//! bottom of the screen while the player is touching it.
//!
//! The hint text comes straight from the LDtk `TutorialTextType` enum value,
//! with underscores read as spaces (`Press_XJUMPX_To_Jump`). Any word wrapped
//! in `X…X` — e.g. `XJUMPX`, `XDASHX` — is a *button token*: it's swapped for
//! the glyph of whatever input device was used last (SPACE on keyboard, A on an
//! Xbox pad, X on a PlayStation pad, …). Add a new prompt by adding an enum
//! value in LDtk; add a new button token by adding an arm to `action_glyph`.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::level::GamePhase;
use crate::player::Player;

/// The kind of input device the player most recently used. Drives which button
/// glyph the tutorial prompts show.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InputDevice {
    #[default]
    Keyboard,
    Xbox,
    PlayStation,
    /// A gamepad we couldn't brand — treated as Xbox-style (South = A).
    Generic,
}

/// Tracks the most recently used input device (see [`InputDevice`]).
#[derive(Resource, Default)]
pub struct LastInputDevice(pub InputDevice);

/// A trigger zone that shows `text` (a raw `TutorialTextType` enum value) while
/// the player overlaps it. Spawned from the LDtk `TutorialText` entity.
#[derive(Component)]
pub struct TutorialTrigger {
    pub text: String,
}

/// Root UI node of the on-screen tutorial prompt.
#[derive(Component)]
struct TutorialHud;

/// What the HUD is currently showing, so we only rebuild the text when the
/// trigger under the player or the input device actually changes.
#[derive(Resource, Default)]
struct ActiveTutorial(Option<(Entity, InputDevice)>);

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastInputDevice>()
            .init_resource::<ActiveTutorial>()
            // Track the input device everywhere (menus included).
            .add_systems(Update, track_input_device)
            .add_systems(
                Update,
                tutorial_trigger.run_if(in_state(GamePhase::Playing)),
            )
            .add_systems(OnExit(GamePhase::Playing), clear_tutorial);
    }
}

/// Watch for input activity and remember which device produced it.
fn track_input_device(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(&Gamepad, Option<&Name>)>,
    mut last: ResMut<LastInputDevice>,
) {
    if keys.get_just_pressed().next().is_some() {
        last.0 = InputDevice::Keyboard;
        return;
    }
    for (gp, name) in &gamepads {
        let button = gp.get_just_pressed().next().is_some();
        let stick = gp.left_stick().length() > 0.5 || gp.right_stick().length() > 0.5;
        let trigger = gp.get(GamepadButton::LeftTrigger2).unwrap_or(0.0) > 0.5
            || gp.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > 0.5;
        if button || stick || trigger {
            last.0 = classify_gamepad(gp, name.map(Name::as_str));
            return;
        }
    }
}

/// Guess a gamepad's brand from its USB vendor id (most reliable), then its
/// reported name, defaulting to Xbox-style layout for anything unknown.
fn classify_gamepad(gp: &Gamepad, name: Option<&str>) -> InputDevice {
    match gp.vendor_id() {
        Some(0x054C) => return InputDevice::PlayStation, // Sony
        Some(0x045E) => return InputDevice::Xbox,        // Microsoft
        _ => {}
    }
    if let Some(n) = name {
        let n = n.to_ascii_lowercase();
        if ["playstation", "dualshock", "dualsense", "ps4", "ps5", "sony"]
            .iter()
            .any(|k| n.contains(k))
        {
            return InputDevice::PlayStation;
        }
        if n.contains("xbox") || n.contains("xinput") {
            return InputDevice::Xbox;
        }
    }
    InputDevice::Generic
}

/// Turn a raw `TutorialTextType` value into display text: underscores become
/// spaces, and `X…X` button tokens become device-specific glyphs.
pub fn format_tutorial(raw: &str, device: InputDevice) -> String {
    raw.split('_')
        .map(|word| {
            match word.strip_prefix('X').and_then(|w| w.strip_suffix('X')) {
                Some(action) if !action.is_empty() => {
                    action_glyph(action, device).unwrap_or_else(|| action.to_string())
                }
                _ => word.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The button glyph for an action on a given device. Returns `None` for an
/// unknown action so the caller can fall back to the action's plain name.
fn action_glyph(action: &str, device: InputDevice) -> Option<String> {
    use InputDevice::*;
    let glyph = match (action, device) {
        ("JUMP", Keyboard) => "SPACE",
        ("JUMP", PlayStation) => "X",
        ("JUMP", Xbox | Generic) => "A",
        ("DASH", Keyboard) => "SHIFT",
        ("DASH", PlayStation) => "L2",
        ("DASH", Xbox | Generic) => "LT",
        _ => return None,
    };
    Some(glyph.to_string())
}

/// While the player overlaps a `TutorialTrigger`, show its prompt; otherwise
/// hide it. Rebuilds only when the trigger or the input device changes.
fn tutorial_trigger(
    mut commands: Commands,
    collisions: Collisions,
    player_q: Query<Entity, With<Player>>,
    triggers: Query<(Entity, &TutorialTrigger)>,
    last_device: Res<LastInputDevice>,
    mut active: ResMut<ActiveTutorial>,
    hud: Query<Entity, With<TutorialHud>>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };

    let overlapped = triggers
        .iter()
        .find(|(e, _)| collisions.contains(player, *e));

    let device = last_device.0;
    let desired = overlapped.map(|(e, _)| (e, device));

    if desired == active.0 {
        return; // Nothing changed — leave the current prompt as-is.
    }

    for e in &hud {
        commands.entity(e).despawn();
    }
    if let Some((entity, _)) = desired {
        // Safe: `overlapped` is Some exactly when `desired` is.
        let raw = &triggers.get(entity).unwrap().1.text;
        spawn_tutorial_hud(&mut commands, &format_tutorial(raw, device));
    }
    active.0 = desired;
}

/// Despawn the prompt and forget it when leaving play (pause, menu, replay).
fn clear_tutorial(
    mut commands: Commands,
    hud: Query<Entity, With<TutorialHud>>,
    mut active: ResMut<ActiveTutorial>,
) {
    for e in &hud {
        commands.entity(e).despawn();
    }
    active.0 = None;
}

fn spawn_tutorial_hud(commands: &mut Commands, text: &str) {
    commands
        .spawn((
            TutorialHud,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(40.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                ))
                .with_children(|panel| {
                    // Font is applied globally by font::apply_font_to_new_text.
                    panel.spawn((
                        Text::new(text),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}
