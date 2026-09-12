"""Groq Whisper-compatible audio transcription provider."""

import requests

from .base import TranscriptionError, TranscriptionProvider


class GroqProvider(TranscriptionProvider):
    endpoint = "https://api.groq.com/openai/v1/audio/transcriptions"

    def __init__(self, api_key, model, timeout=45.0, session=None):
        self.api_key = api_key
        self.model = model
        self.timeout = timeout
        self.session = session or requests

    def transcribe(self, audio_path, language, context=""):
        if not self.api_key:
            raise TranscriptionError("Add a Groq API key in Flow Settings", "configuration")
        try:
            with open(audio_path, "rb") as audio:
                response = self.session.post(
                    self.endpoint,
                    headers={"Authorization": f"Bearer {self.api_key}"},
                    files={"file": ("speech.wav", audio, "audio/wav")},
                    data={
                        "model": self.model,
                        "language": language,
                        "response_format": "json",
                        "temperature": "0",
                    },
                    timeout=self.timeout,
                )
            if response.status_code == 401:
                raise TranscriptionError("Invalid Groq API key", "configuration")
            if response.status_code == 429:
                raise TranscriptionError("Groq rate limit; try again shortly", "rate_limit")
            response.raise_for_status()
            result = response.json()
            return result.get("text", "").strip() if isinstance(result, dict) else ""
        except TranscriptionError:
            raise
        except requests.Timeout as exc:
            raise TranscriptionError("Transcription timed out", "network") from exc
        except requests.ConnectionError as exc:
            raise TranscriptionError("No internet connection", "network") from exc
        except requests.HTTPError as exc:
            status = exc.response.status_code if exc.response is not None else 0
            message = "Groq service error" if status >= 500 else "Transcription request failed"
            raise TranscriptionError(message, "api") from exc
        except (OSError, ValueError, requests.RequestException) as exc:
            raise TranscriptionError("Transcription failed", "api") from exc
