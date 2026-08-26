// TICK client.
//
// The loop: sample input at a fixed 64 Hz, predict the local player with the
// same WebAssembly simulation the server runs, render other players 100 ms in
// the past so their motion is smooth, and reconcile against every snapshot.
// Everything authoritative — damage, hit detection, score — happens on the
// server; this file is a renderer with opinions about latency.

import { Audio } from "./audio";
import { Hud } from "./hud";
import { InputState } from "./input";
import { Net } from "./net";
import { BTN, type InputCmd, type Snapshot } from "./proto";
import { Renderer, type RenderPlayer } from "./render";
import { Sim } from "./sim";

const TICK_HZ = 64;
const TICK_MS = 1000 / TICK_HZ;
const EYE_HEIGHT = 1.62;
const CROUCH_EYE = 0.97;
/** Bit positions match StaticEvent in the simulation crate. */
const EVENT_GRAVITY_DIP = 1 << 1;
const EVENT_BLACKOUT = 1 << 0;
const EVENT_SILENCE = 1 << 4;

type Phase = "lobby" | "queued" | "match" | "results" | "standby";

interface RosterEntry {
  slot: number;
  name: string;
  team: number;
  character: number;
  weapon: number;
  bot: boolean;
}

/** Loadout picks by number key: 1 AR, 2 sniper, 3 sidearm, 4 melee. */
const LOADOUT_WEAPONS = [0, 1, 4, 6];

const canvas = document.getElementById("view") as HTMLCanvasElement;
const lobby = document.getElementById("lobby") as HTMLElement;
const deathOverlay = document.getElementById("deathOverlay") as HTMLElement;
const deathTitle = document.getElementById("deathTitle") as HTMLElement;
const loadoutRow = document.getElementById("loadoutRow") as HTMLElement;
const spawnCard = document.getElementById("spawnCard") as HTMLElement;
const results = document.getElementById("results") as HTMLElement;
const standby = document.getElementById("standby") as HTMLElement;
const playButton = document.getElementById("playButton") as HTMLButtonElement;
const againButton = document.getElementById("againButton") as HTMLButtonElement;
const nameInput = document.getElementById("nameInput") as HTMLInputElement;
const queueState = document.getElementById("queueState") as HTMLElement;

const renderer = new Renderer(canvas);
const hud = new Hud();
const audio = new Audio();
const input = new InputState(canvas);
const net = new Net();

let sim: Sim | null = null;
let phase: Phase = "lobby";
let roster: RosterEntry[] = [];
let mySlot = 0;
let myCharacter = 0;
let myWeapon = 0;
let myMode = 0;
let ghostPingSpent = false;
let currentWeather = 0;

let snapshots: Snapshot[] = [];
let latest: Snapshot | null = null;
let inputSeq = 0;
let pending: InputCmd[] = [];
let accumulator = 0;
let lastFrame = performance.now();
let fps = 60;
/** Visual correction offset, decayed to zero so reconciliation never snaps. */
let smoothing: [number, number, number] = [0, 0, 0];
let standbySince = 0;
let botTookOver = false;
let againCountdown = 0;
let adsAmount = 0;
let isDead = false;
/** Where our last landed shot hit, for anchoring damage numbers. */
let lastImpact: { p: number[]; at: number } | null = null;
const counters = { frames: 0, matchFrames: 0, steps: 0, sent: 0 };

nameInput.value = localStorage.getItem("tick.name") ?? "";

// ---------------------------------------------------------------- lifecycle

async function boot() {
  sim = await Sim.load("/tick_sim.wasm");
  net.onJson = onJson;
  net.onSnapshot = onSnapshot;
  net.onClose = () => {
    queueState.textContent = "Disconnected. Reload to reconnect.";
    setPhase("lobby");
  };
  net.connect(nameInput.value || "Player");
  requestAnimationFrame(frame);
}

playButton.addEventListener("click", () => {
  audio.start();
  const name = nameInput.value.trim() || "Player";
  localStorage.setItem("tick.name", name);
  net.send({ t: "hello", name });
  net.send({ t: "play" });
  setPhase("queued");
  queueState.textContent = "Finding a match…";
});

againButton.addEventListener("click", () => {
  net.send({ t: "play" });
  results.classList.add("hidden");
  setPhase("queued");
  queueState.textContent = "Finding a match…";
});

// Clicks do three jobs outside the lobby. A click is also the only reliable
// way back into pointer lock: the browser rejects requestPointerLock calls
// made outside a user gesture, so returning from a tab switch or a lost lock
// has to go through here.
window.addEventListener("mousedown", () => {
  if (phase === "standby") {
    leaveStandby();
    return;
  }
  if (phase !== "match") return;
  if (isDead) {
    // The mouse is deliberately free while dead: loadout buttons are
    // clickable, and in Last Light a click spends the ghost ping.
    if (myMode === 3 && !ghostPingSpent) {
      net.send({ t: "ghostping" });
      ghostPingSpent = true;
    }
    return;
  }
  if (!input.locked) input.requestLock();
});

input.onStandby = () => {
  if (phase === "match") enterStandby();
  else if (phase === "standby") leaveStandby();
};

// A hidden tab stops receiving animation frames, which means it stops sending
// input: the browser has already decided the player stepped away. Treat that
// as Standby rather than as a player standing still in the open.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    if (phase === "match") enterStandby();
  } else if (phase === "standby") {
    leaveStandby();
  }
});

input.onLockChange = (locked) => {
  // Losing the pointer lock mid-match is the most common way a player steps
  // away, so it is treated as the start of standby rather than as nothing —
  // except while dead, where the lock is released on purpose so the mouse
  // can click the loadout buttons.
  if (!locked && phase === "match" && !isDead) enterStandby();
};

input.onWeaponKey = (n) => chooseLoadout(LOADOUT_WEAPONS[n - 1]);
for (const btn of Array.from(loadoutRow.querySelectorAll<HTMLButtonElement>(".loadoutBtn"))) {
  btn.addEventListener("click", () => chooseLoadout(Number(btn.dataset.w)));
}

function chooseLoadout(w: number) {
  if (phase !== "match" || !isDead || myMode === 3) return;
  net.send({ t: "loadout", w });
  for (const btn of Array.from(loadoutRow.querySelectorAll<HTMLButtonElement>(".loadoutBtn"))) {
    btn.classList.toggle("chosen", Number(btn.dataset.w) === w);
  }
}

/** Death: freeze the backdrop, hand the mouse back, offer the loadout. */
function enterDeath() {
  isDead = true;
  renderer.frozen = true;
  input.releaseLock();
  const hint = document.getElementById("deathHint") as HTMLElement;
  if (myMode === 3) {
    deathTitle.textContent = "Eliminated";
    loadoutRow.classList.add("hidden");
    hint.textContent = ghostPingSpent
      ? "Ghost ping spent · watching"
      : "Click to spend your ghost ping";
  } else {
    deathTitle.textContent = "Respawning";
    loadoutRow.classList.remove("hidden");
    hint.textContent = "Pick with 1–4 or click · applies when you respawn";
  }
  deathOverlay.classList.remove("hidden");
}

function leaveDeath() {
  isDead = false;
  renderer.frozen = false;
  deathOverlay.classList.add("hidden");
  if (phase === "match") input.requestLock();
}

function setPhase(next: Phase) {
  phase = next;
  lobby.classList.toggle("hidden", next !== "lobby" && next !== "queued");
  standby.classList.toggle("hidden", next !== "standby");
  if (next === "match") {
    hud.show();
    if (!isDead) input.requestLock();
  } else {
    if (next !== "standby") {
      hud.hide();
      deathOverlay.classList.add("hidden");
      renderer.frozen = false;
      isDead = false;
    }
    input.releaseLock();
  }
  playButton.disabled = next === "queued";
}

function enterStandby() {
  if (phase !== "match") return;
  setPhase("standby");
  standbySince = performance.now();
  botTookOver = false;
  audio.setMuted(true);
}

function leaveStandby() {
  if (phase !== "standby") return;
  if (botTookOver) net.send({ t: "standby", on: false });
  botTookOver = false;
  audio.setMuted(false);
  setPhase("match");
}

// ------------------------------------------------------------------ messages

function onJson(msg: any) {
  switch (msg.t) {
    case "queued":
      queueState.textContent =
        msg.position > 1 ? `In queue · ${msg.position} waiting` : "Finding a match…";
      break;

    case "start": {
      roster = msg.players as RosterEntry[];
      mySlot = msg.you;
      const me = roster[mySlot];
      myCharacter = me.character;
      myWeapon = me.weapon;
      currentWeather = msg.weather;
      myMode = msg.mode;
      ghostPingSpent = false;
      snapshots = [];
      latest = null;
      pending = [];
      inputSeq = 0;
      smoothing = [0, 0, 0];
      isDead = false;
      renderer.frozen = false;
      deathOverlay.classList.add("hidden");
      for (const btn of Array.from(loadoutRow.querySelectorAll(".loadoutBtn"))) {
        btn.classList.remove("chosen");
      }

      const brushes = sim!.loadMap(msg.map);
      renderer.buildMap(brushes, msg.weather);
      renderer.setViewmodel(myWeapon);
      hud.setMode(msg.modeName);
      showSpawnCard(msg);
      setPhase("match");
      break;
    }

    case "ev":
      for (const e of msg.e) handleEvent(e);
      break;

    case "end":
      showResults(msg);
      break;
  }
}

function showSpawnCard(msg: any) {
  const names = ["Ward", "Vane", "Echo", "Kiln"];
  const guns = ["Sting", "Ridge", "Maul", "Arc", "Tack", "Lance", "Blade"];
  (document.getElementById("draftChar") as HTMLElement).textContent = names[myCharacter];
  (document.getElementById("draftWeapon") as HTMLElement).textContent = guns[myWeapon];
  (document.getElementById("draftMap") as HTMLElement).textContent = msg.mapName;
  (document.getElementById("draftWeather") as HTMLElement).textContent = msg.weatherName;
  (document.getElementById("draftMode") as HTMLElement).textContent =
    `${msg.modeName} · first to ${msg.scoreTarget}`;
  spawnCard.classList.remove("hidden");
  window.setTimeout(() => spawnCard.classList.add("hidden"), 2000);
}

function nameOf(slot: number): string {
  return roster[slot]?.name ?? `#${slot}`;
}

function handleEvent(e: any) {
  switch (e.e) {
    case "shot": {
      const silenced = (latest?.eventBits ?? 0) & EVENT_SILENCE;
      // Our own shot originates at our eye, so the server's line is collinear
      // with the view and renders as a dot. Start it from the viewmodel's
      // muzzle instead; everyone else's tracer uses the true origin.
      let origin = e.o;
      if (e.slot === mySlot) {
        const cp = Math.cos(input.pitch);
        const fwd = [Math.sin(input.yaw) * cp, Math.sin(input.pitch), Math.cos(input.yaw) * cp];
        const right = [-Math.cos(input.yaw), 0, Math.sin(input.yaw)];
        origin = [
          e.o[0] + right[0] * 0.2 + fwd[0] * 0.5,
          e.o[1] - 0.16 + fwd[1] * 0.5,
          e.o[2] + right[2] * 0.2 + fwd[2] * 0.5,
        ];
      }
      renderer.spawnTracer(origin, e.p, e.hit);
      if (e.hit) renderer.spawnImpact(e.p, e.hs ? 0xffd447 : 0xffe0b0);
      if (e.slot === mySlot && e.hit) lastImpact = { p: e.p, at: performance.now() };
      if (!silenced) {
        const eye = cameraEye();
        audio.shot(e.w, e.o, eye, input.yaw);
      }
      if (e.slot === mySlot) renderer.kickRecoil(0.012 + Math.random() * 0.01);
      break;
    }
    case "dmg":
      if (e.a === mySlot && e.v !== mySlot) {
        hud.hitmark(e.hs);
        audio.hitmarker(e.hs);
        // Anchor the number to the shot's impact point when it is fresh
        // (melee and projectile damage arrive without one).
        const fresh = lastImpact && performance.now() - lastImpact.at < 200;
        const at = fresh ? renderer.projectToScreen(lastImpact!.p) : null;
        hud.damageNumber(e.n, e.hs, at);
      }
      break;
    case "kill": {
      const mine = e.attacker === mySlot;
      hud.killRow(nameOf(e.attacker), nameOf(e.victim), e.weapon, e.headshot, mine);
      if (mine) {
        const labels = (e.bonuses as { label: string; points: number }[])
          .map((b) => (b.points > 0 ? `${b.label} +${b.points}` : b.label));
        if (e.headshot) labels.unshift("Headshot");
        hud.bonusPopup(e.points, labels);
        audio.blip(660, 0.18, "triangle", 0.3);
      }
      break;
    }
    case "telegraph":
      hud.telegraph(e.name, e.blurb);
      audio.telegraph();
      break;
    case "eventStart":
      hud.eventStart(e.name, e.blurb, e.underdog);
      if (e.k === 0) renderer.setBlackout(true);
      if (e.k === 5) currentWeather = (currentWeather + 1) % 3;
      renderer.setWeather(currentWeather);
      audio.blip(180, 0.4, "sawtooth", 0.3);
      break;
    case "eventEnd":
      hud.eventEnd();
      if (e.k === 0) renderer.setBlackout(false);
      break;
    case "pickup":
      if (e.slot === mySlot) {
        myWeapon = e.w;
        renderer.setViewmodel(myWeapon);
      }
      break;
    case "revealed":
      // Being scanned is information you are given, not information taken
      // from you: the outline is mutual.
      if (e.slot === mySlot) {
        hud.warn("Scanned");
        audio.blip(1180, 0.22, "sine", 0.28);
      }
      break;
    case "ghostPing":
      if (roster[e.by]?.team === roster[mySlot]?.team) {
        hud.warn(`Ghost ping · ${nameOf(e.target)} marked`);
        audio.blip(880, 0.14, "triangle", 0.24);
      }
      break;
    case "roundEnd":
      ghostPingSpent = false;
      hud.warn(e.winner === 255 ? "Round drawn" : `Round to ${e.winner === (roster[mySlot]?.team ?? 0) ? "your team" : "them"}`);
      break;
    case "spawn":
      if (e.slot === mySlot) {
        smoothing = [0, 0, 0];
        audio.blip(520, 0.1, "sine", 0.2);
      }
      break;
  }
}

function onSnapshot(snap: Snapshot) {
  latest = snap;
  snapshots.push(snap);
  if (snapshots.length > 24) snapshots.shift();
  reconcile(snap);
  const meRow = snap.players.find((p) => p.slot === mySlot);
  if (meRow) {
    if (meRow.weapon !== myWeapon) {
      myWeapon = meRow.weapon;
      renderer.setViewmodel(myWeapon);
    }
    // The snapshot is the authority on being dead, so the death screen keys
    // off it rather than off kill events.
    if (phase === "match" && !meRow.alive && !isDead) enterDeath();
    else if (meRow.alive && isDead) leaveDeath();
  }
  renderer.syncProps(snap);
  if (snap.weather !== currentWeather) {
    currentWeather = snap.weather;
    renderer.setWeather(currentWeather);
  }
}

/**
 * Rewind to the server's authoritative state and replay every input it has
 * not acknowledged yet. The replay runs the same compiled movement code the
 * server ran, so the only difference that can survive is packet loss.
 */
function reconcile(snap: Snapshot) {
  if (!sim) return;
  const me = snap.players.find((p) => p.slot === mySlot);
  if (!me) return;

  const before = sim.pos;
  sim.setState(me.x, me.y, me.z, snap.vel[0], snap.vel[1], snap.vel[2], snap.onGround, me.crouching);
  pending = pending.filter((c) => c.seq > snap.ack);
  for (const cmd of pending) {
    sim.step(
      cmd.buttons,
      cmd.yaw,
      cmd.pitch,
      speedMultiplier(),
      gravityMultiplier(),
      canSprint(),
    );
  }
  const after = sim.pos;
  // Carry the correction as a decaying visual offset instead of teleporting
  // the camera; a 3 cm disagreement should never be visible.
  smoothing = [
    smoothing[0] + (before[0] - after[0]),
    smoothing[1] + (before[1] - after[1]),
    smoothing[2] + (before[2] - after[2]),
  ];
  const mag = Math.hypot(smoothing[0], smoothing[1], smoothing[2]);
  if (mag > 2.5) smoothing = [0, 0, 0];
}

function speedMultiplier(): number {
  let m = myCharacter === 1 ? 1.1 : 1.0;
  const me = latest?.players.find((p) => p.slot === mySlot);
  if (me?.staggered) m *= 0.45;
  if (me?.carrying) m *= 0.88;
  return m;
}

function gravityMultiplier(): number {
  return (latest?.eventBits ?? 0) & EVENT_GRAVITY_DIP ? 0.4 : 1.0;
}

function canSprint(): boolean {
  const me = latest?.players.find((p) => p.slot === mySlot);
  return !me?.staggered && !me?.carrying;
}

function interpDelayMs(): number {
  // Wide enough to cover jitter, narrow enough that a peek is not a surprise.
  return Math.min(200, Math.max(80, 80 + net.rtt * 0.25));
}

function cameraEye(): [number, number, number] {
  if (!sim) return [0, 0, 0];
  const p = sim.pos;
  const eye = sim.crouching ? CROUCH_EYE : EYE_HEIGHT;
  return [p[0] + smoothing[0], p[1] + eye + smoothing[1], p[2] + smoothing[2]];
}

// -------------------------------------------------------------------- loop

function frame(now: number) {
  requestAnimationFrame(frame);
  counters.frames++;
  const dt = Math.min(0.1, (now - lastFrame) / 1000);
  lastFrame = now;
  fps = fps * 0.9 + (1 / Math.max(dt, 0.0001)) * 0.1;

  if (phase === "standby") {
    // Standby keeps the connection alive and hands the character to a bot
    // after twelve seconds, so the team never plays a man down.
    if (!botTookOver && now - standbySince > 12000) {
      botTookOver = true;
      net.send({ t: "standby", on: true });
    }
    renderer.update(dt, cameraEye(), input.yaw, input.pitch, 0, 0);
    return;
  }

  if (phase === "match" && sim) {
    counters.matchFrames++;
    accumulator += dt * 1000;
    let steps = 0;
    while (accumulator >= TICK_MS && steps < 8) {
      accumulator -= TICK_MS;
      steps++;
      stepLocal();
    }
    // Decay the reconciliation offset toward zero.
    const decay = Math.max(0, 1 - dt * 12);
    smoothing = [smoothing[0] * decay, smoothing[1] * decay, smoothing[2] * decay];

    const wantAds = (input.buttons & BTN.ADS) !== 0 ? 1 : 0;
    adsAmount += (wantAds - adsAmount) * Math.min(1, dt * 12);

    renderPlayers();
    updateHud();
  }

  const speed = sim ? Math.hypot(sim.vel[0], sim.vel[2]) : 0;
  renderer.update(dt, cameraEye(), input.yaw, input.pitch, adsAmount, speed);
  hud.tick(dt);

  if (againCountdown > 0) {
    againCountdown -= dt;
    const el = document.getElementById("againTimer");
    if (el) el.textContent = `· ${Math.max(0, Math.ceil(againCountdown))}`;
    if (againCountdown <= 0) againButton.click();
  }
}

function stepLocal() {
  counters.steps++;
  if (!sim) return;
  const cmd: InputCmd = {
    seq: ++inputSeq,
    yaw: input.yaw,
    pitch: input.pitch,
    buttons: input.buttons,
  };
  pending.push(cmd);
  if (pending.length > 64) pending.shift();
  sim.step(cmd.buttons, cmd.yaw, cmd.pitch, speedMultiplier(), gravityMultiplier(), canSprint());
  // Report our own interpolation width so the server rewinds by exactly the
  // amount we are actually rendering behind.
  const interpTicks = Math.round((net.rtt / 2 + interpDelayMs()) / TICK_MS);
  net.sendInput(pending, interpTicks);
  counters.sent++;
}

/** Draw other players 100-200 ms in the past, between two real snapshots. */
function renderPlayers() {
  const renderTime = performance.now() - interpDelayMs();
  let older: Snapshot | null = null;
  let newer: Snapshot | null = null;
  for (let i = snapshots.length - 1; i >= 0; i--) {
    if (snapshots[i].received <= renderTime) {
      older = snapshots[i];
      newer = snapshots[i + 1] ?? null;
      break;
    }
  }
  if (!older) older = snapshots[0] ?? null;
  if (!older) return;

  const span = newer ? newer.received - older.received : 0;
  const t = span > 0 ? Math.min(1, Math.max(0, (renderTime - older.received) / span)) : 0;

  const out: RenderPlayer[] = [];
  for (const p of older.players) {
    const n = newer?.players.find((q) => q.slot === p.slot);
    const x = n ? p.x + (n.x - p.x) * t : p.x;
    const y = n ? p.y + (n.y - p.y) * t : p.y;
    const z = n ? p.z + (n.z - p.z) * t : p.z;
    const yaw = n ? lerpAngle(p.yaw, n.yaw, t) : p.yaw;
    out.push({
      slot: p.slot,
      x, y, z, yaw,
      pitch: p.pitch,
      team: p.team,
      alive: p.alive,
      crouching: p.crouching,
      marked: p.marked,
      staggered: p.staggered,
      carrying: p.carrying,
      isLocal: p.slot === mySlot,
    });
  }
  renderer.syncPlayers(out);
}

function lerpAngle(a: number, b: number, t: number): number {
  let d = b - a;
  while (d > Math.PI) d -= Math.PI * 2;
  while (d < -Math.PI) d += Math.PI * 2;
  return a + d * t;
}

function updateHud() {
  if (!latest) return;
  const me = latest.players.find((p) => p.slot === mySlot);
  hud.setScores(latest.scoreA, latest.scoreB);
  hud.setClock(latest.timeLeft);
  // Last Light has no respawn, so the dead notice says what is actually
  // true and what the player can still do about it.
  const deadLabel =
    myMode === 3
      ? ghostPingSpent
        ? "Eliminated · ping spent"
        : "Eliminated · click to ghost ping"
      : "Respawning";
  hud.setVitals(me?.health ?? 0, me?.armor ?? 0, me?.alive ?? false, deadLabel);
  hud.setKit(myCharacter, latest.abilityCooldown);
  hud.setCharge(latest.charge, latest.focus > 0);
  hud.setWeapon(myWeapon, latest.ammo, latest.reload > 0);
  hud.setNet(net.rtt, fps, interpDelayMs());
  renderer.setBlackout((latest.eventBits & EVENT_BLACKOUT) !== 0);
}

// ----------------------------------------------------------------- results

function showResults(msg: any) {
  setPhase("results");
  results.classList.remove("hidden");
  hud.hide();

  const mine = (msg.table as any[]).find((r) => r.slot === msg.you);
  const ar = Math.round(mine?.aimRating ?? 0);
  const history: number[] = JSON.parse(localStorage.getItem("tick.ar") ?? "[]");
  const average = history.length
    ? history.reduce((a, b) => a + b, 0) / history.length
    : ar;
  history.push(ar);
  localStorage.setItem("tick.ar", JSON.stringify(history.slice(-20)));

  (document.getElementById("arNumber") as HTMLElement).textContent = String(ar);
  const delta = document.getElementById("arDelta") as HTMLElement;
  const diff = ar - Math.round(average);
  delta.textContent = history.length > 1 ? `${diff >= 0 ? "+" : ""}${diff} vs your last 20` : "";
  delta.classList.toggle("down", diff < 0);

  const outcome = document.getElementById("outcome") as HTMLElement;
  const myTeam = roster[msg.you]?.team ?? 0;
  outcome.textContent =
    msg.winner === 255 ? "Draw" : msg.winner === myTeam ? "Victory" : "Defeat";

  const best = document.getElementById("bestKill") as HTMLElement;
  if (msg.bestKill) {
    const b = msg.bestKill;
    const labels = (b.bonuses as { label: string }[]).map((x) => x.label).join(" · ");
    best.textContent =
      `Best kill: ${b.points} pts on ${nameOf(b.victim)} at ${b.distance.toFixed(1)} m` +
      (labels ? ` — ${labels}` : "");
  } else {
    best.textContent = "No kills this match. The next one is four minutes away.";
  }

  const table = document.getElementById("scoreTable") as HTMLTableElement;
  table.innerHTML =
    "<tr><th>Player</th><th>Kit</th><th>Score</th><th>K</th><th>D</th><th>HS</th>" +
    "<th>Acc</th><th>AR</th></tr>";
  for (const r of msg.table as any[]) {
    const tr = document.createElement("tr");
    if (r.slot === msg.you) tr.className = "you";
    tr.innerHTML =
      `<td class="team${r.team}">${r.name}${r.bot ? " ·bot" : ""}</td>` +
      `<td>${r.character} / ${r.weapon}</td>` +
      `<td>${r.score}</td><td>${r.kills}</td><td>${r.deaths}</td>` +
      `<td>${r.headshotKills}</td>` +
      `<td>${Math.round(r.accuracy * 100)}%</td>` +
      `<td>${Math.round(r.aimRating)}</td>`;
    table.appendChild(tr);
  }

  againCountdown = 8;
}

// Development aid: inspect and drive the game without a pointer lock, so the
// client can be exercised from a script during verification.
(window as any).tick = {
  look(yaw: number, pitch: number) {
    input.yaw = yaw;
    input.pitch = pitch;
  },
  press(mask: number) {
    input.buttons |= mask;
  },
  release(mask: number) {
    input.buttons &= ~mask;
  },
  BTN,
  renderer,
  hud,
  state: () => ({
    phase,
    mySlot,
    character: myCharacter,
    weapon: myWeapon,
    weather: currentWeather,
    pos: sim?.pos,
    onGround: sim?.onGround,
    visiblePlayers: latest?.players.length ?? 0,
    scores: latest ? [latest.scoreA, latest.scoreB] : null,
    timeLeft: latest?.timeLeft ?? 0,
    eventBits: latest?.eventBits ?? 0,
    rtt: net.rtt,
    fps,
    buttons: input.buttons,
    seq: inputSeq,
    ack: latest?.ack ?? 0,
    pending: pending.length,
    counters: { ...counters },
    accumulator,
  }),
  players: () => latest?.players ?? [],
  /** Aim at a world point, for scripted verification. */
  aimAt(x: number, y: number, z: number) {
    const eye = cameraEye();
    const dx = x - eye[0];
    const dy = y - eye[1];
    const dz = z - eye[2];
    input.yaw = Math.atan2(dx, dz);
    input.pitch = Math.asin(dy / Math.hypot(dx, dy, dz));
  },
};

boot();
