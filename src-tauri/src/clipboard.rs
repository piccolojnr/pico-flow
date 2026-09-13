use std::{
    io::Write,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
fn output(command: &mut Command) -> Option<Vec<u8>> {
    let result = command.output().ok()?;
    result.status.success().then_some(result.stdout)
}
pub fn active_window() -> Option<String> {
    output(Command::new("xdotool").arg("getactivewindow"))
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .filter(|s| !s.is_empty())
}
pub fn paste(text: &str, target: Option<&str>, restore_delay: f64) -> Result<(), String> {
    if text.is_empty() {
        return Err("No text to insert".into());
    }
    let previous = output(Command::new("xclip").args(["-selection", "clipboard", "-o"]));
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard", "-i"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| "Clipboard unavailable; install xclip".to_string())?;
    child
        .stdin
        .take()
        .ok_or("Clipboard unavailable")?
        .write_all(text.as_bytes())
        .map_err(|_| "Could not set clipboard text")?;
    if !child
        .wait()
        .map_err(|_| "Could not set clipboard text")?
        .success()
    {
        return Err("Could not set clipboard text".into());
    }
    if let Some(window) = target {
        let status = Command::new("xdotool")
            .args(["windowactivate", "--sync", window])
            .status()
            .map_err(|_| "Could not return to the original window; install xdotool")?;
        if !status.success() {
            return Err("Could not return focus to the original window".into());
        }
    }
    let status = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .status()
        .map_err(|_| "Text insertion failed; install xdotool")?;
    if !status.success() {
        return Err("Text insertion failed; install xdotool".into());
    }
    if let Some(previous) = previous {
        thread::sleep(Duration::from_secs_f64(restore_delay.min(10.0)));
        if let Ok(mut restore) = Command::new("xclip")
            .args(["-selection", "clipboard", "-i"])
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut input) = restore.stdin.take() {
                let _ = input.write_all(&previous);
            }
            let _ = restore.wait();
        }
    }
    Ok(())
}
