# Freefall

A 2D platformer with procedurally generated levels, built with Bevy 0.18 and avian2d physics.

Climb procedurally generated vertical towers using jumps, wall jumps, dashes, and wavedashes. Reach the green checkpoint at the top to advance to the next level. Each level gets progressively harder with narrower platforms, wider gaps, and more demanding sections.

## Prerequisites

- **Rust** (stable, 1.80+)
- **System libraries** (Linux): wayland, vulkan, alsa, udev, libxkbcommon, X11 libs

### NixOS

```sh
nix-shell
```

### Ubuntu/Debian

```sh
sudo apt install pkg-config libwayland-dev libxkbcommon-dev libudev-dev libasound2-dev libvulkan-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev
```

### Fedora

```sh
sudo dnf install wayland-devel libxkbcommon-devel libudev-devel alsa-lib-devel vulkan-loader-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel
```

## Build & Run

```sh
cargo run
```

For playable performance, dependencies are optimized even in dev builds (`[profile.dev.package."*"] opt-level = 2` in Cargo.toml). The first build will be slow; subsequent rebuilds are fast.

## Controls

| Action | Gamepad | Keyboard |
|--------|---------|----------|
| Move | Left Stick | A/D or Arrow Keys |
| Jump | South (A) | Space |
| Dash | West (X) | E |
| Sprint | RT | Left Shift |

- **Variable jump**: release jump early for a shorter hop
- **Dash**: eight-way burst, once per ground touch; briefly disables gravity
- **Wavedash**: dash into the ground then jump to preserve momentum
- **Wall jump**: jump while sliding on a wall to kick off in the opposite direction
- **Wall slide**: hold toward a wall while airborne to slow your descent
