#![feature(portable_simd)]
use std::simd::f32x8;
use std::simd::usizex8;

use num::Complex;
use std::f32::consts::PI;
use crate::utilities::{dc_remove::DcRemoverSimd, nco_lut::NcoLut};

struct DigitalFrontend {
    // NCO for frequency shifting
    nco: NcoLut,
    // DC offset removal
    dc_remove: DcRemoverSimd,
    // Resampling
    input_sample_rate: f64,
    output_sample_rate: f64,
}

impl DigitalFrontend {
    fn new(f_if: f32, fs_in: f64, fs_out: f64) -> Self {
        let mut nco = NcoLut::new(f_if, fs_in as f32);
        let mut dc_remove = DcRemoverSimd::new(0.001);

        DigitalFrontend {
            nco,
            dc_remove,
            input_sample_rate: fs_in,
            output_sample_rate: fs_out,
        }
    }


    /// Process a block of samples in-place, using SIMD for performance, the input samples are in size 1024N
    pub fn process_block(&mut self, input_re: &mut[f32], input_im: &mut[f32]) {
        // Process 8 samples at a time using SIMD
        for (re, im) in input_re.chunks_exact_mut(8).zip(input_im.chunks_exact_mut(8)) {
            let mut re_v = f32x8::from_slice(re);
            let mut im_v = f32x8::from_slice(im);
            // DC offset removal
            let (dc_removed_re, dc_removed_im) = self.dc_remove.process_block(re_v, im_v);
            re_v = dc_removed_re;
            im_v = dc_removed_im;
            
            // Generate 8 indices for the LUT based on the current phase
            let mut indices = [0usize; 8];
            for i in 0..8 {
                indices[i] = self.nco.phase_accumulator as usize % self.nco.lut_re.len();
                self.nco.phase_accumulator = (self.nco.phase_accumulator + self.nco.phase_step) % self.nco.lut_size;
            }
            /// A bit slower than gather, but hardware support for gather is not good, and this can be optimized by compiler to use SIMD load
            // let cos_v = f32x8::from_array([self.lut_re[indices[0]], self.lut_re[indices[1]], self.lut_re[indices[2]], self.lut_re[indices[3]], self.lut_re[indices[4]], self.lut_re[indices[5]], self.lut_re[indices[6]], self.lut_re[indices[7]]]);
            // let sin_v = f32x8::from_array([self.lut_im[indices[0]], self.lut_im[indices[1]], self.lut_im[indices[2]], self.lut_im[indices[3]], self.lut_im[indices[4]], self.lut_im[indices[5]], self.lut_im[indices[6]], self.lut_im[indices[7]]]);
            let idx_v = usizex8::from_array(indices);
            let cos_v = f32x8::gather_or_default(&self.lut_re, idx_v);
            let sin_v = f32x8::gather_or_default(&self.lut_im, idx_v);
            let (res_re, res_im) = mix_simd(re_v, im_v, cos_v, sin_v);
            re.copy_from_slice(&res_re.to_array());
            im.copy_from_slice(&res_im.to_array());
        }
        
        // Pulse blanking
        // (Placeholder: Implement pulse blanking logic here, e.g., based on amplitude threshold)
        todo!("Implement pulse blanking logic");

        // Resampling

    }
}