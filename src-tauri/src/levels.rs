//! Volume (default speakers) and brightness (built-in laptop screen).
//!
//! Both APIs are COM, which must be used from the thread that set it up, so one
//! worker thread owns them and the commands send it requests. Scrolling fires
//! many "set" requests quickly; the worker only applies the newest of each.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::State;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use wmi::WMIConnection;

#[derive(Clone, Copy, Serialize)]
pub struct Volume {
    /// 0.0 to 1.0
    level: f32,
    muted: bool,
}

enum Request {
    GetVolume(Sender<Result<Volume, String>>),
    SetVolume(f32),
    /// Replies None when the screen has no software brightness control (e.g. a desktop monitor).
    GetBrightness(Sender<Option<u8>>),
    SetBrightness(u8),
}

pub struct Levels(Mutex<Sender<Request>>);

pub fn start() -> Levels {
    let (tx, rx) = channel();
    thread::spawn(move || worker(rx));
    Levels(Mutex::new(tx))
}

fn worker(rx: Receiver<Request>) {
    // Connecting to WMI first also sets up COM (multithreaded) and its security settings
    // for this thread, which the audio API then uses too.
    let wmi = WMIConnection::with_namespace_path("ROOT\\WMI").expect("can't connect to WMI");

    while let Ok(first) = rx.recv() {
        // Coalesce: of everything waiting, keep only the newest volume and brightness.
        let mut volume = None;
        let mut brightness = None;
        for request in std::iter::once(first).chain(rx.try_iter()) {
            match request {
                Request::SetVolume(level) => volume = Some(level),
                Request::SetBrightness(level) => brightness = Some(level),
                Request::GetVolume(reply) => _ = reply.send(get_volume()),
                Request::GetBrightness(reply) => _ = reply.send(get_brightness(&wmi)),
            }
        }
        if let Some(level) = volume {
            if let Err(e) = set_volume(level) {
                eprintln!("[levels] set volume failed: {e}");
            }
        }
        if let Some(level) = brightness {
            if let Err(e) = set_brightness(&wmi, level) {
                eprintln!("[levels] set brightness failed: {e}");
            }
        }
    }
}

/// The default output device; looked up each time so switching speakers/headphones just works.
fn endpoint() -> windows::core::Result<IAudioEndpointVolume> {
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        devices.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate(CLSCTX_ALL, None)
    }
}

fn get_volume() -> Result<Volume, String> {
    let endpoint = endpoint().map_err(|e| e.to_string())?;
    unsafe {
        Ok(Volume {
            level: endpoint.GetMasterVolumeLevelScalar().map_err(|e| e.to_string())?,
            muted: endpoint.GetMute().map_err(|e| e.to_string())?.as_bool(),
        })
    }
}

fn set_volume(level: f32) -> windows::core::Result<()> {
    let endpoint = endpoint()?;
    unsafe {
        endpoint.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null())?;
        // Turning the volume up unmutes, like the Windows volume keys.
        if level > 0.0 {
            endpoint.SetMute(false, std::ptr::null())?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename = "WmiMonitorBrightness", rename_all = "PascalCase")]
struct WmiMonitorBrightness {
    current_brightness: u8,
}

#[derive(Deserialize)]
#[serde(rename = "WmiMonitorBrightnessMethods")]
struct WmiMonitorBrightnessMethods {
    #[serde(rename = "__Path")]
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct WmiSetBrightness {
    /// Seconds Windows keeps this level before its own policies may change it; 0 = no timeout.
    timeout: u32,
    brightness: u8,
}

fn get_brightness(wmi: &WMIConnection) -> Option<u8> {
    let screens: Vec<WmiMonitorBrightness> = wmi.query().ok()?;
    screens.first().map(|s| s.current_brightness)
}

fn set_brightness(wmi: &WMIConnection, level: u8) -> Result<(), String> {
    let screens: Vec<WmiMonitorBrightnessMethods> = wmi.query().map_err(|e| e.to_string())?;
    let screen = screens.first().ok_or("this screen has no brightness control")?;
    let _: wmi::Variant = wmi
        .exec_instance_method::<WmiMonitorBrightnessMethods, _>(
            &screen.path,
            "WmiSetBrightness",
            WmiSetBrightness { timeout: 0, brightness: level.min(100) },
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn send(levels: &State<Levels>, request: Request) {
    levels.0.lock().unwrap().send(request).expect("levels worker stopped");
}

#[tauri::command]
pub async fn get_volume_level(levels: State<'_, Levels>) -> Result<Volume, String> {
    let (tx, rx) = channel();
    send(&levels, Request::GetVolume(tx));
    rx.recv().map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_volume_level(level: f32, levels: State<Levels>) {
    send(&levels, Request::SetVolume(level));
}

#[tauri::command]
pub async fn get_brightness_level(levels: State<'_, Levels>) -> Result<Option<u8>, String> {
    let (tx, rx) = channel();
    send(&levels, Request::GetBrightness(tx));
    rx.recv().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_brightness_level(level: u8, levels: State<Levels>) {
    send(&levels, Request::SetBrightness(level));
}
