//! Sizing and placing the dock window from the settings.
//!
//! With auto-hide off, the dock registers as an "app bar": Windows then keeps
//! maximized windows above it, like it does for the taskbar. With auto-hide on,
//! nothing is reserved; the dock slides away and comes back at the screen edge.

use std::sync::Mutex;

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::{ABE_BOTTOM, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA, SHAppBarMessage};

use crate::overlay::DOCK;
use crate::settings::{DOCK_PADDING, Settings};

/// Logical width of the dock window. The bar is centred inside it.
const DOCK_WIDTH: f64 = 1100.0;
/// Gap between the bar and the bottom of the screen.
const BOTTOM_GAP: u32 = 8;
/// Room above the bar for magnified icons, labels and the right-click menu.
const MIN_HEADROOM: u32 = 120;
/// Icons grow to this multiple of their size under the cursor (keep in sync with src/dock/Dock.tsx).
pub const MAGNIFY: f64 = 1.55;

/// The dock window handle while it is registered as an app bar.
static APP_BAR: Mutex<Option<isize>> = Mutex::new(None);

pub fn layout(app: &AppHandle, settings: &Settings) {
    let window = app.get_webview_window(DOCK).expect("dock window missing");
    let monitor = app.primary_monitor().unwrap().expect("no primary monitor found");
    let scale = monitor.scale_factor();

    let overshoot = (settings.icon_size as f64 * (MAGNIFY - 1.0)) as u32 + 40;
    let headroom = overshoot.max(MIN_HEADROOM);
    let height = settings.dock_height + BOTTOM_GAP + headroom;
    window.set_size(LogicalSize::new(DOCK_WIDTH, height as f64)).expect("can't resize dock");

    let hwnd = window.hwnd().expect("no dock window handle");
    let bottom = if settings.dock_autohide {
        remove_app_bar();
        // The real taskbar, when shown, keeps its own space; sit just above it.
        let area = monitor.work_area();
        if settings.replace_taskbar {
            monitor.position().y + monitor.size().height as i32
        } else {
            area.position.y + area.size.height as i32
        }
    } else {
        let reserved = ((settings.dock_height + BOTTOM_GAP + DOCK_PADDING) as f64 * scale) as i32;
        reserve(hwnd, &monitor, reserved)
    };

    let width = (DOCK_WIDTH * scale) as i32;
    let x = monitor.position().x + (monitor.size().width as i32 - width) / 2;
    let y = bottom - (height as f64 * scale) as i32;
    window.set_position(PhysicalPosition::new(x, y)).expect("can't move dock");
    eprintln!(
        "[dock] window {width}x{} px at ({x}, {y}); monitor {}x{} at ({}, {}), scale {scale}; autohide {}",
        (height as f64 * scale) as i32,
        monitor.size().width,
        monitor.size().height,
        monitor.position().x,
        monitor.position().y,
        settings.dock_autohide,
    );
}

/// Registers the dock as a bottom app bar of `height` physical px; returns the bottom edge Windows gave it.
fn reserve(hwnd: HWND, monitor: &tauri::Monitor, height: i32) -> i32 {
    let mut registered = APP_BAR.lock().unwrap();
    let mut data = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uEdge: ABE_BOTTOM,
        rc: RECT {
            left: monitor.position().x,
            right: monitor.position().x + monitor.size().width as i32,
            top: 0,
            bottom: monitor.position().y + monitor.size().height as i32,
        },
        ..Default::default()
    };
    unsafe {
        if registered.is_none() {
            assert!(SHAppBarMessage(ABM_NEW, &mut data) != 0, "Windows refused to register the dock as an app bar");
            *registered = Some(hwnd.0 as isize);
        }
        data.rc.top = data.rc.bottom - height;
        SHAppBarMessage(ABM_QUERYPOS, &mut data);
        // QUERYPOS may move the bottom up (e.g. above a visible taskbar); keep our height.
        data.rc.top = data.rc.bottom - height;
        SHAppBarMessage(ABM_SETPOS, &mut data);
    }
    data.rc.bottom
}

/// Gives the reserved screen space back. Safe to call at any time, including on shutdown after a crash.
pub fn remove_app_bar() {
    if let Some(hwnd) = APP_BAR.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let mut data = APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: HWND(hwnd as *mut _),
            ..Default::default()
        };
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut data) };
    }
}
