use std::collections::{HashMap, HashSet};

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component)]
pub struct Wall;

#[derive(Clone, Debug, Default, Bundle, LdtkIntCell)]
pub struct WallBundle {
    wall: Wall,
}

pub struct WallPlugin;

impl Plugin for WallPlugin {
    fn build(&self, app: &mut App) {
        app.register_ldtk_int_cell::<WallBundle>(2)
            .add_systems(Update, spawn_wall_collision);
    }
}

/// Merges adjacent wall tiles into larger rectangle colliders to reduce physics load.
///
/// Algorithm:
/// 1. Collect wall tile positions per level
/// 2. Combine adjacent tiles into horizontal "plates" per row
/// 3. Merge plates vertically into rectangles
/// 4. Spawn a single collider per rectangle
pub fn spawn_wall_collision(
    mut commands: Commands,
    wall_query: Query<(&GridCoords, &ChildOf), Added<Wall>>,
    parent_query: Query<&ChildOf, Without<Wall>>,
    level_query: Query<(Entity, &LevelIid)>,
    ldtk_projects: Query<&LdtkProjectHandle>,
    ldtk_project_assets: Res<Assets<LdtkProject>>,
) {
    #[derive(Clone, Eq, PartialEq, Debug, Default, Hash)]
    struct Plate {
        left: i32,
        right: i32,
    }

    struct Rect {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    }

    // Collect wall locations grouped by level entity
    let mut level_to_wall_locations: HashMap<Entity, HashSet<GridCoords>> = HashMap::new();

    wall_query.iter().for_each(|(&grid_coords, child_of)| {
        // IntGrid tile → layer entity (parent) → level entity (grandparent)
        if let Ok(grandparent) = parent_query.get(child_of.parent()) {
            level_to_wall_locations
                .entry(grandparent.parent())
                .or_default()
                .insert(grid_coords);
        }
    });

    if wall_query.is_empty() {
        return;
    }

    level_query.iter().for_each(|(level_entity, level_iid)| {
        let Some(level_walls) = level_to_wall_locations.get(&level_entity) else {
            return;
        };

        let ldtk_project = ldtk_project_assets
            .get(ldtk_projects.single().unwrap())
            .expect("Project should be loaded if level has spawned");

        let level = ldtk_project
            .as_standalone()
            .get_loaded_level_by_iid(&level_iid.to_string())
            .expect("Spawned level should exist in LDtk project");

        let LayerInstance {
            c_wid: width,
            c_hei: height,
            grid_size,
            ..
        } = level.layer_instances()[0];

        // Stage 1: Create horizontal plates per row
        let mut plate_stack: Vec<Vec<Plate>> = Vec::new();

        for y in 0..height {
            let mut row_plates: Vec<Plate> = Vec::new();
            let mut plate_start = None;

            for x in 0..width + 1 {
                match (plate_start, level_walls.contains(&GridCoords { x, y })) {
                    (Some(s), false) => {
                        row_plates.push(Plate {
                            left: s,
                            right: x - 1,
                        });
                        plate_start = None;
                    }
                    (None, true) => plate_start = Some(x),
                    _ => (),
                }
            }

            plate_stack.push(row_plates);
        }

        // Stage 2: Merge plates vertically into rectangles
        let mut rect_builder: HashMap<Plate, Rect> = HashMap::new();
        let mut prev_row: Vec<Plate> = Vec::new();
        let mut wall_rects: Vec<Rect> = Vec::new();

        // Extra empty row to finalize rects touching the top edge
        plate_stack.push(Vec::new());

        for (y, current_row) in plate_stack.into_iter().enumerate() {
            for prev_plate in &prev_row {
                if !current_row.contains(prev_plate) {
                    if let Some(rect) = rect_builder.remove(prev_plate) {
                        wall_rects.push(rect);
                    }
                }
            }
            for plate in &current_row {
                rect_builder
                    .entry(plate.clone())
                    .and_modify(|e| e.top += 1)
                    .or_insert(Rect {
                        bottom: y as i32,
                        top: y as i32,
                        left: plate.left,
                        right: plate.right,
                    });
            }
            prev_row = current_row;
        }

        // Stage 3: Spawn colliders as children of the level entity
        commands.entity(level_entity).with_children(|level| {
            for wall_rect in wall_rects {
                level.spawn((
                    Collider::rectangle(
                        (wall_rect.right as f32 - wall_rect.left as f32 + 1.)
                            * grid_size as f32,
                        (wall_rect.top as f32 - wall_rect.bottom as f32 + 1.)
                            * grid_size as f32,
                    ),
                    RigidBody::Static,
                    Friction::new(0.0),
                    Transform::from_xyz(
                        (wall_rect.left + wall_rect.right + 1) as f32 * grid_size as f32 / 2.,
                        (wall_rect.bottom + wall_rect.top + 1) as f32 * grid_size as f32 / 2.,
                        0.,
                    ),
                ));
            }
        });
    });
}
