import subprocess

import pytest

from flow import clipboard
from flow.clipboard import InsertionError


def test_unicode_insert_targets_original_window_clears_modifiers_and_restores_clipboard(monkeypatch):
    calls = []
    writes = []
    monkeypatch.setattr(clipboard, "_read_clipboard", lambda: b"previous clipboard")
    monkeypatch.setattr(clipboard, "_write_clipboard", writes.append)
    monkeypatch.setattr(clipboard, "_run", lambda command, **kwargs: calls.append(command))
    sleeps = []

    clipboard.insert_text("héllo 世界", "123", 0.25, sleeps.append)

    assert writes == ["héllo 世界".encode(), b"previous clipboard"]
    assert calls == [
        ["xdotool", "windowactivate", "--sync", "123"],
        ["xdotool", "key", "--clearmodifiers", "ctrl+v"],
    ]
    assert sleeps == [0.25]


def test_restores_clipboard_when_paste_fails(monkeypatch):
    writes = []
    monkeypatch.setattr(clipboard, "_read_clipboard", lambda: b"old")
    monkeypatch.setattr(clipboard, "_write_clipboard", writes.append)

    def fail(command, **kwargs):
        raise subprocess.CalledProcessError(1, command)

    monkeypatch.setattr(clipboard, "_run", fail)
    with pytest.raises(InsertionError, match="original window"):
        clipboard.insert_text("text", "123", sleep=lambda _: None)
    assert writes == [b"text", b"old"]
