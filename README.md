# Flow Linux

Flow is a small X11 desktop dictation app built with Tauri and Rust. Hold **Ctrl+Super**, speak, then release either key to transcribe and paste into the window that was active when recording began. The floating listening pill appears during microphone capture and stays hidden at idle. A tray icon keeps Flow running in the background.

## Requirements

- Kali/Debian Linux, X11, and a working PipeWire microphone
- Rust/Cargo for building from source
- `pw-record`, `curl`, `xclip`, `xdotool`, `xrandr`, and `sqlite3`
- A Groq API key with access to `whisper-large-v3-turbo`

Wayland is not supported yet. Global shortcuts, focus restoration, and paste-back use X11 behavior.

## Install

Install the system packages and Rust toolchain if they are not already available. On Debian/Kali, the desktop build libraries include GTK/WebKitGTK and AppIndicator development packages. Then run:

```sh
./install.sh
```

The installer builds the Rust app and replaces the old Python launcher at `~/.local/bin/flow-linux`. It removes only the retired Python runtime files under `~/.local/share/flow-linux`; your configuration, Groq key, and `history.db` remain in place.

Open **Flow Linux** from the application menu or tray. Enter or confirm the Groq API key in Settings. Settings live in `~/.config/flow-linux/config.toml` with private file permissions. An existing TOML file is loaded as-is; if only the older `.env` exists, Flow imports its Groq key, model, and language without deleting it.

## Use

- **Push to talk:** focus a text field, hold **Ctrl+Super**, speak, and release either modifier.
- **Hands-free:** enable the option in Settings, then press **Ctrl+Super+Space** to start listening and press it again to stop and transcribe.
- Use the tray menu to open Flow or quit. Closing the main window hides it to the tray.

The overlay appears only while listening. Audio is captured from the system default PipeWire source, kept in memory during recording, written to a private temporary WAV file for the Groq request, then deleted. The API key is stored in the private config and passed to curl through a private temporary config file, not in process arguments.

## Settings and history

Settings include the Groq key, model, language, hands-free mode, login autostart, and optional local history. The microphone uses the system default PipeWire source. Existing PortAudio device numbers are retained in the config for compatibility but are not used by PipeWire.

When history is enabled, Flow stores transcript text, timestamp, duration, provider, model, and paste status in `~/.local/share/flow-linux/history.db` (or `$XDG_DATA_HOME/flow-linux/history.db`). It stores no audio, sends no telemetry, and keeps at most 500 entries. The History tab can search or delete saved transcripts. The existing Python app database is used directly, without conversion.

## Development and checks

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --offline --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml
```

The main Rust modules are under `src-tauri/src/`; the local Tauri UI is under `ui/`. Runtime dependencies are system programs, so they do not add network-fetched Rust crates to the application.

For a desktop smoke test, launch Flow in an X11 session, open a text editor, hold the shortcut while speaking, and verify the pill appears only during capture and that the transcript is pasted into the editor. Test with a real Groq key to verify the network transcription path. Microphone and Groq live checks require a desktop session, microphone, internet access, and API credentials.
