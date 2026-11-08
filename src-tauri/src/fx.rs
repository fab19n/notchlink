//! Genie (minimize / restore) and TV switch-off (close) animations for dock actions.
//!
//! Windows doesn't let us animate another app's window, so we snapshot it and
//! animate the snapshot in the full-screen fx layer:
//!
//! 1. Rust takes the snapshot and sends the fx layer a small `fx` event.
//! 2. The fx layer fetches the raw pixels (`get_fx_image`), paints the first
//!    frame, and calls `fx_ready`: minimize/close happen now, hidden under the snapshot.
//! 3. When the animation ends it calls `fx_done`: a restore happens now, so the
//!    real window appears exactly where the animation lands.
//!
//! Minimize keeps its snapshot, so restoring that window can play the genie in reverse.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    ANIMATIONINFO, GetWindowRect, IsIconic, IsWindow, PW_RENDERFULLCONTENT, PostMessageW, SHOW_WINDOW_CMD,
    SPI_GETANIMATION, SPI_SETANIMATION, SW_MINIMIZE, SW_RESTORE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SetForegroundWindow, ShowWindow, SystemParametersInfoW, WM_CLOSE,
};

use crate::hit::Rect;
use crate::image;
use crate::overlay::{DOCK, FX};
use crate::win;

/// Snapshots wider than this are halved, to keep them quick to move around.
const MAX_SNAPSHOT_WIDTH: u32 = 2560;

#[derive(Clone)]
struct Snapshot {
    rgba: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    /// Visible window bounds on screen, physical px.
    bounds: RECT,
}

#[derive(Clone, Copy)]
enum Action {
    Minimize,
    Close,
    Restore,
}

struct Pending {
    hwnd: isize,
    action: Action,
    rgba: Arc<Vec<u8>>,
}

#[derive(Default)]
pub struct FxState {
    next_id: AtomicU32,
    pending: Mutex<HashMap<u32, Pending>>,
    /// Snapshot of each window we minimized, for the restore animation.
    minimized: Mutex<HashMap<isize, Snapshot>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FxEvent {
    id: u32,
    /// "genie-out" (minimize), "genie-in" (restore) or "tv-off" (close).
    kind: &'static str,
    width: u32,
    height: u32,
    /// The window, in fx-layer CSS px.
    window: Rect,
    /// The dock icon, for the genie.
    icon: Option<Rect>,
}

/// Minimize with the genie into the dock icon at `icon` (CSS px in the dock window).
#[tauri::command]
pub fn minimize_window(app: AppHandle, hwnd: isize, icon: Rect) {
    thread::spawn(move || {
        let Some(shot) = snapshot(HWND(hwnd as *mut _)) else {
            without_system_animation(|| show(hwnd, SW_MINIMIZE));
            return;
        };
        let state = app.state::<FxState>();
        let mut minimized = state.minimized.lock().unwrap();
        minimized.retain(|&h, _| unsafe { IsWindow(Some(HWND(h as *mut _))).as_bool() });
        minimized.insert(hwnd, shot.clone());
        drop(minimized);
        start(&app, hwnd, Action::Minimize, "genie-out", &shot, Some(dock_to_fx(&app, icon)));
    });
}

/// Restore a minimized window: the genie in reverse if we have its snapshot, else just show it.
#[tauri::command]
pub fn restore_window(app: AppHandle, hwnd: isize, icon: Rect, state: State<FxState>) {
    let shot = state.minimized.lock().unwrap().remove(&hwnd);
    match shot {
        Some(shot) => start(&app, hwnd, Action::Restore, "genie-in", &shot, Some(dock_to_fx(&app, icon))),
        None => win::activate_window(hwnd),
    }
}

/// Close with the TV switch-off. The app may still ask to save first; then the window stays.
#[tauri::command]
pub fn close_window(app: AppHandle, hwnd: isize) {
    thread::spawn(move || match snapshot(HWND(hwnd as *mut _)) {
        Some(shot) => start(&app, hwnd, Action::Close, "tv-off", &shot, None),
        None => close(hwnd),
    });
}

fn start(app: &AppHandle, hwnd: isize, action: Action, kind: &'static str, shot: &Snapshot, icon: Option<Rect>) {
    let state = app.state::<FxState>();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    state.pending.lock().unwrap().insert(id, Pending { hwnd, action, rgba: shot.rgba.clone() });
    let event = FxEvent {
        id,
        kind,
        width: shot.width,
        height: shot.height,
        window: screen_to_fx(app, shot.bounds),
        icon,
    };
    app.emit_to(FX, "fx", event).expect("emit fx");
}

/// The snapshot's raw RGBA pixels (sent as bytes, not JSON, so large windows stay fast).
#[tauri::command]
pub fn get_fx_image(id: u32, state: State<FxState>) -> Result<Response, String> {
    let pending = state.pending.lock().unwrap();
    let pending = pending.get(&id).ok_or("unknown fx id")?;
    Ok(Response::new(pending.rgba.as_ref().clone()))
}

/// The fx layer has painted the first frame over the window.
#[tauri::command]
pub fn fx_ready(id: u32, state: State<FxState>) -> Result<(), String> {
    let pending = state.pending.lock().unwrap();
    let pending = pending.get(&id).ok_or("unknown fx id")?;
    match pending.action {
        Action::Minimize => without_system_animation(|| show(pending.hwnd, SW_MINIMIZE)),
        Action::Close => close(pending.hwnd),
        Action::Restore => {}
    }
    Ok(())
}

/// The animation has finished.
#[tauri::command]
pub fn fx_done(id: u32, state: State<FxState>) -> Result<(), String> {
    let pending = state.pending.lock().unwrap().remove(&id).ok_or("unknown fx id")?;
    if let Action::Restore = pending.action {
        without_system_animation(|| show(pending.hwnd, SW_RESTORE));
        unsafe { _ = SetForegroundWindow(HWND(pending.hwnd as *mut _)) };
    }
    Ok(())
}

fn show(hwnd: isize, command: SHOW_WINDOW_CMD) {
    unsafe { _ = ShowWindow(HWND(hwnd as *mut _), command) };
}

fn close(hwnd: isize) {
    unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)).expect("can't close window") };
}

/// Runs `f` with Windows' own minimize/restore animation switched off, so only ours plays.
fn without_system_animation(f: impl FnOnce()) {
    let mut info = ANIMATIONINFO { cbSize: size_of::<ANIMATIONINFO>() as u32, iMinAnimate: 0 };
    let size = info.cbSize;
    let no_save = SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0);
    unsafe {
        SystemParametersInfoW(SPI_GETANIMATION, size, Some(&mut info as *mut _ as *mut _), no_save)
            .expect("can't read animation setting");
        let animated = info.iMinAnimate;
        if animated != 0 {
            info.iMinAnimate = 0;
            SystemParametersInfoW(SPI_SETANIMATION, size, Some(&mut info as *mut _ as *mut _), no_save)
                .expect("can't change animation setting");
        }
        f();
        if animated != 0 {
            info.iMinAnimate = animated;
            SystemParametersInfoW(SPI_SETANIMATION, size, Some(&mut info as *mut _ as *mut _), no_save)
                .expect("can't restore animation setting");
        }
    }
}

/// The window's pixels and visible bounds. None for minimized windows or when the app refuses to draw itself.
fn snapshot(hwnd: HWND) -> Option<Snapshot> {
    let started = Instant::now();
    unsafe {
        if IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut outer = RECT::default();
        GetWindowRect(hwnd, &mut outer).ok()?;
        // The visible frame, without the invisible resize borders Windows 10/11 add.
        let mut visible = RECT::default();
        DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut visible as *mut _ as *mut _, size_of::<RECT>() as u32)
            .ok()?;
        let (width, height) = ((outer.right - outer.left) as u32, (outer.bottom - outer.top) as u32);
        if width == 0 || height == 0 {
            return None;
        }

        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
        let previous = SelectObject(dc, bitmap.into());
        let drawn = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        SelectObject(dc, previous);
        let captured = started.elapsed();
        let pixels = image::bitmap_rgba(bitmap, width, height);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        if !drawn {
            return None;
        }
        let pixels = pixels.ok()?;

        let crop = RECT {
            left: (visible.left - outer.left).max(0),
            top: (visible.top - outer.top).max(0),
            right: (visible.right - outer.left).min(width as i32),
            bottom: (visible.bottom - outer.top).min(height as i32),
        };
        let (rgba, out_w, out_h) = crop_and_shrink(&pixels, width, crop);
        eprintln!(
            "[fx] snapshot {out_w}x{out_h}: capture {} ms, total {} ms",
            captured.as_millis(),
            started.elapsed().as_millis()
        );
        Some(Snapshot { rgba: Arc::new(rgba), width: out_w, height: out_h, bounds: visible })
    }
}

/// Cuts `crop` out of a `width`-wide RGBA image; halves it (nearest pixel) when it's very wide.
fn crop_and_shrink(pixels: &[u8], width: u32, crop: RECT) -> (Vec<u8>, u32, u32) {
    let (cw, ch) = ((crop.right - crop.left) as u32, (crop.bottom - crop.top) as u32);
    let step = if cw > MAX_SNAPSHOT_WIDTH { 2 } else { 1 };
    let (ow, oh) = (cw / step, ch / step);
    let mut out = Vec::with_capacity((ow * oh * 4) as usize);
    for y in 0..oh {
        let row = ((crop.top as u32 + y * step) * width + crop.left as u32) as usize * 4;
        if step == 1 {
            out.extend_from_slice(&pixels[row..row + ow as usize * 4]);
        } else {
            for x in 0..ow as usize {
                let i = row + x * 2 * 4;
                out.extend_from_slice(&pixels[i..i + 4]);
            }
        }
    }
    (out, ow, oh)
}

fn fx_origin(app: &AppHandle) -> (f64, f64, f64) {
    let monitor = app.primary_monitor().unwrap().expect("no primary monitor found");
    (monitor.position().x as f64, monitor.position().y as f64, monitor.scale_factor())
}

fn screen_to_fx(app: &AppHandle, r: RECT) -> Rect {
    let (ox, oy, scale) = fx_origin(app);
    Rect {
        x: (r.left as f64 - ox) / scale,
        y: (r.top as f64 - oy) / scale,
        width: (r.right - r.left) as f64 / scale,
        height: (r.bottom - r.top) as f64 / scale,
    }
}

fn dock_to_fx(app: &AppHandle, icon: Rect) -> Rect {
    let dock = app.get_webview_window(DOCK).expect("dock window missing");
    let origin = dock.outer_position().expect("dock position");
    let (ox, oy, scale) = fx_origin(app);
    Rect {
        x: (origin.x as f64 - ox) / scale + icon.x,
        y: (origin.y as f64 - oy) / scale + icon.y,
        width: icon.width,
        height: icon.height,
    }
}
