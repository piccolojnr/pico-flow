use std::{
    fs::{self, OpenOptions},
    io::{BufReader, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub struct Capture {
    pub wav_path: PathBuf,
    pub duration: f64,
    pub rms: f64,
}
pub struct Recorder {
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    samples: Arc<Mutex<Vec<f32>>>,
    started: Option<Instant>,
}
impl Default for Recorder {
    fn default() -> Self {
        Self {
            child: None,
            reader: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            started: None,
        }
    }
}

impl Recorder {
    pub fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        self.samples
            .lock()
            .map_err(|_| "Audio buffer unavailable")?
            .clear();
        let mut child = Command::new("pw-record").args(["--raw", "--format", "f32", "--rate", "48000", "--channels", "1", "--channel-map", "MONO", "-"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not open the default microphone. Install PipeWire tools (pw-record): {e}"))?;
        let stdout = child.stdout.take().ok_or("Microphone stream unavailable")?;
        let samples = Arc::clone(&self.samples);
        self.reader = Some(thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut bytes = [0; 4096];
            let mut partial = Vec::new();
            loop {
                let count = match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                partial.extend_from_slice(&bytes[..count]);
                let complete = partial.len() / 4 * 4;
                if let Ok(mut out) = samples.lock() {
                    out.extend(
                        partial[..complete]
                            .chunks_exact(4)
                            .map(|x| f32::from_le_bytes(x.try_into().unwrap())),
                    );
                } else {
                    break;
                }
                partial.drain(..complete);
            }
        }));
        self.child = Some(child);
        self.started = Some(Instant::now());
        Ok(())
    }
    pub fn stop(&mut self) -> Result<Capture, String> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let elapsed = self
            .started
            .take()
            .map(|x| x.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let samples = self
            .samples
            .lock()
            .map_err(|_| "Audio buffer unavailable")?
            .clone();
        let rms = if samples.is_empty() {
            0.0
        } else {
            (samples
                .iter()
                .map(|x| f64::from(*x) * f64::from(*x))
                .sum::<f64>()
                / samples.len() as f64)
                .sqrt()
                * 32768.0
        };
        self.samples
            .lock()
            .map_err(|_| "Audio buffer unavailable")?
            .clear();
        let duration = (samples.len() as f64 / 48_000.0).max(elapsed.min(0.15));
        let wav_path = write_private_wav(&samples)?;
        Ok(Capture {
            wav_path,
            duration,
            rms,
        })
    }
}

fn write_private_wav(samples: &[f32]) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("flow-linux-{}-{nonce}.wav", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("Could not create temporary audio: {e}"))?;
    // Downsample 48 kHz to 16 kHz by averaging each group of three frames.
    let pcm: Vec<i16> = samples
        .chunks_exact(3)
        .map(|group| {
            let mean = group.iter().map(|x| x.clamp(-1.0, 1.0)).sum::<f32>() / 3.0;
            (mean * if mean < 0.0 { 32768.0 } else { 32767.0 }).round() as i16
        })
        .collect();
    let data_len = (pcm.len() * 2) as u32;
    file.write_all(b"RIFF")
        .and_then(|_| file.write_all(&(36 + data_len).to_le_bytes()))
        .and_then(|_| file.write_all(b"WAVEfmt "))
        .and_then(|_| file.write_all(&16u32.to_le_bytes()))
        .and_then(|_| file.write_all(&1u16.to_le_bytes()))
        .and_then(|_| file.write_all(&1u16.to_le_bytes()))
        .and_then(|_| file.write_all(&16_000u32.to_le_bytes()))
        .and_then(|_| file.write_all(&32_000u32.to_le_bytes()))
        .and_then(|_| file.write_all(&2u16.to_le_bytes()))
        .and_then(|_| file.write_all(&16u16.to_le_bytes()))
        .and_then(|_| file.write_all(b"data"))
        .and_then(|_| file.write_all(&data_len.to_le_bytes()))
        .map_err(|e| {
            let _ = fs::remove_file(&path);
            format!("Could not write temporary audio: {e}")
        })?;
    for value in pcm {
        file.write_all(&value.to_le_bytes())
            .map_err(|e| format!("Could not write temporary audio: {e}"))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_has_mono_16khz_header_and_downsamples() {
        let p = write_private_wav(&[0.5, 0.5, 0.5, -0.5, -0.5, -0.5]).unwrap();
        let b = fs::read(&p).unwrap();
        let _ = fs::remove_file(p);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 16000);
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 4);
    }
}
