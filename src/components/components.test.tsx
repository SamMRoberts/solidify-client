import userEvent from "@testing-library/user-event";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { it, expect, vi } from "vitest";
import { CommandInput } from "./CommandInput";
import { Transcript, nearBottom } from "../terminal/Transcript";
import { TranscriptModel, defaultStyle } from "../terminal/model";

it("keeps a failed draft, masks it manually, and clears only accepted commands", async () => {
  const send = vi
    .fn()
    .mockRejectedValueOnce("Queue full")
    .mockResolvedValue(undefined);
  const error = vi.fn();
  render(<CommandInput connected send={send} onError={error} />);
  const input = screen.getByLabelText("Command");
  fireEvent.change(input, { target: { value: "  hello  " } });
  fireEvent.click(screen.getByLabelText("Mask input"));
  expect(input).toHaveAttribute("type", "password");
  fireEvent.click(screen.getByText("Send"));
  await waitFor(() => expect(error).toHaveBeenCalledWith("Queue full"));
  expect(input).toHaveValue("  hello  ");
  fireEvent.click(screen.getByText("Send"));
  await waitFor(() => expect(input).toHaveValue(""));
  expect(send).toHaveBeenLastCalledWith("  hello  ");
});
it("rejects control input and disables sends while disconnected", async () => {
  const send = vi.fn();
  const error = vi.fn();
  const { rerender } = render(
    <CommandInput connected send={send} onError={error} />,
  );
  fireEvent.change(screen.getByLabelText("Command"), {
    target: { value: "bad\x1bcommand" },
  });
  fireEvent.click(screen.getByText("Send"));
  expect(send).not.toHaveBeenCalled();
  expect(error).toHaveBeenCalled();
  rerender(<CommandInput connected={false} send={send} onError={error} />);
  expect(screen.getByText("Send")).toBeDisabled();
});
it("renders hostile markup literally and maps typed styles", () => {
  const m = new TranscriptModel();
  m.apply([{ type: "text", text: "<script>alert('x')</script><img src=x>" }]);
  m.apply([
    {
      type: "style",
      style: {
        ...defaultStyle(),
        foreground: "red",
        background: "blue",
        inverse: true,
        bold: true,
        italic: true,
        underline: true,
      },
    },
    { type: "text", text: "styled" },
  ]);
  const { container } = render(<Transcript model={m} revision={1} />);
  expect(screen.getByText("styled")).toHaveClass(
    "fg-blue",
    "bg-red",
    "bold",
    "italic",
    "underline",
  );
  expect(container.querySelector("script")).toBeNull();
  expect(container.querySelector("img")).toBeNull();
  expect(
    screen.getByText("<script>alert('x')</script><img src=x>"),
  ).toBeInTheDocument();
});
it("shows new-output navigation only when the reader is away from the bottom", () => {
  const m = new TranscriptModel();
  m.apply([{ type: "text", text: "first" }]);
  const { rerender } = render(<Transcript model={m} revision={1} />);
  const node = screen.getByLabelText("Transcript");
  Object.defineProperties(node, {
    scrollHeight: { value: 1000, configurable: true },
    clientHeight: { value: 100, configurable: true },
  });
  node.scrollTop = 200;
  fireEvent.scroll(node);
  m.apply([{ type: "text", text: " next" }]);
  rerender(<Transcript model={m} revision={2} />);
  expect(node.scrollTop).toBe(200);
  fireEvent.click(screen.getByText("Latest output ↓"));
  expect(node.scrollTop).toBe(1000);
  expect(nearBottom(1000, 890, 100)).toBe(true);
  expect(nearBottom(1000, 100, 100)).toBe(false);
});

it("accepts empty commands and exact UTF-8 limits while retaining focus", async () => {
  const send = vi.fn().mockResolvedValue(undefined);
  const error = vi.fn();
  render(<CommandInput connected send={send} onError={error} />);
  const input = screen.getByLabelText("Command");
  await userEvent.setup().keyboard("{Enter}");
  await waitFor(() => expect(send).toHaveBeenCalledWith(""));
  expect(input).toHaveFocus();
  fireEvent.change(input, { target: { value: "é".repeat(8191) } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(input).toHaveValue(""));
  expect(send).toHaveBeenLastCalledWith("é".repeat(8191));
  fireEvent.change(input, { target: { value: "é".repeat(8192) } });
  fireEvent.submit(input.closest("form")!);
  expect(send).toHaveBeenCalledTimes(2);
  expect(input).toHaveValue("é".repeat(8192));
});
it("starts following output again for a fresh connection model", () => {
  const old = new TranscriptModel();
  const { rerender } = render(<Transcript model={old} revision={0} />);
  const node = screen.getByLabelText("Transcript");
  Object.defineProperties(node, {
    scrollHeight: { value: 1000 },
    clientHeight: { value: 100 },
  });
  node.scrollTop = 50;
  fireEvent.scroll(node);
  const next = new TranscriptModel();
  next.apply([{ type: "text", text: "new" }]);
  rerender(<Transcript model={next} revision={1} />);
  expect(node.scrollTop).toBe(1000);
  expect(screen.queryByText("Latest output ↓")).toBeNull();
});

it("protects masked drafts across automatic and manual resets and failed sends", async () => {
  const send = vi
    .fn()
    .mockRejectedValueOnce("Queue full")
    .mockResolvedValue(undefined);
  const onError = vi.fn();
  const view = render(
    <CommandInput
      connected
      send={send}
      onError={onError}
      remoteEcho
      maskingGeneration="1"
    />,
  );
  const input = screen.getByLabelText("Command");
  fireEvent.change(input, { target: { value: "synthetic input" } });
  view.rerender(
    <CommandInput
      connected
      send={send}
      onError={onError}
      remoteEcho={false}
      maskingGeneration="1"
    />,
  );
  expect(input).toHaveAttribute("type", "password");
  expect(
    screen.getByText("Draft stays masked until sent or cleared"),
  ).toBeInTheDocument();
  fireEvent.click(screen.getByText("Send"));
  await waitFor(() => expect(onError).toHaveBeenCalledWith("Queue full"));
  expect(input).toHaveAttribute("type", "password");
  expect(input).toHaveValue("synthetic input");
  fireEvent.click(screen.getByText("Send"));
  await waitFor(() => expect(input).toHaveValue(""));
  expect(input).toHaveAttribute("type", "text");
  fireEvent.change(input, { target: { value: "draft" } });
  fireEvent.click(screen.getByLabelText("Mask input"));
  fireEvent.click(screen.getByLabelText("Mask input"));
  expect(input).toHaveAttribute("type", "password");
  fireEvent.change(input, { target: { value: "" } });
  expect(input).toHaveAttribute("type", "text");
});
it("remembers a transient server mask and resets only for a new connection", () => {
  const props = { connected: true, send: vi.fn(), onError: vi.fn() };
  const view = render(
    <CommandInput key="1" {...props} maskingGeneration="0" />,
  );
  fireEvent.change(screen.getByLabelText("Command"), {
    target: { value: "draft" },
  });
  view.rerender(<CommandInput key="1" {...props} maskingGeneration="2" />);
  expect(screen.getByLabelText("Command")).toHaveAttribute("type", "password");
  view.rerender(<CommandInput key="2" {...props} maskingGeneration="0" />);
  expect(screen.getByLabelText("Command")).toHaveValue("");
  expect(screen.getByLabelText("Command")).toHaveAttribute("type", "text");
});
it("manual force masking survives server disabling and empty drafts", () => {
  const props = { connected: true, send: vi.fn(), onError: vi.fn() };
  const view = render(
    <CommandInput {...props} remoteEcho maskingGeneration="1" />,
  );
  fireEvent.click(screen.getByLabelText("Mask input"));
  view.rerender(<CommandInput {...props} maskingGeneration="1" />);
  expect(screen.getByLabelText("Command")).toHaveAttribute("type", "password");
  fireEvent.click(screen.getByLabelText("Mask input"));
  expect(screen.getByLabelText("Command")).toHaveAttribute("type", "text");
});
