"""Compact GTK4 browser for locally saved dictations."""

from datetime import datetime

import gi

gi.require_version("Gdk", "4.0")
gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, GLib, Gtk


class HistoryWindow:
    def __init__(self, application, history_store, history_enabled):
        self.application = application
        self.history_store = history_store
        self.history_enabled = history_enabled
        self.clear_dialog = None
        self.window = Gtk.ApplicationWindow(application=application)
        self.window.set_title("Flow Linux History")
        self.window.set_default_size(620, 560)
        self.window.connect("close-request", self._close_request)

        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        outer.set_margin_top(18)
        outer.set_margin_bottom(16)
        outer.set_margin_start(18)
        outer.set_margin_end(18)
        self.window.set_child(outer)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        outer.append(header)
        title = Gtk.Label(label="Dictation History")
        title.set_xalign(0)
        title.set_hexpand(True)
        title.add_css_class("title-2")
        header.append(title)
        self.clear_button = Gtk.Button(label="Clear history")
        self.clear_button.connect("clicked", self._confirm_clear)
        header.append(self.clear_button)

        self.privacy_note = Gtk.Label()
        self.privacy_note.set_xalign(0)
        self.privacy_note.set_wrap(True)
        self.privacy_note.add_css_class("dim-label")
        outer.append(self.privacy_note)

        self.search = Gtk.SearchEntry()
        self.search.set_placeholder_text("Search saved dictations")
        self.search.connect("search-changed", lambda *_: self.refresh())
        outer.append(self.search)

        self.error = Gtk.Label()
        self.error.set_xalign(0)
        self.error.set_wrap(True)
        self.error.add_css_class("error")
        outer.append(self.error)

        self.list_box = Gtk.ListBox()
        self.list_box.set_selection_mode(Gtk.SelectionMode.NONE)
        self.list_box.set_show_separators(True)
        scroller = Gtk.ScrolledWindow()
        scroller.set_vexpand(True)
        scroller.set_child(self.list_box)
        outer.append(scroller)

    def present(self):
        self.privacy_note.set_text(
            "History saving is off. New dictations will not be saved. Enable "
            "Save transcription history in Settings."
            if not self.history_enabled()
            else "Stored only on this device. Audio is never saved."
        )
        self.refresh()
        self.window.present()

    @staticmethod
    def _timestamp(value):
        try:
            return datetime.fromisoformat(value).astimezone().strftime("%Y-%m-%d %H:%M:%S")
        except (TypeError, ValueError):
            return value

    @staticmethod
    def _status_text(status):
        return {
            "inserted": "Inserted",
            "failed": "Paste failed · saved here",
            "pending": "Insertion status unknown",
        }.get(status, status)

    def refresh(self):
        while child := self.list_box.get_first_child():
            self.list_box.remove(child)
        try:
            entries = self.history_store.list_entries(search=self.search.get_text())
        except Exception as exc:
            self.error.set_text(f"Could not read local History: {exc}")
            return
        self.error.set_text("")
        if not entries:
            empty = Gtk.Label(
                label="No saved dictations match this search." if self.search.get_text()
                else "No saved dictations yet."
            )
            empty.set_margin_top(28)
            empty.add_css_class("dim-label")
            self.list_box.append(empty)
            return
        for entry in entries:
            self.list_box.append(self._entry_row(entry))

    def _entry_row(self, entry):
        row = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        row.set_margin_top(10)
        row.set_margin_bottom(10)
        row.set_margin_start(10)
        row.set_margin_end(10)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        row.append(header)
        timestamp = Gtk.Label(label=self._timestamp(entry.created_at))
        timestamp.set_xalign(0)
        timestamp.set_hexpand(True)
        timestamp.add_css_class("heading")
        header.append(timestamp)
        status = Gtk.Label(label=self._status_text(entry.insertion_status))
        status.add_css_class("dim-label")
        header.append(status)

        preview_text = " ".join(entry.transcript.split())
        if len(preview_text) > 240:
            preview_text = preview_text[:237].rstrip() + "…"
        preview = Gtk.Label(label=preview_text)
        preview.set_xalign(0)
        preview.set_wrap(True)
        preview.set_selectable(True)
        row.append(preview)

        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        actions.set_halign(Gtk.Align.END)
        row.append(actions)
        copy = Gtk.Button(label="Copy")
        copy.connect("clicked", self._copy, entry.transcript)
        actions.append(copy)
        delete = Gtk.Button(label="Delete")
        delete.connect("clicked", self._delete, entry.id)
        actions.append(delete)
        return row

    def _copy(self, button, transcript):
        display = Gdk.Display.get_default()
        if display is None:
            self.error.set_text("Clipboard is not available.")
            return
        clipboard = display.get_clipboard()
        provider = Gdk.ContentProvider.new_for_value(transcript)
        if not clipboard.set_content(provider):
            self.error.set_text("Could not copy this transcript to the clipboard.")
            return
        button.set_label("Copied")
        GLib.timeout_add(1000, lambda: (button.set_label("Copy"), GLib.SOURCE_REMOVE)[1])

    def _delete(self, _button, entry_id):
        try:
            self.history_store.delete(entry_id)
            self.refresh()
        except Exception as exc:
            self.error.set_text(f"Could not delete this dictation: {exc}")

    def _confirm_clear(self, *_args):
        dialog = Gtk.Window(application=self.application)
        dialog.set_title("Clear History?")
        dialog.set_transient_for(self.window)
        dialog.set_modal(True)
        dialog.set_resizable(False)
        self.clear_dialog = dialog
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        box.set_margin_top(20)
        box.set_margin_bottom(20)
        box.set_margin_start(22)
        box.set_margin_end(22)
        dialog.set_child(box)
        message = Gtk.Label(label="Permanently delete all saved dictations?")
        message.set_wrap(True)
        box.append(message)
        buttons = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        buttons.set_halign(Gtk.Align.END)
        box.append(buttons)
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", self._cancel_clear, dialog)
        buttons.append(cancel)
        clear = Gtk.Button(label="Delete all")
        clear.add_css_class("destructive-action")
        clear.connect("clicked", self._clear, dialog)
        buttons.append(clear)
        dialog.present()

    def _cancel_clear(self, _button, dialog):
        dialog.close()
        self.clear_dialog = None

    def _clear(self, _button, dialog):
        try:
            self.history_store.clear()
            dialog.close()
            self.clear_dialog = None
            self.refresh()
        except Exception as exc:
            self.error.set_text(f"Could not clear History: {exc}")

    def _close_request(self, _window):
        self.window.set_visible(False)
        return True
