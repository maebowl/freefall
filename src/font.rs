//! Fonts: the title ("FREEFALL", tagged [`TitleText`]) renders in Archivo Black;
//! all other text uses Source Sans Pro Black. A single system applies them to
//! text as it spawns, so individual text-spawning sites don't have to set a font.
//!
//! Both live in `assets/fonts/` and are SIL OFL — free to embed and distribute.

use bevy::prelude::*;

const TITLE_FONT: &str = "fonts/ArchivoBlack-Regular.ttf";
const BODY_FONT: &str = "fonts/SourceSansPro-Black.ttf";

/// Marks the game-title text so it renders in the title font.
#[derive(Component)]
pub struct TitleText;

#[derive(Resource)]
struct GameFonts {
    body: Handle<Font>,
    title: Handle<Font>,
}

pub struct FontPlugin;

impl Plugin for FontPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_fonts)
            // Set the font before UI text is measured, in the same frame the text
            // spawns — otherwise newly-spawned menu text is measured once in the
            // default font, causing a one-frame width flash when menus rebuild.
            .add_systems(
                PostUpdate,
                apply_font_to_new_text.before(bevy::ui::UiSystems::Content),
            );
    }
}

fn load_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameFonts {
        body: asset_server.load(BODY_FONT),
        title: asset_server.load(TITLE_FONT),
    });
}

/// Apply fonts to text as it spawns: the title font to [`TitleText`], the body
/// font to everything else.
fn apply_font_to_new_text(
    fonts: Res<GameFonts>,
    mut new_text: Query<(&mut TextFont, Has<TitleText>), Added<TextFont>>,
) {
    for (mut font, is_title) in &mut new_text {
        font.font = if is_title {
            fonts.title.clone()
        } else {
            fonts.body.clone()
        };
    }
}
