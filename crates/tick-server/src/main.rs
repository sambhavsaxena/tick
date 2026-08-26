//! TICK server.
//!
//! Lobby, matchmaking, the Server Draft, and one task per live match. Written
//! in Rust for the reason the design document gives: a 64 Hz authoritative
//! shooter is judged on its p99 tick time, not its average, and a garbage
//! collector puts variance in exactly that tail.

mod bot;
mod game;
mod proto;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tick_sim::*;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use game::{spawn_match, Link, LinkCell, MatchSpec, Seat, ToClient, ToMatch};

/// Nobody waits longer than this. Once it expires the match starts with bots
/// in the empty seats.
const BOT_FILL_SECONDS: f32 = 6.0;

struct Waiting {
    name: String,
    out: mpsc::Sender<ToClient>,
    link: LinkCell,
    queued_at: Instant,
    id: u64,
}

struct Hub {
    queue: Mutex<Vec<Waiting>>,
    /// The last few maps and modes, so the draft can bias away from repeats.
    recent_maps: Mutex<VecDeque<MapId>>,
    recent_modes: Mutex<VecDeque<Mode>>,
    next_id: AtomicU64,
    live_matches: AtomicU64,
    rng: Mutex<Rng>,
}

const BOT_NAMES: [&str; 12] = [
    "Halberd", "Nine", "Tessellate", "Rook", "Cinder", "Verge", "Quarry", "Pike", "Auger",
    "Nimbus", "Cassette", "Fathom",
];

fn now_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED)
}

#[tokio::main]
async fn main() {
    let hub = Arc::new(Hub {
        queue: Mutex::new(Vec::new()),
        recent_maps: Mutex::new(VecDeque::new()),
        recent_modes: Mutex::new(VecDeque::new()),
        next_id: AtomicU64::new(1),
        live_matches: AtomicU64::new(0),
        rng: Mutex::new(Rng::new(now_seed())),
    });

    {
        let hub = hub.clone();
        tokio::spawn(async move { matchmaker(hub).await });
    }

    let static_dir = resolve_static_dir();
    println!("serving client from {static_dir}");
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/health", get(health))
        .fallback_service(ServeDir::new(static_dir))
        .layer(CorsLayer::permissive())
        .with_state(hub.clone());

    let port: u16 = std::env::var("TICK_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("tick-server listening on http://{addr}  (ws://{addr}/ws)");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}

async fn health(State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    let queued = hub.queue.lock().unwrap().len();
    axum::Json(json!({
        "ok": true,
        "queued": queued,
        "liveMatches": hub.live_matches.load(Ordering::Relaxed),
        "tickHz": TICK_HZ,
    }))
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| connection(socket, hub))
}

async fn connection(socket: WebSocket, hub: Arc<Hub>) {
    let id = hub.next_id.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<ToClient>(256);

    // One writer task per connection, so the simulation never blocks on a
    // slow socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let frame = match msg {
                ToClient::Json(s) => Message::Text(s.into()),
                ToClient::Bin(b) => Message::Binary(b.into()),
            };
            if sink.send(frame).await.is_err() {
                break;
            }
        }
    });

    let link: LinkCell = Arc::new(Mutex::new(None));
    let mut name = format!("Player{id}");

    let _ = out_tx
        .send(ToClient::Json(
            json!({"t":"welcome","id":id,"tickHz":TICK_HZ,"name":name}).to_string(),
        ))
        .await;

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Binary(data) => {
                let l = { link.lock().unwrap().clone() };
                if let Some(l) = l {
                    let _ = l.tx.try_send(ToMatch::Packet {
                        slot: l.slot,
                        data: data.to_vec(),
                    });
                }
            }
            Message::Text(text) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("t").and_then(|t| t.as_str()).unwrap_or("") {
                    "hello" => {
                        if let Some(n) = v.get("name").and_then(|n| n.as_str()) {
                            let trimmed = n.trim();
                            if !trimmed.is_empty() {
                                name = trimmed.chars().take(16).collect();
                            }
                        }
                    }
                    // Round-trip probe. The client turns this into the
                    // interpolation delay it reports with every input packet.
                    "ping" => {
                        let idv = v.get("id").cloned().unwrap_or(json!(0));
                        let _ = out_tx
                            .send(ToClient::Json(json!({"t":"pong","id":idv}).to_string()))
                            .await;
                    }
                    "play" => {
                        // Every lock is taken and released inside this block:
                        // a guard held across an await would make the whole
                        // connection future non-Send.
                        let position = {
                            if link.lock().unwrap().is_some() {
                                None
                            } else {
                                let mut q = hub.queue.lock().unwrap();
                                if q.iter().any(|w| w.id == id) {
                                    None
                                } else {
                                    q.push(Waiting {
                                        name: name.clone(),
                                        out: out_tx.clone(),
                                        link: link.clone(),
                                        queued_at: Instant::now(),
                                        id,
                                    });
                                    Some(q.len())
                                }
                            }
                        };
                        if let Some(position) = position {
                            let _ = out_tx
                                .send(ToClient::Json(
                                    json!({"t":"queued","position":position}).to_string(),
                                ))
                                .await;
                        }
                    }
                    "cancel" => {
                        {
                            hub.queue.lock().unwrap().retain(|w| w.id != id);
                        }
                        let _ = out_tx
                            .send(ToClient::Json(json!({"t":"cancelled"}).to_string()))
                            .await;
                    }
                    // A dead player choosing their respawn weapon.
                    "loadout" => {
                        let w = v.get("w").and_then(|w| w.as_u64()).unwrap_or(0) as u8;
                        let l = { link.lock().unwrap().clone() };
                        if let Some(l) = l {
                            let _ = l.tx.try_send(ToMatch::Loadout { slot: l.slot, weapon: w });
                        }
                    }
                    // Last Light: a dead player spends their one ping.
                    "ghostping" => {
                        let l = { link.lock().unwrap().clone() };
                        if let Some(l) = l {
                            let _ = l.tx.try_send(ToMatch::GhostPing { slot: l.slot });
                        }
                    }
                    // Standby: the player pressed Escape twice or lost focus.
                    // A bot takes their character; the socket stays open.
                    "standby" => {
                        let on = v.get("on").and_then(|b| b.as_bool()).unwrap_or(true);
                        let l = { link.lock().unwrap().clone() };
                        if let Some(l) = l {
                            let _ = l.tx.try_send(ToMatch::Standby { slot: l.slot, on });
                        }
                    }
                    "leave" => {
                        let l = { link.lock().unwrap().take() };
                        if let Some(l) = l {
                            let _ = l.tx.try_send(ToMatch::Leave { slot: l.slot });
                        }
                    }
                    _ => {}
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    hub.queue.lock().unwrap().retain(|w| w.id != id);
    let l = link.lock().unwrap().take();
    if let Some(l) = l {
        let _ = l.tx.try_send(ToMatch::Leave { slot: l.slot });
    }
    writer.abort();
}

async fn matchmaker(hub: Arc<Hub>) {
    let mut ticker = interval(Duration::from_millis(250));
    loop {
        ticker.tick().await;
        let ready = {
            let q = hub.queue.lock().unwrap();
            if q.is_empty() {
                false
            } else {
                let oldest = q
                    .iter()
                    .map(|w| w.queued_at.elapsed().as_secs_f32())
                    .fold(0.0f32, f32::max);
                q.len() >= MAX_PLAYERS || oldest >= BOT_FILL_SECONDS
            }
        };
        if !ready {
            continue;
        }
        let humans: Vec<Waiting> = {
            let mut q = hub.queue.lock().unwrap();
            let take = q.len().min(MAX_PLAYERS);
            q.drain(..take).collect()
        };
        start_match(&hub, humans);
    }
}

/// The Server Draft.
///
/// Team, character, weapon, map, weather and the event schedule are all
/// decided here and announced to the player in a two second spawn card. The
/// client never gets a say, which is the whole point: no loadout screen, no
/// unlock disparity, no pre-match friction.
fn start_match(hub: &Arc<Hub>, humans: Vec<Waiting>) {
    let mut rng = { *hub.rng.lock().unwrap() };
    let seed = rng.next_u64() ^ now_seed();
    let mut draft = Rng::new(seed);

    // Map and mode, biased away from what these players just had.
    let recent_maps = hub.recent_maps.lock().unwrap().clone();
    let mut map = MapId::from_u8(draft.next_u32(4) as u8);
    for _ in 0..6 {
        if !recent_maps.contains(&map) {
            break;
        }
        map = MapId::from_u8(draft.next_u32(4) as u8);
    }
    let recent_modes = hub.recent_modes.lock().unwrap().clone();
    let mut mode = Mode::from_u8(draft.next_u32(4) as u8);
    for _ in 0..6 {
        // Never the same mode three times running.
        let repeated = recent_modes.len() >= 2
            && recent_modes.iter().take(2).all(|m| *m == mode);
        if !repeated {
            break;
        }
        mode = Mode::from_u8(draft.next_u32(4) as u8);
    }
    let weather = Weather::from_u8(draft.next_u32(3) as u8);

    // Mirrored composition across teams: while balance data is thin, both
    // teams get the same four characters and the same four weapons, so a loss
    // is never explainable by the draft.
    let mut chars = [
        Character::Ward,
        Character::Vane,
        Character::Echo,
        Character::Kiln,
    ];
    shuffle(&mut chars, &mut draft);
    let mut guns = [Weapon::Sting, Weapon::Ridge, Weapon::Maul, Weapon::Arc];
    shuffle(&mut guns, &mut draft);

    let mut seats: Vec<Seat> = Vec::new();
    let mut teams: Vec<u8> = Vec::new();
    let mut characters: Vec<Character> = Vec::new();
    let mut weapons: Vec<Weapon> = Vec::new();
    let mut links: Vec<Option<LinkCell>> = Vec::new();

    let mut humans = humans;
    let mut used_names: Vec<&str> = Vec::new();
    // Alternate teams so a queue of friends does not stack one side.
    for slot in 0..MAX_PLAYERS {
        let team = (slot % 2) as u8;
        let index_in_team = slot / 2;
        teams.push(team);
        characters.push(chars[index_in_team]);
        weapons.push(guns[index_in_team]);
        if let Some(w) = humans.pop() {
            seats.push(Seat {
                name: w.name.clone(),
                out: Some(w.out.clone()),
                bot: false,
                link: Some(w.link.clone()),
            });
            links.push(Some(w.link.clone()));
        } else {
            // Bot callsigns are unique within a match: a kill feed with two
            // players called Auger is unreadable.
            let mut n = BOT_NAMES[draft.next_u32(BOT_NAMES.len() as u32) as usize];
            let mut guard = 0;
            while used_names.contains(&n) && guard < 32 {
                n = BOT_NAMES[draft.next_u32(BOT_NAMES.len() as u32) as usize];
                guard += 1;
            }
            used_names.push(n);
            seats.push(Seat {
                name: n.to_string(),
                out: None,
                bot: true,
                link: None,
            });
            links.push(None);
        }
    }

    let (tx, rx) = mpsc::channel::<ToMatch>(1024);
    for (slot, link) in links.iter().enumerate() {
        if let Some(l) = link {
            *l.lock().unwrap() = Some(Link {
                tx: tx.clone(),
                slot: slot as u8,
            });
        }
    }

    {
        let mut m = hub.recent_maps.lock().unwrap();
        m.push_front(map);
        m.truncate(2);
        let mut md = hub.recent_modes.lock().unwrap();
        md.push_front(mode);
        md.truncate(2);
        *hub.rng.lock().unwrap() = draft;
    }
    hub.live_matches.fetch_add(1, Ordering::Relaxed);

    println!(
        "match start: {} on {} in {} ({} human{})",
        mode.name(),
        map.name(),
        weather.name(),
        seats.iter().filter(|s| !s.bot).count(),
        if seats.iter().filter(|s| !s.bot).count() == 1 {
            ""
        } else {
            "s"
        }
    );

    spawn_match(
        MatchSpec {
            mode,
            map,
            weather,
            seed,
            seats,
            characters,
            weapons,
            teams,
        },
        rx,
    );
}

/// Find the built client. Honours `TICK_STATIC`, then the working directory,
/// then the layout relative to the binary itself, so the server runs the same
/// whether it is launched from the repository root or from `target/release`.
fn resolve_static_dir() -> String {
    if let Ok(dir) = std::env::var("TICK_STATIC") {
        return dir;
    }
    let cwd = std::path::Path::new("client/dist");
    if cwd.is_dir() {
        return "client/dist".to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
            let candidate = root.join("client/dist");
            if candidate.is_dir() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    "client/dist".to_string()
}

fn shuffle<T: Copy>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        let j = rng.next_u32(i as u32 + 1) as usize;
        items.swap(i, j);
    }
}
