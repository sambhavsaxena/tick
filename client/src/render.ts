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
  /** Character index (Ward/Vane/Echo/Kiln) — fixed per player for the match. */
  character: number;
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
  // Clear: long sightlines, harsh directional shadows, warm and well-lit.
  {
    fog: 0xb8c2c8, fogNear: 50, fogFar: 210, ambient: 0xa8b6c0, ambientIntensity: 1.05,
    sun: 0xffe8c8, sunIntensity: 1.8, ground: 0xa8b0b6, rain: false,
  },
  // Rain: visibility falls off past 45 m, flat and cold but still readable.
  {
    fog: 0x66727e, fogNear: 10, fogFar: 56, ambient: 0x8494a2, ambientIntensity: 0.9,
    sun: 0xc8d4de, sunIntensity: 0.6, ground: 0x5e6870, rain: true,
  },
  // Night: about 30 m of usable sight, and your muzzle flash is a flare.
  // Dark enough that ambush beats aim, light enough that geometry still reads
  // as geometry — an unreadable map is not atmosphere, it is a bug.
  {
    fog: 0x0c1219, fogNear: 6, fogFar: 34, ambient: 0x35496a, ambientIntensity: 0.95,
    sun: 0x5d7ba6, sunIntensity: 0.45, ground: 0x1d2530, rain: false,
  },
];

// ------------------------------------------------------------------ textures
//
// Every surface texture is drawn once into a small canvas at runtime: real
// material variation without a single downloaded asset. Textures stay
// grayscale so the existing material colors (and weather ground tints) keep
// multiplying through unchanged.

const texCache = new Map<string, THREE.CanvasTexture>();

function surface(kind: "concrete" | "wood" | "ground" | "rock" | "metal"): THREE.CanvasTexture {
  const hit = texCache.get(kind);
  if (hit) return hit;
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d")!;
  // The base must be white: a map multiplies the material colour, so any
  // darker fill would quietly dim every surface in the game.
  g.fillStyle = "#ffffff";
  g.fillRect(0, 0, 128, 128);

  const speck = (n: number, size: number, spread: number) => {
    for (let i = 0; i < n; i++) {
      const v = (Math.random() - 0.5) * spread;
      g.fillStyle = v > 0 ? `rgba(255,255,255,${v})` : `rgba(0,0,0,${-v})`;
      g.fillRect(Math.random() * 128, Math.random() * 128, size, size);
    }
  };

  switch (kind) {
    case "concrete":
      speck(1200, 2, 0.16);
      // Faint pour seams and streaks of weathering.
      g.fillStyle = "rgba(0,0,0,0.10)";
      for (const y of [31, 63, 95]) g.fillRect(0, y, 128, 1);
      for (let i = 0; i < 8; i++) {
        g.fillStyle = `rgba(0,0,0,${0.03 + Math.random() * 0.05})`;
        const x = Math.random() * 128;
        g.fillRect(x, 0, 1 + Math.random() * 2, 40 + Math.random() * 88);
      }
      break;
    case "wood":
      // Plank rows with grain streaks.
      for (let row = 0; row < 8; row++) {
        g.fillStyle = `rgba(${row % 2 ? 0 : 255},${row % 2 ? 0 : 255},${row % 2 ? 0 : 255},0.05)`;
        g.fillRect(0, row * 16, 128, 16);
        g.fillStyle = "rgba(0,0,0,0.22)";
        g.fillRect(0, row * 16, 128, 1);
      }
      for (let i = 0; i < 60; i++) {
        g.fillStyle = `rgba(0,0,0,${0.04 + Math.random() * 0.08})`;
        g.fillRect(Math.random() * 128, Math.random() * 128, 6 + Math.random() * 26, 1);
      }
      break;
    case "ground":
      // Fine grain only: at the floor's tiling rate anything larger reads as
      // a smear rather than as texture.
      speck(2400, 1, 0.18);
      speck(500, 2, 0.1);
      break;
    case "rock":
      speck(900, 3, 0.2);
      for (let i = 0; i < 10; i++) {
        g.strokeStyle = `rgba(0,0,0,${0.08 + Math.random() * 0.1})`;
        g.beginPath();
        g.moveTo(Math.random() * 128, Math.random() * 128);
        g.lineTo(Math.random() * 128, Math.random() * 128);
        g.stroke();
      }
      break;
    case "metal":
      speck(400, 1, 0.1);
      for (let y = 0; y < 128; y += 4) {
        g.fillStyle = "rgba(255,255,255,0.03)";
        g.fillRect(0, y, 128, 1);
      }
      // Rivet dots along the edges.
      g.fillStyle = "rgba(0,0,0,0.35)";
      for (let x = 8; x < 128; x += 24) {
        g.fillRect(x, 6, 3, 3);
        g.fillRect(x, 119, 3, 3);
      }
      break;
  }

  const tex = new THREE.CanvasTexture(c);
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.colorSpace = THREE.SRGBColorSpace;
  texCache.set(kind, tex);
  return tex;
}

/**
 * Retile a box's UVs so its texture repeats every `tile` metres instead of
 * stretching once across each face. BoxGeometry lays faces out in the order
 * +X, -X, +Y, -Y, +Z, -Z with four vertices each, and each face's two axes
 * are known, so the scale is exact and costs no extra texture memory.
 */
function tileBox(geo: THREE.BoxGeometry, sx: number, sy: number, sz: number, tile = 2): THREE.BoxGeometry {
  const uv = geo.getAttribute("uv") as THREE.BufferAttribute;
  const spans: [number, number][] = [
    [sz, sy], [sz, sy], // +X, -X
    [sx, sz], [sx, sz], // +Y, -Y
    [sx, sy], [sx, sy], // +Z, -Z
  ];
  for (let face = 0; face < 6; face++) {
    const [su, sv] = spans[face];
    for (let v = 0; v < 4; v++) {
      const i = face * 4 + v;
      uv.setXY(i, uv.getX(i) * (su / tile), uv.getY(i) * (sv / tile));
    }
  }
  uv.needsUpdate = true;
  return geo;
}

/** Deterministic 0..1 hash for scatter placement. */
function hash01(i: number, salt: number): number {
  const x = Math.sin(i * 127.1 + salt * 311.7) * 43758.5453;
  return x - Math.floor(x);
}

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
  /** Horizon scenery: ocean, sunset, black hole, stars. Weather-dependent. */
  private vistaGroup = new THREE.Group();
  /** Trees and rocks beyond the walls. Map-dependent. */
  private sceneryGroup = new THREE.Group();
  /** Night-only neon strips and their lights. */
  private neonGroup = new THREE.Group();
  /** Half-extent of the current map's footprint, from its brushes. */
  private mapExtent = { x: 20, z: 28 };
  private weather = 0;
  private weapon = 0;
  private blackout = false;
  private recoilKick = 0;
  private viewBob = 0;
  /** While true, update() presents nothing new: the death backdrop. */
  frozen = false;
  /** Lazily built scene for the death screen's killer portrait. */
  private portrait: {
    renderer: THREE.WebGLRenderer;
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    avatar: THREE.Group | null;
    character: number;
    team: number;
    spin: number;
  } | null = null;

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
    // Full-resolution rendering, pixel ratio capped so integrated graphics
    // still holds 60 fps on a retina display.
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.2;
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.camera = new THREE.PerspectiveCamera(95, 1, 0.05, 1500);
    this.scene.add(this.mapGroup, this.shimmerGroup, this.cinderGroup, this.propGroup);
    this.scene.add(this.vistaGroup, this.sceneryGroup, this.neonGroup);

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
    this.renderer.setSize(w, h, false);
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
    this.neonGroup.visible = this.weather === 2 && !this.blackout;
    this.buildVista();
  }

  /**
   * The world past the walls. Day looks west onto a sunset over open ocean;
   * Night hangs a black hole on the +X horizon — the same direction the
   * simulation's gentle pull drags everyone toward. All of it is fog-exempt
   * so it reads at any weather, and none of it is reachable.
   */
  private buildVista() {
    this.vistaGroup.clear();
    const flat = (color: number, opacity = 1) =>
      new THREE.MeshBasicMaterial({
        color,
        fog: false,
        transparent: opacity < 1,
        opacity,
        depthWrite: false,
      });

    // Ocean: an enormous disc just below floor level, out to the horizon.
    const oceanColor = this.weather === 2 ? 0x060a12 : this.weather === 1 ? 0x24404e : 0x2a5a78;
    const ocean = new THREE.Mesh(new THREE.CircleGeometry(1200, 40), flat(oceanColor));
    ocean.rotation.x = -Math.PI / 2;
    ocean.position.y = -0.8;
    this.vistaGroup.add(ocean);

    if (this.weather !== 2) {
      // Sunset in the -X sky: the sun, its glow, and a light path on the sea.
      const sun = new THREE.Mesh(new THREE.CircleGeometry(56, 32), flat(0xffb04a));
      sun.position.set(-950, 260, 0);
      sun.lookAt(0, 260, 0);
      const glow = new THREE.Mesh(new THREE.CircleGeometry(150, 32), flat(0xff7a3a, 0.35));
      glow.position.set(-955, 250, 0);
      glow.lookAt(0, 250, 0);
      const path = new THREE.Mesh(new THREE.PlaneGeometry(700, 24), flat(0xff9a50, 0.25));
      path.rotation.x = -Math.PI / 2;
      path.rotation.z = Math.PI / 2;
      path.position.set(-500, -0.7, 0);
      this.vistaGroup.add(sun, glow, path);

      // A scattered cloud deck: flat discs high overhead, denser and darker
      // in Rain, sparse and warm-lit in Clear.
      const cloudColor = this.weather === 1 ? 0x76828e : 0xf2e2ce;
      const cloudCount = this.weather === 1 ? 16 : 9;
      for (let i = 0; i < cloudCount; i++) {
        const cloud = new THREE.Mesh(
          new THREE.CircleGeometry(90 + hash01(i, 5) * 130, 18),
          flat(cloudColor, this.weather === 1 ? 0.28 : 0.22),
        );
        cloud.rotation.x = Math.PI / 2;
        const a = hash01(i, 9) * Math.PI * 2;
        const r = 300 + hash01(i, 13) * 700;
        cloud.position.set(Math.cos(a) * r, 320 + hash01(i, 21) * 180, Math.sin(a) * r);
        cloud.scale.x = 1.6 + hash01(i, 33);
        this.vistaGroup.add(cloud);
      }
    } else {
      // Black hole low on the +X horizon: a void disc, a hot accretion ring,
      // a faint outer lens, and a sky of stars.
      const hole = new THREE.Mesh(new THREE.CircleGeometry(64, 40), flat(0x000000));
      hole.position.set(950, 280, 0);
      hole.lookAt(0, 280, 0);
      const ring = new THREE.Mesh(new THREE.RingGeometry(64, 82, 48), flat(0xffc890, 0.95));
      ring.position.set(948, 280, 0);
      ring.lookAt(0, 280, 0);
      const lens = new THREE.Mesh(new THREE.RingGeometry(82, 130, 48), flat(0x7cc4ff, 0.2));
      lens.position.set(946, 280, 0);
      lens.lookAt(0, 280, 0);
      this.vistaGroup.add(hole, ring, lens);

      const starCount = 400;
      const pos = new Float32Array(starCount * 3);
      for (let i = 0; i < starCount; i++) {
        // Deterministic scatter on the upper hemisphere.
        const a = (i * 2.399963) % (Math.PI * 2);
        const r = 700 + ((i * 97) % 400);
        const y = 60 + ((i * 53) % 500);
        pos[i * 3] = Math.cos(a) * r;
        pos[i * 3 + 1] = y;
        pos[i * 3 + 2] = Math.sin(a) * r;
      }
      const geo = new THREE.BufferGeometry();
      geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
      const stars = new THREE.Points(
        geo,
        new THREE.PointsMaterial({ color: 0xcfd8e6, size: 2.2, fog: false, sizeAttenuation: false }),
      );
      this.vistaGroup.add(stars);
    }
  }

  /** Build the level out of the collision brushes the shared sim handed us. */
  buildMap(brushes: Brush[], weather: number) {
    this.mapGroup.clear();
    // A new match may seat new characters in old slots: rebuild every avatar.
    for (const [, g] of this.avatars) this.scene.remove(g);
    this.avatars.clear();
    const look = LOOKS[weather] ?? LOOKS[0];

    // Three concrete shades so adjacent structures never read as one slab.
    const solidMats = [0x9aa3aa, 0x8f99a1, 0xa4adb4].map(
      (color) => new THREE.MeshLambertMaterial({ color, map: surface("concrete") }),
    );
    const floorMat = new THREE.MeshLambertMaterial({ color: look.ground, map: surface("ground") });
    const thinMat = new THREE.MeshLambertMaterial({ color: 0xb2895e, map: surface("wood") });
    const trunkMat = new THREE.MeshLambertMaterial({ color: 0x6b4a32, map: surface("wood") });
    const rockMat = new THREE.MeshLambertMaterial({ color: 0x7d8188, map: surface("rock") });
    const canopyMats = [0x4a7a4e, 0x3e6b44, 0x568a52].map(
      (color) => new THREE.MeshLambertMaterial({ color }),
    );
    const glassMat = new THREE.MeshLambertMaterial({
      color: 0xbfe4ea,
      transparent: true,
      opacity: 0.22,
      depthWrite: false,
    });
    const frameMat = new THREE.MeshLambertMaterial({ color: 0x3a424b, map: surface("metal") });

    let ex = 10;
    let ez = 10;
    let brushIndex = 0;
    for (const b of brushes) {
      brushIndex++;
      const sx = b.max[0] - b.min[0];
      const sy = b.max[1] - b.min[1];
      const sz = b.max[2] - b.min[2];
      ex = Math.max(ex, Math.abs(b.min[0]), Math.abs(b.max[0]));
      ez = Math.max(ez, Math.abs(b.min[2]), Math.abs(b.max[2]));
      const cx = (b.min[0] + b.max[0]) / 2;
      const cy = (b.min[1] + b.max[1]) / 2;
      const cz = (b.min[2] + b.max[2]) / 2;
      const isFloor = sy > 1.5 && b.max[1] <= 0.05;
      // Dress collision boxes by their shape: slim tall boxes are tree
      // trunks (and get a canopy), squat boxes hugging the ground are rocks.
      const isTrunk = !b.thin && !b.glass && sx <= 0.8 && sz <= 0.8 && sy >= 1.8;
      const isRock =
        !b.thin && !b.glass && !isFloor && b.min[1] <= 0.05 && sy <= 1.2 &&
        sx >= 1.6 && sx <= 4.2 && sz >= 1.6 && sz <= 4.2;

      if (isTrunk) {
        // A tapered trunk plus a clumped three-blob canopy above head
        // height: bullets and eyes pass the simulation's checks unchanged,
        // the silhouette just reads as a real tree.
        const trunk = new THREE.Mesh(
          new THREE.CylinderGeometry(sx * 0.32, sx * 0.48, sy, 7),
          trunkMat,
        );
        trunk.position.set(cx, cy, cz);
        trunk.castShadow = true;
        trunk.receiveShadow = true;
        this.mapGroup.add(trunk);
        const blobs: [number, number, number, number][] = [
          [0, 1.15, 0, 1.35],
          [0.7, 0.55, 0.4, 0.95],
          [-0.6, 0.65, -0.45, 0.85],
        ];
        blobs.forEach(([ox, oy, oz, r], k) => {
          const blob = new THREE.Mesh(
            new THREE.IcosahedronGeometry(r, 0),
            canopyMats[(brushIndex + k) % canopyMats.length],
          );
          blob.position.set(cx + ox, b.max[1] + oy, cz + oz);
          blob.rotation.set(k * 0.9, brushIndex * 0.7, 0);
          blob.castShadow = true;
          this.mapGroup.add(blob);
        });
        continue;
      }

      if (isRock) {
        // An irregular boulder mesh sized to the collision box, instead of a
        // literal box: same footprint, natural silhouette.
        const r = Math.max(sx, sz) * 0.62;
        const rock = new THREE.Mesh(new THREE.DodecahedronGeometry(r, 0), rockMat);
        rock.scale.y = sy / r;
        rock.position.set(cx, b.min[1] + sy * 0.45, cz);
        rock.rotation.y = brushIndex * 1.7;
        rock.castShadow = true;
        rock.receiveShadow = true;
        this.mapGroup.add(rock);
        continue;
      }

      const mat = b.glass
        ? glassMat
        : b.thin
        ? thinMat
        : isFloor
        ? floorMat
        : solidMats[brushIndex % solidMats.length];
      const geo = new THREE.BoxGeometry(sx, sy, sz);
      if (!b.glass) tileBox(geo, sx, sy, sz, isFloor ? 4 : 2);
      const mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(cx, cy, cz);
      if (!b.glass) {
        mesh.castShadow = !isFloor;
        mesh.receiveShadow = true;
      }
      this.mapGroup.add(mesh);

      // Glass panes get a slim frame so a pane reads as architecture, not a
      // rendering artifact.
      if (b.glass) {
        const alongX = sx > sz;
        const frame = (w: number, h: number, d: number, px: number, py: number, pz: number) => {
          const m = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), frameMat);
          m.position.set(px, py, pz);
          this.mapGroup.add(m);
        };
        if (alongX) {
          frame(sx + 0.12, 0.1, sz + 0.1, cx, b.max[1] + 0.05, cz);
          frame(sx + 0.12, 0.1, sz + 0.1, cx, b.min[1] - 0.02, cz);
          frame(0.1, sy + 0.1, sz + 0.1, b.min[0], cy, cz);
          frame(0.1, sy + 0.1, sz + 0.1, b.max[0], cy, cz);
        } else {
          frame(sx + 0.1, 0.1, sz + 0.12, cx, b.max[1] + 0.05, cz);
          frame(sx + 0.1, 0.1, sz + 0.12, cx, b.min[1] - 0.02, cz);
          frame(sx + 0.1, sy + 0.1, 0.1, cx, cy, b.min[2]);
          frame(sx + 0.1, sy + 0.1, 0.1, cx, cy, b.max[2]);
        }
        continue;
      }

      // A thin dark cap on every solid, so edges read at distance without
      // paying for an outline pass.
      if (!isFloor && sy > 0.3) {
        const cap = new THREE.Mesh(
          new THREE.BoxGeometry(sx * 1.005, 0.06, sz * 1.005),
          new THREE.MeshBasicMaterial({ color: 0x2b3138 }),
        );
        cap.position.set(cx, b.max[1] + 0.03, cz);
        this.mapGroup.add(cap);
      }
    }
    this.mapExtent = { x: ex, z: ez };

    this.buildGroundDetail();
    this.buildWallDetail();
    this.buildScenery();
    this.buildNeon();
    this.buildRain();
    this.setWeather(weather);
  }

  /**
   * Life on the floor plane: a worn dirt path between the spawn ends, grass
   * tufts and pebbles scattered deterministically. All of it sits millimetres
   * above the floor and collides with nothing.
   */
  private buildGroundDetail() {
    const { x: ex, z: ez } = this.mapExtent;
    const ix = ex - 2.2; // inside the boundary walls
    const iz = ez - 2.2;

    // The main worn path runs the long axis (spawn to spawn), with a soft
    // cross-lane through mid. Slight opacity so the ground texture shows
    // through and the edge never reads as a hard decal.
    const pathMat = new THREE.MeshLambertMaterial({
      color: 0x8a7b6a,
      map: surface("ground"),
      transparent: true,
      opacity: 0.32,
      depthWrite: false,
    });
    const main = new THREE.Mesh(new THREE.PlaneGeometry(3.2, iz * 2), pathMat);
    main.rotation.x = -Math.PI / 2;
    main.position.set(0, 0.015, 0);
    main.receiveShadow = true;
    const cross = new THREE.Mesh(new THREE.PlaneGeometry(ix * 2, 2.6), pathMat);
    cross.rotation.x = -Math.PI / 2;
    cross.position.set(0, 0.014, 0);
    cross.receiveShadow = true;
    this.mapGroup.add(main, cross);

    // Grass tufts: little cones in three greens, kept off the worn path.
    const grassMats = [0x4f7a4a, 0x5d8a52, 0x44693f].map(
      (color) => new THREE.MeshLambertMaterial({ color }),
    );
    for (let i = 0; i < 70; i++) {
      const x = (hash01(i, 3) - 0.5) * ix * 2;
      const z = (hash01(i, 7) - 0.5) * iz * 2;
      if (Math.abs(x) < 2.2) continue; // stay off the main path
      const h = 0.16 + hash01(i, 11) * 0.2;
      const tuft = new THREE.Mesh(
        new THREE.ConeGeometry(0.09 + hash01(i, 17) * 0.08, h, 4),
        grassMats[i % grassMats.length],
      );
      tuft.position.set(x, h / 2, z);
      tuft.rotation.y = i * 1.3;
      this.mapGroup.add(tuft);
    }

    // Pebbles.
    const pebbleMat = new THREE.MeshLambertMaterial({ color: 0x82868c, map: surface("rock") });
    for (let i = 0; i < 16; i++) {
      const pebble = new THREE.Mesh(
        new THREE.DodecahedronGeometry(0.08 + hash01(i, 23) * 0.12, 0),
        pebbleMat,
      );
      pebble.position.set((hash01(i, 29) - 0.5) * ix * 2, 0.06, (hash01(i, 31) - 0.5) * iz * 2);
      pebble.rotation.set(i, i * 2.3, 0);
      this.mapGroup.add(pebble);
    }
  }

  /**
   * Architecture on the boundary walls: a dark skirting line at the base and
   * pilasters every few metres, so the arena's edge reads as a built place
   * rather than four abstract planes. Visual only — everything hugs the wall
   * face and collides with nothing.
   */
  private buildWallDetail() {
    const { x: ex, z: ez } = this.mapExtent;
    const ix = ex - 2.0; // inner wall face
    const iz = ez - 2.0;
    const pilasterMat = new THREE.MeshLambertMaterial({ color: 0x828b93, map: surface("concrete") });
    const skirtMat = new THREE.MeshLambertMaterial({ color: 0x40474e });

    const walls: { x?: number; z?: number }[] = [
      { z: -iz }, { z: iz }, { x: -ix }, { x: ix },
    ];
    for (const w of walls) {
      const alongX = w.z !== undefined;
      const len = (alongX ? ix : iz) * 2;
      // Skirting.
      const skirt = new THREE.Mesh(
        alongX
          ? new THREE.BoxGeometry(len, 0.5, 0.12)
          : new THREE.BoxGeometry(0.12, 0.5, len),
        skirtMat,
      );
      skirt.position.set(
        alongX ? 0 : w.x! * 0.995,
        0.25,
        alongX ? w.z! * 0.995 : 0,
      );
      this.mapGroup.add(skirt);
      // Pilasters.
      const count = Math.floor(len / 7);
      for (let i = 0; i <= count; i++) {
        const t = count > 0 ? i / count : 0.5;
        const along = (t - 0.5) * (len - 2);
        const pilaster = new THREE.Mesh(
          alongX
            ? tileBox(new THREE.BoxGeometry(0.5, 4.6, 0.22), 0.5, 4.6, 0.22)
            : tileBox(new THREE.BoxGeometry(0.22, 4.6, 0.5), 0.22, 4.6, 0.5),
          pilasterMat,
        );
        pilaster.position.set(
          alongX ? along : w.x! * 0.99,
          2.3,
          alongX ? w.z! * 0.99 : along,
        );
        pilaster.castShadow = true;
        this.mapGroup.add(pilaster);
      }
    }
  }

  /**
   * Tall trees and rock piles in a ring beyond the boundary walls — visible
   * over the wall line, never reachable, and deterministic per map size.
   */
  private buildScenery() {
    this.sceneryGroup.clear();
    const trunkMat = new THREE.MeshLambertMaterial({ color: 0x5d4028, map: surface("wood") });
    const canopyMats = [0x3e6b44, 0x4a7a4e, 0x35603c].map(
      (color) => new THREE.MeshLambertMaterial({ color }),
    );
    const rockMat = new THREE.MeshLambertMaterial({ color: 0x6c7076, map: surface("rock") });
    const bushMat = new THREE.MeshLambertMaterial({ color: 0x486e42 });
    const ring = Math.max(this.mapExtent.x, this.mapExtent.z);

    // The near ring: mixed forest with undergrowth and outcrops.
    for (let i = 0; i < 34; i++) {
      const a = i * 0.483 * Math.PI;
      const r = ring + 7 + ((i * 37) % 20);
      const x = Math.cos(a) * r;
      const z = Math.sin(a) * r;
      const kind = i % 5;
      if (kind === 2) {
        const rock = new THREE.Mesh(new THREE.DodecahedronGeometry(2.2 + (i % 4) * 0.7), rockMat);
        rock.position.set(x, 1.0, z);
        rock.rotation.set(i * 0.7, i * 1.3, 0);
        this.sceneryGroup.add(rock);
      } else if (kind === 4) {
        // A bush cluster: two squashed blobs.
        for (let k = 0; k < 2; k++) {
          const bush = new THREE.Mesh(new THREE.IcosahedronGeometry(1.2 + k * 0.5, 0), bushMat);
          bush.scale.y = 0.6;
          bush.position.set(x + k * 1.4, 0.7, z - k * 0.8);
          bush.rotation.y = i + k;
          this.sceneryGroup.add(bush);
        }
      } else {
        // A layered conifer: trunk plus three stacked canopy tiers, height
        // and shade varied per tree so the ring never reads as copies.
        const h = 8 + ((i * 53) % 7);
        const trunk = new THREE.Mesh(new THREE.CylinderGeometry(0.3, 0.5, h, 6), trunkMat);
        trunk.position.set(x, h / 2, z);
        this.sceneryGroup.add(trunk);
        const mat = canopyMats[i % canopyMats.length];
        for (let tier = 0; tier < 3; tier++) {
          const tr = 3.4 - tier * 0.95;
          const canopy = new THREE.Mesh(new THREE.ConeGeometry(tr, 3.4, 7), mat);
          canopy.position.set(x, h - 1.5 + tier * 2.2, z);
          canopy.rotation.y = i + tier * 0.5;
          this.sceneryGroup.add(canopy);
        }
      }
    }

    // The far ring: a low skyline of buildings, so the world past the trees
    // reads as somewhere people built, matching the arena's architecture.
    const buildingMats = [0x525c66, 0x47505a, 0x5c6670].map(
      (color) => new THREE.MeshLambertMaterial({ color, map: surface("concrete") }),
    );
    const windowMat = new THREE.MeshBasicMaterial({ color: 0xffd9a0 });
    for (let i = 0; i < 9; i++) {
      const a = i * 0.72 * Math.PI + 0.4;
      const r = ring + 46 + ((i * 61) % 30);
      const x = Math.cos(a) * r;
      const z = Math.sin(a) * r;
      const w = 8 + ((i * 29) % 8);
      const h = 14 + ((i * 43) % 22);
      const tower = new THREE.Mesh(
        new THREE.BoxGeometry(w, h, w * 0.8),
        buildingMats[i % buildingMats.length],
      );
      tower.position.set(x, h / 2, z);
      tower.rotation.y = a + Math.PI / 2;
      this.sceneryGroup.add(tower);
      // A few lit windows, brightest at night.
      for (let k = 0; k < 5; k++) {
        const win = new THREE.Mesh(new THREE.PlaneGeometry(0.9, 1.3), windowMat);
        const face = new THREE.Vector3(x, 0, z).normalize().multiplyScalar(-w * 0.41);
        win.position.set(
          x + face.x + (hash01(i * 7 + k, 51) - 0.5) * w * 0.7,
          3 + hash01(i * 7 + k, 57) * (h - 5),
          z + face.z + (hash01(i * 7 + k, 59) - 0.5) * w * 0.5,
        );
        win.lookAt(0, win.position.y, 0);
        this.sceneryGroup.add(win);
      }
      // A rooftop water tank or antenna block.
      const cap = new THREE.Mesh(new THREE.BoxGeometry(2, 1.6, 2), buildingMats[(i + 1) % 3]);
      cap.position.set(x, h + 0.8, z);
      this.sceneryGroup.add(cap);
    }
  }

  /**
   * Night neon: emissive strips along the boundary walls plus a handful of
   * real point lights, so Night stays moody but never unreadable.
   */
  private buildNeon() {
    this.neonGroup.clear();
    const { x: ex, z: ez } = this.mapExtent;
    const colors = [0x2fe0d0, 0xff4fd8, 0xffb02e, 0x7c9dff];
    const stripY = 3.4;
    const strips: { x: number; z: number; alongX: boolean }[] = [
      { x: 0, z: -ez + 0.35, alongX: true },
      { x: 0, z: ez - 0.35, alongX: true },
      { x: -ex + 0.35, z: 0, alongX: false },
      { x: ex - 0.35, z: 0, alongX: false },
    ];
    strips.forEach((s, i) => {
      const len = (s.alongX ? ex : ez) * 1.5;
      const geo = s.alongX
        ? new THREE.BoxGeometry(len, 0.16, 0.1)
        : new THREE.BoxGeometry(0.1, 0.16, len);
      const mesh = new THREE.Mesh(
        geo,
        new THREE.MeshBasicMaterial({ color: colors[i % colors.length] }),
      );
      mesh.position.set(s.x, stripY, s.z);
      this.neonGroup.add(mesh);
      const light = new THREE.PointLight(colors[i % colors.length], 2.6, 26, 1.6);
      light.position.set(s.x * 0.9, stripY + 0.5, s.z * 0.9);
      this.neonGroup.add(light);
    });
    // One warm lamp over the middle of the map.
    const centre = new THREE.PointLight(0xffd9a0, 2.2, 30, 1.5);
    centre.position.set(0, 6.0, 0);
    this.neonGroup.add(centre);
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

  /**
   * A humanoid figure: articulated legs and arms for a walk cycle, a
   * distinct head volume (because it is a distinct hitbox), and per-character
   * gear so Ward, Vane, Echo and Kiln read as different people at a glance.
   * Team identity stays on the torso, arms and head via one shared material,
   * which is also what the state tints (marked / staggered / carrying) drive.
   */
  private makeAvatar(team: number, character: number): THREE.Group {
    const g = new THREE.Group();
    const color = TEAM_COLOR[team];
    const bodyMat = new THREE.MeshLambertMaterial({ color });
    const darkMat = new THREE.MeshLambertMaterial({ color: 0x20262c });
    const gearMat = new THREE.MeshLambertMaterial({ color: 0x3a424b });
    const skinMat = new THREE.MeshLambertMaterial({ color: 0xc9a184 });
    const box = (w: number, h: number, d: number, mat: THREE.Material) =>
      new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat);

    // A limb is a pivot group at the joint with the mesh hanging below it,
    // so the walk cycle can rotate it about the hip or shoulder.
    const limb = (name: string, w: number, len: number, mat: THREE.Material) => {
      const pivot = new THREE.Group();
      pivot.name = name;
      const m = box(w, len, w, mat);
      m.position.y = -len / 2;
      pivot.add(m);
      return pivot;
    };

    const legL = limb("legL", 0.22, 0.9, darkMat);
    legL.position.set(-0.16, 0.9, 0);
    const legR = limb("legR", 0.22, 0.9, darkMat);
    legR.position.set(0.16, 0.9, 0);

    const torso = box(0.6, 0.62, 0.32, bodyMat);
    torso.position.y = 1.22;
    const belt = box(0.62, 0.1, 0.34, gearMat);
    belt.position.y = 0.93;

    const armL = limb("armL", 0.15, 0.7, bodyMat);
    armL.position.set(-0.39, 1.48, 0);
    armL.rotation.x = -0.3;
    // The right arm is raised toward the weapon and stays out of the cycle.
    const armR = limb("armR", 0.15, 0.7, bodyMat);
    armR.position.set(0.39, 1.48, 0);
    armR.rotation.x = -1.15;
    // Hands.
    for (const arm of [armL, armR]) {
      const hand = box(0.13, 0.12, 0.13, skinMat);
      hand.position.y = -0.72;
      arm.add(hand);
    }

    const neck = box(0.14, 0.1, 0.14, skinMat);
    neck.position.y = 1.56;
    const head = box(0.34, 0.32, 0.34, skinMat);
    head.position.y = 1.72;
    head.name = "head";
    // Eyes / visor line on the face.
    const visor = box(0.3, 0.07, 0.04, darkMat);
    visor.name = "visor";
    visor.position.set(0, 1.74, 0.18);

    const gun = box(0.11, 0.11, 0.8, darkMat);
    gun.position.set(0.28, 1.26, 0.42);

    g.add(legL, legR, torso, belt, armL, armR, neck, head, visor, gun);

    // Character kits: silhouette accents that never recolor with the team.
    switch (character) {
      case 0: { // Ward: armour — full helmet, heavy chest plate, broad frame.
        const helmet = box(0.4, 0.24, 0.4, gearMat);
        helmet.position.y = 1.84;
        const chest = box(0.5, 0.42, 0.08, gearMat);
        chest.position.set(0, 1.26, 0.19);
        torso.scale.set(1.12, 1, 1.15);
        g.add(helmet, chest);
        break;
      }
      case 1: { // Vane: fast and slim — hood and a trailing scarf.
        const hood = new THREE.Mesh(new THREE.ConeGeometry(0.26, 0.34, 6), gearMat);
        hood.position.y = 1.92;
        const scarf = box(0.3, 0.1, 0.5, gearMat);
        scarf.position.set(0, 1.5, -0.28);
        torso.scale.set(0.92, 1, 0.92);
        g.add(hood, scarf);
        break;
      }
      case 2: { // Echo: sensors — glowing visor band and an antenna.
        const band = box(0.36, 0.08, 0.06, new THREE.MeshBasicMaterial({ color: 0x35e0ff }));
        band.name = "visor";
        band.position.set(0, 1.74, 0.19);
        const mast = new THREE.Mesh(new THREE.CylinderGeometry(0.015, 0.015, 0.34, 4), darkMat);
        mast.position.set(0.14, 2.02, -0.1);
        const tip = new THREE.Mesh(
          new THREE.SphereGeometry(0.035, 6, 5),
          new THREE.MeshBasicMaterial({ color: 0x35e0ff }),
        );
        tip.position.set(0.14, 2.2, -0.1);
        g.add(band, mast, tip);
        break;
      }
      case 3: { // Kiln: fire — shoulder pauldrons and an ember line on the chest.
        for (const side of [-1, 1]) {
          const pad = box(0.24, 0.14, 0.3, gearMat);
          pad.position.set(0.36 * side, 1.6, 0);
          g.add(pad);
        }
        const ember = box(0.06, 0.4, 0.05, new THREE.MeshBasicMaterial({ color: 0xff7a1e }));
        ember.position.set(0, 1.24, 0.18);
        g.add(ember);
        break;
      }
    }

    g.traverse((o) => {
      if ((o as THREE.Mesh).isMesh) o.castShadow = true;
    });
    g.userData.bodyMat = bodyMat;
    return g;
  }

  syncPlayers(players: RenderPlayer[]) {
    const seen = new Set<number>();
    for (const p of players) {
      seen.add(p.slot);
      let g = this.avatars.get(p.slot);
      if (!g) {
        g = this.makeAvatar(p.team, p.character);
        this.avatars.set(p.slot, g);
        this.scene.add(g);
      }
      g.visible = p.alive && !p.isLocal;
      if (!g.visible) continue;

      // Walk cycle driven by how far the avatar actually moved this frame.
      const ud = g.userData as {
        bodyMat: THREE.MeshLambertMaterial;
        px?: number; pz?: number; phase?: number; amp?: number;
      };
      const dist = Math.hypot(p.x - (ud.px ?? p.x), p.z - (ud.pz ?? p.z));
      ud.px = p.x;
      ud.pz = p.z;
      ud.phase = (ud.phase ?? 0) + dist * 5.5;
      ud.amp = (ud.amp ?? 0) * 0.85 + Math.min(1, dist * 25) * 0.15;
      const swing = Math.sin(ud.phase) * 0.6 * ud.amp;
      (g.getObjectByName("legL") as THREE.Group).rotation.x = swing;
      (g.getObjectByName("legR") as THREE.Group).rotation.x = -swing;
      (g.getObjectByName("armL") as THREE.Group).rotation.x = -0.3 - swing * 0.7;

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
      ud.bodyMat.color.setHex(tint);
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

  /**
   * Draw one character alone on a small canvas, slowly turning: the death
   * screen's "who got you" card. It runs its own tiny renderer and scene
   * because the main canvas is frozen as the death backdrop and cannot be
   * drawn into. Built once per character and reused after that.
   */
  portraitFrame(canvas: HTMLCanvasElement, character: number, team: number, dt: number) {
    if (!this.portrait) {
      const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setSize(canvas.clientWidth || 160, canvas.clientHeight || 200, false);
      const scene = new THREE.Scene();
      scene.add(new THREE.HemisphereLight(0xc8d8e8, 0x1b2026, 1.5));
      const key = new THREE.DirectionalLight(0xfff0dc, 2.0);
      key.position.set(2.5, 4, 3);
      const rim = new THREE.DirectionalLight(0x7fb0d8, 1.1);
      rim.position.set(-3, 2, -2.5);
      scene.add(key, rim);
      const camera = new THREE.PerspectiveCamera(32, (canvas.clientWidth || 160) / (canvas.clientHeight || 200), 0.1, 20);
      camera.position.set(0, 1.35, 4.6);
      camera.lookAt(0, 1.15, 0);
      this.portrait = { renderer, scene, camera, avatar: null, character: -1, team: -1, spin: 0 };
    }
    const p = this.portrait;
    if (p.character !== character || p.team !== team) {
      if (p.avatar) p.scene.remove(p.avatar);
      p.avatar = this.makeAvatar(team, character);
      p.scene.add(p.avatar);
      p.character = character;
      p.team = team;
      p.spin = -0.5;
    }
    // A slow turn, so the silhouette reads from more than one angle.
    p.spin += dt * 0.7;
    p.avatar!.rotation.y = Math.sin(p.spin) * 0.6 + Math.PI;
    p.renderer.render(p.scene, p.camera);
  }

  /** Age tracers, impacts and the muzzle flash, retiring the expired ones. */
  private stepTransients(dt: number) {
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
    // Transients age even while frozen. Shots go on being reported from all
    // over the map during a respawn, and if their tracers did not expire they
    // would all still be in the scene, at full opacity, on the first frame
    // after the freeze lifts.
    this.stepTransients(dt);

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
