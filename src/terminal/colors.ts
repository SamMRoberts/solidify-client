import type { CSSProperties } from "react";
import type { Color, NamedColor } from "../bridge/client";

// Preserve the original eight colors. Bright entries are fixed, never profile defaults.
export const ansiPalette = [
  "#111318",
  "#d66b72",
  "#83bd8a",
  "#d8bd78",
  "#7d9bd5",
  "#be8ecc",
  "#7ec5cb",
  "#e6e8eb",
  "#697586",
  "#ff8b92",
  "#a7e0af",
  "#ffe19a",
  "#a4bdff",
  "#e0b1ef",
  "#a5edf2",
  "#ffffff",
] as const;
const names: readonly NamedColor[] = [
  "default",
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
];
const byte = (value: number) =>
  Number.isInteger(value) && value >= 0 && value <= 255;
const rgb = (r: number, g: number, b: number) => `rgb(${r}, ${g}, ${b})`;
/** Only fixed palette entries or validated numeric channels become CSS. */
export function explicitColor(color: Color): string | undefined {
  if (typeof color === "string") return undefined;
  if (color?.kind === "rgb") {
    return [color.red, color.green, color.blue].every(byte)
      ? rgb(color.red, color.green, color.blue)
      : undefined;
  }
  if (color?.kind !== "indexed" || !byte(color.index)) return undefined;
  const index = color.index;
  if (index < 16) return ansiPalette[index];
  if (index >= 232) {
    const gray = 8 + 10 * (index - 232);
    return rgb(gray, gray, gray);
  }
  const offset = index - 16,
    levels = [0, 95, 135, 175, 215, 255];
  return rgb(
    levels[Math.floor(offset / 36)],
    levels[Math.floor(offset / 6) % 6],
    levels[offset % 6],
  );
}
export function colorKey(color: Color): string {
  if (typeof color === "string")
    return names.includes(color) ? color : "default";
  if (explicitColor(color) === undefined) return "default";
  return color.kind === "indexed"
    ? `index-${color.index}`
    : `rgb-${color.red}-${color.green}-${color.blue}`;
}
export function colorPresentation(
  foreground: Color,
  background: Color,
  inverse: boolean,
): { className: string; style: CSSProperties } {
  const fg = inverse ? background : foreground,
    bg = inverse ? foreground : background;
  const fgDefault = inverse ? "base" : "default",
    bgDefault = inverse ? "ink" : "default";
  const named = (color: Color, fallback: string) =>
    typeof color === "string" && color !== "default" && names.includes(color)
      ? color
      : fallback;
  return {
    className: `fg-${named(fg, fgDefault)} bg-${named(bg, bgDefault)}`,
    style: { color: explicitColor(fg), backgroundColor: explicitColor(bg) },
  };
}
