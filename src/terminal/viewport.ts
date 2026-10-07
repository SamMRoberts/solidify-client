import type { ViewportSize } from "../bridge/client";

export function characterDimensions(
  width: number,
  height: number,
  advance: number,
  lineHeight: number,
): ViewportSize {
  if (
    ![width, height, advance, lineHeight].every(
      (v) => Number.isFinite(v) && v > 0,
    )
  )
    return { columns: 80, rows: 24 };
  const bounded = (value: number) =>
    Math.max(1, Math.min(65535, Math.floor(value)));
  return {
    columns: bounded(width / advance),
    rows: bounded(height / lineHeight),
  };
}

/** Measures usable content and inherited monospace metrics, never server text. */
export function observeViewport(
  node: HTMLElement,
  receive: (size: ViewportSize) => void,
) {
  const probe = document.createElement("span");
  probe.textContent = "MMMMMMMMMM";
  probe.setAttribute("aria-hidden", "true");
  probe.className = "viewport-probe";
  node.append(probe);
  let stopped = false;
  let previous: ViewportSize | undefined;
  function measure() {
    if (stopped) return;
    const css = getComputedStyle(node);
    const size = characterDimensions(
      node.clientWidth -
        parseFloat(css.paddingLeft) -
        parseFloat(css.paddingRight),
      node.clientHeight -
        parseFloat(css.paddingTop) -
        parseFloat(css.paddingBottom),
      probe.getBoundingClientRect().width / 10,
      parseFloat(css.lineHeight),
    );
    if (size.columns !== previous?.columns || size.rows !== previous?.rows) {
      previous = size;
      receive(size);
    }
  }
  const observer =
    typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
  observer?.observe(node);
  observer?.observe(probe);
  measure();
  void document.fonts?.ready.then(measure);
  return () => {
    stopped = true;
    observer?.disconnect();
    probe.remove();
  };
}
