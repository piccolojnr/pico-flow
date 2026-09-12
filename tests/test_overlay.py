from types import SimpleNamespace

from flow import overlay


def test_overlay_uses_focused_window_monitor_before_pointer_monitor(monkeypatch):
    calls = []

    def run(command, **_kwargs):
        calls.append(command)
        if command[:2] == ["xrandr", "--listactivemonitors"]:
            return SimpleNamespace(stdout=(
                "Monitors: 2\n"
                " 0: +*eDP-1 1920/300x1080/170+0+0\n"
                " 1: +HDMI-1 1920/520x1080/290+1920+0\n"
            ))
        if command[:2] == ["xdotool", "getwindowgeometry"]:
            return SimpleNamespace(stdout="X=2300\nY=100\nWIDTH=800\nHEIGHT=600\n")
        raise AssertionError(f"unexpected command: {command}")

    monkeypatch.setattr(overlay.subprocess, "run", run)

    assert overlay.Overlay._active_monitor("window-42") == (1920, 0, 1920, 1080)
    assert len(calls) == 2


def test_overlay_moves_its_xid_to_the_bottom_center(monkeypatch):
    instance = overlay.Overlay.__new__(overlay.Overlay)
    instance.window_id = 12345
    instance._return_focus_window = "window-42"
    commands = []

    monkeypatch.setattr(
        overlay.Overlay, "_active_monitor",
        staticmethod(lambda _window_id: (0, 0, 1920, 1080)),
    )
    monkeypatch.setattr(overlay, "_set_x11_window_states", lambda *_args: None)

    def run(command, **_kwargs):
        commands.append(command)
        return SimpleNamespace(stdout="")

    monkeypatch.setattr(overlay.subprocess, "run", run)

    instance._place_bottom_center()

    assert commands == [
        ["xdotool", "windowraise", "12345"],
        ["xdotool", "windowmove", "12345", "810", "985"],
    ]
