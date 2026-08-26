# TICK, explained simply

TICK is a fast shooting game you play in your browser with 7 other players
(real people or bots), 4 versus 4. A whole match lasts about four minutes.
The big idea: **aiming well is worth more than shooting a lot.** A careful
headshot from far away scores about three times more than a plain kill.

## How a match goes

1. Click **Play**. If nobody else is waiting, bots fill the empty seats after
   six seconds.
2. The server picks *everything* for you: your character, your weapon, the
   map, the weather, and the game mode. You see it all on a card for two
   seconds. Nobody gets to pick a "better" loadout — both teams always get
   the same set.
3. The match starts. Shoot enemies (orange), protect teammates (teal), and do
   whatever the mode asks.
4. When you die, the screen freezes behind a menu. While you wait to respawn
   you may pick a different weapon with the number keys (see below).
5. When time runs out or a team hits the score target, you get a results
   screen with your score and an **Aim Rating** (0–100) that tells you how
   well you aimed compared to your own last 20 matches.

## Controls

| Key | What it does |
|---|---|
| W A S D | Move |
| Mouse | Look; left click shoots, right click aims down sights |
| Shift | Sprint (only forward, and you can't shoot while sprinting) |
| Space | Jump |
| Ctrl or C | Crouch |
| R | Reload |
| Q or F | Use your character's power |
| V | Melee (knife) |
| 1 2 3 4 | While dead: choose your respawn weapon |
| Esc Esc | Step away (a bot plays for you until you come back) |

Everyone has **100 health**. Run out and you die.

## Choosing a weapon when you die

Only the dead get to shop. While waiting to respawn, press:

- **1 — Sting** (assault rifle)
- **2 — Ridge** (sniper, opens a real zoomed scope when you aim)
- **3 — Tack** (small pistol)
- **4 — Blade** (knife only — left click swings it)

Your pick appears when you respawn, with full ammo.

## The weapons and their damage

| Weapon | What it is | Body hit | Head hit | Shots in mag | Notes |
|---|---|---|---|---|---|
| Sting | Fast assault rifle | 14 (drops to 9 past 18 m) | 25 | 30 | Steady and forgiving |
| Ridge | Sniper | 45 | **100 — one-shot kill** | 12 | Zoomed scope, slow fire |
| Maul | Shotgun | 12 × 9 pellets | 18 × pellets | 6 | Deadly inside 6 m, useless past 12 m |
| Arc | Burst rifle | 26 per bullet | 44 | 24 | Fires 3-bullet bursts that travel and pierce thin walls at half damage |
| Tack | Pistol | 30 | 65 | 8 | Two headshots kill |
| Lance | Prize rifle | 55 | 110 | 5 | Only appears from an Airdrop event |
| Blade | Knife | — | — | — | A clean front hit **kills instantly**; so does any knife from behind |

Everyone also has a melee attack (V): 55 damage from the front, instant kill
from behind.

## The characters and their powers

Each player is given one of four characters. Each has a passive (always on)
and an active power (Q/F, with a cooldown).

- **Ward** — the tank. Passive: 25 armor that soaks body shots (headshots go
  straight through it; armor regrows 5 s after you stop being hit). Power —
  **Shimmer** (18 s): drops a glowing wall that blocks enemy bullets but not
  yours.
- **Vane** — the runner. Passive — **Softstep**: moves 10% faster and reloads
  15% faster. Power — **Blink** (10 s): instantly dash 12 m forward.
- **Echo** — the scout. Power — **Pulse** (22 s): scans 22 m around you and
  outlines enemies through walls for a moment. Fair warning: anyone you scan
  is told they were scanned.
- **Kiln** — the arsonist. Passive — **Fireproof**: immune to fire. Power —
  **Cinderline** (20 s): lays a line of fire that burns anyone standing in it
  (12 damage per second — it hurts, but it can't finish a healthy player by
  itself).

## The four game modes

- **Skirmish** (4 min) — score points, first team to 3000 wins. Kills give
  points, but *good* kills give far more: a plain body kill is 100, a clean
  long-range headshot can be 325. At 2:30 a "Bounty" makes the best enemy
  worth extra.
- **Headhunt** (3.5 min) — first team to 30 finishing blows. Body shots can't
  kill here: they knock a player down to 1 health and stagger them (slow and
  unable to shoot). Only a **headshot or a melee** finishes. First to 30
  finishes wins.
- **Uplink** (5 min) — one glowing core sits on the map. Pick it up (you'll
  be slower and visible to everyone through walls) and carry it to the
  terminal to "bank" it. First team to 4 banks wins. The terminal moves after
  every bank.
- **Last Light** (5 min) — rounds with **one life each**. A wall of fog slowly
  shrinks the map. Last team standing takes the round; first to 4 rounds
  wins. When you're dead you can click once per round to "ghost ping" —
  briefly revealing the nearest enemy to your team. In round four, the losing
  team gets one player back ("Second Wind").

## Weather

The server also picks the weather, and it really matters:

- **Clear** — you can see all the way across the map. Snipers love it.
- **Rain** — you can't see much past 45 m. Mid-range fights.
- **Night** — about 30 m of visibility. Ambushes beat aim, and every muzzle
  flash is a beacon.

## Random events

A few times per match, something happens to shake things up. You always get a
**five-second warning** before it lands, and the schedule was decided at
match start — the server never invents one to rescue a losing team. The
events:

- **Blackout** — lights out, everyone gets green night vision (20 s).
- **Gravity Dip** — gravity drops to 40%; huge floaty jumps (25 s).
- **Golden Clip** — one player (on the losing side) gets one-shot kills for
  15 s, but is marked for everyone to see.
- **Weapon Roulette** — everyone's weapon is swapped at random.
- **Silence** — all sound is gone for 15 s. Watch your back.
- **Weather Turn** — the weather changes mid-match.
- **Airdrop** — a Lance (the best gun) drops somewhere; go take it.
- **The Mark** — the top player glows and is worth triple points for 30 s.
- **Hard Light** — for 20 s bullets pass through cover.
- **Overtime Coin** — the final 30 seconds of every match: everything scores
  double.

## The precision economy (how scoring works)

Kills earn bonus labels on top of the base points:

- **Clean** — you didn't miss a shot in the fight.
- **Surgical** — headshots only.
- **Longshot** — from far away.
- **Blindside** — they never saw you.
- **Duel** — you won a fair face-to-face fight.
- **Rescue** — you saved a teammate who was being attacked.
- **First Blood** — first kill of the match.

Landing headshots also fills your **Precision Charge**. When it's full, hold
aim and press your power key to spend it on **Focus**: two seconds of
extra-steady aim.

## Small print worth knowing

- Damage you deal pops up as floating numbers — gold and bigger for
  headshots.
- The server never sends you the position of an enemy you couldn't see, so
  wallhack cheats have nothing to read.
- Leaving, tabbing out, or pressing Escape twice hands your character to a
  bot and keeps your seat — your team never plays a player short.
- Your team is always teal; enemies are always orange. That orange appears
  nowhere else in the world, so if you see it, shoot it.
