import { message, type Bridge, type ViewportSize } from "./client";

/** One in-flight invocation and one latest pending size per connection. */
export function viewportDelivery(
  api: Bridge,
  id: string,
  failed: (error: string) => void,
) {
  let stopped = false;
  let pending: ViewportSize | null = null;
  let sent: ViewportSize | null = null;
  let busy = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const equal = (a: ViewportSize | null, b: ViewportSize) =>
    a?.columns === b.columns && a.rows === b.rows;
  async function flush() {
    timer = undefined;
    if (stopped || busy || !pending) return;
    const size = pending;
    pending = null;
    if (equal(sent, size)) return;
    busy = true;
    try {
      await api.viewport(id, size);
      if (!stopped) sent = size;
    } catch (error) {
      if (!stopped) {
        stopped = true;
        failed(message(error));
        void api.disconnect(id).catch(() => {});
      }
    } finally {
      busy = false;
      if (!stopped && pending && timer === undefined) void flush();
    }
  }
  return {
    update(size: ViewportSize) {
      if (stopped) return;
      pending = size;
      clearTimeout(timer);
      timer = setTimeout(() => {
        void flush();
      }, 100);
    },
    stop() {
      stopped = true;
      pending = null;
      clearTimeout(timer);
    },
  };
}
