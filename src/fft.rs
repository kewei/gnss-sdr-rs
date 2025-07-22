use rustfft::{Fft, FftPlanner, FftNum, num_complex::Complex};
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct FFT<T: FftNum> {
    fft: Arc<dyn Fft<T>>,
    len: usize,
}

impl<T: FftNum> FFT<T> {
    pub fn new(len: usize) -> Self {
        let mut planner = FftPlanner::<T>::new();
        let fft = planner.plan_fft_forward(len);
        Self {
            fft,
            len,
        }
    }

    pub fn execute(&self, input: &mut [Complex<T>]) -> Vec<Complex<T>> {
        // let mut buffer = vec![Complex{ re: T::zero(), im: T::zero()}; self.len];
        self.fft.process(input);
        input.to_vec()
    }

    pub fn power_spectrum(&self, input: &mut [Complex<T>]) -> Vec<T> {
        self.execute(input).iter().map(|c| c.norm_sqr()).collect()
    }
}

pub struct RealFFT<T: FftNum> {
    fft: Arc<dyn RealToComplex<T>>,
    len: usize,
}

impl<T: FftNum> RealFFT<T> {
    pub fn new(len: usize) -> Self {
        let mut planner = RealFftPlanner::<T>::new();
        let fft = planner.plan_fft_forward(len);
        Self {
            fft,
            len,
        }
    }

    pub fn execute(&self, input: &mut [T]) -> Vec<Complex<T>> {
        let mut buffer = vec![Complex{ re: T::zero(), im: T::zero()}; self.len / 2 + 1];
        self.fft.process(input, &mut buffer);
        buffer
    }

    pub fn power_spectrum(&self, input: &mut [T]) -> Vec<T> {
        self.execute(input).iter().map(|c| c.norm_sqr()).collect()
    }
}