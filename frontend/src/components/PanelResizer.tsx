import { useRef, type KeyboardEvent, type PointerEvent } from "react";

interface Props {
  /** Current panel width in pixels. */
  width: number;
  /** Lower clamp bound, in pixels. */
  min: number;
  /** Upper clamp bound, in pixels. */
  max: number;
  /** `1` when dragging right widens the panel (left panel); `-1` for a right panel. */
  grow: 1 | -1;
  /** Called with the new width, already clamped to `[min, max]`. */
  onWidth: (width: number) => void;
  /** Accessible name for the separator. */
  label: string;
}

/** Keyboard resize step, in pixels. */
const KEY_STEP = 16;

interface DragState {
  pointerId: number;
  originX: number;
  startWidth: number;
}

/**
 * Draggable vertical handle that resizes an adjacent panel by pixel width.
 * Pointer drag or the arrow keys change `width` (clamped to `[min, max]`) and
 * report the new value through `onWidth`.
 *
 * Unlike `SplitDivider` this is standalone and pixel-based rather than tied to
 * split-tree weights, so it can resize the free-standing side panels.
 */
export function PanelResizer({ width, min, max, grow, onWidth, label }: Props) {
  const drag = useRef<DragState | null>(null);

  function clamp(value: number): number {
    return Math.min(Math.max(value, min), max);
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      originX: event.clientX,
      startWidth: width,
    };
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>): void {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    onWidth(clamp(current.startWidth + grow * (event.clientX - current.originX)));
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
    if (event.key === "ArrowLeft") delta = -KEY_STEP;
    else if (event.key === "ArrowRight") delta = KEY_STEP;
    if (delta === 0) return;
    event.preventDefault();
    onWidth(clamp(width + grow * delta));
  }

  return (
    <div
      className="panel-resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(width)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      onKeyDown={handleKeyDown}
    />
  );
}
