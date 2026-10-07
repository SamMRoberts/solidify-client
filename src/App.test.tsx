import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { App } from "./App";
import type { Bridge, Snapshot } from "./bridge/client";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
it("never connects automatically and cleans up a start completed after unmount", async () => {
  const started = deferred<string>();
  const api: Bridge = {
    start: vi.fn(() => started.promise),
    poll: vi.fn(),
    send: vi.fn(),
    viewport: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn().mockResolvedValue(undefined),
  };
  const view = render(<App api={api} />);
  expect(api.start).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Connect"));
  expect(api.start).toHaveBeenCalledWith("localhost", 4000);
  view.unmount();
  await act(async () => {
    started.resolve("7");
  });
  expect(api.disconnect).toHaveBeenCalledWith("7");
  expect(api.poll).not.toHaveBeenCalled();
});
it("renders delivered output, resets it for replacement sessions and cancels on unmount", async () => {
  const first = deferred<Snapshot>();
  const second = deferred<Snapshot>();
  const api: Bridge = {
    start: vi.fn().mockResolvedValueOnce("1").mockResolvedValueOnce("2"),
    poll: vi.fn((id) => (id === "1" ? first.promise : second.promise)),
    send: vi.fn().mockResolvedValue(undefined),
    viewport: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn().mockResolvedValue(undefined),
  };
  const view = render(<App api={api} />);
  fireEvent.click(screen.getByText("Connect"));
  await waitFor(() => expect(api.poll).toHaveBeenCalledWith("1"));
  await act(async () =>
    first.resolve({
      id: "1",
      phase: "closed",
      message: "Closed.",
      events: [{ type: "text", text: "old prompt" }],
      finished: true,
      remoteEcho: false,
      maskingGeneration: "0",
    }),
  );
  expect(screen.getByText("old prompt")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Connect"));
  await waitFor(() => expect(api.poll).toHaveBeenCalledWith("2"));
  expect(screen.queryByText("old prompt")).toBeNull();
  await act(async () =>
    second.resolve({
      id: "2",
      phase: "connected",
      message: "Connected.",
      events: [{ type: "text", text: "new prompt" }],
      finished: true,
      remoteEcho: false,
      maskingGeneration: "0",
    }),
  );
  expect(screen.getByLabelText("Command")).toHaveFocus();
  fireEvent.change(screen.getByLabelText("Command"), {
    target: { value: "look" },
  });
  fireEvent.submit(screen.getByLabelText("Command").closest("form")!);
  await waitFor(() => expect(api.send).toHaveBeenCalledWith("2", "look"));
  view.unmount();
  expect(api.disconnect).toHaveBeenCalledWith("2");
});
