import { useEffect, useState } from "react";
import {
  DOCK_HEIGHT_RANGE,
  DOCK_PADDING,
  ICON_SIZE_MIN,
  quit,
  Settings,
  updateSettings,
  useSettings,
} from "../shared/settings";

/** Slider changes are saved once they stop moving for this long. */
const SAVE_DELAY_MS = 250;

export default function SettingsApp() {
  const saved = useSettings();
  if (!saved) return null;
  return <Form saved={saved} />;
}

function Form({ saved }: { saved: Settings }) {
  const [draft, setDraft] = useState(saved);
  const [error, setError] = useState<string | null>(null);

  // Every change applies live; sliders are debounced so the dock isn't re-laid-out on every pixel.
  useEffect(() => {
    const timer = setTimeout(() => {
      updateSettings(draft).then(
        () => setError(null),
        (e) => setError(String(e)),
      );
    }, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [draft]);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => setDraft((d) => ({ ...d, [key]: value }));
  const maxIcon = draft.dockHeight - 2 * DOCK_PADDING;

  return (
    <main>
      <h1>notchlink</h1>

      <section>
        <h2>Notch</h2>
        <Row label="When the mouse hovers over the notch">
          <Segmented
            value={draft.islandOpen}
            options={[
              { value: "peek", label: "Peek, click to open" },
              { value: "hover", label: "Open fully" },
            ]}
            onChange={(v) => set("islandOpen", v)}
          />
        </Row>
      </section>

      <section>
        <h2>Dock</h2>
        <Row label="Replace the Windows taskbar" hint="Off brings the normal taskbar back.">
          <Toggle checked={draft.replaceTaskbar} onChange={(v) => set("replaceTaskbar", v)} />
        </Row>
        <Row label="Auto-hide" hint="Slides away; move the mouse to the bottom edge to show it.">
          <Toggle checked={draft.dockAutohide} onChange={(v) => set("dockAutohide", v)} />
        </Row>
        <Row label={`Height: ${draft.dockHeight}px`}>
          <input
            type="range"
            min={DOCK_HEIGHT_RANGE[0]}
            max={DOCK_HEIGHT_RANGE[1]}
            value={draft.dockHeight}
            onChange={(e) => {
              const height = Number(e.target.value);
              // Shrinking the bar shrinks the icons with it.
              setDraft((d) => ({ ...d, dockHeight: height, iconSize: Math.min(d.iconSize, height - 2 * DOCK_PADDING) }));
            }}
          />
        </Row>
        <Row label={`Icon size: ${draft.iconSize}px`} hint={`Up to ${maxIcon}px at this height.`}>
          <input
            type="range"
            min={ICON_SIZE_MIN}
            max={maxIcon}
            value={draft.iconSize}
            onChange={(e) => set("iconSize", Number(e.target.value))}
          />
        </Row>
      </section>

      <section>
        <h2>Shortcut</h2>
        <p className="hint">Ctrl + Alt + N opens this window from anywhere.</p>
      </section>

      {error && <p className="error">{error}</p>}

      <footer>
        <button className="quit" onClick={quit}>
          Quit notchlink
        </button>
        <span className="hint">Quitting brings the Windows taskbar back.</span>
      </footer>
    </main>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="row">
      <div>
        <div className="label">{label}</div>
        {hint && <div className="hint">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button role="switch" aria-checked={checked} className={`toggle ${checked ? "on" : ""}`} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

type Option<T> = { value: T; label: string };

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: Option<T>[]; onChange: (v: T) => void }) {
  return (
    <div className="segmented">
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? "selected" : ""} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}
