// Renderer.
//
// Flat-shaded low-poly with strong silhouette contrast, chosen for readability
// at low resolution and for download size rather than for style: enemies use a
// reserved hot orange that appears nowhere in any environment, allies a cool
// teal, and the world stays desaturated so neither ever has to compete with
// scenery for attention.

import * as THREE from "three";
import type { Brush } from "./sim";
import type { Snapshot } from "./proto";

// Ally teal, enemy orange. The enemy colour is reserved: it appears nowhere
// in any environment, so a silhouette never has to compete with scenery.
export const TEAM_COLOR = [0x3fd0c0, 0xff5a2e];

export interface RenderPlayer {
  slot: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  team: number;
  alive: boolean;
  crouching: boolean;
  marked: boolean;
  staggered: boolean;
  carrying: boolean;
  isLocal: boolean;
}

interface WeatherLook {
  fog: number;
  fogNear: number;
  fogFar: number;
  ambient: number;
  ambientIntensity: number;
  sun: number;
  sunIntensity: number;
  ground: number;
  rain: boolean;
}

const LOOKS: WeatherLook[] = [
  // Clear: long sightlines, harsh directional shadows.
  {
    fog: 0xa8b4bd, fogNear: 40, fogFar: 190, ambient: 0x8ea0ad, ambientIntensity: 0.85,
    sun: 0xfff2e0, sunIntensity: 1.5, ground: 0x9aa3aa, rain: false,
  },
  // Rain: visibility falls off past 45 m, everything goes flat and cold.
  {
    fog: 0x5a6570, fogNear: 8, fogFar: 52, ambient: 0x6f7d88, ambientIntensity: 0.7,
    sun: 0xc8d4de, sunIntensity: 0.5, ground: 0x545d64, rain: true,
  },
  // Night: about 30 m of usable sight, and your muzzle flash is a flare.
  // Dark enough that ambush beats aim, light enough that geometry still reads
  // as geometry — an unreadable map is not atmosphere, it is a bug.
  {
    fog: 0x0c1219, fogNear: 6, fogFar: 34, ambient: 0x35496a, ambientIntensity: 0.95,
    sun: 0x5d7ba6, sunIntensity: 0.45, ground: 0x1d2530, rain: false,
  },
];

export class Renderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly camera: THREE.PerspectiveCamera;
  readonly canvas: HTMLCanvasElement;

  private mapGroup = new THREE.Group();
  private avatars = new Map<number, THREE.Group>();
  private tracers: { mesh: THREE.Mesh; life: number; ttl: number }[] = [];
  private impacts: { mesh: THREE.Mesh; life: number }[] = [];
  private muzzle: THREE.PointLight;
  private muzzleLife = 0;
  private rain: THREE.Points | null = null;
  private ambient: THREE.HemisphereLight;
  private sun: THREE.DirectionalLight;
  private viewmodel = new THREE.Group();
  private shimmerGroup = new THREE.Group();
  private cinderGroup = new THREE.Group();
  private propGroup = new THREE.Group();
  private weather = 0;
  private weapon = 0;
  private blackout = false;
  private recoilKick = 0;
  private viewBob = 0;
  /** While true, update() presents nothing new: the death backdrop. */
  frozen = false;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: false,
      powerPreference: "high-performance",
      // The death screen freezes on the last presented frame, which only
      // stays on the canvas if the buffer survives compositing.
      preserveDrawingBuffer: true,
    });
    // Deliberately low internal resolution, upscaled with nearest-neighbour
    // CSS: the pixelated look is the style, and it is also what keeps the
    // tone mapping and shadow passes cheap on integrated graphics.
    this.renderer.setPixelRatio(1);
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.2;
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.camera = new THREE.PerspectiveCamera(95, 1, 0.05, 300);
    this.scene.add(this.mapGroup, this.shimmerGroup, this.cinderGroup, this.propGroup);

    this.ambient = new THREE.HemisphereLight(0x8ea0ad, 0x2b3138, 0.9);
    this.sun = new THREE.DirectionalLight(0xfff2e0, 1.4);
    this.sun.position.set(24, 40, 12);
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(2048, 2048);
    const sc = this.sun.shadow.camera;
    sc.left = -60; sc.right = 60; sc.top = 60; sc.bottom = -60;
    sc.near = 1; sc.far = 120;
    this.sun.shadow.bias = -0.002;
    this.scene.add(this.ambient, this.sun);

    this.muzzle = new THREE.PointLight(0xffc07a, 0, 16, 2);
    this.scene.add(this.muzzle);

    this.camera.add(this.viewmodel);
    this.scene.add(this.camera);

    this.resize();
    window.addEventListener("resize", () => this.resize());
  }

  resize() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    // Render at a fixed 400-line internal height; CSS stretches the canvas
    // to the window with image-rendering: pixelated.
    const scale = Math.min(1, 400 / h);
    this.renderer.setSize(Math.round(w * scale), Math.round(h * scale), false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  setWeather(weather: number) {
    this.weather = weather;
    this.applyLook();
  }

  setBlackout(on: boolean) {
    this.blackout = on;
    this.applyLook();
  }

  private applyLook() {
    const look = LOOKS[this.weather] ?? LOOKS[0];
    if (this.blackout) {
      // Night vision: everything collapses to a short green range.
      this.scene.fog = new THREE.Fog(0x04170c, 3, 26);
      this.scene.background = new THREE.Color(0x04170c);
      this.ambient.color.setHex(0x2fbf6a);
      this.ambient.groundColor.setHex(0x06200f);
      this.ambient.intensity = 0.9;
      this.sun.intensity = 0.1;
      this.sun.color.setHex(0x2fbf6a);
    } else {
      this.scene.fog = new THREE.Fog(look.fog, look.fogNear, look.fogFar);
      this.scene.background = new THREE.Color(look.fog);
      this.ambient.color.setHex(look.ambient);
      this.ambient.groundColor.setHex(0x22282e);
      this.ambient.intensity = look.ambientIntensity;
      this.sun.color.setHex(look.sun);
      this.sun.intensity = look.sunIntensity;
    }
    if (this.rain) this.rain.visible = look.rain && !this.blackout;
  }

  /** Build the level out of the collision brushes the shared sim handed us. */
  buildMap(brushes: Brush[], weather: number) {
    this.mapGroup.clear();
    const look = LOOKS[weather] ?? LOOKS[0];

    const solidMat = new THREE.MeshLambertMaterial({ color: 0x9aa3aa });
    const floorMat = new THREE.MeshLambertMaterial({ color: look.ground });
    const thinMat = new THREE.MeshLambertMaterial({ color: 0xb2895e });
    const glassMat = new THREE.MeshLambertMaterial({
      color: 0xbfe4ea,
      transparent: true,
      opacity: 0.22,
      depthWrite: false,
    });

    for (const b of brushes) {
      const sx = b.max[0] - b.min[0];
      const sy = b.max[1] - b.min[1];
      const sz = b.max[2] - b.min[2];
      const geo = new THREE.BoxGeometry(sx, sy, sz);
      const isFloor = sy > 1.5 && b.max[1] <= 0.05;
      const mat = b.glass ? glassMat : b.thin ? thinMat : isFloor ? floorMat : solidMat;
      const mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(
        (b.min[0] + b.max[0]) / 2,
        (b.min[1] + b.max[1]) / 2,
        (b.min[2] + b.max[2]) / 2,
      );
      if (!b.glass) {
        mesh.castShadow = !isFloor;
        mesh.receiveShadow = true;
      }
      this.mapGroup.add(mesh);

      // A thin dark cap on every solid, so edges read at distance without
      // paying for an outline pass.
      if (!b.glass && !isFloor && sy > 0.3) {
        const cap = new THREE.Mesh(
          new THREE.BoxGeometry(sx * 1.005, 0.06, sz * 1.005),
          new THREE.MeshBasicMaterial({ color: 0x2b3138 }),
        );
        cap.position.set(mesh.position.x, b.max[1] + 0.03, mesh.position.z);
        this.mapGroup.add(cap);
      }
    }

    this.buildRain();
    this.setWeather(weather);
  }

  private buildRain() {
    if (this.rain) {
      this.scene.remove(this.rain);
      this.rain.geometry.dispose();
    }
    const count = 2200;
    const pos = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      pos[i * 3] = (Math.random() - 0.5) * 70;
      pos[i * 3 + 1] = Math.random() * 26;
      pos[i * 3 + 2] = (Math.random() - 0.5) * 70;
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    this.rain = new THREE.Points(
      geo,
      new THREE.PointsMaterial({ color: 0xc9d8e4, size: 0.06, transparent: true, opacity: 0.55 }),
    );
    this.rain.frustumCulled = false;
    this.scene.add(this.rain);
  }

  private makeAvatar(team: number): THREE.Group {
    const g = new THREE.Group();
    const color = TEAM_COLOR[team];
    const bodyMat = new THREE.MeshLambertMaterial({ color });
    const darkMat = new THREE.MeshLambertMaterial({ color: 0x20262c });

    const torso = new THREE.Mesh(new THREE.BoxGeometry(0.72, 1.05, 0.42), bodyMat);
    torso.position.y = 0.95;
    const legs = new THREE.Mesh(new THREE.BoxGeometry(0.6, 0.9, 0.36), darkMat);
    legs.position.y = 0.45;
    // The head is a distinct volume, because it is a distinct hitbox.
    const head = new THREE.Mesh(new THREE.BoxGeometry(0.42, 0.36, 0.42), bodyMat);
    head.position.y = 1.66;
    head.name = "head";
    const visor = new THREE.Mesh(
      new THREE.BoxGeometry(0.44, 0.1, 0.06),
      new THREE.MeshBasicMaterial({ color: 0x101418 }),
    );
    visor.position.set(0, 1.68, 0.21);
    const gun = new THREE.Mesh(new THREE.BoxGeometry(0.12, 0.12, 0.85), darkMat);
    gun.position.set(0.26, 1.28, 0.4);
    g.add(torso, legs, head, visor, gun);
    g.traverse((o) => {
      if ((o as THREE.Mesh).isMesh) o.castShadow = true;
    });
    return g;
  }

  syncPlayers(players: RenderPlayer[]) {
    const seen = new Set<number>();
    for (const p of players) {
      seen.add(p.slot);
      let g = this.avatars.get(p.slot);
      if (!g) {
        g = this.makeAvatar(p.team);
        this.avatars.set(p.slot, g);
        this.scene.add(g);
      }
      g.visible = p.alive && !p.isLocal;
      if (!g.visible) continue;
      g.position.set(p.x, p.y, p.z);
      g.rotation.y = p.yaw;
      const squash = p.crouching ? 0.64 : 1;
      g.scale.set(1, squash, 1);

      // Staggered, marked and core-carrying players are all deliberately
      // easy to read at a glance: each state is a colour, not an icon.
      const tint = p.marked
        ? 0xffd447
        : p.staggered
        ? 0xff2d55
        : p.carrying
        ? 0x7cf7ff
        : TEAM_COLOR[p.team];
      g.traverse((o) => {
        const m = (o as THREE.Mesh).material as THREE.MeshLambertMaterial | undefined;
        if (m && m.color && o.name !== "visor") {
          if (m.color.getHex() !== 0x20262c && m.color.getHex() !== 0x101418) {
            m.color.setHex(tint);
          }
        }
      });
    }
    for (const [slot, g] of this.avatars) {
      if (!seen.has(slot)) g.visible = false;
    }
  }

  setViewmodel(weapon: number) {
    this.weapon = weapon;
    this.viewmodel.clear();
    const dark = new THREE.MeshLambertMaterial({ color: 0x2a3138 });
    const accent = new THREE.MeshLambertMaterial({ color: 0x6f7d88 });
    const parts: THREE.Mesh[] = [];
    const box = (w: number, h: number, d: number, mat: THREE.Material) =>
      new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat);

    switch (weapon) {
      case 1: // Ridge: long barrel, scope
        parts.push(box(0.07, 0.07, 0.95, dark), box(0.09, 0.14, 0.3, accent));
        parts[1].position.set(0, -0.03, 0.28);
        parts.push(box(0.06, 0.06, 0.2, accent));
        parts[2].position.set(0, 0.09, 0.1);
        break;
      case 2: // Maul: fat and short
        parts.push(box(0.11, 0.12, 0.6, dark), box(0.08, 0.08, 0.4, accent));
        parts[1].position.set(0, -0.09, 0.05);
        break;
      case 3: // Arc: blocky emitter
        parts.push(box(0.1, 0.11, 0.62, dark), box(0.14, 0.05, 0.16, accent));
        parts[1].position.set(0, 0.06, -0.22);
        break;
      case 5: // Lance
        parts.push(box(0.08, 0.09, 1.05, dark), box(0.1, 0.16, 0.26, accent));
        parts[1].position.set(0, -0.04, 0.3);
        break;
      case 6: // Blade: short grip, flat blade
        parts.push(box(0.05, 0.12, 0.16, dark), box(0.025, 0.07, 0.42, accent));
        parts[1].position.set(0, 0.02, 0.28);
        break;
      default: // Sting
        parts.push(box(0.09, 0.1, 0.5, dark), box(0.06, 0.2, 0.1, accent));
        parts[1].position.set(0, -0.13, 0.06);
    }
    for (const p of parts) this.viewmodel.add(p);
    this.viewmodel.position.set(0.22, -0.18, -0.42);
    this.viewmodel.rotation.set(0, 0.04, 0);
  }

  kickRecoil(amount: number) {
    this.recoilKick = Math.min(this.recoilKick + amount, 0.22);
  }

  /** World point to CSS pixel coordinates, or null when behind the camera. */
  projectToScreen(pos: number[]): { x: number; y: number } | null {
    const v = new THREE.Vector3(pos[0], pos[1], pos[2]).project(this.camera);
    if (v.z > 1) return null;
    return {
      x: (v.x * 0.5 + 0.5) * window.innerWidth,
      y: (-v.y * 0.5 + 0.5) * window.innerHeight,
    };
  }

  spawnTracer(from: number[], to: number[], hit: boolean) {
    // A thin stretched box rather than a 1px line: WebGL ignores line width,
    // and a line has no visible thickness at any distance.
    const a = new THREE.Vector3(from[0], from[1], from[2]);
    const b = new THREE.Vector3(to[0], to[1], to[2]);
    const len = a.distanceTo(b);
    if (len < 0.05) return;
    const mesh = new THREE.Mesh(
      new THREE.BoxGeometry(0.03, 0.03, len),
      new THREE.MeshBasicMaterial({
        color: hit ? 0xffd0a0 : 0xfff0d0,
        transparent: true,
        opacity: 0.9,
      }),
    );
    mesh.position.copy(a.clone().add(b).multiplyScalar(0.5));
    mesh.lookAt(b);
    this.scene.add(mesh);
    this.tracers.push({ mesh, life: 0.14, ttl: 0.14 });
    this.muzzle.position.set(from[0], from[1], from[2]);
    this.muzzleLife = 0.05;
  }

  spawnImpact(pos: number[], color = 0xffe0b0) {
    const mesh = new THREE.Mesh(
      new THREE.SphereGeometry(0.07, 6, 5),
      new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.9 }),
    );
    mesh.position.set(pos[0], pos[1], pos[2]);
    this.scene.add(mesh);
    this.impacts.push({ mesh, life: 0.22 });
  }

  syncProps(snap: Snapshot) {
    this.shimmerGroup.clear();
    for (const s of snap.shimmers) {
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(3.0, 2.5, 0.12),
        new THREE.MeshBasicMaterial({
          color: TEAM_COLOR[s.team],
          transparent: true,
          opacity: 0.28,
          depthWrite: false,
        }),
      );
      mesh.position.set(s.x, s.y + 1.25, s.z);
      mesh.rotation.y = s.yaw;
      this.shimmerGroup.add(mesh);
    }

    this.cinderGroup.clear();
    for (const c of snap.cinders) {
      const a = new THREE.Vector3(c.ax, c.ay + 0.1, c.az);
      const b = new THREE.Vector3(c.bx, c.by + 0.1, c.bz);
      const len = a.distanceTo(b);
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(1.8, 0.9, len),
        new THREE.MeshBasicMaterial({ color: 0xff7a1e, transparent: true, opacity: 0.42 }),
      );
      mesh.position.copy(a.clone().add(b).multiplyScalar(0.5));
      mesh.lookAt(b);
      this.cinderGroup.add(mesh);
    }

    this.propGroup.clear();
    for (const p of snap.pickups) {
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(0.7, 0.5, 0.7),
        new THREE.MeshLambertMaterial({ color: 0xffd447 }),
      );
      mesh.position.set(p.x, p.y, p.z);
      mesh.rotation.y = performance.now() * 0.001;
      this.propGroup.add(mesh);
    }
    if (snap.coreState === 1) {
      const core = new THREE.Mesh(
        new THREE.OctahedronGeometry(0.45),
        new THREE.MeshBasicMaterial({ color: 0x7cf7ff }),
      );
      core.position.set(snap.corePos[0], snap.corePos[1] + 0.5, snap.corePos[2]);
      core.rotation.y = performance.now() * 0.002;
      this.propGroup.add(core);
    }
  }

  /** Place the camera and advance every transient effect. */
  update(
    dt: number,
    eye: [number, number, number],
    yaw: number,
    pitch: number,
    ads: number,
    speed: number,
  ) {
    // Death freeze: the last frame stays on the canvas as the backdrop.
    if (this.frozen) return;
    this.recoilKick *= Math.max(0, 1 - dt * 9);
    this.viewBob += dt * Math.min(speed, 8) * 1.4;
    const bob = Math.sin(this.viewBob) * 0.012 * Math.min(speed / 6, 1);

    this.camera.position.set(eye[0], eye[1] + bob, eye[2]);
    this.camera.rotation.set(0, 0, 0);
    // The sim's look_dir is (sin yaw, sin pitch, cos yaw): +Z at yaw 0. A
    // three.js camera looks down -Z, so it needs a half-turn on top of yaw or
    // the view faces exactly opposite the direction the server resolves shots.
    this.camera.rotateY(yaw + Math.PI);
    this.camera.rotateX(pitch + this.recoilKick);

    // The Ridge aims through a real scope: much deeper zoom, and the HUD
    // draws the scope mask once the transition is nearly done.
    const targetFov = this.weapon === 1 ? 95 - ads * 67 : 95 - ads * 32;
    if (Math.abs(this.camera.fov - targetFov) > 0.1) {
      this.camera.fov += (targetFov - this.camera.fov) * Math.min(1, dt * 14);
      this.camera.updateProjectionMatrix();
    }
    this.viewmodel.position.set(0.22 - ads * 0.22, -0.18 + ads * 0.09 - bob * 2, -0.42 - ads * 0.1);
    this.viewmodel.visible = ads < (this.weapon === 1 ? 0.5 : 0.85);

    for (let i = this.tracers.length - 1; i >= 0; i--) {
      const t = this.tracers[i];
      t.life -= dt;
      const mat = t.mesh.material as THREE.MeshBasicMaterial;
      mat.opacity = Math.max(0, (t.life / t.ttl) * 0.9);
      if (t.life <= 0) {
        this.scene.remove(t.mesh);
        t.mesh.geometry.dispose();
        this.tracers.splice(i, 1);
      }
    }
    for (let i = this.impacts.length - 1; i >= 0; i--) {
      const im = this.impacts[i];
      im.life -= dt;
      im.mesh.scale.setScalar(1 + (0.22 - im.life) * 6);
      (im.mesh.material as THREE.MeshBasicMaterial).opacity = Math.max(0, im.life / 0.22);
      if (im.life <= 0) {
        this.scene.remove(im.mesh);
        im.mesh.geometry.dispose();
        this.impacts.splice(i, 1);
      }
    }
    this.muzzleLife -= dt;
    // At night the muzzle flash is the single loudest thing on the map.
    const flashPower = this.weather === 2 || this.blackout ? 9 : 4;
    this.muzzle.intensity = this.muzzleLife > 0 ? flashPower : 0;

    if (this.rain && this.rain.visible) {
      const pos = this.rain.geometry.getAttribute("position") as THREE.BufferAttribute;
      const arr = pos.array as Float32Array;
      for (let i = 1; i < arr.length; i += 3) {
        arr[i] -= dt * 26;
        if (arr[i] < 0) {
          arr[i] = 26;
          arr[i - 1] = eye[0] + (Math.random() - 0.5) * 70;
          arr[i + 1] = eye[2] + (Math.random() - 0.5) * 70;
        }
      }
      pos.needsUpdate = true;
      this.rain.position.set(0, 0, 0);
    }

    this.renderer.render(this.scene, this.camera);
  }
}
