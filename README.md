# Flow Linux

Flow is a small X11 desktop dictation app built with Tauri and Rust. Hold **Ctrl+Super**, speak, then release either key to transcribe and paste into the window active when transcription finishes. The floating pill shows capture and transcription status and stays hidden at idle. A tray icon keeps Flow running in the background.

## Requirements

- Kali/Debian Linux, X11, and a working PipeWire microphone
- Rust/Cargo for building from source (only needed with `--source`)
- `curl`, `pw-record`, `xclip`, `xdotool`, `xrandr`, and `sqlite3`
- A Groq API key with access to `whisper-large-v3-turbo`

Wayland is not supported yet. Global shortcuts and paste-back use X11 behavior.

## Install

The normal installer downloads the latest Linux AppImage from GitHub Releases, verifies it against the published `SHA256SUMS`, and installs it under `~/.local/opt/flow-linux`, with a stable launcher at `~/.local/bin/flow-linux`:

```sh
curl -fsSL https://raw.githubusercontent.com/piccolojnr/pico-flow/master/install.sh | bash
```

From a checkout, use `./install.sh --source` to build and install the current code; to pin a release, use `./install.sh --version 0.2.0`. The installer removes only retired Python runtime files under `~/.local/share/flow-linux`; your configuration, Groq key, and `history.db` remain in place.

On Debian/Kali, install the runtime tools with:

```sh
sudo apt install pipewire-bin xclip xdotool x11-xserver-utils sqlite3 curl
```

Open **Flow Linux** from the application menu or tray. Enter or confirm the Groq API key in Settings. Settings live in `~/.config/flow-linux/config.toml` with private file permissions. An existing TOML file is loaded as-is; if only the older `.env` exists, Flow imports its Groq key, model, and language without deleting it.

## Use

- **Push to talk:** hold **Ctrl+Super**, speak, and release either modifier to transcribe. Flow inserts the text into the window focused when transcription finishes, so you can switch to the destination while Flow works.
- **Hands-free:** enable the option in Settings, then press **Ctrl+Super+Space** to start listening and press it again to stop. The floating pill identifies hands-free listening and remains visible while transcribing.
- Use the tray menu to open Flow or quit. Closing the main window hides it to the tray.

The overlay appears while listening and transcribing, and shows whether Flow is in push-to-talk, hands-free, or transcription mode. Audio is captured from the system default PipeWire source, kept in memory during recording, written to a private temporary WAV file for the Groq request, then deleted. The API key is stored in the private config and passed to curl through a private temporary config file, not in process arguments.

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

When inserting a transcript, Flow uses Ctrl+V in regular text fields and Shift+Insert in detected terminal windows. The terminal shortcut pastes transcript text without triggering Codex CLI's Ctrl+V image-attachment action.

For a desktop smoke test, launch Flow in an X11 session, open a text editor, hold the shortcut while speaking, and verify the pill appears only during capture and that the transcript is pasted into the editor. Test with a real Groq key to verify the network transcription path. Microphone and Groq live checks require a desktop session, microphone, internet access, and API credentials.

### Desktop UI

Open the workspace directly with:

```sh
cargo run --manifest-path src-tauri/Cargo.toml -- --settings
```

The UI lives in `ui/index.html`, `ui/styles.css`, and `ui/app.js`; the recording pill is `ui/overlay.html`. Fonts are bundled in `ui/assets/` so the interface makes no CDN requests. See `ui/assets/NOTICE.md` for sources and `DESIGN.md` for the design references. The interface follows the system light/dark theme and reduced-motion preferences.

For a visual-only preview, open `ui/index.html` in a browser. Saving settings requires the running Tauri app; the browser preview identifies itself and disables saving.

### Releases

Releases are built for Linux x86_64 (`.deb` and `.AppImage`) and Windows x86_64 (`.exe`). In GitHub Actions, open **Tagged release → Run workflow**, enter the exact version in `package.json` (for example `0.2.0`), and run it. The workflow validates every version file, creates the matching `v0.2.0` tag, builds both platforms, generates `SHA256SUMS`, and publishes the GitHub Release. A manually pushed matching `v…` tag also starts the same build.
