use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{traits::*, HeapRb};
use std::sync::Arc;


pub struct Resampler {
    pub input_rate: f32,
    pub target_rate: f32,
    last_sample: f32,
    phase: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum AudioSource {
    Microphone,
    SystemAudio,
}


impl Resampler {
    pub fn new(input_rate: u32, ) -> Self {
        Self { 
            input_rate: input_rate as f32, 
            target_rate: 16000.0, // 16kHz
            last_sample: 0.0, 
            phase: 0.0 
        }
    }
    pub fn process(&mut self, input:&[f32], output: &mut Vec<f32>) {

        if input.is_empty() {
            return;
        }
 let ratio = self.input_rate / self.target_rate;
        let mut in_idx = 0;
        while in_idx < input.len() {
            let (left, right) = if in_idx == 0 {
                (self.last_sample, input[0])
            } else {
                (input[in_idx - 1], input[in_idx])
            };
            let t = self.phase;
            let interpolated = left + t * (right - left);
            output.push(interpolated);
            self.phase += ratio;
            if self.phase >= 1.0 {
                let advance = self.phase.floor() as usize;
                in_idx += advance;
                self.phase -= advance as f32;
            }
        }
        self.last_sample = *input.last().unwrap();
    }
}
pub struct HighPassFilter {
    last_input: f32,
}
impl HighPassFilter {
    pub fn new() -> Self {
        Self { last_input: 0.0 }
    }
    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let current_input = *sample;
            *sample = current_input - 0.95 * self.last_input;
            self.last_input = current_input;
        }
    }
}
pub struct NoiseGate {
    threshold: f32,
    envelope: f32,
    attack: f32,
    release: f32,
}
impl NoiseGate {
    pub fn new(threshold: f32, sample_rate: f32) -> Self {
        let attack = (-1.0 / (sample_rate * 0.010)).exp();
        let release = (-1.0 / (sample_rate * 0.150)).exp();
        Self {
            threshold,
            envelope: 0.0,
            attack,
            release,
        }
    }
    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let abs_val = sample.abs();
            if abs_val > self.envelope {
                self.envelope = self.attack * self.envelope + (1.0 - self.attack) * abs_val;
            } else {
                self.envelope = self.release * self.envelope + (1.0 - self.release) * abs_val;
            }
            if self.envelope < self.threshold {
                *sample = 0.0;
            }
        }
    }
}


pub struct AudioInput {
    _stream: cpal::Stream,
}
impl AudioInput {
    pub fn new(source: AudioSource, mut producer: ringbuf::CachingProd<Arc<HeapRb<f32>>>) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = match source {
            AudioSource::Microphone => host
                .default_input_device()
                .ok_or("No input audio device found.")?,
            AudioSource::SystemAudio => host
                .default_output_device()
                .ok_or("No Output audio device found.")?,
        };
        let config = match source {
            AudioSource::Microphone => device.default_input_config()?,
            AudioSource::SystemAudio => device.default_output_config()?,
        };
        let sample_rate = config.sample_rate();
        let channels = config.channels();
        println!(
            "Opened default input device: {} (Rate: {}Hz, Channels: {})",
            device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "Unknown".to_string()),
            sample_rate,
            channels
        );
        let mut filter = HighPassFilter::new();
        let mut resampler = Resampler::new(sample_rate);
        let mut gate = NoiseGate::new(0.005, 16000.0); 
        let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);
        let mut process_buffer = move |raw_samples: &[f32]| {
            let mono: Vec<f32> = if channels == 1 {
                raw_samples.to_vec()
            } else {
                raw_samples
                    .chunks_exact(channels as usize)
                    .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                    .collect()
            };
            let mut filtered = mono;
            filter.process(&mut filtered);
            let ratio = resampler.input_rate / resampler.target_rate;
            let mut resampled = Vec::with_capacity((filtered.len() as f32 / ratio) as usize + 1);
            resampler.process(&filtered, &mut resampled);
            gate.process(&mut resampled);
            let _ = producer.push_slice(&resampled);
        };
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config.into(),
                move |data: &[f32], _| process_buffer(data),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                config.into(),
                move |data: &[i16], _| {
                    let f32_data: Vec<f32> = data
                        .iter()
                        .map(|&s| s as f32 / i16::MAX as f32)
                        .collect();
                    process_buffer(&f32_data);
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                config.into(),
                move |data: &[u16], _| {
                    let f32_data: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - i16::MAX as f32) / i16::MAX as f32)
                        .collect();
                    process_buffer(&f32_data);
                },
                err_fn,
                None,
            )?,
            _ => return Err("Unsupported sample format".into()),
        };
        stream.play()?;
        Ok(Self { _stream: stream })
    }
}