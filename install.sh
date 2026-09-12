#!/bin/sh
set -eu

SOURCE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_HOME=${XDG_CONFIG_HOME:-"$HOME/.config"}
APP_DIR="$DATA_HOME/flow-linux"
CONFIG_DIR="$CONFIG_HOME/flow-linux"
BIN_DIR=${XDG_BIN_HOME:-"$HOME/.local/bin"}
APPLICATIONS_DIR="$DATA_HOME/applications"

for command in python3 xclip xdotool xrandr gapplication; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Missing required command: $command" >&2
        echo "Install the Kali/Debian packages listed in README.md, then retry." >&2
        exit 1
    fi
done

mkdir -p "$APP_DIR" "$CONFIG_DIR" "$BIN_DIR" "$CONFIG_HOME/autostart" "$APPLICATIONS_DIR"
cp "$SOURCE_DIR/app.py" "$APP_DIR/app.py"
rm -rf "$APP_DIR/flow"
cp -R "$SOURCE_DIR/flow" "$APP_DIR/flow"
cp "$SOURCE_DIR/requirements.txt" "$APP_DIR/requirements.txt"

if [ ! -x "$APP_DIR/.venv/bin/python" ]; then
    python3 -m venv --system-site-packages "$APP_DIR/.venv"
fi
"$APP_DIR/.venv/bin/python" -m pip install -r "$APP_DIR/requirements.txt"

cat > "$BIN_DIR/flow-linux" <<EOF
#!/bin/sh
cd "$CONFIG_DIR"
exec "$APP_DIR/.venv/bin/python" "$APP_DIR/app.py" "\$@"
EOF
chmod 755 "$BIN_DIR/flow-linux"

cat > "$APPLICATIONS_DIR/flow-linux.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Flow Linux
Comment=Push-to-talk dictation settings
Exec="$BIN_DIR/flow-linux" --settings
Icon=audio-input-microphone
Terminal=false
StartupNotify=false
Categories=Utility;Accessibility;
EOF
chmod 644 "$APPLICATIONS_DIR/flow-linux.desktop"

cat > "$CONFIG_HOME/autostart/flow-linux.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Flow Linux
Comment=Background push-to-talk dictation
Exec="$BIN_DIR/flow-linux" --background
Terminal=false
X-GNOME-Autostart-enabled=true
EOF
chmod 644 "$CONFIG_HOME/autostart/flow-linux.desktop"

echo "Installed. Legacy .env settings migrate to $CONFIG_DIR/config.toml if no TOML file exists."
echo "Log out/in or run $BIN_DIR/flow-linux to start Flow now."
