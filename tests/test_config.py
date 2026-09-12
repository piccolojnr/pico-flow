import os

import pytest

from flow.config import Config, ConfigStore


def test_toml_round_trip_and_private_file_mode(tmp_path):
    path = tmp_path / "flow-linux" / "config.toml"
    store = ConfigStore(path=path, legacy_paths=[])
    config = Config(
        groq_api_key='secret "quoted" value', input_device="5", language="fr",
        handsfree_enabled=False, autostart_enabled=False, save_history=True,
    )
    store.save(config)

    assert store.load() == config
    assert path.stat().st_mode & 0o777 == 0o600
    if os.name == "posix":
        assert path.parent.stat().st_mode & 0o777 == 0o700


def test_migrates_legacy_env_once_and_keeps_toml_authoritative(tmp_path):
    legacy = tmp_path / ".env"
    legacy.write_text(
        "GROQ_API_KEY=legacy-key\nFLOW_LANGUAGE=fr\nFLOW_INPUT_DEVICE=5\n"
        "FLOW_SHORTCUT=ctrl+alt\nFLOW_SILENCE_THRESHOLD=180\n",
        encoding="utf-8",
    )
    store = ConfigStore(path=tmp_path / "config.toml", legacy_paths=[legacy])
    migrated = store.load()
    assert migrated.groq_api_key == "legacy-key"
    assert migrated.language == "fr"
    assert migrated.input_device == "5"
    assert migrated.shortcut == "ctrl+alt"
    assert migrated.silence_threshold == 180
    assert migrated.handsfree_enabled is True
    assert migrated.save_history is False
    assert store.path.exists()

    legacy.write_text("GROQ_API_KEY=changed-later\n", encoding="utf-8")
    assert store.load().groq_api_key == "legacy-key"


def test_invalid_shortcut_and_negative_threshold_fail(tmp_path):
    path = tmp_path / "config.toml"
    store = ConfigStore(path=path, legacy_paths=[])
    path.write_text("[shortcuts]\npush_to_talk = 'ctrl+space'\n", encoding="utf-8")
    with pytest.raises(ValueError, match="push_to_talk"):
        store.load()

    path.write_text("[audio]\nsilence_threshold = -1\n", encoding="utf-8")
    with pytest.raises(ValueError, match="cannot be negative"):
        store.load()


def test_default_config_is_written_to_toml(tmp_path):
    store = ConfigStore(path=tmp_path / "config.toml", legacy_paths=[])
    assert store.load() == Config()
    assert "[transcription]" in store.path.read_text(encoding="utf-8")
