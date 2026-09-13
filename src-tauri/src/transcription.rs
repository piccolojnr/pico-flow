use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
struct Response {
    text: Option<String>,
}
struct PrivateTempFile(PathBuf);
impl Drop for PrivateTempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
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
    transcribe_at(
        "https://api.groq.com/openai/v1/audio/transcriptions",
        wav,
        key,
        model,
        language,
        timeout,
    )
}

fn transcribe_at(
    endpoint: &str,
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
        "url = {}\nheader = {}\nform = {}\nform = {}\nform = {}\nform = {}\nmax-time = {}\nfail-with-body\n",
        curl_quote(endpoint),
        curl_quote(&format!("Authorization: Bearer {key}")),
        curl_quote(&format!("model={model}")),
        curl_quote(&format!("language={language}")),
        curl_quote("response_format=json"),
        curl_quote("temperature=0"),
        timeout.ceil().max(1.0)
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&config)
        .map_err(|e| format!("Could not prepare transcription request: {e}"))?;
    let _cleanup = PrivateTempFile(config.clone());
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Could not prepare transcription request: {e}"))?;
    drop(file);
    let result = Command::new("curl")
        .args(["--silent", "--show-error", "--config"])
        .arg(&config)
        .arg("--form")
        .arg(format!("file=@{};type=audio/wav", wav.display()))
        .args(["--write-out", "\nFLOW_HTTP_STATUS:%{http_code}"])
        .output();
    let output = result.map_err(|_| "Could not start curl; install curl".to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let (body, http_status) = stdout
        .rsplit_once("\nFLOW_HTTP_STATUS:")
        .map(|(body, status)| (body, status.trim().parse::<u16>().unwrap_or(0)))
        .unwrap_or((&stdout, 0));
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let body_lower = body.to_lowercase();
        if http_status == 401 || body_lower.contains("invalid_api_key") {
            return Err("Invalid Groq API key".into());
        }
        if http_status == 429 || body_lower.contains("rate_limit") {
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
    let response: Response = serde_json::from_str(body)
        .map_err(|_| "Groq returned an unreadable response".to_string())?;
    Ok(response.text.unwrap_or_default().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn sends_private_multipart_request_and_parses_transcript() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/transcriptions", server.local_addr().unwrap());
        let server_thread = thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            let header_end;
            loop {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
                if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    header_end = position + 4;
                    let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    while request.len() < header_end + length {
                        let count = stream.read(&mut buffer).unwrap();
                        if count == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..count]);
                    }
                    break;
                }
            }
            let request_text = String::from_utf8_lossy(&request).to_lowercase();
            assert!(request_text.contains("authorization: bearer test-key"));
            assert!(request_text.contains("name=\"model\""));
            assert!(request_text.contains("whisper-test"));
            assert!(request_text.contains("name=\"file\""));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"text\":\"hello\"}").unwrap();
        });

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let wav =
            std::env::temp_dir().join(format!("flow-test-{}-{nonce}.wav", std::process::id()));
        fs::write(&wav, b"synthetic test audio").unwrap();
        let result = transcribe_at(&endpoint, &wav, "test-key", "whisper-test", "en", 3.0);
        let _ = fs::remove_file(wav);
        server_thread.join().unwrap();
        assert_eq!(result.unwrap(), "hello");
    }
}
