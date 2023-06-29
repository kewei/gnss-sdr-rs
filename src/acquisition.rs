use realfft::RealFftPlanner;
use rustfft::num_complex::{Complex, Complex32};
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
    freq_IF: f32,
) -> Result<AcquistionStatistics, Box<dyn Error>> {
    let fft_length = (FFT_LENGTH_MS as f32 * 1.0e-3 * freq_sampling) as usize;

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

    let mut complex_planner = FftPlanner::new();
    let c_fft = complex_planner.plan_fft_forward(fft_length);

    let mut inv_planner = FftPlanner::new();
    let inv_fft = inv_planner.plan_fft_inverse(fft_length);

    let steps: i32 = 2 * FREQ_SEARCH_ACQUISITION_HZ as i32 / FREQ_SEARCH_STEP_HZ + 1;

    for prn in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let mut ca_code_fft = r_fft.make_output_vec();
        let ca_code = generate_ca_code((prn + 1) as usize);
        let samples_per_chip = freq_sampling / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S;
        let ca_code_sampled: Vec<i32> = ca_code
            .iter()
            .flat_map(|&x| iter::repeat(x).take(samples_per_chip as usize))
            .collect();
        let mut ca_code_input: Vec<f32> = ca_code_sampled.iter().map(|x| *x as f32).collect();
        assert_eq!(ca_code_input.len(), fft_length);
        r_fft.process(&mut ca_code_input, &mut ca_code_fft)?;

        // realfft does not calculate all fft results, so need to get the rest part
        let mut second_part = vec![Complex32::new(0.0, 0.0); ca_code_fft.len() - 2];
        second_part.copy_from_slice(&ca_code_fft[1..(ca_code_fft.len() - 1)]);
        let mut ca_code_fft_conj: Vec<Complex<f32>> =
            ca_code_fft.iter().map(|x| x.conj()).collect();
        second_part.reverse();
        ca_code_fft_conj.extend(second_part.iter());

        let mut d_max_2d: Vec<Vec<f32>> = Vec::with_capacity(steps as usize);
        for step in 0..steps {
            let freq =
                freq_IF + -1.0 * FREQ_SEARCH_ACQUISITION_HZ + (step * FREQ_SEARCH_STEP_HZ) as f32;
            let mut sum_i_q: Vec<Complex32> = (0..fft_length)
                .map(|x| {
                    Complex32::new(
                        (2.0 * PI * freq * 1.0 / freq_sampling * x as f32).cos(),
                        (2.0 * PI * freq * 1.0 / freq_sampling * x as f32).sin(),
                    ) * samples_iq[x]
                })
                .collect();
            c_fft.process(&mut sum_i_q);

            let mut cross_corr: Vec<Complex32> = sum_i_q
                .iter()
                .zip(ca_code_fft_conj.iter())
                .map(|(x, y)| x * y)
                .collect();

            inv_fft.process(&mut cross_corr);
            d_max_2d.push(cross_corr.iter().map(|x| x.norm()).collect());
        }
    }

    Ok(AcquistionStatistics {})
}

/// Check whether the satellite is visible with Cell-Averaging Constant False Alarm Rate (CA-CFAR) algorithm
///
fn ca_cfar_detector(
    corr_results: Vec<Vec<f32>>,
    window_size: usize,
    false_alarm_rate: f32,
) -> Option<f32> {
    let num_freq_bins = corr_results.len();
    let num_cells = corr_results[0].len();

    // Compute the number of guard cells and training cells based on the false alarm rate
    let num_guard_cells = ((false_alarm_rate + 1.0)
        * (window_size as f32 / (false_alarm_rate + 2.0)))
        .ceil() as usize;
    let num_training_cells = window_size - num_guard_cells * 2;
    let mut thresholded_signal = vec![vec![0_i8; num_cells - window_size]; num_freq_bins];
    for i in 0..num_freq_bins {
        for j in num_guard_cells..=num_cells - window_size + num_guard_cells {
            // Compute the average of the neighboring training cells within the window
            let noise_lvl = (corr_results[i]
                [j - num_guard_cells..j - num_guard_cells + num_training_cells]
                .iter()
                .sum::<f32>()
                + corr_results[i]
                    [j + num_guard_cells + 1..j + num_guard_cells + 1 + num_training_cells]
                    .iter()
                    .sum::<f32>())
                / (2 * num_training_cells) as f32;
            // Compute the threshold based on the noise level and the false alarm rate
            let threshold =
                noise_lvl * ((2.0_f32).powf(false_alarm_rate / num_training_cells as f32));

            // Compare the current window's cells with the threshold
            if corr_results[i][j..j + window_size]
                .to_vec()
                .into_iter()
                .reduce(f32::max)?
                > threshold
            {
                thresholded_signal[i][j] = 1; //Target detected in the window
            }
        }
    }
    // Check if the satellite is absent in all Doppler frequency offsets

    Some(0.0)
}
