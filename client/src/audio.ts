// Audio.
//
// Positional accuracy is a competitive requirement in a shooter, not polish,
// so every world sound goes through an HRTF panner. Everything is synthesised
// at runtime — no samples to download, which keeps the whole client inside its
// download budget. Nothing plays before the first user gesture, and the master
// volume starts low, because this is a game played at a desk with other people
// nearby.

/**
 * Per-weapon synthesis recipe. Every shot is three layers: a low "thump"
 * (the pressure wave), a pitched "body" (the mechanism), and a filtered
 * noise "crack" (the report). Snipers add a decaying echo tail so a Ridge
 * or Lance shot sounds like it crosses the whole map.
 */
const WEAPON_TONE = [
  // Sting: tight automatic punch — short, mid-bright, snappy.
  { freq: 220, decay: 0.08, noise: 0.7, thump: 0.5, crackHz: 2600, tail: 0 },
  // Ridge: the sniper. Sharp high crack, deep boom, long rolling echo.
  { freq: 82, decay: 0.5, noise: 1.0, thump: 1.0, crackHz: 3600, tail: 0.5 },
  // Maul: shotgun — huge low boom, wide noise, no ring.
  { freq: 60, decay: 0.34, noise: 1.0, thump: 1.1, crackHz: 900, tail: 0 },
  // Arc: energy burst — synthetic zap, little noise.
  { freq: 520, decay: 0.16, noise: 0.25, thump: 0.3, crackHz: 4200, tail: 0 },
  // Tack: sidearm pop.
  { freq: 300, decay: 0.09, noise: 0.6, thump: 0.45, crackHz: 2200, tail: 0 },
  // Lance: heavy rifle — like Ridge but deeper.
  { freq: 70, decay: 0.55, noise: 1.0, thump: 1.1, crackHz: 2800, tail: 0.6 },
  // Blade: no report (melee uses swing()).
  { freq: 0, decay: 0.05, noise: 0, thump: 0, crackHz: 0, tail: 0 },
];

export class Audio {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private noise: AudioBuffer | null = null;
  private muted = false;

  start() {
    if (this.ctx) return;
    this.ctx = new AudioContext();
    this.master = this.ctx.createGain();
    this.master.gain.value = 0.4;
    this.master.connect(this.ctx.destination);

    const len = Math.floor(this.ctx.sampleRate * 0.4);
    const buf = this.ctx.createBuffer(1, len, this.ctx.sampleRate);
    const data = buf.getChannelData(0);
    for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
    this.noise = buf;
  }

  setMuted(muted: boolean) {
    this.muted = muted;
    if (this.master) this.master.gain.value = muted ? 0 : 0.4;
  }

  /** A gunshot at a world position, heard through HRTF. */
  shot(weapon: number, at: [number, number, number], listener: [number, number, number], yaw: number) {
    if (!this.ctx || !this.master || this.muted) return;
    const t = this.ctx.currentTime;
    const tone = WEAPON_TONE[weapon] ?? WEAPON_TONE[0];
    if (tone.thump === 0 && tone.noise === 0) return; // Blade: no report

    const panner = this.spatial(at, listener, yaw);

    // Layer 1 — thump: a pitch-dropping sine, the pressure wave you feel.
    if (tone.thump > 0) {
      const gain = this.ctx.createGain();
      gain.gain.setValueAtTime(tone.thump, t);
      gain.gain.exponentialRampToValueAtTime(0.001, t + tone.decay * 0.8);
      gain.connect(panner);
      const osc = this.ctx.createOscillator();
      osc.type = "sine";
      osc.frequency.setValueAtTime(Math.max(50, tone.freq * 1.6), t);
      osc.frequency.exponentialRampToValueAtTime(38, t + tone.decay * 0.8);
      osc.connect(gain);
      osc.start(t);
      osc.stop(t + tone.decay * 0.8);
    }

    // Layer 2 — body: the mechanism's pitched bark.
    if (tone.freq > 0) {
      const gain = this.ctx.createGain();
      gain.gain.setValueAtTime(0.55, t);
      gain.gain.exponentialRampToValueAtTime(0.001, t + tone.decay * 0.6);
      gain.connect(panner);
      const osc = this.ctx.createOscillator();
      osc.type = weapon === 3 ? "sawtooth" : "square";
      osc.frequency.setValueAtTime(tone.freq * 2.2, t);
      osc.frequency.exponentialRampToValueAtTime(tone.freq * 0.6, t + tone.decay * 0.6);
      osc.connect(gain);
      osc.start(t);
      osc.stop(t + tone.decay * 0.6);
    }

    // Layer 3 — crack: filtered noise burst whose filter sweeps down, which
    // is what makes it read as a report rather than static.
    if (this.noise && tone.noise > 0) {
      const src = this.ctx.createBufferSource();
      src.buffer = this.noise;
      const ng = this.ctx.createGain();
      ng.gain.setValueAtTime(tone.noise, t);
      ng.gain.exponentialRampToValueAtTime(0.001, t + tone.decay);
      const filter = this.ctx.createBiquadFilter();
      filter.type = weapon === 2 ? "lowpass" : "bandpass";
      filter.frequency.setValueAtTime(tone.crackHz, t);
      filter.frequency.exponentialRampToValueAtTime(Math.max(300, tone.crackHz * 0.25), t + tone.decay);
      filter.Q.value = 0.8;
      src.connect(filter).connect(ng).connect(panner);
      src.start(t);
      src.stop(t + tone.decay);
    }

    // Layer 4 — echo tail for the long rifles: two delayed, muffled copies
    // of the crack, the sound bouncing off the far walls.
    if (this.noise && tone.tail > 0) {
      for (const [delay, level] of [[0.16, 0.3], [0.34, 0.14]] as const) {
        const src = this.ctx.createBufferSource();
        src.buffer = this.noise;
        const eg = this.ctx.createGain();
        eg.gain.setValueAtTime(0.0001, t);
        eg.gain.setValueAtTime(level * tone.tail, t + delay);
        eg.gain.exponentialRampToValueAtTime(0.001, t + delay + tone.tail);
        const lp = this.ctx.createBiquadFilter();
        lp.type = "lowpass";
        lp.frequency.value = 900 - delay * 1200;
        src.connect(lp).connect(eg).connect(panner);
        src.start(t + delay);
        src.stop(t + delay + tone.tail);
      }
    }
  }

  /** Melee swing: a whoosh of band-swept noise. Played locally on the key. */
  swing() {
    if (!this.ctx || !this.master || this.muted || !this.noise) return;
    const t = this.ctx.currentTime;
    const src = this.ctx.createBufferSource();
    src.buffer = this.noise;
    const gain = this.ctx.createGain();
    gain.gain.setValueAtTime(0.0001, t);
    gain.gain.exponentialRampToValueAtTime(0.4, t + 0.06);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.22);
    const filter = this.ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.setValueAtTime(420, t);
    filter.frequency.exponentialRampToValueAtTime(2400, t + 0.16);
    filter.Q.value = 1.6;
    src.connect(filter).connect(gain).connect(this.master);
    src.start(t);
    src.stop(t + 0.25);
  }

  /** Shared HRTF panner setup for world-positioned sounds. */
  private spatial(
    at: [number, number, number],
    listener: [number, number, number],
    yaw: number,
  ): PannerNode {
    const ctx = this.ctx!;
    const panner = ctx.createPanner();
    panner.panningModel = "HRTF";
    panner.distanceModel = "inverse";
    panner.refDistance = 3;
    panner.maxDistance = 90;
    panner.rolloffFactor = 1.1;
    panner.positionX.value = at[0];
    panner.positionY.value = at[1];
    panner.positionZ.value = at[2];
    panner.connect(this.master!);

    const l = ctx.listener;
    l.positionX.value = listener[0];
    l.positionY.value = listener[1];
    l.positionZ.value = listener[2];
    l.forwardX.value = Math.sin(yaw);
    l.forwardZ.value = Math.cos(yaw);
    l.upY.value = 1;
    return panner;
  }

  /** Non-positional interface sounds: hitmarker, event telegraph, spawn. */
  blip(freq: number, decay = 0.08, type: OscillatorType = "sine", gainValue = 0.35) {
    if (!this.ctx || !this.master || this.muted) return;
    const t = this.ctx.currentTime;
    const gain = this.ctx.createGain();
    gain.gain.setValueAtTime(gainValue, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + decay);
    gain.connect(this.master);
    const osc = this.ctx.createOscillator();
    osc.type = type;
    osc.frequency.value = freq;
    osc.connect(gain);
    osc.start(t);
    osc.stop(t + decay);
  }

  hitmarker(headshot: boolean) {
    this.blip(headshot ? 1400 : 900, 0.06, "square", 0.25);
  }

  /** The five second warning that precedes every Static Event. */
  telegraph() {
    this.blip(320, 0.5, "sawtooth", 0.22);
    window.setTimeout(() => this.blip(240, 0.6, "sawtooth", 0.2), 160);
  }
}
