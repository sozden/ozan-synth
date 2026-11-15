//! Listening lab for the synth.
//!
//! `ozan-lab render <folder>`: plays a fixed test phrase through every factory
//! preset with the real engine, writes a WAV and a spectrogram per preset and
//! prints measurements (level, DC, clicks at note starts/ends, silence after).
//!
//! `ozan-lab loopback <seconds> <file.wav>`: records what the computer is playing
//! (WASAPI loopback of the default output) and looks for dropouts and clicks.

mod analysis;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use analysis::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ozan_core::{Engine, Event, Patch};

const SR: f32 = 48_000.0;

/// What happens in the test phrase, and when (seconds).
fn phrase() -> Vec<(f32, Event, &'static str)> {
    let on = |note| Event::NoteOn { note, velocity: 100 };
    let off = |note| Event::NoteOff { note };
    let mut v = vec![
        (0.0, on(48), "C3 on"),
        (1.0, off(48), "C3 off"),
        (1.2, on(60), "chord on"),
        (1.2, on(64), ""),
        (1.2, on(67), ""),
        (2.7, off(60), "chord off"),
        (2.7, off(64), ""),
        (2.7, off(67), ""),
        (3.0, on(72), "C5, wheel up"),
    ];
    for i in 0..=100 {
        v.push((3.0 + i as f32 * 0.01, Event::ModWheel(i as f32 / 100.0), ""));
    }
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        v.push((4.0 + t, Event::PitchBend((t * std::f32::consts::TAU).sin()), ""));
    }
    v.push((4.0, Event::ModWheel(0.0), "bend swing"));
    v.push((5.2, off(72), "C5 off"));
    for k in 0..8 {
        let t = 5.6 + k as f32 * 0.12;
        v.push((t, on(55), if k == 0 { "fast repeats" } else { "" }));
        v.push((t + 0.08, off(55), ""));
    }
    v.push((6.8, on(96), "C7 (aliasing check)"));
    v.push((7.6, off(96), "C7 off"));
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

const LENGTH: f32 = 12.0;

/// Loudness the factory presets are matched to (dB RMS while notes play).
const TARGET_DB: f32 = -25.0;

/// Left, right and their mix.
fn render(patch: &Patch) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut engine = Engine::new(SR);
    engine.handle(Event::Patch(*patch));
    let total = (LENGTH * SR) as usize;
    let mut l = vec![0.0f32; total];
    let mut r = vec![0.0f32; total];
    let mut pos = 0;
    for (t, event, _) in phrase() {
        let at = ((t * SR) as usize).min(total);
        if at > pos {
            engine.render_stereo(&mut l[pos..at], &mut r[pos..at]);
            pos = at;
        }
        engine.handle(event);
    }
    engine.render_stereo(&mut l[pos..], &mut r[pos..]);
    let mid = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    (l, r, mid)
}

fn render_all(dir: PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(&dir)?;
    let events = phrase();
    let note_times: Vec<(f32, &str)> = events
        .iter()
        .filter(|(_, e, _)| matches!(e, Event::NoteOn { .. } | Event::NoteOff { .. }))
        .map(|(t, e, _)| (*t, if matches!(e, Event::NoteOn { .. }) { "on" } else { "off" }))
        .collect();
    let marks: Vec<f32> = events.iter().filter(|(_, _, l)| !l.is_empty()).map(|(t, _, _)| *t).collect();
    println!("{:<14} {:>7} {:>7} {:>8} {:>7} {:>8} {:>6} {:>9} {:>6}  worst click", "preset", "peak dB", "rms dB", "DC", "tail", "C7 HF dB", "L/R", "click", "vol→");
    for (i, (name, patch)) in ozan_core::presets::factory().into_iter().enumerate() {
        let (left, right, x) = render(&patch);
        let slug: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        let base = dir.join(format!("{:02}_{slug}", i + 1));
        write_wav(&base.with_extension("wav"), &[&left, &right], SR as u32)?;
        spectrogram_png(&base.with_extension("png"), &x, SR, &marks)?;

        let nan = x.iter().any(|v| !v.is_finite());
        // Loudness while notes play; the same window for every preset, so tails
        // (reverb, release) do not drag it down.
        let loud = db(rms(&x[..(7.6 * SR) as usize]));
        let suggested = (patch.volume * 10f32.powf((TARGET_DB - loud) / 20.0)).min(1.0);
        let dc = x.iter().sum::<f32>() / x.len() as f32;
        let tail = rms(&x[(11.5 * SR) as usize..]);
        let width = correlation(&left, &right);
        let d2 = second_difference(&x);
        let typical = quantile(&d2, 0.999);
        // Steps and clicks show up as energy in the second difference; C7's own
        // harmonics are few, so this segment shows them best.
        let hf = db(rms(&d2[(6.9 * SR) as usize..(7.5 * SR) as usize]));
        let (worst, at) = note_times
            .iter()
            .map(|&(t, kind)| (click_score(&d2, typical, (t * SR) as usize, SR), (t, kind)))
            .fold((0.0f32, (0.0, "")), |a, b| if b.0 > a.0 { b } else { a });
        println!(
            "{:<14} {:>7.1} {:>7.1} {:>8.5} {:>7.1} {:>8.1} {:>6.2} {:>9.2} {:>6.2}  {:.2} s note-{}{}",
            name,
            db(peak(&x)),
            loud,
            dc,
            db(tail),
            hf,
            width,
            worst,
            suggested,
            at.0,
            at.1,
            if nan { "  NaN!" } else { "" }
        );
    }
    println!("\nWAVs and spectrograms: {}", dir.display());
    Ok(())
}

fn loopback(seconds: f32, path: PathBuf) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("unexpected sample format {:?}", config.sample_format()));
    }
    let channels = config.channels() as usize;
    let rate = config.sample_rate();
    let recorded = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sink = recorded.clone();
    // An output device opened as input records what it plays (WASAPI loopback).
    let stream = device
        .build_input_stream(
            config.config(),
            move |data: &[f32], _| {
                let mut r = sink.lock().unwrap();
                r.extend(data.chunks_exact(channels).map(|f| f[0]));
            },
            |e| eprintln!("recording: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    println!("recording: {seconds} s @ {rate} Hz");
    std::thread::sleep(std::time::Duration::from_secs_f32(seconds));
    drop(stream);
    let x = recorded.lock().unwrap().clone();
    write_wav(&path, &[&x], rate).map_err(|e| e.to_string())?;
    spectrogram_png(&path.with_extension("png"), &x, rate as f32, &[]).map_err(|e| e.to_string())?;

    let expected = (seconds * rate as f32) as usize;
    println!("samples: {} (expected ~{expected})", x.len());
    report(&x, rate);
    println!("WAV: {}", path.display());
    Ok(())
}

/// Measurements on a recording of the real output.
fn report(x: &[f32], rate: u32) {
    let sr = rate as f32;
    println!("peak {:.1} dB, rms {:.1} dB", db(peak(x)), db(rms(x)));
    let env = envelope_db(x, sr);
    let drops = dropouts(&env);
    println!("dropouts (sudden 30 dB drop, back within 60 ms): {}", if drops.is_empty() { "none".to_string() } else { format!("{}, at ms: {:?}", drops.len(), &drops[..drops.len().min(20)]) });
    // WASAPI loopback leaves gaps (zeros) where the device played nothing; while
    // sound is on, a run of exact zeros is a glitch.
    let mut zero_runs = 0;
    let mut run = 0;
    for (i, v) in x.iter().enumerate() {
        if *v == 0.0 {
            run += 1;
        } else {
            if run >= 16 && i > run && x[i - run - 1].abs() > 1e-3 && v.abs() > 1e-3 {
                zero_runs += 1;
            }
            run = 0;
        }
    }
    println!("runs of zeros inside sound (≥16 samples): {zero_runs}");
    let d2 = second_difference(x);
    let typical = quantile(&d2, 0.999);
    let spikes = d2.iter().filter(|v| **v > typical * 6.0).count();
    println!("sharp jumps (6× the typical top 0.1%): {spikes}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("render") => render_all(PathBuf::from(args.get(2).map_or("lab-out", |s| s))).map_err(|e| e.to_string()),
        Some("steps") => {
            // Sharpness around LFO steps: per-ms peak of the second difference in the
            // C7 part of a preset, as max / median.
            let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let (name, patch) = ozan_core::presets::factory().swap_remove(index - 1);
            let (_, _, x) = render(&patch);
            let d2 = second_difference(&x[(6.9 * SR) as usize..(7.5 * SR) as usize]);
            let per_ms: Vec<f32> = d2.chunks(48).map(|c| c.iter().fold(0.0f32, |m, v| m.max(*v))).collect();
            let median = quantile(&per_ms, 0.5);
            let worst = quantile(&per_ms, 1.0);
            println!("{name}: sharpness per ms, worst/median = {:.2}  (99th percentile/median = {:.2})", worst / median, quantile(&per_ms, 0.99) / median);
            Ok(())
        }
        Some("analyze") => {
            let path = PathBuf::from(args.get(2).map_or("loopback.wav", |s| s));
            read_wav(&path).map(|(x, rate)| report(&x, rate)).map_err(|e| e.to_string())
        }
        Some("loopback") => {
            let secs = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5.0);
            loopback(secs, PathBuf::from(args.get(3).map_or("loopback.wav", |s| s)))
        }
        _ => Err("usage: ozan-lab render <folder> | loopback <seconds> <file.wav> | analyze <file.wav> | steps <preset number>".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
