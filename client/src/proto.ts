// Wire format, mirroring crates/tick-server/src/proto.rs. Snapshots and inputs
// are hand-packed binary because that is where the bandwidth is; everything
// else on the socket is JSON.

export const MSG_INPUT = 1;
export const MSG_SNAPSHOT = 2;

export const BTN = {
  FWD: 1 << 0,
  BACK: 1 << 1,
  LEFT: 1 << 2,
  RIGHT: 1 << 3,
  JUMP: 1 << 4,
  CROUCH: 1 << 5,
  FIRE: 1 << 6,
  ADS: 1 << 7,
  ABILITY: 1 << 8,
  RELOAD: 1 << 9,
  SPRINT: 1 << 10,
  MELEE: 1 << 11,
} as const;

export interface InputCmd {
  seq: number;
  yaw: number;
  pitch: number;
  buttons: number;
}

/** Pack up to three inputs, so one dropped packet costs nothing. */
export function encodeInput(inputs: InputCmd[], interpTicks: number): ArrayBuffer {
  const n = Math.min(inputs.length, 3);
  const buf = new ArrayBuffer(3 + n * 10);
  const v = new DataView(buf);
  v.setUint8(0, MSG_INPUT);
  v.setUint8(1, n);
  v.setUint8(2, Math.max(0, Math.min(13, Math.round(interpTicks))));
  let o = 3;
  for (let i = inputs.length - n; i < inputs.length; i++) {
    const c = inputs[i];
    v.setUint32(o, c.seq, true);
    v.setInt16(o + 4, Math.round(clamp(c.yaw, -3.2, 3.2) * 10000), true);
    v.setInt16(o + 6, Math.round(clamp(c.pitch, -3.2, 3.2) * 10000), true);
    v.setUint16(o + 8, c.buttons, true);
    o += 10;
  }
  return buf;
}

export interface SnapPlayer {
  slot: number;
  alive: boolean;
  crouching: boolean;
  staggered: boolean;
  marked: boolean;
  team: number;
  carrying: boolean;
  firing: boolean;
  ads: boolean;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  health: number;
  armor: number;
  weapon: number;
  score: number;
}

export interface Snapshot {
  tick: number;
  ack: number;
  you: number;
  timeLeft: number;
  scoreA: number;
  scoreB: number;
  eventBits: number;
  fogRadius: number;
  weather: number;
  round: number;
  vel: [number, number, number];
  onGround: boolean;
  charge: number;
  focus: number;
  abilityCooldown: number;
  ammo: number;
  reload: number;
  players: SnapPlayer[];
  shimmers: { x: number; y: number; z: number; yaw: number; team: number }[];
  cinders: { ax: number; ay: number; az: number; bx: number; by: number; bz: number; team: number }[];
  pickups: { x: number; y: number; z: number; weapon: number }[];
  /**
   * Uplink cores. One in a normal match; the Twin Core event adds a second.
   * `state` is 0 dormant, 1 loose on the ground, 2 being carried.
   */
  cores: { state: number; pos: [number, number, number]; carrier: number }[];
  terminalIndex: number;
  /** Slot whose eyes we are watching through while dead; 255 for nobody. */
  spectate: number;
  /** Wall-clock arrival time, used for entity interpolation. */
  received: number;
}

export function decodeSnapshot(data: ArrayBuffer): Snapshot | null {
  const v = new DataView(data);
  if (v.getUint8(0) !== MSG_SNAPSHOT) return null;
  let o = 1;
  const tick = v.getUint32(o, true); o += 4;
  const ack = v.getUint32(o, true); o += 4;
  const you = v.getUint8(o); o += 1;
  const timeLeft = v.getFloat32(o, true); o += 4;
  const scoreA = v.getInt32(o, true); o += 4;
  const scoreB = v.getInt32(o, true); o += 4;
  const eventBits = v.getUint16(o, true); o += 2;
  const fogRadius = v.getFloat32(o, true); o += 4;
  const weather = v.getUint8(o); o += 1;
  const round = v.getUint8(o); o += 1;
  const vel: [number, number, number] = [
    v.getFloat32(o, true),
    v.getFloat32(o + 4, true),
    v.getFloat32(o + 8, true),
  ];
  o += 12;
  const onGround = v.getUint8(o) !== 0; o += 1;
  const charge = v.getFloat32(o, true); o += 4;
  const focus = v.getFloat32(o, true); o += 4;
  const abilityCooldown = v.getFloat32(o, true); o += 4;
  const ammo = v.getInt32(o, true); o += 4;
  const reload = v.getFloat32(o, true); o += 4;

  const count = v.getUint8(o); o += 1;
  const players: SnapPlayer[] = [];
  for (let i = 0; i < count; i++) {
    const slot = v.getUint8(o); o += 1;
    const flags = v.getUint8(o); o += 1;
    const x = v.getFloat32(o, true); o += 4;
    const y = v.getFloat32(o, true); o += 4;
    const z = v.getFloat32(o, true); o += 4;
    const yaw = v.getInt16(o, true) / 10000; o += 2;
    const pitch = v.getInt16(o, true) / 10000; o += 2;
    const health = v.getUint8(o); o += 1;
    const armor = v.getUint8(o); o += 1;
    const weapon = v.getUint8(o); o += 1;
    const score = v.getInt32(o, true); o += 4;
    players.push({
      slot,
      alive: (flags & 1) !== 0,
      crouching: (flags & 2) !== 0,
      staggered: (flags & 4) !== 0,
      marked: (flags & 8) !== 0,
      team: (flags & 16) !== 0 ? 1 : 0,
      carrying: (flags & 32) !== 0,
      firing: (flags & 64) !== 0,
      ads: (flags & 128) !== 0,
      x, y, z, yaw, pitch, health, armor, weapon, score,
    });
  }

  const shimmers = [];
  const sc = v.getUint8(o); o += 1;
  for (let i = 0; i < sc; i++) {
    const x = v.getFloat32(o, true); o += 4;
    const y = v.getFloat32(o, true); o += 4;
    const z = v.getFloat32(o, true); o += 4;
    const yaw = v.getInt16(o, true) / 10000; o += 2;
    const team = v.getUint8(o); o += 1;
    shimmers.push({ x, y, z, yaw, team });
  }
  const cinders = [];
  const cc = v.getUint8(o); o += 1;
  for (let i = 0; i < cc; i++) {
    const ax = v.getFloat32(o, true); o += 4;
    const ay = v.getFloat32(o, true); o += 4;
    const az = v.getFloat32(o, true); o += 4;
    const bx = v.getFloat32(o, true); o += 4;
    const by = v.getFloat32(o, true); o += 4;
    const bz = v.getFloat32(o, true); o += 4;
    const team = v.getUint8(o); o += 1;
    cinders.push({ ax, ay, az, bx, by, bz, team });
  }
  const pickups = [];
  const pc = v.getUint8(o); o += 1;
  for (let i = 0; i < pc; i++) {
    const x = v.getFloat32(o, true); o += 4;
    const y = v.getFloat32(o, true); o += 4;
    const z = v.getFloat32(o, true); o += 4;
    const weapon = v.getUint8(o); o += 1;
    pickups.push({ x, y, z, weapon });
  }
  const cores = [];
  const coreCount = v.getUint8(o); o += 1;
  for (let i = 0; i < coreCount; i++) {
    const state = v.getUint8(o); o += 1;
    const pos: [number, number, number] = [
      v.getFloat32(o, true),
      v.getFloat32(o + 4, true),
      v.getFloat32(o + 8, true),
    ];
    o += 12;
    const carrier = v.getUint8(o); o += 1;
    cores.push({ state, pos, carrier });
  }
  const terminalIndex = v.getUint8(o); o += 1;
  // Trailing fields are read defensively. A server built from older sources
  // sends a shorter snapshot, and reading past the end throws a RangeError
  // that would silently swallow every snapshot from then on. Failing loudly
  // here is worth far more than a mystery: the symptom of a stale server is
  // otherwise just "one feature quietly does nothing".
  if (o >= v.byteLength) {
    warnShortSnapshot();
    return null;
  }
  const spectate = v.getUint8(o); o += 1;

  return {
    tick, ack, you, timeLeft, scoreA, scoreB, eventBits, fogRadius, weather, round,
    vel, onGround, charge, focus, abilityCooldown, ammo, reload,
    players, shimmers, cinders, pickups, cores, terminalIndex, spectate,
    received: performance.now(),
  };
}

let warnedShort = false;
function warnShortSnapshot() {
  if (warnedShort) return;
  warnedShort = true;
  console.error(
    "TICK: snapshot is shorter than this client expects — the server is " +
      "running older code than the browser bundle. Rebuild and restart it " +
      "(./run.sh), or `cargo build --release -p tick-server`.",
  );
}

function clamp(v: number, lo: number, hi: number) {
  return v < lo ? lo : v > hi ? hi : v;
}
