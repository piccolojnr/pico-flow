"""Microphone capture and temporary WAV handling."""

from dataclasses import dataclass
import os
import tempfile
import wave

import numpy as np
import sounddevice as sd


SAMPLE_RATE = 16000
CHANNELS = 1


@dataclass(frozen=True)
class Capture:
    samples: np.ndarray
    duration: float
    rms: float


class Recorder:
    def __init__(self, samplerate=SAMPLE_RATE, channels=CHANNELS, stream_factory=None, device=None):
        self.samplerate = samplerate
        self.channels = channels
        self.stream_factory = stream_factory or sd.InputStream
        self.device = device
        self._stream = None
        self._chunks = []

    def _callback(self, indata, frame_count, time_info, status):
        if status:
            print(f"[Flow] Audio input status: {status}")
        self._chunks.append(indata.copy())

    def start(self):
        if self._stream is not None:
            return False
        self._chunks = []
        stream_options = {}
        if self.device not in (None, ""):
            device = self.device
            if isinstance(device, str) and device.isdecimal():
                device = int(device)
            stream_options["device"] = device
        stream = self.stream_factory(
            samplerate=self.samplerate,
            channels=self.channels,
            dtype="int16",
            callback=self._callback,
            **stream_options,
        )
        try:
            stream.start()
        except Exception:
            # PortAudio may allocate a stream before device startup fails.
            stream.close()
            raise
        self._stream = stream
        return True

    def stop(self):
        stream, self._stream = self._stream, None
        if stream is None:
            return Capture(np.empty((0, self.channels), dtype=np.int16), 0.0, 0.0)
        try:
            stream.stop()
        finally:
            stream.close()
        if not self._chunks:
            samples = np.empty((0, self.channels), dtype=np.int16)
        else:
            samples = np.concatenate(self._chunks, axis=0)
        duration = len(samples) / self.samplerate
        rms = float(np.sqrt(np.mean(samples.astype(np.float64) ** 2))) if samples.size else 0.0
        return Capture(samples=samples, duration=duration, rms=rms)


def write_wav(capture: Capture) -> str:
    fd, path = tempfile.mkstemp(prefix="flow-linux-", suffix=".wav")
    os.close(fd)
    try:
        with wave.open(path, "wb") as output:
            output.setnchannels(CHANNELS)
            output.setsampwidth(2)
            output.setframerate(SAMPLE_RATE)
            output.writeframes(capture.samples.astype(np.int16, copy=False).tobytes())
        return path
    except Exception:
        try:
            os.unlink(path)
        except OSError:
            pass
        raise
