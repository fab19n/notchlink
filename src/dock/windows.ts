import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Rect } from "../shared/overlay";

/** Mirrors `AppWindow` in src-tauri/src/win.rs. */
export type AppWindow = { hwnd: number; title: string; exePath: string; isForeground: boolean; isMinimized: boolean };

/** One dock icon: all open windows of one program, front-most first. */
export type DockApp = { exePath: string; name: string; windows: AppWindow[]; isForeground: boolean };

/**
 * Open apps, in a stable order: an app keeps its place while it stays open and
 * new apps join at the end (Windows reports windows in z-order, which changes on every focus).
 */
export function useDockApps(): DockApp[] {
  const [windows, setWindows] = useState<AppWindow[]>([]);
  const [order, setOrder] = useState<string[]>([]);

  useEffect(() => {
    invoke<AppWindow[]>("get_windows").then(setWindows);
    const unlisten = getCurrentWebviewWindow().listen<AppWindow[]>("windows-changed", (e) => setWindows(e.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const open = new Set(windows.map((w) => w.exePath));
    setOrder((prev) => {
      const kept = prev.filter((p) => open.has(p));
      const added = [...open].filter((p) => !kept.includes(p));
      return [...kept, ...added];
    });
  }, [windows]);

  return order
    .map((exePath) => {
      const appWindows = windows.filter((w) => w.exePath === exePath);
      return {
        exePath,
        name: appName(exePath),
        windows: appWindows,
        isForeground: appWindows.some((w) => w.isForeground),
      };
    })
    .filter((app) => app.windows.length > 0);
}

/** "C:\\Program Files\\Foo\\foo.exe" -> "foo" */
function appName(exePath: string) {
  const file = exePath.split("\\").pop() ?? exePath;
  return file.replace(/\.exe$/i, "");
}

/** App icon as a data URL, loaded once per program. */
export function useIcon(exePath: string) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    invoke<string>("get_icon", { exePath }).then(setUrl, (err) => console.warn(err));
  }, [exePath]);
  return url;
}

export function activate(hwnd: number) {
  return invoke("activate_window", { hwnd });
}

/** Minimize with the genie animation into `icon` (the dock icon's rect in this window). */
export function minimize(hwnd: number, icon: Rect) {
  return invoke("minimize_window", { hwnd, icon });
}

/** Restore a minimized window with the genie coming out of `icon`. */
export function restore(hwnd: number, icon: Rect) {
  return invoke("restore_window", { hwnd, icon });
}

/** Click on an app's dock icon: minimize it if it's in front, otherwise bring it up. */
export function toggle(app: DockApp, icon: Rect) {
  const front = app.windows[0];
  if (app.isForeground) return minimize(front.hwnd, icon);
  if (front.isMinimized) return restore(front.hwnd, icon);
  return activate(front.hwnd);
}

/** Close with the TV switch-off animation. */
export function closeWindow(hwnd: number) {
  return invoke("close_window", { hwnd });
}
