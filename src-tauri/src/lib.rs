#[cfg(not(windows))]
compile_error!("notchlink only supports Windows.");

mod hit;
mod overlay;
mod win;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(hit::HitRects::default())
        .manage(win::IconCache::default())
        .invoke_handler(tauri::generate_handler![
            hit::set_hit_rect,
            win::get_windows,
            win::get_icon,
            win::activate_window,
        ])
        .setup(|app| {
            overlay::create_island(app.handle())?;
            overlay::create_dock(app.handle())?;
            hit::spawn_hit_tester(app.handle().clone());
            win::spawn_window_watcher(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running notchlink");
}
