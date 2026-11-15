//! Listening lab for the synth.
//!
//! `synth-lab render <folder>`: plays a fixed test phrase through every factory
//! preset with the real engine, writes a WAV and a spectrogram per preset and
//! prints measurements (level, DC, clicks at note starts/ends, silence after).
//!
//! `synth-lab loopback <seconds> <file.wav>`: records what the computer is playing
//! (WASAPI loopback of the default output) and looks for dropouts and clicks.

mod analysis;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use analysis::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use synth_core::{Engine, Event, Patch};

const SR: f32 = 48_000.0;

/// What happens in the test phrase, and when (seconds).
fn phrase() -> Vec<(f32, Event, &'static str)> {
    let on = |note| Event::NoteOn { note, velocity: 100 };
    let off = |note| Event::NoteOff { note };
    let mut v = vec![
        (0.0, on(48), "C3 başlar"),
        (1.0, off(48), "C3 biter"),
        (1.2, on(60), "akor başlar"),
        (1.2, on(64), ""),
        (1.2, on(67), ""),
        (2.7, off(60), "akor biter"),
        (2.7, off(64), ""),
        (2.7, off(67), ""),
        (3.0, on(72), "C5, tekerlek açılıyor"),
    ];
    for i in 0..=100 {
        v.push((3.0 + i as f32 * 0.01, Event::ModWheel(i as f32 / 100.0), ""));
    }
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        v.push((4.0 + t, Event::PitchBend((t * std::f32::consts::TAU).sin()), ""));
    }
    v.push((4.0, Event::ModWheel(0.0), "bend salınımı"));
    v.push((5.2, off(72), "C5 biter"));
    for k in 0..8 {
        let t = 5.6 + k as f32 * 0.12;
        v.push((t, on(55), if k == 0 { "hızlı tekrar" } else { "" }));
        v.push((t + 0.08, off(55), ""));
    }
    v.push((6.8, on(96), "C7 (aliasing kontrolü)"));
    v.push((7.6, off(96), "C7 biter"));
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

const LENGTH: f32 = 10.5;

fn render(patch: &Patch) -> Vec<f32> {
    let mut engine = Engine::new(SR);
    engine.handle(Event::Patch(*patch));
    let total = (LENGTH * SR) as usize;
    let mut out = vec![0.0f32; total];
    let mut pos = 0;
    for (t, event, _) in phrase() {
        let at = ((t * SR) as usize).min(total);
        if at > pos {
            engine.render(&mut out[pos..at]);
            pos = at;
        }
        engine.handle(event);
    }
    engine.render(&mut out[pos..]);
    out
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
    println!("{:<14} {:>7} {:>7} {:>8} {:>7} {:>8} {:>9}  en kötü tık", "preset", "tepe dB", "rms dB", "DC", "kuyruk", "C7 HF dB", "tık skoru");
    for (i, (name, patch)) in synth_core::presets::factory().into_iter().enumerate() {
        let x = render(&patch);
        let slug: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        let base = dir.join(format!("{:02}_{slug}", i + 1));
        write_wav(&base.with_extension("wav"), &x, SR as u32)?;
        spectrogram_png(&base.with_extension("png"), &x, SR, &marks)?;

        let nan = x.iter().any(|v| !v.is_finite());
        let active: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-4).collect();
        let dc = x.iter().sum::<f32>() / x.len() as f32;
        let tail = rms(&x[(10.0 * SR) as usize..]);
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
            "{:<14} {:>7.1} {:>7.1} {:>8.5} {:>7.1} {:>8.1} {:>9.2}  {:.2} s note-{}{}",
            name,
            db(peak(&x)),
            db(rms(&active)),
            dc,
            db(tail),
            hf,
            worst,
            at.0,
            at.1,
            if nan { "  NaN!" } else { "" }
        );
    }
    println!("\nWAV ve spektrogramlar: {}", dir.display());
    Ok(())
}

fn loopback(seconds: f32, path: PathBuf) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("çıkış aygıtı yok")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("beklenmeyen örnek biçimi {:?}", config.sample_format()));
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
            |e| eprintln!("kayıt: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    println!("kaydediliyor: {seconds} s @ {rate} Hz");
    std::thread::sleep(std::time::Duration::from_secs_f32(seconds));
    drop(stream);
    let x = recorded.lock().unwrap().clone();
    write_wav(&path, &x, rate).map_err(|e| e.to_string())?;
    spectrogram_png(&path.with_extension("png"), &x, rate as f32, &[]).map_err(|e| e.to_string())?;

    let expected = (seconds * rate as f32) as usize;
    println!("örnek: {} (beklenen ~{expected})", x.len());
    report(&x, rate);
    println!("WAV: {}", path.display());
    Ok(())
}

/// Measurements on a recording of the real output.
fn report(x: &[f32], rate: u32) {
    let sr = rate as f32;
    println!("tepe {:.1} dB, rms {:.1} dB", db(peak(x)), db(rms(x)));
    let env = envelope_db(x, sr);
    let drops = dropouts(&env);
    println!("kesinti (30 dB ani düşüş, 60 ms içinde dönen): {}", if drops.is_empty() { "yok".to_string() } else { format!("{} adet, ms: {:?}", drops.len(), &drops[..drops.len().min(20)]) });
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
    println!("ses ortasında sıfır boşluğu (≥16 örnek): {zero_runs}");
    let d2 = second_difference(x);
    let typical = quantile(&d2, 0.999);
    let spikes = d2.iter().filter(|v| **v > typical * 6.0).count();
    println!("keskin sıçrama (tipik 0.1% üst değerin 6 katı): {spikes}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("render") => render_all(PathBuf::from(args.get(2).map_or("lab-out", |s| s))).map_err(|e| e.to_string()),
        Some("steps") => {
            // Sharpness around LFO steps: per-ms peak of the second difference in the
            // C7 part of a preset, as max / median.
            let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let (name, patch) = synth_core::presets::factory().swap_remove(index - 1);
            let x = render(&patch);
            let d2 = second_difference(&x[(6.9 * SR) as usize..(7.5 * SR) as usize]);
            let per_ms: Vec<f32> = d2.chunks(48).map(|c| c.iter().fold(0.0f32, |m, v| m.max(*v))).collect();
            let median = quantile(&per_ms, 0.5);
            let worst = quantile(&per_ms, 1.0);
            println!("{name}: ms başına keskinlik en kötü/ortanca = {:.2}  (99. yüzdelik/ortanca = {:.2})", worst / median, quantile(&per_ms, 0.99) / median);
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
        _ => Err("kullanım: synth-lab render <klasör> | loopback <saniye> <dosya.wav> | analyze <dosya.wav> | steps <preset no>".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
