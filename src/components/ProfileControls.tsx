import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  defaultAppearance,
  profileMessage,
  validAppearance,
  type Appearance,
  type ProfileDraft,
} from "../bridge/profiles";
import type { useProfiles } from "./useProfiles";

type Profiles = ReturnType<typeof useProfiles>;
function Modal({
  title,
  children,
  onCancel,
  busy,
}: {
  title: string;
  children: ReactNode;
  onCancel: () => void;
  busy: boolean;
}) {
  const node = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = node.current!;
    dialog.showModal();
    return () => dialog.close();
  }, []);
  return (
    <dialog
      ref={node}
      className="profile-dialog"
      aria-label={title}
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) onCancel();
      }}
    >
      <h2>{title}</h2>
      {children}
    </dialog>
  );
}
function Editor({
  profiles,
  mode,
  onClose,
}: {
  profiles: Profiles;
  mode: "create" | "edit" | "appearance";
  onClose: () => void;
}) {
  const original = useRef(profiles.appearance);
  const saved = profiles.snapshot.profiles.find(
    (p) => p.id === profiles.selected,
  );
  const [name, setName] = useState(
    mode === "create" ? "" : (saved?.name ?? "Custom"),
  );
  const [host, setHost] = useState(profiles.host);
  const [port, setPort] = useState(profiles.port);
  const [appearance, setAppearance] = useState(profiles.appearance);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const appearanceOnly = mode === "appearance" || (mode === "edit" && !saved);
  const ephemeral = mode !== "create" && !saved;
  const valid = validAppearance(appearance);
  function preview(next: Appearance) {
    setAppearance(next);
    if (validAppearance(next)) profiles.setAppearance(next);
  }
  function cancel() {
    profiles.setAppearance(original.current);
    onClose();
  }
  async function save() {
    if (saving) return;
    setError("");
    const trimmed = name.trim();
    if (
      !appearanceOnly &&
      (!trimmed ||
        new TextEncoder().encode(trimmed).length > 128 ||
        /[\p{Cc}]/u.test(trimmed))
    ) {
      setError(
        "Use a nonempty name of at most 128 UTF-8 bytes without control characters.",
      );
      return;
    }
    if (
      !appearanceOnly &&
      profiles.snapshot.profiles.some(
        (p) =>
          p.name === trimmed &&
          (mode === "create" || p.id !== profiles.selected),
      )
    ) {
      setError("A saved connection already has that name.");
      return;
    }
    if (
      !appearanceOnly &&
      (!/^\d+$/.test(port) || Number(port) < 1 || Number(port) > 65535)
    ) {
      setError("Port must be between 1 and 65535.");
      return;
    }
    if (!valid) {
      setError("Use font size 10–24 and six-digit hex colors.");
      return;
    }
    setSaving(true);
    const draft: ProfileDraft = {
      name: trimmed,
      host,
      port: Number(port),
      appearance,
    };
    try {
      await profiles.save(mode, draft);
      onClose();
    } catch (failure) {
      setError(profileMessage(failure));
      setSaving(false);
    }
  }
  return (
    <Modal
      title={
        mode === "create"
          ? "Save connection as"
          : appearanceOnly
            ? "Transcript appearance"
            : "Edit saved connection"
      }
      busy={saving}
      onCancel={cancel}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <fieldset disabled={saving}>
          {!appearanceOnly && (
            <>
              <label>
                Name
                <input
                  autoFocus
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                />
              </label>
              <div className="profile-endpoint">
                <label>
                  Profile host
                  <input
                    value={host}
                    onChange={(event) => setHost(event.target.value)}
                  />
                </label>
                <label>
                  Profile port
                  <input
                    inputMode="numeric"
                    value={port}
                    onChange={(event) => setPort(event.target.value)}
                  />
                </label>
              </div>
            </>
          )}
          <label>
            Transcript font size (px)
            <input
              autoFocus={appearanceOnly}
              type="number"
              min={10}
              max={24}
              step={1}
              value={
                Number.isNaN(appearance.fontSize) ? "" : appearance.fontSize
              }
              onChange={(event) =>
                preview({ ...appearance, fontSize: event.target.valueAsNumber })
              }
            />
          </label>
          <div className="profile-colors">
            <label>
              Default text color
              <input
                value={appearance.foreground}
                spellCheck={false}
                onChange={(event) =>
                  preview({ ...appearance, foreground: event.target.value })
                }
              />
            </label>
            <label>
              Default background color
              <input
                value={appearance.background}
                spellCheck={false}
                onChange={(event) =>
                  preview({ ...appearance, background: event.target.value })
                }
              />
            </label>
          </div>
          <button
            type="button"
            className="quiet"
            onClick={() => preview({ ...defaultAppearance })}
          >
            Reset to defaults
          </button>
        </fieldset>
        <p className="editor-hint">
          {ephemeral
            ? "Appearance applies for this run. Use Save as to keep it in a named profile."
            : "Valid appearance changes preview immediately. Save keeps them; Cancel restores them."}
        </p>
        {!ephemeral && !profiles.snapshot.writable && (
          <p role="status">
            Storage is read-only. Cancel and Retry loading before saving.
          </p>
        )}
        {error && (
          <p role="alert" className="editor-error">
            {error}
          </p>
        )}
        <div className="dialog-actions">
          <button type="button" disabled={saving} onClick={cancel}>
            Cancel
          </button>
          <button
            className="primary"
            disabled={
              saving || !valid || (!ephemeral && !profiles.snapshot.writable)
            }
          >
            {saving ? "Saving…" : ephemeral ? "Apply" : "Save"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
export function ProfileControls({
  profiles,
  active,
}: {
  profiles: Profiles;
  active: boolean;
}) {
  const [editor, setEditor] = useState<"create" | "edit" | "appearance" | null>(
    null,
  );
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState("");
  const locked = active || profiles.busy;
  const selected = profiles.snapshot.profiles.find(
    (p) => p.id === profiles.selected,
  );
  function edit(mode: "create" | "edit" | "appearance") {
    profiles.touch();
    setEditor(mode);
  }
  return (
    <>
      <div className="profile-bar">
        <label>
          Connection profile
          <select
            aria-label="Connection profile"
            value={profiles.selected ?? ""}
            disabled={locked}
            onChange={(event) => profiles.select(event.target.value || null)}
          >
            <option value="">Custom</option>
            {profiles.snapshot.profiles.map((p) => (
              <option value={p.id} key={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={
            locked ||
            !profiles.snapshot.writable ||
            profiles.snapshot.profiles.length >= 100
          }
          onClick={() => edit("create")}
        >
          Save as
        </button>
        <button
          disabled={profiles.busy}
          onClick={() => edit(active ? "appearance" : "edit")}
        >
          {active ? "Appearance" : "Edit"}
        </button>
        <button
          disabled={locked || !selected || !profiles.snapshot.writable}
          onClick={() => {
            profiles.touch();
            setError("");
            setDeleting(true);
          }}
        >
          Delete
        </button>
      </div>
      {(profiles.snapshot.warning || profiles.notice) && (
        <div className="storage-warning" role="status">
          <span>
            {profiles.snapshot.warning && (
              <span className="warning-message">
                {profiles.snapshot.warning}
              </span>
            )}
            {profiles.notice && (
              <span className="warning-message">{profiles.notice}</span>
            )}
          </span>
          <button
            disabled={profiles.busy || !!editor || deleting}
            onClick={() => {
              void profiles.load();
            }}
          >
            Retry loading
          </button>
          {profiles.notice && (
            <button
              disabled={profiles.busy || !!editor || deleting}
              onClick={() => {
                void profiles.remember(profiles.selected);
              }}
            >
              Remember selection
            </button>
          )}
        </div>
      )}
      {editor && (
        <Editor
          profiles={profiles}
          mode={editor}
          onClose={() => setEditor(null)}
        />
      )}
      {deleting && (
        <Modal
          title="Delete saved connection"
          busy={profiles.busy}
          onCancel={() => setDeleting(false)}
        >
          <p>
            Delete “{selected?.name}”? Its endpoint and appearance will remain
            as Custom for this run.
          </p>
          {error && <p role="alert">{error}</p>}
          <div className="dialog-actions">
            <button disabled={profiles.busy} onClick={() => setDeleting(false)}>
              Cancel
            </button>
            <button
              disabled={profiles.busy}
              onClick={() => {
                void profiles
                  .remove()
                  .then(() => setDeleting(false))
                  .catch((failure) => setError(profileMessage(failure)));
              }}
            >
              Confirm delete
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
