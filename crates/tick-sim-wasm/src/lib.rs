//! The browser's copy of the simulation.
//!
//! This is a raw `wasm32-unknown-unknown` cdylib with a C ABI: no wasm-bindgen,
//! no JavaScript glue to keep in step, and no build tool beyond `cargo`. The
//! client instantiates it with `WebAssembly.instantiate` and reads results out
//! of linear memory.
//!
//! Two jobs:
//!   1. Predict the local player's movement, using the same `step_movement`
//!      the server runs authoritatively, so prediction can never disagree with
//!      the server about how a jump arcs or how a wall slides.
//!   2. Hand the renderer the map's collision geometry, so what you see is
//!      exactly what bullets and feet collide with.

use core::ptr::addr_of_mut;
use tick_sim::defs::{load_map, MapData, MapId};
use tick_sim::math::v3;
use tick_sim::movement::{step_movement, Input, MoveState};

static mut MAP: Option<MapData> = None;

/// Local player state, shared with JavaScript as a flat f32 view:
/// `[x, y, z, vx, vy, vz, on_ground, crouching]`.
static mut STATE: [f32; 8] = [0.0; 8];

/// Geometry scratch buffer: seven floats per brush,
/// `[minx, miny, minz, maxx, maxy, maxz, flags]` where flags is
/// 1 = thin cover, 2 = breakable glass.
static mut GEOMETRY: [f32; 7 * 512] = [0.0; 7 * 512];
static mut GEOMETRY_COUNT: u32 = 0;

fn map_ref() -> &'static MapData {
    unsafe { (*addr_of_mut!(MAP)).as_ref().expect("world_init not called") }
}

#[no_mangle]
pub extern "C" fn world_init(map_id: u32) {
    let map = load_map(MapId::from_u8(map_id as u8));
    let n = map.brushes.len().min(512);
    unsafe {
        let g = &mut *addr_of_mut!(GEOMETRY);
        for (i, b) in map.brushes.iter().take(n).enumerate() {
            let o = i * 7;
            g[o] = b.aabb.min.x;
            g[o + 1] = b.aabb.min.y;
            g[o + 2] = b.aabb.min.z;
            g[o + 3] = b.aabb.max.x;
            g[o + 4] = b.aabb.max.y;
            g[o + 5] = b.aabb.max.z;
            g[o + 6] = (b.thin as u32 as f32) + (b.glass as u32 as f32) * 2.0;
        }
        GEOMETRY_COUNT = n as u32;
        MAP = Some(map);
    }
}

#[no_mangle]
pub extern "C" fn state_ptr() -> *const f32 {
    addr_of_mut!(STATE) as *const f32
}

#[no_mangle]
pub extern "C" fn geometry_ptr() -> *const f32 {
    addr_of_mut!(GEOMETRY) as *const f32
}

#[no_mangle]
pub extern "C" fn geometry_count() -> u32 {
    unsafe { GEOMETRY_COUNT }
}

/// Overwrite the predicted state, used when reconciling against a snapshot.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn set_state(
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    on_ground: u32,
    crouching: u32,
) {
    unsafe {
        let s = &mut *addr_of_mut!(STATE);
        s[0] = x;
        s[1] = y;
        s[2] = z;
        s[3] = vx;
        s[4] = vy;
        s[5] = vz;
        s[6] = on_ground as f32;
        s[7] = crouching as f32;
    }
}

/// Advance the local player one 64 Hz tick. Called once per unacknowledged
/// input during reconciliation, and once per new input during prediction.
#[no_mangle]
pub extern "C" fn step(
    buttons: u32,
    yaw: f32,
    pitch: f32,
    speed_mult: f32,
    gravity_mult: f32,
    can_sprint: u32,
) {
    let map = map_ref();
    unsafe {
        let s = &mut *addr_of_mut!(STATE);
        let mut mv = MoveState {
            pos: v3(s[0], s[1], s[2]),
            vel: v3(s[3], s[4], s[5]),
            on_ground: s[6] != 0.0,
            crouching: s[7] != 0.0,
        };
        let input = Input {
            seq: 0,
            yaw,
            pitch,
            buttons: buttons as u16,
        };
        step_movement(
            &mut mv,
            &input,
            &map.brushes,
            speed_mult,
            gravity_mult,
            can_sprint != 0,
        );
        s[0] = mv.pos.x;
        s[1] = mv.pos.y;
        s[2] = mv.pos.z;
        s[3] = mv.vel.x;
        s[4] = mv.vel.y;
        s[5] = mv.vel.z;
        s[6] = mv.on_ground as u32 as f32;
        s[7] = mv.crouching as u32 as f32;
    }
}
