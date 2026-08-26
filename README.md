# TICK

A four-minute, 4v4, browser first-person shooter. The server decides your
character, your weapon, the map, the weather and the match's random events;
you get a two-second spawn card and then a gunfight scored on precision rather
than volume.

This repository is a working vertical slice of the design in `design.md`:
an authoritative Rust server running the simulation at
64 Hz, the same simulation compiled to WebAssembly for client-side prediction,
and a WebGL client.

## Quick start

```bash
./run.sh
```

That builds the WebAssembly simulation, the client bundle and the server, then
serves the game on <http://localhost:8080>. Click **Play**; if nobody else is
queued, bots fill the remaining seats after six seconds. Set `TICK_PORT` (or
`PORT`) to serve somewhere else.

Rust and Node are the only prerequisites:

```bash
brew install rustup && rustup default stable
rustup target add wasm32-unknown-unknown
```

### Development

```bash
cargo test -p tick-sim                     # simulation rules
cargo run  -p tick-server                  # server on :8080
npm --prefix client run dev                # Vite on :5173, proxies /ws to :8080
node tools/headless-client.mjs --clients 8 # eight scripted players, one match
```

`reference.md` maps each part of the game to the code that implements it.

## Layout

```
crates/tick-sim        the simulation: movement, weapons, damage, scoring, modes
crates/tick-sim-wasm   the same crate compiled to wasm32, exposed over a C ABI
crates/tick-server     lobby, matchmaking, the Server Draft, one task per match
client/                TypeScript + three.js renderer, HUD, prediction
tools/headless-client.mjs  a scripted client that speaks the real protocol
```

## How it fits together

**One simulation, compiled twice.** `tick-sim` is a plain Rust crate with a
fixed 64 Hz timestep and no dependency on the clock, the network or the
filesystem. The server links it natively; the browser loads it as a 34 KB
`wasm32-unknown-unknown` cdylib with a raw C ABI — no wasm-bindgen, no
generated glue. Client prediction is therefore not a reimplementation of the
server's movement rules, it is the same compiled code, which removes the whole
category of prediction-versus-authority desync bugs. The renderer also reads
the map's collision brushes out of the WebAssembly module, so what you see is
literally what bullets and feet collide with.

Every transcendental in the simulation goes through `libm` rather than the
platform's, so native and wasm builds produce identical results and a match is
reproducible from its seed.

**Netcode.** 64 Hz authoritative tick, 32 Hz snapshots, client-side prediction
with replay-based reconciliation, and 100–200 ms entity interpolation sized
from measured round-trip time. Shots are resolved against rewound positions:
the client reports how far behind it is rendering with every input packet, and
the server rewinds other players by exactly that much, clamped to 200 ms.
Input packets carry the last three inputs, so a dropped packet costs nothing.

Transport is WebSocket today. The design calls for WebTransport over HTTP/3
with unreliable datagrams as the primary path and WebSocket as the fallback;
the fallback shipped first deliberately, so it is never a bolt-on.

**Anti-cheat by omission.** The server runs a visibility pass per player per
snapshot and does not transmit the position of an enemy you could not see.
A modified client has nothing to read because the data never left the server.
All damage, hit detection and scoring are server-side; the client sends aim
and fire intent, never outcomes.

**Wire format.** Control traffic (match setup, kill feed, event announcements,
results) is JSON. The 32 Hz snapshot stream and 64 Hz input stream are
hand-packed binary. Layouts live in `crates/tick-server/src/proto.rs` and
`client/src/proto.ts` — change one, change the other.

## What is implemented

- **Four modes** — Skirmish (points, not kills, plus the team precision
  multiplier and the 2:30 Bounty), Headhunt (body damage staggers to 1 HP,
  only head shots and melee finish), Uplink (one core, carrier visible through
  walls to everyone, terminal rotates after each bank), Last Light (one life
  per round, closing fog wall, one ghost ping per dead player per round,
  Second Wind in round four).
- **Four weapons** — Sting, Ridge, Maul, Arc, on the design's damage and
  time-to-kill tables, plus the universal Tack sidearm and the airdrop-only
  Lance. Arc fires travelling projectiles that penetrate thin cover at half
  damage.
- **Four characters** — Ward (armour that head shots ignore, Shimmer wall),
  Vane (Softstep, Blink), Echo (Tremor, Pulse), Kiln (Fireproof, Cinderline).
- **Four maps** — Vault, Depot, Terrace, Substation, built from collision
  brushes with thin cover and glass.
- **Three weather conditions** with real gameplay deltas: sight range feeds
  both the renderer's fog and the bots' perception, so Night genuinely blinds
  everyone.
- **A dressed world** — surface textures drawn to a canvas at load time rather
  than downloaded, so concrete, timber, rock and soil all read as materials
  without costing a byte of transfer; grass, pebbles and worn paths on the
  floor; skirting and pilasters along the arena walls; layered conifers,
  undergrowth and boulders past the boundary, and a lit skyline behind those.
  Collision is untouched: every added mesh is decoration the simulation has
  never heard of.
- **Humanoid characters** — articulated arms and legs with a walk cycle driven
  by real movement, and gear that identifies the kit on sight: Ward's helmet
  and chest plate, Vane's hood and scarf, Echo's lit visor band and antenna,
  Kiln's pauldrons and ember line. The head stays a distinct volume because it
  is a distinct hitbox.
- **The Static Event system** — ten events, seeded at match start so the
  server can never invent one in reaction to the score, five-second telegraph,
  45-second minimum spacing, symmetric or underdog-tilted, and Overtime Coin
  closing every match.
- **The precision economy** — the full bonus table (Clean, Surgical, Longshot,
  Blindside, Duel, Rescue, First Blood), Precision Charge spent on Focus, and
  Aim Rating on the results screen with a rolling delta against your last 20.
- **Server Draft, bot backfill, standby** — the server assigns everything;
  leaving, tabbing out or pressing Escape twice hands your character to a bot
  and keeps your seat.
- **Synthesised weapon audio** — every shot is built from a pressure thump, a
  pitched mechanism bark and a filtered noise crack whose filter sweeps as it
  decays; the two long rifles add delayed, muffled echoes so a Ridge shot
  sounds like it crossed the map. Melee has its own swing. Still no samples.
- **Waiting always shows progress** — the queue runs a spinner and an elapsed
  clock, and death runs a spinner and a respawn countdown, so no screen in the
  game can be mistaken for a hang.

## What is not implemented yet

Named explicitly so the gap is not mistaken for a bug:

- WebTransport / HTTP/3. WebSocket only for now.
- Deterministic replay capture and sharing, and the daily Precision Report.
- Free-fly ghost spectating in Last Light. Ghost pings work; the dead player's
  camera stays where they fell rather than flying.
- Breakable atrium glass on Terrace, and the moving containers on Depot,
  the floodlight cycle on Vault, and the shutter cycle on Substation. The
  geometry is there; the dynamic element is not yet driven. Glass currently
  behaves as thin cover: you can shoot through it, not walk through it.
- Twin Core (Uplink) and Pinhead (Headhunt) mode events.
- Cosmetics, callsigns, accounts. Play is guest-only.
- Skill-based matchmaking. The queue is first-come, with bot fill.

## Tuning

Every balance number lives in `crates/tick-sim/src/defs.rs`: weapon damage and
fire rates, movement constants, character passives, mode durations and score
targets, weather sight ranges, and the maps themselves. Scoring bonuses and the
Aim Rating formula are in `crates/tick-sim/src/lib.rs`.

## Tests

`cargo test -p tick-sim` covers the rules that the design is actually made of:
a Ridge head shot kills outright, Ward's armour soaks body damage and is
ignored by head shots, Headhunt body damage staggers rather than kills, a
pellet past its falloff is not counted as a hit, no spawn point or terminal
sits inside geometry, no two players spawn on top of each other, the event
schedule obeys its own spacing rules, and the simulation is deterministic from
its seed.

`tools/headless-client.mjs` is the end-to-end check: it speaks the real
protocol at the real cadence, so it exercises encoding, transport, lag
compensation, hit registration and scoring, and reports the server's worst
tick time for the match. Eight simultaneous clients currently peak at about
0.1 ms per tick against a 15.6 ms budget.
