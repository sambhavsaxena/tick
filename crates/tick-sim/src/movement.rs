//! Movement and collision.
//!
//! This module is the reason the simulation is a separate crate: the server
//! calls it inside the authoritative tick, and the browser calls the exact
//! same compiled code through WebAssembly to predict the local player. There
//! is no second implementation to drift out of sync.

use crate::defs::*;
use crate::math::*;

pub mod buttons {
    pub const FWD: u16 = 1 << 0;
    pub const BACK: u16 = 1 << 1;
    pub const LEFT: u16 = 1 << 2;
    pub const RIGHT: u16 = 1 << 3;
    pub const JUMP: u16 = 1 << 4;
    pub const CROUCH: u16 = 1 << 5;
    pub const FIRE: u16 = 1 << 6;
    pub const ADS: u16 = 1 << 7;
    pub const ABILITY: u16 = 1 << 8;
    pub const RELOAD: u16 = 1 << 9;
    pub const SPRINT: u16 = 1 << 10;
    pub const MELEE: u16 = 1 << 11;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub seq: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub buttons: u16,
}

impl Input {
    pub fn held(&self, b: u16) -> bool {
        self.buttons & b != 0
    }
}

/// The slice of player state that movement touches.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
    pub crouching: bool,
}

pub fn player_box(pos: Vec3, crouching: bool) -> Aabb {
    let h = if crouching {
        PLAYER_CROUCH_HEIGHT
    } else {
        PLAYER_HEIGHT
    };
    Aabb {
        min: v3(pos.x - PLAYER_RADIUS, pos.y, pos.z - PLAYER_RADIUS),
        max: v3(pos.x + PLAYER_RADIUS, pos.y + h, pos.z + PLAYER_RADIUS),
    }
}

fn blocked(pos: Vec3, crouching: bool, brushes: &[Brush]) -> bool {
    let b = player_box(pos, crouching);
    brushes.iter().any(|s| !s.broken && s.aabb.overlaps(&b))
}

/// Move one axis at a time and stop at the first blocking brush, then decide
/// whether the result is a move the player actually asked for.
///
/// Splitting the axes is what lets a player slide along a wall instead of
/// sticking to it, and against axis-aligned geometry it is the right
/// mechanism — but on its own it is too generous. Walk forward into a wall at
/// an angle and one axis survives, so holding W alone carries you sideways
/// along a surface you never asked to travel along, faster the more sharply
/// you face it.
///
/// So the slide is kept, and then judged. A move whose direction ends up far
/// from the direction that was asked for is refused outright and the player
/// stops dead. Two things are allowed through:
///
///   * a small deflection — you are brushing past a wall that runs alongside
///     you, and you are still going where you meant to. Without this, running
///     down a corridor would require aiming perfectly parallel to it;
///   * any deflection at all while a strafe key is down, because moving
///     sideways is exactly what that key means.
fn move_axis(
    st: &mut MoveState,
    delta: Vec3,
    input: &Input,
    brushes: &[Brush],
) -> (bool, bool) {
    let mut hit_wall = false;
    let mut hit_floor = false;

    let try_axis = |st: &mut MoveState, d: Vec3| -> bool {
        let next = st.pos.add(d);
        if blocked(next, st.crouching, brushes) {
            false
        } else {
            st.pos = next;
            true
        }
    };

    // Horizontal first, as one phase, so the whole of it can be taken back.
    let before = st.pos;
    if delta.x != 0.0 && !try_axis(st, v3(delta.x, 0.0, 0.0)) {
        // Try to step over a low obstacle before giving up on the axis.
        let lifted = st.pos.add(v3(delta.x, STEP_HEIGHT, 0.0));
        if st.on_ground && !blocked(lifted, st.crouching, brushes) {
            st.pos = lifted;
        } else {
            st.vel.x = 0.0;
            hit_wall = true;
        }
    }
    if delta.z != 0.0 && !try_axis(st, v3(0.0, 0.0, delta.z)) {
        let lifted = st.pos.add(v3(0.0, STEP_HEIGHT, delta.z));
        if st.on_ground && !blocked(lifted, st.crouching, brushes) {
            st.pos = lifted;
        } else {
            st.vel.z = 0.0;
            hit_wall = true;
        }
    }

    if hit_wall {
        let moved = v3(st.pos.x - before.x, 0.0, st.pos.z - before.z);
        let wanted = v3(delta.x, 0.0, delta.z);
        let strafing = input.held(buttons::LEFT) || input.held(buttons::RIGHT);
        if !strafing && moved.len_xz() > 1e-6 && wanted.len_xz() > 1e-6 {
            if wanted.normalized().dot(moved.normalized()) < SLIDE_LIMIT {
                // The wall was in front, not alongside. Take the whole
                // horizontal step back — including any step-up it climbed —
                // and drop the speed, so leaning on the key against a wall
                // cannot store up momentum to be released along it later.
                st.pos = before;
                st.vel.x = 0.0;
                st.vel.z = 0.0;
            }
        }
    }

    if delta.y != 0.0 && !try_axis(st, v3(0.0, delta.y, 0.0)) {
        if delta.y < 0.0 {
            hit_floor = true;
        }
        st.vel.y = 0.0;
    }
    (hit_wall, hit_floor)
}

/// Advance one player by one 64 Hz tick.
///
/// `speed_mult` folds in the character passive and any staggered penalty;
/// `gravity_mult` is how the Gravity Dip event reaches the simulation.
pub fn step_movement(
    st: &mut MoveState,
    input: &Input,
    brushes: &[Brush],
    speed_mult: f32,
    gravity_mult: f32,
    can_sprint: bool,
    pull: Vec3,
) {
    let want_crouch = input.held(buttons::CROUCH);
    if !want_crouch && st.crouching {
        // Only stand up when there is room to.
        if !blocked(st.pos, false, brushes) {
            st.crouching = false;
        }
    } else {
        st.crouching = want_crouch;
    }

    let mut wish = Vec3::ZERO;
    let fwd = v3(sin(input.yaw), 0.0, cos(input.yaw));
    // right = fwd x up, so RIGHT strafes toward the camera's screen-right.
    let right = v3(-fwd.z, 0.0, fwd.x);
    if input.held(buttons::FWD) {
        wish = wish.add(fwd);
    }
    if input.held(buttons::BACK) {
        wish = wish.sub(fwd);
    }
    if input.held(buttons::RIGHT) {
        wish = wish.add(right);
    }
    if input.held(buttons::LEFT) {
        wish = wish.sub(right);
    }
    wish = wish.normalized();

    let sprinting = can_sprint
        && input.held(buttons::SPRINT)
        && input.held(buttons::FWD)
        && !st.crouching
        && !input.held(buttons::ADS);

    let target_speed = if st.crouching {
        CROUCH_SPEED
    } else if sprinting {
        SPRINT_SPEED
    } else {
        WALK_SPEED
    } * speed_mult;

    // Ground friction, applied to the horizontal component only.
    if st.on_ground {
        let speed = st.vel.len_xz();
        if speed > 0.0 {
            let drop = speed * FRICTION * DT;
            let scale = if speed - drop < 0.0 {
                0.0
            } else {
                (speed - drop) / speed
            };
            st.vel.x *= scale;
            st.vel.z *= scale;
        }
    }

    // Quake-style acceleration: only accelerate up to the wish speed along the
    // wish direction, which is what makes air control feel correct.
    let accel = if st.on_ground { GROUND_ACCEL } else { AIR_ACCEL };
    let current = st.vel.x * wish.x + st.vel.z * wish.z;
    let add = target_speed - current;
    if add > 0.0 {
        let step = clamp(accel * DT * target_speed, 0.0, add);
        st.vel.x += wish.x * step;
        st.vel.z += wish.z * step;
    }

    if st.on_ground && input.held(buttons::JUMP) {
        st.vel.y = JUMP_SPEED;
        st.on_ground = false;
    }

    // Environmental pull (the Night sky's black hole): a constant gentle
    // acceleration, shared with the client's prediction through this same
    // function so it can never cause a reconciliation fight.
    st.vel.x += pull.x * DT;
    st.vel.z += pull.z * DT;

    st.vel.y -= GRAVITY * gravity_mult * DT;
    if st.vel.y < -60.0 {
        st.vel.y = -60.0;
    }

    let delta = st.vel.scale(DT);
    let (_, hit_floor) = move_axis(st, delta, input, brushes);

    // Ground check: probe a hair below the feet.
    let probe = st.pos.add(v3(0.0, -0.03, 0.0));
    st.on_ground = hit_floor || blocked(probe, st.crouching, brushes);
    if st.on_ground && st.vel.y < 0.0 {
        st.vel.y = 0.0;
    }
}

/// Cast a ray against level geometry. Returns the distance to the nearest
/// brush, whether the first thing hit was thin cover, and which brush it was.
///
/// Glass is marked thin, so a shot through the atrium reaches the far side at
/// half damage while still stopping a player from walking through the window —
/// until the pane is shot out, at which point the brush is broken and this
/// stops seeing it at all.
pub fn trace_world(origin: Vec3, dir: Vec3, max_t: f32, brushes: &[Brush]) -> WorldHit {
    let mut hit = WorldHit {
        t: max_t,
        thin: false,
        brush: usize::MAX,
    };
    for (i, b) in brushes.iter().enumerate() {
        if b.broken {
            continue;
        }
        if let Some(t) = b.aabb.ray(origin, dir, max_t) {
            if t < hit.t {
                hit.t = t;
                hit.thin = b.thin;
                hit.brush = i;
            }
        }
    }
    hit
}

/// Cast a ray against the geometry that blocks *sight*.
///
/// Thin cover is deliberately transparent here. Bullets pass through it, so
/// if it also hid the shooter you would be shot by someone the server had
/// culled from your screen — cover that grants concealment it was never meant
/// to grant. Anything you can be shot through, you can see through; only
/// solid geometry hides a player.
pub fn trace_sight(origin: Vec3, dir: Vec3, max_t: f32, brushes: &[Brush]) -> f32 {
    let mut best = max_t;
    for b in brushes {
        if b.thin || b.broken {
            continue;
        }
        if let Some(t) = b.aabb.ray(origin, dir, max_t) {
            if t < best {
                best = t;
            }
        }
    }
    best
}

/// What a ray met in the level. `brush` is `usize::MAX` when nothing was hit
/// inside the trace's range.
#[derive(Clone, Copy, Debug)]
pub struct WorldHit {
    pub t: f32,
    pub thin: bool,
    pub brush: usize,
}
