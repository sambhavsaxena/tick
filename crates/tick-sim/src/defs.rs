//! Static game data: weapons, characters, modes, weather and maps.
//!
//! These are the numbers from the design document. They live in one place so
//! that balance changes are a single-file diff and so that the server and the
//! client read the same table.

use crate::math::{v3, Aabb, Vec3};

pub const TICK_HZ: u32 = 64;
pub const DT: f32 = 1.0 / TICK_HZ as f32;

pub const MAX_PLAYERS: usize = 8;
pub const TEAM_SIZE: usize = 4;

pub const MAX_HEALTH: i32 = 100;

// Player capsule, approximated by two boxes so that a head hit is a distinct,
// generous volume. The dimensions are identical for every character: character
// choice never changes how hard you are to hit.
pub const PLAYER_RADIUS: f32 = 0.4;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_CROUCH_HEIGHT: f32 = 1.15;
pub const EYE_HEIGHT: f32 = 1.62;
pub const HEAD_BOTTOM: f32 = 1.45;

// Movement, tuned for 20-30 second map traversal.
pub const WALK_SPEED: f32 = 5.0;
pub const SPRINT_SPEED: f32 = 7.2;
pub const CROUCH_SPEED: f32 = 2.6;
pub const GROUND_ACCEL: f32 = 70.0;
pub const AIR_ACCEL: f32 = 14.0;
pub const FRICTION: f32 = 9.0;
pub const GRAVITY: f32 = 22.0;
pub const JUMP_SPEED: f32 = 7.2;
pub const STEP_HEIGHT: f32 = 0.45;

/// How long after a sprint a shot still carries the sprint spread penalty,
/// so the cost tapers off instead of vanishing the instant Shift is released.
pub const SPRINT_SPREAD_LINGER: f32 = 0.12;

/// How far a wall is allowed to bend your movement before the move is refused
/// outright, as the cosine of the angle between the direction you asked for
/// and the direction the geometry would give you. Above this you are brushing
/// past a wall that runs alongside you and should keep going; below it the
/// wall is in front of you, and the classic per-axis slide would be turning a
/// press of W into sideways travel nobody asked for.
///
/// 0.87 is a shade under 30 degrees.
pub const SLIDE_LIMIT: f32 = 0.87;

/// Spread multiplier applied while sprinting. Running and gunning is allowed
/// on every weapon; it just costs accuracy rather than costing the shot.
pub const SPRINT_SPREAD_MULT: f32 = 1.6;

/// A hit this close to the head box still counts as a head shot. Wide enough
/// that a shot clipping the neck or the top of a shoulder reads the way it
/// looked, narrow enough that a centre-mass hit never does.
pub const HEAD_GRACE: f32 = 0.16;

/// Level geometry is built to one of two heights and nothing in between: a
/// top at or below `MANTLE_HEIGHT` is cleared with a single jump (the jump
/// apex is JUMP_SPEED^2 / 2 GRAVITY = 1.18 m), and anything a player is not
/// meant to climb is at least `WALL_HEIGHT` tall so it reads as a wall.
pub const MANTLE_HEIGHT: f32 = 1.10;
pub const WALL_HEIGHT: f32 = 3.20;

/// Horizontal acceleration toward the Night sky's black hole (+X horizon).
/// Ground friction (9/s) caps the standing drift at PULL/9 ≈ 0.28 m/s —
/// a lean, not a slide — while jumps drift visibly further toward it.
pub const BLACK_HOLE_PULL: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Weapon {
    Sting = 0,
    Ridge = 1,
    Maul = 2,
    Arc = 3,
    Tack = 4,
    /// Airdrop-only rifle. Exists nowhere else in the game.
    Lance = 5,
    /// Melee-only loadout: no ranged fire at all. Left click swings.
    Blade = 6,
}

impl Weapon {
    pub fn from_u8(v: u8) -> Weapon {
        match v {
            1 => Weapon::Ridge,
            2 => Weapon::Maul,
            3 => Weapon::Arc,
            4 => Weapon::Tack,
            5 => Weapon::Lance,
            6 => Weapon::Blade,
            _ => Weapon::Sting,
        }
    }
    pub fn stats(self) -> WeaponStats {
        match self {
            // 900 RPM, 14 body falling to 9 past 18 m, 25 head.
            Weapon::Sting => WeaponStats {
                name: "Sting",
                interval: 60.0 / 900.0,
                burst: 1,
                burst_interval: 0.0,
                body: 14.0,
                head: 25.0,
                pellets: 1,
                spread: 0.006,
                mag: 30,
                reload: 1.6,
                shell_reload: false,
                ads_time: 0.18,
                falloff_start: 18.0,
                falloff_end: 30.0,
                falloff_mult: 9.0 / 14.0,
                projectile_speed: 0.0,
                recoil: 0.55,
            },
            // 240 RPM, 45 body, 100 head: one head hit kills at any range.
            Weapon::Ridge => WeaponStats {
                name: "Ridge",
                interval: 60.0 / 240.0,
                burst: 1,
                burst_interval: 0.0,
                body: 45.0,
                head: 100.0,
                pellets: 1,
                spread: 0.0,
                mag: 12,
                reload: 1.9,
                shell_reload: false,
                ads_time: 0.28,
                falloff_start: 200.0,
                falloff_end: 200.0,
                falloff_mult: 1.0,
                projectile_speed: 0.0,
                recoil: 2.4,
            },
            // Nine pellets of 12, one-shot inside 6 m on a full pellet count.
            Weapon::Maul => WeaponStats {
                name: "Maul",
                interval: 0.85,
                burst: 1,
                burst_interval: 0.0,
                body: 12.0,
                head: 18.0,
                pellets: 9,
                spread: 0.055,
                mag: 6,
                reload: 0.45,
                shell_reload: true,
                ads_time: 0.0,
                falloff_start: 6.0,
                falloff_end: 12.0,
                falloff_mult: 0.0,
                projectile_speed: 0.0,
                recoil: 3.2,
            },
            // Three-round burst of travelling projectiles: 132 damage on a
            // full head burst, so one clean burst kills.
            Weapon::Arc => WeaponStats {
                name: "Arc",
                interval: 0.36,
                burst: 3,
                burst_interval: 0.06,
                body: 26.0,
                head: 44.0,
                pellets: 1,
                spread: 0.004,
                mag: 24,
                reload: 2.1,
                shell_reload: false,
                ads_time: 0.22,
                falloff_start: 200.0,
                falloff_end: 200.0,
                falloff_mult: 1.0,
                projectile_speed: 180.0,
                recoil: 1.1,
            },
            // Universal sidearm. Two head shots kill.
            Weapon::Tack => WeaponStats {
                name: "Tack",
                interval: 0.22,
                burst: 1,
                burst_interval: 0.0,
                body: 30.0,
                head: 65.0,
                pellets: 1,
                spread: 0.004,
                mag: 8,
                reload: 1.3,
                shell_reload: false,
                ads_time: 0.16,
                falloff_start: 20.0,
                falloff_end: 34.0,
                falloff_mult: 0.6,
                projectile_speed: 0.0,
                recoil: 1.4,
            },
            // Airdrop prize: two body shots kill.
            Weapon::Lance => WeaponStats {
                name: "Lance",
                interval: 0.42,
                burst: 1,
                burst_interval: 0.0,
                body: 55.0,
                head: 110.0,
                pellets: 1,
                spread: 0.0,
                mag: 5,
                reload: 2.4,
                shell_reload: false,
                ads_time: 0.3,
                falloff_start: 200.0,
                falloff_end: 200.0,
                falloff_mult: 1.0,
                projectile_speed: 0.0,
                recoil: 3.0,
            },
            // No gun at all. Fire and melee both swing the knife; a clean
            // front hit kills, so choosing it is a commitment, not a handicap.
            Weapon::Blade => WeaponStats {
                name: "Blade",
                interval: 10.0,
                burst: 1,
                burst_interval: 0.0,
                body: 0.0,
                head: 0.0,
                pellets: 0,
                spread: 0.0,
                mag: 0,
                reload: 10.0,
                shell_reload: false,
                ads_time: 0.1,
                falloff_start: 0.0,
                falloff_end: 0.0,
                falloff_mult: 0.0,
                projectile_speed: 0.0,
                recoil: 0.0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub name: &'static str,
    /// Seconds between shots (or between bursts).
    pub interval: f32,
    pub burst: u8,
    pub burst_interval: f32,
    pub body: f32,
    pub head: f32,
    pub pellets: u8,
    /// Radians of cone spread, per pellet.
    pub spread: f32,
    pub mag: i32,
    pub reload: f32,
    /// Shotguns reload one shell at a time and can be interrupted.
    pub shell_reload: bool,
    pub ads_time: f32,
    pub falloff_start: f32,
    pub falloff_end: f32,
    /// Damage multiplier at and past `falloff_end`.
    pub falloff_mult: f32,
    /// 0.0 means hitscan.
    pub projectile_speed: f32,
    /// Vertical kick in degrees per shot, purely a client-side feel value.
    pub recoil: f32,
}

impl WeaponStats {
    pub fn is_hitscan(&self) -> bool {
        self.projectile_speed <= 0.0
    }
    pub fn falloff(&self, distance: f32) -> f32 {
        if distance <= self.falloff_start {
            1.0
        } else if distance >= self.falloff_end {
            self.falloff_mult
        } else {
            let t = (distance - self.falloff_start) / (self.falloff_end - self.falloff_start);
            1.0 + (self.falloff_mult - 1.0) * t
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Character {
    Ward = 0,
    Vane = 1,
    Echo = 2,
    Kiln = 3,
}

impl Character {
    pub fn from_u8(v: u8) -> Character {
        match v {
            1 => Character::Vane,
            2 => Character::Echo,
            3 => Character::Kiln,
            _ => Character::Ward,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Character::Ward => "Ward",
            Character::Vane => "Vane",
            Character::Echo => "Echo",
            Character::Kiln => "Kiln",
        }
    }
    /// Plating. Armour soaks body damage first; head shots ignore it entirely.
    pub fn armor(self) -> i32 {
        match self {
            Character::Ward => 25,
            _ => 0,
        }
    }
    /// Softstep gives Vane 10% base movement and 15% faster reloads.
    pub fn speed_mult(self) -> f32 {
        match self {
            Character::Vane => 1.10,
            _ => 1.0,
        }
    }
    pub fn reload_mult(self) -> f32 {
        match self {
            Character::Vane => 1.0 / 1.15,
            _ => 1.0,
        }
    }
    pub fn ability_cooldown(self) -> f32 {
        match self {
            Character::Ward => 18.0,
            Character::Vane => 10.0,
            Character::Echo => 22.0,
            Character::Kiln => 20.0,
        }
    }
    pub fn ability_name(self) -> &'static str {
        match self {
            Character::Ward => "Shimmer",
            Character::Vane => "Blink",
            Character::Echo => "Pulse",
            Character::Kiln => "Cinderline",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    Skirmish = 0,
    Headhunt = 1,
    Uplink = 2,
    LastLight = 3,
}

impl Mode {
    pub fn from_u8(v: u8) -> Mode {
        match v {
            1 => Mode::Headhunt,
            2 => Mode::Uplink,
            3 => Mode::LastLight,
            _ => Mode::Skirmish,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::Skirmish => "Skirmish",
            Mode::Headhunt => "Headhunt",
            Mode::Uplink => "Uplink",
            Mode::LastLight => "Last Light",
        }
    }
    /// Match length in seconds.
    pub fn duration(self) -> f32 {
        match self {
            Mode::Skirmish => 240.0,
            Mode::Headhunt => 210.0,
            Mode::Uplink => 300.0,
            Mode::LastLight => 300.0,
        }
    }
    pub fn respawn_delay(self) -> f32 {
        match self {
            Mode::Skirmish => 3.0,
            Mode::Headhunt => 2.0,
            Mode::Uplink => 4.0,
            Mode::LastLight => f32::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Weather {
    Clear = 0,
    Rain = 1,
    Night = 2,
}

impl Weather {
    pub fn from_u8(v: u8) -> Weather {
        match v {
            1 => Weather::Rain,
            2 => Weather::Night,
            _ => Weather::Clear,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Rain => "Rain",
            Weather::Night => "Night",
        }
    }
    /// How far a player can be resolved. Simulated because it gates bot
    /// perception and the client's fog distance from the same number.
    pub fn sight_range(self) -> f32 {
        match self {
            Weather::Clear => 120.0,
            Weather::Rain => 45.0,
            Weather::Night => 30.0,
        }
    }
    /// Multiplier on the radius at which footsteps are audible.
    pub fn audio_mult(self) -> f32 {
        match self {
            Weather::Rain => 0.7,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MapId {
    Vault = 0,
    Depot = 1,
    Terrace = 2,
    Substation = 3,
}

impl MapId {
    pub fn from_u8(v: u8) -> MapId {
        match v {
            1 => MapId::Depot,
            2 => MapId::Terrace,
            3 => MapId::Substation,
            _ => MapId::Vault,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            MapId::Vault => "Vault",
            MapId::Depot => "Depot",
            MapId::Terrace => "Terrace",
            MapId::Substation => "Substation",
        }
    }
}

/// A solid box of level geometry.
#[derive(Clone, Copy, Debug)]
pub struct Brush {
    pub aabb: Aabb,
    /// Thin cover: Arc's projectiles punch through it at half damage.
    pub thin: bool,
    /// Breakable glass (Terrace atrium). Shot out, it stops colliding.
    pub glass: bool,
    /// Set when glass has been broken. A broken brush is skipped by every
    /// collision and trace query, and the client is told to stop drawing it.
    pub broken: bool,
    /// Scenery: a rock, a tree trunk, a hedge. The simulation treats it like
    /// any other brush; the renderer dresses it as terrain instead of as
    /// architecture, and the height rule above does not apply to it.
    pub natural: bool,
}

pub struct MapData {
    pub id: MapId,
    pub brushes: Vec<Brush>,
    pub spawns_a: Vec<Vec3>,
    pub spawns_b: Vec<Vec3>,
    pub center: Vec3,
    /// Uplink terminal positions, cycled after every bank.
    pub terminals: Vec<Vec3>,
    pub bounds: Aabb,
}

fn solid(cx: f32, cy: f32, cz: f32, hx: f32, hy: f32, hz: f32) -> Brush {
    Brush {
        aabb: Aabb::from_center(v3(cx, cy, cz), v3(hx, hy, hz)),
        thin: false,
        glass: false,
        broken: false,
        natural: false,
    }
}

fn thin(cx: f32, cy: f32, cz: f32, hx: f32, hy: f32, hz: f32) -> Brush {
    Brush {
        aabb: Aabb::from_center(v3(cx, cy, cz), v3(hx, hy, hz)),
        thin: true,
        glass: false,
        broken: false,
        natural: false,
    }
}

fn glass(cx: f32, cy: f32, cz: f32, hx: f32, hy: f32, hz: f32) -> Brush {
    Brush {
        aabb: Aabb::from_center(v3(cx, cy, cz), v3(hx, hy, hz)),
        thin: true,
        glass: true,
        broken: false,
        natural: false,
    }
}

/// Terrain: rock, boulder, tree trunk, hedge. Collides exactly like `solid`,
/// but the renderer gives it an organic silhouette instead of a box.
fn nature(cx: f32, cy: f32, cz: f32, hx: f32, hy: f32, hz: f32) -> Brush {
    Brush {
        aabb: Aabb::from_center(v3(cx, cy, cz), v3(hx, hy, hz)),
        thin: false,
        glass: false,
        broken: false,
        natural: true,
    }
}

/// Floor plus four perimeter walls, shared by every map.
fn shell(hx: f32, hz: f32, height: f32) -> Vec<Brush> {
    vec![
        solid(0.0, -1.0, 0.0, hx + 2.0, 1.0, hz + 2.0),
        solid(0.0, height * 0.5, hz + 1.0, hx + 2.0, height * 0.5, 1.0),
        solid(0.0, height * 0.5, -hz - 1.0, hx + 2.0, height * 0.5, 1.0),
        solid(hx + 1.0, height * 0.5, 0.0, 1.0, height * 0.5, hz + 2.0),
        solid(-hx - 1.0, height * 0.5, 0.0, 1.0, height * 0.5, hz + 2.0),
    ]
}

pub fn load_map(id: MapId) -> MapData {
    match id {
        // Symmetric interior. A wide central hall with catwalks above, flanked
        // by two tight record-stack corridors, one clean sightline down the
        // length of the hall.
        MapId::Vault => {
            let mut b = shell(18.0, 26.0, 7.0);
            // Corridor dividers: full walls, so the flanking routes are
            // committed rather than half-vaultable.
            for z in [-18.0f32, -6.0, 6.0, 18.0] {
                b.push(solid(-9.5, 2.0, z, 0.6, 2.0, 4.0));
                b.push(solid(9.5, 2.0, z, 0.6, 2.0, 4.0));
            }
            // Record stacks inside the flanking corridors: 1.1 m, so every
            // one of them is cover you can also stand on.
            for z in [-21.0f32, -13.0, -3.0, 7.0, 17.0] {
                b.push(solid(-14.0, 0.55, z, 2.5, 0.55, 1.2));
                b.push(solid(14.0, 0.55, z, 2.5, 0.55, 1.2));
            }
            // Hall furniture: mantle-height crates and two full-height pillars.
            b.push(solid(0.0, 0.55, -8.0, 3.0, 0.55, 1.2));
            b.push(solid(0.0, 0.55, 8.0, 3.0, 0.55, 1.2));
            b.push(solid(-4.0, 2.5, 0.0, 0.8, 2.5, 0.8));
            b.push(solid(4.0, 2.5, 0.0, 0.8, 2.5, 0.8));
            // Catwalks over the hall, reachable from the end stairs.
            b.push(solid(-6.0, 3.2, 0.0, 1.6, 0.2, 20.0));
            b.push(solid(6.0, 3.2, 0.0, 1.6, 0.2, 20.0));
            b.push(solid(0.0, 3.2, -20.0, 7.6, 0.2, 2.0));
            b.push(solid(0.0, 3.2, 20.0, 7.6, 0.2, 2.0));
            // End stairs up to the catwalk decks. Three treads at 1.0 / 2.0 /
            // 3.0, then a 0.4 m step onto the 3.4 m deck: every rise is inside
            // a single jump, which the old two-block ramp's 2.0 m first step
            // was not.
            for s in [1.0f32, -1.0] {
                b.push(solid(0.0, 0.5, 25.2 * s, 3.0, 0.5, 0.6));
                b.push(solid(0.0, 1.0, 24.0 * s, 3.0, 1.0, 0.6));
                b.push(solid(0.0, 1.5, 22.8 * s, 3.0, 1.5, 0.8));
            }
            // Thin partitions Arc can shoot through, at divider height.
            b.push(thin(-9.5, 2.0, 0.0, 0.3, 2.0, 2.0));
            b.push(thin(9.5, 2.0, 0.0, 0.3, 2.0, 2.0));
            // Vaultable mid-hall barrier: 0.9 m, cleared with one jump.
            b.push(solid(0.0, 0.45, 0.0, 4.0, 0.45, 0.5));
            // Crate stairways up to the catwalks mid-hall, mirrored on both
            // sides and both ends: 0.9 / 1.8 / 2.7 tops, then a hop onto the
            // 3.4 m deck. A new route that does not use the end ramps.
            for (sx, sz) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                b.push(solid(7.9 * sx, 0.45, 10.0 * sz, 0.7, 0.45, 0.7));
                b.push(solid(7.9 * sx, 0.9, 11.6 * sz, 0.7, 0.9, 0.7));
                b.push(solid(7.9 * sx, 1.35, 13.2 * sz, 0.7, 1.35, 0.7));
            }
            MapData {
                id,
                brushes: b,
                spawns_a: vec![
                    v3(-7.0, 0.1, -24.5),
                    v3(7.0, 0.1, -24.5),
                    v3(-14.0, 0.1, -24.5),
                    v3(14.0, 0.1, -24.5),
                ],
                spawns_b: vec![
                    v3(-7.0, 0.1, 24.5),
                    v3(7.0, 0.1, 24.5),
                    v3(-14.0, 0.1, 24.5),
                    v3(14.0, 0.1, 24.5),
                ],
                center: v3(0.0, 0.1, 0.0),
                terminals: vec![v3(0.0, 0.1, -16.0), v3(-14.0, 0.1, 0.0), v3(14.0, 0.1, 0.0)],
                bounds: Aabb::from_center(v3(0.0, 4.0, 0.0), v3(20.0, 6.0, 28.0)),
            }
        }
        // Container maze with long straight lanes, a raised gantry, and a
        // flooded pit at centre.
        MapId::Depot => {
            let mut b = shell(22.0, 22.0, 9.0);
            let mut rows = 0;
            for gx in [-16.0f32, -8.0, 8.0, 16.0] {
                for gz in [-16.0f32, -8.0, 0.0, 8.0, 16.0] {
                    rows += 1;
                    // Either a stacked container you cannot climb, or a
                    // single one at mantle height that you can. Nothing in
                    // between, so a lane always reads as either blocked or
                    // crossable at a glance.
                    let h = if rows % 3 == 0 { 1.7 } else { 0.55 };
                    b.push(Brush {
                        aabb: Aabb::from_center(v3(gx, h, gz), v3(3.0, h, 1.4)),
                        thin: rows % 4 == 0,
                        glass: false,
                        broken: false,
                        natural: false,
                    });
                }
            }
            // Raised gantry across the middle.
            b.push(solid(0.0, 4.0, 0.0, 20.0, 0.2, 1.6));
            b.push(solid(-20.0, 2.0, 0.0, 1.0, 2.0, 1.6));
            b.push(solid(20.0, 2.0, 0.0, 1.0, 2.0, 1.6));
            // Low walls around the flooded pit.
            b.push(solid(0.0, 0.35, -5.0, 5.0, 0.35, 0.4));
            b.push(solid(0.0, 0.35, 5.0, 5.0, 0.35, 0.4));
            // Crate stairways up to the central gantry (1.0 / 2.1 / 3.2 tops,
            // then a 1.0 m hop onto the 4.2 m walkway), mirrored through the
            // origin to keep the diagonal symmetry.
            for s in [1.0f32, -1.0] {
                b.push(solid(3.0 * s, 0.5, 6.2 * s, 0.8, 0.5, 0.8));
                b.push(solid(3.0 * s, 1.05, 4.6 * s, 0.8, 1.05, 0.8));
                b.push(solid(3.0 * s, 1.6, 3.0 * s, 0.8, 1.6, 0.8));
            }
            // Boulders between the container rows: jumpable cover that reads
            // as terrain rather than cargo.
            for (bx, bz) in [(12.0f32, -4.0f32), (-12.0, 4.0), (4.0, 12.0), (-4.0, -12.0)] {
                b.push(nature(bx, 0.45, bz, 1.1, 0.45, 1.3));
            }
            // A stand of trees along the north-west lane: cover that is not
            // cargo, and the only thing on Depot taller than the containers.
            for (tx, tz) in [(-18.0f32, 12.0f32), (18.0, -12.0), (-6.0, 19.5), (6.0, -19.5)] {
                b.push(nature(tx, 0.35, tz, 0.9, 0.35, 0.9));
                b.push(nature(tx, 1.55, tz, 0.28, 1.2, 0.28));
            }
            MapData {
                id,
                brushes: b,
                spawns_a: vec![
                    v3(-19.0, 0.1, -19.0),
                    v3(-12.0, 0.1, -19.0),
                    v3(-19.0, 0.1, -12.0),
                    v3(-5.0, 0.1, -20.0),
                ],
                spawns_b: vec![
                    v3(19.0, 0.1, 19.0),
                    v3(12.0, 0.1, 19.0),
                    v3(19.0, 0.1, 12.0),
                    v3(5.0, 0.1, 20.0),
                ],
                center: v3(0.0, 0.1, 0.0),
                terminals: vec![v3(-14.0, 0.1, 12.0), v3(14.0, 0.1, -12.0), v3(0.0, 0.1, 18.0)],
                bounds: Aabb::from_center(v3(0.0, 5.0, 0.0), v3(24.0, 7.0, 24.0)),
            }
        }
        // Rooftop restaurant: a breakable glass atrium at the centre, kitchen
        // corridor on one flank, open terrace on the other.
        MapId::Terrace => {
            let mut b = shell(16.0, 20.0, 8.0);
            // Atrium: four glass panes around an open middle.
            b.push(glass(-3.5, 1.7, -4.0, 3.5, 1.7, 0.15));
            b.push(glass(3.5, 1.7, -4.0, 3.5, 1.7, 0.15));
            b.push(glass(-3.5, 1.7, 4.0, 3.5, 1.7, 0.15));
            b.push(glass(3.5, 1.7, 4.0, 3.5, 1.7, 0.15));
            b.push(glass(-7.0, 1.7, 0.0, 0.15, 1.7, 4.0));
            b.push(glass(7.0, 1.7, 0.0, 0.15, 1.7, 4.0));
            // Kitchen corridor: full-height partition walls.
            b.push(solid(-11.0, 1.7, -6.0, 0.5, 1.7, 8.0));
            b.push(solid(-11.0, 1.7, 10.0, 0.5, 1.7, 5.0));
            // Kitchen counters: 1.1 m, so the corridor can be fought over the
            // top of them and crossed on top of them.
            for z in [-12.0f32, -4.0, 4.0, 12.0] {
                b.push(solid(-13.5, 0.55, z, 2.0, 0.55, 1.0));
            }
            // Terrace furniture, same rule.
            for z in [-10.0f32, 0.0, 10.0] {
                b.push(solid(12.0, 0.55, z, 2.5, 0.55, 1.5));
            }
            // Awning deck with a one-way drop onto the terrace, and the
            // corner pier that carries it, built flush with the deck.
            b.push(solid(9.0, 3.0, -16.0, 6.0, 0.2, 3.0));
            b.push(solid(14.0, 1.6, -19.0, 1.5, 1.6, 1.0));
            // Crates that climb up to the awning deck: 1.0 then 2.1, then a
            // 1.1 m hop onto the deck — a second way up besides the drop.
            b.push(solid(3.5, 0.5, -16.0, 0.8, 0.5, 0.8));
            b.push(solid(5.2, 1.05, -16.0, 0.8, 1.05, 0.8));
            // Matching platform and crates on the B side so neither team owns
            // the only high ground.
            b.push(solid(9.0, 3.0, 16.0, 6.0, 0.2, 3.0));
            b.push(solid(14.0, 1.6, 19.0, 1.5, 1.6, 1.0));
            b.push(solid(3.5, 0.5, 16.0, 0.8, 0.5, 0.8));
            b.push(solid(5.2, 1.05, 16.0, 0.8, 1.05, 0.8));
            // Planters with trees: a solid jumpable base and a slim trunk the
            // renderer dresses with a canopy. Real cover, natural silhouette.
            for (px, pz) in [(-13.5f32, 7.0f32), (13.5, 7.0), (-13.5, -7.0), (13.5, -7.0)] {
                b.push(nature(px, 0.35, pz, 0.9, 0.35, 0.9));
                b.push(nature(px, 1.55, pz, 0.28, 1.2, 0.28));
            }
            // Hedge rows flanking the atrium: 0.75 m, vaultable.
            b.push(nature(0.0, 0.375, -8.0, 2.6, 0.375, 0.5));
            b.push(nature(0.0, 0.375, 8.0, 2.6, 0.375, 0.5));
            MapData {
                id,
                brushes: b,
                spawns_a: vec![
                    v3(-8.0, 0.1, -17.0),
                    v3(0.0, 0.1, -18.0),
                    v3(8.0, 0.1, -17.0),
                    v3(-13.0, 0.1, -16.0),
                ],
                spawns_b: vec![
                    v3(-8.0, 0.1, 17.0),
                    v3(0.0, 0.1, 18.0),
                    v3(8.0, 0.1, 17.0),
                    v3(13.0, 0.1, 16.0),
                ],
                center: v3(0.0, 0.1, 0.0),
                terminals: vec![v3(-13.0, 0.1, 0.0), v3(12.0, 0.1, 6.0), v3(0.0, 0.1, -12.0)],
                bounds: Aabb::from_center(v3(0.0, 4.5, 0.0), v3(18.0, 6.0, 22.0)),
            }
        }
        // The open map: a long central approach, two bypass tunnels, and a
        // raised control room overlooking everything.
        MapId::Substation => {
            let mut b = shell(20.0, 28.0, 10.0);
            for z in [-18.0f32, -6.0, 6.0, 18.0] {
                b.push(solid(-9.0, 1.8, z, 2.2, 1.8, 2.2));
                b.push(solid(9.0, 1.8, z, 2.2, 1.8, 2.2));
            }
            // Tunnel walls.
            b.push(solid(-16.0, 2.0, 0.0, 0.6, 2.0, 20.0));
            b.push(solid(16.0, 2.0, 0.0, 0.6, 2.0, 20.0));
            b.push(solid(-18.5, 2.0, 0.0, 0.6, 2.0, 20.0));
            b.push(solid(18.5, 2.0, 0.0, 0.6, 2.0, 20.0));
            b.push(solid(-17.2, 4.2, 0.0, 1.9, 0.2, 20.0));
            b.push(solid(17.2, 4.2, 0.0, 1.9, 0.2, 20.0));
            // Control room deck at centre, reached from either end by three
            // treads at 1.1 / 2.2 / 3.15 and a last 0.5 m step onto the deck.
            // Every rise clears in one jump; the pair of 2.4 m blocks this
            // replaces could not be climbed from the floor at all.
            b.push(solid(0.0, 3.4, 0.0, 5.0, 0.25, 5.0));
            for s in [1.0f32, -1.0] {
                b.push(solid(0.0, 0.55, 9.8 * s, 2.0, 0.55, 1.2));
                b.push(solid(0.0, 1.1, 8.6 * s, 2.0, 1.1, 1.2));
                b.push(solid(0.0, 1.575, 6.2 * s, 2.0, 1.575, 1.2));
            }
            // Shutters down each flank of the control room: thin, so Arc can
            // contest the deck through them, and fixed, so the sightline they
            // leave is the same one every round.
            b.push(thin(-5.0, 4.6, 0.0, 0.2, 1.2, 5.0));
            b.push(thin(5.0, 4.6, 0.0, 0.2, 1.2, 5.0));
            // Boulders along the open approach: jumpable tops, natural cover
            // on an otherwise architectural map. Mirrored in z.
            for (bx, bz) in [(5.0f32, 12.0f32), (-5.0, 12.0), (5.0, -12.0), (-5.0, -12.0)] {
                b.push(nature(bx, 0.45, bz, 1.1, 0.45, 1.3));
            }
            // Bigger rocks near the corners: full hides you can also mantle
            // from the low boulder side (1.1 m tops).
            for (bx, bz) in [(13.5f32, 18.0f32), (-13.5, 18.0), (13.5, -18.0), (-13.5, -18.0)] {
                b.push(nature(bx, 0.55, bz, 1.6, 0.55, 1.2));
            }
            // Trees down both flanks of the approach: the only tall cover on
            // the open half of the map, and the only thing on it that is not
            // poured concrete.
            for (tx, tz) in [(-12.5f32, 4.0f32), (12.5, 4.0), (-12.5, -4.0), (12.5, -4.0)] {
                b.push(nature(tx, 0.35, tz, 0.9, 0.35, 0.9));
                b.push(nature(tx, 1.55, tz, 0.28, 1.2, 0.28));
            }
            MapData {
                id,
                brushes: b,
                spawns_a: vec![
                    v3(0.0, 0.1, -25.0),
                    v3(-9.0, 0.1, -25.0),
                    v3(9.0, 0.1, -25.0),
                    v3(-17.0, 0.1, -22.0),
                ],
                spawns_b: vec![
                    v3(0.0, 0.1, 25.0),
                    v3(-9.0, 0.1, 25.0),
                    v3(9.0, 0.1, 25.0),
                    v3(17.0, 0.1, 22.0),
                ],
                center: v3(0.0, 0.1, 0.0),
                terminals: vec![v3(0.0, 0.1, -16.0), v3(-12.0, 0.1, 10.0), v3(12.0, 0.1, 10.0)],
                bounds: Aabb::from_center(v3(0.0, 5.0, 0.0), v3(22.0, 7.0, 30.0)),
            }
        }
    }
}
