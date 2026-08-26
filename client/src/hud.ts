// HUD.
//
// The rule for everything here: it earns its pixels by telling you something
// changed. Numbers that never move (mode name, callsigns) sit small and dim;
// the things you act on — health, ammo, the event that is about to land —
// are the only elements allowed to move or colour.

const WEAPON_NAMES = ["Sting", "Ridge", "Maul", "Arc", "Tack", "Lance", "Blade"];
const CHARACTER_NAMES = ["Ward", "Vane", "Echo", "Kiln"];
const ABILITY_NAMES = ["Shimmer", "Blink", "Pulse", "Cinderline"];

const el = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`missing element #${id}`);
  return found as T;
};

export class Hud {
  private hitmarkerTimer = 0;

  readonly root = el("hud");
  private crosshair = el("crosshair");
  private hitmarker = el("hitmarker");
  private scoreA = el("scoreA");
  private scoreB = el("scoreB");
  private clock = el("clock");
  private modeName = el("modeName");
  private eventBanner = el("eventBanner");
  private eventName = el("eventName");
  private eventBlurb = el("eventBlurb");
  private killfeed = el("killfeed");
  private bonuses = el("bonuses");
  private healthFill = el("healthFill");
  private armorFill = el("armorFill");
  private healthNum = el("healthNum");
  private armorNum = el("armorNum");
  private charName = el("charName");
  private abilityState = el("abilityState");
  private chargeFill = el("chargeFill");
  private ammoNum = el("ammoNum");
  private weaponName = el("weaponName");
  private netreadout = el("netreadout");

  show() { this.root.classList.remove("hidden"); }
  hide() { this.root.classList.add("hidden"); }

  setMode(name: string) { this.modeName.textContent = name; }

  setScores(a: number, b: number) {
    this.scoreA.textContent = String(a);
    this.scoreB.textContent = String(b);
  }

  setClock(secondsLeft: number) {
    const s = Math.max(0, Math.floor(secondsLeft));
    const m = Math.floor(s / 60);
    this.clock.textContent = `${m}:${String(s % 60).padStart(2, "0")}`;
    this.clock.classList.toggle("urgent", s <= 30);
  }

  /** Being dead is announced by the death overlay, not by the status bar. */
  setVitals(health: number, armor: number) {
    this.healthFill.style.width = `${Math.max(0, Math.min(100, health))}%`;
    this.armorFill.style.width = `${Math.max(0, Math.min(100, (armor / 25) * 100))}%`;
    this.armorFill.parentElement!.style.opacity = armor > 0 ? "1" : "0.25";
    this.healthNum.textContent = String(Math.max(0, health));
    this.armorNum.textContent = armor > 0 ? `+${armor}` : "";
  }

  setKit(character: number, abilityCooldown: number) {
    this.charName.textContent = CHARACTER_NAMES[character] ?? "";
    const ready = abilityCooldown <= 0;
    this.abilityState.textContent = ready
      ? `${ABILITY_NAMES[character]} ready`
      : `${ABILITY_NAMES[character]} ${abilityCooldown.toFixed(1)}s`;
    this.abilityState.classList.toggle("ready", ready);
  }

  setCharge(charge: number, focusing: boolean) {
    this.chargeFill.style.width = `${Math.round(charge * 100)}%`;
    this.chargeFill.parentElement!.classList.toggle("full", charge >= 1);
    this.crosshair.classList.toggle("focus", focusing);
  }

  setWeapon(weapon: number, ammo: number, reloading: boolean) {
    this.weaponName.textContent = reloading
      ? "Reloading"
      : WEAPON_NAMES[weapon] ?? "";
    // The Blade has no ammunition to count.
    if (weapon === 6) {
      this.ammoNum.textContent = "—";
      this.ammoNum.classList.remove("empty");
    } else {
      this.ammoNum.textContent = String(Math.max(0, ammo));
      this.ammoNum.classList.toggle("empty", ammo <= 0);
    }
  }

  setNet(rtt: number, fps: number, interpMs: number) {
    this.netreadout.textContent =
      `${Math.round(rtt)} ms rtt · ${Math.round(interpMs)} ms interp · ${Math.round(fps)} fps`;
  }

  /**
   * Floating damage number at the hit's screen position (crosshair when the
   * hit point is unknown). Headshots read gold and larger.
   */
  damageNumber(amount: number, headshot: boolean, at: { x: number; y: number } | null) {
    const span = document.createElement("span");
    span.className = headshot ? "dmgNum head" : "dmgNum";
    span.textContent = String(amount);
    const x = (at?.x ?? window.innerWidth / 2) + (Math.random() - 0.5) * 30;
    const y = (at?.y ?? window.innerHeight / 2 - 30) + (Math.random() - 0.5) * 14;
    span.style.left = `${x}px`;
    span.style.top = `${y}px`;
    this.root.appendChild(span);
    window.setTimeout(() => span.remove(), 750);
  }

  hitmark(headshot: boolean) {
    this.hitmarker.classList.add("on");
    this.hitmarker.classList.toggle("head", headshot);
    this.hitmarkerTimer = 0.12;
  }

  tick(dt: number) {
    if (this.hitmarkerTimer > 0) {
      this.hitmarkerTimer -= dt;
      if (this.hitmarkerTimer <= 0) this.hitmarker.classList.remove("on");
    }
  }

  /** A short one-line notice: scanned, ghost ping, mode prompt. */
  warn(text: string) {
    const row = document.createElement("div");
    row.className = "bonusRow";
    row.textContent = text;
    this.bonuses.appendChild(row);
    window.setTimeout(() => row.remove(), 1500);
  }

  /** Five second warning, then the event itself. */
  telegraph(name: string, blurb: string) {
    this.eventBanner.classList.remove("hidden");
    this.eventBanner.classList.add("telegraph");
    this.eventName.textContent = `${name} incoming`;
    this.eventBlurb.textContent = blurb;
  }

  eventStart(name: string, blurb: string, underdog: boolean) {
    this.eventBanner.classList.remove("hidden", "telegraph");
    this.eventName.textContent = name;
    this.eventBlurb.textContent = underdog ? `${blurb}  ·  favours the trailing team` : blurb;
  }

  eventEnd() {
    this.eventBanner.classList.add("hidden");
  }

  killRow(attacker: string, victim: string, weapon: number, headshot: boolean, mine: boolean) {
    const row = document.createElement("div");
    row.className = mine ? "feedRow mine" : "feedRow";
    row.innerHTML =
      `<span class="who">${escape(attacker)}</span> ` +
      `<span class="${headshot ? "hs" : ""}">${headshot ? "◎" : "—"} ${WEAPON_NAMES[weapon] ?? ""}</span> ` +
      `<span class="who">${escape(victim)}</span>`;
    this.killfeed.appendChild(row);
    window.setTimeout(() => row.remove(), 6000);
    while (this.killfeed.children.length > 6) this.killfeed.firstChild?.remove();
  }

  /** The points readout after a kill: the base, then each bonus by name. */
  bonusPopup(points: number, labels: string[]) {
    const total = document.createElement("div");
    total.className = "bonusRow big";
    total.textContent = `+${points}`;
    this.bonuses.appendChild(total);
    window.setTimeout(() => total.remove(), 1500);
    labels.forEach((label, i) => {
      window.setTimeout(() => {
        const row = document.createElement("div");
        row.className = "bonusRow";
        row.textContent = label;
        this.bonuses.appendChild(row);
        window.setTimeout(() => row.remove(), 1500);
      }, 90 * (i + 1));
    });
  }
}

function escape(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c] ?? c));
}
