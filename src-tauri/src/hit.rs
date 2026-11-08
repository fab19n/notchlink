//! Click-through for the overlay windows.
//!
//! Each overlay window is mostly transparent. The frontend reports the one
//! rectangle that should receive the mouse (the island or the dock bar).
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

#[derive(Default)]
pub struct HitRects(Mutex<HashMap<String, Rect>>);

#[tauri::command]
pub fn set_hit_rect(window: WebviewWindow, rect: Rect, rects: State<HitRects>) {
    rects.0.lock().unwrap().insert(window.label().to_string(), rect);
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
                let Some(rect) = app.state::<HitRects>().0.lock().unwrap().get(label).copied() else {
                    continue;
                };
                let origin = window.outer_position().expect("window position unavailable");
                let scale = window.scale_factor().expect("scale factor unavailable");
                let x = (cursor.x - origin.x as f64) / scale;
                let y = (cursor.y - origin.y as f64) / scale;
                let now_inside = rect.contains(x, y);
                if inside.get(label) != Some(&now_inside) {
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
