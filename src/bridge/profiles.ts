import { invoke } from "@tauri-apps/api/core";

export interface Appearance {
  fontSize: number;
  foreground: string;
  background: string;
}
export const defaultAppearance: Appearance = {
  fontSize: 14,
  foreground: "#e6e8eb",
  background: "#111318",
};
export interface ProfileDraft {
  name: string;
  host: string;
  port: number;
  appearance: Appearance;
}
export interface Profile extends ProfileDraft {
  id: string;
}
export interface ProfilesSnapshot {
  profiles: Profile[];
  selected: string | null;
  writable: boolean;
  warning: string | null;
}
export interface ProfilesBridge {
  load(): Promise<ProfilesSnapshot>;
  create(draft: ProfileDraft): Promise<ProfilesSnapshot>;
  update(id: string, draft: ProfileDraft): Promise<ProfilesSnapshot>;
  appearance(id: string, appearance: Appearance): Promise<ProfilesSnapshot>;
  delete(id: string): Promise<ProfilesSnapshot>;
  select(selected: string | null): Promise<ProfilesSnapshot>;
}
export const profilesBridge: ProfilesBridge = {
  load: () => invoke("load_profiles"),
  create: (draft) => invoke("create_profile", { draft }),
  update: (profileIdValue, draft) =>
    invoke("update_profile", { profileIdValue, draft }),
  appearance: (profileIdValue, appearance) =>
    invoke("update_profile_appearance", { profileIdValue, appearance }),
  delete: (profileIdValue) => invoke("delete_profile", { profileIdValue }),
  select: (selected) => invoke("select_profile", { selected }),
};
export function validAppearance(value: Appearance) {
  return (
    Number.isInteger(value.fontSize) &&
    value.fontSize >= 10 &&
    value.fontSize <= 24 &&
    /^#[0-9a-f]{6}$/i.test(value.foreground) &&
    /^#[0-9a-f]{6}$/i.test(value.background)
  );
}
// Native errors are categories; unexpected bridge failures must not expose payloads.
export function profileMessage(error: unknown): string {
  const messages = [
    "A saved connection already has that name.",
    "Use a nonempty name of at most 128 UTF-8 bytes without control characters.",
    "Enter a valid IP address or ASCII hostname and port 1–65535.",
    "Use font size 10–24 and six-digit hex colors.",
    "Saved connections exceed the 100-profile or 128 KiB limit.",
    "This saved connection no longer exists. Retry loading.",
    "Saved connections are read-only. Another instance may be using them; retry loading when it closes.",
    "A saved-connection operation is still finishing. Try again shortly.",
    "The application is shutting down.",
  ];
  return typeof error === "string" && messages.includes(error)
    ? error
    : "Saved connections are unavailable. Check storage access and retry loading.";
}
