import { useRef, type KeyboardEvent, type PointerEvent, type RefObject } from "react";

interface Props {
  /** Layout axis of the enclosing split: a row is `horizontal`, a column `vertical`. */
  orientation: "horizontal" | "vertical";
  /** Index of the divider: it sits between `sizes[index]` and `sizes[index + 1]`. */
  index: number;
  /** The enclosing split's current weights, one per child. */
  sizes: number[];
  /** The split container, measured to convert pointer pixels into weight deltas. */
  containerRef: RefObject<HTMLDivElement | null>;
  onResize: (sizes: number[]) => void;
}

/** Neither adjacent side may drop below 10% of their combined weight. */
const MIN_FRACTION = 0.1;
/** Keyboard resize step, as a fraction of the pair's combined weight. */
const KEY_STEP = 0.02;

interface DragState {
  pointerId: number;
  origin: number;
  sizes: number[];
}

/**
 * Draggable divider between two adjacent split siblings. Pointer drag or the
 * arrow keys matching its axis rebalance the pair's weights (clamped) and report
 * the new full size vector through `onResize`.
 */
export function SplitDivider({ orientation, index, sizes, containerRef, onResize }: Props) {
  const drag = useRef<DragState | null>(null);

  // The divider line runs across the layout axis: side-by-side panes get a
  // vertical line, stacked panes a horizontal one.
  const lineOrientation = orientation === "horizontal" ? "vertical" : "horizontal";

  function extent(): number {
    const el = containerRef.current;
    if (!el) return 0;
    const rect = el.getBoundingClientRect();
    return orientation === "horizontal" ? rect.width : rect.height;
  }

  function rebalance(delta: number, base: number[]): void {
    const left = base[index];
    const right = base[index + 1];
    const combined = left + right;
    const min = combined * MIN_FRACTION;
    const nextLeft = Math.min(Math.max(left + delta, min), combined - min);
    const nextRight = combined - nextLeft;
    if (nextLeft === left && nextRight === right) return;
    const next = [...base];
    next[index] = nextLeft;
    next[index + 1] = nextRight;
    onResize(next);
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      origin: orientation === "horizontal" ? event.clientX : event.clientY,
      sizes: [...sizes],
    };
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>): void {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const span = extent();
    if (span <= 0) return;
    const position = orientation === "horizontal" ? event.clientX : event.clientY;
    rebalance((position - current.origin) / span, current.sizes);
  }

  function handlePointerUp(event: PointerEvent<HTMLDivElement>): void {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    let delta = 0;
    if (orientation === "horizontal") {
      if (event.key === "ArrowLeft") delta = -KEY_STEP;
      else if (event.key === "ArrowRight") delta = KEY_STEP;
    } else {
      if (event.key === "ArrowUp") delta = -KEY_STEP;
      else if (event.key === "ArrowDown") delta = KEY_STEP;
    }
    if (delta === 0) return;
    event.preventDefault();
    rebalance(delta, [...sizes]);
  }

  return (
    <div
      className={`split-divider split-divider--${lineOrientation}`}
      role="separator"
      aria-orientation={lineOrientation}
      aria-label="Resize panes"
      tabIndex={0}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      onKeyDown={handleKeyDown}
    />
  );
}
