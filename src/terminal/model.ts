import type { Output, Style } from "../bridge/client";
export const MAX_LINES = 2000,
  MAX_BYTES = 1024 * 1024,
  MAX_RUNS = 10000,
  MAX_LINE_BYTES = 16 * 1024;
export const defaultStyle = (): Style => ({
  foreground: "default",
  background: "default",
  bold: false,
  italic: false,
  underline: false,
  inverse: false,
});
export interface Run {
  text: string;
  style: Style;
  key: string;
}
export interface Line {
  id: number;
  runs: Run[];
  bytes: number;
}
const encoder = new TextEncoder();
const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const size = (value: string) => encoder.encode(value).length;
const key = (s: Style) =>
  [s.foreground, s.background, s.bold, s.italic, s.underline, s.inverse].join(
    ":",
  );

/** Bounded transcript semantics, deliberately not a terminal cell grid. */
export class TranscriptModel {
  lines: Line[] = [];
  bytes = 0;
  runs = 0;
  bells = 0;
  private nextId = 0;
  private style = defaultStyle();
  private carriageReturn = false;
  constructor() {
    this.newLine();
  }
  clear() {
    this.lines = [];
    this.bytes = 0;
    this.runs = 0;
    this.carriageReturn = false;
    this.newLine();
  }
  apply(events: Output[]) {
    for (const event of events) {
      if (event.type === "style") {
        this.style = { ...event.style };
        continue;
      }
      if (event.type === "text") {
        this.append(event.text);
        continue;
      }
      switch (event.control) {
        case "CarriageReturn":
          this.carriageReturn = true;
          break;
        case "LineFeed":
          this.carriageReturn = false;
          this.newLine();
          break;
        case "Backspace":
          this.backspace();
          break;
        case "Tab": {
          if (this.carriageReturn) this.replaceLine();
          const columns = Array.from(
            segmenter.segment(
              this.current()
                .runs.map((r) => r.text)
                .join(""),
            ),
          ).length;
          this.append(" ".repeat(8 - (columns % 8)));
          break;
        }
        case "Bell":
          this.bells++;
          break;
      }
    }
  }
  private current() {
    return this.lines[this.lines.length - 1];
  }
  private newLine() {
    this.lines.push({ id: this.nextId++, runs: [], bytes: 0 });
    this.trim();
  }
  private trim() {
    while (
      this.lines.length > 1 &&
      (this.lines.length > MAX_LINES ||
        this.bytes > MAX_BYTES ||
        this.runs > MAX_RUNS)
    ) {
      const line = this.lines.shift()!;
      this.bytes -= line.bytes;
      this.runs -= line.runs.length;
    }
  }
  private replaceLine() {
    const line = this.current();
    this.bytes -= line.bytes;
    this.runs -= line.runs.length;
    line.runs = [];
    line.bytes = 0;
    this.carriageReturn = false;
  }
  private append(text: string) {
    if (!text) return;
    if (this.carriageReturn) this.replaceLine();
    const styleKey = key(this.style);
    for (const character of text) {
      const bytes = size(character);
      if (this.current().bytes + bytes > MAX_LINE_BYTES) this.newLine();
      let line = this.current();
      let last = line.runs.at(-1);
      if (last?.key === styleKey) last.text += character;
      else {
        if (line.runs.length === MAX_RUNS) {
          this.newLine();
          line = this.current();
        }
        line.runs.push({
          text: character,
          style: { ...this.style },
          key: styleKey,
        });
        this.runs++;
      }
      line.bytes += bytes;
      this.bytes += bytes;
      this.trim();
    }
  }
  private backspace() {
    const line = this.current();
    const text = line.runs.map((r) => r.text).join("");
    const last = Array.from(segmenter.segment(text)).at(-1);
    if (!last) return;
    let remaining = text.length - last.index;
    while (remaining > 0) {
      const run = line.runs.at(-1)!;
      const count = Math.min(remaining, run.text.length);
      const removed = run.text.slice(run.text.length - count);
      run.text = run.text.slice(0, run.text.length - count);
      remaining -= count;
      const bytes = size(removed);
      line.bytes -= bytes;
      this.bytes -= bytes;
      if (!run.text) {
        line.runs.pop();
        this.runs--;
      }
    }
  }
}
