import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  hasLiveBackend,
  onTerminalExit,
  onTerminalOutput,
  terminalAttach,
  terminalKill,
  terminalResize,
  terminalSnapshot,
  terminalWrite,
} from "../lib/bridge";

/** A live terminal tab. */
export interface TerminalTab {
  id: string;
  title: string;
  /** Marks a finished process (scrollback stays until closed). */
  ended?: boolean;
}

const SCROLLBACK_LINES = 5_000;
const HEIGHT_MIN = 160;
const HEIGHT_MAX = 560;
const HEIGHT_KEY = "aij.term.height";
const COLLAPSED_KEY = "aij.term.collapsed";

function readNumber(key: string, fallback: number, min: number, max: number): number {
  const raw = Number(localStorage.getItem(key));
  if (!Number.isFinite(raw)) return fallback;
  return Math.min(max, Math.max(min, raw));
}

function decodeBase64(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * One xterm instance bound to a backend PTY: streams output events, sends
 * keystrokes, resizes with its container, and replays the retained tail on
 * (re)mount so closing a tab's panel and reopening loses nothing.
 */
function TerminalView({
  tab,
  visible,
}: {
  tab: TerminalTab;
  visible: boolean;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || termRef.current) return;

    const term = new Terminal({
      scrollback: SCROLLBACK_LINES,
      cursorBlink: true,
      fontSize: 12.5,
      fontFamily: "'SF Mono', 'JetBrains Mono', 'Fira Code', Menlo, monospace",
      allowProposedApi: true,
      theme: {
        background: "#05070b",
        foreground: "#cdd5e0",
        cursor: "#7c7cf4",
        selectionBackground: "#7c7cf440",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;

    if (hasLiveBackend()) {
      // Replay the retained tail, then attach for the live stream. The
      // backend keeps one forwarder per id; re-attach is idempotent.
      terminalSnapshot(tab.id).then((b64) => {
        if (b64) term.write(decodeBase64(b64));
      });
      terminalAttach(tab.id);
      term.onData((data) => {
        void terminalWrite(tab.id, data);
      });
    } else {
      // Design preview: a plausible guided-login transcript.
      term.write(
        [
          "\u001b[1;38;5;140mClaude Code\u001b[0m — sign in to finish setting up this account.",
          "",
          "  This terminal is isolated: it cannot see your other Claude accounts.",
          "",
          "  ? Press Enter to open the browser and sign in with your Anthropic account",
          "  …",
          "",
          "\u001b[38;5;245m(preview — run the app for a live terminal)\u001b[0m",
          "",
        ].join("\r\n"),
      );
    }

    const containerObserver = new ResizeObserver(() => {
      if (!host.offsetParent) return; // hidden panels can't be measured
      try {
        const dims = fit.proposeDimensions();
        if (dims && dims.cols > 0 && dims.rows > 0) {
          term.resize(dims.cols, dims.rows);
          if (hasLiveBackend()) void terminalResize(tab.id, dims.cols, dims.rows);
        }
      } catch {
        /* mid-teardown resizes are harmless */
      }
    });
    containerObserver.observe(host);

    return () => {
      containerObserver.disconnect();
      term.dispose();
      termRef.current = null;
    };
  }, [tab.id]);

  useEffect(() => {
    if (!visible || !fitRef.current || !hostRef.current?.offsetParent) return;
    try {
      fitRef.current.fit();
      termRef.current?.focus();
    } catch {
      /* panel mid-animation */
    }
  }, [visible]);

  // Live output stream → terminal. Listener lifetime matches the tab.
  useEffect(() => {
    if (!hasLiveBackend()) return;
    const off = onTerminalOutput(tab.id, (b64) => termRef.current?.write(decodeBase64(b64)));
    return off;
  }, [tab.id]);

  return (
    <div
      ref={hostRef}
      className={
        "h-full w-full px-3 pt-1 " + (visible ? "block" : "hidden")
      }
      data-terminal-host={tab.id}
    />
  );
}

/**
 * The terminal dock: tabbed embedded terminals at the bottom of the app,
 * drag-resizable, collapsible (⌘J), persisted layout — the home for
 * interactive sessions and guided logins.
 */
export function TerminalDock({
  tabs,
  activeId,
  onSelect,
  onClose,
  onExit,
}: {
  tabs: TerminalTab[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onExit: (id: string) => void;
}) {
  const [collapsed, setCollapsed] = useState(
    () => localStorage.getItem(COLLAPSED_KEY) === "1",
  );
  const [height, setHeight] = useState(() =>
    readNumber(HEIGHT_KEY, 300, HEIGHT_MIN, HEIGHT_MAX),
  );
  const dragging = useRef(false);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "j") {
        event.preventDefault();
        setCollapsed((was) => {
          localStorage.setItem(COLLAPSED_KEY, was ? "0" : "1");
          return !was;
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Mark tabs whose process exited (scrollback stays until closed).
  useEffect(() => {
    if (!hasLiveBackend()) return;
    return onTerminalExit(onExit);
  }, [onExit]);

  const beginDrag = useCallback((event: React.PointerEvent) => {
    event.preventDefault();
    dragging.current = true;
    const onMove = (move: PointerEvent) => {
      if (!dragging.current) return;
      const next = Math.min(
        HEIGHT_MAX,
        Math.max(HEIGHT_MIN, window.innerHeight - move.clientY),
      );
      setHeight(next);
    };
    const onUp = () => {
      dragging.current = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      setHeight((h) => {
        localStorage.setItem(HEIGHT_KEY, String(h));
        return h;
      });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }, []);

  const hasTabs = tabs.length > 0;
  const active = useMemo(
    () => tabs.find((t) => t.id === activeId) ?? null,
    [tabs, activeId],
  );

  if (!hasTabs && !collapsed) {
    // No terminals open: keep a slim collapsed strip so the dock's home is
    // discoverable, not a modal surprise.
    return null;
  }
  if (!hasTabs && collapsed) return null;

  return (
    <section
      aria-label="Terminal"
      style={{ height: collapsed ? 36 : height }}
      className={
        "relative shrink-0 border-t border-hairline bg-[#05070b] transition-[height] duration-150 " +
        (dragging.current ? "" : "ease-out")
      }
    >
      {/* drag handle */}
      <div
        onPointerDown={beginDrag}
        className="absolute -top-1 left-0 right-0 z-20 h-2 cursor-row-resize"
        aria-hidden
      />
      <header className="flex h-9 items-center gap-1 border-b border-hairline/60 px-2">
        <button
          type="button"
          onClick={() =>
            setCollapsed((was) => {
              localStorage.setItem(COLLAPSED_KEY, was ? "0" : "1");
              return !was;
            })
          }
          className="rounded-md px-2 py-1 text-[11px] text-muted hover:bg-surface hover:text-ink"
          title="Toggle terminal (⌘J)"
        >
          <span className="mr-1.5 inline-block size-1.5 rounded-full bg-live/80 align-middle" />
          Terminal
        </button>
        <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => onSelect(tab.id)}
              className={
                "group flex max-w-52 shrink-0 items-center gap-2 rounded-md px-2.5 py-1 text-[11px] transition-colors " +
                (tab.id === activeId
                  ? "bg-surface text-ink sheen"
                  : "text-muted hover:bg-surface/60 hover:text-ink")
              }
            >
              <span
                className={
                  "size-1.5 shrink-0 rounded-full " +
                  (tab.ended ? "bg-faint" : "bg-live animate-breathe")
                }
              />
              <span className="truncate">{tab.title}</span>
              <span
                role="button"
                tabIndex={-1}
                onClick={(event) => {
                  event.stopPropagation();
                  onClose(tab.id);
                }}
                className="rounded p-0.5 text-faint opacity-0 transition-opacity hover:text-ink group-hover:opacity-100"
                title="Close terminal"
              >
                ✕
              </span>
            </button>
          ))}
        </div>
        {active && !active.ended && hasLiveBackend() ? (
          <button
            type="button"
            onClick={() => void terminalKill(active.id)}
            className="rounded-md px-2 py-1 text-[11px] text-muted hover:bg-surface hover:text-ink"
            title="Stop the process (graceful)"
          >
            Stop
          </button>
        ) : null}
      </header>
      {!collapsed ? (
        <div className="h-[calc(100%-2.25rem)]">
          {tabs.map((tab) => (
            <TerminalView key={tab.id} tab={tab} visible={tab.id === activeId} />
          ))}
        </div>
      ) : null}
    </section>
  );
}
