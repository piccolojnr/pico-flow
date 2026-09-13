use serde::{Deserialize, Serialize};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

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
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn database() -> PathBuf {
    crate::config::data_dir().join("history.db")
}
fn run(sql: &str) -> Result<String, String> {
    let path = database();
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).ok();
    let result = Command::new("sqlite3")
        .args(["-json"])
        .arg(&path)
        .arg(sql)
        .output()
        .map_err(|_| "Local history requires sqlite3".to_string())?;
    if !result.status.success() {
        return Err(format!(
            "History database error: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok();
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_string())
}
pub fn initialize() -> Result<(), String> {
    run("PRAGMA busy_timeout=5000; CREATE TABLE IF NOT EXISTS dictations (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at TEXT NOT NULL, transcript TEXT NOT NULL, duration REAL NOT NULL CHECK(duration >= 0), provider TEXT NOT NULL, model TEXT NOT NULL, insertion_status TEXT NOT NULL CHECK(insertion_status IN ('pending','inserted','failed'))); CREATE INDEX IF NOT EXISTS dictations_created_at ON dictations(created_at DESC,id DESC);")?;
    Ok(())
}
pub fn add(text: &str, duration: f64, model: &str) -> Result<i64, String> {
    initialize()?;
    let sql = format!("INSERT INTO dictations(created_at,transcript,duration,provider,model,insertion_status) VALUES(strftime('%Y-%m-%dT%H:%M:%fZ','now'),{},{},'Groq',{},'pending'); DELETE FROM dictations WHERE id NOT IN (SELECT id FROM dictations ORDER BY created_at DESC,id DESC LIMIT 500); SELECT last_insert_rowid() AS id;", quote(text), duration.max(0.0), quote(model));
    #[derive(Deserialize)]
    struct Id {
        id: i64,
    }
    let rows: Vec<Id> = serde_json::from_str(&run(&sql)?)
        .map_err(|_| "Could not save local history".to_string())?;
    rows.first()
        .map(|row| row.id)
        .ok_or_else(|| "Could not save local history".to_string())
}
pub fn mark(id: i64, status: &str) {
    let _ = run(&format!(
        "UPDATE dictations SET insertion_status={} WHERE id={id};",
        quote(status)
    ));
}
pub fn list(search: &str) -> Result<Vec<Entry>, String> {
    initialize()?;
    let sql = format!("SELECT id,created_at,transcript,duration,provider,model,insertion_status FROM dictations WHERE instr(lower(transcript),lower({}))>0 ORDER BY created_at DESC,id DESC LIMIT 500;", quote(search));
    serde_json::from_str(&run(&sql)?.chars().collect::<String>())
        .map_err(|_| "Could not read local history".to_string())
}
pub fn delete(id: i64) -> Result<(), String> {
    initialize()?;
    run(&format!("DELETE FROM dictations WHERE id={id};"))?;
    Ok(())
}
pub fn clear() -> Result<(), String> {
    initialize()?;
    run("DELETE FROM dictations;")?;
    Ok(())
}
