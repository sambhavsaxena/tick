# TICK — a four-minute browser FPS

**Working title:** TICK
**Format:** browser-only, no install, no download gate, guest play in one click
**Match length:** 3:30 – 5:00 hard cap
**Team size:** 4v4
**Pitch:** You have the length of a coffee refill or a CI build. You click Play, the server hands you a character, a gun, a map and the weather, and thirty seconds later you are in a gunfight that scores you on how precisely you shoot rather than how long you have been grinding. Four minutes later you get one number, one highlight clip, and one button that says *again*.

---

## 1. Design pillars

1. **Zero friction to the first bullet.** Play button to first shot fired in under 20 seconds, cold cache included. Every menu that stands between a bored person and a gunfight is a menu that loses them.
2. **The server decides everything.** Character, weapon, team, map, weather, and event schedule are all server-assigned. No loadouts, no pre-match lobby screens, no meta anxiety, no pay-to-win surface. You adapt; you do not prepare.
3. **Precision is the currency.** Kills are the floor. Headshots, clean one-shots, no-miss engagements and long-range hits are where the score actually comes from. Two players can finish with identical kill counts and very different scores.
4. **Surprise on a leash.** Every match rolls unpredictable global events, but every event is either symmetric across both teams or tilted toward whoever is losing. Chaos should generate stories, never unearned kills.
5. **Absence is not punished.** Cosmetic-only progression. Skipping four days costs you nothing but practice. This is a game for people with jobs.
6. **It runs on a work laptop.** 60 fps on five-year-old integrated graphics, under 8 MB of initial download, quiet audio defaults, and a one-key standby screen.

---

## 2. Session shape

The entire product is a loop measured in seconds:

| Step | Target time | Notes |
|---|---|---|
| Page load to interactive lobby | < 3.0 s | Guest identity issued instantly; login optional and deferred |
| Click **Play** to match found | < 8 s median | Skill bracket widens every 2 s; bots backfill after 12 s |
| Match found to spawn | < 4 s | Assets already warm; map streamed during queue |
| Match | 3:30 – 5:00 | Hard cap, no overtime longer than 45 s |
| Results screen | 12 s | One number, one clip, one button |
| Auto-requeue | 8 s countdown | Cancellable; defaults to *yes* |

**Total commitment: about five and a half minutes.** That number is the product. Every feature request gets measured against it.

### Leaving is a first-class action
Quitting mid-match instantly spawns a bot with the departing player's exact state. There is no leaver penalty for the first three abandons per day, and none at all if your team is more than 40% ahead or behind. People will be pulled into meetings. Design for it instead of taxing it.

### Standby
Pressing `Esc` twice replaces the canvas with a plain grey panel reading "Rebuilding project…", mutes audio, and drops render to 1 fps while holding the connection. After 12 seconds of standby a bot takes over your character; you can reclaim it at any point until the match ends. This is half a joke and half the single most-requested feature of any game played at a desk.

---

## 3. The Server Draft

When a match is allocated, the server rolls, seeds, and locks:

- **Team assignment** — balanced by hidden skill rating, then by recent Aim Rating.
- **Character** — each team gets four distinct characters; no duplicates within a team, mirrored composition across teams for the first 200 hours of the game's life (guaranteed symmetry while balance data is thin).
- **Primary weapon** — drawn so that each team has coverage across close, mid and long range. You may pick up a fallen enemy's weapon; you may never choose your own at spawn.
- **Map and weather** — weighted random, biased against whatever you played in your last two matches.
- **Event schedule** — the match's random events and their trigger times, seeded and committed at match start so the server never "reacts" to a losing team in a way that feels rigged.

Everything is announced in a two-second spawn card: *Vane · Ridge · Depot · Night Rain*. Then the gate opens.

**Why this matters:** it removes the three biggest onboarding cliffs in shooters at once — loadout paralysis, unlock disparity, and the sense that veterans arrived with better tools. It also makes every match a fresh constraint puzzle, which is exactly the texture that survives being played twice a day for months.

---

## 4. Modes

Four modes, each with a distinct emotional shape. The mode is announced at match start; you cannot queue for a specific one (queue fragmentation is death for a game with short sessions), but each mode has a fixed slot weight and you will never receive the same mode three times running.

### 4.1 SKIRMISH — the baseline
*4v4 · 4:00 · respawn 3 s · first team to 3000 points*

Straight team combat, but the win condition is **points, not kills**. A player with six clean headshot kills will out-earn a player with ten sprayed body kills. This single change reframes the whole game on the first match a new player plays: they see the scoreboard and immediately understand that *where* you hit is the game.

- Respawns are individual, not wave-based, at the spawn point furthest from live enemies.
- A running **team precision multiplier** builds as your team lands consecutive headshots without a body-only kill in between: ×1.1 at three, ×1.25 at six, resets on any body-only kill. Teams start yelling about it within one match.
- **Mode event — Bounty:** at 2:30 the highest-scoring player on either team is marked with a visible glow and a map icon. Killing them is worth triple. Being them and surviving 30 seconds is worth 400 points. The leader instantly becomes the most interesting object on the map.

**Why it hooks:** it is the familiar mode, so nobody bounces off it — but the scoring teaches the game's actual thesis in four minutes without a tutorial.

### 4.2 HEADHUNT — the precision flex
*4v4 · 3:30 · respawn 2 s · first team to 30 heads*

Body shots cannot kill. Damage to the body drains a player to a floor of 1 HP and leaves them **staggered**: 45% movement speed, no sprint, no ability use, weapon sway doubled, and a soft red outline visible to enemies within 25 m. Only a headshot or a melee execution finishes them. Staggered players regenerate to full after 4 seconds out of combat.

- Headshot kill: 2 points. Melee execution: 1 point. Nothing else scores.
- Ridge and Arc are drafted more often here; Maul is rare.
- Staggered players crawling for a doorway while an enemy lines up a shot produces the best emergent comedy in the game, and the 4-second regen means chasing is a real decision, not a formality.
- **Mode event — Pinhead:** for 20 seconds all head hitboxes shrink by 20% and all head points double. Announced with a five-second telegraph so people can choose to disengage and wait it out, which is itself a decision.

**Why it hooks:** it is the mode people screenshot. It creates a visible, unambiguous skill ceiling, and a bad player still contributes by staggering targets for a better teammate to finish — cooperation falls out of the ruleset instead of being asked for.

### 4.3 UPLINK — the focal point
*4v4 · first to 4 banks or 5:00 · respawn 4 s*

A single neutral **core** spawns at map centre 15 seconds in. Carrying it makes you:

- visible to everyone on both teams, through walls, permanently;
- unable to sprint or aim down sights;
- worth double points to kill;
- and vulnerable to **core crack** — headshots on a carrier do 2× damage.

Bank the core at your team's uplink terminal to score. **The terminal moves after every bank**, cycling through three fixed positions per map, so the map's dominant lane changes every 40–60 seconds. Dropped cores are neutral for 3 seconds, then decay back to centre after 20.

- Banking is a 1.5-second channel — long enough for a defender to punish, short enough not to be hopeless.
- Down 0–3? The **Twin Core** event spawns a second core for 30 seconds. Two focal points means the leading team must split, which is a comeback mechanic that looks like a surprise instead of a handicap.

**Why it hooks:** there is always exactly one place worth being, so nobody hides, nobody gets lost, and every match has a clean narrative spine. It is also the mode where a weaker aimer can be the match MVP by carrying, dying loudly, and creating space.

### 4.4 LAST LIGHT — the tension mode
*4v4 · best of 7 rounds · 60 s per round · no respawn*

One life per round. No respawns, no revives. At 40 seconds a wall of luminous fog closes from the map edges toward a random interior point, forcing contact before the timer. Round is won by last team standing, or by most players alive when the clock expires.

**Ghosts.** Dead players do not sit out. They become free-flying spectators with one meaningful action each: **a single ghost ping per round**, which marks one enemy's live position for 1.5 seconds for their whole team, with a visible tell on the pinged enemy so they know they have been spotted. This turns the worst moment in round-based shooters — dying first and watching for 50 seconds — into a held card. Dead players lean in instead of alt-tabbing.

- Ghost pings cannot stack; a team may not spend more than one ping per 8 seconds.
- **Mode event — Second Wind:** in round 4 only, the first player eliminated on the trailing team respawns once, 10 seconds later, at a random edge spawn. Announced to everyone. The enemy team now has a ghost that is not a ghost walking around behind them.

**Why it hooks:** it is the mode that produces the 1v3 clutch, and the ghost-ping system means eight people are engaged for the full round instead of two.

---

## 5. Weapons

Four primaries, no attachments, no unlocks, no recoil-pattern memorisation deeper than two seconds of learning. They cover the four corners of the engagement space so that map position and range read instantly.

| | **Sting** | **Ridge** | **Maul** | **Arc** |
|---|---|---|---|---|
| Class | SMG | Semi-auto marksman | Pump shotgun | Burst carbine |
| Delivery | Hitscan | Hitscan | Hitscan pellets | **Projectile**, 180 m/s |
| Fire rate | 900 RPM | 240 RPM | 0.85 s pump | 3-round burst, 0.36 s cycle |
| Body damage | 14 (9 past 18 m) | 45 | 9 pellets × 12 | 26 |
| Head damage | 25 | **100** | pellets ×1.5 | 44 |
| Magazine | 30 | 12 | 6 | 24 |
| Reload | 1.6 s | 1.9 s | 0.45 s per shell | 2.1 s |
| Ideal range | 0–18 m | 20–60 m | 0–6 m | 8–30 m |
| Quirk | Fastest ADS, no scope | Instant kill on any head hit at any range | One-shot inside 6 m with full pellet count | Penetrates thin cover at 50% damage |

**Time to kill against a 100 HP target (no armour):**

| Weapon | All body | All head | Mixed (1 head, rest body) |
|---|---|---|---|
| Sting | 8 shots — 467 ms | 4 shots — 200 ms | 6 shots — 333 ms |
| Ridge | 3 shots — 500 ms | 1 shot — 0 ms | 2 shots — 250 ms |
| Maul | 1–2 shots — 0–850 ms | 1 shot — 0 ms | — |
| Arc | 4 hits — ~430 ms | **3 hits — 240 ms (one full burst)** | 3 hits — 360 ms |

Two deliberate consequences fall out of this table:

- **Ridge punishes body spam harder than any weapon rewards it.** Three body shots at 500 ms is the slowest kill in the game; one head shot is the fastest. Handing a mediocre player a Ridge is not a gift, it is a question.
- **Arc has travel time.** At 30 m a target is roughly 170 ms of lead. It is the only weapon where the skill is prediction rather than reaction, and it is the weapon that separates good players from great ones on the open maps.

**Universal rules:** no bullet drop. No sway while stationary. Headshot hitbox is generous and identical on every character — character choice never changes how hard you are to hit. Sprint-to-fire delay is 120 ms on all four; ADS time varies (Sting 180 ms, Arc 220 ms, Ridge 280 ms, Maul hip-fire only).

**Secondary:** everyone carries the same sidearm, the **Tack** — 8 rounds, 30 body / 65 head, fast swap (0.25 s). Two head shots kill. It exists so that running dry is a decision point, not a death sentence.

**Melee:** one universal knife. 55 damage from the front, instant kill from behind. Executions on staggered targets in Headhunt.

---

## 6. Characters

Four characters. Each has one passive and one active on a cooldown. The kits are deliberately small: **no ability may directly kill**, with one narrow exception. Guns win fights; abilities decide where fights happen.

### Ward — the anchor
- **Passive · Plating:** +25 armour. Armour absorbs body damage first and regenerates 5 s after last damage taken. **Headshots ignore armour entirely.**
- **Active · Shimmer (18 s cooldown):** deploys a 3 m × 2.5 m one-way translucent wall for 8 seconds. Ward's team shoots through it freely; enemies see a distorted blur and their bullets are stopped. Destructible with 150 damage.
- **Reads as:** the reason you should be aiming at heads. A body-spraying player takes 33% longer to kill Ward; a headshot player does not notice the armour at all.

### Vane — the tempo
- **Passive · Softstep:** silent footsteps, +15% reload speed, +10% base movement.
- **Active · Blink (10 s cooldown):** a 12 m directional dash over 0.25 s. Cannot pass through geometry, can be used mid-air, cannot be used while carrying the Uplink core.
- **Reads as:** the flanker. Punishing to play badly — Blink out of position and you die alone.

### Echo — the eyes
- **Passive · Tremor:** enemy footsteps within 15 m appear as directional ground pings, even through walls.
- **Active · Pulse (22 s cooldown):** a radial scan out to 22 m that outlines enemies through walls for 1.5 seconds. Enemies caught in it hear a distinct chime and see the outline direction, so information flows both ways.
- **Reads as:** the reason a team wins a round they should have lost. Strongest at night and in rain.

### Kiln — the zoner
- **Passive · Fireproof:** immune to own and allied fire damage.
- **Active · Cinderline (20 s cooldown):** a 9 m line of burning ground for 6 seconds. 12 damage per second, no headshot component, fully escapable in under a second, never kills a full-health player. Blocks a lane, denies a bank, breaks a push.
- **Reads as:** the only kit that can technically finish someone — deliberately weak enough that it never steals a kill, only ends a stalemate.

**Balance rule of thumb:** if a character's win-rate delta exceeds 3% across 50,000 matches, adjust cooldowns first, numbers second, and never add power to compensate for a weak kit — reduce the strong one.

---

## 7. Maps

Four maps. Every one is small enough to cross in 20–30 seconds, built on three lanes with two vertical connectors, and readable after two matches. Each contains exactly one dynamic element so the map is not memorised into staleness.

### Terrace — rooftop restaurant, evening city
Tight three-lane rooftop over a dense city block. Central glass atrium, kitchen corridor on one flank, open dining terrace on the other. Verticality via two staircases and a one-way drop from the awning to the terrace.
**Dynamic element:** the atrium glass is fully breakable. Every broken pane permanently changes sightlines and creates loud, obvious audio. By minute three the middle of the map is a different map.
**Favours:** Sting and Maul. Vane's Blink across the atrium gap is the map's signature play.

### Depot — rain-slick shipping yard
Container maze with long straight lanes between stacks, a raised gantry, and a flooded loading pit that slows movement and splashes audibly.
**Dynamic element:** every 60 seconds a crane lifts and relocates two containers, opening one lane and sealing another. Telegraphed 4 seconds ahead by a horn. Rotations are seeded per match, so nobody can pre-learn the order.
**Favours:** Ridge down the long lanes, Arc through thin container walls (its cover penetration is genuinely map-defining here).

### Vault — subterranean archive, always interior
Symmetric indoors. A wide central hall with catwalks above, flanked by two tight record-stack corridors. One long clean sightline runs the full length of the hall.
**Dynamic element:** floodlights on a 45-second cycle, alternating between the hall and the corridors. Whichever half is lit is the half where Ridge is deadly and Echo is unnecessary.
**Favours:** Maul in the stacks, Ridge in the hall. The most competitively "pure" map.

### Substation — desert power relay, wide open
The open map. Long central approach flanked by transformer housings, with a raised control room overlooking everything and two tunnels that bypass the open ground entirely.
**Dynamic element:** the control room's shutters open and close on a 30-second cycle. When open it dominates the map; when closed the tunnels are the only sane route.
**Favours:** Ridge and Arc. Kiln's Cinderline across a tunnel mouth is the standard defensive play.

**Map budget:** every map must be under 4 MB of streamed assets, use fewer than 12 material variants, and hold 60 fps on Intel Iris integrated graphics at 1080p with dynamic resolution allowed to drop to 900p.

---

## 8. Weather

Three conditions, server-assigned. Each changes gameplay measurably, not just the skybox — a weather condition that is only a colour grade is wasted disk space.

### Clear (day)
Baseline. Full sightlines. Harsh directional shadows that reveal players around corners. Ridge scope produces a visible glint at over 40 m, so long-range holders can be spotted. Nothing is masked; audio is clean at full range.

### Rain (overcast day)
- Ambient rain noise cuts effective footstep detection radius by roughly 30%.
- Visibility falls off past 45 m; a stationary player at range is genuinely hard to resolve.
- Wet ground: sprinting players kick up visible splash particles, so movement is quieter but more visible. Walking is genuinely stealthy for the first time.
- Muzzle flash and tracers are the clearest positional tell on the map.
- **Net effect:** rewards patience and punishes spraying. Echo's Tremor becomes the best passive in the game.

### Night
- Sight range collapses to roughly 30 m outside lit areas; the maps' practical geometry shrinks.
- Player silhouettes are lit only by map lighting, muzzle flash, and the Uplink core's glow.
- Tracers are extremely visible — the first shot you fire is a flare announcing your position.
- Ability visual effects (Shimmer, Cinderline, Pulse) all read much more strongly, so ability usage becomes a location broadcast.
- **Net effect:** the most chaotic and the most fun for weaker players, because ambush beats aim more often.

**Mid-match weather shift** is one of the available random events. Rolling from Clear into Night at minute three re-reads the entire map for both teams simultaneously.

---

## 9. The precision economy

This is the scoring system, and it is the retention mechanic. Points, not kills, decide Skirmish; and every mode feeds the same personal rating.

### Scoring events

| Event | Points |
|---|---|
| Body kill | 100 |
| Headshot kill | 175 |
| **Clean** — headshot kill with the target at full health | +50 |
| **Surgical** — kill with 100% accuracy in that engagement | +40 |
| **Longshot** — headshot over 35 m | +60 |
| **Blindside** — kill through smoke, cover penetration, or Shimmer | +50 |
| **Duel** — kill someone who is actively shooting at you and has landed a hit | +35 |
| **Rescue** — kill an enemy who has damaged a teammate in the last 2 s | +30 |
| First blood of the match | +75 |
| Assist (≥40 damage) | 45 |
| Core bank / carry / crack (Uplink) | 200 / 60 / 80 |
| Ghost ping leading to a kill (Last Light) | 50 |

A body-shot spray kill is worth 100. A clean, no-miss, long-range headshot is worth 325. That ratio is the whole design.

### Precision Charge
Each headshot fills a small meter by 25%. At full, the player may spend it on **Focus**: two seconds of 60% reduced recoil and a 15% tighter ADS zoom, with a faint golden tracer so opponents can see it happening. Precision buys more precision — never damage, never health, never speed. The reward for aiming well can be more aiming, or the loop turns into a snowball.

### Aim Rating (AR)
One number, 0–100, computed per match and shown as the single largest element on the results screen:

```
AR = 40 · headshot_rate_pct
   + 30 · accuracy_pct
   + 20 · damage_per_shot_normalised
   + 10 · time_to_kill_percentile
```

All four terms are normalised against the player's own skill bracket, not the global population, so a bracket-average player scores near 50 and improvement is always visible. Rolling AR over the last 20 matches is the number the player actually chases. It is not a rank, it cannot go down from losing, and it does not decay if you disappear for a month.

### Post-match, in twelve seconds
1. **AR, huge, centre screen**, with the delta against your 20-match average.
2. **One highlight** — your best kill of the match, auto-selected by point value, replayed for 6 seconds.
3. **One button** — *Again*, with an 8-second auto-requeue countdown.

Nothing else. No XP bar crawl, no card packs, no unlock reel. Those exist in the profile for people who want them; they are not in the path back to the next match.

---

## 10. Static Events — the surprise layer

Every match rolls a seeded schedule of **1 to 3 global events**. This is the system that makes the twelfth match of the week feel different from the eleventh.

### The rules that keep it fair
1. **Seeded at match start.** The schedule is decided before the first shot and cannot react to the score. Trailing-team-favouring events are chosen by the *schedule*, evaluated at trigger time, not invented on the fly.
2. **Symmetric or underdog-tilted.** Every event either applies identically to both teams, or advantages whoever is behind. No event ever advantages the leader.
3. **Telegraphed.** A five-second warning: a distinct audio sting, a HUD glyph, and a single line of text. Players get to reposition. A surprise you can react to is a decision; one you cannot is a dice roll.
4. **Never decisive alone.** No event grants a kill, only an opportunity. Duration 15–30 seconds.
5. **Never in the last 20 seconds**, except Overtime Coin, which is explicitly a closing event.
6. **One at a time.** No stacking, minimum 45 seconds between events.

### The event pool

| Event | Effect | Duration | Tilt |
|---|---|---|---|
| **Blackout** | All map lighting fails; everyone gets low-fidelity green night vision with a 25 m range. Weather effectively becomes Night. | 20 s | Symmetric |
| **Gravity Dip** | Gravity drops to 40%. Jumps triple in height, falls are slow, air control increases. Every map becomes vertical. | 25 s | Symmetric |
| **Golden Clip** | One random player receives a one-shot-kill magnum and is marked on everyone's map, including their own team's. Killing them is worth 3×. | 15 s or until they die | Underdog-weighted selection |
| **Weapon Roulette** | Every player's primary is swapped for a different random primary, simultaneously. Ammo counts reset. | Rest of match | Symmetric |
| **Silence** | All gunfire, footsteps and ability audio are muted for everyone. Only a low hum and your own heartbeat. Minimap pings persist. | 15 s | Symmetric |
| **Weather Turn** | The map rolls to a different weather condition, with a visible front sweeping across the level. | Rest of match | Symmetric |
| **Airdrop** | A crate lands at a marked location holding one **Lance** — a five-shot, two-body-shot-kill rifle that exists nowhere else in the game. First to reach it keeps it until death. | Until claimed and holder dies | Drop point biased toward trailing team's half |
| **The Mark** | The highest-scoring player on the field glows and is map-visible to both teams. Killing them is worth 3×; surviving 30 s as them is worth 400. | 30 s | Anti-leader by definition |
| **Overtime Coin** | Final 30 seconds. All points double. Announced with a countdown. | 30 s | Underdog by arithmetic |
| **Hard Light** | All player collision with other players is disabled and all bullets penetrate one thin wall. Angles stop existing. | 20 s | Symmetric |

Mode-specific events (Bounty, Pinhead, Twin Core, Second Wind) are drawn from a separate table and do not consume a global event slot.

**The point of all of it:** a player who has 20 matches in should still be able to say "you will not believe what happened" to a colleague. That sentence is the marketing budget.

---

## 11. Progression and retention

**Cosmetic only. Permanently.** Weapon finishes, player cards, callsigns, ghost trail colours. Nothing that touches damage, speed, hitboxes, or visibility.

- **Callsigns are earned by precision feats,** not playtime: *"Fifty clean headshots with Ridge."* *"Win a Last Light round as the last player alive with under 20 HP."* *"Land a Longshot during Blackout."* They are stated as challenges, visible in the profile, and never blocked behind a battle pass.
- **The daily Precision Report** — three numbers, delivered on first load of the day: yesterday's AR, your best single shot, and one specific thing to work on ("your first-shot accuracy after sprinting is in your bracket's bottom quartile"). Coaching, not a chore list.
- **Free replays.** Because the simulation is deterministic and shared between client and server, a replay is just an input stream plus a seed — measured in kilobytes, not megabytes. Every notable kill is automatically preserved as a shareable six-second link that renders live in the recipient's browser at full fidelity, from any camera angle. This is close to zero marginal cost and is the single strongest organic growth lever the game has.
- **Streaks are gentle.** Play any two matches in a day to keep a streak. Missing days costs cosmetic momentum only, never capability.

---

## 12. Server: language and architecture

### The choice: Rust

The authoritative simulation, the network gateway, the matchmaker and the lobby service are all written in **Rust**. Five reasons, in order of weight:

1. **No garbage collector, therefore no tick jitter.** A 64 Hz authoritative shooter has a 15.6 ms budget per tick, and what players actually feel is not average latency but the p99 spike. Rust's deterministic deallocation plus an allocation-free hot loop (pre-allocated arenas, an ECS with dense storage) keeps tick time under 1 ms at p99. A GC'd runtime — Go included, despite its excellent collector — introduces variance in exactly the tail that produces "that shot should have hit".
2. **One simulation, compiled twice.** The core sim is a `no_std`-friendly crate with fixed timestep and integer/fixed-point math where determinism matters. It compiles natively for the server and to `wasm32-unknown-unknown` for the browser. The client's prediction is not a reimplementation of the server's rules — it is *the same code*. This eliminates the entire category of desync bugs that consumes most of the schedule on a browser shooter, and it makes deterministic replays free.
3. **Density, and therefore unit economics.** A match instance is a task holding a few hundred kilobytes of state. A single 8-core server comfortably runs several hundred concurrent matches. For a free-to-play browser game with cosmetic-only revenue, cost per concurrent player is an existential number, not an optimisation.
4. **Mature QUIC in-process.** `quinn` and `wtransport` give production-grade WebTransport over HTTP/3 with unreliable datagrams — the only way to get UDP-like semantics into a browser. Running the transport in the same process as the sim, with no cross-language bridge, keeps the input-to-tick path short.
5. **Memory safety on a public UDP endpoint.** A game server parses hostile, attacker-controlled binary packets at high volume from the open internet. That is the exact threat model where C++ produces CVEs.

**What is not Rust:** the client UI shell, build tooling, and analytics. TypeScript for the browser application layer; SQL and ClickHouse for telemetry.

**Alternatives considered and rejected:**

| Option | Why not |
|---|---|
| **Go** | Genuinely good, excellent networking ergonomics, fast to write. Rejected on two counts: GC tail latency under allocation pressure in the tick loop, and a weak WebAssembly story — the shared-simulation trick, which is the largest single engineering win available here, is not practical. |
| **C++** | Comparable performance, worse safety on a hostile network boundary, and materially slower iteration for a small team. The performance delta over Rust does not exist in practice. |
| **Node / Bun / TypeScript on the server** | Attractive for code sharing, but single-threaded event loops plus GC cannot economically hold 64 Hz across many concurrent matches, and hot-path numeric code is at the mercy of JIT deoptimisation. |
| **Elixir / Erlang** | Superb for lobby, presence, chat and supervision trees. Poor fit for tight numeric simulation loops. Worth revisiting for the social layer if the game grows a persistent community; not worth a second language on day one. |

### Transport and netcode

- **Primary transport: WebTransport over HTTP/3.** Unreliable, unordered datagrams for state and input — exactly what a shooter needs. Reliable streams reserved for match setup, chat and results.
- **Fallback: WebSocket over TCP** for browsers or networks where QUIC is blocked. Head-of-line blocking is unavoidable there, so the client shows a small honest indicator and the server increases the interpolation buffer for that player.
- **Simulation: 64 Hz fixed tick.** Snapshots to clients at 32 Hz, delta-compressed against the last acknowledged snapshot, with per-client interest management.
- **Client-side prediction** of your own movement and weapon state, running the shared WASM sim; reconciliation replays unacknowledged inputs when a correction arrives.
- **Entity interpolation** of other players with a 100 ms buffer, adaptive to observed jitter.
- **Lag compensation via server-side rewind.** On a fire event the server rewinds every other player's collision state to the shooter's rendered view, clamped to 200 ms. Beyond that clamp the shot is resolved at present-time, and the client is told, so high-ping players are never invisible-advantaged.
- **Input packets are bit-packed to roughly 12 bytes** and sent at 64 Hz, each carrying the last three inputs for redundancy. Losing 5% of packets then costs nothing.
- **Hit feedback is predicted locally** (instant hitmarker and audio) and reconciled against the server's authoritative result. On a mismatch the hitmarker fades rather than being yanked away — the correction is honest but not jarring.

**Bandwidth per player:** roughly 16 KB/s down, 2.5 KB/s up. One thousand concurrent players is about 130 Mbit/s of downstream. This is not a cost centre.

### Services

```
browser client (TS + WASM sim)
        │  HTTPS            WebTransport/H3 (datagrams)
        ▼                              ▼
  ┌───────────┐  gRPC/msgpack   ┌──────────────┐
  │  Gateway  │────────────────▶│ Match Server │  (Rust, N match tasks per process)
  │  (Rust)   │                 └──────────────┘
  └─────┬─────┘                        │
        │                              │ results
   ┌────▼─────┐   ┌────────┐    ┌──────▼──────┐
   │Matchmaker│──▶│ Redis  │    │  Postgres   │  accounts, cosmetics, stats
   │  (Rust)  │   │ queue, │    └─────────────┘
   └──────────┘   │presence│    ┌─────────────┐
                  └────────┘    │ ClickHouse  │  telemetry, balance data
                                └─────────────┘
```

- **Matchmaking:** Redis sorted sets bucketed by hidden skill rating. Target queue time under 8 seconds; the acceptable rating window widens every 2 seconds and bots backfill at 12 seconds so nobody ever waits longer than that. Region is chosen by a client-side ping probe fired the moment the lobby loads, before the player has decided to play.
- **Deployment:** dedicated or bare-metal instances in four to six regions. Not serverless — the workload is long-lived stateful UDP with strict jitter requirements, which is the exact opposite of what a function platform is good at.
- **Match lifecycle:** lobby over WebSocket → matchmaker allocates a room on the least-loaded regional server → client receives a signed room token and a WebTransport URL → play → results posted → straight back to lobby with the requeue timer already running.
- **Observability:** per-tick timing histograms, per-player RTT and loss, rewind-clamp hit rate, and a balance dashboard fed by ClickHouse (weapon pick and win rates, character deltas, event outcome swings, map side-win-rate imbalance).

---

## 13. Client

- **TypeScript** application shell; **WebGPU** renderer with a **WebGL2** fallback path.
- **Shared Rust simulation compiled to WASM** for prediction and replay playback.
- **Art direction: flat-shaded low-poly with strong silhouette contrast.** Chosen for readability at low resolution and for download size, not for style points. Enemy silhouettes use a reserved colour that appears nowhere in any environment.
- **Audio: HRTF spatialisation via Web Audio.** Positional accuracy of footsteps and gunfire is a competitive necessity, not a polish item. Master volume defaults to 40% and audio never plays before the first user gesture.
- **Input: Pointer Lock with raw movement deltas**, no smoothing, no acceleration, and a sensitivity field that accepts values copied from other shooters.
- **Performance budget:**

| Constraint | Target |
|---|---|
| Initial download | < 8 MB gzipped |
| Time to interactive lobby | < 3 s on a cold cache |
| Frame rate floor | 60 fps at 1080p on Intel Iris integrated graphics |
| Dynamic resolution | Allowed to drop to 900p before dropping frames |
| Frame time budget | 16.6 ms; render decoupled from the fixed 64 Hz sim |
| Memory ceiling | < 500 MB, because this is running next to 40 browser tabs and an IDE |

---

## 14. Anti-cheat

Browser games are the easiest possible target, so the defence is architectural rather than reactive.

- **The client is never told what it cannot see.** The server runs potentially-visible-set and occlusion checks per player per snapshot and simply does not transmit the positions of enemies who could not be visible. Wallhacks have nothing to read.
- **All damage, hit detection and scoring are server-side.** The client sends intent — aim direction and fire events — never outcomes.
- **Movement validation.** Position deltas, speed and acceleration are checked against the same shared sim the client is running; any divergence beyond tolerance snaps the player back and increments a suspicion counter.
- **Aim-behaviour heuristics** — snap-to-target angular velocity distributions, reaction-time histograms, and unnatural consistency in flick error — feed a passive scoring model. Flagged accounts are shadow-queued together rather than banned instantly, so cheat developers get slow, noisy feedback.
- **Rate limiting and token binding** on every gateway endpoint; room tokens are single-use and bound to the issuing session.

---

## 15. Build phases

| Phase | Scope | Exit criterion |
|---|---|---|
| **M0 — Spike** | Rust sim crate compiling to native and WASM; WebTransport echo; one box moving on one flat plane with prediction and reconciliation. | A cube moves smoothly at 120 ms simulated RTT with 5% loss. |
| **M1 — Vertical slice** | One map (Vault), one weapon (Sting), one character (Vane), Skirmish mode, 4v4, clear weather, server-side rewind hit registration. | Eight real players in one match, hit registration feels correct in blind testing. |
| **M2 — The game** | All four weapons, all four characters, two maps, two modes, the precision economy, Aim Rating, results screen. | A play session of six consecutive matches without anyone asking what to do. |
| **M3 — Surprise and weather** | Full Static Event system, all three weather conditions, map dynamic elements. | Playtesters spontaneously recount an event to someone who was not there. |
| **M4 — Scale** | Matchmaking with skill brackets, bot backfill, multi-region deployment, anti-cheat baseline, telemetry pipeline. | 500 concurrent players, p99 tick under 1 ms, median queue under 8 s. |
| **M5 — The loop** | Replay sharing, daily Precision Report, cosmetics, callsigns, standby screen, all four modes and maps. | Day-7 retention above target with organic replay sharing measurable in acquisition. |

---

## 16. Principal risks

| Risk | Mitigation |
|---|---|
| **Browser input latency feels worse than a native shooter** | Pointer Lock raw deltas, render decoupled from sim, aggressive prediction, and an in-game latency readout that separates network RTT from render latency so problems are diagnosable rather than vague. |
| **WebTransport support gaps** (Safari in particular) | WebSocket fallback shipped from M1, not bolted on later; increased interpolation buffer and an honest connection-quality indicator for fallback players. |
| **Cheating in a fully client-visible runtime** | Server-side visibility culling as the primary defence, not obfuscation. Assume the client is fully compromised and design so it does not matter. |
| **Matchmaking cold start** | Bot backfill from day one, single global queue with no mode selection, and regional consolidation during low-population hours. |
| **Modes fragment an already small population** | Modes are never separately queueable. The server assigns; the queue stays unified. |
| **Art scope creep** | Hard asset budgets per map enforced in CI. A map that fails the budget does not merge. |
| **The four-minute promise erodes** | Match length is a tested invariant, not a preference. Any feature that adds time to the loop must remove an equivalent amount somewhere else. |

---

## 17. Open questions

1. Should Skirmish's team precision multiplier be visible to the enemy team? Visible creates pressure and comebacks; hidden reduces HUD clutter.
2. Is Weapon Roulette lasting the rest of the match too disruptive, or is that precisely why it is memorable? Needs playtesting both ways.
3. Does Last Light need a fifth and sixth map at launch, given that round-based play exposes map knowledge faster than respawn modes?
4. Should Aim Rating be visible to opponents? Almost certainly not at launch — the moment it is public it becomes a rank, and ranks are how short-session games acquire long-session anxiety.
5. How aggressive should bot backfill be during the first month, and at what point does a match with three bots stop being worth playing?
