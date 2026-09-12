"""Small GTK4 settings window for Flow Linux."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk

from .config import Config


class SettingsWindow:
    def __init__(self, application, config, on_save):
        self.config = config
        self.on_save = on_save
        self.window = Gtk.ApplicationWindow(application=application)
        self.window.set_title("Flow Linux Settings")
        self.window.set_default_size(480, 430)
        self.window.set_resizable(False)
        self.window.connect("close-request", self._close_request)

        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        outer.set_margin_top(22)
        outer.set_margin_bottom(20)
        outer.set_margin_start(24)
        outer.set_margin_end(24)
        self.window.set_child(outer)

        title = Gtk.Label(label="Flow Linux Settings")
        title.set_xalign(0)
        title.add_css_class("title-2")
        outer.append(title)
        intro = Gtk.Label(label="Choose how Flow listens and transcribes.")
        intro.set_xalign(0)
        intro.add_css_class("dim-label")
        outer.append(intro)

        grid = Gtk.Grid(column_spacing=14, row_spacing=12)
        grid.set_hexpand(True)
        outer.append(grid)

        self.api_key = self._entry(config.groq_api_key)
        self.api_key.set_visibility(False)
        self.api_key.set_placeholder_text("Groq API key")
        self.model = self._entry(config.model)
        self.language = self._entry(config.language)
        self.device = self._entry(config.input_device)
        self.device.set_placeholder_text("System default (or device number/name)")

        self._row(grid, 0, "Groq API key", self.api_key)
        self._row(grid, 1, "Model", self.model)
        self._row(grid, 2, "Language", self.language)
        self._row(grid, 3, "Microphone", self.device)

        self.handsfree = Gtk.CheckButton(label="Enable Ctrl+Super+Space hands-free mode")
        self.handsfree.set_active(config.handsfree_enabled)
        outer.append(self.handsfree)
        self.autostart = Gtk.CheckButton(label="Start Flow automatically when I log in")
        self.autostart.set_active(config.autostart_enabled)
        outer.append(self.autostart)

        shortcut_note = Gtk.Label(
            label="Hold Ctrl+Super to dictate. Press Ctrl+Super+Space to start, "
                  "then press it again to stop and transcribe."
        )
        shortcut_note.set_xalign(0)
        shortcut_note.set_wrap(True)
        shortcut_note.add_css_class("dim-label")
        outer.append(shortcut_note)

        self.error = Gtk.Label()
        self.error.set_xalign(0)
        self.error.add_css_class("error")
        outer.append(self.error)

        buttons = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        buttons.set_halign(Gtk.Align.END)
        outer.append(buttons)
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", lambda *_: self.window.set_visible(False))
        buttons.append(cancel)
        save = Gtk.Button(label="Save")
        save.add_css_class("suggested-action")
        save.connect("clicked", self._save)
        buttons.append(save)

    @staticmethod
    def _entry(value):
        entry = Gtk.Entry()
        entry.set_hexpand(True)
        entry.set_text(value)
        return entry

    @staticmethod
    def _row(grid, row, label_text, widget):
        label = Gtk.Label(label=label_text)
        label.set_xalign(0)
        label.set_valign(Gtk.Align.CENTER)
        grid.attach(label, 0, row, 1, 1)
        grid.attach(widget, 1, row, 1, 1)

    def _save(self, *_):
        try:
            config = Config(
                groq_api_key=self.api_key.get_text().strip(),
                input_device=self.device.get_text().strip(),
                model=self.model.get_text().strip() or "whisper-large-v3-turbo",
                language=self.language.get_text().strip() or "en",
                shortcut=self.config.shortcut,
                minimum_duration=self.config.minimum_duration,
                silence_threshold=self.config.silence_threshold,
                api_timeout=self.config.api_timeout,
                clipboard_restore_delay=self.config.clipboard_restore_delay,
                handsfree_enabled=self.handsfree.get_active(),
                autostart_enabled=self.autostart.get_active(),
            )
            if not config.language:
                raise ValueError("Language cannot be empty")
            self.on_save(config)
            self.config = config
            self.error.set_text("")
            self.window.set_visible(False)
        except (OSError, ValueError) as exc:
            self.error.set_text(str(exc))

    def present(self):
        self.api_key.set_text(self.config.groq_api_key)
        self.model.set_text(self.config.model)
        self.language.set_text(self.config.language)
        self.device.set_text(self.config.input_device)
        self.handsfree.set_active(self.config.handsfree_enabled)
        self.autostart.set_active(self.config.autostart_enabled)
        self.window.present()

    def _close_request(self, window):
        window.set_visible(False)
        return True
