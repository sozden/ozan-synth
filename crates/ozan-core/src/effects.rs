//! Effects after the voices: distortion → phaser → chorus → delay → reverb → EQ,
//! stereo. The distortion, phaser and EQ are skipped entirely while off.
//!
//! All buffers are made in [`Effects::new`]; processing does not allocate.

use std::f32::consts::TAU;

use crate::patch::{ChorusParams, DelayParams, DistortionParams, DriveMode, EffectsParams, EqParams, PhaserParams, ReverbParams};

/// Longest delay time, in seconds.
pub const MAX_DELAY: f32 = 2.0;

/// One-pole smoothing factor for a time constant of `seconds`.
fn smoothing(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate).max(1.0)).exp()
}

/// One-pole low-pass factor for `hz`.
fn lowpass(hz: f32, sample_rate: f32) -> f32 {
    1.0 - (-TAU * hz.min(sample_rate * 0.45) / sample_rate).exp()
}

/// One-pole high-pass (DC blocker): keeps sub-bass and offset out of feedback
/// loops, which would otherwise build them up for as long as the tail lasts.
#[derive(Clone, Copy, Debug, Default)]
struct HighPass {
    x1: f32,
    y1: f32,
}

impl HighPass {
    #[inline]
    fn process(&mut self, x: f32, hz: f32, sample_rate: f32) -> f32 {
        let r = 1.0 - TAU * hz / sample_rate;
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// Circular buffer with fractional (linear) read.
#[derive(Clone, Debug)]
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(4)], pos: 0 }
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
    }

    /// Sample written `delay` samples ago (1 ≤ delay < len).
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let d = delay.clamp(1.0, (len - 2) as f32);
        let whole = d as usize;
        let frac = d - whole as f32;
        let a = self.buf[(self.pos + len - whole) % len];
        let b = self.buf[(self.pos + len - whole - 1) % len];
        a + (b - a) * frac
    }

    #[inline]
    fn write(&mut self, v: f32) {
        self.pos = (self.pos + 1) % self.buf.len();
        self.buf[self.pos] = v;
    }
}

/// Stereo chorus: two delay taps around 10 ms swept by a sine, a quarter cycle
/// apart left and right.
#[derive(Clone, Debug)]
pub struct Chorus {
    line: Line,
    phase: f32,
}

impl Chorus {
    fn new(sample_rate: f32) -> Self {
        Self { line: Line::new((0.03 * sample_rate) as usize), phase: 0.0 }
    }

    #[inline]
    fn process(&mut self, dry_l: f32, dry_r: f32, p: &ChorusParams, wet: f32, sr: f32) -> (f32, f32) {
        self.line.write(0.5 * (dry_l + dry_r));
        let base = 0.010 * sr;
        let depth = p.depth.clamp(0.0, 1.0) * 0.006 * sr;
        let s = |ph: f32| 0.5 + 0.5 * (ph * TAU).sin();
        let l = self.line.read(base + depth * s(self.phase));
        let r = self.line.read(base + depth * s(self.phase + 0.25));
        self.phase = (self.phase + p.rate.max(0.0) / sr).fract();
        // At full mix half dry, half delayed, as on classic chorus units.
        let w = 0.5 * wet;
        (dry_l * (1.0 - w) + l * w, dry_r * (1.0 - w) + r * w)
    }
}

/// Stereo echo with filtered feedback; optional ping-pong between the sides.
#[derive(Clone, Debug)]
pub struct Delay {
    left: Line,
    right: Line,
    /// Delay time in samples, glides to the knob (like tape when it is turned).
    time: f32,
    tone_l: f32,
    tone_r: f32,
    dc_l: HighPass,
    dc_r: HighPass,
}

impl Delay {
    fn new(sample_rate: f32, p: &DelayParams) -> Self {
        let len = (MAX_DELAY * sample_rate) as usize + 4;
        Self { left: Line::new(len), right: Line::new(len), time: p.time_ms * 0.001 * sample_rate, tone_l: 0.0, tone_r: 0.0, dc_l: HighPass::default(), dc_r: HighPass::default() }
    }

    #[inline]
    fn process(&mut self, l: f32, r: f32, p: &DelayParams, wet: f32, sr: f32) -> (f32, f32) {
        let target = (p.time_ms * 0.001).clamp(0.001, MAX_DELAY) * sr;
        self.time += (target - self.time) * smoothing(0.08, sr);
        let dl = self.left.read(self.time);
        let dr = self.right.read(self.time);
        let c = lowpass(p.tone, sr);
        self.tone_l += (dl - self.tone_l) * c;
        self.tone_r += (dr - self.tone_r) * c;
        let fb = p.feedback.clamp(0.0, 0.95);
        let back_l = self.dc_l.process(self.tone_l, 20.0, sr) * fb;
        let back_r = self.dc_r.process(self.tone_r, 20.0, sr) * fb;
        if p.ping_pong {
            // Input enters on the left; each repeat crosses to the other side.
            self.left.write(0.5 * (l + r) + back_r);
            self.right.write(back_l);
        } else {
            self.left.write(l + back_l);
            self.right.write(r + back_r);
        }
        (l + dl * wet, r + dr * wet)
    }
}

/// Lengths (ms) of the reverb's eight feedback lines; no common factors, so their
/// echoes do not pile up on the same moments.
const LINES_MS: [f32; 8] = [31.3, 37.9, 41.3, 47.9, 53.1, 61.7, 67.3, 73.9];
/// Input diffusers (all-pass, ms), smearing the attack before the tank.
const DIFFUSERS_MS: [f32; 4] = [4.7, 3.6, 12.7, 9.3];
const MAX_SIZE: f32 = 1.5;
const MAX_PREDELAY: f32 = 0.1;

/// Feedback delay network reverb: 8 damped lines mixed by a Householder matrix.
#[derive(Clone, Debug)]
pub struct Reverb {
    predelay: Line,
    diffusers: Vec<Line>,
    lines: Vec<Line>,
    damp: [f32; 8],
    /// Smoothed length scale from the size knob.
    scale: f32,
    /// Keeps bass below ~50 Hz out of the tank, where it would boom and build up.
    input_hp: HighPass,
}

impl Reverb {
    fn new(sample_rate: f32) -> Self {
        let ms = |m: f32| (m * 0.001 * sample_rate) as usize + 4;
        Self {
            predelay: Line::new(ms(MAX_PREDELAY * 1000.0)),
            diffusers: DIFFUSERS_MS.iter().map(|&m| Line::new(ms(m))).collect(),
            lines: LINES_MS.iter().map(|&m| Line::new(ms(m * MAX_SIZE))).collect(),
            damp: [0.0; 8],
            scale: 1.0,
            input_hp: HighPass::default(),
        }
    }

    fn scale_for(size: f32) -> f32 {
        0.5 + size.clamp(0.0, 1.0) * (MAX_SIZE - 0.5)
    }

    // Parallel arrays (lines, damping state, lengths): indexing reads clearest.
    #[allow(clippy::needless_range_loop)]
    #[inline]
    fn process(&mut self, l: f32, r: f32, p: &ReverbParams, wet: f32, sr: f32) -> (f32, f32) {
        self.scale += (Self::scale_for(p.size) - self.scale) * smoothing(0.2, sr);
        self.predelay.write(self.input_hp.process(0.5 * (l + r), 50.0, sr));
        let mut x = self.predelay.read((p.predelay_ms * 0.001).clamp(0.0, MAX_PREDELAY) * sr + 1.0);
        for (d, &m) in self.diffusers.iter_mut().zip(&DIFFUSERS_MS) {
            // Schroeder all-pass, gain 0.7.
            let delayed = d.read(m * 0.001 * sr);
            let v = x + 0.7 * delayed;
            d.write(v);
            x = delayed - 0.7 * v;
        }
        // Damping: a low-pass in each line, from 12 kHz (bright) to 1.5 kHz (dark).
        let c = lowpass(12_000.0 * (1.5 / 12.0f32).powf(p.damping.clamp(0.0, 1.0)), sr);
        let decay = p.decay.max(0.1);
        let mut v = [0.0f32; 8];
        for i in 0..8 {
            let len = LINES_MS[i] * 0.001 * sr * self.scale;
            let y = self.lines[i].read(len);
            self.damp[i] += (y - self.damp[i]) * c;
            // Gain for a 60 dB drop over `decay` seconds.
            let g = 10f32.powf(-3.0 * len / (decay * sr));
            v[i] = self.damp[i] * g;
        }
        let s = v.iter().sum::<f32>() * (2.0 / 8.0);
        let (mut out_l, mut out_r) = (0.0, 0.0);
        for i in 0..8 {
            let sign = if i % 4 < 2 { 1.0 } else { -1.0 };
            self.lines[i].write(x * sign + v[i] - s);
            if i % 2 == 0 { out_l += sign * v[i] } else { out_r += sign * v[i] }
        }
        let w = 0.6 * wet;
        (l + out_l * w, r + out_r * w)
    }
}

/// The distortion: drive into a shaper, a tone low-pass, and a level that stays
/// about the same however hard it is driven.
#[derive(Clone, Debug, Default)]
pub struct Distortion {
    tone: [f32; 2],
}

impl Distortion {
    #[inline]
    fn shape(mode: DriveMode, x: f32) -> f32 {
        match mode {
            DriveMode::Soft => x.tanh(),
            DriveMode::Hard => x.clamp(-1.0, 1.0),
            DriveMode::Fold => (x * std::f32::consts::FRAC_PI_2).sin(),
        }
    }

    #[inline]
    fn process(&mut self, l: f32, r: f32, p: &DistortionParams, wet: f32, sr: f32) -> (f32, f32) {
        let gain = 10f32.powf(p.drive.clamp(0.0, 1.0) * 36.0 / 20.0);
        // Measured on synth sounds: this keeps loud and quiet settings close.
        let makeup = gain.powf(-0.55);
        let c = lowpass(p.tone.clamp(500.0, 16_000.0), sr);
        let mut out = [l, r];
        for (k, x) in out.iter_mut().enumerate() {
            let y = Self::shape(p.mode, *x * gain) * makeup;
            self.tone[k] += (y - self.tone[k]) * c;
            *x = *x * (1.0 - wet) + self.tone[k] * wet;
        }
        (out[0], out[1])
    }
}

/// Phaser stages per side.
const STAGES: usize = 6;

/// Six first-order all-passes swept by a sine (a quarter cycle apart left and
/// right), with feedback; mixed with the dry signal they cut moving notches.
#[derive(Clone, Debug, Default)]
pub struct Phaser {
    phase: f32,
    /// Last input and output of each stage, per side.
    x1: [[f32; STAGES]; 2],
    y1: [[f32; STAGES]; 2],
    last: [f32; 2],
}

impl Phaser {
    #[inline]
    fn process(&mut self, l: f32, r: f32, p: &PhaserParams, wet: f32, sr: f32) -> (f32, f32) {
        let depth = p.depth.clamp(0.0, 1.0);
        let fb = p.feedback.clamp(0.0, 0.9);
        let mut out = [l, r];
        for (k, x) in out.iter_mut().enumerate() {
            let lfo = ((self.phase + 0.25 * k as f32) * TAU).sin();
            let hz = (700.0 * (lfo * 2.5 * depth).exp2()).min(sr * 0.45);
            let t = (std::f32::consts::PI * hz / sr).tan();
            let a = (t - 1.0) / (t + 1.0);
            let mut v = *x + fb * self.last[k];
            for s in 0..STAGES {
                let y = a * v + self.x1[k][s] - a * self.y1[k][s];
                self.x1[k][s] = v;
                self.y1[k][s] = y;
                v = y;
            }
            self.last[k] = v;
            let w = 0.5 * wet;
            *x = *x * (1.0 - w) + v * w;
        }
        self.phase = (self.phase + p.rate.clamp(0.0, 20.0) / sr).fract();
        (out[0], out[1])
    }
}

/// RBJ cookbook biquad, direct form I.
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Band {
    LowShelf,
    Bell,
    HighShelf,
}

impl Biquad {
    fn set(&mut self, band: Band, hz: f32, db: f32, sr: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * hz / sr;
        let (sin, cos) = w.sin_cos();
        let (b, a0, a1, a2) = match band {
            Band::Bell => {
                let alpha = sin / (2.0 * 0.7);
                ([1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a], 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            Band::LowShelf | Band::HighShelf => {
                // Shelf slope 1.
                let k = 2.0 * a.sqrt() * sin / std::f32::consts::SQRT_2;
                let (p, m) = (a + 1.0, a - 1.0);
                if band == Band::LowShelf {
                    (
                        [a * (p - m * cos + k), 2.0 * a * (m - p * cos), a * (p - m * cos - k)],
                        p + m * cos + k,
                        -2.0 * (m + p * cos),
                        p + m * cos - k,
                    )
                } else {
                    (
                        [a * (p + m * cos + k), -2.0 * a * (m + p * cos), a * (p + m * cos - k)],
                        p - m * cos + k,
                        2.0 * (m - p * cos),
                        p - m * cos - k,
                    )
                }
            }
        };
        self.b = [b[0] / a0, b[1] / a0, b[2] / a0];
        self.a = [a1 / a0, a2 / a0];
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Where the three bands sit.
const EQ_BANDS: [(Band, f32); 3] = [(Band::LowShelf, 250.0), (Band::Bell, 1000.0), (Band::HighShelf, 4000.0)];
/// Samples between coefficient updates while a band's gain glides.
const EQ_STEP: u32 = 32;

#[derive(Clone, Debug, Default)]
pub struct Eq {
    filters: [[Biquad; 3]; 2],
    /// Gains the coefficients were made for, gliding to the knobs.
    gains: Option<[f32; 3]>,
    count: u32,
}

impl Eq {
    #[inline]
    fn process(&mut self, l: f32, r: f32, p: &EqParams, wet: f32, sr: f32) -> (f32, f32) {
        let target = [p.low, p.mid, p.high].map(|g| g.clamp(-12.0, 12.0));
        if self.count == 0 || self.gains.is_none() {
            let k = smoothing(0.03, sr / EQ_STEP as f32);
            let mut g = self.gains.unwrap_or(target);
            for (g, t) in g.iter_mut().zip(target) {
                *g += (t - *g) * k;
            }
            if self.gains != Some(g) {
                for side in &mut self.filters {
                    for (f, (&(band, hz), &db)) in side.iter_mut().zip(EQ_BANDS.iter().zip(&g)) {
                        f.set(band, hz, db, sr);
                    }
                }
                self.gains = Some(g);
            }
        }
        self.count = (self.count + 1) % EQ_STEP;
        let mut out = [l, r];
        for (x, side) in out.iter_mut().zip(&mut self.filters) {
            let y = side.iter_mut().fold(*x, |v, f| f.process(v));
            *x = *x * (1.0 - wet) + y * wet;
        }
        (out[0], out[1])
    }
}

/// The effect chain with each effect's wet level smoothed, so switching one on or
/// off fades instead of clicking.
#[derive(Clone, Debug)]
pub struct Effects {
    sample_rate: f32,
    chorus: Chorus,
    delay: Delay,
    reverb: Reverb,
    distortion: Distortion,
    phaser: Phaser,
    eq: Eq,
    /// Chorus, delay, reverb, distortion, phaser, EQ.
    wet: [f32; 6],
}

impl Effects {
    pub fn new(sample_rate: f32, p: &EffectsParams) -> Self {
        Self {
            sample_rate,
            chorus: Chorus::new(sample_rate),
            delay: Delay::new(sample_rate, &p.delay),
            reverb: Reverb::new(sample_rate),
            distortion: Distortion::default(),
            phaser: Phaser::default(),
            eq: Eq::default(),
            wet: [0.0; 6],
        }
    }

    /// Silences all tails at once.
    pub fn clear(&mut self) {
        self.chorus.line.clear();
        self.delay.left.clear();
        self.delay.right.clear();
        self.delay.tone_l = 0.0;
        self.delay.tone_r = 0.0;
        self.delay.dc_l = HighPass::default();
        self.delay.dc_r = HighPass::default();
        self.reverb.input_hp = HighPass::default();
        self.reverb.predelay.clear();
        for line in self.reverb.diffusers.iter_mut().chain(self.reverb.lines.iter_mut()) {
            line.clear();
        }
        self.reverb.damp = [0.0; 8];
        self.distortion = Distortion::default();
        self.phaser = Phaser::default();
        self.eq = Eq::default();
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32, p: &EffectsParams) -> (f32, f32) {
        let sr = self.sample_rate;
        let k = smoothing(0.02, sr);
        let targets = [
            if p.chorus.on { p.chorus.mix.clamp(0.0, 1.0) } else { 0.0 },
            if p.delay.on { p.delay.mix.clamp(0.0, 1.0) } else { 0.0 },
            if p.reverb.on { p.reverb.mix.clamp(0.0, 1.0) } else { 0.0 },
            if p.distortion.on { p.distortion.mix.clamp(0.0, 1.0) } else { 0.0 },
            if p.phaser.on { p.phaser.mix.clamp(0.0, 1.0) } else { 0.0 },
            if p.eq.on { 1.0 } else { 0.0 },
        ];
        for (i, (w, t)) in self.wet.iter_mut().zip(targets).enumerate() {
            *w += (t - *w) * k;
            // The newer effects stop altogether once faded out.
            if i >= 3 && t == 0.0 && *w < 1e-5 {
                *w = 0.0;
            }
        }
        let (mut l, mut r) = (l, r);
        if self.wet[3] > 0.0 {
            (l, r) = self.distortion.process(l, r, &p.distortion, self.wet[3], sr);
        }
        if self.wet[4] > 0.0 {
            (l, r) = self.phaser.process(l, r, &p.phaser, self.wet[4], sr);
        }
        let (l, r) = self.chorus.process(l, r, &p.chorus, self.wet[0], sr);
        let (l, r) = self.delay.process(l, r, &p.delay, self.wet[1], sr);
        let (l, r) = self.reverb.process(l, r, &p.reverb, self.wet[2], sr);
        if self.wet[5] > 0.0 { self.eq.process(l, r, &p.eq, self.wet[5], sr) } else { (l, r) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run(fx: &mut Effects, p: &EffectsParams, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        input.iter().map(|&x| fx.process(x, x, p)).unzip()
    }

    fn impulse(n: usize) -> Vec<f32> {
        let mut v = vec![0.0; n];
        v[0] = 1.0;
        v
    }

    fn settled(mut p: EffectsParams) -> (Effects, EffectsParams) {
        // Let the wet levels reach their targets on silence first.
        let mut fx = Effects::new(SR, &p);
        run(&mut fx, &p, &vec![0.0; 10_000]);
        p.delay.time_ms = p.delay.time_ms.max(1.0);
        (fx, p)
    }

    #[test]
    fn all_off_is_dry_and_mono() {
        let p = EffectsParams::default();
        let (mut fx, p) = settled(p);
        let x: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.05).sin()).collect();
        let (l, r) = run(&mut fx, &p, &x);
        assert_eq!(l, x);
        assert_eq!(r, x);
    }

    #[test]
    fn chorus_is_stereo_and_keeps_level() {
        let p = EffectsParams { chorus: ChorusParams { on: true, rate: 0.8, depth: 1.0, mix: 1.0 }, ..Default::default() };
        let (mut fx, p) = settled(p);
        let x: Vec<f32> = (0..48_000).map(|i| (TAU * 440.0 * i as f32 / SR).sin()).collect();
        let (l, r) = run(&mut fx, &p, &x);
        let differs = l.iter().zip(&r).filter(|(a, b)| (*a - *b).abs() > 0.01).count();
        assert!(differs > 10_000, "{differs}");
        assert!(l.iter().chain(&r).all(|v| v.abs() <= 1.01));
    }

    #[test]
    fn delay_echoes_at_its_time_and_ping_pongs() {
        let p = EffectsParams { delay: DelayParams { on: true, time_ms: 100.0, feedback: 0.5, tone: 20_000.0, ping_pong: true, mix: 1.0, ..Default::default() }, ..Default::default() };
        let (mut fx, p) = settled(p);
        let (l, r) = run(&mut fx, &p, &impulse(30_000));
        let peak_at = |v: &[f32], from: usize, to: usize| (from..to).max_by(|&a, &b| v[a].abs().total_cmp(&v[b].abs())).unwrap();
        // First repeat on the left after 100 ms (4800 samples), the next on the right.
        let first = peak_at(&l, 100, 7000);
        assert!((first as i32 - 4800).abs() <= 2, "{first}");
        assert!(l[first].abs() > 0.3 && r[first].abs() < 0.01);
        let second = peak_at(&r, 7000, 12000);
        assert!((second as i32 - 9600).abs() <= 3, "{second}");
        assert!(r[second].abs() > 0.1 && r[second].abs() < l[first].abs());
    }

    #[test]
    fn reverb_decays_in_about_its_time_and_stays_stable() {
        let p = EffectsParams { reverb: ReverbParams { on: true, size: 0.5, decay: 1.5, damping: 0.0, predelay_ms: 0.0, mix: 1.0 }, ..Default::default() };
        let (mut fx, p) = settled(p);
        let n = (4.0 * SR) as usize;
        let (l, r) = run(&mut fx, &p, &impulse(n));
        assert!(l.iter().chain(&r).all(|v| v.is_finite()));
        // Level per 50 ms after the direct sound.
        let block = (0.05 * SR) as usize;
        let level: Vec<f32> = l[1..].chunks(block).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect();
        let start = level[2].max(1e-9);
        let t60 = level.iter().position(|&v| v < start * 1e-3).map(|i| i as f32 * 0.05).unwrap_or(99.0);
        assert!((1.0..2.3).contains(&t60), "T60 {t60}");
        let corr: f32 = l.iter().zip(&r).skip(2000).map(|(a, b)| a * b).sum::<f32>()
            / (l.iter().skip(2000).map(|a| a * a).sum::<f32>() * r.iter().skip(2000).map(|b| b * b).sum::<f32>()).sqrt();
        assert!(corr.abs() < 0.5, "left/right too alike: {corr}");
    }

    #[test]
    fn longest_reverb_with_loud_input_does_not_blow_up() {
        let p = EffectsParams {
            reverb: ReverbParams { on: true, size: 1.0, decay: 20.0, damping: 0.0, predelay_ms: 100.0, mix: 1.0 },
            delay: DelayParams { on: true, time_ms: 7.0, feedback: 0.95, tone: 20_000.0, ping_pong: false, mix: 1.0, ..Default::default() },
            ..Default::default()
        };
        let (mut fx, p) = settled(p);
        let x: Vec<f32> = (0..(10.0 * SR) as usize).map(|i| if (i / 2400) % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let (l, r) = run(&mut fx, &p, &x);
        let peak = l.iter().chain(&r).fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak.is_finite() && peak < 200.0, "{peak}");
    }

    #[test]
    fn offset_does_not_build_up_in_long_tails() {
        let p = EffectsParams {
            reverb: ReverbParams { on: true, size: 1.0, decay: 8.0, damping: 0.0, predelay_ms: 0.0, mix: 1.0 },
            delay: DelayParams { on: true, time_ms: 50.0, feedback: 0.95, tone: 20_000.0, ping_pong: false, mix: 1.0, ..Default::default() },
            ..Default::default()
        };
        let (mut fx, p) = settled(p);
        // A steady offset of 0.1 for a second, then silence.
        let mut x = vec![0.1f32; 48_000];
        x.extend(vec![0.0; 96_000]);
        let (l, r) = run(&mut fx, &p, &x);
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        // Dry 0.1 plus the first echo 0.1; without the high-passes the 0.95
        // feedback alone would pile the offset up towards 0.1 / (1 - 0.95) = 2.
        assert!(mean(&l[24_000..48_000]).abs() < 0.4, "{}", mean(&l[24_000..48_000]));
        // A second after it stops only a few percent of the step's edge still
        // circulates (measured 0.004; without the high-passes the loops reach ~10).
        assert!(mean(&l[96_000..]).abs() < 0.02 && mean(&r[96_000..]).abs() < 0.02, "{} {}", mean(&l[96_000..]), mean(&r[96_000..]));
    }

    fn sine(hz: f32, n: usize, amp: f32) -> Vec<f32> {
        (0..n).map(|i| amp * (TAU * hz * i as f32 / SR).sin()).collect()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn distortion_adds_harmonics_at_an_even_level() {
        for mode in DriveMode::ALL {
            let mut levels = Vec::new();
            for drive in [0.2, 0.6, 1.0] {
                let p = EffectsParams { distortion: DistortionParams { on: true, mode, drive, tone: 16_000.0, mix: 1.0 }, ..Default::default() };
                let (mut fx, p) = settled(p);
                let x = sine(220.0, 24_000, 0.2);
                let (l, r) = run(&mut fx, &p, &x);
                assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() <= 1.0));
                // A pure sine has smooth steps; the shaped one has sharper ones.
                let step = |v: &[f32]| v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
                if drive == 1.0 {
                    assert!(step(&l[12_000..]) > 1.5 * step(&x[12_000..]), "{mode:?}");
                }
                levels.push(rms(&l[12_000..]));
            }
            // From light to full drive the level stays within about 14 dB.
            let (lo, hi) = levels.iter().fold((f32::MAX, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
            assert!(hi / lo < 5.0, "{mode:?}: {levels:?}");
        }
    }

    #[test]
    fn phaser_moves_notches_in_stereo_and_stays_stable() {
        let p = EffectsParams { phaser: PhaserParams { on: true, rate: 1.0, depth: 1.0, feedback: 0.9, mix: 1.0 }, ..Default::default() };
        let (mut fx, p) = settled(p);
        let noise: Vec<f32> = (0..96_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 8) as f32 / (1u32 << 24) as f32 - 0.5).collect();
        let (l, r) = run(&mut fx, &p, &noise);
        assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() < 4.0));
        let differs = l.iter().zip(&r).filter(|(a, b)| (*a - *b).abs() > 0.01).count();
        assert!(differs > 30_000, "{differs}");
        // A steady tone rises and falls in level as the notches pass over it.
        let p2 = EffectsParams { phaser: PhaserParams { feedback: 0.0, ..p.phaser }, ..Default::default() };
        let (mut fx, p2) = settled(p2);
        let (l, _) = run(&mut fx, &p2, &sine(700.0, 96_000, 0.5));
        let blocks: Vec<f32> = l[4800..].chunks(960).map(rms).collect();
        let (lo, hi) = blocks.iter().fold((f32::MAX, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi > 3.0 * lo, "{lo} {hi}");
    }

    #[test]
    fn eq_bands_boost_and_cut_where_they_should() {
        let gain_at = |hz: f32, eq: EqParams| {
            let p = EffectsParams { eq, ..Default::default() };
            let (mut fx, p) = settled(p);
            let x = sine(hz, 48_000, 0.1);
            let (l, _) = run(&mut fx, &p, &x);
            20.0 * (rms(&l[24_000..]) / rms(&x[24_000..])).log10()
        };
        let flat = EqParams { on: true, ..Default::default() };
        for hz in [60.0, 1000.0, 8000.0] {
            assert!(gain_at(hz, flat).abs() < 0.1, "flat at {hz}");
        }
        let low = gain_at(60.0, EqParams { low: 12.0, ..flat });
        let mid = gain_at(1000.0, EqParams { mid: 6.0, ..flat });
        let high = gain_at(12_000.0, EqParams { high: -12.0, ..flat });
        assert!((low - 12.0).abs() < 1.0 && (mid - 6.0).abs() < 0.5 && (high + 12.0).abs() < 1.0, "{low} {mid} {high}");
        // A low boost leaves the highs alone.
        assert!(gain_at(8000.0, EqParams { low: 12.0, ..flat }).abs() < 0.5);
    }

    #[test]
    fn switching_off_fades() {
        let p = EffectsParams { reverb: ReverbParams { on: true, size: 0.5, decay: 3.0, damping: 0.3, predelay_ms: 0.0, mix: 1.0 }, ..Default::default() };
        let (mut fx, mut p) = settled(p);
        let noise: Vec<f32> = (0..24_000).map(|i| ((i * 7919) % 101) as f32 / 101.0 - 0.5).collect();
        run(&mut fx, &p, &noise);
        p.reverb.on = false;
        let (l, _) = run(&mut fx, &p, &vec![0.0; 4800]);
        // No jump in the first samples after switching off; tail gone after 100 ms.
        let jump = l.windows(2).take(50).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        assert!(jump < 0.2, "{jump}");
        assert!(l[4799].abs() < 1e-3, "{}", l[4799]);
    }
}
