use bevy::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component)]
pub struct Wall;

/// A wall you cannot wall-jump off of (you still collide and wall-slide on it).
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component)]
pub struct SlippyWall;
