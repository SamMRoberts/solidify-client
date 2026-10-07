import { useEffect, useRef, useState } from "react";
import { message } from "../bridge/client";
export function CommandInput({
  connected,
  send,
  onError,
  remoteEcho = false,
  maskingGeneration = "0",
}: {
  connected: boolean;
  remoteEcho?: boolean;
  maskingGeneration?: string;
  send: (text: string) => Promise<void>;
  onError: (error: string) => void;
}) {
  const [draft, setDraft] = useState("");
  const [masked, setMasked] = useState(false);
  const [protectedDraft, setProtectedDraft] = useState(false);
  const [seenGeneration, setSeenGeneration] = useState(maskingGeneration);
  // Adjust during render so a server transition cannot paint an unmasked draft.
  if (seenGeneration !== maskingGeneration) {
    setSeenGeneration(maskingGeneration);
    if (draft) setProtectedDraft(true);
  }
  if (remoteEcho && draft && !protectedDraft) setProtectedDraft(true);
  const effectiveMask = masked || remoteEcho || protectedDraft;
  const [sending, setSending] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (connected) input.current?.focus();
  }, [connected]);
  async function submit() {
    if (!connected || sending) return;
    if (/[\u0000-\u001f\u007f-\u009f]/u.test(draft)) {
      onError("Commands must be one line without control characters.");
      return;
    }
    if (new TextEncoder().encode(draft).length > 16382) {
      onError("Command exceeds 16 KiB including its line ending.");
      return;
    }
    setSending(true);
    try {
      await send(draft);
      setDraft("");
      setProtectedDraft(false);
      onError("");
    } catch (error) {
      onError(message(error));
    } finally {
      setSending(false);
    }
  }
  return (
    <form
      className="command-bar"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label className="command">
        Command
        <input
          ref={input}
          aria-label="Command"
          type={effectiveMask ? "password" : "text"}
          value={draft}
          onChange={(e) => {
            const text = e.target.value;
            setDraft(text);
            setProtectedDraft(Boolean(text) && effectiveMask);
          }}
          disabled={!connected}
          readOnly={sending}
          onKeyDown={(event) => {
            if (event.key === "Enter" && event.nativeEvent.isComposing)
              event.preventDefault();
          }}
          autoComplete="off"
          autoCapitalize="none"
          spellCheck={false}
          placeholder={
            connected ? "Enter a command…" : "Connect to send commands"
          }
        />
      </label>
      <button
        type="submit"
        className="primary"
        disabled={!connected || sending}
      >
        Send
      </button>
      <label className="mask">
        <input
          type="checkbox"
          checked={masked}
          onChange={(e) => {
            setMasked(e.target.checked);
            if (e.target.checked && draft) setProtectedDraft(true);
          }}
        />
        Mask input
      </label>
      {(remoteEcho || protectedDraft) && (
        <span className="mask-reason" role="status">
          {remoteEcho
            ? "Server-requested masking"
            : "Draft stays masked until sent or cleared"}
        </span>
      )}
    </form>
  );
}
