import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/**
 * Collapsible, resizable navigation rail (design-review requirement).
 *
 * - Draggable right edge resizes between RAIL_MIN and RAIL_MAX px.
 * - Collapse (toggle or ⌘B) shrinks to an icon rail; a double-click on the
 *   edge or the toggle restores the last width.
 * - Width and collapsed state persist per-machine (localStorage until the
 *   app-state store lands in M1-3).
 */

const RAIL_MIN = 200;
const RAIL_MAX = 340;
const RAIL_COLLAPSED = 68;
const STORE_WIDTH = "aij.rail.width";
const STORE_COLLAPSED = "aij.rail.collapsed";

export interface RailItem {
  id: string;
  label: string;
  icon: ReactNode;
}

export function Rail({
  items,
  active,
  onSelect,
  footer,
  header,
}: {
  items: RailItem[];
  active: string;
  onSelect: (id: string) => void;
  /** Render-prop so the footer can adapt to the collapsed state (status
   * text hidden, live dot centered). */
  footer?: (collapsed: boolean) => ReactNode;
  /** Render-prop so the header can adapt to the collapsed state (e.g.
   * brand text hidden, logo centered). */
  header?: (collapsed: boolean) => ReactNode;
}) {
  const [width, setWidth] = useState(() => {
    const stored = Number(localStorage.getItem(STORE_WIDTH));
    return stored >= RAIL_MIN && stored <= RAIL_MAX ? stored : 232;
  });
  const [collapsed, setCollapsed] = useState(
    () => localStorage.getItem(STORE_COLLAPSED) === "1",
  );

  useEffect(() => {
    localStorage.setItem(STORE_WIDTH, String(width));
    localStorage.setItem(STORE_COLLAPSED, collapsed ? "1" : "0");
  }, [width, collapsed]);

  // ⌘B / Ctrl+B toggles the rail — native-feeling chrome shortcut.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
        event.preventDefault();
        setCollapsed((value) => !value);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const dragging = useRef(false);
  const onResizeStart = useCallback(
    (event: React.PointerEvent) => {
      if (collapsed) return;
      event.preventDefault();
      dragging.current = true;
      const startX = event.clientX;
      const startWidth = width;
      const onMove = (move: PointerEvent) => {
        const next = startWidth + (move.clientX - startX);
        setWidth(Math.min(RAIL_MAX, Math.max(RAIL_MIN, next)));
      };
      const onUp = () => {
        dragging.current = false;
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    },
    [collapsed, width],
  );

  const effectiveWidth = collapsed ? RAIL_COLLAPSED : width;

  return (
    <nav
      className="relative flex shrink-0 flex-col border-r border-hairline bg-surface/70 backdrop-blur-xl transition-[width] duration-200"
      style={{ width: effectiveWidth }}
    >
      {header?.(collapsed)}

      <button
        onClick={() => setCollapsed((value) => !value)}
        title={collapsed ? "Expand sidebar (⌘B)" : "Collapse sidebar (⌘B)"}
        className="absolute -right-3 top-[52px] z-10 flex size-6 items-center justify-center
          rounded-md border border-hairline bg-surface-2 text-faint shadow-md
          transition-colors hover:text-ink"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={`transition-transform duration-200 ${collapsed ? "" : "rotate-180"}`}
        >
          <path d="m9 6 6 6-6 6" />
        </svg>
      </button>

      <div className="flex flex-col gap-0.5 px-3 pt-2">
        {items.map((item) => {
          const isActive = item.id === active;
          return (
            <button
              key={item.id}
              onClick={() => onSelect(item.id)}
              title={item.label}
              className={`group relative flex h-9 items-center rounded-lg text-sm
                transition-all duration-150 cursor-default
                ${collapsed ? "justify-center px-0" : "gap-2.5 px-3"}
                ${
                  isActive
                    ? "bg-accent-soft text-ink"
                    : "text-muted hover:bg-surface-2 hover:text-ink"
                }`}
            >
              {isActive && (
                <span
                  className="absolute left-0 top-1/2 h-4 w-[3px] -translate-y-1/2
                    rounded-full bg-accent shadow-glow-accent"
                />
              )}
              <span className={isActive ? "text-accent-hi" : ""}>{item.icon}</span>
              {!collapsed && <span className="truncate">{item.label}</span>}
            </button>
          );
        })}
      </div>

      {footer && (
        <div className={`mt-auto p-4 text-[11px] text-faint ${collapsed ? "px-2" : ""}`}>
          {footer(collapsed)}
        </div>
      )}

      {/* Resize edge */}
      <div
        onPointerDown={onResizeStart}
        onDoubleClick={() => setCollapsed((value) => !value)}
        className="absolute inset-y-0 right-0 z-10 w-1.5 cursor-col-resize
          transition-colors hover:bg-accent/40 active:bg-accent/60"
      />
    </nav>
  );
}
