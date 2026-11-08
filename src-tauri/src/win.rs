//! Open windows for the dock: list them, get app icons, and focus or minimize on click.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetForegroundWindow, GetIconInfo, GetWindow,
    GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId, HICON, ICONINFO, IsIconic,
    IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, HSTRING, PWSTR};

use crate::image;
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
    is_minimized: bool,
}

/// For the dock page. Async so it runs off the main thread.
#[tauri::command]
pub async fn get_windows() -> Vec<AppWindow> {
    list_windows()
}

/// Top-level windows a user would expect in a taskbar, front-most first.
fn list_windows() -> Vec<AppWindow> {
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
                is_minimized: unsafe { IsIconic(hwnd).as_bool() },
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
        // Failing to read it means the window just closed: skip it.
        let mut cloaked: u32 = 0;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, size_of::<u32>() as u32).is_ok()
            && cloaked == 0
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

/// Click on a dock icon for an app that isn't in front: restore and focus its window.
/// (Minimizing the front app goes through fx.rs, for the animation.)
#[tauri::command]
pub fn activate_window(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

#[derive(Default)]
pub struct IconCache(Mutex<HashMap<String, String>>);

/// The program's icon as a PNG data URL, cached per exe path.
/// Async so icon extraction runs off the main thread.
#[tauri::command]
pub async fn get_icon(exe_path: String, cache: State<'_, IconCache>) -> Result<String, String> {
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
        let result = SHDefExtractIconW(&HSTRING::from(exe_path), 0, 0, Some(&mut icon), None, ICON_SIZE);
        // S_FALSE ("no icon") counts as success, so also check that we really got one.
        result.ok().map_err(|e| format!("no icon in {exe_path}: {e}"))?;
        if icon.is_invalid() {
            return Err(format!("no icon in {exe_path}"));
        }
        let rgba = icon_to_rgba(icon);
        DestroyIcon(icon).map_err(|e| format!("DestroyIcon failed for {exe_path}: {e}"))?;
        Ok(image::png_data_url(&rgba?, ICON_SIZE, ICON_SIZE))
    }
}

/// Reads the icon's colour bitmap as ICON_SIZE x ICON_SIZE RGBA pixels.
unsafe fn icon_to_rgba(icon: HICON) -> Result<Vec<u8>, String> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).map_err(|e| format!("GetIconInfo failed: {e}"))?;
        let pixels = image::bitmap_rgba(info.hbmColor, ICON_SIZE, ICON_SIZE);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        pixels
    }
}

/// Sends the window list to the dock whenever it changes.
pub fn spawn_window_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut last: Vec<AppWindow> = Vec::new();
        loop {
            thread::sleep(WATCH_INTERVAL);
            let current = list_windows();
            if current != last {
                app.emit_to(DOCK, "windows-changed", &current).expect("failed to emit windows");
                last = current;
            }
        }
    });
}
