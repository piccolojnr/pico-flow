use std::{
    io::{BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use rdev::{listen, EventType, Key};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    webview::WebviewWindowBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WindowEvent,
};

fn append_f32_samples(output: &mut Vec<f32>, pending: &mut Vec<u8>, incoming: &[u8]) {
    pending.extend_from_slice(incoming);
    let complete_len = pending.len() / 4 * 4;
    output.extend(
        pending[..complete_len]
            .chunks_exact(4)
            .map(|sample| f32::from_le_bytes(sample.try_into().expect("four-byte sample"))),
    );
    pending.drain(..complete_len);
}

struct Recorder {
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    samples: Arc<Mutex<Vec<f32>>>,
    started: Option<Instant>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            child: None,
            reader: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            started: None,
        }
    }
}

impl Recorder {
    fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        let mut child = Command::new("pw-record")
            .args([
                "--raw",
                "--format",
                "f32",
                "--rate",
                "48000",
                "--channels",
                "1",
                "--channel-map",
                "MONO",
                "-",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| {
                format!("could not start pw-record (install PipeWire tools): {error}")
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "pw-record did not provide an audio stream".to_string())?;
        let buffer = Arc::clone(&self.samples);
        self.reader = Some(thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut bytes = [0u8; 4096];
            let mut pending = Vec::with_capacity(3);
            loop {
                let count = match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => count,
                };
                if let Ok(mut output) = buffer.lock() {
                    append_f32_samples(&mut output, &mut pending, &bytes[..count]);
                } else {
                    break;
                }
            }
        }));
        self.child = Some(child);
        self.started = Some(Instant::now());
        println!("[Flow POC] input source: PipeWire default microphone");
        Ok(())
    }

    fn stop(&mut self) -> (f64, f64, usize) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let duration = self
            .started
            .take()
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let (rms, sample_count) = self
            .samples
            .lock()
            .map(|samples| {
                let sum_squares: f64 = samples
                    .iter()
                    .map(|sample| f64::from(*sample) * f64::from(*sample))
                    .sum();
                let rms = if samples.is_empty() {
                    0.0
                } else {
                    (sum_squares / samples.len() as f64).sqrt()
                };
                (rms, samples.len())
            })
            .unwrap_or((0.0, 0));
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
        let capture_duration = sample_count as f64 / 48_000.0;
        (capture_duration.max(duration.min(0.15)), rms, sample_count)
    }
}

#[derive(Default)]
struct PttState {
    recorder: Mutex<Recorder>,
    active: AtomicBool,
    last_status: Mutex<String>,
}

#[tauri::command]
fn get_last_status(state: tauri::State<'_, PttState>) -> String {
    state
        .last_status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| "Status unavailable".to_string())
}

fn parse_monitor_geometry(geometry: &str) -> Option<(i32, i32, i32, i32)> {
    let x_separator = geometry.find('x')?;
    let width = geometry[..x_separator].split('/').next()?.parse().ok()?;
    let height_and_offsets = &geometry[x_separator + 1..];
    let (height, millimeters_and_offsets) = height_and_offsets.split_once('/')?;
    let height = height.parse().ok()?;
    let offset_start = millimeters_and_offsets.find(['+', '-'])?;
    let offsets = &millimeters_and_offsets[offset_start..];
    let y_offset_start = offsets[1..].find(['+', '-'])? + 1;
    let x = offsets[..y_offset_start].parse().ok()?;
    let y = offsets[y_offset_start..].parse().ok()?;
    Some((x, y, width, height))
}

fn active_window_id() -> Option<String> {
    Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|id| !id.is_empty())
}

fn active_monitor_bounds(window_id: Option<&str>) -> Option<(i32, i32, i32, i32)> {
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
                let mut values = std::collections::HashMap::new();
                for line in geometry.lines() {
                    if let Some((key, value)) = line.split_once('=') {
                        values.insert(key, value);
                    }
                }
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

fn show_overlay(app: &AppHandle, target_window: Option<&str>) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.set_size(PhysicalSize::new(320, 64));
        let bounds = active_monitor_bounds(target_window).or_else(|| {
            overlay.primary_monitor().ok().flatten().map(|monitor| {
                let size = monitor.size();
                let origin = monitor.position();
                (origin.x, origin.y, size.width as i32, size.height as i32)
            })
        });
        if let Some((x, y, width, height)) = bounds {
            let _ = overlay.set_position(PhysicalPosition::new(
                x + (width - 320) / 2,
                y + height - 94,
            ));
        }
        let _ = overlay.show();
    }
}

fn start_recording(app: &AppHandle, target_window: Option<String>) {
    let state = app.state::<PttState>();
    if state.active.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = state
        .recorder
        .lock()
        .map_err(|_| "microphone state lock failed".to_string())
        .and_then(|mut recorder| recorder.start());
    match result {
        Ok(()) => {
            show_overlay(app, target_window.as_deref());
            if let Ok(mut status) = state.last_status.lock() {
                *status = "Listening · microphone is capturing".to_string();
            }
            let _ = app.emit("ptt-state", serde_json::json!({"active": true}));
            println!("[Flow POC] recording started");
        }
        Err(error) => {
            state.active.store(false, Ordering::SeqCst);
            let message = format!("Microphone unavailable: {error}");
            if let Ok(mut status) = state.last_status.lock() {
                *status = message.clone();
            }
            let _ = app.emit(
                "ptt-state",
                serde_json::json!({"active": false, "message": message}),
            );
            eprintln!("[Flow POC] microphone unavailable: {error}");
        }
    }
}

fn stop_recording(app: &AppHandle) {
    let state = app.state::<PttState>();
    if !state.active.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
    let (duration, rms, sample_count) = state
        .recorder
        .lock()
        .map(|mut recorder| recorder.stop())
        .unwrap_or((0.0, 0.0, 0));
    let summary = format!("Stopped · {duration:.2}s · RMS {rms:.4} · {sample_count} samples");
    if let Ok(mut status) = state.last_status.lock() {
        *status = summary.clone();
    }
    let _ = app.emit(
        "ptt-state",
        serde_json::json!({"active": false, "message": summary}),
    );
    println!(
        "[Flow POC] duration={duration:.2}s samples={sample_count} rms={rms:.4}; audio discarded"
    );
}

fn start_keyboard_listener(app: AppHandle) {
    thread::spawn(move || {
        let mut ctrl = false;
        let mut super_key = false;
        let mut active = false;
        if let Err(error) = listen(move |event| {
            match event.event_type {
                EventType::KeyPress(Key::ControlLeft | Key::ControlRight) => ctrl = true,
                EventType::KeyRelease(Key::ControlLeft | Key::ControlRight) => ctrl = false,
                EventType::KeyPress(Key::MetaLeft | Key::MetaRight) => super_key = true,
                EventType::KeyRelease(Key::MetaLeft | Key::MetaRight) => super_key = false,
                _ => {}
            }
            let chord = ctrl && super_key;
            if chord && !active {
                active = true;
                start_recording(&app, active_window_id());
            } else if !chord && active {
                active = false;
                stop_recording(&app);
            }
        }) {
            eprintln!("[Flow POC] global keyboard listener stopped: {error:?}");
        }
    });
}

fn create_overlay(app: &tauri::App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Flow Linux · Listening")
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
    tauri::Builder::default()
        .manage(PttState {
            recorder: Mutex::new(Recorder::default()),
            active: AtomicBool::new(false),
            last_status: Mutex::new("Idle · no microphone capture".to_string()),
        })
        .invoke_handler(tauri::generate_handler![get_last_status])
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            create_overlay(app)?;
            let open = MenuItem::with_id(app, "open", "Open Flow", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let icon = tauri::image::Image::new(include_bytes!("../icons/icon.rgba"), 64, 64);
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("Flow Linux Tauri POC")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            start_keyboard_listener(app.handle().clone());
            println!("[Flow POC] ready; idle overlay is hidden");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Flow Linux Tauri POC");
}

#[cfg(test)]
mod tests {
    use super::{append_f32_samples, parse_monitor_geometry};

    #[test]
    fn decodes_pipewire_f32_samples_across_partial_reads() {
        let expected = [0.25f32, -0.5f32];
        let encoded: Vec<u8> = expected
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let mut output = Vec::new();
        let mut pending = Vec::new();
        append_f32_samples(&mut output, &mut pending, &encoded[..5]);
        assert_eq!(output, vec![0.25]);
        append_f32_samples(&mut output, &mut pending, &encoded[5..]);
        assert_eq!(output, expected);
        assert!(pending.is_empty());
    }

    #[test]
    fn parses_xrandr_monitor_geometry_with_signed_offsets() {
        assert_eq!(
            parse_monitor_geometry("1920/300x1080/170+0+0"),
            Some((0, 0, 1920, 1080))
        );
        assert_eq!(
            parse_monitor_geometry("1920/520x1080/300-1920+0"),
            Some((-1920, 0, 1920, 1080))
        );
    }
}
