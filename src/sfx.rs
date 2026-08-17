//! Sound effects. Any system fires a `SfxEvent`; this plays the matching clip
//! (menu tick/ding, dash whoosh).

use bevy::prelude::*;

#[derive(Message)]
pub enum SfxEvent {
    MenuTick,
    MenuDing,
    Dash,
}

#[derive(Resource)]
struct SfxHandles {
    menu_tick: Handle<AudioSource>,
    menu_ding: Handle<AudioSource>,
    dash: Handle<AudioSource>,
}

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SfxEvent>()
            .add_systems(Startup, load_sfx)
            .add_systems(PostUpdate, play_sfx);
    }
}

fn load_sfx(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(SfxHandles {
        menu_tick: asset_server.load("menu-tick.mp3"),
        menu_ding: asset_server.load("menu-ding.mp3"),
        dash: asset_server.load("dash.mp3"),
    });
}

fn play_sfx(mut commands: Commands, mut events: MessageReader<SfxEvent>, handles: Res<SfxHandles>) {
    for event in events.read() {
        let handle = match event {
            SfxEvent::MenuTick => handles.menu_tick.clone(),
            SfxEvent::MenuDing => handles.menu_ding.clone(),
            SfxEvent::Dash => handles.dash.clone(),
        };
        commands.spawn(AudioPlayer::new(handle));
    }
}
