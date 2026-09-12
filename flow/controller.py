"""Coordinates recording, transcription, status, and text insertion."""

from concurrent.futures import ThreadPoolExecutor
import threading
import time

from . import audio, clipboard
from .audio import Recorder
from .clipboard import InsertionError
from .providers import GroqProvider, TranscriptionError


class DictationController:
    def __init__(self, config, overlay, recorder=None, provider=None, inserter=None):
        self.config = config
        self.overlay = overlay
        self.recorder = recorder or Recorder(device=config.input_device or None)
        self.provider = provider or GroqProvider(
            config.groq_api_key, config.model, timeout=config.api_timeout
        )
        self.inserter = inserter or clipboard.insert_text
        self._lock = threading.RLock()
        self._recording = False
        self._recording_mode = None
        self._session = 0
        self._target_window = None
        self._pool = ThreadPoolExecutor(max_workers=1, thread_name_prefix="flow-transcribe")

    def start(self):
        self._start_recording("push_to_talk")

    def _start_recording(self, mode):
        with self._lock:
            if self._recording:
                return
            self._session += 1
            session = self._session
            try:
                self.recorder.start()
                target_window = clipboard.active_window()
            except Exception as exc:
                print(f"[Flow] Microphone unavailable: {exc}")
                self._error(session, "Microphone unavailable", 1.4)
                return
            self._target_window = target_window
            self._recording = True
            self._recording_mode = mode
        print("[Flow] Recording started")
        label = "Hands-free listening" if mode == "handsfree" else "Listening"
        self._status(session, f"●   {label}")

    def stop(self):
        with self._lock:
            if not self._recording or self._recording_mode == "handsfree":
                return
            stopped = self._stop_recording_locked()
        self._finish_recording(*stopped)

    def is_handsfree_active(self):
        with self._lock:
            return self._recording and self._recording_mode == "handsfree"

    def toggle_handsfree(self):
        should_start = False
        should_stop = False
        with self._lock:
            if self._recording_mode == "handsfree":
                should_stop = True
            elif self._recording_mode == "push_to_talk":
                # Ctrl+Super may arrive before Space. Promote that already-live
                # capture so the original PTT chord remains immediate.
                self._recording_mode = "handsfree"
                session = self._session
            else:
                should_start = True
        if should_start:
            self._start_recording("handsfree")
        elif should_stop:
            with self._lock:
                if self._recording and self._recording_mode == "handsfree":
                    stopped = self._stop_recording_locked()
                else:
                    stopped = None
            if stopped:
                self._finish_recording(*stopped)
        elif not should_start and not should_stop:
            self._status(session, "●   Hands-free listening")

    def _stop_recording_locked(self):
        self._recording = False
        self._recording_mode = None
        session = self._session
        target_window = self._target_window
        try:
            capture = self.recorder.stop()
        except Exception as exc:
            print(f"[Flow] Microphone error: {exc}")
            return (session, target_window, None, exc)
        return (session, target_window, capture, None)

    def _finish_recording(self, session, target_window, capture, error):
        if error is not None:
            self._error(session, "Microphone unavailable", 1.4)
            return

        print(f"[Flow] duration={capture.duration:.2f}s rms={capture.rms:.0f}")
        if capture.duration < self.config.minimum_duration:
            self._error(session, "Recording too short", 0.8)
            return
        if capture.rms < self.config.silence_threshold:
            self._error(session, "No speech detected", 1.0)
            return

        self._status(session, "◌   Transcribing…")
        self._pool.submit(self._transcribe_and_insert, session, capture, target_window)

    def _is_current(self, session):
        with self._lock:
            return session == self._session

    def _status(self, session, message):
        if self._is_current(session):
            self.overlay.show(message)

    def _error(self, session, message, delay):
        self._status(session, message)
        self._pool.submit(self._dismiss_after, session, delay)

    def _dismiss_after(self, session, delay):
        time.sleep(delay)
        if self._is_current(session):
            self.overlay.ready()

    def _transcribe_and_insert(self, session, capture, target_window):
        path = None
        try:
            path = audio.write_wav(capture)
            print("[Flow] Transcribing")
            started = time.monotonic()
            text = self.provider.transcribe(path, self.config.language)
            print(f"[Flow] Transcript received in {time.monotonic() - started:.2f}s")
            if not text.strip():
                self._error(session, "No speech detected", 1.0)
                return
            self.inserter(
                text.strip(), target_window=target_window,
                restore_delay=self.config.clipboard_restore_delay,
            )
            print("[Flow] Text inserted")
            self._status(session, "✓   Done")
            self._dismiss_after(session, 0.8)
        except TranscriptionError as exc:
            print(f"[Flow] {exc}")
            label = str(exc)
            if exc.kind == "api":
                label = "Transcription failed"
            self._error(session, label, 1.4)
        except InsertionError as exc:
            print(f"[Flow] {exc}")
            self._error(session, "Text insertion failed", 1.4)
        except Exception as exc:
            print(f"[Flow] Dictation failed: {exc}")
            self._error(session, "Transcription failed", 1.4)
        finally:
            if path:
                try:
                    import os
                    os.remove(path)
                except OSError:
                    pass

    def update_config(self, config):
        with self._lock:
            self.config = config
            if hasattr(self.recorder, "device"):
                self.recorder.device = config.input_device or None
            self.provider = GroqProvider(
                config.groq_api_key, config.model, timeout=config.api_timeout
            )

    def shutdown(self):
        """Stop an active capture without submitting it during application quit."""
        with self._lock:
            if self._recording:
                self._recording = False
                self._recording_mode = None
                try:
                    self.recorder.stop()
                except Exception as exc:
                    print(f"[Flow] Microphone cleanup failed: {exc}")
        self._pool.shutdown(wait=False, cancel_futures=True)
