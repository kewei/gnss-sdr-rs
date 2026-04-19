use std::simd::f32x8;
use std::simd::usizex8;

use crate::rf::dc_remove::DcRemoverSimd;
use crate::rf::nco_lut::{mix_simd, NcoLut, LUT_SIZE};
use num::Complex;
use std::f32::consts::PI;

pub struct DigitalFrontend {
    // NCO for frequency shifting
    nco: NcoLut,
    // DC offset removal
    dc_remove: DcRemoverSimd,
    // Resampling
    input_sample_rate: f32,
    output_sample_rate: f32,
}

impl DigitalFrontend {
    pub fn new(f_if: f32, fs_in: f32, fs_out: f32) -> Self {
        let mut nco = NcoLut::new(f_if, fs_in as f32);
        let mut dc_remove = DcRemoverSimd::new(0.001);

        DigitalFrontend {
            nco,
            dc_remove,
            input_sample_rate: fs_in,
            output_sample_rate: fs_out,
        }
    }

    /// Process a block of samples in-place, using SIMD for performance, the input samples are in size 4096
    pub fn process_block(&mut self, raw_floats: &mut [f32]) {
        // Process 16 samples at a time using SIMD
        for chunk in raw_floats.chunks_exact_mut(16) {
            let a = f32x8::from_slice(&chunk[0..8]);
            let b = f32x8::from_slice(&chunk[8..16]);

            let (mut re_v, mut im_v) = a.deinterleave(b);  // Now i_v and q_v contain the I and Q components of 8 samples separately
 
            // DC offset removal
            let (dc_removed_re, dc_removed_im) = self.dc_remove.process_block(re_v, im_v);
            re_v = dc_removed_re;
            im_v = dc_removed_im;

            // Generate 8 indices for the LUT based on the current phase
            let mut indices = [0usize; 8];
            for i in 0..8 {
                indices[i] = self.nco.phase_accumulator as usize % LUT_SIZE;
                self.nco.phase_accumulator =
                    (self.nco.phase_accumulator + self.nco.phase_step) % LUT_SIZE as f32;
            }
            /// A bit slower than gather, but hardware support for gather is not good, and this can be optimized by compiler to use SIMD load
            // let cos_v = f32x8::from_array([self.lut_re[indices[0]], self.lut_re[indices[1]], self.lut_re[indices[2]], self.lut_re[indices[3]], self.lut_re[indices[4]], self.lut_re[indices[5]], self.lut_re[indices[6]], self.lut_re[indices[7]]]);
            // let sin_v = f32x8::from_array([self.lut_im[indices[0]], self.lut_im[indices[1]], self.lut_im[indices[2]], self.lut_im[indices[3]], self.lut_im[indices[4]], self.lut_im[indices[5]], self.lut_im[indices[6]], self.lut_im[indices[7]]]);
            let idx_v = usizex8::from_array(indices);
            let cos_v = f32x8::gather_or_default(&self.nco.lut_re, idx_v);
            let sin_v = f32x8::gather_or_default(&self.nco.lut_im, idx_v);
            let (res_re, res_im) = mix_simd(re_v, im_v, cos_v, sin_v);
            let (out_a, out_b) = res_re.interleave(res_im);
            out_a.copy_to_slice(&mut chunk[0..8]);
            out_b.copy_to_slice(&mut chunk[8..16]);
        }

        // Pulse blanking
        // (Placeholder: Implement pulse blanking logic here, e.g., based on amplitude threshold)
        todo!("Implement pulse blanking logic");

        // Resampling
    }
}
