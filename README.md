# notchlink

A notch island and dock for Windows, built with Tauri 2, Rust, React and Motion.

## Phase 1: what's in it

- **Island:** a black notch on the top edge of the main monitor, with placeholder eyes that blink.
  - Hover: the eyes widen and the notch grows a little (peek).
  - Click: the notch springs open into a card grid (only the clock is live so far).
  - Mouse leaves: it closes after a short delay.
- **Dock:** sits just above the normal Windows taskbar.
  - Shows your real open apps (one icon per program), with a dot for the app in front.
  - Icons magnify near the cursor; the window title shows on hover.
  - Click: focus that app's front window, or minimize it if it's already in front.
- Both windows are click-through everywhere except the notch and the dock bar.

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
To quit, close the terminal or press Ctrl+C in it (the overlays have no close button yet).

## How it's built

| Part | File | Job |
| --- | --- | --- |
| Window setup | `src-tauri/src/overlay.rs` | Creates the transparent, always-on-top, non-focusable island and dock windows on the primary monitor |
| Click-through | `src-tauri/src/hit.rs` | Polls the cursor; each window only catches the mouse inside the rectangle its page reports |
| Open windows | `src-tauri/src/win.rs` | Lists taskbar-style windows, extracts app icons, focuses or minimizes on click |
| Island UI | `src/island/` | The notch states and spring morphs |
| Dock UI | `src/dock/` | Grouping windows into apps, magnification |

## Please test and report back

- [ ] Does the notch sit centred and flush with the top of the screen?
- [ ] Do apps behind the transparent parts still get clicks (top of the screen, beside the dock)?
- [ ] Peek on hover, open on click, close on leave: does the spring feel right? Too bouncy, too slow?
- [ ] Does the island ever steal focus from the app you're typing in?
- [ ] Dock: are the right apps shown? Any missing, or any that shouldn't be there?
- [ ] Dock: does clicking an icon bring that app to the front? Does clicking the front app minimize it?
- [ ] Do the app icons look sharp?
- [ ] What is your display scaling (100%, 125%, 150%)?

## Known limits in Phase 1

- Store (UWP) apps may show the wrong icon, because their window belongs to a host process.
- Apps running as administrator are skipped (Windows doesn't let a normal app read their details).
- The dock doesn't hide the real taskbar yet, and has no pinned apps.
- The eyes are placeholders until the mascot design is ready.
