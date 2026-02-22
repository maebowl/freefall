use avian2d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use crate::level::{merge_grid_to_rects, Checkpoint, LevelEntity};
use crate::walls::Wall;

const TILE: f32 = 16.0;
const LEVEL_PX: f32 = 640.0;

#[derive(Deserialize)]
struct LevelData {
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

/// Convert LDtk y-down position (top-left origin) to Bevy y-up center position.
fn ldtk_to_bevy(x: f32, y: f32, w: f32, h: f32) -> Vec2 {
    Vec2::new(x + w / 2.0, LEVEL_PX - y - h / 2.0)
}

pub fn build_ldtk_level(commands: &mut Commands) -> Vec2 {
    // Load and parse data.json
    let data_str = include_str!("../assets/levels/Mosaic_demo/data.json");
    let data: LevelData = serde_json::from_str(data_str).expect("Failed to parse LDtk data.json");

    // Load and parse Walls.csv into a bool grid
    let csv_str = include_str!("../assets/levels/Mosaic_demo/Walls.csv");
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

    commands.entity(level_entity).with_children(|parent| {
        // Spawn wall colliders
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
                Sprite::from_color(
                    Color::srgb(0.55, 0.35, 0.2),
                    Vec2::new(w, h),
                ),
                Transform::from_xyz(cx, cy, 0.0),
            ));
        }

        // Spawn checkpoints
        for cp in &data.entities.checkpoint {
            let pos = ldtk_to_bevy(cp.x, cp.y, cp.width, cp.height);
            parent.spawn((
                Checkpoint,
                Collider::rectangle(cp.width, cp.height),
                Sensor,
                RigidBody::Static,
                Sprite::from_color(
                    Color::srgb(0.2, 0.9, 0.3),
                    Vec2::new(cp.width, cp.height),
                ),
                Transform::from_xyz(pos.x, pos.y, 1.0),
            ));
        }
    });

    // Return spawn point from PlayerSpawn entity
    let sp = if let Some(ps) = data.entities.player_spawn.first() {
        ldtk_to_bevy(ps.x, ps.y, ps.width, ps.height)
    } else {
        // Fallback: center bottom
        Vec2::new(LEVEL_PX / 2.0, TILE * 3.0)
    };

    info!("Loaded LDtk level Mosaic_demo (spawn at {:?})", sp);

    sp
}
