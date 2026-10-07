import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { defaultAppearance, type Appearance } from "../bridge/profiles";
import { observeViewport } from "./viewport";
import type { ViewportSize, Style } from "../bridge/client";
import type { TranscriptModel } from "./model";

function classes(style: Style) {
  const foreground = style.inverse
    ? style.background === "default"
      ? "base"
      : style.background
    : style.foreground;
  const background = style.inverse
    ? style.foreground === "default"
      ? "ink"
      : style.foreground
    : style.background;
  return [
    `fg-${foreground}`,
    `bg-${background}`,
    style.bold ? "bold" : "",
    style.italic ? "italic" : "",
    style.underline ? "underline" : "",
  ].join(" ");
}
export function nearBottom(height: number, top: number, viewport: number) {
  return height - top - viewport <= 24;
}
export function Transcript({
  model,
  revision,
  onViewport,
  appearance = defaultAppearance,
}: {
  model: TranscriptModel;
  revision: number;
  appearance?: Appearance;
  onViewport?: (size: ViewportSize) => void;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (viewport.current && onViewport)
      return observeViewport(viewport.current, onViewport);
  }, [onViewport]);
  const follow = useRef(true);
  const anchor = useRef<{ id: string; offset: number } | null>(null);
  const lastRevision = useRef(revision);
  const [unread, setUnread] = useState(false);
  const [bell, setBell] = useState(false);
  const bellCount = useRef(model.bells);
  useLayoutEffect(() => {
    follow.current = true;
    anchor.current = null;
    bellCount.current = model.bells;
    setBell(false);
  }, [model]);
  useLayoutEffect(() => {
    const node = viewport.current;
    if (!node) return;
    if (follow.current) {
      node.scrollTop = node.scrollHeight;
      setUnread(false);
    } else {
      if (lastRevision.current !== revision) setUnread(true);
      const saved = anchor.current;
      const line = saved
        ? node.querySelector<HTMLElement>(`[data-line="${saved.id}"]`)
        : null;
      if (line && saved)
        node.scrollTop +=
          line.getBoundingClientRect().top -
          node.getBoundingClientRect().top -
          saved.offset;
    }
    lastRevision.current = revision;
  }, [
    model,
    revision,
    appearance.fontSize,
    appearance.foreground,
    appearance.background,
  ]);
  useLayoutEffect(() => {
    if (model.bells === bellCount.current) return;
    bellCount.current = model.bells;
    setBell(true);
    const timer = setTimeout(() => setBell(false), 250);
    return () => clearTimeout(timer);
  }, [model, model.bells]);
  function scrolled() {
    const node = viewport.current!;
    follow.current = nearBottom(
      node.scrollHeight,
      node.scrollTop,
      node.clientHeight,
    );
    if (follow.current) {
      setUnread(false);
      anchor.current = null;
      return;
    }
    const top = node.getBoundingClientRect().top;
    const line = Array.from(
      node.querySelectorAll<HTMLElement>("[data-line]"),
    ).find((line) => line.getBoundingClientRect().bottom > top);
    if (line)
      anchor.current = {
        id: line.dataset.line!,
        offset: line.getBoundingClientRect().top - top,
      };
  }
  return (
    <section
      className={`output ${bell ? "bell" : ""}`}
      aria-label="Server output"
      style={
        {
          "--transcript-font-size": `${appearance.fontSize}px`,
          "--transcript-foreground": appearance.foreground,
          "--transcript-background": appearance.background,
        } as CSSProperties
      }
    >
      <div
        className="transcript"
        ref={viewport}
        onScroll={scrolled}
        tabIndex={0}
        aria-label="Transcript"
      >
        {model.lines.map((line) => (
          <div className="line" key={line.id} data-line={line.id}>
            {line.runs.length ? (
              line.runs.map((run, index) => (
                <span key={index} className={classes(run.style)}>
                  {run.text}
                </span>
              ))
            ) : (
              <br />
            )}
          </div>
        ))}
      </div>
      {unread && (
        <button
          className="latest"
          onClick={() => {
            follow.current = true;
            viewport.current!.scrollTop = viewport.current!.scrollHeight;
            setUnread(false);
          }}
        >
          Latest output ↓
        </button>
      )}
    </section>
  );
}
