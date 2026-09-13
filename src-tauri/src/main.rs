mod audio;
mod clipboard;
mod config;
mod history;
mod instance;
mod transcription;

use audio::{Capture, Recorder};
use config::Config;
use rdev::{listen, EventType};
use std::{
    collections::HashSet, fs, path::PathBuf, process::Command, sync::Mutex, thread, time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    webview::WebviewWindowBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WindowEvent,
};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    PushToTalk,
    HandsFree,
}
struct State {
    config: Mutex<Config>,
    recorder: Mutex<Recorder>,
    mode: Mutex<Option<Mode>>,
    target: Mutex<Option<String>>,
    status: Mutex<String>,
}

#[tauri::command]
fn get_config(state: tauri::State<'_, State>) -> Config {
    state.config.lock().map(|c| c.clone()).unwrap_or_default()
}
#[tauri::command]
fn get_status(state: tauri::State<'_, State>) -> String {
    state
        .status
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "Status unavailable".into())
}
#[tauri::command]
fn save_config(
    app: AppHandle,
    state: tauri::State<'_, State>,
    config: Config,
) -> Result<(), String> {
    config::save(&config)?;
    if let Ok(mut current) = state.config.lock() {
        *current = config.clone();
    }
    set_autostart(config.app.autostart)?;
    let _ = app.emit("config-saved", ());
    Ok(())
}
#[tauri::command]
fn history_list(search: String) -> Result<Vec<history::Entry>, String> {
    history::list(&search)
}
#[tauri::command]
fn history_delete(id: i64) -> Result<(), String> {
    history::delete(id)
}
#[tauri::command]
fn history_clear() -> Result<(), String> {
    history::clear()
}

fn set_autostart(enabled: bool) -> Result<(), String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let path = base.join("autostart/flow-linux.desktop");
    if !enabled {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        };
        return Ok(());
    }
    let installed =
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/bin/flow-linux");
    let executable = if installed.is_file() {
        installed
    } else {
        std::env::current_exe().map_err(|e| e.to_string())?
    };
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let escaped = executable
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    fs::write(&path, format!("[Desktop Entry]\nType=Application\nName=Flow Linux\nExec=\"{escaped}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n" )).map_err(|e| format!("Could not update login startup: {e}"))
}

fn status(app: &AppHandle, message: &str, active: bool) {
    if let Some(state) = app.try_state::<State>() {
        if let Ok(mut last) = state.status.lock() {
            *last = message.to_string();
        }
    }
    let _ = app.emit(
        "flow-status",
        serde_json::json!({"message": message, "active": active}),
    );
}
fn parse_monitor_geometry(geometry: &str) -> Option<(i32, i32, i32, i32)> {
    let x_separator = geometry.find('x')?;
    let width = geometry[..x_separator].split('/').next()?.parse().ok()?;
    let (height, dimensions) = geometry[x_separator + 1..].split_once('/')?;
    let height = height.parse().ok()?;
    let offset_start = dimensions.find(['+', '-'])?;
    let offsets = &dimensions[offset_start..];
    let y_offset_start = offsets[1..].find(['+', '-'])? + 1;
    Some((
        offsets[..y_offset_start].parse().ok()?,
        offsets[y_offset_start..].parse().ok()?,
        width,
        height,
    ))
}
fn monitor_bounds_for_window(window_id: Option<&str>) -> Option<(i32, i32, i32, i32)> {
    let output = Command::new("xrandr")
        .args(["--listactivemonitors"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let monitors: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| {
            let geometry = line.split_whitespace().nth(2)?;
            Some((line.contains('*'), parse_monitor_geometry(geometry)?))
        })
        .collect();

    if let Some(id) = window_id {
        if let Ok(output) = Command::new("xdotool")
            .args(["getwindowgeometry", "--shell", id])
            .output()
        {
            if output.status.success() {
                let geometry = String::from_utf8_lossy(&output.stdout);
                let values: std::collections::HashMap<_, _> = geometry
                    .lines()
                    .filter_map(|line| line.split_once('='))
                    .collect();
                let center_x = values.get("X")?.parse::<i32>().ok()?
                    + values.get("WIDTH")?.parse::<i32>().ok()? / 2;
                let center_y = values.get("Y")?.parse::<i32>().ok()?
                    + values.get("HEIGHT")?.parse::<i32>().ok()? / 2;
                if let Some((_, bounds)) = monitors.iter().find(|(_, (x, y, width, height))| {
                    *x <= center_x
                        && center_x < *x + *width
                        && *y <= center_y
                        && center_y < *y + *height
                }) {
                    return Some(*bounds);
                }
            }
        }
    }
    monitors
        .iter()
        .find(|(primary, _)| *primary)
        .or(monitors.first())
        .map(|(_, bounds)| *bounds)
}
fn set_overlay(app: &AppHandle, show: bool, target_window: Option<&str>) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if show {
            let _ = overlay.set_size(PhysicalSize::new(320, 64));
            let bounds = monitor_bounds_for_window(target_window).or_else(|| {
                overlay.primary_monitor().ok().flatten().map(|monitor| {
                    let size = monitor.size();
                    let pos = monitor.position();
                    (pos.x, pos.y, size.width as i32, size.height as i32)
                })
            });
            if let Some((x, y, width, height)) = bounds {
                let _ = overlay.set_position(PhysicalPosition::new(
                    x + (width - 320) / 2,
                    y + height - 100,
                ));
            }
            let _ = overlay.show();
        } else {
            let _ = overlay.hide();
        }
    }
}
fn start_recording(app: &AppHandle, mode: Mode) {
    let Some(state) = app.try_state::<State>() else {
        return;
    };
    let mut current_mode = match state.mode.lock() {
        Ok(m) => m,
        Err(_) => return,
    };
    if current_mode.is_some() {
        if mode == Mode::HandsFree && *current_mode == Some(Mode::PushToTalk) {
            *current_mode = Some(Mode::HandsFree);
            status(app, "Hands-free listening", true);
        }
        return;
    }
    let target = clipboard::active_window();
    let result = state
        .recorder
        .lock()
        .map_err(|_| "Microphone state unavailable".to_string())
        .and_then(|mut recorder| recorder.start());
    match result {
        Ok(()) => {
            *current_mode = Some(mode);
            set_overlay(app, true, target.as_deref());
            if let Ok(mut t) = state.target.lock() {
                *t = target;
            }
            status(
                app,
                if mode == Mode::HandsFree {
                    "Hands-free listening"
                } else {
                    "Listening"
                },
                true,
            );
            println!("[Flow] Recording started");
        }
        Err(error) => {
            *current_mode = None;
            status(app, &format!("Microphone unavailable: {error}"), false);
            eprintln!("[Flow] {error}");
        }
    }
}
fn stop_recording(app: &AppHandle, allow_handsfree: bool) {
    let Some(state) = app.try_state::<State>() else {
        return;
    };
    let mode = match state.mode.lock() {
        Ok(mut mode) => {
            let old = *mode;
            if old == Some(Mode::HandsFree) && !allow_handsfree {
                return;
            }
            *mode = None;
            old
        }
        Err(_) => None,
    };
    if mode.is_none() {
        return;
    }
    set_overlay(app, false, None);
    let capture = state
        .recorder
        .lock()
        .map_err(|_| "Microphone state unavailable".to_string())
        .and_then(|mut r| r.stop());
    let target = state.target.lock().ok().and_then(|mut t| t.take());
    match capture {
        Ok(capture) => finish_capture(app.clone(), capture, target),
        Err(error) => report_error(app.clone(), error),
    }
}
fn report_error(app: AppHandle, message: String) {
    eprintln!("[Flow] {message}");
    status(&app, &message, false);
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));
        if let Some(state) = app.try_state::<State>() {
            if let Ok(s) = state.status.lock() {
                if *s == message {
                    drop(s);
                    status(&app, "Ready · hold Ctrl+Super to dictate", false);
                }
            }
        }
    });
}
fn finish_capture(app: AppHandle, capture: Capture, target: Option<String>) {
    let Some(state) = app.try_state::<State>() else {
        let _ = fs::remove_file(capture.wav_path);
        return;
    };
    let config = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    println!(
        "[Flow] duration={:.2}s rms={:.0}",
        capture.duration, capture.rms
    );
    if capture.duration < config.audio.minimum_duration {
        let _ = fs::remove_file(capture.wav_path);
        report_error(app, "Recording too short".into());
        return;
    }
    if capture.rms < config.audio.silence_threshold {
        let _ = fs::remove_file(capture.wav_path);
        report_error(app, "No speech detected".into());
        return;
    }
    status(&app, "Transcribing…", false);
    thread::spawn(move || {
        let result = transcription::transcribe(
            &capture.wav_path,
            &config.transcription.groq_api_key,
            &config.transcription.model,
            &config.transcription.language,
            config.transcription.api_timeout,
        );
        let _ = fs::remove_file(capture.wav_path);
        match result {
            Ok(text) if text.trim().is_empty() => report_error(app, "No speech detected".into()),
            Ok(text) => {
                println!("[Flow] Transcript received; inserting text");
                let entry_id = if config.app.save_history {
                    history::add(text.trim(), capture.duration, &config.transcription.model).ok()
                } else {
                    None
                };
                match clipboard::paste(
                    text.trim(),
                    target.as_deref(),
                    config.app.clipboard_restore_delay,
                ) {
                    Ok(()) => {
                        if let Some(id) = entry_id {
                            history::mark(id, "inserted");
                        }
                        status(&app, "Text inserted", false);
                        let _ = app.emit("history-updated", ());
                        let app2 = app.clone();
                        thread::spawn(move || {
                            thread::sleep(Duration::from_secs(1));
                            status(&app2, "Ready · hold Ctrl+Super to dictate", false);
                        });
                    }
                    Err(error) => {
                        if let Some(id) = entry_id {
                            history::mark(id, "failed");
                        }
                        report_error(app, error);
                    }
                }
            }
            Err(error) => report_error(app, error),
        }
    });
}

fn pressed_chord_parts(pressed: &HashSet<String>) -> (bool, bool, bool) {
    (
        pressed.iter().any(|key| key.contains("Control")),
        pressed.iter().any(|key| key.contains("Meta")),
        pressed.iter().any(|key| key.contains("Space")),
    )
}

fn start_keyboard_listener(app: AppHandle) {
    thread::spawn(move || {
        let mut pressed = HashSet::<String>::new();
        let mut ptt = false;
        let mut handsfree_chord = false;
        let callback = move |event: rdev::Event| {
            let key = match event.event_type {
                EventType::KeyPress(key) | EventType::KeyRelease(key) => key,
                _ => return,
            };
            let name = format!("{key:?}");
            match event.event_type {
                EventType::KeyPress(_) => {
                    pressed.insert(name.clone());
                }
                EventType::KeyRelease(_) => {
                    pressed.remove(&name);
                }
                _ => {}
            }
            let (ctrl, super_key, space) = pressed_chord_parts(&pressed);
            let chord = ctrl && super_key;
            let hf = chord && space;
            if hf && !handsfree_chord {
                let enabled = app
                    .try_state::<State>()
                    .map(|s| {
                        s.config
                            .lock()
                            .map(|c| c.shortcuts.handsfree_enabled)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if enabled {
                    let active_handsfree = app
                        .try_state::<State>()
                        .and_then(|s| s.mode.lock().ok().map(|m| *m == Some(Mode::HandsFree)))
                        .unwrap_or(false);
                    if active_handsfree {
                        stop_recording(&app, true);
                    } else {
                        start_recording(&app, Mode::HandsFree);
                    }
                }
            }
            handsfree_chord = hf;
            let handsfree_enabled = app
                .try_state::<State>()
                .map(|s| {
                    s.config
                        .lock()
                        .map(|c| c.shortcuts.handsfree_enabled)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if chord && !ptt && (!hf || !handsfree_enabled) {
                ptt = true;
                start_recording(&app, Mode::PushToTalk);
            }
            if !chord && ptt {
                ptt = false;
                stop_recording(&app, false);
            }
        };
        if let Err(error) = listen(callback) {
            eprintln!("[Flow] Global hotkey listener stopped: {error:?}");
        }
    });
}

fn build_overlay(app: &tauri::App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Flow Listening")
        .inner_size(320.0, 64.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .build()?;
    Ok(())
}
fn main() {
    let show_window = std::env::args().any(|arg| arg == "--settings" || arg == "--show");
    let claim = match instance::claim(show_window) {
        Ok(claim) => claim,
        Err(error) => {
            eprintln!("[Flow] Could not coordinate the running instance: {error}");
            std::process::exit(1);
        }
    };
    let instance::Claim::Primary { listener, path } = claim else {
        return;
    };
    let config = config::load().unwrap_or_else(|error| {
        eprintln!("[Flow] Settings error: {error}");
        Config::default()
    });
    let config = config;
    tauri::Builder::default()
        .manage(State {
            config: Mutex::new(config),
            recorder: Mutex::new(Recorder::default()),
            mode: Mutex::new(None),
            target: Mutex::new(None),
            status: Mutex::new("Ready · hold Ctrl+Super to dictate".into()),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_status,
            save_config,
            history_list,
            history_delete,
            history_clear
        ])
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            build_overlay(app)?;
            instance::serve(app.handle().clone(), listener, path);
            let autostart = app
                .state::<State>()
                .config
                .lock()
                .map(|config| config.app.autostart)
                .unwrap_or(false);
            if let Err(error) = set_autostart(autostart) {
                eprintln!("[Flow] Could not update login startup: {error}");
            }
            let open = MenuItem::with_id(app, "open", "Open Flow", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Flow", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let image = tauri::image::Image::new(include_bytes!("../icons/icon.rgba"), 64, 64);
            TrayIconBuilder::new()
                .icon(image)
                .menu(&menu)
                .tooltip("Flow Linux")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("flow-status-refresh", ());
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            start_keyboard_listener(app.handle().clone());
            if show_window {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            println!("[Flow] Ready in tray; overlay hidden until recording");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Flow Linux");
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_monitor_bounds_with_signed_offsets() {
        assert_eq!(
            super::parse_monitor_geometry("1920/300x1080/170+0+0"),
            Some((0, 0, 1920, 1080))
        );
        assert_eq!(
            super::parse_monitor_geometry("1920/520x1080/300-1920+0"),
            Some((-1920, 0, 1920, 1080))
        );
    }

    #[test]
    fn releasing_one_control_key_keeps_modifier_chord_active() {
        let mut pressed = std::collections::HashSet::from([
            "ControlLeft".to_string(),
            "ControlRight".to_string(),
            "MetaLeft".to_string(),
        ]);
        assert_eq!(super::pressed_chord_parts(&pressed), (true, true, false));
        pressed.remove("ControlLeft");
        assert_eq!(super::pressed_chord_parts(&pressed), (true, true, false));
        pressed.remove("ControlRight");
        assert_eq!(super::pressed_chord_parts(&pressed), (false, true, false));
    }
}
