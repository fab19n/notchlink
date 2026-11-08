//! Click-through for the overlay windows.
//!
//! Each overlay window is mostly transparent. The frontend reports the one
//! area that should receive the mouse: a rectangle (the island, the dock bar),
//! or the bottom edge of the screen (the hidden dock's wake-up strip).
//! A background thread polls the cursor: outside that rectangle the window
//! ignores the mouse, so clicks reach the apps underneath. The thread also
//! tells the frontend when the pointer enters or leaves, because the page
//! cannot see the mouse while it is being ignored.
//!
//! The same loop keeps the Windows taskbar hidden (see taskbar.rs).

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::overlay::{DOCK, ISLAND};
use crate::taskbar;

const POLL_INTERVAL: Duration = Duration::from_millis(30);

/// A rectangle in logical (CSS) pixels, relative to the window's top-left corner.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HitArea {
    /// CSS px relative to the window's top-left corner.
    Rect(Rect),
    /// The bottom `height` CSS px of the primary monitor, across its full width.
    BottomEdge { height: f64 },
    /// Either of the two: an auto-hidden dock stays up while the mouse is on it or on the screen edge.
    RectAndBottomEdge { rect: Rect, height: f64 },
}

#[derive(Default)]
pub struct HitAreas(Mutex<HashMap<String, HitArea>>);

#[tauri::command]
pub fn set_hit_area(window: WebviewWindow, area: HitArea, areas: State<HitAreas>) {
    areas.0.lock().unwrap().insert(window.label().to_string(), area);
}

pub fn spawn_hit_tester(app: AppHandle) {
    thread::spawn(move || {
        let mut inside: HashMap<&str, bool> = HashMap::new();
        loop {
            thread::sleep(POLL_INTERVAL);
            taskbar::keep_hidden();
            let cursor = app.cursor_position().expect("cursor position unavailable");
            for label in [ISLAND, DOCK] {
                let window = app.get_webview_window(label).expect("overlay window missing");
                let Some(area) = app.state::<HitAreas>().0.lock().unwrap().get(label).copied() else {
                    continue;
                };
                let in_rect = |rect: Rect| {
                    let origin = window.outer_position().expect("window position unavailable");
                    let scale = window.scale_factor().expect("scale factor unavailable");
                    rect.contains((cursor.x - origin.x as f64) / scale, (cursor.y - origin.y as f64) / scale)
                };
                let on_bottom_edge = |height: f64| {
                    let monitor = app.primary_monitor().unwrap().expect("no primary monitor found");
                    let (left, top) = (monitor.position().x as f64, monitor.position().y as f64);
                    let (width, bottom) = (monitor.size().width as f64, top + monitor.size().height as f64);
                    cursor.x >= left && cursor.x < left + width && cursor.y >= bottom - height * monitor.scale_factor()
                };
                let now_inside = match area {
                    HitArea::Rect(rect) => in_rect(rect),
                    HitArea::BottomEdge { height } => on_bottom_edge(height),
                    HitArea::RectAndBottomEdge { rect, height } => in_rect(rect) || on_bottom_edge(height),
                };
                if inside.get(label) != Some(&now_inside) {
                    eprintln!("[hit] {label}: pointer {} at ({:.0}, {:.0})", if now_inside { "in" } else { "out" }, cursor.x, cursor.y);
                    inside.insert(label, now_inside);
                    window
                        .set_ignore_cursor_events(!now_inside)
                        .expect("failed to toggle click-through");
                    app.emit_to(label, "pointer", now_inside).expect("failed to emit pointer event");
                }
            }
        }
    });
}
