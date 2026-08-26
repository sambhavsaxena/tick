//! TICK — the authoritative simulation.
//!
//! One fixed-timestep world, stepped at 64 Hz. The server owns an instance of
//! [`World`] per match; the browser owns a cut-down instance used only to
//! predict the local player. Nothing in here touches the network, the clock,
//! or the filesystem, which is what makes a match replayable from its seed.

pub mod defs;
pub mod math;
pub mod movement;

pub use defs::*;
pub use math::*;
pub use movement::{buttons, Input, MoveState};

use movement::{player_box, step_movement, trace_world};

/// How far back lag compensation is allowed to rewind. Shots from a client
/// further behind than this resolve at present time instead.
pub const MAX_REWIND_TICKS: usize = 13; // ~200 ms at 64 Hz

/// Every Static Event gets a five second warning before it lands.
pub const TELEGRAPH: f32 = 5.0;
/// Minimum spacing between Static Events. No two are ever live at once.
pub const MIN_EVENT_GAP: f32 = 45.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StaticEvent {
    Blackout = 0,
    GravityDip = 1,
    GoldenClip = 2,
    WeaponRoulette = 3,
    Silence = 4,
    WeatherTurn = 5,
    Airdrop = 6,
    TheMark = 7,
    OvertimeCoin = 8,
    HardLight = 9,
}

impl StaticEvent {
    pub fn name(self) -> &'static str {
        match self {
            StaticEvent::Blackout => "Blackout",
            StaticEvent::GravityDip => "Gravity Dip",
            StaticEvent::GoldenClip => "Golden Clip",
            StaticEvent::WeaponRoulette => "Weapon Roulette",
            StaticEvent::Silence => "Silence",
            StaticEvent::WeatherTurn => "Weather Turn",
            StaticEvent::Airdrop => "Airdrop",
            StaticEvent::TheMark => "The Mark",
            StaticEvent::OvertimeCoin => "Overtime Coin",
            StaticEvent::HardLight => "Hard Light",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            StaticEvent::Blackout => "The lights are gone. Night vision only.",
            StaticEvent::GravityDip => "Gravity at 40%. Everything is vertical now.",
            StaticEvent::GoldenClip => "One player, one-shot kills, marked for everyone.",
            StaticEvent::WeaponRoulette => "Everyone's weapon just changed.",
            StaticEvent::Silence => "All sound is gone. Watch, do not listen.",
            StaticEvent::WeatherTurn => "The weather is turning.",
            StaticEvent::Airdrop => "A Lance is on the ground. Go and take it.",
            StaticEvent::TheMark => "The leader is lit up. Triple points.",
            StaticEvent::OvertimeCoin => "Final 30 seconds. Everything scores double.",
            StaticEvent::HardLight => "Bullets pass through cover. Angles are gone.",
        }
    }
    pub fn duration(self) -> f32 {
        match self {
            StaticEvent::Blackout => 20.0,
            StaticEvent::GravityDip => 25.0,
            StaticEvent::GoldenClip => 15.0,
            StaticEvent::WeaponRoulette => 9_999.0,
            StaticEvent::Silence => 15.0,
            StaticEvent::WeatherTurn => 9_999.0,
            StaticEvent::Airdrop => 9_999.0,
            StaticEvent::TheMark => 30.0,
            StaticEvent::OvertimeCoin => 30.0,
            StaticEvent::HardLight => 20.0,
        }
    }
    /// Events that tilt toward the trailing team rather than applying evenly.
    pub fn is_underdog_tilted(self) -> bool {
        matches!(
            self,
            StaticEvent::GoldenClip
                | StaticEvent::Airdrop
                | StaticEvent::TheMark
                | StaticEvent::OvertimeCoin
        )
    }
    /// The draw pool for scheduled events. Overtime Coin is deliberately not
    /// in it: it is placed by hand at the end of every match.
    pub const POOL: [StaticEvent; 9] = [
        StaticEvent::Blackout,
        StaticEvent::GravityDip,
        StaticEvent::GoldenClip,
        StaticEvent::WeaponRoulette,
        StaticEvent::Silence,
        StaticEvent::WeatherTurn,
        StaticEvent::Airdrop,
        StaticEvent::TheMark,
        StaticEvent::HardLight,
    ];
    pub const ALL: [StaticEvent; 10] = [
        StaticEvent::Blackout,
        StaticEvent::GravityDip,
        StaticEvent::GoldenClip,
        StaticEvent::WeaponRoulette,
        StaticEvent::Silence,
        StaticEvent::WeatherTurn,
        StaticEvent::Airdrop,
        StaticEvent::TheMark,
        StaticEvent::OvertimeCoin,
        StaticEvent::HardLight,
    ];
}

#[derive(Clone, Copy, Debug)]
pub struct ScheduledEvent {
    pub kind: StaticEvent,
    /// Seconds from match start when the five second telegraph begins.
    pub telegraph_at: f32,
    pub fires_at: f32,
    pub fired: bool,
}

#[derive(Clone, Debug)]
pub struct ActiveEvent {
    pub kind: StaticEvent,
    pub ends_at: f32,
}

/// A scoring bonus attached to a kill.
#[derive(Clone, Copy, Debug)]
pub struct Bonus {
    pub label: &'static str,
    pub points: i32,
}

#[derive(Clone, Debug)]
pub enum SimEvent {
    Shot {
        slot: u8,
        weapon: u8,
        origin: Vec3,
        end: Vec3,
        hit: bool,
        headshot: bool,
    },
    Damage {
        attacker: u8,
        victim: u8,
        amount: i32,
        headshot: bool,
    },
    Kill {
        attacker: u8,
        victim: u8,
        weapon: u8,
        headshot: bool,
        distance: f32,
        points: i32,
        bonuses: Vec<Bonus>,
    },
    Stagger {
        slot: u8,
    },
    /// You were scanned. The scan is visible to its target by design, so
    /// information flows both ways.
    Revealed {
        slot: u8,
        by_team: u8,
    },
    GhostPing {
        by: u8,
        target: u8,
    },
    Spawn {
        slot: u8,
    },
    Ability {
        slot: u8,
        kind: u8,
        pos: Vec3,
        yaw: f32,
    },
    GlassBroken {
        index: u32,
    },
    Telegraph {
        kind: u8,
    },
    EventStart {
        kind: u8,
    },
    EventEnd {
        kind: u8,
    },
    Bank {
        slot: u8,
        team: u8,
    },
    CoreTaken {
        slot: u8,
    },
    CoreDropped {
        pos: Vec3,
    },
    PickupTaken {
        slot: u8,
        weapon: u8,
    },
    RoundEnd {
        winner: u8,
    },
    MatchEnd {
        winner: u8,
    },
}

#[derive(Clone, Debug, Default)]
pub struct PlayerStats {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub headshot_kills: u32,
    pub shots_fired: u32,
    pub shots_hit: u32,
    pub head_hits: u32,
    pub damage: f32,
    pub score: i32,
    pub clean: u32,
    pub surgical: u32,
    pub longshot: u32,
    /// Sum of engagement durations, used for the time-to-kill term of AR.
    pub ttk_total: f32,
    pub ttk_count: u32,
}

impl PlayerStats {
    pub fn accuracy(&self) -> f32 {
        if self.shots_fired == 0 {
            0.0
        } else {
            self.shots_hit as f32 / self.shots_fired as f32
        }
    }
    pub fn headshot_rate(&self) -> f32 {
        if self.shots_hit == 0 {
            0.0
        } else {
            self.head_hits as f32 / self.shots_hit as f32
        }
    }
    pub fn damage_per_shot(&self) -> f32 {
        if self.shots_fired == 0 {
            0.0
        } else {
            self.damage / self.shots_fired as f32
        }
    }
    /// Aim Rating, 0-100. The one number on the results screen.
    ///
    /// Each term is normalised against a bracket-average reference rather than
    /// the global population, so an average player lands near 50 and movement
    /// in the number is always legible.
    pub fn aim_rating(&self) -> f32 {
        if self.shots_fired < 5 {
            return 0.0;
        }
        let head = clamp(self.headshot_rate() / 0.30, 0.0, 2.0) * 0.5;
        let acc = clamp(self.accuracy() / 0.34, 0.0, 2.0) * 0.5;
        let dps = clamp(self.damage_per_shot() / 16.0, 0.0, 2.0) * 0.5;
        let ttk = if self.ttk_count == 0 {
            0.5
        } else {
            let avg = self.ttk_total / self.ttk_count as f32;
            clamp(1.0 - (avg - 0.35) / 1.4, 0.0, 1.0)
        };
        clamp(
            100.0 * (0.40 * head + 0.30 * acc + 0.20 * dps + 0.10 * ttk),
            0.0,
            100.0,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub slot: u8,
    pub team: u8,
    pub bot: bool,
    pub name: String,
    pub character: Character,
    pub weapon: Weapon,
    pub mv: MoveState,
    pub yaw: f32,
    pub pitch: f32,

    pub alive: bool,
    pub health: i32,
    pub armor: i32,
    pub respawn_at: f32,

    /// Headhunt: reduced to 1 HP by body damage, finishable only by a head
    /// shot or a melee execution, regenerating after four seconds clear.
    pub staggered: bool,
    pub stagger_clear_at: f32,

    pub ammo: i32,
    pub fire_cooldown: f32,
    pub reload_timer: f32,
    pub burst_left: u8,
    pub burst_timer: f32,
    pub ads: f32,
    pub sprint_lock: f32,
    pub ability_cooldown: f32,
    pub melee_cooldown: f32,

    /// Precision Charge, 0.0-1.0. Full charge can be spent on Focus.
    pub charge: f32,
    pub focus_timer: f32,

    pub carrying_core: bool,
    pub marked: bool,

    /// Rewind used for this player's shots, in ticks.
    pub rewind_ticks: u8,

    pub last_damaged_at: f32,
    pub last_damaged_by: Option<u8>,
    pub damaged_teammate_at: f32,
    /// Per-engagement accuracy, for the Surgical bonus.
    pub eng_shots: u32,
    pub eng_hits: u32,
    pub eng_started_at: f32,
    /// Recent damage dealt to each other player, for assists.
    pub dealt: [f32; MAX_PLAYERS],
    pub dealt_at: [f32; MAX_PLAYERS],

    pub stats: PlayerStats,
    pub last_input: Input,
    pub connected: bool,
}

impl Player {
    pub fn new(slot: u8, team: u8, name: String, character: Character, weapon: Weapon) -> Player {
        Player {
            slot,
            team,
            bot: false,
            name,
            character,
            weapon,
            mv: MoveState::default(),
            yaw: 0.0,
            pitch: 0.0,
            alive: false,
            health: MAX_HEALTH,
            armor: character.armor(),
            respawn_at: 0.0,
            staggered: false,
            stagger_clear_at: 0.0,
            ammo: weapon.stats().mag,
            fire_cooldown: 0.0,
            reload_timer: 0.0,
            burst_left: 0,
            burst_timer: 0.0,
            ads: 0.0,
            sprint_lock: 0.0,
            ability_cooldown: 0.0,
            melee_cooldown: 0.0,
            charge: 0.0,
            focus_timer: 0.0,
            carrying_core: false,
            marked: false,
            rewind_ticks: 6,
            last_damaged_at: -99.0,
            last_damaged_by: None,
            damaged_teammate_at: -99.0,
            eng_shots: 0,
            eng_hits: 0,
            eng_started_at: 0.0,
            dealt: [0.0; MAX_PLAYERS],
            dealt_at: [-99.0; MAX_PLAYERS],
            stats: PlayerStats::default(),
            last_input: Input::default(),
            connected: true,
        }
    }

    pub fn eye(&self) -> Vec3 {
        let h = if self.mv.crouching {
            PLAYER_CROUCH_HEIGHT - 0.18
        } else {
            EYE_HEIGHT
        };
        self.mv.pos.add(v3(0.0, h, 0.0))
    }

    pub fn head_box(&self, pos: Vec3) -> Aabb {
        let scale = if self.mv.crouching {
            PLAYER_CROUCH_HEIGHT / PLAYER_HEIGHT
        } else {
            1.0
        };
        Aabb {
            min: v3(
                pos.x - PLAYER_RADIUS * 0.62,
                pos.y + HEAD_BOTTOM * scale,
                pos.z - PLAYER_RADIUS * 0.62,
            ),
            max: v3(
                pos.x + PLAYER_RADIUS * 0.62,
                pos.y + PLAYER_HEIGHT * scale,
                pos.z + PLAYER_RADIUS * 0.62,
            ),
        }
    }

    pub fn body_box(&self, pos: Vec3) -> Aabb {
        let scale = if self.mv.crouching {
            PLAYER_CROUCH_HEIGHT / PLAYER_HEIGHT
        } else {
            1.0
        };
        Aabb {
            min: v3(pos.x - PLAYER_RADIUS, pos.y, pos.z - PLAYER_RADIUS),
            max: v3(
                pos.x + PLAYER_RADIUS,
                pos.y + HEAD_BOTTOM * scale,
                pos.z + PLAYER_RADIUS,
            ),
        }
    }

    pub fn speed_mult(&self) -> f32 {
        let mut m = self.character.speed_mult();
        if self.staggered {
            m *= 0.45;
        }
        if self.carrying_core {
            m *= 0.88;
        }
        m
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub owner: u8,
    pub team: u8,
    pub pos: Vec3,
    pub vel: Vec3,
    pub body: f32,
    pub head: f32,
    pub weapon: u8,
    pub life: f32,
    /// Set once the projectile has punched through a thin brush; damage is
    /// halved and it cannot penetrate a second time.
    pub penetrated: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Shimmer {
    pub team: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: f32,
    pub ends_at: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Cinder {
    pub owner: u8,
    pub team: u8,
    pub a: Vec3,
    pub b: Vec3,
    pub ends_at: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub pos: Vec3,
    pub weapon: Weapon,
    pub taken: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct HistoryFrame {
    pos: [Vec3; MAX_PLAYERS],
    alive: [bool; MAX_PLAYERS],
    crouch: [bool; MAX_PLAYERS],
}

pub struct MatchConfig {
    pub mode: Mode,
    pub map: MapId,
    pub weather: Weather,
    pub seed: u64,
}

pub struct World {
    pub cfg_mode: Mode,
    pub map: MapData,
    pub weather: Weather,
    pub rng: Rng,

    pub tick: u32,
    pub time: f32,
    pub time_left: f32,
    pub finished: bool,
    pub winner: u8,

    pub players: Vec<Player>,
    pub projectiles: Vec<Projectile>,
    pub shimmers: Vec<Shimmer>,
    pub cinders: Vec<Cinder>,
    pub pickups: Vec<Pickup>,

    pub team_score: [i32; 2],
    /// Skirmish: consecutive headshot kills without a body-only kill.
    pub head_streak: [u32; 2],
    pub first_blood_taken: bool,

    // Uplink
    pub core_pos: Vec3,
    pub core_carrier: Option<u8>,
    pub core_active: bool,
    pub core_respawn_at: f32,
    pub terminal_index: usize,

    // Last Light
    pub round: u32,
    pub round_wins: [u32; 2],
    pub round_ends_at: f32,
    pub round_intermission_until: f32,
    pub fog_radius: f32,

    /// `revealed_until[team][slot]`: a team can see that player through walls
    /// until this time. Echo's Pulse and Last Light's ghost pings both write
    /// here, and the snapshot's visibility pass reads it.
    pub revealed_until: [[f32; MAX_PLAYERS]; 2],
    /// Skirmish's Bounty fires once, at 2:30.
    bounty_done: bool,
    /// Last Light: one ghost ping per player per round, 8 s per team.
    pub ghost_ping_ready: [bool; MAX_PLAYERS],
    team_ping_cooldown: [f32; 2],
    second_wind_used: bool,

    pub schedule: Vec<ScheduledEvent>,
    pub active: Vec<ActiveEvent>,
    pub events: Vec<SimEvent>,

    history: Vec<HistoryFrame>,
    history_head: usize,
}

impl World {
    pub fn new(cfg: MatchConfig) -> World {
        let mut map = load_map(cfg.map);
        // Terminals are hand-placed alongside the geometry, so resolve them
        // the same way spawns are resolved rather than trusting the numbers.
        map.terminals = map
            .terminals
            .iter()
            .map(|t| free_spot(*t, &map.brushes))
            .collect();
        let mut rng = Rng::new(cfg.seed);
        let schedule = build_schedule(&mut rng, cfg.mode);
        World {
            cfg_mode: cfg.mode,
            map,
            weather: cfg.weather,
            rng,
            tick: 0,
            time: 0.0,
            time_left: cfg.mode.duration(),
            finished: false,
            winner: 255,
            players: Vec::new(),
            projectiles: Vec::new(),
            shimmers: Vec::new(),
            cinders: Vec::new(),
            pickups: Vec::new(),
            team_score: [0, 0],
            head_streak: [0, 0],
            first_blood_taken: false,
            core_pos: Vec3::ZERO,
            core_carrier: None,
            core_active: false,
            core_respawn_at: 15.0,
            terminal_index: 0,
            round: 1,
            round_wins: [0, 0],
            round_ends_at: 60.0,
            round_intermission_until: 3.0,
            fog_radius: 100.0,
            revealed_until: [[-99.0; MAX_PLAYERS]; 2],
            bounty_done: false,
            ghost_ping_ready: [true; MAX_PLAYERS],
            team_ping_cooldown: [0.0; 2],
            second_wind_used: false,
            schedule,
            active: Vec::new(),
            events: Vec::new(),
            history: vec![HistoryFrame::default(); MAX_REWIND_TICKS + 4],
            history_head: 0,
        }
    }

    pub fn add_player(&mut self, mut p: Player) {
        // Two players per team share every other slot, so the spawn index is
        // the player's index within their team, not their slot.
        let nth = p.slot as usize / 2;
        p.mv.pos = self.spawn_point(p.team, nth);
        p.yaw = if p.team == 0 { 0.0 } else { core::f32::consts::PI };
        // Everyone is alive from the first tick, Last Light included: its
        // opening round starts with the match, it does not wait for one.
        p.alive = true;
        p.health = MAX_HEALTH;
        p.armor = p.character.armor();
        self.players.push(p);
    }

    pub fn team_precision_mult(&self, team: u8) -> f32 {
        match self.head_streak[team as usize] {
            0..=2 => 1.0,
            3..=5 => 1.1,
            _ => 1.25,
        }
    }

    pub fn event_active(&self, kind: StaticEvent) -> bool {
        self.active.iter().any(|a| a.kind == kind)
    }

    fn gravity_mult(&self) -> f32 {
        if self.event_active(StaticEvent::GravityDip) {
            0.4
        } else {
            1.0
        }
    }

    fn score_mult(&self) -> f32 {
        if self.event_active(StaticEvent::OvertimeCoin) {
            2.0
        } else {
            1.0
        }
    }

    fn spawn_point(&mut self, team: u8, nth: usize) -> Vec3 {
        let list = if team == 0 {
            &self.map.spawns_a
        } else {
            &self.map.spawns_b
        };
        let base = list[nth % list.len()];
        free_spot(base, &self.map.brushes)
    }

    /// Respawn away from live enemies, which is the whole spawn rule: pick the
    /// friendly spawn point with the greatest distance to the nearest enemy.
    fn best_spawn(&self, team: u8) -> Vec3 {
        let list = if team == 0 {
            &self.map.spawns_a
        } else {
            &self.map.spawns_b
        };
        let mut best = list[0];
        let mut best_d = -1.0f32;
        for &s in list {
            let mut nearest = f32::MAX;
            for p in &self.players {
                if p.team != team && p.alive {
                    let d = p.mv.pos.sub(s).len();
                    if d < nearest {
                        nearest = d;
                    }
                }
            }
            if nearest > best_d {
                best_d = nearest;
                best = s;
            }
        }
        free_spot(best, &self.map.brushes)
    }

    fn record_history(&mut self) {
        let mut f = HistoryFrame::default();
        for p in &self.players {
            let i = p.slot as usize;
            f.pos[i] = p.mv.pos;
            f.alive[i] = p.alive;
            f.crouch[i] = p.mv.crouching;
        }
        let n = self.history.len();
        self.history_head = (self.history_head + 1) % n;
        self.history[self.history_head] = f;
    }

    /// Position of `slot` as it was `back` ticks ago, clamped to the rewind
    /// window. Shots from further behind resolve at present time.
    fn rewound(&self, slot: usize, back: usize) -> (Vec3, bool) {
        let back = back.min(MAX_REWIND_TICKS);
        let n = self.history.len();
        let idx = (self.history_head + n - back) % n;
        let f = &self.history[idx];
        (f.pos[slot], f.crouch[slot])
    }

    pub fn set_input(&mut self, slot: u8, input: Input) {
        if let Some(p) = self.players.iter_mut().find(|p| p.slot == slot) {
            p.last_input = input;
            p.yaw = input.yaw;
            p.pitch = clamp(input.pitch, -1.55, 1.55);
        }
    }

    /// Advance the world by exactly one tick.
    pub fn step(&mut self) {
        self.events.clear();
        if self.finished {
            return;
        }
        self.tick += 1;
        self.time += DT;
        self.time_left -= DT;

        self.step_schedule();
        self.step_players();
        self.step_projectiles();
        self.step_entities();
        match self.cfg_mode {
            Mode::Uplink => self.step_uplink(),
            Mode::LastLight => self.step_last_light(),
            _ => {}
        }
        self.record_history();
        self.check_end();
    }

    // ---------------------------------------------------------------- events

    fn step_schedule(&mut self) {
        let now = self.time;
        let mut to_fire: Vec<StaticEvent> = Vec::new();
        let mut to_telegraph: Vec<StaticEvent> = Vec::new();
        for s in self.schedule.iter_mut() {
            if !s.fired && now >= s.telegraph_at && now < s.fires_at {
                if now - s.telegraph_at < DT {
                    to_telegraph.push(s.kind);
                }
            }
            if !s.fired && now >= s.fires_at {
                s.fired = true;
                to_fire.push(s.kind);
            }
        }
        for k in to_telegraph {
            self.events.push(SimEvent::Telegraph { kind: k as u8 });
        }
        for k in to_fire {
            self.start_event(k);
        }

        // Skirmish's Bounty: at 2:30 the leader is lit up for everyone. It is
        // a mode event, so it does not consume one of the match's global
        // Static Event slots.
        if self.cfg_mode == Mode::Skirmish && !self.bounty_done && self.time >= 150.0 {
            self.bounty_done = true;
            if !self.event_active(StaticEvent::TheMark) {
                self.start_event(StaticEvent::TheMark);
            }
        }

        for t in 0..2 {
            self.team_ping_cooldown[t] = (self.team_ping_cooldown[t] - DT).max(0.0);
        }
        let now = self.time;
        let mut ended: Vec<StaticEvent> = Vec::new();
        self.active.retain(|a| {
            if now >= a.ends_at {
                ended.push(a.kind);
                false
            } else {
                true
            }
        });
        for k in ended {
            if k == StaticEvent::GoldenClip {
                for p in self.players.iter_mut() {
                    if p.weapon == Weapon::Lance && p.marked {
                        p.marked = false;
                    }
                }
            }
            if k == StaticEvent::TheMark {
                for p in self.players.iter_mut() {
                    p.marked = false;
                }
            }
            self.events.push(SimEvent::EventEnd { kind: k as u8 });
        }
    }

    fn trailing_team(&self) -> u8 {
        if self.team_score[0] < self.team_score[1] {
            0
        } else {
            1
        }
    }

    fn start_event(&mut self, kind: StaticEvent) {
        self.active.push(ActiveEvent {
            kind,
            ends_at: self.time + kind.duration(),
        });
        self.events.push(SimEvent::EventStart { kind: kind as u8 });

        match kind {
            // Underdog-weighted pick: the trailing team's lowest scorer.
            StaticEvent::GoldenClip => {
                let team = self.trailing_team();
                let mut pick: Option<usize> = None;
                let mut low = i32::MAX;
                for (i, p) in self.players.iter().enumerate() {
                    if p.team == team && p.connected && p.stats.score < low {
                        low = p.stats.score;
                        pick = Some(i);
                    }
                }
                if let Some(i) = pick {
                    self.players[i].weapon = Weapon::Lance;
                    self.players[i].ammo = Weapon::Lance.stats().mag;
                    self.players[i].marked = true;
                }
            }
            StaticEvent::TheMark => {
                let mut best = 0usize;
                let mut high = i32::MIN;
                for (i, p) in self.players.iter().enumerate() {
                    if p.stats.score > high {
                        high = p.stats.score;
                        best = i;
                    }
                }
                if !self.players.is_empty() {
                    self.players[best].marked = true;
                }
            }
            StaticEvent::WeaponRoulette => {
                let picks = [Weapon::Sting, Weapon::Ridge, Weapon::Maul, Weapon::Arc];
                for i in 0..self.players.len() {
                    let mut w = picks[self.rng.next_u32(4) as usize];
                    if w == self.players[i].weapon {
                        w = picks[(self.rng.next_u32(3) + 1) as usize % 4];
                    }
                    self.players[i].weapon = w;
                    self.players[i].ammo = w.stats().mag;
                    self.players[i].reload_timer = 0.0;
                }
            }
            StaticEvent::WeatherTurn => {
                self.weather = match self.weather {
                    Weather::Clear => Weather::Rain,
                    Weather::Rain => Weather::Night,
                    Weather::Night => Weather::Clear,
                };
            }
            // The crate lands in the trailing team's half of the map.
            StaticEvent::Airdrop => {
                let team = self.trailing_team();
                let sign = if team == 0 { -1.0 } else { 1.0 };
                let z = self.map.bounds.max.z * 0.45 * sign;
                let x = self.rng.next_signed() * self.map.bounds.max.x * 0.4;
                self.pickups.push(Pickup {
                    pos: v3(x, 0.6, z),
                    weapon: Weapon::Lance,
                    taken: false,
                });
            }
            _ => {}
        }
    }

    // --------------------------------------------------------------- players

    fn step_players(&mut self) {
        let gravity = self.gravity_mult();
        let n = self.players.len();

        for i in 0..n {
            let (alive, respawn_at, team) = {
                let p = &self.players[i];
                (p.alive, p.respawn_at, p.team)
            };
            if !alive {
                if self.cfg_mode != Mode::LastLight && self.time >= respawn_at {
                    let spawn = self.best_spawn(team);
                    let p = &mut self.players[i];
                    p.mv = MoveState {
                        pos: spawn,
                        ..Default::default()
                    };
                    p.alive = true;
                    p.health = MAX_HEALTH;
                    p.armor = p.character.armor();
                    p.staggered = false;
                    p.ammo = p.weapon.stats().mag;
                    p.reload_timer = 0.0;
                    p.burst_left = 0;
                    p.eng_shots = 0;
                    p.eng_hits = 0;
                    self.events.push(SimEvent::Spawn { slot: i as u8 });
                }
                continue;
            }

            // Timers.
            {
                let p = &mut self.players[i];
                p.fire_cooldown = (p.fire_cooldown - DT).max(0.0);
                p.burst_timer = (p.burst_timer - DT).max(0.0);
                p.ability_cooldown = (p.ability_cooldown - DT).max(0.0);
                p.melee_cooldown = (p.melee_cooldown - DT).max(0.0);
                p.focus_timer = (p.focus_timer - DT).max(0.0);
                let ads_target = if p.last_input.held(buttons::ADS) {
                    1.0
                } else {
                    0.0
                };
                let ads_speed = {
                    let t = p.weapon.stats().ads_time.max(0.05);
                    DT / t
                };
                p.ads = clamp(p.ads + (ads_target - p.ads).signum() * ads_speed, 0.0, 1.0);

                let sprinting = p.last_input.held(buttons::SPRINT)
                    && p.last_input.held(buttons::FWD)
                    && !p.staggered;
                if sprinting {
                    p.sprint_lock = SPRINT_FIRE_DELAY;
                } else {
                    p.sprint_lock = (p.sprint_lock - DT).max(0.0);
                }

                // Armour regenerates five seconds after the last hit taken.
                let max_armor = p.character.armor();
                if max_armor > 0 && p.armor < max_armor && self.time - p.last_damaged_at > 5.0 {
                    p.armor = max_armor;
                }
                // Headhunt: a staggered player recovers after four clear seconds.
                if p.staggered && self.time >= p.stagger_clear_at {
                    p.staggered = false;
                    p.health = MAX_HEALTH;
                }
                if p.eng_shots > 0 && self.time - p.eng_started_at > 4.0 {
                    p.eng_shots = 0;
                    p.eng_hits = 0;
                }
            }

            // Movement.
            {
                let input = self.players[i].last_input;
                let speed = self.players[i].speed_mult();
                let can_sprint = !self.players[i].staggered && !self.players[i].carrying_core;
                let mut mv = self.players[i].mv;
                step_movement(
                    &mut mv,
                    &input,
                    &self.map.brushes,
                    speed,
                    gravity,
                    can_sprint,
                );
                self.players[i].mv = mv;
            }

            self.step_weapon(i);
            self.step_ability(i);
            self.step_pickups(i);
        }
    }

    fn step_weapon(&mut self, i: usize) {
        let input = self.players[i].last_input;
        let stats = self.players[i].weapon.stats();

        // Reloading, including the interruptible shell-at-a-time path.
        if self.players[i].reload_timer > 0.0 {
            self.players[i].reload_timer -= DT;
            if self.players[i].reload_timer <= 0.0 {
                let p = &mut self.players[i];
                if stats.shell_reload {
                    p.ammo = (p.ammo + 1).min(stats.mag);
                    if p.ammo < stats.mag && !p.last_input.held(buttons::FIRE) {
                        p.reload_timer = stats.reload * p.character.reload_mult();
                    }
                } else {
                    p.ammo = stats.mag;
                }
            }
            if !stats.shell_reload {
                return;
            }
        }

        let wants_reload = input.held(buttons::RELOAD) && self.players[i].ammo < stats.mag;
        if wants_reload && self.players[i].reload_timer <= 0.0 {
            self.players[i].reload_timer = stats.reload * self.players[i].character.reload_mult();
            return;
        }

        // Melee. Instant from behind, and the execution tool in Headhunt.
        // The Blade loadout has no gun, so its primary fire swings too, on a
        // shorter cooldown.
        let blade = self.players[i].weapon == Weapon::Blade;
        let wants_melee =
            input.held(buttons::MELEE) || (blade && input.held(buttons::FIRE));
        if wants_melee && self.players[i].melee_cooldown <= 0.0 {
            self.players[i].melee_cooldown = if blade { 0.5 } else { 0.7 };
            self.resolve_melee(i);
        }
        if blade {
            return;
        }

        // Focus: spend a full Precision Charge for two seconds of steadier aim.
        if input.held(buttons::ABILITY)
            && input.held(buttons::ADS)
            && self.players[i].charge >= 1.0
        {
            self.players[i].charge = 0.0;
            self.players[i].focus_timer = 2.0;
        }

        // Continue an in-flight Arc burst.
        if self.players[i].burst_left > 0 && self.players[i].burst_timer <= 0.0 {
            self.players[i].burst_left -= 1;
            self.players[i].burst_timer = stats.burst_interval;
            self.fire_once(i);
            return;
        }

        let can_fire = input.held(buttons::FIRE)
            && self.players[i].fire_cooldown <= 0.0
            && self.players[i].sprint_lock <= 0.0
            && self.players[i].reload_timer <= 0.0
            && !self.players[i].staggered;

        if !can_fire {
            return;
        }
        if self.players[i].ammo <= 0 {
            self.players[i].reload_timer = stats.reload * self.players[i].character.reload_mult();
            return;
        }

        self.players[i].fire_cooldown = stats.interval;
        if stats.burst > 1 {
            self.players[i].burst_left = stats.burst - 1;
            self.players[i].burst_timer = stats.burst_interval;
        }
        self.fire_once(i);
    }

    fn fire_once(&mut self, i: usize) {
        let stats = self.players[i].weapon.stats();
        if self.players[i].ammo <= 0 {
            return;
        }
        self.players[i].ammo -= 1;
        self.players[i].stats.shots_fired += 1;
        if self.players[i].eng_shots == 0 {
            self.players[i].eng_started_at = self.time;
        }
        self.players[i].eng_shots += 1;

        let origin = self.players[i].eye();
        let yaw = self.players[i].yaw;
        let pitch = self.players[i].pitch;
        let base = look_dir(yaw, pitch);

        // Focus and aiming down sights both tighten the cone.
        let mut spread = stats.spread;
        if self.players[i].ads > 0.5 {
            spread *= 0.4;
        }
        if self.players[i].focus_timer > 0.0 {
            spread *= 0.4;
        }
        if self.players[i].mv.vel.len_xz() > 4.0 {
            spread *= 1.8;
        }

        if stats.is_hitscan() {
            let pellets = stats.pellets.max(1);
            let mut any_hit = false;
            let mut any_head = false;
            let mut end = origin.add(base.scale(120.0));
            for pellet in 0..pellets {
                let dir = if spread > 0.0 {
                    let a = self.rng.next_signed() * spread;
                    let b = self.rng.next_signed() * spread;
                    // Deterministic cone: perturb in the two axes orthogonal
                    // to the aim vector.
                    let right = v3(base.z, 0.0, -base.x).normalized();
                    let up = v3(
                        right.y * base.z - right.z * base.y,
                        right.z * base.x - right.x * base.z,
                        right.x * base.y - right.y * base.x,
                    );
                    base.add(right.scale(a)).add(up.scale(b)).normalized()
                } else {
                    base
                };
                let (hit, head, point) = self.resolve_hitscan(i, origin, dir, stats.body, stats.head);
                if pellet == 0 {
                    end = point;
                }
                if hit {
                    any_hit = true;
                }
                if head {
                    any_head = true;
                }
            }
            if any_hit {
                self.players[i].stats.shots_hit += 1;
                self.players[i].eng_hits += 1;
                if any_head {
                    self.players[i].stats.head_hits += 1;
                    let c = self.players[i].charge;
                    self.players[i].charge = (c + 0.25).min(1.0);
                }
            }
            self.events.push(SimEvent::Shot {
                slot: i as u8,
                weapon: self.players[i].weapon as u8,
                origin,
                end,
                hit: any_hit,
                headshot: any_head,
            });
        } else {
            let dir = base;
            self.projectiles.push(Projectile {
                owner: i as u8,
                team: self.players[i].team,
                pos: origin,
                vel: dir.scale(stats.projectile_speed),
                body: stats.body,
                head: stats.head,
                weapon: self.players[i].weapon as u8,
                life: 2.5,
                penetrated: false,
            });
            self.events.push(SimEvent::Shot {
                slot: i as u8,
                weapon: self.players[i].weapon as u8,
                origin,
                end: origin.add(dir.scale(3.0)),
                hit: false,
                headshot: false,
            });
        }
    }

    /// Resolve one hitscan pellet against rewound player positions.
    fn resolve_hitscan(
        &mut self,
        shooter: usize,
        origin: Vec3,
        dir: Vec3,
        body_dmg: f32,
        head_dmg: f32,
    ) -> (bool, bool, Vec3) {
        let back = self.players[shooter].rewind_ticks as usize;
        let hard_light = self.event_active(StaticEvent::HardLight);
        let (mut world_t, world_thin) = trace_world(origin, dir, 120.0, &self.map.brushes);
        // Hard Light lets every shot punch one wall.
        let penetrating = hard_light || world_thin;
        let wall_limit = if penetrating { 120.0 } else { world_t };

        // Shimmer walls stop enemy bullets, but not their owners'.
        let mut shimmer_t = 120.0f32;
        for s in &self.shimmers {
            if s.team == self.players[shooter].team {
                continue;
            }
            let half = v3(1.5 * cos(s.yaw) + 0.15, 1.25, 1.5 * sin(s.yaw) + 0.15);
            let bx = Aabb::from_center(s.pos.add(v3(0.0, 1.25, 0.0)), half);
            if let Some(t) = bx.ray(origin, dir, wall_limit) {
                if t < shimmer_t {
                    shimmer_t = t;
                }
            }
        }
        let block_t = wall_limit.min(shimmer_t);

        let mut best_t = block_t;
        let mut best_target: Option<(usize, bool)> = None;
        for j in 0..self.players.len() {
            if j == shooter || !self.players[j].alive {
                continue;
            }
            if self.players[j].team == self.players[shooter].team {
                continue;
            }
            let (pos, _crouch) = self.rewound(j, back);
            let head = self.players[j].head_box(pos);
            let body = self.players[j].body_box(pos);
            if let Some(t) = head.ray(origin, dir, best_t) {
                if t < best_t {
                    best_t = t;
                    best_target = Some((j, true));
                }
            }
            if let Some(t) = body.ray(origin, dir, best_t) {
                if t < best_t {
                    best_t = t;
                    best_target = Some((j, false));
                }
            }
        }

        if let Some((victim, headshot)) = best_target {
            let dist = best_t;
            let stats = self.players[shooter].weapon.stats();
            let mut dmg = if headshot { head_dmg } else { body_dmg } * stats.falloff(dist);
            let through_cover = dist > world_t && penetrating;
            if through_cover {
                dmg *= 0.5;
            }
            let point = origin.add(dir.scale(best_t));
            // A pellet that lands beyond the weapon's falloff does no damage,
            // and a shot that does no damage is not a hit. Counting it would
            // inflate accuracy and, through it, Aim Rating — a shotgun player
            // spraying at forty metres would read as a marksman.
            if dmg <= 0.0 {
                return (false, false, point);
            }
            self.apply_damage(shooter, victim, dmg, headshot, dist, through_cover);
            (true, headshot, point)
        } else {
            world_t = world_t.min(shimmer_t);
            (false, false, origin.add(dir.scale(world_t)))
        }
    }

    fn resolve_melee(&mut self, i: usize) {
        let dir = look_dir(self.players[i].yaw, self.players[i].pitch);
        for j in 0..self.players.len() {
            if j == i || !self.players[j].alive || self.players[j].team == self.players[i].team {
                continue;
            }
            let d = self.players[j].mv.pos.sub(self.players[i].mv.pos);
            if d.len() > 2.2 {
                continue;
            }
            let facing = d.normalized().dot(dir);
            if facing < 0.4 {
                continue;
            }
            // From behind, or into a staggered target, the knife finishes.
            let victim_dir = look_dir(self.players[j].yaw, 0.0);
            let from_behind = victim_dir.dot(d.normalized()) > 0.4;
            let dmg = if from_behind || self.players[j].staggered {
                999.0
            } else if self.players[i].weapon == Weapon::Blade {
                // A dedicated knife kills from the front too — it is the
                // whole loadout.
                100.0
            } else {
                55.0
            };
            self.apply_damage(i, j, dmg, false, d.len(), false);
            return;
        }
    }

    fn apply_damage(
        &mut self,
        attacker: usize,
        victim: usize,
        raw: f32,
        headshot: bool,
        distance: f32,
        through_cover: bool,
    ) {
        if !self.players[victim].alive || raw <= 0.0 {
            return;
        }
        let full_health_before =
            self.players[victim].health >= MAX_HEALTH && self.players[victim].armor > 0
            || self.players[victim].health >= MAX_HEALTH;

        // Round to the nearest point instead of truncating: a hit that passed
        // the falloff check always costs at least one point of health or
        // armour, so the balance-table time-to-kill holds and health cannot
        // linger just above zero on fractional falloff damage.
        let mut dmg = ((raw + 0.5) as i32).max(1);
        // Plating: armour soaks body damage first, head shots ignore it.
        if !headshot && self.players[victim].armor > 0 {
            let soak = self.players[victim].armor.min(dmg);
            self.players[victim].armor -= soak;
            dmg -= soak;
        }
        let dealt = dmg;
        self.players[victim].health -= dmg;
        self.players[victim].last_damaged_at = self.time;
        self.players[victim].last_damaged_by = Some(attacker as u8);

        self.players[attacker].stats.damage += raw;
        self.players[attacker].dealt[victim] += raw;
        self.players[attacker].dealt_at[victim] = self.time;

        self.events.push(SimEvent::Damage {
            attacker: attacker as u8,
            victim: victim as u8,
            amount: raw as i32,
            headshot,
        });
        let _ = dealt;

        // Headhunt: body damage cannot kill. It staggers instead.
        if self.cfg_mode == Mode::Headhunt && !headshot && self.players[victim].health <= 0 {
            let already = self.players[victim].staggered;
            self.players[victim].health = 1;
            self.players[victim].staggered = true;
            self.players[victim].stagger_clear_at = self.time + 4.0;
            if !already {
                self.events.push(SimEvent::Stagger {
                    slot: victim as u8,
                });
            }
            return;
        }

        if self.players[victim].health <= 0 {
            self.kill(attacker, victim, headshot, distance, through_cover, full_health_before);
        }
    }

    fn kill(
        &mut self,
        attacker: usize,
        victim: usize,
        headshot: bool,
        distance: f32,
        through_cover: bool,
        victim_was_full: bool,
    ) {
        let mut bonuses: Vec<Bonus> = Vec::new();
        let base = if headshot { 175 } else { 100 };
        let mut points = base;

        if headshot && victim_was_full {
            bonuses.push(Bonus {
                label: "Clean",
                points: 50,
            });
        }
        if self.players[attacker].eng_shots > 0
            && self.players[attacker].eng_hits >= self.players[attacker].eng_shots
        {
            bonuses.push(Bonus {
                label: "Surgical",
                points: 40,
            });
            self.players[attacker].stats.surgical += 1;
        }
        if headshot && distance > 35.0 {
            bonuses.push(Bonus {
                label: "Longshot",
                points: 60,
            });
            self.players[attacker].stats.longshot += 1;
        }
        if through_cover {
            bonuses.push(Bonus {
                label: "Blindside",
                points: 50,
            });
        }
        // Duel: the victim had you under fire and had landed a hit.
        if self.players[victim].dealt[attacker] > 0.0
            && self.time - self.players[victim].dealt_at[attacker] < 3.0
        {
            bonuses.push(Bonus {
                label: "Duel",
                points: 35,
            });
        }
        // Rescue: the victim had damaged a teammate in the last two seconds.
        let my_team = self.players[attacker].team;
        let rescued = (0..self.players.len()).any(|k| {
            k != attacker
                && self.players[k].team == my_team
                && self.players[victim].dealt[k] > 0.0
                && self.time - self.players[victim].dealt_at[k] < 2.0
        });
        if rescued {
            bonuses.push(Bonus {
                label: "Rescue",
                points: 30,
            });
        }
        if !self.first_blood_taken {
            self.first_blood_taken = true;
            bonuses.push(Bonus {
                label: "First Blood",
                points: 75,
            });
        }
        if self.players[victim].marked {
            points *= 3;
            bonuses.push(Bonus {
                label: "Bounty",
                points: 0,
            });
            self.players[victim].marked = false;
        }
        if self.players[victim].carrying_core {
            bonuses.push(Bonus {
                label: "Core Kill",
                points: 80,
            });
        }

        for b in &bonuses {
            points += b.points;
        }
        let mult = self.score_mult() * self.team_precision_mult(my_team);
        points = (points as f32 * mult) as i32;

        // Assists: anyone else who put 40+ damage into the victim recently.
        for k in 0..self.players.len() {
            if k == attacker || self.players[k].team != my_team {
                continue;
            }
            if self.players[k].dealt[victim] >= 40.0 && self.time - self.players[k].dealt_at[victim] < 6.0
            {
                self.players[k].stats.assists += 1;
                self.players[k].stats.score += 45;
                if self.cfg_mode == Mode::Skirmish {
                    self.team_score[my_team as usize] += 45;
                }
            }
        }

        self.players[attacker].stats.kills += 1;
        self.players[attacker].stats.score += points;
        if headshot {
            self.players[attacker].stats.headshot_kills += 1;
            self.head_streak[my_team as usize] += 1;
            if victim_was_full {
                self.players[attacker].stats.clean += 1;
            }
        } else {
            self.head_streak[my_team as usize] = 0;
        }
        let eng = self.time - self.players[attacker].eng_started_at;
        if eng > 0.0 && eng < 8.0 {
            self.players[attacker].stats.ttk_total += eng;
            self.players[attacker].stats.ttk_count += 1;
        }
        self.players[attacker].eng_shots = 0;
        self.players[attacker].eng_hits = 0;

        match self.cfg_mode {
            Mode::Skirmish => self.team_score[my_team as usize] += points,
            Mode::Headhunt => {
                self.team_score[my_team as usize] += if headshot { 2 } else { 1 };
            }
            _ => {}
        }

        let weapon = self.players[attacker].weapon as u8;
        self.events.push(SimEvent::Kill {
            attacker: attacker as u8,
            victim: victim as u8,
            weapon,
            headshot,
            distance,
            points,
            bonuses,
        });

        // Drop the core where the carrier fell.
        if self.players[victim].carrying_core {
            self.players[victim].carrying_core = false;
            self.core_carrier = None;
            self.core_pos = self.players[victim].mv.pos.add(v3(0.0, 0.6, 0.0));
            self.events.push(SimEvent::CoreDropped { pos: self.core_pos });
        }

        let p = &mut self.players[victim];
        p.alive = false;
        p.staggered = false;
        p.health = 0;
        p.stats.deaths += 1;
        p.respawn_at = self.time + self.cfg_mode.respawn_delay();
        p.carrying_core = false;
        for k in 0..MAX_PLAYERS {
            p.dealt[k] = 0.0;
        }
    }

    // ------------------------------------------------------------ abilities

    fn step_ability(&mut self, i: usize) {
        let input = self.players[i].last_input;
        // Ability without ADS held; ADS + ability is the Focus spend.
        if !input.held(buttons::ABILITY)
            || input.held(buttons::ADS)
            || self.players[i].ability_cooldown > 0.0
            || self.players[i].staggered
        {
            return;
        }
        let ch = self.players[i].character;
        self.players[i].ability_cooldown = ch.ability_cooldown();
        let pos = self.players[i].mv.pos;
        let yaw = self.players[i].yaw;
        let team = self.players[i].team;
        let dir = look_dir(yaw, 0.0);

        match ch {
            Character::Ward => {
                let wall = pos.add(dir.scale(2.2));
                self.shimmers.push(Shimmer {
                    team,
                    pos: wall,
                    yaw,
                    hp: 150.0,
                    ends_at: self.time + 8.0,
                });
            }
            Character::Vane => {
                // Blink: 12 m dash, stopped by geometry, disabled while
                // carrying the core.
                if !self.players[i].carrying_core {
                    let mut best = pos;
                    for s in 1..=24 {
                        let t = s as f32 * 0.5;
                        let cand = pos.add(dir.scale(t));
                        let bx = player_box(cand, self.players[i].mv.crouching);
                        let hit = self.map.brushes.iter().any(|b| b.aabb.overlaps(&bx));
                        if hit {
                            break;
                        }
                        best = cand;
                    }
                    self.players[i].mv.pos = best;
                }
            }
            Character::Echo => {
                // Pulse: a 22 m scan that outlines enemies through walls for
                // 1.5 s. Everyone caught in it is told they were caught, so
                // the information flows both ways.
                let mut caught: Vec<u8> = Vec::new();
                for j in 0..self.players.len() {
                    if self.players[j].team == team || !self.players[j].alive {
                        continue;
                    }
                    if self.players[j].mv.pos.sub(pos).len() <= 22.0 {
                        self.revealed_until[team as usize][j] = self.time + 1.5;
                        caught.push(j as u8);
                    }
                }
                for slot in caught {
                    self.events.push(SimEvent::Revealed {
                        slot,
                        by_team: team,
                    });
                }
            }
            Character::Kiln => {
                let a = pos.add(dir.scale(1.5));
                let b = pos.add(dir.scale(10.5));
                self.cinders.push(Cinder {
                    owner: i as u8,
                    team,
                    a,
                    b,
                    ends_at: self.time + 6.0,
                });
            }
        }
        self.events.push(SimEvent::Ability {
            slot: i as u8,
            kind: ch as u8,
            pos,
            yaw,
        });
    }

    fn step_pickups(&mut self, i: usize) {
        let pos = self.players[i].mv.pos;
        for k in 0..self.pickups.len() {
            if self.pickups[k].taken {
                continue;
            }
            if self.pickups[k].pos.sub(pos).len() < 1.4 {
                self.pickups[k].taken = true;
                let w = self.pickups[k].weapon;
                self.players[i].weapon = w;
                self.players[i].ammo = w.stats().mag;
                self.events.push(SimEvent::PickupTaken {
                    slot: i as u8,
                    weapon: w as u8,
                });
            }
        }
    }

    // ---------------------------------------------------------- projectiles

    fn step_projectiles(&mut self) {
        let mut hits: Vec<(usize, usize, f32, bool, f32, bool)> = Vec::new();
        let mut remove: Vec<usize> = Vec::new();
        let hard_light = self.event_active(StaticEvent::HardLight);

        for (idx, pr) in self.projectiles.iter_mut().enumerate() {
            pr.life -= DT;
            if pr.life <= 0.0 {
                remove.push(idx);
                continue;
            }
            let step = pr.vel.scale(DT);
            let dist = step.len();
            let dir = step.normalized();
            let origin = pr.pos;

            let (world_t, world_thin) = trace_world(origin, dir, dist, &self.map.brushes);
            let mut best_t = dist;
            let mut target: Option<(usize, bool)> = None;
            for j in 0..self.players.len() {
                if !self.players[j].alive || self.players[j].team == pr.team {
                    continue;
                }
                let head = self.players[j].head_box(self.players[j].mv.pos);
                let body = self.players[j].body_box(self.players[j].mv.pos);
                if let Some(t) = head.ray(origin, dir, best_t) {
                    if t < best_t {
                        best_t = t;
                        target = Some((j, true));
                    }
                }
                if let Some(t) = body.ray(origin, dir, best_t) {
                    if t < best_t {
                        best_t = t;
                        target = Some((j, false));
                    }
                }
            }

            if world_t < best_t {
                // Thin cover: punch through once at half damage.
                if (world_thin || hard_light) && !pr.penetrated {
                    pr.penetrated = true;
                    pr.body *= 0.5;
                    pr.head *= 0.5;
                    pr.pos = origin.add(dir.scale(world_t + 0.35));
                    continue;
                }
                remove.push(idx);
                continue;
            }

            if let Some((j, head)) = target {
                let travelled = origin.sub(self.players[pr.owner as usize].eye()).len() + best_t;
                hits.push((
                    pr.owner as usize,
                    j,
                    if head { pr.head } else { pr.body },
                    head,
                    travelled,
                    pr.penetrated,
                ));
                remove.push(idx);
                continue;
            }
            pr.pos = origin.add(step);
        }

        for idx in remove.into_iter().rev() {
            self.projectiles.remove(idx);
        }
        for (attacker, victim, dmg, head, dist, cover) in hits {
            self.players[attacker].stats.shots_hit += 1;
            self.players[attacker].eng_hits += 1;
            if head {
                self.players[attacker].stats.head_hits += 1;
                let c = self.players[attacker].charge;
                self.players[attacker].charge = (c + 0.25).min(1.0);
            }
            self.apply_damage(attacker, victim, dmg, head, dist, cover);
        }
    }

    fn step_entities(&mut self) {
        let now = self.time;
        self.shimmers.retain(|s| now < s.ends_at && s.hp > 0.0);
        self.cinders.retain(|c| now < c.ends_at);

        // Cinderline burns anyone standing in it: 12 dps, no head component,
        // and never lethal to a full-health player inside its six seconds.
        let mut burns: Vec<(usize, usize)> = Vec::new();
        for c in &self.cinders {
            for j in 0..self.players.len() {
                if !self.players[j].alive || self.players[j].team == c.team {
                    continue;
                }
                let p = self.players[j].mv.pos;
                let ab = c.b.sub(c.a);
                let t = clamp(p.sub(c.a).dot(ab) / ab.dot(ab), 0.0, 1.0);
                let closest = c.a.add(ab.scale(t));
                if p.sub(closest).len_xz() < 1.3 && abs(p.y - closest.y) < 2.0 {
                    burns.push((c.owner as usize, j));
                }
            }
        }
        for (owner, victim) in burns {
            self.apply_damage(owner, victim, 12.0 * DT, false, 0.0, false);
        }
    }

    // -------------------------------------------------------------- uplink

    fn step_uplink(&mut self) {
        if !self.core_active {
            if self.time >= self.core_respawn_at {
                self.core_active = true;
                self.core_pos = self.map.center;
                self.core_carrier = None;
            }
            return;
        }
        if let Some(c) = self.core_carrier {
            let ci = c as usize;
            if !self.players[ci].alive {
                self.core_carrier = None;
            } else {
                self.core_pos = self.players[ci].mv.pos.add(v3(0.0, 1.0, 0.0));
                let term = self.map.terminals[self.terminal_index % self.map.terminals.len()];
                if self.players[ci].mv.pos.sub(term).len() < 2.0 {
                    let team = self.players[ci].team as usize;
                    self.team_score[team] += 1;
                    self.players[ci].stats.score += (200.0 * self.score_mult()) as i32;
                    self.players[ci].carrying_core = false;
                    self.core_carrier = None;
                    self.core_active = false;
                    self.core_respawn_at = self.time + 5.0;
                    self.terminal_index += 1;
                    self.events.push(SimEvent::Bank {
                        slot: c,
                        team: team as u8,
                    });
                }
            }
            return;
        }
        for j in 0..self.players.len() {
            if self.players[j].alive && self.players[j].mv.pos.sub(self.core_pos).len() < 1.5 {
                self.core_carrier = Some(j as u8);
                self.players[j].carrying_core = true;
                self.players[j].stats.score += 60;
                self.events.push(SimEvent::CoreTaken { slot: j as u8 });
                return;
            }
        }
    }

    // ----------------------------------------------------------- last light

    fn step_last_light(&mut self) {
        if self.time < self.round_intermission_until {
            return;
        }
        let elapsed = self.time - (self.round_intermission_until - 3.0);
        // The fog wall closes from 40 s into the round, forcing contact.
        self.fog_radius = if elapsed > 40.0 {
            clamp(40.0 - (elapsed - 40.0) * 1.6, 6.0, 100.0)
        } else {
            100.0
        };
        if self.fog_radius < 40.0 {
            let center = self.map.center;
            let mut out: Vec<usize> = Vec::new();
            for j in 0..self.players.len() {
                if self.players[j].alive
                    && self.players[j].mv.pos.sub(center).len_xz() > self.fog_radius
                {
                    out.push(j);
                }
            }
            for j in out {
                self.apply_damage(j, j, 18.0 * DT, false, 0.0, false);
            }
        }

        // Second Wind: in round four only, the first player the trailing team
        // loses comes back once, ten seconds later, announced to everyone.
        if self.round == 4 && !self.second_wind_used {
            let trailing = self.trailing_team();
            let down = self
                .players
                .iter()
                .position(|p| p.team == trailing && !p.alive && p.connected);
            if let Some(i) = down {
                self.second_wind_used = true;
                let spawn = self.best_spawn(trailing);
                let p = &mut self.players[i];
                p.respawn_at = self.time + 10.0;
                p.mv = MoveState {
                    pos: spawn,
                    ..Default::default()
                };
                self.events.push(SimEvent::EventStart {
                    kind: StaticEvent::GoldenClip as u8,
                });
            }
        }
        if self.round == 4 && self.second_wind_used {
            for i in 0..self.players.len() {
                if !self.players[i].alive
                    && self.players[i].respawn_at > 0.0
                    && self.time >= self.players[i].respawn_at
                {
                    let team = self.players[i].team;
                    let spawn = self.best_spawn(team);
                    let p = &mut self.players[i];
                    p.respawn_at = 0.0;
                    p.alive = true;
                    p.health = MAX_HEALTH;
                    p.armor = p.character.armor();
                    p.ammo = p.weapon.stats().mag;
                    p.mv = MoveState {
                        pos: spawn,
                        ..Default::default()
                    };
                    self.events.push(SimEvent::Spawn { slot: i as u8 });
                }
            }
        }

        let alive_a = self.alive_count(0);
        let alive_b = self.alive_count(1);
        let timeout = elapsed >= 60.0;
        if alive_a == 0 || alive_b == 0 || timeout {
            let winner = if alive_a > alive_b {
                0u8
            } else if alive_b > alive_a {
                1u8
            } else {
                255u8
            };
            if winner != 255 {
                self.round_wins[winner as usize] += 1;
                self.team_score[winner as usize] += 1;
            }
            self.events.push(SimEvent::RoundEnd { winner });
            self.round += 1;
            self.round_intermission_until = self.time + 5.0;
            self.fog_radius = 100.0;
            self.ghost_ping_ready = [true; MAX_PLAYERS];
            self.team_ping_cooldown = [0.0; 2];
            self.second_wind_used = false;
            for j in 0..self.players.len() {
                let team = self.players[j].team;
                let spawn = self.best_spawn(team);
                let p = &mut self.players[j];
                p.mv = MoveState {
                    pos: spawn,
                    ..Default::default()
                };
                p.alive = p.connected;
                p.health = MAX_HEALTH;
                p.armor = p.character.armor();
                p.ammo = p.weapon.stats().mag;
                p.staggered = false;
            }
        }
    }

    /// A dead player's one contribution per round: mark the enemy nearest to
    /// where they fell, for their whole team, for 1.5 seconds.
    ///
    /// This exists because the worst moment in a round-based shooter is dying
    /// first and watching for fifty seconds. A held card turns that into
    /// something to spend.
    /// A dead player choosing what to respawn with. Only settable while dead,
    /// so a live player can never swap mid-fight; the respawn path reads
    /// `weapon` and fills the magazine.
    pub fn set_loadout(&mut self, slot: u8, weapon: Weapon) -> bool {
        let i = slot as usize;
        if i >= self.players.len() || self.players[i].alive {
            return false;
        }
        self.players[i].weapon = weapon;
        true
    }

    pub fn ghost_ping(&mut self, slot: u8) -> bool {
        let i = slot as usize;
        if self.cfg_mode != Mode::LastLight || i >= self.players.len() {
            return false;
        }
        if self.players[i].alive || !self.ghost_ping_ready[i] {
            return false;
        }
        let team = self.players[i].team;
        if self.team_ping_cooldown[team as usize] > 0.0 {
            return false;
        }
        let from = self.players[i].mv.pos;
        let mut best: Option<usize> = None;
        let mut best_d = f32::MAX;
        for j in 0..self.players.len() {
            if self.players[j].team == team || !self.players[j].alive {
                continue;
            }
            let d = self.players[j].mv.pos.sub(from).len();
            if d < best_d {
                best_d = d;
                best = Some(j);
            }
        }
        let Some(target) = best else { return false };
        self.ghost_ping_ready[i] = false;
        self.team_ping_cooldown[team as usize] = 8.0;
        self.revealed_until[team as usize][target] = self.time + 1.5;
        self.events.push(SimEvent::GhostPing {
            by: slot,
            target: target as u8,
        });
        true
    }

    fn alive_count(&self, team: u8) -> usize {
        self.players
            .iter()
            .filter(|p| p.team == team && p.alive)
            .count()
    }

    fn check_end(&mut self) {
        let target = self.cfg_mode.score_target();
        let hit_target = self.team_score[0] >= target || self.team_score[1] >= target;
        if hit_target || self.time_left <= 0.0 {
            self.finished = true;
            self.winner = if self.team_score[0] > self.team_score[1] {
                0
            } else if self.team_score[1] > self.team_score[0] {
                1
            } else {
                255
            };
            self.events.push(SimEvent::MatchEnd {
                winner: self.winner,
            });
        }
    }
}

/// Nudge a spawn point out of any geometry it happens to overlap.
///
/// Level layout and spawn placement are edited independently, so this is the
/// belt to the unit test's braces: a spawn that ends up inside a ramp puts a
/// player's camera inside a wall, which is the single worst first impression
/// the game could make.
pub fn free_spot(base: Vec3, brushes: &[Brush]) -> Vec3 {
    if !spot_blocked(base, brushes) {
        return base;
    }
    for ring in 1..=8 {
        let r = ring as f32 * 0.8;
        for step in 0..12 {
            let a = step as f32 * (core::f32::consts::TAU / 12.0);
            let cand = v3(base.x + cos(a) * r, base.y, base.z + sin(a) * r);
            if !spot_blocked(cand, brushes) {
                return cand;
            }
        }
        let lifted = v3(base.x, base.y + r, base.z);
        if !spot_blocked(lifted, brushes) {
            return lifted;
        }
    }
    base
}

fn spot_blocked(pos: Vec3, brushes: &[Brush]) -> bool {
    let b = player_box(pos, false);
    brushes.iter().any(|s| s.aabb.overlaps(&b))
}

/// Build a match's event schedule up front.
///
/// Committing the schedule at match start is the rule that keeps the surprise
/// layer honest: the server can never invent an event in reaction to the
/// scoreline. Underdog tilt is applied when the event fires, by choosing its
/// target from the trailing team, not by choosing the event itself.
pub fn build_schedule(rng: &mut Rng, mode: Mode) -> Vec<ScheduledEvent> {
    let duration = mode.duration();
    let count = 1 + rng.next_u32(3); // one to three events

    // Overtime Coin is a closing event and only a closing event. Every match
    // ends on it, and the random events are laid out behind it.
    let coin_at = duration - 30.0;
    // Nothing before 60 s, nothing within 45 s of another event, and nothing
    // close enough to the coin to still be running when it lands.
    let first = 60.0;
    let last = coin_at - MIN_EVENT_GAP;

    let mut out: Vec<ScheduledEvent> = Vec::new();
    let mut used: Vec<StaticEvent> = Vec::new();
    let mut cursor = first;
    for _ in 0..count {
        if cursor > last {
            break;
        }
        let span = (last - cursor).min(50.0);
        let at = cursor + rng.next_f32() * span;
        // Draw an event that has not already been used this match. Overtime
        // Coin is excluded from the pool entirely.
        let pool = &StaticEvent::POOL;
        let mut kind = pool[rng.next_u32(pool.len() as u32) as usize];
        let mut guard = 0;
        while used.contains(&kind) && guard < 12 {
            kind = pool[rng.next_u32(pool.len() as u32) as usize];
            guard += 1;
        }
        if used.contains(&kind) {
            break;
        }
        used.push(kind);
        out.push(ScheduledEvent {
            kind,
            telegraph_at: at - TELEGRAPH,
            fires_at: at,
            fired: false,
        });
        cursor = at + MIN_EVENT_GAP;
    }
    out.push(ScheduledEvent {
        kind: StaticEvent::OvertimeCoin,
        telegraph_at: coin_at - TELEGRAPH,
        fires_at: coin_at,
        fired: false,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World::new(MatchConfig {
            mode: Mode::Skirmish,
            map: MapId::Vault,
            weather: Weather::Clear,
            seed: 42,
        })
    }

    #[test]
    fn ridge_head_shot_kills_outright() {
        let mut w = world();
        w.add_player(Player::new(0, 0, "a".into(), Character::Vane, Weapon::Ridge));
        w.add_player(Player::new(1, 1, "b".into(), Character::Vane, Weapon::Sting));
        w.players[0].mv.pos = v3(0.0, 0.1, 0.0);
        w.players[1].mv.pos = v3(0.0, 0.1, 10.0);
        for _ in 0..4 {
            w.step();
        }
        w.players[0].rewind_ticks = 0;
        // Aim at the head box of the target ten metres down +Z.
        let shooter_eye = w.players[0].eye();
        let target_head = v3(0.0, 0.1 + 1.6, 10.0);
        let d = target_head.sub(shooter_eye);
        w.players[0].yaw = atan2(d.x, d.z);
        w.players[0].pitch = asin(d.normalized().y);
        w.set_input(
            0,
            Input {
                seq: 1,
                yaw: w.players[0].yaw,
                pitch: w.players[0].pitch,
                buttons: buttons::FIRE,
            },
        );
        w.step();
        assert!(!w.players[1].alive, "one Ridge head shot must kill");
        assert_eq!(w.players[0].stats.headshot_kills, 1);
        assert!(w.players[0].stats.score >= 175);
    }

    #[test]
    fn ward_armour_is_ignored_by_head_shots() {
        let ward = Character::Ward;
        assert_eq!(ward.armor(), 25);
        let mut w = world();
        w.add_player(Player::new(0, 0, "a".into(), Character::Vane, Weapon::Sting));
        w.add_player(Player::new(1, 1, "b".into(), ward, Weapon::Sting));
        w.players[0].mv.pos = v3(0.0, 0.1, 0.0);
        w.players[1].mv.pos = v3(0.0, 0.1, 6.0);
        w.step();
        w.apply_damage(0, 1, 14.0, false, 6.0, false);
        assert_eq!(w.players[1].armor, 11, "body damage eats armour first");
        assert_eq!(w.players[1].health, 100);
        w.apply_damage(0, 1, 25.0, true, 6.0, false);
        assert_eq!(w.players[1].armor, 11, "head shots bypass armour");
        assert_eq!(w.players[1].health, 75);
    }

    #[test]
    fn headhunt_body_damage_staggers_instead_of_killing() {
        let mut w = World::new(MatchConfig {
            mode: Mode::Headhunt,
            map: MapId::Vault,
            weather: Weather::Clear,
            seed: 7,
        });
        w.add_player(Player::new(0, 0, "a".into(), Character::Vane, Weapon::Ridge));
        w.add_player(Player::new(1, 1, "b".into(), Character::Vane, Weapon::Ridge));
        w.step();
        w.apply_damage(0, 1, 300.0, false, 5.0, false);
        assert!(w.players[1].alive, "body damage must not kill in Headhunt");
        assert!(w.players[1].staggered);
        assert_eq!(w.players[1].health, 1);
        w.apply_damage(0, 1, 100.0, true, 5.0, false);
        assert!(!w.players[1].alive, "a head shot finishes a staggered target");
    }

    #[test]
    fn loadout_swaps_only_while_dead_and_blade_knifes_from_the_front() {
        let mut w = world();
        w.add_player(Player::new(0, 0, "a".into(), Character::Vane, Weapon::Sting));
        w.add_player(Player::new(1, 1, "b".into(), Character::Vane, Weapon::Sting));
        w.step();
        assert!(!w.set_loadout(0, Weapon::Blade), "a live player cannot swap");
        assert_eq!(w.players[0].weapon, Weapon::Sting);
        w.apply_damage(1, 0, 300.0, true, 5.0, false);
        assert!(!w.players[0].alive);
        assert!(w.set_loadout(0, Weapon::Blade), "a dead player can swap");
        assert_eq!(w.players[0].weapon, Weapon::Blade);

        // A frontal Blade melee kills a full-health target outright.
        w.players[1].mv.pos = w.players[0].mv.pos.add(v3(0.0, 0.0, 1.5));
        w.players[0].yaw = 0.0;
        // Victim faces the attacker, so this is a front hit, not an execution.
        w.players[1].yaw = 3.14;
        w.players[0].alive = true;
        w.players[0].health = MAX_HEALTH;
        w.resolve_melee(0);
        assert!(!w.players[1].alive, "a front Blade hit must kill");
    }

    #[test]
    fn no_spawn_point_sits_inside_geometry() {
        for id in [MapId::Vault, MapId::Depot, MapId::Terrace, MapId::Substation] {
            let map = load_map(id);
            for (label, list) in [("A", &map.spawns_a), ("B", &map.spawns_b)] {
                for (i, s) in list.iter().enumerate() {
                    assert!(
                        !spot_blocked(*s, &map.brushes),
                        "{} spawn {}{} is inside a brush",
                        map.id.name(),
                        label,
                        i
                    );
                }
            }
            for t in &map.terminals {
                assert!(
                    !spot_blocked(*t, &map.brushes),
                    "{} has a terminal inside a brush",
                    map.id.name()
                );
            }
        }
    }

    #[test]
    fn team_spawns_do_not_stack_two_players_in_one_place() {
        let mut w = World::new(MatchConfig {
            mode: Mode::Skirmish,
            map: MapId::Vault,
            weather: Weather::Clear,
            seed: 3,
        });
        for slot in 0..MAX_PLAYERS as u8 {
            w.add_player(Player::new(
                slot,
                slot % 2,
                format!("p{slot}"),
                Character::Vane,
                Weapon::Sting,
            ));
        }
        for i in 0..w.players.len() {
            for j in (i + 1)..w.players.len() {
                let d = w.players[i].mv.pos.sub(w.players[j].mv.pos).len();
                assert!(d > 1.0, "players {i} and {j} spawned on top of each other");
            }
        }
    }

    #[test]
    fn schedule_respects_its_own_rules() {
        for seed in 0..200u64 {
            let mut rng = Rng::new(seed);
            let s = build_schedule(&mut rng, Mode::Skirmish);
            let dur = Mode::Skirmish.duration();
            assert!(s.len() <= 4);
            assert!(
                s.iter().filter(|e| e.kind == StaticEvent::OvertimeCoin).count() == 1,
                "exactly one closing event"
            );
            for e in &s {
                assert!(e.fires_at >= 60.0, "no event before 60 s");
                assert!(e.fires_at <= dur - 30.0 + 0.001, "nothing lands inside the closing window");
                assert!((e.fires_at - e.telegraph_at - 5.0).abs() < 1e-4);
            }
            let mut times: Vec<f32> = s.iter().map(|e| e.fires_at).collect();
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for w in times.windows(2) {
                assert!(w[1] - w[0] >= MIN_EVENT_GAP - 0.001, "events must not bunch up");
            }
        }
    }

    #[test]
    fn simulation_is_deterministic_from_its_seed() {
        let run = || {
            let mut w = World::new(MatchConfig {
                mode: Mode::Skirmish,
                map: MapId::Vault,
                weather: Weather::Clear,
                seed: 99,
            });
            w.add_player(Player::new(0, 0, "a".into(), Character::Kiln, Weapon::Maul));
            w.add_player(Player::new(1, 1, "b".into(), Character::Ward, Weapon::Sting));
            for t in 0..600 {
                let yaw = t as f32 * 0.01;
                w.set_input(
                    0,
                    Input {
                        seq: t,
                        yaw,
                        pitch: 0.0,
                        buttons: buttons::FWD | buttons::FIRE,
                    },
                );
                w.set_input(
                    1,
                    Input {
                        seq: t,
                        yaw: -yaw,
                        pitch: 0.0,
                        buttons: buttons::FWD | buttons::JUMP,
                    },
                );
                w.step();
            }
            (
                w.players[0].mv.pos,
                w.players[1].mv.pos,
                w.players[0].stats.shots_fired,
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_zero_damage_pellet_is_not_counted_as_a_hit() {
        let mut w = world();
        w.add_player(Player::new(0, 0, "a".into(), Character::Vane, Weapon::Maul));
        w.add_player(Player::new(1, 1, "b".into(), Character::Vane, Weapon::Sting));
        w.players[0].mv.pos = v3(0.0, 0.1, 0.0);
        w.players[1].mv.pos = v3(0.0, 0.1, 18.0);
        for _ in 0..4 {
            w.step();
        }
        w.players[0].rewind_ticks = 0;
        let eye = w.players[0].eye();
        let d = v3(0.0, 0.1 + 1.0, 18.0).sub(eye);
        w.players[0].yaw = atan2(d.x, d.z);
        w.players[0].pitch = asin(d.normalized().y);
        w.set_input(
            0,
            Input {
                seq: 1,
                yaw: w.players[0].yaw,
                pitch: w.players[0].pitch,
                buttons: buttons::FIRE,
            },
        );
        w.step();
        assert_eq!(w.players[0].stats.shots_fired, 1);
        assert_eq!(
            w.players[0].stats.shots_hit, 0,
            "Maul past its falloff does no damage, so it lands no hits"
        );
        assert_eq!(w.players[1].health, MAX_HEALTH);
    }

    #[test]
    fn falloff_matches_the_balance_table() {
        let sting = Weapon::Sting.stats();
        assert!((sting.body * sting.falloff(10.0) - 14.0).abs() < 0.01);
        assert!((sting.body * sting.falloff(30.0) - 9.0).abs() < 0.01);
        let maul = Weapon::Maul.stats();
        assert_eq!(maul.pellets as f32 * maul.body * maul.falloff(5.0), 108.0);
        assert_eq!(maul.falloff(12.0), 0.0);
    }
}
