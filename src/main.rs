mod camera;
mod level;
mod player;
mod replay;
mod ui;
mod walls;

use avian2d::prelude::*;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Freefall".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(PhysicsPlugins::default().with_length_unit(16.0))
        .insert_resource(Gravity(Vec2::NEG_Y * 600.0))
        .add_plugins((
            level::LevelPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            ui::UiPlugin,
            replay::ReplayPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            close_on_escape.run_if(not(in_state(level::GamePhase::Replaying))),
        )
        .run();
}

fn close_on_escape(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<Entity, With<Window>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &windows {
            commands.entity(entity).despawn();
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 0.5,
            ..OrthographicProjection::default_2d()
        }),
    ));
}
