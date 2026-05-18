#![feature(portable_simd)]
use std::simd::f32x8;

pub struct DcRemoverSimd {
    bias_re: f32x8,
    bias_im: f32x8,
    alpha: f32x8,
    con: f32x8,
}

impl DcRemoverSimd {
    pub fn new(alpha: f32) -> Self {
        let alpha_splat = f32x8::splat(alpha);
        let con = f32x8::splat(1.0) - alpha_splat;
        Self {
            bias_re: f32x8::splat(0.0),
            bias_im: f32x8::splat(0.0),
            alpha: alpha_splat,
            con,
        }
    }

    #[inline(always)]
    pub fn process_block(&mut self, input_re: f32x8, input_im: f32x8) -> (f32x8, f32x8) {
        // bias = (1-alpha) * bias + alpha * input
        self.bias_re = self.bias_re * self.con + input_re * self.alpha;
        self.bias_im = self.bias_im * self.con + input_im * self.alpha;

        (input_re - self.bias_re, input_im - self.bias_im)
    }
}