"""Compact, non-focusable GTK4 status overlay."""

import re
import subprocess
import ctypes

try:
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import Gtk, GLib, Gdk
    GTK_AVAILABLE = True
except Exception:
    GTK_AVAILABLE = False


class _XWMHints(ctypes.Structure):
    """Xlib WM_HINTS layout; InputHint tells the WM this window takes no focus."""

    _fields_ = [
        ("flags", ctypes.c_long),
        ("input", ctypes.c_int),
        ("initial_state", ctypes.c_int),
        ("icon_pixmap", ctypes.c_ulong),
        ("icon_window", ctypes.c_ulong),
        ("icon_x", ctypes.c_int),
        ("icon_y", ctypes.c_int),
        ("icon_mask", ctypes.c_ulong),
        ("window_group", ctypes.c_ulong),
    ]


class _XClientMessageEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("message_type", ctypes.c_ulong),
        ("format", ctypes.c_int),
        ("data", ctypes.c_long * 5),
    ]


class _XEvent(ctypes.Union):
    _fields_ = [("xclient", _XClientMessageEvent), ("padding", ctypes.c_long * 24)]


def _set_x11_no_input_hint(window):
    """Keep GTK's top-level overlay from taking focus when it is presented."""
    try:
        gi.require_version("GdkX11", "4.0")
        from gi.repository import GdkX11

        surface = window.get_surface()
        xid = GdkX11.X11Surface.get_xid(surface)
        x11 = ctypes.CDLL("libX11.so.6")
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XSetWMHints.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(_XWMHints)]
        x11.XSetWMHints.restype = ctypes.c_int
        x11.XFlush.argtypes = [ctypes.c_void_p]
        x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        display = x11.XOpenDisplay(None)
        if not display:
            return
        hints = _XWMHints()
        hints.flags = 1  # InputHint
        hints.input = 0
        hints.initial_state = 1  # NormalState
        x11.XSetWMHints(display, xid, ctypes.byref(hints))
        x11.XFlush(display)
        x11.XCloseDisplay(display)
    except Exception:
        # GdkX11/libX11 are part of the target X11 environment; retain GTK's
        # non-focusable widget setting as a fallback on unusual builds.
        pass


def _set_x11_window_states(window_id, states):
    """Request EWMH states such as above and skip-taskbar from the WM."""
    display = None
    try:
        x11 = ctypes.CDLL("libX11.so.6")
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        x11.XDefaultRootWindow.restype = ctypes.c_ulong
        x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        x11.XInternAtom.restype = ctypes.c_ulong
        x11.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int,
                                   ctypes.c_long, ctypes.POINTER(_XEvent)]
        x11.XSendEvent.restype = ctypes.c_int
        x11.XFlush.argtypes = [ctypes.c_void_p]
        x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        display = x11.XOpenDisplay(None)
        if not display:
            return
        root = x11.XDefaultRootWindow(display)
        state_atom = x11.XInternAtom(display, b"_NET_WM_STATE", 0)
        atoms = [x11.XInternAtom(display, name.encode(), 0) for name in states]
        for offset in range(0, len(atoms), 2):
            event = _XEvent()
            message = event.xclient
            message.type = 33  # ClientMessage
            message.send_event = 1
            message.display = display
            message.window = int(window_id)
            message.message_type = state_atom
            message.format = 32
            message.data[0] = 1  # _NET_WM_STATE_ADD
            message.data[1] = atoms[offset]
            message.data[2] = atoms[offset + 1] if offset + 1 < len(atoms) else 0
            message.data[3] = 1  # Normal application request
            x11.XSendEvent(display, root, 0, (1 << 20) | (1 << 19), ctypes.byref(event))
        x11.XFlush(display)
    except Exception:
        pass
    finally:
        if display:
            try:
                x11.XCloseDisplay(display)
            except Exception:
                pass


class Overlay:
    def __init__(self):
        self.available = GTK_AVAILABLE
        self.app = None
        self.window = None
        self.label = None
        self._return_focus_window = None

    def create_window(self, app):
        if not self.available:
            return
        self.app = app
        self._activate(app)

    def _activate(self, app):
        self.window = Gtk.Window()
        app.add_window(self.window)
        self.window.set_title("Flow Linux")
        self.window.set_default_size(300, 58)
        self.window.set_decorated(False)
        self.window.set_resizable(False)
        self.window.set_focusable(False)
        self.window.add_css_class("flow-overlay")

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=11)
        box.set_margin_top(13)
        box.set_margin_bottom(13)
        box.set_margin_start(19)
        box.set_margin_end(19)
        self.label = Gtk.Label(label="●  Listening")
        self.label.add_css_class("flow-label")
        box.append(self.label)
        self.window.set_child(box)

        css = Gtk.CssProvider()
        css.load_from_data(b"""
          .flow-overlay { background: #171a22; border: 1px solid #3a4050;
                          border-radius: 18px; }
          .flow-label { color: #f5f6fa; font: 500 14px sans-serif; }
        """)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )
        self.window.realize()
        self.window.hide()

    def show(self, text):
        if not self.available or self.window is None:
            print(f"[Flow] {text}")
            return
        GLib.idle_add(self._show, text)

    def _show(self, text):
        self.label.set_text(text)
        if not self.window.get_visible():
            self._return_focus_window = self._active_window_id()
        # Mapping a top-level with set_visible avoids Gtk.Window.present()'s
        # activation request, which can steal focus even when it is not
        # focusable. Apply WM_HINTS after mapping in case GTK rewrites them.
        self.window.set_visible(True)
        _set_x11_no_input_hint(self.window)
        self._restore_original_focus()
        # XFCE can apply its focus-stealing policy asynchronously after the
        # map event, so assert focus again after the WM has processed it.
        GLib.timeout_add(220, self._restore_original_focus)
        GLib.timeout_add(120, self._place_bottom_center)
        return False

    @staticmethod
    def _active_window_id():
        try:
            result = subprocess.run(
                ["xdotool", "getactivewindow"], check=True,
                capture_output=True, text=True, timeout=0.5,
            )
            return result.stdout.strip()
        except (FileNotFoundError, subprocess.SubprocessError):
            return None

    def _restore_original_focus(self):
        target = self._return_focus_window
        if not target:
            return GLib.SOURCE_REMOVE
        try:
            subprocess.run(
                ["xdotool", "windowactivate", "--sync", target],
                check=True, timeout=1.0,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
        except (FileNotFoundError, subprocess.SubprocessError):
            pass
        return GLib.SOURCE_REMOVE

    def _place_bottom_center(self):
        try:
            result = subprocess.run(
                ["xdotool", "search", "--onlyvisible", "--name", "^Flow Linux$"],
                check=True, capture_output=True, text=True, timeout=1.0,
            )
            window_id = result.stdout.strip().splitlines()[-1]
            monitor = self._active_monitor()
            if monitor:
                x, y, width, height = monitor
                _set_x11_window_states(window_id, [
                    "_NET_WM_STATE_ABOVE", "_NET_WM_STATE_SKIP_TASKBAR",
                    "_NET_WM_STATE_SKIP_PAGER",
                ])
                subprocess.run(
                    ["xdotool", "windowraise", window_id], check=False, timeout=1.0
                )
                subprocess.run(
                    ["xdotool", "windowmove", window_id, str(x + (width - 300) // 2), str(y + height - 95)],
                    check=False, timeout=1.0,
                )
        except (FileNotFoundError, subprocess.SubprocessError, IndexError):
            pass
        return GLib.SOURCE_REMOVE

    @staticmethod
    def _active_monitor():
        try:
            monitors = subprocess.run(
                ["xrandr", "--listactivemonitors"], check=True, capture_output=True,
                text=True, timeout=1.0,
            ).stdout.splitlines()[1:]
            pointer = subprocess.run(
                ["xdotool", "getmouselocation", "--shell"], check=True,
                capture_output=True, text=True, timeout=1.0,
            ).stdout
            values = dict(line.split("=", 1) for line in pointer.splitlines() if "=" in line)
            px, py = int(values["X"]), int(values["Y"])
            parsed = []
            for line in monitors:
                match = re.search(r"(\d+)/\d+x(\d+)/\d+([+-]\d+)([+-]\d+)", line)
                if match:
                    width, height, x, y = map(int, match.groups())
                    item = (x, y, width, height)
                    parsed.append(("*" in line, item))
                    if x <= px < x + width and y <= py < y + height:
                        return item
            return next((item for primary, item in parsed if primary), parsed[0][1] if parsed else None)
        except (FileNotFoundError, subprocess.SubprocessError, KeyError, ValueError, IndexError):
            return None

    def hide(self):
        if self.available and self.window is not None:
            GLib.idle_add(self._hide)

    def _hide(self):
        self.window.hide()
        self._return_focus_window = None
        return GLib.SOURCE_REMOVE
