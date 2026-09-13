# Flow Linux Tauri proof of concept

This is an isolated desktop prototype for evaluating a Tauri/Rust migration. It does not replace the Python application.

The prototype tests the parts with the most migration risk:

- Holding Ctrl+Super starts microphone capture; releasing either modifier stops it.
- The listening pill is hidden at idle, shown during capture, and placed at the bottom of the monitor containing the active app window (with a primary-monitor fallback).
- A tray menu opens the status window or quits the prototype.
- The POC reads the default PipeWire microphone stream through pw-record and measures duration/RMS. This follows the desktop's selected source, avoiding a silent direct-ALSA device. Audio stays in memory and is discarded at stop. The prototype does not save, upload, transcribe, or insert audio/text.

The global key listener currently uses rdev and X11. It is appropriate for testing this X11 desktop, but it is not a Wayland implementation. A Wayland-capable evdev approach needs broader input-device access and a packaging/udev plan before it would be suitable for users.

## Run

Install Tauri's Linux prerequisites and PipeWire tools (pw-record), then run from this directory:

    cargo run --manifest-path src-tauri/Cargo.toml

Right-click the Flow tray icon and choose Open Flow to see the last status. Hold Ctrl+Super and speak for a couple of seconds; release either key. The log and status window report capture duration and RMS, then the in-memory samples are cleared.

The mic test requires you to speak; synthetic keyboard events only verify that the stream starts and stops. No audio leaves the process or is written to disk.
