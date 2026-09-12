import numpy as np

from flow.audio import Recorder, write_wav


class FakeStream:
    def __init__(self, callback, **kwargs):
        self.callback = callback
        self.stopped = False
        self.closed = False
        self.fail_start = False

    def start(self):
        if self.fail_start:
            raise RuntimeError("device disconnected")
        self.callback(np.array([[300], [400]], dtype=np.int16), 2, None, None)

    def stop(self):
        self.stopped = True

    def close(self):
        self.closed = True


def test_recorder_calculates_duration_and_rms_and_writes_wav():
    streams = []

    def factory(**kwargs):
        stream = FakeStream(kwargs["callback"])
        streams.append(stream)
        return stream

    recorder = Recorder(samplerate=2, stream_factory=factory)
    assert recorder.start()
    capture = recorder.stop()
    assert capture.duration == 1.0
    assert capture.rms == 353.5533905932738
    assert streams[0].stopped and streams[0].closed

    # WAV format is the application's fixed mono/16kHz format.
    path = write_wav(capture)
    try:
        import wave
        with wave.open(path, "rb") as wav:
            assert wav.getnchannels() == 1
            assert wav.getframerate() == 16000
            assert wav.getnframes() == 2
    finally:
        import os
        os.remove(path)


def test_recorder_closes_stream_if_device_start_fails():
    stream = FakeStream(lambda *args: None)
    stream.fail_start = True
    recorder = Recorder(stream_factory=lambda **kwargs: stream)
    try:
        recorder.start()
    except RuntimeError as error:
        assert str(error) == "device disconnected"
    else:
        raise AssertionError("expected device startup to fail")
    assert stream.closed


def test_recorder_passes_configured_input_device():
    options = {}
    def factory(**kwargs):
        options.update(kwargs)
        return FakeStream(kwargs["callback"])

    recorder = Recorder(stream_factory=factory, device="4")
    assert recorder.start()
    recorder.stop()
    assert options["device"] == 4
