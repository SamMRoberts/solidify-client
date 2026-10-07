import { useEffect, useRef, useState } from "react";
import {
  defaultAppearance,
  profileMessage,
  type ProfileDraft,
  type ProfilesBridge,
  type ProfilesSnapshot,
} from "../bridge/profiles";

export function useProfiles(api: ProfilesBridge) {
  const [host, setHost] = useState("localhost");
  const [port, setPort] = useState("4000");
  const [appearance, setAppearance] = useState(defaultAppearance);
  const [selected, setSelected] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<ProfilesSnapshot>({
    profiles: [],
    selected: null,
    writable: false,
    warning: null,
  });
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const touched = useRef(false);
  const alive = useRef(true);
  const admitted = useRef(false);
  const loaded = useRef(false);
  const endpointCustomized = useRef(false);
  const preferenceGeneration = useRef(0);
  const touch = () => {
    touched.current = true;
  };
  async function perform(operation: () => Promise<ProfilesSnapshot>) {
    if (admitted.current)
      throw "A saved-connection operation is still finishing. Try again shortly.";
    admitted.current = true;
    setBusy(true);
    try {
      const next = await operation();
      if (alive.current) setSnapshot(next);
      return next;
    } finally {
      admitted.current = false;
      if (alive.current) setBusy(false);
    }
  }
  async function load() {
    try {
      const next = await perform(() => api.load());
      if (!alive.current) return;
      loaded.current = true;
      if (!touched.current) {
        const profile = next.profiles.find((p) => p.id === next.selected);
        setSelected(profile?.id ?? null);
        if (profile) {
          setHost(profile.host);
          setPort(String(profile.port));
          setAppearance(profile.appearance);
        }
      } else if (selected && !next.profiles.some((p) => p.id === selected)) {
        setSelected(null);
      }
    } catch (error) {
      if (alive.current)
        setSnapshot((previous) => ({
          ...previous,
          writable: false,
          warning: profileMessage(error),
        }));
    }
  }
  useEffect(() => {
    alive.current = true;
    void load();
    return () => {
      alive.current = false;
      touched.current = true;
    };
    // The app supplies one stable bridge for its lifetime.
  }, [api]);
  async function remember(id: string | null) {
    const attempt = ++preferenceGeneration.current;
    try {
      await perform(() => api.select(id));
      if (alive.current && attempt === preferenceGeneration.current)
        setNotice("");
    } catch {
      if (alive.current && attempt === preferenceGeneration.current)
        setNotice(
          "This selection is usable, but was not remembered. Select it again to retry.",
        );
    }
  }
  function select(id: string | null) {
    touch();
    setSelected(id);
    const profile = snapshot.profiles.find((p) => p.id === id);
    if (profile) {
      setHost(profile.host);
      setPort(String(profile.port));
      setAppearance(profile.appearance);
    }
    void remember(id);
  }
  function endpoint(field: "host" | "port", value: string) {
    touch();
    if (field === "host") setHost(value);
    else setPort(value);
    if (selected !== null || (!loaded.current && !endpointCustomized.current)) {
      setSelected(null);
      void remember(null);
    }
    endpointCustomized.current = true;
  }
  async function save(
    mode: "create" | "edit" | "appearance",
    draft: ProfileDraft,
  ) {
    touch();
    if (mode !== "create" && selected === null) {
      setAppearance(draft.appearance);
      return;
    }
    const next = await perform(() =>
      mode === "create"
        ? api.create(draft)
        : mode === "appearance"
          ? api.appearance(selected!, draft.appearance)
          : api.update(selected!, draft),
    );
    if (!alive.current) return;
    const profile = next.profiles.find(
      (p) => p.id === (mode === "create" ? next.selected : selected),
    );
    if (profile) {
      setSelected(profile.id);
      setAppearance(profile.appearance);
      if (mode !== "appearance") {
        setHost(profile.host);
        setPort(String(profile.port));
      }
    }
    setNotice("");
  }
  async function remove() {
    if (!selected) return;
    touch();
    await perform(() => api.delete(selected));
    if (alive.current) {
      setSelected(null);
      setNotice("");
    }
  }
  return {
    host,
    port,
    appearance,
    setAppearance,
    selected,
    snapshot,
    notice,
    busy,
    touch,
    load,
    select,
    endpoint,
    save,
    remove,
    remember,
  };
}
