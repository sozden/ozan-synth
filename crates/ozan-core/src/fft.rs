//! A small radix-2 FFT, enough to build band-limited wavetables. Not used while
//! rendering, so it may allocate.

use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn mul(self, o: Self) -> Self {
        Self::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }

    pub fn norm(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// In-place FFT; `inverse` uses the opposite sign and divides by the length.
/// The length must be a power of two.
pub fn fft(data: &mut [Complex], inverse: bool) {
    let n = data.len();
    assert!(n.is_power_of_two(), "FFT length must be a power of two");
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let w = Complex::new((TAU / len as f64).cos(), sign * (TAU / len as f64).sin());
        for start in (0..n).step_by(len) {
            let mut wk = Complex::new(1.0, 0.0);
            for k in 0..len / 2 {
                let a = data[start + k];
                let b = data[start + k + len / 2].mul(wk);
                data[start + k] = Complex::new(a.re + b.re, a.im + b.im);
                data[start + k + len / 2] = Complex::new(a.re - b.re, a.im - b.im);
                wk = wk.mul(w);
            }
        }
        len <<= 1;
    }
    if inverse {
        for c in data.iter_mut() {
            c.re /= n as f64;
            c.im /= n as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_bin_is_a_cosine_and_round_trips() {
        let n = 64;
        let x: Vec<Complex> = (0..n).map(|i| Complex::new((TAU * 3.0 * i as f64 / n as f64).cos(), 0.0)).collect();
        let mut spec = x.clone();
        fft(&mut spec, false);
        for (k, c) in spec.iter().enumerate() {
            let expect = if k == 3 || k == n - 3 { n as f64 / 2.0 } else { 0.0 };
            assert!((c.norm() - expect).abs() < 1e-9, "bin {k}: {}", c.norm());
        }
        fft(&mut spec, true);
        for (a, b) in spec.iter().zip(&x) {
            assert!((a.re - b.re).abs() < 1e-12 && a.im.abs() < 1e-12);
        }
    }
}
