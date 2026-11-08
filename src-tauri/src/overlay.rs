//! Creates the transparent, always-on-top overlay windows on the primary monitor,
//! and the settings window.

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const ISLAND: &str = "island";
pub const DOCK: &str = "dock";
/// Full-screen, always click-through layer for the minimize and close animations.
pub const FX: &str = "fx";
pub const SETTINGS: &str = "settings";
/// Volume panel on the left screen edge, brightness on the right.
pub const VOLUME: &str = "volume";
pub const BRIGHTNESS: &str = "brightness";

/// Logical size of each edge panel window (room for the panel and its curved corners).
const EDGE_SIZE: (f64, f64) = (80.0, 340.0);

/// Logical size of the island window: big enough for the expanded island.
const ISLAND_SIZE: (f64, f64) = (680.0, 320.0);

fn overlay(app: &AppHandle, label: &str, page: &str) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(label)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .visible(false)
        .build()?;
    // Click-through until the page reports where it is (see hit.rs). The fx layer stays click-through.
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}

fn primary_monitor(app: &AppHandle) -> tauri::Result<tauri::Monitor> {
    Ok(app.primary_monitor()?.expect("no primary monitor found"))
}

/// Island: centred on the top edge of the primary monitor.
pub fn create_island(app: &AppHandle) -> tauri::Result<()> {
    let window = overlay(app, ISLAND, "index.html")?;
    window.set_size(tauri::LogicalSize::new(ISLAND_SIZE.0, ISLAND_SIZE.1))?;
    let monitor = primary_monitor(app)?;
    let width = (ISLAND_SIZE.0 * monitor.scale_factor()) as i32;
    let x = monitor.position().x + (monitor.size().width as i32 - width) / 2;
    window.set_position(PhysicalPosition::new(x, monitor.position().y))?;
    window.show()
}

/// Dock: size and position come from the settings (see dock.rs).
pub fn create_dock(app: &AppHandle) -> tauri::Result<()> {
    overlay(app, DOCK, "dock.html")?;
    Ok(())
}

pub fn show_dock(app: &AppHandle) -> tauri::Result<()> {
    app.get_webview_window(DOCK).expect("dock window missing").show()
}

/// Volume (left) and brightness (right) panels, vertically centred on the screen edges.
pub fn create_edges(app: &AppHandle) -> tauri::Result<()> {
    let monitor = primary_monitor(app)?;
    let scale = monitor.scale_factor();
    let (width, height) = ((EDGE_SIZE.0 * scale) as i32, (EDGE_SIZE.1 * scale) as i32);
    let y = monitor.position().y + (monitor.size().height as i32 - height) / 2;
    for (label, x) in [
        (VOLUME, monitor.position().x),
        (BRIGHTNESS, monitor.position().x + monitor.size().width as i32 - width),
    ] {
        let window = overlay(app, label, "edge.html")?;
        window.set_size(tauri::LogicalSize::new(EDGE_SIZE.0, EDGE_SIZE.1))?;
        window.set_position(PhysicalPosition::new(x, y))?;
        window.show()?;
    }
    Ok(())
}

/// Covers the primary monitor exactly.
pub fn create_fx(app: &AppHandle) -> tauri::Result<()> {
    let window = overlay(app, FX, "fx.html")?;
    let monitor = primary_monitor(app)?;
    window.set_size(PhysicalSize::new(monitor.size().width, monitor.size().height))?;
    window.set_position(*monitor.position())?;
    window.show()?;
    eprintln!(
        "[fx] window at {:?}, size {:?}, visible {}",
        window.outer_position()?,
        window.outer_size()?,
        window.is_visible()?
    );
    Ok(())
}

/// Async on purpose: on Windows, creating a window from a sync command deadlocks the app.
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app).map_err(|e| e.to_string())
}

/// Must not run inside a sync command or an event handler (it deadlocks on Windows); see `open_settings`.
pub fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("notchlink settings")
        .inner_size(440.0, 680.0)
        .resizable(false)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}
