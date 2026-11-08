#[cfg(not(windows))]
compile_error!("notchlink only supports Windows.");

mod dock;
mod fx;
mod hit;
mod image;
mod overlay;
mod settings;
mod taskbar;
mod win;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::core::BOOL;

const SETTINGS_SHORTCUT: &str = "ctrl+alt+n";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([SETTINGS_SHORTCUT])
                .expect("invalid shortcut")
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        overlay::open_settings_window(app).expect("can't open settings");
                    }
                })
                .build(),
        )
        .manage(hit::HitRects::default())
        .manage(win::IconCache::default())
        .invoke_handler(tauri::generate_handler![
            hit::set_hit_rect,
            win::get_windows,
            win::get_icon,
            win::activate_window,
            fx::minimize_window,
            fx::close_window,
            settings::get_settings,
            settings::update_settings,
            overlay::open_settings,
            quit,
        ])
        .setup(|app| {
            let handle = app.handle();
            taskbar::init(handle.path().app_config_dir()?.join("taskbar-hidden"));
            // Ctrl+C in the terminal (or closing it) must also bring the taskbar back.
            unsafe { SetConsoleCtrlHandler(Some(on_console_exit), true)? };

            let settings = settings::load(handle)?;
            overlay::create_island(handle)?;
            overlay::create_dock(handle)?;
            overlay::create_fx(handle)?;
            settings::apply(handle, &settings);
            overlay::show_dock(handle)?;
            app.manage(settings::SettingsState(Mutex::new(settings)));

            hit::spawn_hit_tester(handle.clone());
            win::spawn_window_watcher(handle.clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building notchlink");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            cleanup();
        }
    });
}

/// Gives back everything we changed in Windows. Safe to call more than once.
fn cleanup() {
    dock::remove_app_bar();
    taskbar::restore();
}

#[tauri::command]
fn quit(app: AppHandle) {
    cleanup();
    app.exit(0);
}

unsafe extern "system" fn on_console_exit(_ctrl_type: u32) -> BOOL {
    cleanup();
    BOOL(0) // let Windows end the process as usual
}
