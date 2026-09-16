#!/usr/bin/env bash
set -euo pipefail

# Release-first installer. Use --source when developing from a checkout.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="${FLOW_REPO:-piccolojnr/pico-flow}"
BIN_DIR="${HOME}/.local/bin"
OPT_DIR="${HOME}/.local/opt/flow-linux"
DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/flow-linux"
CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
DESKTOP_DIR="${HOME}/.local/share/applications"
SOURCE_INSTALL=false
INSTALL_FORMAT="deb"
VERSION=""

usage() {
  cat <<'EOF'
Usage: ./install.sh [--source] [--appimage] [--version VERSION]

  (default) install the latest Debian package from GitHub Releases
  --appimage           install the portable AppImage instead
  --version VERSION  install a specific release, such as 0.2.0
  --source            build and install the current checkout with Cargo
EOF
}

while (($#)); do
  case "$1" in
    --source) SOURCE_INSTALL=true ;;
    --appimage) INSTALL_FORMAT="appimage" ;;
    --version) shift; VERSION="${1:-}" ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

mkdir -p "${BIN_DIR}" "${OPT_DIR}" "${DATA_DIR}" "${DESKTOP_DIR}"

if [[ "${SOURCE_INSTALL}" == true ]]; then
  cargo build --release --locked --manifest-path "${ROOT}/src-tauri/Cargo.toml"
  install -m 0755 "${ROOT}/src-tauri/target/release/flow-linux" "${BIN_DIR}/flow-linux.new"
  mv -f "${BIN_DIR}/flow-linux.new" "${BIN_DIR}/flow-linux"
else
  command -v curl >/dev/null || { echo 'curl is required to install a release.' >&2; exit 1; }
  [[ -z "${VERSION}" || "${VERSION}" == v* ]] || VERSION="v${VERSION}"
  if [[ -n "${VERSION}" ]]; then
    RELEASE_URL="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
  else
    RELEASE_URL="https://api.github.com/repos/${REPO}/releases/latest"
  fi
  metadata="$(curl --fail --silent --show-error --location --header 'Accept: application/vnd.github+json' "${RELEASE_URL}")"
  if [[ "${INSTALL_FORMAT}" == "deb" ]]; then
    asset_url="$(printf '%s\n' "${metadata}" | sed -nE 's/.*"browser_download_url": "([^"]*x86_64-unknown-linux-gnu\.deb)".*/\1/p' | head -n 1)"
    [[ -n "${asset_url}" ]] || { echo "No x86_64 Linux Debian package was found in ${RELEASE_URL}." >&2; echo 'Use --appimage or --source instead.' >&2; exit 1; }
  else
    asset_url="$(printf '%s\n' "${metadata}" | sed -nE 's/.*"browser_download_url": "([^"]*x86_64-unknown-linux-gnu\.AppImage)".*/\1/p' | head -n 1)"
    [[ -n "${asset_url}" ]] || { echo "No x86_64 Linux AppImage was found in ${RELEASE_URL}." >&2; echo 'Use --source to build the current checkout instead.' >&2; exit 1; }
  fi
  sums_url="$(printf '%s\n' "${metadata}" | sed -nE 's/.*"browser_download_url": "([^"]*SHA256SUMS)".*/\1/p' | head -n 1)"
  [[ -n "${sums_url}" ]] || { echo "No SHA256SUMS file was found in ${RELEASE_URL}." >&2; exit 1; }
  tmp="$(mktemp "${TMPDIR:-/tmp}/flow-linux.XXXXXX")"
  sums_tmp="$(mktemp "${TMPDIR:-/tmp}/flow-linux-sums.XXXXXX")"
  trap 'rm -f "${tmp}" "${sums_tmp}"' EXIT
  curl --fail --silent --show-error --location "${asset_url}" -o "${tmp}"
  curl --fail --silent --show-error --location "${sums_url}" -o "${sums_tmp}"
  asset_name="${asset_url##*/}"
  checksum="$(awk -v name="${asset_name}" '$2 == name {print $1; exit}' "${sums_tmp}")"
  [[ -n "${checksum}" ]] || { echo "No checksum was published for ${asset_name}." >&2; exit 1; }
  printf '%s  %s\n' "${checksum}" "${tmp}" | sha256sum --check --status - || { echo 'Release checksum verification failed.' >&2; exit 1; }
  if [[ "${INSTALL_FORMAT}" == "deb" ]]; then
    command -v apt-get >/dev/null || { echo 'apt-get is required for the default Debian install. Use --appimage on another distribution.' >&2; exit 1; }
    if (( EUID == 0 )); then
      apt-get install --yes "${tmp}"
    else
      command -v sudo >/dev/null || { echo 'sudo is required to install the Debian package. Use --appimage or run as root.' >&2; exit 1; }
      sudo apt-get install --yes "${tmp}"
    fi
    ln -sfn /usr/bin/flow-linux "${BIN_DIR}/flow-linux"
  else
    install -m 0755 "${tmp}" "${OPT_DIR}/flow-linux.AppImage.new"
    mv -f "${OPT_DIR}/flow-linux.AppImage.new" "${OPT_DIR}/flow-linux.AppImage"
    cat > "${BIN_DIR}/flow-linux.new" <<EOF
#!/usr/bin/env bash
exec env APPIMAGE_EXTRACT_AND_RUN=1 "${OPT_DIR}/flow-linux.AppImage" "\$@"
EOF
    chmod 0755 "${BIN_DIR}/flow-linux.new"
    mv -f "${BIN_DIR}/flow-linux.new" "${BIN_DIR}/flow-linux"
  fi
fi

cat > "${DESKTOP_DIR}/flow-linux.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Flow Linux
Comment=Private voice dictation
Exec=${BIN_DIR}/flow-linux --settings
Icon=audio-input-microphone
Terminal=false
Categories=Utility;AudioVideo;
StartupWMClass=flow-linux
DESKTOP

# Retire only the old Python runtime. Config, credentials, and history remain intact.
rm -rf "${DATA_DIR}/.venv" "${DATA_DIR}/flow" "${DATA_DIR}/app.py" "${DATA_DIR}/requirements.txt"
chmod 700 "${DATA_DIR}"
[[ -x "${BIN_DIR}/flow-linux" ]] || { echo 'Flow install failed.' >&2; exit 1; }

printf 'Installed Flow Linux at %s\n' "${BIN_DIR}/flow-linux"
printf 'Settings and history remain under %s and %s\n' "${CONFIG_HOME}/flow-linux" "${DATA_DIR}"
missing=()
for tool in pw-record curl xclip xdotool xrandr sqlite3; do command -v "${tool}" >/dev/null || missing+=("${tool}"); done
if ((${#missing[@]})); then printf 'Install these runtime tools to enable all features: %s\n' "${missing[*]}"; fi
