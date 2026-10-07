import { afterEach, expect, it, vi } from "vitest";
import { viewportDelivery } from "./viewport";
import { characterDimensions, observeViewport } from "../terminal/viewport";
import type { Bridge } from "./client";
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
function api(): Bridge {
  return {
    start: vi.fn(),
    poll: vi.fn(),
    send: vi.fn(),
    disconnect: vi.fn().mockResolvedValue(undefined),
    viewport: vi.fn().mockResolvedValue(undefined),
  };
}
it("calculates usable dimensions with fallback, flooring and bounds", () => {
  expect(characterDimensions(800, 480, 10, 20)).toEqual({
    columns: 80,
    rows: 24,
  });
  expect(characterDimensions(819, 499, 10, 20)).toEqual({
    columns: 81,
    rows: 24,
  });
  expect(characterDimensions(1, 1, 10, 20)).toEqual({ columns: 1, rows: 1 });
  expect(characterDimensions(1e9, 1e9, 1, 1)).toEqual({
    columns: 65535,
    rows: 65535,
  });
  for (const bad of [0, -1, NaN, Infinity])
    expect(characterDimensions(bad, 480, 10, 20)).toEqual({
      columns: 80,
      rows: 24,
    });
});
it("measures padding and metrics, skips unchanged sizes, and cleans up observer", () => {
  const node = document.createElement("div");
  document.body.append(node);
  node.style.cssText = "padding: 10px 20px; line-height: 20px";
  Object.defineProperties(node, {
    clientWidth: { value: 840, configurable: true },
    clientHeight: { value: 500 },
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 100,
  } as DOMRect);
  let resize!: () => void;
  const disconnect = vi.fn();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(fn: () => void) {
        resize = fn;
      }
      observe() {}
      disconnect = disconnect;
    },
  );
  const receive = vi.fn();
  const stop = observeViewport(node, receive);
  expect(receive).toHaveBeenLastCalledWith({ columns: 80, rows: 24 });
  resize();
  expect(receive).toHaveBeenCalledTimes(1);
  Object.defineProperty(node, "clientWidth", { value: 1040 });
  resize();
  expect(receive).toHaveBeenLastCalledWith({ columns: 100, rows: 24 });
  stop();
  resize();
  expect(receive).toHaveBeenCalledTimes(2);
  expect(disconnect).toHaveBeenCalled();
  expect(node.children).toHaveLength(0);
  node.remove();
});
it("debounces and coalesces during an outstanding invocation", async () => {
  vi.useFakeTimers();
  const bridge = api();
  let resolve!: () => void;
  vi.mocked(bridge.viewport).mockImplementationOnce(
    () =>
      new Promise<void>((r) => {
        resolve = r;
      }),
  );
  const delivery = viewportDelivery(bridge, "1", vi.fn());
  delivery.update({ columns: 80, rows: 24 });
  delivery.update({ columns: 90, rows: 24 });
  await vi.advanceTimersByTimeAsync(99);
  expect(bridge.viewport).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(bridge.viewport).toHaveBeenCalledTimes(1);
  for (let columns = 100; columns <= 200; columns++)
    delivery.update({ columns, rows: 24 });
  await vi.advanceTimersByTimeAsync(100);
  expect(bridge.viewport).toHaveBeenCalledTimes(1);
  resolve();
  await Promise.resolve();
  expect(bridge.viewport).toHaveBeenLastCalledWith("1", {
    columns: 200,
    rows: 24,
  });
  await Promise.resolve();
  delivery.update({ columns: 200, rows: 24 });
  await vi.advanceTimersByTimeAsync(100);
  expect(bridge.viewport).toHaveBeenCalledTimes(2);
  delivery.stop();
});
it("ignores stale completions and stops with cleanup after bridge errors", async () => {
  vi.useFakeTimers();
  const bridge = api();
  const failed = vi.fn();
  let reject!: (e: string) => void;
  vi.mocked(bridge.viewport).mockImplementationOnce(
    () =>
      new Promise<void>((_, r) => {
        reject = r;
      }),
  );
  const old = viewportDelivery(bridge, "old", failed);
  old.update({ columns: 80, rows: 24 });
  await vi.advanceTimersByTimeAsync(100);
  old.stop();
  reject("stale");
  await Promise.resolve();
  expect(failed).not.toHaveBeenCalled();
  expect(bridge.disconnect).not.toHaveBeenCalled();
  vi.mocked(bridge.viewport).mockRejectedValue("Bridge failed");
  const next = viewportDelivery(bridge, "next", failed);
  next.update({ columns: 90, rows: 30 });
  await vi.advanceTimersByTimeAsync(100);
  expect(failed).toHaveBeenCalledWith("Bridge failed");
  expect(bridge.disconnect).toHaveBeenCalledWith("next");
  next.update({ columns: 100, rows: 30 });
  await vi.advanceTimersByTimeAsync(100);
  expect(bridge.viewport).toHaveBeenCalledTimes(2);
  next.stop();
});

it("font metric changes recalculate NAWS through the existing debounce path", async () => {
  vi.useFakeTimers();
  const node = document.createElement("div");
  document.body.append(node);
  node.style.cssText = "padding: 0px; line-height: 20px";
  Object.defineProperties(node, {
    clientWidth: { value: 800 },
    clientHeight: { value: 480 },
  });
  const metrics = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockReturnValue({ width: 100 } as DOMRect);
  let resized!: () => void;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: () => void) {
        resized = callback;
      }
      observe() {}
      disconnect() {}
    },
  );
  const bridge = api();
  const delivery = viewportDelivery(bridge, "font-test", vi.fn());
  const stop = observeViewport(node, delivery.update);
  await vi.advanceTimersByTimeAsync(100);
  expect(bridge.viewport).toHaveBeenLastCalledWith("font-test", {
    columns: 80,
    rows: 24,
  });
  metrics.mockReturnValue({ width: 150 } as DOMRect);
  node.style.lineHeight = "30px";
  resized();
  await vi.advanceTimersByTimeAsync(99);
  expect(bridge.viewport).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(bridge.viewport).toHaveBeenLastCalledWith("font-test", {
    columns: 53,
    rows: 16,
  });
  stop();
  delivery.stop();
  node.remove();
});
