import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { setHitRect, usePointerInside } from "../shared/overlay";

/** This window is either the volume panel (left edge) or the brightness panel (right edge). */
const KIND = getCurrentWebviewWindow().label as "volume" | "brightness";
const SIDE = KIND === "volume" ? "left" : "right";

/** Width of the strip along the screen edge that brings the panel out. */
const WAKE_STRIP = 3;
const HIDE_DELAY_MS = 400;
/** Level change per scroll-wheel notch (one notch is ~100 deltaY). */
const STEP_PER_NOTCH = 0.02;
/** Size of the curved corners where the panel meets the screen edge (keep in sync with --ear in edge.css). */
const EAR = 12;

export default function Edge() {
  const inside = usePointerInside();
  const panel = useRef<HTMLDivElement>(null);
  const track = useRef<HTMLDivElement>(null);

  // Slide out while the mouse is on the edge strip or the panel; slide back a moment after it leaves.
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (inside) {
      setShown(true);
      return;
    }
    const timer = setTimeout(() => setShown(false), HIDE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [inside]);

  // Hidden: a thin strip on the screen edge. Shown: the panel and its curved corners.
  useEffect(() => {
    const root = document.documentElement;
    if (!shown) {
      setHitRect({ x: SIDE === "left" ? 0 : root.clientWidth - WAKE_STRIP, y: 0, width: WAKE_STRIP, height: root.clientHeight });
      return;
    }
    const p = panel.current!;
    setHitRect({ x: p.offsetLeft, y: p.offsetTop - EAR, width: p.offsetWidth, height: p.offsetHeight + 2 * EAR });
  }, [shown]);

  const [level, setLevel] = useLevel(shown);

  const change = (next: number) => setLevel(Math.min(1, Math.max(0, next)));

  const fromPointer = (clientY: number) => {
    const r = track.current!.getBoundingClientRect();
    change(1 - (clientY - r.top) / r.height);
  };

  const hiddenX = SIDE === "left" ? -100 : 100;

  return (
    <div className={`edge-stage ${SIDE}`}>
      <motion.div
        ref={panel}
        className={`edge-panel ${SIDE}`}
        initial={false}
        animate={{ x: shown ? "0%" : `${hiddenX}%` }}
        transition={{ type: "spring", stiffness: 500, damping: 36 }}
        onWheel={(e) => level !== null && change(level - (e.deltaY / 100) * STEP_PER_NOTCH)}
      >
        <span className="edge-ear top" />
        <span className="edge-ear bottom" />
        <div
          ref={track}
          className={`edge-track ${level === null ? "unsupported" : ""}`}
          onPointerDown={(e) => {
            if (level === null) return;
            e.currentTarget.setPointerCapture(e.pointerId);
            fromPointer(e.clientY);
          }}
          onPointerMove={(e) => e.buttons === 1 && level !== null && fromPointer(e.clientY)}
        >
          <motion.div
            className="edge-fill"
            animate={{ height: `${(level ?? 0) * 100}%` }}
            transition={{ type: "spring", stiffness: 700, damping: 45 }}
          />
        </div>
        {KIND === "volume" ? <SpeakerIcon level={level ?? 0} /> : <SunIcon dim={level === null} />}
      </motion.div>
    </div>
  );
}

/**
 * The current level (0..1), re-read each time the panel opens so key presses made meanwhile show up.
 * Null for brightness when the screen has no software brightness control.
 */
function useLevel(shown: boolean): [number | null, (level: number) => void] {
  const [level, setLevel] = useState<number | null>(null);

  useEffect(() => {
    if (!shown) return;
    if (KIND === "volume") {
      invoke<{ level: number; muted: boolean }>("get_volume_level").then((v) => setLevel(v.muted ? 0 : v.level));
    } else {
      invoke<number | null>("get_brightness_level").then((b) => setLevel(b === null ? null : b / 100));
    }
  }, [shown]);

  const set = (next: number) => {
    setLevel(next);
    if (KIND === "volume") invoke("set_volume_level", { level: next });
    else invoke("set_brightness_level", { level: Math.round(next * 100) });
  };

  return [level, set];
}

function SpeakerIcon({ level }: { level: number }) {
  return (
    <svg className="edge-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9h4l5-4v14l-5-4H4z" fill="currentColor" />
      {level === 0 ? (
        <path d="M17 9l5 6M22 9l-5 6" />
      ) : (
        <>
          <path d="M16.5 9.5a3.5 3.5 0 0 1 0 5" />
          {level > 0.5 && <path d="M19 7a7 7 0 0 1 0 10" />}
        </>
      )}
    </svg>
  );
}

function SunIcon({ dim }: { dim: boolean }) {
  return (
    <svg className={`edge-icon ${dim ? "dim" : ""}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </svg>
  );
}
