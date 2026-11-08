import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, MotionValue, useMotionValue, useSpring, useTransform } from "motion/react";
import { Rect, setHitBottomEdge, setHitRect, setHitRectAndBottomEdge, usePointerInside } from "../shared/overlay";
import { DOCK_PADDING, openSettings, quit, Settings, useSettings } from "../shared/settings";
import { closeWindow, DockApp, toggle, useDockApps, useIcon } from "./windows";

/** Icons grow to this multiple of their size under the cursor (keep in sync with src-tauri/src/dock.rs). */
const MAGNIFY = 1.55;
/** How far (in icon widths) from the cursor icons start to grow. */
const REACH = 3;
/** Auto-hide waits this long after the mouse leaves. */
const HIDE_DELAY_MS = 500;
/** With the dock hidden, this strip along the whole bottom edge of the screen brings it back. */
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
  const menuBox = useRef<HTMLDivElement>(null);
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
  // Hidden: a thin strip along the whole bottom edge of the screen. Shown: the bar (plus that strip when
  // auto-hiding, so resting the mouse in a corner keeps it up). In use: the bar, plus just enough above it
  // for magnified icons, plus the menu when open. Nothing else: everywhere else, including the space
  // just above the dock, clicks go through to the apps behind.
  useEffect(() => {
    const element = bar.current!;
    const report = () => {
      const windowHeight = document.documentElement.clientHeight;
      if (!shown) {
        setHitBottomEdge(WAKE_STRIP);
        return;
      }
      // How far magnified icons stick out above the bar.
      const overflow = inside ? Math.max(0, settings.iconSize * MAGNIFY + DOCK_PADDING - settings.dockHeight) + 4 : 0;
      let rect: Rect = {
        x: element.offsetLeft,
        y: element.offsetTop - overflow,
        width: element.offsetWidth,
        height: windowHeight - element.offsetTop + overflow,
      };
      const m = menuBox.current;
      if (m) {
        // offset* ignore the menu's CSS translate(-50%), so centre it by hand.
        rect = union(rect, { x: m.offsetLeft - m.offsetWidth / 2, y: m.offsetTop, width: m.offsetWidth, height: m.offsetHeight });
      }
      if (settings.dockAutohide) setHitRectAndBottomEdge(rect, WAKE_STRIP);
      else setHitRect(rect);
    };
    const observer = new ResizeObserver(report);
    observer.observe(element);
    report();
    return () => observer.disconnect();
  }, [shown, inside, menu, settings.dockAutohide, settings.iconSize, settings.dockHeight]);

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
            ref={menuBox}
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
  const onClick = () => toggle(app, iconRect());

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

type MenuProps = { menu: Menu; bottom: number; onDone: () => void; ref: React.Ref<HTMLDivElement> };

function ContextMenu({ menu, bottom, onDone, ref }: MenuProps) {
  const items =
    menu.kind === "app"
      ? [
          { label: menu.app.isForeground ? "Minimize" : "Show", run: () => toggle(menu.app, menu.icon) },
          { label: "Close window", run: () => closeWindow(menu.app.windows[0].hwnd) },
        ]
      : [
          { label: "Settings", run: openSettings },
          { label: "Quit notchlink", run: quit },
        ];

  return (
    <motion.div
      ref={ref}
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

function union(a: Rect, b: Rect): Rect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return {
    x,
    y,
    width: Math.max(a.x + a.width, b.x + b.width) - x,
    height: Math.max(a.y + a.height, b.y + b.height) - y,
  };
}
