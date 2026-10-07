import { fireEvent, render, screen } from "@testing-library/react";
import { it, expect } from "vitest";
import { Transcript } from "./Transcript";
import { TranscriptModel, defaultStyle } from "./model";
import { defaultAppearance } from "../bridge/profiles";
it("applies appearance to retained and incoming defaults without changing explicit ANSI colors or style state", () => {
  const model = new TranscriptModel();
  model.apply([
    { type: "text", text: "default" },
    {
      type: "style",
      style: { ...defaultStyle(), foreground: "white", background: "black" },
    },
    { type: "text", text: "ansi" },
    { type: "style", style: { ...defaultStyle(), inverse: true } },
    { type: "text", text: "inverse defaults" },
    {
      type: "style",
      style: { ...defaultStyle(), foreground: "red", inverse: true },
    },
    { type: "text", text: "inverse red" },
  ]);
  const view = render(<Transcript model={model} revision={1} />);
  const appearance = {
    fontSize: 22,
    foreground: "#abcdef",
    background: "#123456",
  };
  view.rerender(
    <Transcript model={model} revision={1} appearance={appearance} />,
  );
  expect(
    screen
      .getByLabelText("Server output")
      .style.getPropertyValue("--transcript-font-size"),
  ).toBe("22px");
  expect(
    screen
      .getByLabelText("Server output")
      .style.getPropertyValue("--transcript-foreground"),
  ).toBe("#abcdef");
  expect(screen.getByText("default")).toHaveClass("fg-default", "bg-default");
  expect(screen.getByText("ansi")).toHaveClass("fg-white", "bg-black");
  expect(screen.getByText("inverse defaults")).toHaveClass("fg-base", "bg-ink");
  expect(screen.getByText("inverse red")).toHaveClass("fg-base", "bg-red");
  model.apply([{ type: "text", text: " incoming" }]);
  view.rerender(
    <Transcript model={model} revision={2} appearance={appearance} />,
  );
  expect(screen.getByText("inverse red incoming")).toHaveClass("bg-red");
});
it("font preview anchors a retained visible line without marking output unread or changing follow mode", () => {
  const model = new TranscriptModel();
  model.apply([{ type: "text", text: "visible line" }]);
  const view = render(<Transcript model={model} revision={1} />);
  const node = screen.getByLabelText("Transcript"),
    line = screen.getByText("visible line").parentElement!;
  Object.defineProperties(node, {
    scrollHeight: { value: 1000 },
    clientHeight: { value: 100 },
  });
  node.getBoundingClientRect = () => ({ top: 0 }) as DOMRect;
  let lineTop = -5;
  line.getBoundingClientRect = () =>
    ({ top: lineTop, bottom: lineTop + 20 }) as DOMRect;
  node.scrollTop = 200;
  fireEvent.scroll(node);
  lineTop = 95;
  view.rerender(
    <Transcript
      model={model}
      revision={1}
      appearance={{ ...defaultAppearance, fontSize: 24 }}
    />,
  );
  expect(node.scrollTop).toBe(300);
  expect(screen.queryByText("Latest output ↓")).toBeNull();
  model.apply([{ type: "text", text: " more" }]);
  view.rerender(
    <Transcript
      model={model}
      revision={2}
      appearance={{ ...defaultAppearance, fontSize: 24 }}
    />,
  );
  expect(screen.getByText("Latest output ↓")).toBeInTheDocument();
});
