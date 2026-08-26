// Audio.
//
// Positional accuracy is a competitive requirement in a shooter, not polish,
// so every world sound goes through an HRTF panner. Everything is synthesised
// at runtime — no samples to download, which keeps the whole client inside its
// download budget. Nothing plays before the first user gesture, and the master
// volume starts low, because this is a game played at a desk with other people
// nearby.

const WEAPON_TONE = [
  { freq: 240, decay: 0.09, noise: 0.7 }, // Sting
  { freq: 130, decay: 0.24, noise: 0.5 }, // Ridge
  { freq: 90, decay: 0.3, noise: 0.9 },   // Maul
  { freq: 420, decay: 0.14, noise: 0.3 }, // Arc
  { freq: 300, decay: 0.1, noise: 0.6 },  // Tack
  { freq: 110, decay: 0.3, noise: 0.5 },  // Lance
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

    const panner = this.ctx.createPanner();
    panner.panningModel = "HRTF";
    panner.distanceModel = "inverse";
    panner.refDistance = 3;
    panner.maxDistance = 90;
    panner.rolloffFactor = 1.1;
    panner.positionX.value = at[0];
    panner.positionY.value = at[1];
    panner.positionZ.value = at[2];
    panner.connect(this.master);

    const l = this.ctx.listener;
    l.positionX.value = listener[0];
    l.positionY.value = listener[1];
    l.positionZ.value = listener[2];
    l.forwardX.value = Math.sin(yaw);
    l.forwardZ.value = Math.cos(yaw);
    l.upY.value = 1;

    const gain = this.ctx.createGain();
    gain.gain.setValueAtTime(0.9, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + tone.decay);
    gain.connect(panner);

    const osc = this.ctx.createOscillator();
    osc.type = "square";
    osc.frequency.setValueAtTime(tone.freq, t);
    osc.frequency.exponentialRampToValueAtTime(tone.freq * 0.35, t + tone.decay);
    osc.connect(gain);
    osc.start(t);
    osc.stop(t + tone.decay);

    if (this.noise) {
      const src = this.ctx.createBufferSource();
      src.buffer = this.noise;
      const ng = this.ctx.createGain();
      ng.gain.setValueAtTime(tone.noise, t);
      ng.gain.exponentialRampToValueAtTime(0.001, t + tone.decay * 1.2);
      const filter = this.ctx.createBiquadFilter();
      filter.type = "bandpass";
      filter.frequency.value = 1400;
      src.connect(filter).connect(ng).connect(panner);
      src.start(t);
      src.stop(t + tone.decay * 1.2);
    }
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
