import numpy as np

from flow.audio import Capture
from flow.clipboard import InsertionError
from flow.config import Config
from flow.controller import DictationController
from flow.history import HistoryStore


class Overlay:
    def __init__(self):
        self.messages = []
        self.hidden = 0

    def show(self, message):
        self.messages.append(message)

    def hide(self):
        self.hidden += 1


class Recorder:
    def __init__(self, capture):
        self.capture = capture
        self.started = 0
        self.stopped = 0

    def start(self):
        self.started += 1

    def stop(self):
        self.stopped += 1
        return self.capture


class Provider:
    def __init__(self, result="recognized speech"):
        self.result = result
        self.calls = 0

    def transcribe(self, *args):
        self.calls += 1
        return self.result


def capture(duration=1.0, rms=700):
    return Capture(np.ones((16000, 1), dtype=np.int16), duration, rms)


def controller(
    recorder, provider, inserted, monkeypatch, *, save_history=False,
    history_store=None, recovery_notifier=None, inserter=None,
):
    overlay = Overlay()
    config = Config(
        groq_api_key="test", minimum_duration=0.35,
        silence_threshold=220, save_history=save_history,
    )
    monkeypatch.setattr("flow.controller.clipboard.active_window", lambda: "window-1")
    item = DictationController(
        config, overlay, recorder=recorder, provider=provider,
        inserter=inserter or (lambda text, **kwargs: inserted.append((text, kwargs))),
        history_store=history_store, recovery_notifier=recovery_notifier,
    )
    item._session = 1
    return item, overlay


def test_rejects_short_and_silent_capture_without_provider(monkeypatch):
    short_provider = Provider()
    inserted = []
    item, overlay = controller(Recorder(capture(duration=0.1)), short_provider, inserted, monkeypatch)
    item.start()
    item.start()  # duplicate press while recording is ignored
    assert item.recorder.started == 1
    item.stop()
    assert short_provider.calls == 0
    assert overlay.messages == ["●   Listening"]
    assert overlay.hidden >= 1
    item._pool.shutdown(wait=True)

    silent_provider = Provider()
    item, overlay = controller(Recorder(capture(rms=10)), silent_provider, [], monkeypatch)
    item.start()
    item.stop()
    assert silent_provider.calls == 0
    assert overlay.messages == ["●   Listening"]
    assert overlay.hidden >= 1
    item._pool.shutdown(wait=True)


def test_audio_file_is_removed_after_provider_failure(tmp_path, monkeypatch):
    path = tmp_path / "temporary.wav"
    path.write_bytes(b"private audio")
    monkeypatch.setattr("flow.controller.audio.write_wav", lambda _capture: str(path))
    inserted = []

    class FailingProvider:
        def transcribe(self, *args):
            raise RuntimeError("offline")

    item, overlay = controller(Recorder(capture()), FailingProvider(), inserted, monkeypatch)
    item._transcribe_and_insert(1, capture(), "window-1")
    assert not path.exists()
    assert inserted == []
    assert overlay.messages == []
    item._pool.shutdown(wait=True)


def test_transcript_is_inserted_without_logging_or_duplicate_submission(monkeypatch):
    inserted = []
    provider = Provider("  hello  ")
    item, overlay = controller(Recorder(capture()), provider, inserted, monkeypatch)
    item._transcribe_and_insert(1, capture(), "window-1")
    assert inserted == [("hello", {"target_window": "window-1", "restore_delay": 0.6})]
    item._pool.shutdown(wait=True)
    assert overlay.messages == []
    assert overlay.hidden == 1


def test_history_disabled_does_not_persist_but_dictation_still_inserts(monkeypatch, tmp_path):
    history = HistoryStore(path=tmp_path / "history.db")
    inserted = []
    item, _overlay = controller(
        Recorder(capture()), Provider("still works"), inserted, monkeypatch,
        save_history=False, history_store=history,
    )

    item._transcribe_and_insert(1, capture(), "window-1")
    item._pool.shutdown(wait=True)

    assert inserted[0][0] == "still works"
    assert not history.path.exists()


def test_successful_dictation_updates_saved_history_status(monkeypatch):
    class MemoryHistoryStore:
        def __init__(self):
            self.added = []
            self.statuses = []

        def add(self, *args):
            self.added.append(args)
            return 17

        def mark_insertion(self, entry_id, status):
            self.statuses.append((entry_id, status))

    history = MemoryHistoryStore()
    item, _overlay = controller(
        Recorder(capture()), Provider("saved text"), [], monkeypatch,
        save_history=True, history_store=history,
    )

    item._transcribe_and_insert(1, capture(duration=1.25), "window-1")
    item._pool.shutdown(wait=True)

    assert history.added == [("saved text", 1.25, "Groq", "whisper-large-v3-turbo")]
    assert history.statuses == [(17, "inserted")]


def test_insertion_failure_keeps_transcript_and_notifies_history_recovery(monkeypatch, tmp_path):
    history = HistoryStore(tmp_path / "history.db")
    notifications = []

    def fail_insertion(*_args, **_kwargs):
        raise InsertionError("clipboard unavailable")

    item, _overlay = controller(
        Recorder(capture()), Provider("recover this sentence"), [], monkeypatch,
        save_history=True, history_store=history, recovery_notifier=notifications.append,
        inserter=fail_insertion,
    )

    item._transcribe_and_insert(1, capture(), "window-1")
    item._pool.shutdown(wait=True)

    entries = history.list_entries()
    assert entries[0].transcript == "recover this sentence"
    assert entries[0].insertion_status == "failed"
    assert notifications == [True]


def test_history_database_failure_does_not_interrupt_successful_paste(monkeypatch):
    class BrokenHistoryStore:
        def add(self, *_args):
            raise OSError("read-only data directory")

    inserted = []
    item, _overlay = controller(
        Recorder(capture()), Provider("still pasted"), inserted, monkeypatch,
        save_history=True, history_store=BrokenHistoryStore(),
    )
    item._transcribe_and_insert(1, capture(), "window-1")
    item._pool.shutdown(wait=True)

    assert inserted[0][0] == "still pasted"


def test_handsfree_toggles_recording_and_ptt_release_does_not_stop_it(monkeypatch):
    inserted = []
    provider = Provider("hands-free words")
    recorder = Recorder(capture())
    item, _overlay = controller(recorder, provider, inserted, monkeypatch)

    item.toggle_handsfree()
    assert item.is_handsfree_active()
    assert recorder.started == 1

    item.stop()  # releasing Ctrl or Super does not end hands-free capture
    assert item.is_handsfree_active()
    assert recorder.stopped == 0

    item.toggle_handsfree()
    assert not item.is_handsfree_active()
    item._pool.shutdown(wait=True)
    assert recorder.stopped == 1
    assert provider.calls == 1
    assert inserted[0][0] == "hands-free words"


def test_handsfree_chord_promotes_existing_ptt_capture(monkeypatch):
    inserted = []
    provider = Provider("promoted capture")
    recorder = Recorder(capture())
    item, _overlay = controller(recorder, provider, inserted, monkeypatch)

    item.start()
    item.toggle_handsfree()
    item.stop()  # PTT chord releases after Space; promoted capture continues
    assert item.is_handsfree_active()
    assert recorder.stopped == 0

    item.toggle_handsfree()
    item._pool.shutdown(wait=True)
    assert recorder.started == 1
    assert recorder.stopped == 1
    assert provider.calls == 1
    assert inserted[0][0] == "promoted capture"
