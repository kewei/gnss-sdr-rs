#![feature(portable_simd)]

use num_complex::Complex;
use std::f32::consts::PI;
use std::simd::f32x8;

const LUT_SIZE: usize = 2048;
const LUT_MASK: usize = LUT_SIZE - 1;

#[inline(always)]
pub fn mix_simd(samples_i: f32x8, samples_q: f32x8, lut_cos:f32x8, lut_sin:f32x8) -> (f32x8, f32x8) {
    // Complex multiplication: (I + jQ) * (cos - jsin)
    // Real part: I*cos + Q*sin
    // Imaginary part: Q*cos - I*sin
    let mixed_i = samples_i * lut_cos + samples_q * lut_sin;
    let mixed_q = samples_i * lut_sin - samples_q * lut_cos;
    (mixed_i, mixed_q)
}

struct NcoLut {
    lut_re: [f32; LUT_SIZE],
    lut_im: [f32; LUT_SIZE],
    phase_accumulator: f32,
    phase_step: f32,
}

impl NcoLut {
    pub fn new(freq: f32, sample_rate: f32) -> Self {
        let mut lut_re = [0.0; LUT_SIZE];
        let mut lut_im = [0.0; LUT_SIZE];
        for i in 0..LUT_SIZE {
            let angle = (2.0 * PI * i as f32) / LUT_SIZE as f32;
            lut_re[i] = angle.cos();
            lut_im[i] = -angle.sin(); // Negative for downconversion
        }

        let phase_step = (freq / sample_rate) * LUT_SIZE as f32;

        Self {
            lut_re,
            lut_im,
            phase_accumulator: 0.0,
            phase_step,
        }
    }

    // /// Linear Interpolation, If 1024 points isn't enough, do interpolation between points
    // #[inline(always)]
    // pub fn mix(&mut self, sample: Complex<f32>) -> Complex<f32> {
    //     let index = self.phase_accumulator as usize & LUT_MASK;
    //     let nco_value = Complex::new(self.lut_re[index], self.lut_im[index]);
    //     self.phase_accumulator += self.phase_step;
    //     if self.phase_accumulator >= LUT_SIZE as f32 {
    //         self.phase_accumulator -= LUT_SIZE as f32;
    //     }
    //     sample * nco_value
    // }

}