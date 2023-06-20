use realfft::{FftError, RealFftPlanner};
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use std::error::Error;
use std::f32::consts::PI;
use std::fmt;
use std::iter;

use crate::gps_ca_prn::generate_ca_code;
use crate::gps_constants;

static FFT_LENGTH_MS: usize = 1;
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
    let fft_length = FFT_LENGTH_MS * freq_sampling as usize;
    let mut real_planner = RealFftPlanner::<f64>::new();
    let r2c_fft = real_planner.plan_fft_forward(fft_length);
    let mut input_fft = r2c_fft.make_output_vec();

    let steps: i32 = 2 * FREQ_SEARCH_ACQUISITION_HZ as i32 / FREQ_SEARCH_STEP_HZ + 1;

    for prn in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let ca_code = generate_ca_code(prn as usize);
        let samples_per_chip = freq_sampling / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S;
        let ca_code_sampled: Vec<i32> = ca_code
            .iter()
            .flat_map(|&x| iter::repeat(x).take(samples_per_chip as usize))
            .collect();
        let mut input_data: Vec<f64> = ca_code_sampled.iter().map(|x| *x as f64).collect();
        assert_eq!(input_data.len(), fft_length);
        assert_eq!(input_fft.len(), fft_length / 2 + 1);
        r2c_fft.process(&mut input_data, &mut input_fft)?; // realfft::FftError

        for step in 0..steps {
            let freq = -1.0 * FREQ_SEARCH_ACQUISITION_HZ + (step * FREQ_SEARCH_STEP_HZ) as f32;
            let q_arm: Vec<f32> = (0..samples.len())
                .map(|x| {
                    ((2.0 * PI * freq * 1.0 / freq_sampling * x as f32).cos()) * samples[x] as f32
                })
                .collect();
            let i_arm: Vec<f32> = (0..samples.len())
                .map(|x| {
                    ((2.0 * PI * freq * 1.0 / freq_sampling * x as f32).sin()) * samples[x] as f32
                })
                .collect();
            let c_sum: Vec<f32> = q_arm.iter().zip(i_arm.iter()).map(|(x, y)| x + y).collect();
        }
    }

    Ok(AcquistionStatistics {})
}
