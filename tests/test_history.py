import os

import pytest

from flow.history import HistoryStore, default_history_path


def test_add_and_newest_first_with_insertion_status(tmp_path):
    store = HistoryStore(tmp_path / "history.db")
    first = store.add("first dictated text", 1.25, "Groq", "whisper-test")
    second = store.add("second dictated text", 2.5, "Groq", "whisper-test")
    store.mark_insertion(first, "inserted")

    entries = store.list_entries()
    assert [entry.transcript for entry in entries] == [
        "second dictated text", "first dictated text",
    ]
    assert entries[0].duration == 2.5
    assert entries[0].provider == "Groq"
    assert entries[0].model == "whisper-test"
    assert entries[0].insertion_status == "pending"
    assert entries[1].insertion_status == "inserted"
    assert entries[0].created_at.endswith("+00:00")


def test_retention_removes_oldest_entries(tmp_path):
    store = HistoryStore(tmp_path / "history.db", max_entries=3)
    assert HistoryStore().max_entries == 500
    for index in range(5):
        store.add(f"dictation {index}", 1, "Groq", "model")

    assert [entry.transcript for entry in store.list_entries()] == [
        "dictation 4", "dictation 3", "dictation 2",
    ]


def test_delete_clear_and_search(tmp_path):
    store = HistoryStore(tmp_path / "history.db")
    first = store.add("the quick brown fox", 1, "Groq", "model")
    store.add("another line", 1, "Groq", "model")

    assert [entry.transcript for entry in store.list_entries(search="quick")] == [
        "the quick brown fox",
    ]
    assert store.delete(first) is True
    assert store.delete(first) is False
    assert store.clear() == 1
    assert store.list_entries() == []


def test_search_treats_sql_wildcards_as_literal_text(tmp_path):
    store = HistoryStore(tmp_path / "history.db")
    store.add("100% certain", 1, "Groq", "model")
    store.add("100 percent", 1, "Groq", "model")

    assert [entry.transcript for entry in store.list_entries(search="%")] == ["100% certain"]


def test_database_path_uses_xdg_data_home(monkeypatch, tmp_path):
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "xdg-data"))
    assert default_history_path() == tmp_path / "xdg-data" / "flow-linux" / "history.db"


def test_malformed_database_is_preserved_and_recreated(tmp_path):
    path = tmp_path / "history.db"
    path.write_bytes(b"this is not a sqlite database")
    store = HistoryStore(path)

    store.add("recoverable transcript", 0.75, "Groq", "model")

    assert [item.transcript for item in store.list_entries()] == ["recoverable transcript"]
    backups = list(tmp_path.glob("history.db.corrupt-*"))
    assert len(backups) == 1
    assert backups[0].read_bytes() == b"this is not a sqlite database"
    assert path.stat().st_mode & 0o777 == 0o600
    if os.name == "posix":
        assert path.parent.stat().st_mode & 0o777 == 0o700


def test_rejects_empty_transcripts_and_unknown_statuses(tmp_path):
    store = HistoryStore(tmp_path / "history.db")
    with pytest.raises(ValueError, match="transcript"):
        store.add("  \n", 1, "Groq", "model")
    entry_id = store.add("text", 1, "Groq", "model")
    with pytest.raises(ValueError, match="insertion status"):
        store.mark_insertion(entry_id, "lost")
