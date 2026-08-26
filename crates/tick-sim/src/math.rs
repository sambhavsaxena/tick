//! Minimal deterministic vector math.
//!
//! Every transcendental goes through `libm` rather than the platform libm, so
//! that the native server build and the `wasm32` client build produce bit
//! identical results. Basic f32 arithmetic and `sqrt` are IEEE-754 exact
//! everywhere, so they are used directly.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);

    pub fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, s: f32) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn len(self) -> f32 {
        sqrt(self.dot(self))
    }
    pub fn len_xz(self) -> f32 {
        sqrt(self.x * self.x + self.z * self.z)
    }
    pub fn normalized(self) -> Vec3 {
        let l = self.len();
        if l > 1e-6 {
            self.scale(1.0 / l)
        } else {
            Vec3::ZERO
        }
    }
}

pub fn sqrt(v: f32) -> f32 {
    libm::sqrtf(v)
}
pub fn sin(v: f32) -> f32 {
    libm::sinf(v)
}
pub fn cos(v: f32) -> f32 {
    libm::cosf(v)
}
pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}
pub fn asin(v: f32) -> f32 {
    libm::asinf(v)
}
pub fn abs(v: f32) -> f32 {
    if v < 0.0 {
        -v
    } else {
        v
    }
}
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Yaw/pitch (radians) to a unit forward vector. Yaw 0 looks down +Z.
pub fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let cp = cos(pitch);
    v3(sin(yaw) * cp, sin(pitch), cos(yaw) * cp)
}

/// Axis aligned box, used for both level geometry and player hitboxes.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn from_center(center: Vec3, half: Vec3) -> Aabb {
        Aabb {
            min: center.sub(half),
            max: center.add(half),
        }
    }
    pub fn center(&self) -> Vec3 {
        v3(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }
    pub fn half(&self) -> Vec3 {
        v3(
            (self.max.x - self.min.x) * 0.5,
            (self.max.y - self.min.y) * 0.5,
            (self.max.z - self.min.z) * 0.5,
        )
    }
    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Slab ray/box intersection. Returns the entry distance along `dir`
    /// (assumed normalized) when the ray hits within `max_t`.
    pub fn ray(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let mut tmin = 0.0f32;
        let mut tmax = max_t;
        let o = [origin.x, origin.y, origin.z];
        let d = [dir.x, dir.y, dir.z];
        let lo = [self.min.x, self.min.y, self.min.z];
        let hi = [self.max.x, self.max.y, self.max.z];
        for i in 0..3 {
            if abs(d[i]) < 1e-6 {
                if o[i] < lo[i] || o[i] > hi[i] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d[i];
            let mut t1 = (lo[i] - o[i]) * inv;
            let mut t2 = (hi[i] - o[i]) * inv;
            if t1 > t2 {
                core::mem::swap(&mut t1, &mut t2);
            }
            if t1 > tmin {
                tmin = t1;
            }
            if t2 < tmax {
                tmax = t2;
            }
            if tmin > tmax {
                return None;
            }
        }
        Some(tmin)
    }
}

/// Deterministic small-state PRNG (SplitMix64). Every random decision in the
/// simulation draws from one of these, seeded per match, so a match replays
/// identically from its seed.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn next_u32(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as u32
    }
    /// Uniform in [0,1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }
    /// Uniform in [-1,1).
    pub fn next_signed(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}
