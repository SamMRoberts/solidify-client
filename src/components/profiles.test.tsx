import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ProfileControls } from "./ProfileControls";
import { useProfiles } from "./useProfiles";
import {
  defaultAppearance,
  type ProfilesBridge,
  type ProfilesSnapshot,
} from "../bridge/profiles";
import { App } from "../App";
import type { Bridge } from "../bridge/client";
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
const saved: ProfilesSnapshot = {
  profiles: [
    {
      id: "12",
      name: "Local demo",
      host: "127.0.0.1",
      port: 4001,
      appearance: {
        fontSize: 18,
        foreground: "#abcdef",
        background: "#123456",
      },
    },
  ],
  selected: "12",
  writable: true,
  warning: null,
};
function api(): ProfilesBridge {
  let state = structuredClone(saved);
  return {
    load: vi.fn(async () => structuredClone(state)),
    create: vi.fn(async (draft) => {
      state = {
        ...state,
        selected: "13",
        profiles: [...state.profiles, { ...draft, id: "13" }],
      };
      return structuredClone(state);
    }),
    update: vi.fn(async (id, draft) => {
      state = {
        ...state,
        profiles: state.profiles.map((p) =>
          p.id === id ? { ...draft, id } : p,
        ),
      };
      return structuredClone(state);
    }),
    appearance: vi.fn(async (id, appearance) => {
      state = {
        ...state,
        profiles: state.profiles.map((p) =>
          p.id === id ? { ...p, appearance } : p,
        ),
      };
      return structuredClone(state);
    }),
    delete: vi.fn(async (id) => {
      state = {
        ...state,
        selected: null,
        profiles: state.profiles.filter((p) => p.id !== id),
      };
      return structuredClone(state);
    }),
    select: vi.fn(async (selected) => {
      state = { ...state, selected };
      return structuredClone(state);
    }),
  };
}
function Harness({
  bridge,
  active = false,
}: {
  bridge: ProfilesBridge;
  active?: boolean;
}) {
  const profiles = useProfiles(bridge);
  return (
    <>
      <ProfileControls profiles={profiles} active={active} />
      <input
        aria-label="Host"
        value={profiles.host}
        onChange={(e) => profiles.endpoint("host", e.target.value)}
      />
      <input
        aria-label="Port"
        value={profiles.port}
        onChange={(e) => profiles.endpoint("port", e.target.value)}
      />
      <output data-testid="appearance">
        {JSON.stringify(profiles.appearance)}
      </output>
      <button onClick={profiles.touch}>Connect snapshot</button>
    </>
  );
}
async function ready() {
  await waitFor(() =>
    expect(screen.getByLabelText("Connection profile")).toHaveValue("12"),
  );
}
function change(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}
function click(name: string) {
  fireEvent.click(screen.getByRole("button", { name }));
}
it("restores saved selection without connecting and switches endpoint edits to Custom", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  expect(screen.getByTestId("appearance")).toHaveTextContent('"fontSize":18');
  change("Host", "new.example");
  expect(screen.getByLabelText("Connection profile")).toHaveValue("");
  await waitFor(() => expect(bridge.select).toHaveBeenCalledWith(null));
  expect(bridge.update).not.toHaveBeenCalled();
  change("Connection profile", "12");
  await waitFor(() => expect(bridge.select).toHaveBeenCalledWith("12"));
  expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  change("Connection profile", "");
  expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
});
it.each(["edit", "connect"])(
  "late startup cannot overwrite a prior %s",
  async (action) => {
    const bridge = api(),
      pending = deferred<ProfilesSnapshot>();
    bridge.load = vi.fn(() => pending.promise);
    render(<Harness bridge={bridge} />);
    if (action === "edit") change("Host", "manual.example");
    else click("Connect snapshot");
    await act(async () => pending.resolve(saved));
    expect(screen.getByLabelText("Host")).toHaveValue(
      action === "edit" ? "manual.example" : "localhost",
    );
    expect(screen.getByLabelText("Connection profile")).toHaveValue("");
  },
);
it("first run or a cleared preference uses defaults", async () => {
  const bridge = api();
  bridge.load = vi.fn().mockResolvedValue({ ...saved, selected: null });
  render(<Harness bridge={bridge} />);
  await waitFor(() => expect(screen.getByText("Save as")).toBeEnabled());
  expect(screen.getByLabelText("Host")).toHaveValue("localhost");
  expect(screen.getByTestId("appearance")).toHaveTextContent(
    JSON.stringify(defaultAppearance),
  );
});
it("failed selection preference keeps selected profile usable and supports explicit retry", async () => {
  const bridge = api();
  vi.mocked(bridge.select).mockRejectedValueOnce("untrusted failure");
  render(<Harness bridge={bridge} />);
  await ready();
  change("Connection profile", "");
  await screen.findByText(/not remembered/);
  expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  click("Remember selection");
  await waitFor(() => expect(screen.queryByText(/not remembered/)).toBeNull());
  expect(bridge.select).toHaveBeenCalledTimes(2);
});
it("creates only after acceptance, rejects duplicate names, and retains failed drafts", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  click("Save as");
  change("Name", " Local demo ");
  click("Save");
  expect(screen.getByRole("alert")).toHaveTextContent("already has that name");
  expect(bridge.create).not.toHaveBeenCalled();
  change("Name", " Another ");
  vi.mocked(bridge.create).mockRejectedValueOnce("sensitive filesystem path");
  click("Save");
  await screen.findByText(/unavailable/);
  expect(screen.getByLabelText("Name")).toHaveValue(" Another ");
  expect(screen.getByLabelText("Connection profile")).toHaveValue("12");
  expect(screen.queryByText(/sensitive/)).toBeNull();
  expect(bridge.create).toHaveBeenCalledTimes(1);
  const pending = deferred<ProfilesSnapshot>();
  vi.mocked(bridge.create).mockReturnValueOnce(pending.promise);
  click("Save");
  expect(screen.getByText("Saving…")).toBeDisabled();
  expect(screen.getByLabelText("Connection profile")).toHaveValue("12");
  await act(async () =>
    pending.resolve({
      ...saved,
      selected: "13",
      profiles: [
        ...saved.profiles,
        { ...saved.profiles[0], id: "13", name: "Another" },
      ],
    }),
  );
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByLabelText("Connection profile")).toHaveValue("13");
});
it("edits endpoint/name with explicit Save and delete confirmation retains current values", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  click("Edit");
  change("Name", "Renamed");
  change("Profile port", "4999");
  click("Save");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByLabelText("Port")).toHaveValue("4999");
  expect(bridge.update).toHaveBeenCalledWith(
    "12",
    expect.objectContaining({ name: "Renamed", port: 4999 }),
  );
  click("Delete");
  click("Cancel");
  expect(bridge.delete).not.toHaveBeenCalled();
  click("Delete");
  click("Confirm delete");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByLabelText("Connection profile")).toHaveValue("");
  expect(screen.getByLabelText("Port")).toHaveValue("4999");
  expect(screen.getByTestId("appearance")).toHaveTextContent('"fontSize":18');
});
it("active sessions disable profile and endpoint changes but permit appearance-only save", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} active />);
  await ready();
  expect(screen.getByLabelText("Connection profile")).toBeDisabled();
  expect(screen.getByText("Save as")).toBeDisabled();
  expect(screen.getByText("Delete")).toBeDisabled();
  click("Appearance");
  expect(screen.queryByLabelText("Name")).toBeNull();
  expect(screen.queryByLabelText("Profile host")).toBeNull();
  change("Transcript font size (px)", "22");
  click("Save");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(bridge.appearance).toHaveBeenCalledWith(
    "12",
    expect.objectContaining({ fontSize: 22 }),
  );
  expect(bridge.update).not.toHaveBeenCalled();
});
it("valid changes preview immediately, invalid colors do not, and Cancel restores appearance", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  click("Edit");
  change("Default text color", "#fedcba");
  expect(screen.getByTestId("appearance")).toHaveTextContent("#fedcba");
  change("Default background color", "url(evil)");
  expect(screen.getByTestId("appearance")).not.toHaveTextContent("evil");
  expect(screen.getByText("Save")).toBeDisabled();
  click("Reset to defaults");
  expect(screen.getByTestId("appearance")).toHaveTextContent(
    JSON.stringify(defaultAppearance),
  );
  expect(bridge.update).not.toHaveBeenCalled();
  click("Cancel");
  expect(screen.getByTestId("appearance")).toHaveTextContent(
    JSON.stringify(saved.profiles[0].appearance),
  );
});
it("Custom appearance applies for this run without persistence", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  change("Connection profile", "");
  await waitFor(() => expect(screen.getByText("Edit")).toBeEnabled());
  click("Edit");
  change("Transcript font size (px)", "10");
  click("Apply");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByTestId("appearance")).toHaveTextContent('"fontSize":10');
  expect(bridge.appearance).not.toHaveBeenCalled();
  expect(bridge.update).not.toHaveBeenCalled();
});
it("bad storage warning persists and read-only profiles remain selectable until Retry loading", async () => {
  const bridge = api();
  vi.mocked(bridge.load).mockResolvedValueOnce({
    ...saved,
    writable: false,
    warning: "Saved connections are read-only.",
  });
  render(<Harness bridge={bridge} />);
  await ready();
  expect(screen.getByText("Save as")).toBeDisabled();
  expect(screen.getByLabelText("Connection profile")).toBeEnabled();
  click("Retry loading");
  await waitFor(() =>
    expect(screen.queryByText("Saved connections are read-only.")).toBeNull(),
  );
  expect(screen.getByText("Save as")).toBeEnabled();
});
it("restoring, editing and selecting profiles never call the transport", async () => {
  const bridge = api();
  const transport: Bridge = {
    start: vi.fn(),
    poll: vi.fn(),
    send: vi.fn(),
    disconnect: vi.fn(),
    viewport: vi.fn(),
  };
  render(<App api={transport} profilesApi={bridge} />);
  await ready();
  click("Edit");
  const dialog = screen.getByRole("dialog");
  fireEvent.click(within(dialog).getByText("Cancel"));
  change("Connection profile", "");
  expect(transport.start).not.toHaveBeenCalled();
});

it("failed appearance save preserves preview and draft until explicit Cancel", async () => {
  const bridge = api();
  vi.mocked(bridge.appearance).mockRejectedValue("I/O payload must not escape");
  render(<Harness bridge={bridge} active />);
  await ready();
  click("Appearance");
  change("Transcript font size (px)", "21");
  click("Save");
  await screen.findByText(/unavailable/);
  expect(screen.getByLabelText("Transcript font size (px)")).toHaveValue(21);
  expect(screen.getByTestId("appearance")).toHaveTextContent('"fontSize":21');
  expect(bridge.appearance).toHaveBeenCalledTimes(1);
  click("Cancel");
  expect(screen.getByTestId("appearance")).toHaveTextContent('"fontSize":18');
});

it("an older successful preference write cannot hide a newer failed Custom preference", async () => {
  const bridge = api();
  render(<Harness bridge={bridge} />);
  await ready();
  change("Connection profile", "");
  await waitFor(() =>
    expect(screen.getByLabelText("Connection profile")).toBeEnabled(),
  );
  const pending = deferred<ProfilesSnapshot>();
  vi.mocked(bridge.select).mockReturnValueOnce(pending.promise);
  change("Connection profile", "12");
  change("Host", "custom.example");
  await screen.findByText(/not remembered/);
  await act(async () => pending.resolve(saved));
  expect(screen.getByLabelText("Connection profile")).toHaveValue("");
  expect(screen.getByLabelText("Host")).toHaveValue("custom.example");
  expect(screen.getByText(/not remembered/)).toBeInTheDocument();
  expect(bridge.select).toHaveBeenCalledTimes(2);
});
