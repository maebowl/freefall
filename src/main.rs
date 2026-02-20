mod camera;
mod player;
mod walls;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::*;

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
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: ".".into(),
                    ..default()
                }),
        )
        .add_plugins(LdtkPlugin)
        .add_plugins(PhysicsPlugins::default().with_length_unit(16.0))
        .insert_resource(Gravity(Vec2::NEG_Y * 600.0))
        .insert_resource(LevelSelection::index(0))
        .add_plugins((
            player::PlayerPlugin,
            walls::WallPlugin,
            camera::CameraPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, close_on_escape)
        .run();
}

fn close_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 0.5,
            ..OrthographicProjection::default_2d()
        }),
    ));
    commands.spawn(LdtkWorldBundle {
        ldtk_handle: asset_server.load("freefall.ldtk").into(),
        ..default()
    });
}
