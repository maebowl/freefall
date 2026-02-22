use avian2d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use crate::level::{merge_grid_to_rects, Checkpoint, LevelEntity};
use crate::walls::Wall;

const TILE: f32 = 16.0;
const LEVEL_PX: f32 = 640.0;

pub const LEVEL_ORDER: &[&str] = &["Level_1", "Level_2"];

#[derive(Resource)]
pub struct CurrentLevel(pub usize);

impl Default for CurrentLevel {
    fn default() -> Self {
        Self(0)
    }
}

impl CurrentLevel {
    pub fn name(&self) -> &'static str {
        LEVEL_ORDER[self.0]
    }

    /// Advance to next level. Returns true if there is a next level.
    pub fn advance(&mut self) -> bool {
        if self.0 + 1 < LEVEL_ORDER.len() {
            self.0 += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Deserialize)]
struct LevelData {
    layers: Vec<String>,
    entities: Entities,
}

#[derive(Deserialize)]
struct Entities {
    #[serde(default)]
    #[serde(rename = "PlayerSpawn")]
    player_spawn: Vec<EntityEntry>,
    #[serde(default)]
    #[serde(rename = "Checkpoint")]
    checkpoint: Vec<EntityEntry>,
}

#[derive(Deserialize)]
struct EntityEntry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn level_data(name: &str) -> (&'static str, &'static str) {
    match name {
        "Level_1" => (
            include_str!("../assets/levels/Level_1/data.json"),
            include_str!("../assets/levels/Level_1/Walls.csv"),
        ),
        "Level_2" => (
            include_str!("../assets/levels/Level_2/data.json"),
            include_str!("../assets/levels/Level_2/Walls.csv"),
        ),
        _ => panic!("Unknown level: {name}"),
    }
}

/// Convert LDtk y-down position (top-left origin) to Bevy y-up center position.
fn ldtk_to_bevy(x: f32, y: f32, w: f32, h: f32) -> Vec2 {
    Vec2::new(x + w / 2.0, LEVEL_PX - y - h / 2.0)
}

pub fn build_ldtk_level(commands: &mut Commands, asset_server: &AssetServer, level_name: &str) -> Vec2 {
    let (data_str, csv_str) = level_data(level_name);

    let data: LevelData = serde_json::from_str(data_str).expect("Failed to parse LDtk data.json");

    // Parse Walls.csv into a bool grid
    let mut grid: Vec<Vec<bool>> = Vec::new();
    for line in csv_str.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Vec<bool> = line
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim() == "1")
            .collect();
        grid.push(row);
    }

    // Flip rows: LDtk row 0 = top, Bevy row 0 = bottom
    grid.reverse();

    let rects = merge_grid_to_rects(&grid);

    // Spawn level root
    let level_entity = commands
        .spawn((
            LevelEntity,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // Level center for sprite positioning
    let center = Vec3::new(LEVEL_PX / 2.0, LEVEL_PX / 2.0, 0.0);

    commands.entity(level_entity).with_children(|parent| {
        // Spawn layer images behind gameplay sprites
        for (i, layer_file) in data.layers.iter().enumerate() {
            let path = format!("levels/{}/{}", level_name, layer_file);
            parent.spawn((
                Sprite::from_image(asset_server.load(&path)),
                Transform::from_translation(center + Vec3::new(0.0, 0.0, -10.0 + i as f32)),
            ));
        }

        // Spawn invisible wall colliders (visuals come from layer PNGs)
        for rect in &rects {
            let w = (rect.right - rect.left + 1) as f32 * TILE;
            let h = (rect.top - rect.bottom + 1) as f32 * TILE;
            let cx = (rect.left + rect.right + 1) as f32 * TILE / 2.0;
            let cy = (rect.bottom + rect.top + 1) as f32 * TILE / 2.0;

            parent.spawn((
                Wall,
                Collider::rectangle(w, h),
                RigidBody::Static,
                Friction::new(0.0),
                Transform::from_xyz(cx, cy, 0.0),
                Visibility::Hidden,
            ));
        }

        // Spawn checkpoint sensors
        for cp in &data.entities.checkpoint {
            let pos = ldtk_to_bevy(cp.x, cp.y, cp.width, cp.height);
            parent.spawn((
                Checkpoint,
                Collider::rectangle(cp.width, cp.height),
                Sensor,
                RigidBody::Static,
                Sprite::from_color(
                    Color::srgba(0.2, 0.9, 0.3, 0.4),
                    Vec2::new(cp.width, cp.height),
                ),
                Transform::from_xyz(pos.x, pos.y, 5.0),
            ));
        }
    });

    // Return spawn point from PlayerSpawn entity
    let sp = if let Some(ps) = data.entities.player_spawn.first() {
        ldtk_to_bevy(ps.x, ps.y, ps.width, ps.height)
    } else {
        Vec2::new(LEVEL_PX / 2.0, TILE * 3.0)
    };

    info!("Loaded LDtk level {} (spawn at {:?})", level_name, sp);

    sp
}
