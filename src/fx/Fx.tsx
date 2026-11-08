import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Rect } from "../shared/overlay";

/** Mirrors `FxEvent` in src-tauri/src/fx.rs. Rects are in this window's CSS px. */
type FxEvent = { kind: "genie" | "tv-off"; image: string; from: Rect; to: Rect | null };
type Effect = FxEvent & { id: number };

const GENIE_MS = 520;
/** Horizontal slices the genie bends the snapshot into. More = smoother curve. */
const GENIE_SLICES = 120;

/** Full-screen, click-through layer that plays the minimize and close animations. */
export default function Fx() {
  const [effects, setEffects] = useState<Effect[]>([]);

  useEffect(() => {
    let next = 0;
    const unlisten = getCurrentWebviewWindow().listen<FxEvent>("fx", (e) =>
      setEffects((list) => [...list, { ...e.payload, id: next++ }]),
    );
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const remove = (id: number) => setEffects((list) => list.filter((e) => e.id !== id));

  return (
    <>
      {effects.map((effect) =>
        effect.kind === "genie" ? (
          <Genie key={effect.id} effect={effect} onDone={() => remove(effect.id)} />
        ) : (
          <TvOff key={effect.id} effect={effect} onDone={() => remove(effect.id)} />
        ),
      )}
    </>
  );
}

/**
 * macOS-style genie: the snapshot is drawn as thin horizontal slices. The bottom slices
 * narrow toward the dock icon first and the top follows, which bends the sides into a funnel.
 */
function Genie({ effect, onDone }: { effect: Effect; onDone: () => void }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current!;
    const dpr = window.devicePixelRatio;
    element.width = window.innerWidth * dpr;
    element.height = window.innerHeight * dpr;
    const ctx = element.getContext("2d")!;
    ctx.scale(dpr, dpr);

    const { from, to } = effect;
    const target = to!;
    const image = new Image();
    let frame = 0;
    image.onload = () => {
      const start = performance.now();
      const draw = (now: number) => {
        const p = Math.min(1, (now - start) / GENIE_MS);
        ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
        ctx.globalAlpha = p > 0.85 ? (1 - p) / 0.15 : 1;

        const topY = lerp(from.y, target.y, easeInOut(p));
        const bottomY = lerp(from.y + from.height, target.y + target.height, easeOut(Math.min(1, p * 1.5)));
        const sliceSrc = image.height / GENIE_SLICES;
        const sliceDst = (bottomY - topY) / GENIE_SLICES;
        for (let i = 0; i < GENIE_SLICES; i++) {
          const v = (i + 0.5) / GENIE_SLICES; // 0 = top of window, 1 = bottom
          const pull = smoothstep(clamp(p * 1.6 - (1 - v) * 0.6));
          const left = lerp(from.x, target.x, pull);
          const right = lerp(from.x + from.width, target.x + target.width, pull);
          ctx.drawImage(image, 0, i * sliceSrc, image.width, sliceSrc, left, topY + i * sliceDst, right - left, sliceDst + 0.5);
        }

        if (p < 1) frame = requestAnimationFrame(draw);
        else onDone();
      };
      frame = requestAnimationFrame(draw);
    };
    image.src = effect.image;
    return () => cancelAnimationFrame(frame);
    // Runs once per effect; onDone only removes this effect, so it isn't a dependency.
  }, [effect]);

  return <canvas ref={canvas} style={{ position: "fixed", inset: 0, width: "100%", height: "100%" }} />;
}

/** Old CRT switching off: collapses to a bright line, then to a dot, then fades. */
function TvOff({ effect, onDone }: { effect: Effect; onDone: () => void }) {
  const { from } = effect;
  return (
    <motion.div
      style={{
        position: "fixed",
        left: from.x,
        top: from.y,
        width: from.width,
        height: from.height,
        overflow: "hidden",
        background: "#fff",
        boxShadow: "0 0 24px 6px rgba(255,255,255,0.8)",
      }}
      initial={{ scaleY: 1, scaleX: 1, opacity: 1 }}
      animate={{ scaleY: [1, 0.004, 0.004, 0.004], scaleX: [1, 1, 0.003, 0.003], opacity: [1, 1, 1, 0] }}
      transition={{ duration: 0.5, times: [0, 0.45, 0.8, 1], ease: ["easeIn", "easeIn", "linear"] }}
      onAnimationComplete={onDone}
    >
      <motion.img
        src={effect.image}
        draggable={false}
        style={{ width: "100%", height: "100%", display: "block" }}
        initial={{ filter: "brightness(1)", opacity: 1 }}
        animate={{ filter: ["brightness(1)", "brightness(3)"], opacity: [1, 0] }}
        transition={{ duration: 0.25, ease: "easeIn" }}
      />
    </motion.div>
  );
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const clamp = (t: number) => Math.max(0, Math.min(1, t));
const smoothstep = (t: number) => t * t * (3 - 2 * t);
const easeOut = (t: number) => 1 - (1 - t) ** 3;
const easeInOut = (t: number) => (t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2);
