use std::sync::Arc;
use num::Complex;
use rustfft::{Fft, FftPlanner};
use crate::acquisition::doppler_shift::{DopplerShiftTable, apply_doppler_shift};
use crate::utilities::ca_code::generate_ca_code_samples;

const FFT_LENGTH_MS: usize = 1;
const FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
const FREQ_SEARCH_STEP_HZ: i32 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: usize = 32; // 32 PRN codes to search
const LONG_SAMPLES_LENGTH: i8 = 11; // ms

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error happens while doing signal acquisition!")
    }
}

impl Error for AcqError {}

#[derive(Debug, Clone)]
pub struct AcquisitionResult {
    pub prn: usize,
    pub code_phase: usize,
    pub carrier_freq: f32,
    pub mag_relative: f32,
    pub ca_code: Vec<i16>,
    pub ca_code_samples: Vec<i16>,
    pub cn0: f32,
}

impl AcquisitionResult {
    pub fn new(prn: usize, f_sampling: f32) -> Self {
        let (ca_code_samples, ca_code) = generate_ca_code_samples(prn, f_sampling);
        Self {
            prn,
            code_phase: 0,
            carrier_freq: 0.0,
            mag_relative: 0.0,
            ca_code,
            ca_code_samples,
            cn0: 0.0,
        }
    }
}

pub enum ChannelState {
    Idle,
    Acquiring,
    Tracking,
}

pub struct AcquisitionWorker {
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    fft_size: usize,
    freq_sampling_hz: f32,
    doppler_table: Vec<DopplerShiftTable>,
    result_buf: Vec<Complex<f32>>,
    freq_replica: Vec<Complex<f32>>,
}

impl AcquisitionWorker {
    fn new(fft_size: usize, freq_sampling_hz: f32) -> Self {
        let mut planner = FftPlanner::new();
        let capacity = (FREQ_SEARCH_ACQUISITION_HZ / FREQ_SEARCH_STEP_HZ) as usize + 1;
        let mut doppler_table = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let doppler_freq = -FREQ_SEARCH_ACQUISITION_HZ / 2.0 + i as f32 * FREQ_SEARCH_STEP_HZ as f32;
            doppler_table.push(DopplerShiftTable::new(doppler_freq, freq_sampling_hz, fft_size));
        }
        Self {
            fft: planner.plan_fft_forward(fft_size),
            ifft: planner.plan_fft_inverse(fft_size),
            fft_size: fft_size,
            freq_sampling_hz: freq_sampling_hz,
            doppler_table: doppler_table,
            result_buf: vec![Complex::new(0.0, 0.0); fft_size],
            freq_replica: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    pub fn do_acquisition(
        acquisition_result: Arc<Mutex<AcquisitionResult>>,
        freq_IF: f32,
        is_complex: bool,
    ) -> Result<usize, &'static str> {
    }

    fn search_satellite(
        &mut self,
        samples_chunk: &[Complex<f32>],
        ca_code_fft: &[Complex<f32>],
        fs: f32,
    ) -> Option<AcquisitionResult> {
        let mut max_val: f32 = 0.0;
        let mut best_doper_freq: f32 = 0.0;
        let mut best_code_phase: usize = 0;

        for doppler in ((-FREQ_SEARCH_ACQUISITION_HZ / 2) ..=(FREQ_SEARCH_ACQUISITION_HZ / 2)).step_by(FREQ_SEARCH_STEP_HZ) {
            apply_doppler_shift(samples_chunk, doppler_table, &mut self.result_buf);
        }

    }
}
