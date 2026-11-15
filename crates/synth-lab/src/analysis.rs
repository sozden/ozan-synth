//! Measurements on rendered or recorded audio, and spectrogram pictures.

use std::path::Path;

use rustfft::{num_complex::Complex, FftPlanner};

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

pub fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}

/// Second difference: large where the waveform bends sharply (clicks, steps).
pub fn second_difference(x: &[f32]) -> Vec<f32> {
    let mut d = vec![0.0; x.len()];
    for n in 2..x.len() {
        d[n] = (x[n] - 2.0 * x[n - 1] + x[n - 2]).abs();
    }
    d
}

/// Value below which `q` (0..1) of the values lie.
pub fn quantile(values: &[f32], q: f32) -> f32 {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q) as usize]
}

/// How much sharper the waveform bends right at `at` (sample index, ±2 ms) than
/// it usually does: the "click score". Around 1–2 is normal; above ~4 is a
/// candidate for an audible click.
pub fn click_score(d2: &[f32], typical: f32, at: usize, sample_rate: f32) -> f32 {
    let w = (0.002 * sample_rate) as usize;
    let lo = at.saturating_sub(w);
    let hi = (at + w).min(d2.len());
    if lo >= hi || typical <= 0.0 {
        return 0.0;
    }
    d2[lo..hi].iter().fold(0.0f32, |m, v| m.max(*v)) / typical
}

/// Level per 5 ms block, in dB. Shorter blocks swing with the beating of chords.
pub fn envelope_db(x: &[f32], sample_rate: f32) -> Vec<f32> {
    let n = (sample_rate * BLOCK_MS / 1000.0) as usize;
    x.chunks(n).map(|c| db(rms(c))).collect()
}

pub const BLOCK_MS: f32 = 5.0;

/// Short sudden drops in level while sound is playing: a block more than 30 dB
/// below the one before it, with the level back within 60 ms. Returns the times
/// in milliseconds.
pub fn dropouts(env_db: &[f32]) -> Vec<usize> {
    let back_within = (60.0 / BLOCK_MS) as usize;
    let mut found = Vec::new();
    let mut i = 1;
    while i + back_within < env_db.len() {
        let before = env_db[i - 1];
        if before > -50.0 && env_db[i] < before - 30.0 {
            if let Some(back) = (i + 1..i + back_within).find(|&j| env_db[j] > before - 6.0) {
                found.push((i as f32 * BLOCK_MS) as usize);
                i = back;
                continue;
            }
        }
        i += 1;
    }
    found
}

/// Reads a 32-bit float WAV as written by [`write_wav`]; several channels are mixed.
pub fn read_wav(path: &Path) -> std::io::Result<(Vec<f32>, u32)> {
    let d = std::fs::read(path)?;
    if d.len() < 44 || &d[0..4] != b"RIFF" || u16::from_le_bytes([d[20], d[21]]) != 3 {
        return Err(std::io::Error::other("32-bit float WAV bekleniyor"));
    }
    let channels = u16::from_le_bytes([d[22], d[23]]).max(1) as usize;
    let rate = u32::from_le_bytes([d[24], d[25], d[26], d[27]]);
    let samples: Vec<f32> = d[44..].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let x = samples.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect();
    Ok((x, rate))
}

/// Writes 32-bit float WAV; `channels` are interleaved (all the same length).
pub fn write_wav(path: &Path, channels: &[&[f32]], sample_rate: u32) -> std::io::Result<()> {
    let n = channels.len().max(1);
    let frames = channels.first().map_or(0, |c| c.len());
    let data_len = (frames * n * 4) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&(n as u16).to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 4 * n as u32).to_le_bytes());
    out.extend_from_slice(&(4 * n as u16).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        for c in channels {
            out.extend_from_slice(&c[i].to_le_bytes());
        }
    }
    std::fs::write(path, out)
}

/// Correlation of left and right: 1 = the same (mono), 0 = unrelated (wide).
pub fn correlation(l: &[f32], r: &[f32]) -> f32 {
    let lr: f32 = l.iter().zip(r).map(|(a, b)| a * b).sum();
    let ll: f32 = l.iter().map(|a| a * a).sum();
    let rr: f32 = r.iter().map(|b| b * b).sum();
    if ll == 0.0 || rr == 0.0 { 1.0 } else { lr / (ll * rr).sqrt() }
}

/// Spectrogram as a PNG: time left to right, frequency (linear, 0 to Nyquist)
/// bottom to top, level from black through purple and orange to white over 90 dB.
/// `marks` are times (s) drawn as thin grey lines.
pub fn spectrogram_png(path: &Path, x: &[f32], sample_rate: f32, marks: &[f32]) -> std::io::Result<()> {
    const N: usize = 2048;
    const HOP: usize = 256;
    const HEIGHT: usize = 360;
    let frames = x.len().saturating_sub(N) / HOP + 1;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let window: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos()).collect();
    let mut img = vec![0u8; frames * HEIGHT * 3];
    let mut buf = vec![Complex::new(0.0f32, 0.0); N];
    for f in 0..frames {
        for i in 0..N {
            buf[i] = Complex::new(x.get(f * HOP + i).copied().unwrap_or(0.0) * window[i], 0.0);
        }
        fft.process(&mut buf);
        for row in 0..HEIGHT {
            // Each image row covers a band of FFT bins; take the loudest.
            let b0 = row * (N / 2) / HEIGHT;
            let b1 = ((row + 1) * (N / 2) / HEIGHT).max(b0 + 1);
            let mag = buf[b0..b1].iter().map(|c| c.norm()).fold(0.0f32, f32::max) / (N as f32 / 4.0);
            let t = ((db(mag) + 90.0) / 90.0).clamp(0.0, 1.0);
            let (r, g, b) = heat(t);
            let y = HEIGHT - 1 - row;
            let o = (y * frames + f) * 3;
            img[o..o + 3].copy_from_slice(&[r, g, b]);
        }
    }
    for &m in marks {
        let f = (m * sample_rate) as usize / HOP;
        if f < frames {
            for y in (0..HEIGHT).step_by(3) {
                let o = (y * frames + f) * 3;
                img[o..o + 3].copy_from_slice(&[90, 90, 90]);
            }
        }
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, frames as u32, HEIGHT as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    w.write_image_data(&img).map_err(std::io::Error::other)?;
    Ok(())
}

fn heat(t: f32) -> (u8, u8, u8) {
    let stops = [(0.0, [0.0, 0.0, 0.0]), (0.35, [60.0, 20.0, 110.0]), (0.65, [220.0, 90.0, 30.0]), (0.85, [250.0, 200.0, 60.0]), (1.0, [255.0, 255.0, 255.0])];
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let k = (t - t0) / (t1 - t0);
            let c = |i: usize| (c0[i] + (c1[i] - c0[i]) * k) as u8;
            return (c(0), c(1), c(2));
        }
    }
    (255, 255, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn tone(freqs: &[f32], seconds: f32) -> Vec<f32> {
        (0..(seconds * SR) as usize)
            .map(|i| freqs.iter().map(|f| (std::f32::consts::TAU * f * i as f32 / SR).sin()).sum::<f32>() * 0.2)
            .collect()
    }

    #[test]
    fn finds_a_short_gap() {
        let mut x = tone(&[440.0], 1.0);
        let at = (0.5 * SR) as usize;
        x[at..at + (0.006 * SR) as usize].fill(0.0);
        let found = dropouts(&envelope_db(&x, SR));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!((found[0] as i32 - 500).abs() <= 5);
    }

    #[test]
    fn beating_chord_is_not_a_dropout() {
        // Low C major chord: its level swings strongly over a few milliseconds.
        let x = tone(&[130.8, 164.8, 196.0, 261.6], 2.0);
        assert!(dropouts(&envelope_db(&x, SR)).is_empty());
    }
}
