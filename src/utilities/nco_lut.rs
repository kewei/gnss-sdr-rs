#![feature(portable_simd)]

use num_complex::Complex;
use std::f32::consts::PI;
use std::simd::f32x8;
use std::simd::simd_index_select;

const LUT_SIZE: usize = 1024;
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
    table: [Complex<f32>; LUT_SIZE],
    phase_accumulator: f32,
    phase_step: f32,
}

impl NcoLut {
    pub fn new(freq: f32, sample_rate: f32) -> Self {
        let mut table = [Complex::new(0.0, 0.0); LUT_SIZE];
        for i in 0..LUT_SIZE {
            let angle = (2.0 * PI * i as f32) / LUT_SIZE as f32;
            table[i] = Complex::new(angle.cos(), -angle.sin());
        }

        let phase_step = (freq / sample_rate) * LUT_SIZE as f32;

        Self {
            table,
            phase_accumulator: 0.0,
            phase_step,
        }
    }

    /// Linear Interpolation, If 1024 points isn't enough, do interpolation between points
    #[inline(always)]
    pub fn mix(&mut self, sample: Complex<f32>) -> Complex<f32> {
        let index = self.phase_accumulator as usize & LUT_MASK;
        let nco_value = self.table[index];
        self.phase_accumulator += self.phase_step;
        if self.phase_accumulator >= LUT_SIZE as f32 {
            self.phase_accumulator -= LUT_SIZE as f32;
        }
        sample * nco_value
    }

    pub fn process_block(&mut self, input_re: &mut[f32], input_im: &mut[f32]) {
        // Process 8 samples at a time using SIMD
        for (re, im) in input_re.chunks_exact_mut(8).zip(input_im.chunks_exact_mut(8)) {
            // Generate 8 indices for the LUT based on the current phase
            let mut indices = [0usize; 8];
            for i in 0..8 {
                indices[i] = self.phase_accu as usize % self.lut_re.len();
                self.phase_accu = (self.phase_accu + self.phase_step) % self.lut_size;
            }
            /// A bit slower than gather, but hardware support for gather is not good, and this can be optimized by compiler to use SIMD load
            // let cos_v = f32x8::from_array([self.lut_re[indices[0]], self.lut_re[indices[1]], self.lut_re[indices[2]], self.lut_re[indices[3]], self.lut_re[indices[4]], self.lut_re[indices[5]], self.lut_re[indices[6]], self.lut_re[indices[7]]]);
            // let sin_v = f32x8::from_array([self.lut_im[indices[0]], self.lut_im[indices[1]], self.lut_im[indices[2]], self.lut_im[indices[3]], self.lut_im[indices[4]], self.lut_im[indices[5]], self.lut_im[indices[6]], self.lut_im[indices[7]]]);
            let idx_v = usizex8::from_array(indices);
            let cos_v = f32x8::gather_or_default(&self.lut_re, idx_v);
            let sin_v = f32x8::gather_or_default(&self.lut_im, idx_v);
            let (res_re, res_im) = mix_simd(
                f32x8::from_slice(re),
                f32x8::from_slice(im),
                cos_v,
                sin_v,
            );
            re.copy_from_slice(&res_re.to_array());
            im.copy_from_slice(&res_im.to_array());
        }
    }

}