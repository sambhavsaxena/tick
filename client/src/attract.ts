// The lobby's backdrop: a match playing behind the menu.
//
// Everything here is a puppet show. There is no server, no simulation and no
// prediction — the point is to answer "what does this game look like" before
// the player has spent a second in a queue, so it borrows the real renderer,
// the real map geometry and the real avatars, and drives them with the
// crudest possible AI.
//
// It deliberately does not touch `Sim.step`: the wasm simulation holds exactly
// one player's state, and that state belongs to the local player. Attract
// actors keep their own positions and walk the floor plane.

import type { Renderer, RenderPlayer } from "./render";
import type { Brush } from "./sim";

const PLAYER_RADIUS = 0.4;
const PLAYER_HEIGHT = 1.8;
const EYE = 1.62;
const WALK = 3.4;
/** How long the camera holds one angle before cutting to the next. */
const SHOT_LENGTH = 8.5;

interface Actor {
  x: number;
  z: number;
  yaw: number;
  team: number;
  character: number;
  /** Where it is walking. */
  tx: number;
  tz: number;
  /** Seconds until it takes its next shot. */
  fireIn: number;
  /** Seconds left of the current burst, and the gap between its shots. */
  burst: number;
  burstGap: number;
  aim: number;
  crouching: number;
}

/** Does a standing player at this spot overlap anything solid? */
function blocked(brushes: Brush[], x: number, z: number): boolean {
  for (const b of brushes) {
    if (b.broken) continue;
    if (
      x - PLAYER_RADIUS < b.max[0] &&
      x + PLAYER_RADIUS > b.min[0] &&
      0.1 < b.max[1] &&
      0.1 + PLAYER_HEIGHT > b.min[1] &&
      z - PLAYER_RADIUS < b.max[2] &&
      z + PLAYER_RADIUS > b.min[2]
    ) {
      return true;
    }
  }
  return false;
}

/**
 * Coarse line of sight between two eye points: twenty samples, rejected if
 * any of them lands inside a solid brush. Precision does not matter — this
 * only decides whether a tracer would look like it went through a wall.
 */
function clearShot(brushes: Brush[], a: number[], b: number[]): boolean {
  for (let i = 1; i < 20; i++) {
    const t = i / 20;
    const x = a[0] + (b[0] - a[0]) * t;
    const y = a[1] + (b[1] - a[1]) * t;
    const z = a[2] + (b[2] - a[2]) * t;
    for (const s of brushes) {
      if (s.broken || s.thin) continue;
      if (
        x > s.min[0] && x < s.max[0] &&
        y > s.min[1] && y < s.max[1] &&
        z > s.min[2] && z < s.max[2]
      ) {
        return false;
      }
    }
  }
  return true;
}

export class Attract {
  private actors: Actor[] = [];
  private brushes: Brush[] = [];
  private extent = { x: 20, z: 24 };
  private time = 0;
  /** Current camera shot: azimuth, height and the point it frames. */
  private shot = { angle: 0, height: 8, hold: 0, look: [0, 1.2, 0] as number[] };
  private eye: [number, number, number] = [0, 8, 0];

  constructor(private renderer: Renderer) {}

  /** Start over on a freshly built map. */
  reset(brushes: Brush[]) {
    this.brushes = brushes;
    let ex = 10;
    let ez = 10;
    for (const b of brushes) {
      ex = Math.max(ex, Math.abs(b.min[0]), Math.abs(b.max[0]));
      ez = Math.max(ez, Math.abs(b.min[2]), Math.abs(b.max[2]));
    }
    this.extent = { x: ex - 3.5, z: ez - 3.5 };

    this.actors = [];
    for (let i = 0; i < 8; i++) {
      const spot = this.freeSpot();
      this.actors.push({
        x: spot[0],
        z: spot[1],
        yaw: Math.random() * Math.PI * 2,
        team: i % 2,
        character: i % 4,
        tx: spot[0],
        tz: spot[1],
        fireIn: 0.5 + Math.random() * 3,
        burst: 0,
        burstGap: 0,
        aim: -1,
        crouching: 0,
      });
      this.retarget(this.actors[i]);
    }
    this.time = 0;
    this.shot.hold = 0;
    this.eye = [0, 8, 0];
  }

  private freeSpot(): [number, number] {
    for (let i = 0; i < 60; i++) {
      const x = (Math.random() - 0.5) * this.extent.x * 2;
      const z = (Math.random() - 0.5) * this.extent.z * 2;
      if (!blocked(this.brushes, x, z)) return [x, z];
    }
    return [0, 0];
  }

  private retarget(a: Actor) {
    const spot = this.freeSpot();
    a.tx = spot[0];
    a.tz = spot[1];
  }

  /** One frame of the show. Returns nothing; it drives the renderer itself. */
  step(dt: number) {
    if (this.actors.length === 0) return;
    this.time += dt;

    for (const a of this.actors) {
      this.stepActor(a, dt);
    }
    this.stepCamera(dt);

    const out: RenderPlayer[] = this.actors.map((a, i) => ({
      slot: i,
      x: a.x,
      y: 0.1,
      z: a.z,
      yaw: a.yaw,
      pitch: 0,
      team: a.team,
      character: a.character,
      alive: true,
      crouching: a.crouching > 0,
      marked: false,
      staggered: false,
      carrying: false,
      isLocal: false,
    }));
    this.renderer.syncPlayers(out);
    this.renderer.hideViewmodel = true;
    this.renderer.update(dt, this.eye, this.camYaw, this.camPitch, 0, 0);
  }

  private camYaw = 0;
  private camPitch = 0;

  private stepActor(a: Actor, dt: number) {
    // Walk toward the current waypoint, sliding one axis at a time so a
    // clipped corner costs a step rather than the whole route.
    const dx = a.tx - a.x;
    const dz = a.tz - a.z;
    const d = Math.hypot(dx, dz);
    if (d < 1.0) {
      this.retarget(a);
    } else {
      const step = (WALK * dt) / d;
      const nx = a.x + dx * step;
      const nz = a.z + dz * step;
      let moved = false;
      if (!blocked(this.brushes, nx, a.z)) {
        a.x = nx;
        moved = true;
      }
      if (!blocked(this.brushes, a.x, nz)) {
        a.z = nz;
        moved = true;
      }
      if (!moved) this.retarget(a);
      // Face the way it is walking, unless it is shooting at someone.
      if (a.aim < 0) {
        const want = Math.atan2(dx, dz);
        a.yaw += angleDelta(a.yaw, want) * Math.min(1, dt * 5);
      }
    }

    if (a.crouching > 0) a.crouching -= dt;

    // Shooting. A burst is a handful of tracers over a few tenths of a
    // second, aimed at whoever it picked when the burst started.
    if (a.burst > 0) {
      a.burst -= dt;
      a.burstGap -= dt;
      const target = this.actors[a.aim];
      if (target) {
        const want = Math.atan2(target.x - a.x, target.z - a.z);
        a.yaw += angleDelta(a.yaw, want) * Math.min(1, dt * 12);
      }
      if (a.burstGap <= 0 && target) {
        a.burstGap = 0.09;
        this.fire(a, target);
      }
      if (a.burst <= 0) {
        a.aim = -1;
        a.fireIn = 1.2 + Math.random() * 3.5;
        // Half the time, duck behind whatever it was shooting from.
        if (Math.random() < 0.5) a.crouching = 0.8 + Math.random();
      }
      return;
    }

    a.fireIn -= dt;
    if (a.fireIn > 0) return;
    const target = this.pickTarget(a);
    if (target < 0) {
      a.fireIn = 0.6;
      return;
    }
    a.aim = target;
    a.burst = 0.25 + Math.random() * 0.4;
    a.burstGap = 0;
  }

  private pickTarget(a: Actor): number {
    const from = [a.x, 0.1 + EYE, a.z];
    let best = -1;
    let bestD = 42;
    for (let i = 0; i < this.actors.length; i++) {
      const o = this.actors[i];
      if (o === a || o.team === a.team) continue;
      const d = Math.hypot(o.x - a.x, o.z - a.z);
      if (d > bestD) continue;
      if (!clearShot(this.brushes, from, [o.x, 0.1 + 1.1, o.z])) continue;
      best = i;
      bestD = d;
    }
    return best;
  }

  private fire(a: Actor, target: Actor) {
    const from: number[] = [a.x, 0.1 + EYE - 0.12, a.z];
    // Scatter around the target so the show has near misses in it, not eight
    // players who never miss.
    const miss = Math.random() < 0.45;
    const to: number[] = [
      target.x + (Math.random() - 0.5) * (miss ? 1.6 : 0.35),
      0.1 + 1.0 + (Math.random() - 0.5) * (miss ? 1.4 : 0.5),
      target.z + (Math.random() - 0.5) * (miss ? 1.6 : 0.35),
    ];
    this.renderer.spawnTracer(from, to, !miss);
    if (!miss) this.renderer.spawnImpact(to, 0xffe0b0);
  }

  /**
   * The camera works like a spectator director: it holds one angle for a few
   * seconds, drifting slowly, then cuts to a new one framed on whichever pair
   * of enemies is closest to each other — which is where the shooting is.
   */
  private stepCamera(dt: number) {
    this.shot.hold -= dt;
    if (this.shot.hold <= 0) {
      this.shot.hold = SHOT_LENGTH;
      this.shot.angle = Math.random() * Math.PI * 2;
      this.shot.height = 5.5 + Math.random() * 5;
      this.shot.look = this.hotspot();
    }
    this.shot.angle += dt * 0.075;

    const radius = Math.max(this.extent.x, this.extent.z) * 0.62 + 6;
    const want: [number, number, number] = [
      this.shot.look[0] + Math.cos(this.shot.angle) * radius,
      this.shot.height + Math.sin(this.time * 0.35) * 0.35,
      this.shot.look[2] + Math.sin(this.shot.angle) * radius,
    ];
    // Ease rather than snap, so a cut reads as a camera move and the drift in
    // between reads as one continuous shot.
    const k = Math.min(1, dt * 1.6);
    this.eye = [
      this.eye[0] + (want[0] - this.eye[0]) * k,
      this.eye[1] + (want[1] - this.eye[1]) * k,
      this.eye[2] + (want[2] - this.eye[2]) * k,
    ];

    const dx = this.shot.look[0] - this.eye[0];
    const dy = this.shot.look[1] - this.eye[1];
    const dz = this.shot.look[2] - this.eye[2];
    const len = Math.hypot(dx, dy, dz) || 1;
    // Matches the simulation's look_dir: yaw 0 faces +Z.
    this.camYaw = Math.atan2(dx, dz);
    this.camPitch = Math.asin(dy / len);
  }

  /** The midpoint of the closest pair of enemies: where the fight is. */
  private hotspot(): number[] {
    let best: number[] = [0, 1.2, 0];
    let bestD = Infinity;
    for (const a of this.actors) {
      for (const b of this.actors) {
        if (a.team === b.team) continue;
        const d = Math.hypot(a.x - b.x, a.z - b.z);
        if (d < bestD) {
          bestD = d;
          best = [(a.x + b.x) / 2, 1.4, (a.z + b.z) / 2];
        }
      }
    }
    return best;
  }
}

function angleDelta(from: number, to: number): number {
  let d = to - from;
  while (d > Math.PI) d -= Math.PI * 2;
  while (d < -Math.PI) d += Math.PI * 2;
  return d;
}
