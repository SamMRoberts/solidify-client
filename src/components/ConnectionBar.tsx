import type { Phase } from "../bridge/client";
export function ConnectionBar({
  host,
  port,
  phase,
  busy,
  onHost,
  onPort,
  onConnect,
  onDisconnect,
  onClear,
}: {
  host: string;
  port: string;
  phase: Phase;
  busy: boolean;
  onHost: (v: string) => void;
  onPort: (v: string) => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onClear: () => void;
}) {
  const active = busy || !["idle", "closed"].includes(phase);
  return (
    <form
      className="connection-bar"
      onSubmit={(e) => {
        e.preventDefault();
        if (!active) onConnect();
      }}
    >
      <div className="brand">
        solidify<span>MUD client</span>
      </div>
      <label className="host">
        Host
        <input
          value={host}
          onChange={(e) => onHost(e.target.value)}
          disabled={active}
          autoCapitalize="none"
          spellCheck={false}
        />
      </label>
      <label className="port">
        Port
        <input
          value={port}
          onChange={(e) => onPort(e.target.value)}
          disabled={active}
          inputMode="numeric"
        />
      </label>
      {active ? (
        <button
          type="button"
          onClick={onDisconnect}
          disabled={busy || phase === "disconnecting"}
        >
          {phase === "connected"
            ? "Disconnect"
            : phase === "disconnecting"
              ? "Disconnecting…"
              : "Cancel"}
        </button>
      ) : (
        <button className="primary" type="submit">
          Connect
        </button>
      )}
      <button type="button" className="quiet" onClick={onClear}>
        Clear output
      </button>
    </form>
  );
}
