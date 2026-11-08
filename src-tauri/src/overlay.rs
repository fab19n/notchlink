//! Creates the transparent, always-on-top overlay windows on the primary monitor.

use tauri::{AppHandle, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const ISLAND: &str = "island";
pub const DOCK: &str = "dock";

/// Logical size of the island window: big enough for the expanded island.
const ISLAND_SIZE: (f64, f64) = (680.0, 320.0);
/// Logical size of the dock window: room for magnified icons above the bar.
const DOCK_SIZE: (f64, f64) = (1100.0, 140.0);

fn overlay(app: &AppHandle, label: &str, page: &str, size: (f64, f64)) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(label)
        .inner_size(size.0, size.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .visible(false)
        .build()?;
    // Click-through until the page reports where it is (see hit.rs).
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}

fn primary_monitor(app: &AppHandle) -> tauri::Result<tauri::Monitor> {
    Ok(app.primary_monitor()?.expect("no primary monitor found"))
}

/// Island: centred on the top edge of the primary monitor.
pub fn create_island(app: &AppHandle) -> tauri::Result<()> {
    let window = overlay(app, ISLAND, "index.html", ISLAND_SIZE)?;
    let monitor = primary_monitor(app)?;
    let width = (ISLAND_SIZE.0 * monitor.scale_factor()) as i32;
    let x = monitor.position().x + (monitor.size().width as i32 - width) / 2;
    window.set_position(PhysicalPosition::new(x, monitor.position().y))?;
    window.show()?;
    Ok(())
}

/// Dock: centred just above the taskbar (bottom of the monitor's work area).
pub fn create_dock(app: &AppHandle) -> tauri::Result<()> {
    let window = overlay(app, DOCK, "dock.html", DOCK_SIZE)?;
    let monitor = primary_monitor(app)?;
    let scale = monitor.scale_factor();
    let width = (DOCK_SIZE.0 * scale) as i32;
    let height = (DOCK_SIZE.1 * scale) as i32;
    let area = monitor.work_area();
    let x = area.position.x + (area.size.width as i32 - width) / 2;
    let y = area.position.y + area.size.height as i32 - height;
    window.set_position(PhysicalPosition::new(x, y))?;
    window.show()?;
    Ok(())
}
