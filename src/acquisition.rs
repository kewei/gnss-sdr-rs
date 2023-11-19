use itertools::Itertools;
use puruspe::invgammp;
use rayon::prelude::*;
use realfft::RealFftPlanner;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::{Complex, Complex32, ComplexFloat};
use rustfft::FftPlanner;
use rustfft::{Fft, FftDirection};
use std::error::Error;
use std::f32::consts::PI;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::app_buffer_utilities::{get_current_buffer, APPBUFF, BUFFER_SIZE};
use crate::comm_func::max_float_vec;
use crate::gps_ca_prn::generate_ca_code;
use crate::gps_constants;

static FFT_LENGTH_MS: usize = 1;
static FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
static FREQ_SEARCH_STEP_HZ: i32 = 500; // Hz
pub const PRN_SEARCH_ACQUISITION_TOTAL: usize = 32; // 32 PRN codes to search
static LONG_SAMPLES_LENGTH: i8 = 11; // ms

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
        }
    }
}

pub fn do_acquisition(
    acquisition_result: Arc<Mutex<AcquisitionResult>>,
    freq_sampling: f32,
    freq_IF: f32,
    is_complex: bool,
) -> Result<usize, &'static str> {
    let mut acq_result = acquisition_result
        .lock()
        .expect("Error in locking in do_acquisition");
    let num_ca_code_samples = (freq_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let fft_length = num_ca_code_samples; // One CA code length
    let samples_per_chip =
        (freq_sampling / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S).round() as usize;

    let app_buff_clone = unsafe { Arc::clone(&APPBUFF) };
    let app_buff_value = app_buff_clone
        .read()
        .expect("Error in reading buff_cnt in acquisition");
    let n_samples: usize = LONG_SAMPLES_LENGTH as usize * num_ca_code_samples;
    let mut buffer_location = app_buff_value.buff_cnt * BUFFER_SIZE - n_samples;
    let long_samples = get_current_buffer(buffer_location, 2 * n_samples);

    let samples_iq: Vec<Complex32> = long_samples[..4 * num_ca_code_samples]
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect();

    let samples_iq_1st = &samples_iq[..num_ca_code_samples];
    let samples_iq_2nd = &samples_iq[num_ca_code_samples..];

    let mut real_planner = RealFftPlanner::<f32>::new();
    let r_fft = real_planner.plan_fft_forward(fft_length);

    let mut complex_planner = FftPlanner::new();
    let c_fft = complex_planner.plan_fft_forward(fft_length);

    let mut inv_planner = FftPlanner::new();
    let inv_fft = inv_planner.plan_fft_inverse(fft_length);

    let steps: i32 = 2 * FREQ_SEARCH_ACQUISITION_HZ as i32 / FREQ_SEARCH_STEP_HZ + 1;
    let test_threhold = (2.0 * invgammp(0.8, 2.0)) as f32;

    let mut ca_code_fft = r_fft.make_output_vec();
    let prn = acq_result.prn;
    let mut ca_code_input: Vec<f32> = acq_result
        .ca_code_samples
        .iter()
        .map(|x| *x as f32)
        .collect();
    assert_eq!(ca_code_input.len(), fft_length);
    if let Ok(()) = r_fft.process(&mut ca_code_input, &mut ca_code_fft) {
    } else {
        return Err("Error in RealFftPlanner");
    };

    // realfft does not calculate all fft results, so need to get the rest part
    let mut second_part = vec![Complex32::new(0.0, 0.0); ca_code_fft.len() - 2];
    second_part.copy_from_slice(&ca_code_fft[1..(ca_code_fft.len() - 1)]);
    let mut ca_code_fft_conj: Vec<Complex<f32>> = ca_code_fft.iter().map(|x| x.conj()).collect();
    second_part.reverse();
    ca_code_fft_conj.extend(second_part.iter());

    let mut d_max_2d: Vec<Vec<f32>> = Vec::with_capacity(steps as usize);
    let mut d_max_2d_2nd: Vec<Vec<f32>> = Vec::with_capacity(steps as usize);

    for step in 0..steps {
        let carrier_freq =
            freq_IF + -1.0 * FREQ_SEARCH_ACQUISITION_HZ + (step * FREQ_SEARCH_STEP_HZ) as f32;
        let mut sum_i_q_1st: Vec<Complex32> = (0..fft_length)
            .map(|x| {
                Complex32::new(
                    (2.0 * PI * carrier_freq * 1.0 / freq_sampling * x as f32).cos(),
                    (2.0 * PI * carrier_freq * 1.0 / freq_sampling * x as f32).sin(),
                ) * samples_iq_1st[x]
            })
            .collect();
        c_fft.process(&mut sum_i_q_1st);

        let mut cross_corr_1st: Vec<Complex32> = sum_i_q_1st
            .iter()
            .zip(ca_code_fft_conj.iter())
            .map(|(x, y)| x * y)
            .collect();

        inv_fft.process(&mut cross_corr_1st);
        let result1: Vec<f32> = cross_corr_1st.iter().map(|x| x.norm()).collect();
        //d_max_2d_1st.push(cross_corr_1st.iter().map(|x| x.norm()).collect());

        let mut sum_i_q_2nd: Vec<Complex32> = (0..fft_length)
            .map(|x| {
                Complex32::new(
                    (2.0 * PI * carrier_freq * 1.0 / freq_sampling * x as f32).cos(),
                    (2.0 * PI * carrier_freq * 1.0 / freq_sampling * x as f32).sin(),
                ) * samples_iq_2nd[x]
            })
            .collect();
        c_fft.process(&mut sum_i_q_2nd);

        let mut cross_corr_2nd: Vec<Complex32> = sum_i_q_2nd
            .iter()
            .zip(ca_code_fft_conj.iter())
            .map(|(x, y)| x * y)
            .collect();

        inv_fft.process(&mut cross_corr_2nd);
        let result2: Vec<f32> = cross_corr_2nd.iter().map(|x| x.norm()).collect();
        //d_max_2d_2nd.push(cross_corr_2nd.iter().map(|x| x.norm()).collect());
        d_max_2d.push(
            if max_float_vec(result1.to_owned())?.0 >= max_float_vec(result2.to_owned())?.0 {
                result1
            } else {
                result2
            },
        );
    }
    if let Some((code_phase, doppler_freq_step, mag_relative)) =
        satellite_detection_two_peaks(d_max_2d, samples_per_chip, num_ca_code_samples, 1.4)
    //satellite_detection_ca_cfar(d_max_2d, test_threhold)
    {
        acq_result.code_phase = code_phase;
        acq_result.mag_relative = mag_relative;
    } else {
        return Err("Satellite acquisition failed!");
    }

    if let Some(()) = finer_doppler(
        &long_samples,
        is_complex,
        &mut acq_result,
        freq_sampling,
        freq_IF,
    ) {
    } else {
        return Err("Error in finding finer doppler frequency.");
    };

    buffer_location += acq_result.code_phase;
    Ok(buffer_location)
}

/// Find more accurate doppler frequency
///
/// # Arguments
///
/// * 'long_samples' - A long signal samples, e.g., 5ms or 10ms.
/// * 'acq_statistic' - Acquisition results.
/// * 'freq_IF' - intermiate frequency.
///
fn finer_doppler(
    long_samples: &Vec<i16>,
    is_complex: bool,
    acq_result: &mut AcquisitionResult,
    freq_sampling: f32,
    freq_IF: f32,
) -> Option<()> {
    long_samples.iter().find(|&x| !((*x as f32).is_nan()))?; // Check whether there is nan in the data

    let mut samples_iq: Vec<Complex32> = long_samples
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect();
    let mean = samples_iq.iter().sum::<Complex32>() / samples_iq.len() as f32;
    samples_iq = samples_iq.iter().map(|x| x - mean).collect();
    let num_ca_code_samples = (freq_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let size_signal_use = (LONG_SAMPLES_LENGTH - 1) as usize * num_ca_code_samples;
    let ca_code_samples_ind: Vec<usize> = (0..size_signal_use)
        .map(|x| {
            (x as f32 * gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / freq_sampling).floor()
                as usize
                % gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize
        })
        .collect();

    let fft_size: usize = 8 * size_signal_use.next_power_of_two();
    let one_side_fft_points = ((fft_size as f32 + 1.0) / 2.0).ceil() as usize;
    let fft_freq_bins: Vec<f32> = (0..one_side_fft_points)
        .map(|x| x as f32 * freq_sampling / fft_size as f32)
        .collect();
    let c_fft = Radix4::new(fft_size, FftDirection::Forward);
    let zero_padding: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); fft_size - size_signal_use];

    let code_phase = acq_result.code_phase;
    let mut carrier_sig: Vec<Complex32> = Vec::with_capacity(fft_size);
    let mut signal_use: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); size_signal_use];
    signal_use.copy_from_slice(&samples_iq[code_phase..size_signal_use + code_phase]);
    let long_ca_code_samples: Vec<i16> = ca_code_samples_ind
        .iter()
        .map(|&ind| acq_result.ca_code[ind])
        .collect();
    carrier_sig.extend(
        signal_use
            .iter()
            .zip(long_ca_code_samples.iter())
            .map(|(x, y)| x * Complex32::new(*y as f32, 0.0)),
    );
    carrier_sig.extend(&zero_padding);

    c_fft.process(&mut carrier_sig);
    let mag_carrier_sig: Vec<f32> = carrier_sig.iter().map(|x| x.abs()).collect();

    let mag_temp = mag_carrier_sig.clone().into_iter().reduce(f32::max)?;
    let (idx, _) = mag_carrier_sig
        .iter()
        .enumerate()
        .find(|(ind, val)| **val == mag_temp)?;

    if idx > one_side_fft_points {
        let fft_freq_bin_new: Vec<f32> = (2..=one_side_fft_points)
            .rev()
            .map(|x| -fft_freq_bins[x])
            .collect();
        let mag_temp = mag_carrier_sig[one_side_fft_points..]
            .iter()
            .copied()
            .reduce(f32::max)?;
        let (idx, _) = mag_carrier_sig[one_side_fft_points..]
            .iter()
            .enumerate()
            .find(|(ind, val)| **val == mag_temp)?;
        acq_result.carrier_freq = -fft_freq_bin_new[idx];
    } else {
        acq_result.carrier_freq =
            ((-1i8).pow(if is_complex { 1 } else { 0 })) as f32 * fft_freq_bins[idx];
    }

    Some(())
}

/// Check whether the satellite is visible with Cell-Averaging Constant False Alarm Rate (CA-CFAR) algorithm
///
fn satellite_detection_ca_cfar(
    corr_results: Vec<Vec<f32>>,
    threshold: f32,
) -> Option<(usize, usize, f32)> {
    let mut mag_max: f32 = 0.0;
    let mut code_phase: usize = 0;
    let mut power: f32 = 0.0;
    let mut doppler_freq_step: usize = 0;
    let mut test_statistic: f32 = 0.0;
    let num_samples = corr_results[0].len();
    for i in 0..corr_results.len() {
        corr_results[i].iter().find(|x| !(x.is_nan()))?; // Check whether there is nan in the data
        let mag_temp = corr_results[i].clone().into_iter().reduce(f32::max)?;
        let (idx, _) = corr_results[i]
            .iter()
            .enumerate()
            .find(|(ind, val)| **val == mag_temp)?;
        if mag_temp > mag_max {
            mag_max = mag_temp;
            let sum: f32 = corr_results[i].iter().sum();
            power = (sum - mag_temp) / (1.0 * num_samples as f32);
            if mag_max / power > test_statistic {
                test_statistic = mag_max / power;
                code_phase = idx;
                doppler_freq_step = i;
            }
        }
    }

    if test_statistic > threshold {
        Some((code_phase, doppler_freq_step, test_statistic))
    } else {
        None
    }
}

fn satellite_detection_two_peaks(
    corr_results: Vec<Vec<f32>>,
    samples_per_chip: usize,
    num_ca_code_samples: usize,
    threashold: f32,
) -> Option<(usize, usize, f32)> {
    let max_col: Vec<f32> = (0..corr_results.len())
        .map(|i| {
            max_float_vec(corr_results[i].to_owned())
                .expect("Error in max float column vector in satellite detection")
                .0
        })
        .collect();
    let freq_index = max_float_vec(max_col)
        .expect("Error in find frequency index in satellite detection")
        .1;

    let max_row: Vec<f32> = (0..corr_results[0].len())
        .map(|x| {
            max_float_vec(
                (0..corr_results.len())
                    .map(|y| corr_results[y][x])
                    .collect::<Vec<f32>>(),
            )
            .expect("Error in max float row vector in satellite detection")
            .0
        })
        .collect();
    let (first_peak, code_phase) = max_float_vec(max_row.to_owned())
        .expect("Error in find the first peak in satellite detection");

    let left_index = code_phase as i64 - samples_per_chip as i64;
    let right_index = code_phase + samples_per_chip;
    let mut new_corr_results: Vec<f32> = Vec::new();

    if left_index < 1 {
        let idx = (num_ca_code_samples as i64 + left_index) as usize;
        new_corr_results = corr_results[freq_index][right_index - 1..idx].to_vec();
    } else if right_index >= num_ca_code_samples {
        new_corr_results = corr_results[freq_index]
            [right_index - num_ca_code_samples - 1..left_index as usize]
            .to_vec();
    } else {
        new_corr_results = corr_results[freq_index][0..left_index as usize]
            .iter()
            .chain(corr_results[freq_index][right_index..num_ca_code_samples].iter())
            .copied()
            .collect();
    }

    let (second_peak, _) = max_float_vec(new_corr_results)
        .expect("Error in finding second peak in satellite detection");
    if first_peak / second_peak > threashold {
        Some((code_phase, freq_index, first_peak / second_peak))
    } else {
        None
    }
}

/// Generate CA code samples for 32 PRN code based on sampling frequency which might not be multiples of CA code rate
pub fn generate_ca_code_samples(prn: usize, f_sampling: f32) -> (Vec<i16>, Vec<i16>) {
    let num_samples = (f_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let samples_ind: Vec<usize> = (0..num_samples)
        .map(|x| {
            (x as f32 * gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / f_sampling).floor()
                as usize
        })
        .collect();

    let ca_code = generate_ca_code(prn);
    (
        samples_ind.iter().map(|&ind| ca_code[ind]).collect(),
        ca_code,
    )
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinReaderExt;
    use std::fs::File;
    use std::time::Instant;

    #[test]
    fn test_satellite_acquistion() {
        let t1 = Instant::now();
        let mut f = File::open("src/test_data/GPS_recordings/gioveAandB_short.bin")
            .expect("Error in opening file");
        let f_sampling: f32 = 16.3676e6;
        let f_inter_freq: f32 = 4.1304e6;
        let num_ca_code_samples = (f_sampling
            / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
                / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
            .round() as usize;
        let mut buffer: Vec<i8> = Vec::with_capacity(2 * num_ca_code_samples);
        while buffer.len() < 2 * num_ca_code_samples {
            buffer.push(f.read_be().expect("Error in reading data"));
            buffer.push(0);
        }

        let buffer_samples: Vec<i16> = buffer.iter().map(|&x| x as i16).collect();
        let mut acq_results: Vec<AcquistionStatistics> = Vec::new();

        do_acquisition(&buffer_samples, &mut acq_results, f_sampling, f_inter_freq)
            .expect("Error in Signal Acquisition");
        assert!(acq_results.len() > 4);
        let t2 = t1.elapsed().as_millis();
        println!("Elapsed time: {}ms", t2);
        dbg!(acq_results);
    }
}
*/
