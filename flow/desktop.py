"""Per-user desktop autostart integration."""

import os
from pathlib import Path


def set_autostart(enabled, config_home=None, launcher=None):
    config_home = Path(config_home or os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    entry = config_home / "autostart" / "flow-linux.desktop"
    if not enabled:
        try:
            entry.unlink()
        except FileNotFoundError:
            pass
        return entry

    launcher = Path(launcher or (Path.home() / ".local" / "bin" / "flow-linux"))
    escaped = str(launcher).replace("\\", "\\\\").replace('"', '\\"')
    entry.parent.mkdir(parents=True, exist_ok=True)
    entry.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Flow Linux\n"
        "Comment=Background push-to-talk dictation\n"
        f'Exec="{escaped}" --background\n'
        "Terminal=false\n"
        "X-GNOME-Autostart-enabled=true\n",
        encoding="utf-8",
    )
    entry.chmod(0o644)
    return entry
