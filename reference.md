# TICK — component reference

A map from "the thing you see in the game" to "the code that makes it happen".
The spec it is built against is `design.md`; what is and is not built yet is in
`README.md`.
Symbol names are the durable anchor; line numbers are a hint and drift as the
code changes.

**This file is updated whenever a change lands.** If something here is wrong,
that is a bug in this file.

---

## Where the authority lives

| Concern | Owner |
|---|---|
| Rules, physics, damage, scoring | `crates/tick-sim` — the only place they exist |
| Matchmaking, draft, per-match loop, visibility | `crates/tick-server` |
| Prediction, rendering, HUD, audio, input | `client/src` |
| Client's copy of the simulation | `crates/tick-sim-wasm` → `client/public/tick_sim.wasm` |

The client never decides an outcome. It sends aim direction and button state;
everything else it draws came from a server snapshot.

---

## Balance numbers — one file

Everything tunable is in **`crates/tick-sim/src/defs.rs`**:

| What | Symbol |
|---|---|
| Tick rate, player size, hitbox split | consts at top (`TICK_HZ`, `PLAYER_HEIGHT`, `HEAD_BOTTOM`) |
| Movement feel | `WALK_SPEED`…`STEP_HEIGHT`, `SPRINT_FIRE_DELAY` |
| Weapon damage / fire rate / falloff | `Weapon::stats` → `WeaponStats` |
| Character passives and cooldowns | `Character::armor`, `speed_mult`, `reload_mult`, `ability_cooldown` |
| Mode length and respawn delay | `Mode::duration`, `Mode::respawn_delay` |
| Weather sight range and audio masking | `Weather::sight_range`, `audio_mult` |
| Map geometry, spawns, Uplink terminals | `load_map` (one arm per map), helpers `solid` / `thin` / `glass` / `shell` |

Scoring bonuses and the Aim Rating formula are the exception — they live in
`crates/tick-sim/src/lib.rs` (`World::kill`, `PlayerStats::aim_rating`).

---

## Modes

All four share one `World`; mode-specific behaviour branches off `cfg_mode`.

| Mode | Where |
|---|---|
| Shared: clock, match end | `World::check_end` — the clock is the only thing that ends a match; score decides the winner, never the length. Length is `Mode::duration` |
| **Skirmish** — points not kills | `World::kill` (points path), `World::team_precision_mult` (headshot streak) |
| Skirmish Bounty at 2:30 | `World::step_schedule` (fires `StaticEvent::TheMark`) |
| **Headhunt** — body damage staggers | `World::apply_damage` (the `Mode::Headhunt` branch), stagger recovery in `World::step_players` |
| Melee execution | `World::resolve_melee` |
| **Uplink** — core, carrier, banking | `World::step_uplink`; carrier penalties in `Player::speed_mult`; core-crack bonus in `World::kill` |
| **Last Light** — rounds, fog wall | `World::step_last_light` |
| Ghost pings | `World::ghost_ping`, routed via `ToMatch::GhostPing` (`game.rs`), sent by client on click-while-dead (`main.ts`) |
| Killcam (watch your killer, then *their* killer) | `Player::killed_by` and `World::spectate_target` (`lib.rs`); visibility resolved from the spectated eye in `compute_visibility` (`game.rs`); camera in `viewEye` (`main.ts`), honoured by both the match and standby render paths in `frame` |
| Death and respawn state | `onSnapshot` in `main.ts` — the snapshot's `alive` flag drives `enterDeath` / `leaveDeath`, in Standby as well as in the match, because a player who steps away is still a body on the server |
| Diagnosing the killcam | `window.tick.state()` — `spectating` is the slot the server sent, `spectated` is the one the client resolved, `camera` is where the view actually is |
| Twin Core / Pinhead | `World::start_event` and `World::apply_damage`; drawn only in their own mode via `StaticEvent::mode_lock` |
| Second Wind (round 4) | inside `World::step_last_light` |

---

## Weapons

| Piece | Where |
|---|---|
| Stats table | `Weapon::stats` (`defs.rs`) |
| Range falloff curve | `WeaponStats::falloff` |
| Firing, cooldowns, bursts, reloads | `World::step_weapon` |
| One shot leaving the barrel | `World::fire_once` (spread cone, ADS/Focus tightening) |
| Hitscan resolution + lag compensation | `World::resolve_hitscan` |
| Arc's travelling projectiles + cover penetration | `World::step_projectiles` |
| Viewmodel shapes | `Renderer.setViewmodel` (`client/src/render.ts`) |
| Gunshot sound per weapon | `WEAPON_TONE` in `client/src/audio.ts`; layered in `Audio.shot`. `thudTop` / `thudFloor` / `muffle` / `soft` on a tone shape it into a deep thud rather than a bright report — Ridge uses all four |
| Melee swing whoosh | `Audio.swing`, fired on the melee key's rising edge in `main.ts` |

---

## Characters

| Piece | Where |
|---|---|
| Passives (armour, speed, reload) | `Character::*` in `defs.rs`; armour applied in `World::apply_damage` |
| All four actives | `World::step_ability` — Ward's Shimmer, Vane's Blink, Echo's Pulse, Kiln's Cinderline |
| Shimmer blocking bullets | `World::resolve_hitscan` (shimmer ray test) |
| Cinderline burn tick | `World::step_entities` |
| Pulse reveals | `World::step_ability` writes `World::revealed_until`; read by `compute_visibility` (`game.rs`) |
| Ability visuals | `Renderer.syncProps` |
| Cooldown readout | `Hud.setKit` |
| Per-character look (Ward's plate, Vane's hood, Echo's antenna, Kiln's pauldrons) | `Renderer.makeAvatar` |
| Walk cycle | `Renderer.syncPlayers` — limb pivots driven by distance moved |

---

## Maps and weather

| Piece | Where |
|---|---|
| Geometry, spawns, terminals | `load_map` in `defs.rs` |
| Breakable glass | `World::break_glass`, called from `resolve_hitscan` and `step_projectiles`; `SimEvent::GlassBroken` → wasm `break_glass` → `Renderer.syncGeometry` |
| Spawn safety (never inside a wall) | `free_spot` / `spot_blocked` in `lib.rs`, plus `World::best_spawn` |
| Client geometry (same brushes) | `Sim.loadMap` (`client/src/sim.ts`) → `Renderer.buildMap` |
| Who can see whom (snapshot culling) | `compute_visibility` in `game.rs`, using `movement::trace_sight` — only **solid** geometry hides a player, because thin cover passes bullets |
| Weather gameplay effect | `Weather::sight_range` — feeds bot perception (`bot.rs`) and snapshot culling (`game.rs`) |
| Weather look (fog, light, rain, cloud deck) | `LOOKS` table + `Renderer.applyLook` / `buildVista` (`render.ts`) |
| Surface textures (concrete, wood, ground, rock, metal) | `surface` in `render.ts` — drawn to a canvas at runtime, no assets |
| Texture tiling by world size | `tileBox` — rewrites a box's UVs so a metre is a metre |
| Grass, pebbles, worn paths on the floor | `Renderer.buildGroundDetail` |
| Wall skirting and pilasters | `Renderer.buildWallDetail` |
| Forest, boulders and the far skyline past the walls | `Renderer.buildScenery` |

---

## Static Events (the surprise layer)

| Piece | Where |
|---|---|
| The event list, durations, blurbs | `StaticEvent` in `lib.rs` (twelve, two mode-locked) |
| Schedule generation and its rules | `build_schedule` (seeded at match start, 60 s floor, 45 s gap, coin last) |
| Telegraph → fire → expire | `World::step_schedule` |
| Per-event effects | `World::start_event` |
| Gravity Dip / Overtime multipliers | `World::gravity_mult`, `World::score_mult` |
| Hard Light / Blackout reads | `World::resolve_hitscan`, `compute_visibility` |
| Banner and audio sting | `Hud.telegraph` / `Hud.eventStart`, `Audio.telegraph` |

---

## Precision economy

| Piece | Where |
|---|---|
| Bonus table (Clean, Surgical, Longshot, Blindside, Duel, Rescue, First Blood) | `World::kill` |
| Per-engagement accuracy tracking (Surgical) | `Player::eng_shots` / `eng_hits`, reset in `World::step_players` |
| Precision Charge accrual | `World::fire_once` (on head hits) |
| Focus spend | `World::step_weapon` (ADS + ability) |
| Aim Rating | `PlayerStats::aim_rating` |
| Results screen and rolling AR history | `showResults` in `client/src/main.ts` (`localStorage` key `tick.ar`) |

---

## Netcode

| Piece | Where |
|---|---|
| Tick loop, snapshot cadence | `run` in `crates/tick-server/src/game.rs` (`SNAPSHOT_EVERY`) |
| Input buffering and starvation handling | same loop, `Seatled::pending` |
| Lag compensation rewind | `World::record_history`, `World::rewound`, `MAX_REWIND_TICKS` |
| Client prediction step | `stepLocal` in `client/src/main.ts` → `Sim.step` → wasm `step_movement` |
| Reconciliation and error smoothing | `reconcile` in `main.ts` |
| Entity interpolation | `renderPlayers` in `main.ts`, width from `interpDelayMs` |
| RTT measurement | `Net.ping` (`client/src/net.ts`), served by the `"ping"` arm in `main.rs` |
| Wire layouts | `crates/tick-server/src/proto.rs` ↔ `client/src/proto.ts` — **change both** |

---

## Server lifecycle

| Piece | Where |
|---|---|
| HTTP, static files, `/ws` upgrade, `/health` | `crates/tick-server/src/main.rs` |
| Listen port (`TICK_PORT`, then `PORT`, then 8080) | bottom of `main` in `main.rs` |
| Per-connection message handling | `connection` in `main.rs` |
| Queue and bot fill (`BOT_FILL_SECONDS`) | `matchmaker` in `main.rs` |
| **The Server Draft** (team, character, weapon, map, weather, seed) | `start_match` in `main.rs` |
| Match task | `spawn_match` / `run` in `game.rs` |
| Visibility culling (anti-cheat) | `compute_visibility` in `game.rs` |
| Bot AI | `crates/tick-server/src/bot.rs` (`Bot::think`) |
| Standby / leave → bot takeover | `ToMatch::Standby`, `ToMatch::Leave` in `game.rs` |
| Requeue after a match (link release) | end of `run` in `game.rs` — clears each `Seat::link` |

---

## Client shell

| Piece | Where |
|---|---|
| Phase machine (lobby → queued → match → results → standby) | `setPhase` in `main.ts` |
| Spawn card | `showSpawnCard` in `main.ts`, markup in `client/index.html` |
| Waiting feedback (queue spinner and clock, respawn countdown) | `#queueWait` / `#respawnWait` in `index.html`, driven in `frame` (`main.ts`) |
| Killer card on the death screen | `showKillerCard` (`main.ts`) + `Renderer.portraitFrame` — its own small WebGL context, because the main canvas is frozen |
| Callsign assignment | `random_callsign` in `main.rs` — dictionary pair plus a four-digit connection tag |
| Aim dot while ADS | `#crosshair.ads` in `style.css`, toggled in `frame` (`main.ts`) |
| Ridge's dashed black-and-white hipfire reticle | `#crosshair.sniper` in `style.css`, toggled in `frame` (`main.ts`) |
| Scope reticle (dashed lines, red centre dot) | `.scopeLine` / `.scopeDot` in `style.css`, markup in `client/index.html` |
| Standby (`Esc Esc`, tab hidden, lost pointer lock) | `enterStandby` / `leaveStandby` in `main.ts` |
| Pointer lock, key mapping, sensitivity | `client/src/input.ts` (`KEY_BUTTONS`) |
| HUD elements | `client/src/hud.ts` + `client/index.html` + `client/src/style.css` |
| Kill feed, bonus popups, warnings | `Hud.killRow`, `Hud.bonusPopup`, `Hud.warn` |
| Positional audio | `client/src/audio.ts` (HRTF panner, synthesised — no samples) |
| Debug hook (`window.tick`) | bottom of `main.ts` — `state()`, `players()`, `aimAt()`, `press()`, `release()` |

---

## Build and tooling

| Piece | Where |
|---|---|
| Build everything and serve | `run.sh` |
| Preview config used by the harness | `/Users/sambhavsaxena/code/.claude/launch.json` (entry `tick`) |
| Simulation tests | `crates/tick-sim/src/lib.rs`, `mod tests` at the bottom |
| End-to-end / load check | `tools/headless-client.mjs` |
| Static file resolution | `resolve_static_dir` in `main.rs` |

---

## Known gaps

Tracked here so they are not re-reported as bugs. Full wording in `README.md`.

- WebTransport / HTTP-3 (WebSocket only, by decision — see `README.md`)
- Vault's floodlight cycle (Terrace breakable glass is implemented)
- Cosmetics and accounts

Out of scope by decision, not backlog: replay capture, the daily Precision
Report, free-fly ghost spectating (the killcam replaces it), skill-based
matchmaking, and moving level geometry — brushes that slid on a schedule were
removed because prediction, spawn safety and sight traces all had to agree on
where a brush was *right now*, and the cost of that agreement outweighed what
the movement added.
