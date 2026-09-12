"""X11 clipboard-based Unicode paste with best-effort clipboard restoration."""

import subprocess
import time


class InsertionError(RuntimeError):
    pass


def _run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def active_window():
    try:
        result = _run(["xdotool", "getactivewindow"], capture_output=True, text=True, timeout=2)
        return result.stdout.strip()
    except (FileNotFoundError, subprocess.SubprocessError):
        return None


def _read_clipboard():
    try:
        result = _run(
            ["xclip", "-selection", "clipboard", "-o"],
            capture_output=True,
            timeout=1.0,
        )
        return result.stdout
    except (FileNotFoundError, subprocess.SubprocessError):
        return None


def _write_clipboard(data):
    try:
        _run(["xclip", "-selection", "clipboard", "-i"], input=data, timeout=2.0)
    except (FileNotFoundError, subprocess.SubprocessError) as exc:
        raise InsertionError("Clipboard unavailable; install xclip") from exc


def insert_text(text, target_window=None, restore_delay=0.6, sleep=time.sleep):
    if not text:
        raise InsertionError("No text to insert")
    previous = _read_clipboard()
    encoded = text.encode("utf-8")
    _write_clipboard(encoded)
    try:
        if target_window:
            try:
                _run(["xdotool", "windowactivate", "--sync", target_window], timeout=2.0)
            except (FileNotFoundError, subprocess.SubprocessError) as exc:
                raise InsertionError("Could not return focus to the original window") from exc
        try:
            _run(["xdotool", "key", "--clearmodifiers", "ctrl+v"], timeout=2.0)
        except (FileNotFoundError, subprocess.SubprocessError) as exc:
            raise InsertionError("Text insertion failed; install xdotool") from exc
    finally:
        if previous is not None:
            sleep(restore_delay)
            try:
                _write_clipboard(previous)
            except InsertionError:
                # The paste has already succeeded; restoration is best-effort.
                pass
