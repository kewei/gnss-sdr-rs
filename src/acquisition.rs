use realfft::{FftError, RealFftPlanner};
use rustfft::num_complex::{Complex, Complex32};
use rustfft::num_traits::Zero;
use rustfft::FftPlanner;
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
    let samples_iq: Vec<Complex32> = samples
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect();

    let mut real_planner = RealFftPlanner::<f32>::new();
    let r_fft = real_planner.plan_fft_forward(fft_length);
    let mut ca_code_fft = r_fft.make_output_vec();

    let mut complex_planner = FftPlanner::new();
    let c_fft = complex_planner.plan_fft_forward(fft_length);

    let mut inv_planner = FftPlanner::new();
    let inv_fft = inv_planner.plan_fft_inverse(fft_length);

    let steps: i32 = 2 * FREQ_SEARCH_ACQUISITION_HZ as i32 / FREQ_SEARCH_STEP_HZ + 1;

    for prn in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let ca_code = generate_ca_code(prn as usize);
        let samples_per_chip = freq_sampling / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S;
        let ca_code_sampled: Vec<i32> = ca_code
            .iter()
            .flat_map(|&x| iter::repeat(x).take(samples_per_chip as usize))
            .collect();
        let mut ca_code_input: Vec<f32> = ca_code_sampled.iter().map(|x| *x as f32).collect();
        assert_eq!(ca_code_input.len(), fft_length);
        r_fft.process(&mut ca_code_input, &mut ca_code_fft)?;
        let mut ca_code_fft_conj: Vec<Complex<f32>> =
            ca_code_fft.iter().map(|x| x.conj()).collect();

        let mut d_max: Vec<f32> = vec![0.0; steps as usize];
        for step in 0..steps {
            let freq = -1.0 * FREQ_SEARCH_ACQUISITION_HZ + (step * FREQ_SEARCH_STEP_HZ) as f32;
            let q_arm: Vec<Complex32> = (0..fft_length)
                .map(|x| ((2.0 * PI * freq * 1.0 / freq_sampling * x as f32).cos()) * samples_iq[x])
                .collect();
            let i_arm: Vec<Complex32> = (0..fft_length)
                .map(|x| ((2.0 * PI * freq * 1.0 / freq_sampling * x as f32).sin()) * samples_iq[x])
                .collect();
            let mut c_sum: Vec<Complex32> =
                q_arm.iter().zip(i_arm.iter()).map(|(x, y)| x + y).collect();
            c_fft.process(&mut c_sum);

            let mut cross_corr: Vec<Complex32> = c_sum
                .iter()
                .zip(ca_code_fft_conj.iter())
                .map(|(x, y)| x * y)
                .collect();
            inv_fft.process(&mut cross_corr);
            d_max[step as usize] = cross_corr.iter().fold(0.0, |acc, x| acc + x.norm());
        }
    }

    Ok(AcquistionStatistics {})
}
