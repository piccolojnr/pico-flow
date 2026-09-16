use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub transcript: String,
    pub duration: f64,
    pub provider: String,
    pub model: String,
    pub insertion_status: String,
}
fn database() -> PathBuf {
    crate::config::data_dir().join("history.db")
}
fn connect(path: &Path) -> Result<Connection, String> {
    crate::storage::secure_directory(path.parent().ok_or("Invalid history path")?)?;
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    crate::storage::secure_file(path)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS dictations (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at TEXT NOT NULL, transcript TEXT NOT NULL, duration REAL NOT NULL CHECK(duration >= 0), provider TEXT NOT NULL, model TEXT NOT NULL, insertion_status TEXT NOT NULL CHECK(insertion_status IN ('pending','inserted','failed'))); CREATE INDEX IF NOT EXISTS dictations_created_at ON dictations(created_at DESC,id DESC);").map_err(|e| e.to_string())?;
    Ok(connection)
}
pub fn add(text: &str, duration: f64, model: &str) -> Result<i64, String> {
    add_at(&database(), text, duration, model)
}
fn add_at(path: &Path, text: &str, duration: f64, model: &str) -> Result<i64, String> {
    let mut connection = connect(path)?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    transaction.execute("INSERT INTO dictations(created_at,transcript,duration,provider,model,insertion_status) VALUES(strftime('%Y-%m-%dT%H:%M:%fZ','now'),?1,?2,'Groq',?3,'pending')", params![text,duration.max(0.0),model]).map_err(|e| e.to_string())?;
    let id = transaction.last_insert_rowid();
    transaction.execute("DELETE FROM dictations WHERE id NOT IN (SELECT id FROM dictations ORDER BY created_at DESC,id DESC LIMIT 500)", []).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(id)
}
pub fn mark(id: i64, status: &str) {
    let _ = mark_at(&database(), id, status);
}
fn mark_at(path: &Path, id: i64, status: &str) -> Result<(), String> {
    connect(path)?
        .execute(
            "UPDATE dictations SET insertion_status=?1 WHERE id=?2",
            params![status, id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn list(search: &str) -> Result<Vec<Entry>, String> {
    list_at(&database(), search)
}
fn list_at(path: &Path, search: &str) -> Result<Vec<Entry>, String> {
    let connection = connect(path)?;
    let mut statement = connection.prepare("SELECT id,created_at,transcript,duration,provider,model,insertion_status FROM dictations WHERE instr(lower(transcript),lower(?1))>0 ORDER BY created_at DESC,id DESC LIMIT 500").map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([search], |row| {
            Ok(Entry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                transcript: row.get(2)?,
                duration: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                insertion_status: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn delete(id: i64) -> Result<(), String> {
    delete_at(&database(), id)
}
fn delete_at(path: &Path, id: i64) -> Result<(), String> {
    connect(path)?
        .execute("DELETE FROM dictations WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn clear() -> Result<(), String> {
    clear_at(&database())
}
fn clear_at(path: &Path) -> Result<(), String> {
    connect(path)?
        .execute("DELETE FROM dictations", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_history_supports_insert_search_status_and_delete() {
        let folder = std::env::temp_dir().join(format!(
            "flow-history-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&folder).unwrap();
        crate::storage::secure_directory(&folder).unwrap();
        let path = folder.join("history.db");
        let text = "Flow's dictation; DROP TABLE dictations; --";
        let id = add_at(&path, text, 1.25, "whisper-test").unwrap();
        let found = list_at(&path, "DROP TABLE").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].transcript, text);
        mark_at(&path, id, "inserted").unwrap();
        assert_eq!(list_at(&path, "").unwrap()[0].insertion_status, "inserted");
        delete_at(&path, id).unwrap();
        assert!(list_at(&path, "").unwrap().is_empty());
        clear_at(&path).unwrap();
        fs::remove_dir_all(folder).unwrap();
    }
}
