import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, MotionValue, useMotionValue, useSpring, useTransform } from "motion/react";
import { Rect, setHitRect, usePointerInside } from "../shared/overlay";
import { openSettings, quit, Settings, useSettings } from "../shared/settings";
import { activate, closeWindow, DockApp, minimize, useDockApps, useIcon } from "./windows";

/** Icons grow to this multiple of their size under the cursor (keep in sync with src-tauri/src/dock.rs). */
const MAGNIFY = 1.55;
/** How far (in icon widths) from the cursor icons start to grow. */
const REACH = 3;
/** Auto-hide waits this long after the mouse leaves. */
const HIDE_DELAY_MS = 500;
/** With the dock hidden, this strip at the bottom of the screen brings it back. */
const WAKE_STRIP = 3;
/** Gap between the bar and the screen edge (keep in sync with BOTTOM_GAP in dock.rs). */
const BOTTOM_GAP = 8;

type Menu = { kind: "app"; app: DockApp; x: number; icon: Rect } | { kind: "dock"; x: number };

export default function Dock() {
  const settings = useSettings();
  if (!settings) return null;
  return <DockBar settings={settings} />;
}

function DockBar({ settings }: { settings: Settings }) {
  const apps = useDockApps();
  const inside = usePointerInside();
  const mouseX = useMotionValue(Infinity);
  const bar = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<Menu | null>(null);

  // Leaving the dock clears magnification and closes any menu.
  useEffect(() => {
    if (!inside) {
      mouseX.set(Infinity);
      setMenu(null);
    }
  }, [inside, mouseX]);

  // Auto-hide: show while the mouse is on the dock; hide a moment after it leaves.
  const [shown, setShown] = useState(true);
  useEffect(() => {
    if (!settings.dockAutohide || inside) {
      setShown(true);
      return;
    }
    const timer = setTimeout(() => setShown(false), HIDE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [settings.dockAutohide, inside]);

  // Where the window catches the mouse. Uses layout (offset*) values, so the slide animation doesn't move it.
  // Hidden: a thin strip at the screen edge. Shown: the bar. In use: the bar plus the space above it
  // (magnified icons, labels, menu). Everywhere else clicks go through to the apps behind.
  useEffect(() => {
    const element = bar.current!;
    const report = () => {
      const windowHeight = document.documentElement.clientHeight;
      if (!shown) {
        setHitRect({ x: 0, y: windowHeight - WAKE_STRIP, width: document.documentElement.clientWidth, height: WAKE_STRIP });
        return;
      }
      const top = inside || menu ? 0 : element.offsetTop;
      setHitRect({
        x: element.offsetLeft,
        y: top,
        width: element.offsetWidth,
        height: windowHeight - top,
      });
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, [shown, inside, menu]);

  const hiddenOffset = settings.dockHeight + BOTTOM_GAP + 4;

  return (
    <div className="dock-stage" style={{ paddingBottom: BOTTOM_GAP }}>
      <motion.div
        ref={bar}
        className="dock"
        style={{ height: settings.dockHeight }}
        animate={{ y: shown ? 0 : hiddenOffset }}
        transition={{ type: "spring", stiffness: 420, damping: 36 }}
        onMouseMove={(e) => mouseX.set(e.clientX)}
        onMouseLeave={() => mouseX.set(Infinity)}
        onContextMenu={(e) => (e.preventDefault(), setMenu({ kind: "dock", x: e.clientX }))}
      >
        <AnimatePresence initial={false}>
          {apps.map((app) => (
            <DockIcon
              key={app.exePath}
              app={app}
              size={settings.iconSize}
              mouseX={mouseX}
              menuOpen={menu !== null}
              onMenu={(icon) => setMenu({ kind: "app", app, x: icon.x + icon.width / 2, icon })}
            />
          ))}
        </AnimatePresence>
      </motion.div>
      <AnimatePresence>
        {menu && (
          <ContextMenu
            key={menu.kind === "app" ? menu.app.exePath : "dock"}
            menu={menu}
            bottom={settings.dockHeight + BOTTOM_GAP + 10}
            onDone={() => setMenu(null)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

type IconProps = {
  app: DockApp;
  size: number;
  mouseX: MotionValue<number>;
  menuOpen: boolean;
  onMenu: (icon: Rect) => void;
};

function DockIcon({ app, size, mouseX, menuOpen, onMenu }: IconProps) {
  const ref = useRef<HTMLButtonElement>(null);
  const icon = useIcon(app.exePath);
  const [hovered, setHovered] = useState(false);

  // Magnify by distance between the cursor and this icon's centre.
  const reach = size * REACH;
  const distance = useTransform(mouseX, (x) => {
    const r = ref.current?.getBoundingClientRect();
    return r ? x - (r.x + r.width / 2) : Infinity;
  });
  const target = useTransform(distance, [-reach, 0, reach], [size, size * MAGNIFY, size], { clamp: true });
  const current = useSpring(target, { stiffness: 520, damping: 28, mass: 0.2 });

  const front = app.windows[0];
  const iconRect = (): Rect => {
    const r = ref.current!.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  };
  const onClick = () => {
    if (app.isForeground) {
      minimize(front.hwnd, iconRect());
    } else {
      activate(front.hwnd);
    }
  };

  return (
    <motion.button
      ref={ref}
      className="dock-icon"
      style={{ width: current, height: current }}
      initial={{ opacity: 0, y: 24, scale: 0.6 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.4, transition: { duration: 0.15 } }}
      whileTap={{ scale: 0.88 }}
      onHoverStart={() => setHovered(true)}
      onHoverEnd={() => setHovered(false)}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onMenu(iconRect());
      }}
    >
      <AnimatePresence>
        {hovered && !menuOpen && (
          <motion.span
            className="dock-label"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4, transition: { duration: 0.1 } }}
          >
            {front.title || app.name}
          </motion.span>
        )}
      </AnimatePresence>
      {icon ? <img src={icon} alt={app.name} draggable={false} /> : <span className="dock-fallback">{app.name[0]}</span>}
      <span className={`dock-dot ${app.isForeground ? "active" : ""}`} />
    </motion.button>
  );
}

function ContextMenu({ menu, bottom, onDone }: { menu: Menu; bottom: number; onDone: () => void }) {
  const items =
    menu.kind === "app"
      ? [
          menu.app.isForeground
            ? { label: "Minimize", run: () => minimize(menu.app.windows[0].hwnd, menu.icon) }
            : { label: "Show", run: () => activate(menu.app.windows[0].hwnd) },
          { label: "Close window", run: () => closeWindow(menu.app.windows[0].hwnd) },
        ]
      : [
          { label: "Settings", run: openSettings },
          { label: "Quit notchlink", run: quit },
        ];

  return (
    <motion.div
      className="dock-menu"
      style={{ left: menu.x, bottom }}
      initial={{ opacity: 0, y: 6, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 6, scale: 0.96, transition: { duration: 0.1 } }}
      transition={{ type: "spring", stiffness: 600, damping: 34 }}
    >
      {menu.kind === "app" && <div className="dock-menu-title">{menu.app.name}</div>}
      {items.map((item) => (
        <button
          key={item.label}
          className="dock-menu-item"
          onClick={() => {
            onDone();
            item.run();
          }}
        >
          {item.label}
        </button>
      ))}
    </motion.div>
  );
}
