/**
 * Typed bridge to the Rust backend. Types are generated from the Rust
 * definitions (ts-rs export) — never hand-written on this side.
 *
 * All calls degrade gracefully when running in a plain browser (vite dev
 * outside Tauri): the UI renders with placeholder state instead of failing.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppInfo } from "../types/generated/AppInfo";

export type { AppInfo };

/** Event names mirrored from juggler_app constants (single source: Rust). */
export const EVENTS = {
  appReady: "app://ready",
} as const;

export async function fetchAppInfo(): Promise<AppInfo | null> {
  try {
    return await invoke<AppInfo>("app_info");
  } catch {
    return null; // plain-browser dev: no backend, render placeholders
  }
}

export function onAppReady(handler: (info: AppInfo) => void): () => void {
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<AppInfo>(EVENTS.appReady, (event) => handler(event.payload))
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {
      // No backend in plain-browser dev; fetchAppInfo covers the fallback.
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}
