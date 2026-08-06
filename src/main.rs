mod camera;
mod font;
mod ldtk;
mod level;
mod net;
mod pieces;
mod player;
mod replay;
mod sfx;
mod tutorial;
mod ui;
mod username;
mod walls;
mod zen_fx;

use avian2d::prelude::*;
use bevy::prelude::*;
// Only needed when embedding (wasm, or the `embed` feature); otherwise disk.
#[cfg(any(target_family = "wasm", feature = "embed"))]
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};

fn main() {
    let mut app = App::new();

    // Embed assets into the binary for wasm (always) and for standalone native
    // builds (the `embed` feature). Otherwise — including `cargo run` and
    // `cargo run --release` — assets load from disk, so edited level art shows up
    // on the next run instead of lagging a build behind the build.rs asset sync.
    #[cfg(any(target_family = "wasm", feature = "embed"))]
    app.add_plugins(EmbeddedAssetPlugin { mode: PluginMode::ReplaceDefault });

    // On the web, render into the <canvas id="game-canvas"> in index.html.
    // Ignored on native (no effect off wasm).
    #[cfg(target_family = "wasm")]
    let canvas = Some("#game-canvas".to_string());
    #[cfg(not(target_family = "wasm"))]
    let canvas = None;

    app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Freefall".into(),
                        resolution: (1280, 720).into(),
                        canvas,
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        // Interpolate rigid-body transforms between the 64 Hz physics steps so
        // the player renders smoothly at high/variable framerates (e.g. fullscreen)
        // instead of stepping against the smoothly-scrolling world.
        .add_plugins(
            PhysicsPlugins::default()
                .with_length_unit(16.0)
                .set(PhysicsInterpolationPlugin::interpolate_all()),
        )
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
            zen_fx::ZenFxPlugin,
            font::FontPlugin,
            tutorial::TutorialPlugin,
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
