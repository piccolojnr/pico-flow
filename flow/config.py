"""Configuration stored in the user's private TOML file."""

from dataclasses import dataclass
import json
import os
from pathlib import Path
import tomllib


@dataclass(frozen=True)
class Config:
    groq_api_key: str = ""
    input_device: str = ""
    model: str = "whisper-large-v3-turbo"
    language: str = "en"
    shortcut: str = "ctrl+super"
    minimum_duration: float = 0.35
    silence_threshold: float = 220.0
    api_timeout: float = 45.0
    clipboard_restore_delay: float = 0.6
    handsfree_enabled: bool = True
    autostart_enabled: bool = True


def default_config_path() -> Path:
    config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    return config_home / "flow-linux" / "config.toml"


def _legacy_env(path: Path) -> dict[str, str]:
    values = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return values
    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def _bool(value, name):
    if isinstance(value, bool):
        return value
    raise ValueError(f"{name} must be true or false")


def _number(value, name, default, cast):
    if value is None:
        return default
    if isinstance(value, bool):
        raise ValueError(f"{name} must be a number")
    try:
        result = cast(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{name} must be a valid {cast.__name__}") from exc
    if result < 0:
        raise ValueError(f"{name} cannot be negative")
    return result


def _from_toml(data) -> Config:
    if not isinstance(data, dict):
        raise ValueError("config.toml must contain a TOML table")
    transcription = data.get("transcription", {})
    audio = data.get("audio", {})
    shortcuts = data.get("shortcuts", {})
    app = data.get("app", {})
    if not all(isinstance(section, dict) for section in (transcription, audio, shortcuts, app)):
        raise ValueError("config.toml sections must be tables")

    shortcut = str(shortcuts.get("push_to_talk", "ctrl+super")).strip().lower()
    keys = [part.strip() for part in shortcut.split("+") if part.strip()]
    supported = {"ctrl", "super", "alt", "shift"}
    if len(keys) != 2 or len(set(keys)) != 2 or not set(keys) <= supported:
        raise ValueError("shortcuts.push_to_talk must be a pair such as ctrl+super")

    return Config(
        groq_api_key=str(transcription.get("groq_api_key", "")).strip(),
        model=str(transcription.get("model", "whisper-large-v3-turbo")).strip(),
        language=str(transcription.get("language", "en")).strip(),
        api_timeout=_number(transcription.get("api_timeout"), "api_timeout", 45.0, float),
        input_device=str(audio.get("input_device", "")).strip(),
        minimum_duration=_number(audio.get("minimum_duration"), "minimum_duration", 0.35, float),
        silence_threshold=_number(audio.get("silence_threshold"), "silence_threshold", 220.0, float),
        shortcut="+".join(keys),
        handsfree_enabled=_bool(shortcuts.get("handsfree_enabled", True), "handsfree_enabled"),
        autostart_enabled=_bool(app.get("autostart", True), "autostart"),
        clipboard_restore_delay=_number(
            app.get("clipboard_restore_delay"), "clipboard_restore_delay", 0.6, float
        ),
    )


def _toml_string(value: str) -> str:
    # JSON strings are a valid TOML basic-string subset and correctly escape keys.
    return json.dumps(value, ensure_ascii=False)


def serialize_config(config: Config) -> str:
    return "\n".join([
        "# Flow Linux configuration. Keep this file private (mode 0600).",
        "",
        "[transcription]",
        f"groq_api_key = {_toml_string(config.groq_api_key)}",
        f"model = {_toml_string(config.model)}",
        f"language = {_toml_string(config.language)}",
        f"api_timeout = {config.api_timeout:g}",
        "",
        "[audio]",
        f"input_device = {_toml_string(config.input_device)}",
        f"minimum_duration = {config.minimum_duration:g}",
        f"silence_threshold = {config.silence_threshold:g}",
        "",
        "[shortcuts]",
        f"push_to_talk = {_toml_string(config.shortcut)}",
        f"handsfree_enabled = {str(config.handsfree_enabled).lower()}",
        "",
        "[app]",
        f"autostart = {str(config.autostart_enabled).lower()}",
        f"clipboard_restore_delay = {config.clipboard_restore_delay:g}",
        "",
    ])


class ConfigStore:
    """Load/save config.toml, migrating a previous .env once when needed."""

    def __init__(self, path=None, legacy_paths=None):
        self.path = Path(path) if path is not None else default_config_path()
        if legacy_paths is None:
            legacy_paths = [self.path.parent / ".env", Path.cwd() / ".env"]
        self.legacy_paths = list(dict.fromkeys(Path(item) for item in legacy_paths))

    def load(self) -> Config:
        if self.path.exists():
            try:
                self.path.parent.chmod(0o700)
                self.path.chmod(0o600)
                data = tomllib.loads(self.path.read_text(encoding="utf-8"))
                return _from_toml(data)
            except (tomllib.TOMLDecodeError, OSError) as exc:
                raise ValueError(f"Could not read {self.path}: {exc}") from exc

        config = Config()
        legacy = next((path for path in self.legacy_paths if path.is_file()), None)
        if legacy is not None:
            values = _legacy_env(legacy)
            shortcut = (values.get("FLOW_SHORTCUT") or config.shortcut).strip().lower()
            handsfree = (values.get("FLOW_HANDSFREE_ENABLED") or "true").strip().lower()
            autostart = (values.get("FLOW_AUTOSTART_ENABLED") or "true").strip().lower()
            if handsfree not in {"true", "false", "1", "0", "yes", "no"}:
                raise ValueError("FLOW_HANDSFREE_ENABLED must be true or false")
            if autostart not in {"true", "false", "1", "0", "yes", "no"}:
                raise ValueError("FLOW_AUTOSTART_ENABLED must be true or false")
            # Reuse the normal validator for numeric options and the shortcut.
            config = _from_toml({
                "transcription": {
                    "groq_api_key": values.get("GROQ_API_KEY", ""),
                    "model": values.get("FLOW_MODEL", config.model),
                    "language": values.get("FLOW_LANGUAGE", config.language),
                    "api_timeout": _number(values.get("FLOW_API_TIMEOUT"), "FLOW_API_TIMEOUT", 45.0, float),
                },
                "audio": {
                    "input_device": values.get("FLOW_INPUT_DEVICE", ""),
                    "minimum_duration": _number(values.get("FLOW_MINIMUM_DURATION"), "FLOW_MINIMUM_DURATION", 0.35, float),
                    "silence_threshold": _number(values.get("FLOW_SILENCE_THRESHOLD"), "FLOW_SILENCE_THRESHOLD", 220.0, float),
                },
                "shortcuts": {
                    "push_to_talk": shortcut,
                    "handsfree_enabled": handsfree in {"true", "1", "yes"},
                },
                "app": {
                    "autostart": autostart in {"true", "1", "yes"},
                    "clipboard_restore_delay": _number(
                        values.get("FLOW_CLIPBOARD_RESTORE_DELAY"),
                        "FLOW_CLIPBOARD_RESTORE_DELAY", 0.6, float,
                    ),
                },
            })
        self.save(config)
        return config

    def save(self, config: Config) -> None:
        content = serialize_config(config)
        self.path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        try:
            self.path.parent.chmod(0o700)
        except OSError:
            pass
        temporary = self.path.with_suffix(".toml.tmp")
        try:
            temporary.write_text(content, encoding="utf-8")
            temporary.chmod(0o600)
            temporary.replace(self.path)
            self.path.chmod(0o600)
        finally:
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
