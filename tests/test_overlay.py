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
