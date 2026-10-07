import { useEffect, useRef, useState } from "react";
import {
  bridge,
  message,
  pollConnection,
  type Bridge,
  type Phase,
  type ViewportSize,
} from "./bridge/client";
import { viewportDelivery } from "./bridge/viewport";
import { ConnectionBar } from "./components/ConnectionBar";
import { CommandInput } from "./components/CommandInput";
import { Transcript } from "./terminal/Transcript";
import { TranscriptModel } from "./terminal/model";
import { profilesBridge, type ProfilesBridge } from "./bridge/profiles";
import { useProfiles } from "./components/useProfiles";
import { ProfileControls } from "./components/ProfileControls";

export function App({
  api = bridge,
  profilesApi = profilesBridge,
}: {
  api?: Bridge;
  profilesApi?: ProfilesBridge;
}) {
  const profiles = useProfiles(profilesApi);
  const { host, port } = profiles;
  const [phase, setPhase] = useState<Phase>("idle"),
    [status, setStatus] = useState(
      "Ready. Start the local demo or enter a server address.",
    );
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0);
  const [id, setId] = useState<string | null>(null);
  const [options, setOptions] = useState({
    remoteEcho: false,
    maskingGeneration: "0",
  });
  const [viewport, setViewport] = useState<ViewportSize>({
    columns: 80,
    rows: 24,
  });
  const sizes = useRef<ReturnType<typeof viewportDelivery> | null>(null);
  useEffect(() => {
    if (!id) return;
    const delivery = viewportDelivery(api, id, (failure) => {
      if (current.current === id) setError(failure);
    });
    sizes.current = delivery;
    return () => {
      delivery.stop();
      sizes.current = null;
    };
  }, [api, id]);
  useEffect(() => {
    sizes.current?.update(viewport);
  }, [id, viewport]);
  const model = useRef(new TranscriptModel());
  const current = useRef<string | null>(null);
  const generation = useRef(0);
  useEffect(
    () => () => {
      generation.current++;
      const old = current.current;
      current.current = null;
      if (old) void api.disconnect(old).catch(() => {});
    },
    [api],
  );
  useEffect(() => {
    if (!id) return;
    return pollConnection(
      api,
      id,
      (value) => {
        if (current.current !== id) return;
        if (value.events.length) {
          model.current.apply(value.events);
          setRevision((v) => v + 1);
        }
        setOptions({
          remoteEcho: value.remoteEcho,
          maskingGeneration: value.maskingGeneration,
        });
        if (value.phase === "closed" || value.phase === "disconnecting")
          sizes.current?.stop();
        setPhase(value.phase);
        setStatus(value.message);
        setBusy(value.phase === "closed" && !value.finished);
      },
      (failure) => {
        if (current.current === id) {
          sizes.current?.stop();
          setError(failure);
          setPhase("closed");
          setStatus("Connection stopped.");
          setBusy(false);
        }
      },
    );
  }, [api, id]);
  async function start() {
    if (busy) return;
    if (!/^\d+$/.test(port) || Number(port) < 1 || Number(port) > 65535) {
      setError("Port must be between 1 and 65535.");
      return;
    }
    profiles.touch();
    const attempt = ++generation.current;
    setBusy(true);
    setError("");
    try {
      const next = await api.start(host, Number(port));
      if (generation.current !== attempt) {
        await api.disconnect(next);
        return;
      }
      current.current = next;
      setOptions({ remoteEcho: false, maskingGeneration: "0" });
      model.current = new TranscriptModel();
      setRevision((v) => v + 1);
      setId(next);
      setPhase("resolving");
      setStatus("Resolving…");
    } catch (error) {
      if (generation.current === attempt) setError(message(error));
    } finally {
      if (generation.current === attempt) setBusy(false);
    }
  }
  async function disconnect() {
    const target = current.current;
    if (!target) return;
    sizes.current?.stop();
    setPhase("disconnecting");
    setStatus("Disconnecting…");
    try {
      await api.disconnect(target);
    } catch (error) {
      if (current.current === target) setError(message(error));
    }
  }
  return (
    <main className="app">
      <ProfileControls
        profiles={profiles}
        active={busy || (phase !== "idle" && phase !== "closed")}
      />
      <ConnectionBar
        host={host}
        port={port}
        phase={phase}
        busy={busy}
        onHost={(value) => profiles.endpoint("host", value)}
        onPort={(value) => profiles.endpoint("port", value)}
        onConnect={() => {
          void start();
        }}
        onDisconnect={() => {
          void disconnect();
        }}
        onClear={() => {
          model.current.clear();
          setRevision((v) => v + 1);
        }}
      />
      <div className="status-bar">
        <span className={`status-dot ${phase}`} aria-hidden="true" />
        <span role="status">{status}</span>
        <span className="transport">Plain TCP</span>
      </div>
      <Transcript
        model={model.current}
        revision={revision}
        onViewport={setViewport}
        appearance={profiles.appearance}
      />
      {error && (
        <div className="error" role="alert">
          {error}
        </div>
      )}
      <CommandInput
        key={id ?? "idle"}
        connected={phase === "connected"}
        remoteEcho={options.remoteEcho}
        maskingGeneration={options.maskingGeneration}
        send={(text) => api.send(id!, text)}
        onError={(failure) => {
          if (current.current === id) setError(failure);
        }}
      />
    </main>
  );
}
