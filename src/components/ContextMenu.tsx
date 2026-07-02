import { useLayoutEffect, useRef } from "react";

/** One row of the menu: an action, or a separator when `separator` is set. */
export type ContextMenuItem =
  | { separator: true }
  | {
      separator?: undefined;
      label: string;
      /** Greyed out and unclickable (e.g. Paste with an empty clipboard). */
      disabled?: boolean;
      onClick: () => void;
    };

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

/**
 * The app's own right-click menu. The webview's default menu is suppressed
 * app-wide (see the root `onContextMenu` in App.tsx) — this replaces it with
 * product actions. Fixed-position, clamped to the viewport; closes on outside
 * pointer-down, Escape, another context-click, resize, or after any action.
 */
export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  // Clamp into the viewport once rendered (menus opened near the bottom/right
  // edge flip inward rather than clipping).
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    el.style.left = `${Math.min(x, window.innerWidth - r.width - 8)}px`;
    el.style.top = `${Math.min(y, window.innerHeight - r.height - 8)}px`;
  }, [x, y, items]);

  // Global dismissal. Capture phase so a click that lands on some other
  // interactive element still closes the menu first.
  useLayoutEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return (
    <div ref={ref} className="context-menu" style={{ left: x, top: y }} role="menu">
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="context-menu__separator" role="separator" />
        ) : (
          <button
            key={i}
            type="button"
            role="menuitem"
            className="context-menu__item"
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onClick();
            }}
          >
            {item.label}
          </button>
        )
      )}
    </div>
  );
}
