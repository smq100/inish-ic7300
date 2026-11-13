use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BUFFER_SECONDS: usize = 12;

pub struct AudioInput {
    pub name: String,
    pub sample_rate: u32,
    pub samples: Arc<Mutex<VecDeque<f32>>>,
    _stream: Stream,
}

pub struct AudioService;

impl AudioService {
    pub fn list_devices() -> Result<Vec<(usize, String)>> {
        let host = cpal::default_host();
        let mut found = Vec::new();
        for (i, dev) in host.input_devices()?.enumerate() {
            let name = dev.name().unwrap_or_else(|_| "<unknown>".into());
            found.push((i, name));
        }
        Ok(found)
    }

    pub fn connect(index: usize) -> Result<AudioInput> {
        let host = cpal::default_host();
        let device = host
            .input_devices()?
            .nth(index)
            .ok_or_else(|| format!("Input device index {index} not found"))?;
        Self::open(device)
    }

    fn open(device: Device) -> Result<AudioInput> {
        let name = device.name()?;
        let supported = device.default_input_config()?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let rate = config.sample_rate.0;
        let channels = config.channels as usize;
        let capacity = (rate as usize) * BUFFER_SECONDS;
        let samples = Arc::new(Mutex::new(VecDeque::with_capacity(capacity)));
        let shared = Arc::clone(&samples);
        let err_fn = |e| eprintln!("Audio stream error: {e}");
        let stream = match format {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    push_mono(data.iter().copied(), channels, &shared, capacity);
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => {
                let shared = Arc::clone(&samples);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        push_mono(
                            data.iter().map(|&v| v as f32 / 32768.0),
                            channels,
                            &shared,
                            capacity,
                        );
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let shared = Arc::clone(&samples);
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        push_mono(
                            data.iter().map(|&v| (v as f32 - 32768.0) / 32768.0),
                            channels,
                            &shared,
                            capacity,
                        );
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(format!("Unsupported sample format: {other:?}").into()),
        };
        stream.play()?;
        Ok(AudioInput {
            name,
            sample_rate: rate,
            samples,
            _stream: stream,
        })
    }
}

fn push_mono<I: Iterator<Item = f32>>(
    data: I,
    channels: usize,
    shared: &Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
) {
    if channels == 0 {
        return;
    }
    if let Ok(mut buffer) = shared.lock() {
        let mut frame = Vec::with_capacity(channels);
        for value in data {
            frame.push(value);
            if frame.len() == channels {
                let mono = frame.iter().copied().sum::<f32>() / channels as f32;
                if buffer.len() >= capacity {
                    buffer.pop_front();
                }
                buffer.push_back(mono);
                frame.clear();
            }
        }
    }
}

impl AudioInput {
    pub fn snapshot(&self, seconds: usize) -> Vec<f32> {
        let Ok(buffer) = self.samples.lock() else {
            return Vec::new();
        };
        let count = (seconds * self.sample_rate as usize).min(buffer.len());
        buffer.iter().skip(buffer.len() - count).copied().collect()
    }
}
