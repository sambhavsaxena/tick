// Raw mouse input and key state.
//
// Pointer Lock with unfiltered movement deltas: no smoothing, no acceleration,
// and a sensitivity number that accepts values copied straight out of another
// shooter.

import { BTN } from "./proto";

const KEY_BUTTONS: Record<string, number> = {
  KeyW: BTN.FWD,
  KeyS: BTN.BACK,
  KeyA: BTN.LEFT,
  KeyD: BTN.RIGHT,
  Space: BTN.JUMP,
  ControlLeft: BTN.CROUCH,
  ControlRight: BTN.CROUCH,
  KeyC: BTN.CROUCH,
  ShiftLeft: BTN.SPRINT,
  ShiftRight: BTN.SPRINT,
  KeyR: BTN.RELOAD,
  KeyQ: BTN.ABILITY,
  KeyF: BTN.ABILITY,
  KeyV: BTN.MELEE,
};

export class InputState {
  yaw = 0;
  pitch = 0;
  buttons = 0;
  sensitivity = 0.0022;
  /** ADS sensitivity multiplier; the scope sets it lower than iron sights. */
  adsSensScale = 0.7;
  locked = false;

  /** Two presses of Escape put the game into Standby. */
  private lastEscape = 0;
  onStandby: () => void = () => {};
  onLockChange: (locked: boolean) => void = () => {};
  /** Number row 1-4, fired even without pointer lock: the dead pick loadouts. */
  onWeaponKey: (n: number) => void = () => {};

  private keys = new Set<string>();
  private canvas: HTMLElement;

  constructor(canvas: HTMLElement) {
    this.canvas = canvas;
    window.addEventListener("keydown", this.onKeyDown);
    window.addEventListener("keyup", this.onKeyUp);
    window.addEventListener("mousedown", this.onMouseDown);
    window.addEventListener("mouseup", this.onMouseUp);
    window.addEventListener("mousemove", this.onMouseMove);
    document.addEventListener("pointerlockchange", this.onPointerLockChange);
    // Anything that stops key events reaching the page must also drop the
    // keys already held, or the player walks into a wall forever: a lost
    // focus, a hidden tab, or a window the compositor took away.
    window.addEventListener("blur", () => this.clearKeys());
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.clearKeys();
    });
  }

  requestLock() {
    // unadjustedMovement bypasses OS pointer acceleration: raw deltas, so a
    // given hand motion always turns the same amount. Not every browser
    // supports the option (or returns a promise), so fall back to the plain
    // call. Failures are expected — the browser refuses requests made outside
    // a user gesture and within ~1.3 s of an Escape unlock — and are safe to
    // swallow: the click-to-recapture handler retries on the next gesture.
    let result: unknown;
    try {
      result = (this.canvas as any).requestPointerLock({ unadjustedMovement: true });
    } catch {
      result = this.canvas.requestPointerLock();
    }
    if (result instanceof Promise) {
      result.catch(() => {
        try {
          const plain = this.canvas.requestPointerLock() as unknown;
          if (plain instanceof Promise) plain.catch(() => {});
        } catch {
          /* recaptured on the next click */
        }
      });
    }
  }

  releaseLock() {
    if (document.pointerLockElement) document.exitPointerLock();
  }

  private onPointerLockChange = () => {
    this.locked = document.pointerLockElement === this.canvas;
    if (!this.locked) this.clearKeys();
    this.onLockChange(this.locked);
  };

  private onKeyDown = (e: KeyboardEvent) => {
    if (e.code === "Escape") {
      const now = performance.now();
      if (now - this.lastEscape < 600) {
        this.lastEscape = 0;
        this.onStandby();
      } else {
        this.lastEscape = now;
      }
      return;
    }
    if (e.code.startsWith("Digit")) {
      const n = Number(e.code.slice(5));
      if (n >= 1 && n <= 4) this.onWeaponKey(n);
    }
    if (!this.locked) return;
    // Every bound key is claimed outright while the pointer is locked. Space
    // scrolls, and Control and C are the first half of a browser shortcut on
    // every platform — Ctrl+W, Ctrl+D, Ctrl+C and the rest. Left to itself the
    // browser acts on the combination and the game never sees a clean crouch.
    if (KEY_BUTTONS[e.code] !== undefined) e.preventDefault();
    this.keys.add(e.code);
    this.recompute(e);
  };

  private onKeyUp = (e: KeyboardEvent) => {
    this.keys.delete(e.code);
    this.recompute(e);
  };

  private onMouseDown = (e: MouseEvent) => {
    if (!this.locked) return;
    if (e.button === 0) this.buttons |= BTN.FIRE;
    if (e.button === 2) this.buttons |= BTN.ADS;
  };

  private onMouseUp = (e: MouseEvent) => {
    if (e.button === 0) this.buttons &= ~BTN.FIRE;
    if (e.button === 2) this.buttons &= ~BTN.ADS;
  };

  private onMouseMove = (e: MouseEvent) => {
    if (!this.locked) return;
    // Aiming down sights scales sensitivity with the zoom, so muscle memory
    // survives the transition.
    const scale = (this.buttons & BTN.ADS) !== 0 ? this.adsSensScale : 1.0;
    this.yaw -= e.movementX * this.sensitivity * scale;
    this.pitch -= e.movementY * this.sensitivity * scale;
    if (this.pitch > 1.55) this.pitch = 1.55;
    if (this.pitch < -1.55) this.pitch = -1.55;
    while (this.yaw > Math.PI) this.yaw -= Math.PI * 2;
    while (this.yaw < -Math.PI) this.yaw += Math.PI * 2;
  };

  /**
   * Rebuild the button mask from the keys currently held.
   *
   * Modifiers get a second source of truth. A key event carries the live
   * state of Control and Shift in `ctrlKey` / `shiftKey`, and that state is
   * correct even when the keydown or the keyup itself went missing — which is
   * exactly what happens when a modifier combination is swallowed by the
   * browser or the window loses focus mid-chord. Without this, one lost event
   * leaves crouch stuck on, or off, until the key is pressed again.
   */
  private recompute(e?: KeyboardEvent) {
    let b = this.buttons & (BTN.FIRE | BTN.ADS);
    for (const code of this.keys) {
      const bit = KEY_BUTTONS[code];
      if (bit) b |= bit;
    }
    if (e) {
      if (e.ctrlKey) {
        b |= BTN.CROUCH;
      } else {
        this.keys.delete("ControlLeft");
        this.keys.delete("ControlRight");
        if (!this.keys.has("KeyC")) b &= ~BTN.CROUCH;
      }
      if (e.shiftKey) {
        b |= BTN.SPRINT;
      } else {
        this.keys.delete("ShiftLeft");
        this.keys.delete("ShiftRight");
        b &= ~BTN.SPRINT;
      }
    }
    this.buttons = b;
  }

  /** Drop every held key. Used whenever the page stops receiving key events. */
  clearKeys() {
    this.keys.clear();
    this.buttons = 0;
  }
}
