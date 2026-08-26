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
  KeyC: BTN.CROUCH,
  ShiftLeft: BTN.SPRINT,
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
  locked = false;

  /** Two presses of Escape put the game into Standby. */
  private lastEscape = 0;
  onStandby: () => void = () => {};
  onLockChange: (locked: boolean) => void = () => {};

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
    window.addEventListener("blur", () => {
      this.keys.clear();
      this.buttons = 0;
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
    if (!this.locked) {
      this.keys.clear();
      this.buttons = 0;
    }
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
    if (!this.locked) return;
    if (e.code === "Space") e.preventDefault();
    this.keys.add(e.code);
    this.recompute();
  };

  private onKeyUp = (e: KeyboardEvent) => {
    this.keys.delete(e.code);
    this.recompute();
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
    const scale = (this.buttons & BTN.ADS) !== 0 ? 0.7 : 1.0;
    this.yaw -= e.movementX * this.sensitivity * scale;
    this.pitch -= e.movementY * this.sensitivity * scale;
    if (this.pitch > 1.55) this.pitch = 1.55;
    if (this.pitch < -1.55) this.pitch = -1.55;
    while (this.yaw > Math.PI) this.yaw -= Math.PI * 2;
    while (this.yaw < -Math.PI) this.yaw += Math.PI * 2;
  };

  private recompute() {
    let b = this.buttons & (BTN.FIRE | BTN.ADS);
    for (const code of this.keys) {
      const bit = KEY_BUTTONS[code];
      if (bit) b |= bit;
    }
    this.buttons = b;
  }
}
