import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Rect } from "../shared/overlay";

/** Mirrors `FxEvent` in src-tauri/src/fx.rs. Rects are in this window's CSS px. */
type FxEvent = {
  id: number;
  kind: "genie-out" | "genie-in" | "tv-off";
  width: number;
  height: number;
  window: Rect;
  icon: Rect | null;
};

const GENIE_MS = 560;
const TV_OFF_MS = 500;
/** Horizontal slices the genie bends the snapshot into. More = smoother curves. */
const GENIE_SLICES = 180;
/** After a restore, the last frame stays this long so the real window can appear under it. */
const HANDOFF_MS = 80;

/**
 * Full-screen, click-through layer that plays the dock's window animations.
 * Protocol with Rust (fx.rs): fetch the snapshot, paint the first frame, call `fx_ready`
 * (the real window minimizes/closes under it), animate, call `fx_done` (a restore shows the window).
 */
export default function Fx() {
  const [effects, setEffects] = useState<FxEvent[]>([]);
  const [selfTest, setSelfTest] = useState(import.meta.env.DEV);

  // Diagnostics: what this layer looks like from the inside, and (dev builds only) a red square
  // for 3 s at startup, to check this window shows up on screen at all.
  useEffect(() => {
    log(`page ${window.innerWidth}x${window.innerHeight} css px, dpr ${window.devicePixelRatio}, ${document.visibilityState}`);
    const timer = setTimeout(() => setSelfTest(false), 3000);
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    const unlisten = getCurrentWebviewWindow().listen<FxEvent>("fx", (e) => setEffects((list) => [...list, e.payload]));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const remove = (id: number) => setEffects((list) => list.filter((e) => e.id !== id));

  return (
    <>
      {selfTest && (
        <div style={{ position: "fixed", left: "50%", top: "50%", width: 160, height: 160, margin: -80, background: "red" }} />
      )}
      {effects.map((effect) =>
        effect.kind === "tv-off" ? (
          <TvOff key={effect.id} effect={effect} onDone={() => remove(effect.id)} />
        ) : (
          <Genie key={effect.id} effect={effect} onDone={() => remove(effect.id)} />
        ),
      )}
    </>
  );
}

/** The snapshot's raw RGBA pixels, as something a canvas can draw. */
async function loadSnapshot(effect: FxEvent): Promise<ImageBitmap> {
  const bytes = await invoke<ArrayBuffer>("get_fx_image", { id: effect.id });
  return createImageBitmap(new ImageData(new Uint8ClampedArray(bytes), effect.width, effect.height));
}

function log(message: string) {
  invoke("fx_log", { message });
}

const r = (rect: Rect | null) => (rect ? `${Math.round(rect.x)},${Math.round(rect.y)} ${Math.round(rect.width)}x${Math.round(rect.height)}` : "none");

const nextFrame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

function Genie({ effect, onDone }: { effect: FxEvent; onDone: () => void }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const element = canvas.current!;
      const dpr = window.devicePixelRatio;
      element.width = window.innerWidth * dpr;
      element.height = window.innerHeight * dpr;
      const ctx = element.getContext("2d")!;
      ctx.scale(dpr, dpr);
      const image = await loadSnapshot(effect);
      // Minimizing goes window -> icon (p 0 -> 1); restoring plays it backwards.
      const progress = (t: number) => (effect.kind === "genie-out" ? t : 1 - t);
      const frame = (t: number) => {
        ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
        drawGenie(ctx, image, effect.window, effect.icon!, progress(t));
      };

      frame(0);
      await nextFrame(); // first frame is on screen
      await invoke("fx_ready", { id: effect.id });

      const start = performance.now();
      let frames = 0;
      for (let t = 0; t < 1 && !cancelled; frames++) {
        t = Math.min(1, (await nextFrame() - start) / GENIE_MS);
        frame(t);
      }
      log(
        `${effect.kind}: window ${r(effect.window)}, icon ${r(effect.icon)}, image ${image.width}x${image.height}, ` +
          `canvas ${element.width}x${element.height}, ${frames} frames, ${document.visibilityState}`,
      );
      await invoke("fx_done", { id: effect.id });
      if (effect.kind === "genie-in") await new Promise((r) => setTimeout(r, HANDOFF_MS));
      onDone();
    })();
    return () => {
      cancelled = true;
    };
    // Runs once per effect; onDone only removes this effect, so it isn't a dependency.
  }, [effect]);

  return <canvas ref={canvas} style={{ position: "fixed", inset: 0, width: "100%", height: "100%" }} />;
}

/**
 * macOS-style genie at progress p (0 = the window, 1 = inside the icon).
 *
 * The sides follow a funnel: an S-curve from the window's left/right edges down to the icon's.
 * Early on, the sides bend into that funnel and the bottom edge is pulled down into the icon;
 * then the rest of the window slides down through the funnel. The snapshot is drawn as thin
 * horizontal slices, each as wide as the funnel at the height it currently sits.
 */
function drawGenie(ctx: CanvasRenderingContext2D, image: ImageBitmap, win: Rect, icon: Rect, p: number) {
  const bend = easeOutCubic(clamp(p / 0.4)); // how far the sides have bent into the funnel
  const topY = lerp(win.y, icon.y, easeInCubic(clamp((p - 0.1) / 0.9)));
  const bottomY = lerp(win.y + win.height, icon.y + icon.height, easeInOutCubic(clamp(p / 0.6)));
  const funnelLength = Math.max(1, icon.y - win.y);

  ctx.globalAlpha = p > 0.9 ? (1 - p) / 0.1 : 1;
  const srcSlice = image.height / GENIE_SLICES;
  const dstSlice = (bottomY - topY) / GENIE_SLICES;
  for (let i = 0; i < GENIE_SLICES; i++) {
    const y = topY + i * dstSlice;
    const along = smoothstep(clamp((y + dstSlice / 2 - win.y) / funnelLength));
    const funnelLeft = lerp(win.x, icon.x, along);
    const funnelRight = lerp(win.x + win.width, icon.x + icon.width, along);
    const left = lerp(win.x, funnelLeft, bend);
    const right = lerp(win.x + win.width, funnelRight, bend);
    ctx.drawImage(image, 0, i * srcSlice, image.width, srcSlice, left, y, right - left, dstSlice + 0.6);
  }
  ctx.globalAlpha = 1;
}

/** Old CRT switching off: collapses to a bright line, then to a dot, then fades. */
function TvOff({ effect, onDone }: { effect: FxEvent; onDone: () => void }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [playing, setPlaying] = useState(false);
  const { window: win } = effect;

  useEffect(() => {
    (async () => {
      const element = canvas.current!;
      element.width = effect.width;
      element.height = effect.height;
      element.getContext("2d")!.drawImage(await loadSnapshot(effect), 0, 0);
      await nextFrame();
      await invoke("fx_ready", { id: effect.id });
      setPlaying(true);
    })();
  }, [effect]);

  return (
    <motion.div
      style={{
        position: "fixed",
        left: win.x,
        top: win.y,
        width: win.width,
        height: win.height,
        overflow: "hidden",
        background: "#fff",
        boxShadow: "0 0 24px 6px rgba(255,255,255,0.8)",
      }}
      initial={{ scaleY: 1, scaleX: 1, opacity: 1 }}
      animate={playing ? { scaleY: [1, 0.004, 0.004, 0.004], scaleX: [1, 1, 0.003, 0.003], opacity: [1, 1, 1, 0] } : undefined}
      transition={{ duration: TV_OFF_MS / 1000, times: [0, 0.45, 0.8, 1], ease: ["easeIn", "easeIn", "linear"] }}
      onAnimationComplete={() => playing && invoke("fx_done", { id: effect.id }).then(onDone)}
    >
      <motion.canvas
        ref={canvas}
        style={{ width: "100%", height: "100%", display: "block" }}
        animate={playing ? { filter: ["brightness(1)", "brightness(3)"], opacity: [1, 0] } : undefined}
        transition={{ duration: 0.25, ease: "easeIn" }}
      />
    </motion.div>
  );
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const clamp = (t: number) => Math.max(0, Math.min(1, t));
const smoothstep = (t: number) => t * t * (3 - 2 * t);
const easeOutCubic = (t: number) => 1 - (1 - t) ** 3;
const easeInCubic = (t: number) => t ** 3;
const easeInOutCubic = (t: number) => (t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2);
