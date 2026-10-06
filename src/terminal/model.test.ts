import { describe, it, expect } from "vitest";
import {
  TranscriptModel,
  defaultStyle,
  MAX_BYTES,
  MAX_LINES,
  MAX_RUNS,
  MAX_LINE_BYTES,
} from "./model";
import type { Output } from "../bridge/client";
const text = (text: string): Output => ({ type: "text", text });
const control = (
  control: Extract<Output, { type: "control" }>["control"],
): Output => ({ type: "control", control });
const plain = (model: TranscriptModel) =>
  model.lines.map((l) => l.runs.map((r) => r.text).join(""));
describe("bounded transcript", () => {
  it("preserves prompts, CRLF across batches, replacement lines, tabs and bell", () => {
    const m = new TranscriptModel();
    m.apply([text("Prompt> ")]);
    expect(plain(m)).toEqual(["Prompt> "]);
    m.apply([control("CarriageReturn")]);
    m.apply([control("LineFeed"), text("10%"), control("CarriageReturn")]);
    m.apply([
      text("100%"),
      control("LineFeed"),
      text("A"),
      control("Tab"),
      text("B"),
      control("Bell"),
    ]);
    expect(plain(m)).toEqual(["Prompt> ", "100%", "A       B"]);
    expect(m.bells).toBe(1);
  });
  it("deletes a whole grapheme across style and batch boundaries", () => {
    const m = new TranscriptModel();
    m.apply([
      text("Ae"),
      { type: "style", style: { ...defaultStyle(), bold: true } },
      text("\u0301"),
      control("Backspace"),
    ]);
    expect(plain(m)).toEqual(["A"]);
    expect(m.bytes).toBe(1);
    expect(m.runs).toBe(1);
    m.apply([text("👩‍💻"), control("Backspace")]);
    expect(plain(m)).toEqual(["A"]);
  });
  it("preserves style through clear and coalesces equal runs", () => {
    const m = new TranscriptModel();
    m.apply([
      { type: "style", style: { ...defaultStyle(), foreground: "red" } },
      text("a"),
      text("b"),
    ]);
    expect(m.runs).toBe(1);
    m.clear();
    m.apply([text("c")]);
    expect(m.lines[0].runs[0].style.foreground).toBe("red");
  });
  it("bounds lines and long current lines without splitting scalars", () => {
    const m = new TranscriptModel();
    for (let i = 0; i < 2100; i++) m.apply([text(`${i}`), control("LineFeed")]);
    expect(m.lines.length).toBe(MAX_LINES);
    expect(plain(m)[0]).toBe("101");
    m.clear();
    m.apply([text("a".repeat(MAX_LINE_BYTES - 1) + "🌍")]);
    expect(plain(m)).toEqual(["a".repeat(MAX_LINE_BYTES - 1), "🌍"]);
  });
  it("bounds total bytes and styled runs even within a single current line", () => {
    const m = new TranscriptModel();
    m.apply([text("a".repeat(MAX_BYTES + MAX_LINE_BYTES))]);
    expect(m.bytes).toBeLessThanOrEqual(MAX_BYTES);
    m.clear();
    for (let i = 0; i < MAX_RUNS + 1; i++)
      m.apply([
        { type: "style", style: { ...defaultStyle(), bold: i % 2 === 0 } },
        text("x"),
      ]);
    expect(m.runs).toBeLessThanOrEqual(MAX_RUNS);
    expect(m.lines.length).toBeLessThanOrEqual(MAX_LINES);
  });
});
