//! Sound card output through cpal (WASAPI on Windows). The engine runs inside the
//! sound card's callback.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use synth_core::{Engine, Event};

use crate::shared::Status;

/// Playing stops when this is dropped.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub name: String,
}

pub fn open(events: rtrb::Consumer<Event>, status: Arc<Status>) -> Result<AudioOutput, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("ses çıkış aygıtı bulunamadı")?;
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| device.to_string());
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, events, status),
        SampleFormat::I16 => build::<i16>(&device, config, events, status),
        SampleFormat::U16 => build::<u16>(&device, config, events, status),
        SampleFormat::I32 => build::<i32>(&device, config, events, status),
        other => return Err(format!("desteklenmeyen örnek biçimi {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(AudioOutput { _stream: stream, sample_rate: config.sample_rate, name })
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut events: rtrb::Consumer<Event>,
    status: Arc<Status>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    let mut engine = Engine::new(config.sample_rate as f32);
    // Grown on the first callback only, then reused.
    let mut mono: Vec<f32> = Vec::with_capacity(8192);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                while let Ok(event) = events.pop() {
                    engine.handle(event);
                }
                let frames = data.len() / channels;
                if mono.len() < frames {
                    mono.resize(frames, 0.0);
                }
                let block = &mut mono[..frames];
                engine.render(block);
                let mut peak = 0.0f32;
                for (frame, &v) in data.chunks_exact_mut(channels).zip(block.iter()) {
                    let v = v.clamp(-1.0, 1.0);
                    peak = peak.max(v.abs());
                    frame.fill(T::from_sample(v));
                }
                status.report_audio(engine.active_voices(), peak, engine.held_notes(), engine.lfo_value());
            },
            |e| eprintln!("ses: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
