//! Sound card output through cpal (WASAPI on Windows). The engine runs inside the
//! sound card's callback.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use ozan_core::wavetable::WavetableBank;
use ozan_core::{Engine, Event};

use crate::shared::Status;

/// Playing stops when this is dropped.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub name: String,
}

pub fn open(events: rtrb::Consumer<Event>, status: Arc<Status>, bank: Arc<WavetableBank>) -> Result<AudioOutput, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(ozan_ui::i18n::s().no_audio_device)?;
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| device.to_string());
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, events, status, bank),
        SampleFormat::I16 => build::<i16>(&device, config, events, status, bank),
        SampleFormat::U16 => build::<u16>(&device, config, events, status, bank),
        SampleFormat::I32 => build::<i32>(&device, config, events, status, bank),
        other => return Err(ozan_ui::i18n::fill(ozan_ui::i18n::s().sample_format, &[&format!("{other:?}")])),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(AudioOutput { _stream: stream, sample_rate: config.sample_rate, name })
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut events: rtrb::Consumer<Event>,
    status: Arc<Status>,
    bank: Arc<WavetableBank>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    let mut engine = Engine::with_bank(config.sample_rate as f32, bank);
    // Grown on the first callback only, then reused.
    let mut left: Vec<f32> = Vec::with_capacity(8192);
    let mut right: Vec<f32> = Vec::with_capacity(8192);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                while let Ok(event) = events.pop() {
                    engine.handle(event);
                }
                let frames = data.len() / channels;
                if left.len() < frames {
                    left.resize(frames, 0.0);
                    right.resize(frames, 0.0);
                }
                let (l, r) = (&mut left[..frames], &mut right[..frames]);
                engine.render_stereo(l, r);
                let mut peak = 0.0f32;
                for (i, frame) in data.chunks_exact_mut(channels).enumerate() {
                    let (a, b) = (l[i].clamp(-1.0, 1.0), r[i].clamp(-1.0, 1.0));
                    peak = peak.max(a.abs()).max(b.abs());
                    match frame {
                        [mono] => *mono = T::from_sample(0.5 * (a + b)),
                        [fl, fr, rest @ ..] => {
                            *fl = T::from_sample(a);
                            *fr = T::from_sample(b);
                            // Surround channels stay silent.
                            rest.fill(T::from_sample(0.0));
                        }
                        [] => {}
                    }
                }
                status.report_audio(engine.active_voices(), peak, engine.held_notes(), engine.lfo_value());
            },
            |e| eprintln!("audio: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
