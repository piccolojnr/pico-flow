#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/flow-linux"
CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
mkdir -p "${BIN_DIR}" "${DATA_DIR}" "${HOME}/.local/share/applications"

cargo build --release --locked --manifest-path "${ROOT}/src-tauri/Cargo.toml"
install -m 0755 "${ROOT}/src-tauri/target/release/flow-linux" "${BIN_DIR}/flow-linux.new"
mv -f "${BIN_DIR}/flow-linux.new" "${BIN_DIR}/flow-linux"

cat > "${HOME}/.local/share/applications/flow-linux.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Flow Linux
Comment=Private voice dictation
Exec=${BIN_DIR}/flow-linux --settings
Icon=audio-input-microphone
Terminal=false
Categories=Utility;AudioVideo;
DESKTOP

# Retire the former Python runtime and launcher artifacts while keeping config,
# credentials, and the user's SQLite history database intact.
rm -rf "${DATA_DIR}/.venv" "${DATA_DIR}/flow" "${DATA_DIR}/app.py" "${DATA_DIR}/requirements.txt"
chmod 700 "${DATA_DIR}"
if [[ ! -x "${BIN_DIR}/flow-linux" ]]; then echo "Flow install failed" >&2; exit 1; fi

printf 'Installed Flow Linux (Tauri) at %s\n' "${BIN_DIR}/flow-linux"
printf 'Your config and history database were preserved under %s and %s\n' "${CONFIG_HOME}/flow-linux" "${DATA_DIR}"
missing=()
for tool in pw-record curl xclip xdotool sqlite3; do command -v "${tool}" >/dev/null || missing+=("${tool}"); done
if ((${#missing[@]})); then printf 'Install these runtime tools to enable all features: %s\n' "${missing[*]}"; fi
