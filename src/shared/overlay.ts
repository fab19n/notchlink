import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** A rectangle in CSS pixels relative to the window's top-left corner. */
export type Rect = { x: number; y: number; width: number; height: number };

/** Mirrors `HitArea` in src-tauri/src/hit.rs. */
type HitArea =
  | ({ kind: "rect" } & Rect)
  | { kind: "bottomEdge"; height: number }
  | { kind: "rectAndBottomEdge"; rect: Rect; height: number };

/** Tells Rust which part of this window should catch the mouse; the rest is click-through. */
export function setHitRect(rect: Rect) {
  return setHitArea({ kind: "rect", ...rect });
}

/** Catch the mouse along the whole bottom edge of the screen (the hidden dock's wake-up strip). */
export function setHitBottomEdge(height: number) {
  return setHitArea({ kind: "bottomEdge", height });
}

/** Catch the mouse in `rect` or anywhere along the bottom edge of the screen. */
export function setHitRectAndBottomEdge(rect: Rect, height: number) {
  return setHitArea({ kind: "rectAndBottomEdge", rect, height });
}

function setHitArea(area: HitArea) {
  return invoke("set_hit_area", { area });
}

/** True while the mouse is over this window's hit rect. Rust tracks this (see hit.rs). */
export function usePointerInside() {
  const [inside, setInside] = useState(false);
  useEffect(() => {
    const unlisten = getCurrentWebviewWindow().listen<boolean>("pointer", (e) => setInside(e.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
  return inside;
}
