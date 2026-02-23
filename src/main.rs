mod camera;
mod ldtk;
mod level;
mod net;
mod pieces;
mod player;
mod replay;
mod sfx;
mod ui;
mod username;
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
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
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
            net::NetPlugin,
            username::UsernamePlugin,
            ldtk::LdtkPlugin,
            sfx::SfxPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
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
