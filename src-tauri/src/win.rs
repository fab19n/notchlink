//! Open windows for the dock: list them, get app icons, and focus or minimize on click.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, ReleaseDC,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetForegroundWindow, GetIconInfo, GetWindow,
    GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId, HICON, ICONINFO, IsIconic,
    IsWindowVisible, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, HSTRING, PWSTR};

use crate::overlay::DOCK;

const WATCH_INTERVAL: Duration = Duration::from_millis(800);
const ICON_SIZE: u32 = 64;

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppWindow {
    /// The window handle, passed back to `activate_window`.
    hwnd: isize,
    title: String,
    /// Full path of the program that owns the window; the dock groups windows by it.
    exe_path: String,
    is_foreground: bool,
}

/// Top-level windows a user would expect in a taskbar, front-most first.
#[tauri::command]
pub fn get_windows() -> Vec<AppWindow> {
    let mut handles: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize)).expect("EnumWindows failed");
    }
    let own_pid = unsafe { GetCurrentProcessId() };
    let foreground = unsafe { GetForegroundWindow() };
    handles
        .into_iter()
        .filter(|&hwnd| is_taskbar_window(hwnd) && process_id(hwnd) != own_pid)
        .filter_map(|hwnd| {
            Some(AppWindow {
                hwnd: hwnd.0 as isize,
                title: title(hwnd),
                exe_path: exe_path(hwnd)?,
                is_foreground: hwnd == foreground,
            })
        })
        .collect()
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let handles = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    handles.push(hwnd);
    BOOL(1)
}

fn is_taskbar_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || title(hwnd).is_empty() {
            return false;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return false;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        // Windows on other virtual desktops and suspended UWP windows are "cloaked".
        let mut cloaked: u32 = 0;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, size_of::<u32>() as u32)
            .expect("DwmGetWindowAttribute failed");
        cloaked == 0
    }
}

fn title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..len as usize])
}

fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

/// None when the process can't be opened (for example an elevated app); such windows are skipped.
fn exe_path(hwnd: HWND) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id(hwnd)).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len);
        CloseHandle(process).expect("CloseHandle failed");
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// Click on a dock icon: restore and focus the window, or minimize it if it is already in front.
#[tauri::command]
pub fn activate_window(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if GetForegroundWindow() == hwnd {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
            return;
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

#[derive(Default)]
pub struct IconCache(Mutex<HashMap<String, String>>);

/// The program's icon as a PNG data URL, cached per exe path.
#[tauri::command]
pub fn get_icon(exe_path: String, cache: State<IconCache>) -> Result<String, String> {
    if let Some(url) = cache.0.lock().unwrap().get(&exe_path) {
        return Ok(url.clone());
    }
    let url = extract_icon(&exe_path)?;
    cache.0.lock().unwrap().insert(exe_path, url.clone());
    Ok(url)
}

fn extract_icon(exe_path: &str) -> Result<String, String> {
    unsafe {
        let mut icon = HICON::default();
        SHDefExtractIconW(&HSTRING::from(exe_path), 0, 0, Some(&mut icon), None, ICON_SIZE)
            .ok()
            .map_err(|e| format!("no icon in {exe_path}: {e}"))?;
        let rgba = icon_to_rgba(icon);
        DestroyIcon(icon).expect("DestroyIcon failed");
        let rgba = rgba?;
        Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(encode_png(&rgba))))
    }
}

/// Reads the icon's colour bitmap as ICON_SIZE x ICON_SIZE RGBA pixels.
unsafe fn icon_to_rgba(icon: HICON) -> Result<Vec<u8>, String> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).map_err(|e| format!("GetIconInfo failed: {e}"))?;
        let mut header = BITMAPINFO::default();
        header.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ICON_SIZE as i32,
            biHeight: -(ICON_SIZE as i32), // negative = top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut pixels = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
        let dc = GetDC(None);
        let rows = GetDIBits(
            dc,
            info.hbmColor,
            0,
            ICON_SIZE,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        if rows == 0 {
            return Err("GetDIBits failed".into());
        }
        // BGRA -> RGBA. Old icons without an alpha channel are fully opaque.
        let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
        for p in pixels.chunks_exact_mut(4) {
            p.swap(0, 2);
            if !has_alpha {
                p[3] = 255;
            }
        }
        Ok(pixels)
    }
}

fn encode_png(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, ICON_SIZE, ICON_SIZE);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png finish");
    out
}

/// Sends the window list to the dock whenever it changes.
pub fn spawn_window_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut last: Vec<AppWindow> = Vec::new();
        loop {
            thread::sleep(WATCH_INTERVAL);
            let current = get_windows();
            if current != last {
                app.emit_to(DOCK, "windows-changed", &current).expect("failed to emit windows");
                last = current;
            }
        }
    });
}
