import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** A rectangle in CSS pixels relative to the window's top-left corner. */
export type Rect = { x: number; y: number; width: number; height: number };

/** Tells Rust which part of this window should catch the mouse; the rest is click-through. */
export function setHitRect(rect: Rect) {
  return invoke("set_hit_rect", { rect });
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
