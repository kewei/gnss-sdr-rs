use puruspe::invgammp;
use rayon::prelude::*;
use realfft::RealFftPlanner;
use rustfft::num_complex::{Complex, Complex32, ComplexFloat};
use rustfft::FftPlanner;
use std::error::Error;
use std::f32::consts::PI;
use std::fmt;
use std::process::id;

use crate::gps_ca_prn::generate_ca_code;
use crate::gps_constants;

static FFT_LENGTH_MS: usize = 1;
static FREQ_SEARCH_ACQUISITION_HZ: f32 = 14e3; // Hz
static FREQ_SEARCH_STEP_HZ: i32 = 500; // Hz
static PRN_SEARCH_ACQUISITION_TOTAL: i16 = 32; // 32 PRN codes to search

#[derive(Debug, Clone)]
struct AcqError;

impl fmt::Display for AcqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error happens while doing signal acquisition!")
    }
}

impl Error for AcqError {}

#[derive(Debug, Clone)]
pub struct AcquistionStatistics {
    pub prn: i16,
    pub code_phase: usize,
    pub doppler_freq: f32,
    pub mag_relative: f32,
    pub ca_code: Vec<i16>,
}

impl AcquistionStatistics {
    pub fn new() -> Self {
        Self {
            prn: 0,
            code_phase: 0,
            doppler_freq: 0.0,
            mag_relative: 0.0,
            ca_code: Vec::new(),
        }
    }
}

pub fn do_acquisition(
    samples: &Vec<i16>,
    acq_statistic: &mut Vec<AcquistionStatistics>,
    freq_sampling: f32,
    freq_IF: f32,
) -> Result<(), Box<dyn Error>> {
    let num_ca_code_samples = (freq_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let fft_length = num_ca_code_samples; // One CA code length

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
    // For this, the sampling frequency must be multiple of CA code rate, need to improved later todo!()
    let samples_per_chip =
        (freq_sampling / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S) as usize;

    let test_threhold = (2.0 * invgammp(0.8, 2.0)) as f32;
    let (ca_code_samples_all_prn, ca_code_all_prn) =
        generate_ca_code_samples(freq_sampling, num_ca_code_samples);

    for prn in 0..PRN_SEARCH_ACQUISITION_TOTAL {
        let mut ca_code_fft = r_fft.make_output_vec();

        let mut ca_code_input: Vec<f32> = ca_code_samples_all_prn[prn as usize]
            .iter()
            .map(|x| *x as f32)
            .collect();
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
        let mut freq: f32 = 0.0;
        for step in 0..steps {
            freq =
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
        if let Some((code_phase, doppler_freq_step, mag_relative)) =
            satellite_detection(d_max_2d, test_threhold)
        {
            let doppler_freq = freq - freq_IF;
            acq_statistic.push(AcquistionStatistics {
                prn: prn + 1,
                code_phase,
                doppler_freq,
                mag_relative,
                ca_code: ca_code_all_prn[prn as usize].clone(),
            });
        } else {
            println!("PRN {} is not present.", prn + 1);
        }
    }
    Ok(())
}

/// Find more accurate doppler frequency
///
/// # Arguments
///
/// * 'long_samples' - A long signal samples, e.g., 5ms or 10ms.
/// * 'acq_statistic' - Acquisition results.
/// * 'freq_IF' - intermiate frequency.
///
pub fn finer_doppler(
    long_samples: &Vec<i16>,
    length_signal_ms: usize,
    is_complex: bool,
    acq_statistic: &mut Vec<AcquistionStatistics>,
    freq_sampling: f32,
    freq_IF: f32,
) -> Option<()> {
    long_samples.iter().find(|&x| !((*x as f32).is_nan()))?; // Check whether there is nan in the data
    let samples_iq: Vec<Complex32> = long_samples
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect();
    let num_ca_code_samples = (freq_sampling
        / (gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S
            / gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS))
        .round() as usize;
    let size_signal_use = (length_signal_ms - 1) * num_ca_code_samples;
    let ca_code_samples_ind: Vec<usize> = (0..size_signal_use)
        .map(|x| {
            (x as f32 * gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / freq_sampling).floor()
                as usize
        })
        .collect();

    let fft_size = 8 * (size_signal_use.next_power_of_two().pow(2));
    let fft_points = ((fft_size as f32 + 1.0) / 2.0).ceil() as usize;
    let mut complex_planner = FftPlanner::new();
    let c_fft = complex_planner.plan_fft_forward(fft_size);

    for acq_result in acq_statistic {
        let ca_code = &acq_result.ca_code;
        let code_phase = acq_result.code_phase;
        let mut signal_use: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); size_signal_use];
        signal_use.copy_from_slice(&samples_iq[code_phase..size_signal_use + code_phase]);
        let long_ca_code_samples: Vec<i16> = ca_code_samples_ind
            .iter()
            .map(|&ind| ca_code[ind % gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize])
            .collect();
        let mut carrier_sig: Vec<Complex32> = signal_use
            .iter()
            .zip(long_ca_code_samples.iter())
            .map(|(x, y)| x * (*y as f32))
            .collect();
        c_fft.process(&mut carrier_sig);
        let mag_carrier_sig: Vec<f32> = carrier_sig.iter().map(|x| x.abs()).collect();

        let mag_temp = mag_carrier_sig.clone().into_iter().reduce(f32::max)?;
        let (idx, _) = mag_carrier_sig
            .iter()
            .enumerate()
            .find(|(ind, val)| **val == mag_temp)?;
        let fft_freq_bins: Vec<f32> = (0..fft_points)
            .map(|x| x as f32 * freq_sampling / fft_points as f32)
            .collect();
        if idx > fft_points {
            let fft_freq_bin_new: Vec<f32> =
                (2..=fft_points).rev().map(|x| -fft_freq_bins[x]).collect();
            let mag_temp = mag_carrier_sig[fft_points..]
                .to_vec()
                .into_iter()
                .reduce(f32::max)?;
            let (idx, _) = mag_carrier_sig[fft_points..]
                .iter()
                .enumerate()
                .find(|(ind, val)| **val == mag_temp)?;
            acq_result.doppler_freq = -fft_freq_bin_new[idx];
        } else {
            acq_result.doppler_freq =
                ((-1i8).pow((if is_complex { 1 } else { 0 }))) as f32 * fft_freq_bins[idx];
        }
        dbg!(acq_result.doppler_freq);
    }
    Some(())
}

/// Check whether the satellite is visible with Cell-Averaging Constant False Alarm Rate (CA-CFAR) algorithm
///
fn satellite_detection(corr_results: Vec<Vec<f32>>, threshold: f32) -> Option<(usize, usize, f32)> {
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

/// Generate CA code samples for 32 PRN code based on sampling frequency which might not be multiples of CA code rate
pub fn generate_ca_code_samples(
    f_sampling: f32,
    num_ca_code_samples: usize,
) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
    let t_sampling: f32 = 1.0 / f_sampling;
    let t_chip: f32 = 1.0 / gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S;
    let samples_ind: Vec<usize> = (0..num_ca_code_samples)
        .map(|x| {
            (x as f32 * gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / f_sampling).floor()
                as usize
        })
        .collect();
    let mut ca_code_samples_all_prn: Vec<Vec<i16>> = Vec::new();
    let mut ca_code_all_prn: Vec<Vec<i16>> = Vec::new();
    let inner_index = 0;
    let mut ca_code: Vec<i16> = vec![0; gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize];
    for i in 0..32 {
        ca_code = generate_ca_code(i + 1);
        ca_code_samples_all_prn.push(samples_ind.iter().map(|&ind| ca_code[ind]).collect());
        ca_code_all_prn.push(ca_code);
    }
    (ca_code_samples_all_prn, ca_code_all_prn)
}

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
