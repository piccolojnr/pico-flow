use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, path::Path, time::Duration};

#[derive(Deserialize)]
struct Response {
    text: Option<String>,
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
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err("Invalid transcription timeout".into());
    }
    let audio = fs::read(wav).map_err(|e| format!("Could not read recording: {e}"))?;
    let part = multipart::Part::bytes(audio)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = multipart::Form::new()
        .text("model", model.to_owned())
        .text("language", language.to_owned())
        .text("response_format", "json")
        .text("temperature", "0")
        .part("file", part);
    let client = Client::builder()
        .timeout(Duration::from_secs_f64(timeout))
        .build()
        .map_err(|_| "Could not initialize secure transcription connection")?;
    let response = client
        .post(endpoint)
        .bearer_auth(key)
        .multipart(form)
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                "Transcription timed out".to_string()
            } else if error.is_connect() {
                "Could not connect to Groq".to_string()
            } else {
                "Transcription request failed".to_string()
            }
        })?;
    match response.status().as_u16() {
        401 | 403 => return Err("Invalid Groq API key or model access".into()),
        429 => return Err("Groq rate limit; try again shortly".into()),
        200..=299 => {}
        _ => return Err("Transcription failed; check the Groq settings and connection".into()),
    }
    response
        .json::<Response>()
        .map(|r| r.text.unwrap_or_default().trim().to_string())
        .map_err(|_| "Groq returned an unreadable response".into())
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
