import { it, expect, vi } from "vitest";
import { pollConnection, type Bridge, type Snapshot } from "./client";
const snapshot: Snapshot = {
  id: "1",
  phase: "connected",
  message: "Connected.",
  events: [],
  finished: true,
  remoteEcho: false,
  maskingGeneration: "0",
};
function api(poll: Bridge["poll"]): Bridge {
  return {
    start: vi.fn(),
    send: vi.fn(),
    viewport: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn().mockResolvedValue(undefined),
    poll,
  };
}
it("ignores late responses after disposal", async () => {
  let resolve!: (s: Snapshot) => void;
  const bridge = api(vi.fn(() => new Promise<Snapshot>((r) => (resolve = r))));
  const receive = vi.fn();
  const stop = pollConnection(bridge, "1", receive, vi.fn());
  stop();
  resolve(snapshot);
  await Promise.resolve();
  expect(receive).not.toHaveBeenCalled();
});
it("rejects stale IDs and disconnects after bridge failure", async () => {
  const bridge = api(vi.fn().mockResolvedValue({ ...snapshot, id: "old" }));
  const receive = vi.fn();
  const fail = vi.fn();
  pollConnection(bridge, "1", receive, fail);
  await Promise.resolve();
  await Promise.resolve();
  expect(receive).not.toHaveBeenCalled();
  expect(fail).toHaveBeenCalled();
  expect(bridge.disconnect).toHaveBeenCalledWith("1");
});
it("keeps only one poll outstanding and stops on final output", async () => {
  let resolve!: (value: Snapshot) => void;
  const frames: FrameRequestCallback[] = [];
  const frame = vi
    .spyOn(globalThis, "requestAnimationFrame")
    .mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
  try {
    const bridge = api(
      vi
        .fn()
        .mockImplementationOnce(
          () =>
            new Promise<Snapshot>((r) => {
              resolve = r;
            }),
        )
        .mockResolvedValue(snapshot),
    );
    const receive = vi.fn();
    pollConnection(bridge, "1", receive, vi.fn());
    await Promise.resolve();
    expect(bridge.poll).toHaveBeenCalledTimes(1);
    expect(frames).toHaveLength(0);
    resolve({ ...snapshot, finished: false });
    await Promise.resolve();
    expect(bridge.poll).toHaveBeenCalledTimes(1);
    expect(frames).toHaveLength(1);
    frames.shift()!(0);
    await Promise.resolve();
    expect(bridge.poll).toHaveBeenCalledTimes(2);
    expect(receive).toHaveBeenLastCalledWith(snapshot);
    expect(frames).toHaveLength(0);
  } finally {
    frame.mockRestore();
  }
});
