//! Minimize (genie) and close (TV switch-off) animations for dock actions.
//!
//! Windows doesn't let us animate another app's window, so we take a snapshot
//! of it, show the snapshot in the full-screen fx layer, make the real window
//! disappear instantly underneath, and animate the snapshot.

use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    ANIMATIONINFO, GetWindowRect, IsIconic, PW_RENDERFULLCONTENT, PostMessageW, SPI_GETANIMATION,
    SPI_SETANIMATION, SW_MINIMIZE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ShowWindow, SystemParametersInfoW, WM_CLOSE,
};

use crate::hit::Rect;
use crate::image;
use crate::overlay::{DOCK, FX};

/// Time for the fx layer to paint the snapshot before the real window vanishes.
const PAINT_DELAY: Duration = Duration::from_millis(70);
/// Snapshots wider than this are halved, to keep encoding quick.
const MAX_SNAPSHOT_WIDTH: u32 = 1600;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FxEvent {
    kind: &'static str,
    image: String,
    /// Where the window is, in fx-layer CSS px.
    from: Rect,
    /// Where it goes (the dock icon), for the genie.
    to: Option<Rect>,
}

/// Minimize with the genie effect into the dock icon at `icon` (CSS px in the dock window).
#[tauri::command]
pub fn minimize_window(app: AppHandle, hwnd: isize, icon: Rect) {
    thread::spawn(move || {
        let hwnd = HWND(hwnd as *mut _);
        if let Some((image, bounds)) = snapshot(hwnd) {
            let to = dock_to_fx(&app, icon);
            let from = screen_to_fx(&app, bounds);
            app.emit_to(FX, "fx", FxEvent { kind: "genie", image, from, to: Some(to) }).expect("emit fx");
            thread::sleep(PAINT_DELAY);
        }
        minimize_without_system_animation(hwnd);
    });
}

/// Close with the TV switch-off effect. The app may still ask to save first; then the window stays.
#[tauri::command]
pub fn close_window(app: AppHandle, hwnd: isize) {
    thread::spawn(move || {
        let hwnd = HWND(hwnd as *mut _);
        if let Some((image, bounds)) = snapshot(hwnd) {
            let from = screen_to_fx(&app, bounds);
            app.emit_to(FX, "fx", FxEvent { kind: "tv-off", image, from, to: None }).expect("emit fx");
            thread::sleep(PAINT_DELAY);
        }
        unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).expect("can't close window") };
    });
}

/// Turns Windows' own minimize animation off just for this minimize, so only ours plays.
fn minimize_without_system_animation(hwnd: HWND) {
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
        let _ = ShowWindow(hwnd, SW_MINIMIZE);
        if animated != 0 {
            info.iMinAnimate = animated;
            SystemParametersInfoW(SPI_SETANIMATION, size, Some(&mut info as *mut _ as *mut _), no_save)
                .expect("can't restore animation setting");
        }
    }
}

/// The window as a PNG data URL, plus its visible bounds on screen (physical px).
/// None for minimized windows or when the app refuses to draw itself.
fn snapshot(hwnd: HWND) -> Option<(String, RECT)> {
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
        Some((image::png_data_url(&rgba, out_w, out_h), visible))
    }
}

/// Cuts `crop` out of a `width`-wide RGBA image; halves it (nearest pixel) when it's very wide.
fn crop_and_shrink(pixels: &[u8], width: u32, crop: RECT) -> (Vec<u8>, u32, u32) {
    let (cw, ch) = ((crop.right - crop.left) as u32, (crop.bottom - crop.top) as u32);
    let step = if cw > MAX_SNAPSHOT_WIDTH { 2 } else { 1 };
    let (ow, oh) = (cw / step, ch / step);
    let mut out = Vec::with_capacity((ow * oh * 4) as usize);
    for y in 0..oh {
        let src_y = crop.top as u32 + y * step;
        for x in 0..ow {
            let i = ((src_y * width + crop.left as u32 + x * step) * 4) as usize;
            out.extend_from_slice(&pixels[i..i + 4]);
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
