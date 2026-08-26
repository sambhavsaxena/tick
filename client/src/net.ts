// Transport.
//
// WebSocket today. The design calls for WebTransport over HTTP/3 with
// unreliable datagrams as the primary path and this as the fallback; the
// fallback is what ships first so it is never a bolt-on. Head-of-line blocking
// on TCP is real, so the client measures round-trip time and widens its own
// interpolation buffer to match, and reports that width to the server with
// every input packet so lag compensation rewinds by the right amount.

import { decodeSnapshot, encodeInput, type InputCmd, type Snapshot } from "./proto";

export type JsonHandler = (msg: any) => void;
export type SnapHandler = (snap: Snapshot) => void;

export class Net {
  private ws: WebSocket | null = null;
  private pingId = 0;
  private pingSentAt = new Map<number, number>();
  private pingTimer: number | null = null;

  /** Smoothed round-trip time in milliseconds. */
  rtt = 60;
  connected = false;
  onJson: JsonHandler = () => {};
  onSnapshot: SnapHandler = () => {};
  onClose: () => void = () => {};

  connect() {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const url = `${proto}://${location.host}/ws`;
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    this.ws = ws;

    ws.onopen = () => {
      this.connected = true;
      this.send({ t: "hello" });
      this.pingTimer = window.setInterval(() => this.ping(), 1000);
      this.ping();
    };
    ws.onclose = () => {
      this.connected = false;
      if (this.pingTimer !== null) window.clearInterval(this.pingTimer);
      this.onClose();
    };
    ws.onmessage = (ev) => {
      if (typeof ev.data === "string") {
        const msg = JSON.parse(ev.data);
        if (msg.t === "pong") {
          const sent = this.pingSentAt.get(msg.id);
          if (sent !== undefined) {
            this.pingSentAt.delete(msg.id);
            const sample = performance.now() - sent;
            // Exponential smoothing: one bad sample should not move the
            // interpolation buffer.
            this.rtt = this.rtt * 0.8 + sample * 0.2;
          }
          return;
        }
        this.onJson(msg);
      } else {
        const snap = decodeSnapshot(ev.data as ArrayBuffer);
        if (snap) this.onSnapshot(snap);
      }
    };
  }

  private ping() {
    const id = ++this.pingId;
    this.pingSentAt.set(id, performance.now());
    this.send({ t: "ping", id });
  }

  send(obj: unknown) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(obj));
    }
  }

  sendInput(recent: InputCmd[], interpTicks: number) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(encodeInput(recent, interpTicks));
    }
  }

  close() {
    this.ws?.close();
  }
}
