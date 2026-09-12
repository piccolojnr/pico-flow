from abc import ABC, abstractmethod


class TranscriptionError(Exception):
    def __init__(self, message, kind="failed"):
        super().__init__(message)
        self.kind = kind


class TranscriptionProvider(ABC):
    @abstractmethod
    def transcribe(self, audio_path, language, context=""):
        """Return transcription text for an audio file."""
