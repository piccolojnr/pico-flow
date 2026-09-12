import numpy as np

from flow.audio import Capture
from flow.config import Config
from flow.controller import DictationController


class Overlay:
    def __init__(self):
        self.messages = []
        self.ready_count = 0

    def show(self, message):
        self.messages.append(message)

    def ready(self):
        self.ready_count += 1


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


def controller(recorder, provider, inserted, monkeypatch):
    overlay = Overlay()
    config = Config(groq_api_key="test", minimum_duration=0.35, silence_threshold=220)
    monkeypatch.setattr("flow.controller.clipboard.active_window", lambda: "window-1")
    item = DictationController(
        config, overlay, recorder=recorder, provider=provider,
        inserter=lambda text, **kwargs: inserted.append((text, kwargs)),
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
    assert overlay.messages[-1] == "Recording too short"
    item._pool.shutdown(wait=True)

    silent_provider = Provider()
    item, overlay = controller(Recorder(capture(rms=10)), silent_provider, [], monkeypatch)
    item.start()
    item.stop()
    assert silent_provider.calls == 0
    assert overlay.messages[-1] == "No speech detected"
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
    assert overlay.messages[-1] == "Transcription failed"
    item._pool.shutdown(wait=True)


def test_transcript_is_inserted_without_logging_or_duplicate_submission(monkeypatch):
    inserted = []
    provider = Provider("  hello  ")
    item, overlay = controller(Recorder(capture()), provider, inserted, monkeypatch)
    item._transcribe_and_insert(1, capture(), "window-1")
    assert inserted == [("hello", {"target_window": "window-1", "restore_delay": 0.6})]
    assert overlay.messages[-1] == "✓   Done"
    item._pool.shutdown(wait=True)
    assert overlay.ready_count == 1


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
