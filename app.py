#!/usr/bin/env python3
"""Single-instance background application entry point."""

from pathlib import Path
import os
import subprocess
import sys
import threading

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gio, GLib, Gtk

from flow.config import Config, ConfigStore
from flow.controller import DictationController
from flow.desktop import set_autostart
from flow.history import HistoryStore
from flow.history_window import HistoryWindow
from flow.hotkeys import PynputHotkeyListener
from flow.overlay import Overlay
from flow.settings import SettingsWindow


APP_ID = "dev.flowlinux.Dictation"


class FlowApplication:
    def __init__(self, argv=None):
        self.argv = list(sys.argv[1:] if argv is None else argv)
        self.open_settings_on_start = "--settings" in self.argv
        self.application = Gtk.Application(application_id=APP_ID)
        self.store = ConfigStore()
        self.config_error = ""
        try:
            self.config = self.store.load()
        except ValueError as exc:
            self.config = Config()
            self.config_error = str(exc)
        self.overlay = Overlay()
        self.history_store = HistoryStore()
        self.controller = DictationController(
            self.config,
            self.overlay,
            history_store=self.history_store,
            recovery_notifier=self._notify_insertion_recovery,
        )
        self.listener = PynputHotkeyListener(
            self.config.shortcut,
            self.controller.start,
            self.controller.stop,
            self.controller.toggle_handsfree,
            self.config.handsfree_enabled,
            self.controller.is_handsfree_active,
        )
        self.settings_window = None
        self.history_window = None
        self.tray_process = None

        self._add_action("settings", self.open_settings)
        self._add_action("history", self.open_history)
        self._add_action("quit", self.quit)
        self.application.connect("startup", self._startup)
        self.application.connect("activate", self._activate)

    def _add_action(self, name, callback):
        action = Gio.SimpleAction.new(name, None)
        action.connect("activate", lambda *_: callback())
        self.application.add_action(action)

    def _startup(self, application):
        application.hold()
        try:
            set_autostart(
                self.config.autostart_enabled,
                launcher=Path.home() / ".local" / "bin" / "flow-linux",
            )
        except OSError as exc:
            print(f"[Flow] Could not update autostart entry: {exc}")
        self.overlay.create_window(application)
        # Spawn the separate GTK3 tray process before pynput starts worker
        # threads. This keeps the fork/exec boundary simple on Linux desktops.
        self._start_tray()
        threading.Thread(
            target=self.listener.run, name="flow-hotkeys", daemon=True
        ).start()

    def _start_tray(self):
        tray_path = Path(__file__).with_name("flow") / "tray.py"
        tray_env = os.environ.copy()
        # GTK4 can leave these variables present but empty. GTK3 treats an
        # empty GDK_BACKEND as an explicit (invalid) backend and aborts.
        for name in ("GDK_BACKEND", "GTK_MODULES"):
            if not tray_env.get(name):
                tray_env.pop(name, None)
        try:
            self.tray_process = subprocess.Popen(
                [sys.executable, str(tray_path)],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                env=tray_env,
                start_new_session=True,
            )
        except OSError as exc:
            print(f"[Flow] Tray could not start: {exc}")

    def _activate(self, _application):
        if self.open_settings_on_start:
            GLib.idle_add(self.open_settings)

    def open_settings(self):
        if self.settings_window is None:
            self.settings_window = SettingsWindow(
                self.application, self.config, self._save_settings
            )
        if self.config_error:
            self.settings_window.error.set_text(self.config_error)
        self.settings_window.present()

    def open_history(self):
        if self.history_window is None:
            self.history_window = HistoryWindow(
                self.application,
                self.history_store,
                history_enabled=lambda: self.config.save_history,
            )
        self.history_window.present()

    def _notify_insertion_recovery(self, transcript_saved):
        GLib.idle_add(self._send_insertion_recovery_notification, transcript_saved)

    def _send_insertion_recovery_notification(self, transcript_saved):
        try:
            notification = Gio.Notification.new("Flow Linux")
            if transcript_saved:
                notification.set_body(
                    "Text could not be inserted. The transcript is saved in local History."
                )
                notification.set_default_action("app.history")
            else:
                notification.set_body(
                    "Text could not be inserted and was not saved. Check Settings or dictate again."
                )
            self.application.send_notification("insertion-recovery", notification)
        except Exception as exc:
            print(f"[Flow] Could not show insertion recovery notification: {exc}")
        return GLib.SOURCE_REMOVE

    def _save_settings(self, config):
        was_handsfree = self.controller.is_handsfree_active()
        set_autostart(
            config.autostart_enabled,
            launcher=Path.home() / ".local" / "bin" / "flow-linux",
        )
        self.store.save(config)
        self.config = config
        self.config_error = ""
        self.controller.update_config(config)
        self.listener.state.handsfree_enabled = config.handsfree_enabled
        if was_handsfree and not config.handsfree_enabled:
            self.controller.toggle_handsfree()

    def quit(self):
        self.controller.shutdown()
        if self.tray_process is not None and self.tray_process.poll() is None:
            self.tray_process.terminate()
        self.application.quit()

    def run(self):
        return self.application.run([])


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if "--settings" in args:
        try:
            result = subprocess.run(
                ["gapplication", "action", APP_ID, "settings"],
                check=False, timeout=1.5,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            if result.returncode == 0:
                return 0
        except (OSError, subprocess.SubprocessError):
            pass
    app = FlowApplication(args)
    return app.run()


if __name__ == "__main__":
    raise SystemExit(main())
