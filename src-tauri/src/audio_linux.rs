use super::Capture;
use std::{
    io::{BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Instant,
};

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
        let wav_path = super::write_private_wav(&samples, 48_000)?;
        Ok(Capture {
            wav_path,
            duration,
            rms,
        })
    }
}
