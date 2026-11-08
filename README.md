# notchlink

A notch island and dock for Windows, built with Tauri 2, Rust, React and Motion.

## What's in it

- **Island:** a black notch on the top edge of the main monitor, with placeholder eyes that blink.
  - Hover: peeks (eyes widen, notch grows a little), or opens fully, depending on settings.
  - Click: the notch springs open into a card grid (the clock is live; Settings opens the settings window).
  - Mouse leaves: it closes after a short delay.
- **Dock:** replaces the Windows taskbar (switchable in settings).
  - Shows your open apps (one icon per program), with a dot for the app in front.
  - Icons magnify near the cursor; the window title shows on hover.
  - Click: minimize the front app with a genie into its icon; click again to restore it with the genie in reverse.
    Other apps just come to the front.
  - Right-click an icon: Minimize/Show, Close window (with a TV switch-off animation).
  - Right-click empty dock space: Settings, Quit notchlink.
  - Auto-hide: slides away, comes back when the mouse touches the bottom edge. With auto-hide off,
    maximized windows stop above the dock.
- **Volume and brightness panels:** move the mouse to the middle of the left screen edge (volume) or
  right screen edge (brightness); the panel slides out. Scroll or drag to change the level. Brightness
  works on built-in laptop screens only.
- **Settings** (island Settings card, dock right-click, or Ctrl+Alt+N): notch hover behaviour,
  replace taskbar, auto-hide, dock height, icon size, and Quit. Saved to
  `%APPDATA%\com.fab19n.notchlink\settings.json`.
- Everything is click-through except the notch and the dock bar.

## Run it on Windows

One-time setup:

1. Install the Tauri prerequisites for Windows (Microsoft C++ Build Tools and WebView2, which Windows 10/11 usually already has): https://tauri.app/start/prerequisites/
2. Install Rust: https://rustup.rs
3. Install Node.js 22 or newer, then pnpm: `npm install -g pnpm`

Then, in this folder:

```bash
pnpm install
```

```bash
pnpm tauri dev
```

The first run compiles all the Rust dependencies and takes a few minutes. Later runs are fast.

To quit, use **Quit notchlink** (settings or dock right-click). Ctrl+C in the terminal also works.
Both bring the Windows taskbar back.

### If the taskbar doesn't come back

If notchlink crashes while the taskbar is hidden, start notchlink again: it restores the taskbar on
startup. If that doesn't work, open Task Manager (Ctrl+Shift+Esc), restart "Windows Explorer", and
turn off "Automatically hide the taskbar" in Windows Settings > Personalization > Taskbar.

## How it's built

| Part | File | Job |
| --- | --- | --- |
| Window setup | `src-tauri/src/overlay.rs` | Creates the island, dock, animation and settings windows |
| Settings | `src-tauri/src/settings.rs` | Loads, validates, saves and applies settings |
| Taskbar | `src-tauri/src/taskbar.rs` | Hides and restores the Windows taskbar |
| Dock placement | `src-tauri/src/dock.rs` | Sizes and places the dock; reserves screen space when auto-hide is off |
| Animations | `src-tauri/src/fx.rs`, `src/fx/` | Window snapshots and the genie / TV-off effects |
| Volume, brightness | `src-tauri/src/levels.rs`, `src/edge/` | Audio and WMI brightness control; the edge panels |
| Click-through | `src-tauri/src/hit.rs` | Polls the cursor; each window only catches the mouse inside the rectangle its page reports. Also keeps the taskbar hidden |
| Open windows | `src-tauri/src/win.rs` | Lists taskbar-style windows, extracts app icons, focuses or minimizes on click |
| Island UI | `src/island/` | The notch states and spring morphs |
| Dock UI | `src/dock/` | Grouping windows into apps, magnification, auto-hide, right-click menu |
| Settings UI | `src/settings/` | The settings window |

## Please test and report back

- [ ] Is the Windows taskbar gone, and do maximized windows use the full screen height?
- [ ] Does the taskbar ever flash back (Win key, mouse at the bottom edge, opening Start)?
- [ ] Auto-hide: does the dock come back at the bottom edge and hide after you leave?
- [ ] Auto-hide off: do maximized windows stop above the dock?
- [ ] Can you click things right above the dock and beside it now?
- [ ] Genie minimize and TV-off close: do they look right? Any flash of the real window?
- [ ] Do the settings apply instantly? Does Ctrl+Alt+N open them?
- [ ] Does Quit (and Ctrl+C) bring the taskbar back?

## Known limits

- Store (UWP) apps may show the wrong icon, because their window belongs to a host process.
- Apps running as administrator are skipped (Windows doesn't let a normal app read their details).
- The genie and TV-off animations only play for dock actions, not the apps' own buttons. Restoring
  plays the genie only for windows minimized from the dock (it reuses their snapshot).
- If an app asks "save changes?" when closed from the dock, the TV-off animation still plays.
- The system tray, notifications and quick settings are hidden with the taskbar until they move into the island.
- The dock has no pinned apps yet.
- The eyes are placeholders until the mascot design is ready.
