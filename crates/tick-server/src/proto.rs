//! Wire format.
//!
//! Two channels over one socket. Control traffic — match setup, the kill feed,
//! event announcements, results — is JSON, because it is low frequency and
//! being readable in devtools is worth more than the bytes. The 32 Hz snapshot
//! stream and the 64 Hz input stream are hand-packed binary, because that is
//! where the bandwidth actually is.
//!
//! Every layout here has a mirror in `client/src/proto.ts`. Change one, change
//! the other.

use tick_sim::*;

pub const MSG_INPUT: u8 = 1;
pub const MSG_SNAPSHOT: u8 = 2;

/// Decoded client input packet. Each packet carries up to three inputs so that
/// a dropped datagram costs nothing: the next one re-delivers it.
pub struct InputPacket {
    pub inputs: Vec<Input>,
    /// Client's own estimate of how far behind it is rendering, in ticks.
    pub interp_ticks: u8,
}

pub fn decode_input(buf: &[u8]) -> Option<InputPacket> {
    if buf.len() < 3 || buf[0] != MSG_INPUT {
        return None;
    }
    let count = buf[1] as usize;
    let interp_ticks = buf[2];
    let mut inputs = Vec::with_capacity(count);
    let mut o = 3;
    for _ in 0..count {
        if o + 10 > buf.len() {
            break;
        }
        let seq = u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        let yaw = i16::from_le_bytes([buf[o + 4], buf[o + 5]]) as f32 / 10000.0;
        let pitch = i16::from_le_bytes([buf[o + 6], buf[o + 7]]) as f32 / 10000.0;
        let btn = u16::from_le_bytes([buf[o + 8], buf[o + 9]]);
        inputs.push(Input {
            seq,
            yaw,
            pitch,
            buttons: btn,
        });
        o += 10;
    }
    Some(InputPacket {
        inputs,
        interp_ticks,
    })
}

struct Writer(Vec<u8>);

impl Writer {
    fn new() -> Writer {
        Writer(Vec::with_capacity(512))
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    fn angle(&mut self, v: f32) {
        self.i16((clamp(v, -3.2, 3.2) * 10000.0) as i16);
    }
}

/// Build the snapshot for one recipient.
///
/// `visible` is the recipient's own visibility set. Players not in it are not
/// written at all — the client is never told where an enemy is that it could
/// not see. That is the anti-cheat design: a modified client has nothing to
/// read, because the data never left the server.
pub fn encode_snapshot(
    w: &World,
    recipient: u8,
    ack_seq: u32,
    visible: &[bool; MAX_PLAYERS],
) -> Vec<u8> {
    let mut b = Writer::new();
    b.u8(MSG_SNAPSHOT);
    b.u32(w.tick);
    b.u32(ack_seq);
    b.u8(recipient);
    b.f32(w.time_left.max(0.0));
    b.i32(w.team_score[0]);
    b.i32(w.team_score[1]);

    let mut event_bits: u16 = 0;
    for a in &w.active {
        event_bits |= 1 << (a.kind as u16);
    }
    b.u16(event_bits);
    b.f32(w.fog_radius);
    b.u8(w.weather as u8);
    b.u8(w.round as u8);

    // The recipient's own authoritative state, used for reconciliation. The
    // client rewinds to exactly this and replays its unacknowledged inputs.
    let me = &w.players[recipient as usize];
    b.vec3(me.mv.vel);
    b.u8(me.mv.on_ground as u8);
    b.f32(me.charge);
    b.f32(me.focus_timer);
    b.f32(me.ability_cooldown);
    b.i32(me.ammo);
    b.f32(me.reload_timer);

    let listed: Vec<&Player> = w
        .players
        .iter()
        .filter(|p| visible[p.slot as usize])
        .collect();
    b.u8(listed.len() as u8);
    for p in listed {
        b.u8(p.slot);
        let mut flags: u8 = 0;
        if p.alive {
            flags |= 1;
        }
        if p.mv.crouching {
            flags |= 2;
        }
        if p.staggered {
            flags |= 4;
        }
        if p.marked {
            flags |= 8;
        }
        if p.team == 1 {
            flags |= 16;
        }
        if p.carrying_core {
            flags |= 32;
        }
        if p.fire_cooldown > 0.0 {
            flags |= 64;
        }
        if p.ads > 0.5 {
            flags |= 128;
        }
        b.u8(flags);
        b.vec3(p.mv.pos);
        b.angle(p.yaw);
        b.angle(p.pitch);
        b.u8(p.health.clamp(0, 255) as u8);
        b.u8(p.armor.clamp(0, 255) as u8);
        b.u8(p.weapon as u8);
        b.i32(p.stats.score);
    }

    b.u8(w.shimmers.len().min(16) as u8);
    for s in w.shimmers.iter().take(16) {
        b.vec3(s.pos);
        b.angle(s.yaw);
        b.u8(s.team);
    }
    b.u8(w.cinders.len().min(16) as u8);
    for c in w.cinders.iter().take(16) {
        b.vec3(c.a);
        b.vec3(c.b);
        b.u8(c.team);
    }
    let live: Vec<&Pickup> = w.pickups.iter().filter(|p| !p.taken).collect();
    b.u8(live.len().min(8) as u8);
    for p in live.iter().take(8) {
        b.vec3(p.pos);
        b.u8(p.weapon as u8);
    }
    let core_state: u8 = if !w.core_active {
        0
    } else if w.core_carrier.is_some() {
        2
    } else {
        1
    };
    b.u8(core_state);
    b.vec3(w.core_pos);
    b.u8(w.core_carrier.unwrap_or(255));
    b.u8(w.terminal_index as u8);
    b.0
}
