//! Wavetables: a row of single-cycle waves (frames) that the oscillator morphs
//! through with its position knob.
//!
//! Every frame is stored once per octave of pitch ("mip levels"), each copy with
//! only as many harmonics as fit below half the sample rate at that pitch. The
//! oscillator picks the copy for the note it plays, so even high notes do not alias.
//! All tables are computed from formulas when the engine starts; no sample files.

use std::f64::consts::TAU;

use crate::fft::{fft, Complex};

/// Samples per frame.
pub const SIZE: usize = 2048;
/// Frames per table.
pub const FRAMES: usize = 32;
/// Mip levels: level `l` keeps harmonics up to `1024 >> l`.
pub const LEVELS: usize = 11;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WavetableKind {
    /// Sine → triangle → saw → square.
    #[default]
    Basic,
    /// A square narrowing to a thin pulse.
    Pulse,
    /// Harmonics added one after another, like an organ's drawbars opening.
    Harmonics,
    /// Vowels a-e-i-o-u from formant peaks.
    Vowels,
    /// Hard-sync sweep: a saw restarted by a slower master, ratio 1 → 8.
    Sync,
}

impl WavetableKind {
    pub const ALL: [WavetableKind; 5] =
        [WavetableKind::Basic, WavetableKind::Pulse, WavetableKind::Harmonics, WavetableKind::Vowels, WavetableKind::Sync];

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// Highest harmonic kept at mip level `l`.
pub fn max_harmonic(level: usize) -> usize {
    (1024 >> level).min(SIZE / 2 - 1)
}

/// One table: `FRAMES × LEVELS × SIZE` samples.
pub struct Wavetable {
    data: Vec<f32>,
}

impl Wavetable {
    fn at(&self, frame: usize, level: usize) -> &[f32] {
        let start = (frame * LEVELS + level) * SIZE;
        &self.data[start..start + SIZE]
    }

    /// The level to use for a phase increment `dt` (frequency / sample rate): the
    /// most harmonics whose highest still stays below half the sample rate.
    #[inline]
    pub fn level_for(dt: f32) -> usize {
        let allowed = 0.5 / dt.max(1e-9);
        (0..LEVELS).find(|&l| max_harmonic(l) as f32 <= allowed).unwrap_or(LEVELS - 1)
    }

    /// Sample at `phase` (0..1) and `position` (0..1 across the frames).
    #[inline]
    pub fn sample(&self, phase: f32, position: f32, level: usize) -> f32 {
        let fpos = position.clamp(0.0, 1.0) * (FRAMES - 1) as f32;
        let f0 = (fpos as usize).min(FRAMES - 2);
        let ff = fpos - f0 as f32;
        let spos = phase * SIZE as f32;
        let i0 = (spos as usize) % SIZE;
        let i1 = (i0 + 1) % SIZE;
        let si = spos - spos.floor();
        let read = |frame: &[f32]| frame[i0] + (frame[i1] - frame[i0]) * si;
        let a = read(self.at(f0, level));
        let b = read(self.at(f0 + 1, level));
        a + (b - a) * ff
    }

    /// One frame at full bandwidth, for drawing.
    pub fn frame(&self, position: f32) -> &[f32] {
        let f = (position.clamp(0.0, 1.0) * (FRAMES - 1) as f32).round() as usize;
        self.at(f, 0)
    }
}

/// All tables, shared (read only) by the voices.
pub struct WavetableBank {
    tables: Vec<Wavetable>,
}

impl WavetableBank {
    pub fn new() -> Self {
        Self { tables: WavetableKind::ALL.iter().map(|k| build(*k)).collect() }
    }

    pub fn get(&self, kind: WavetableKind) -> &Wavetable {
        &self.tables[kind.index()]
    }
}

impl Default for WavetableBank {
    fn default() -> Self {
        Self::new()
    }
}

/// One cycle of `kind` at morph point `t` (0..1), not yet band-limited.
fn raw_frame(kind: WavetableKind, t: f64) -> Vec<f64> {
    let n = SIZE;
    let ph = |i: usize| i as f64 / n as f64;
    match kind {
        WavetableKind::Basic => {
            let sine = |p: f64| (TAU * p).sin();
            let tri = |p: f64| {
                let q = (p + 0.25).fract();
                if q < 0.5 { 4.0 * q - 1.0 } else { 3.0 - 4.0 * q }
            };
            let saw = |p: f64| 2.0 * (p + 0.5).fract() - 1.0;
            let square = |p: f64| if p < 0.5 { 1.0 } else { -1.0 };
            let seg = t * 3.0;
            (0..n)
                .map(|i| {
                    let p = ph(i);
                    let (a, b, k) = match seg {
                        s if s < 1.0 => (sine(p), tri(p), s),
                        s if s < 2.0 => (tri(p), saw(p), s - 1.0),
                        s => (saw(p), square(p), (s - 2.0).min(1.0)),
                    };
                    a + (b - a) * k
                })
                .collect()
        }
        WavetableKind::Pulse => {
            let width = 0.5 - 0.45 * t;
            (0..n).map(|i| if ph(i) < width { 1.0 } else { -1.0 }).collect()
        }
        WavetableKind::Harmonics => {
            // Harmonics with saw-like weights switched in one by one (soft edge).
            let count = 1.0 + t * 39.0;
            (0..n)
                .map(|i| {
                    (1..=40)
                        .map(|h| {
                            let gate = (count - h as f64 + 1.0).clamp(0.0, 1.0);
                            gate / h as f64 * (TAU * h as f64 * ph(i)).sin()
                        })
                        .sum()
                })
                .collect()
        }
        WavetableKind::Vowels => {
            // Formant frequencies (Hz) for a, e, i, o, u, sounded on a 130 Hz
            // fundamental; the table keeps their shape at any pitch.
            const VOWELS: [[f64; 3]; 5] =
                [[730.0, 1090.0, 2440.0], [530.0, 1840.0, 2480.0], [270.0, 2290.0, 3010.0], [570.0, 840.0, 2410.0], [300.0, 870.0, 2240.0]];
            const WIDTH: [f64; 3] = [90.0, 110.0, 160.0];
            const GAIN: [f64; 3] = [1.0, 0.6, 0.3];
            let pos = t * 4.0;
            let v0 = (pos as usize).min(3);
            let k = pos - v0 as f64;
            let formants: Vec<f64> = (0..3).map(|j| VOWELS[v0][j] + (VOWELS[v0 + 1][j] - VOWELS[v0][j]) * k).collect();
            let f0 = 130.0;
            let amps: Vec<f64> = (1..=48)
                .map(|h| {
                    let f = f0 * h as f64;
                    (0..3).map(|j| GAIN[j] * (-((f - formants[j]) / WIDTH[j]).powi(2)).exp()).sum::<f64>() / (h as f64).sqrt()
                })
                .collect();
            (0..n)
                .map(|i| amps.iter().enumerate().map(|(h, a)| a * (TAU * (h + 1) as f64 * ph(i)).sin()).sum())
                .collect()
        }
        WavetableKind::Sync => {
            let ratio = 1.0 + 7.0 * t;
            (0..n).map(|i| 2.0 * (ph(i) * ratio).fract() - 1.0).collect()
        }
    }
}

fn build(kind: WavetableKind) -> Wavetable {
    let mut data = vec![0.0f32; FRAMES * LEVELS * SIZE];
    let mut spectrum = vec![Complex::default(); SIZE];
    let mut level_buf = vec![Complex::default(); SIZE];
    for f in 0..FRAMES {
        let raw = raw_frame(kind, f as f64 / (FRAMES - 1) as f64);
        for (c, v) in spectrum.iter_mut().zip(&raw) {
            *c = Complex::new(*v, 0.0);
        }
        fft(&mut spectrum, false);
        spectrum[0] = Complex::default(); // no DC
        let mut scale = 0.0;
        for level in 0..LEVELS {
            let keep = max_harmonic(level);
            level_buf.fill(Complex::default());
            for h in 1..=keep {
                level_buf[h] = spectrum[h];
                level_buf[SIZE - h] = spectrum[SIZE - h];
            }
            fft(&mut level_buf, true);
            if level == 0 {
                // One factor for all levels of a frame, so switching levels as the
                // pitch moves does not change the loudness.
                let peak = level_buf.iter().fold(0.0f64, |m, c| m.max(c.re.abs()));
                scale = if peak > 0.0 { 1.0 / peak } else { 0.0 };
            }
            let start = (f * LEVELS + level) * SIZE;
            for (d, c) in data[start..start + SIZE].iter_mut().zip(&level_buf) {
                *d = (c.re * scale) as f32;
            }
        }
    }
    Wavetable { data }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harmonic_energy(x: &[f32], from: usize) -> f64 {
        let mut s: Vec<Complex> = x.iter().map(|v| Complex::new(*v as f64, 0.0)).collect();
        fft(&mut s, false);
        s[from..SIZE / 2].iter().map(|c| c.norm().powi(2)).sum()
    }

    #[test]
    fn levels_hold_only_their_harmonics() {
        let bank = WavetableBank::new();
        for kind in WavetableKind::ALL {
            let t = bank.get(kind);
            for level in [0, 4, 8] {
                let frame = t.at(FRAMES - 1, level);
                let total = harmonic_energy(frame, 1);
                let above = harmonic_energy(frame, max_harmonic(level) + 1);
                assert!(above <= total * 1e-9, "{kind:?} level {level}: {above} of {total}");
            }
        }
    }

    #[test]
    fn frames_are_normalised_and_basic_starts_as_sine() {
        let bank = WavetableBank::new();
        for kind in WavetableKind::ALL {
            for f in 0..FRAMES {
                let peak = bank.get(kind).at(f, 0).iter().fold(0.0f32, |m, v| m.max(v.abs()));
                assert!((peak - 1.0).abs() < 1e-3, "{kind:?} frame {f}: {peak}");
            }
        }
        let sine = bank.get(WavetableKind::Basic).at(0, 0);
        for (i, v) in sine.iter().enumerate() {
            let expect = (TAU * i as f64 / SIZE as f64).sin() as f32;
            assert!((v - expect).abs() < 1e-3);
        }
    }

    #[test]
    fn level_choice_keeps_harmonics_below_nyquist() {
        for freq in [20.0f32, 110.0, 440.0, 2000.0, 8000.0] {
            let dt = freq / 48_000.0;
            let l = Wavetable::level_for(dt);
            assert!(max_harmonic(l) as f32 * freq <= 24_000.0 || l == LEVELS - 1, "{freq}");
            if l > 0 {
                // and not needlessly dull: the level above would alias.
                assert!(max_harmonic(l - 1) as f32 * freq > 24_000.0, "{freq}");
            }
        }
    }
}
