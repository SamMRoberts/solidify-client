import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  ansiPalette,
  colorKey,
  colorPresentation,
  explicitColor,
} from "./colors";
import { defaultStyle, MAX_RUNS, TranscriptModel } from "./model";
import { Transcript } from "./Transcript";
import type { Color } from "../bridge/client";

it("maps the fixed ANSI palette, cube corners and grayscale endpoints exactly", () => {
  expect(ansiPalette.slice(0, 8)).toEqual([
    "#111318",
    "#d66b72",
    "#83bd8a",
    "#d8bd78",
    "#7d9bd5",
    "#be8ecc",
    "#7ec5cb",
    "#e6e8eb",
  ]);
  for (let index = 0; index < 16; index++)
    expect(explicitColor({ kind: "indexed", index })).toBe(ansiPalette[index]);
  for (const [index, expected] of [
    [16, "rgb(0, 0, 0)"],
    [17, "rgb(0, 0, 95)"],
    [21, "rgb(0, 0, 255)"],
    [46, "rgb(0, 255, 0)"],
    [196, "rgb(255, 0, 0)"],
    [231, "rgb(255, 255, 255)"],
    [232, "rgb(8, 8, 8)"],
    [255, "rgb(238, 238, 238)"],
  ] as const)
    expect(explicitColor({ kind: "indexed", index })).toBe(expected);
  for (let index = 16; index < 256; index++)
    expect(explicitColor({ kind: "indexed", index })).toMatch(
      /^rgb\(\d+, \d+, \d+\)$/,
    );
  expect(explicitColor({ kind: "rgb", red: 0, green: 127, blue: 255 })).toBe(
    "rgb(0, 127, 255)",
  );
});
it("ignores malformed color records and never interprets CSS or markup from them", () => {
  for (const invalid of [
    null,
    undefined,
    "url(evil)",
    "<script>",
    { kind: "rgb", red: "0);background:url(evil)", green: 0, blue: 0 },
    { kind: "rgb", red: -1, green: 0, blue: 0 },
    { kind: "rgb", red: 0.5, green: 0, blue: 0 },
    { kind: "rgb", red: 256, green: 0, blue: 0 },
    { kind: "indexed", index: NaN },
    { kind: "indexed", index: Infinity },
    { kind: "indexed", index: -1 },
    { kind: "indexed", index: 256 },
    { kind: "css", value: "url(evil)" },
  ]) {
    const color = invalid as Color;
    expect(explicitColor(color)).toBeUndefined();
    expect(colorKey(color)).toBe("default");
    expect(colorPresentation(color, color, false)).toEqual({
      className: "fg-default bg-default",
      style: { color: undefined, backgroundColor: undefined },
    });
  }
});
it("coalesces equal numeric colors by value and keeps distinct colors in distinct bounded runs", () => {
  const model = new TranscriptModel();
  for (const red of [1, 1, 2, 1])
    model.apply([
      {
        type: "style",
        style: {
          ...defaultStyle(),
          foreground: { kind: "rgb", red, green: 3, blue: 4 },
        },
      },
      { type: "text", text: "x" },
    ]);
  expect(model.runs).toBe(3);
  expect(model.lines[0].runs.map((r) => r.text)).toEqual(["xx", "x", "x"]);
  model.clear();
  model.apply([{ type: "text", text: "after clear" }]);
  expect(model.lines[0].runs[0].style.foreground).toEqual({
    kind: "rgb",
    red: 1,
    green: 3,
    blue: 4,
  });
  for (let index = 0; index < MAX_RUNS + 1; index++)
    model.apply([
      {
        type: "style",
        style: {
          ...defaultStyle(),
          foreground: { kind: "indexed", index: index % 256 },
        },
      },
      { type: "text", text: "x" },
    ]);
  expect(model.runs).toBeLessThanOrEqual(MAX_RUNS);
  expect(new TranscriptModel().lines[0].runs).toHaveLength(0);
});
describe("rendered colors", () => {
  it("preserves explicit colors while profile defaults and inverse defaults change", () => {
    const model = new TranscriptModel();
    const write = (
      text: string,
      foreground: Color,
      background: Color,
      inverse = false,
    ) =>
      model.apply([
        {
          type: "style",
          style: { ...defaultStyle(), foreground, background, inverse },
        },
        { type: "text", text },
      ]);
    write("RGB", { kind: "rgb", red: 12, green: 34, blue: 56 }, "default");
    write("indexed", { kind: "indexed", index: 196 }, "default");
    write(
      "inverse",
      "default",
      { kind: "rgb", red: 99, green: 88, blue: 77 },
      true,
    );
    write(
      "mixed",
      { kind: "rgb", red: 1, green: 2, blue: 3 },
      { kind: "indexed", index: 21 },
      true,
    );
    write("default", "default", "default");
    const view = render(<Transcript model={model} revision={1} />);
    expect(screen.getByText("RGB")).toHaveStyle({ color: "rgb(12, 34, 56)" });
    expect(screen.getByText("indexed")).toHaveStyle({
      color: "rgb(255, 0, 0)",
    });
    expect(screen.getByText("inverse")).toHaveClass("bg-ink");
    expect(screen.getByText("inverse")).toHaveStyle({
      color: "rgb(99, 88, 77)",
    });
    expect(screen.getByText("mixed")).toHaveStyle({
      color: "rgb(0, 0, 255)",
      backgroundColor: "rgb(1, 2, 3)",
    });
    view.rerender(
      <Transcript
        model={model}
        revision={1}
        appearance={{
          fontSize: 20,
          foreground: "#aabbcc",
          background: "#112233",
        }}
      />,
    );
    expect(screen.getByText("RGB")).toHaveStyle({ color: "rgb(12, 34, 56)" });
    expect(screen.getByText("default")).toHaveClass("fg-default");
    expect(
      screen
        .getByLabelText("Server output")
        .style.getPropertyValue("--transcript-foreground"),
    ).toBe("#aabbcc");
    expect(screen.getByText("inverse")).toHaveClass("bg-ink");
  });
  it("keeps bold independent from bright and treats color-looking text literally", () => {
    const model = new TranscriptModel();
    model.apply([
      {
        type: "style",
        style: { ...defaultStyle(), foreground: "red", bold: true },
      },
      { type: "text", text: "bold red" },
      {
        type: "style",
        style: { ...defaultStyle(), foreground: { kind: "indexed", index: 9 } },
      },
      { type: "text", text: "bright red" },
      { type: "text", text: " <img src=x>" },
    ]);
    const view = render(<Transcript model={model} revision={1} />);
    expect(screen.getByText("bold red")).toHaveClass("fg-red", "bold");
    expect(screen.getByText("bright red <img src=x>")).not.toHaveClass("bold");
    expect(screen.getByText("bright red <img src=x>")).toHaveStyle({
      color: "rgb(255, 139, 146)",
    });
    expect(view.container.querySelector("img")).toBeNull();
  });
});
