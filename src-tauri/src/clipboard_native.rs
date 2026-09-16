use arboard::Clipboard;
use rdev::{simulate, EventType, Key};
use std::{thread, time::Duration};

// Overlay placement falls back to the active monitor through Tauri.
pub fn active_window() -> Option<String> {
    None
}

pub fn copy(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("No text to copy".into());
    }
    Clipboard::new()
        .map_err(|e| format!("Clipboard unavailable: {e}"))?
        .set_text(text)
        .map_err(|e| format!("Could not set clipboard text: {e}"))
}

pub fn paste(text: &str, restore_delay: f64) -> Result<(), String> {
    if text.is_empty() {
        return Err("No text to insert".into());
    }
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
    let previous = clipboard.get_text().ok();
    clipboard
        .set_text(text)
        .map_err(|e| format!("Could not set clipboard text: {e}"))?;
    thread::sleep(Duration::from_millis(80));
    #[cfg(target_os = "macos")]
    let modifier = Key::MetaLeft;
    #[cfg(target_os = "windows")]
    let modifier = Key::ControlLeft;
    let result = (|| {
        simulate(&EventType::KeyPress(modifier))?;
        simulate(&EventType::KeyPress(Key::KeyV))?;
        simulate(&EventType::KeyRelease(Key::KeyV))?;
        simulate(&EventType::KeyRelease(modifier))
    })();
    // Always release injected keys, even if a preceding event failed.
    let _ = simulate(&EventType::KeyRelease(Key::KeyV));
    let _ = simulate(&EventType::KeyRelease(modifier));
    result.map_err(|_| {
        "Text copied, but paste failed. Check accessibility permissions.".to_string()
    })?;
    thread::sleep(Duration::from_secs_f64(restore_delay.max(0.1)));
    // Do not overwrite a clipboard change made by the user while we waited.
    if clipboard.get_text().ok().as_deref() == Some(text) {
        if let Some(previous) = previous {
            let _ = clipboard.set_text(previous);
        }
    }
    Ok(())
}
