#[cfg(test)]
use std::fs;
use std::{io::Write, path::PathBuf};

#[cfg(target_os = "linux")]
#[path = "audio_linux.rs"]
mod backend;
#[cfg(not(target_os = "linux"))]
#[path = "audio_native.rs"]
mod backend;
pub use backend::Recorder;

pub struct Capture {
    pub wav_path: PathBuf,
    pub duration: f64,
    pub rms: f64,
}

// Average source frames into mono 16 kHz PCM. Low-rate inputs repeat frames.
fn resample(samples: &[f32], rate: u32) -> Vec<i16> {
    if rate == 0 {
        return Vec::new();
    }
    let count = samples.len() as u64 * 16_000 / u64::from(rate);
    (0..count)
        .map(|i| {
            let start = (i * u64::from(rate) / 16_000) as usize;
            let end = (((i + 1) * u64::from(rate) / 16_000) as usize)
                .max(start + 1)
                .min(samples.len());
            let group = &samples[start..end];
            let mean = group
                .iter()
                .map(|x| {
                    if x.is_finite() {
                        x.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
                / group.len() as f32;
            (mean * if mean < 0.0 { 32768.0 } else { 32767.0 }).round() as i16
        })
        .collect()
}

fn write_private_wav(samples: &[f32], sample_rate: u32) -> Result<PathBuf, String> {
    let mut temporary = tempfile::Builder::new()
        .prefix("flow-audio-")
        .suffix(".wav")
        .tempfile()
        .map_err(|e| format!("Could not create temporary audio: {e}"))?;
    let file = temporary.as_file_mut();
    let pcm = resample(samples, sample_rate);
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
        .map_err(|e| format!("Could not write temporary audio: {e}"))?;
    for value in pcm {
        if let Err(error) = file.write_all(&value.to_le_bytes()) {
            return Err(format!("Could not write temporary audio: {error}"));
        }
    }
    let (_, path) = temporary.keep().map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_has_mono_16khz_header_and_downsamples() {
        let p = write_private_wav(&[0.5, 0.5, 0.5, -0.5, -0.5, -0.5], 48_000).unwrap();
        let b = fs::read(&p).unwrap();
        let _ = fs::remove_file(p);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 16000);
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 4);
    }
}
