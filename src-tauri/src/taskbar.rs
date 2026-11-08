//! Hiding and restoring the Windows taskbar.
//!
//! Hiding = switch the taskbar to auto-hide (so Windows gives its screen space
//! back to apps) and hide its windows. Explorer sometimes shows it again (for
//! example when the mouse touches the screen edge), so `keep_hidden` is called
//! regularly to hide it again.
//!
//! Restoring puts back the auto-hide state the user had before. That original
//! state is also saved to a marker file while the taskbar is hidden: if
//! notchlink crashes, the next start restores the taskbar from it.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, FindWindowW, IsWindowVisible, SW_HIDE, SW_SHOW, ShowWindow};
use windows::core::w;

/// The auto-hide state from before we hid the taskbar. `Some` exactly while it's hidden.
static ORIGINAL_STATE: Mutex<Option<u32>> = Mutex::new(None);
static MARKER: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Call once at startup, before anything else here. Restores the taskbar if a previous run crashed while it was hidden.
pub fn init(marker: PathBuf) {
    if let Ok(text) = fs::read_to_string(&marker) {
        let state: u32 = text.trim().parse().expect("corrupt taskbar marker file");
        *ORIGINAL_STATE.lock().unwrap() = Some(state);
        *MARKER.lock().unwrap() = Some(marker);
        restore();
    } else {
        *MARKER.lock().unwrap() = Some(marker);
    }
}

pub fn hide() {
    let mut original = ORIGINAL_STATE.lock().unwrap();
    if original.is_none() {
        let state = app_bar_state();
        let marker = MARKER.lock().unwrap().clone().expect("taskbar::init not called");
        fs::create_dir_all(marker.parent().unwrap()).expect("can't create config folder");
        fs::write(&marker, state.to_string()).expect("can't write taskbar marker");
        *original = Some(state);
    }
    set_app_bar_state(ABS_AUTOHIDE);
    for hwnd in taskbar_windows() {
        unsafe { _ = ShowWindow(hwnd, SW_HIDE) };
    }
}

/// Safe to call at any time; does nothing if the taskbar isn't hidden.
pub fn restore() {
    let Some(state) = ORIGINAL_STATE.lock().unwrap().take() else {
        return;
    };
    set_app_bar_state(state);
    for hwnd in taskbar_windows() {
        unsafe { _ = ShowWindow(hwnd, SW_SHOW) };
    }
    if let Some(marker) = MARKER.lock().unwrap().as_ref() {
        _ = fs::remove_file(marker);
    }
}

/// Hide the taskbar again if Explorer brought it back.
pub fn keep_hidden() {
    if ORIGINAL_STATE.lock().unwrap().is_none() {
        return;
    }
    for hwnd in taskbar_windows() {
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

/// The main taskbar plus one per extra monitor.
fn taskbar_windows() -> Vec<HWND> {
    let mut found = Vec::new();
    unsafe {
        if let Ok(main) = FindWindowW(w!("Shell_TrayWnd"), None) {
            found.push(main);
        }
        let mut after = None;
        while let Ok(hwnd) = FindWindowExW(None, after, w!("Shell_SecondaryTrayWnd"), None) {
            found.push(hwnd);
            after = Some(hwnd);
        }
    }
    found
}

fn main_taskbar() -> HWND {
    unsafe { FindWindowW(w!("Shell_TrayWnd"), None).expect("Windows taskbar not found") }
}

fn app_bar_state() -> u32 {
    let mut data = APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: main_taskbar(), ..Default::default() };
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 }
}

fn set_app_bar_state(state: u32) {
    let mut data = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: main_taskbar(),
        lParam: LPARAM(state as isize),
        ..Default::default()
    };
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
}
