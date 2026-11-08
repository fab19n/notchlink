import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, MotionValue, useMotionValue, useSpring, useTransform } from "motion/react";
import { setHitRect, usePointerInside } from "../shared/overlay";
import { activate, DockApp, useDockApps, useIcon } from "./windows";

const ICON = 46; // resting icon size, CSS px
const ICON_MAX = 72; // icon size right under the cursor
const REACH = 140; // how far (px) from the cursor icons start to grow
/** Room above the bar for magnified icons and the name label; also catches the mouse. */
const HEADROOM = 60;

export default function Dock() {
  const apps = useDockApps();
  const inside = usePointerInside();
  const mouseX = useMotionValue(Infinity);
  const bar = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!inside) mouseX.set(Infinity);
  }, [inside, mouseX]);

  // Keep the hit rect on the bar (plus headroom) as it changes size.
  useEffect(() => {
    const element = bar.current!;
    const report = () => {
      const r = element.getBoundingClientRect();
      setHitRect({ x: r.x, y: r.y - HEADROOM, width: r.width, height: r.height + HEADROOM });
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, []);

  return (
    <div className="dock-stage">
      <motion.div
        ref={bar}
        className="dock"
        onMouseMove={(e) => mouseX.set(e.clientX)}
        onMouseLeave={() => mouseX.set(Infinity)}
        layout
        transition={{ type: "spring", stiffness: 400, damping: 34 }}
      >
        <AnimatePresence initial={false}>
          {apps.map((app) => (
            <DockIcon key={app.exePath} app={app} mouseX={mouseX} />
          ))}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function DockIcon({ app, mouseX }: { app: DockApp; mouseX: MotionValue<number> }) {
  const ref = useRef<HTMLButtonElement>(null);
  const icon = useIcon(app.exePath);
  const [hovered, setHovered] = useState(false);

  // Magnify by distance between the cursor and this icon's centre.
  const distance = useTransform(mouseX, (x) => {
    const r = ref.current?.getBoundingClientRect();
    return r ? x - (r.x + r.width / 2) : Infinity;
  });
  const target = useTransform(distance, [-REACH, 0, REACH], [ICON, ICON_MAX, ICON], { clamp: true });
  const size = useSpring(target, { stiffness: 520, damping: 28, mass: 0.2 });

  return (
    <motion.button
      ref={ref}
      className="dock-icon"
      style={{ width: size, height: size }}
      initial={{ opacity: 0, y: 24, scale: 0.6 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.4, transition: { duration: 0.15 } }}
      whileTap={{ scale: 0.88 }}
      onHoverStart={() => setHovered(true)}
      onHoverEnd={() => setHovered(false)}
      onClick={() => activate(app.windows[0].hwnd)}
    >
      <AnimatePresence>
        {hovered && (
          <motion.span
            className="dock-label"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4, transition: { duration: 0.1 } }}
          >
            {app.windows[0].title || app.name}
          </motion.span>
        )}
      </AnimatePresence>
      {icon ? <img src={icon} alt={app.name} draggable={false} /> : <span className="dock-fallback">{app.name[0]}</span>}
      <span className={`dock-dot ${app.isForeground ? "active" : ""}`} />
    </motion.button>
  );
}
