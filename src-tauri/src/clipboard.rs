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
fn window_class(window: &str) -> Option<String> {
    output(Command::new("xdotool").args(["getwindowclassname", window]))
        .map(|b| String::from_utf8_lossy(&b).trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}
fn window_title(window: &str) -> Option<String> {
    output(Command::new("xdotool").args(["getwindowname", window]))
        .map(|b| String::from_utf8_lossy(&b).trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}
fn is_terminal(class: &str, title: &str) -> bool {
    let class = class.to_ascii_lowercase();
    let title = title.to_ascii_lowercase();
    [
        "terminal",
        "xterm",
        "kitty",
        "alacritty",
        "wezterm",
        "konsole",
        "foot",
        "urxvt",
        "rxvt",
        "st-",
        "stterm",
        "tilix",
        "terminator",
        "qterminal",
        "lxterminal",
        "mate-terminal",
        "ghostty",
        "contour",
        "hyper",
        "tabby",
        "rio",
        "cool-retro-term",
        "blackbox",
        "ptyxis",
        "kgx",
    ]
    .iter()
    .any(|name| class.contains(name) || title.contains(name))
}
fn paste_shortcut(class: Option<&str>, title: Option<&str>) -> &'static str {
    let class = class.unwrap_or_default().to_ascii_lowercase();
    let title = title.unwrap_or_default().to_ascii_lowercase();
    if class.contains("codex") || title.contains("codex") {
        // Codex CLI treats Ctrl+V as an image attachment command. Shift+Insert
        // asks the terminal to paste the text clipboard as bracketed input.
        "shift+Insert"
    } else if is_terminal(&class, &title) {
        // Avoid forwarding Ctrl+V to terminal TUIs where it can mean image paste.
        "shift+Insert"
    } else {
        "ctrl+v"
    }
}
fn write_clipboard(value: &[u8]) -> Result<(), String> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard", "-i"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| "Clipboard unavailable; install xclip".to_string())?;
    child
        .stdin
        .take()
        .ok_or("Clipboard unavailable")?
        .write_all(value)
        .map_err(|_| "Could not set clipboard text")?;
    if !child
        .wait()
        .map_err(|_| "Could not set clipboard text")?
        .success()
    {
        return Err("Could not set clipboard text".into());
    }
    Ok(())
}
pub fn paste(text: &str, restore_delay: f64) -> Result<(), String> {
    if text.is_empty() {
        return Err("No text to insert".into());
    }
    // Paste into the window focused when transcription finishes. Dictation may
    // take several seconds, during which the user can move to another window.
    let target = active_window();
    let class = target.as_deref().and_then(window_class);
    let title = target.as_deref().and_then(window_title);
    let previous = output(Command::new("xclip").args(["-selection", "clipboard", "-o"]));
    let write_result = write_clipboard(text.as_bytes());
    let paste_result = write_result.and_then(|()| {
        let shortcut = paste_shortcut(class.as_deref(), title.as_deref());
        println!("[Flow] Inserting text with {shortcut}");
        let status = Command::new("xdotool")
            .args(["key", "--clearmodifiers", shortcut])
            .status()
            .map_err(|_| "Text insertion failed; install xdotool")?;
        if status.success() {
            Ok(())
        } else {
            Err("Text insertion failed; install xdotool".into())
        }
    });
    if let Some(previous) = previous {
        thread::sleep(Duration::from_secs_f64(restore_delay.min(10.0)));
        let _ = write_clipboard(&previous);
    }
    paste_result
}

#[cfg(test)]
mod tests {
    use super::{is_terminal, paste_shortcut};

    #[test]
    fn recognizes_common_terminal_windows() {
        for class in [
            "XTerm",
            "kitty",
            "Alacritty",
            "org.gnome.Terminal",
            "xfce4-terminal",
            "com.mitchellh.ghostty",
            "st-256color",
            "org.wezfurlong.wezterm",
        ] {
            assert!(
                is_terminal(class, ""),
                "{class} should be treated as a terminal"
            );
        }
    }

    #[test]
    fn uses_regular_paste_for_non_terminal_windows() {
        assert_eq!(paste_shortcut(Some("code"), Some("editor")), "ctrl+v");
        assert_eq!(paste_shortcut(None, None), "ctrl+v");
    }

    #[test]
    fn uses_text_paste_shortcut_for_terminals_and_codex() {
        assert_eq!(paste_shortcut(Some("kitty"), None), "shift+Insert");
        assert_eq!(
            paste_shortcut(Some("xfce4-terminal"), Some("codex")),
            "shift+Insert"
        );
        assert_eq!(
            paste_shortcut(None, Some("Codex CLI — ~/project")),
            "shift+Insert"
        );
    }

    #[test]
    fn recognizes_terminal_from_window_title() {
        assert!(is_terminal("unknown-window", "Codex - Terminal"));
    }
}
