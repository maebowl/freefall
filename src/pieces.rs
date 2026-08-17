//! The original ASCII-art procedural generator for Zen mode: hand-drawn piece
//! templates (`#` = solid, `v`/`^` = entry/exit openings) stacked into a tower.
//!
//! NOTE: live Zen now uses the LDtk stitcher in `ldtk::build_zen_world`. This
//! module is only reached through `level::build_level`, which the Zen *replay*
//! path still calls — so Zen replays are generated differently from live Zen
//! runs. Legacy; kept until the two Zen paths are unified.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const GRID_W: i32 = 40;
const SIDE_WALL: i32 = 2;
const FLOOR_H: i32 = 3;
const CEILING_H: i32 = 2;
const PLAYABLE_W: i32 = GRID_W - SIDE_WALL * 2; // 36

struct Connector {
    x: i32,
    w: i32,
}

struct Piece {
    grid: Vec<Vec<bool>>,
    width: i32,
    height: i32,
    entry: Connector, // bottom opening
    exit: Connector,  // top opening
}

fn parse_piece(ascii: &str) -> Piece {
    let lines: Vec<&str> = ascii.lines().filter(|l| !l.is_empty()).collect();
    let height = lines.len() as i32;
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i32;
    assert!(width == PLAYABLE_W, "Piece width must be {PLAYABLE_W}, got {width}");

    // Bottom row -> entry (scan for 'v')
    let bottom = lines[height as usize - 1];
    let entry_start = bottom.find('v').expect("No entry marker 'v' in bottom row") as i32;
    let entry_w = bottom.chars().skip(entry_start as usize).take_while(|&c| c == 'v').count() as i32;

    // Top row -> exit (scan for '^')
    let top = lines[0];
    let exit_start = top.find('^').expect("No exit marker '^' in top row") as i32;
    let exit_w = top.chars().skip(exit_start as usize).take_while(|&c| c == '^').count() as i32;

    // All rows -> grid (# = solid, everything else = air)
    let grid: Vec<Vec<bool>> = lines
        .iter()
        .map(|line| {
            let mut row = vec![false; width as usize];
            for (i, ch) in line.chars().enumerate() {
                if ch == '#' {
                    row[i] = true;
                }
            }
            row
        })
        .collect();

    Piece {
        grid,
        width,
        height,
        entry: Connector { x: entry_start, w: entry_w },
        exit: Connector { x: exit_start, w: exit_w },
    }
}

// --- Piece definitions (each line exactly 36 chars) ---
// Columns: 000000000011111111112222222222333333
//           012345678901234567890123456789012345

// 10 rows, exit pos=9 w=10, entry pos=14 w=10
const WIDE_STAIRCASE: &str = "\
#########^^^^^^^^^^#################
#..................................#
##....####..........................
..####..............................
............####....................
............................####....
####................................
#............................####..#
##.....#####........................
##############vvvvvvvvvv############";

// 8 rows, exit pos=11 w=10, entry pos=14 w=10
const SIMPLE_PLATFORMS: &str = "\
###########^^^^^^^^^^###############
#..................................#
##..####.........####...............
............####....................
....####..............####..........
................................####
####........####....................
##############vvvvvvvvvv############";

// 10 rows, exit pos=2 w=26, entry pos=4 w=26
const ZIGZAG: &str = "\
##^^^^^^^^^^^^^^^^^^^^^^^^^^########
#..................................#
##..####............................
............####....................
............................####....
................####................
........####........................
####................................
#...............................####
####vvvvvvvvvvvvvvvvvvvvvvvvvv######";

// 6 rows, exit pos=6 w=10, entry pos=14 w=10
const REST_STOP: &str = "\
######^^^^^^^^^^####################
#..................................#
##........################..........
............####....................
....................................
##############vvvvvvvvvv############";

// 12 rows, exit pos=14 w=10, entry pos=14 w=10
const WALL_JUMP_CORRIDOR: &str = "\
##############^^^^^^^^^^############
##############..........############
##############..........############
##############..........############
##############..###.....############
##############..........############
##############..........############
##############.....###..############
##############..........############
##############..........############
##############..........############
##############vvvvvvvvvv############";

// 12 rows, exit pos=4 w=26, entry pos=4 w=26
const TWIN_CHIMNEYS: &str = "\
####^^^^^^^^^^^^^^^^^^^^^^^^^^######
####......##########......##########
####......##########......##########
####......##########......##########
####.###..##########..###.##########
####......##########......##########
####......##########......##########
####..###.##########.###..##########
####......##########......##########
####......##########......##########
####......##########......##########
####vvvvvvvvvvvvvvvvvvvvvvvvvv######";

// 10 rows, exit pos=9 w=10, entry pos=14 w=10
const SCATTERED_STEPS: &str = "\
#########^^^^^^^^^^#################
#..................................#
##.##...............................
.........##.........................
....................##..............
...........##.......................
.......##...........................
##.........................##.....##
........##..........##..............
##############vvvvvvvvvv############";

// 10 rows, exit pos=2 w=26, entry pos=28 w=4
const L_SHAPE: &str = "\
##^^^^^^^^^^^^^^^^^^^^^^^^^^########
#..................................#
##......############################
........############################
........############################
........############################
....................................
....................................
....................................
############################vvvv####";

// 10 rows, exit pos=9 w=10, entry pos=14 w=10
const DASH_GAUNTLET: &str = "\
#########^^^^^^^^^^#################
#..................................#
##..##..............................
................##..................
................................##..
........##..........................
..........................##........
##..##..........................##.#
............##......................
##############vvvvvvvvvv############";

// 14 rows, exit pos=14 w=10, entry pos=14 w=10
const TIGHT_CHIMNEY: &str = "\
##############^^^^^^^^^^############
##############..........############
################......##############
################......##############
################.###..##############
################......##############
################......##############
################..###.##############
################......##############
################.###..##############
################......##############
################......##############
##############..........############
##############vvvvvvvvvv############";

// 10 rows, exit pos=9 w=10, entry pos=14 w=10
const PRECISION_HOPS: &str = "\
#########^^^^^^^^^^#################
#..................................#
##..##..............................
..............##....................
............................##......
................##..................
........##..........................
##........................##......##
..##............................##..
##############vvvvvvvvvv############";

// 8 rows, exit pos=2 w=26, entry pos=4 w=26
const SPRINT_RUN: &str = "\
##^^^^^^^^^^^^^^^^^^^^^^^^^^########
#..................................#
##..####............................
....................####............
..####..............................
............................####....
####................................
####vvvvvvvvvvvvvvvvvvvvvvvvvv######";

fn build_piece_library() -> Vec<Piece> {
    vec![
        parse_piece(WIDE_STAIRCASE),
        parse_piece(SIMPLE_PLATFORMS),
        parse_piece(ZIGZAG),
        parse_piece(REST_STOP),
        parse_piece(WALL_JUMP_CORRIDOR),
        parse_piece(TWIN_CHIMNEYS),
        parse_piece(SCATTERED_STEPS),
        parse_piece(L_SHAPE),
        parse_piece(DASH_GAUNTLET),
        parse_piece(TIGHT_CHIMNEY),
        parse_piece(PRECISION_HOPS),
        parse_piece(SPRINT_RUN),
    ]
}

/// Stamp a piece into the full level grid at the given y offset.
/// piece_y is the bottom row of the piece in the level grid.
/// Skips the entry row (bottom) and exit row (top) — those rows only
/// define connector positions and should not create physical barriers.
fn stamp_piece(level_grid: &mut Vec<Vec<bool>>, piece: &Piece, piece_y: i32) {
    let grid_h = level_grid.len() as i32;
    for py in 0..piece.height {
        // Skip entry row (bottom, py = height-1) and exit row (top, py = 0)
        if py == 0 || py == piece.height - 1 {
            continue;
        }
        // Piece row 0 is the top (exit), last row is bottom (entry).
        // We place bottom of piece at piece_y, growing upward.
        let level_y = piece_y + (piece.height - 1 - py);
        if level_y < 0 || level_y >= grid_h {
            continue;
        }
        for px in 0..piece.width {
            if piece.grid[py as usize][px as usize] {
                let lx = SIDE_WALL + px;
                if lx >= 0 && lx < GRID_W {
                    level_grid[level_y as usize][lx as usize] = true;
                }
            }
        }
    }
}

/// Generate a 3-row bridge connecting prev_exit to next_entry.
fn generate_bridge(
    level_grid: &mut Vec<Vec<bool>>,
    bridge_y: i32,
    prev_exit_x: i32,
    prev_exit_w: i32,
    next_entry_x: i32,
    next_entry_w: i32,
) {
    let grid_h = level_grid.len() as i32;

    // Platform at prev exit position (row 0 of bridge)
    let y0 = bridge_y;
    if y0 >= 0 && y0 < grid_h {
        let cx = SIDE_WALL + prev_exit_x;
        for dx in 0..prev_exit_w.max(4) {
            let x = cx + dx;
            if x >= 0 && x < GRID_W {
                level_grid[y0 as usize][x as usize] = true;
            }
        }
    }

    // Platform at next entry position (row 2 of bridge)
    let y2 = bridge_y + 2;
    if y2 >= 0 && y2 < grid_h {
        let cx = SIDE_WALL + next_entry_x;
        for dx in 0..next_entry_w.max(4) {
            let x = cx + dx;
            if x >= 0 && x < GRID_W {
                level_grid[y2 as usize][x as usize] = true;
            }
        }
    }

    // If horizontal gap > 8, add intermediate stepping platform at row 1
    let exit_center = prev_exit_x + prev_exit_w / 2;
    let entry_center = next_entry_x + next_entry_w / 2;
    let h_gap = (exit_center - entry_center).abs();
    if h_gap > 8 {
        let y1 = bridge_y + 1;
        if y1 >= 0 && y1 < grid_h {
            let mid_x = SIDE_WALL + (exit_center + entry_center) / 2 - 2;
            for dx in 0..4 {
                let x = mid_x + dx;
                if x >= 0 && x < GRID_W {
                    level_grid[y1 as usize][x as usize] = true;
                }
            }
        }
    }
}

/// Returns (grid, grid_height, checkpoint_x, checkpoint_y)
/// checkpoint_x and checkpoint_y are in grid coordinates.
pub fn select_and_layout(seed: u64) -> (Vec<Vec<bool>>, i32, i32, i32) {
    let mut rng = StdRng::seed_from_u64(seed);
    let pieces = build_piece_library();

    // Determine grid height
    let grid_h = rng.random_range(80..=100) as i32;
    let grid_w = GRID_W;

    // Initialize empty grid
    let mut grid = vec![vec![false; grid_w as usize]; grid_h as usize];

    // Floor
    for y in 0..FLOOR_H {
        for x in 0..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Ceiling
    for y in (grid_h - CEILING_H)..grid_h {
        for x in 0..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Side walls
    for y in 0..grid_h {
        for x in 0..SIDE_WALL {
            grid[y as usize][x as usize] = true;
        }
        for x in (grid_w - SIDE_WALL)..grid_w {
            grid[y as usize][x as usize] = true;
        }
    }

    // Landing platform at center
    let center_x = PLAYABLE_W / 2 - 2; // piece-local x
    let landing_y = FLOOR_H;
    for dx in 0..5 {
        let x = SIDE_WALL + center_x + dx;
        if x >= 0 && x < grid_w {
            grid[landing_y as usize][x as usize] = true;
        }
    }

    let mut y = FLOOR_H + 2; // current placement y (bottom of next piece)
    let mut prev_exit_x = center_x;
    let mut prev_exit_w = 5;
    let ceiling_limit = grid_h - CEILING_H - 10;

    let mut last_exit_x = center_x;
    let mut last_exit_y = landing_y;

    // Loop: place pieces until near ceiling
    while y + 6 <= ceiling_limit {
        let piece_idx = rng.random_range(0..pieces.len());
        let piece = &pieces[piece_idx];

        if y + piece.height > ceiling_limit {
            break;
        }

        // Connector check: does prev exit overlap with candidate entry by >= 4?
        let overlap_start = prev_exit_x.max(piece.entry.x);
        let overlap_end = (prev_exit_x + prev_exit_w).min(piece.entry.x + piece.entry.w);
        let overlap = overlap_end - overlap_start;

        if overlap >= 4 {
            // Direct placement
            stamp_piece(&mut grid, piece, y);
            last_exit_x = piece.exit.x;
            last_exit_y = y + piece.height - 1;
            prev_exit_x = piece.exit.x;
            prev_exit_w = piece.exit.w;
            y += piece.height;
        } else {
            // Insert bridge then piece
            if y + 3 + piece.height > ceiling_limit {
                break;
            }
            generate_bridge(
                &mut grid,
                y,
                prev_exit_x,
                prev_exit_w,
                piece.entry.x,
                piece.entry.w,
            );
            y += 3;
            stamp_piece(&mut grid, piece, y);
            last_exit_x = piece.exit.x;
            last_exit_y = y + piece.height - 1;
            prev_exit_x = piece.exit.x;
            prev_exit_w = piece.exit.w;
            y += piece.height;
        }
    }

    // Place checkpoint platform at the last exit position
    let cp_y = (last_exit_y + 2).min(grid_h - CEILING_H - 3);
    let cp_x = (SIDE_WALL + last_exit_x).clamp(SIDE_WALL + 1, grid_w - SIDE_WALL - 4);
    for dx in 0..5 {
        let x = cp_x + dx;
        if x >= 0 && x < grid_w && cp_y >= 0 && cp_y < grid_h {
            grid[cp_y as usize][x as usize] = true;
        }
    }

    (grid, grid_h, cp_x, cp_y)
}
