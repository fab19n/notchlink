//! User settings: stored as JSON in the app's config folder, applied live, and
//! broadcast to every window as a `settings-changed` event.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{dock, taskbar};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IslandOpen {
    /// Hover peeks, click opens.
    Peek,
    /// Hover opens fully.
    Hover,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub island_open: IslandOpen,
    pub replace_taskbar: bool,
    pub dock_autohide: bool,
    /// Height of the dock bar, in CSS px.
    pub dock_height: u32,
    /// Resting icon size, in CSS px.
    pub icon_size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            island_open: IslandOpen::Peek,
            replace_taskbar: true,
            dock_autohide: true,
            dock_height: 64,
            icon_size: 46,
        }
    }
}

pub const DOCK_HEIGHT_RANGE: (u32, u32) = (44, 110);
pub const ICON_SIZE_MIN: u32 = 24;
/// Space between the icons and the top and bottom of the bar.
pub const DOCK_PADDING: u32 = 8;

impl Settings {
    fn validate(&self) -> Result<(), String> {
        let (min_h, max_h) = DOCK_HEIGHT_RANGE;
        if !(min_h..=max_h).contains(&self.dock_height) {
            return Err(format!("dock height must be {min_h} to {max_h}"));
        }
        let max_icon = self.dock_height - 2 * DOCK_PADDING;
        if !(ICON_SIZE_MIN..=max_icon).contains(&self.icon_size) {
            return Err(format!("icon size must be {ICON_SIZE_MIN} to {max_icon} for this dock height"));
        }
        Ok(())
    }
}

pub struct SettingsState(pub Mutex<Settings>);

fn path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().expect("no app config folder").join("settings.json")
}

/// Missing file = defaults. A file that exists but can't be read is an error, so a typo never silently resets everything.
pub fn load(app: &AppHandle) -> Result<Settings, String> {
    let path = path(app);
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let settings: Settings =
        serde_json::from_str(&text).map_err(|e| format!("invalid settings in {}: {e}", path.display()))?;
    settings.validate().map_err(|e| format!("invalid settings in {}: {e}", path.display()))?;
    Ok(settings)
}

fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&path, serde_json::to_string_pretty(settings).unwrap()).map_err(|e| e.to_string())
}

/// Windows updates the screen's work area a moment after the taskbar changes.
const TASKBAR_SETTLE: Duration = Duration::from_millis(300);

/// Puts the taskbar and dock in the state the settings describe.
pub fn apply(app: &AppHandle, settings: &Settings) {
    if settings.replace_taskbar {
        taskbar::hide();
    } else {
        taskbar::restore();
    }
    dock::layout(app, settings);
}

#[tauri::command]
pub fn get_settings(state: State<SettingsState>) -> Settings {
    state.0.lock().unwrap().clone()
}

/// Async so the wait below doesn't block the app's main thread.
#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings, state: State<'_, SettingsState>) -> Result<(), String> {
    settings.validate()?;
    let previous = state.0.lock().unwrap().clone();
    if previous == settings {
        return Ok(());
    }
    save(&app, &settings)?;
    apply(&app, &settings);
    if previous.replace_taskbar != settings.replace_taskbar {
        std::thread::sleep(TASKBAR_SETTLE);
        dock::layout(&app, &settings);
    }
    *state.0.lock().unwrap() = settings.clone();
    app.emit("settings-changed", &settings).map_err(|e| e.to_string())
}
