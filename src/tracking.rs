use itertools::izip;
use rustfft::num_complex::Complex32;
use std::collections::HashMap;
use std::error::Error;
use std::f32::consts::PI;

use crate::acquisition::AcquistionStatistics;
use crate::gps_ca_prn::generate_ca_code;
use crate::gps_constants;

static DLL_DUMPING_RATIO: f32 = 0.7;
static PLL_DUMPING_RATIO: f32 = 0.7;
static DLL_NOISE_BANDWIDTH: f32 = 2.0;
static PLL_NOISE_BANDWIDTH: f32 = 25.0;
// Summation interval
static PLL_SUM_CARR: f32 = 0.001;
static DLL_SUM_CODE: f32 = 0.001;
static PLL_GAIN: f32 = 0.25;
static DLL_GAIN: f32 = 1.0;
static EARLY_LATE_SPACE: f32 = 0.5;

#[derive(Clone)]
pub struct TrackingResults {
    i_prompt: Vec<f32>,
    q_prompt: Vec<f32>,
    i_early: Vec<f32>,
    q_early: Vec<f32>,
    i_late: Vec<f32>,
    q_late: Vec<f32>,
    pub code_error: Vec<f32>,
    pub code_error_filtered: Vec<f32>,
    pub code_phase_error: Vec<f32>,
    pub code_freq: Vec<f32>,
    pub carrier_error: Vec<f32>,
    pub carrier_error_filtered: Vec<f32>,
    pub carrier_phase_error: Vec<f32>,
    pub carrier_freq: Vec<f32>,
}

impl TrackingResults {
    fn new() -> Self {
        Self {
            i_prompt: Vec::new(),
            q_prompt: Vec::new(),
            i_early: Vec::new(),
            q_early: Vec::new(),
            i_late: Vec::new(),
            q_late: Vec::new(),
            code_error: Vec::new(),
            code_error_filtered: Vec::new(),
            code_phase_error: Vec::new(),
            code_freq: Vec::new(),
            carrier_error: Vec::new(),
            carrier_error_filtered: Vec::new(),
            carrier_phase_error: Vec::new(),
            carrier_freq: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct TrackingStatistics {
    pub tracking_stat: TrackingResults,
    pub ca_code_prompt: Vec<Vec<f32>>,
}

impl TrackingStatistics {
    pub fn new() -> Self {
        Self {
            tracking_stat: TrackingResults::new(),
            ca_code_prompt: Vec::new(),
        }
    }
}

pub fn do_track(
    signal_input: &Vec<i16>,
    acq_results: &Vec<AcquistionStatistics>,
    tracking_statistic: &mut HashMap<i16, TrackingStatistics>,
    f_sampling: f32,
    f_IF: f32,
) -> Result<(), Box<dyn Error>> {
    let samples_iq: Vec<Complex32> = signal_input
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect(); // Duplicated

    for acq_result in acq_results {
        let mut tracking_result = &mut TrackingStatistics::new();
        let prn = acq_result.prn;

        if let Some(result) = tracking_statistic.get_mut(&prn) {
            tracking_result = result;
        };

        let code_freq: f32 = *tracking_result
            .tracking_stat
            .code_freq
            .last()
            .expect("Error occurs while reading the last code_freq");
        let code_phase_error: f32 = *tracking_result
            .tracking_stat
            .code_phase_error
            .last()
            .expect("Error occurs while reading the last code_phase_error");
        let code_nco: f32 = *tracking_result
            .tracking_stat
            .code_error_filtered
            .last()
            .expect("Error occurs while reading the last code_error_filtered");
        let code_error: f32 = *tracking_result
            .tracking_stat
            .code_error
            .last()
            .expect("Error occurs while reading the last code_error");
        let carrier_freq: f32 = *tracking_result
            .tracking_stat
            .carrier_freq
            .last()
            .expect("Error occurs while reading the last carrier_freq");
        let carrier_phase_error: f32 = *tracking_result
            .tracking_stat
            .carrier_phase_error
            .last()
            .expect("Error occurs while reading the last carrier_phase_error");

        let carrier_nco: f32 = *tracking_result
            .tracking_stat
            .carrier_error_filtered
            .last()
            .expect("Error occurs while reading the last carrier_error_filtered");
        let carrier_error: f32 = *tracking_result
            .tracking_stat
            .carrier_error
            .last()
            .expect("Error occurs while reading the last carrier_error");

        let mut ca_code = acq_result.ca_code.clone();
        ca_code.insert(
            0,
            ca_code[gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize - 1],
        );
        ca_code.push(ca_code[0]);

        let ca_code_prompt: Vec<f32> = tracking_result
            .ca_code_prompt
            .last()
            .expect("Error occurs while reading the last ca_code_prompt")
            .to_vec();

        let (
            carrier_freq,
            q_prompt,
            i_prompt,
            carrier_error,
            carrier_nco,
            carrier_phase_error,
            q_arm,
            i_arm,
        ) = costas_loop(
            &samples_iq,
            ca_code_prompt,
            carrier_freq,
            carrier_error,
            carrier_phase_error,
            carrier_nco,
            code_freq,
            code_phase_error,
            f_sampling,
        );

        let (
            ca_code_prompt,
            code_freq,
            d_code_error,
            code_nco,
            code_phase_error,
            i_early,
            q_early,
            i_late,
            q_late,
            i_prompt,
            q_prompt,
        ) = dll_early_late(
            q_arm,
            i_arm,
            code_freq,
            code_phase_error,
            code_error,
            code_nco,
            ca_code,
            f_sampling,
        );
        tracking_result.tracking_stat.i_prompt.push(i_prompt);
        tracking_result.tracking_stat.q_prompt.push(q_prompt);
        tracking_result.tracking_stat.i_early.push(i_early);
        tracking_result.tracking_stat.q_early.push(q_early);
        tracking_result.tracking_stat.i_late.push(i_late);
        tracking_result.tracking_stat.q_late.push(q_late);
        tracking_result.tracking_stat.code_error.push(d_code_error);
        tracking_result
            .tracking_stat
            .code_error_filtered
            .push(code_nco);
        tracking_result
            .tracking_stat
            .code_phase_error
            .push(code_phase_error);
        tracking_result
            .tracking_stat
            .carrier_error
            .push(carrier_error);
        tracking_result
            .tracking_stat
            .carrier_error_filtered
            .push(carrier_nco);
        tracking_result
            .tracking_stat
            .carrier_phase_error
            .push(carrier_phase_error);
        tracking_result
            .tracking_stat
            .carrier_freq
            .push(carrier_freq);
        tracking_result.tracking_stat.code_freq.push(code_freq);
        tracking_result.ca_code_prompt.push(ca_code_prompt);
    }
    Ok(())
}

fn costas_loop(
    signal_samples: &Vec<Complex32>,
    prompt_code_samples: Vec<f32>,
    freq: f32,
    carrier_error: f32,
    carrier_phase_error: f32,
    carrier_nco: f32,
    code_freq: f32,
    code_phase_error: f32,
    f_sampling: f32,
) -> (f32, f32, f32, f32, f32, f32, Vec<f32>, Vec<f32>) {
    let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase_error)
        / (code_freq / f_sampling))
        .ceil() as usize;
    let local_carrier: Vec<Complex32> = (0..num_ca_code_samples)
        .map(|x| {
            Complex32::new(
                (2.0 * PI * freq / f_sampling * x as f32 + carrier_phase_error).cos(),
                (2.0 * PI * freq / f_sampling * x as f32 + carrier_phase_error).sin(),
            )
        })
        .collect();
    let carrier_phase_error = (2.0 * PI * freq / f_sampling * num_ca_code_samples as f32
        + carrier_phase_error)
        % (2.0 * PI);

    let (q_arm, i_arm): (Vec<f32>, Vec<f32>) = local_carrier
        .iter()
        .zip(signal_samples.iter())
        .map(|(x, y)| ((x * y).re, (x * y).im))
        .unzip();
    let (q_prompt, i_prompt): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, prompt_code_samples)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_prompt: f32 = q_prompt.iter().sum();
    let i_prompt: f32 = i_prompt.iter().sum();
    let d_carrier_error = (q_prompt / i_prompt).atan() / (2.0 * PI);

    let (tau1_carr, tau2_carr) =
        calculate_loop_efficient(PLL_NOISE_BANDWIDTH, PLL_DUMPING_RATIO, PLL_GAIN);

    let carrier_nco = carrier_nco
        + (tau2_carr / tau1_carr) * (d_carrier_error - carrier_error)
        + d_carrier_error * (PLL_SUM_CARR / tau1_carr);

    // Update carrier frequency
    let carrier_freq = freq + carrier_nco;

    (
        carrier_freq,
        q_prompt,
        i_prompt,
        carrier_error,
        carrier_nco,
        carrier_phase_error,
        q_arm,
        i_arm,
    )
}

fn dll_early_late(
    q_arm: Vec<f32>,
    i_arm: Vec<f32>,
    code_freq: f32,
    code_phase_error: f32,
    old_code_error: f32,
    code_nco: f32,
    ca_code: Vec<i32>,
    f_sampling: f32,
) -> (Vec<f32>, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) {
    let code_phase_step: f32 = code_freq / f_sampling;
    let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase_error)
        / code_phase_step)
        .ceil() as usize;
    let ca_code_early: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| {
            ca_code
                [(x as f32 * code_phase_step + code_phase_error - EARLY_LATE_SPACE).ceil() as usize]
                as f32
        })
        .collect();
    let ca_code_late: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| {
            ca_code
                [(x as f32 * code_phase_step + code_phase_error + EARLY_LATE_SPACE).ceil() as usize]
                as f32
        })
        .collect();
    let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| ca_code[(x as f32 * code_phase_step + code_phase_error).ceil() as usize] as f32)
        .collect();
    let code_phase_error =
        (num_ca_code_samples - 1) as f32 * code_phase_step + code_phase_error + code_phase_step
            - gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS;

    let (q_early, i_early): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, ca_code_early)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_early: f32 = q_early.iter().sum();
    let i_early: f32 = i_early.iter().sum();
    let (q_late, i_late): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, ca_code_late)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_late: f32 = q_late.iter().sum();
    let i_late: f32 = i_late.iter().sum();
    let (q_prompt, i_prompt): (Vec<f32>, Vec<f32>) = izip!(&q_arm, &i_arm, &ca_code_prompt)
        .map(|(x, y, z)| (x * z, y * z))
        .unzip();
    let q_prompt: f32 = q_prompt.iter().sum();
    let i_prompt: f32 = i_prompt.iter().sum();

    let d_code_error: f32 = ((i_early.powf(2.0) + q_early.powf(2.0)).sqrt()
        - (i_late.powf(2.0) + q_late.powf(2.0)).sqrt())
        / ((i_early.powf(2.0) + q_early.powf(2.0)).sqrt()
            + (i_late.powf(2.0) + q_late.powf(2.0)).sqrt());
    let (tau1_code, tau2_code) =
        calculate_loop_efficient(DLL_NOISE_BANDWIDTH, DLL_DUMPING_RATIO, DLL_GAIN);
    let code_nco = code_nco
        + (tau2_code / tau1_code) * (d_code_error - old_code_error)
        + d_code_error * (DLL_SUM_CODE / tau1_code);
    let code_freq: f32 = code_freq - code_nco;
    (
        ca_code_prompt,
        code_freq,
        d_code_error,
        code_nco,
        code_phase_error,
        i_early,
        q_early,
        i_late,
        q_late,
        i_prompt,
        q_prompt,
    )
}

fn calculate_loop_efficient(noise_bw: f32, dumping_ratio: f32, gain: f32) -> (f32, f32) {
    let w: f32 = noise_bw * 8.0 * dumping_ratio / (4.0 * dumping_ratio.powf(2.0) + 1.0);
    let tau1: f32 = gain / (w * w);
    let tau2: f32 = 2.0 * dumping_ratio / w;
    (tau1, tau2)
}
