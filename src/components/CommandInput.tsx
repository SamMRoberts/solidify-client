import { useEffect, useRef, useState } from "react";
import { message } from "../bridge/client";
export function CommandInput({
  connected,
  send,
  onError,
}: {
  connected: boolean;
  send: (text: string) => Promise<void>;
  onError: (error: string) => void;
}) {
  const [draft, setDraft] = useState("");
  const [masked, setMasked] = useState(false);
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
          type={masked ? "password" : "text"}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
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
          onChange={(e) => setMasked(e.target.checked)}
        />
        Mask input
      </label>
    </form>
  );
}
