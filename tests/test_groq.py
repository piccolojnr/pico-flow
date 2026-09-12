import requests
import pytest

from flow.providers.groq import GroqProvider
from flow.providers.base import TranscriptionError


class Response:
    def __init__(self, status=200, payload=None):
        self.status_code = status
        self.payload = payload or {"text": " hello "}

    def raise_for_status(self):
        if self.status_code >= 400:
            error = requests.HTTPError("http error")
            error.response = self
            raise error

    def json(self):
        return self.payload


class Session:
    def __init__(self, response=None, error=None):
        self.response = response or Response()
        self.error = error
        self.calls = []

    def post(self, *args, **kwargs):
        self.calls.append((args, kwargs))
        if self.error:
            raise self.error
        return self.response


def test_groq_transcribes_and_sends_model_language_and_timeout(tmp_path):
    audio = tmp_path / "test.wav"
    audio.write_bytes(b"audio")
    session = Session()
    provider = GroqProvider("secret", "whisper-large-v3-turbo", timeout=12, session=session)
    assert provider.transcribe(audio, "en") == "hello"
    kwargs = session.calls[0][1]
    assert kwargs["data"]["model"] == "whisper-large-v3-turbo"
    assert kwargs["data"]["language"] == "en"
    assert kwargs["timeout"] == 12


@pytest.mark.parametrize("status,kind", [(401, "configuration"), (429, "rate_limit"), (503, "api")])
def test_groq_classifies_api_failures(tmp_path, status, kind):
    audio = tmp_path / "test.wav"
    audio.write_bytes(b"audio")
    provider = GroqProvider("secret", "model", session=Session(Response(status)))
    with pytest.raises(TranscriptionError) as result:
        provider.transcribe(audio, "en")
    assert result.value.kind == kind


def test_groq_maps_network_and_missing_key(tmp_path):
    audio = tmp_path / "test.wav"
    audio.write_bytes(b"audio")
    provider = GroqProvider("secret", "model", session=Session(error=requests.Timeout()))
    with pytest.raises(TranscriptionError, match="timed out"):
        provider.transcribe(audio, "en")
    with pytest.raises(TranscriptionError, match="Flow Settings"):
        GroqProvider("", "model").transcribe(audio, "en")
