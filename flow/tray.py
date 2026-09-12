"""GTK3 XEmbed tray icon kept in a helper process separate from GTK4."""

import subprocess

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk


APP_ID = "dev.flowlinux.Dictation"


class Tray:
    def __init__(self):
        self.icon = Gtk.StatusIcon.new_from_icon_name("audio-input-microphone")
        self.icon.set_title("Flow Linux")
        self.icon.set_tooltip_text("Flow Linux dictation")
        self.icon.set_visible(True)
        self.icon.connect("activate", self._open_settings)
        self.icon.connect("popup-menu", self._popup)
        self.menu = Gtk.Menu()
        settings = Gtk.MenuItem(label="Settings")
        settings.connect("activate", self._open_settings)
        self.menu.append(settings)
        self.menu.append(Gtk.SeparatorMenuItem())
        quit_item = Gtk.MenuItem(label="Quit Flow Linux")
        quit_item.connect("activate", self._quit)
        self.menu.append(quit_item)
        self.menu.show_all()

    @staticmethod
    def _activate_action(action):
        try:
            subprocess.Popen(
                ["gapplication", "action", APP_ID, action],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL, start_new_session=True,
            )
        except OSError as exc:
            print(f"[Flow] Could not contact the background app: {exc}")

    def _open_settings(self, *_args):
        self._activate_action("settings")

    def _quit(self, *_args):
        self._activate_action("quit")
        Gtk.main_quit()

    def _popup(self, _icon, _button, _activate_time):
        self.menu.popup_at_pointer(None)


def main():
    tray = Tray()
    Gtk.main()
    return tray


if __name__ == "__main__":
    main()
