"""Private, local SQLite storage for recent transcription text."""

from dataclasses import dataclass
from datetime import datetime, timezone
import os
from pathlib import Path
import sqlite3
import threading


MAX_HISTORY_ENTRIES = 500


def default_history_path() -> Path:
    data_home = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
    return data_home / "flow-linux" / "history.db"


@dataclass(frozen=True)
class HistoryEntry:
    id: int
    created_at: str
    transcript: str
    duration: float
    provider: str
    model: str
    insertion_status: str


class HistoryStore:
    """Open short-lived SQLite connections so worker and GTK threads can share storage."""

    def __init__(self, path=None, max_entries=MAX_HISTORY_ENTRIES):
        if isinstance(max_entries, bool) or int(max_entries) < 1:
            raise ValueError("max_entries must be a positive integer")
        self.path = Path(path) if path is not None else default_history_path()
        self.max_entries = int(max_entries)
        self._lock = threading.RLock()
        self._initialized = False

    def _connect(self):
        self.path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        try:
            self.path.parent.chmod(0o700)
        except OSError:
            pass
        connection = sqlite3.connect(self.path, timeout=5.0)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA busy_timeout = 5000")
        try:
            self.path.chmod(0o600)
        except OSError:
            pass
        return connection

    def _initialize_database(self):
        connection = self._connect()
        try:
            integrity = connection.execute("PRAGMA quick_check").fetchone()[0]
            if integrity != "ok":
                raise sqlite3.DatabaseError(f"database integrity check failed: {integrity}")
            connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS dictations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    created_at TEXT NOT NULL,
                    transcript TEXT NOT NULL,
                    duration REAL NOT NULL CHECK (duration >= 0),
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    insertion_status TEXT NOT NULL
                        CHECK (insertion_status IN ('pending', 'inserted', 'failed'))
                );
                CREATE INDEX IF NOT EXISTS dictations_created_at
                    ON dictations(created_at DESC, id DESC);
                """
            )
            connection.commit()
        finally:
            connection.close()
        try:
            self.path.chmod(0o600)
        except OSError:
            pass

    @staticmethod
    def _is_corrupt_error(error):
        message = str(error).lower()
        return any(phrase in message for phrase in (
            "malformed", "not a database", "database disk image", "file is encrypted",
        ))

    def _preserve_corrupt_database(self):
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        damaged = self.path.with_name(f"{self.path.name}.corrupt-{stamp}")
        os.replace(self.path, damaged)
        # Preserve any SQLite sidecars alongside the damaged file for possible
        # manual recovery; normal operation never creates audio files here.
        for suffix in ("-wal", "-shm"):
            sidecar = Path(f"{self.path}{suffix}")
            if sidecar.exists():
                os.replace(sidecar, Path(f"{damaged}{suffix}"))

    def _ensure_initialized(self):
        with self._lock:
            if self._initialized:
                return
            try:
                self._initialize_database()
            except sqlite3.DatabaseError as exc:
                if not self.path.exists() or not self._is_corrupt_error(exc):
                    raise
                self._preserve_corrupt_database()
                self._initialize_database()
            self._initialized = True

    @staticmethod
    def _entry(row):
        return HistoryEntry(
            id=row["id"], created_at=row["created_at"],
            transcript=row["transcript"], duration=row["duration"],
            provider=row["provider"], model=row["model"],
            insertion_status=row["insertion_status"],
        )

    def add(self, transcript, duration, provider, model):
        if not isinstance(transcript, str) or not transcript.strip():
            raise ValueError("transcript must contain text")
        if float(duration) < 0:
            raise ValueError("duration cannot be negative")
        self._ensure_initialized()
        created_at = datetime.now(timezone.utc).isoformat(timespec="microseconds")
        with self._lock:
            connection = self._connect()
            try:
                connection.execute("BEGIN IMMEDIATE")
                cursor = connection.execute(
                    """INSERT INTO dictations
                       (created_at, transcript, duration, provider, model, insertion_status)
                       VALUES (?, ?, ?, ?, ?, 'pending')""",
                    (created_at, transcript, float(duration), str(provider), str(model)),
                )
                entry_id = cursor.lastrowid
                connection.execute(
                    """DELETE FROM dictations WHERE id NOT IN (
                           SELECT id FROM dictations
                           ORDER BY created_at DESC, id DESC LIMIT ?
                       )""",
                    (self.max_entries,),
                )
                connection.commit()
                return entry_id
            except Exception:
                connection.rollback()
                raise
            finally:
                connection.close()

    def mark_insertion(self, entry_id, status):
        if status not in {"pending", "inserted", "failed"}:
            raise ValueError("insertion status must be pending, inserted, or failed")
        self._ensure_initialized()
        with self._lock:
            connection = self._connect()
            try:
                with connection:
                    connection.execute(
                        "UPDATE dictations SET insertion_status = ? WHERE id = ?",
                        (status, int(entry_id)),
                    )
            finally:
                connection.close()

    def list_entries(self, search="", limit=MAX_HISTORY_ENTRIES):
        self._ensure_initialized()
        try:
            limit = max(1, min(int(limit), MAX_HISTORY_ENTRIES))
        except (TypeError, ValueError):
            limit = MAX_HISTORY_ENTRIES
        search = str(search or "").strip()
        pattern = ("%" + search.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%")
        with self._lock:
            connection = self._connect()
            try:
                rows = connection.execute(
                    """SELECT id, created_at, transcript, duration, provider, model,
                              insertion_status
                       FROM dictations
                       WHERE ? = '' OR transcript LIKE ? ESCAPE '\\'
                       ORDER BY created_at DESC, id DESC LIMIT ?""",
                    (search, pattern, limit),
                ).fetchall()
                return [self._entry(row) for row in rows]
            finally:
                connection.close()

    def delete(self, entry_id):
        self._ensure_initialized()
        with self._lock:
            connection = self._connect()
            try:
                with connection:
                    cursor = connection.execute("DELETE FROM dictations WHERE id = ?", (int(entry_id),))
                    return cursor.rowcount > 0
            finally:
                connection.close()

    def clear(self):
        self._ensure_initialized()
        with self._lock:
            connection = self._connect()
            try:
                with connection:
                    cursor = connection.execute("DELETE FROM dictations")
                    return cursor.rowcount
            finally:
                connection.close()
