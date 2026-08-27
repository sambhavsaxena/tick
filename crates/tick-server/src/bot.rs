//! Bot players.
//!
//! Bots exist for one product reason: nobody should ever wait more than twelve
//! seconds for a match, and a player who leaves mid-match should not hand
//! their team a 3v4. A bot produces exactly the same `Input` a human client
//! sends, so from the simulation's point of view there is no difference
//! between the two — no special-cased physics, no server-side aimbot with a
//! different damage model.

use tick_sim::movement::buttons;
use tick_sim::*;

pub struct Bot {
    pub slot: u8,
    /// 0.0 = clumsy, 1.0 = unpleasant. Set from the lobby's skill target.
    pub skill: f32,
    rng: Rng,
    yaw: f32,
    pitch: f32,
    target: Option<u8>,
    /// Time left on the current strafe, used to peel around obstacles.
    strafe_timer: f32,
    strafe_dir: f32,
    repath_timer: f32,
    wander: Vec3,
    reaction: f32,
}

impl Bot {
    pub fn new(slot: u8, skill: f32, seed: u64) -> Bot {
        Bot {
            slot,
            skill,
            rng: Rng::new(seed ^ (slot as u64) << 32),
            yaw: 0.0,
            pitch: 0.0,
            target: None,
            strafe_timer: 0.0,
            strafe_dir: 1.0,
            repath_timer: 0.0,
            wander: Vec3::ZERO,
            reaction: 0.0,
        }
    }

    pub fn think(&mut self, w: &World, seq: u32) -> Input {
        let me = &w.players[self.slot as usize];
        if !me.alive {
            return Input {
                seq,
                yaw: self.yaw,
                pitch: self.pitch,
                buttons: 0,
            };
        }
        let eye = me.eye();
        let sight = w.weather.sight_range();

        // Acquire: nearest enemy with a clear line of sight inside the
        // weather's sight range. Bots are subject to the same visibility the
        // renderer is, so Night genuinely blinds them.
        let mut best: Option<(u8, f32)> = None;
        for p in &w.players {
            if p.team == me.team || !p.alive {
                continue;
            }
            let to = p.eye().sub(eye);
            let dist = to.len();
            if dist > sight {
                continue;
            }
            let dir = to.normalized();
            let t = movement::trace_sight(eye, dir, dist, &w.map.brushes);
            if t < dist - 0.4 {
                continue;
            }
            if best.map(|(_, d)| dist < d).unwrap_or(true) {
                best = Some((p.slot, dist));
            }
        }

        if best.map(|(s, _)| Some(s) != self.target).unwrap_or(false) {
            // Reaction time: better bots swing onto a new target faster.
            self.reaction = 0.36 - 0.22 * self.skill;
        }
        self.target = best.map(|(s, _)| s);
        self.reaction = (self.reaction - DT).max(0.0);

        let mut btn: u16 = 0;
        let mut goal: Vec3;

        if let Some((slot, dist)) = best {
            let tp = &w.players[slot as usize];
            // Aim at the head when close enough to be worth it, which is how
            // bots end up producing head shots without a separate code path.
            let aim_head = self.skill > 0.35 && dist < 40.0;
            let mut aim = if aim_head {
                tp.mv.pos.add(v3(0.0, HEAD_BOTTOM + 0.17, 0.0))
            } else {
                tp.mv.pos.add(v3(0.0, 0.9, 0.0))
            };
            // Lead the shot for travelling projectiles.
            let stats = me.weapon.stats();
            if !stats.is_hitscan() {
                let t = dist / stats.projectile_speed;
                aim = aim.add(tp.mv.vel.scale(t));
            }
            let to = aim.sub(eye);
            let want_yaw = atan2(to.x, to.z);
            let want_pitch = asin(clamp(to.normalized().y, -1.0, 1.0));

            // Turn at a bounded rate, with a jitter floor that never fully
            // disappears. A bot that snaps is a bot that reads as a cheat.
            let turn = (3.0 + 9.0 * self.skill) * DT;
            self.yaw = approach_angle(self.yaw, want_yaw, turn);
            self.pitch = approach_angle(self.pitch, want_pitch, turn);
            let jitter = (1.0 - self.skill) * 0.05 + 0.008;
            self.yaw += self.rng.next_signed() * jitter;
            self.pitch += self.rng.next_signed() * jitter * 0.5;

            let aligned = abs(angle_delta(self.yaw, want_yaw)) < 0.07
                && abs(angle_delta(self.pitch, want_pitch)) < 0.07;
            let in_range = dist < effective_range(me.weapon);
            if aligned && in_range && self.reaction <= 0.0 && me.ammo > 0 {
                btn |= buttons::FIRE;
                // Deliberately dumb: three out of four trigger pulls are
                // thrown wide. The offset is sized from the distance so it
                // always clears a body's half-width no matter the range.
                if self.rng.next_f32() < 0.75 {
                    let off = 0.7 / dist.max(2.0) + self.rng.next_f32() * 0.05;
                    let sign = if self.rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
                    self.yaw += sign * off;
                    self.pitch += self.rng.next_signed() * 0.04;
                }
            }
            if me.ammo == 0 {
                btn |= buttons::RELOAD;
            }
            if dist > 22.0 && stats.ads_time > 0.0 {
                btn |= buttons::ADS;
            }

            // Hold a comfortable range for the weapon in hand.
            let ideal = effective_range(me.weapon) * 0.55;
            goal = if dist > ideal + 4.0 {
                tp.mv.pos
            } else if dist < ideal - 4.0 {
                me.mv.pos.sub(tp.mv.pos.sub(me.mv.pos).normalized().scale(6.0))
            } else {
                // Strafe across the target rather than standing still.
                let side = v3(
                    cos(self.yaw) * self.strafe_dir,
                    0.0,
                    -sin(self.yaw) * self.strafe_dir,
                );
                me.mv.pos.add(side.scale(4.0))
            };
            if me.ability_cooldown <= 0.0 && dist < 18.0 && self.rng.next_f32() < 0.01 {
                btn |= buttons::ABILITY;
            }
        } else {
            // Nothing visible: head for the objective, or the map centre.
            self.repath_timer -= DT;
            if self.repath_timer <= 0.0 {
                self.repath_timer = 2.5;
                let c = w.map.center;
                self.wander = v3(
                    c.x + self.rng.next_signed() * w.map.bounds.max.x * 0.6,
                    0.0,
                    c.z + self.rng.next_signed() * w.map.bounds.max.z * 0.6,
                );
            }
            // Head for the nearest live, unheld core; with Twin Core running
            // that keeps the two teams from stacking on the same one.
            goal = match w.cfg_mode {
                Mode::Uplink => w
                    .cores
                    .iter()
                    .filter(|c| c.active && c.carrier.is_none())
                    .min_by(|a, b| {
                        let da = a.pos.sub(me.mv.pos).len();
                        let db = b.pos.sub(me.mv.pos).len();
                        da.partial_cmp(&db).unwrap_or(core::cmp::Ordering::Equal)
                    })
                    .map(|c| c.pos)
                    .unwrap_or(self.wander),
                _ => self.wander,
            };
            if me.carrying_core {
                goal = w.map.terminals[w.terminal_index % w.map.terminals.len()];
            }
            let to = goal.sub(me.mv.pos);
            if to.len_xz() > 1.0 {
                self.yaw = approach_angle(self.yaw, atan2(to.x, to.z), 4.0 * DT);
            }
            self.pitch = approach_angle(self.pitch, 0.0, 2.0 * DT);
            btn |= buttons::SPRINT;
        }

        // Walk toward the goal, peeling sideways when the path is blocked.
        let to_goal = goal.sub(me.mv.pos);
        if to_goal.len_xz() > 1.2 {
            let fwd = look_dir(self.yaw, 0.0);
            let want = to_goal.normalized();
            let dot = fwd.x * want.x + fwd.z * want.z;
            let right = v3(fwd.z, 0.0, -fwd.x);
            let side = right.x * want.x + right.z * want.z;

            let probe = me.mv.pos.add(v3(0.0, 0.6, 0.0));
            let clear = movement::trace_world(probe, want, 2.0, &w.map.brushes).t;
            self.strafe_timer -= DT;
            if clear < 1.6 && self.strafe_timer <= 0.0 {
                self.strafe_timer = 0.6;
                self.strafe_dir = if self.rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
            }
            if self.strafe_timer > 0.0 {
                btn |= if self.strafe_dir > 0.0 {
                    buttons::RIGHT
                } else {
                    buttons::LEFT
                };
                btn |= buttons::FWD;
            } else {
                if dot > 0.35 {
                    btn |= buttons::FWD;
                } else if dot < -0.35 {
                    btn |= buttons::BACK;
                }
                if side > 0.35 {
                    btn |= buttons::RIGHT;
                } else if side < -0.35 {
                    btn |= buttons::LEFT;
                }
            }
            // Hop over the low crates rather than grinding into them.
            if clear < 1.2 && me.mv.on_ground && self.rng.next_f32() < 0.08 {
                btn |= buttons::JUMP;
            }
        }

        Input {
            seq,
            yaw: self.yaw,
            pitch: self.pitch,
            buttons: btn,
        }
    }
}

fn effective_range(w: Weapon) -> f32 {
    match w {
        Weapon::Sting => 20.0,
        Weapon::Ridge => 60.0,
        Weapon::Maul => 8.0,
        Weapon::Arc => 32.0,
        Weapon::Tack => 18.0,
        Weapon::Lance => 60.0,
        // Knife range: a bot holding a Blade closes to melee distance, and
        // its FIRE press swings rather than shoots.
        Weapon::Blade => 2.0,
    }
}

fn angle_delta(a: f32, b: f32) -> f32 {
    let mut d = b - a;
    while d > core::f32::consts::PI {
        d -= core::f32::consts::TAU;
    }
    while d < -core::f32::consts::PI {
        d += core::f32::consts::TAU;
    }
    d
}

fn approach_angle(from: f32, to: f32, max_step: f32) -> f32 {
    let d = angle_delta(from, to);
    from + clamp(d, -max_step, max_step)
}
