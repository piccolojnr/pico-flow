use super::Capture;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample,
};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Default)]
pub struct Recorder {
    stop: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<Result<Capture, String>>>,
}
impl Recorder {
    pub fn start(&mut self) -> Result<(), String> {
        if self.worker.is_some() {
            return Ok(());
        }
        let (stop, receive_stop) = mpsc::channel();
        let (ready, receive_ready) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let result = capture(receive_stop, &ready);
            if let Err(error) = &result {
                let _ = ready.send(Err(error.clone()));
            }
            result
        });
        match receive_ready
            .recv()
            .map_err(|_| "Microphone worker stopped".to_string())?
        {
            Ok(()) => {
                self.stop = Some(stop);
                self.worker = Some(worker);
                Ok(())
            }
            Err(error) => {
                let _ = worker.join();
                Err(error)
            }
        }
    }
    pub fn stop(&mut self) -> Result<Capture, String> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.worker
            .take()
            .ok_or("Microphone is not recording")?
            .join()
            .map_err(|_| "Microphone worker failed".to_string())?
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            if let Ok(Ok(capture)) = worker.join() {
                let _ = std::fs::remove_file(capture.wav_path);
            }
        }
    }
}
fn capture(
    stop: mpsc::Receiver<()>,
    ready: &mpsc::SyncSender<Result<(), String>>,
) -> Result<Capture, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No default microphone. Check your system sound settings.")?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("Could not open microphone: {e}"))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let error = Arc::new(Mutex::new(None));
    let stream = match format {
        SampleFormat::I8 => input::<i8>(&device, &config, &samples, &error),
        SampleFormat::I16 => input::<i16>(&device, &config, &samples, &error),
        SampleFormat::I32 => input::<i32>(&device, &config, &samples, &error),
        SampleFormat::I64 => input::<i64>(&device, &config, &samples, &error),
        SampleFormat::U8 => input::<u8>(&device, &config, &samples, &error),
        SampleFormat::U16 => input::<u16>(&device, &config, &samples, &error),
        SampleFormat::U32 => input::<u32>(&device, &config, &samples, &error),
        SampleFormat::U64 => input::<u64>(&device, &config, &samples, &error),
        SampleFormat::F32 => input::<f32>(&device, &config, &samples, &error),
        SampleFormat::F64 => input::<f64>(&device, &config, &samples, &error),
        _ => return Err("Unsupported microphone sample format".into()),
    }?;
    stream
        .play()
        .map_err(|e| format!("Microphone permission or device error: {e}"))?;
    ready.send(Ok(())).map_err(|_| "Recording cancelled")?;
    let _ = stop.recv();
    drop(stream);
    if let Some(error) = error
        .lock()
        .map_err(|_| "Microphone state unavailable")?
        .take()
    {
        return Err(error);
    }
    let samples = samples.lock().map_err(|_| "Audio buffer unavailable")?;
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
            * 32768.0
    };
    Ok(Capture {
        wav_path: super::write_private_wav(&samples, config.sample_rate.0)?,
        duration: samples.len() as f64 / f64::from(config.sample_rate.0),
        rms,
    })
}
fn input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: &Arc<Mutex<Vec<f32>>>,
    error: &Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let samples = Arc::clone(samples);
    let error = Arc::clone(error);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut samples) = samples.lock() {
                    samples.extend(data.chunks_exact(channels).map(|frame| {
                        frame.iter().map(|v| v.to_sample::<f32>()).sum::<f32>() / channels as f32
                    }));
                }
            },
            move |failure| {
                if let Ok(mut error) = error.lock() {
                    *error = Some(format!("Microphone disconnected or unavailable: {failure}"));
                }
            },
            None,
        )
        .map_err(|e| format!("Could not start microphone: {e}"))
}
