import { invoke } from "@tauri-apps/api/core";
export type NamedColor =
  | "default"
  | "black"
  | "red"
  | "green"
  | "yellow"
  | "blue"
  | "magenta"
  | "cyan"
  | "white";
export type Color =
  | NamedColor
  | { kind: "indexed"; index: number }
  | { kind: "rgb"; red: number; green: number; blue: number };
export interface Style {
  foreground: Color;
  background: Color;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  inverse: boolean;
}
export type Output =
  | { type: "text"; text: string }
  | { type: "style"; style: Style }
  | {
      type: "control";
      control: "CarriageReturn" | "LineFeed" | "Tab" | "Backspace" | "Bell";
    };
export type Phase =
  | "idle"
  | "resolving"
  | "connecting"
  | "connected"
  | "disconnecting"
  | "closed";
export interface Snapshot {
  id: string;
  phase: Phase;
  message: string;
  events: Output[];
  finished: boolean;
  remoteEcho: boolean;
  maskingGeneration: string;
}
export interface ViewportSize {
  columns: number;
  rows: number;
}
export interface Bridge {
  start(host: string, port: number): Promise<string>;
  poll(id: string): Promise<Snapshot>;
  send(id: string, text: string): Promise<void>;
  disconnect(id: string): Promise<void>;
  viewport(id: string, size: ViewportSize): Promise<void>;
}
export const bridge: Bridge = {
  start: (host, port) => invoke("start_connection", { host, port }),
  poll: (sessionId) => invoke("poll_connection", { sessionId }),
  send: (sessionId, text) => invoke("send_line", { sessionId, text }),
  viewport: (sessionId, size) =>
    invoke("update_viewport", { sessionId, ...size }),
  disconnect: (sessionId) => invoke("disconnect", { sessionId }),
};
export function message(error: unknown): string {
  return typeof error === "string"
    ? error
    : "The desktop connection failed. Please reconnect.";
}

/** A single poll chain. Late responses cannot update a replacement/unmounted view. */
export function pollConnection(
  api: Bridge,
  id: string,
  receive: (value: Snapshot) => void,
  failed: (message: string) => void,
): () => void {
  let stopped = false;
  let frame = 0;
  const poll = async () => {
    try {
      const value = await api.poll(id);
      if (stopped) return;
      if (value.id !== id) throw new Error("stale response");
      receive(value);
      if (!value.finished)
        frame = requestAnimationFrame(() => {
          void poll();
        });
    } catch (error) {
      if (stopped) return;
      stopped = true;
      failed(message(error));
      try {
        await api.disconnect(id);
      } catch {
        /* Original bridge error remains visible. */
      }
    }
  };
  void poll();
  return () => {
    stopped = true;
    cancelAnimationFrame(frame);
  };
}
