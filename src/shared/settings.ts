import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `Settings` in src-tauri/src/settings.rs. */
export type Settings = {
  islandOpen: "peek" | "hover";
  replaceTaskbar: boolean;
  dockAutohide: boolean;
  dockHeight: number;
  iconSize: number;
};

/** Limits enforced in Rust too (settings.rs). */
export const DOCK_HEIGHT_RANGE = [44, 110] as const;
export const ICON_SIZE_MIN = 24;
export const DOCK_PADDING = 8;

/** Current settings, kept live; null until loaded. */
export function useSettings(): Settings | null {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
    const unlisten = listen<Settings>("settings-changed", (e) => setSettings(e.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
  return settings;
}

export function updateSettings(settings: Settings) {
  return invoke("update_settings", { settings });
}

export function openSettings() {
  return invoke("open_settings");
}

export function quit() {
  return invoke("quit");
}
