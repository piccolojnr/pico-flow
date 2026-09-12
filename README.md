# Flow Linux

Flow Linux is a small background dictation app for Kali/Debian XFCE on X11. Hold a shortcut, speak, release, and Flow sends the recording to Groq before pasting the transcript into the window that was active when recording began. A non-focusable status pill appears above the active app on its monitor only while recording; it stays hidden when idle and during transcription.

## Requirements

- Kali Linux or Debian with XFCE on X11
- Python 3.11 or newer, GTK 4 and GTK 3 introspection, and PortAudio
- `xclip`, `xdotool`, `xrandr`, and `gapplication`
- A microphone and a Groq API key with access to `whisper-large-v3-turbo`

Wayland is not supported: global key handling, tray integration, window activation, and text insertion rely on X11/XFCE behavior.

## Install

Install the native dependencies:

```sh
sudo apt update
sudo apt install -y python3 python3-venv python3-pip python3-gi \
  gir1.2-gtk-4.0 gir1.2-gtk-3.0 python3-numpy libportaudio2 portaudio19-dev \
  python3-pytest xclip xdotool x11-xserver-utils libglib2.0-bin
```

Then run the per-user installer from the project directory:

```sh
./install.sh
```

The installer creates a background launcher, an applications-menu entry, and an XFCE login autostart entry. On first launch, Flow creates `~/.config/flow-linux/config.toml` with private file permissions. If an older `~/.config/flow-linux/.env` or project `.env` exists, Flow migrates those values to TOML once and leaves the old file untouched.

Open **Flow Linux** from the desktop applications menu, choose **Settings** from its tray icon, or run:

```sh
~/.local/bin/flow-linux --settings
```

Enter the Groq API key in Settings. The key is stored in `config.toml`, which is mode `0600` and ignored by Git. Settings also let you select the microphone, language, model, hands-free mode, and login autostart.

## Use

- **Push-to-talk:** focus a text field, hold **Ctrl+Super**, speak, then release either modifier. This keeps the existing push-to-talk behavior.
- **Hands-free:** when enabled in Settings, press **Ctrl+Super+Space** once to start continuous recording, then press the same chord again to stop and transcribe.
- Right-click the tray icon for **Settings** or **Quit Flow Linux**. Clicking the icon opens Settings.

The app starts hidden in the background at login. Turn off **Start Flow automatically when I log in** in Settings to remove the autostart entry. The applications-menu launcher always opens Settings.

Flow does not keep audio or transcript history. Audio is temporary and deleted after each transcription attempt. Logs report timing and audio level but do not include dictated words, API keys, or audio data.

## Configuration file

Settings are stored at `~/.config/flow-linux/config.toml` (or `$XDG_CONFIG_HOME/flow-linux/config.toml`). The default values are:

```toml
[transcription]
groq_api_key = ""
model = "whisper-large-v3-turbo"
language = "en"
api_timeout = 45

[audio]
input_device = ""
minimum_duration = 0.35
silence_threshold = 220

[shortcuts]
push_to_talk = "ctrl+super"
handsfree_enabled = true

[app]
autostart = true
clipboard_restore_delay = 0.6
```

An empty `input_device` uses the system default. To list available devices during troubleshooting:

```sh
python3 -c 'import sounddevice as sd; print(sd.query_devices())'
```

## Development and tests

```sh
python3 -m venv --system-site-packages .venv
. .venv/bin/activate
pip install -r requirements.txt
python -m pytest -q
```

The main application and floating status overlay use GTK 4. A small separate GTK 3 process owns the X11 tray icon so GTK 3 and GTK 4 do not load into the same process. `Gtk.Application` actions let the tray and launcher open Settings or quit the background instance.

## Manual XFCE/X11 check

After installing and entering a Groq key in Settings, open an editor and test both shortcuts. Confirm the overlay remains informational, the original editor keeps focus, text appears at the cursor, and a second dictation works immediately. Also test a short tap, silence, and a temporary network failure; each should show a brief status and leave the background app running. Check that the previous text clipboard contents are restored after a successful paste. These desktop and live-audio checks need a real XFCE/X11 session, microphone, and Groq credentials.

Clipboard restoration covers text and is best-effort; other clipboard target types may not be preserved.
