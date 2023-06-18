use realfft::{FftError, RealFftPlanner};
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use std::error::Error;
use std::f32::consts::PI;
use std::fmt;

use crate::gps_ca_prn::generate_ca_code;

static FFT_LENGTH: usize = 4096;
static FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
static FREQ_SEARCH_STEP_HZ: i32 = 500; // Hz
static PRN_SEARCH_ACQUISITION_TOTAL: i8 = 32; // 32 PRN codes to search

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error happens while doing signal acquisition!")
    }
}

impl Error for AcqError {}
pub struct AcquistionStatistics {}

pub fn do_acquisition(
    samples: Vec<u8>,
    freq_sampling: f32,
) -> Result<AcquistionStatistics, Box<dyn Error>> {
    let mut real_planner = RealFftPlanner::<f64>::new();
    let r2c_fft = real_planner.plan_fft_forward(FFT_LENGTH);
    let mut input_fft = r2c_fft.make_output_vec();
    let mut input_data: Vec<f64> = samples.iter().map(|x| *x as f64).collect();

    assert_eq!(input_data.len(), FFT_LENGTH);
    assert_eq!(input_fft.len(), FFT_LENGTH / 2 + 1);
    r2c_fft.process(&mut input_data, &mut input_fft)?; // realfft::FftError

    let steps: i32 = 2 * FREQ_SEARCH_ACQUISITION_HZ as i32 / FREQ_SEARCH_STEP_HZ + 1;

    for prn in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let ca_code = generate_ca_code(prn as usize);
        for step in 0..steps {
            let freq = -1.0 * FREQ_SEARCH_ACQUISITION_HZ + (step * FREQ_SEARCH_STEP_HZ) as f32;
            let cos_phases: Vec<f32> = (0..samples.len())
                .map(|x| (2.0 * PI * freq * 1.0 / freq_sampling * x as f32).cos())
                .collect();
            let sin_phases: Vec<f32> = (0..samples.len())
                .map(|x| (2.0 * PI * freq * 1.0 / freq_sampling * x as f32).sin())
                .collect();
        }
    }

    Ok(AcquistionStatistics {})
}
