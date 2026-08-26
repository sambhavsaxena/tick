// A headless TICK client.
//
// Speaks the real wire protocol over the real socket: same input packets, same
// snapshot decoding, same 64 Hz cadence as the browser. It exists so the whole
// path — encode, transport, authoritative tick, lag-compensated hit
// registration, scoring — can be exercised without a renderer, and so the
// server can be load-tested with more players than there are humans around.
//
//   node tools/headless-client.mjs [--clients N] [--url ws://127.0.0.1:8080/ws]
//                                  [--skill 0..1] [--quiet]

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const URL = opt("url", "ws://127.0.0.1:8080/ws");
const CLIENTS = Number(opt("clients", "1"));
const SKILL = Number(opt("skill", "0.7"));
const QUIET = args.includes("--quiet");

const TICK_MS = 1000 / 64;
const BTN = { FWD: 1, BACK: 2, LEFT: 4, RIGHT: 8, JUMP: 16, CROUCH: 32, FIRE: 64, ADS: 128, RELOAD: 512, SPRINT: 1024 };

function decodeSnapshot(buf) {
  const v = new DataView(buf);
  if (v.getUint8(0) !== 2) return null;
  let o = 1;
  const rd = {
    u8: () => v.getUint8(o++),
    u16: () => { const x = v.getUint16(o, true); o += 2; return x; },
    i16: () => { const x = v.getInt16(o, true); o += 2; return x; },
    u32: () => { const x = v.getUint32(o, true); o += 4; return x; },
    i32: () => { const x = v.getInt32(o, true); o += 4; return x; },
    f32: () => { const x = v.getFloat32(o, true); o += 4; return x; },
  };
  const s = {
    tick: rd.u32(), ack: rd.u32(), you: rd.u8(), timeLeft: rd.f32(),
    scoreA: rd.i32(), scoreB: rd.i32(), eventBits: rd.u16(), fog: rd.f32(),
    weather: rd.u8(), round: rd.u8(),
    vel: [rd.f32(), rd.f32(), rd.f32()], onGround: rd.u8() !== 0,
    charge: rd.f32(), focus: rd.f32(), abilityCooldown: rd.f32(),
    ammo: rd.i32(), reload: rd.f32(),
    players: [],
  };
  const count = rd.u8();
  for (let i = 0; i < count; i++) {
    const slot = rd.u8();
    const flags = rd.u8();
    s.players.push({
      slot,
      alive: (flags & 1) !== 0,
      team: (flags & 16) !== 0 ? 1 : 0,
      x: rd.f32(), y: rd.f32(), z: rd.f32(),
      yaw: rd.i16() / 10000, pitch: rd.i16() / 10000,
      health: rd.u8(), armor: rd.u8(), weapon: rd.u8(), score: rd.i32(),
    });
  }
  return s;
}

function encodeInput(inputs, interpTicks) {
  const n = Math.min(inputs.length, 3);
  const buf = new ArrayBuffer(3 + n * 10);
  const v = new DataView(buf);
  v.setUint8(0, 1);
  v.setUint8(1, n);
  v.setUint8(2, interpTicks);
  let o = 3;
  for (let i = inputs.length - n; i < inputs.length; i++) {
    const c = inputs[i];
    v.setUint32(o, c.seq, true);
    v.setInt16(o + 4, Math.round(c.yaw * 10000), true);
    v.setInt16(o + 6, Math.round(c.pitch * 10000), true);
    v.setUint16(o + 8, c.buttons, true);
    o += 10;
  }
  return buf;
}

function run(index) {
  return new Promise((resolve) => {
    const name = `Headless${index}`;
    const ws = new WebSocket(URL);
    ws.binaryType = "arraybuffer";

    let you = -1;
    let team = 0;
    let seq = 0;
    let history = [];
    let snap = null;
    let timer = null;
    let yaw = 0;
    let pitch = 0;
    let started = false;
    const log = (...a) => { if (!QUIET) console.log(`[${name}]`, ...a); };

    ws.onopen = () => {
      ws.send(JSON.stringify({ t: "hello", name }));
      ws.send(JSON.stringify({ t: "play" }));
      log("queued");
    };

    ws.onmessage = (ev) => {
      if (typeof ev.data === "string") {
        const msg = JSON.parse(ev.data);
        if (msg.t === "start") {
          you = msg.you;
          team = msg.players[you].team;
          started = true;
          log(`match: ${msg.modeName} on ${msg.mapName} in ${msg.weatherName}` +
              ` as ${["Ward", "Vane", "Echo", "Kiln"][msg.players[you].character]}` +
              ` / ${["Sting", "Ridge", "Maul", "Arc", "Tack", "Lance"][msg.players[you].weapon]}`);
          timer = setInterval(step, TICK_MS);
        } else if (msg.t === "end") {
          clearInterval(timer);
          const row = msg.table.find((r) => r.slot === msg.you);
          log(
            `result ${msg.scoreA}-${msg.scoreB} · score ${row.score}` +
            ` · ${row.kills}K/${row.deaths}D · ${row.headshotKills} headshot kills` +
            ` · ${Math.round(row.accuracy * 100)}% accuracy · AR ${Math.round(row.aimRating)}` +
            ` · worst tick ${msg.worstTickMs.toFixed(3)} ms`,
          );
          ws.close();
          resolve({ name, row, worstTickMs: msg.worstTickMs, scoreA: msg.scoreA, scoreB: msg.scoreB });
        }
        return;
      }
      const s = decodeSnapshot(ev.data);
      if (s) {
        snap = s;
        history = history.filter((h) => h.seq > s.ack);
      }
    };

    ws.onclose = () => { clearInterval(timer); resolve({ name, row: null }); };
    ws.onerror = () => {};

    function step() {
      if (!started || !snap) return;
      const me = snap.players.find((p) => p.slot === you);
      let buttons = 0;

      if (me && me.alive) {
        const enemies = snap.players.filter((p) => p.team !== team && p.alive && p.slot !== you);
        if (enemies.length) {
          let best = enemies[0];
          let bd = Infinity;
          for (const e of enemies) {
            const d = Math.hypot(e.x - me.x, e.y - me.y, e.z - me.z);
            if (d < bd) { bd = d; best = e; }
          }
          // Aim at the head, with an error floor that scales with skill.
          const jitter = (1 - SKILL) * 0.09;
          const dx = best.x - me.x + (Math.random() - 0.5) * jitter;
          const dy = best.y + 1.62 - (me.y + 1.62) + (Math.random() - 0.5) * jitter;
          const dz = best.z - me.z + (Math.random() - 0.5) * jitter;
          yaw = Math.atan2(dx, dz);
          pitch = Math.asin(dy / Math.hypot(dx, dy, dz));
          buttons |= BTN.FIRE;
          if (bd > 24) buttons |= BTN.ADS;
          if (bd > 14) buttons |= BTN.FWD;
        } else {
          // Nothing in sight: walk toward the middle of the map.
          yaw += 0.01;
          buttons |= BTN.FWD | BTN.SPRINT;
        }
        if (snap.ammo <= 0) buttons |= BTN.RELOAD;
      }

      const cmd = { seq: ++seq, yaw, pitch, buttons };
      history.push(cmd);
      if (history.length > 64) history.shift();
      if (ws.readyState === WebSocket.OPEN) ws.send(encodeInput(history, 6));
    }
  });
}

const results = await Promise.all(
  Array.from({ length: CLIENTS }, (_, i) => run(i + 1)),
);

const played = results.filter((r) => r.row);
if (played.length === 0) {
  console.error("no client completed a match");
  process.exit(1);
}
const worst = Math.max(...played.map((r) => r.worstTickMs ?? 0));
const shots = played.reduce((a, r) => a + (r.row.accuracy > 0 ? 1 : 0), 0);
console.log(
  `\n${played.length}/${CLIENTS} clients finished · worst server tick ${worst.toFixed(3)} ms` +
  ` · ${shots}/${played.length} landed at least one shot`,
);
process.exit(played.some((r) => r.row.kills > 0 || r.row.accuracy > 0) ? 0 : 2);
