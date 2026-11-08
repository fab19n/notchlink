import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import Eyes from "./Eyes";
import Grid from "./Grid";
import { setHitRect, usePointerInside } from "../shared/overlay";
import { useSettings } from "../shared/settings";

type Mode = "idle" | "peek" | "expanded";

/** Island size per mode, in CSS px. `radius` rounds the bottom corners; the top sits flush with the screen edge. */
const SHAPES: Record<Mode, { width: number; height: number; radius: number }> = {
  idle: { width: 170, height: 32, radius: 14 },
  peek: { width: 220, height: 42, radius: 20 },
  expanded: { width: 600, height: 270, radius: 34 },
};

/** iOS-like: quick, with a small overshoot. Tune by feel. */
const MORPH = { type: "spring", stiffness: 380, damping: 30, mass: 0.9 } as const;

/** How long the expanded island waits after the mouse leaves before closing. */
const CLOSE_DELAY_MS = 450;

/** Extra px around the island that still counts as "on the island". */
const HIT_MARGIN = 6;

export default function Island() {
  const inside = usePointerInside();
  const hoverOpens = useSettings()?.islandOpen === "hover";
  const [mode, setMode] = useState<Mode>("idle");

  // Hover peeks (or opens fully, per settings); leaving closes, with a delay when
  // expanded so overshooting doesn't snap it shut.
  useEffect(() => {
    if (inside) {
      setMode((m) => (m === "idle" ? (hoverOpens ? "expanded" : "peek") : m));
      return;
    }
    if (mode === "expanded") {
      const timer = setTimeout(() => setMode("idle"), CLOSE_DELAY_MS);
      return () => clearTimeout(timer);
    }
    setMode("idle");
  }, [inside, mode, hoverOpens]);

  // Catch the mouse over whichever is larger, where the island is now or where it is going,
  // so it never turns click-through under the cursor mid-animation.
  const [settled, setSettled] = useState<Mode>("idle");
  useEffect(() => {
    reportHitRect(larger(SHAPES[mode], SHAPES[settled]));
  }, [mode, settled]);

  const shape = SHAPES[mode];
  const expanded = mode === "expanded";

  return (
    <div className="stage">
      <motion.div
        className="island"
        initial={false}
        animate={{
          width: shape.width,
          height: shape.height,
          borderBottomLeftRadius: shape.radius,
          borderBottomRightRadius: shape.radius,
        }}
        transition={MORPH}
        onAnimationComplete={() => setSettled(mode)}
        onClick={() => setMode("expanded")}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          {expanded ? (
            <motion.div
              key="expanded"
              className="expanded"
              initial={{ opacity: 0, filter: "blur(6px)" }}
              animate={{ opacity: 1, filter: "blur(0px)", transition: { delay: 0.08 } }}
              exit={{ opacity: 0, filter: "blur(6px)", transition: { duration: 0.12 } }}
            >
              <header className="expanded-header">
                <Eyes awake={false} size={0.8} />
                <button className="home" onClick={(e) => (e.stopPropagation(), setMode("peek"))}>
                  Close
                </button>
              </header>
              <Grid />
            </motion.div>
          ) : (
            <motion.div
              key="collapsed"
              className="collapsed"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0, transition: { duration: 0.1 } }}
            >
              <Eyes awake={mode === "peek"} />
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function larger(a: { width: number; height: number }, b: { width: number; height: number }) {
  return { width: Math.max(a.width, b.width), height: Math.max(a.height, b.height) };
}

function reportHitRect({ width, height }: { width: number; height: number }) {
  const windowWidth = document.documentElement.clientWidth;
  setHitRect({
    x: (windowWidth - width) / 2 - HIT_MARGIN,
    y: 0,
    width: width + HIT_MARGIN * 2,
    height: height + HIT_MARGIN,
  });
}
