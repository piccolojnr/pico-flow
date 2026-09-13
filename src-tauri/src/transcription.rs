use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
struct Response {
    text: Option<String>,
}
fn curl_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}
pub fn transcribe(
    wav: &Path,
    key: &str,
    model: &str,
    language: &str,
    timeout: f64,
) -> Result<String, String> {
    if key.trim().is_empty() {
        return Err("Add a Groq API key in Settings".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let config =
        std::env::temp_dir().join(format!("flow-curl-{}-{nonce}.conf", std::process::id()));
    let content = format!(
        "url = {}\nheader = {}\nform = {}\nform = {}\nform = {}\nmax-time = {}\nfail-with-body\n",
        curl_quote("https://api.groq.com/openai/v1/audio/transcriptions"),
        curl_quote(&format!("Authorization: Bearer {key}")),
        curl_quote(&format!("model={model}")),
        curl_quote(&format!("language={language}")),
        curl_quote("response_format=json"),
        timeout.ceil().max(1.0)
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&config)
        .map_err(|e| format!("Could not prepare transcription request: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Could not prepare transcription request: {e}"))?;
    drop(file);
    let result = Command::new("curl")
        .args(["--silent", "--show-error", "--config"])
        .arg(&config)
        .arg("--form")
        .arg(format!("file=@{};type=audio/wav", wav.display()))
        .output();
    let _ = fs::remove_file(config);
    let output = result.map_err(|_| "Could not start curl; install curl".to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let body = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if body.contains("\"status\":401") || body.contains("invalid_api_key") {
            return Err("Invalid Groq API key".into());
        }
        if body.contains("\"status\":429") || body.contains("rate_limit") {
            return Err("Groq rate limit; try again shortly".into());
        }
        if error.contains("timed out") || error.contains("timeout") {
            return Err("Transcription timed out".into());
        }
        if error.contains("resolve") || error.contains("connect") {
            return Err("No internet connection".into());
        }
        return Err("Transcription failed; check the Groq settings and connection".into());
    }
    let response: Response = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Groq returned an unreadable response".to_string())?;
    Ok(response.text.unwrap_or_default().trim().to_string())
}
