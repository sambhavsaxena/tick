// The browser's half of the shared simulation.
//
// This is the same Rust crate the server runs, compiled to wasm32 and loaded
// without any bindgen glue. Prediction cannot disagree with the server about
// movement, because it is not a second implementation of movement.

export interface SimExports {
  world_init(mapId: number): void;
  state_ptr(): number;
  geometry_ptr(): number;
  geometry_count(): number;
  set_state(
    x: number, y: number, z: number,
    vx: number, vy: number, vz: number,
    onGround: number, crouching: number,
  ): void;
  step(
    buttons: number, yaw: number, pitch: number,
    speedMult: number, gravityMult: number, canSprint: number,
  ): void;
  memory: WebAssembly.Memory;
}

export interface Brush {
  min: [number, number, number];
  max: [number, number, number];
  thin: boolean;
  glass: boolean;
}

export class Sim {
  private ex: SimExports;
  private state: Float32Array;

  private constructor(ex: SimExports) {
    this.ex = ex;
    this.state = new Float32Array(ex.memory.buffer, ex.state_ptr(), 8);
  }

  static async load(url: string): Promise<Sim> {
    const res = await fetch(url);
    const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {});
    return new Sim(instance.exports as unknown as SimExports);
  }

  /** Load a map's collision geometry. The renderer draws exactly this. */
  loadMap(mapId: number): Brush[] {
    this.ex.world_init(mapId);
    // Re-view: instantiating may have grown linear memory.
    this.state = new Float32Array(this.ex.memory.buffer, this.ex.state_ptr(), 8);
    const n = this.ex.geometry_count();
    const g = new Float32Array(this.ex.memory.buffer, this.ex.geometry_ptr(), n * 7);
    const out: Brush[] = [];
    for (let i = 0; i < n; i++) {
      const o = i * 7;
      const flags = g[o + 6];
      out.push({
        min: [g[o], g[o + 1], g[o + 2]],
        max: [g[o + 3], g[o + 4], g[o + 5]],
        thin: (flags % 2) === 1,
        glass: flags >= 2,
      });
    }
    return out;
  }

  setState(
    x: number, y: number, z: number,
    vx: number, vy: number, vz: number,
    onGround: boolean, crouching: boolean,
  ) {
    this.ex.set_state(x, y, z, vx, vy, vz, onGround ? 1 : 0, crouching ? 1 : 0);
  }

  step(buttons: number, yaw: number, pitch: number, speedMult: number, gravityMult: number, canSprint: boolean) {
    this.ex.step(buttons, yaw, pitch, speedMult, gravityMult, canSprint ? 1 : 0);
  }

  get pos(): [number, number, number] {
    return [this.state[0], this.state[1], this.state[2]];
  }
  get vel(): [number, number, number] {
    return [this.state[3], this.state[4], this.state[5]];
  }
  get onGround(): boolean {
    return this.state[6] !== 0;
  }
  get crouching(): boolean {
    return this.state[7] !== 0;
  }
}
