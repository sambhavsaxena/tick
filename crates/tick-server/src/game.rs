//! One match: a 64 Hz authoritative loop, owned by a single task.
//!
//! Everything the match needs lives inside this task. There is no shared
//! mutable world state and no lock on the hot path — inputs arrive on a
//! channel, snapshots leave on per-client channels, and the simulation in
//! between is plain single-threaded code with no allocation in the tick.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};

use serde_json::json;
use tick_sim::*;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant, MissedTickBehavior};

use crate::bot::Bot;
use crate::proto::{decode_input, encode_snapshot};

/// Snapshots go out at 32 Hz: every second tick.
const SNAPSHOT_EVERY: u32 = 2;
/// How long an enemy stays in your snapshot after you lose sight of them.
/// Without a grace window players strobe in and out at every doorway.
const VISIBILITY_GRACE: f32 = 0.4;
/// Enemies this close are always sent — you can hear them, so hiding them
/// from the client would break positional audio, not stop a cheat.
const ALWAYS_VISIBLE_RADIUS: f32 = 9.0;

pub enum ToMatch {
    Packet { slot: u8, data: Vec<u8> },
    Leave { slot: u8 },
    /// The player stepped away. A bot drives their character until they come
    /// back, and they keep receiving snapshots the whole time.
    Standby { slot: u8, on: bool },
    /// Last Light: a dead player spending their one ping for the round.
    GhostPing { slot: u8 },
    /// A dead player picking the weapon they respawn with.
    Loadout { slot: u8, weapon: u8 },
}

#[derive(Clone)]
pub enum ToClient {
    Json(String),
    Bin(Vec<u8>),
}

/// The connection's handle on its match. Cleared when the match ends, so the
/// player can queue again.
#[derive(Clone)]
pub struct Link {
    pub tx: mpsc::Sender<ToMatch>,
    pub slot: u8,
}

pub type LinkCell = Arc<StdMutex<Option<Link>>>;

pub struct Seat {
    pub name: String,
    pub out: Option<mpsc::Sender<ToClient>>,
    pub bot: bool,
    pub link: Option<LinkCell>,
}

pub struct MatchSpec {
    pub mode: Mode,
    pub map: MapId,
    pub weather: Weather,
    pub seed: u64,
    pub seats: Vec<Seat>,
    pub characters: Vec<Character>,
    pub weapons: Vec<Weapon>,
    pub teams: Vec<u8>,
}

struct Seatled {
    out: Option<mpsc::Sender<ToClient>>,
    pending: VecDeque<Input>,
    last_seq: u32,
    /// Round-trip estimate in ticks, used to size lag compensation.
    rewind: u8,
    last_seen: [f32; MAX_PLAYERS],
    best_kill: Option<serde_json::Value>,
}

pub fn spawn_match(spec: MatchSpec, rx: mpsc::Receiver<ToMatch>) {
    tokio::spawn(async move {
        run(spec, rx).await;
    });
}

async fn run(spec: MatchSpec, mut rx: mpsc::Receiver<ToMatch>) {
    let mut world = World::new(MatchConfig {
        mode: spec.mode,
        map: spec.map,
        weather: spec.weather,
        seed: spec.seed,
    });

    let mut seats: Vec<Seatled> = Vec::new();
    let mut bots: HashMap<u8, Bot> = HashMap::new();

    for (i, seat) in spec.seats.iter().enumerate() {
        let slot = i as u8;
        let mut p = Player::new(
            slot,
            spec.teams[i],
            seat.name.clone(),
            spec.characters[i],
            spec.weapons[i],
        );
        p.bot = seat.bot;
        world.add_player(p);
        if seat.bot {
            bots.insert(slot, Bot::new(slot, 0.45, spec.seed));
        }
        seats.push(Seatled {
            out: seat.out.clone(),
            pending: VecDeque::new(),
            last_seq: 0,
            rewind: 6,
            last_seen: [-99.0; MAX_PLAYERS],
            best_kill: None,
        });
    }

    // Match start card: everything the Server Draft decided, in one message.
    let roster: Vec<serde_json::Value> = world
        .players
        .iter()
        .map(|p| {
            json!({
                "slot": p.slot, "name": p.name, "team": p.team,
                "character": p.character as u8, "weapon": p.weapon as u8, "bot": p.bot
            })
        })
        .collect();
    for (i, s) in seats.iter().enumerate() {
        let msg = json!({
            "t": "start",
            "mode": spec.mode as u8, "modeName": spec.mode.name(),
            "map": spec.map as u8, "mapName": spec.map.name(),
            "weather": spec.weather as u8, "weatherName": spec.weather.name(),
            "seed": spec.seed.to_string(),
            "you": i as u8,
            "duration": spec.mode.duration(),
            "players": roster,
        })
        .to_string();
        send(s, ToClient::Json(msg)).await;
    }

    let mut ticker = interval(Duration::from_secs_f64(1.0 / TICK_HZ as f64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let started = Instant::now();
    let mut tick_budget_worst = 0f64;

    loop {
        ticker.tick().await;

        // Drain everything the connections have sent since the last tick.
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ToMatch::Packet { slot, data } => {
                    let i = slot as usize;
                    if i >= seats.len() {
                        continue;
                    }
                    if let Some(pkt) = decode_input(&data) {
                        seats[i].rewind = pkt.interp_ticks.min(MAX_REWIND_TICKS as u8);
                        for inp in pkt.inputs {
                            if inp.seq > seats[i].last_seq
                                && !seats[i].pending.iter().any(|q| q.seq == inp.seq)
                            {
                                seats[i].pending.push_back(inp);
                            }
                        }
                        // A client that falls far behind gets caught up rather
                        // than accumulating a queue that plays back in slow
                        // motion.
                        while seats[i].pending.len() > 8 {
                            seats[i].pending.pop_front();
                        }
                    }
                }
                ToMatch::GhostPing { slot } => {
                    world.ghost_ping(slot);
                }
                ToMatch::Loadout { slot, weapon } => {
                    // Only the four self-service loadouts; the Lance stays
                    // airdrop-only and Maul/Arc stay draft-only.
                    if matches!(weapon, 0 | 1 | 4 | 6) {
                        world.set_loadout(slot, Weapon::from_u8(weapon));
                    }
                }
                ToMatch::Standby { slot, on } => {
                    let i = slot as usize;
                    if i >= seats.len() {
                        continue;
                    }
                    if on {
                        let seed = world.rng.next_u64();
                        bots.entry(slot).or_insert_with(|| Bot::new(slot, 0.4, seed));
                    } else if !spec.seats[i].bot {
                        bots.remove(&slot);
                        seats[i].pending.clear();
                    }
                }
                // A player leaving is instantly replaced by a bot holding
                // their exact state. No leaver hole, no 3v4.
                ToMatch::Leave { slot } => {
                    let i = slot as usize;
                    if i < seats.len() {
                        seats[i].out = None;
                        world.players[i].bot = true;
                        world.players[i].connected = true;
                        bots.entry(slot).or_insert_with(|| {
                            Bot::new(slot, 0.4, world.rng.next_u64())
                        });
                    }
                }
            }
        }

        let t0 = Instant::now();

        // Apply one queued input per player per tick. Repeating the last input
        // on starvation is what keeps a player moving through a dropped packet
        // instead of stuttering.
        for i in 0..seats.len() {
            let slot = i as u8;
            if let Some(bot) = bots.get_mut(&slot) {
                let inp = bot.think(&world, world.tick);
                world.set_input(slot, inp);
                world.players[i].rewind_ticks = 0;
                continue;
            }
            let input = if let Some(inp) = seats[i].pending.pop_front() {
                seats[i].last_seq = inp.seq;
                inp
            } else {
                world.players[i].last_input
            };
            world.players[i].rewind_ticks = seats[i].rewind;
            world.set_input(slot, input);
        }

        world.step();

        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        if elapsed > tick_budget_worst {
            tick_budget_worst = elapsed;
        }

        // Fan out simulation events as a single JSON message per tick.
        if !world.events.is_empty() {
            let payload = encode_events(&world);
            for (i, seat) in seats.iter_mut().enumerate() {
                for e in world.events.iter() {
                    if let SimEvent::Kill {
                        attacker, points, ..
                    } = e
                    {
                        if *attacker as usize == i {
                            let better = seat
                                .best_kill
                                .as_ref()
                                .and_then(|v| v.get("points").and_then(|p| p.as_i64()))
                                .map(|p| *points as i64 > p)
                                .unwrap_or(true);
                            if better {
                                seat.best_kill = Some(kill_json(e));
                            }
                        }
                    }
                }
                let _ = i;
            }
            let msg = ToClient::Json(payload);
            for s in seats.iter() {
                send(s, msg.clone()).await;
            }
        }

        if world.tick % SNAPSHOT_EVERY == 0 {
            let now = world.time;
            for i in 0..seats.len() {
                if seats[i].out.is_none() {
                    continue;
                }
                let visible = compute_visibility(&world, i as u8, &mut seats[i].last_seen, now);
                let buf = encode_snapshot(&world, i as u8, seats[i].last_seq, &visible);
                send(&seats[i], ToClient::Bin(buf)).await;
            }
        }

        if world.finished {
            break;
        }
    }

    // Release every connection from this match before the results go out, so
    // a player who hits Again during the results screen is queued instantly
    // rather than silently ignored.
    for seat in &spec.seats {
        if let Some(cell) = &seat.link {
            *cell.lock().unwrap() = None;
        }
    }

    // Results: one number each, plus the best kill we saw them make.
    let mut table: Vec<serde_json::Value> = world
        .players
        .iter()
        .map(|p| {
            json!({
                "slot": p.slot, "name": p.name, "team": p.team, "bot": p.bot,
                "character": p.character.name(), "weapon": p.weapon.stats().name,
                "score": p.stats.score, "kills": p.stats.kills, "deaths": p.stats.deaths,
                "assists": p.stats.assists, "headshotKills": p.stats.headshot_kills,
                "accuracy": p.stats.accuracy(), "headshotRate": p.stats.headshot_rate(),
                "damage": p.stats.damage as i32, "clean": p.stats.clean,
                "surgical": p.stats.surgical, "longshot": p.stats.longshot,
                "aimRating": p.stats.aim_rating(),
            })
        })
        .collect();
    table.sort_by_key(|v| -(v["score"].as_i64().unwrap_or(0)));

    for (i, s) in seats.iter().enumerate() {
        let msg = json!({
            "t": "end",
            "winner": world.winner,
            "you": i as u8,
            "scoreA": world.team_score[0],
            "scoreB": world.team_score[1],
            "worstTickMs": tick_budget_worst,
            "lengthSeconds": started.elapsed().as_secs_f32(),
            "bestKill": s.best_kill.clone().unwrap_or(serde_json::Value::Null),
            "table": table,
        })
        .to_string();
        send(s, ToClient::Json(msg)).await;
    }
}

async fn send(seat: &Seatled, msg: ToClient) {
    if let Some(tx) = &seat.out {
        let _ = tx.try_send(msg);
    }
}

/// Server-side visibility culling.
///
/// The recipient is told about themselves, their team, and any enemy they
/// could plausibly see or hear right now. Everything else is simply absent
/// from the packet, so a modified client has nothing to draw. This is the
/// primary anti-cheat measure, and it costs one ray per enemy per snapshot.
fn compute_visibility(
    w: &World,
    recipient: u8,
    last_seen: &mut [f32; MAX_PLAYERS],
    now: f32,
) -> [bool; MAX_PLAYERS] {
    let mut out = [false; MAX_PLAYERS];
    let me = &w.players[recipient as usize];
    let eye = me.eye();
    let sight = w.weather.sight_range();
    let blackout = w.event_active(StaticEvent::Blackout);
    let range = if blackout { sight.min(25.0) } else { sight };

    for p in &w.players {
        let i = p.slot as usize;
        if p.slot == recipient || p.team == me.team {
            out[i] = true;
            last_seen[i] = now;
            continue;
        }
        // Revealed by Echo's Pulse or by a ghost ping: through walls, at any
        // range, for as long as the reveal lasts.
        if w.revealed_until[me.team as usize][i] > w.time {
            out[i] = true;
            last_seen[i] = now;
            continue;
        }
        // Marked players and the core carrier are visible to everyone by
        // design — that is the point of both mechanics.
        if p.marked || p.carrying_core {
            out[i] = true;
            last_seen[i] = now;
            continue;
        }
        if !p.alive {
            continue;
        }
        let to = p.mv.pos.sub(me.mv.pos);
        let dist = to.len();
        if dist < ALWAYS_VISIBLE_RADIUS {
            out[i] = true;
            last_seen[i] = now;
            continue;
        }
        if dist <= range {
            // Three probes: feet, chest, head. A player leaning out of cover
            // should appear before their whole body clears it.
            let targets = [
                p.mv.pos.add(v3(0.0, 0.2, 0.0)),
                p.mv.pos.add(v3(0.0, 1.0, 0.0)),
                p.mv.pos.add(v3(0.0, 1.7, 0.0)),
            ];
            let seen = targets.iter().any(|t| {
                let d = t.sub(eye);
                let len = d.len();
                let (hit, _) = movement::trace_world(eye, d.normalized(), len, &w.map.brushes);
                hit >= len - 0.3
            });
            if seen {
                out[i] = true;
                last_seen[i] = now;
                continue;
            }
        }
        if now - last_seen[i] < VISIBILITY_GRACE {
            out[i] = true;
        }
    }
    out
}

fn kill_json(e: &SimEvent) -> serde_json::Value {
    match e {
        SimEvent::Kill {
            attacker,
            victim,
            weapon,
            headshot,
            distance,
            points,
            bonuses,
        } => json!({
            "attacker": attacker, "victim": victim, "weapon": weapon,
            "headshot": headshot, "distance": distance, "points": points,
            "bonuses": bonuses.iter().map(|b| json!({"label": b.label, "points": b.points}))
                .collect::<Vec<_>>(),
        }),
        _ => serde_json::Value::Null,
    }
}

fn encode_events(w: &World) -> String {
    let list: Vec<serde_json::Value> = w
        .events
        .iter()
        .map(|e| match e {
            SimEvent::Shot {
                slot,
                weapon,
                origin,
                end,
                hit,
                headshot,
            } => json!({"e":"shot","slot":slot,"w":weapon,
                "o":[origin.x,origin.y,origin.z],"p":[end.x,end.y,end.z],
                "hit":hit,"hs":headshot}),
            SimEvent::Damage {
                attacker,
                victim,
                amount,
                headshot,
            } => json!({"e":"dmg","a":attacker,"v":victim,"n":amount,"hs":headshot}),
            SimEvent::Kill { .. } => {
                let mut v = kill_json(e);
                v["e"] = json!("kill");
                v
            }
            SimEvent::Stagger { slot } => json!({"e":"stagger","slot":slot}),
            SimEvent::Revealed { slot, by_team } => {
                json!({"e":"revealed","slot":slot,"team":by_team})
            }
            SimEvent::GhostPing { by, target } => {
                json!({"e":"ghostPing","by":by,"target":target})
            }
            SimEvent::Spawn { slot } => json!({"e":"spawn","slot":slot}),
            SimEvent::Ability {
                slot,
                kind,
                pos,
                yaw,
            } => json!({"e":"ability","slot":slot,"k":kind,"p":[pos.x,pos.y,pos.z],"yaw":yaw}),
            SimEvent::GlassBroken { index } => json!({"e":"glass","i":index}),
            SimEvent::Telegraph { kind } => {
                let k = StaticEvent::ALL[*kind as usize];
                json!({"e":"telegraph","k":kind,"name":k.name(),"blurb":k.blurb()})
            }
            SimEvent::EventStart { kind } => {
                let k = StaticEvent::ALL[*kind as usize];
                json!({"e":"eventStart","k":kind,"name":k.name(),"blurb":k.blurb(),
                    "duration":k.duration(),"underdog":k.is_underdog_tilted()})
            }
            SimEvent::EventEnd { kind } => {
                json!({"e":"eventEnd","k":kind,"name":StaticEvent::ALL[*kind as usize].name()})
            }
            SimEvent::Bank { slot, team } => json!({"e":"bank","slot":slot,"team":team}),
            SimEvent::CoreTaken { slot } => json!({"e":"coreTaken","slot":slot}),
            SimEvent::CoreDropped { pos } => json!({"e":"coreDropped","p":[pos.x,pos.y,pos.z]}),
            SimEvent::PickupTaken { slot, weapon } => {
                json!({"e":"pickup","slot":slot,"w":weapon})
            }
            SimEvent::RoundEnd { winner } => json!({"e":"roundEnd","winner":winner}),
            SimEvent::MatchEnd { winner } => json!({"e":"matchEnd","winner":winner}),
        })
        .collect();
    json!({"t":"ev","tick":w.tick,"e":list}).to_string()
}
