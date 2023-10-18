use itertools::{izip, Itertools};
use rustfft::num_complex::Complex32;
use std::collections::HashMap;
use std::error::Error;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};

use crate::acquisition::AcquisitionResult;
use crate::app_buffer_utilities::{get_current_buffer, APPBUFF, BUFFER_SIZE};
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

#[derive(Clone, Debug)]
pub struct TrackingResult {
    pub prn: usize,
    pub i_prompt: f32,
    pub q_prompt: f32,
    pub i_early: f32,
    pub q_early: f32,
    pub i_late: f32,
    pub q_late: f32,
    pub code_error: f32,
    pub code_error_filtered: f32,
    pub code_phase_error: f32,
    pub code_freq: f32,
    pub carrier_error: f32,
    pub carrier_error_filtered: f32,
    pub carrier_phase_error: f32,
    pub carrier_freq: f32,
}

impl TrackingResult {
    pub fn new(prn: usize) -> Self {
        Self {
            prn,
            i_prompt: 0.0,
            q_prompt: 0.0,
            i_early: 0.0,
            q_early: 0.0,
            i_late: 0.0,
            q_late: 0.0,
            code_error: 0.0,
            code_error_filtered: 0.0,
            code_phase_error: 0.0,
            code_freq: gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S,
            carrier_error: 0.0,
            carrier_error_filtered: 0.0,
            carrier_phase_error: 0.0,
            carrier_freq: 0.0,
        }
    }
}

pub fn do_track(
    acquisition_result: Arc<Mutex<AcquisitionResult>>,
    tracking_result: Arc<Mutex<TrackingResult>>,
    f_sampling: f32,
    f_IF: f32,
    buffer_location: usize,
) -> Result<usize, Box<dyn Error>> {
    let mut acq_result = acquisition_result
        .lock()
        .expect("Error in locking in tracking");
    let mut trk_result = tracking_result
        .lock()
        .expect("Error in locking in tracking");
    let prn = acq_result.prn;
    assert_eq!(prn, trk_result.prn);

    let code_freq: f32 = trk_result.code_freq;
    let code_phase_error: f32 = trk_result.code_phase_error;
    let code_nco: f32 = trk_result.code_error_filtered;
    let code_error: f32 = trk_result.code_error;

    let carrier_freq = if trk_result.carrier_freq == 0.0 {
        acq_result.carrier_freq
    } else {
        trk_result.carrier_freq
    };
    let carrier_phase_error: f32 = trk_result.carrier_phase_error;
    let carrier_nco: f32 = trk_result.carrier_error_filtered;
    let carrier_error: f32 = trk_result.carrier_error;

    let code_phase_step: f32 = code_freq / f_sampling;
    let num_ca_code_samples = ((gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS - code_phase_error)
        / code_phase_step)
        .ceil() as usize;

    let signal_input = get_current_buffer(buffer_location, 2 * num_ca_code_samples);
    let samples_iq: Vec<Complex32> = signal_input
        .chunks_exact(2)
        .map(|chunk| {
            if let [i, q] = chunk {
                Complex32::new(*i as f32, *q as f32)
            } else {
                panic!("Problem with converting input samples to complex values.");
            }
        })
        .collect();

    let mut ca_code = acq_result.ca_code.clone();
    ca_code.insert(
        0,
        ca_code[gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize - 1],
    );
    ca_code.push(ca_code[1]);

    let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
        .map(|x| ca_code[(x as f32 * code_phase_step + code_phase_error).ceil() as usize] as f32)
        .collect();

    let (carrier_error, carrier_nco, carrier_phase_error, q_arm, i_arm) = costas_loop(
        &samples_iq,
        ca_code_prompt,
        num_ca_code_samples,
        carrier_freq,
        carrier_error,
        carrier_phase_error,
        carrier_nco,
        f_sampling,
    );

    // Update carrier frequency
    let carrier_freq = acq_result.carrier_freq + carrier_nco;

    let (
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
        code_phase_step,
        code_phase_error,
        code_error,
        code_nco,
        ca_code,
        num_ca_code_samples,
        f_sampling,
    );
    trk_result.i_prompt = i_prompt;
    trk_result.q_prompt = q_prompt;
    trk_result.i_early = i_early;
    trk_result.q_early = q_early;
    trk_result.i_late = i_late;
    trk_result.q_late = q_late;
    trk_result.code_error = d_code_error;
    trk_result.code_error_filtered = code_nco;
    trk_result.code_phase_error = code_phase_error;
    trk_result.carrier_error = carrier_error;
    trk_result.carrier_error_filtered = carrier_nco;
    trk_result.carrier_phase_error = carrier_phase_error;
    trk_result.carrier_freq = carrier_freq;
    trk_result.code_freq = code_freq;

    let buffer_loc = buffer_location + num_ca_code_samples;
    println!("buffer location: {}", buffer_loc);

    Ok(buffer_loc)
}

fn costas_loop(
    signal_samples: &[Complex32],
    prompt_code_samples: Vec<f32>,
    num_ca_code_samples: usize,
    freq: f32,
    carrier_error: f32,
    carrier_phase_error: f32,
    carrier_nco: f32,
    f_sampling: f32,
) -> (f32, f32, f32, Vec<f32>, Vec<f32>) {
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

    (
        d_carrier_error,
        carrier_nco,
        carrier_phase_error,
        q_arm,
        i_arm,
    )
}

fn dll_early_late(
    q_arm: Vec<f32>,
    i_arm: Vec<f32>,
    code_phase_step: f32,
    code_phase_error: f32,
    old_code_error: f32,
    code_nco: f32,
    ca_code: Vec<i16>,
    num_ca_code_samples: usize,
    f_sampling: f32,
) -> (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) {
    let (ca_code_early, ca_code_late, ca_code_prompt): (Vec<f32>, Vec<f32>, Vec<f32>) =
        izip!(0..num_ca_code_samples)
            .map(|x| {
                (
                    ca_code[(x as f32 * code_phase_step + code_phase_error - EARLY_LATE_SPACE)
                        .ceil() as usize] as f32,
                    ca_code[(x as f32 * code_phase_step + code_phase_error + EARLY_LATE_SPACE)
                        .ceil() as usize] as f32,
                    ca_code[(x as f32 * code_phase_step + code_phase_error).ceil() as usize] as f32,
                )
            })
            .multiunzip();
    /* let ca_code_early: Vec<f32> = (0..num_ca_code_samples)
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
        .collect(); */
    let code_phase_error = num_ca_code_samples as f32 * code_phase_step + code_phase_error
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
    let code_freq: f32 = gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S - code_nco;

    (
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
