use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::*;

const WALK_SPEED: f32 = 120.0;
const SPRINT_MULTIPLIER: f32 = 1.8;
const JUMP_VELOCITY: f32 = 300.0;
const DASH_SPEED: f32 = 400.0;
const DASH_DURATION: f32 = 0.15;
const COYOTE_TIME: f32 = 0.1;
const ACCEL: f32 = 600.0;
const DECEL: f32 = 400.0;

#[derive(Default, Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerState {
    pub grounded: bool,
    pub facing: f32,
    pub dashing: bool,
    pub dash_timer: f32,
    pub has_air_dash: bool,
    pub dash_dir: Vec2,
    pub coyote_timer: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            grounded: false,
            facing: 1.0,
            dashing: false,
            dash_timer: 0.0,
            has_air_dash: true,
            dash_dir: Vec2::X,
            coyote_timer: 0.0,
        }
    }
}

#[derive(Default, Bundle, LdtkEntity)]
pub struct PlayerBundle {
    player: Player,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.register_ldtk_entity::<PlayerBundle>("PlayerStart")
            .add_systems(
                Update,
                (setup_player, ground_detection, player_movement).chain(),
            );
    }
}

fn setup_player(mut commands: Commands, query: Query<Entity, Added<Player>>) {
    for entity in &query {
        commands.entity(entity).insert((
            PlayerState::default(),
            RigidBody::Dynamic,
            Collider::rectangle(14.0, 14.0),
            LinearVelocity::default(),
            GravityScale(1.0),
            Friction::new(0.0),
            LockedAxes::ROTATION_LOCKED,
            Sprite::from_color(Color::srgb(0.4, 0.7, 1.0), Vec2::splat(16.0)),
        ));
    }
}

fn ground_detection(
    spatial_query: SpatialQuery,
    mut query: Query<(Entity, &Transform, &mut PlayerState), With<Player>>,
    time: Res<Time>,
) {
    for (entity, transform, mut state) in &mut query {
        let base = transform.translation.truncate() + Vec2::new(0.0, -7.0);
        let filter = SpatialQueryFilter::default().with_excluded_entities([entity]);

        // Cast 3 rays: center, left edge, right edge for reliable edge detection
        let hit = spatial_query
            .cast_ray(base, Dir2::NEG_Y, 3.0, true, &filter)
            .is_some()
            || spatial_query
                .cast_ray(base + Vec2::new(-6.0, 0.0), Dir2::NEG_Y, 3.0, true, &filter)
                .is_some()
            || spatial_query
                .cast_ray(base + Vec2::new(6.0, 0.0), Dir2::NEG_Y, 3.0, true, &filter)
                .is_some();

        if hit {
            state.grounded = true;
            state.has_air_dash = true;
            state.coyote_timer = COYOTE_TIME;
        } else {
            state.coyote_timer -= time.delta_secs();
            if state.coyote_timer <= 0.0 {
                state.grounded = false;
            }
        }
    }
}

fn player_movement(
    gamepads: Query<&Gamepad>,
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<
        (&mut LinearVelocity, &mut GravityScale, &mut PlayerState),
        With<Player>,
    >,
    time: Res<Time>,
) {
    let Ok((mut velocity, mut gravity_scale, mut state)) = players.single_mut() else {
        return;
    };

    // Gamepad input
    let gamepad = gamepads.iter().next();
    let stick_x = gamepad.map(|g| g.left_stick().x).unwrap_or(0.0);
    let stick_y = gamepad.map(|g| g.left_stick().y).unwrap_or(0.0);
    let gp_sprint =
        gamepad.is_some_and(|g| g.get(GamepadButton::LeftTrigger2).unwrap_or(0.0) > 0.5);
    let gp_jump_pressed = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::South));
    let gp_jump_released = gamepad.is_some_and(|g| g.just_released(GamepadButton::South));
    let gp_dash = gamepad.is_some_and(|g| g.just_pressed(GamepadButton::East));

    // Keyboard fallback
    let kb_x = if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        -1.0
    } else if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        1.0
    } else {
        0.0
    };
    let kb_y = if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        1.0
    } else if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        -1.0
    } else {
        0.0
    };
    let kb_sprint = keys.pressed(KeyCode::ShiftLeft);
    let kb_jump_pressed = keys.just_pressed(KeyCode::Space);
    let kb_jump_released = keys.just_released(KeyCode::Space);
    let kb_dash = keys.just_pressed(KeyCode::KeyE);

    // Merge inputs
    let move_x = if stick_x.abs() > 0.1 { stick_x } else { kb_x };
    let move_y = if stick_y.abs() > 0.1 { stick_y } else { kb_y };
    let sprint = gp_sprint || kb_sprint;
    let jump_pressed = gp_jump_pressed || kb_jump_pressed;
    let jump_released = gp_jump_released || kb_jump_released;
    let dash_pressed = gp_dash || kb_dash;

    // Dash initiation — eight-way, once per ground touch
    if dash_pressed && state.has_air_dash && !state.dashing {
        let raw = Vec2::new(move_x, move_y);
        let dir = if raw.length_squared() > 0.01 {
            raw.normalize()
        } else {
            Vec2::new(state.facing, 0.0)
        };
        state.dashing = true;
        state.dash_timer = DASH_DURATION;
        state.has_air_dash = false;
        state.dash_dir = dir;
    }

    // Movement
    let dt = time.delta_secs();
    if state.dashing {
        velocity.x = state.dash_dir.x * DASH_SPEED;
        velocity.y = state.dash_dir.y * DASH_SPEED;
        gravity_scale.0 = 0.0;
        state.dash_timer -= dt;
        if state.dash_timer <= 0.0 {
            state.dashing = false;
            gravity_scale.0 = 1.0;
        }
    } else {
        let speed = if sprint && state.grounded {
            WALK_SPEED * SPRINT_MULTIPLIER
        } else {
            WALK_SPEED
        };
        let target_vx = move_x * speed;
        let accel = if move_x.abs() > 0.1 { ACCEL } else { DECEL };
        let diff = target_vx - velocity.x;
        let change = accel * dt;
        if diff.abs() <= change {
            velocity.x = target_vx;
        } else {
            velocity.x += change * diff.signum();
        }
        gravity_scale.0 = 1.0;
    }

    // Jump (with coyote time)
    if jump_pressed && state.grounded && !state.dashing {
        velocity.y = JUMP_VELOCITY;
        state.grounded = false;
        state.coyote_timer = 0.0;
    }
    if jump_released && velocity.y > 0.0 {
        velocity.y *= 0.5;
    }

    // Track facing direction
    if move_x.abs() > 0.1 {
        state.facing = move_x.signum();
    }
}
